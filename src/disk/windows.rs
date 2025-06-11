// Windows-specific disk operations

use crate::disk::common::{DiskDevice, PartitionFileProxy};
use anyhow::{Result, anyhow};
// GptConfig is used in handle_gpt_error implementations
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::os::windows::process::CommandExt;
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
    pub path: String,
    // Detected sector size of the disk
    pub sector_size: u32,
}

impl WindowsDiskAccess {
    /// Clear disk partitions using diskpart with progress reporting
    ///
    /// # Arguments
    /// * `path` - The path to the disk device
    /// * `cancel_token` - Token to cancel the operation
    ///
    /// # Returns
    /// * A sipper that reports progress updates as the clearing proceeds
    pub fn clear_disk_partitions(
        path: &str,
        cancel_token: crate::models::CancelToken,
    ) -> impl iced::task::Sipper<
        Result<crate::disk::common::WriteProgress>,
        crate::disk::common::WriteProgress,
    > + Send
    + 'static {
        use crate::disk::common::WriteProgress;
        use iced::task;

        let path_owned = path.to_string();

        task::sipper(async move |mut sipper| -> Result<WriteProgress> {
            sipper
                .send(WriteProgress::ClearingPartitions { progress: 0.0 })
                .await;

            // Use blocking task for diskpart operations
            let r = tokio::task::spawn_blocking(move || -> Result<WriteProgress> {
                // Check if operation was cancelled before starting
                if cancel_token.is_cancelled() {
                    info!("Partition clearing cancelled by user before starting");
                    return Err(anyhow::anyhow!("Operation cancelled by user"));
                }

                info!("Starting disk partition clearing for path: {}", path_owned);

                // Extract disk number from path using robust regex-based approach
                let disk_num = match Self::extract_disk_number_from_path_robust(&path_owned) {
                    Ok(num) => num,
                    Err(e) => {
                        error!(
                            "Failed to extract disk number from path '{}': {}",
                            path_owned, e
                        );
                        return Err(anyhow::anyhow!("Invalid disk path: {}", e));
                    }
                };

                info!("Clearing partitions on PhysicalDrive{}", disk_num);

                // Create enhanced diskpart commands - include online disk to handle offline disks with signature collisions
                let script_content = format!(
                    "select disk {}\ndetail disk\nonline disk\ndetail disk\nclean\ndetail disk\nrescan\nexit\n",
                    disk_num
                );

                info!("Diskpart commands: {}", script_content.replace('\n', "; "));

                // Execute diskpart with enhanced retry logic using stdin
                let mut success = false;
                let mut last_error = String::new();

                for attempt in 1..=3 {
                    // Check for cancellation before each attempt
                    if cancel_token.is_cancelled() {
                        info!(
                            "Partition clearing cancelled by user during attempt {}",
                            attempt
                        );
                        return Err(anyhow::anyhow!("Operation cancelled by user"));
                    }

                    info!(
                        "Diskpart attempt {}/3 for PhysicalDrive{}",
                        attempt, disk_num
                    );

                    // Update progress based on attempt
                    let _progress = (attempt as f32 - 1.0) / 3.0;
                    // Note: We can't send progress updates from inside spawn_blocking
                    // The UI will show progress based on the subscription

                    let mut child = match std::process::Command::new("diskpart")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .spawn()
                    {
                        Ok(child) => child,
                        Err(e) => {
                            last_error = format!("Failed to spawn diskpart: {}", e);
                            error!("Diskpart spawn failed on attempt {}: {}", attempt, e);

                            if attempt < 3 {
                                warn!("Retrying partition clearing in 500ms...");
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            continue;
                        }
                    };

                    // Write commands to stdin
                    if let Some(stdin) = child.stdin.take() {
                        use std::io::Write;
                        let mut stdin = stdin;
                        if let Err(e) = stdin.write_all(script_content.as_bytes()) {
                            warn!("Failed to write to diskpart stdin: {}", e);
                        }
                        // stdin is automatically closed when dropped
                    }

                    // Wait for completion and get output
                    let output = match child.wait_with_output() {
                        Ok(output) => output,
                        Err(e) => {
                            last_error = format!("Failed to get diskpart output: {}", e);
                            error!("Diskpart output failed on attempt {}: {}", attempt, e);

                            if attempt < 3 {
                                warn!("Retrying partition clearing in 500ms...");
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            continue;
                        }
                    };

                    let output_msg = String::from_utf8_lossy(&output.stdout);
                    let error_msg = String::from_utf8_lossy(&output.stderr);

                    info!("Diskpart attempt {} output: {}", attempt, output_msg);
                    if !error_msg.is_empty() {
                        warn!("Diskpart attempt {} stderr: {}", attempt, error_msg);
                    }

                    // Enhanced error detection for offline disks and signature collisions
                    let has_diskpart_error = output_msg.contains("DiskPart has encountered an error");
                    let has_offline_error = output_msg.contains("Offline") || 
                                          output_msg.contains("offline") ||
                                          output_msg.contains("nie jest dozwolona dla dysku w trybie offline"); // Polish
                    let has_signature_collision = output_msg.contains("Signature Collision") ||
                                               output_msg.contains("signature collision");
                    let has_vds_error = output_msg.contains("Virtual Disk Service error");

                    if output.status.success() && !has_diskpart_error && !has_offline_error && !has_vds_error {
                        info!(
                            "Successfully cleared partitions on disk {} (attempt {})",
                            disk_num, attempt
                        );
                        success = true;
                        break;
                    } else {
                        // Provide specific error information
                        if has_offline_error || has_signature_collision {
                            warn!("Diskpart failed on attempt {} - disk is offline with signature collision", attempt);
                            warn!("This usually happens when Windows detects duplicate disk signatures");
                        } else if has_vds_error {
                            warn!("Diskpart failed on attempt {} - Virtual Disk Service error", attempt);
                        }
                        
                        last_error = format!(
                            "Diskpart failed - stdout: {}, stderr: {}",
                            output_msg, error_msg
                        );
                        error!("Diskpart attempt {} failed: {}", attempt, last_error);

                        if attempt < 3 {
                            warn!("Retrying partition clearing in 500ms...");
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                }

                if !success {
                    error!(
                        "All diskpart attempts failed for disk {}: {}",
                        disk_num, last_error
                    );
                    return Err(anyhow::anyhow!(
                        "Failed to clear disk partitions: {}",
                        last_error
                    ));
                } else {
                    // Add sleep after successful diskpart operations to allow Windows to process changes
                    info!("Waiting 2 seconds for Windows to process diskpart changes...");
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                }

                // Dismount any remaining volumes
                info!("Dismounting volumes on PhysicalDrive{}", disk_num);
                let volumes = Self::get_volumes_for_physical_drive(disk_num as usize);

                for volume in volumes {
                    if cancel_token.is_cancelled() {
                        return Err(anyhow::anyhow!("Operation cancelled by user"));
                    }

                    info!("Dismounting volume {}", volume);
                    if let Err(e) = Self::dismount_volume_path(&volume) {
                        warn!("Failed to dismount volume {}: {}", volume, e);
                        // Continue with other volumes - dismount failures are non-fatal
                    }
                }

                info!("Successfully cleared all partitions on disk {}", disk_num);
                Ok(WriteProgress::Finish)
            })
            .await?;

            r
        })
    }

    /// Extract disk number from path using robust regex pattern matching
    ///
    /// Uses the same approach as the C++ example: std::regex("\\\\\.\\PHYSICALDRIVE(\d+)", std::regex_constants::icase)
    pub fn extract_disk_number_from_path_robust(path_str: &str) -> Result<u32> {
        use regex::Regex;

        // Create case-insensitive regex pattern matching the C++ example
        let pattern = r"(?i)\\\\\.\\PHYSICALDRIVE(\d+)";
        let re = Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("Failed to compile regex pattern: {}", e))?;

        // First try the robust regex approach
        if let Some(caps) = re.captures(path_str) {
            if let Some(num_match) = caps.get(1) {
                return num_match
                    .as_str()
                    .parse::<u32>()
                    .map_err(|e| anyhow::anyhow!("Invalid disk number format: {}", e));
            }
        }

        // Fallback to simple number parsing if path is just a number
        if let Ok(num) = path_str.parse::<u32>() {
            return Ok(num);
        }

        // Enhanced fallback with more patterns
        if let Some(disk_num_str) = path_str.strip_prefix(r"\\.\PhysicalDrive") {
            return disk_num_str
                .parse::<u32>()
                .map_err(|e| anyhow::anyhow!("Invalid disk number in path: {}", e));
        }

        if let Some(disk_num_str) = path_str.strip_prefix("PhysicalDrive") {
            return disk_num_str
                .parse::<u32>()
                .map_err(|e| anyhow::anyhow!("Invalid disk number in path: {}", e));
        }

        Err(anyhow::anyhow!(
            "Could not extract disk number from path: {}",
            path_str
        ))
    }

    /// Open and lock a disk by its path
    ///
    /// # Arguments
    /// * `path` - The path to the disk device
    /// * `edit_mode` - When true, we're opening for editing configuration only, not writing an image.
    ///   This skips diskpart cleaning on Windows, which avoids potential data loss during editing.
    pub async fn lock_path(path: &str, edit_mode: bool) -> Result<(File, Self)> {
        info!(
            "Locking Windows disk path: {} (edit_mode: {})",
            path, edit_mode
        );

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
            match Self::dismount_volume_path(path) {
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
        } else if (path.contains("PhysicalDrive") || path.parse::<usize>().is_ok()) && !edit_mode {
            // If it's a physical drive AND we're not in edit mode, clean the disk with diskpart BEFORE locking it
            // This is critical for Windows to allow writing to the disk, but should be skipped in edit mode
            // to avoid data loss while editing configuration
            let drive_number_result = Self::extract_disk_number_from_path(path);

            if let Ok(disk_num) = drive_number_result {
                info!(
                    "Physical drive {} specified - cleaning disk before locking",
                    disk_num
                );

                // Clean the disk with diskpart BEFORE opening it
                // This is important as diskpart can't clean a locked disk
                info!("Attempting to clean disk {} with diskpart", disk_num);

                // Create diskpart commands - include online disk to handle offline disks with signature collisions
                let script_content = format!(
                    "select disk {}\ndetail disk\nonline disk\ndetail disk\nclean\ndetail disk\nrescan\nexit\n",
                    disk_num
                );

                info!("Diskpart commands: {}", script_content.replace('\n', "; "));

                // Try to run diskpart with multiple attempts using stdin
                let mut success = false;
                let mut error_output = String::new();

                // Try up to 3 times to clean the disk with diskpart
                for attempt in 1..=3 {
                    info!(
                        "Disk cleaning attempt {}/3 with diskpart for PhysicalDrive{}",
                        attempt, disk_num
                    );

                    // Execute diskpart using stdin
                    let mut child = match std::process::Command::new("diskpart")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .spawn()
                    {
                        Ok(child) => child,
                        Err(e) => {
                            error!(
                                "Failed to spawn diskpart command on attempt {}: {}",
                                attempt, e
                            );
                            if attempt < 3 {
                                warn!("Retrying disk cleaning in 500ms...");
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            continue;
                        }
                    };

                    // Write commands to stdin
                    if let Some(stdin) = child.stdin.take() {
                        use std::io::Write;
                        let mut stdin = stdin;
                        if let Err(e) = stdin.write_all(script_content.as_bytes()) {
                            warn!("Failed to write to diskpart stdin: {}", e);
                        }
                        // stdin is automatically closed when dropped
                    }

                    // Wait for completion and get output
                    let output = match child.wait_with_output() {
                        Ok(output) => output,
                        Err(e) => {
                            error!(
                                "Failed to get diskpart output on attempt {}: {}",
                                attempt, e
                            );
                            if attempt < 3 {
                                warn!("Retrying disk cleaning in 500ms...");
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            continue;
                        }
                    };

                    let output_msg = String::from_utf8_lossy(&output.stdout);
                    let error_msg = String::from_utf8_lossy(&output.stderr);

                    // Always log diskpart output for diagnosis
                    info!("Diskpart stdout: {}", output_msg);

                    // Enhanced error detection for offline disks and signature collisions
                    let has_diskpart_error = output_msg.contains("DiskPart has encountered an error");
                    let has_offline_error = output_msg.contains("Offline") || 
                                          output_msg.contains("offline") ||
                                          output_msg.contains("nie jest dozwolona dla dysku w trybie offline"); // Polish
                    let has_signature_collision = output_msg.contains("Signature Collision") ||
                                               output_msg.contains("signature collision");
                    let has_vds_error = output_msg.contains("Virtual Disk Service error");

                    if output.status.success() && !has_diskpart_error && !has_offline_error && !has_vds_error {
                        info!(
                            "Successfully cleaned disk {} with diskpart on attempt {}",
                            disk_num, attempt
                        );
                        success = true;
                        break;
                    } else {
                        error_output = format!("stderr: {}, stdout: {}", error_msg, output_msg);
                        
                        // Provide specific error information
                        if has_offline_error || has_signature_collision {
                            error!("Diskpart failed on attempt {} - disk is offline with signature collision", attempt);
                            error!("This usually happens when Windows detects duplicate disk signatures");
                        } else if has_vds_error {
                            error!("Diskpart failed on attempt {} - Virtual Disk Service error", attempt);
                        } else {
                            error!("Diskpart cleaning failed on attempt {}", attempt);
                        }
                        error!("Error details: {}", error_output);

                        if attempt < 3 {
                            warn!("Retrying disk cleaning in 500ms...");
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                }

                if !success {
                    warn!("ALL ATTEMPTS TO CLEAN DISK FAILED: {}", error_output);
                    warn!("This may lead to access denied errors when writing to the disk.");
                    // Continue anyway - we'll still try to open the disk
                } else {
                    // Add sleep after successful diskpart operations to allow Windows to process changes
                    info!("Waiting 2 seconds for Windows to process diskpart changes...");
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                }

                // Now dismount all volumes on this drive before locking it
                info!(
                    "Attempting to dismount all volumes on physical drive {}",
                    disk_num
                );

                // Try to get all mounted volumes for this physical drive
                let volumes = Self::get_volumes_for_physical_drive(disk_num as usize);
                if volumes.is_empty() {
                    warn!("No volumes found for physical drive {}", disk_num);
                }

                // Try to dismount each volume
                for volume in volumes {
                    info!(
                        "Attempting to dismount volume {} on physical drive {}",
                        volume, disk_num
                    );
                    match Self::dismount_volume_path(&volume) {
                        Ok(_) => info!(
                            "Successfully dismounted volume {} on drive {}",
                            volume, disk_num
                        ),
                        Err(e) => warn!(
                            "Failed to dismount volume {} on drive {}: {}",
                            volume, disk_num, e
                        ),
                    }
                }
            } else {
                warn!("Could not parse drive number from path: {}", path);
                warn!("This may fail if any volumes on this drive are in use by Windows");
            }
        } else if edit_mode && (path.contains("PhysicalDrive") || path.parse::<usize>().is_ok()) {
            // We're in edit mode with a physical drive, so we skip diskpart cleaning
            info!("Edit mode enabled - skipping diskpart cleaning to preserve existing data");

            // Extract disk number for logging purposes
            if let Ok(disk_num) = Self::extract_disk_number_from_path(path) {
                info!(
                    "Edit mode for physical drive {} - no disk cleaning will be performed",
                    disk_num
                );
            }
        }

        // Try to get the sector size for this disk for better performance
        // Note: We don't actually use this immediately, but the struct needs it
        // and detecting it early can help with error diagnosis
        let _sector_size = match Self::get_disk_sector_size(path) {
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
            let is_physical_drive =
                disk_path.contains("PhysicalDrive") || path.parse::<usize>().is_ok();
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
                            FILE_FLAG_NO_BUFFERING
                                | FILE_FLAG_WRITE_THROUGH
                                | FILE_FLAG_SEQUENTIAL_SCAN, // Always try with direct I/O for last attempt
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
        let detected_sector_size =
            Self::get_disk_sector_size(&disk_path).unwrap_or(DEFAULT_SECTOR_SIZE);

        info!("Detected disk sector size: {} bytes", detected_sector_size);

        // For ANY disk operations, always use the larger sector size for maximum compatibility
        // This is critical for Windows direct I/O which requires proper alignment
        info!(
            "Using physical sector size ({} bytes) for all disk I/O operations",
            PHYSICAL_SECTOR_SIZE
        );
        let sector_size = PHYSICAL_SECTOR_SIZE;

        // Create platform data
        let platform = WindowsDiskAccess {
            path: path.to_string(),
            sector_size,
        };

        // Convert Windows HANDLE to Rust File
        let file = unsafe { File::from_raw_handle(handle as *mut _) };

        // Log successful open
        info!(
            "Successfully opened Windows disk device with handle: {:?}",
            handle
        );
        info!(
            "Using sector size: {} bytes for I/O operations",
            sector_size
        );

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
    ///
    /// This function is no longer used as we now use in-memory partition operations
    /// to avoid alignment issues with small reads/writes on Windows.
    /// It is kept for compatibility with existing code.
    #[allow(dead_code)]
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
            }
            Err(_) => String::new(),
        };

        // Check if this is a physical drive path
        let is_physical_drive =
            path_str.contains("PhysicalDrive") || path_str.contains("PHYSICALDRIVE");

        // Log all the path information we have for debugging
        debug!("Path string from handle: '{}'", path_str);
        debug!("Is physical drive: {}", is_physical_drive);

        // For ANY disk operations, always use the larger sector size for maximum compatibility
        // This is critical for Windows direct I/O which requires proper alignment
        // Modern drives typically use 4K sectors internally even if they report 512 bytes
        info!(
            "Using physical sector size ({} bytes) for all disk I/O operations",
            PHYSICAL_SECTOR_SIZE
        );
        let sector_size = PHYSICAL_SECTOR_SIZE;

        // Use our aligned I/O implementation for Windows
        use crate::disk::windows_aligned_io::aligned_disk_io;
        let aligned_file = match aligned_disk_io(file, sector_size) {
            Ok(aligned) => aligned,
            Err(e) => {
                error!("Failed to create aligned I/O wrapper: {}", e);
                return Err(anyhow::anyhow!(
                    "Failed to create aligned I/O wrapper: {}",
                    e
                ));
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
    /// Note: Disk cleaning is now performed earlier during lock_path to avoid conflicts
    pub fn pre_write_checks(disk_file: &File, original_path: Option<&str>) -> Result<()> {
        // Log the original path if provided, but we won't use it for cleaning anymore
        // since cleaning is now done before the disk is locked
        if let Some(path) = original_path {
            info!("Path from original handle for verification: {}", path);
        }

        // Next, try to lock the volume for exclusive access
        let handle = disk_file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;

        info!("Attempting to lock disk volume for exclusive access");

        // First, enable extended DASD I/O (Direct Access Storage Device) like RPI Imager does
        // This is essential for some operations on Windows
        info!("Enabling extended DASD I/O access");
        unsafe {
            DeviceIoControl(
                handle,
                FSCTL_ALLOW_EXTENDED_DASD_IO,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        // Try to lock the volume with multiple attempts
        let mut locked = false;
        for attempt in 0..30 {
            // DeviceIoControl with FSCTL_LOCK_VOLUME
            info!("Locking volume, attempt {}/30", attempt + 1);

            let lock_result = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_LOCK_VOLUME, // Control code for locking a volume
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };

            if lock_result != 0 {
                locked = true;
                info!("Successfully locked disk volume on attempt {}", attempt + 1);
                break;
            }

            let error_code = unsafe { GetLastError() };

            // Progressive delay strategy
            let delay_ms = match attempt {
                0..=5 => 100,  // First 5 attempts: 100ms
                6..=15 => 200, // Next 10 attempts: 200ms
                _ => 500,      // Final attempts: 500ms
            };

            info!(
                "Lock attempt {} failed, error code: {}, waiting {}ms before retrying...",
                attempt + 1,
                error_code,
                delay_ms
            );

            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

        // Check if locking was successful after all attempts
        if !locked {
            // Get the last error for diagnostic purposes
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);

            error!(
                "Failed to lock disk after 20 attempts, error code: {} ({})",
                error_code, error_msg
            );

            match error_code {
                32 => {
                    // ERROR_SHARING_VIOLATION
                    warn!(
                        "Disk access is still blocked by another process after multiple retry attempts"
                    );
                    warn!("Some volumes might not have been properly dismounted");

                    return Err(anyhow::anyhow!(
                        "The disk is in use by another process and cannot be locked"
                    )
                    .context("Close any programs that might be using this disk")
                    .context(
                        "If it's a system disk, you cannot write to it while Windows is running",
                    ));
                }
                5 => {
                    // ERROR_ACCESS_DENIED
                    return Err(
                        anyhow::anyhow!("Access denied when trying to lock the disk")
                            .context("Make sure you're running with Administrator privileges")
                            .context("The disk may be write-protected or reserved by the system"),
                    );
                }
                _ => {
                    warn!(
                        "Could not lock volume (error code: {}), continuing with caution",
                        error_code
                    );
                    warn!("Write operations may fail or be inconsistent");
                }
            }
        }

        // Dismount all volumes directly using the physical drive handle
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
            info!(
                "Note: Could not dismount directly from physical device: {} ({})",
                error_code, error_msg
            );
            info!("This is often normal when writing to physical drives rather than volumes");
        } else {
            info!("Successfully dismounted volumes from physical drive handle");
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
                    error!(
                        "This error often occurs when disk cleaning via diskpart failed or was skipped."
                    );
                    error!("Check the logs for diskpart output or errors above.");

                    return Some(
                        anyhow::anyhow!(
                            "Access denied when writing to disk. Error code: 5 ({})",
                            error_msg
                        )
                        .context("Make sure you're running with Administrator privileges")
                        .context("The disk may be locked by another process or write-protected")
                        .context("Ensure diskpart is available and successfully cleaned the disk"),
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
                        .context("Try using a larger capacity disk"),
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
                        .context("Try restarting the application and using a different USB port"),
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
                    .context("The disk may have been disconnected or experienced an error"),
            );
        }

        None
    }

    /// Extract disk number from a Windows drive path
    ///
    /// This function handles various formats of Windows disk paths:
    /// - \\.\PhysicalDrive0
    /// - \\.\PHYSICALDRIVE0
    /// - PhysicalDrive0
    /// - PHYSICALDRIVE0
    /// - 0 (just a number)
    /// - Any string that contains "PhysicalDrive" followed by a number
    pub fn extract_disk_number_from_path(path_str: &str) -> Result<u32> {
        // Use the new robust implementation
        Self::extract_disk_number_from_path_robust(path_str)
    }

    /// Clean the disk by removing all partitions using diskpart (used by older disk-image-writer)
    /// This function is kept for reference but is no longer explicitly called
    pub fn clean_disk(&self) -> Result<()> {
        // Try to extract disk number from the path (like "\\.\PHYSICALDRIVE0")
        let path_str = self.path.as_str();
        let disk_num = Self::extract_disk_number_from_path(path_str)?;

        info!("Cleaning disk {} using diskpart", disk_num);

        // Create diskpart commands
        let script_content = format!("select disk {}\nclean\nrescan\nexit\n", disk_num);

        info!("Diskpart commands: {}", script_content.replace('\n', "; "));

        // Execute diskpart using stdin
        let mut child = std::process::Command::new("diskpart")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;

        // Write commands to stdin
        if let Some(stdin) = child.stdin.take() {
            use std::io::Write;
            let mut stdin = stdin;
            stdin.write_all(script_content.as_bytes())?;
            // stdin is automatically closed when dropped
        }

        // Wait for completion and get output
        let output = child.wait_with_output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            let output_msg = String::from_utf8_lossy(&output.stdout);

            error!("Diskpart error output: {}", error_msg);
            error!("Diskpart standard output: {}", output_msg);

            return Err(anyhow!(
                "Error running diskpart clean command: {}",
                if error_msg.is_empty() {
                    output_msg
                } else {
                    error_msg
                }
            ));
        }

        info!("Disk {} cleaned successfully with diskpart", disk_num);
        Ok(())
    }

    /// Internal helper to unlock a volume with just a file handle - for easy retry/error handling
    fn unlock_volume_internal(handle: HANDLE) -> Result<bool> {
        let mut bytes_returned: u32 = 0;

        // Use DeviceIoControl with FSCTL_UNLOCK_VOLUME
        let unlock_result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_UNLOCK_VOLUME, // Control code for unlocking a volume
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if unlock_result != 0 {
            info!("Volume unlocked successfully");
            Ok(true)
        } else {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);

            warn!("Failed to unlock volume: {} ({})", error_code, error_msg);
            Err(anyhow!(
                "Failed to unlock volume: {} ({})",
                error_code,
                error_msg
            ))
        }
    }

    /// Unlock a previously locked volume - with retry mechanism
    pub fn unlock_volume(disk_file: &File) -> Result<()> {
        let handle = disk_file.as_raw_handle() as HANDLE;

        info!(
            "Attempting to unlock disk volume, handle = {:p}",
            handle as *const std::ffi::c_void
        );

        // Try to unlock the volume multiple times
        for attempt in 0..5 {
            info!("Unlock attempt {}/5", attempt + 1);

            // Get the timing for this unlock attempt
            let attempt_start = std::time::Instant::now();

            // Try to unlock using the internal helper
            let unlock_result = Self::unlock_volume_internal(handle);
            let attempt_duration = attempt_start.elapsed();

            match unlock_result {
                Ok(_) => {
                    info!(
                        "Successfully unlocked disk volume on attempt {} in {:?}",
                        attempt + 1,
                        attempt_duration
                    );
                    return Ok(());
                }
                Err(e) => {
                    if attempt < 4 {
                        warn!(
                            "Unlock attempt {} failed after {:?}: {}, waiting 100ms before retrying...",
                            attempt + 1,
                            attempt_duration,
                            e
                        );

                        // Wait 100ms between attempts - exactly like disk-image-writer
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    } else {
                        // Last attempt
                        warn!(
                            "Final unlock attempt failed after {:?}: {}",
                            attempt_duration, e
                        );
                    }
                }
            }
        }

        // All unlock attempts failed - but we continue anyway since this is non-critical
        warn!("Failed to unlock disk volume after 5 attempts");
        warn!("This may prevent other applications from accessing the disk until reboot");

        // Log some additional information about the disk file that might be useful
        info!("Continuing despite unlock failure - this is non-critical");

        // Return success anyway since unlocking failure isn't critical
        Ok(())
    }

    /// Enable extended DASD I/O access on a file handle
    pub fn enable_extended_dasd_io(file: &File) -> bool {
        use std::os::windows::io::AsRawHandle;

        // Get the raw handle
        let handle = file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;

        // Define the FSCTL_ALLOW_EXTENDED_DASD_IO constant if needed
        // FSCTL_ALLOW_EXTENDED_DASD_IO = 0x00090083
        const FSCTL_ALLOW_EXTENDED_DASD_IO: u32 = 0x00090083;

        // Call DeviceIoControl
        let result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_ALLOW_EXTENDED_DASD_IO,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        // Return true if successful
        result != 0
    }

    /// Lock a volume with multiple retry attempts
    pub fn lock_volume_with_retry(file: &File, max_attempts: u32) -> bool {
        use std::os::windows::io::AsRawHandle;

        // Get the raw handle
        let handle = file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;

        // Define the FSCTL_LOCK_VOLUME constant if needed
        // FSCTL_LOCK_VOLUME = 0x00090018
        const FSCTL_LOCK_VOLUME: u32 = 0x00090018;

        // Try to lock the volume with multiple attempts
        let mut locked = false;
        for attempt in 0..max_attempts {
            // DeviceIoControl with FSCTL_LOCK_VOLUME
            info!("Locking volume, attempt {}/{}", attempt + 1, max_attempts);

            let lock_result = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_LOCK_VOLUME, // Control code for locking a volume
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                )
            };

            if lock_result != 0 {
                locked = true;
                info!("Successfully locked disk volume on attempt {}", attempt + 1);
                break;
            }

            let error_code = unsafe { GetLastError() };
            info!(
                "Lock attempt {} failed, error code: {}, waiting 100ms before retrying...",
                attempt + 1,
                error_code
            );

            // Wait 100ms between attempts
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        locked
    }

    /// Dismount a volume from a file handle
    pub fn dismount_volume_from_handle(file: &File) -> Result<()> {
        use std::os::windows::io::AsRawHandle;

        // Get the raw handle
        let handle = file.as_raw_handle() as HANDLE;
        let mut bytes_returned: u32 = 0;

        // Define the FSCTL_DISMOUNT_VOLUME constant if needed
        // FSCTL_DISMOUNT_VOLUME = 0x00090020
        const FSCTL_DISMOUNT_VOLUME: u32 = 0x00090020;

        // Call DeviceIoControl
        let result = unsafe {
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

        if result == 0 {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            Err(anyhow!(
                "Failed to dismount volume: {} ({})",
                error_code,
                error_msg
            ))
        } else {
            Ok(())
        }
    }

    /// Handle GPT reading errors with Windows-specific solutions
    pub fn handle_gpt_error(
        disk: &crate::disk::Disk,
        error: anyhow::Error,
    ) -> Result<Option<gpt::GptDisk<'_>>> {
        // On Windows, attempt to reopen the device with different flags
        warn!(
            "Failed to parse GPT partition table: {}. This may be due to insufficient permissions or alignment issues.",
            error
        );

        // Try a different approach with aligned I/O for Windows
        info!("Attempting Windows-specific GPT reading with aligned I/O");

        // Get a new file handle
        let file = match disk.file.try_clone() {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to clone disk file handle: {}", e);
                return Ok(None); // Let the original error propagate
            }
        };

        // Use our AlignedDiskIO implementation for better Windows compatibility
        use crate::disk::windows_aligned_io::aligned_disk_io;

        // Always use 4KB sector size for maximum compatibility with modern disks
        let sector_size = 512;
        // Try to create an aligned disk I/O wrapper
        let aligned_file = match aligned_disk_io(file, sector_size) {
            Ok(aligned) => aligned,
            Err(e) => {
                error!("Failed to create aligned I/O wrapper: {}", e);
                return Ok(None); // Let the original error propagate
            }
        };

        // Create a new GptConfig with relaxed validation
        let cfg = gpt::GptConfig::new()
            .writable(false)
            .initialized(true) // Skip checking LBA0 for MBR
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb512);

        // Try to open the GPT disk with our aligned wrapper
        match cfg.open_from_device(Box::new(aligned_file)) {
            Ok(disk) => {
                info!("Successfully read GPT partition table with aligned I/O");
                Ok(Some(disk))
            }
            Err(e) => {
                error!("Even with aligned I/O, failed to parse GPT: {}", e);
                Ok(None) // Let the original error propagate
            }
        }
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

    /// Dismount a Windows volume by path (not a file handle)
    pub fn dismount_volume_path(drive_path: &str) -> Result<()> {
        info!("Dismounting Windows volume: {}", drive_path);

        // Prepare the path for Windows API
        let drive_path = format!(r"\\.\{}", drive_path.trim_end_matches('\\'));

        // Convert the path to a wide string for Windows API
        let path_wide: Vec<u16> = drive_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Open the volume with required privileges - following RPI Imager's approach
        // Important: exclusive access (no file sharing) and using direct I/O flags
        let handle = unsafe {
            CreateFileW(
                path_wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0, // No sharing - exclusive access
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH, // Direct I/O
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            info!(
                "Could not open volume {} exclusively, trying with shared access: {} ({})",
                drive_path, error_code, error_msg
            );

            // Try again with shared access as fallback
            let handle = unsafe {
                CreateFileW(
                    path_wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH, // Direct I/O
                    0,
                )
            };

            if handle == INVALID_HANDLE_VALUE {
                let error_code = unsafe { GetLastError() };
                let error_msg = Self::get_windows_error_message(error_code);
                return Err(anyhow!(
                    "Failed to open volume {}, error code: {} ({})",
                    drive_path,
                    error_code,
                    error_msg
                ));
            }

            // Continue with the obtained handle
            Self::dismount_volume_with_handle(handle, &drive_path)
        } else {
            // Continue with the exclusively opened handle
            Self::dismount_volume_with_handle(handle, &drive_path)
        }
    }

    /// Helper function to dismount a volume using an already opened handle - with RPI Imager's retry logic
    fn dismount_volume_with_handle(handle: HANDLE, drive_path: &str) -> Result<()> {
        let mut bytes_returned: u32 = 0;

        // Enable extended DASD I/O access first, just like RPI Imager
        info!(
            "Enabling extended DASD I/O access for volume {}",
            drive_path
        );
        unsafe {
            DeviceIoControl(
                handle,
                FSCTL_ALLOW_EXTENDED_DASD_IO,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        // Step 1: Lock the volume with retries (just like RPI Imager)
        info!("Locking volume: {} with up to 20 attempts", drive_path);

        let mut locked = false;
        for attempt in 0..20 {
            info!("Lock attempt {}/20 for volume {}", attempt + 1, drive_path);

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

            if lock_result != 0 {
                locked = true;
                info!(
                    "Successfully locked volume {} on attempt {}",
                    drive_path,
                    attempt + 1
                );
                break;
            }

            let error_code = unsafe { GetLastError() };
            info!(
                "Lock attempt {} failed for volume {}, error: {}, waiting 100ms...",
                attempt + 1,
                drive_path,
                error_code
            );

            // Wait 100ms between attempts (exactly like RPI Imager)
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if !locked {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            warn!(
                "Failed to lock volume {} after 20 attempts: {} ({})",
                drive_path, error_code, error_msg
            );

            // For volumes (unlike physical drives), we'll return an error if we can't lock
            // This helps diagnose issues with specific volumes
            unsafe { CloseHandle(handle) };
            return Err(anyhow!(
                "Failed to lock volume {} after multiple attempts: {} ({})",
                drive_path,
                error_code,
                error_msg
            ));
        }

        // Step 2: Dismount the volume - also with retries
        info!("Dismounting volume: {}", drive_path);

        let mut dismounted = false;
        for attempt in 0..5 {
            // Fewer retries for dismounting, usually succeeds on first try if locked
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

            if dismount_result != 0 {
                dismounted = true;
                info!(
                    "Successfully dismounted volume {} on attempt {}",
                    drive_path,
                    attempt + 1
                );
                break;
            }

            let error_code = unsafe { GetLastError() };
            info!(
                "Dismount attempt {} failed for volume {}, error: {}, waiting 100ms...",
                attempt + 1,
                drive_path,
                error_code
            );

            // Wait 100ms between attempts
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if !dismounted {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            warn!(
                "Failed to dismount volume {} after multiple attempts: {} ({})",
                drive_path, error_code, error_msg
            );
            // We'll continue despite dismount failures
        }

        // Step 3: Try to take the volume offline (like RPI Imager does)
        info!("Taking volume {} offline", drive_path);

        // Define IOCTL code for taking volume offline (not in the windows-sys crate)
        const IOCTL_VOLUME_OFFLINE: u32 = 0x56C000;

        let offline_result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_VOLUME_OFFLINE,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if offline_result == 0 {
            let error_code = unsafe { GetLastError() };
            let error_msg = Self::get_windows_error_message(error_code);
            warn!(
                "Note: Failed to take volume {} offline: {} ({})",
                drive_path, error_code, error_msg
            );
            // This is expected on many systems - continue
        } else {
            info!("Successfully took volume {} offline", drive_path);
        }

        // Just to be safe, sleep another 100ms to ensure Windows has fully processed our requests
        // RPI Imager sometimes does this to ensure operations have time to complete
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Close the handle but keep the volume locked
        // Windows will automatically unlock the volume when all handles are closed
        info!(
            "Closing handle but volume {} remains dismounted",
            drive_path
        );
        unsafe { CloseHandle(handle) };

        debug!("Successfully processed volume: {}", drive_path);
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

impl WindowsDiskAccess {
    /// Get a list of volume drive letters mounted on a physical drive
    fn get_volumes_for_physical_drive(drive_number: usize) -> Vec<String> {
        debug!("Getting volumes for physical drive {}", drive_number);
        let mut volumes = Vec::new();

        // Try to use rs-drivelist to map physical drives to volumes
        match rs_drivelist::drive_list() {
            Ok(drives) => {
                for drive in drives {
                    // Get device path and check if it matches our physical drive
                    let device_path = drive
                        .devicePath
                        .as_ref()
                        .map_or_else(|| drive.device.clone(), |p| p.clone());

                    if device_path.contains(&format!("PHYSICALDRIVE{}", drive_number)) {
                        // This drive matches our physical drive, get all mount points
                        for mountpoint in &drive.mountpoints {
                            let mount_path = &mountpoint.path;
                            if mount_path.ends_with(":") || mount_path.ends_with(":\\") {
                                // Extract just the drive letter (e.g., "C:")
                                let drive_letter = mount_path.trim_end_matches('\\').to_string();
                                debug!(
                                    "Found volume {} on physical drive {}",
                                    drive_letter, drive_number
                                );
                                volumes.push(drive_letter);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to list drives using rs-drivelist: {}", e);

                // Fallback approach: try common drive letters and check their physical drive
                for letter in b'C'..=b'Z' {
                    let drive_letter = format!("{}:", char::from(letter));

                    // Try to check if this drive letter is on the target physical drive
                    // This is a simplified approach and may not work in all cases
                    if let Ok(true) = Self::is_volume_on_physical_drive(&drive_letter, drive_number)
                    {
                        debug!(
                            "Found volume {} on physical drive {}",
                            drive_letter, drive_number
                        );
                        volumes.push(drive_letter);
                    }
                }
            }
        }

        volumes
    }

    /// Check if a volume (e.g., "C:") is on a specific physical drive
    /// This is a simplified version that uses a heuristic instead of exact Windows API calls
    fn is_volume_on_physical_drive(volume: &str, drive_number: usize) -> Result<bool> {
        // For cross-compilation purposes, we'll use a simplified approach that assumes
        // information from rs-drivelist is sufficient

        // This is a simpler fallback approach that doesn't use the native Windows IOCTL
        // We can use this method as a simplified way to check if we think a volume is on a physical drive

        // Simply try to open the volume and see if we can read from it
        let volume_path = format!(r"\\.\{}", volume.trim_end_matches('\\'));
        let path_wide: Vec<u16> = volume_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Try to open the volume
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
            return Ok(false); // Can't access this volume
        }

        // If we can open it, we'll check if the volume is accessible
        // This is a very simple approach - in a real implementation, we'd use the
        // IOCTL_STORAGE_GET_DEVICE_NUMBER to definitively check the drive number

        // For now, we close the handle and log that we're checking
        unsafe { CloseHandle(handle) };

        // Log this as an assumption rather than a confirmed fact
        debug!(
            "Checking volume {} - treating as potentially on physical drive {}",
            volume, drive_number
        );

        // For safety, let's treat this volume as potentially on the target drive
        // Better to dismount too many volumes than too few
        Ok(true)
    }
}

// Unit tests for Windows-specific functionality
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_disk_number_from_path() {
        // Test full Windows path with backslashes
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path(r"\\.\PhysicalDrive0").unwrap(),
            0
        );
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path(r"\\.\PhysicalDrive12").unwrap(),
            12
        );

        // Test uppercase variant
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path(r"\\.\PHYSICALDRIVE3").unwrap(),
            3
        );

        // Test without leading backslashes
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path("PhysicalDrive5").unwrap(),
            5
        );

        // Test uppercase without backslashes
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path("PHYSICALDRIVE7").unwrap(),
            7
        );

        // Test just the number
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path("9").unwrap(),
            9
        );

        // Test with text before or after
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path("Selected disk: PhysicalDrive11")
                .unwrap(),
            11
        );
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path("PhysicalDrive8 (External USB Drive)")
                .unwrap(),
            8
        );

        // Test invalid input
        assert!(WindowsDiskAccess::extract_disk_number_from_path("No disk number here").is_err());
        assert!(WindowsDiskAccess::extract_disk_number_from_path("").is_err());
    }

    #[test]
    fn test_extract_disk_number_from_path_robust() {
        // Test the robust regex-based implementation directly

        // Test full Windows path with backslashes (C++ style pattern)
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust(r"\\.\PHYSICALDRIVE0").unwrap(),
            0
        );
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust(r"\\.\PHYSICALDRIVE12")
                .unwrap(),
            12
        );

        // Test case insensitive matching
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust(r"\\.\physicaldrive3").unwrap(),
            3
        );
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust(r"\\.\PhysicalDrive5").unwrap(),
            5
        );

        // Test without leading backslashes
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("PhysicalDrive7").unwrap(),
            7
        );
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("PHYSICALDRIVE9").unwrap(),
            9
        );

        // Test just number
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("15").unwrap(),
            15
        );

        // Test paths with extra text (regex should find the pattern)
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust(
                "Selected disk: PhysicalDrive11"
            )
            .unwrap(),
            11
        );

        // Test mixed case patterns
        assert_eq!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("physicaldrive2").unwrap(),
            2
        );

        // Test invalid inputs
        assert!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("No disk number here").is_err()
        );
        assert!(WindowsDiskAccess::extract_disk_number_from_path_robust("").is_err());
        assert!(
            WindowsDiskAccess::extract_disk_number_from_path_robust("PhysicalDriveABC").is_err()
        );
        assert!(WindowsDiskAccess::extract_disk_number_from_path_robust("Drive5").is_err()); // Must contain "PhysicalDrive"
    }
}
