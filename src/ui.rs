pub mod application;
pub mod edit;
pub mod flash;
mod icons;
pub mod start_screen;

pub use application::GolemGpuImager;
pub use edit::{view_edit_completion, view_edit_configuration, view_select_existing_device};
pub use flash::{
    view_configure_settings, view_flash_completion, view_select_os_image,
    view_select_target_device, view_writing_process,
};
use iced::Font;
pub use start_screen::view_start_screen;

// Include the logo SVG data
pub const LOGO_SVG: &[u8] = include_bytes!("assets/logo.svg");

pub const ICON_FONT: &[u8] = include_bytes!("assets/icons.ttf");
