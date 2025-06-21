use crate::models::{ConfigurationPreset, NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub struct ConfigurationState {
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_wallet_valid: bool,
    pub non_interactive_install: bool,
    pub ssh_keys: String, // UI representation (newline separated)
    pub configuration_server: String,
    pub metrics_server: String,
    pub central_net_host: String,
    pub advanced_options_expanded: bool,
    pub selected_preset: Option<usize>,
}

impl ConfigurationState {
    pub fn new() -> Self {
        Self {
            payment_network: PaymentNetwork::Testnet,
            subnet: "public".to_string(),
            network_type: NetworkType::Central,
            wallet_address: String::new(),
            is_wallet_valid: true,
            non_interactive_install: false,
            ssh_keys: String::new(),
            configuration_server: String::new(),
            metrics_server: String::new(),
            central_net_host: String::new(),
            advanced_options_expanded: false,
            selected_preset: None,
        }
    }

    pub fn from_preset(preset: &ConfigurationPreset) -> Self {
        Self {
            payment_network: preset.payment_network,
            subnet: preset.subnet.clone(),
            network_type: preset.network_type,
            wallet_address: preset.wallet_address.clone(),
            is_wallet_valid: preset.wallet_address.is_empty()
                || crate::utils::eth::is_valid_eth_address(&preset.wallet_address),
            non_interactive_install: preset.non_interactive_install,
            ssh_keys: preset.ssh_keys.join("\n"), // Convert Vec<String> to String
            configuration_server: preset.configuration_server.clone().unwrap_or_default(),
            metrics_server: preset.metrics_server.clone().unwrap_or_default(),
            central_net_host: preset.central_net_host.clone().unwrap_or_default(),
            advanced_options_expanded: false,
            selected_preset: None, // Will be set by the caller when loading from a specific preset
        }
    }

    pub fn to_preset(&self, name: String, is_default: bool) -> ConfigurationPreset {
        // Parse SSH keys from string (newline or comma separated)
        let ssh_keys_vec: Vec<String> = if self.ssh_keys.trim().is_empty() {
            Vec::new()
        } else if self.ssh_keys.contains('\n') {
            self.ssh_keys.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        } else {
            self.ssh_keys.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        };

        ConfigurationPreset {
            name,
            payment_network: self.payment_network,
            subnet: self.subnet.clone(),
            network_type: self.network_type,
            wallet_address: self.wallet_address.clone(),
            is_default,
            non_interactive_install: self.non_interactive_install,
            ssh_keys: ssh_keys_vec,
            configuration_server: if self.configuration_server.trim().is_empty() { None } else { Some(self.configuration_server.clone()) },
            metrics_server: if self.metrics_server.trim().is_empty() { None } else { Some(self.metrics_server.clone()) },
            central_net_host: if self.central_net_host.trim().is_empty() { None } else { Some(self.central_net_host.clone()) },
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.subnet.trim().is_empty() && self.is_wallet_valid
    }
}
