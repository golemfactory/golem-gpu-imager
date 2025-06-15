use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, svg, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Theme, Vector};
use iced::gradient;

use crate::ui::messages::Message;
use crate::ui::{LOGO_SVG, icons};

// Elegant gradient background styling
fn elegant_gradient_background() -> impl Fn(&Theme) -> container::Style {
    |theme: &Theme| {
        let palette = theme.extended_palette();
        let gradient = gradient::Linear::new(45.0)
            .add_stop(0.0, Color::from_rgb(0.03, 0.03, 0.08))
            .add_stop(1.0, Color::from_rgb(0.08, 0.08, 0.15));
        
        container::Style {
            background: Some(Background::Gradient(iced::Gradient::Linear(gradient))),
            ..container::Style::default()
        }
    }
}

// Elegant card container for buttons
fn elegant_button_card() -> impl Fn(&Theme) -> container::Style {
    |_theme: &Theme| {
        container::Style {
            background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.18, 0.8))),
            border: Border {
                width: 1.0,
                radius: 16.0.into(),
                color: Color::from_rgba(0.3, 0.3, 0.4, 0.3),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        }
    }
}

// Modern elegant button styling
fn elegant_primary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme: &Theme, status: button::Status| {
        let is_hover = matches!(status, button::Status::Hovered);
        let is_pressed = matches!(status, button::Status::Pressed);
        
        let background_color = if is_pressed {
            Color::from_rgb(0.0, 0.3, 0.7)
        } else if is_hover {
            Color::from_rgb(0.0, 0.45, 0.9)
        } else {
            Color::from_rgb(0.0, 0.4, 0.8)
        };
        
        button::Style {
            background: Some(Background::Color(background_color)),
            text_color: Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 12.0.into(),
                color: Color::TRANSPARENT,
            },
            shadow: Shadow {
                color: if is_hover {
                    Color::from_rgba(0.0, 0.4, 0.8, 0.5)
                } else {
                    Color::from_rgba(0.0, 0.4, 0.8, 0.3)
                },
                offset: Vector::new(0.0, if is_pressed { 2.0 } else { 6.0 }),
                blur_radius: if is_hover { 20.0 } else { 12.0 },
            },
        }
    }
}

// Modern elegant secondary button styling
fn elegant_secondary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme: &Theme, status: button::Status| {
        let is_hover = matches!(status, button::Status::Hovered);
        let is_pressed = matches!(status, button::Status::Pressed);
        
        let background_color = if is_pressed {
            Color::from_rgba(0.2, 0.2, 0.3, 0.8)
        } else if is_hover {
            Color::from_rgba(0.15, 0.15, 0.25, 0.9)
        } else {
            Color::from_rgba(0.1, 0.1, 0.2, 0.7)
        };
        
        button::Style {
            background: Some(Background::Color(background_color)),
            text_color: Color::from_rgb(0.9, 0.9, 0.9),
            border: Border {
                width: 1.0,
                radius: 12.0.into(),
                color: if is_hover {
                    Color::from_rgba(0.4, 0.4, 0.5, 0.6)
                } else {
                    Color::from_rgba(0.3, 0.3, 0.4, 0.4)
                },
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
                offset: Vector::new(0.0, if is_pressed { 1.0 } else { 4.0 }),
                blur_radius: if is_hover { 12.0 } else { 8.0 },
            },
        }
    }
}

pub fn view_start_screen<'a>(
    error_message: Option<&'a str>,
    is_elevated: bool,
    _elevation_status: &'a str,
) -> Element<'a, Message> {
    // Create the logo widget with subtle direct glow
    let logo = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(160)
        .height(160);

    let title = text("Golem GPU Imager")
        .size(38)
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .color(Color::from_rgb(0.95, 0.95, 0.95));

    let description = text(
        "A utility to flash OS images onto Golem GPU devices or edit existing configurations.",
    )
    .size(16)
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .color(Color::from_rgb(0.7, 0.7, 0.8));

    // Create buttons - disable on Windows if not elevated
    let buttons_enabled = if cfg!(windows) { is_elevated } else { true };

    let mut flash_button = button(
        container(
            iced::widget::row![
                icons::start().size(20), 
                text("Flash New Image").size(16)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ).center_x(Length::Fill),
    )
    .width(280)
    .padding(16);

    if buttons_enabled {
        flash_button = flash_button
            .style(elegant_primary_button())
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
        container(
            iced::widget::row![
                icons::edit().size(20), 
                text("Edit Existing Disk").size(16)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ).center_x(Length::Fill),
    )
    .width(280)
    .padding(16);

    if buttons_enabled {
        edit_button = edit_button
            .style(elegant_secondary_button())
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
        container(
            iced::widget::row![
                icons::settings().size(20), 
                text("Manage Presets").size(16)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ).center_x(Length::Fill),
    )
    .width(280)
    .padding(16);

    if buttons_enabled {
        presets_button = presets_button
            .style(elegant_secondary_button())
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

    // Add version and build time info with elegant styling
    let version_info = format!(
        "v{} • Built {}",
        crate::version::VERSION,
        crate::version::BUILD_TIME
    );
    let version_text = text(version_info)
        .size(12)
        .color(Color::from_rgb(0.5, 0.5, 0.6));

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

    // Create elegant button card container with optimized spacing
    let button_card = container(
        column![
            flash_button,
            edit_button,
            presets_button,
        ]
        .spacing(12)
        .align_x(Alignment::Center)
    )
    .style(elegant_button_card())
    .padding(20)
    .width(Length::Shrink);

    content_items.extend([
        container(iced::widget::row![]).height(Length::Fill).into(),
        button_card.into(),
        container(column![]).height(Length::Fill).into(),
        version_text.into(),
    ]);

    let content = column(content_items)
        .width(Length::Fill)
        .spacing(16)
        .align_x(Alignment::Center)
        .padding(20);

    // Main container with elegant gradient background
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(elegant_gradient_background())
        .into()
}
