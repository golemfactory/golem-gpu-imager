// Disk operations module with platform abstraction
//
// This module provides platform-independent disk access with platform-specific
// implementations where necessary. Common operations share implementation code.

use anyhow::{Context, Result, anyhow};
use crc32fast::Hasher;
use gpt::GptConfig;
use iced::task::{self, Sipper};
use sha2::Digest;
use std::cmp;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
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
#[allow(unused_imports)]
pub use aligned_reader::AlignedReader;

/// Common functionality for disk access regardless of platform
mod common;
pub use common::{DiskDevice, WriteProgress};

/// Platform-specific disk operations trait
#[cfg(target_os = "linux")]
use linux::LinuxDiskAccess as PlatformDiskAccess;

#[cfg(windows)]
use windows::WindowsDiskAccess as PlatformDiskAccess;

/// Configuration structure returned by read_configuration
#[derive(Debug)]
pub struct GolemConfig {
    pub payment_network: crate::models::PaymentNetwork,
    pub network_type: crate::models::NetworkType,
    pub subnet: String,
    pub wallet_address: String,
    pub glm_per_hour: String,
}

/// Configuration for image writing and partition setup
#[derive(Debug, Clone)]
pub struct ImageConfiguration {
    pub payment_network: crate::models::PaymentNetwork,
    pub network_type: crate::models::NetworkType,
    pub subnet: String,
    pub wallet_address: String,
    pub glm_per_hour: String,
}

impl ImageConfiguration {
    /// Create a new ImageConfiguration with default values
    pub fn new(
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: String,
        wallet_address: String,
    ) -> Self {
        Self {
            payment_network,
            network_type,
            subnet,
            wallet_address,
            glm_per_hour: "0.25".to_string(),
        }
    }
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
        let file = self
            .platform
            .clone_file_handle(&self.file)
            .expect("Failed to clone file handle");

        // Create a new Disk with cloned file and platform
        Disk {
            file,
            platform: self.platform.clone(),
            original_path: self.original_path.clone(),
        }
    }
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

    /// Write configuration to a specific partition using an existing file handle
    ///
    /// # Arguments
    /// * `disk_file` - The locked disk file handle
    /// * `config` - Configuration to write to the partition
    ///
    /// # Returns
    /// * Result indicating success or failure
    fn write_configuration_to_partition(
        disk_file: &mut File,
        config: &ImageConfiguration,
    ) -> Result<()> {
        use std::io::{Cursor, Read, Seek, SeekFrom, Write};

        // Configuration partition UUID (commonly used for boot config)
        const CONFIG_PARTITION_UUID: &str = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

        info!(
            "Writing configuration to partition {}",
            CONFIG_PARTITION_UUID
        );

        // Parse UUID
        let target_uuid = Uuid::parse_str(CONFIG_PARTITION_UUID)
            .with_context(|| format!("Failed to parse UUID: {}", CONFIG_PARTITION_UUID))?;

        // Read GPT manually to find the configuration partition (no GPT library, no file cloning)
        info!("Reading GPT header manually to find configuration partition");

        const LOGICAL_SECTOR_SIZE: u64 = 512;
        const PHYSICAL_SECTOR_SIZE: u64 = 4096; // Windows direct I/O alignment requirement
        const GPT_SIGNATURE: [u8; 8] = [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54]; // "EFI PART"

        // For Windows direct I/O, we need to read aligned blocks
        // Read from LBA 0 (which includes LBA 1 where GPT header is) using 4KB alignment
        let read_start = 0u64; // Start from beginning of disk
        let read_size = PHYSICAL_SECTOR_SIZE * 2; // Read 8KB to ensure we get GPT header at LBA 1

        disk_file.seek(SeekFrom::Start(read_start))?;
        let mut aligned_buffer = vec![0u8; read_size as usize];
        disk_file.read_exact(&mut aligned_buffer)?;

        // Extract GPT header from LBA 1 (starts at offset 512 in our buffer)
        let gpt_header_offset = LOGICAL_SECTOR_SIZE as usize;
        if aligned_buffer.len() < gpt_header_offset + LOGICAL_SECTOR_SIZE as usize {
            return Err(anyhow!("Buffer too small for GPT header"));
        }
        let header_buffer =
            &aligned_buffer[gpt_header_offset..gpt_header_offset + LOGICAL_SECTOR_SIZE as usize];

        // Verify GPT signature
        if header_buffer[0..8] != GPT_SIGNATURE {
            return Err(anyhow!("No valid GPT found on disk"));
        }

        // Extract partition table info from GPT header
        let partition_entries_lba = u64::from_le_bytes([
            header_buffer[72],
            header_buffer[73],
            header_buffer[74],
            header_buffer[75],
            header_buffer[76],
            header_buffer[77],
            header_buffer[78],
            header_buffer[79],
        ]);
        let num_partition_entries = u32::from_le_bytes([
            header_buffer[80],
            header_buffer[81],
            header_buffer[82],
            header_buffer[83],
        ]);
        let partition_entry_size = u32::from_le_bytes([
            header_buffer[84],
            header_buffer[85],
            header_buffer[86],
            header_buffer[87],
        ]);

        info!(
            "GPT: {} partition entries at LBA {}, {} bytes each",
            num_partition_entries, partition_entries_lba, partition_entry_size
        );

        // Read partition entries with proper alignment for Windows direct I/O
        let partition_entries_offset = partition_entries_lba * LOGICAL_SECTOR_SIZE;
        let partition_table_logical_size =
            num_partition_entries as u64 * partition_entry_size as u64;

        // Round up to physical sector alignment for Windows
        let aligned_offset =
            (partition_entries_offset / PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;
        let offset_within_read = (partition_entries_offset - aligned_offset) as usize;
        // Need to account for the offset when calculating aligned size
        let total_needed = offset_within_read as u64 + partition_table_logical_size;
        let aligned_size = total_needed.div_ceil(PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;

        info!(
            "Reading partition table: logical offset={}, logical size={}, aligned offset={}, aligned size={}, offset within read={}, total needed={}",
            partition_entries_offset,
            partition_table_logical_size,
            aligned_offset,
            aligned_size,
            offset_within_read,
            total_needed
        );

        disk_file.seek(SeekFrom::Start(aligned_offset))?;
        let mut aligned_partition_buffer = vec![0u8; aligned_size as usize];
        disk_file.read_exact(&mut aligned_partition_buffer)?;

        // Extract the actual partition table from the aligned buffer
        let partition_table_end = offset_within_read + partition_table_logical_size as usize;
        if aligned_partition_buffer.len() < partition_table_end {
            return Err(anyhow!("Aligned buffer too small for partition table"));
        }
        let partition_table = &aligned_partition_buffer[offset_within_read..partition_table_end];

        // Find our target partition by scanning entries
        let mut start_offset = 0u64;
        let mut partition_size = 0u64;
        let mut found = false;
        let mut discovered_partitions = Vec::new();

        for i in 0..num_partition_entries {
            let entry_offset = (i as usize) * (partition_entry_size as usize);
            if entry_offset + 128 > partition_table.len() {
                break; // Safety check
            }

            // Extract partition GUID (bytes 16-31 of partition entry)
            let partition_guid = &partition_table[entry_offset + 16..entry_offset + 32];

            // Skip empty partition entries (all zeros)
            if partition_guid.iter().all(|&b| b == 0) {
                continue;
            }

            // Convert partition GUID to UUID string for logging
            // GPT uses mixed-endian format: first 8 bytes need byte-swapping, last 8 bytes unchanged
            let mut guid_bytes = [0u8; 16];
            guid_bytes.copy_from_slice(partition_guid);

            // Apply UEFI GUID mixed-endian conversion:
            // - Bytes 0-3: little-endian (swap)
            // - Bytes 4-5: little-endian (swap)
            // - Bytes 6-7: little-endian (swap)
            // - Bytes 8-15: big-endian (unchanged)
            guid_bytes[0..4].reverse(); // First 4 bytes
            guid_bytes[4..6].reverse(); // Next 2 bytes  
            guid_bytes[6..8].reverse(); // Next 2 bytes
            // Last 8 bytes remain unchanged

            let partition_uuid = Uuid::from_bytes(guid_bytes);

            // Extract partition type GUID (bytes 0-15 of partition entry)
            let type_guid = &partition_table[entry_offset..entry_offset + 16];
            let mut type_bytes = [0u8; 16];
            type_bytes.copy_from_slice(type_guid);

            // Apply same UEFI GUID conversion for type UUID
            type_bytes[0..4].reverse(); // First 4 bytes
            type_bytes[4..6].reverse(); // Next 2 bytes  
            type_bytes[6..8].reverse(); // Next 2 bytes
            // Last 8 bytes remain unchanged

            let type_uuid = Uuid::from_bytes(type_bytes);

            // Extract LBA range for size calculation
            let first_lba = u64::from_le_bytes([
                partition_table[entry_offset + 32],
                partition_table[entry_offset + 33],
                partition_table[entry_offset + 34],
                partition_table[entry_offset + 35],
                partition_table[entry_offset + 36],
                partition_table[entry_offset + 37],
                partition_table[entry_offset + 38],
                partition_table[entry_offset + 39],
            ]);
            let last_lba = u64::from_le_bytes([
                partition_table[entry_offset + 40],
                partition_table[entry_offset + 41],
                partition_table[entry_offset + 42],
                partition_table[entry_offset + 43],
                partition_table[entry_offset + 44],
                partition_table[entry_offset + 45],
                partition_table[entry_offset + 46],
                partition_table[entry_offset + 47],
            ]);
            let part_size = (last_lba - first_lba + 1) * LOGICAL_SECTOR_SIZE;

            // Store discovered partition info for logging
            discovered_partitions.push(format!(
                "Partition {}: UUID={}, Type={}, Size={}MB",
                i,
                partition_uuid,
                type_uuid,
                part_size / (1024 * 1024)
            ));

            // Convert target UUID to byte array for comparison
            let target_bytes = target_uuid.as_bytes();

            // Apply same UEFI GUID conversion to partition_guid for comparison
            let mut comparison_guid_bytes = [0u8; 16];
            comparison_guid_bytes.copy_from_slice(partition_guid);
            comparison_guid_bytes[0..4].reverse(); // First 4 bytes
            comparison_guid_bytes[4..6].reverse(); // Next 2 bytes  
            comparison_guid_bytes[6..8].reverse(); // Next 2 bytes
            // Last 8 bytes remain unchanged

            if comparison_guid_bytes == *target_bytes {
                // Found our partition! Use already extracted LBA values
                start_offset = first_lba * LOGICAL_SECTOR_SIZE;
                partition_size = (last_lba - first_lba + 1) * LOGICAL_SECTOR_SIZE;
                found = true;

                info!(
                    "Found configuration partition: LBA {}-{}, offset {}, size {} bytes",
                    first_lba, last_lba, start_offset, partition_size
                );
                break;
            }
        }

        // Log all discovered partitions for debugging
        info!(
            "Discovered {} partitions in GPT:",
            discovered_partitions.len()
        );
        for partition_info in &discovered_partitions {
            info!("  {}", partition_info);
        }

        if !found {
            error!(
                "Configuration partition {} not found",
                CONFIG_PARTITION_UUID
            );
            error!("Available partitions:");
            for partition_info in &discovered_partitions {
                error!("  {}", partition_info);
            }
            return Err(anyhow!(
                "Configuration partition {} not found. Available partitions: {}",
                CONFIG_PARTITION_UUID,
                if discovered_partitions.is_empty() {
                    "None".to_string()
                } else {
                    discovered_partitions.join(", ")
                }
            ));
        }

        info!(
            "Found partition at offset {}, size {} bytes",
            start_offset, partition_size
        );

        // Read partition into memory with proper alignment for Windows direct I/O
        // Round partition boundaries to physical sector alignment
        let aligned_start = (start_offset / PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;
        let offset_within_aligned = (start_offset - aligned_start) as usize;
        // Need to account for the offset when calculating aligned size
        let total_needed = offset_within_aligned as u64 + partition_size;
        let aligned_size = total_needed.div_ceil(PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;

        info!(
            "Reading partition data: logical offset={}, logical size={}, aligned offset={}, aligned size={}, offset within aligned={}, total needed={}",
            start_offset,
            partition_size,
            aligned_start,
            aligned_size,
            offset_within_aligned,
            total_needed
        );

        disk_file.seek(SeekFrom::Start(aligned_start))?;
        let mut aligned_partition_data = vec![0u8; aligned_size as usize];
        disk_file.read_exact(&mut aligned_partition_data)?;

        // Extract the actual partition data from the aligned buffer
        let partition_end = offset_within_aligned + partition_size as usize;
        if aligned_partition_data.len() < partition_end {
            return Err(anyhow!("Aligned buffer too small for partition data"));
        }
        let mut partition_data =
            aligned_partition_data[offset_within_aligned..partition_end].to_vec();

        // Format partition with FAT filesystem
        {
            let cursor = Cursor::new(&mut partition_data[..]);
            fatfs::format_volume(
                cursor,
                fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  "),
            )?;
        }

        // Create filesystem on formatted data and write files
        {
            let cursor = Cursor::new(&mut partition_data[..]);
            let fs = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new())?;
            let root_dir = fs.root_dir();

            // Write golemwz.toml
            let toml_content = format!(
                "accepted_terms = true\nglm_account = \"{}\"\nglm_per_hour = \"{}\"\n",
                config.wallet_address, config.glm_per_hour
            );
            let mut toml_file = root_dir.create_file("golemwz.toml")?;
            toml_file.write_all(toml_content.as_bytes())?;
            toml_file.flush()?;
            drop(toml_file);

            // Write golem.env
            let payment_network_str = match config.payment_network {
                crate::models::PaymentNetwork::Testnet => "testnet",
                crate::models::PaymentNetwork::Mainnet => "mainnet",
            };
            let network_type_str = match config.network_type {
                crate::models::NetworkType::Hybrid => "hybrid",
                crate::models::NetworkType::Central => "central",
            };
            let env_content = format!(
                "YA_NET_TYPE={}\nSUBNET={}\nYA_PAYMENT_NETWORK_GROUP={}\n",
                network_type_str, config.subnet, payment_network_str
            );
            let mut env_file = root_dir.create_file("golem.env")?;
            env_file.write_all(env_content.as_bytes())?;
            env_file.flush()?;
            drop(env_file);

            // Filesystem will be dropped at end of this block, releasing the mutable borrow
        }

        // Write partition back to disk with proper alignment for Windows direct I/O
        // We need to write back the entire aligned block to preserve data outside our partition
        // Copy our modified partition data back into the aligned buffer
        aligned_partition_data[offset_within_aligned..partition_end]
            .copy_from_slice(&partition_data);

        info!(
            "Writing aligned partition data back to disk: offset={}, size={}",
            aligned_start, aligned_size
        );
        disk_file.seek(SeekFrom::Start(aligned_start))?;
        disk_file.write_all(&aligned_partition_data)?;
        disk_file.flush()?;

        info!("Successfully wrote configuration to partition");
        Ok(())
    }

    /// Write an image file to the disk with progress reporting
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file to write
    /// * `metadata` - Image metadata containing expected size and hash
    /// * `cancel_token` - Token to cancel the operation
    /// * `config` - Optional configuration to write after image writing
    ///
    /// # Returns
    /// * A sipper that reports progress updates as the write proceeds
    pub fn write_image(
        self,
        image_path: &str,
        metadata: crate::models::ImageMetadata,
        cancel_token: crate::models::CancelToken,
        config: Option<ImageConfiguration>,
    ) -> impl Sipper<Result<WriteProgress>, WriteProgress> + Send + 'static {
        debug!("Opening image file: {}", image_path);
        let image_path_owned = image_path.to_string();
        let image_file_r = File::open(&image_path_owned)
            .with_context(|| format!("Failed to open image file: {}", image_path_owned));

        // Use a larger buffer for better performance (matching disk-image-writer)
        const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer

        // Save original path and platform data before moving self into the task
        let original_path = self.original_path.clone();
        let _platform_data = self.platform.clone();

        let disk_file_r = self.get_cloned_file_handle();
        task::sipper(async move |mut sipper| -> Result<WriteProgress> {
            let image_file = std::io::BufReader::with_capacity(BUFFER_SIZE, image_file_r?);
            let _size = image_file.get_ref().metadata()?.len();

            // Don't use buffered writers as they can interfere with direct I/O alignment
            // For consistent behavior across platforms, use unbuffered writes everywhere
            let mut disk_file = disk_file_r?;

            // Set up progress tracking
            //let (tracked_image_file, events) = tracker::track_progress(image_file, size);
            let tracked_image_file = image_file;

            sipper.send(WriteProgress::Start).await;

            // Use blocking task for I/O operations to avoid blocking the async runtime
            tokio::task::spawn_blocking(move || {
                // Platform-specific pre-write checks
                // Note: Disk cleaning is now done during lock_path, before we have an exclusive lock
                // We still pass the original path for verification purposes
                info!(
                    "Using original path for final pre-write checks: {}",
                    original_path
                );

                // Pass the original_path to pre_write_checks for any platform-specific final checks
                // Use ? operator for more concise error handling
                PlatformDiskAccess::pre_write_checks(&disk_file, Some(&original_path))?;

                // Clear first and last 4MB of disk to remove any existing partition tables or file systems
                info!("Clearing first and last 4MB of disk");
                
                // Get disk size
                let disk_size = get_disk_size_windows(&mut disk_file)?;
                
                // Create 4MB zero buffer (sector-aligned for Windows compatibility)
                let zero_buffer = vec![0u8; 4 * 1024 * 1024];
                
                // Clear first 4MB
                disk_file.seek(SeekFrom::Start(0))?;
                disk_file.write_all(&zero_buffer)?;
                
                // Clear last 4MB (if disk is large enough)
                if disk_size > 8 * 1024 * 1024 {
                    let last_4mb_start = disk_size - (4 * 1024 * 1024);
                    disk_file.seek(SeekFrom::Start(last_4mb_start))?;
                    disk_file.write_all(&zero_buffer)?;
                    info!("Cleared first and last 4MB of disk ({} MB total disk size)", disk_size / (1024 * 1024));
                } else {
                    info!("Disk too small ({} MB), only cleared first 4MB", disk_size / (1024 * 1024));
                }

                // Seek back to the beginning of the disk to start writing image data
                disk_file.seek(SeekFrom::Start(0))?;

                // Create XZ reader with our tracked file
                // Force buffer size to be a multiple of 4096 for Windows direct I/O
                let buffer_size = std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap(); // 4MB aligned buffer
                info!(
                    "Creating XZ reader with aligned buffer size: {} bytes",
                    buffer_size
                );

                // XzReader::new_with_buffer_size returns XzReader directly, not a Result
                let mut source_file =
                    XzReader::new_with_buffer_size(tracked_image_file, buffer_size);

                info!("Starting to copy decompressed image data to disk");

                // Track total bytes copied for verification later
                let mut total_copied: u64 = 0;
                let mut total_written: u64 = 0;

                // Use a properly aligned buffer for consistent behavior across platforms
                // Direct I/O on Windows requires alignment, and this approach helps with
                // buffer management on all platforms
                {
                    // Use aligned buffer copies instead of direct copy
                    const ALIGNED_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer aligned to 4K

                    // Create a buffer that's a multiple of sector size for alignment
                    let mut buffer = vec![0u8; ALIGNED_BUFFER_SIZE];

                    info!(
                        "Windows: Using aligned intermediate buffer of {} bytes",
                        ALIGNED_BUFFER_SIZE
                    );

                    let mut ramaining_bytes = metadata.uncompressed_size;
                    let total_size = metadata.uncompressed_size;

                    while ramaining_bytes > 0 {
                        // Check if operation was cancelled before reading the next chunk
                        if cancel_token.is_cancelled() {
                            info!("Disk write operation cancelled by user");
                            return Err(anyhow::anyhow!("Operation cancelled by user"));
                        }

                        let bytes_to_write: usize = cmp::min(ramaining_bytes, ALIGNED_BUFFER_SIZE as u64).try_into()?;

                        source_file.read_exact(&mut buffer[..bytes_to_write])?;
                        disk_file.write_all(&buffer[0..bytes_to_write])?;

                        total_copied += bytes_to_write as u64;
                        total_written += bytes_to_write as u64;
                        ramaining_bytes -= bytes_to_write as u64;

                        {
                            let mut sipper = sipper.clone();
                            std::mem::drop(tokio::spawn(async move {
                                sipper
                                    .send(WriteProgress::Write {
                                        total_written,
                                        total_size,
                                    })
                                    .await
                            }));
                        }
                    }

                    info!(
                        "Successfully copied {} bytes with aligned buffers",
                        total_copied
                    );
                }

                // DEBUG: Block-by-block comparison of XZ content vs disk content
                #[cfg(feature = "debug")]
                {
                    info!("DEBUG: Starting block-by-block comparison of XZ content vs disk content");
                        
                        // Re-open XZ file for comparison
                        let debug_image_file = File::open(&image_path_owned)?;
                        let debug_buffer_size = std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap();
                        let mut debug_xz_reader = XzReader::new_with_buffer_size(debug_image_file, debug_buffer_size);
                        
                        // Seek disk back to start for comparison
                        disk_file.seek(SeekFrom::Start(0))?;
                        
                        let block_size = 1024 * 1024; // 1MB blocks
                        let mut debug_xz_buffer = vec![0u8; block_size];
                        let mut debug_disk_buffer = vec![0u8; block_size];
                        let mut block_number = 0;
                        let mut total_compared = 0u64;
                        let mut differences_found = 0;
                        
                        loop {
                            // Check if we've compared enough (limit to image size)
                            if total_compared >= metadata.uncompressed_size {
                                break;
                            }
                            
                            // Calculate how much to read in this block
                            let remaining = metadata.uncompressed_size - total_compared;
                            let bytes_to_read = std::cmp::min(block_size as u64, remaining) as usize;
                            if remaining == 0 {
                                break
                            }
                            
                            // Read from XZ
                            debug_xz_reader.read_exact(&mut debug_xz_buffer[0..bytes_to_read])?;
                            
                            // Read from disk
                            let disk_bytes = disk_file.read(&mut debug_disk_buffer[0..bytes_to_read])?;
                            
                            if disk_bytes != bytes_to_read {
                                error!("DEBUG: Block {} size mismatch: XZ={}, Disk={}", block_number, bytes_to_read, disk_bytes);
                                differences_found += 1;
                                break;
                            }
                            
                            // Compare the blocks
                            if debug_xz_buffer[0..bytes_to_read] != debug_disk_buffer[0..bytes_to_read] {
                                error!("DEBUG: Block {} differs at offset {}", block_number, total_compared);
                                differences_found += 1;
                                
                                // Find first differing byte
                                for i in 0..bytes_to_read {
                                    if debug_xz_buffer[i] != debug_disk_buffer[i] {
                                        error!("DEBUG: First difference at byte {}: XZ=0x{:02x}, Disk=0x{:02x}", 
                                               total_compared + i as u64, debug_xz_buffer[i], debug_disk_buffer[i]);
                                        
                                        // Show context around the difference
                                        let start = i.saturating_sub(8);
                                        let end = (i + 8).min(bytes_to_read);
                                        error!("DEBUG: XZ  context: {:02x?}", &debug_xz_buffer[start..end]);
                                        error!("DEBUG: Disk context: {:02x?}", &debug_disk_buffer[start..end]);
                                        break;
                                    }
                                }
                                
                                // Calculate hashes of this block
                                let xz_block_hash = sha2::Sha256::digest(&debug_xz_buffer[0..bytes_to_read]);
                                let disk_block_hash = sha2::Sha256::digest(&debug_disk_buffer[0..bytes_to_read]);
                                error!("DEBUG: Block {} XZ hash:   {}", block_number, hex::encode(&xz_block_hash[..8]));
                                error!("DEBUG: Block {} Disk hash: {}", block_number, hex::encode(&disk_block_hash[..8]));
                                
                                // Limit to first few differing blocks to avoid spam
                                if differences_found >= 5 {
                                    error!("DEBUG: Stopping after {} differences to avoid log spam", differences_found);
                                    break;
                                }
                            }
                            
                            total_compared += bytes_to_read as u64;
                            block_number += 1;
                            
                            // Safety limit to avoid infinite loops
                            if block_number > 20000 { // ~20GB worth of 1MB blocks
                                warn!("DEBUG: Stopping comparison after {} blocks to avoid excessive processing", block_number);
                                break;
                            }
                        }
                        
                        if differences_found == 0 {
                            info!("DEBUG: No differences found between XZ and disk content ({} bytes in {} blocks)", total_compared, block_number);
                        } else {
                            error!("DEBUG: Found {} differences in {} blocks ({} bytes compared)", differences_found, block_number, total_compared);
                        }
                }

                // Verify written data
                info!("Starting written data verification");

                // Seek to start of disk for verification
                disk_file.seek(SeekFrom::Start(0))?;

                // Initialize hasher for verification
                let mut verifier = sha2::Sha256::new();
                let mut verified_bytes = 0u64;
                // Use metadata.uncompressed_size for verification to match hash calculation
                // This ensures we only verify the exact bytes that were in the original image
                let total_size = metadata.uncompressed_size;
                    const SECTOR_SIZE: u64 = 4096; // Use 4KB alignment for Windows compatibility
                    let buffer_size = 4 * 1024 * 1024; // 4MB buffer
                    let mut buffer = vec![0u8; buffer_size];

                    info!(
                        "Reading back {} bytes for verification (actual bytes written)",
                        total_size
                    );

                    while verified_bytes < total_size {
                        // Check for cancellation
                        if cancel_token.is_cancelled() {
                            info!("Verification cancelled by user");
                            return Err(anyhow::anyhow!("Verification cancelled by user"));
                        }

                        let remaining = total_size - verified_bytes;

                        // For Windows direct I/O, we need to read in sector-aligned chunks
                        // Calculate aligned read size, but ensure we don't read beyond what we need

                        // For the final chunk, we need to be careful not to read beyond the image boundary
                        // even when aligning to sectors. Only align if we're reading the final incomplete chunk.
                        let aligned_read_size = if remaining <= buffer.len() as u64 {
                            // This is the final chunk - only align if the remaining bytes are not already aligned
                            if remaining % SECTOR_SIZE == 0 {
                                // Already sector-aligned, read exactly what we need
                                remaining
                            } else {
                                // Not aligned, so we need to read a sector-aligned amount
                                // But limit it to avoid reading beyond device boundaries
                                let aligned_size =
                                    remaining.div_ceil(SECTOR_SIZE) * SECTOR_SIZE;
                                // Only use aligned size if it's within reasonable bounds (not more than one extra sector)
                                if aligned_size - remaining <= SECTOR_SIZE {
                                    aligned_size
                                } else {
                                    // Fall back to exact size if alignment would read too much
                                    remaining
                                }
                            }
                        } else {
                            // For full buffer reads, use the buffer size (which should be sector-aligned)
                            buffer.len() as u64
                        };

                        // Ensure we don't exceed buffer size
                        let final_read_size =
                            std::cmp::min(aligned_read_size, buffer.len() as u64) as usize;

                        match disk_file.read(&mut buffer[0..final_read_size]) {
                            Ok(0) => {
                                warn!(
                                    "Unexpected EOF during verification at {} bytes",
                                    verified_bytes
                                );
                                break;
                            }
                            Ok(bytes_read) => {
                                // Only hash the actual data bytes, not padding
                                let actual_data_bytes =
                                    std::cmp::min(bytes_read as u64, remaining) as usize;
                                verifier.update(&buffer[0..actual_data_bytes]);
                                verified_bytes += actual_data_bytes as u64;

                                // Send verification progress
                                let mut sipper_clone = sipper.clone();
                                let total_size_copy = total_size;
                                std::mem::drop(tokio::spawn(async move {
                                    sipper_clone
                                        .send(WriteProgress::Verifying {
                                            verified_bytes,
                                            total_size: total_size_copy,
                                        })
                                        .await
                                }));

                                // Log progress every 100MB
                                if verified_bytes % (100 * 1024 * 1024) == 0
                                    || verified_bytes == total_size
                                {
                                    info!(
                                        "Verified {} / {} MB",
                                        verified_bytes / (1024 * 1024),
                                        total_size / (1024 * 1024)
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Error reading data for verification: {}", e);
                                
                                // Check for specific Windows errors that indicate device disconnection
                                if let Some(error_code) = e.raw_os_error() {
                                    match error_code {
                                        433 => {
                                            // Device does not exist - USB disconnected or reassigned
                                            warn!("USB device became unavailable during verification (error 433)");
                                            warn!("This often happens with USB devices during long operations");
                                            return Err(anyhow::anyhow!(
                                                "Device became unavailable during verification. \
                                                The image was successfully written, but verification failed because \
                                                the USB device was disconnected or reassigned by Windows. \
                                                You can safely use the device - the write operation completed successfully."
                                            ));
                                        }
                                        21 => {
                                            // Device not ready
                                            warn!("Device not ready during verification (error 21)");
                                            return Err(anyhow::anyhow!(
                                                "Device not ready during verification. \
                                                The image was successfully written, but the device may need to be \
                                                reconnected for verification."
                                            ));
                                        }
                                        _ => {
                                            // Other I/O errors
                                            return Err(anyhow::anyhow!("Verification read failed: {}", e));
                                        }
                                    }
                                } else {
                                    return Err(anyhow::anyhow!("Verification read failed: {}", e));
                                }
                            }
                        }
                    }

                    // Finalize hash and compare
                    let calculated_hash = verifier.finalize();
                    let calculated_hash_hex = hex::encode(calculated_hash);

                    info!("Calculated hash: {}", &calculated_hash_hex[..16]);
                    info!("Expected hash:   {}", &metadata.uncompressed_hash[..16]);

                    if calculated_hash_hex != metadata.uncompressed_hash {
                        error!("Hash verification failed!");
                        error!("Expected: {}", metadata.uncompressed_hash);
                        error!("Got:      {}", calculated_hash_hex);
                        return Err(anyhow::anyhow!(
                            "Data verification failed: written data does not match expected hash"
                        ));
                    }

                    info!("Hash verification successful - written data is correct");

                info!("Post-copy checks starting");

                // Fix GPT backup header location after unlocking volume
                info!("Checking and fixing GPT backup header location if needed");
                if let Err(e) = fix_gpt_backup_header(&mut disk_file) {
                    warn!("Failed to fix GPT backup header (non-fatal): {:?}", e);
                }
                if let Some(config) = config {
                    Self::write_configuration_to_partition(&mut disk_file, &config).context("failed to write configuration")?;
                }

                // On Windows, unlock the volume first to allow GPT operations
                #[cfg(windows)]
                {
                    info!("Unlocking volume before GPT operations (Windows only)");
                    let unlock_start = std::time::Instant::now();
                    if let Err(e) = PlatformDiskAccess::unlock_volume(&disk_file) {
                        warn!(
                            "Failed to unlock disk volume before GPT operations after {:?}: {}",
                            unlock_start.elapsed(),
                            e
                        );
                    } else {
                        info!(
                            "Volume unlocked successfully in {:?} - GPT operations can now proceed",
                            unlock_start.elapsed()
                        );
                    }
                }



                // We already handled the copy with our manual implementation
                let copy_result = Ok(0); // Placeholder since we already did the copy
                if let Err(e) = &copy_result {
                    error!("Failed to write image to disk: {}", e);

                    // Platform-specific error handling
                    if let Some(error_context) = PlatformDiskAccess::handle_write_error(e) {
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

                    let handle =
                        disk_file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
                    let sync_result = unsafe { FlushFileBuffers(handle) };
                    if sync_result == 0 {
                        let err = std::io::Error::last_os_error();
                        warn!("FlushFileBuffers failed: {}", err);
                    } else {
                        info!(
                            "FlushFileBuffers completed successfully in {:?}",
                            sync_start.elapsed()
                        );
                    }
                }

                // Now do the regular flush
                info!("Starting disk flush operation");
                let flush_start = std::time::Instant::now();
                info!("Attempting to flush disk buffer...");
                let flush_result = disk_file.flush();
                let flush_duration = flush_start.elapsed();

                if let Err(e) = flush_result {
                    error!(
                        "Failed to flush disk buffer after {:?}: {}",
                        flush_duration, e
                    );

                    // Platform-specific flush error handling
                    if let Some(error_context) = PlatformDiskAccess::handle_flush_error(&e) {
                        return Err(error_context);
                    }

                    return Err(anyhow::anyhow!(
                        "Failed to complete disk write operation: {}",
                        e
                    ));
                } else {
                    info!("Disk flush completed successfully in {:?}", flush_duration);
                }
                info!("Successfully wrote image to disk");

                anyhow::Ok(WriteProgress::Finish)
            })
            .await?
        })
    }

    /// Write configuration to a disk after image has been written
    ///
    /// # Arguments
    /// * `disk_path` - Path to the disk device
    /// * `config` - Configuration to write
    ///
    /// # Returns
    /// * Result indicating success or failure
    #[allow(dead_code)]
    pub async fn write_configuration_to_disk(
        disk_path: &str,
        config: ImageConfiguration,
    ) -> Result<()> {
        info!("Opening disk for configuration writing: {}", disk_path);

        // Open disk in edit mode to write configuration
        let mut disk = Self::lock_path(disk_path, true).await?;

        // Configuration partition UUID (commonly used for boot config)
        const CONFIG_PARTITION_UUID: &str = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

        // Write the configuration to the partition
        disk.write_configuration(
            CONFIG_PARTITION_UUID,
            config.payment_network,
            config.network_type,
            &config.subnet,
            &config.wallet_address,
        )?;

        info!("Successfully wrote configuration to disk");
        Ok(())
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
        use tracing::{error, info};

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
                    error!(
                        "Warning: Read fewer bytes than expected: {} of {} bytes",
                        bytes_read, partition_size
                    );

                    // Truncate the buffer to the actual size read
                    partition_data.truncate(bytes_read);

                    // Update partition_size to reflect what we actually read
                    let actual_partition_size = bytes_read as u64;
                    info!(
                        "Adjusted partition size to {} bytes based on actual read",
                        actual_partition_size
                    );

                    return Ok((start_offset, actual_partition_size, partition_data));
                }

                info!(
                    "Successfully read entire partition ({} bytes) into memory",
                    bytes_read
                );
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
        use tracing::{error, info};

        // Seek to the start of the partition
        match self.file.seek(SeekFrom::Start(start_offset)) {
            Ok(_) => info!("Successfully sought to partition offset {}", start_offset),
            Err(e) => {
                error!("Failed to seek to partition offset: {}", e);
                #[cfg(windows)]
                if let Some(code) = e.raw_os_error() {
                    if code == 5 {
                        // Access denied
                        return Err(anyhow!(
                            "Access denied when seeking to partition offset. Ensure you are running with administrator privileges"
                        ));
                    }
                }
                return Err(anyhow!("Failed to seek to partition offset: {}", e));
            }
        }

        // Write the entire partition in one operation with robust error handling
        info!(
            "Writing partition data ({} bytes) back to disk at offset {}",
            partition_data.len(),
            start_offset
        );

        let write_result = self.file.write(partition_data);

        match write_result {
            Ok(bytes_written) => {
                if bytes_written < partition_data.len() {
                    error!(
                        "Warning: Wrote fewer bytes than expected: {} of {} bytes",
                        bytes_written,
                        partition_data.len()
                    );
                    return Err(anyhow!(
                        "Short write: wrote only {} of {} bytes",
                        bytes_written,
                        partition_data.len()
                    ));
                }
                info!("Successfully wrote {} bytes to disk", bytes_written);
            }
            Err(e) => {
                error!("Failed to write partition data: {}", e);
                return Err(anyhow!("Failed to write partition data: {}", e));
            }
        }

        // Make sure data is flushed to disk with proper error handling
        info!("Flushing data to disk");
        if let Err(e) = self.file.flush() {
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
        use tracing::{debug, error};

        // Read the entire partition into memory
        let (start_offset, partition_size, partition_data) =
            self.read_partition_to_memory(uuid_str)?;

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

                        debug!(
                            "Successfully created FAT filesystem from newly formatted partition"
                        );
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
        use std::io::Read;
        use tracing::debug;

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
                    debug!(
                        "Successfully read golemwz.toml file: {} bytes",
                        toml_content.len()
                    );

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
                    debug!(
                        "Successfully read golem.env file: {} bytes",
                        env_content.len()
                    );

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
        use std::io::{Cursor, Seek, SeekFrom, Write};
        use tracing::{info, warn};

        // First, read the entire partition into memory
        let (start_offset, _partition_size, mut partition_data) =
            self.read_partition_to_memory(uuid_str)?;

        info!(
            "Read partition data ({} bytes) from disk at offset {}",
            partition_data.len(),
            start_offset
        );

        // Create a cursor that provides Read+Write+Seek for the FAT filesystem
        // The cursor operates directly on our partition data
        let mut cursor = Cursor::new(&mut partition_data[..]);

        // Format the partition if needed
        let format_result = if true {
            // Always format to ensure clean state
            info!("Formatting in-memory partition with volume label GOLEMCONF");
            fatfs::format_volume(
                &mut cursor,
                fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  "), // 11 bytes padded with spaces
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

/// Get disk size using Windows-specific IOCTL (for when seek to end fails)
#[cfg(windows)]
fn get_disk_size_windows(disk_file: &mut File) -> Result<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    // Use IOCTL_DISK_GET_LENGTH_INFO to get disk size
    const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007405C;

    #[repr(C)]
    struct GET_LENGTH_INFORMATION {
        length: u64,
    }

    let handle = disk_file.as_raw_handle() as HANDLE;
    let mut length_info = GET_LENGTH_INFORMATION { length: 0 };
    let mut bytes_returned: u32 = 0;

    info!("Windows: Attempting to get disk size using IOCTL_DISK_GET_LENGTH_INFO");

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
        info!(
            "Windows: Got disk size: {} bytes using IOCTL",
            length_info.length
        );
        Ok(length_info.length)
    } else {
        let error_code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        warn!(
            "Windows: IOCTL_DISK_GET_LENGTH_INFO failed with error {}",
            error_code
        );

        // For direct I/O context, seeking to end often fails, so don't even try it
        // Instead, use a reasonable estimate based on common disk sizes
        warn!("Windows: Direct I/O context - cannot seek to end, using size estimation");

        // Try to estimate disk size by seeking to different positions to find the end
        // This is more compatible with direct I/O restrictions
        let test_positions = [
            8 * 1024 * 1024 * 1024u64,   // 8GB
            16 * 1024 * 1024 * 1024u64,  // 16GB
            32 * 1024 * 1024 * 1024u64,  // 32GB
            64 * 1024 * 1024 * 1024u64,  // 64GB
            128 * 1024 * 1024 * 1024u64, // 128GB
        ];

        let mut estimated_size = 32 * 1024 * 1024 * 1024u64; // Default 32GB

        for &test_size in &test_positions {
            // Try to seek near the end of the disk at this size
            if let Ok(_) = disk_file.seek(SeekFrom::Start(test_size - 4096)) {
                estimated_size = test_size;
                info!("Windows: Disk appears to be at least {} bytes", test_size);
            } else {
                break; // This size is too large
            }
        }

        warn!(
            "Windows: Using estimated disk size: {} bytes",
            estimated_size
        );
        Ok(estimated_size)
    }
}

#[cfg(not(windows))]
fn get_disk_size_windows(disk_file: &mut File) -> Result<u64> {
    // On non-Windows platforms, just use seek
    Ok(disk_file.seek(SeekFrom::End(0))?)
}

/// Fix GPT backup header location when device is larger than image
///
/// When a smaller image is written to a larger device, the backup GPT header
/// remains at the image's end position instead of the device's end position.
/// This causes CRC validation failures. This function relocates the backup
/// header to the correct position at the end of the device.
///
/// All reads and writes are sector-aligned for Windows compatibility.
///
/// # Arguments
/// * `disk_file` - The disk file handle
///
/// # Returns
/// * `Result<()>` - Ok on success, Error on failure
fn fix_gpt_backup_header(disk_file: &mut File) -> Result<()> {
    // GPT uses 512-byte logical sectors, but Windows I/O requires 4KB physical alignment
    const LOGICAL_SECTOR_SIZE: u64 = 512;
    const GPT_SIGNATURE: [u8; 8] = [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54]; // "EFI PART"

    // Physical sector size for I/O alignment (Windows needs 4KB, Linux can use 512B)
    #[cfg(windows)]
    const PHYSICAL_SECTOR_SIZE: u64 = 4096;
    #[cfg(not(windows))]
    const PHYSICAL_SECTOR_SIZE: u64 = 512;

    // Get disk size (must be aligned to logical sector boundary for GPT calculations)
    info!("Step 1: Getting disk size");
    let disk_size = get_disk_size_windows(disk_file)?;
    let disk_sectors = disk_size / LOGICAL_SECTOR_SIZE;

    info!(
        "Disk size: {} bytes ({} logical sectors), physical sector size: {} bytes",
        disk_size, disk_sectors, PHYSICAL_SECTOR_SIZE
    );

    // Read primary GPT header (physically-aligned buffer for Windows compatibility)
    info!("Step 2: Reading primary GPT header");

    // For Windows direct I/O, read from sector 0 to get both MBR and GPT header
    let read_start = 0u64;
    let read_size = PHYSICAL_SECTOR_SIZE * 2; // Read 8KB to ensure we get GPT header at LBA 1

    info!(
        "Reading aligned data from offset {} with size {} bytes",
        read_start, read_size
    );
    disk_file
        .seek(SeekFrom::Start(read_start))
        .with_context(|| format!("Failed to seek to aligned position {}", read_start))?;

    let mut aligned_buffer = vec![0u8; read_size as usize];
    disk_file.read_exact(&mut aligned_buffer).with_context(|| {
        format!(
            "Failed to read {} bytes for aligned GPT data",
            aligned_buffer.len()
        )
    })?;

    // Extract GPT header from LBA 1 (starts at offset 512 in our buffer)
    let gpt_header_offset_in_buffer = LOGICAL_SECTOR_SIZE as usize;
    if aligned_buffer.len() < gpt_header_offset_in_buffer + LOGICAL_SECTOR_SIZE as usize {
        return Err(anyhow!("Buffer too small for GPT header"));
    }
    let mut header_buffer = aligned_buffer
        [gpt_header_offset_in_buffer..gpt_header_offset_in_buffer + LOGICAL_SECTOR_SIZE as usize]
        .to_vec();
    // Pad to physical sector size for later writing
    header_buffer.resize(PHYSICAL_SECTOR_SIZE as usize, 0);

    // Verify GPT signature (first 512 bytes contain the GPT header)
    if header_buffer[0..8] != GPT_SIGNATURE {
        info!("No GPT signature found - skipping GPT backup header fix");
        return Ok(());
    }

    // Extract backup_lba field (bytes 32-39)
    let current_backup_lba = u64::from_le_bytes([
        header_buffer[32],
        header_buffer[33],
        header_buffer[34],
        header_buffer[35],
        header_buffer[36],
        header_buffer[37],
        header_buffer[38],
        header_buffer[39],
    ]);

    // Calculate expected backup LBA (last sector of disk)
    let expected_backup_lba = disk_sectors - 1;

    info!(
        "Current backup LBA: {}, Expected backup LBA: {}",
        current_backup_lba, expected_backup_lba
    );

    // If backup header is already at the correct location, no fix needed
    if current_backup_lba == expected_backup_lba {
        info!("GPT backup header is already at correct location");
        return Ok(());
    }

    info!(
        "Fixing GPT backup header location from LBA {} to LBA {}",
        current_backup_lba, expected_backup_lba
    );

    // Read the current backup header from its current location (physically-aligned)
    info!("Step 3: Reading current backup header");
    let backup_header_logical_offset = current_backup_lba * LOGICAL_SECTOR_SIZE;

    // Align backup header reading to physical sector boundaries
    let backup_aligned_start =
        (backup_header_logical_offset / PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;
    let backup_offset_within_read = (backup_header_logical_offset - backup_aligned_start) as usize;

    info!(
        "Reading backup header: logical offset={}, aligned offset={}, offset within read={}",
        backup_header_logical_offset, backup_aligned_start, backup_offset_within_read
    );

    disk_file
        .seek(SeekFrom::Start(backup_aligned_start))
        .with_context(|| {
            format!(
                "Failed to seek to aligned backup position {}",
                backup_aligned_start
            )
        })?;

    let mut backup_aligned_buffer = vec![0u8; PHYSICAL_SECTOR_SIZE as usize];
    disk_file
        .read_exact(&mut backup_aligned_buffer)
        .with_context(|| {
            format!(
                "Failed to read {} bytes for aligned backup data",
                backup_aligned_buffer.len()
            )
        })?;

    // Extract backup header from the aligned buffer
    if backup_aligned_buffer.len() < backup_offset_within_read + LOGICAL_SECTOR_SIZE as usize {
        return Err(anyhow!("Aligned buffer too small for backup header"));
    }
    let mut backup_buffer = backup_aligned_buffer
        [backup_offset_within_read..backup_offset_within_read + LOGICAL_SECTOR_SIZE as usize]
        .to_vec();
    // Pad to physical sector size for later writing
    backup_buffer.resize(PHYSICAL_SECTOR_SIZE as usize, 0);

    // Update the current_lba field in the backup header to point to new location
    let new_backup_lba_bytes = expected_backup_lba.to_le_bytes();
    backup_buffer[24..32].copy_from_slice(&new_backup_lba_bytes);

    // Calculate partition entries LBA for backup header (typically backup_lba - 32)
    let partition_entries_sectors = 32u64; // Standard GPT partition entries use 32 sectors
    let backup_partition_entries_lba =
        expected_backup_lba.saturating_sub(partition_entries_sectors);
    let backup_partition_entries_bytes = backup_partition_entries_lba.to_le_bytes();
    backup_buffer[72..80].copy_from_slice(&backup_partition_entries_bytes);

    // Zero out CRC32 field before recalculating
    backup_buffer[16] = 0;
    backup_buffer[17] = 0;
    backup_buffer[18] = 0;
    backup_buffer[19] = 0;

    // Calculate new CRC32 for backup header (standard GPT header size is 92 bytes)
    let mut hasher = Hasher::new();
    hasher.update(&backup_buffer[0..92]);
    let backup_crc32 = hasher.finalize();
    let backup_crc32_bytes = backup_crc32.to_le_bytes();
    backup_buffer[16..20].copy_from_slice(&backup_crc32_bytes);

    // Write backup header to new location (physically-aligned write)
    info!("Step 4: Writing backup header to new location");
    let new_backup_logical_offset = expected_backup_lba * LOGICAL_SECTOR_SIZE;

    // Align new backup header writing to physical sector boundaries
    let new_backup_aligned_start =
        (new_backup_logical_offset / PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;
    let new_backup_offset_within_write =
        (new_backup_logical_offset - new_backup_aligned_start) as usize;

    info!(
        "Writing backup header: logical offset={}, aligned offset={}, offset within write={}",
        new_backup_logical_offset, new_backup_aligned_start, new_backup_offset_within_write
    );

    // Read the existing data at the target location to preserve surrounding data
    disk_file
        .seek(SeekFrom::Start(new_backup_aligned_start))
        .with_context(|| {
            format!(
                "Failed to seek to new backup aligned position {}",
                new_backup_aligned_start
            )
        })?;

    let mut new_backup_aligned_buffer = vec![0u8; PHYSICAL_SECTOR_SIZE as usize];
    // Try to read existing data, but don't fail if we can't (might be at end of disk)
    let _ = disk_file.read_exact(&mut new_backup_aligned_buffer);

    // Copy our backup header into the aligned buffer
    let backup_end = new_backup_offset_within_write + LOGICAL_SECTOR_SIZE as usize;
    if new_backup_aligned_buffer.len() >= backup_end {
        new_backup_aligned_buffer[new_backup_offset_within_write..backup_end]
            .copy_from_slice(&backup_buffer[0..LOGICAL_SECTOR_SIZE as usize]);
    }

    // Write the aligned buffer
    disk_file
        .seek(SeekFrom::Start(new_backup_aligned_start))
        .with_context(|| {
            format!(
                "Failed to seek to new backup location at offset {}",
                new_backup_aligned_start
            )
        })?;
    info!(
        "Writing {} bytes for aligned backup header",
        new_backup_aligned_buffer.len()
    );
    disk_file
        .write_all(&new_backup_aligned_buffer)
        .with_context(|| {
            format!(
                "Failed to write {} bytes for backup header",
                new_backup_aligned_buffer.len()
            )
        })?;

    // Update primary header's backup_lba field
    info!("Step 5: Updating primary header");
    header_buffer[32..40].copy_from_slice(&new_backup_lba_bytes);

    // Zero out primary header CRC32 before recalculating
    header_buffer[16] = 0;
    header_buffer[17] = 0;
    header_buffer[18] = 0;
    header_buffer[19] = 0;

    // Calculate new CRC32 for primary header
    let mut hasher = Hasher::new();
    hasher.update(&header_buffer[0..92]);
    let primary_crc32 = hasher.finalize();
    let primary_crc32_bytes = primary_crc32.to_le_bytes();
    header_buffer[16..20].copy_from_slice(&primary_crc32_bytes);

    // Write updated primary header (physically-aligned write)
    info!("Step 5b: Writing updated primary header");

    // We already read the aligned buffer earlier, now copy the updated header back
    let primary_header_offset_in_buffer = LOGICAL_SECTOR_SIZE as usize;
    aligned_buffer[primary_header_offset_in_buffer
        ..primary_header_offset_in_buffer + LOGICAL_SECTOR_SIZE as usize]
        .copy_from_slice(&header_buffer[0..LOGICAL_SECTOR_SIZE as usize]);

    // Write the entire aligned buffer back
    disk_file
        .seek(SeekFrom::Start(0))
        .with_context(|| "Failed to seek back to beginning for primary header update")?;
    info!(
        "Writing {} bytes for updated primary header area",
        aligned_buffer.len()
    );
    disk_file.write_all(&aligned_buffer).with_context(|| {
        format!(
            "Failed to write {} bytes for primary header area",
            aligned_buffer.len()
        )
    })?;

    // Move partition entries table for backup if needed
    if current_backup_lba != expected_backup_lba {
        info!("Step 6: Moving partition entries table");
        // Read partition entries from after primary header (physically-aligned)
        let primary_partition_entries_lba = 2u64; // Standard location
        let primary_partition_entries_logical_offset =
            primary_partition_entries_lba * LOGICAL_SECTOR_SIZE;
        let partition_entries_logical_size =
            (partition_entries_sectors * LOGICAL_SECTOR_SIZE) as usize;

        // Align partition entries reading to physical sector boundaries
        let entries_aligned_start = (primary_partition_entries_logical_offset
            / PHYSICAL_SECTOR_SIZE)
            * PHYSICAL_SECTOR_SIZE;
        let entries_offset_within_read =
            (primary_partition_entries_logical_offset - entries_aligned_start) as usize;
        let entries_aligned_size = (partition_entries_logical_size + entries_offset_within_read)
            .div_ceil(PHYSICAL_SECTOR_SIZE as usize)
            * PHYSICAL_SECTOR_SIZE as usize;

        info!(
            "Reading partition entries: logical offset={}, logical size={}, aligned offset={}, aligned size={}",
            primary_partition_entries_logical_offset,
            partition_entries_logical_size,
            entries_aligned_start,
            entries_aligned_size
        );

        disk_file
            .seek(SeekFrom::Start(entries_aligned_start))
            .with_context(|| {
                format!(
                    "Failed to seek to aligned partition entries at offset {}",
                    entries_aligned_start
                )
            })?;

        let mut entries_aligned_buffer = vec![0u8; entries_aligned_size];
        disk_file
            .read_exact(&mut entries_aligned_buffer)
            .with_context(|| {
                format!(
                    "Failed to read {} bytes for aligned partition entries",
                    entries_aligned_size
                )
            })?;

        // Extract the actual partition entries from the aligned buffer
        let entries_end = entries_offset_within_read + partition_entries_logical_size;
        if entries_aligned_buffer.len() < entries_end {
            return Err(anyhow!("Aligned buffer too small for partition entries"));
        }
        let partition_entries = &entries_aligned_buffer[entries_offset_within_read..entries_end];

        // Write partition entries to backup location (physically-aligned write)
        let backup_partition_entries_logical_offset =
            backup_partition_entries_lba * LOGICAL_SECTOR_SIZE;

        // Align backup partition entries writing to physical sector boundaries
        let backup_entries_aligned_start =
            (backup_partition_entries_logical_offset / PHYSICAL_SECTOR_SIZE) * PHYSICAL_SECTOR_SIZE;
        let backup_entries_offset_within_write =
            (backup_partition_entries_logical_offset - backup_entries_aligned_start) as usize;

        info!(
            "Writing backup partition entries: logical offset={}, aligned offset={}, offset within write={}",
            backup_partition_entries_logical_offset,
            backup_entries_aligned_start,
            backup_entries_offset_within_write
        );

        // Read existing data at backup location to preserve surrounding data
        disk_file
            .seek(SeekFrom::Start(backup_entries_aligned_start))
            .with_context(|| {
                format!(
                    "Failed to seek to backup entries aligned position {}",
                    backup_entries_aligned_start
                )
            })?;

        let mut backup_entries_aligned_buffer = vec![0u8; entries_aligned_size];
        // Try to read existing data, but don't fail if we can't (might be at end of disk)
        let _ = disk_file.read_exact(&mut backup_entries_aligned_buffer);

        // Copy partition entries into the aligned buffer
        let backup_entries_end = backup_entries_offset_within_write + partition_entries.len();
        if backup_entries_aligned_buffer.len() >= backup_entries_end {
            backup_entries_aligned_buffer[backup_entries_offset_within_write..backup_entries_end]
                .copy_from_slice(partition_entries);
        }

        // Write the aligned buffer
        disk_file
            .seek(SeekFrom::Start(backup_entries_aligned_start))
            .with_context(|| {
                format!(
                    "Failed to seek to backup partition entries at offset {}",
                    backup_entries_aligned_start
                )
            })?;
        info!(
            "Writing {} bytes for aligned backup partition entries",
            backup_entries_aligned_buffer.len()
        );
        disk_file
            .write_all(&backup_entries_aligned_buffer)
            .with_context(|| {
                format!(
                    "Failed to write {} bytes for partition entries",
                    backup_entries_aligned_buffer.len()
                )
            })?;
    }

    // Ensure all data is flushed to disk
    info!("Step 7: Flushing all data to disk");
    disk_file
        .flush()
        .with_context(|| "Failed to flush data to disk")?;

    info!("Successfully fixed GPT backup header location");
    Ok(())
}

/// Lists available disk devices in the system
///
/// # Returns
/// * `Result<Vec<DiskDevice>>` - A list of available disk devices
#[allow(dead_code)]
pub async fn list_available_disks() -> Result<Vec<DiskDevice>> {
    PlatformDiskAccess::list_available_disks().await
}
