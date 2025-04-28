pub mod start_screen;
pub mod flash;
pub mod edit;
pub mod application;

pub use start_screen::view_start_screen;
pub use flash::{view_select_os_image, view_configure_settings, view_select_target_device, view_writing_process, view_flash_completion};
pub use edit::{view_select_existing_device, view_edit_configuration, view_edit_completion};
pub use application::GolemGpuImager;

// Include the logo SVG data
pub const LOGO_SVG: &[u8] = include_bytes!("assets/logo.svg");