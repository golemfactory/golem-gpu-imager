use crate::models::{ConfigurationPreset, NetworkType, PaymentNetwork};
use crate::ui::configuration::ConfigurationState;

#[derive(Debug, Clone)]
pub struct PresetEditor {
    pub editing_index: Option<usize>, // None for new preset, Some(index) for editing existing
    pub name: String,
    pub configuration: ConfigurationState,
    pub is_default: bool,
}

impl PresetEditor {
    pub fn new(preset_index: usize, preset: &ConfigurationPreset) -> Self {
        Self {
            editing_index: Some(preset_index),
            name: preset.name.clone(),
            configuration: ConfigurationState::from_preset(preset),
            is_default: preset.is_default,
        }
    }

    pub fn new_preset() -> Self {
        Self {
            editing_index: None,
            name: String::new(),
            configuration: ConfigurationState::new(),
            is_default: false,
        }
    }

    pub fn to_preset(&self) -> ConfigurationPreset {
        self.configuration.to_preset(self.name.clone(), self.is_default)
    }

    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && self.configuration.is_valid()
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
                non_interactive_install: false,
                ssh_keys: Vec::new(),
                configuration_server: None,
                metrics_server: None,
                central_net_host: None,
            },
            ConfigurationPreset {
                name: "Mainnet Production".to_string(),
                payment_network: PaymentNetwork::Mainnet,
                subnet: "public".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "".to_string(),
                is_default: false,
                non_interactive_install: false,
                ssh_keys: Vec::new(),
                configuration_server: None,
                metrics_server: None,
                central_net_host: None,
            },
        ];
        state
    }
}
