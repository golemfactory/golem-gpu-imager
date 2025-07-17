use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum ConfigurationMessage {
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    SetNonInteractiveInstall(bool),
    AddSSHKey,
    RemoveSSHKey(usize),
    UpdateSSHKey(usize, String),
    SetConfigurationServer(String),
    SetMetricsServer(String),
    SetCentralNetHost(String),
    ToggleAdvancedOptions,
    SelectPreset(usize),
    LoadFromPreset(usize),
    LoadFromDevice(crate::disk::GolemConfig),
    SaveToDevice(String),
    Reset,
    ValidateConfiguration,
    FetchFromConfigurationServer,
    ConfigurationServerFetched(String),
    ConfigurationServerFetchFailed(String),
    CancelServerConfigurationFetch,
    ApplyServerConfiguration,
    DismissServerConfiguration,
}
