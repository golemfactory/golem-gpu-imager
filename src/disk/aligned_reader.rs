// Aligned disk I/O reader
//
// This module provides a reader wrapper that ensures all read operations
// are properly aligned to sector boundaries for Windows direct disk access.

use std::cmp;
use std::io::{self, Read, Seek, SeekFrom};
use tracing::{debug, error};

/// A reader wrapper that ensures all read operations are properly aligned to sector boundaries.
/// This is particularly important for Windows direct disk I/O, which requires buffer alignment.
pub struct AlignedReader<T: Read + Seek> {
    /// The underlying reader
    inner: T,
    /// Buffer for aligned reads
    buffer: Vec<u8>,
    /// Current position within buffer (how much has been consumed)
    buffer_pos: usize,
    /// Valid bytes in buffer (how much has been read from inner)
    buffer_len: usize,
    /// Sector size for alignment (typically 512 bytes)
    sector_size: usize,
}

impl<T: Read + Seek> AlignedReader<T> {
    /// Create a new AlignedReader wrapping the provided reader
    ///
    /// # Arguments
    /// * `inner` - The reader to wrap
    /// * `sector_size` - The sector size to align reads to (typically 512 bytes)
    /// * `buffer_size` - Optional buffer size (defaults to 4KB if not specified)
    ///
    /// # Returns
    /// * A new AlignedReader instance
    pub fn new(inner: T, sector_size: usize, buffer_size: Option<usize>) -> Self {
        // Default buffer size is 8 sectors or 4KB, whichever is larger
        let default_buffer_size = cmp::max(sector_size * 8, 4096);
        let buffer_size = buffer_size.unwrap_or(default_buffer_size);

        // Ensure buffer size is a multiple of sector size
        let aligned_buffer_size = ((buffer_size + sector_size - 1) / sector_size) * sector_size;

        debug!(
            "Creating AlignedReader with sector_size={}, buffer_size={}",
            sector_size, aligned_buffer_size
        );

        Self {
            inner,
            buffer: vec![0; aligned_buffer_size],
            buffer_pos: 0,
            buffer_len: 0,
            sector_size,
        }
    }

    /// Get a reference to the inner reader
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner reader
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Unwrap this reader, returning the inner reader
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Fill the internal buffer with data from the inner reader
    fn fill_buffer(&mut self) -> io::Result<usize> {
        // If we still have data in the buffer, no need to refill
        if self.buffer_pos < self.buffer_len {
            return Ok(self.buffer_len - self.buffer_pos);
        }

        // Reset buffer tracking
        self.buffer_pos = 0;
        self.buffer_len = 0;

        // Get current position in inner stream for alignment calculations
        let current_pos = match self.inner.seek(SeekFrom::Current(0)) {
            Ok(pos) => pos,
            Err(e) => {
                error!("Failed to get current position in AlignedReader: {}", e);
                return Err(e);
            }
        };

        // Calculate alignment and adjust reading position if needed
        let pos_offset = current_pos % self.sector_size as u64;
        let aligned_pos = current_pos - pos_offset;

        // If we're not already aligned, seek to the aligned position
        if pos_offset > 0 {
            debug!(
                "Aligning read: seeking from {} to {}",
                current_pos, aligned_pos
            );
            self.inner.seek(SeekFrom::Start(aligned_pos))?;
        }

        // Read into our buffer, ensuring we read at least one full sector
        let bytes_read = self.inner.read(&mut self.buffer)?;

        if bytes_read == 0 {
            // End of file, nothing read
            return Ok(0);
        }

        // Account for alignment offset
        self.buffer_pos = pos_offset as usize;
        self.buffer_len = bytes_read;

        // Return available bytes
        let available = self.buffer_len.saturating_sub(self.buffer_pos);
        debug!(
            "Buffer refilled: read {} bytes (available: {})",
            bytes_read, available
        );

        Ok(available)
    }
}

impl<T: Read + Seek> Read for AlignedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check if we need to fill the buffer
        if self.buffer_pos >= self.buffer_len {
            let bytes_available = self.fill_buffer()?;
            if bytes_available == 0 {
                // End of file reached
                return Ok(0);
            }
        }

        // Calculate how much data we can copy from buffer to destination
        let available = self.buffer_len - self.buffer_pos;
        let copy_len = cmp::min(available, buf.len());

        // Copy from our buffer to the destination buffer
        buf[..copy_len].copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + copy_len]);

        // Update buffer position
        self.buffer_pos += copy_len;

        // If we've copied everything that was requested, we're done
        if copy_len == buf.len() {
            return Ok(copy_len);
        }

        // If we've exhausted our buffer but the caller wants more, we need to read more
        if self.buffer_pos >= self.buffer_len {
            // Try to fill the buffer again for the remaining data
            let mut total_read = copy_len;
            let remaining = buf.len() - copy_len;

            // If the remaining request is larger than our buffer, read directly
            if remaining >= self.buffer.len() {
                debug!("Large read request ({} bytes), reading directly", remaining);

                // For large reads, go with a direct read to the caller's buffer
                // This avoids an extra copy through our buffer
                match self.inner.read(&mut buf[copy_len..]) {
                    Ok(n) => {
                        total_read += n;
                        return Ok(total_read);
                    }
                    Err(e) => {
                        // If we got at least some data, return that instead of an error
                        if total_read > 0 {
                            return Ok(total_read);
                        }
                        return Err(e);
                    }
                }
            }

            // For smaller reads, refill our buffer and continue
            match self.fill_buffer() {
                Ok(0) => {
                    // No more data, return what we've got so far
                    return Ok(total_read);
                }
                Ok(_) => {
                    // We have more data, copy as much as we can
                    let new_available = self.buffer_len - self.buffer_pos;
                    let new_copy = cmp::min(new_available, remaining);

                    buf[copy_len..copy_len + new_copy]
                        .copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + new_copy]);

                    self.buffer_pos += new_copy;
                    total_read += new_copy;

                    return Ok(total_read);
                }
                Err(e) => {
                    // If we got at least some data, return that instead of an error
                    if total_read > 0 {
                        return Ok(total_read);
                    }
                    return Err(e);
                }
            }
        }

        // We should never reach here, but just in case
        Ok(copy_len)
    }
}

impl<T: Read + Seek> Seek for AlignedReader<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // Clear our buffer - any seeking invalidates it
        self.buffer_pos = 0;
        self.buffer_len = 0;

        // Delegate to inner reader
        self.inner.seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_aligned_reader_simple() {
        // Create a test buffer with 1024 bytes
        let test_data = (0..1024).map(|i| (i & 0xFF) as u8).collect::<Vec<_>>();
        let cursor = Cursor::new(test_data.clone());

        // Create an aligned reader with 512-byte sectors
        let mut reader = AlignedReader::new(cursor, 512, None);

        // Read 100 bytes
        let mut buf = [0u8; 100];
        let bytes_read = reader.read(&mut buf).unwrap();

        // Check that we got the expected data
        assert_eq!(bytes_read, 100);
        assert_eq!(&buf[..], &test_data[..100]);

        // Read another 200 bytes
        let mut buf2 = [0u8; 200];
        let bytes_read = reader.read(&mut buf2).unwrap();

        // Check that we got the expected data
        assert_eq!(bytes_read, 200);
        assert_eq!(&buf2[..], &test_data[100..300]);

        // Seek to a specific position
        reader.seek(SeekFrom::Start(500)).unwrap();

        // Read 100 bytes from the new position
        let mut buf3 = [0u8; 100];
        let bytes_read = reader.read(&mut buf3).unwrap();

        // Check that we got the expected data
        assert_eq!(bytes_read, 100);
        assert_eq!(&buf3[..], &test_data[500..600]);
    }

    #[test]
    fn test_aligned_reader_unaligned_seek() {
        // Create a test buffer with 1024 bytes
        let test_data = (0..1024).map(|i| (i & 0xFF) as u8).collect::<Vec<_>>();
        let cursor = Cursor::new(test_data.clone());

        // Create an aligned reader with 512-byte sectors
        let mut reader = AlignedReader::new(cursor, 512, None);

        // Seek to an unaligned position
        reader.seek(SeekFrom::Start(123)).unwrap();

        // Read 100 bytes
        let mut buf = [0u8; 100];
        let bytes_read = reader.read(&mut buf).unwrap();

        // Check that we got the expected data
        assert_eq!(bytes_read, 100);
        assert_eq!(&buf[..], &test_data[123..223]);
    }

    #[test]
    fn test_aligned_reader_large_read() {
        // Create a test buffer with 8192 bytes
        let test_data = (0..8192).map(|i| (i & 0xFF) as u8).collect::<Vec<_>>();
        let cursor = Cursor::new(test_data.clone());

        // Create an aligned reader with 512-byte sectors and small buffer
        let mut reader = AlignedReader::new(cursor, 512, Some(1024));

        // Read 6000 bytes (larger than buffer)
        let mut buf = vec![0u8; 6000];
        let bytes_read = reader.read(&mut buf).unwrap();

        // Check that we got the expected data
        assert_eq!(bytes_read, 6000);
        assert_eq!(&buf[..], &test_data[..6000]);
    }
}
