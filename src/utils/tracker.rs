use std::io;
use std::io::Read;
use tokio::sync::mpsc;
use tracing::info;

// Constant for expected image size in bytes (16GB)
const EXPECTED_IMAGE_SIZE: u64 = 16 * 1024 * 1024 * 1024;
const MB: f64 = 1.0 / 1024.0 / 1024.0;

struct ProgressTracker<R: Read> {
    inner: R,
    sipper: mpsc::UnboundedSender<u64>,
    bytes_read: u64,
    bytes_written: u64,
    total_size: u64,
}

impl<R: Read> ProgressTracker<R> {
    // Update the written bytes counter
    pub fn update_written(&mut self, additional_bytes: u64) {
        self.bytes_written += additional_bytes;
        
        // Calculate total bytes processed
        let total_processed = self.bytes_read + self.bytes_written;
        
        // Log progress
        info!(
            "Total: {} MB / {} MB (Read: {} MB, Written: {} MB)",
            total_processed as f64 * MB,
            EXPECTED_IMAGE_SIZE as f64 * MB,
            self.bytes_read as f64 * MB,
            self.bytes_written as f64 * MB
        );
        
        // Send total processed amount
        self.sipper.send(total_processed).ok();
    }
}

impl<R: Read> Read for ProgressTracker<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.inner.read(buf)?;
        if bytes > 0 {
            self.bytes_read += bytes as u64;
            
            // Calculate total bytes processed
            let total_processed = self.bytes_read + self.bytes_written;
            
            // Log progress
            info!(
                "Total: {} MB / {} MB (Read: {} MB, Written: {} MB)",
                total_processed as f64 * MB,
                EXPECTED_IMAGE_SIZE as f64 * MB,
                self.bytes_read as f64 * MB,
                self.bytes_written as f64 * MB
            );
            
            // Send total processed amount
            self.sipper.send(total_processed).ok();
        }
        Ok(bytes)
    }
}

pub fn track_progress<R: Read>(inner: R, _size: u64) -> (impl Read, mpsc::UnboundedReceiver<u64>) {
    let (tx, rx) = mpsc::unbounded_channel();

    (
        ProgressTracker {
            inner,
            sipper: tx,
            bytes_read: 0,
            bytes_written: 0,
            total_size: EXPECTED_IMAGE_SIZE,
        },
        rx,
    )
}
