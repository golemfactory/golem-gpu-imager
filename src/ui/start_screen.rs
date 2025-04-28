use iced::alignment::Horizontal;
use iced::widget::{button, column, container, svg, text};
use iced::{Alignment, Element, Length};

use crate::models::Message;
use crate::ui::LOGO_SVG;

pub fn view_start_screen() -> Element<'static, Message> {
    // Create the logo widget from the included SVG data
    let logo = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(180)
        .height(180);

    let title = text("Golem GPU Imager")
        .size(38)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let description = text(
        "A utility to flash OS images onto Golem GPU devices or edit existing configurations.",
    )
    .size(16)
    .width(Length::Fill)
    .align_x(Horizontal::Center);

    // Create buttons
    let flash_button = button("Flash New Image")
        .width(250)
        .padding(14)
        .style(button::primary)
        .on_press(Message::FlashNewImage);

    let edit_button = button("Edit Existing Disk")
        .width(250)
        .padding(14)
        .style(button::secondary)
        .on_press(Message::EditExistingDisk);

    // Add version info
    let version_text = text("v0.1.0")
        .size(12);

    // Main content column
    let content = column![
        logo,
        title, 
        container(description).padding([0, 20]),
        flash_button, 
        edit_button,
        container(column![]).height(Length::Fill),
        version_text
    ]
    .width(Length::Fill)
    .spacing(15)
    .align_x(Alignment::Center)
    .padding(30);

    // Main container with background
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}