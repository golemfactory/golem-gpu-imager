#![allow(dead_code)]

use iced::Font;
use iced::widget::text;

fn icon(unicode: char) -> iced::widget::Text<'static> {
    text(unicode.to_string())
        .font(Font::with_name("Material Icons"))
        .width(25)
        .align_x(iced::Center)
}

// Larger icon function for device cards and prominent UI elements
fn device_icon(unicode: char) -> iced::widget::Text<'static> {
    text(unicode.to_string())
        .font(Font::with_name("Material Icons"))
        .size(32)
        .width(40)
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

#[allow(dead_code)]
pub fn download() -> iced::widget::Text<'static> {
    icon('\u{F090}')
}

pub fn warning() -> iced::widget::Text<'static> {
    icon('\u{E002}')
}

pub fn storage() -> iced::widget::Text<'static> {
    device_icon('\u{e1dB}') // Material Icons storage - larger for device cards
}

#[allow(dead_code)]
pub fn sd_storage() -> iced::widget::Text<'static> {
    icon('\u{E1C2}')
}

pub fn usb() -> iced::widget::Text<'static> {
    device_icon('\u{E1E0}') // Material Icons usb - larger for device cards
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
    icon('\u{E872}') // Material Icons delete (more appropriate for cancel)
}

pub fn timer() -> iced::widget::Text<'static> {
    icon('\u{E425}')
}

pub fn check_circle() -> iced::widget::Text<'static> {
    icon('\u{E86C}')
}

#[allow(dead_code)]
pub fn help() -> iced::widget::Text<'static> {
    icon('\u{E887}')
}

#[allow(dead_code)]
pub fn warning_amber() -> iced::widget::Text<'static> {
    icon('\u{E002}')
}

#[allow(dead_code)]
pub fn downloading() -> iced::widget::Text<'static> {
    icon('\u{F090}')
}

#[allow(dead_code)]
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

pub fn analytics() -> iced::widget::Text<'static> {
    icon('\u{E880}') // Material Icons analytics
}

pub fn verified() -> iced::widget::Text<'static> {
    icon('\u{E86C}') // Material Icons check_circle (verified)
}

// Device type icons for better device identification - larger size for device cards
pub fn sd_card() -> iced::widget::Text<'static> {
    device_icon('\u{E1C2}') // Material Icons sd_card
}

pub fn memory() -> iced::widget::Text<'static> {
    device_icon('\u{E322}') // Material Icons memory (for eMMC/internal storage)
}

pub fn hard_drive() -> iced::widget::Text<'static> {
    device_icon('\u{E1DB}') // Material Icons storage (for hard drives)
}

// Security and elevation icons
pub fn shield() -> iced::widget::Text<'static> {
    icon('\u{E9E0}') // Material Icons shield
}

pub fn security() -> iced::widget::Text<'static> {
    icon('\u{E32A}') // Material Icons security
}

pub fn file_download() -> iced::widget::Text<'static> {
    icon('\u{E2C4}') // Material Icons file_download
}

pub fn file_upload() -> iced::widget::Text<'static> {
    icon('\u{E2C6}') // Material Icons file_upload
}
