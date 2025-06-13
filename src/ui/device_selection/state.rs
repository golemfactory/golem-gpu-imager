use crate::ui::flash_workflow::StorageDevice as FlashStorageDevice;
use crate::ui::edit_workflow::StorageDevice as EditStorageDevice;

// Shared storage device representation
#[derive(Debug, Clone)]
pub struct StorageDevice {
    pub name: String,
    pub path: String,
    pub size: String,
}

// Convert between different storage device types
impl From<StorageDevice> for FlashStorageDevice {
    fn from(device: StorageDevice) -> Self {
        FlashStorageDevice {
            name: device.name,
            path: device.path,
            size: device.size,
        }
    }
}

impl From<StorageDevice> for EditStorageDevice {
    fn from(device: StorageDevice) -> Self {
        EditStorageDevice {
            name: device.name,
            path: device.path,
            size: device.size,
        }
    }
}

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