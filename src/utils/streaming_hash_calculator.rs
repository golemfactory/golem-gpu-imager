use crate::models::{CancelToken, ImageMetadata};
use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::num::NonZeroUsize;
use tracing::{debug, error, info};
use xz4rust::XzReader;

#[derive(Debug, Clone)]
pub enum ProcessingPhase {
    Download,
    Metadata,
    Complete,
}

#[derive(Debug, Clone)]
pub struct ProcessingProgress {
    pub phase: ProcessingPhase,
    pub download_progress: f32, // 0.0-1.0 for download phase
    pub metadata_progress: f32, // 0.0-1.0 for metadata phase
    pub overall_progress: f32,  // Combined progress for UI (0.0-1.0)
    pub bytes_downloaded: u64,
    pub total_download_bytes: u64,
    pub uncompressed_size: Option<u64>,
    pub compressed_hash: Option<String>,
    pub uncompressed_hash: Option<String>,
}

impl ProcessingProgress {
    pub fn new_download(bytes_downloaded: u64, total_bytes: u64) -> Self {
        let download_progress = if total_bytes > 0 {
            (bytes_downloaded as f32 / total_bytes as f32).min(1.0)
        } else {
            0.0
        };

        Self {
            phase: ProcessingPhase::Download,
            download_progress,
            metadata_progress: 0.0,
            overall_progress: download_progress * 0.5, // Download is first half
            bytes_downloaded,
            total_download_bytes: total_bytes,
            uncompressed_size: None,
            compressed_hash: None,
            uncompressed_hash: None,
        }
    }

    pub fn new_metadata(
        metadata_progress: f32,
        uncompressed_size: Option<u64>,
        compressed_hash: Option<String>,
        uncompressed_hash: Option<String>,
    ) -> Self {
        Self {
            phase: ProcessingPhase::Metadata,
            download_progress: 1.0,
            metadata_progress,
            overall_progress: 0.5 + (metadata_progress * 0.5), // Metadata is second half
            bytes_downloaded: 0,
            total_download_bytes: 0,
            uncompressed_size,
            compressed_hash,
            uncompressed_hash,
        }
    }

    pub fn completed(metadata: ImageMetadata) -> Self {
        Self {
            phase: ProcessingPhase::Complete,
            download_progress: 1.0,
            metadata_progress: 1.0,
            overall_progress: 1.0,
            bytes_downloaded: 0,
            total_download_bytes: 0,
            uncompressed_size: Some(metadata.uncompressed_size),
            compressed_hash: Some(metadata.compressed_hash),
            uncompressed_hash: Some(metadata.uncompressed_hash),
        }
    }
}

/// Streaming hash calculator that processes download chunks and calculates
/// both compressed and uncompressed hashes + size during download
pub struct StreamingHashCalculator {
    compressed_hasher: Sha256,
    cancel_token: CancelToken,
}

impl StreamingHashCalculator {
    pub fn new(cancel_token: CancelToken) -> Self {
        Self {
            compressed_hasher: Sha256::new(),
            cancel_token,
        }
    }

    /// Process a downloaded chunk and update compressed hash
    pub fn process_download_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        if self.cancel_token.is_cancelled() {
            return Err(anyhow!("Operation cancelled by user"));
        }

        self.compressed_hasher.update(chunk);
        Ok(())
    }

    /// Finalize compressed hash and start metadata calculation
    pub fn finalize_compressed_hash(&self) -> String {
        let hash = self.compressed_hasher.clone().finalize();
        hex::encode(hash)
    }

    /// Calculate metadata from the downloaded file
    /// This processes the compressed file to extract uncompressed hash and size
    pub async fn calculate_metadata(
        &self,
        file_path: &std::path::Path,
        compressed_hash: String,
        progress_sender: tokio::sync::mpsc::UnboundedSender<ProcessingProgress>,
    ) -> Result<ImageMetadata> {
        let file_path = file_path.to_path_buf();
        let cancel_token = self.cancel_token.clone();

        // Use blocking task for I/O operations
        tokio::task::spawn_blocking(move || {
            calculate_metadata_blocking(&file_path, &compressed_hash, cancel_token, progress_sender)
        })
        .await?
    }
}

/// Blocking implementation of metadata calculation with progress reporting
fn calculate_metadata_blocking(
    file_path: &std::path::Path,
    compressed_hash: &str,
    cancel_token: CancelToken,
    progress_sender: tokio::sync::mpsc::UnboundedSender<ProcessingProgress>,
) -> Result<ImageMetadata> {
    const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer

    debug!("Starting metadata calculation for: {:?}", file_path);

    // Open the compressed file
    let compressed_file = std::fs::File::open(file_path)?;
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
    let estimated_uncompressed = compressed_size * 4;

    // Send initial metadata progress
    let _ = progress_sender.send(ProcessingProgress::new_metadata(
        0.0,
        None,
        Some(compressed_hash.to_string()),
        None,
    ));

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

        // Calculate progress based on estimated uncompressed size
        let estimated_progress =
            (total_uncompressed as f64 / estimated_uncompressed as f64).min(0.95) as f32;

        // Send progress updates periodically (every 100MB of uncompressed data)
        if total_uncompressed % (100 * 1024 * 1024) == 0 || bytes_read < BUFFER_SIZE {
            debug!(
                "Processed {} MB of uncompressed data (estimated progress: {:.1}%)",
                total_uncompressed / (1024 * 1024),
                estimated_progress * 100.0
            );

            let _ = progress_sender.send(ProcessingProgress::new_metadata(
                estimated_progress,
                Some(total_uncompressed),
                Some(compressed_hash.to_string()),
                None,
            ));
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

    // Send final progress
    let _ = progress_sender.send(ProcessingProgress::completed(metadata.clone()));

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_progress_download() {
        let progress = ProcessingProgress::new_download(500, 1000);
        assert_eq!(progress.download_progress, 0.5);
        assert_eq!(progress.overall_progress, 0.25); // 50% of first half
        assert!(matches!(progress.phase, ProcessingPhase::Download));
    }

    #[test]
    fn test_processing_progress_metadata() {
        let progress = ProcessingProgress::new_metadata(0.6, Some(1024), None, None);
        assert_eq!(progress.metadata_progress, 0.6);
        assert_eq!(progress.overall_progress, 0.8); // 50% + 60% of second half
        assert!(matches!(progress.phase, ProcessingPhase::Metadata));
    }

    #[test]
    fn test_streaming_hash_calculator() {
        let cancel_token = CancelToken::new();
        let mut calculator = StreamingHashCalculator::new(cancel_token);

        // Test processing chunks
        let chunk1 = b"hello";
        let chunk2 = b"world";

        calculator.process_download_chunk(chunk1).unwrap();
        calculator.process_download_chunk(chunk2).unwrap();

        let hash = calculator.finalize_compressed_hash();

        // Verify hash is calculated correctly
        let mut expected_hasher = Sha256::new();
        expected_hasher.update(chunk1);
        expected_hasher.update(chunk2);
        let expected_hash = hex::encode(expected_hasher.finalize());

        assert_eq!(hash, expected_hash);
    }
}
