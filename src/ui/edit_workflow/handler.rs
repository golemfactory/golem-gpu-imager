use super::{EditState, EditMessage, EditWorkflowState};
use iced::Task;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut EditState,
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
            if state.selected_device.is_some() {
                state.workflow_state = EditWorkflowState::EditConfiguration {
                    payment_network: crate::models::PaymentNetwork::Testnet,
                    subnet: "public".to_string(),
                    network_type: crate::models::NetworkType::Central,
                    wallet_address: String::new(),
                    is_wallet_valid: true,
                };
            }
            Task::none()
        }
        
        EditMessage::SetPaymentNetwork(network) => {
            if let EditWorkflowState::EditConfiguration { payment_network, .. } = &mut state.workflow_state {
                *payment_network = network;
            }
            Task::none()
        }
        
        EditMessage::SetSubnet(subnet) => {
            if let EditWorkflowState::EditConfiguration { subnet: current_subnet, .. } = &mut state.workflow_state {
                *current_subnet = subnet;
            }
            Task::none()
        }
        
        EditMessage::SetNetworkType(network_type) => {
            if let EditWorkflowState::EditConfiguration { network_type: current_type, .. } = &mut state.workflow_state {
                *current_type = network_type;
            }
            Task::none()
        }
        
        EditMessage::SetWalletAddress(address) => {
            if let EditWorkflowState::EditConfiguration { wallet_address, is_wallet_valid, .. } = &mut state.workflow_state {
                *wallet_address = address.clone();
                *is_wallet_valid = address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            }
            Task::none()
        }
        
        EditMessage::RefreshDevices => {
            debug!("Delegating device refresh to DeviceSelection module");
            Task::done(crate::ui::messages::Message::DeviceSelection(
                crate::ui::device_selection::DeviceMessage::RefreshDevices
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
            // This would trigger saving configuration to the device
            debug!("Saving configuration to device");
            Task::none()
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
        EditMessage::BackToMainMenu => {
            Task::done(crate::ui::messages::Message::BackToMainMenu)
        }
        
        EditMessage::EditAnother => {
            *state = EditState::new();
            Task::none()
        }
    }
}