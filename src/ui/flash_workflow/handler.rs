use super::{FlashState, FlashMessage, FlashWorkflowState};
use crate::models::CancelToken;
use crate::utils::repo::ImageRepo;
use iced::Task;
use std::sync::Arc;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut FlashState,
    image_repo: &Arc<ImageRepo>,
    cancel_token: &CancelToken,
    message: FlashMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        FlashMessage::SelectOsImage(index) => {
            if let Some(image) = state.os_images.get(index) {
                state.selected_os_image = Some(index);
                debug!("Selected OS image: {}", image.name);
            }
            Task::none()
        }
        
        FlashMessage::SelectOsImageFromGroup(group_index, version_index) => {
            if let Some(group) = state.os_image_groups.get(group_index) {
                let image = if version_index == 0 {
                    &group.latest_version
                } else if let Some(older_image) = group.older_versions.get(version_index - 1) {
                    older_image
                } else {
                    return Task::none();
                };

                state.selected_os_image_group = Some((group_index, version_index));
                debug!("Selected OS image from group: {} version {}", image.name, image.version);
            }
            Task::none()
        }
        
        FlashMessage::ToggleVersionHistory(group_index) => {
            if let Some(group) = state.os_image_groups.get_mut(group_index) {
                group.expanded = !group.expanded;
                debug!("Toggled version history for group {}: expanded={}", group_index, group.expanded);
            }
            Task::none()
        }
        
        FlashMessage::GotoSelectTargetDevice => {
            state.workflow_state = FlashWorkflowState::SelectTargetDevice;
            Task::none()
        }
        
        FlashMessage::GotoConfigureSettings => {
            state.workflow_state = FlashWorkflowState::ConfigureSettings {
                payment_network: crate::models::PaymentNetwork::Testnet,
                subnet: "public".to_string(),
                network_type: crate::models::NetworkType::Central,
                wallet_address: String::new(),
                is_wallet_valid: true,
            };
            Task::none()
        }
        
        FlashMessage::SetPaymentNetwork(network) => {
            if let FlashWorkflowState::ConfigureSettings { payment_network, .. } = &mut state.workflow_state {
                *payment_network = network;
            }
            Task::none()
        }
        
        FlashMessage::SetSubnet(subnet) => {
            if let FlashWorkflowState::ConfigureSettings { subnet: current_subnet, .. } = &mut state.workflow_state {
                *current_subnet = subnet;
            }
            Task::none()
        }
        
        FlashMessage::SetNetworkType(network_type) => {
            if let FlashWorkflowState::ConfigureSettings { network_type: current_type, .. } = &mut state.workflow_state {
                *current_type = network_type;
            }
            Task::none()
        }
        
        FlashMessage::SetWalletAddress(address) => {
            if let FlashWorkflowState::ConfigureSettings { wallet_address, is_wallet_valid, .. } = &mut state.workflow_state {
                *wallet_address = address.clone();
                *is_wallet_valid = address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            }
            Task::none()
        }
        
        FlashMessage::SelectTargetDevice(index) => {
            state.selected_device = Some(index);
            debug!("Selected target device: {}", index);
            Task::none()
        }
        
        FlashMessage::ProcessingProgress(version_id, progress) => {
            // Update download progress for specific version
            if let Some(download) = state.downloads_in_progress.iter_mut().find(|(id, _)| id == &version_id) {
                download.1 = progress.overall_progress;
            }
            
            // Update state if this is the currently processing image
            if let FlashWorkflowState::ProcessingImage { version_id: current_id, .. } = &mut state.workflow_state {
                if current_id == &version_id {
                    *current_id = version_id;
                    // Update the state with progress details
                    if let FlashWorkflowState::ProcessingImage { 
                        download_progress, 
                        metadata_progress, 
                        overall_progress, 
                        phase,
                        uncompressed_size,
                        .. 
                    } = &mut state.workflow_state {
                        *download_progress = progress.download_progress;
                        *metadata_progress = progress.metadata_progress;
                        *overall_progress = progress.overall_progress;
                        *phase = progress.phase.clone();
                        if let Some(size) = progress.uncompressed_size {
                            *uncompressed_size = Some(size);
                        }
                    }
                }
            }
            Task::none()
        }
        
        FlashMessage::ProcessingCompleted(version_id, metadata) => {
            // Remove from downloads in progress
            state.downloads_in_progress.retain(|(id, _)| id != &version_id);
            
            // Update the image metadata
            if let Some(image) = state.os_images.iter_mut().find(|img| img.version == version_id) {
                image.metadata = Some(metadata.clone());
            }
            
            // Also update in groups
            for group in &mut state.os_image_groups {
                if group.latest_version.version == version_id {
                    group.latest_version.metadata = Some(metadata.clone());
                }
                for older_version in &mut group.older_versions {
                    if older_version.version == version_id {
                        older_version.metadata = Some(metadata.clone());
                    }
                }
            }
            
            // Go back to image selection
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            info!("Processing completed for version: {}", version_id);
            Task::none()
        }
        
        FlashMessage::ProcessingFailed(version_id, error) => {
            // Remove from downloads in progress
            state.downloads_in_progress.retain(|(id, _)| id != &version_id);
            
            // Go back to image selection
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            error!("Processing failed for version {}: {}", version_id, error);
            Task::done(crate::ui::messages::Message::ShowError(format!("Failed to process image: {}", error)))
        }
        
        FlashMessage::BackToSelectOsImage => {
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            Task::none()
        }
        
        FlashMessage::FlashAnother => {
            *state = FlashState::new();
            Task::none()
        }
        
        // Add more message handlers as needed
        _ => {
            debug!("Unhandled flash message: {:?}", message);
            Task::none()
        }
    }
}