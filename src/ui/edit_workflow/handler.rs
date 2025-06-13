use super::{EditState, EditMessage, EditWorkflowState};
use iced::Task;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut EditState,
    message: EditMessage,
) -> Task<crate::models::Message> {
    match message {
        EditMessage::SelectExistingDevice(index) => {
            if index < state.storage_devices.len() {
                state.selected_device = Some(index);
                debug!("Selected device for editing: {}", index);
            }
            Task::none()
        }
        
        EditMessage::GotoEditConfiguration => {
            if state.selected_device.is_some() {
                state.workflow_state = EditWorkflowState::EditConfiguration {
                    payment_network: crate::ui::flash_workflow::PaymentNetwork::Testnet,
                    subnet: "public".to_string(),
                    network_type: crate::ui::flash_workflow::NetworkType::Central,
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
            // This would typically trigger a device enumeration
            debug!("Refreshing device list");
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
    }
}