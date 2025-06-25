pub mod application;
mod icons;
pub mod preset_editor;
pub mod start_screen;

// New modular workflow modules
pub mod configuration;
pub mod device_selection;
pub mod edit_workflow;
pub mod flash_workflow;
pub mod preset_manager;

// Unified message system
pub mod messages;

#[allow(unused_imports)]
pub use application::GolemGpuImager;
#[allow(unused_imports)]
pub use configuration::view::view_configuration_editor;
pub use edit_workflow::{
    view_edit_completion, view_edit_configuration, view_select_existing_device,
};
#[allow(unused_imports)]
pub use flash_workflow::{
    view_flash_completion, view_select_os_image, view_select_target_device, view_writing_process,
};
#[allow(unused_imports)]
use iced::Font;
pub use start_screen::view_start_screen;

// Include the logo SVG data
pub const LOGO_SVG: &[u8] = include_bytes!("assets/logo.svg");

pub const ICON_FONT: &[u8] = include_bytes!("assets/icons.ttf");
