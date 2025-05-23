use anyhow::{Context, Result};
use clap::Parser;
use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

/// CLI tool to repair GPT headers with incorrect partition entry LBA values
#[derive(Parser, Debug)]
#[clap(name = "golem-gpt-repair", about = "Repair GPT headers with incorrect partition entry LBA values")]
struct Args {
    /// Path to the disk device (e.g., /dev/sda, \\.\PhysicalDrive0)
    #[clap(short, long)]
    device: String,

    /// Show verbose output
    #[clap(short, long)]
    verbose: bool,

    /// Target partition entries LBA value (usually should be 2)
    #[clap(short, long, default_value = "2")]
    target_lba: u64,

    /// Only diagnose the issue without making changes
    #[clap(short = 'n', long)]
    dry_run: bool,

    /// Skip confirmation prompt (use with caution!)
    #[clap(short = 'y', long)]
    yes: bool,
}

// Constants for GPT operations
const SECTOR_SIZE: usize = 512;
const GPT_HEADER_SECTOR: u64 = 1;
const GPT_SIGNATURE: [u8; 8] = [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54]; // "EFI PART"

fn main() -> Result<()> {
    let args = Args::parse();
    
    // First, analyze the GPT header to see if a repair is needed
    let header_info = analyze_gpt_header(&args.device, args.verbose)?;
    
    // Check if the partition entries LBA is already correct
    if header_info.partition_entries_lba == args.target_lba {
        println!("✓ GPT header already has the correct partition entries LBA value ({})", args.target_lba);
        return Ok(());
    }
    
    // If this is a dry run, just report the issue and exit
    if args.dry_run {
        println!("Issues found:");
        println!("  ✗ Partition entries LBA is incorrect: {} (should be {})", 
                 header_info.partition_entries_lba, args.target_lba);
        println!("\nNo changes made (dry-run mode).");
        return Ok(());
    }
    
    // Get confirmation unless --yes was specified
    if !args.yes {
        println!("\n⚠️ WARNING: This operation will modify the GPT header on {}", args.device);
        println!("Current partition entries LBA: {}", header_info.partition_entries_lba);
        println!("Target partition entries LBA: {}", args.target_lba);
        println!("This may render your disk unbootable if done incorrectly.");
        println!("Make sure you have backups before proceeding.");
        
        println!("\nDo you want to continue? (y/N): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Perform the repair
    repair_gpt_header(&args.device, &header_info, args.target_lba, args.verbose)?;
    
    println!("\n✓ GPT header successfully repaired");
    println!("  Partition entries LBA updated from {} to {}", 
             header_info.partition_entries_lba, args.target_lba);
    
    Ok(())
}

/// Structure to hold GPT header information
#[derive(Debug, Clone)]
struct GptHeaderInfo {
    revision: u32,
    header_size: u32,
    header_crc32: u32,
    current_lba: u64,
    backup_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    disk_guid: [u8; 16],
    partition_entries_lba: u64,
    num_partition_entries: u32,
    partition_entry_size: u32,
    partition_entry_array_crc32: u32,
    header_bytes: [u8; SECTOR_SIZE],
}

/// Analyze the GPT header on a disk
fn analyze_gpt_header(device_path: &str, verbose: bool) -> Result<GptHeaderInfo> {
    println!("Analyzing GPT header on device: {}", device_path);
    
    // Open the device
    let path = PathBuf::from(device_path);
    let mut file = open_device_read_only(&path)?;
    
    // Read the GPT header (LBA 1)
    let mut header_bytes = [0u8; SECTOR_SIZE];
    file.seek(SeekFrom::Start(GPT_HEADER_SECTOR * SECTOR_SIZE as u64))?;
    file.read_exact(&mut header_bytes)?;
    
    // Check GPT signature
    if header_bytes[0..8] != GPT_SIGNATURE {
        return Err(anyhow::anyhow!("Invalid GPT signature - not a GPT disk or damaged header"));
    }
    
    // Extract header fields
    let revision = u32::from_le_bytes([
        header_bytes[8], header_bytes[9], 
        header_bytes[10], header_bytes[11]
    ]);
    
    let header_size = u32::from_le_bytes([
        header_bytes[12], header_bytes[13], 
        header_bytes[14], header_bytes[15]
    ]);
    
    let header_crc32 = u32::from_le_bytes([
        header_bytes[16], header_bytes[17], 
        header_bytes[18], header_bytes[19]
    ]);
    
    let current_lba = u64::from_le_bytes([
        header_bytes[24], header_bytes[25], 
        header_bytes[26], header_bytes[27],
        header_bytes[28], header_bytes[29], 
        header_bytes[30], header_bytes[31]
    ]);
    
    let backup_lba = u64::from_le_bytes([
        header_bytes[32], header_bytes[33], 
        header_bytes[34], header_bytes[35],
        header_bytes[36], header_bytes[37], 
        header_bytes[38], header_bytes[39]
    ]);
    
    let first_usable_lba = u64::from_le_bytes([
        header_bytes[40], header_bytes[41], 
        header_bytes[42], header_bytes[43],
        header_bytes[44], header_bytes[45], 
        header_bytes[46], header_bytes[47]
    ]);
    
    let last_usable_lba = u64::from_le_bytes([
        header_bytes[48], header_bytes[49], 
        header_bytes[50], header_bytes[51],
        header_bytes[52], header_bytes[53], 
        header_bytes[54], header_bytes[55]
    ]);
    
    // Extract disk GUID (bytes 56-71)
    let mut disk_guid = [0u8; 16];
    disk_guid.copy_from_slice(&header_bytes[56..72]);
    
    let partition_entries_lba = u64::from_le_bytes([
        header_bytes[72], header_bytes[73], 
        header_bytes[74], header_bytes[75],
        header_bytes[76], header_bytes[77], 
        header_bytes[78], header_bytes[79]
    ]);
    
    let num_partition_entries = u32::from_le_bytes([
        header_bytes[80], header_bytes[81], 
        header_bytes[82], header_bytes[83]
    ]);
    
    let partition_entry_size = u32::from_le_bytes([
        header_bytes[84], header_bytes[85], 
        header_bytes[86], header_bytes[87]
    ]);
    
    let partition_entry_array_crc32 = u32::from_le_bytes([
        header_bytes[88], header_bytes[89], 
        header_bytes[90], header_bytes[91]
    ]);
    
    let header_info = GptHeaderInfo {
        revision,
        header_size,
        header_crc32,
        current_lba,
        backup_lba,
        first_usable_lba,
        last_usable_lba,
        disk_guid,
        partition_entries_lba,
        num_partition_entries,
        partition_entry_size,
        partition_entry_array_crc32,
        header_bytes,
    };
    
    // Print header info if verbose mode is enabled
    if verbose {
        println!("  GPT Revision: {:X}.{:02X}", (revision >> 16) & 0xFFFF, revision & 0xFFFF);
        println!("  Header Size: {} bytes", header_size);
        println!("  Header CRC32: {:08X}", header_crc32);
        println!("  Current LBA: {}", current_lba);
        println!("  Backup LBA: {}", backup_lba);
        println!("  First Usable LBA: {}", first_usable_lba);
        println!("  Last Usable LBA: {}", last_usable_lba);
        println!("  Partition Entries LBA: {}", partition_entries_lba);
        println!("  Number of Partition Entries: {}", num_partition_entries);
        println!("  Size of Partition Entry: {} bytes", partition_entry_size);
        println!("  Partition Entry Array CRC32: {:08X}", partition_entry_array_crc32);
    }
    
    // Validate the current_lba field
    if current_lba != GPT_HEADER_SECTOR {
        println!("⚠️ Warning: Current LBA ({}) is not the expected value for primary GPT header ({})",
                 current_lba, GPT_HEADER_SECTOR);
    }
    
    // Check if partition entries start at LBA 2 (normal for primary GPT)
    if partition_entries_lba == 2 {
        println!("✓ Partition entries start at expected location (LBA 2)");
    } else {
        println!("⚠️ Partition entries start at non-standard location: LBA {}", partition_entries_lba);
        println!("  This may cause compatibility issues with some tools and operating systems");
    }
    
    Ok(header_info)
}

/// Repair the GPT header to use the correct partition entries LBA
fn repair_gpt_header(device_path: &str, header_info: &GptHeaderInfo, target_lba: u64, verbose: bool) -> Result<()> {
    println!("Repairing GPT header on device: {}", device_path);
    
    // Open the device for writing
    let path = PathBuf::from(device_path);
    let mut file = open_device_for_writing(&path)?;
    
    // Create a copy of the header bytes that we can modify
    let mut new_header = header_info.header_bytes;
    
    // Update the partition entries LBA field (bytes 72-79)
    let target_lba_bytes = target_lba.to_le_bytes();
    new_header[72..80].copy_from_slice(&target_lba_bytes);
    
    // Zero out the CRC32 field before calculating the new CRC32
    new_header[16] = 0;
    new_header[17] = 0;
    new_header[18] = 0;
    new_header[19] = 0;
    
    // Calculate the new CRC32 value
    let mut hasher = Hasher::new();
    hasher.update(&new_header[0..header_info.header_size as usize]);
    let new_crc32 = hasher.finalize();
    
    if verbose {
        println!("  Original CRC32: {:08X}", header_info.header_crc32);
        println!("  New CRC32: {:08X}", new_crc32);
    }
    
    // Update the CRC32 field in the header
    let new_crc32_bytes = new_crc32.to_le_bytes();
    new_header[16..20].copy_from_slice(&new_crc32_bytes);
    
    // Write the modified header back to the disk
    file.seek(SeekFrom::Start(GPT_HEADER_SECTOR * SECTOR_SIZE as u64))?;
    file.write_all(&new_header)?;
    file.flush()?;
    
    println!("  Updated partition entries LBA from {} to {}", header_info.partition_entries_lba, target_lba);
    println!("  Updated header CRC32 from {:08X} to {:08X}", header_info.header_crc32, new_crc32);
    
    // TODO: Consider also updating the backup GPT header at backup_lba
    
    Ok(())
}

/// Open a device for read-only access with platform-specific options
fn open_device_read_only(path: &PathBuf) -> Result<File> {
    if cfg!(windows) {
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            
            // On Windows, use OpenOptionsExt to set access flags
            OpenOptions::new()
                .read(true)
                // FILE_SHARE_READ | FILE_SHARE_WRITE (0x1 | 0x2) allows other processes to read/write
                .custom_flags(0x1 | 0x2)
                .open(path)
                .with_context(|| format!("Failed to open device at path: {}", path.display()))
        }
        
        #[cfg(not(windows))]
        {
            // This branch is unreachable on non-Windows platforms, but needed for compilation
            File::open(path)
                .with_context(|| format!("Failed to open device at path: {}", path.display()))
        }
    } else {
        File::open(path)
            .with_context(|| format!("Failed to open device at path: {}", path.display()))
    }
}

/// Open a device for writing with platform-specific options
fn open_device_for_writing(path: &PathBuf) -> Result<File> {
    if cfg!(windows) {
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            
            // On Windows, use OpenOptionsExt to set access flags
            OpenOptions::new()
                .read(true)
                .write(true)
                // FILE_SHARE_READ | FILE_SHARE_WRITE (0x1 | 0x2) allows other processes to read/write
                .custom_flags(0x1 | 0x2)
                .open(path)
                .with_context(|| format!("Failed to open device for writing at path: {}", path.display()))
        }
        
        #[cfg(not(windows))]
        {
            // This branch is unreachable on non-Windows platforms, but needed for compilation
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .with_context(|| format!("Failed to open device for writing at path: {}", path.display()))
        }
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("Failed to open device for writing at path: {}", path.display()))
    }
}