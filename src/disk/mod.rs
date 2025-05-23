// Disk operations module with platform abstraction
//
// This module provides platform-independent disk access with platform-specific
// implementations where necessary. Common operations share implementation code.

use anyhow::{Context, Result, anyhow};
use gpt::GptConfig;
use iced::task::{self, Sipper};
use std::fs::File;
use std::io::{Read, Seek, Write};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use xz4rust::XzReader;

// Platform-specific modules
#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
mod windows_aligned_io;
#[cfg(windows)]
pub use windows_aligned_io::aligned_disk_io;

// Aligned I/O modules
mod aligned_reader;
pub use aligned_reader::AlignedReader;

/// Common functionality for disk access regardless of platform
mod common;
pub use common::{DiskDevice, WriteProgress};

/// Platform-specific disk operations trait
#[cfg(target_os = "linux")]
use linux::LinuxDiskAccess as PlatformDiskAccess;

#[cfg(windows)]
use windows::WindowsDiskAccess as PlatformDiskAccess;
use crate::disk::common::bytes_to_mb;
use crate::utils::tracker;

/// Configuration structure returned by read_configuration
#[derive(Debug)]
pub struct GolemConfig {
    pub payment_network: crate::models::PaymentNetwork,
    pub network_type: crate::models::NetworkType,
    pub subnet: String,
    pub wallet_address: String,
    pub glm_per_hour: String,
}

/// Main disk access struct that provides platform-independent access to disks
#[derive(Debug)]
pub struct Disk {
    // The file handle for the disk
    file: File,

    // Platform-specific data and operations
    platform: PlatformDiskAccess,
    
    // Original path used to open this disk - preserved for operations that need path info
    // This is particularly important for Windows disk cleaning
    original_path: String,
}

// We can't #[derive(Clone)] because File doesn't implement Clone
// Instead, we implement it manually using clone_file_handle
impl Clone for Disk {
    fn clone(&self) -> Self {
        // Clone the file handle using platform-specific method
        let file = self.platform.clone_file_handle(&self.file)
            .expect("Failed to clone file handle");
            
        // Create a new Disk with cloned file and platform
        Disk {
            file,
            platform: self.platform.clone(),
            original_path: self.original_path.clone(),
        }
    }
}

#[cfg(windows)]
fn path_str_from_file(file: &File) -> Option<String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    
    match file.try_clone() {
        Ok(f) => {
            // Try to get path from file using Windows-specific API
            let handle = f.as_raw_handle() as HANDLE;
            let mut name_buf = [0u16; 260]; // MAX_PATH
            let len = unsafe {
                GetFinalPathNameByHandleW(
                    handle,
                    name_buf.as_mut_ptr(),
                    name_buf.len() as u32,
                    0,
                )
            };
            if len > 0 {
                match String::from_utf16(&name_buf[0..len as usize]) {
                    Ok(s) => {
                        // For Windows, this will likely be a path like \\?\GLOBALROOT\Device\HarddiskVolume1
                        // or \\?\PhysicalDrive0 - we need to check for PhysicalDrive pattern
                        Some(s)
                    },
                    Err(_) => None,
                }
            } else {
                // If we can't get the path, use the disk_number from platform data if available
                None
            }
        },
        Err(_) => None,
    }
}

#[cfg(not(windows))]
fn path_str_from_file(_file: &File) -> Option<String> {
    // On non-Windows platforms, this is less critical since we don't use diskpart
    None
}

impl Disk {
    /// Open and lock a disk by its path
    ///
    /// # Arguments
    /// * `path` - The path to the disk device
    ///   (e.g., "/dev/sda" on Linux, "\\.\PhysicalDrive0" or "C:" on Windows)
    /// * `edit_mode` - When true, we're opening for editing configuration only, not writing an image.
    ///   This skips diskpart cleaning on Windows, which avoids potential data loss during editing.
    ///
    /// # Returns
    /// * `Result<Self>` - A new Disk instance on success, Error on failure
    pub async fn lock_path(path: &str, edit_mode: bool) -> Result<Self> {
        // Platform-specific implementation to open and lock disk
        let (file, platform) = PlatformDiskAccess::lock_path(path, edit_mode).await?;

        Ok(Disk { 
            file, 
            platform,
            original_path: path.to_string(),
        })
    }

    /// Get a cloned file handle to the disk
    fn get_cloned_file_handle(&self) -> Result<File> {
        self.platform.clone_file_handle(&self.file)
    }

    /// Write an image file to the disk with progress reporting
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file to write
    ///
    /// # Returns
    /// * A sipper that reports progress updates as the write proceeds
    pub fn write_image(
        mut self,
        image_path: &str,
        cancel_token: crate::models::CancelToken,
    ) -> impl Sipper<Result<WriteProgress>, WriteProgress> + Send + 'static {
        debug!("Opening image file: {}", image_path);
        let image_file_r = File::open(image_path)
            .with_context(|| format!("Failed to open image file: {}", image_path));

        // Use a larger buffer for better performance (matching disk-image-writer)
        const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer

        // Save original path before moving self into the task
        let original_path = self.original_path.clone();
        
        let disk_file_r = self.get_cloned_file_handle();
        task::sipper(async move |mut sipper| -> Result<WriteProgress> {
            let image_file = std::io::BufReader::with_capacity(BUFFER_SIZE, image_file_r?);
            let size = image_file.get_ref().metadata()?.len();

            // Don't use buffered writers as they can interfere with direct I/O alignment
            // For consistent behavior across platforms, use unbuffered writes everywhere
            let mut disk_file = disk_file_r?;

            // Set up progress tracking
            //let (tracked_image_file, events) = tracker::track_progress(image_file, size);
            let tracked_image_file = image_file;

            sipper.send(WriteProgress::Start).await;

           

            // Use blocking task for I/O operations to avoid blocking the async runtime
            let r = tokio::task::spawn_blocking(move || {
                // Platform-specific pre-write checks
                // Note: Disk cleaning is now done during lock_path, before we have an exclusive lock
                // We still pass the original path for verification purposes
                info!("Using original path for final pre-write checks: {}", original_path);
                
                // Pass the original_path to pre_write_checks for any platform-specific final checks
                // Use ? operator for more concise error handling
                PlatformDiskAccess::pre_write_checks(&disk_file, Some(&original_path))?;

                // Create XZ reader with our tracked file
                // Force buffer size to be a multiple of 4096 for Windows direct I/O
                let buffer_size = std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap(); // 4MB aligned buffer
                info!("Creating XZ reader with aligned buffer size: {} bytes", buffer_size);

                // XzReader::new_with_buffer_size returns XzReader directly, not a Result
                let mut source_file =
                    XzReader::new_with_buffer_size(tracked_image_file, buffer_size);

                info!("Starting to copy decompressed image data to disk");
                
                // Use a properly aligned buffer for consistent behavior across platforms
                // Direct I/O on Windows requires alignment, and this approach helps with
                // buffer management on all platforms
                {
                    
                    // Use aligned buffer copies instead of direct copy
                    const ALIGNED_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer aligned to 4K
                    const SECTOR_SIZE: usize = 4096;
                    
                    // Create a buffer that's a multiple of sector size for alignment
                    let mut buffer = vec![0u8; ALIGNED_BUFFER_SIZE];
                    
                    info!("Windows: Using aligned intermediate buffer of {} bytes", ALIGNED_BUFFER_SIZE);
                    
                    // Read from source, write to disk in aligned chunks
                    let mut total_copied: u64 = 0;
                    let mut total_written: u64 = 0;
                    
                    // Track how much data is in the buffer between reads
                    let mut buffer_used: usize = 0;
                    
                    loop {
                        // Check if operation was cancelled before reading the next chunk
                        if cancel_token.is_cancelled() {
                            info!("Disk write operation cancelled by user");
                            return Err(anyhow::anyhow!("Operation cancelled by user"));
                        }
                        
                        // Shift any remaining data to the start of the buffer if needed
                        if buffer_used > 0 && buffer_used < ALIGNED_BUFFER_SIZE {
                            buffer.copy_within(buffer_used.., 0);
                        }
                        
                        // Reset buffer usage to account for any shifting
                        // We don't actually need to track remaining_space since we use buffer_used
                        
                        // Read a chunk of data into our aligned buffer, starting after any existing data
                        let bytes_read = match source_file.read(&mut buffer[buffer_used..]) {
                            Ok(0) => {
                                // EOF - check if we have any remaining data to write
                                if buffer_used == 0 {
                                    break; // No data left, we're done
                                }
                                0 // No new bytes read, but we still have data to process
                            },
                            Ok(n) => n,
                            Err(e) => {
                                error!("Error reading from source: {}", e);
                                return Err(anyhow::anyhow!("Failed to read from source: {}", e));
                            }
                        };
                        
                        // Update total bytes in buffer
                        buffer_used += bytes_read;
                        
                        // Calculate padding needed to align to sector boundary
                        let remainder = buffer_used % SECTOR_SIZE;
                        
                        // Calculate how many bytes we can write aligned to sector size
                        let aligned_size = if remainder == 0 {
                            // Already aligned - use all data in buffer
                            buffer_used
                        } else if bytes_read == 0 && buffer_used > 0 {
                            // Final chunk with unaligned data - pad with zeros
                            let padding = SECTOR_SIZE - remainder;
                            // Use iterator approach instead of range loop for better performance
                            buffer.iter_mut().skip(buffer_used).take(padding).for_each(|b| *b = 0);
                            buffer_used + padding
                        } else {
                            // Still more data to come - only write up to the last complete sector
                            buffer_used - remainder
                        };
                        
                        // Only write if we have a complete sector
                        if aligned_size >= SECTOR_SIZE {
                            // Check if operation was cancelled before writing to disk
                            if cancel_token.is_cancelled() {
                                info!("Disk write operation cancelled by user after reading data");
                                return Err(anyhow::anyhow!("Operation cancelled by user"));
                            }
                            
                            info!("Writing {} bytes to disk", aligned_size);
                            // Write the aligned buffer to disk
                            match disk_file.write_all(&buffer[0..aligned_size]) {
                                Ok(_) => {
                                    // Calculate actual data bytes (not including padding)
                                    let actual_data = (aligned_size - (if remainder == 0 { 0 } else { SECTOR_SIZE - remainder })) as u64;
                                    total_copied += actual_data;
                                    
                                    // Update tracking counts
                                    // Only use total_written for sector-size aligned data
                                    // total_copied is for actual data bytes
                                    total_written += aligned_size as u64;
                                    
                                    // Update total progress with written bytes
                                    let mut sipper = sipper.clone();
                                    let _ = tokio::spawn(async move { sipper.send(WriteProgress::Write(total_written)).await });                                    
                                },
                                Err(e) => {
                                    error!("Error writing to disk: {}", e);
                                    return Err(anyhow::anyhow!("Failed to write to disk: {}", e));
                                }
                            };
                            info!("Wrote {} bytes to disk, ", bytes_to_mb(total_written));
                            info!("Total copied: {} bytes", bytes_to_mb(total_copied));
                            
                            // Keep track of any unaligned data at the end for the next iteration
                            buffer_used -= aligned_size;
                        }
                        
                        // If EOF and all data has been written, exit loop
                        if bytes_read == 0 && buffer_used == 0 {
                            break;
                        }
                    }
                    
                    info!("Successfully copied {} bytes with aligned buffers", total_copied);
                }
                
                info!("Post-copy checks starting");
                
                // We already handled the copy with our manual implementation
                let copy_result = Ok(0); // Placeholder since we already did the copy
                if let Err(e) = &copy_result {
                    error!("Failed to write image to disk: {}", e);

                    // Platform-specific error handling
                    if let Some(error_context) = PlatformDiskAccess::handle_write_error(&e) {
                        return Err(error_context);
                    }

                    return Err(anyhow::anyhow!("Failed to write image to disk: {}", e));
                }

                // We need to ensure all data is physically written to disk
                // First sync to ensure filesystem operations are complete
                info!("Starting disk sync operation");
                let sync_start = std::time::Instant::now();
                #[cfg(unix)]
                {
                    use std::os::unix::io::AsRawFd;
                    let fd = disk_file.as_raw_fd();
                    unsafe {
                        info!("Calling fsync on file descriptor...");
                        let sync_result = libc::fsync(fd);
                        if sync_result != 0 {
                            let err = std::io::Error::last_os_error();
                            warn!("fsync failed: {}", err);
                        } else {
                            info!("fsync completed successfully in {:?}", sync_start.elapsed());
                        }
                    }
                }
                #[cfg(windows)]
                {
                    info!("Windows: Using FlushFileBuffers API for disk sync");
                    use std::os::windows::io::AsRawHandle;
                    use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;
                    
                    let handle = disk_file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
                    let sync_result = unsafe { FlushFileBuffers(handle) };
                    if sync_result == 0 {
                        let err = std::io::Error::last_os_error();
                        warn!("FlushFileBuffers failed: {}", err);
                    } else {
                        info!("FlushFileBuffers completed successfully in {:?}", sync_start.elapsed());
                    }
                }

                // Now do the regular flush
                info!("Starting disk flush operation");
                let flush_start = std::time::Instant::now();
                info!("Attempting to flush disk buffer...");
                let flush_result = disk_file.flush();
                let flush_duration = flush_start.elapsed();
                
                if let Err(e) = flush_result {
                    error!("Failed to flush disk buffer after {:?}: {}", flush_duration, e);

                    // Platform-specific flush error handling
                    if let Some(error_context) = PlatformDiskAccess::handle_flush_error(&e) {
                        return Err(error_context);
                    }

                    return Err(
                        anyhow::anyhow!("Failed to complete disk write operation: {}", e)
                    );
                } else {
                    info!("Disk flush completed successfully in {:?}", flush_duration);
                }

                info!("Starting volume unlock (Windows only)");
                // On Windows, unlock the volume
                #[cfg(windows)]
                {
                    info!("Attempting to unlock volume after successful write");
                    let unlock_start = std::time::Instant::now();
                    // We don't propagate unlock errors as they're not critical for the write operation itself
                    if let Err(e) = PlatformDiskAccess::unlock_volume(&disk_file) {
                        warn!("Failed to unlock disk volume after {:?}: {}", unlock_start.elapsed(), e);
                    } else {
                        info!("Volume unlocked successfully in {:?}", unlock_start.elapsed());
                    }
                }

                info!("Successfully wrote image to disk");
                anyhow::Ok(WriteProgress::Finish)
            })
            .await?;

            r
        })
    }

    /// Read an entire partition into memory
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID of the partition to read
    ///
    /// # Returns
    /// * A tuple containing (start_offset, partition_size, partition_data) if the partition is found
    fn read_partition_to_memory(&mut self, uuid_str: &str) -> Result<(u64, u64, Vec<u8>)> {
        use std::io::{Read, Seek, SeekFrom};
        use tracing::{debug, info, error};

        // Parse the provided UUID string
        let target_uuid = Uuid::parse_str(uuid_str)
            .context(format!("Failed to parse UUID string: {}", uuid_str))?;

        // Create a GPT configuration with the default logical block size (usually 512 bytes)
        let cfg = GptConfig::new().writable(false);

        // Clone the file handle
        let file_for_gpt = self.get_cloned_file_handle()?;

        // Parse GPT header and partition table from the disk
        let disk_result = cfg.open_from_device(Box::new(file_for_gpt));

        // Handle potential GPT reading errors with platform-specific behavior
        let disk = match disk_result {
            Ok(disk) => disk,
            Err(e) => {
                let error_msg = format!("Failed to parse GPT partition table: {}", e);
                if let Some(fixed_disk) = PlatformDiskAccess::handle_gpt_error(self, e.into())? {
                    fixed_disk
                } else {
                    return Err(anyhow!(error_msg));
                }
            }
        };

        // Get partitions from the disk
        let partitions = disk.partitions();

        // Find the partition with matching UUID
        for (_, part) in partitions.iter() {
            // Check for matching UUID
            if part.part_guid == target_uuid {
                info!("Found partition with UUID {}: {}", target_uuid, part.name);

                // Get start sector and length for the partition
                let start_sector = part.first_lba;
                const SECTOR_SIZE: u64 = 512;
                let start_offset = start_sector * SECTOR_SIZE;

                // Calculate partition size for better boundary checking
                let partition_size = part
                    .last_lba
                    .checked_sub(part.first_lba)
                    .map(|sectors| sectors * SECTOR_SIZE)
                    .unwrap_or(0);

                info!(
                    "Partition size: {} bytes ({} MB)",
                    partition_size,
                    partition_size / (1024 * 1024)
                );

                // Create a new file handle
                let mut partition_file = self.get_cloned_file_handle()?;
                
                // Seek to the start of the partition
                partition_file.seek(SeekFrom::Start(start_offset))?;
                
                // Read the entire partition into memory
                let mut partition_data = vec![0u8; partition_size as usize];
                let bytes_read = partition_file.read(&mut partition_data)?;
                
                if bytes_read < partition_size as usize {
                    error!("Warning: Read fewer bytes than expected: {} of {} bytes", 
                          bytes_read, partition_size);
                    
                    // Truncate the buffer to the actual size read
                    partition_data.truncate(bytes_read);
                    
                    // Update partition_size to reflect what we actually read
                    let actual_partition_size = bytes_read as u64;
                    info!("Adjusted partition size to {} bytes based on actual read", actual_partition_size);
                    
                    return Ok((start_offset, actual_partition_size, partition_data));
                }
                
                info!("Successfully read entire partition ({} bytes) into memory", bytes_read);
                return Ok((start_offset, partition_size, partition_data));
            }
        }

        // No partition with matching UUID found
        Err(anyhow!("No partition found with UUID: {}", uuid_str))
    }
    
    /// Write partition data back to disk
    ///
    /// # Arguments
    /// * `start_offset` - The offset where the partition starts
    /// * `partition_data` - The partition data to write
    ///
    /// # Returns
    /// * Result indicating success or failure
    fn write_partition_to_disk(&mut self, start_offset: u64, partition_data: &[u8]) -> Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        use tracing::{debug, info, error, warn};
        
        #[cfg(windows)]
        {
            info!("Windows-specific preparation before writing partition data");
            
            // On Windows, we need more aggressive locking for partition writes
            // The standard pre_write_checks might not be sufficient for partition writes
            // Let's use a more direct approach
            use windows::WindowsDiskAccess;
            
            // Use WindowsDiskAccess for all Windows-specific operations
            info!("Using WindowsDiskAccess for volume locking and dismounting");
            
            // First enable extended DASD I/O
            info!("Enabling extended DASD I/O access for partition write");
            WindowsDiskAccess::enable_extended_dasd_io(&self.file);
            
            // Try to lock the volume with multiple attempts
            info!("Attempting to lock volume for partition write");
            let locked = WindowsDiskAccess::lock_volume_with_retry(&self.file, 20);
            
            // If we couldn't lock after all attempts, warn but continue
            if !locked {
                warn!("WARNING: Failed to lock disk after multiple attempts");
                warn!("Continuing with partition write, but it may fail due to permission issues");
            } else {
                info!("Successfully locked disk volume for partition write");
            }
            
            // Dismount all volumes 
            info!("Attempting to dismount volumes");
            let dismount_result = WindowsDiskAccess::dismount_volume_from_handle(&self.file);
            
            if let Err(e) = dismount_result {
                warn!("Could not dismount volumes: {}", e);
            } else {
                info!("Successfully dismounted volumes for partition write");
            }
        }
        
        // Create a new file handle
        let mut partition_file = self.get_cloned_file_handle()?;
        
        // Seek to the start of the partition
        match partition_file.seek(SeekFrom::Start(start_offset)) {
            Ok(_) => info!("Successfully sought to partition offset {}", start_offset),
            Err(e) => {
                error!("Failed to seek to partition offset: {}", e);
                #[cfg(windows)]
                if let Some(code) = e.raw_os_error() {
                    if code == 5 { // Access denied
                        return Err(anyhow!("Access denied when seeking to partition offset. Ensure you are running with administrator privileges"));
                    }
                }
                return Err(anyhow!("Failed to seek to partition offset: {}", e));
            }
        }
        
        // Write the entire partition in one operation with robust error handling
        info!("Writing partition data ({} bytes) back to disk at offset {}", 
             partition_data.len(), start_offset);
        
        let write_result = partition_file.write(partition_data);
        
        match write_result {
            Ok(bytes_written) => {
                if bytes_written < partition_data.len() {
                    error!("Warning: Wrote fewer bytes than expected: {} of {} bytes", 
                          bytes_written, partition_data.len());
                    return Err(anyhow!("Short write: wrote only {} of {} bytes", 
                                     bytes_written, partition_data.len()));
                }
                info!("Successfully wrote {} bytes to disk", bytes_written);
            },
            Err(e) => {
                error!("Failed to write partition data: {}", e);
                
                #[cfg(windows)]
                {
                    // Check for multi-language error messages
                    let error_message = e.to_string();
                    
                    // Polish "Access denied" - "Odmowa dostępu"
                    if error_message.contains("Odmowa dostępu") {
                        error!("Access denied error in Polish locale when writing to disk");
                        return Err(anyhow!(
                            "Odmowa dostępu. Run as administrator (uruchom jako administrator)"
                        ).context("To jest operacja wymagająca uprawnień administratora"));
                    }
                    
                    // German "Access denied" - "Zugriff verweigert"
                    if error_message.contains("Zugriff verweigert") {
                        error!("Access denied error in German locale when writing to disk");
                        return Err(anyhow!(
                            "Zugriff verweigert. Als Administrator ausführen"
                        ).context("Dieser Vorgang erfordert Administratorrechte"));
                    }
                    
                    // Provide Windows-specific error guidance
                    if let Some(code) = e.raw_os_error() {
                        match code {
                            5 => { // Access denied
                                error!("Access denied (code 5) when writing to disk");
                                return Err(anyhow!(
                                    "Access denied when writing to disk. Ensure you are running with administrator privileges"
                                ).context("This operation requires elevated permissions on Windows"));
                            },
                            87 => { // Invalid parameter
                                error!("Invalid parameter (code 87) when writing to disk");
                                return Err(anyhow!(
                                    "Invalid parameter when writing to disk. The operation may require proper alignment"
                                ).context("Ensure the disk is properly prepared for writing"));
                            },
                            32 => { // Sharing violation
                                error!("Sharing violation (code 32) when writing to disk");
                                return Err(anyhow!(
                                    "The disk is in use by another process. Close any applications that might be using this disk"
                                ).context("Try dismounting all volumes on this disk before writing"));
                            },
                            _ => {
                                // Handle other error codes with platform-specific messages
                                #[cfg(windows)]
                                {
                                    // On Windows, use the platform-specific error message
                                    use windows::WindowsDiskAccess;
                                    let windows_error_msg = format!("Windows error: {} ({})", 
                                                                  code, 
                                                                  WindowsDiskAccess::get_windows_error_message(code as u32));
                                    return Err(anyhow!("Failed to write partition data: {}", windows_error_msg));
                                }
                                
                                #[cfg(not(windows))]
                                {
                                    // Default error handling for non-Windows platforms
                                    let generic_error_msg = format!("Error code: {}", code);
                                    return Err(anyhow!("Failed to write partition data: {}", generic_error_msg));
                                }
                            }
                        }
                    }
                }
                
                // General error handling for non-Windows platforms
                return Err(anyhow!("Failed to write partition data: {}", e));
            }
        }
        
        // Make sure data is flushed to disk with proper error handling
        info!("Flushing data to disk");
        if let Err(e) = partition_file.flush() {
            error!("Failed to flush data to disk: {}", e);
            
            #[cfg(windows)]
            if let Some(platform_error) = PlatformDiskAccess::handle_flush_error(&e) {
                return Err(platform_error);
            }
            
            return Err(anyhow!("Failed to flush data to disk: {}", e));
        }
        
        info!("Successfully wrote partition data to disk");
        Ok(())
    }

    /// Find a FAT filesystem on a partition with the specified UUID
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID of the partition to find
    ///
    /// # Returns
    /// * A FAT filesystem if the partition is found and contains a valid filesystem
    pub fn find_partition<'a>(
        &'a mut self,
        uuid_str: &str,
    ) -> Result<fatfs::FileSystem<impl Read + Write + Seek + 'a>> {
        self.find_or_create_partition(uuid_str, false)
    }

    /// Find a FAT filesystem on a partition with the specified UUID,
    /// formatting the partition if needed.
    ///
    /// This implementation reads the entire partition into memory,
    /// which avoids alignment issues with small reads/writes on Windows.
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID of the partition to find
    /// * `format_if_needed` - Whether to format the partition if a filesystem can't be found
    ///
    /// # Returns
    /// * A FAT filesystem if the partition is found or successfully formatted
    pub fn find_or_create_partition<'a>(
        &'a mut self,
        uuid_str: &str,
        format_if_needed: bool,
    ) -> Result<fatfs::FileSystem<impl Read + Write + Seek + 'a>> {
        use std::io::Cursor;
        use tracing::{debug, info, error};
        
        // Read the entire partition into memory
        let (start_offset, partition_size, partition_data) = self.read_partition_to_memory(uuid_str)?;
        
        // We need to create a cursor with ownership for writing
        // We'll clone the data since we need it for potential formatting later
        let cursor = Cursor::new(partition_data.clone());
        
        // Attempt to create a FAT filesystem from the in-memory partition
        let fs_result = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new());
        
        // Check if we encountered a FAT filesystem error
        match fs_result {
            Ok(fs) => {
                // Successfully created filesystem, return it
                debug!("Successfully created FAT filesystem from in-memory partition");
                Ok(fs)
            }
            Err(error) => {
                if format_if_needed {
                    // Check if it's the specific error we want to handle
                    let error_string = error.to_string();
                    if error_string.contains("Invalid total_sectors_16 value in BPB")
                        || error_string.contains("no FAT filesystem")
                    {
                        debug!("FAT filesystem error: {}", error_string);
                        debug!("Formatting partition with UUID: {}", uuid_str);
                        
                        // Create a fresh buffer for formatting
                        let mut format_data = vec![0u8; partition_size as usize];
                        
                        // Format the in-memory partition
                        debug!("Using format options with volume label GOLEMCONF");
                        {
                            // Create a cursor that will be dropped after formatting
                            let format_cursor = Cursor::new(&mut format_data[..]);
                            
                            fatfs::format_volume(
                                format_cursor,
                                fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  "), // 11 bytes padded with spaces
                            )?;
                        }
                        
                        debug!("Successfully formatted in-memory partition");
                        
                        // Use the formatted data directly
                        let formatted_data = format_data;
                        
                        // Write the formatted partition back to disk
                        self.write_partition_to_disk(start_offset, &formatted_data)?;
                        
                        // Create a cursor with ownership for writing
                        let new_cursor = Cursor::new(formatted_data.clone());
                        
                        // Create a filesystem from the formatted data
                        let new_fs = fatfs::FileSystem::new(new_cursor, fatfs::FsOptions::new())
                            .with_context(|| {
                                format!("Failed to open newly formatted FAT filesystem on partition with UUID {}", uuid_str)
                            })?;
                        
                        debug!("Successfully created FAT filesystem from newly formatted partition");
                        return Ok(new_fs);
                    }
                }
                // If we're not formatting or it's a different error, just return the error
                error!("Failed to create FAT filesystem: {}", error);
                Err(error.into())
            }
        }
    }

    /// Helper function to extract string values from TOML lines
    fn extract_toml_string_value(line: &str) -> Option<String> {
        if let Some(equals_pos) = line.find('=') {
            let value_part = line[equals_pos + 1..].trim();

            // Look for quoted strings
            if value_part.starts_with('"') && value_part.ends_with('"') && value_part.len() >= 2 {
                // Extract the content between quotes
                return Some(value_part[1..value_part.len() - 1].to_string());
            }

            // If no quotes, just return the value as is
            return Some(value_part.to_string());
        }
        None
    }

    /// Read Golem configuration from a partition
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID of the partition containing the configuration
    ///
    /// # Returns
    /// * The Golem configuration if found
    pub fn read_configuration(&mut self, uuid_str: &str) -> Result<GolemConfig> {
        // Use the in-memory approach to avoid small I/O operations
        let config = self.read_configuration_in_memory(uuid_str)?;
        Ok(config)
    }

    /// Read Golem configuration from a partition using in-memory approach
    /// 
    /// This implementation reads the partition contents directly rather than
    /// using the FAT filesystem API which can cause alignment issues.
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID of the partition containing the configuration
    ///
    /// # Returns
    /// * The Golem configuration if found
    fn read_configuration_in_memory(&mut self, uuid_str: &str) -> Result<GolemConfig> {
        use std::io::{Read, Seek, SeekFrom};
        use tracing::{debug, info};

        // Default values in case some files or settings are missing
        let mut config = GolemConfig {
            payment_network: crate::models::PaymentNetwork::Testnet,
            network_type: crate::models::NetworkType::Central,
            subnet: "public".to_string(),
            wallet_address: "".to_string(),
            glm_per_hour: "0.25".to_string(),
        };
        
        // Use the find_partition function to get a properly initialized FAT filesystem
        // This uses our disk-wide aligned I/O implementation under the hood
        let fs = self.find_partition(uuid_str)?;
        
        debug!("Using find_partition to get a properly initialized FAT filesystem");
        let root_dir = fs.root_dir();
        
        // Read entire files into memory at once, rather than small chunks
        // Try to read golemwz.toml
        if let Ok(mut toml_file) = root_dir.open_file("golemwz.toml") {
            // Read the entire file content at once
            let mut toml_content = String::new();
            match toml_file.read_to_string(&mut toml_content) {
                Ok(_) => {
                    debug!("Successfully read golemwz.toml file: {} bytes", toml_content.len());
                    
                    // Process each line to extract values
                    for line in toml_content.lines() {
                        if line.starts_with("glm_account") {
                            // Extract wallet address
                            if let Some(value) = Self::extract_toml_string_value(line) {
                                config.wallet_address = value;
                            }
                        } else if line.starts_with("glm_per_hour") {
                            // Extract rate
                            if let Some(value) = Self::extract_toml_string_value(line) {
                                config.glm_per_hour = value;
                            }
                        }
                    }
                },
                Err(e) => {
                    debug!("Error reading golemwz.toml: {}", e);
                }
            }
        }
        
        // Try to read golem.env
        if let Ok(mut env_file) = root_dir.open_file("golem.env") {
            // Read the entire file content at once
            let mut env_content = String::new();
            match env_file.read_to_string(&mut env_content) {
                Ok(_) => {
                    debug!("Successfully read golem.env file: {} bytes", env_content.len());
                    
                    // Process each line to extract values
                    for line in env_content.lines() {
                        if line.starts_with("YA_NET_TYPE=") {
                            let value = line.trim_start_matches("YA_NET_TYPE=").trim();
                            config.network_type = match value.to_lowercase().as_str() {
                                "hybrid" => crate::models::NetworkType::Hybrid,
                                _ => crate::models::NetworkType::Central,
                            };
                        } else if line.starts_with("SUBNET=") {
                            config.subnet = line.trim_start_matches("SUBNET=").trim().to_string();
                        } else if line.starts_with("YA_PAYMENT_NETWORK_GROUP=") {
                            let value = line.trim_start_matches("YA_PAYMENT_NETWORK_GROUP=").trim();
                            config.payment_network = match value.to_lowercase().as_str() {
                                "mainnet" => crate::models::PaymentNetwork::Mainnet,
                                _ => crate::models::PaymentNetwork::Testnet,
                            };
                        }
                    }
                },
                Err(e) => {
                    debug!("Error reading golem.env: {}", e);
                }
            }
        }
        
        // Return the config we found
        Ok(config)
    }

    /// Write Golem configuration to a partition
    ///
    /// # Arguments
    /// * `uuid_str` - The target partition UUID (e.g. "33b921b8-edc5-46a0-8baa-d0b7ad84fc71")
    /// * `payment_network` - The payment network (Testnet or Mainnet)
    /// * `network_type` - The network type (Hybrid or Central)
    /// * `subnet` - The subnet name
    /// * `wallet_address` - The GLM wallet address
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Error on failure
    pub fn write_configuration(
        &mut self,
        uuid_str: &str,
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: &str,
        wallet_address: &str,
    ) -> Result<()> {
        // Use the in-memory approach to avoid small I/O operations
        self.write_configuration_in_memory(
            uuid_str,
            payment_network,
            network_type,
            subnet,
            wallet_address,
        )
    }
    
    /// Write Golem configuration to a partition using FAT filesystem
    /// 
    /// This implementation uses our find_or_create_partition function which
    /// already handles alignment issues via the aligned_disk_io wrapper.
    /// We write one complete file at a time to avoid small I/O operations.
    ///
    /// # Arguments
    /// * `uuid_str` - The target partition UUID
    /// * `payment_network` - The payment network (Testnet or Mainnet)
    /// * `network_type` - The network type (Hybrid or Central)
    /// * `subnet` - The subnet name
    /// * `wallet_address` - The GLM wallet address
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Error on failure
    fn write_configuration_in_memory(
        &mut self,
        uuid_str: &str,
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: &str,
        wallet_address: &str,
    ) -> Result<()> {
        use std::io::{Write, Cursor, Seek, SeekFrom};
        use tracing::{debug, info, warn};
        
        // First, read the entire partition into memory
        let (start_offset, _partition_size, mut partition_data) = self.read_partition_to_memory(uuid_str)?;
        
        info!("Read partition data ({} bytes) from disk at offset {}", partition_data.len(), start_offset);
        
        // Create a cursor that provides Read+Write+Seek for the FAT filesystem
        // The cursor operates directly on our partition data
        let mut cursor = Cursor::new(&mut partition_data[..]);
        
        // Format the partition if needed
        let format_result = if true { // Always format to ensure clean state
            info!("Formatting in-memory partition with volume label GOLEMCONF");
            fatfs::format_volume(
                &mut cursor,
                fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  ") // 11 bytes padded with spaces
            )
        } else {
            Ok(())
        };
        
        if let Err(e) = format_result {
            warn!("Format error (non-fatal): {}", e);
        }
        
        // Reset cursor position to beginning of data
        cursor.seek(SeekFrom::Start(0))?;
        
        // Prepare the TOML content (complete file) before writing
        let toml_content = format!(
            "accepted_terms = true\nglm_account = \"{}\"\nglm_per_hour = \"0.25\"\n",
            wallet_address
        );
        
        // Prepare ENV content (complete file) before writing
        let payment_network_str = match payment_network {
            crate::models::PaymentNetwork::Testnet => "testnet",
            crate::models::PaymentNetwork::Mainnet => "mainnet",
        };
        
        let network_type_str = match network_type {
            crate::models::NetworkType::Hybrid => "hybrid",
            crate::models::NetworkType::Central => "central",
        };
        
        let env_content = format!(
            "YA_NET_TYPE={}\nSUBNET={}\nYA_PAYMENT_NETWORK_GROUP={}\n",
            network_type_str, subnet, payment_network_str
        );
        
        info!("Subnet value being written: '{}'", subnet);
        
        // Create a block to ensure root_dir and fs are dropped before we attempt to write partition data
        {
            // Create a FAT filesystem on the in-memory data
            let fs = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new())?;
            
            // Get the root directory
            let root_dir = fs.root_dir();
            
            // Write golemwz.toml as a complete file
            info!("Writing golemwz.toml file ({} bytes)", toml_content.len());
            let mut toml_file = root_dir.create_file("golemwz.toml")?;
            toml_file.write_all(toml_content.as_bytes())?;
            toml_file.flush()?;
            drop(toml_file); // Close the file to ensure it's flushed
            
            // Write golem.env as a complete file
            info!("Writing golem.env file ({} bytes)", env_content.len());
            let mut env_file = root_dir.create_file("golem.env")?;
            env_file.write_all(env_content.as_bytes())?;
            env_file.flush()?;
            drop(env_file); // Close the file to ensure it's flushed
            
            // root_dir and fs will be dropped automatically at the end of this block
            // which will flush all changes to our cursor_data
        }
        
        // Now we need to write the modified partition data back to disk
        info!("Writing modified partition data back to disk");
        self.write_partition_to_disk(start_offset, &partition_data)?;
        
        info!("Successfully wrote configuration to partition and saved to disk");
        Ok(())
    }
}

/// Lists available disk devices in the system
///
/// # Returns
/// * `Result<Vec<DiskDevice>>` - A list of available disk devices
pub async fn list_available_disks() -> Result<Vec<DiskDevice>> {
    PlatformDiskAccess::list_available_disks().await
}
