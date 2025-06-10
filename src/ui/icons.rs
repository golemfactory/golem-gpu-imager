use iced::Font;
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

// Preset management icons
pub fn star() -> iced::widget::Text<'static> {
    icon('\u{E838}')
}

pub fn star_border() -> iced::widget::Text<'static> {
    icon('\u{E83A}')
}

pub fn delete() -> iced::widget::Text<'static> {
    icon('\u{E872}')
}

pub fn save() -> iced::widget::Text<'static> {
    icon('\u{E161}')
}

pub fn settings() -> iced::widget::Text<'static> {
    icon('\u{E8B8}')
}

pub fn tune() -> iced::widget::Text<'static> {
    icon('\u{E429}')
}

pub fn send() -> iced::widget::Text<'static> {
    icon('\u{E163}')
}

pub fn edit() -> iced::widget::Text<'static> {
    icon('\u{E3C9}')
}

// Additional icons for improved UI/UX
pub fn cancel() -> iced::widget::Text<'static> {
    icon('\u{E5C9}')
}

pub fn timer() -> iced::widget::Text<'static> {
    icon('\u{E425}')
}

pub fn check_circle() -> iced::widget::Text<'static> {
    icon('\u{E86C}')
}

pub fn help() -> iced::widget::Text<'static> {
    icon('\u{E887}')
}

pub fn warning_amber() -> iced::widget::Text<'static> {
    icon('\u{E002}')
}

pub fn downloading() -> iced::widget::Text<'static> {
    icon('\u{F090}')
}

pub fn device_hub() -> iced::widget::Text<'static> {
    icon('\u{E335}')
}

pub fn install() -> iced::widget::Text<'static> {
    icon('\u{E923}')
}

// Expand/collapse icons for version history
pub fn expand_more() -> iced::widget::Text<'static> {
    icon('\u{E5CF}') // Material Icons expand_more (down arrow)
}

pub fn expand_less() -> iced::widget::Text<'static> {
    icon('\u{E5CE}') // Material Icons expand_less (up arrow)
}

// Additional icons for improved UX
pub fn check() -> iced::widget::Text<'static> {
    icon('\u{E5CA}') // Material Icons check
}

pub fn get_app() -> iced::widget::Text<'static> {
    icon('\u{E884}') // Material Icons get_app (download)
}
