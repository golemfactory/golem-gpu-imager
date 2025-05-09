use iced::Font;
use iced::wgpu::naga::MathFunction::Length;
use iced::widget::text;

fn icon(unicode: char) -> iced::widget::Text<'static> {
    text(unicode.to_string())
        .font(Font::with_name("Material Icons"))
        .width(25)
        .align_x(iced::Center)
}

pub fn house() -> iced::widget::Text<'static> {
    icon('\u{E88a}')
}

pub fn start() -> iced::widget::Text<'static> {
    icon('\u{E089}')
}

pub fn navigate_before() -> iced::widget::Text<'static> {
    icon('\u{E408}')
}

pub fn navigate_next() -> iced::widget::Text<'static> {
    icon('\u{E409}')
}

pub fn download() -> iced::widget::Text<'static> {
    icon('\u{F090}')
}

pub fn warning() -> iced::widget::Text<'static> {
    icon('\u{E002}')
}

pub fn storage() -> iced::widget::Text<'static> {
    icon('\u{e1dB}')
}

pub fn sd_storage() -> iced::widget::Text<'static> {
    icon('\u{E1C2}')
}

pub fn usb() -> iced::widget::Text<'static> {
    icon('\u{E1E0}')
}

pub fn refresh() -> iced::widget::Text<'static> {
    icon('\u{E5D5}')
}

pub fn rocket_launch() -> iced::widget::Text<'static> {
    icon('\u{EB9B}')
}

pub fn info() -> iced::widget::Text<'static> {
    icon('\u{E88E}')
}

pub fn checkmark() -> iced::widget::Text<'static> {
    icon('\u{E5CA}')
}

pub fn error() -> iced::widget::Text<'static> {
    icon('\u{E000}')
}
