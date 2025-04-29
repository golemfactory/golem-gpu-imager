use iced::widget::container;
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
