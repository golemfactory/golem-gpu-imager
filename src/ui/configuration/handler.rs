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
            state.central_net_host = host.clone();
            state.is_central_net_host_valid = host.is_empty() || crate::utils::validation::is_valid_central_net_host(&host);
            debug!("Set central net host: {} (valid: {})", host, state.is_central_net_host_valid);
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
            state.is_central_net_host_valid = state.central_net_host.is_empty() || crate::utils::validation::is_valid_central_net_host(&state.central_net_host);
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
            let server_config_content = state.server_config_content.clone();

            debug!("Starting configuration save to device: {}", device_path);

            Task::perform(
                async move {
                    use crate::disk::{Disk, ImageConfiguration};

                    // Create configuration from current settings
                    let mut config = ImageConfiguration::new_with_options(
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

                    // If we have server configuration content, preserve it
                    if let Some(server_content) = server_config_content {
                        config = config.with_server_content(server_content);
                    }

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

        ConfigurationMessage::FetchFromConfigurationServer => {
            if state.configuration_server.trim().is_empty() {
                state.server_config_error = Some("Please enter a configuration server URL".to_string());
                debug!("Configuration server URL is empty");
                return Task::none();
            }

            let server_url = state.configuration_server.clone();
            state.server_config_fetching = true;
            state.server_config_error = None;
            state.server_config_content = None;
            debug!("Fetching configuration from server: {}", server_url);

            Task::perform(
                async move {
                    match fetch_configuration_from_server(&server_url).await {
                        Ok(config_content) => {
                            debug!("Successfully fetched configuration from server");
                            Ok(config_content)
                        }
                        Err(e) => {
                            debug!("Failed to fetch configuration from server: {}", e);
                            Err(e)
                        }
                    }
                },
                |result: Result<String, String>| match result {
                    Ok(content) => crate::ui::messages::Message::Configuration(
                        ConfigurationMessage::ConfigurationServerFetched(content),
                    ),
                    Err(error) => crate::ui::messages::Message::Configuration(
                        ConfigurationMessage::ConfigurationServerFetchFailed(error),
                    ),
                },
            )
        }

        ConfigurationMessage::ConfigurationServerFetched(content) => {
            state.server_config_fetching = false;
            state.server_config_content = Some(content);
            state.server_config_error = None;
            debug!("Configuration server fetch completed successfully");
            Task::none()
        }

        ConfigurationMessage::ConfigurationServerFetchFailed(error) => {
            state.server_config_fetching = false;
            state.server_config_content = None;
            state.server_config_error = Some(error);
            debug!("Configuration server fetch failed");
            Task::none()
        }

        ConfigurationMessage::ApplyServerConfiguration => {
            if let Some(content) = &state.server_config_content {
                if let Ok(config) = toml::from_str::<toml::Value>(content) {
                    apply_server_configuration_to_state(state, &config);
                    // Keep the server content for writing to disk later
                    debug!("Applied server configuration to state");
                }
            }
            Task::none()
        }

        ConfigurationMessage::CancelServerConfigurationFetch => {
            state.server_config_fetching = false;
            state.server_config_error = Some("Fetch cancelled by user".to_string());
            debug!("Server configuration fetch cancelled by user");
            Task::none()
        }

        ConfigurationMessage::DismissServerConfiguration => {
            state.server_config_content = None;
            debug!("Dismissed server configuration preview");
            Task::none()
        }
    }
}

async fn fetch_configuration_from_server(url: &str) -> Result<String, String> {
    use std::time::Duration;
    
    debug!("Starting HTTP request to: {}", url);
    
    // Create HTTP client with timeouts
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    debug!("HTTP client created, sending request...");
    
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            let error_msg = if e.is_timeout() {
                format!("Request timed out after 30 seconds: {}", e)
            } else if e.is_connect() {
                format!("Failed to connect to server: {}", e)
            } else {
                format!("Network error: {}", e)
            };
            debug!("Request failed: {}", error_msg);
            error_msg
        })?;

    debug!("Received response with status: {}", response.status());

    if !response.status().is_success() {
        let error_msg = format!(
            "Server returned error status: {} ({})",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown error")
        );
        debug!("{}", error_msg);
        return Err(error_msg);
    }

    debug!("Reading response content...");
    let content = response
        .text()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to read response content: {}", e);
            debug!("{}", error_msg);
            error_msg
        })?;

    debug!("Response content length: {} bytes", content.len());
    debug!("Validating TOML format...");
    
    // Validate that it's valid TOML
    toml::from_str::<toml::Value>(&content)
        .map_err(|e| {
            let error_msg = format!("Invalid TOML format: {}", e);
            debug!("{}", error_msg);
            error_msg
        })?;

    debug!("TOML validation successful");
    Ok(content)
}

fn apply_server_configuration_to_state(state: &mut ConfigurationState, config: &toml::Value) {
    if let Some(table) = config.as_table() {
        // Apply main configuration fields
        if let Some(accepted_terms) = table.get("accepted_terms").and_then(|v| v.as_bool()) {
            // We don't directly store accepted_terms in state, but we could validate it
            debug!("Server config accepted_terms: {}", accepted_terms);
        }

        if let Some(glm_account) = table.get("glm_account").and_then(|v| v.as_str()) {
            state.wallet_address = glm_account.to_string();
            state.is_wallet_valid = crate::utils::eth::is_valid_eth_address(glm_account);
        }

        if let Some(non_interactive) = table.get("non_interactive_install").and_then(|v| v.as_bool()) {
            state.non_interactive_install = non_interactive;
        }

        if let Some(ssh_keys) = table.get("ssh_keys").and_then(|v| v.as_array()) {
            let keys: Vec<String> = ssh_keys
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();
            
            if !keys.is_empty() {
                state.ssh_keys = keys;
                state.ssh_key_errors = vec![None; state.ssh_keys.len()];
                // Validate SSH keys
                for (index, key) in state.ssh_keys.iter().enumerate() {
                    if !key.trim().is_empty() && !crate::utils::validation::is_valid_ssh_public_key(key) {
                        state.ssh_key_errors[index] = Some("Invalid SSH public key format".to_string());
                    }
                }
            }
        }

        if let Some(config_server) = table.get("configuration_server").and_then(|v| v.as_str()) {
            state.configuration_server = config_server.to_string();
        }

        // Apply environment variables from [env] section
        if let Some(env_table) = table.get("env").and_then(|v| v.as_table()) {
            // Map environment variables to configuration fields
            if let Some(ya_net_type) = env_table.get("YA_NET_TYPE").and_then(|v| v.as_str()) {
                match ya_net_type {
                    "central" => state.network_type = crate::models::NetworkType::Central,
                    "hybrid" => state.network_type = crate::models::NetworkType::Hybrid,
                    _ => {}
                }
            }

            if let Some(subnet) = env_table.get("SUBNET").and_then(|v| v.as_str()) {
                state.subnet = subnet.to_string();
            }

            if let Some(payment_network) = env_table.get("YA_PAYMENT_NETWORK_GROUP").and_then(|v| v.as_str()) {
                match payment_network {
                    "testnet" => state.payment_network = crate::models::PaymentNetwork::Testnet,
                    "mainnet" => state.payment_network = crate::models::PaymentNetwork::Mainnet,
                    _ => {}
                }
            }

            if let Some(metrics_url) = env_table.get("YAGNA_METRICS_URL").and_then(|v| v.as_str()) {
                state.metrics_server = metrics_url.to_string();
            }

            if let Some(central_host) = env_table.get("CENTRAL_NET_HOST").and_then(|v| v.as_str()) {
                state.central_net_host = central_host.to_string();
                state.is_central_net_host_valid = central_host.is_empty() || crate::utils::validation::is_valid_central_net_host(central_host);
            }
        }

        // Revalidate configuration
        state.is_wallet_valid = state.wallet_address.is_empty()
            || crate::utils::eth::is_valid_eth_address(&state.wallet_address);
        state.is_central_net_host_valid = state.central_net_host.is_empty()
            || crate::utils::validation::is_valid_central_net_host(&state.central_net_host);
    }
}
