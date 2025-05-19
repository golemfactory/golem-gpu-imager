// Linux-specific disk operations

use crate::disk::common::{DiskDevice, PartitionFileProxy};
use anyhow::{Context, Result, anyhow};
// Keep gpt imported for GptDisk
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write, Seek};
use std::os::unix::io::{AsRawFd, FromRawFd};
use tracing::{debug, error, info, warn};

// Linux-specific imports
use libc::{O_CLOEXEC, O_EXCL, O_SYNC};
// Removed unused import: use udisks2::filesystem::FilesystemProxy;
use udisks2::zbus::zvariant::{ObjectPath, OwnedObjectPath};
use udisks2::{Client, zbus};

/// Linux-specific disk access functionality
#[derive(Debug, Clone)]
pub struct LinuxDiskAccess {
    // Original path used to open the disk
    path: String,
}

impl LinuxDiskAccess {
    /// Open and lock a disk by its path
    ///
    /// # Arguments
    /// * `path` - The path to the disk device (e.g., "/dev/sda")
    ///
    /// # Returns
    /// * `Result<(File, Self)>` - A tuple with the disk file handle and platform-specific data
    pub async fn lock_path(path: &str) -> Result<(File, Self)> {
        info!("Locking Linux disk path: {}", path);

        // Create the Linux UDisks2 client
        let client = Client::new().await?;

        // Resolve the device path to a UDisks2 object path
        let drive_path = Self::resolve_device(&client, path).await?;

        // Unmount all mounted partitions on this disk
        Self::umount_all(&client, drive_path.as_ref())
            .await
            .context("Failed to unmount partitions")?;

        // Get the block device interface
        let block = client.object(drive_path)?.block().await?;

        // Set up open flags: O_EXCL for exclusive access, O_SYNC for sync I/O, O_CLOEXEC to close on exec
        let flags = O_EXCL | O_SYNC | O_CLOEXEC;

        // Open the device with read-write access
        let owned_fd = block
            .open_device(
                "rw",
                [("flags", zbus::zvariant::Value::from(flags))]
                    .into_iter()
                    .collect(),
            )
            .await?;

        // Convert the file descriptor to a Rust File
        if let zbus::zvariant::Fd::Owned(owned_fd) = owned_fd.into() {
            // Create a Rust File from the file descriptor
            let file = std::fs::File::from(owned_fd);

            // Create the platform data
            let platform = LinuxDiskAccess {
                path: path.to_string(),
            };

            Ok((file, platform))
        } else {
            Err(anyhow!(
                "Failed to open device: UDisks2 did not provide an owned file descriptor"
            ))
        }
    }

    /// Clone a file handle (uses dup() on Linux)
    pub fn clone_file_handle(&self, file: &File) -> Result<File> {
        // Get the raw file descriptor
        let fd = file.as_raw_fd();

        // Use libc dup to duplicate the file descriptor
        let new_fd = unsafe { libc::dup(fd) };

        if new_fd < 0 {
            // An error occurred, get the error code
            let err = io::Error::last_os_error();
            return Err(anyhow!("Failed to duplicate file handle: {}", err));
        }

        // Convert the new file descriptor to a Rust File
        let new_file = unsafe { File::from_raw_fd(new_fd) };

        Ok(new_file)
    }

    /// Create a partition file proxy for Linux
    pub fn create_partition_proxy(
        file: File,
        partition_offset: u64,
        partition_size: u64,
    ) -> Result<PartitionFileProxy<impl Read + Write + Seek>> {
        // Create a basic proxy without any platform-specific handling
        Ok(PartitionFileProxy {
            file,
            partition_offset,
            partition_size,
            current_position: 0,
        })
    }

    /// Verify disk is ready for writing (Linux implementation)
    pub fn pre_write_checks(disk_file: &File) -> Result<()> {
        // Check basic disk access permissions
        match disk_file.try_clone() {
            Ok(mut test_file) => {
                // Test write permission with zero-byte write
                let write_test = test_file.write(&[]);
                if let Err(e) = write_test {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        error!("Linux disk error: Disk is write-protected, permission denied");
                        return Err(anyhow::anyhow!("The disk is write-protected and cannot be written to")
                            .context("Make sure you're running with appropriate permissions (sudo, root, etc.)")
                            .context("Check if the disk has a hardware write-protect switch"));
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

        info!("Linux: Disk is ready for writing");
        Ok(())
    }

    /// Handle disk write errors with Linux-specific context
    pub fn handle_write_error(e: &io::Error) -> Option<anyhow::Error> {
        let os_error = e.raw_os_error();

        // Log error details
        if let Some(code) = os_error {
            error!("Linux error code: {}, error: {}", code, e);

            match code {
                libc::EACCES => {
                    error!(
                        "Permission denied error (code {}) when writing to disk",
                        code
                    );
                    return Some(anyhow::anyhow!("Permission denied when writing to disk: {}", e)
                        .context("Make sure you're running with appropriate permissions (sudo, root, etc.)")
                        .context("The disk may be locked by another process or write-protected"));
                }
                libc::EIO => {
                    error!("I/O error (code {}) when writing to disk", code);
                    return Some(
                        anyhow::anyhow!("I/O error when writing to disk: {}", e)
                            .context("The disk may be damaged or have hardware issues")
                            .context("Try using a different USB port or disk")
                    );
                }
                libc::ENOSPC => {
                    error!("No space left error (code {}) when writing to disk", code);
                    return Some(
                        anyhow::anyhow!("No space left on disk: {}", e)
                            .context("Check that the disk has enough free space for the image")
                            .context("Try using a larger capacity disk")
                    );
                }
                libc::ENODEV => {
                    error!("No device error (code {}) when writing to disk", code);
                    return Some(
                        anyhow::anyhow!("Device not available: {}", e)
                            .context("The disk was disconnected during the write operation")
                            .context("Ensure the disk remains connected throughout the process")
                    );
                }
                _ => {
                    error!("Unrecognized Linux error code: {}", code);
                    return Some(
                        anyhow::anyhow!("Failed to write image to disk: {}", e)
                            .context("An unexpected Linux error occurred during disk write")
                            .context("Try checking dmesg or system logs for more information")
                    );
                }
            }
        }

        None
    }

    /// Handle disk flush errors with Linux-specific context
    pub fn handle_flush_error(e: &io::Error) -> Option<anyhow::Error> {
        let os_error = e.raw_os_error();
        if let Some(code) = os_error {
            error!("Linux flush error code: {}, error: {}", code, e);

            return Some(
                anyhow::anyhow!("Failed to flush disk buffer: {}", e)
                    .context("Unable to ensure all data was written to disk")
                    .context("The disk may have been disconnected or experienced an error")
            );
        }

        None
    }

    /// Handle GPT reading errors with Linux-specific solutions
    pub fn handle_gpt_error(
        disk: &crate::disk::Disk,
        error: anyhow::Error,
    ) -> Result<Option<gpt::GptDisk<'_>>> {
        warn!(
            "Failed to parse GPT partition table: {}. Attempting Linux-specific fixes.",
            error
        );

        // For Linux, we try with a different logical block size
        let cfg = gpt::GptConfig::new()
            .writable(false)
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096);

        // Clone the file handle and try again with different block size
        let disk_result = cfg.open_from_device(Box::new(disk.get_cloned_file_handle()?));

        if let Ok(disk) = disk_result {
            info!("Successfully reopened GPT disk with 4096-byte logical blocks");
            return Ok(Some(disk));
        }

        // If that didn't work, try with MBR instead of GPT
        warn!("Couldn't read as GPT with 4096-byte blocks, checking for MBR format");

        // Let the original error propagate
        Ok(None)
    }

    /// Resolve a device path to a UDisks2 object path
    async fn resolve_device(client: &Client, path: &str) -> Result<OwnedObjectPath> {
        debug!("Resolving Linux device path: {}", path);

        // Create specification for device lookup
        let mut spec = HashMap::new();
        spec.insert("path", path.into());

        // Resolve the device using UDisks2
        let mut obj = client
            .manager()
            .resolve_device(spec, HashMap::default())
            .await?;

        // Return the first object path found or an error if none were found
        Ok(obj
            .pop()
            .ok_or(anyhow!("No device found for path: {}", path))?)
    }

    /// Unmount all mounted filesystems on or below the given object path
    async fn umount_all(client: &Client, path: ObjectPath<'_>) -> Result<()> {
        debug!("Unmounting all filesystems on Linux device: {:?}", path);

        // Get all block devices from UDisks2
        let block_devices = client
            .manager()
            .get_block_devices(HashMap::default())
            .await?;

        for dev_path in block_devices {
            // Check if this device is on our target disk (it starts with the same path)
            let path_str = path.as_str();
            if dev_path.as_str().starts_with(path_str) {
                debug!("Checking device for mounted filesystems: {:?}", dev_path);

                // Clone the device path to avoid ownership issues
                let dev_path_clone = dev_path.clone();

                // Try to get the filesystem interface for this device
                if let Ok(d) = client.object(dev_path)?.filesystem().await {
                    // Check if there are any mount points
                    if !d.mount_points().await?.is_empty() {
                        info!("Unmounting filesystem on device: {:?}", dev_path_clone);
                        // Unmount the filesystem
                        d.unmount(HashMap::new()).await?;
                    }
                }
            }
        }

        debug!("Successfully unmounted all filesystems");
        Ok(())
    }

    /// List available disks on Linux
    pub async fn list_available_disks() -> Result<Vec<DiskDevice>> {
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
            debug!("Using fallback method to find Linux block devices");

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
                        debug!("Found device: {}", path);
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
}

// Unlike Windows, Linux doesn't need special handling for read/write operations
// as it doesn't have the same alignment requirements.
// The standard implementation in common.rs will work correctly.
