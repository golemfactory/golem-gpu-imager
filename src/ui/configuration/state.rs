use crate::models::{ConfigurationPreset, NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub struct ConfigurationState {
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_wallet_valid: bool,
    pub non_interactive_install: bool,
    pub ssh_keys: Vec<String>,
    pub ssh_key_errors: Vec<Option<String>>,
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
            ssh_keys: vec![String::new()],
            ssh_key_errors: vec![None],
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
            ssh_keys: if preset.ssh_keys.is_empty() {
                vec![String::new()]
            } else {
                preset.ssh_keys.clone()
            },
            ssh_key_errors: vec![
                None;
                if preset.ssh_keys.is_empty() {
                    1
                } else {
                    preset.ssh_keys.len()
                }
            ],
            configuration_server: preset.configuration_server.clone().unwrap_or_default(),
            metrics_server: preset.metrics_server.clone().unwrap_or_default(),
            central_net_host: preset.central_net_host.clone().unwrap_or_default(),
            advanced_options_expanded: false,
            selected_preset: None, // Will be set by the caller when loading from a specific preset
        }
    }

    pub fn to_preset(&self, name: String, is_default: bool) -> ConfigurationPreset {
        let ssh_keys_vec: Vec<String> = self
            .ssh_keys
            .iter()
            .filter(|key| !key.trim().is_empty())
            .map(|key| key.clone())
            .collect();

        ConfigurationPreset {
            name,
            payment_network: self.payment_network,
            subnet: self.subnet.clone(),
            network_type: self.network_type,
            wallet_address: self.wallet_address.clone(),
            is_default,
            non_interactive_install: self.non_interactive_install,
            ssh_keys: ssh_keys_vec,
            configuration_server: if self.configuration_server.trim().is_empty() {
                None
            } else {
                Some(self.configuration_server.clone())
            },
            metrics_server: if self.metrics_server.trim().is_empty() {
                None
            } else {
                Some(self.metrics_server.clone())
            },
            central_net_host: if self.central_net_host.trim().is_empty() {
                None
            } else {
                Some(self.central_net_host.clone())
            },
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.subnet.trim().is_empty() && self.is_wallet_valid && self.are_ssh_keys_valid()
    }

    pub fn are_ssh_keys_valid(&self) -> bool {
        self.ssh_key_errors.iter().all(|error| error.is_none())
    }

    pub fn add_ssh_key(&mut self) {
        self.ssh_keys.push(String::new());
        self.ssh_key_errors.push(None);
    }

    pub fn remove_ssh_key(&mut self, index: usize) {
        if index < self.ssh_keys.len() && self.ssh_keys.len() > 1 {
            self.ssh_keys.remove(index);
            self.ssh_key_errors.remove(index);
        }
    }

    pub fn update_ssh_key(&mut self, index: usize, key: String) {
        if index < self.ssh_keys.len() {
            self.ssh_keys[index] = key.clone();

            // Validate the SSH key
            if key.trim().is_empty() {
                self.ssh_key_errors[index] = None;
            } else if crate::utils::validation::is_valid_ssh_public_key(&key) {
                self.ssh_key_errors[index] = None;
            } else {
                self.ssh_key_errors[index] = Some("Invalid SSH public key format".to_string());
            }
        }
    }
}
