use iced::widget::{Column, Container, button, column, container, row, text};
use iced::{Alignment, Color, Element, Length};

use super::EditMessage;
use crate::models::{NetworkType, PaymentNetwork};
use crate::ui::{device_selection::StorageDevice, icons, messages::Message};

/// Select existing device for editing - pure edit workflow function
pub fn view_select_existing_device<'a>(
    storage_devices: &'a [StorageDevice],
    selected_device: Option<usize>,
) -> Element<'a, EditMessage> {
    let title = container(
        column![
            text("Select Device to Edit").size(28),
            text("Select an existing device to edit its configuration").size(16)
        ]
        .spacing(5),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::page_header);

    let device_list: Element<'a, EditMessage> = if storage_devices.is_empty() {
        container(
            column![
                text("No devices found").size(18),
                text("Please connect a device and try again").size(14),
                button(
                    row![icons::refresh(), text("Refresh")]
                        .spacing(5)
                        .align_y(Alignment::Center),
                )
                .on_press(EditMessage::RefreshDevices)
                .padding(8)
                .style(button::primary)
            ]
            .spacing(15),
        )
        .padding(20)
        .style(crate::style::bordered_box)
        .into()
    } else {
        column(storage_devices.iter().enumerate().map(|(i, device)| {
            let is_selected = Some(i) == selected_device;

            // Device type icon and info
            let device_header = row![
                device.type_icon().color(if is_selected {
                    crate::style::PRIMARY
                } else {
                    Color::from_rgb(0.6, 0.6, 0.6)
                }),
                column![
                    text(&device.name).size(18).color(if is_selected {
                        Color::from_rgb(0.1, 0.1, 0.1) // Dark text on light background
                    } else {
                        Color::from_rgb(0.9, 0.9, 0.9)
                    }),
                    text(device.type_name()).size(12).color(if is_selected {
                        crate::style::PRIMARY
                    } else {
                        Color::from_rgb(0.7, 0.7, 0.7)
                    }),
                ]
                .spacing(2)
            ]
            .spacing(15) // Increased spacing to accommodate larger icon
            .align_y(Alignment::Center);

            // Device details with better formatting
            let device_details = column![
                row![
                    text("Path:").size(14).color(if is_selected {
                        Color::from_rgb(0.3, 0.3, 0.3) // Darker gray for better contrast
                    } else {
                        Color::from_rgb(0.6, 0.6, 0.6)
                    }),
                    text(&device.path).size(14).color(if is_selected {
                        Color::from_rgb(0.1, 0.1, 0.1) // Dark text on light background
                    } else {
                        Color::from_rgb(0.8, 0.8, 0.8)
                    })
                ]
                .spacing(8),
                row![
                    text("Size:").size(14).color(if is_selected {
                        Color::from_rgb(0.3, 0.3, 0.3) // Darker gray for better contrast
                    } else {
                        Color::from_rgb(0.6, 0.6, 0.6)
                    }),
                    text(&device.size).size(14).color(if is_selected {
                        Color::from_rgb(0.1, 0.1, 0.1) // Dark text on light background
                    } else {
                        Color::from_rgb(0.8, 0.8, 0.8)
                    })
                ]
                .spacing(8),
            ]
            .spacing(4);

            let device_info = column![device_header, device_details]
                .spacing(8)
                .width(Length::Fill);

            let select_button = button(
                row![icons::edit(), text("Edit")]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .on_press(EditMessage::SelectExistingDevice(i))
            .padding(10)
            .style(if is_selected {
                button::success
            } else {
                button::primary
            });

            container(
                row![device_info, select_button]
                    .spacing(20)
                    .padding(15)
                    .width(Length::Fill)
                    .align_y(Alignment::Center),
            )
            .style(if is_selected {
                crate::style::selected_device_card_container
            } else {
                crate::style::device_card_container
            })
            .width(Length::Fill)
            .into()
        }))
        .spacing(10)
        .width(Length::Fill)
        .into()
    };

    let next_button = if selected_device.is_some() {
        button(
            row!["Edit Configuration", icons::navigate_next()]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(EditMessage::GotoEditConfiguration)
        .padding(12)
        .style(crate::style::navigation_action_button)
    } else {
        button(
            row!["Select a device to continue", icons::navigate_next()]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .padding(12)
        .style(button::secondary)
    };

    let back_button = button(
        row![icons::navigate_before(), "Back"]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(EditMessage::BackToMainMenu)
    .padding(12)
    .style(crate::style::navigation_back_button);

    // Add a spacer to push buttons to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    let buttons = container(
        row![back_button, next_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    let content = column![title, device_list, spacer, buttons]
        .spacing(20)
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .style(crate::style::main_box)
        .into()
}

/// Edit completion view - pure edit workflow function  
pub fn view_edit_completion(success: bool) -> Element<'static, EditMessage> {
    let title = if success {
        text("Configuration Saved Successfully").size(24)
    } else {
        text("Failed to Save Configuration").size(24)
    };

    let message = if success {
        text("Your device configuration has been updated.")
            .size(16)
            .color(Color::from_rgb(0.0, 0.7, 0.0))
    } else {
        text("There was an error saving the configuration.")
            .size(16)
            .color(Color::from_rgb(0.8, 0.0, 0.0))
    };

    let icon = if success {
        icons::check_circle()
    } else {
        icons::error()
    };

    let edit_another_button = button(
        row![icons::edit(), "Edit Another Device"]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(EditMessage::EditAnother)
    .padding(12)
    .style(button::primary);

    let back_button = button(
        row![icons::house(), "Back"]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(EditMessage::BackToMainMenu)
    .padding(12)
    .style(crate::style::navigation_back_button);

    let buttons = row![edit_another_button, back_button].spacing(15);

    container(
        column![
            row![icon, title].spacing(10).align_y(Alignment::Center),
            message,
            buttons
        ]
        .spacing(20)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Edit configuration view - now uses centralized configuration system
pub fn view_edit_configuration<'a>(
    configuration: &'a crate::ui::configuration::ConfigurationState,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
    new_preset_name: &'a str,
) -> Element<'a, Message> {
    // Use the shared configuration editor from the shared module
    crate::ui::configuration::view::view_configuration_editor(
        configuration,
        "Edit Configuration",
        "Edit the configuration settings for your device:",
        Message::Edit(EditMessage::BackToDeviceSelection),
        Some(Message::Edit(EditMessage::SaveConfiguration)),
        "Back to Devices",
        "Save Changes",
        configuration_presets,
        new_preset_name,
        Message::ManagePresets,
        |config_msg| Message::Configuration(config_msg),
    )
}

/// Loading configuration from device - shows progress indicator
pub fn view_loading_configuration<'a>() -> Element<'a, EditMessage> {
    let loading_content = container(
        column![
            icons::timer(),
            text("Loading Configuration...").size(20),
            text("Reading current settings from device").size(16),
        ]
        .spacing(20)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(50)
    .style(crate::style::bordered_box)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    loading_content.into()
}
