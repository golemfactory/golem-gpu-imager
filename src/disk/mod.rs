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

/// Common functionality for disk access regardless of platform
mod common;
pub use common::{DiskDevice, WriteProgress};

/// Platform-specific disk operations trait
#[cfg(target_os = "linux")]
use linux::LinuxDiskAccess as PlatformDiskAccess;

#[cfg(windows)]
use windows::WindowsDiskAccess as PlatformDiskAccess;
use crate::disk::common::bytes_to_mb;

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
    use windows_sys::Win32::Foundation::GetLastError;
    
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
            let (tracked_image_file, events) = common::track_progress(image_file, size);

            sipper.send(WriteProgress::Start).await;

            // Set up a channel to forward progress events to the sipper
            {
                let mut s = sipper.clone();
                let mut events = events;
                tokio::task::spawn(async move {
                    while let Some(ev) = events.recv().await {
                        s.send(WriteProgress::Write(ev)).await;
                    }
                });
            }

            // Use blocking task for I/O operations to avoid blocking the async runtime
            let r = tokio::task::spawn_blocking(move || {
                // Platform-specific pre-write checks
                // Note: Disk cleaning is now done during lock_path, before we have an exclusive lock
                // We still pass the original path for verification purposes
                info!("Using original path for final pre-write checks: {}", original_path);
                
                // Pass the original_path to pre_write_checks for any platform-specific final checks
                let pre_write_result = PlatformDiskAccess::pre_write_checks(&disk_file, Some(&original_path));
                
                if let Err(e) = pre_write_result {
                    return Err(e);
                }

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
                    let mut buffer = vec![0u8; ALIGNED_BUFFER_SIZE];
                    
                    info!("Windows: Using aligned intermediate buffer of {} bytes", ALIGNED_BUFFER_SIZE);
                    
                    // Read from source, write to disk in aligned chunks
                    let mut total_copied: u64 = 0;
                    let mut total_written: u64 = 0;
                    loop {
                        // Check if operation was cancelled before reading the next chunk
                        if cancel_token.is_cancelled() {
                            info!("Disk write operation cancelled by user");
                            return Err(anyhow::anyhow!("Operation cancelled by user"));
                        }
                        
                        // Read a chunk of data into our aligned buffer
                        let bytes_read = match source_file.read(&mut buffer) {
                            Ok(0) => break, // EOF
                            Ok(n) => n,
                            Err(e) => {
                                error!("Error reading from source: {}", e);
                                return Err(anyhow::anyhow!("Failed to read from source: {}", e));
                            }
                        };
                        
                        // Calculate padding needed to align to 4K sector
                        const SECTOR_SIZE: usize = 4096;
                        let remainder = bytes_read % SECTOR_SIZE;
                        let aligned_size = if remainder == 0 {
                            bytes_read // Already aligned
                        } else {
                            // Pad with zeros to next sector boundary
                            let padding = SECTOR_SIZE - remainder;
                            for i in bytes_read..bytes_read+padding {
                                buffer[i] = 0;
                            }
                            bytes_read + padding
                        };
                        
                        // Check if operation was cancelled before writing to disk
                        if cancel_token.is_cancelled() {
                            info!("Disk write operation cancelled by user after reading data");
                            return Err(anyhow::anyhow!("Operation cancelled by user"));
                        }

                        info!("Writing {} bytes to disk", aligned_size);
                        // Write the aligned buffer to disk
                        match disk_file.write_all(&buffer[0..aligned_size]) {
                            Ok(_) => {
                                total_copied += bytes_read as u64;
                                // Only count actual data bytes, not padding
                            },
                            Err(e) => {
                                error!("Error writing to disk: {}", e);
                                return Err(anyhow::anyhow!("Failed to write to disk: {}", e));
                            }
                        };
                        total_written += aligned_size as u64;
                        info!("Wrote {} bytes to disk, ", bytes_to_mb(total_written) );
                        info!("Total copied: {} bytes", bytes_to_mb(total_copied));
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
                debug!("Found partition with UUID {}: {}", target_uuid, part.name);

                // Get start sector and length for the partition
                let start_sector = part.first_lba;
                const SECTOR_SIZE: u64 = 512;
                let start_offset = start_sector * SECTOR_SIZE;

                // Create a new file handle for the FAT filesystem
                let partition_file = self.get_cloned_file_handle()?;

                // Get partition size for better boundary checking
                let partition_size = part
                    .last_lba
                    .checked_sub(part.first_lba)
                    .map(|sectors| sectors * SECTOR_SIZE)
                    .unwrap_or(0);

                debug!(
                    "Partition size: {} bytes ({} MB)",
                    partition_size,
                    partition_size / (1024 * 1024)
                );

                // Create a PartitionFileProxy that handles seeks relative to the partition
                let proxy = PlatformDiskAccess::create_partition_proxy(
                    partition_file,
                    start_offset,
                    partition_size,
                )?;

                // Attempt to create a FAT filesystem from the partition
                let fs_result = fatfs::FileSystem::new(proxy, fatfs::FsOptions::new());

                // Check if we encountered a FAT filesystem error
                match fs_result {
                    Ok(fs) => {
                        return Ok(fs);
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

                                // Create a new file handle for formatting
                                let format_file = self.get_cloned_file_handle()?;

                                // Create formatting proxy with appropriate platform-specific handling
                                let format_proxy = PlatformDiskAccess::create_partition_proxy(
                                    format_file,
                                    start_offset,
                                    partition_size,
                                )?;

                                // Format the partition
                                debug!("Using format options with volume label GOLEMCONF");
                                fatfs::format_volume(
                                    format_proxy,
                                    fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  "), // 11 bytes padded with spaces
                                )?;

                                debug!("Successfully formatted partition");

                                // Now try to open the freshly formatted filesystem
                                let new_file = self.get_cloned_file_handle()?;

                                // Create a new proxy with platform-specific handling
                                let new_proxy = PlatformDiskAccess::create_partition_proxy(
                                    new_file,
                                    start_offset,
                                    partition_size,
                                )?;

                                let new_fs = fatfs::FileSystem::new(
                                    new_proxy, 
                                    fatfs::FsOptions::new()
                                ).with_context(|| {
                                    format!("Failed to open newly formatted FAT filesystem on partition with UUID {}", uuid_str)
                                })?;

                                return Ok(new_fs);
                            }
                        }
                        // If we're not formatting or it's a different error, just return the error
                        return Err(error.into());
                    }
                }
            }
        }

        // No partition with matching UUID found
        Err(anyhow!("No partition found with UUID: {}", uuid_str))
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
        // Find the partition with the given UUID
        let fs = self.find_partition(uuid_str)?;

        // Get the root directory
        let root_dir = fs.root_dir();

        // Default values in case some files or settings are missing
        let mut config = GolemConfig {
            payment_network: crate::models::PaymentNetwork::Testnet,
            network_type: crate::models::NetworkType::Central,
            subnet: "public".to_string(),
            wallet_address: "".to_string(),
            glm_per_hour: "0.25".to_string(),
        };

        // Try to read golemwz.toml
        if let Ok(mut toml_file) = root_dir.open_file("golemwz.toml") {
            let mut toml_content = String::new();
            toml_file.read_to_string(&mut toml_content)?;

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
        }

        // Try to read golem.env
        if let Ok(mut env_file) = root_dir.open_file("golem.env") {
            let mut env_content = String::new();
            env_file.read_to_string(&mut env_content)?;

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
        }

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
        // Find the partition with the given UUID
        let fs = self.find_partition(uuid_str)?;

        // Get the root directory
        let root_dir = fs.root_dir();

        // Write golemwz.toml
        let toml_content = format!(
            "accepted_terms = true\nglm_account = \"{}\"\nglm_per_hour = \"0.25\"\n",
            wallet_address
        );

        // Create or overwrite the file
        let mut toml_file = root_dir.create_file("golemwz.toml")?;
        toml_file.write_all(toml_content.as_bytes())?;
        toml_file.flush()?;

        // Write golem.env
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

        // Create or overwrite the env file
        let mut env_file = root_dir.create_file("golem.env")?;
        env_file.write_all(env_content.as_bytes())?;
        env_file.flush()?;

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
