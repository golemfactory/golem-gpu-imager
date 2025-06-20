use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum EditMessage {
    SelectExistingDevice(usize),
    GotoEditConfiguration,
    DeviceConfigurationLoaded(crate::disk::GolemConfig),
    DeviceConfigurationLoadFailed(String),
    SaveConfiguration,
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    SelectPreset(usize),
    RefreshDevices,
    DevicesLoaded(Vec<crate::ui::device_selection::StorageDevice>),
    DeviceLoadFailed(String),
    DeviceLocked(Option<crate::disk::Disk>),
    ConfigurationSaved,
    ConfigurationSaveFailed,
    BackToMainMenu,
    BackToDeviceSelection,
    EditAnother,
}
