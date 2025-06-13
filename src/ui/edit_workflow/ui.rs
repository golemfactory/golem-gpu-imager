use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Color, Element, Length};

use crate::ui::{
    device_selection::StorageDevice,
    icons,
    messages::Message,
};
use crate::models::{NetworkType, PaymentNetwork};
use super::EditMessage;

/// Select existing device for editing - pure edit workflow function
pub fn view_select_existing_device<'a>(
    storage_devices: &'a [StorageDevice],
    selected_device: Option<usize>,
) -> Element<'a, EditMessage> {
    let title = text("Select Device to Edit")
        .size(28)
        .width(Length::Fill);

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
                .padding(10)
                .style(button::primary)
            ]
            .spacing(15)
        )
        .padding(20)
        .style(crate::style::bordered_box)
        .into()
    } else {
        column(storage_devices.iter().enumerate().map(|(i, device)| {
            let device_info = column![
                text(&device.name).size(18),
                text(format!("Path: {}", device.path)).size(14),
                text(format!("Size: {}", device.size)).size(14),
            ]
            .spacing(5)
            .width(Length::Fill);

            let select_button = button(
                row![icons::edit(), text("Edit")]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .on_press(EditMessage::SelectExistingDevice(i))
            .padding(10);

            let is_selected = Some(i) == selected_device;

            container(
                row![device_info, select_button,]
                    .spacing(20)
                    .padding(10)
                    .width(Length::Fill)
                    .align_y(Alignment::Center),
            )
            .style(if is_selected {
                crate::style::selected_container
            } else {
                crate::style::bordered_box
            })
            .width(Length::Fill)
            .into()
        }))
        .spacing(10)
        .width(Length::Fill)
        .into()
    };

    let next_button = if selected_device.is_some() {
        button("Next: Edit Configuration")
            .on_press(EditMessage::GotoEditConfiguration)
            .padding(12)
            .style(button::primary)
    } else {
        button("Select a device to continue")
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
    .style(button::secondary);

    let buttons = row![back_button, next_button]
        .spacing(15)
        .width(Length::Fill)
        .align_y(Alignment::Center);

    column![title, device_list, buttons]
        .spacing(20)
        .padding(20)
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

    let edit_another_button = button("Edit Another Device")
        .on_press(EditMessage::EditAnother)
        .padding(12)
        .style(button::primary);

    let back_button = button("Back to Main Menu")
        .on_press(EditMessage::BackToMainMenu)
        .padding(12)
        .style(button::secondary);

    let buttons = row![edit_another_button, back_button]
        .spacing(15);

    container(
        column![
            row![icon, title].spacing(10).align_y(Alignment::Center),
            message,
            buttons
        ]
        .spacing(20)
        .align_x(Alignment::Center)
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Edit configuration view - delegates to shared configuration editor
pub fn view_edit_configuration<'a>(
    payment_network: PaymentNetwork,
    subnet: String,
    network_type: NetworkType,
    wallet_address: String,
    is_wallet_valid: bool,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
    show_preset_manager: bool,
    preset_editor: Option<&'a crate::ui::preset_manager::PresetEditor>,
) -> Element<'a, Message> {
    // Use the shared configuration editor
    crate::ui::view_configuration_editor(
        payment_network,
        subnet,
        network_type,
        wallet_address,
        is_wallet_valid,
        "Edit Configuration",
        "Edit the configuration settings for your device:",
        Message::Edit(EditMessage::SelectExistingDevice(0)), // Placeholder - should be proper back action
        Message::Edit(EditMessage::SaveConfiguration),
        "Back to Device Selection",
        "Save Changes",
        configuration_presets,
        selected_preset,
        new_preset_name,
        show_preset_manager,
        preset_editor,
        Message::PresetManager(crate::ui::preset_manager::PresetManagerMessage::ToggleManager),
    )
}