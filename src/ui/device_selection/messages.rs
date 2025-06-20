#[derive(Debug, Clone)]
pub enum DeviceMessage {
    RefreshDevices,
    DevicesLoaded(Vec<super::StorageDevice>),
    DeviceLoadFailed(String),
    SelectDevice(usize),
    ClearSelection,
}
