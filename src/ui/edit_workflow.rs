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
    edit_state: &'a EditState,
    device_selection: &'a crate::ui::device_selection::DeviceSelectionState,
    preset_manager: &'a crate::ui::preset_manager::PresetManagerState,
) -> Element<'a, crate::ui::messages::Message> {
    match &edit_state.workflow_state {
        EditWorkflowState::SelectDevice => {
            ui::view_select_existing_device(&device_selection.devices, edit_state.selected_device)
                .map(crate::ui::messages::Message::Edit)
        }
        EditWorkflowState::EditConfiguration {
            payment_network,
            subnet,
            network_type,
            wallet_address,
            is_wallet_valid,
        } => ui::view_edit_configuration(
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
        ),
        EditWorkflowState::Completion(success) => {
            ui::view_edit_completion(*success).map(crate::ui::messages::Message::Edit)
        }
    }
}
