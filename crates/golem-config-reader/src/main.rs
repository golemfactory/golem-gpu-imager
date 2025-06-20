use anyhow::{Context, Result, anyhow};
use clap::Parser;
use fatfs;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::{self, fmt::format::FmtSpan};
use uuid::Uuid;

/// CLI tool to read the Golem config partition and list its files
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the disk device (e.g., "/dev/sda" on Linux, "\\.\PhysicalDrive0" or "C:" on Windows)
    #[clap(short, long)]
    disk: String,

    /// UUID of the config partition to look for (default: Golem config partition)
    #[clap(short, long, default_value = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71")]
    uuid: String,

    /// Output directory to save the contents of the files (optional)
    #[clap(short, long)]
    output_dir: Option<PathBuf>,
}

// Simplified model structs (copied from the main project)
#[derive(Debug, Copy, Clone, PartialEq)]
enum PaymentNetwork {
    Testnet,
    Mainnet,
}

impl fmt::Display for PaymentNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentNetwork::Testnet => write!(f, "testnet"),
            PaymentNetwork::Mainnet => write!(f, "mainnet"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum NetworkType {
    Central,
    Hybrid,
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkType::Central => write!(f, "central"),
            NetworkType::Hybrid => write!(f, "hybrid"),
        }
    }
}

#[derive(Debug)]
struct GolemConfig {
    pub payment_network: PaymentNetwork,
    pub network_type: NetworkType,
    pub subnet: String,
    pub wallet_address: String,
    pub glm_per_hour: String,
}

// Simplified disk struct
#[cfg(windows)]
// Define constants missing from windows-sys when cross-compiling
mod win_constants {
    // These match the Windows SDK values
    pub const GENERIC_READ: u32 = 0x80000000;
    pub const GENERIC_WRITE: u32 = 0x40000000;
}

struct Disk {
    file: File,
}

impl Disk {
    #[cfg(windows)]
    /// Helper function to dismount a volume on Windows
    fn dismount_volume(drive_path: &str) {
        use crate::win_constants::{GENERIC_READ, GENERIC_WRITE};
        use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::FSCTL_DISMOUNT_VOLUME;

        // Format the path correctly for Windows API
        // Convert \\.\C: to \\.\C:
        let volume_path = if drive_path.starts_with(r"\\.\") {
            drive_path.to_string()
        } else {
            format!(r"\\.\{}", drive_path)
        };

        // Convert to UTF-16 for Windows API
        let path_wide: Vec<u16> = volume_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Try to open the volume
        info!("Opening volume {} for dismounting", volume_path);
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
            warn!(
                "Could not open volume for dismounting: error code {}",
                error_code
            );
            return;
        }

        // Try to dismount the volume
        info!("Attempting to dismount volume {}", volume_path);
        let mut bytes_returned: u32 = 0;
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
            warn!("Could not dismount volume: error code {}", error_code);
        } else {
            info!("Successfully dismounted volume {}", volume_path);
        }

        // Close the handle
        unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
    }

    // Check if a partition exists on a disk
    #[cfg(windows)]
    fn check_has_partitions(path: &str) -> bool {
        use crate::win_constants::GENERIC_READ;
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::IOCTL_DISK_GET_DRIVE_LAYOUT;

        info!("Checking if disk {} has partitions", path);

        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

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
            warn!("Could not open disk to check partitions");
            return false;
        }

        // Use IOCTL_DISK_GET_DRIVE_LAYOUT to get disk information
        // We don't actually care about the data, just if the call succeeds
        let mut disk_layout_buffer = [0u8; 512]; // Buffer for disk layout info
        let mut bytes_returned: u32 = 0;

        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DRIVE_LAYOUT,
                std::ptr::null_mut(),
                0,
                disk_layout_buffer.as_mut_ptr() as *mut _,
                disk_layout_buffer.len() as u32,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        unsafe { CloseHandle(handle) };

        if result == 0 {
            warn!("Failed to get disk layout information");
            return false;
        }

        // If we got valid layout information, assume the disk has partitions
        true
    }

    // Check if a disk is busy by trying to get exclusive access
    #[cfg(windows)]
    fn check_disk_busy(path: &str) -> Result<bool> {
        use crate::win_constants::GENERIC_READ;
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};

        info!("Checking if disk {} is busy", path);

        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // Try to open with exclusive access
        let handle = unsafe {
            CreateFileW(
                path_wide.as_ptr(),
                GENERIC_READ,
                0, // No sharing - exclusive access
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let error_code = unsafe { GetLastError() };

            // Error code 32 = ERROR_SHARING_VIOLATION (disk is busy)
            if error_code == 32 {
                info!("Disk is busy (error code 32)");
                return Ok(true);
            }

            // For other errors, just report the disk is not busy
            warn!("Failed to check if disk is busy: error code {}", error_code);
            return Ok(false);
        }

        // If we could open with exclusive access, disk is not busy
        unsafe { CloseHandle(handle) };
        info!("Disk is not busy");
        Ok(false)
    }

    // Try to get drive letter for a physical disk on Windows
    #[cfg(windows)]
    fn get_drive_letter_for_disk(disk_num: usize) -> Option<String> {
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

        info!("Trying to find drive letter for PhysicalDrive{}", disk_num);

        // This is a simplistic approach - in a production environment we would need
        // to match volumes to physical disks via more complex methods
        let drive_bits = unsafe { GetLogicalDrives() };

        for i in 0..26 {
            if (drive_bits & (1 << i)) != 0 {
                let drive_letter = (b'A' + i as u8) as char;
                let drive_path = format!("{}:", drive_letter);

                info!("Found drive letter: {}", drive_path);

                // Check if this is a fixed disk
                let wide_path: Vec<u16> = format!("{}:\\", drive_letter)
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let drive_type = unsafe { GetDriveTypeW(wide_path.as_ptr()) };

                // DRIVE_FIXED = 3
                if drive_type == 3 {
                    info!("Drive {} is a fixed disk", drive_path);
                    return Some(drive_path);
                }
            }
        }

        // Could not find a matching drive letter
        None
    }

    // Try to determine partition style for a disk (MBR or GPT)
    #[cfg(windows)]
    fn check_partition_style(
        handle: windows_sys::Win32::Foundation::HANDLE,
    ) -> Option<&'static str> {
        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::IOCTL_DISK_GET_DRIVE_LAYOUT;

        // We'll use a generic buffer and just check the first u32 value which represents partition style
        let mut disk_layout_buffer = [0u8; 512];
        let mut bytes_returned: u32 = 0;

        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DRIVE_LAYOUT,
                std::ptr::null_mut(),
                0,
                disk_layout_buffer.as_mut_ptr() as *mut _,
                disk_layout_buffer.len() as u32,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if result == 0 {
            warn!("Failed to get partition style information");
            return None;
        }

        // First DWORD (4 bytes) indicates partition style
        // But we need to be careful with this approach as it's not as reliable
        // as using the proper structure
        let partition_style = u32::from_ne_bytes([
            disk_layout_buffer[0],
            disk_layout_buffer[1],
            disk_layout_buffer[2],
            disk_layout_buffer[3],
        ]);

        // 0 = MBR, 1 = GPT (these values are a simplified assumption)
        match partition_style {
            0 => Some("MBR"),
            1 => Some("GPT"),
            _ => Some("Unknown"),
        }
    }

    // Open a disk device
    async fn open(path: &str) -> Result<Self> {
        let path_str = if path.contains("PhysicalDrive") {
            format!(r"\\.\{}", path.trim_start_matches(r"\\.\"))
        } else if path.ends_with(":") {
            format!(r"\\.\{}", path.trim_end_matches('\\'))
        } else if path.parse::<usize>().is_ok() {
            format!(r"\\.\PhysicalDrive{}", path)
        } else {
            #[cfg(windows)]
            {
                format!(r"\\.\{}", path)
            }
            #[cfg(not(windows))]
            {
                path.to_string()
            }
        };

        info!("Opening disk: {}", path_str);

        // Open the disk with the appropriate permissions for the platform
        #[cfg(windows)]
        let file = {
            use crate::win_constants::GENERIC_READ;
            use std::os::windows::io::FromRawHandle;
            use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
            use windows_sys::Win32::Storage::FileSystem::{
                CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            };
            use windows_sys::Win32::System::IO::DeviceIoControl;
            use windows_sys::Win32::System::Ioctl::FSCTL_LOCK_VOLUME;

            info!("Windows: Using low-level CreateFileW API for disk access");

            // If the path is a numeric value, try to get the drive letter
            if let Ok(disk_num) = path.parse::<usize>() {
                if let Some(drive_letter) = Self::get_drive_letter_for_disk(disk_num) {
                    info!(
                        "Found drive letter {} for physical disk {}",
                        drive_letter, disk_num
                    );

                    // Check if disk is busy
                    let busy = Self::check_disk_busy(&format!(r"\\.\{}", drive_letter))?;
                    if busy {
                        // Try to dismount the volume to free it up
                        info!("Drive {} is busy, attempting to dismount", drive_letter);
                        Self::dismount_volume(&drive_letter);
                    }
                }
            }

            // If this is a drive letter (like "C:"), try to dismount the volume first
            if path_str.contains(":") && path_str.len() <= 5 {
                info!("Attempting to dismount volume {} before opening", path_str);
                Self::dismount_volume(&path_str);
            }

            // Convert path to UTF-16 for Windows API
            let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

            // Try different permissions strategies to open the disk
            info!(
                "Attempting to open disk {} with several permission strategies",
                path_str
            );

            // Try different access mode combinations with multiple retries
            let mut handle = INVALID_HANDLE_VALUE;
            let access_modes = [
                // Try exclusive read access first
                (GENERIC_READ, 0, "read access (exclusive)"),
                // Then try with shared read
                (GENERIC_READ, FILE_SHARE_READ, "read access with share_read"),
                // Try with full sharing
                (
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    "read access with full sharing",
                ),
                // As a last resort, try with FILE_ATTRIBUTE_NORMAL flag
                (
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    "read access with normal attributes",
                ),
            ];

            const MAX_RETRIES: usize = 3;
            let mut last_error_code = 0;

            'access_mode_loop: for (access_rights, share_mode, description) in access_modes.iter() {
                for retry in 1..=MAX_RETRIES {
                    info!(
                        "Trying to open disk with {} (attempt {}/{})",
                        description, retry, MAX_RETRIES
                    );

                    // Special case for the last access mode - use FILE_ATTRIBUTE_NORMAL
                    let file_flags = if description.contains("normal attributes") {
                        0x80 // FILE_ATTRIBUTE_NORMAL
                    } else {
                        0
                    };

                    handle = unsafe {
                        CreateFileW(
                            path_wide.as_ptr(),
                            *access_rights,
                            *share_mode,
                            std::ptr::null_mut(),
                            OPEN_EXISTING,
                            file_flags,
                            0,
                        )
                    };

                    if handle != INVALID_HANDLE_VALUE {
                        info!("Successfully opened disk with {}", description);

                        // Try to determine partition style
                        if let Some(style) = Self::check_partition_style(handle) {
                            info!("Detected {} partition style", style);

                            // For GPT disks, we might need special handling
                            if style == "GPT" {
                                info!("Detected GPT disk format");

                                // Instead of using FSCTL_ALLOW_EXTENDED_DASD_IO which may not be available,
                                // we'll just log that we found a GPT disk and continue
                                info!("Using standard direct I/O access for GPT disk");
                            }
                        }

                        break 'access_mode_loop;
                    } else {
                        last_error_code = unsafe { GetLastError() };
                        warn!(
                            "Failed to open with {}: error code {} (attempt {}/{})",
                            description, last_error_code, retry, MAX_RETRIES
                        );

                        // Small delay between retries
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }

            if handle == INVALID_HANDLE_VALUE {
                // Map error codes to friendly messages with detailed guidance
                let error_msg = match last_error_code {
                    5 => {
                        "Access denied. Please run as Administrator. For GPT disks, you may need to close any applications using the disk."
                    }
                    2 => {
                        "The system cannot find the disk specified. Make sure the disk path is correct."
                    }
                    32 => {
                        "The disk is in use by another process. Try dismounting the volume or closing any applications using the disk."
                    }
                    123 => {
                        "Invalid filename. Make sure you're using a valid format: PhysicalDrive0, C:, etc."
                    }
                    1 => {
                        "Invalid function. This might be a permissions issue with Windows security features."
                    }
                    _ => "Unknown error accessing disk",
                };

                error!(
                    "Failed to open disk: error code {} - {}",
                    last_error_code, error_msg
                );
                return Err(anyhow!(
                    "Failed to open disk: {} (error code {})",
                    error_msg,
                    last_error_code
                ));
            }

            // Try to lock the volume for exclusive access
            info!("Attempting to lock the volume for exclusive access");
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
                warn!(
                    "Could not lock volume, error code: {}. Continuing with caution.",
                    error_code
                );

                // If we couldn't lock, try to determine if volume is mounted
                if Self::check_has_partitions(&path_str) {
                    info!("Disk has partitions, continuing with caution");
                }
            } else {
                info!("Successfully locked volume");
            }

            // Convert Windows HANDLE to Rust File
            unsafe { File::from_raw_handle(handle as *mut _) }
        };

        #[cfg(not(windows))]
        let file = match File::open(&path_str) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open disk: {}", e);
                return Err(anyhow!(
                    "Failed to open disk: {}. Make sure you have sufficient permissions.",
                    e
                ));
            }
        };

        info!("Successfully opened disk: {}", path_str);
        Ok(Disk { file })
    }

    // Find a partition by UUID
    fn find_partition<'a>(
        &'a mut self,
        uuid_str: &str,
    ) -> Result<fatfs::FileSystem<PartitionFileProxy<&'a File>>> {
        // Parse the provided UUID string
        let target_uuid = Uuid::parse_str(uuid_str)
            .context(format!("Failed to parse UUID string: {}", uuid_str))?;

        info!("Looking for partition with UUID: {}", target_uuid);

        // Create a GPT configuration with the default logical block size (usually 512 bytes)
        let cfg = gpt::GptConfig::new().writable(false);

        // Parse GPT header and partition table from the disk
        let disk_result = cfg.open_from_device(Box::new(&self.file));

        let disk = match disk_result {
            Ok(d) => {
                info!("Successfully parsed GPT partition table");
                d
            }
            Err(e) => {
                // Log detailed error information to help diagnose GPT parsing issues
                error!("Failed to parse GPT partition table: {}", e);

                #[cfg(windows)]
                {
                    // On Windows, try to provide more specific guidance
                    error!(
                        "For Windows users: This might be due to insufficient permissions or the disk being in use."
                    );
                    error!(
                        "Try running as Administrator or using a different access method (PhysicalDrive# vs drive letter)."
                    );

                    // Try to get disk information to help diagnose
                    use std::os::windows::io::AsRawHandle;
                    use windows_sys::Win32::Foundation::GetLastError;
                    use windows_sys::Win32::System::IO::DeviceIoControl;
                    use windows_sys::Win32::System::Ioctl::IOCTL_DISK_GET_DRIVE_LAYOUT;

                    let handle =
                        self.file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
                    let mut disk_layout_buffer = [0u8; 512]; // Buffer for disk layout info
                    let mut bytes_returned: u32 = 0;

                    let result = unsafe {
                        DeviceIoControl(
                            handle,
                            IOCTL_DISK_GET_DRIVE_LAYOUT,
                            std::ptr::null_mut(),
                            0,
                            disk_layout_buffer.as_mut_ptr() as *mut _,
                            disk_layout_buffer.len() as u32,
                            &mut bytes_returned,
                            std::ptr::null_mut(),
                        )
                    };

                    if result == 0 {
                        let error_code = unsafe { GetLastError() };
                        error!(
                            "Failed to get disk layout information. Error code: {}",
                            error_code
                        );
                    } else {
                        info!(
                            "Successfully retrieved disk layout information ({} bytes)",
                            bytes_returned
                        );
                        // First DWORD (4 bytes) of the DRIVE_LAYOUT_INFORMATION_EX struct is the partition style
                        let partition_style = u32::from_ne_bytes([
                            disk_layout_buffer[0],
                            disk_layout_buffer[1],
                            disk_layout_buffer[2],
                            disk_layout_buffer[3],
                        ]);

                        match partition_style {
                            0 => info!("Disk has MBR partition style"),
                            1 => info!("Disk has GPT partition style"),
                            _ => info!("Disk has unknown partition style: {}", partition_style),
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    // On Linux, try to provide different guidance
                    error!(
                        "For Linux users: This might be due to insufficient permissions. Try running with sudo."
                    );
                }

                return Err(anyhow!(
                    "Failed to parse GPT partition table: {}. The disk may not have a valid GPT.",
                    e
                ));
            }
        };

        // Get partitions from the disk
        let partitions = disk.partitions();
        info!("Found {} partitions on disk", partitions.len());

        // Find the partition with matching UUID
        for (i, (_, part)) in partitions.iter().enumerate() {
            info!(
                "Checking partition {}: UUID={}, Name={}",
                i, part.part_guid, part.name
            );

            // Check for matching UUID
            if part.part_guid == target_uuid {
                info!("Found partition with UUID {}: {}", target_uuid, part.name);

                // Get start sector and length for the partition
                let start_sector = part.first_lba;
                const SECTOR_SIZE: u64 = 512;
                let start_offset = start_sector * SECTOR_SIZE;

                // Get partition size for better boundary checking
                let partition_size = part
                    .last_lba
                    .checked_sub(part.first_lba)
                    .map(|sectors| sectors * SECTOR_SIZE)
                    .unwrap_or(0);

                info!(
                    "Partition starts at sector {} (offset: {}) with size: {} bytes ({} MB)",
                    start_sector,
                    start_offset,
                    partition_size,
                    partition_size / (1024 * 1024)
                );

                // Create a PartitionFileProxy that handles seeks relative to the partition
                let proxy = PartitionFileProxy {
                    file: &self.file,
                    partition_offset: start_offset,
                    partition_size,
                    current_position: 0,
                };

                // Attempt to create a FAT filesystem from the partition
                info!("Attempting to open FAT filesystem from partition");
                let fs_result = fatfs::FileSystem::new(proxy, fatfs::FsOptions::new());

                match fs_result {
                    Ok(fs) => {
                        info!("Successfully opened FAT filesystem");
                        return Ok(fs);
                    }
                    Err(e) => {
                        error!("Failed to open FAT filesystem: {}", e);
                        return Err(anyhow!(
                            "Found partition with matching UUID, but could not open as FAT filesystem: {}. The partition may be corrupt or not formatted as FAT.",
                            e
                        ));
                    }
                }
            }
        }

        // If we have partitions but none match, list all UUIDs to help debugging
        if !partitions.is_empty() {
            error!(
                "No partition with UUID {} found. Available partitions:",
                target_uuid
            );
            for (i, (_, part)) in partitions.iter().enumerate() {
                error!("  {}: UUID={}, Name={}", i, part.part_guid, part.name);
            }
        }

        // No partition with matching UUID found
        Err(anyhow!(
            "No partition found with UUID: {}. Make sure the disk contains a Golem configuration partition.",
            uuid_str
        ))
    }

    // Read Golem configuration from a partition
    fn read_configuration(&mut self, uuid_str: &str) -> Result<GolemConfig> {
        // Find the partition with the given UUID
        let fs = self.find_partition(uuid_str)?;

        // Get the root directory
        let root_dir = fs.root_dir();

        // Default values in case some files or settings are missing
        let mut config = GolemConfig {
            payment_network: PaymentNetwork::Testnet,
            network_type: NetworkType::Central,
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
                        "hybrid" => NetworkType::Hybrid,
                        _ => NetworkType::Central,
                    };
                } else if line.starts_with("SUBNET=") {
                    config.subnet = line.trim_start_matches("SUBNET=").trim().to_string();
                } else if line.starts_with("YA_PAYMENT_NETWORK_GROUP=") {
                    let value = line.trim_start_matches("YA_PAYMENT_NETWORK_GROUP=").trim();
                    config.payment_network = match value.to_lowercase().as_str() {
                        "mainnet" => PaymentNetwork::Mainnet,
                        _ => PaymentNetwork::Testnet,
                    };
                }
            }
        }

        Ok(config)
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

/// Proxy for accessing a specific partition on a disk
pub struct PartitionFileProxy<T: Read + Write + Seek> {
    /// The underlying file handle for the entire disk
    pub file: T,
    /// The offset in bytes where the partition starts
    pub partition_offset: u64,
    /// The size of the partition in bytes
    pub partition_size: u64,
    /// The current position relative to the start of the partition
    pub current_position: u64,
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
}

impl<T: Read + Write + Seek> Read for PartitionFileProxy<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position()?;

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

        // Calculate the absolute position
        let current_abs_pos = self.to_absolute_position();

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

impl<T: Read + Write + Seek> Write for PartitionFileProxy<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check position is within partition boundaries
        self.check_position()?;

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

        // Calculate the absolute position
        let current_abs_pos = self.to_absolute_position();

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

/// Save the content of a file to the output directory
fn save_file_content(
    root_dir: &fatfs::Dir<impl std::io::Read + std::io::Write + std::io::Seek>,
    file_name: &str,
    output_dir: &PathBuf,
) -> Result<()> {
    use std::fs::File;
    use std::io::Read;
    use std::io::Write;

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Open source file in the FAT filesystem
    let mut src_file = root_dir.open_file(file_name)?;
    let mut contents = String::new();

    info!("Reading file content: {}", file_name);
    match src_file.read_to_string(&mut contents) {
        Ok(_) => {}
        Err(e) => {
            // If we can't read as UTF-8 string, try binary mode
            warn!("Failed to read file as text, trying binary mode: {}", e);
            let mut binary_content = Vec::new();
            src_file.seek(SeekFrom::Start(0))?;
            src_file.read_to_end(&mut binary_content)?;

            // Create and write to destination file in binary mode
            let dest_path = output_dir.join(file_name);
            let mut dest_file = File::create(&dest_path)?;
            dest_file.write_all(&binary_content)?;

            info!("Saved binary file: {:?}", dest_path);
            println!("Saved binary file: {:?}", dest_path);
            return Ok(());
        }
    }

    // Create and write to destination file as text
    let dest_path = output_dir.join(file_name);
    let mut dest_file = File::create(&dest_path)?;
    dest_file.write_all(contents.as_bytes())?;

    info!("Saved text file: {:?}", dest_path);
    println!("Saved file: {:?}", dest_path);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Set up tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_span_events(FmtSpan::CLOSE)
        .init();

    println!("Golem Config Reader");
    println!("==================");
    println!();

    info!("Reading Golem config from disk: {}", args.disk);
    info!("Looking for partition UUID: {}", args.uuid);

    println!("Reading Golem configuration from disk: {}", args.disk);
    println!("Looking for partition UUID: {}", args.uuid);

    // Open disk
    let disk_result = Disk::open(&args.disk).await;

    let mut disk = match disk_result {
        Ok(d) => {
            println!("Successfully opened disk: {}", args.disk);
            d
        }
        Err(e) => {
            println!("Failed to open disk: {}", e);
            println!();

            #[cfg(windows)]
            {
                println!("Windows troubleshooting tips:");
                println!("  - Make sure you're running the tool as Administrator");
                println!("  - Close any applications that might be using the disk");
                println!("  - Try specifying the disk using different formats:");
                println!("    • PhysicalDrive number (e.g., '2' for PhysicalDrive2)");
                println!("    • Drive letter (e.g., 'E:')");
                println!("    • Full device path (e.g., '\\\\?\\Device\\Harddisk2\\Partition0')");
            }

            #[cfg(not(windows))]
            {
                println!("Linux troubleshooting tips:");
                println!("  - Make sure you're running the tool with sudo");
                println!("  - Check that the disk path exists: ls -l {}", args.disk);
                println!("  - Make sure you have read permissions for the disk");
            }

            return Err(e);
        }
    };

    // First, read configuration from config partition
    info!("Reading Golem configuration...");
    println!("Reading Golem configuration...");
    let config_result = disk.read_configuration(&args.uuid);

    // Now find the partition again for file listing
    info!("Searching for config partition for file listing...");
    println!("Searching for config partition to list files...");
    let fs_result = disk.find_partition(&args.uuid);

    let fs = match fs_result {
        Ok(fs) => {
            println!("Found Golem config partition");
            fs
        }
        Err(e) => {
            println!("Failed to find Golem config partition: {}", e);
            println!();

            #[cfg(windows)]
            {
                println!("Windows troubleshooting tips for GPT partition access:");
                println!("  - If using a GPT disk, make sure the disk isn't in use by Windows");
                println!("  - Try using a different physical disk access method");
                println!("  - Ensure the Golem device has the correct configuration partition");
            }

            return Err(anyhow!("Failed to find config partition"));
        }
    };

    // Get the root directory
    let root_dir = fs.root_dir();

    // List files in the directory
    info!("Found partition, listing files...");

    let entries_result = root_dir.iter().collect::<Result<Vec<_>, _>>();

    let entries = match entries_result {
        Ok(entries) => entries,
        Err(e) => {
            println!("Failed to list files in partition: {}", e);
            return Err(anyhow!("Failed to list files in partition"));
        }
    };

    if entries.is_empty() {
        info!("No files found in the partition");
        println!("No files found in the partition");
    } else {
        // Calculate column width for nice formatting
        let max_name_len = entries
            .iter()
            .map(|entry| entry.file_name().len())
            .max()
            .unwrap_or(10);

        // Print header
        println!("\nFiles found in config partition:");
        println!("--------------------------------");
        println!(
            "{:<width$} {:>10} {:<20}",
            "Filename",
            "Size",
            "Last Modified",
            width = max_name_len
        );
        println!(
            "{:-<width$} {:->10} {:-<20}",
            "",
            "",
            "",
            width = max_name_len
        );

        // Print each file with details
        for entry in entries {
            let file_name = entry.file_name();

            // Get file attributes - FatFS doesn't have metadata like std::fs
            let attributes = entry.attributes();
            // Directory flag is 0x10 in FAT
            let is_dir = attributes.contains(fatfs::FileAttributes::DIRECTORY);

            // Get file size - need to read for FAT filesystem
            let size = if !is_dir {
                // Read the file to determine its size
                let mut contents = Vec::new();
                match root_dir.open_file(&file_name) {
                    Ok(mut file) => {
                        if let Ok(_) = file.read_to_end(&mut contents) {
                            contents.len() as u64
                        } else {
                            0
                        }
                    }
                    Err(_) => 0,
                }
            } else {
                0
            };

            // Format modified time if available
            let mtime = entry.modified();
            let modified = format!("{:?}", mtime);

            println!(
                "{:<width$} {:>10} {:<20}",
                file_name,
                size,
                modified,
                width = max_name_len
            );

            info!("Found file: {} (size: {} bytes)", file_name, size);

            // If output directory is specified, copy the file contents
            if let Some(ref output_dir) = args.output_dir {
                if !is_dir {
                    if let Err(e) = save_file_content(&root_dir, &file_name, output_dir) {
                        println!("Failed to save file {}: {}", file_name, e);
                    }
                }
            }
        }

        // Print summary of saved files if output directory was specified
        if let Some(ref output_dir) = args.output_dir {
            println!("\nFiles saved to: {:?}", output_dir);
        }
    }

    // Display the configuration results
    info!("Displaying Golem configuration...");
    match config_result {
        Ok(config) => {
            info!("Successfully read configuration");

            println!("\nGolem Configuration:");
            println!("-------------------");
            println!("Payment Network: {:?}", config.payment_network);
            println!("Network Type:    {:?}", config.network_type);
            println!("Subnet:          {}", config.subnet);
            println!("Wallet Address:  {}", config.wallet_address);
            println!("GLM per Hour:    {}", config.glm_per_hour);
        }
        Err(e) => {
            error!("Failed to read configuration: {}", e);
            println!("Failed to read configuration: {}", e);
        }
    }

    println!("\nOperation completed");

    Ok(())
}
