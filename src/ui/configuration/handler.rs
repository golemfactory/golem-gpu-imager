use super::{ConfigurationMessage, ConfigurationState};
use iced::Task;
use tracing::debug;

pub fn handle_message(
    state: &mut ConfigurationState,
    presets: &[crate::models::ConfigurationPreset],
    message: ConfigurationMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        ConfigurationMessage::SetPaymentNetwork(network) => {
            state.payment_network = network;
            debug!("Set payment network: {:?}", network);
            Task::none()
        }

        ConfigurationMessage::SetSubnet(subnet) => {
            state.subnet = subnet;
            debug!("Set subnet: {}", state.subnet);
            Task::none()
        }

        ConfigurationMessage::SetNetworkType(network_type) => {
            state.network_type = network_type;
            debug!("Set network type: {:?}", network_type);
            Task::none()
        }

        ConfigurationMessage::SetWalletAddress(address) => {
            state.wallet_address = address.clone();
            state.is_wallet_valid =
                address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            debug!(
                "Set wallet address: {} (valid: {})",
                address, state.is_wallet_valid
            );
            Task::none()
        }

        ConfigurationMessage::SetNonInteractiveInstall(enabled) => {
            state.non_interactive_install = enabled;
            debug!("Set non-interactive install: {}", enabled);
            Task::none()
        }

        ConfigurationMessage::AddSSHKey => {
            state.add_ssh_key();
            debug!("Added new SSH key field");
            Task::none()
        }

        ConfigurationMessage::RemoveSSHKey(index) => {
            state.remove_ssh_key(index);
            debug!("Removed SSH key at index: {}", index);
            Task::none()
        }

        ConfigurationMessage::UpdateSSHKey(index, key) => {
            state.update_ssh_key(index, key);
            debug!("Updated SSH key at index: {}", index);
            Task::none()
        }

        ConfigurationMessage::SetConfigurationServer(server) => {
            state.configuration_server = server;
            debug!("Set configuration server: {}", state.configuration_server);
            Task::none()
        }

        ConfigurationMessage::SetMetricsServer(server) => {
            state.metrics_server = server;
            debug!("Set metrics server: {}", state.metrics_server);
            Task::none()
        }

        ConfigurationMessage::SetCentralNetHost(host) => {
            state.central_net_host = host;
            debug!("Set central net host: {}", state.central_net_host);
            Task::none()
        }

        ConfigurationMessage::ToggleAdvancedOptions => {
            state.advanced_options_expanded = !state.advanced_options_expanded;
            debug!(
                "Toggled advanced options: {}",
                state.advanced_options_expanded
            );
            Task::none()
        }

        ConfigurationMessage::SelectPreset(index) => {
            if let Some(preset) = presets.get(index) {
                *state = ConfigurationState::from_preset(preset);
                state.selected_preset = Some(index);
                debug!("Selected and loaded preset {}: {}", index, preset.name);
            }
            Task::none()
        }

        ConfigurationMessage::LoadFromPreset(index) => {
            if let Some(preset) = presets.get(index) {
                *state = ConfigurationState::from_preset(preset);
                debug!("Loaded configuration from preset: {}", preset.name);
            }
            Task::none()
        }

        ConfigurationMessage::LoadFromDevice(config) => {
            // Load configuration from device into the state
            state.payment_network = config.payment_network;
            state.subnet = config.subnet;
            state.network_type = config.network_type;
            state.wallet_address = config.wallet_address.clone();
            state.is_wallet_valid = config.wallet_address.is_empty()
                || crate::utils::eth::is_valid_eth_address(&config.wallet_address);
            state.non_interactive_install = config.non_interactive_install;
            state.ssh_keys = if config.ssh_keys.is_empty() {
                vec![String::new()]
            } else {
                config.ssh_keys
            };
            state.ssh_key_errors = vec![None; state.ssh_keys.len()];
            state.configuration_server = config.configuration_server.unwrap_or_default();
            state.metrics_server = config.metrics_server.unwrap_or_default();
            state.central_net_host = config.central_net_host.unwrap_or_default();
            debug!("Loaded configuration from device");
            Task::none()
        }

        ConfigurationMessage::SaveToDevice(device_path) => {
            // Save current configuration to device
            let payment_network = state.payment_network;
            let subnet = state.subnet.clone();
            let network_type = state.network_type;
            let wallet_address = state.wallet_address.clone();
            let non_interactive_install = state.non_interactive_install;
            let ssh_keys = state
                .ssh_keys
                .iter()
                .filter(|key| !key.trim().is_empty())
                .cloned()
                .collect::<Vec<String>>()
                .join("\n");
            let configuration_server = state.configuration_server.clone();
            let metrics_server = state.metrics_server.clone();
            let central_net_host = state.central_net_host.clone();

            debug!("Starting configuration save to device: {}", device_path);

            Task::perform(
                async move {
                    use crate::disk::{Disk, ImageConfiguration};

                    // Create configuration from current settings
                    let config = ImageConfiguration::new_with_options(
                        payment_network,
                        network_type,
                        subnet,
                        wallet_address,
                        non_interactive_install,
                        ssh_keys,
                        configuration_server,
                        metrics_server,
                        central_net_host,
                    );

                    // Write configuration to device
                    match Disk::write_configuration_to_disk(&device_path, config).await {
                        Ok(()) => {
                            debug!(
                                "Configuration successfully saved to device: {}",
                                device_path
                            );
                            Ok(())
                        }
                        Err(e) => {
                            debug!(
                                "Failed to save configuration to device {}: {}",
                                device_path, e
                            );
                            Err(format!("Failed to save configuration: {}", e))
                        }
                    }
                },
                |result: Result<(), String>| match result {
                    Ok(()) => crate::ui::messages::Message::Edit(
                        crate::ui::edit_workflow::EditMessage::ConfigurationSaved,
                    ),
                    Err(_) => crate::ui::messages::Message::Edit(
                        crate::ui::edit_workflow::EditMessage::ConfigurationSaveFailed,
                    ),
                },
            )
        }

        ConfigurationMessage::Reset => {
            // Reset to default configuration
            *state = ConfigurationState::new();
            debug!("Reset configuration to defaults");
            Task::none()
        }

        ConfigurationMessage::ValidateConfiguration => {
            // Revalidate wallet address
            state.is_wallet_valid = state.wallet_address.is_empty()
                || crate::utils::eth::is_valid_eth_address(&state.wallet_address);
            debug!("Configuration validation: valid={}", state.is_valid());
            Task::none()
        }
    }
}
