use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, svg, text};
use iced::{Alignment, Color, Element, Length, Shadow, Theme, Vector};

use crate::models::Message;
use crate::ui::{LOGO_SVG, icons};

pub fn view_start_screen<'a>(
    error_message: Option<&'a str>,
    is_elevated: bool,
    _elevation_status: &'a str,
) -> Element<'a, Message> {
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

    // Create buttons - disable on Windows if not elevated
    let buttons_enabled = if cfg!(windows) { is_elevated } else { true };

    let mut flash_button = button(
        container(iced::widget::row![icons::start(), "Flash New Image",]).center_x(Length::Fill),
    )
    .width(250)
    .padding(14);

    if buttons_enabled {
        flash_button = flash_button
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
    } else {
        flash_button = flash_button.style(|theme: &Theme, _state| {
            let palette = theme.extended_palette();
            button::Style {
                background: Some(palette.background.weak.color.into()),
                text_color: palette.background.strong.color,
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Shadow::default(),
            }
        });
    }

    let mut edit_button = button(
        container(iced::widget::row![icons::edit(), "Edit Existing Disk"]).center_x(Length::Fill),
    )
    .width(250)
    .padding(14);

    if buttons_enabled {
        edit_button = edit_button
            .style(button::secondary)
            .on_press(Message::EditExistingDisk);
    } else {
        edit_button = edit_button.style(|theme: &Theme, _state| {
            let palette = theme.extended_palette();
            button::Style {
                background: Some(palette.background.weak.color.into()),
                text_color: palette.background.strong.color,
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Shadow::default(),
            }
        });
    }

    let mut presets_button = button(
        container(iced::widget::row![icons::settings(), "Manage Presets"]).center_x(Length::Fill),
    )
    .width(250)
    .padding(14);

    if buttons_enabled {
        presets_button = presets_button
            .style(button::secondary)
            .on_press(Message::ManagePresets);
    } else {
        presets_button = presets_button.style(|theme: &Theme, _state| {
            let palette = theme.extended_palette();
            button::Style {
                background: Some(palette.background.weak.color.into()),
                text_color: palette.background.strong.color,
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Shadow::default(),
            }
        });
    }

    // Error message container (only shown if error_message is Some)
    let error_container = if let Some(error) = error_message {
        let error_column = column![
            row![
                if error.contains("Error") || error.contains("Failed") {
                    icons::error().color(Color::from_rgb(0.8, 0.0, 0.0))
                } else {
                    icons::warning().color(Color::from_rgb(0.9, 0.6, 0.0))
                },
                text(error).size(14).color(
                    if error.contains("Error") || error.contains("Failed") {
                        Color::from_rgb(0.7, 0.0, 0.0) // Dark red for error text
                    } else {
                        Color::from_rgb(0.6, 0.4, 0.0) // Dark yellow/brown for warning text
                    }
                )
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ]
        .spacing(10)
        .align_x(Alignment::Center);

        container(error_column)
            .width(Length::Fill)
            .padding(15)
            .style(|_theme| container::Style {
                background: Some(
                    if error.contains("Error") || error.contains("Failed") {
                        Color::from_rgb(1.0, 0.95, 0.95) // Light red for errors
                    } else {
                        Color::from_rgb(1.0, 0.98, 0.9) // Light yellow for warnings  
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if error.contains("Error") || error.contains("Failed") {
                        Color::from_rgb(0.8, 0.0, 0.0) // Red border for errors
                    } else {
                        Color::from_rgb(0.9, 0.6, 0.0) // Orange border for warnings
                    },
                },
                ..container::Style::default()
            })
    } else {
        container(column![]) // Empty container if no error
    };

    // Create elevation prompt for Windows when not elevated
    let elevation_prompt = if !is_elevated && cfg!(windows) {
        // Show elevation button for all Windows users - UAC can prompt for admin credentials
        container(
            column![
                row![
                    icons::warning().color(Color::from_rgb(1.0, 0.6, 0.0)),
                    text("Administrator privileges required for disk operations")
                        .size(14)
                        .color(Color::from_rgb(0.7, 0.4, 0.0))
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                button(
                    row![icons::rocket_launch(), "Run as Administrator"]
                        .spacing(8)
                        .align_y(Alignment::Center)
                )
                .width(Length::Shrink)
                .padding(12)
                .style(|_theme: &Theme, state| {
                    let is_hover = matches!(state, button::Status::Hovered);
                    button::Style {
                        background: Some(
                            if is_hover {
                                Color::from_rgb(0.15, 0.55, 0.95)
                            } else {
                                Color::from_rgb(0.2, 0.6, 1.0)
                            }
                            .into(),
                        ),
                        text_color: Color::WHITE,
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: Shadow {
                            color: Color::from_rgb(0.0, 0.0, 0.0),
                            offset: Vector::new(2.0, 2.0),
                            blur_radius: if is_hover { 6.0 } else { 4.0 },
                        },
                    }
                })
                .on_press(Message::RequestElevation)
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(20)
        .style(|_theme| container::Style {
            background: Some(Color::from_rgb(1.0, 0.98, 0.9).into()),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.9, 0.6, 0.0),
            },
            ..container::Style::default()
        })
    } else {
        // Either elevated or not Windows - no elevation prompt needed
        container(column![]) // Empty container
    };

    // Add version and build time info
    let version_info = format!(
        "v{} • Built {}",
        crate::version::VERSION,
        crate::version::BUILD_TIME
    );
    let version_text = text(version_info).size(12);

    // Main content column
    let mut content_items = vec![
        logo.into(),
        title.into(),
        container(description).padding([0, 20]).into(),
    ];

    // Add error container if there's an error message
    if error_message.is_some() {
        content_items.push(error_container.into());
    }

    // Add elevation prompt for Windows when not elevated
    if !is_elevated && cfg!(windows) {
        content_items.push(elevation_prompt.into());
    }

    content_items.extend([
        container(iced::widget::row![]).height(Length::Fill).into(),
        flash_button.into(),
        edit_button.into(),
        presets_button.into(),
        container(column![]).height(Length::Fill).into(),
        version_text.into(),
    ]);

    let content = column(content_items)
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
