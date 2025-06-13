// We'll use a single StorageDevice type for all modules

// Shared storage device representation
#[derive(Debug, Clone)]
pub struct StorageDevice {
    pub name: String,
    pub path: String,
    pub size: String,
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