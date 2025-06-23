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
                    debug!(
                        "Reading configuration from device: {} ({})",
                        device.name, device.path
                    );

                    Task::perform(
                        async move {
                            // Lock the device for reading
                            match crate::disk::Disk::lock_path(&device_path, true).await {
                                Ok(mut disk) => {
                                    // Read configuration from device
                                    match disk
                                        .read_configuration("33b921b8-edc5-46a0-8baa-d0b7ad84fc71")
                                    {
                                        Ok(config) => {
                                            info!(
                                                "Successfully read configuration from device: {}",
                                                device_path
                                            );
                                            Ok(config)
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to read configuration from device {}: {}",
                                                device_path, e
                                            );
                                            Err(format!("Failed to read configuration: {}", e))
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to lock device {} for reading: {}",
                                        device_path, e
                                    );
                                    Err(format!("Failed to lock device: {}", e))
                                }
                            }
                        },
                        |result| match result {
                            Ok(config) => crate::ui::messages::Message::Edit(
                                EditMessage::DeviceConfigurationLoaded(config),
                            ),
                            Err(err) => crate::ui::messages::Message::Edit(
                                EditMessage::DeviceConfigurationLoadFailed(err),
                            ),
                        },
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
            // Set the workflow state to configuration mode
            state.workflow_state = EditWorkflowState::EditConfiguration;

            // Send the loaded configuration to the central configuration state
            info!("Configuration loaded from device successfully");
            Task::done(crate::ui::messages::Message::Configuration(
                crate::ui::configuration::ConfigurationMessage::LoadFromDevice(config),
            ))
        }

        EditMessage::DeviceConfigurationLoadFailed(error) => {
            // Fall back to default values if configuration loading failed
            warn!(
                "Failed to load device configuration, using defaults: {}",
                error
            );
            state.workflow_state = EditWorkflowState::EditConfiguration;

            // Reset configuration to defaults
            Task::done(crate::ui::messages::Message::Configuration(
                crate::ui::configuration::ConfigurationMessage::Reset,
            ))
        }


        EditMessage::SaveConfiguration => {
            // Save configuration to the selected device using central configuration
            if let Some(device_index) = state.selected_device {
                // Get the device path from the device selection state
                if let Some(device) = device_selection.devices.get(device_index) {
                    debug!(
                        "Initiating configuration save to device: {} ({})",
                        device.name, device.path
                    );

                    // Forward to central configuration handler which has access to the configuration state
                    Task::done(crate::ui::messages::Message::Configuration(
                        crate::ui::configuration::ConfigurationMessage::SaveToDevice(
                            device.path.clone(),
                        ),
                    ))
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
                error!("Cannot save configuration: no device selected");
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

        EditMessage::RefreshDevices => {
            debug!("Delegating device refresh to DeviceSelection module");
            Task::done(crate::ui::messages::Message::DeviceSelection(
                crate::ui::device_selection::DeviceMessage::RefreshDevices,
            ))
        }
    }
}
