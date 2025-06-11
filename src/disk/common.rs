// Common disk operation functionality shared across platforms

use std::io::{self, Read, Seek, SeekFrom, Write};
#[cfg(windows)]
use tracing::error;
use tracing::{debug, info};

/// Disk device information structure
#[derive(Debug, Clone)]
pub struct DiskDevice {
    /// The disk path (e.g., "/dev/sda" on Linux, "\\.\PhysicalDrive0" on Windows)
    pub path: String,
    /// The disk name or model
    pub name: String,
    /// The disk size in bytes
    pub size: u64,
    /// Whether the disk is removable
    pub removable: bool,
    /// Whether the disk is readonly
    pub readonly: bool,
    /// OS-specific vendor information
    pub vendor: String,
    /// OS-specific product model
    pub model: String,
    /// Whether this disk is a system disk (contains OS)
    pub system: bool,
}

/// Status of a disk write operation, used for progress updates
#[derive(Debug, Clone)]
pub enum WriteStatus {
    /// Write operation has not started yet
    NotStarted,
    /// Write operation is in progress
    InProgress {
        /// Progress from 0.0 to 1.0
        progress: f32,
        /// Bytes written so far
        bytes_written: u64,
        /// Total bytes to write
        total_bytes: u64,
    },
    /// Write operation has completed successfully
    Completed,
    /// Write operation has failed
    Failed {
        /// Error message
        error: String,
    },
}

/// Progress message for disk write operations
#[derive(Debug)]
pub enum WriteProgress {
    Start,
    ClearingPartitions {
        progress: f32,
    },
    Write {
        total_written: u64,
        total_size: u64,
    },
    Verifying {
        verified_bytes: u64,
        total_size: u64,
    },
    Finish,
}

/// Proxy for accessing a specific partition on a disk
pub struct PartitionFileProxy<T: Read + io::Write + io::Seek> {
    /// The underlying file handle for the entire disk
    pub file: T,
    /// The offset in bytes where the partition starts
    pub partition_offset: u64,
    /// The size of the partition in bytes
    pub partition_size: u64,
    /// The current position relative to the start of the partition
    pub current_position: u64,
    /// The sector size for alignment (if needed)
    #[cfg(windows)]
    pub sector_size: u32,
}

impl<T: Read + Write + Seek> PartitionFileProxy<T> {
    /// Check if position is within partition boundaries
    fn check_position(&self) -> io::Result<()> {
        // If partition_size is 0, we're not enforcing boundaries
        if self.partition_size > 0 && self.current_position > self.partition_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Position {} is beyond partition size {}",
                    self.current_position, self.partition_size
                ),
            ));
        }
        Ok(())
    }

    /// Convert a partition-relative position to an absolute disk position
    fn to_absolute_position(&self) -> u64 {
        self.partition_offset + self.current_position
    }

    #[cfg(windows)]
    /// Check if a buffer is aligned for direct I/O on Windows
    fn is_buffer_aligned(&self, buf: &[u8]) -> bool {
        // Get buffer address details
        let ptr_addr = buf.as_ptr() as usize;
        let buffer_len = buf.len();
        let sector_size = self.sector_size as usize;

        // For very small reads, we'll handle them specially
        // The FAT filesystem often does tiny reads (3-4 bytes) that can't be aligned
        if buffer_len < 512 {
            debug!(
                "Small buffer detected (length: {}), will use intermediate aligned buffer",
                buffer_len
            );
            return false;
        }

        // Check if the buffer starts at an address that's aligned to the sector size
        let addr_aligned = ptr_addr % sector_size == 0;

        // Check if the buffer length is a multiple of the sector size
        let len_aligned = buffer_len % sector_size == 0;

        let is_aligned = addr_aligned && len_aligned;

        if !is_aligned {
            // Log detailed alignment issues for debugging
            if !addr_aligned {
                debug!(
                    "Buffer address misalignment: address 0x{:X} is not aligned to sector size {} (remainder: {})",
                    ptr_addr,
                    sector_size,
                    ptr_addr % sector_size
                );
            }

            if !len_aligned {
                debug!(
                    "Buffer length misalignment: length {} is not a multiple of sector size {} (remainder: {})",
                    buffer_len,
                    sector_size,
                    buffer_len % sector_size
                );
            }

            debug!(
                "Using aligned buffer for Windows direct I/O (original address: 0x{:X}, length: {}, sector size: {})",
                ptr_addr, buffer_len, sector_size
            );
        }

        is_aligned
    }

    #[cfg(windows)]
    /// Create an aligned buffer for direct I/O
    fn create_aligned_buffer(&self, size: usize) -> Vec<u8> {
        use std::alloc::{Layout, alloc};

        // Calculate the sector-aligned size (round up to next sector boundary)
        let sector_size = self.sector_size as usize;
        let aligned_size = ((size + sector_size - 1) / sector_size) * sector_size;

        info!(
            "Creating aligned buffer: requested size: {}, aligned size: {}, sector size: {}",
            size, aligned_size, sector_size
        );

        // Create an aligned layout - ensure sector size alignment
        // Windows requires both memory address and buffer size to be aligned to the sector size
        let layout = Layout::from_size_align(aligned_size, sector_size)
            .expect("Invalid layout for aligned buffer");

        // Allocate aligned memory
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            error!(
                "Failed to allocate aligned memory of size {} bytes",
                aligned_size
            );
            panic!(
                "Failed to allocate aligned memory of size {} bytes",
                aligned_size
            );
        }

        // Create a properly aligned vector that uses our allocated memory
        // IMPORTANT: We must use the memory we allocated with alloc(), not create a new Vec
        let mut vec = unsafe {
            // Create an empty Vec without allocating memory
            let mut v: Vec<u8> = Vec::new();

            // Set the Vec's internal fields to use our aligned memory
            v.reserve_exact(0); // Ensure Vec has a buffer pointer (could be null if just created)

            // Create a custom Vec using our aligned memory
            // The Vec will take ownership of the memory we allocated
            Vec::from_raw_parts(ptr, aligned_size, aligned_size)
        };

        // Zero the memory for safety
        unsafe {
            std::ptr::write_bytes(vec.as_mut_ptr(), 0, aligned_size);
        }

        // Double-check memory alignment
        let addr = vec.as_ptr() as usize;
        if addr % sector_size != 0 {
            error!(
                "Buffer address 0x{:X} is not aligned to {} bytes (remainder: {})",
                addr,
                sector_size,
                addr % sector_size
            );
        } else {
            info!(
                "Successfully created aligned buffer at address 0x{:X}, length: {}",
                addr,
                vec.len()
            );
        }

        // Verify the buffer's alignment
        let len_aligned = vec.len() % sector_size == 0;
        if !len_aligned {
            error!(
                "Buffer length {} is not aligned to sector size {}",
                vec.len(),
                sector_size
            );
        }

        vec
    }
}

// Note: Using tracker from utils/tracker.rs instead of duplicating implementation here

const MB: f64 = 1f64 / 1024f64 / 1024f64;

pub fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 * MB
}

// Implement Read for PartitionFileProxy
impl<T: Read + Write + Seek> Read for PartitionFileProxy<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position()?;

        // Check if we're at the end of the partition
        if self.partition_size > 0 && self.current_position == self.partition_size {
            debug!("Read operation at partition boundary - no more bytes to read");
            return Ok(0); // End of partition, no bytes to read
        }

        // Check if read would go beyond partition boundaries
        let max_read_size = if self.partition_size > 0 {
            std::cmp::min(
                buf.len() as u64,
                self.partition_size - self.current_position,
            ) as usize
        } else {
            buf.len()
        };

        // Calculate the absolute position
        let current_abs_pos = self.to_absolute_position();

        #[cfg(windows)]
        {
            // Determine if we need to use an aligned buffer for Windows direct I/O
            let use_aligned_buffer = !self.is_buffer_aligned(buf);

            if use_aligned_buffer {
                debug!("Using aligned buffer for Windows read operation");

                // Create an aligned buffer for direct I/O
                let mut aligned_buf = self.create_aligned_buffer(max_read_size);

                // Ensure we're at the correct position before reading
                let seek_result = self.file.seek(SeekFrom::Start(current_abs_pos));
                if let Err(e) = &seek_result {
                    error!("Windows seek error before read: {}", e);
                    error!(
                        "Attempted to seek to absolute position: {}",
                        current_abs_pos
                    );
                    return Err(io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to seek to position {} before read: {}",
                            current_abs_pos, e
                        ),
                    ));
                }

                // Calculate the safe read size based on buffer capacities
                let read_size = std::cmp::min(max_read_size, aligned_buf.len());

                // Read into the aligned buffer
                debug!("Reading {} bytes into aligned buffer", read_size);
                let read_result = self.file.read(&mut aligned_buf[0..read_size]);

                let mut bytes_read = match read_result {
                    Ok(bytes) => {
                        debug!("Successfully read {} bytes using aligned buffer", bytes);
                        bytes
                    }
                    Err(e) => {
                        error!("Windows read error with aligned buffer: {}", e);
                        error!(
                            "Read attempted at position {} with buffer size {}",
                            current_abs_pos, max_read_size
                        );
                        return Err(e);
                    }
                };

                // Copy data from aligned buffer to user buffer
                if bytes_read > 0 {
                    // Ensure we don't copy more than the destination buffer can hold
                    let copy_size = std::cmp::min(bytes_read, buf.len());
                    debug!(
                        "Copying {} bytes from aligned buffer to user buffer",
                        copy_size
                    );
                    buf[0..copy_size].copy_from_slice(&aligned_buf[0..copy_size]);

                    // If we limited the copy due to buf size constraints,
                    // we need to adjust the reported bytes_read value
                    if copy_size < bytes_read {
                        debug!(
                            "Limited copy to {} bytes due to destination buffer size",
                            copy_size
                        );
                        bytes_read = copy_size;
                    }
                }

                // Update current position
                self.current_position += bytes_read as u64;

                return Ok(bytes_read);
            }
        }

        // Non-Windows path or aligned buffer not needed
        // Use a potentially smaller buffer if needed
        let read_buf = if max_read_size < buf.len() {
            &mut buf[0..max_read_size]
        } else {
            buf
        };

        // Seek to the correct absolute position
        self.file.seek(SeekFrom::Start(current_abs_pos))?;

        // Perform the actual read
        let bytes_read = self.file.read(read_buf)?;

        // Update current position
        self.current_position += bytes_read as u64;

        Ok(bytes_read)
    }
}

// Implement Write for PartitionFileProxy
impl<T: Read + Write + Seek> Write for PartitionFileProxy<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position()?;

        // Check if we're at the end of the partition
        if self.partition_size > 0 && self.current_position == self.partition_size {
            info!("Write operation at partition boundary - no more bytes can be written");
            return Ok(0); // End of partition, can't write any bytes
        }

        // Check if write would go beyond partition boundaries
        let max_write_size = if self.partition_size > 0 {
            std::cmp::min(
                buf.len() as u64,
                self.partition_size - self.current_position,
            ) as usize
        } else {
            buf.len()
        };

        // Calculate the absolute position
        let current_abs_pos = self.to_absolute_position();

        #[cfg(windows)]
        {
            // Determine if we need to use an aligned buffer for Windows direct I/O
            let use_aligned_buffer = !self.is_buffer_aligned(buf);

            if use_aligned_buffer {
                debug!("Using aligned buffer for Windows write operation");

                // Create an aligned buffer for direct I/O
                let mut aligned_buf = self.create_aligned_buffer(max_write_size);

                // Copy data from user buffer to aligned buffer
                // Ensure we don't try to copy more than what is available in either buffer
                let copy_size = std::cmp::min(max_write_size, buf.len());
                let copy_size = std::cmp::min(copy_size, aligned_buf.len());
                aligned_buf[0..copy_size].copy_from_slice(&buf[0..copy_size]);

                // Ensure we're at the correct position before writing
                let seek_result = self.file.seek(SeekFrom::Start(current_abs_pos));
                if let Err(e) = &seek_result {
                    error!("Windows seek error before write: {}", e);
                    error!(
                        "Attempted to seek to absolute position: {}",
                        current_abs_pos
                    );
                    return Err(io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to seek to position {} before write: {}",
                            current_abs_pos, e
                        ),
                    ));
                }

                // Write from the aligned buffer
                debug!("Writing {} bytes from aligned buffer", copy_size);
                let bytes_written = match self.file.write(&aligned_buf[0..copy_size]) {
                    Ok(bytes) => {
                        debug!(
                            "Successfully wrote {} bytes to disk using aligned buffer",
                            bytes
                        );
                        bytes
                    }
                    Err(e) => {
                        error!("Windows write error with aligned buffer: {}", e);
                        error!(
                            "Write attempted at position {} with buffer size {}",
                            current_abs_pos, max_write_size
                        );
                        return Err(e);
                    }
                };

                // Update current position
                self.current_position += bytes_written as u64;

                return Ok(bytes_written);
            }
        }

        // Non-Windows path or aligned buffer not needed
        // Use a potentially smaller buffer if needed
        let write_buf = if max_write_size < buf.len() {
            &buf[0..max_write_size]
        } else {
            buf
        };

        // Seek to the correct absolute position
        self.file.seek(SeekFrom::Start(current_abs_pos))?;

        // Perform the actual write
        let bytes_written = self.file.write(write_buf)?;

        // Update current position
        self.current_position += bytes_written as u64;

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Simply delegate to the underlying file
        self.file.flush()
    }
}

// Implement Seek for PartitionFileProxy
impl<T: Read + Write + Seek> Seek for PartitionFileProxy<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // Calculate the new position based on the seek type
        let new_position = match pos {
            // For SeekStart, the position is relative to the start of the partition
            SeekFrom::Start(offset) => offset,

            // For SeekCurrent, add the offset to the current position
            SeekFrom::Current(offset) => {
                if offset < 0 {
                    self.current_position
                        .checked_sub(offset.abs() as u64)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "Invalid seek to a negative position",
                            )
                        })?
                } else {
                    self.current_position
                        .checked_add(offset as u64)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "Invalid seek - position overflow",
                            )
                        })?
                }
            }

            // For SeekEnd, use the known partition size if available
            SeekFrom::End(offset) => {
                if self.partition_size == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "SeekFrom::End is not supported for partition files with unknown size",
                    ));
                }

                if offset > 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot seek beyond the end of the partition",
                    ));
                }

                // Calculate position from end
                self.partition_size
                    .checked_add(offset as u64)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Invalid seek from end - position underflow",
                        )
                    })?
            }
        };

        // Check if new position is within boundaries
        if self.partition_size > 0 && new_position > self.partition_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Position {} is beyond partition size {}",
                    new_position, self.partition_size
                ),
            ));
        }

        // Store the new position - we don't actually seek in the file yet
        // The actual seek happens when we read or write
        self.current_position = new_position;

        Ok(new_position)
    }
}
