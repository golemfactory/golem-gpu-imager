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
    configuration: &'a crate::ui::configuration::ConfigurationState,
    preset_manager: &'a crate::ui::preset_manager::PresetManagerState,
) -> Element<'a, crate::ui::messages::Message> {
    match &edit_state.workflow_state {
        EditWorkflowState::SelectDevice => {
            ui::view_select_existing_device(&device_selection.devices, edit_state.selected_device)
                .map(crate::ui::messages::Message::Edit)
        }
        EditWorkflowState::LoadingConfiguration => {
            ui::view_loading_configuration().map(crate::ui::messages::Message::Edit)
        }
        EditWorkflowState::EditConfiguration => ui::view_edit_configuration(
            configuration,
            &preset_manager.presets,
            &preset_manager.new_preset_name,
        ),
        EditWorkflowState::Completion(success) => {
            ui::view_edit_completion(*success).map(crate::ui::messages::Message::Edit)
        }
    }
}
