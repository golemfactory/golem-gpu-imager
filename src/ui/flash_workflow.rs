pub mod state;
pub mod messages;
pub mod handler;
pub mod ui;

pub use state::*;
pub use messages::*;
pub use handler::*;
pub use ui::*;

use iced::Element;

/// Module-level view function that delegates to appropriate UI functions based on workflow state
pub fn view<'a>(
    flash_state: &'a FlashState, 
    device_selection: &'a crate::ui::device_selection::DeviceSelectionState,
    is_loading_repo: bool
) -> Element<'a, FlashMessage> {
    match &flash_state.workflow_state {
        FlashWorkflowState::SelectOsImage => {
            if !flash_state.os_image_groups.is_empty() {
                ui::view_select_os_image_groups(
                    &flash_state.os_image_groups,
                    flash_state.selected_os_image_group,
                    is_loading_repo,
                )
            } else {
                ui::view_select_os_image(
                    &flash_state.os_images,
                    flash_state.selected_os_image,
                    is_loading_repo,
                )
            }
        }
        FlashWorkflowState::ProcessingImage { 
            version_id, download_progress, metadata_progress, overall_progress, 
            channel, created_date, phase, uncompressed_size 
        } => {
            ui::view_processing_image(
                version_id,
                *download_progress,
                *metadata_progress,
                *overall_progress,
                channel,
                created_date,
                phase,
                *uncompressed_size,
            )
        }
        FlashWorkflowState::SelectTargetDevice => {
            ui::view_select_target_device(
                &device_selection.devices,
                flash_state.selected_device,
            )
        }
        FlashWorkflowState::ConfigureSettings { 
            payment_network, 
            subnet, 
            network_type, 
            wallet_address, 
            is_wallet_valid 
        } => {
            // For now, create a simple flash-specific configuration view
            ui::view_flash_configure_settings(
                *payment_network,
                subnet.clone(),
                *network_type,
                wallet_address.clone(),
                *is_wallet_valid,
            )
        }
        FlashWorkflowState::ClearingPartitions(progress) => {
            ui::view_writing_process(*progress, "Clearing Partitions")
        }
        FlashWorkflowState::WritingImage(progress) => {
            ui::view_writing_process(*progress, "Writing Image")
        }
        FlashWorkflowState::VerifyingImage(progress) => {
            ui::view_writing_process(*progress, "Verifying Image")
        }
        FlashWorkflowState::WritingConfig(progress) => {
            ui::view_writing_process(*progress, "Writing Configuration")
        }
        FlashWorkflowState::WritingProcess(progress) => {
            ui::view_writing_process(*progress, "Writing Process")
        }
        FlashWorkflowState::Completion(success) => {
            ui::view_flash_completion(*success, None)
        }
    }
}