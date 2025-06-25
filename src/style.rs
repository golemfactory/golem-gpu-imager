use iced::widget::{button, container, text_input};
use iced::{Border, Color, Theme};
use std::sync::Arc;

// Main theme colors
pub const PRIMARY: Color = Color::from_rgb(0.0, 0.4, 0.8);
pub const BACKGROUND: Color = Color::from_rgb(0.05, 0.05, 0.1);
pub const TEXT: Color = Color::from_rgb(0.9, 0.9, 0.9);
pub const ERROR: Color = Color::from_rgb(0.9, 0.2, 0.2);
pub const SUCCESS: Color = Color::from_rgb(0.0, 0.8, 0.3);
pub const WARNING: Color = Color::from_rgb(0.9, 0.6, 0.0);

pub fn custom_theme() -> Theme {
    let palette = iced::theme::Palette {
        background: BACKGROUND,
        text: TEXT,
        primary: PRIMARY,
        success: SUCCESS,
        danger: ERROR,
        warning: WARNING,
    };

    Theme::Custom(Arc::new(iced::theme::Custom::new(
        "golem-dark".to_string(),
        palette,
    )))
}

pub fn main_box(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn bordered_box(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: 1.0,
            radius: 5.0.into(),
            color: palette.background.strong.color,
        },
        ..container::Style::default()
    }
}

// Style function for valid wallet input
pub fn valid_wallet_input(
    theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: palette.background.weak.color.into(),
        border: Border {
            radius: 5.0.into(),
            width: 2.0,
            color: SUCCESS,
        },
        icon: TEXT,
        placeholder: palette.background.strong.color,
        value: TEXT,
        selection: palette.primary.weak.color,
    }
}

// Style function for invalid wallet input
pub fn invalid_wallet_input(
    theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: palette.background.weak.color.into(),
        border: Border {
            radius: 5.0.into(),
            width: 2.0,
            color: ERROR,
        },
        icon: TEXT,
        placeholder: palette.background.strong.color,
        value: TEXT,
        selection: palette.primary.weak.color,
    }
}

// Container style for validation success message
pub fn valid_message_container(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(SUCCESS),
        ..container::Style::default()
    }
}

// Container style for validation error message
pub fn invalid_message_container(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(ERROR),
        ..container::Style::default()
    }
}

// Default text input style
pub fn default_text_input(
    theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: palette.background.weak.color.into(),
        border: Border {
            radius: 5.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        icon: TEXT,
        placeholder: palette.background.strong.color,
        value: TEXT,
        selection: palette.primary.weak.color,
    }
}

// Container style for selected items (like selected preset)
pub fn selected_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.primary.weak.color.into()),
        border: Border {
            width: 2.0,
            radius: 5.0.into(),
            color: palette.primary.base.color,
        },
        text_color: Some(palette.primary.strong.text),
        ..container::Style::default()
    }
}

// Enhanced container style for selected OS images with better readability
pub fn selected_os_image_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 3.0,
            radius: 8.0.into(),
            color: PRIMARY, // Use the custom PRIMARY color for strong visual feedback
        },
        text_color: Some(TEXT), // Ensure high contrast text
        shadow: iced::Shadow {
            color: PRIMARY.scale_alpha(0.3),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
        ..container::Style::default()
    }
}

// Style for pick lists
pub fn pick_list_style(
    theme: &Theme,
    _status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let palette = theme.extended_palette();

    iced::widget::pick_list::Style {
        text_color: theme.palette().text,
        placeholder_color: palette.background.strong.text,
        background: iced::Background::Color(palette.background.weak.color),
        handle_color: palette.background.strong.color,
        border: Border {
            width: 1.0,
            radius: 5.0.into(),
            color: palette.background.strong.color,
        },
    }
}

// Enhanced device card styling for better UX
pub fn device_card_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: 1.0,
            radius: 8.0.into(),
            color: palette.background.strong.color,
        },
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.1),
            offset: iced::Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ..container::Style::default()
    }
}

// Selected device card with proper contrast and visual feedback
pub fn selected_device_card_container(theme: &Theme) -> container::Style {
    let _palette = theme.extended_palette();

    container::Style {
        // Light blue-white background for excellent contrast with dark text
        background: Some(Color::from_rgb(0.95, 0.97, 1.0).into()),
        border: Border {
            width: 2.0,
            radius: 8.0.into(),
            color: PRIMARY, // Strong blue border for clear selection
        },
        // Enhanced shadow for selected state
        shadow: iced::Shadow {
            color: PRIMARY.scale_alpha(0.2),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
        // Dark text color for high contrast on light background
        text_color: Some(Color::from_rgb(0.1, 0.1, 0.1)),
        ..container::Style::default()
    }
}

// Device card with hover effect
pub fn device_card_hover(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 1.5,
            radius: 8.0.into(),
            color: PRIMARY.scale_alpha(0.6),
        },
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.15),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 3.0,
        },
        ..container::Style::default()
    }
}

// Compact preset card styling
pub fn compact_preset_card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: 1.0,
            radius: 12.0.into(),
            color: palette.background.strong.color,
        },
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.08),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}

// Selected compact preset card
pub fn selected_compact_preset_card(theme: &Theme) -> container::Style {
    let _palette = theme.extended_palette();

    container::Style {
        background: Some(Color::from_rgb(0.96, 0.98, 1.0).into()),
        border: Border {
            width: 2.0,
            radius: 12.0.into(),
            color: PRIMARY,
        },
        shadow: iced::Shadow {
            color: PRIMARY.scale_alpha(0.15),
            offset: iced::Vector::new(0.0, 3.0),
            blur_radius: 12.0,
        },
        text_color: Some(Color::from_rgb(0.1, 0.1, 0.1)),
        ..container::Style::default()
    }
}

// Network badge styling for Testnet
pub fn testnet_badge(theme: &Theme) -> container::Style {
    let _palette = theme.extended_palette();

    container::Style {
        background: Some(Color::from_rgb(0.95, 0.7, 0.3).into()),
        border: Border {
            width: 0.0,
            radius: 12.0.into(),
            color: Color::TRANSPARENT,
        },
        text_color: Some(Color::from_rgb(0.4, 0.2, 0.0)),
        ..container::Style::default()
    }
}

// Network badge styling for Mainnet
pub fn mainnet_badge(theme: &Theme) -> container::Style {
    let _palette = theme.extended_palette();

    container::Style {
        background: Some(Color::from_rgb(0.7, 0.9, 0.7).into()),
        border: Border {
            width: 0.0,
            radius: 12.0.into(),
            color: Color::TRANSPARENT,
        },
        text_color: Some(Color::from_rgb(0.0, 0.3, 0.0)),
        ..container::Style::default()
    }
}

// Search input styling
pub fn search_input(theme: &Theme) -> iced::widget::text_input::Style {
    let palette = theme.extended_palette();

    iced::widget::text_input::Style {
        background: palette.background.weak.color.into(),
        border: Border {
            radius: 8.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        icon: TEXT,
        placeholder: palette.background.strong.color,
        value: TEXT,
        selection: palette.primary.weak.color,
    }
}

// Error text input style for invalid SSH keys
pub fn error_text_input(
    theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: palette.background.weak.color.into(),
        border: Border {
            radius: 5.0.into(),
            width: 2.0,
            color: ERROR,
        },
        icon: TEXT,
        placeholder: palette.background.strong.color,
        value: TEXT,
        selection: palette.primary.weak.color,
    }
}

// Default button style
pub fn default_button(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(palette.primary.base.color.into()),
        text_color: palette.primary.base.text,
        border: Border {
            radius: 5.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: iced::Shadow::default(),
    }
}

// Danger button style for remove actions
pub fn danger_button(theme: &Theme, _status: button::Status) -> button::Style {
    let _palette = theme.extended_palette();

    button::Style {
        background: Some(ERROR.into()),
        text_color: Color::WHITE,
        border: Border {
            radius: 5.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: iced::Shadow::default(),
    }
}

// Standardized page header container
pub fn page_header(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: 1.0,
            radius: 5.0.into(),
            color: palette.background.strong.color,
        },
        ..container::Style::default()
    }
}

// Standardized navigation back button (flexible width)
pub fn navigation_back_button(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.strong.text,
        border: Border {
            radius: 5.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: iced::Shadow::default(),
    }
}

// Standardized navigation action button (flexible width)
pub fn navigation_action_button(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(palette.primary.base.color.into()),
        text_color: palette.primary.base.text,
        border: Border {
            radius: 5.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: iced::Shadow::default(),
    }
}

// Standardized secondary cancel button for forms
pub fn cancel_button_secondary(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.strong.text,
        border: Border {
            radius: 5.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: iced::Shadow::default(),
    }
}

// Standardized danger cancel button for destructive actions
pub fn cancel_button_danger(theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(ERROR.into()),
        text_color: Color::WHITE,
        border: Border {
            radius: 5.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: iced::Shadow::default(),
    }
}

// Modal overlay - semi-transparent background covering the entire screen
pub fn modal_overlay(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..container::Style::default()
    }
}

// Confirmation dialog - centered dialog box with border and background
pub fn confirmation_dialog(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 2.0,
            radius: 10.0.into(),
            color: palette.background.strong.color,
        },
        text_color: Some(TEXT),
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.8),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
        ..container::Style::default()
    }
}
