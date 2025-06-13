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
    DeviceLocked(Option<crate::disk::Disk>),
    ConfigurationSaved,
    ConfigurationSaveFailed,
}