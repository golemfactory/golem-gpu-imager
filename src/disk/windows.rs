// Windows-specific disk operations

use crate::disk::common::{DiskDevice, PartitionFileProxy};
use anyhow::{Result, anyhow};
// GptConfig is used in handle_gpt_error implementations
use std::fs::File;
use std::io::{self, Read, Write, Seek};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use tracing::{debug, error, info, warn};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::*;
use windows_sys::Win32::System::Threading::*;

// Sector size constants for Windows
// Standard sector size for most disks
const DEFAULT_SECTOR_SIZE: u32 = 512;
// For advanced format drives and most modern physical disks
const PHYSICAL_SECTOR_SIZE: u32 = 4096;

/// Windows-specific disk access functionality
#[derive(Debug, Clone)]
pub struct WindowsDiskAccess {
    // Original path used to open the disk
    path: String,
    // Detected sector size of the disk
    sector_size: u32,
}

impl WindowsDiskAccess {
    /// Open and lock a disk by its path
    pub async fn lock_path(path: &str) -> Result<(File, Self)> {
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
            match Self::dismount_volume(path).await {
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
        let sector_size = match Self::get_disk_sector_size(path) {
            Ok(size) => {
                info!("Detected disk sector size: {} bytes", size);
                size
            }
            Err(e) => {
                warn!("Failed to detect sector size, using default: {}", e);
                DEFAULT_SECTOR_SIZE // Default sector size
            }
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

            // Check if this is a physical drive (for flags selection)
            let is_physical_drive = disk_path.contains("PhysicalDrive") || path.parse::<usize>().is_ok();
            let direct_io_flags = if is_physical_drive {
                // Use direct I/O flags for physical drives - requires aligned I/O
                // Note: Both NO_BUFFERING and WRITE_THROUGH are needed for physical drives
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH | FILE_FLAG_SEQUENTIAL_SCAN
            } else {
                // No special flags for logical drives
                0
            };
            
            // Log the flags we're using
            if is_physical_drive {
                info!("Using direct I/O flags (NO_BUFFERING | WRITE_THROUGH) for physical drive");
            }
            
            // 1. Try with GENERIC_READ | GENERIC_WRITE, no sharing, and direct I/O if needed
            info!("Attempt 1: Opening with GENERIC_READ | GENERIC_WRITE, exclusive access");
            let h1 = CreateFileW(
                path_wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0, // No sharing - exclusive access
                std::ptr::null_mut(),
                OPEN_EXISTING,
                direct_io_flags, // Use direct I/O for physical drives
                0,
            );

            // 2. If that fails, try with sharing allowed
            if h1 == INVALID_HANDLE_VALUE {
                error_code = GetLastError();
                let error_msg = Self::get_windows_error_message(error_code);
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
                    direct_io_flags, // Use direct I/O for physical drives
                    0,
                );

                // 3. If that still fails, try with just read access
                if h2 == INVALID_HANDLE_VALUE {
                    error_code = GetLastError();
                    let error_msg = Self::get_windows_error_message(error_code);
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
                        direct_io_flags, // Use direct I/O for physical drives
                        0,
                    );

                    // 4. Final attempt with FILE_FLAG_NO_BUFFERING to bypass Windows cache
                    if h3 == INVALID_HANDLE_VALUE {
                        error_code = GetLastError();
                        let error_msg = Self::get_windows_error_message(error_code);
                        warn!(
                            "Attempt 3 failed with error code: {} ({})",
                            error_code, error_msg
                        );

                        info!("Attempt 4: Final attempt with FILE_FLAG_NO_BUFFERING");
                        // Always use FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH for last attempt
                        info!("Using direct I/O flags for final attempt");
                        let h4 = CreateFileW(
                            path_wide.as_ptr(),
                            GENERIC_READ | GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            std::ptr::null_mut(),
                            OPEN_EXISTING,
                            FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH | FILE_FLAG_SEQUENTIAL_SCAN, // Always try with direct I/O for last attempt
                            0,
                        );

                        // If all attempts failed, return a detailed error
                        if h4 == INVALID_HANDLE_VALUE {
                            error_code = GetLastError();
                            let error_msg = Self::get_windows_error_message(error_code);
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

        // Attempt to get disk sector size for confirmation
        let detected_sector_size = Self::get_disk_sector_size(&disk_path).unwrap_or(DEFAULT_SECTOR_SIZE);
        
        info!("Detected disk sector size: {} bytes", detected_sector_size);
        
        // For ANY disk operations, always use the larger sector size for maximum compatibility
        // This is critical for Windows direct I/O which requires proper alignment
        info!("Using physical sector size ({} bytes) for all disk I/O operations", PHYSICAL_SECTOR_SIZE);
        let sector_size = PHYSICAL_SECTOR_SIZE;
        
        // Create platform data
        let platform = WindowsDiskAccess {
            path: path.to_string(),
            sector_size,
        };

        // Convert Windows HANDLE to Rust File
        let file = unsafe { File::from_raw_handle(handle as *mut _) };
        
        // Log successful open
        info!("Successfully opened Windows disk device with handle: {:?}", handle);
        info!("Using sector size: {} bytes for I/O operations", sector_size);
        
        Ok((file, platform))
    }

    /// Clone a file handle (uses Windows DuplicateHandle)
    pub fn clone_file_handle(&self, file: &File) -> Result<File> {
        // On Windows, creating multiple handles to physical disks can cause access issues
        // We need to be careful with permissions when duplicating the handle
        let current_process = unsafe { GetCurrentProcess() };
        let mut target_handle: HANDLE = 0;

        // When duplicating the handle, we need to ensure we preserve the correct access rights
        let success = unsafe {
            DuplicateHandle(
                current_process,
                file.as_raw_handle() as HANDLE,
                current_process,
                &mut target_handle,
                0,
                0, // FALSE for inherit handle
                DUPLICATE_SAME_ACCESS,
            )
        };

        if success == 0 {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            
            // Log detailed error information
            error!(
                "Failed to duplicate file handle, error code: {} ({})",
                error_code, error_msg
            );
            
            return Err(anyhow!(
                "Failed to duplicate file handle, error code: {} ({})",
                error_code,
                error_msg
            ));
        }

        debug!("Successfully duplicated Windows file handle");

        // Convert the Windows HANDLE back to a Rust File
        let new_file = unsafe { File::from_raw_handle(target_handle as *mut _) };

        Ok(new_file)
    }

    /// Create a partition file proxy with Windows-specific considerations
    pub fn create_partition_proxy(
        file: File,
        partition_offset: u64,
        partition_size: u64,
    ) -> Result<PartitionFileProxy<impl Read + Write + Seek>> {
        // Use physical sector size for physical drives
        // This provides better alignment for modern disks
        let path_str = match file.try_clone() {
            Ok(f) => {
                // Try to get path from file
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
                        Ok(s) => s,
                        Err(_) => String::new(),
                    }
                } else {
                    String::new()
                }
            },
            Err(_) => String::new(),
        };
        
        // Check if this is a physical drive path
        let is_physical_drive = 
            path_str.contains("PhysicalDrive") || 
            path_str.contains("PHYSICALDRIVE");
            
        // Log all the path information we have for debugging
        debug!("Path string from handle: '{}'", path_str);
        debug!("Is physical drive: {}", is_physical_drive);
        
        // For ANY disk operations, always use the larger sector size for maximum compatibility
        // This is critical for Windows direct I/O which requires proper alignment
        // Modern drives typically use 4K sectors internally even if they report 512 bytes
        info!("Using physical sector size ({} bytes) for all disk I/O operations", PHYSICAL_SECTOR_SIZE);
        let sector_size = PHYSICAL_SECTOR_SIZE;
        
        // Use our aligned I/O implementation for Windows
        use crate::disk::windows_aligned_io::aligned_disk_io;
        
        // Create an aligned I/O wrapper for the file
        let aligned_file = match aligned_disk_io(file, sector_size) {
            Ok(aligned) => aligned,
            Err(e) => {
                error!("Failed to create aligned I/O wrapper: {}", e);
                return Err(anyhow::anyhow!("Failed to create aligned I/O wrapper: {}", e));
            }
        };
        
        // Create the PartitionFileProxy with the aligned file handle
        Ok(PartitionFileProxy {
            file: aligned_file,
            partition_offset,
            partition_size,
            current_position: 0,
            #[cfg(windows)]
            sector_size,
        })
    }

    /// Verify disk is ready for writing and lock for exclusive access
    pub fn pre_write_checks(disk_file: &File) -> Result<()> {
        // First, try to lock the volume for exclusive access
        let handle = disk_file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;
        
        info!("Attempting to lock disk volume for exclusive access");
        
        // DeviceIoControl with FSCTL_LOCK_VOLUME
        let lock_result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_LOCK_VOLUME,  // Control code for locking a volume
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };
        
        if lock_result == 0 {
            // Lock failed
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            
            error!("Failed to lock disk for writing, error code: {} ({})", error_code, error_msg);
            
            match error_code {
                32 => { // ERROR_SHARING_VIOLATION
                    return Err(anyhow::anyhow!(
                        "The disk is in use by another process and cannot be locked"
                    )
                    .context("Close any programs that might be using this disk")
                    .context("If it's a system disk, you cannot write to it while Windows is running"));
                },
                5 => { // ERROR_ACCESS_DENIED
                    return Err(anyhow::anyhow!(
                        "Access denied when trying to lock the disk"
                    )
                    .context("Make sure you're running with Administrator privileges")
                    .context("The disk may be write-protected or reserved by the system"));
                },
                _ => {
                    warn!("Could not lock volume (error code: {}), continuing with caution", error_code);
                    warn!("Write operations may fail or be inconsistent");
                }
            }
        } else {
            info!("Successfully locked disk volume for exclusive access");
        }
        
        // For physical devices, don't try to get metadata - it will fail
        // Also perform a zero-byte write test to verify we have write access
        match disk_file.try_clone() {
            Ok(mut test_file) => {
                let write_test = test_file.write(&[]);
                if let Err(e) = write_test {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        error!("Windows disk error: Disk is write-protected, permission denied");
                        return Err(anyhow::anyhow!(
                            "The disk is write-protected and cannot be written to"
                        )
                        .context(
                            "Remove write protection from the device or use a different device"
                        ));
                    } else {
                        warn!("Write test failed: {}", e);
                        warn!("Continuing with caution, but write operation may fail later");
                    }
                }
            }
            Err(e) => {
                warn!("Could not clone file handle for write test: {}", e);
                warn!("Continuing with caution, but write operation may fail later");
            }
        }

        info!("Windows: Disk is ready for writing");
        Ok(())
    }

    /// Handle disk write errors with Windows-specific context
    pub fn handle_write_error(e: &io::Error) -> Option<anyhow::Error> {
        let os_error = e.raw_os_error();

        // Log error details
        if let Some(code) = os_error {
            let error_msg = Self::get_windows_error_message(code as u32);
            error!("Windows error code: {} ({})", code, error_msg);

            match code {
                5 => {
                    error!("Access denied error (code 5) when writing to disk");
                    return Some(
                        anyhow::anyhow!(
                            "Access denied when writing to disk. Error code: 5 ({})",
                            error_msg
                        )
                        .context("Make sure you're running with Administrator privileges")
                        .context("The disk may be locked by another process or write-protected")
                    );
                }
                1117 => {
                    error!("I/O device error (code 1117) when writing to disk");
                    return Some(anyhow::anyhow!("The request could not be performed because of an I/O device error. Error code: 1117 ({})", error_msg)
                        .context("The disk may be write-protected, damaged, or have hardware issues")
                        .context("Try using a different USB port or disk"));
                }
                112 => {
                    error!("Not enough space error (code 112) when writing to disk");
                    return Some(
                        anyhow::anyhow!(
                            "There is not enough space on the disk. Error code: 112 ({})",
                            error_msg
                        )
                        .context("Check that the disk has enough free space for the image")
                        .context("Try using a larger capacity disk")
                    );
                }
                1224 => {
                    error!("Removed media error (code 1224) when writing to disk");
                    return Some(anyhow::anyhow!("The disk was removed during the write operation. Error code: 1224 ({})", error_msg)
                        .context("The disk was disconnected during the write operation")
                        .context("Ensure the disk remains connected throughout the process"));
                }
                87 => {
                    error!("Invalid parameter error (code 87) when writing to disk");
                    return Some(
                        anyhow::anyhow!(
                            "The parameter is incorrect. Error code: 87 ({})",
                            error_msg
                        )
                        .context("This may be due to mismatched buffer alignment requirements")
                        .context("Try restarting the application and using a different USB port")
                    );
                }
                _ => {
                    error!("Unrecognized Windows error code: {} ({})", code, error_msg);
                    return Some(anyhow::anyhow!("Failed to write image to disk. Windows error code: {} ({})", code, error_msg)
                        .context("An unexpected Windows error occurred during disk write")
                        .context("Try restarting your computer and running the application as Administrator"));
                }
            }
        }

        None
    }

    /// Handle disk flush errors with Windows-specific context
    pub fn handle_flush_error(e: &io::Error) -> Option<anyhow::Error> {
        let os_error = e.raw_os_error();
        if let Some(code) = os_error {
            let error_msg = Self::get_windows_error_message(code as u32);
            error!("Windows flush error code: {} ({})", code, error_msg);

            return Some(
                anyhow::anyhow!("Failed to flush disk buffer: {} ({})", e, error_msg)
                    .context("Unable to ensure all data was written to disk")
                    .context("The disk may have been disconnected or experienced an error")
            );
        }

        None
    }
    
    /// Unlock a previously locked volume
    pub fn unlock_volume(disk_file: &File) -> Result<()> {
        let handle = disk_file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;
        
        info!("Attempting to unlock disk volume");
        
        // DeviceIoControl with FSCTL_UNLOCK_VOLUME
        let unlock_result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_UNLOCK_VOLUME,  // Control code for unlocking a volume
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };
        
        if unlock_result == 0 {
            // Unlock failed
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            
            warn!("Failed to unlock disk volume, error code: {} ({})", error_code, error_msg);
            warn!("This may prevent other applications from accessing the disk until reboot");
            // We don't return an error here as it's not critical
        } else {
            info!("Successfully unlocked disk volume");
        }
        
        Ok(())
    }

    /// Handle GPT reading errors with Windows-specific solutions
    pub fn handle_gpt_error(
        _disk: &crate::disk::Disk,
        error: anyhow::Error,
    ) -> Result<Option<gpt::GptDisk<'_>>> {
        // On Windows, attempt to reopen the device with different flags
        warn!(
            "Failed to parse GPT partition table: {}. This may be due to insufficient permissions.",
            error
        );

        // Try a different approach specific to Windows
        // This would require more context in real implementation

        Ok(None) // Let the original error propagate
    }

    /// Get the sector size for a Windows disk
    pub fn get_disk_sector_size(disk_path: &str) -> Result<u32> {
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
            let error_msg = Self::get_windows_error_message(error_code);
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
            let error_msg = Self::get_windows_error_message(error_code);
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

    /// Dismount a Windows volume
    pub async fn dismount_volume(drive_path: &str) -> Result<()> {
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
            let error_msg = Self::get_windows_error_message(error_code);
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
            let error_msg = Self::get_windows_error_message(error_code);
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
            let error_msg = Self::get_windows_error_message(error_code);
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

    /// List available disks on Windows
    pub async fn list_available_disks() -> Result<Vec<DiskDevice>> {
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
        }

        // If no devices found, try fallback method
        if devices.is_empty() {
            // Try to enumerate all possible physical drives (usually 0-3 for most systems)
            for i in 0..8 {
                let path = format!(r"{}", i);
                // Try with simplified checking
                match Self::is_disk_accessible(&path).await {
                    true => {
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
                    false => {
                        // Only log as debug since it's expected that some disks might not exist
                        debug!("Could not access disk {}", i);
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

    /// Check if a disk is accessible without fully opening it
    async fn is_disk_accessible(path: &str) -> bool {
        // Format path for Windows API
        let disk_path = format!(r"\\.\PhysicalDrive{}", path);
        let path_wide: Vec<u16> = disk_path.encode_utf16().chain(std::iter::once(0)).collect();

        // Try to open with minimal access
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
            return false;
        }

        // Close the handle
        unsafe { CloseHandle(handle) };

        true
    }

    /// Translate Windows error codes to readable messages
    pub fn get_windows_error_message(code: u32) -> &'static str {
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
}

// NOTE: The specific implementations of Read, Write, and Seek for PartitionFileProxy<File>
// have been moved to common.rs with conditional compilation for Windows
