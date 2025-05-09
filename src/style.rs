use iced::widget::{container, text, text_input};
use iced::{Border, Color, Theme};
use std::sync::Arc;

// Main theme colors
pub const PRIMARY: Color = Color::from_rgb(0.0, 0.4, 0.8);
pub const SECONDARY: Color = Color::from_rgb(0.1, 0.2, 0.3);
pub const ACCENT: Color = Color::from_rgb(0.0, 0.7, 0.4);
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
    status: iced::widget::text_input::Status,
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
    status: iced::widget::text_input::Status,
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

// Style function for validation success text
pub fn valid_text_style(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(SUCCESS),
        ..text::Style::default()
    }
}

// Style function for validation error text
pub fn invalid_text_style(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(ERROR),
        ..text::Style::default()
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
    status: iced::widget::text_input::Status,
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

// Style for pick lists
pub fn pick_list_style(
    theme: &Theme,
    _status: iced::widget::pick_list::Status
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
