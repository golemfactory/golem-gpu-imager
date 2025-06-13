use crate::ui::flash_workflow::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigurationPreset {
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
}

// Implement Display trait so pick_list can properly show the preset
impl std::fmt::Display for ConfigurationPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_default {
            write!(f, "{} (Default)", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresetEditor {
    pub preset_index: usize,
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
}

impl PresetEditor {
    pub fn new(preset_index: usize, preset: &ConfigurationPreset) -> Self {
        Self {
            preset_index,
            name: preset.name.clone(),
            payment_network: preset.payment_network,
            subnet: preset.subnet.clone(),
            network_type: preset.network_type,
            wallet_address: preset.wallet_address.clone(),
            is_default: preset.is_default,
        }
    }

    pub fn to_preset(&self) -> ConfigurationPreset {
        ConfigurationPreset {
            name: self.name.clone(),
            payment_network: self.payment_network,
            subnet: self.subnet.clone(),
            network_type: self.network_type,
            wallet_address: self.wallet_address.clone(),
            is_default: self.is_default,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && !self.subnet.trim().is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct PresetManagerState {
    pub presets: Vec<ConfigurationPreset>,
    pub selected_preset: Option<usize>,
    pub new_preset_name: String,
    pub show_manager: bool,
    pub editor: Option<PresetEditor>,
}

impl PresetManagerState {
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
            selected_preset: None,
            new_preset_name: String::new(),
            show_manager: false,
            editor: None,
        }
    }
    
    pub fn with_defaults() -> Self {
        let mut state = Self::new();
        state.presets = vec![
            ConfigurationPreset {
                name: "Testnet Development".to_string(),
                payment_network: PaymentNetwork::Testnet,
                subnet: "public".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "".to_string(),
                is_default: true,
            },
            ConfigurationPreset {
                name: "Mainnet Production".to_string(),
                payment_network: PaymentNetwork::Mainnet,
                subnet: "public".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "".to_string(),
                is_default: false,
            },
        ];
        state
    }
}