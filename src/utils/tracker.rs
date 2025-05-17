use crate::utils::WriteProgress;
use futures_util::Sink;
use std::io;
use std::io::Read;
use tokio::sync::mpsc;
use tracing::info;

struct ProgressTracker<R: Read> {
    inner: R,
    sipper: mpsc::UnboundedSender<u64>,
    bytes_read: u64,
    total_size: u64,
}

const MB : f64 = 1f64 / 1024f64 / 1024f64;

impl<R: Read> Read for ProgressTracker<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.inner.read(buf)?;
        if bytes > 0 {
            self.bytes_read += bytes as u64;
            info!("Read {} MB / {}", self.bytes_read as f64 * MB, self.total_size as f64 * MB);
            self.sipper.send(self.bytes_read*1000 / self.total_size).ok();
        }
        Ok(bytes)
    }
}

pub fn track_progress<R: Read>(inner: R, size: u64) -> (impl Read, mpsc::UnboundedReceiver<u64>) {
    let (tx, rx) = mpsc::unbounded_channel();

    (
        ProgressTracker {
            inner,
            sipper: tx,
            bytes_read: 0,
            total_size: size,
        },
        rx,
    )
}
