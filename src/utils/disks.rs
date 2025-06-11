use anyhow::{Context, Result, anyhow};
use gpt::GptConfig;
use iced::task;
use iced::task::Sipper;
#[cfg(target_os = "linux")]
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use uuid::Uuid;
// Import tracing for comprehensive logging capability
#[cfg(test)]
use tracing::trace;
#[cfg(windows)]
use tracing::warn;
use tracing::{debug, error, info};
use xz4rust::XzReader;

// OS-specific imports
#[cfg(target_os = "linux")]
use libc::{O_CLOEXEC, O_EXCL, O_SYNC};
#[cfg(target_os = "linux")]
use udisks2::zbus::zvariant::{ObjectPath, OwnedObjectPath};
#[cfg(target_os = "linux")]
use udisks2::{Client, zbus};

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle};
#[cfg(windows)]
use windows_sys::Win32::Foundation::*;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::*;
#[cfg(windows)]
use windows_sys::Win32::System::IO::DeviceIoControl;
#[cfg(windows)]
use windows_sys::Win32::System::Ioctl::*;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::*;

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

/// Lists available disk devices in the system
///
/// # Returns
/// * `Result<Vec<DiskDevice>>` - A list of available disk devices
pub async fn list_available_disks() -> Result<Vec<DiskDevice>> {
    #[cfg(target_os = "linux")]
    {
        list_linux_disks().await
    }

    #[cfg(windows)]
    {
        list_windows_disks().await
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err(anyhow!("Disk listing not implemented for this platform"))
    }
}

#[cfg(target_os = "linux")]
async fn list_linux_disks() -> Result<Vec<DiskDevice>> {
    debug!("Listing available disks on Linux");
    let mut devices = Vec::new();

    // Use rs-drivelist directly for Linux
    if let Ok(drives) = rs_drivelist::drive_list() {
        debug!("Found {} drives with rs-drivelist", drives.len());

        for drive in drives {
            // Get fields directly from DeviceDescriptor struct
            let device_path = drive
                .devicePath
                .as_ref()
                .map_or_else(|| drive.device.clone(), |p| p.clone());

            // Format mountpoints for display
            let mountpoint_str = if !drive.mountpoints.is_empty() {
                let mp = &drive.mountpoints[0].path;
                format!(" ({})", mp)
            } else {
                String::new()
            };

            // Create a name for the device
            let name = format!("{}{}", drive.description, mountpoint_str);

            // Determine system disk
            let is_system = drive.isSystem;

            // Add to our list of devices
            devices.push(DiskDevice {
                path: device_path,
                name,
                size: drive.size,
                removable: drive.isRemovable,
                readonly: drive.isReadOnly,
                vendor: "Unknown".to_string(), // Not directly available in current version
                model: drive.description.clone(),
                system: is_system,
            });
        }
    }

    // If rs-drivelist didn't find any devices or failed, try to find block devices by path pattern
    if devices.is_empty() {
        // Simple fallback using direct path enumeration
        for i in 0..8 {
            // Try common block device patterns
            let device_paths = vec![
                format!("/dev/sd{}", ('a' as u8 + i) as char),
                format!("/dev/hd{}", ('a' as u8 + i) as char),
                format!("/dev/nvme{}n1", i),
                format!("/dev/mmcblk{}", i),
            ];

            for path in device_paths {
                if std::path::Path::new(&path).exists() {
                    devices.push(DiskDevice {
                        path: path.clone(),
                        name: format!("Disk {}", path),
                        size: 0,          // Unknown size
                        removable: false, // Unknown
                        readonly: false,  // Unknown
                        vendor: "Unknown".to_string(),
                        model: "Unknown".to_string(),
                        system: false, // Unknown
                    });
                }
            }
        }
    }

    Ok(devices)
}

#[cfg(windows)]
async fn list_windows_disks() -> Result<Vec<DiskDevice>> {
    debug!("Listing available disks on Windows");
    let mut devices = Vec::new();

    // Use rs-drivelist to get basic disk information
    if let Ok(drives) = rs_drivelist::drive_list() {
        debug!("Found {} drives with rs-drivelist", drives.len());

        for drive in drives {
            // Get device path directly from DeviceDescriptor struct
            let device_path = drive
                .devicePath
                .as_ref()
                .map_or_else(|| drive.device.clone(), |p| p.clone());

            // Format the path for Windows
            let path = if device_path.starts_with(r"\\.\PHYSICALDRIVE") {
                // For physical drives, just use the number
                if let Some(num_str) = device_path.strip_prefix(r"\\.\PHYSICALDRIVE") {
                    num_str.to_string()
                } else {
                    device_path.clone()
                }
            } else {
                // For logical drives (like C:), use as is
                device_path.clone()
            };

            // Format mountpoints for display
            let mountpoint_str = if !drive.mountpoints.is_empty() {
                let mp = &drive.mountpoints[0].path;
                format!(" ({})", mp)
            } else {
                String::new()
            };

            // Create a name for the device
            let name = format!("{}{}", drive.description, mountpoint_str);

            // Determine system disk - for Windows, we specifically check for C: drive
            let is_system = drive.isSystem
                || drive.mountpoints.iter().any(|mp| {
                    mp.path.starts_with("C:")
                        || std::path::Path::new(&format!("{}\\Windows", mp.path)).exists()
                });

            // Add to our list of devices
            devices.push(DiskDevice {
                path,
                name,
                size: drive.size,
                removable: drive.isRemovable,
                readonly: drive.isReadOnly,
                vendor: "Unknown".to_string(), // Not directly available in current version
                model: drive.description.clone(),
                system: is_system,
            });
        }
    } else {
        // Fallback method if rs-drivelist fails
        debug!("rs-drivelist failed, using basic disk enumeration fallback");

        // Try to enumerate all possible physical drives (usually 0-3 for most systems)
        for i in 0..8 {
            let path = format!(r"{}", i);
            // Try to open the disk to check if it exists
            match Disk::lock_path(&path).await {
                Ok(_) => {
                    // Successfully opened, so the disk exists
                    devices.push(DiskDevice {
                        path: path.clone(),
                        name: format!("Disk {}", i),
                        size: 0,          // Unknown size
                        removable: false, // Unknown
                        readonly: false,  // Unknown
                        vendor: "Unknown".to_string(),
                        model: "Unknown".to_string(),
                        system: i == 0, // Assume disk 0 is system disk
                    });
                }
                Err(e) => {
                    // Only log as debug since it's expected that some disks might not exist
                    debug!("Could not open disk {}: {}", i, e);
                }
            }
        }

        // Also add logical drives (C:, D:, etc.)
        for letter in b'C'..=b'Z' {
            let drive_letter = format!("{}:", char::from(letter));
            let path = std::path::Path::new(&drive_letter);
            if path.exists() {
                devices.push(DiskDevice {
                    path: drive_letter.clone(),
                    name: format!("Drive {}", drive_letter),
                    size: 0,          // Unknown size
                    removable: false, // Unknown
                    readonly: false,  // Unknown
                    vendor: "Unknown".to_string(),
                    model: "Unknown".to_string(),
                    system: letter == b'C', // Assume C: is system drive
                });
            }
        }
    }

    Ok(devices)
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

#[derive(Debug)]
pub enum WriteProgress {
    Start,
    Write(u64),
    Finish,
}

/// Helper function to extract string values from TOML lines
/// For example, from a line like: glm_account = "0x1234..."
/// it extracts the value: "0x1234..."
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

#[cfg(target_os = "linux")]
async fn resovle_device(client: &Client, path: &str) -> Result<OwnedObjectPath> {
    let mut spec = HashMap::new();
    spec.insert("path", path.into());
    let mut obj = client
        .manager()
        .resolve_device(spec, HashMap::default())
        .await?;

    Ok(obj.pop().ok_or(anyhow!("no device found"))?)
}

#[cfg(target_os = "linux")]
async fn umount_all(client: &Client, path: ObjectPath<'_>) -> Result<()> {
    for dev_path in client
        .manager()
        .get_block_devices(HashMap::default())
        .await?
    {
        if dev_path.as_str().starts_with(path.as_str()) {
            debug!("Unmounting device: {:?}", dev_path);
            if let Ok(d) = client.object(dev_path)?.filesystem().await {
                if !d.mount_points().await?.is_empty() {
                    d.unmount(HashMap::new()).await?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
/// Helper function to translate Windows error codes to readable messages
fn get_windows_error_message(code: u32) -> &'static str {
    match code {
        0 => "Operation completed successfully",
        1 => "Incorrect function",
        2 => "The system cannot find the file specified",
        3 => "The system cannot find the path specified",
        4 => "The system cannot open the file",
        5 => "Access is denied",
        6 => "The handle is invalid",
        8 => "Not enough memory resources",
        13 => "The data is invalid",
        14 => "Not enough storage is available",
        19 => "Write fault",
        21 => "The device does not recognize the command",
        22 => "Data error (cyclic redundancy check)",
        23 => "The data area passed to a system call is too small",
        32 => "The process cannot access the file because it is in use",
        112 => "There is not enough space on the disk",
        123 => "The filename, directory name, or volume label syntax is incorrect",
        1223 => "The operation was canceled by the user",
        1392 => "The file or directory is corrupted and unreadable",
        _ => "Unknown error code",
    }
}

// Windows-specific implementation for volume dismounting
#[cfg(windows)]
async fn dismount_windows_volume(drive_path: &str) -> Result<()> {
    debug!("Dismounting Windows volume: {}", drive_path);

    // Prepare the path for Windows API
    let drive_path = format!(r"\\.\{}", drive_path.trim_end_matches('\\'));

    // Convert the path to a wide string for Windows API
    let path_wide: Vec<u16> = drive_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // Open the volume with required privileges
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            0,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        let error_code = unsafe { GetLastError() };
        let error_msg = get_windows_error_message(error_code);
        return Err(anyhow!(
            "Failed to open volume, error code: {} ({})",
            error_code,
            error_msg
        ));
    }

    // Lock the volume
    let mut bytes_returned: u32 = 0;
    let lock_result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_LOCK_VOLUME,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if lock_result == 0 {
        let error_code = unsafe { GetLastError() };
        let error_msg = get_windows_error_message(error_code);
        unsafe { CloseHandle(handle) };
        return Err(anyhow!(
            "Failed to lock volume, error code: {} ({})",
            error_code,
            error_msg
        ));
    }

    // Dismount the volume
    let dismount_result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_DISMOUNT_VOLUME,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if dismount_result == 0 {
        let error_code = unsafe { GetLastError() };
        let error_msg = get_windows_error_message(error_code);
        unsafe { CloseHandle(handle) };
        return Err(anyhow!(
            "Failed to dismount volume, error code: {} ({})",
            error_code,
            error_msg
        ));
    }

    // Close the handle
    unsafe { CloseHandle(handle) };

    debug!("Successfully dismounted volume: {}", drive_path);
    Ok(())
}

pub struct Disk {
    file: File,
    // Store disk metadata for later use
    #[cfg(windows)]
    metadata: DiskMetadata,
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct DiskMetadata {
    // Original path used to open the disk
    path: String,
    // Detected sector size of the disk
    sector_size: u32,
}

impl Clone for Disk {
    fn clone(&self) -> Self {
        // This is a simplified implementation - in a real app you would
        // properly handle cloning the file descriptor using dup or similar
        panic!("Disk cannot be cloned in this implementation");
    }
}

impl std::fmt::Debug for Disk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Disk")
            .field("file", &"<file handle>")
            .finish()
    }
}

/// A proxy for a file that provides access to a specific partition
/// by automatically adjusting all seek operations relative to the partition's start offset
/// and ensuring operations stay within the partition boundaries
pub struct PartitionFileProxy {
    /// The underlying file handle for the entire disk
    file: File,
    /// The offset in bytes where the partition starts
    partition_offset: u64,
    /// The size of the partition in bytes
    partition_size: u64,
    /// The current position relative to the start of the partition
    current_position: u64,
    /// The sector size (typically 512 bytes) - important for Windows alignment
    #[cfg(windows)]
    sector_size: u32,
}

impl PartitionFileProxy {
    /// Create a new partition file proxy
    ///
    /// # Arguments
    /// * `file` - File handle to the entire disk
    /// * `partition_offset` - Byte offset where the partition starts
    /// * `partition_size` - Size of the partition in bytes (0 means unknown/unbounded)
    #[cfg(target_os = "linux")]
    pub fn new(file: File, partition_offset: u64, partition_size: u64) -> Self {
        PartitionFileProxy {
            file,
            partition_offset,
            partition_size,
            current_position: 0,
        }
    }

    /// Windows-specific implementation that includes sector size
    /// This constructor variant accepts a sector size parameter
    #[cfg(windows)]
    pub fn new_with_sector_size(
        file: File,
        partition_offset: u64,
        partition_size: u64,
        sector_size: u32,
    ) -> Self {
        debug!(
            "Creating Windows PartitionFileProxy with specified sector size: {} bytes",
            sector_size
        );

        PartitionFileProxy {
            file,
            partition_offset,
            partition_size,
            current_position: 0,
            sector_size,
        }
    }

    /// Windows-specific implementation that uses the default sector size
    /// This maintains the original interface for backward compatibility
    #[cfg(windows)]
    pub fn new(file: File, partition_offset: u64, partition_size: u64) -> Self {
        // Default sector size for when it can't be determined
        const DEFAULT_SECTOR_SIZE: u32 = 512;

        debug!(
            "Creating Windows PartitionFileProxy with default sector size: {} bytes",
            DEFAULT_SECTOR_SIZE
        );

        Self::new_with_sector_size(file, partition_offset, partition_size, DEFAULT_SECTOR_SIZE)
    }

    /// Get the partition offset in bytes
    pub fn partition_offset(&self) -> u64 {
        self.partition_offset
    }

    /// Get the partition size in bytes
    pub fn partition_size(&self) -> u64 {
        self.partition_size
    }

    /// Get the current position relative to the start of the partition
    pub fn position(&self) -> u64 {
        self.current_position
    }

    /// Convert a partition-relative position to an absolute disk position
    fn to_absolute_position(&self, position: u64) -> u64 {
        self.partition_offset + position
    }

    /// Check if a position is within partition boundaries
    fn check_position(&self, position: u64) -> io::Result<()> {
        // If partition_size is 0, we're not enforcing boundaries
        if self.partition_size > 0 && position > self.partition_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Position {} is beyond partition size {}",
                    position, self.partition_size
                ),
            ));
        }
        Ok(())
    }

    #[cfg(windows)]
    /// Check if a buffer is aligned for direct I/O on Windows
    /// This is important because Windows requires buffers to be aligned to sector boundaries
    fn is_buffer_aligned(&self, buf: &[u8]) -> bool {
        // Get buffer address details
        let ptr_addr = buf.as_ptr() as usize;
        let buffer_len = buf.len();
        let sector_size = self.sector_size as usize;

        // Check if the buffer starts at an address that's aligned to the sector size
        let addr_aligned = ptr_addr % sector_size == 0;

        // Check if the buffer length is a multiple of the sector size
        let len_aligned = buffer_len % sector_size == 0;

        let is_aligned = addr_aligned && len_aligned;

        if !is_aligned {
            // Log detailed alignment issues for debugging
            if !addr_aligned {
                info!(
                    "Buffer address misalignment: address 0x{:X} is not aligned to sector size {} (remainder: {})",
                    ptr_addr,
                    sector_size,
                    ptr_addr % sector_size
                );
            }

            if !len_aligned {
                info!(
                    "Buffer length misalignment: length {} is not a multiple of sector size {} (remainder: {})",
                    buffer_len,
                    sector_size,
                    buffer_len % sector_size
                );
            }

            info!(
                "Using aligned buffer for Windows direct I/O (original address: 0x{:X}, length: {}, sector size: {})",
                ptr_addr, buffer_len, sector_size
            );
        }

        is_aligned
    }

    #[cfg(windows)]
    /// Align a buffer to sector boundaries by copying it to an aligned buffer
    /// Returns a new aligned buffer that can be used for direct I/O
    fn create_aligned_buffer(&self, size: usize) -> Vec<u8> {
        use std::alloc::{Layout, alloc};

        // Calculate the sector-aligned size (round up to next sector boundary)
        let sector_size = self.sector_size as usize;
        let aligned_size = ((size + sector_size - 1) / sector_size) * sector_size;

        info!(
            "Creating aligned buffer: requested size: {}, aligned size: {}, sector size: {}",
            size, aligned_size, sector_size
        );

        // Create an aligned layout
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

        // Create a vector that owns this memory
        let mut vec = Vec::with_capacity(aligned_size);
        unsafe {
            vec.set_len(aligned_size);
            std::ptr::copy_nonoverlapping(ptr, vec.as_mut_ptr(), aligned_size);
        }

        // Verify the new buffer's alignment
        let new_ptr_addr = vec.as_ptr() as usize;
        let addr_aligned = new_ptr_addr % sector_size == 0;
        let len_aligned = vec.len() % sector_size == 0;

        if !addr_aligned || !len_aligned {
            error!(
                "Failed to create properly aligned buffer! Address: 0x{:X}, Length: {}, Sector size: {}",
                new_ptr_addr,
                vec.len(),
                sector_size
            );
        } else {
            info!(
                "Successfully created aligned buffer at address 0x{:X}, length: {}",
                new_ptr_addr,
                vec.len()
            );
        }

        vec
    }
}

// Implement Read trait for PartitionFileProxy
#[cfg(target_os = "linux")]
impl Read for PartitionFileProxy {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position(self.current_position)?;

        // Check if we're at the end of the partition
        if self.partition_size > 0 && self.current_position == self.partition_size {
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

        // Use a potentially smaller buffer if needed
        let read_buf = if max_read_size < buf.len() {
            &mut buf[0..max_read_size]
        } else {
            buf
        };

        // Ensure we're at the correct position before reading
        self.file.seek(SeekFrom::Start(
            self.to_absolute_position(self.current_position),
        ))?;

        // Perform the read operation
        let bytes_read = self.file.read(read_buf)?;

        // Update our current position
        self.current_position += bytes_read as u64;

        Ok(bytes_read)
    }
}

// Windows-specific Read implementation that handles buffer alignment
#[cfg(windows)]
impl Read for PartitionFileProxy {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position(self.current_position)?;

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

        // Use a potentially smaller buffer if needed
        let read_size = max_read_size;
        let current_abs_pos = self.to_absolute_position(self.current_position);

        debug!(
            "Windows read operation: buffer len={}, max_read_size={}, absolute position={}",
            buf.len(),
            max_read_size,
            current_abs_pos
        );

        // Determine if we need to use an aligned buffer for Windows direct I/O
        let use_aligned_buffer = !self.is_buffer_aligned(buf);

        let bytes_read = if use_aligned_buffer {
            debug!("Using aligned buffer for Windows read operation");

            // Create an aligned buffer for direct I/O
            let mut aligned_buf = self.create_aligned_buffer(read_size);

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

            // Read into the aligned buffer
            debug!("Reading {} bytes into aligned buffer", read_size);
            let read_result = self.file.read(&mut aligned_buf[0..read_size]);

            let bytes_read = match read_result {
                Ok(bytes) => {
                    debug!("Successfully read {} bytes using aligned buffer", bytes);
                    bytes
                }
                Err(e) => {
                    error!("Windows read error with aligned buffer: {}", e);
                    error!(
                        "Read attempted at position {} with buffer size {}",
                        current_abs_pos, read_size
                    );
                    return Err(e);
                }
            };

            // Copy data from aligned buffer to user buffer
            if bytes_read > 0 {
                debug!(
                    "Copying {} bytes from aligned buffer to user buffer",
                    bytes_read
                );
                buf[0..bytes_read].copy_from_slice(&aligned_buf[0..bytes_read]);
            }

            bytes_read
        } else {
            debug!("Using direct buffer for Windows read operation (buffer already aligned)");

            // If the buffer is already aligned, we can use it directly
            let read_buf = if read_size < buf.len() {
                &mut buf[0..read_size]
            } else {
                buf
            };

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

            // Perform the read operation directly
            debug!("Reading {} bytes directly", read_buf.len());
            let read_result = self.file.read(read_buf);

            match read_result {
                Ok(bytes) => {
                    debug!("Successfully read {} bytes using direct buffer", bytes);
                    bytes
                }
                Err(e) => {
                    error!("Windows read error with direct buffer: {}", e);
                    error!(
                        "Read attempted at position {} with buffer size {}",
                        current_abs_pos,
                        read_buf.len()
                    );
                    return Err(e);
                }
            }
        };

        // Update our current position
        self.current_position += bytes_read as u64;
        debug!(
            "Updated current position to {} after read",
            self.current_position
        );

        Ok(bytes_read)
    }
}

// Implement Write trait for PartitionFileProxy
#[cfg(target_os = "linux")]
impl Write for PartitionFileProxy {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position(self.current_position)?;

        // Check if we're at the end of the partition
        if self.partition_size > 0 && self.current_position == self.partition_size {
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

        // Use a potentially smaller buffer if needed
        let write_buf = if max_write_size < buf.len() {
            &buf[0..max_write_size]
        } else {
            buf
        };

        // Ensure we're at the correct position before writing
        self.file.seek(SeekFrom::Start(
            self.to_absolute_position(self.current_position),
        ))?;

        // Perform the write operation
        let bytes_written = self.file.write(write_buf)?;

        // Update our current position
        self.current_position += bytes_written as u64;

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

// Windows-specific Write implementation that handles buffer alignment
#[cfg(windows)]
impl Write for PartitionFileProxy {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position(self.current_position)?;

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

        // Use a potentially smaller buffer size for the write
        let write_size = max_write_size;
        let current_abs_pos = self.to_absolute_position(self.current_position);

        debug!(
            "Windows write operation: buffer len={}, max_write_size={}, absolute position={}",
            buf.len(),
            max_write_size,
            current_abs_pos
        );

        // Determine if we need to use an aligned buffer for Windows direct I/O
        let use_aligned_buffer = !self.is_buffer_aligned(buf);

        let bytes_written = if use_aligned_buffer {
            debug!("Using aligned buffer for Windows write operation");

            // Create an aligned buffer for direct I/O
            let mut aligned_buf = self.create_aligned_buffer(write_size);

            // Copy data from user buffer to aligned buffer
            aligned_buf[0..write_size].copy_from_slice(&buf[0..write_size]);

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
            debug!("Writing {} bytes from aligned buffer", write_size);
            match self.file.write(&aligned_buf[0..write_size]) {
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
                        current_abs_pos, write_size
                    );
                    return Err(e);
                }
            }
        } else {
            debug!("Using direct buffer for Windows write operation (buffer already aligned)");

            // If the buffer is already aligned, we can use it directly
            let write_buf = if write_size < buf.len() {
                &buf[0..write_size]
            } else {
                buf
            };

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

            // Perform the write operation directly
            debug!("Writing {} bytes directly", write_buf.len());
            match self.file.write(write_buf) {
                Ok(bytes) => {
                    debug!(
                        "Successfully wrote {} bytes to disk using direct buffer",
                        bytes
                    );
                    bytes
                }
                Err(e) => {
                    error!("Windows write error with direct buffer: {}", e);
                    error!(
                        "Write attempted at position {} with buffer size {}",
                        current_abs_pos,
                        write_buf.len()
                    );
                    return Err(e);
                }
            }
        };

        // Update our current position
        self.current_position += bytes_written as u64;
        debug!(
            "Updated current position to {} after write",
            self.current_position
        );

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        debug!("Flushing Windows disk write buffer");
        match self.file.flush() {
            Ok(_) => {
                debug!("Successfully flushed Windows disk write buffer");
                Ok(())
            }
            Err(e) => {
                error!("Failed to flush Windows disk write buffer: {}", e);
                Err(e)
            }
        }
    }
}

// Implement Seek trait for PartitionFileProxy
impl Seek for PartitionFileProxy {
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
        self.check_position(new_position)?;

        // Store the new position - we don't actually seek in the file yet
        // The actual seek happens when we read or write
        self.current_position = new_position;

        Ok(new_position)
    }
}

/// Configuration structure returned by read_configuration
#[derive(Debug)]
pub struct GolemConfig {
    pub payment_network: crate::models::PaymentNetwork,
    pub network_type: crate::models::NetworkType,
    pub subnet: String,
    pub wallet_address: String,
    pub glm_per_hour: String,
}

impl Disk {
    #[cfg(target_os = "linux")]
    pub async fn lock_path(path: &str) -> Result<Self> {
        let client = Client::new().await?;
        let drive_path = resovle_device(&client, path).await?;
        umount_all(&client, drive_path.as_ref())
            .await
            .context("failed to unmount")?;
        let block = client.object(drive_path)?.block().await?;
        let flags = O_EXCL | O_SYNC | O_CLOEXEC;
        let owned_fd = block
            .open_device(
                "rw",
                [("flags", zbus::zvariant::Value::from(flags))]
                    .into_iter()
                    .collect(),
            )
            .await?;
        if let zbus::zvariant::Fd::Owned(owned_fd) = owned_fd.into() {
            let file = std::fs::File::from(owned_fd);
            Ok(Disk { file })
        } else {
            Err(anyhow!("failed to open device"))
        }
    }

    #[cfg(windows)]
    pub async fn lock_path(path: &str) -> Result<Self> {
        info!("Locking Windows disk path: {}", path);

        // Format the path based on whether it's a physical drive or a volume
        let disk_path = if path.contains("PhysicalDrive") {
            // Physical drive already in correct format - use as is with \\.\ prefix
            format!(r"\\.\{}", path.trim_start_matches(r"\\.\"))
        } else if path.ends_with(":") {
            // Drive letter (like "C:") - format as \\.\C:
            format!(r"\\.\{}", path.trim_end_matches('\\'))
        } else if path.parse::<usize>().is_ok() {
            // Just a number - treat as physical drive number
            format!(r"\\.\PhysicalDrive{}", path)
        } else {
            // Assume it's a volume or other device
            format!(r"\\.\{}", path)
        };

        info!("Formatted Windows disk path: {}", disk_path);

        // Note: We should check if user is admin, but for now we'll just warn that
        // administrator privileges are required for Windows disk operations
        warn!("Windows direct disk access typically requires Administrator privileges");

        // Try to dismount all associated volumes
        if path.ends_with(":") {
            // If it's a drive letter (like "C:"), attempt to dismount it
            info!("Attempting to dismount volume: {}", path);
            match dismount_windows_volume(path).await {
                Ok(_) => info!("Successfully dismounted volume: {}", path),
                Err(e) => {
                    // On Windows, dismounting may fail but we can still continue
                    // This is often the case with removable drives
                    warn!(
                        "Failed to dismount volume {}, continuing anyway: {}",
                        path, e
                    );
                }
            }
        } else if path.contains("PhysicalDrive") || path.parse::<usize>().is_ok() {
            // If it's a physical drive, try to enumerate and dismount all its volumes
            // For now we just log a message, but in a full implementation you'd
            // enumerate all volumes and dismount them
            info!("Physical drive specified - attempting to access without dismounting volumes");
            warn!("This may fail if any volumes on this drive are in use by Windows");
            // TODO: Enumerate all volumes on this physical drive and dismount them
        }

        // Try to get the sector size for this disk for better performance
        let sector_size = match get_disk_sector_size(path) {
            Ok(size) => {
                info!("Detected disk sector size: {} bytes", size);
                size
            }
            Err(e) => {
                warn!("Failed to detect sector size, using default: {}", e);
                512 // Default sector size
            }
        };

        // Store disk metadata for later use
        let metadata = DiskMetadata {
            path: path.to_string(),
            sector_size,
        };

        // Convert the path to a wide string for Windows API
        let path_wide: Vec<u16> = disk_path.encode_utf16().chain(std::iter::once(0)).collect();

        // On Windows, try different access modes in order of preference
        // Initialize file handle to invalid value
        let handle;

        info!("Attempting to open Windows disk device: {}", disk_path);

        unsafe {
            let mut error_code: u32;

            // Try several different access combinations, from most to least restrictive
            // This helps handle the variety of Windows configurations

            // 1. Try with GENERIC_READ | GENERIC_WRITE, no sharing
            info!("Attempt 1: Opening with GENERIC_READ | GENERIC_WRITE, exclusive access");
            let h1 = CreateFileW(
                path_wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0, // No sharing - exclusive access
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0, // No special flags for best compatibility
                0,
            );

            // 2. If that fails, try with sharing allowed
            if h1 == INVALID_HANDLE_VALUE {
                error_code = GetLastError();
                let error_msg = get_windows_error_message(error_code);
                warn!(
                    "Attempt 1 failed with error code: {} ({})",
                    error_code, error_msg
                );

                info!(
                    "Attempt 2: Opening with GENERIC_READ | GENERIC_WRITE with FILE_SHARE_READ | FILE_SHARE_WRITE"
                );
                let h2 = CreateFileW(
                    path_wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0, // No special flags
                    0,
                );

                // 3. If that still fails, try with just read access
                if h2 == INVALID_HANDLE_VALUE {
                    error_code = GetLastError();
                    let error_msg = get_windows_error_message(error_code);
                    warn!(
                        "Attempt 2 failed with error code: {} ({})",
                        error_code, error_msg
                    );

                    info!("Attempt 3: Opening with GENERIC_READ only with sharing");
                    let h3 = CreateFileW(
                        path_wide.as_ptr(),
                        GENERIC_READ,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        std::ptr::null_mut(),
                        OPEN_EXISTING,
                        0,
                        0,
                    );

                    // 4. Final attempt with FILE_FLAG_NO_BUFFERING to bypass Windows cache
                    if h3 == INVALID_HANDLE_VALUE {
                        error_code = GetLastError();
                        let error_msg = get_windows_error_message(error_code);
                        warn!(
                            "Attempt 3 failed with error code: {} ({})",
                            error_code, error_msg
                        );

                        info!("Attempt 4: Final attempt with FILE_FLAG_NO_BUFFERING");
                        let h4 = CreateFileW(
                            path_wide.as_ptr(),
                            GENERIC_READ | GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            std::ptr::null_mut(),
                            OPEN_EXISTING,
                            FILE_FLAG_NO_BUFFERING, // Try with direct I/O
                            0,
                        );

                        // If all attempts failed, return a detailed error
                        if h4 == INVALID_HANDLE_VALUE {
                            error_code = GetLastError();
                            let error_msg = get_windows_error_message(error_code);
                            error!(
                                "All open attempts failed, last error code: {} ({})",
                                error_code, error_msg
                            );

                            // Build a detailed error message based on the error code
                            let error_msg = match error_code {
                                5 => format!(
                                    "Access denied for device {}. You must run as Administrator to access disk devices directly.",
                                    disk_path
                                ),
                                32 => format!(
                                    "The device {} is in use by another process. Close any applications that may be using this disk.",
                                    disk_path
                                ),
                                2 => format!(
                                    "The device {} was not found. Verify the disk exists and is connected properly.",
                                    disk_path
                                ),
                                123 => format!(
                                    "Invalid filename syntax for {}. Windows requires specific format for disk devices.",
                                    disk_path
                                ),
                                _ => format!(
                                    "Failed to open device {}, Windows error code: {} ({}). This might be a permissions issue or the device is in use.",
                                    disk_path, error_code, error_msg
                                ),
                            };

                            error!("{}", error_msg);
                            return Err(anyhow!(error_msg));
                        }

                        handle = h4;
                    } else {
                        handle = h3;
                    }
                } else {
                    handle = h2;
                }
            } else {
                handle = h1;
            }
        }

        info!("Successfully opened Windows disk device: {}", disk_path);

        // Convert Windows HANDLE to Rust File
        let file = unsafe { std::fs::File::from_raw_handle(handle as *mut _) };
        Ok(Disk { file, metadata })
    }

    /// Write an image file to the disk with progress reporting
    ///
    /// # Arguments
    /// * `image_path` - The path to the OS image file to write
    /// * `progress_callback` - Callback function to report progress (0.0 to 1.0)
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Error on failure
    pub fn write_image(
        &mut self,
        image_path: &str,
    ) -> impl Sipper<Result<WriteProgress>, WriteProgress> + Send + 'static {
        debug!("Opening image file: {}", image_path);
        let image_file_r = File::open(image_path)
            .with_context(|| format!("Failed to open image file: {}", image_path));

        // Use a larger buffer for better performance
        const BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer

        let disk_file_r = self.get_cloned_file_handle();
        task::sipper(async move |mut sipper| -> Result<WriteProgress> {
            let image_file = BufReader::with_capacity(BUFFER_SIZE, image_file_r?);
            let _size = image_file.get_ref().metadata()?.len();

            // Create a buffered writer with our disk file handle
            let mut disk_file = BufWriter::with_capacity(BUFFER_SIZE, disk_file_r?);

            sipper.send(WriteProgress::Start).await;

            // Use blocking task for I/O operations to avoid blocking the async runtime
            let r = tokio::task::spawn_blocking(move || {
                // Additional Windows-specific error handling
                #[cfg(windows)]
                {
                    info!("Windows: Starting image write operation");
                    // Verify disk is ready for writing
                    let metadata = disk_file.get_ref().metadata();
                    if let Err(e) = metadata {
                        error!("Windows disk error: Failed to get disk metadata: {}", e);
                        return Err(anyhow::anyhow!("Windows disk error: Failed to get disk metadata: {}", e)
                            .context("Disk device is not accessible for writing. Make sure you're running as Administrator.")
                            .into());
                    }
                    
                    // Check basic disk access permissions
                    let metadata_result = disk_file.get_ref().metadata();
                    if let Err(e) = &metadata_result {
                        error!("Windows disk error: Cannot get disk metadata: {}", e);
                        let os_err = e.raw_os_error();
                        if let Some(code) = os_err {
                            let msg = get_windows_error_message(code as u32);
                            error!("Windows error code: {} ({})", code, msg);
                        }
                        
                        if metadata_result.is_err() {
                            return Err(anyhow::anyhow!("Failed to access disk metadata: {}", e)
                                .context("Make sure you're running as Administrator and the disk is accessible")
                                .into());
                        }
                    }
                    
                    // Check if we can write to the disk by attempting a zero-byte write
                    // This is safer than trying to use GetFileAttributesW on a device handle
                    match disk_file.get_ref().try_clone() {
                        Ok(mut test_file) => {
                            let write_test = test_file.write(&[]);
                            if let Err(e) = write_test {
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    error!("Windows disk error: Disk is write-protected, permission denied");
                                    return Err(anyhow::anyhow!("The disk is write-protected and cannot be written to")
                                        .context("Remove write protection from the device or use a different device")
                                        .into());
                                } else {
                                    warn!("Write test failed: {}", e);
                                    warn!("Continuing with caution, but write operation may fail later");
                                }
                            }
                        },
                        Err(e) => {
                            warn!("Could not clone file handle for write test: {}", e);
                            warn!("Continuing with caution, but write operation may fail later");
                        }
                    }
                    
                    info!("Windows: Disk is ready for writing");
                }
                
                // Create XZ reader with our tracked file
                let buffer_size = NonZeroUsize::new(8_00_000usize).unwrap();
                info!("Creating XZ reader with buffer size: {} bytes", buffer_size);
                
                // XzReader::new_with_buffer_size doesn't return a Result, it returns directly XzReader
                let mut source_file = XzReader::new_with_buffer_size(
                    image_file,
                    buffer_size,
                );

                info!("Starting to copy decompressed image data to disk");
                
                // Copy decompressed data to disk
                let copy_result = io::copy(&mut source_file, &mut disk_file);
                if let Err(e) = &copy_result {
                    error!("Failed to write image to disk: {}", e);
                    
                    #[cfg(windows)]
                    {
                        // Provide more specific error messages for common Windows errors
                        let os_error = e.raw_os_error();
                        
                        // Log error details
                        if let Some(code) = os_error {
                            let error_msg = get_windows_error_message(code as u32);
                            error!("Windows error code: {} ({})", code, error_msg);
                            
                            match code {
                                5 => {
                                    error!("Access denied error (code 5) when writing to disk");
                                    return Err(anyhow::anyhow!("Access denied when writing to disk. Error code: 5 ({})", error_msg)
                                        .context("Make sure you're running with Administrator privileges")
                                        .context("The disk may be locked by another process or write-protected")
                                        .into());
                                },
                                1117 => {
                                    error!("I/O device error (code 1117) when writing to disk");
                                    return Err(anyhow::anyhow!("The request could not be performed because of an I/O device error. Error code: 1117 ({})", error_msg)
                                        .context("The disk may be write-protected, damaged, or have hardware issues")
                                        .context("Try using a different USB port or disk")
                                        .into());
                                },
                                112 => {
                                    error!("Not enough space error (code 112) when writing to disk");
                                    return Err(anyhow::anyhow!("There is not enough space on the disk. Error code: 112 ({})", error_msg)
                                        .context("Check that the disk has enough free space for the image")
                                        .context("Try using a larger capacity disk")
                                        .into());
                                },
                                1224 => {
                                    error!("Removed media error (code 1224) when writing to disk");
                                    return Err(anyhow::anyhow!("The disk was removed during the write operation. Error code: 1224 ({})", error_msg)
                                        .context("The disk was disconnected during the write operation")
                                        .context("Ensure the disk remains connected throughout the process")
                                        .into());
                                },
                                87 => {
                                    error!("Invalid parameter error (code 87) when writing to disk");
                                    return Err(anyhow::anyhow!("The parameter is incorrect. Error code: 87 ({})", error_msg)
                                        .context("This may be due to mismatched buffer alignment requirements")
                                        .context("Try restarting the application and using a different USB port")
                                        .into());
                                },
                                _ => {
                                    error!("Unrecognized Windows error code: {} ({})", code, error_msg);
                                    return Err(anyhow::anyhow!("Failed to write image to disk. Windows error code: {} ({})", code, error_msg)
                                        .context("An unexpected Windows error occurred during disk write")
                                        .context("Try restarting your computer and running the application as Administrator")
                                        .into());
                                },
                            }
                        } else {
                            error!("No specific Windows error code available");
                            return Err(anyhow::anyhow!("Failed to write image to disk: {}", e)
                                .context("No specific Windows error code was provided")
                                .context("Make sure you're running as Administrator and the disk is accessible")
                                .into());
                        }
                    }
                    
                    #[cfg(not(windows))]
                    {
                        return Err(anyhow::anyhow!("Failed to write image to disk: {}", e).into());
                    }
                }
                
                info!("Image data copy completed, flushing disk buffers");
                
                // Ensure all data is written to disk
                let flush_result = disk_file.flush();
                if let Err(e) = flush_result {
                    error!("Failed to flush disk buffer: {}", e);
                    
                    #[cfg(windows)]
                    {
                        let os_error = e.raw_os_error();
                        if let Some(code) = os_error {
                            let error_msg = get_windows_error_message(code as u32);
                            error!("Windows flush error code: {} ({})", code, error_msg);
                        } else {
                            error!("Windows flush error with no specific error code");
                        }
                    }
                    
                    return Err(anyhow::anyhow!("Failed to complete disk write operation during flush: {}", e)
                        .context("Unable to ensure all data was written to disk")
                        .context("The disk may have been disconnected or experienced an error")
                        .into());
                }
                
                info!("Successfully wrote image to disk - operation complete");
                anyhow::Ok(WriteProgress::Finish)
            })
            .await?;

            r
        })
    }

    /// Read Golem configuration from a partition
    ///
    /// # Arguments
    /// * `uuid_str` - The target partition UUID (e.g. "33b921b8-edc5-46a0-8baa-d0b7ad84fc71")
    ///
    /// # Returns
    /// * `Result<GolemConfig>` - Configuration values on success, Error on failure
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
                    if let Some(value) = extract_toml_string_value(line) {
                        config.wallet_address = value;
                    }
                } else if line.starts_with("glm_per_hour") {
                    // Extract rate
                    if let Some(value) = extract_toml_string_value(line) {
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

    /// Find a partition with the specified UUID and return a FAT filesystem
    ///
    /// # Arguments
    /// * `uuid` - The UUID to search for (e.g. "33b921b8-edc5-46a0-8baa-d0b7ad84fc71")
    ///
    /// # Returns
    /// * `Result<fatfs::FileSystem<File>>` - A FAT filesystem object on success
    pub fn find_partition(
        &mut self,
        uuid_str: &str,
    ) -> Result<fatfs::FileSystem<PartitionFileProxy>> {
        // Find the partition with the given UUID, but don't format if it has issues
        self.find_or_create_partition(uuid_str, false)
    }

    /// Find a partition with the specified UUID and return a FAT filesystem,
    /// formatting the partition if necessary (e.g., in case of "Invalid total_sectors_16 value in BPB" error)
    ///
    /// # Arguments
    /// * `uuid_str` - The UUID to search for (e.g. "33b921b8-edc5-46a0-8baa-d0b7ad84fc71")
    /// * `format_if_needed` - If true, formats the partition if it can't be read
    ///
    /// # Returns
    /// * `Result<fatfs::FileSystem<PartitionFileProxy>>` - A FAT filesystem object on success
    pub fn find_or_create_partition(
        &mut self,
        uuid_str: &str,
        format_if_needed: bool,
    ) -> Result<fatfs::FileSystem<PartitionFileProxy>> {
        // Parse the provided UUID string
        let target_uuid = Uuid::parse_str(uuid_str)
            .context(format!("Failed to parse UUID string: {}", uuid_str))?;

        // Create a GPT configuration with the default logical block size (usually 512 bytes)
        let cfg = GptConfig::new().writable(false);

        // Clone the file handle
        // Note: We need to ensure we have a separate file handle for each operation
        // to avoid seek position conflicts
        let file_for_gpt = self.get_cloned_file_handle()?;

        // Parse GPT header and partition table from the disk
        #[cfg(windows)]
        debug!("Attempting to read GPT partition table on Windows");

        let disk_result = cfg.open_from_device(Box::new(file_for_gpt));

        // Handle potential GPT reading errors more gracefully
        let disk = match disk_result {
            Ok(disk) => disk,
            Err(e) => {
                #[cfg(windows)]
                {
                    // Windows-specific handling for GPT reading errors
                    warn!(
                        "Failed to parse GPT partition table: {}. This may be due to insufficient permissions.",
                        e
                    );

                    // On Windows, attempt to reopen the device with different flags
                    debug!("Attempting to reopen disk with different access mode");

                    // Get a fresh file handle with different flags
                    self.file = unsafe {
                        // Close the existing handle to release any locks
                        let _ = self.file.flush();

                        // We need to create the path again to reopen it
                        let disk_path = if cfg!(windows) {
                            // For Windows, we need the \\.\PhysicalDrive* format
                            let path_str = format!(
                                r"\\.\PhysicalDrive{}",
                                // Extract drive number if possible, otherwise use 0
                                uuid_str
                                    .chars()
                                    .filter(|c| c.is_digit(10))
                                    .collect::<String>()
                            );
                            path_str
                        } else {
                            // For non-Windows, just return the UUID
                            uuid_str.to_string()
                        };

                        // On Windows, convert to wide chars for API call
                        let path_wide: Vec<u16> =
                            disk_path.encode_utf16().chain(std::iter::once(0)).collect();

                        // Attempt to open with SHARE_READ | SHARE_WRITE which often works better on Windows
                        let h = CreateFileW(
                            path_wide.as_ptr(),
                            GENERIC_READ | GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            std::ptr::null_mut(),
                            OPEN_EXISTING,
                            0, // No flags for better compatibility
                            0,
                        );

                        if h == INVALID_HANDLE_VALUE {
                            let error_code = GetLastError();
                            return Err(anyhow!(
                                "Failed to reopen device {}, error code: {}",
                                disk_path,
                                error_code
                            )
                            .context("Make sure you're running as Administrator")
                            .context(format!("Original error: {}", e)));
                        }

                        // Convert to Rust File
                        std::fs::File::from_raw_handle(h as *mut _)
                    };

                    // Try to open the GPT again with the new handle
                    let file_for_gpt = self
                        .get_cloned_file_handle()
                        .context("Failed to clone file handle after reopening disk")?;

                    let cfg = GptConfig::new().writable(false);

                    match cfg.open_from_device(Box::new(file_for_gpt)) {
                        Ok(disk) => disk,
                        Err(e2) => {
                            return Err(anyhow!("Failed to parse GPT partition table on second attempt: {}", e2)
                                .context("Make sure you run as Administrator and the disk has a valid GPT table")
                                .context(format!("Original error: {}", e)));
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    // Standard error handling for other platforms
                    return Err(anyhow!("Failed to parse GPT partition table: {}", e));
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
                let start_sector = part.first_lba as u64;
                // Standard sector size is 512 bytes for most disks
                const SECTOR_SIZE: u64 = 512;
                let start_offset = start_sector * SECTOR_SIZE;

                // Create a new file handle for the FAT filesystem
                let partition_file = self.get_cloned_file_handle()?;

                // Get partition size for better boundary checking
                let partition_size = part
                    .last_lba
                    .checked_sub(part.first_lba)
                    .map(|sectors| sectors as u64 * SECTOR_SIZE)
                    .unwrap_or(0);

                debug!(
                    "Partition size: {} bytes ({} MB)",
                    partition_size,
                    partition_size / (1024 * 1024)
                );

                // Create a PartitionFileProxy that will handle seeks relative to the partition start
                // and respect partition boundaries
                #[cfg(not(windows))]
                let proxy = PartitionFileProxy::new(partition_file, start_offset, partition_size);

                // On Windows, use the detected sector size
                #[cfg(windows)]
                let proxy = PartitionFileProxy::new_with_sector_size(
                    partition_file,
                    start_offset,
                    partition_size,
                    self.sector_size(), // Get sector size from disk metadata
                );

                // Attempt to create a FAT filesystem from the partition using our proxy
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

                                // Create formatting proxy with appropriate sector size handling
                                #[cfg(not(windows))]
                                let format_proxy = PartitionFileProxy::new(
                                    format_file,
                                    start_offset,
                                    partition_size,
                                );

                                #[cfg(windows)]
                                let format_proxy = PartitionFileProxy::new_with_sector_size(
                                    format_file,
                                    start_offset,
                                    partition_size,
                                    self.sector_size(),
                                );

                                // Format the partition
                                // Use default format options which will select appropriate FAT type based on size
                                // instead of forcing FAT32 which might be too large
                                debug!("Using format options with volume label GOLEMCONF");
                                fatfs::format_volume(
                                    format_proxy,
                                    fatfs::FormatVolumeOptions::new().volume_label(*b"GOLEMCONF  "), // 11 bytes padded with spaces
                                )?;

                                debug!("Successfully formatted partition");

                                // Now try to open the freshly formatted filesystem
                                let new_file = self.get_cloned_file_handle()?;

                                // Create a new proxy with appropriate sector size handling
                                #[cfg(not(windows))]
                                let new_proxy =
                                    PartitionFileProxy::new(new_file, start_offset, partition_size);

                                #[cfg(windows)]
                                let new_proxy = PartitionFileProxy::new_with_sector_size(
                                    new_file,
                                    start_offset,
                                    partition_size,
                                    self.sector_size(),
                                );
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

    /// Helper method to create a cloned file handle to the disk
    /// This is needed because we can't directly clone the File (it doesn't implement Clone)
    /// But we need separate file handles for different operations to avoid seek conflicts
    #[cfg(target_os = "linux")]
    fn get_cloned_file_handle(&self) -> Result<File> {
        // First, get the file descriptor from our existing file
        use std::os::unix::io::{AsRawFd, FromRawFd};
        let fd = self.file.as_raw_fd();

        // Duplicate the file descriptor
        let new_fd = unsafe { libc::dup(fd) };
        if new_fd < 0 {
            return Err(anyhow!("Failed to duplicate file descriptor"));
        }

        // Create a new File from the duplicated descriptor
        let new_file = unsafe { File::from_raw_fd(new_fd) };

        Ok(new_file)
    }

    /// Windows implementation of get_cloned_file_handle
    /// Uses DuplicateHandle to create a new handle to the same file
    #[cfg(windows)]
    fn get_cloned_file_handle(&self) -> Result<File> {
        let current_process = unsafe { GetCurrentProcess() };
        let mut target_handle: HANDLE = 0;

        let success = unsafe {
            DuplicateHandle(
                current_process,
                self.file.as_raw_handle() as HANDLE,
                current_process,
                &mut target_handle,
                0,
                0, // FALSE for inherit handle
                DUPLICATE_SAME_ACCESS,
            )
        };

        if success == 0 {
            let error_code = unsafe { GetLastError() };
            return Err(anyhow!(
                "Failed to duplicate file handle, error code: {}",
                error_code
            ));
        }

        // Convert the Windows HANDLE back to a Rust File
        let new_file = unsafe { File::from_raw_handle(target_handle as *mut _) };

        Ok(new_file)
    }

    /// Get the disk path that was originally used to open this disk
    #[cfg(windows)]
    pub fn path(&self) -> &str {
        &self.metadata.path
    }

    /// Get the detected sector size for this disk
    #[cfg(windows)]
    pub fn sector_size(&self) -> u32 {
        self.metadata.sector_size
    }

    /// Get a PartitionFileProxy for the specified partition UUID
    ///
    /// # Arguments
    /// * `uuid_str` - UUID string of the partition to find
    ///
    /// # Returns
    /// * `Result<(PartitionFileProxy, String)>` - The proxy and partition name on success
    pub fn get_partition_proxy(&mut self, uuid_str: &str) -> Result<(PartitionFileProxy, String)> {
        // Parse the provided UUID string
        let target_uuid = Uuid::parse_str(uuid_str)
            .context(format!("Failed to parse UUID string: {}", uuid_str))?;

        // Create a GPT configuration
        let cfg = GptConfig::new().writable(false);

        // Clone the file handle for GPT parsing
        let file_for_gpt = self.get_cloned_file_handle()?;

        // Parse GPT header and partition table
        let disk = cfg
            .open_from_device(Box::new(file_for_gpt))
            .context("Failed to parse GPT partition table")?;

        // Get partitions
        let partitions = disk.partitions();

        // Find partition with matching UUID
        for (_, part) in partitions.iter() {
            if part.part_guid == target_uuid {
                // Get partition start offset
                let start_sector = part.first_lba as u64;
                const SECTOR_SIZE: u64 = 512;
                let start_offset = start_sector * SECTOR_SIZE;

                // Get partition size
                let partition_size = part
                    .last_lba
                    .checked_sub(part.first_lba)
                    .map(|sectors| sectors as u64 * SECTOR_SIZE)
                    .unwrap_or(0);

                // Create a new file handle
                let partition_file = self.get_cloned_file_handle()?;

                // Create and return the proxy with appropriate sector size handling
                #[cfg(not(windows))]
                let proxy = PartitionFileProxy::new(partition_file, start_offset, partition_size);

                // On Windows, use the detected sector size from disk metadata
                #[cfg(windows)]
                let proxy = PartitionFileProxy::new_with_sector_size(
                    partition_file,
                    start_offset,
                    partition_size,
                    self.sector_size(),
                );

                return Ok((proxy, part.name.clone()));
            }
        }

        Err(anyhow!("No partition found with UUID: {}", uuid_str))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_find() -> Result<()> {
    let _disk = Disk::lock_path("/dev/sda").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_find_partition_with_uuid() -> Result<()> {
    // Create a disk instance using any device path (we'll be searching for the UUID, not using this device)
    // Normally you'd use a real device like "/dev/sda", but for testing we can use any base device
    let mut disk = Disk::lock_path("/dev/sda").await?;

    // The specific UUID we're looking for
    let target_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

    // Find the partition and get the FAT filesystem
    let fs = disk.find_partition(target_uuid)?;

    // Get the root directory of the filesystem
    let root_dir = fs.root_dir();

    // List all files in the root directory
    info!(
        "Files in the root directory of partition with UUID {}:",
        target_uuid
    );
    for entry in root_dir.iter() {
        let entry = entry?;
        let name = entry.file_name();

        if entry.is_dir() {
            debug!("  Directory: {}", name);
        } else {
            let size = entry.len();
            debug!("  File: {} (size: {} bytes)", name, size);

            // If it's a text file, read and print its contents (for small files only)
            if name.ends_with(".txt") && size < 10240 {
                // Less than 10KB
                let mut file = root_dir.open_file(&name)?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                trace!("    Contents: {}", contents);
            }
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_partition_file_proxy() -> Result<()> {
    // Lock a disk device
    let mut disk = Disk::lock_path("/dev/sda").await?;

    // Get a partition proxy for a specific UUID
    let target_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";
    let (mut proxy, partition_name) = disk.get_partition_proxy(target_uuid)?;

    info!(
        "Found partition '{}' with UUID: {}",
        partition_name, target_uuid
    );
    info!(
        "Partition starts at offset: {} bytes, size: {} bytes",
        proxy.partition_offset(),
        proxy.partition_size()
    );

    // Read the first 512 bytes of the partition (the boot sector for FAT)
    let mut buffer = [0u8; 512];
    proxy.seek(SeekFrom::Start(0))?; // Seek to start of partition (not disk)
    let bytes_read = proxy.read(&mut buffer)?;

    info!("Read {} bytes from the start of the partition", bytes_read);
    debug!("First 16 bytes: {:02X?}", &buffer[0..16]);

    // Identify if it's a FAT filesystem by checking for FAT signatures
    if buffer[510] == 0x55 && buffer[511] == 0xAA {
        info!("Valid boot sector signature (0x55AA) found");

        // Check FAT type by bytes per sector and other parameters
        let bytes_per_sector = u16::from_le_bytes([buffer[11], buffer[12]]);
        let sectors_per_cluster = buffer[13];
        let reserved_sectors = u16::from_le_bytes([buffer[14], buffer[15]]);

        debug!("Filesystem parameters:");
        debug!("  Bytes per sector: {}", bytes_per_sector);
        debug!("  Sectors per cluster: {}", sectors_per_cluster);
        debug!("  Reserved sectors: {}", reserved_sectors);
        proxy.seek(SeekFrom::Start(0))?;

        // Create a FAT filesystem from the proxy
        let fs = fatfs::FileSystem::new(proxy, fatfs::FsOptions::new())?;

        // Get and display volume label if any
        let volume_label = fs.volume_label();
        info!("Volume label: {}", volume_label);

        // Now use the filesystem to list root directory
        let root_dir = fs.root_dir();
        info!("Files in root directory:");

        for entry_result in root_dir.iter() {
            let entry = entry_result?;
            let name = entry.file_name();
            if entry.is_dir() {
                debug!("  Directory: {}", name);
            } else {
                let size = entry.len();
                debug!("  File: {} (size: {} bytes)", name, size);
            }
        }
    } else {
        warn!(
            "Invalid boot sector signature: found {:02X}{:02X} - expected 55AA",
            buffer[510], buffer[511]
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_write_configuration() -> Result<()> {
    // Create a disk instance
    let mut disk = Disk::lock_path("/dev/sda").await?;

    // The target UUID of the config partition
    let config_partition_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

    // Test configuration values
    let payment_network = crate::models::PaymentNetwork::Testnet;
    let network_type = crate::models::NetworkType::Central;
    let subnet = "susteen";
    let wallet_address = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e";

    // Write the configuration
    disk.write_configuration(
        config_partition_uuid,
        payment_network,
        network_type,
        subnet,
        wallet_address,
    )?;

    info!("Configuration written successfully!");

    // Now verify the files were created correctly by reading them back
    let fs = disk.find_partition(config_partition_uuid)?;
    let root_dir = fs.root_dir();

    // Check if golemwz.toml exists and has the correct content
    if let Ok(mut toml_file) = root_dir.open_file("golemwz.toml") {
        let mut toml_content = String::new();
        toml_file.read_to_string(&mut toml_content)?;
        debug!("golemwz.toml content:\n{}", toml_content);

        // Verify the wallet address was written correctly
        assert!(
            toml_content.contains(&format!("glm_account = \"{}\"", wallet_address)),
            "Wallet address not found in golemwz.toml"
        );
    } else {
        return Err(anyhow!("golemwz.toml file not found"));
    }

    // Check if golem.env exists and has the correct content
    if let Ok(mut env_file) = root_dir.open_file("golem.env") {
        let mut env_content = String::new();
        env_file.read_to_string(&mut env_content)?;
        debug!("golem.env content:\n{}", env_content);

        // Verify key settings were written correctly
        assert!(
            env_content.contains("YA_NET_TYPE=central"),
            "Network type not found or incorrect in golem.env"
        );
        assert!(
            env_content.contains(&format!("SUBNET={}", subnet)),
            "Subnet setting not found or incorrect in golem.env"
        );
        assert!(
            env_content.contains("YA_PAYMENT_NETWORK_GROUP=testnet"),
            "Payment network not found or incorrect in golem.env"
        );
    } else {
        return Err(anyhow!("golem.env file not found"));
    }

    info!("Verification complete - configuration written correctly!");
    Ok(())
}

/// Windows-specific function to get a disk's sector size
#[cfg(windows)]
pub fn get_disk_sector_size(disk_path: &str) -> Result<u32> {
    // Default sector size if we can't determine it
    const DEFAULT_SECTOR_SIZE: u32 = 512;

    debug!("Getting sector size for Windows disk: {}", disk_path);

    // Format the path for Windows API
    let formatted_path = if disk_path.contains("PhysicalDrive") {
        format!(r"\\.\{}", disk_path.trim_start_matches(r"\\.\"))
    } else if disk_path.ends_with(":") {
        format!(r"\\.\{}", disk_path.trim_end_matches('\\'))
    } else if disk_path.parse::<usize>().is_ok() {
        format!(r"\\.\PhysicalDrive{}", disk_path)
    } else {
        format!(r"\\.\{}", disk_path)
    };

    debug!("Formatted disk path: {}", formatted_path);

    // Convert to wide string for Windows API
    let path_wide: Vec<u16> = formatted_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // Open the disk with read access
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            0,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        let error_code = unsafe { GetLastError() };
        let error_msg = get_windows_error_message(error_code);
        debug!(
            "Failed to open disk for sector size query, error code: {} ({})",
            error_code, error_msg
        );
        return Ok(DEFAULT_SECTOR_SIZE); // Return default on error
    }

    // Structure for disk geometry information
    #[repr(C)]
    struct DiskGeometry {
        cylinders: i64,
        media_type: u32,
        tracks_per_cylinder: u32,
        sectors_per_track: u32,
        bytes_per_sector: u32,
    }

    let mut disk_geometry = DiskGeometry {
        cylinders: 0,
        media_type: 0,
        tracks_per_cylinder: 0,
        sectors_per_track: 0,
        bytes_per_sector: 0,
    };

    let mut bytes_returned: u32 = 0;

    // Get disk geometry information
    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_DRIVE_GEOMETRY, // 0x70000
            std::ptr::null_mut(),
            0,
            &mut disk_geometry as *mut _ as *mut _,
            std::mem::size_of::<DiskGeometry>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    // Close the handle
    unsafe { CloseHandle(handle) };

    if result == 0 || bytes_returned == 0 {
        let error_code = unsafe { GetLastError() };
        let error_msg = get_windows_error_message(error_code);
        debug!(
            "DeviceIoControl failed, error code: {} ({}), returning default sector size",
            error_code, error_msg
        );
        return Ok(DEFAULT_SECTOR_SIZE);
    }

    let sector_size = disk_geometry.bytes_per_sector;
    if sector_size == 0 {
        debug!("Got zero sector size, using default");
        Ok(DEFAULT_SECTOR_SIZE)
    } else {
        debug!("Detected sector size: {} bytes", sector_size);
        Ok(sector_size)
    }
}

/// Test the disk sector size function on Windows
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_get_disk_sector_size() -> Result<()> {
    // Test with a physical drive
    let disk_path = "0"; // Usually the boot drive
    let sector_size = get_disk_sector_size(disk_path)?;
    info!("Disk {} sector size: {} bytes", disk_path, sector_size);

    // Test with a logical drive
    let drive_path = "C:";
    let drive_sector_size = get_disk_sector_size(drive_path)?;
    info!(
        "Drive {} sector size: {} bytes",
        drive_path, drive_sector_size
    );

    Ok(())
}

/// Test Windows disk operations and capabilities
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_windows_disk_operations() -> Result<()> {
    // First, list all available disks
    info!("Listing available Windows disks:");
    let disks = list_available_disks().await?;

    for (i, disk) in disks.iter().enumerate() {
        info!(
            "Disk {}: {} - Size: {} bytes, Removable: {}, System: {}",
            i, disk.name, disk.size, disk.removable, disk.system
        );
    }

    // Skip if no disks available
    if disks.is_empty() {
        info!("No disks found, skipping test");
        return Ok(());
    }

    // Find a non-system disk for testing or use a drive letter
    let test_disk = disks
        .iter()
        .find(|d| !d.system && !d.path.contains("C:"))
        .or_else(|| disks.iter().find(|_| true))
        .ok_or_else(|| anyhow!("No suitable disk found for testing"))?;

    info!(
        "Selected test disk: {} ({})",
        test_disk.name, test_disk.path
    );

    // Try opening the disk
    info!("Opening disk: {}", test_disk.path);
    let result = Disk::lock_path(&test_disk.path).await;

    match result {
        Ok(_) => {
            info!("Successfully opened disk: {}", test_disk.path);

            // Check sector size
            let sector_size = get_disk_sector_size(&test_disk.path)?;
            info!("Disk sector size: {} bytes", sector_size);

            // Test GPT partition listing if this isn't a logical drive (C:, D:, etc.)
            if !test_disk.path.ends_with(":") {
                info!("Attempting to read GPT partition table");
                let mut disk = Disk::lock_path(&test_disk.path).await?;

                // Clone the file handle
                let file_for_gpt = disk.get_cloned_file_handle()?;

                // Parse GPT header and partition table from the disk
                let cfg = GptConfig::new().writable(false);
                match cfg.open_from_device(Box::new(file_for_gpt)) {
                    Ok(disk_gpt) => {
                        // Get partitions from the disk
                        let partitions = disk_gpt.partitions();

                        if partitions.is_empty() {
                            info!("No GPT partitions found on disk");
                        } else {
                            info!("Found {} GPT partitions:", partitions.len());

                            for (i, (_, part)) in partitions.iter().enumerate() {
                                info!("Partition {}: {} - UUID: {}", i, part.name, part.part_guid);

                                let start_sector = part.first_lba as u64;
                                let end_sector = part.last_lba as u64;
                                let size_sectors = end_sector - start_sector;
                                let size_bytes = size_sectors * 512;

                                info!(
                                    "  Start: sector {}, End: sector {}, Size: {} MB",
                                    start_sector,
                                    end_sector,
                                    size_bytes / (1024 * 1024)
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read GPT partition table: {}", e);
                        info!("This is expected for logical drives and some removable media");
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to open disk {}: {}", test_disk.path, e);
            info!("This is expected without administrator privileges");
        }
    }

    Ok(())
}
