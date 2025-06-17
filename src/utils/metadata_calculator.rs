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
                let error_msg = if e.is_cancelled() {
                    "Operation cancelled by user".to_string()
                } else {
                    format!("Task panicked: {}", e)
                };
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
            info!("Metadata calculation cancelled by user at main loop check");
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

        // Send progress updates periodically (every 10MB of uncompressed data for more responsive cancellation)
        if total_uncompressed % (10 * 1024 * 1024) == 0 || bytes_read < BUFFER_SIZE {
            // Double-check for cancellation before sending progress updates
            if cancel_token.is_cancelled() {
                info!("Metadata calculation cancelled by user during progress update");
                return Err(anyhow!("Operation cancelled by user"));
            }

            debug!(
                "Processed {} MB of uncompressed data (estimated progress: {:.1}%) - cancel_token.is_cancelled(): {}",
                total_uncompressed / (1024 * 1024),
                estimated_progress * 100.0,
                cancel_token.is_cancelled()
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

    // Store metadata for future use
    let metadata_manager = crate::utils::image_metadata::MetadataManager::new()
        .map_err(|e| anyhow!("Failed to create metadata manager: {}", e))?;
    
    if let Err(e) = metadata_manager.store_metadata(compressed_hash, &metadata) {
        tracing::warn!("Failed to store metadata: {}", e);
    }

    Ok(metadata)
}


