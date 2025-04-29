use iced::window::{Icon, Settings, icon};

mod models;
mod style;
mod ui;
mod version;

pub fn main() -> iced::Result {
    let mut settings = Settings::default();

    settings.icon = Some(icon::from_file_data(include_bytes!("./assets/icon.png"), None).unwrap());

    iced::application(
        ui::GolemGpuImager::new,
        ui::GolemGpuImager::update,
        ui::GolemGpuImager::view,
    )
    .title(ui::GolemGpuImager::title)
    .font(ui::ICON_FONT)
    .window(settings)
    .window_size(iced::Size::new(560f32, 720f32))
    .theme(|_| style::custom_theme())
    .centered()
    .run()
}
