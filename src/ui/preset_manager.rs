pub mod handler;
pub mod messages;
pub mod state;
pub mod ui;

pub use handler::*;
pub use messages::*;
pub use state::*;
pub use ui::*;

use iced::Element;

/// Module-level view function for preset manager
pub fn view<'a>(state: &'a PresetManagerState) -> Element<'a, PresetManagerMessage> {
    ui::view_preset_manager(
        &state.presets,
        None, // Don't show any preset as selected in management view
        &state.new_preset_name,
        state.editor.as_ref(),
        state.deletion_confirmation.as_ref(),
    )
}
