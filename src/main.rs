mod models;
mod style;
mod ui;

pub fn main() -> iced::Result {
    iced::application(ui::GolemGpuImager::new, ui::GolemGpuImager::update, ui::GolemGpuImager::view)
        .title(ui::GolemGpuImager::title)
        .window_size(iced::Size::new(560f32, 720f32))
        .theme(|_| style::custom_theme())
        .centered()
        .run()
}