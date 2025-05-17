use iced::alignment::Horizontal;
use iced::widget::{button, column, container, svg, text};
use iced::{Alignment, Color, Element, Font, Length, Shadow, Theme, Vector};

use crate::models::Message;
use crate::ui::{LOGO_SVG, icons};

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
    let flash_button = button(
        container(iced::widget::row![icons::start(), "Flash New Image",]).center_x(Length::Fill),
    )
    .width(250)
    .padding(14)
    .style(|theme: &Theme, state| {
        let is_hover = matches!(state, button::Status::Hovered);
        let palette = theme.extended_palette();
        let mut style = button::primary(theme, state);
        style.shadow = Shadow {
            color: if is_hover {
                palette.background.strongest.color
            } else {
                palette.background.strong.color
            },
            offset: Vector::new(4.0, 4.0),
            blur_radius: if is_hover { 2.0 } else { 1.0 },
        };
        style
    })
    .on_press(Message::FlashNewImage);

    let edit_button = button(
        container(iced::widget::row![icons::edit(), "Edit Existing Disk"]).center_x(Length::Fill),
    )
    .width(250)
    .padding(14)
    .style(button::secondary)
    .on_press(Message::EditExistingDisk);

    // Add version info
    let version_text = text(crate::version::VERSION).size(12);

    // Main content column
    let content = column![
        logo,
        title,
        container(description).padding([0, 20]),
        container(iced::widget::row![]).height(Length::Fill),
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
