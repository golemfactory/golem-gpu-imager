use crate::ui::flash_workflow::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum ConfigurationMessage {
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    LoadFromPreset(usize),
    ValidateConfiguration,
}