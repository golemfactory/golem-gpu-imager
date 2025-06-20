pub mod handler;
pub mod messages;
pub mod state;
pub mod ui;

pub use handler::*;
pub use messages::*;
pub use state::*;
pub use ui::*;

use iced::Element;

/// Module-level view function that delegates to appropriate UI functions based on workflow state
pub fn view<'a>(
    flash_state: &'a FlashState,
    device_selection: &'a crate::ui::device_selection::DeviceSelectionState,
    preset_manager: &'a crate::ui::preset_manager::PresetManagerState,
    is_loading_repo: bool,
) -> Element<'a, crate::ui::messages::Message> {
    match &flash_state.workflow_state {
        FlashWorkflowState::SelectOsImage => {
            if !flash_state.os_image_groups.is_empty() {
                ui::view_select_os_image_groups(
                    &flash_state.os_image_groups,
                    flash_state.selected_os_image_group,
                    is_loading_repo,
                )
                .map(crate::ui::messages::Message::Flash)
            } else {
                ui::view_select_os_image(
                    &flash_state.os_images,
                    flash_state.selected_os_image,
                    is_loading_repo,
                )
                .map(crate::ui::messages::Message::Flash)
            }
        }
        FlashWorkflowState::ProcessingImage {
            version_id,
            download_progress,
            metadata_progress,
            overall_progress,
            channel,
            created_date,
            phase,
            uncompressed_size,
        } => ui::view_processing_image(
            version_id,
            *download_progress,
            *metadata_progress,
            *overall_progress,
            channel,
            created_date,
            phase,
            *uncompressed_size,
        )
        .map(crate::ui::messages::Message::Flash),
        FlashWorkflowState::SelectTargetDevice => {
            ui::view_select_target_device(&device_selection.devices, flash_state.selected_device)
                .map(crate::ui::messages::Message::Flash)
        }
        FlashWorkflowState::ConfigureSettings {
            payment_network,
            subnet,
            network_type,
            wallet_address,
            is_wallet_valid,
        } => {
            // Use shared configuration editor with preset support
            // Return app messages directly (no mapping) to match edit workflow pattern
            ui::view_flash_configure_settings(
                *payment_network,
                subnet.clone(),
                *network_type,
                wallet_address.clone(),
                *is_wallet_valid,
                &preset_manager.presets,
                preset_manager.selected_preset,
                &preset_manager.new_preset_name,
                preset_manager.show_manager,
                preset_manager.editor.as_ref(),
            )
        }
        FlashWorkflowState::ClearingPartitions(progress) => {
            ui::view_writing_process(*progress, "Clearing Partitions")
                .map(crate::ui::messages::Message::Flash)
        }
        FlashWorkflowState::WritingImage(progress) => {
            ui::view_writing_process(*progress, "Writing Image")
                .map(crate::ui::messages::Message::Flash)
        }
        FlashWorkflowState::VerifyingImage(progress) => {
            ui::view_writing_process(*progress, "Verifying Image")
                .map(crate::ui::messages::Message::Flash)
        }
        FlashWorkflowState::WritingConfig(progress) => {
            ui::view_writing_process(*progress, "Writing Configuration")
                .map(crate::ui::messages::Message::Flash)
        }
        FlashWorkflowState::Completion(success) => {
            ui::view_flash_completion(*success, None).map(crate::ui::messages::Message::Flash)
        }
    }
}
