use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::Path;
use udisks2::{zbus, Client};
use udisks2::zbus::zvariant::{ObjectPath, OwnedObjectPath};
use anyhow::{Result, Error, anyhow, Context};
use libc::{O_CLOEXEC, O_EXCL, O_SYNC};
use udisks2::filesystem::FilesystemProxy;
use uuid::Uuid;
use gpt::GptConfig;
use gpt::disk::LogicalBlockSize;
use tracing::{debug, error, info, trace, warn};

/// Helper function to extract string values from TOML lines
/// For example, from a line like: glm_account = "0x1234..."
/// it extracts the value: "0x1234..."
fn extract_toml_string_value(line: &str) -> Option<String> {
    if let Some(equals_pos) = line.find('=') {
        let value_part = line[equals_pos + 1..].trim();

        // Look for quoted strings
        if value_part.starts_with('"') && value_part.ends_with('"') && value_part.len() >= 2 {
            // Extract the content between quotes
            return Some(value_part[1..value_part.len()-1].to_string());
        }

        // If no quotes, just return the value as is
        return Some(value_part.to_string());
    }
    None
}

async fn resovle_device(client: &Client, path: &str) -> Result<OwnedObjectPath> {
    let mut spec = HashMap::new();
    spec.insert("path", path.into());
    let mut obj = client
        .manager()
        .resolve_device(spec, HashMap::default())
        .await?;

    Ok(obj.pop().ok_or(anyhow!("no device found"))?)
}

async fn umount_all(client: &Client, path: ObjectPath<'_>) -> Result<()> {
    for dev_path in client.manager().get_block_devices(HashMap::default()).await? {
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

pub struct Disk {
    file: File
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
pub struct PartitionFileProxy {
    /// The underlying file handle for the entire disk
    file: File,
    /// The offset in bytes where the partition starts
    partition_offset: u64,
    /// The current position relative to the start of the partition
    current_position: u64,
}

impl PartitionFileProxy {
    /// Create a new partition file proxy
    ///
    /// # Arguments
    /// * `file` - File handle to the entire disk
    /// * `partition_offset` - Byte offset where the partition starts
    pub fn new(file: File, partition_offset: u64) -> Self {
        PartitionFileProxy {
            file,
            partition_offset,
            current_position: 0,
        }
    }

    /// Get the partition offset in bytes
    pub fn partition_offset(&self) -> u64 {
        self.partition_offset
    }

    /// Get the current position relative to the start of the partition
    pub fn position(&self) -> u64 {
        self.current_position
    }

    /// Convert a partition-relative position to an absolute disk position
    fn to_absolute_position(&self, position: u64) -> u64 {
        self.partition_offset + position
    }
}

// Implement Read trait for PartitionFileProxy
impl Read for PartitionFileProxy {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Ensure we're at the correct position before reading
        self.file.seek(SeekFrom::Start(self.to_absolute_position(self.current_position)))?;

        // Perform the read operation
        let bytes_read = self.file.read(buf)?;

        // Update our current position
        self.current_position += bytes_read as u64;

        Ok(bytes_read)
    }
}

// Implement Write trait for PartitionFileProxy
impl Write for PartitionFileProxy {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Ensure we're at the correct position before writing
        self.file.seek(SeekFrom::Start(self.to_absolute_position(self.current_position)))?;

        // Perform the write operation
        let bytes_written = self.file.write(buf)?;

        // Update our current position
        self.current_position += bytes_written as u64;

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
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
                    self.current_position.checked_sub(offset.abs() as u64)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek to a negative position"))?
                } else {
                    self.current_position.checked_add(offset as u64)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek - position overflow"))?
                }
            }

            // For SeekEnd, we would need to know the size of the partition
            // For simplicity, we don't support this in this implementation
            SeekFrom::End(_) => {
                return Err(io::Error::new(io::ErrorKind::Unsupported, "SeekFrom::End is not supported for partition files"));
            }
        };

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

    pub async fn lock_path(path : &str) -> Result<Self> {
        let client = Client::new().await?;
        let drive_path = resovle_device(&client, path).await?;
        umount_all(&client, drive_path.as_ref()).await.context("failed to unmount")?;
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
        }
        else {
            Err(anyhow!("failed to open device"))
        }
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
        wallet_address: &str
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
            network_type_str,
            subnet,
            payment_network_str
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
    pub fn find_partition(&mut self, uuid_str: &str) -> Result<fatfs::FileSystem<PartitionFileProxy>> {
        // Parse the provided UUID string
        let target_uuid = Uuid::parse_str(uuid_str)
            .context(format!("Failed to parse UUID string: {}", uuid_str))?;

        // Create a GPT configuration with the default logical block size (usually 512 bytes)
        let cfg = GptConfig::new().writable(false);

        // Clone the file handle and seek to the beginning
        // Note: We need to ensure we have a separate file handle for each operation
        // to avoid seek position conflicts
        let file_for_gpt = self.get_cloned_file_handle()?;

        // Parse GPT header and partition table from the disk
        let disk = cfg.open_from_device(Box::new(file_for_gpt))
            .context("Failed to parse GPT partition table")?;

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

                // Create a PartitionFileProxy that will handle seeks relative to the partition start
                let proxy = PartitionFileProxy::new(partition_file, start_offset);

                // Create a FAT filesystem from the partition using our proxy
                let fs = fatfs::FileSystem::new(proxy, fatfs::FsOptions::new())
                    .with_context(|| format!("Failed to open FAT filesystem on partition with UUID {}", uuid_str))?;

                return Ok(fs);
            }
        }

        // No partition with matching UUID found
        Err(anyhow!("No partition found with UUID: {}", uuid_str))
    }

    /// Helper method to create a cloned file handle to the disk
    /// This is needed because we can't directly clone the File (it doesn't implement Clone)
    /// But we need separate file handles for different operations to avoid seek conflicts
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
        let disk = cfg.open_from_device(Box::new(file_for_gpt))
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

                // Create a new file handle
                let partition_file = self.get_cloned_file_handle()?;

                // Create and return the proxy
                let proxy = PartitionFileProxy::new(partition_file, start_offset);
                return Ok((proxy, part.name.clone()));
            }
        }

        Err(anyhow!("No partition found with UUID: {}", uuid_str))
    }

}


#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_find() -> Result<()> {
    let disk = Disk::lock_path("/dev/sda").await?;

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
    info!("Files in the root directory of partition with UUID {}:", target_uuid);
    for entry in root_dir.iter() {
        let entry = entry?;
        let name = entry.file_name();

        if entry.is_dir() {
            debug!("  Directory: {}", name);
        } else {
            let size = entry.len();
            debug!("  File: {} (size: {} bytes)", name, size);

            // If it's a text file, read and print its contents (for small files only)
            if name.ends_with(".txt") && size < 10240 { // Less than 10KB
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

    info!("Found partition '{}' with UUID: {}", partition_name, target_uuid);
    info!("Partition starts at offset: {} bytes", proxy.partition_offset());

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
        if let volume_label = fs.volume_label() {
            info!("Volume label: {}", volume_label);
        }

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
        warn!("Invalid boot sector signature: found {:02X}{:02X} - expected 55AA",
                buffer[510], buffer[511]);
    }

    Ok(())
}


#[test]
#[ignore]
fn test_sda3() -> Result<()> {
    let block = OpenOptions::new().read(true).open("/dev/sda3")?;
    let fs = fatfs::FileSystem::new(block, fatfs::FsOptions::new().update_accessed_date(false))?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_read_write_configuration() -> Result<()> {
    // Create a disk instance
    let mut disk = Disk::lock_path("/dev/sda").await?;

    // The target UUID of the config partition
    let config_partition_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

    // First write some test configuration
    let payment_network = crate::models::PaymentNetwork::Mainnet;
    let network_type = crate::models::NetworkType::Hybrid;
    let subnet = "testing-subnet";
    let wallet_address = "0xABCDEF1234567890ABCDEF1234567890ABCDEF12";

    // Write the configuration
    disk.write_configuration(
        config_partition_uuid,
        payment_network,
        network_type,
        subnet,
        wallet_address,
    )?;

    info!("Wrote test configuration to disk");

    // Now read it back using the read_configuration function
    let config = disk.read_configuration(config_partition_uuid)?;

    debug!("Read configuration: {:?}", config);

    // Verify the values were read correctly
    assert_eq!(config.payment_network, payment_network,
        "Payment network doesn't match the written value");
    assert_eq!(config.network_type, network_type,
        "Network type doesn't match the written value");
    assert_eq!(config.subnet, subnet,
        "Subnet doesn't match the written value");
    assert_eq!(config.wallet_address, wallet_address,
        "Wallet address doesn't match the written value");

    // Test with missing files
    {
        // Delete the config files to test defaults
        let fs = disk.find_partition(config_partition_uuid)?;
        let root_dir = fs.root_dir();

        // Try to delete both files
        let _ = root_dir.remove_file("golemwz.toml");
        let _ = root_dir.remove_file("golem.env");

        // Read the configuration again (should use defaults)
        info!("Testing with no configuration files (should use defaults)");
        let default_config = disk.read_configuration(config_partition_uuid)?;
        debug!("Default configuration: {:?}", default_config);

        // Verify default values
        assert_eq!(default_config.payment_network, crate::models::PaymentNetwork::Testnet,
            "Default payment network should be Testnet");
        assert_eq!(default_config.network_type, crate::models::NetworkType::Central,
            "Default network type should be Central");
        assert_eq!(default_config.subnet, "public",
            "Default subnet should be 'public'");
        assert_eq!(default_config.wallet_address, "",
            "Default wallet address should be empty");
    }

    info!("Read configuration test completed successfully!");
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
        assert!(toml_content.contains(&format!("glm_account = \"{}\"", wallet_address)),
            "Wallet address not found in golemwz.toml");
    } else {
        return Err(anyhow!("golemwz.toml file not found"));
    }

    // Check if golem.env exists and has the correct content
    if let Ok(mut env_file) = root_dir.open_file("golem.env") {
        let mut env_content = String::new();
        env_file.read_to_string(&mut env_content)?;
        debug!("golem.env content:\n{}", env_content);

        // Verify key settings were written correctly
        assert!(env_content.contains("YA_NET_TYPE=central"),
            "Network type not found or incorrect in golem.env");
        assert!(env_content.contains(&format!("SUBNET={}", subnet)),
            "Subnet setting not found or incorrect in golem.env");
        assert!(env_content.contains("YA_PAYMENT_NETWORK_GROUP=testnet"),
            "Payment network not found or incorrect in golem.env");
    } else {
        return Err(anyhow!("golem.env file not found"));
    }

    info!("Verification complete - configuration written correctly!");
    Ok(())
}