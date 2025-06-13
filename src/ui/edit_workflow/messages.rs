use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum EditMessage {
    SelectExistingDevice(usize),
    GotoEditConfiguration,
    SaveConfiguration,
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    RefreshDevices,
    DevicesLoaded(Vec<crate::ui::device_selection::StorageDevice>),
    DeviceLoadFailed(String),
    DeviceLocked(Option<crate::disk::Disk>),
    ConfigurationSaved,
    ConfigurationSaveFailed,
    BackToMainMenu,
    EditAnother,
}