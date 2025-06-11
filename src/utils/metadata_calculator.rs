use crate::models::{CancelToken, ImageMetadata};
use anyhow::{Result, anyhow};
use iced::task::{self, Sipper};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::Path;
use tracing::{debug, error, info};
use xz4rust::XzReader;

#[derive(Debug, Clone)]
pub enum MetadataProgress {
    Start,
    Processing {
        bytes_processed: u64,
        estimated_total: u64,
        progress: f32, // 0.0 to 1.0
    },
    Completed {
        metadata: ImageMetadata,
    },
    Failed {
        error: String,
    },
}

/// Calculate metadata for a compressed XZ image file
///
/// This function streams through the compressed file, decompresses it on-the-fly,
/// and calculates the SHA256 hash and uncompressed size without storing the
/// decompressed data to disk.
pub fn calculate_image_metadata(
    image_path: &Path,
    compressed_hash: String,
    cancel_token: CancelToken,
) -> impl Sipper<Result<MetadataProgress>, MetadataProgress> + Send + 'static {
    let image_path = image_path.to_path_buf();

    task::sipper(async move |mut sipper| -> Result<MetadataProgress> {
        let image_path_str = image_path.to_string_lossy().to_string();
        info!("Starting metadata calculation for: {}", image_path_str);

        // Send initial progress
        sipper.send(MetadataProgress::Start).await;

        // Create a channel for progress updates from the blocking task
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();

        // Clone sipper for progress forwarding
        let mut progress_sipper = sipper.clone();

        // Spawn task to forward progress updates
        let progress_forwarder = tokio::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                let _ = progress_sipper.send(progress).await;
            }
        });

        // Use blocking task for I/O operations to avoid blocking the async runtime
        let result = tokio::task::spawn_blocking(move || {
            calculate_metadata_blocking(&image_path, &compressed_hash, cancel_token, progress_tx)
        })
        .await;

        // Close the progress channel and wait for forwarder to finish
        progress_forwarder.abort();

        match result {
            Ok(Ok(metadata)) => {
                let progress = MetadataProgress::Completed { metadata };
                sipper.send(progress.clone()).await;
                Ok(progress)
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                let progress = MetadataProgress::Failed { error: error_msg };
                sipper.send(progress.clone()).await;
                Err(e)
            }
            Err(e) => {
                let error_msg = format!("Task panicked: {}", e);
                let progress = MetadataProgress::Failed {
                    error: error_msg.clone(),
                };
                sipper.send(progress.clone()).await;
                Err(anyhow!(error_msg))
            }
        }
    })
}

/// Blocking implementation of metadata calculation
fn calculate_metadata_blocking(
    image_path: &Path,
    compressed_hash: &str,
    cancel_token: CancelToken,
    progress_tx: tokio::sync::mpsc::UnboundedSender<MetadataProgress>,
) -> Result<ImageMetadata> {
    // Buffer size matching the write_image implementation for consistency
    const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer

    debug!("Opening compressed image file: {:?}", image_path);

    // Open the compressed file
    let compressed_file = File::open(image_path)?;
    let compressed_size = compressed_file.metadata()?.len();

    info!(
        "Compressed file size: {} bytes ({:.2} MB)",
        compressed_size,
        compressed_size as f64 / (1024.0 * 1024.0)
    );

    // Create XZ reader with large buffer for optimal performance
    let buffered_file = std::io::BufReader::with_capacity(BUFFER_SIZE, compressed_file);
    let buffer_size = NonZeroUsize::new(BUFFER_SIZE).unwrap();
    let mut xz_reader = XzReader::new_with_buffer_size(buffered_file, buffer_size);

    // Initialize hash calculation
    let mut hasher = Sha256::new();
    let mut total_uncompressed = 0u64;
    let mut buffer = vec![0u8; BUFFER_SIZE];

    info!("Starting streaming decompression and hash calculation");

    // Estimate uncompressed size (typically 3-5x for OS images)
    let estimated_uncompressed = compressed_size * 4; // Conservative estimate

    loop {
        // Check for cancellation before each read
        if cancel_token.is_cancelled() {
            info!("Metadata calculation cancelled by user");
            return Err(anyhow!("Operation cancelled by user"));
        }

        // Read decompressed data
        let bytes_read = match xz_reader.read(&mut buffer) {
            Ok(0) => {
                info!("Reached end of compressed stream");
                break; // EOF
            }
            Ok(n) => n,
            Err(e) => {
                error!("Error reading from XZ stream: {}", e);
                return Err(anyhow!("Failed to read from XZ stream: {}", e));
            }
        };

        // Update hash with the decompressed data
        hasher.update(&buffer[..bytes_read]);
        total_uncompressed += bytes_read as u64;

        // Calculate progress based on compressed file position vs total compressed size
        // This is an approximation since we can't easily get exact position in XZ stream
        let estimated_progress =
            (total_uncompressed as f64 / estimated_uncompressed as f64).min(0.95) as f32;

        // Send progress updates periodically (every 100MB of uncompressed data)
        if total_uncompressed % (100 * 1024 * 1024) == 0 || bytes_read < BUFFER_SIZE {
            debug!(
                "Processed {} MB of uncompressed data (estimated progress: {:.1}%)",
                total_uncompressed / (1024 * 1024),
                estimated_progress * 100.0
            );

            // Send progress update to UI
            let progress_update = MetadataProgress::Processing {
                bytes_processed: total_uncompressed,
                estimated_total: estimated_uncompressed,
                progress: estimated_progress,
            };

            // Ignore send errors (channel might be closed if cancelled)
            let _ = progress_tx.send(progress_update);
        }
    }

    // Finalize hash calculation
    let uncompressed_hash = hasher.finalize();
    let uncompressed_hash_hex = hex::encode(uncompressed_hash);

    info!(
        "Completed metadata calculation - Uncompressed size: {} bytes ({:.2} GB), Hash: {}",
        total_uncompressed,
        total_uncompressed as f64 / (1024.0 * 1024.0 * 1024.0),
        &uncompressed_hash_hex[..16] // Show first 16 chars for logging
    );

    // Create metadata structure
    let metadata = ImageMetadata {
        compressed_hash: compressed_hash.to_string(),
        uncompressed_hash: uncompressed_hash_hex,
        uncompressed_size: total_uncompressed,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    Ok(metadata)
}

/// Calculate progress estimation based on compressed file position
/// This is a simplified approach since XzReader doesn't expose exact stream position
fn estimate_progress_from_compressed_position(
    compressed_bytes_read: u64,
    total_compressed_size: u64,
) -> f32 {
    if total_compressed_size == 0 {
        return 0.0;
    }

    let raw_progress = compressed_bytes_read as f32 / total_compressed_size as f32;

    // Cap at 95% since decompression might finish before all compressed data is read
    // due to padding or other factors in XZ format
    raw_progress.min(0.95)
}

/// Validate that a calculated hash matches expected value
pub fn validate_metadata_hash(metadata: &ImageMetadata, expected_compressed_hash: &str) -> bool {
    metadata.compressed_hash.to_lowercase() == expected_compressed_hash.to_lowercase()
}

/// Format uncompressed size for display
pub fn format_uncompressed_size(size_bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;

    if size_bytes >= GB {
        format!("{:.2} GB", size_bytes as f64 / GB as f64)
    } else if size_bytes >= MB {
        format!("{:.1} MB", size_bytes as f64 / MB as f64)
    } else {
        format!("{} KB", size_bytes / 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_format_uncompressed_size() {
        assert_eq!(format_uncompressed_size(1024), "1 KB");
        assert_eq!(format_uncompressed_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_uncompressed_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_validate_metadata_hash() {
        let metadata = ImageMetadata {
            compressed_hash: "abc123".to_string(),
            uncompressed_hash: "def456".to_string(),
            uncompressed_size: 1024,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        assert!(validate_metadata_hash(&metadata, "abc123"));
        assert!(validate_metadata_hash(&metadata, "ABC123")); // Case insensitive
        assert!(!validate_metadata_hash(&metadata, "different"));
    }

    #[test]
    fn test_estimate_progress() {
        assert_eq!(estimate_progress_from_compressed_position(0, 100), 0.0);
        assert_eq!(estimate_progress_from_compressed_position(50, 100), 0.5);
        assert_eq!(estimate_progress_from_compressed_position(100, 100), 0.95); // Capped
        assert_eq!(estimate_progress_from_compressed_position(10, 0), 0.0); // Division by zero
    }
}
