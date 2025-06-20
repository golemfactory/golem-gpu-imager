use super::{EditMessage, EditState, EditWorkflowState};
use iced::Task;
use tracing::{debug, error, info, warn};

pub fn handle_message(
    state: &mut EditState,
    device_selection: &crate::ui::device_selection::DeviceSelectionState,
    message: EditMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        EditMessage::SelectExistingDevice(index) => {
            // Note: Device bounds checking is now handled by the UI layer using shared device state
            state.selected_device = Some(index);
            debug!("Selected device for editing: {}", index);
            Task::none()
        }

        EditMessage::GotoEditConfiguration => {
            if let Some(device_index) = state.selected_device {
                if let Some(device) = device_selection.devices.get(device_index) {
                    // Set loading state
                    state.workflow_state = EditWorkflowState::LoadingConfiguration;
                    
                    // Start async task to read configuration from device
                    let device_path = device.path.clone();
                    debug!("Reading configuration from device: {} ({})", device.name, device.path);
                    
                    Task::perform(
                        async move {
                            // Lock the device for reading
                            match crate::disk::Disk::lock_path(&device_path, true).await {
                                Ok(mut disk) => {
                                    // Read configuration from device
                                    match disk.read_configuration("33b921b8-edc5-46a0-8baa-d0b7ad84fc71") {
                                        Ok(config) => {
                                            info!("Successfully read configuration from device: {}", device_path);
                                            Ok(config)
                                        }
                                        Err(e) => {
                                            warn!("Failed to read configuration from device {}: {}", device_path, e);
                                            Err(format!("Failed to read configuration: {}", e))
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to lock device {} for reading: {}", device_path, e);
                                    Err(format!("Failed to lock device: {}", e))
                                }
                            }
                        },
                        |result| match result {
                            Ok(config) => crate::ui::messages::Message::Edit(EditMessage::DeviceConfigurationLoaded(config)),
                            Err(err) => crate::ui::messages::Message::Edit(EditMessage::DeviceConfigurationLoadFailed(err)),
                        }
                    )
                } else {
                    // Device not found, stay in current state
                    Task::none()
                }
            } else {
                // No device selected, stay in current state
                Task::none()
            }
        }

        EditMessage::DeviceConfigurationLoaded(config) => {
            // Convert loaded configuration to UI state
            let is_wallet_valid = config.wallet_address.is_empty() 
                || crate::utils::eth::is_valid_eth_address(&config.wallet_address);
            
            state.workflow_state = EditWorkflowState::EditConfiguration {
                payment_network: config.payment_network,
                subnet: config.subnet,
                network_type: config.network_type,
                wallet_address: config.wallet_address,
                is_wallet_valid,
            };
            info!("Configuration loaded from device successfully");
            Task::none()
        }

        EditMessage::DeviceConfigurationLoadFailed(error) => {
            // Fall back to default values if configuration loading failed
            warn!("Failed to load device configuration, using defaults: {}", error);
            state.workflow_state = EditWorkflowState::EditConfiguration {
                payment_network: crate::models::PaymentNetwork::Testnet,
                subnet: "public".to_string(),
                network_type: crate::models::NetworkType::Central,
                wallet_address: String::new(),
                is_wallet_valid: true,
            };
            Task::none()
        }

        EditMessage::SetPaymentNetwork(network) => {
            if let EditWorkflowState::EditConfiguration {
                payment_network, ..
            } = &mut state.workflow_state
            {
                *payment_network = network;
            }
            Task::none()
        }

        EditMessage::SetSubnet(subnet) => {
            if let EditWorkflowState::EditConfiguration {
                subnet: current_subnet,
                ..
            } = &mut state.workflow_state
            {
                *current_subnet = subnet;
            }
            Task::none()
        }

        EditMessage::SetNetworkType(network_type) => {
            if let EditWorkflowState::EditConfiguration {
                network_type: current_type,
                ..
            } = &mut state.workflow_state
            {
                *current_type = network_type;
            }
            Task::none()
        }

        EditMessage::SetWalletAddress(address) => {
            if let EditWorkflowState::EditConfiguration {
                wallet_address,
                is_wallet_valid,
                ..
            } = &mut state.workflow_state
            {
                *wallet_address = address.clone();
                *is_wallet_valid =
                    address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            }
            Task::none()
        }

        EditMessage::SelectPreset(index) => {
            // Forward to the application level to handle preset selection
            Task::done(crate::ui::messages::Message::SelectPreset(index))
        }

        EditMessage::RefreshDevices => {
            debug!("Delegating device refresh to DeviceSelection module");
            Task::done(crate::ui::messages::Message::DeviceSelection(
                crate::ui::device_selection::DeviceMessage::RefreshDevices,
            ))
        }

        EditMessage::DevicesLoaded(_devices) => {
            // This message is no longer used - devices are handled by DeviceSelection module
            debug!("DevicesLoaded message deprecated - using shared device selection state");
            Task::none()
        }

        EditMessage::DeviceLoadFailed(error) => {
            // Set error message in edit state for display
            state.error_message = Some(error.clone());
            error!("Device loading failed: {}", error);
            Task::none()
        }

        EditMessage::DeviceLocked(disk) => {
            state.locked_disk = disk;
            if state.locked_disk.is_some() {
                info!("Device locked for editing");
            }
            Task::none()
        }

        EditMessage::SaveConfiguration => {
            // Save configuration to the selected device
            if let (
                Some(device_index),
                EditWorkflowState::EditConfiguration {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    ..
                },
            ) = (state.selected_device, &state.workflow_state)
            {
                // Get the device path from the device selection state
                if let Some(device) = device_selection.devices.get(device_index) {
                    debug!(
                        "Saving configuration to device: {} ({})",
                        device.name, device.path
                    );

                    let device_path = device.path.clone();
                    let payment_network = *payment_network;
                    let subnet = subnet.clone();
                    let network_type = *network_type;
                    let wallet_address = wallet_address.clone();

                    Task::perform(
                        async move {
                            use crate::disk::Disk;
                            use crate::disk::ImageConfiguration;

                            info!("Starting configuration save to device: {}", device_path);

                            // Create configuration from current settings
                            let config = ImageConfiguration {
                                payment_network,
                                network_type,
                                subnet,
                                wallet_address,
                                glm_per_hour: "0.25".to_string(), // Default value
                            };

                            // Attempt to write configuration to the device
                            match Disk::write_configuration_to_disk(&device_path, config).await {
                                Ok(()) => {
                                    info!(
                                        "Configuration successfully saved to device: {}",
                                        device_path
                                    );
                                    Ok(())
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to save configuration to device {}: {}",
                                        device_path, e
                                    );
                                    Err(format!("Failed to save configuration: {}", e))
                                }
                            }
                        },
                        |result: Result<(), String>| match result {
                            Ok(()) => {
                                crate::ui::messages::Message::Edit(EditMessage::ConfigurationSaved)
                            }
                            Err(err) => {
                                error!("Configuration save failed: {}", err);
                                crate::ui::messages::Message::Edit(
                                    EditMessage::ConfigurationSaveFailed,
                                )
                            }
                        },
                    )
                } else {
                    error!(
                        "Cannot save configuration: device index {} not found",
                        device_index
                    );
                    Task::done(crate::ui::messages::Message::Edit(
                        EditMessage::ConfigurationSaveFailed,
                    ))
                }
            } else {
                error!(
                    "Cannot save configuration: no device selected or not in edit configuration state"
                );
                Task::done(crate::ui::messages::Message::Edit(
                    EditMessage::ConfigurationSaveFailed,
                ))
            }
        }

        EditMessage::ConfigurationSaved => {
            state.workflow_state = EditWorkflowState::Completion(true);
            info!("Configuration saved successfully");
            Task::none()
        }

        EditMessage::ConfigurationSaveFailed => {
            state.workflow_state = EditWorkflowState::Completion(false);
            error!("Failed to save configuration");
            Task::none()
        }

        // App-level navigation messages that need to be forwarded
        EditMessage::BackToMainMenu => Task::done(crate::ui::messages::Message::BackToMainMenu),

        EditMessage::BackToDeviceSelection => {
            // Reset to device selection state
            state.workflow_state = EditWorkflowState::SelectDevice;
            Task::none()
        }

        EditMessage::EditAnother => {
            *state = EditState::new();
            Task::none()
        }
    }
}
