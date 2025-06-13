use crate::ui::flash_workflow::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub struct ConfigurationState {
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_wallet_valid: bool,
}

impl ConfigurationState {
    pub fn new() -> Self {
        Self {
            payment_network: PaymentNetwork::Testnet,
            subnet: "public".to_string(),
            network_type: NetworkType::Central,
            wallet_address: String::new(),
            is_wallet_valid: true,
        }
    }
    
    pub fn from_preset(preset: &crate::ui::preset_manager::ConfigurationPreset) -> Self {
        Self {
            payment_network: preset.payment_network,
            subnet: preset.subnet.clone(),
            network_type: preset.network_type,
            wallet_address: preset.wallet_address.clone(),
            is_wallet_valid: preset.wallet_address.is_empty() || 
                crate::utils::eth::is_valid_eth_address(&preset.wallet_address),
        }
    }
    
    pub fn to_preset(&self, name: String, is_default: bool) -> crate::ui::preset_manager::ConfigurationPreset {
        crate::ui::preset_manager::ConfigurationPreset {
            name,
            payment_network: self.payment_network,
            subnet: self.subnet.clone(),
            network_type: self.network_type,
            wallet_address: self.wallet_address.clone(),
            is_default,
        }
    }
    
    pub fn is_valid(&self) -> bool {
        !self.subnet.trim().is_empty() && self.is_wallet_valid
    }
}