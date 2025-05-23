use anyhow::{Context, Result};
use clap::Parser;
use gpt::GptConfig;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
// use std::io::SeekFrom::Start; // Uncomment if needed
use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use gpt::disk::LogicalBlockSize;
// use iced::mouse::Cursor; // Uncomment if needed
use tracing::{debug, info, error, warn};

// Import Windows-specific aligned I/O implementation
#[cfg(windows)]
use golem_gpu_imager::disk::aligned_disk_io;

/// CLI tool to list partitions on a disk using the gpt crate directly
#[derive(Parser, Debug)]
#[clap(name = "golem-partition-lister", about = "List partitions on a disk")]
struct Args {
    /// Path to the disk device (e.g., /dev/sda, \\.\PhysicalDrive0)
    #[clap(short, long)]
    device: String,

    /// Show detailed partition information
    #[clap(short, long)]
    verbose: bool,
    
    /// Try alternative partition reading methods if GPT fails
    #[clap(short, long)]
    fallback: bool,
    
    /// Force raw sector dump (for debugging)
    #[clap(long)]
    raw: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // If raw dump is requested, bypass partition parsing and dump sectors
    if args.raw {
        return dump_raw_sectors(&args.device);
    }

    tracing_subscriber::fmt()
        .with_env_filter("debug,gpt=trace")
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("starting 2");
    
    // Try to list partitions with GPT
    let result = list_partitions(&args.device, args.verbose);
    
    match result {
        Ok(partitions) => {
            // Print partitions to console
            if partitions.is_empty() {
                println!("No partitions found on device: {}", args.device);
            } else {
                println!("Found {} partitions on device: {}", partitions.len(), args.device);
                for (i, partition) in partitions.iter().enumerate() {
                    println!("{}. {}", i+1, partition);
                }
            }
        },
        Err(e) => {
            // If GPT parsing failed and fallback is enabled, try alternative methods
            if args.fallback {
                println!("\nGPT parsing failed. Trying fallback methods...");
                
                #[cfg(windows)]
                {
                    println!("Attempting to use Windows-specific APIs...");
                    if let Err(fallback_err) = list_partitions_windows_fallback(&args.device) {
                        eprintln!("Windows fallback method failed: {}", fallback_err);
                    }
                }
                
                #[cfg(unix)]
                {
                    println!("Attempting to use Linux-specific methods...");
                    if let Err(fallback_err) = list_partitions_linux_fallback(&args.device) {
                        eprintln!("Linux fallback method failed: {}", fallback_err);
                    }
                }
            } else {
                // Just return the original error if no fallback requested
                return Err(e);
            }
        }
    }
    
    Ok(())
}

/// Simple structure to hold partition information
#[derive(Debug)]
struct PartitionInfo {
    number: u32,
    name: String,
    uuid: String,
    first_lba: u64,
    last_lba: u64,
    size_mb: f64,
    partition_type: String,
}

impl std::fmt::Display for PartitionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Partition {} - {}: {} ({:.2} MB)",
            self.number, self.name, self.uuid, self.size_mb
        )
    }
}

/// Dump raw sectors from a disk for debugging purposes
///
/// This function reads and prints the first few sectors of a disk in hex format.
/// Useful for diagnosing partition table issues.
///
/// # Arguments
/// * `device_path` - Path to the disk device
///
/// # Returns
/// * Result with unit type on success
fn dump_raw_sectors(device_path: &str) -> Result<()> {
    println!("Dumping raw sectors from device: {}", device_path);
    
    // Open the device
    let path = PathBuf::from(device_path);
    
    // Different open methods depending on the platform
    let mut file = if cfg!(windows) {
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            
            println!("Opening with Windows-specific options...");
            // On Windows, use OpenOptionsExt to set access flags
            OpenOptions::new()
                .read(true)
                // FILE_SHARE_READ | FILE_SHARE_WRITE (0x1 | 0x2) allows other processes to read/write
                .custom_flags(0x1 | 0x2)
                .open(&path)
                .with_context(|| format!("Failed to open device at path: {}", device_path))?
        }
        
        #[cfg(not(windows))]
        {
            // This branch is unreachable, but needed for compilation
            File::open(&path)
                .with_context(|| format!("Failed to open device at path: {}", device_path))?
        }
    } else {
        println!("Opening with standard options...");
        File::open(&path)
            .with_context(|| format!("Failed to open device at path: {}", device_path))?
    };
    
    // Constants for sector operations
    const SECTOR_SIZE: usize = 512;
    const MBR_SECTOR: usize = 0;
    const GPT_HEADER_SECTOR: usize = 1;
    const GPT_ENTRIES_START_SECTOR: usize = 2;
    
    // Number of sectors to read for different scan modes
    const BASIC_SCAN_SECTORS: usize = 4;     // MBR + GPT header + First entries
    const DETAILED_SCAN_SECTORS: usize = 68; // Covers all possible GPT entries (34 sectors) + MBR + headers
    
    // Read the first part of the disk to analyze key structures
    let sectors_to_read = DETAILED_SCAN_SECTORS;
    let mut buffer = vec![0u8; SECTOR_SIZE * sectors_to_read];
    
    use std::io::{Read, Seek, SeekFrom};
    
    // Seek to beginning of disk
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("Failed to seek to start of device: {}", device_path))?;
    
    let bytes_read = file.read(&mut buffer)
        .with_context(|| format!("Failed to read sectors from device: {}", device_path))?;
    
    println!("Read {} bytes from device ({} sectors)", bytes_read, bytes_read / SECTOR_SIZE);
    
    // Function to validate EFI PART signature in GPT header
    let check_gpt_signature = |sector_data: &[u8]| -> bool {
        if sector_data.len() < 8 {
            return false;
        }
        
        // "EFI PART" signature (45 46 49 20 50 41 52 54)
        let signature = [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54];
        sector_data[0..8] == signature
    };
    
    // Function to check MBR boot signature
    let check_mbr_signature = |sector_data: &[u8]| -> bool {
        if sector_data.len() < SECTOR_SIZE {
            return false;
        }
        
        // MBR boot signature (55 AA at offset 510-511)
        sector_data[510] == 0x55 && sector_data[511] == 0xAA
    };
    
    // Analyze MBR (Sector 0)
    if bytes_read >= SECTOR_SIZE {
        let mbr_sector = &buffer[0..SECTOR_SIZE];
        println!("\n=== MBR SECTOR (LBA 0) ANALYSIS ===");
        
        if check_mbr_signature(mbr_sector) {
            println!("✓ Valid MBR boot signature (55 AA) found");
            
            // Check for protective MBR for GPT
            let partition_type = mbr_sector[450]; // First partition type
            if partition_type == 0xEE {
                println!("✓ Protective MBR for GPT detected (partition type EE)");
            } else {
                println!("✗ No protective MBR for GPT (partition type: {:02X})", partition_type);
                println!("  This might be a standard MBR disk, not GPT");
                
                // Try to decode MBR partitions
                println!("\nMBR Partition Table:");
                for i in 0..4 {
                    let offset = 446 + (i * 16);
                    let _status = mbr_sector[offset]; // Status byte
                    let type_code = mbr_sector[offset + 4];
                    
                    if type_code != 0 {
                        let start_sector = u32::from_le_bytes([
                            mbr_sector[offset + 8], 
                            mbr_sector[offset + 9], 
                            mbr_sector[offset + 10], 
                            mbr_sector[offset + 11]
                        ]);
                        
                        let sector_count = u32::from_le_bytes([
                            mbr_sector[offset + 12], 
                            mbr_sector[offset + 13], 
                            mbr_sector[offset + 14], 
                            mbr_sector[offset + 15]
                        ]);
                        
                        println!("  Partition {}: Type {:02X}, Start Sector: {}, Sectors: {} ({}MB)",
                            i + 1, type_code, start_sector, sector_count, 
                            (sector_count as f64 * SECTOR_SIZE as f64) / (1024.0 * 1024.0));
                    }
                }
            }
        } else {
            println!("✗ Invalid MBR boot signature - not a standard MBR");
        }
    }
    
    // Analyze Primary GPT Header (Sector 1)
    if bytes_read >= SECTOR_SIZE * 2 {
        let gpt_header_sector = &buffer[SECTOR_SIZE..SECTOR_SIZE*2];
        println!("\n=== PRIMARY GPT HEADER (LBA 1) ANALYSIS ===");
        
        if check_gpt_signature(gpt_header_sector) {
            println!("✓ Valid GPT signature ('EFI PART') found");
            
            // Extract GPT header fields
            let revision = u32::from_le_bytes([
                gpt_header_sector[8], gpt_header_sector[9], 
                gpt_header_sector[10], gpt_header_sector[11]
            ]);
            
            let header_size = u32::from_le_bytes([
                gpt_header_sector[12], gpt_header_sector[13], 
                gpt_header_sector[14], gpt_header_sector[15]
            ]);
            
            let current_lba = u64::from_le_bytes([
                gpt_header_sector[24], gpt_header_sector[25], 
                gpt_header_sector[26], gpt_header_sector[27],
                gpt_header_sector[28], gpt_header_sector[29], 
                gpt_header_sector[30], gpt_header_sector[31]
            ]);
            
            let backup_lba = u64::from_le_bytes([
                gpt_header_sector[32], gpt_header_sector[33], 
                gpt_header_sector[34], gpt_header_sector[35],
                gpt_header_sector[36], gpt_header_sector[37], 
                gpt_header_sector[38], gpt_header_sector[39]
            ]);
            
            let first_usable_lba = u64::from_le_bytes([
                gpt_header_sector[40], gpt_header_sector[41], 
                gpt_header_sector[42], gpt_header_sector[43],
                gpt_header_sector[44], gpt_header_sector[45], 
                gpt_header_sector[46], gpt_header_sector[47]
            ]);
            
            let last_usable_lba = u64::from_le_bytes([
                gpt_header_sector[48], gpt_header_sector[49], 
                gpt_header_sector[50], gpt_header_sector[51],
                gpt_header_sector[52], gpt_header_sector[53], 
                gpt_header_sector[54], gpt_header_sector[55]
            ]);
            
            let partition_entries_lba = u64::from_le_bytes([
                gpt_header_sector[72], gpt_header_sector[73], 
                gpt_header_sector[74], gpt_header_sector[75],
                gpt_header_sector[76], gpt_header_sector[77], 
                gpt_header_sector[78], gpt_header_sector[79]
            ]);
            
            let num_partition_entries = u32::from_le_bytes([
                gpt_header_sector[80], gpt_header_sector[81], 
                gpt_header_sector[82], gpt_header_sector[83]
            ]);
            
            let partition_entry_size = u32::from_le_bytes([
                gpt_header_sector[84], gpt_header_sector[85], 
                gpt_header_sector[86], gpt_header_sector[87]
            ]);
            
            println!("  GPT Revision: {:X}.{:02X}", (revision >> 16) & 0xFFFF, revision & 0xFFFF);
            println!("  Header Size: {} bytes", header_size);
            println!("  Current LBA: {}", current_lba);
            println!("  Backup LBA: {}", backup_lba);
            println!("  First Usable LBA: {}", first_usable_lba);
            println!("  Last Usable LBA: {}", last_usable_lba);
            println!("  Partition Entries LBA: {}", partition_entries_lba);
            println!("  Number of Partition Entries: {}", num_partition_entries);
            println!("  Size of Partition Entry: {} bytes", partition_entry_size);
            
            // Calculate where the partition entries should be
            let expected_entries_start = partition_entries_lba * SECTOR_SIZE as u64;
            println!("  Expected partition entries start at byte offset: {}", expected_entries_start);
            
            // Check if the partition entries start at LBA 2 (normal for primary GPT)
            if partition_entries_lba == 2 {
                println!("✓ Partition entries start at expected location (LBA 2)");
            } else {
                println!("! Partition entries start at non-standard location: LBA {}", partition_entries_lba);
            }
        } else {
            println!("✗ Invalid GPT signature - not a GPT disk or damaged header");
        }
    }
    
    // Try to analyze partition entries (starting at LBA 2)
    if bytes_read >= SECTOR_SIZE * 3 {
        println!("\n=== GPT PARTITION ENTRIES ANALYSIS ===");
        
        // GPT partition entry is typically 128 bytes
        const GPT_ENTRY_SIZE: usize = 128;
        
        // Check for partition entries
        let mut found_partitions = false;
        let mut partition_count = 0;
        
        // Read each potential partition entry from sector 2 onwards
        for entry_idx in 0..16 { // Look at first 16 potential entries
            let entry_offset = SECTOR_SIZE * GPT_ENTRIES_START_SECTOR + entry_idx * GPT_ENTRY_SIZE;
            
            if entry_offset + GPT_ENTRY_SIZE > bytes_read {
                break;
            }
            
            let entry = &buffer[entry_offset..entry_offset + GPT_ENTRY_SIZE];
            
            // Check if this is a non-empty partition entry
            // A valid partition type GUID should not be all zeros
            let is_empty = entry.iter().take(16).all(|&b| b == 0);
            
            if !is_empty {
                if !found_partitions {
                    found_partitions = true;
                    println!("Found partition entries:");
                }
                
                partition_count += 1;
                
                // Extract type GUID (first 16 bytes)
                let type_guid = format!(
                    "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    entry[0], entry[1], entry[2], entry[3], 
                    entry[4], entry[5], entry[6], entry[7], 
                    entry[8], entry[9], entry[10], entry[11], 
                    entry[12], entry[13], entry[14], entry[15]
                );
                
                // Extract partition GUID (next 16 bytes)
                let part_guid = format!(
                    "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    entry[16], entry[17], entry[18], entry[19], 
                    entry[20], entry[21], entry[22], entry[23], 
                    entry[24], entry[25], entry[26], entry[27], 
                    entry[28], entry[29], entry[30], entry[31]
                );
                
                // Extract starting and ending LBAs
                let first_lba = u64::from_le_bytes([
                    entry[32], entry[33], entry[34], entry[35],
                    entry[36], entry[37], entry[38], entry[39]
                ]);
                
                let last_lba = u64::from_le_bytes([
                    entry[40], entry[41], entry[42], entry[43],
                    entry[44], entry[45], entry[46], entry[47]
                ]);
                
                // Try to extract partition name (UCS-2 encoded, 36 chars max)
                let mut name = String::new();
                for i in (56..entry.len()).step_by(2) {
                    if i + 1 >= entry.len() {
                        break;
                    }
                    
                    let code_point = u16::from_le_bytes([entry[i], entry[i+1]]);
                    if code_point == 0 {
                        break;
                    }
                    
                    if let Some(c) = std::char::from_u32(code_point as u32) {
                        name.push(c);
                    }
                }
                
                // Print partition info
                let size_mb = ((last_lba - first_lba + 1) * SECTOR_SIZE as u64) as f64 / (1024.0 * 1024.0);
                
                let partition_type = match type_guid.as_str() {
                    "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => "EFI System",
                    "0fc63daf-8483-4772-8e79-3d69d8477de4" => "Linux Filesystem",
                    "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f" => "Linux Swap",
                    "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" => "Windows Data",
                    "48465300-0000-11aa-aa11-00306543ecac" => "APFS",
                    "7c3457ef-0000-11aa-aa11-00306543ecac" => "Apple HFS+",
                    "00000000-0000-0000-0000-000000000000" => "Unused",
                    _ => "Unknown",
                };
                
                println!("  Partition {}: {} - {}", 
                    partition_count, 
                    if !name.is_empty() { &name } else { "[Unnamed]" },
                    partition_type
                );
                println!("    Type GUID: {}", type_guid);
                println!("    Part GUID: {}", part_guid);
                println!("    Range: LBA {} - {} ({:.2} MB)", first_lba, last_lba, size_mb);
            }
        }
        
        if !found_partitions {
            println!("No valid partition entries found in the examined sectors.");
            println!("This could indicate a non-GPT disk or the data structure is damaged.");
        } else {
            println!("Total partitions found: {}", partition_count);
        }
    }
    
    // Display raw sectors (with a limit to avoid overwhelming output)
    println!("\n=== RAW SECTOR DATA ===");
    let sectors_to_display = std::cmp::min(BASIC_SCAN_SECTORS, bytes_read / SECTOR_SIZE);
    println!("Displaying first {} sectors:", sectors_to_display);
    
    for sector in 0..sectors_to_display {
        println!("\nSector {}:", sector);
        let sector_start = sector * SECTOR_SIZE;
        let sector_end = sector_start + SECTOR_SIZE.min(bytes_read - sector_start);
        
        // Print header
        print!("      ");
        for i in 0..16 {
            print!(" {:2X}", i);
        }
        println!();
        
        // Print sector data in hex, 16 bytes per line
        for row in 0..(SECTOR_SIZE / 16) {
            let row_start = sector_start + row * 16;
            let row_end = (row_start + 16).min(sector_end);
            
            if row_start >= sector_end {
                break;
            }
            
            // Print offset
            print!("{:04X}: ", row * 16);
            
            // Print hex values
            for i in row_start..row_end {
                print!("{:02X} ", buffer[i]);
            }
            
            // Fill any remaining space if we're at the end
            for _ in row_end..row_start + 16 {
                print!("   ");
            }
            
            // Print ASCII representation
            print!(" | ");
            for i in row_start..row_end {
                let c = buffer[i];
                if c >= 32 && c <= 126 {
                    // Printable ASCII
                    print!("{}", c as char);
                } else {
                    // Non-printable
                    print!(".");
                }
            }
            println!();
        }
    }
    
    Ok(())
}

/// List partitions on Windows using a fallback method
///
/// This function uses Windows-specific APIs to read the partition table
/// when the GPT crate fails.
///
/// # Arguments
/// * `device_path` - Path to the disk device
///
/// # Returns
/// * Result with unit type on success
#[cfg(windows)]
fn list_partitions_windows_fallback(_device_path: &str) -> Result<()> {
    println!("Windows fallback method not fully implemented yet.");
    println!("For debugging, please use the --raw flag to dump sectors.");
    println!("Then check if your device path is correct.");
    
    // List common Windows disk paths for reference
    println!("\nCommon Windows disk paths:");
    println!("  First physical disk: \\\\.\\PhysicalDrive0");
    println!("  Second physical disk: \\\\.\\PhysicalDrive1");
    println!("  C: drive: \\\\.\\C:");
    
    Ok(())
}

/// List partitions on Linux using a fallback method
///
/// This function uses Linux-specific tools to read the partition table
/// when the GPT crate fails.
///
/// # Arguments
/// * `device_path` - Path to the disk device
///
/// # Returns
/// * Result with unit type on success
#[cfg(unix)]
fn list_partitions_linux_fallback(device_path: &str) -> Result<()> {
    use std::process::Command;
    
    println!("Trying to use 'lsblk' to list partitions...");
    
    // Run lsblk command
    let output = Command::new("lsblk")
        .arg("-o")
        .arg("NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,LABEL,UUID")
        .arg(device_path)
        .output()
        .with_context(|| "Failed to execute lsblk command")?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("\nlsblk output:\n{}", stdout);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("lsblk error: {}", stderr);
        
        // Try fdisk as an alternative
        println!("\nTrying 'fdisk -l' as an alternative...");
        let fdisk_output = Command::new("fdisk")
            .arg("-l")
            .arg(device_path)
            .output()
            .with_context(|| "Failed to execute fdisk command")?;
        
        if fdisk_output.status.success() {
            let stdout = String::from_utf8_lossy(&fdisk_output.stdout);
            println!("\nfdisk output:\n{}", stdout);
        } else {
            let stderr = String::from_utf8_lossy(&fdisk_output.stderr);
            println!("fdisk error: {}", stderr);
            return Err(anyhow::anyhow!("Both lsblk and fdisk failed to list partitions"));
        }
    }
    
    Ok(())
}

/// List partitions on a disk
/// 
/// This function opens a disk and lists all GPT partitions using the gpt crate.
/// 
/// # Arguments
/// * `device_path` - Path to the disk device
/// * `verbose` - Whether to include detailed information
/// 
/// # Returns
/// * A vector of PartitionInfo structs
fn list_partitions(device_path: &str, verbose: bool) -> Result<Vec<PartitionInfo>> {

    // Print access information
    println!("Attempting to access device: {}", device_path);
    
    // Windows specific guidance if applicable
    if cfg!(windows) && !device_path.starts_with(r"\\.\") {
        println!("Note: On Windows, disk paths should use the format: \\\\.\\PhysicalDriveN");
        println!("      For example: \\\\.\\PhysicalDrive0 for the first disk");
    }
    
    // Open the device with platform-specific options
    let path = PathBuf::from(device_path);
    
    // Different open methods depending on the platform
    let mut file = if cfg!(windows) {
        use std::fs::OpenOptions;
        
        // On Windows we need to open with specific access rights
        println!("Opening device with Windows-specific options...");
        
        #[cfg(windows)]
        {
            // On Windows, use OpenOptionsExt to set access flags
            OpenOptions::new()
                .read(true)
                // FILE_SHARE_READ | FILE_SHARE_WRITE (0x1 | 0x2) allows other processes to read/write
                // while we have the device open
                .custom_flags(0x1 | 0x2)
                .open(&path)
                .with_context(|| format!("Failed to open device at path: {}", device_path))?
        }
        
        #[cfg(not(windows))]
        {
            // This branch is unreachable, but needed for compilation
            File::open(&path)
                .with_context(|| format!("Failed to open device at path: {}", device_path))?
        }
    } else {
        // On Linux/Unix, we can just open the file
        println!("Opening device with standard options...");
        File::open(&path)
            .with_context(|| format!("Failed to open device at path: {}", device_path))?
    };
    
    println!("Device opened successfully. Attempting to read partition table...");
    
    // Create a GPT configuration with the default logical block size (usually 512 bytes)
    // Use more forgiving options for Windows
    let cfg = if cfg!(windows) {
        GptConfig::new()
            .writable(false)
            .logical_block_size(LogicalBlockSize::Lb512)
    } else {
        GptConfig::new().writable(false)
    };

    //let mut data = vec![0u8; 16382464*2];
    //file.seek(Start(0))?;
    //file.read_exact(&mut data)?;

    // MyDisk implementation removed - now we directly use:
    // - aligned_disk_io() on Windows
    // - standard File on non-Windows platforms
    //
    // This simplifies the code and avoids cross-compilation issues with
    // traits like AsRawHandle that vary between platforms


    // Create disk wrapper based on platform
    #[allow(unused_mut)]
    let mut disk_result;
    
    #[cfg(windows)]
    {
        // On Windows, we need proper alignment for disk I/O operations
        debug!("Windows: Using aligned disk I/O for GPT reading");
        
        // Standard sector size for most disks
        const SECTOR_SIZE: u32 = 512;
        
        // Directly use the aligned_disk_io implementation with GPT
        let aligned_file = aligned_disk_io(file, SECTOR_SIZE)
            .with_context(|| format!("Failed to create aligned I/O wrapper for {}", device_path))?;
        
        debug!("Successfully created aligned disk I/O wrapper");
        // Use the aligned file directly with the GPT library
        disk_result = cfg.open_from_device(Box::new(aligned_file));
    }
    
    #[cfg(not(windows))]
    {
        // On non-Windows platforms, we can use the file directly
        // No need for MyDisk wrapper or alignment handling
        debug!("Non-Windows platform: Using file directly with GPT");
        disk_result = cfg.open_from_device(Box::new(file));
    }
    
    let disk = match disk_result {
        Ok(disk) => {
            println!("Successfully read GPT partition table.");
            disk
        },
        Err(e) => {
            eprintln!("Error reading GPT partition table: {}", e);
            
            // Provide specific advice for common errors
            if e.to_string().contains("invalid GPT signature") {
                eprintln!("\nThis device may not have a valid GPT partition table.");
                eprintln!("Possible reasons:");
                eprintln!("1. The device uses MBR partitioning instead of GPT");
                eprintln!("2. The device path is incorrect");
                eprintln!("3. Windows may require administrative privileges");
                
                if cfg!(windows) {
                    eprintln!("\nOn Windows, try running the command as Administrator.");
                }
            }
            
            return Err(anyhow::anyhow!("Failed to parse GPT partition table on device: {}", device_path).context(e));
        }
    };
    
    // List the partitions
    let mut partitions = Vec::new();
    for (_i, (part_num, part)) in disk.partitions().iter().enumerate() {
        // Calculate size in MB - handle potential overflows
        let sector_size_bytes: u64 = 512; // Default sector size
        let size_bytes = if part.last_lba >= part.first_lba {
            (part.last_lba - part.first_lba + 1) * sector_size_bytes
        } else {
            // Handle invalid partition table entries to prevent panics
            debug!("Warning: Invalid partition entry with last_lba < first_lba: {} < {}", 
                 part.last_lba, part.first_lba);
            0 // Use zero size for invalid entries
        };
        let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
        
        // Get partition type as string
        // Convert the part type GUID to a debug string
        let part_type_guid_str = format!("{:?}", part.part_type_guid);
        let partition_type = match part_type_guid_str.as_str() {
            "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => "EFI System",
            "0fc63daf-8483-4772-8e79-3d69d8477de4" => "Linux Filesystem",
            "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f" => "Linux Swap",
            "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" => "Windows Data",
            "48465300-0000-11aa-aa11-00306543ecac" => "APFS",
            "7c3457ef-0000-11aa-aa11-00306543ecac" => "Apple HFS+",
            "00000000-0000-0000-0000-000000000000" => "Unused",
            _ => "Unknown",
        };
        
        let info = PartitionInfo {
            number: *part_num,
            name: part.name.clone(),
            uuid: part.part_guid.to_string(),
            first_lba: part.first_lba,
            last_lba: part.last_lba,
            size_mb,
            partition_type: partition_type.to_string(),
        };
        
        partitions.push(info);
    }
    
    // If verbose mode is enabled, print detailed information
    if verbose {
        println!("Device: {}", device_path);
        println!("Disk GUID: {}", disk.guid());
        
        if let Some(header) = disk.primary_header() {
            println!("First usable LBA: {}", header.first_usable);
            println!("Last usable LBA: {}", header.last_usable);
        }
        
        println!("Sector size: {} bytes", 512); // Default sector size
        
        for (i, part) in partitions.iter().enumerate() {
            println!("\nPartition {} details:", i+1);
            println!("  Name: {}", part.name);
            println!("  UUID: {}", part.uuid);
            println!("  Type: {}", part.partition_type);
            println!("  First LBA: {}", part.first_lba);
            println!("  Last LBA: {}", part.last_lba);
            println!("  Size: {:.2} MB", part.size_mb);
        }
    }
    
    Ok(partitions)
}