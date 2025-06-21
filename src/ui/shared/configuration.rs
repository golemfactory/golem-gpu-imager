use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Alignment, Element, Length};

use crate::models::ConfigurationPreset;
use crate::style;
use crate::ui::{
    configuration::{view_configuration, view_header, view_navigation, ConfigurationMessage, ConfigurationState},
    icons,
    messages::Message,
};

/// Workflow configuration editor with preset management
pub fn view_configuration_editor<'a, F>(
    configuration_state: &'a ConfigurationState,
    title: &'a str,
    description: &'a str,
    back_action: Message,
    next_action: Option<Message>,
    back_label: &'a str,
    next_label: &'a str,
    configuration_presets: &'a [ConfigurationPreset],
    new_preset_name: &'a str,
    preset_manager_action: Message,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let header = view_header(title, description);
    let preset_section = view_preset_section(configuration_presets, configuration_state.selected_preset, preset_manager_action, message_factory);
    let configuration_form = view_configuration(configuration_state, "Configuration", "", message_factory);
    let save_preset_section = view_save_preset_section(new_preset_name, configuration_state);
    let navigation = view_navigation(
        back_action,
        next_action,
        back_label,
        next_label,
        configuration_state.is_valid(),
    );

    let content = column![
        header,
        scrollable(
            column![preset_section, configuration_form, save_preset_section]
                .spacing(15)
                .width(Length::Fill)
        )
        .height(Length::Fill),
        navigation,
    ]
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::main_box)
        .into()
}

/// Preset selection section
fn view_preset_section<'a, F>(
    configuration_presets: &'a [ConfigurationPreset],
    selected_preset: Option<usize>,
    preset_manager_action: Message,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    if !configuration_presets.is_empty() {
        let preset_list = pick_list(
            configuration_presets,
            selected_preset.and_then(|i| configuration_presets.get(i)),
            move |preset| {
                if let Some(index) = configuration_presets
                    .iter()
                    .position(|p| p.name == preset.name)
                {
                    message_factory(ConfigurationMessage::SelectPreset(index))
                } else {
                    Message::ShowError("Preset not found".to_string())
                }
            },
        )
        .placeholder("Select a configuration preset...")
        .width(Length::Fill)
        .style(style::pick_list_style);

        let preset_manager_button = button(
            row![icons::settings(), text("Manage Presets")]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(preset_manager_action)
        .padding(8)
        .style(button::secondary);

        container(
            column![
                text("Configuration Presets").size(18),
                row![preset_list, preset_manager_button]
                    .spacing(10)
                    .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(15)
        .style(style::bordered_box)
        .into()
    } else {
        container(
            column![
                text("No Presets Available").size(18),
                text("Configure settings below and save as a preset").size(14),
                button(
                    row![icons::settings(), text("Create First Preset")]
                        .spacing(5)
                        .align_y(Alignment::Center)
                )
                .on_press(preset_manager_action)
                .padding(8)
                .style(button::primary)
            ]
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(15)
        .style(style::bordered_box)
        .into()
    }
}

/// Save as preset section
fn view_save_preset_section<'a>(
    new_preset_name: &'a str,
    configuration_state: &'a ConfigurationState,
) -> Element<'a, Message> {
    if !new_preset_name.trim().is_empty() {
        container(
            column![
                text("Save Current Configuration").size(16),
                row![
                    text("Preset name: ").size(14),
                    text(new_preset_name).size(14),
                    button(
                        row![icons::save(), text("Save")]
                            .spacing(5)
                            .align_y(Alignment::Center)
                    )
                    .on_press(Message::SaveAsPreset(
                        configuration_state.to_preset(new_preset_name.to_string(), false)
                    ))
                    .padding(8)
                    .style(button::primary)
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(15)
        .style(style::bordered_box)
        .into()
    } else {
        container(column![])
            .into()
    }
}