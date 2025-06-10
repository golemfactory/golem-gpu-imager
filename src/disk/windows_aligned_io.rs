// Windows-aligned I/O implementation
//
// This module provides buffered I/O operations that ensure alignment
// to sector boundaries for Windows direct disk access.

use std::cmp;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
// Allow unused imports as these may be used in the future
use std::alloc::{self, Layout};
use std::fmt::Debug;
#[allow(unused_imports)]
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr::NonNull;
use tracing::{debug, error, info, warn};

/// An I/O wrapper that ensures all operations are properly aligned to disk sector boundaries.
pub struct AlignedDiskIO {
    /// The underlying file handle
    file: File,
    /// Current absolute position in the file
    position: u64,
    /// Disk sector size for alignment
    sector_size: u32,
    /// Write buffer for accumulating writes until they can be made in aligned blocks
    buffer: AlignedBuffer,
    /// Buffer position relative to the current file position
    buffer_pos: usize,
}

impl std::fmt::Debug for AlignedDiskIO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlignedDiskIO")
            .field("position", &self.position)
            .field("sector_size", &self.sector_size)
            .field("buffer_pos", &self.buffer_pos)
            .finish()
    }
}

/// A buffer with memory alignment guarantees for direct I/O
#[derive(Debug)]
struct AlignedBuffer {
    /// Aligned memory pointer
    #[allow(dead_code)]
    ptr: NonNull<u8>,
    /// Capacity of the buffer in bytes
    capacity: usize,
    /// Current number of valid bytes in the buffer
    len: usize,
    /// Alignment requirement in bytes
    alignment: usize,
}

impl AlignedBuffer {
    /// Create a new aligned buffer with the specified capacity and alignment
    fn new(capacity: usize, alignment: usize) -> io::Result<Self> {
        // Ensure alignment is a power of 2 and at least 4096 (Windows requirement)
        let safe_alignment = if !alignment.is_power_of_two() || alignment < 4096 {
            let new_alignment = if alignment < 4096 {
                4096
            } else {
                alignment.next_power_of_two()
            };
            warn!(
                "Adjusting buffer alignment from {} to {} bytes (power of 2 requirement for Windows)",
                alignment, new_alignment
            );
            new_alignment
        } else {
            alignment
        };

        // Round up capacity to a multiple of alignment
        let aligned_capacity = ((capacity + safe_alignment - 1) / safe_alignment) * safe_alignment;

        info!(
            "Creating aligned buffer: requested={}, aligned={}, alignment={}",
            capacity, aligned_capacity, safe_alignment
        );

        // Create an aligned layout
        let layout = match Layout::from_size_align(aligned_capacity, safe_alignment) {
            Ok(layout) => layout,
            Err(e) => {
                error!(
                    "Layout error: {:?} (capacity={}, alignment={})",
                    e, aligned_capacity, safe_alignment
                );
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Invalid layout: capacity={}, alignment={}, error={:?}",
                        aligned_capacity, safe_alignment, e
                    ),
                ));
            }
        };

        // Allocate aligned memory
        let ptr = unsafe {
            let ptr = alloc::alloc(layout);
            if ptr.is_null() {
                error!(
                    "Failed to allocate aligned memory of size {} bytes",
                    aligned_capacity
                );
                return Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    format!(
                        "Failed to allocate {} bytes aligned to {} bytes",
                        aligned_capacity, safe_alignment
                    ),
                ));
            }
            NonNull::new_unchecked(ptr)
        };

        // Verify alignment
        let addr = ptr.as_ptr() as usize;
        if addr % safe_alignment != 0 {
            error!(
                "ALIGNMENT ERROR: Address 0x{:X} is not aligned to {} bytes",
                addr, safe_alignment
            );
            unsafe { alloc::dealloc(ptr.as_ptr(), layout) };
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Allocated memory at address 0x{:X} is not aligned to {} bytes",
                    addr, safe_alignment
                ),
            ));
        }

        // Zero the memory for safety
        unsafe {
            std::ptr::write_bytes(ptr.as_ptr(), 0, aligned_capacity);
        }

        info!(
            "Successfully created aligned buffer: capacity={}, alignment={}, address=0x{:X}",
            aligned_capacity, safe_alignment, addr
        );

        Ok(Self {
            ptr,
            capacity: aligned_capacity,
            len: 0,
            alignment: safe_alignment,
        })
    }

    /// Get a slice of the buffer contents
    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Get a mutable slice for the buffer
    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
    }

    /// Copy data from a source buffer into this aligned buffer at the specified offset
    fn copy_from_slice(&mut self, src: &[u8], offset: usize) -> usize {
        let available = self.capacity - offset;
        let copy_size = cmp::min(available, src.len());

        if copy_size == 0 {
            return 0;
        }

        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.ptr.as_ptr().add(offset), copy_size);
        }

        // Update length if we wrote past the current end
        let new_end = offset + copy_size;
        if new_end > self.len {
            self.len = new_end;
        }

        copy_size
    }

    /// Clear the buffer
    fn clear(&mut self) {
        self.len = 0;
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align(self.capacity, self.alignment)
                .expect("Invalid layout in AlignedBuffer::drop");
            alloc::dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

impl AlignedDiskIO {
    /// Create a new AlignedDiskIO wrapping a file
    pub fn new(mut file: File, sector_size: u32) -> io::Result<Self> {
        // Always use at least 4KB sector size for safe direct I/O on most systems
        // This matches the buffer alignment approach from disk-image-writer
        let safe_sector_size = std::cmp::max(sector_size, 4096);
        info!(
            "AlignedDiskIO: Using sector size {} bytes for alignment (original: {} bytes)",
            safe_sector_size, sector_size
        );

        // Use 4MB buffer size aligned to sector size, matching disk-image-writer's buffer size
        let buffer_size = 4 * 1024 * 1024; // 4 MB buffer like in disk-image-writer
        let buffer = AlignedBuffer::new(buffer_size, safe_sector_size as usize)?;

        let position = match file.seek(SeekFrom::Current(0)) {
            Ok(pos) => pos,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("Failed to get current file position: {}", e),
                ));
            }
        };

        Ok(Self {
            file,
            position,
            sector_size: safe_sector_size,
            buffer,
            buffer_pos: 0,
        })
    }

    /// Flush internal buffer to disk, ensuring alignment
    pub fn flush(&mut self) -> io::Result<()> {
        // If there's nothing in the buffer, we're done
        if self.buffer_pos == 0 {
            return Ok(());
        }

        // For proper alignment, we need to:
        // 1. Round down current position to sector boundary
        // 2. Read any partial sector at the beginning into our buffer
        // 3. Pad the end with zeros to sector boundary
        // 4. Write the entire aligned range

        // Calculate aligned start position
        let sector_size = self.sector_size as u64;
        let start_offset = self.position % sector_size;
        let aligned_pos = self.position - start_offset;

        // Calculate aligned buffer size
        let data_size = self.buffer_pos as u64;
        let end_padding = (sector_size - ((start_offset + data_size) % sector_size)) % sector_size;
        let aligned_size = start_offset + data_size + end_padding;

        debug!(
            "Flush aligned I/O: position={}, aligned_pos={}, sector_size={}, buffer_pos={}, aligned_size={}",
            self.position, aligned_pos, sector_size, self.buffer_pos, aligned_size
        );

        // If we need to handle partial start sector, read it first
        if start_offset > 0 {
            self.file.seek(SeekFrom::Start(aligned_pos))?;

            // Create a temporary buffer for the start sector
            let mut start_sector = vec![0u8; sector_size as usize];
            let read_bytes = self.file.read(&mut start_sector)?;

            if read_bytes < start_offset as usize {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "Failed to read start sector: read {} of {} bytes",
                        read_bytes, start_offset
                    ),
                ));
            }

            // Copy this data to the beginning of our buffer
            unsafe {
                std::ptr::copy_nonoverlapping(
                    start_sector.as_ptr(),
                    self.buffer.as_mut_slice().as_mut_ptr(),
                    start_offset as usize,
                );
            }
        }

        // Seek to the aligned position
        if let Err(e) = self.file.seek(SeekFrom::Start(aligned_pos)) {
            error!(
                "AlignedDiskIO: Failed to seek to aligned position {}: {}",
                aligned_pos, e
            );
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "AlignedDiskIO: Failed to seek to aligned position {}: {}",
                    aligned_pos, e
                ),
            ));
        }

        // Write the aligned buffer - ensure it's a multiple of the sector size
        let to_write = ((aligned_size + sector_size - 1) / sector_size * sector_size) as usize;

        // Double-check the buffer's alignment
        let ptr_addr = self.buffer.as_slice().as_ptr() as usize;
        let alignment_ok = ptr_addr % sector_size as usize == 0;
        let size_ok = to_write % sector_size as usize == 0;

        // First check: Do we have enough data in the buffer to write?
        if self.buffer_pos < to_write {
            debug!(
                "Buffer doesn't have enough data for aligned write: buffer_pos={}, to_write={}",
                self.buffer_pos, to_write
            );

            // Pad the buffer with zeros to reach the aligned size
            let current_buffer_size = self.buffer_pos;
            let padding_needed = to_write - current_buffer_size;

            debug!(
                "Padding buffer with {} bytes of zeros to reach aligned size {}",
                padding_needed, to_write
            );

            // Safety check to avoid buffer overflows
            if current_buffer_size + padding_needed <= self.buffer.capacity {
                // Zero out the remaining bytes
                let buffer_slice = self.buffer.as_mut_slice();
                for i in current_buffer_size..current_buffer_size + padding_needed {
                    buffer_slice[i] = 0;
                }
                // Update buffer length to include padding
                self.buffer.len = current_buffer_size + padding_needed;
            } else {
                error!(
                    "Cannot pad buffer: current_size={}, padding_needed={}, capacity={}",
                    current_buffer_size, padding_needed, self.buffer.capacity
                );
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Buffer too small for aligned write: need {} bytes, have {} capacity",
                        current_buffer_size + padding_needed,
                        self.buffer.capacity
                    ),
                ));
            }
        }

        // If the buffer is not aligned, we need to create a new aligned buffer
        if !alignment_ok || !size_ok {
            error!(
                "AlignedDiskIO: Buffer not properly aligned for direct I/O! Address: 0x{:X}, Size: {}, Alignment: {} (Address aligned: {}, Size aligned: {})",
                ptr_addr, to_write, sector_size, alignment_ok, size_ok
            );

            // Create a properly aligned buffer as a fallback
            info!("Creating a new properly aligned buffer as fallback");
            // Use a larger buffer size for better performance
            let fallback_size = std::cmp::max(to_write, 4 * 1024 * 1024); // At least 4MB
            let mut aligned_buf = match AlignedBuffer::new(fallback_size, self.sector_size as usize)
            {
                Ok(buffer) => buffer,
                Err(e) => {
                    return Err(io::Error::new(
                        e.kind(),
                        format!("Failed to create aligned fallback buffer: {}", e),
                    ));
                }
            };

            // Copy the data to the aligned buffer - make sure we don't exceed source buffer length
            let available_data = self.buffer.len;
            let copy_size = std::cmp::min(to_write, available_data);

            debug!(
                "Copying {} bytes from buffer to aligned fallback buffer (buffer.len={}, to_write={})",
                copy_size, available_data, to_write
            );

            aligned_buf.copy_from_slice(&self.buffer.as_slice()[0..copy_size], 0);

            // If we couldn't copy enough data, pad with zeros
            if copy_size < to_write {
                debug!(
                    "Padding fallback buffer with {} zeros",
                    to_write - copy_size
                );
                let padding_slice = aligned_buf.as_mut_slice();
                for i in copy_size..to_write {
                    padding_slice[i] = 0;
                }
                aligned_buf.len = to_write;
            }

            // Verify the new buffer's alignment
            let new_ptr = aligned_buf.as_slice().as_ptr() as usize;
            let new_aligned = new_ptr % sector_size as usize == 0;
            let new_size_ok = aligned_buf.as_slice().len() % sector_size as usize == 0;

            if !new_aligned || !new_size_ok {
                error!(
                    "CRITICAL: Fallback aligned buffer is still not aligned! Address: 0x{:X}, Size: {}",
                    new_ptr,
                    aligned_buf.as_slice().len()
                );
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Even fallback buffer not aligned for direct I/O: Address: 0x{:X}, Size: {}, Sector size: {}",
                        new_ptr,
                        aligned_buf.as_slice().len(),
                        sector_size
                    ),
                ));
            }

            info!(
                "AlignedDiskIO: Using fallback buffer at 0x{:X} with size {}",
                new_ptr,
                aligned_buf.as_slice().len()
            );

            // Make sure we're only writing up to valid data length
            let write_length = std::cmp::min(to_write, aligned_buf.len);
            let write_slice = &aligned_buf.as_slice()[0..write_length];

            // Attempt the write with multiple retries on failure (similar to disk-image-writer)
            let mut written = 0;
            for attempt in 0..5 {
                match self.file.write(write_slice) {
                    Ok(bytes) => {
                        written = bytes;
                        break;
                    }
                    Err(e) => {
                        if attempt < 4 {
                            error!(
                                "AlignedDiskIO: Write failed at position {} on attempt {}/5: {}",
                                aligned_pos,
                                attempt + 1,
                                e
                            );
                            error!("Retrying write operation in 100ms...");
                            // Wait 100ms before retrying
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        } else {
                            // Last attempt failed
                            error!(
                                "AlignedDiskIO: Write failed at position {} after 5 attempts: {}",
                                aligned_pos, e
                            );
                            error!(
                                "AlignedDiskIO: Buffer details: Address: 0x{:X}, Size: {}, Alignment: {}",
                                new_ptr,
                                write_slice.len(),
                                sector_size
                            );
                            return Err(e);
                        }
                    }
                }
            }

            if written < write_slice.len() {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!(
                        "Short write with fallback buffer: {} of {} bytes",
                        written,
                        write_slice.len()
                    ),
                ));
            }

            // Update position and clear buffer
            self.position += data_size;
            self.buffer_pos = 0;

            // Ensure data is flushed to disk
            match self.file.flush() {
                Ok(_) => info!("Successfully flushed file to disk"),
                Err(e) => warn!("Failed to flush file to disk: {}", e),
            }

            return Ok(());
        }

        // Ensure we only try to write up to the amount of data we actually have
        let actual_write_size = std::cmp::min(to_write, self.buffer.len);

        info!(
            "AlignedDiskIO: Writing {} bytes at position {}, aligned properly (address: 0x{:X})",
            actual_write_size, aligned_pos, ptr_addr
        );

        // Create a slice of the correct size for writing
        let write_slice = &self.buffer.as_slice()[0..actual_write_size];

        // Attempt the write with multiple retries on failure (similar to disk-image-writer)
        let mut written = 0;
        for attempt in 0..5 {
            match self.file.write(write_slice) {
                Ok(bytes) => {
                    written = bytes;
                    break;
                }
                Err(e) => {
                    if attempt < 4 {
                        error!(
                            "AlignedDiskIO: Write failed at position {} on attempt {}/5: {}",
                            aligned_pos,
                            attempt + 1,
                            e
                        );
                        error!("Retrying write operation in 100ms...");
                        // Wait 100ms before retrying
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    } else {
                        // Last attempt failed
                        error!(
                            "AlignedDiskIO: Write failed at position {} after 5 attempts: {}",
                            aligned_pos, e
                        );
                        error!(
                            "AlignedDiskIO: Buffer details: Address: 0x{:X}, Size: {}, Alignment: {}",
                            ptr_addr,
                            write_slice.len(),
                            sector_size
                        );
                        return Err(e);
                    }
                }
            }
        }

        if written < write_slice.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("Short write: {} of {} bytes", written, write_slice.len()),
            ));
        }

        // Update position and clear buffer
        self.position += data_size;
        self.buffer_pos = 0;

        // Flush the underlying file to ensure data is written to disk
        // Retry flushing a few times if it fails
        for attempt in 0..3 {
            match self.file.flush() {
                Ok(_) => {
                    info!(
                        "Successfully flushed file to disk on attempt {}",
                        attempt + 1
                    );
                    return Ok(());
                }
                Err(e) => {
                    if attempt < 2 {
                        warn!(
                            "Failed to flush file to disk on attempt {}/3: {}",
                            attempt + 1,
                            e
                        );
                        warn!("Retrying flush operation in 100ms...");
                        // Wait 100ms before retrying
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    } else {
                        // Last attempt failed
                        error!("Failed to flush file to disk after 3 attempts: {}", e);
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get a reference to the underlying file
    pub fn get_ref(&self) -> &File {
        &self.file
    }

    /// Get a mutable reference to the underlying file
    pub fn get_mut(&mut self) -> &mut File {
        &mut self.file
    }

    /// Get the current position in the file
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Unwrap this wrapper and return the inner file
    pub fn into_inner(mut self) -> io::Result<File> {
        self.flush()?;
        Ok(self.file)
    }
}

impl Read for AlignedDiskIO {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // For reads, we first flush any pending writes
        self.flush()?;

        // Calculate aligned read parameters
        let sector_size = self.sector_size as u64;
        let start_offset = self.position % sector_size;
        let aligned_pos = self.position - start_offset;

        // Set up a properly aligned buffer for reading
        let request_size = buf.len();
        let padded_size = start_offset as usize + request_size;
        let aligned_size = ((padded_size + self.sector_size as usize - 1)
            / self.sector_size as usize)
            * self.sector_size as usize;

        // Clear and reuse our internal buffer if it's large enough
        if self.buffer.capacity >= aligned_size {
            self.buffer.clear();
        } else {
            // Create a larger buffer if needed
            let new_size = cmp::max(aligned_size, self.buffer.capacity * 2);
            self.buffer = match AlignedBuffer::new(new_size, self.sector_size as usize) {
                Ok(buffer) => buffer,
                Err(e) => {
                    return Err(io::Error::new(
                        e.kind(),
                        format!("Failed to create aligned read buffer: {}", e),
                    ));
                }
            };
        }

        // Seek to the aligned position
        self.file.seek(SeekFrom::Start(aligned_pos))?;

        // Read into our aligned buffer
        let read_buf = &mut self.buffer.as_mut_slice()[0..aligned_size];
        let bytes_read = self.file.read(read_buf)?;

        if bytes_read == 0 {
            return Ok(0); // EOF
        }

        // Set buffer length to actual bytes read
        self.buffer.len = bytes_read;

        // Copy from our aligned buffer to the caller's buffer
        // Only if we read enough data to satisfy the offset
        if bytes_read <= start_offset as usize {
            // Not enough data was read to satisfy the offset
            debug!(
                "Not enough data read to satisfy offset: read {} bytes, offset is {}",
                bytes_read, start_offset
            );
            return Ok(0);
        }

        let available = bytes_read - start_offset as usize;
        let copy_size = cmp::min(available, buf.len());

        if copy_size > 0 {
            // Ensure we don't go out of bounds
            if start_offset as usize + copy_size <= self.buffer.len {
                buf[0..copy_size].copy_from_slice(
                    &self.buffer.as_slice()
                        [(start_offset as usize)..(start_offset as usize + copy_size)],
                );
            } else {
                // Safer approach - copy only what's available
                let safe_copy_size = self.buffer.len.saturating_sub(start_offset as usize);
                if safe_copy_size > 0 {
                    buf[0..safe_copy_size].copy_from_slice(
                        &self.buffer.as_slice()
                            [(start_offset as usize)..(start_offset as usize + safe_copy_size)],
                    );
                    debug!(
                        "Adjusted copy size from {} to {} bytes due to buffer bounds",
                        copy_size, safe_copy_size
                    );
                    // Update copy_size to what we actually copied
                    return Ok(safe_copy_size);
                } else {
                    // No data available after offset
                    return Ok(0);
                }
            }
        }

        // Update position
        self.position += copy_size as u64;

        Ok(copy_size)
    }
}

impl Write for AlignedDiskIO {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Debug buffer alignment for direct I/O
        let ptr_addr = buf.as_ptr() as usize;
        let alignment_ok = ptr_addr % self.sector_size as usize == 0;
        let size_ok = buf.len() % self.sector_size as usize == 0;

        debug!(
            "Write request: Buffer address: 0x{:X}, size: {}, sector size: {}",
            ptr_addr,
            buf.len(),
            self.sector_size
        );

        if !alignment_ok || !size_ok {
            debug!(
                "Buffer not aligned for direct I/O (address aligned: {}, size aligned: {})",
                alignment_ok, size_ok
            );
        }

        // If the buffer would overflow, flush it first
        if self.buffer_pos + buf.len() > self.buffer.capacity {
            self.flush()?;
        }

        // If the data is larger than our buffer capacity, write it directly
        if buf.len() >= self.buffer.capacity {
            // For large writes, we need to ensure alignment
            let sector_size = self.sector_size as u64;
            let start_offset = self.position % sector_size;

            // If we're not aligned, buffer the write
            if start_offset != 0 || (buf.len() % self.sector_size as usize) != 0 {
                // Buffer in chunks
                let mut bytes_written = 0;

                debug!(
                    "Using buffered write for unaligned data (position offset: {}, buffer size: {})",
                    start_offset,
                    buf.len()
                );

                while bytes_written < buf.len() {
                    let chunk_size = cmp::min(
                        self.buffer.capacity - self.buffer_pos,
                        buf.len() - bytes_written,
                    );

                    let written = self.buffer.copy_from_slice(
                        &buf[bytes_written..(bytes_written + chunk_size)],
                        self.buffer_pos,
                    );

                    self.buffer_pos += written;
                    bytes_written += written;

                    if self.buffer_pos == self.buffer.capacity {
                        self.flush()?;
                    }
                }

                return Ok(bytes_written);
            } else {
                // Direct aligned write
                self.flush()?; // Ensure any buffered data is written first

                // Create an aligned copy if needed
                let aligned_size = ((buf.len() + self.sector_size as usize - 1)
                    / self.sector_size as usize)
                    * self.sector_size as usize;

                let write_result = if buf.len() == aligned_size
                    && (buf.as_ptr() as usize) % self.sector_size as usize == 0
                {
                    // Buffer is already aligned
                    debug!("Using direct write with already aligned buffer");
                    self.file.write(buf)
                } else {
                    // Create an aligned copy
                    debug!("Creating aligned copy for non-aligned buffer");
                    let mut aligned_buf =
                        match AlignedBuffer::new(aligned_size, self.sector_size as usize) {
                            Ok(buffer) => buffer,
                            Err(e) => {
                                return Err(io::Error::new(
                                    e.kind(),
                                    format!("Failed to create aligned write buffer: {}", e),
                                ));
                            }
                        };

                    aligned_buf.copy_from_slice(buf, 0);

                    // Double check alignment before write
                    let new_ptr_addr = aligned_buf.as_slice().as_ptr() as usize;
                    let new_alignment_ok = new_ptr_addr % self.sector_size as usize == 0;
                    let new_size_ok = aligned_buf.as_slice().len() % self.sector_size as usize == 0;

                    if !new_alignment_ok || !new_size_ok {
                        error!("CRITICAL: Created aligned buffer is still not properly aligned!");
                        error!(
                            "Address: 0x{:X}, size: {}, sector size: {}",
                            new_ptr_addr,
                            aligned_buf.as_slice().len(),
                            self.sector_size
                        );
                    } else {
                        debug!(
                            "Successfully created aligned buffer at 0x{:X} with size {}",
                            new_ptr_addr,
                            aligned_buf.as_slice().len()
                        );
                    }

                    let written = self.file.write(aligned_buf.as_slice())?;

                    // Only report up to the original buffer size
                    Ok(cmp::min(written, buf.len()))
                }?;

                self.position += write_result as u64;
                return Ok(write_result);
            }
        }

        // For smaller writes, buffer them
        debug!("Using buffered write for small data: {} bytes", buf.len());
        let bytes_written = self.buffer.copy_from_slice(buf, self.buffer_pos);
        self.buffer_pos += bytes_written;

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Call our manual flush implementation, not this method (which would cause infinite recursion)
        AlignedDiskIO::flush(self)
    }
}

impl Seek for AlignedDiskIO {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // Flush any pending writes
        self.flush()?;

        // Calculate the new position
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => if delta >= 0 {
                self.position.checked_add(delta as u64)
            } else {
                self.position.checked_sub((-delta) as u64)
            }
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek position"))?,
            SeekFrom::End(delta) => {
                // On Windows with direct I/O, seeking to End(0) often fails with physical devices
                // Handle this case specially by getting the size using DeviceIoControl
                if delta == 0 {
                    debug!("Windows: Special handling for seek to end (SeekFrom::End(0))");

                    #[cfg(windows)]
                    {
                        use std::os::windows::io::AsRawHandle;
                        use windows_sys::Win32::Foundation::HANDLE;
                        use windows_sys::Win32::System::IO::DeviceIoControl;

                        // Constants for disk geometry IOCTL
                        const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007405C;

                        #[repr(C)]
                        struct GET_LENGTH_INFORMATION {
                            length: u64,
                        }

                        let handle = self.file.as_raw_handle() as HANDLE;
                        let mut length_info = GET_LENGTH_INFORMATION { length: 0 };
                        let mut bytes_returned: u32 = 0;

                        let result = unsafe {
                            DeviceIoControl(
                                handle,
                                IOCTL_DISK_GET_LENGTH_INFO,
                                std::ptr::null_mut(),
                                0,
                                &mut length_info as *mut _ as *mut _,
                                std::mem::size_of::<GET_LENGTH_INFORMATION>() as u32,
                                &mut bytes_returned,
                                std::ptr::null_mut(),
                            )
                        };

                        if result != 0 {
                            // Success - use the disk size
                            debug!(
                                "Windows: Got disk size: {} bytes using IOCTL",
                                length_info.length
                            );
                            return self.handle_successful_seek(length_info.length);
                        }

                        // If IOCTL fails, try standard seek first
                        debug!("Windows: IOCTL failed, trying standard seek");
                        match self.file.seek(SeekFrom::End(0)) {
                            Ok(file_size) => {
                                debug!(
                                    "Windows: Standard seek to end succeeded: {} bytes",
                                    file_size
                                );
                                return self.handle_successful_seek(file_size);
                            }
                            Err(e) => {
                                debug!(
                                    "Windows: Standard seek to end failed: {}, using fallback size",
                                    e
                                );
                                // Fallback - use a large offset which should be near the end
                                // A reasonable size for most physical drives (16GB)
                                let fallback_size = 16 * 1024 * 1024 * 1024;
                                debug!("Windows: Using fallback size: {} bytes", fallback_size);
                                return self.handle_successful_seek(fallback_size);
                            }
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        // For non-Windows platforms, just do a regular seek
                        let file_size = self.file.seek(SeekFrom::End(0))?;
                        self.file.seek(SeekFrom::Start(self.position))?;
                        return self.handle_successful_seek(file_size);
                    }
                }

                // For non-zero delta or non-Windows platforms, use standard approach
                let file_size = self.file.seek(SeekFrom::End(0))?;

                // Reset to current position
                self.file.seek(SeekFrom::Start(self.position))?;

                if delta >= 0 {
                    file_size.checked_add(delta as u64)
                } else {
                    file_size.checked_sub((-delta) as u64)
                }
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek position")
                })?
            }
        };

        // Update our position
        self.position = new_pos;

        // Seek the underlying file
        self.file.seek(SeekFrom::Start(new_pos))?;

        Ok(new_pos)
    }
}

impl AlignedDiskIO {
    // Helper method to handle successful seek operations
    fn handle_successful_seek(&mut self, file_size: u64) -> io::Result<u64> {
        // Update our position to the file size
        self.position = file_size;

        // Seek the underlying file to our new position
        match self.file.seek(SeekFrom::Start(file_size)) {
            Ok(_) => Ok(file_size),
            Err(e) => {
                debug!(
                    "Failed to seek underlying file to position {}: {}",
                    file_size, e
                );
                // Return the file size anyway since we know it
                // This is a common pattern for Windows physical disks where seek may fail
                // but we still want to report the correct size
                Ok(file_size)
            }
        }
    }
}

/// Simple wrapper to convert a standard File to an aligned I/O version
pub fn aligned_disk_io(
    file: File,
    sector_size: u32,
) -> io::Result<impl Read + Write + Seek + Debug> {
    AlignedDiskIO::new(file, sector_size)
}
