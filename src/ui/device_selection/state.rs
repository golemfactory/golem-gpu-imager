// We'll use a single StorageDevice type for all modules

// Shared storage device representation
#[derive(Debug, Clone)]
pub struct StorageDevice {
    pub name: String,
    pub path: String,
    pub size: String,
    // rs-drivelist device type flags
    pub is_card: bool,
    pub is_usb: bool,
    pub is_scsi: bool,
    pub is_removable: bool,
}

// Device type for better UI representation
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    Usb,
    SdCard,
    Emmc,
    HardDrive,
    Unknown,
}

impl StorageDevice {
    /// Determine device type based on rs-drivelist flags and fallback patterns
    pub fn device_type(&self) -> DeviceType {
        // Use rs-drivelist boolean flags first (most reliable)
        if self.is_card {
            return DeviceType::SdCard;
        }
        
        if self.is_usb {
            return DeviceType::Usb;
        }
        
        // Fallback to path and name pattern matching
        let path_lower = self.path.to_lowercase();
        let name_lower = self.name.to_lowercase();
        
        // Check for eMMC (not covered by rs-drivelist flags)
        if path_lower.contains("emmc") || name_lower.contains("emmc") {
            return DeviceType::Emmc;
        }
        
        // Check for hard drives - but be more specific to avoid false positives
        if path_lower.contains("nvme") || 
           (path_lower.contains("sda") && !self.is_removable) ||
           (path_lower.contains("sdb") && !self.is_removable) {
            return DeviceType::HardDrive;
        }
        
        // Fallback USB detection for devices without proper flags
        if path_lower.contains("usb") || name_lower.contains("usb") {
            return DeviceType::Usb;
        }
        
        // Only treat as SD card if we have strong indicators (not just "sd" in path)
        if name_lower.contains("sd card") || name_lower.contains("mmc") ||
           path_lower.contains("mmcblk") {
            return DeviceType::SdCard;
        }
        
        DeviceType::Unknown
    }
    
    /// Get icon for device type
    pub fn type_icon(&self) -> iced::widget::Text<'static> {
        use crate::ui::icons;
        
        match self.device_type() {
            DeviceType::Usb => icons::usb(),
            DeviceType::SdCard => icons::sd_card(),
            DeviceType::Emmc => icons::memory(),
            DeviceType::HardDrive => icons::hard_drive(),
            DeviceType::Unknown => icons::storage(),
        }
    }
    
    /// Get user-friendly type name
    pub fn type_name(&self) -> &'static str {
        match self.device_type() {
            DeviceType::Usb => "USB Drive",
            DeviceType::SdCard => "SD Card",
            DeviceType::Emmc => "eMMC",
            DeviceType::HardDrive => "Hard Drive",
            DeviceType::Unknown => "Storage Device",
        }
    }
}

// StorageDevice is now shared across all modules

#[derive(Debug, Clone)]
pub struct DeviceSelectionState {
    pub devices: Vec<StorageDevice>,
    pub selected_device: Option<usize>,
    pub is_refreshing: bool,
    pub error_message: Option<String>,
}

impl DeviceSelectionState {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected_device: None,
            is_refreshing: false,
            error_message: None,
        }
    }
}