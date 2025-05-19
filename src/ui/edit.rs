use iced::alignment::Horizontal;
use iced::widget::{Column, Container, button, column, container, row, text};
use iced::{Alignment, Color, Element, Length};

use crate::models::{Message, NetworkType, PaymentNetwork, StorageDevice};
use crate::ui::icons;

pub fn view_select_existing_device<'a>(
    selected_device: Option<usize>,
    storage_devices: &'a [StorageDevice],
    error_message: Option<&'a str>,
) -> Element<'a, Message> {
    // Create a title with icon for device selection
    let title = container(
        row![icons::edit(), text("Select Existing Device").size(30)]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center);

    let device_list = column(storage_devices.iter().enumerate().map(|(i, device)| {
        let device_info = row![
            icons::storage().size(40).width(45),
            column![
                text(device.name.as_str().trim_start()).size(20),
                text(format!("Path: {}", device.path)).size(16),
                text(format!("Size: {}", device.size)).size(16),
            ]
        ]
        .spacing(5)
        .width(Length::Fill);

        let select_button = button("Select")
            .on_press(Message::SelectExistingDevice(i))
            .padding(10);

        let is_selected = Some(i) == selected_device;

        container(
            row![device_info, select_button,]
                .spacing(15)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(15)
        .style(if is_selected {
            container::success
        } else {
            crate::style::bordered_box
        })
        .into()
    }))
    .spacing(10)
    .width(Length::Fill);

    // Add a spacer to push buttons to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    let back_button = button(row![icons::navigate_before(), "Back to Main Menu"])
        .on_press(Message::BackToMainMenu)
        .padding(10);

    let refresh_button = button(row![icons::refresh(), "Refresh Devices"])
        .on_press(Message::BackToMainMenu)
        .style(button::secondary)
        .padding(10);

    let edit_config_button = if selected_device.is_some() {
        button(
            row![icons::edit(), "Edit Configuration"]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(Message::GotoEditConfiguration)
        .style(button::primary)
        .padding(10)
    } else {
        button(
            row![icons::edit(), "Edit Configuration"]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .style(button::primary)
        .padding(10)
    };

    // Error message container (only shown if error_message is Some)
    let error_container = if let Some(error) = error_message {
        container(
            row![
                icons::error().color(Color::from_rgb(0.8, 0.0, 0.0)),
                text(error).size(16).color(Color::from_rgb(0.8, 0.0, 0.0))
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(15)
        .style(|_theme| container::Style {
            text_color: Some(Color::from_rgb(0.8, 0.0, 0.0)),
            background: Some(Color::from_rgb(1.0, 0.9, 0.9).into()),
            border: iced::Border {
                radius: 5.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.8, 0.0, 0.0),
            },
            ..container::Style::default()
        })
    } else {
        // Empty container if no error
        container(text("")).height(0).width(0)
    };

    let content = column![
        title,
        error_container, // Add error message container
        device_list,
        spacer,
        row![back_button, refresh_button, edit_config_button].spacing(20)
    ]
    .spacing(20)
    .padding(20)
    .width(Length::Fill);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Shrink)
        .into()
}

pub fn view_edit_configuration<'a>(
    payment_network: PaymentNetwork,
    subnet: String,
    network_type: NetworkType,
    wallet_address: String,
    is_wallet_valid: bool,
    selected_device: Option<usize>,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
    show_preset_manager: bool,
) -> Element<'a, Message> {
    crate::ui::flash::view_configuration_editor(
        payment_network,
        subnet,
        network_type,
        wallet_address,
        is_wallet_valid,
        "Edit Configuration",
        "Edit the configuration settings for your device:",
        Message::SelectExistingDevice(selected_device.unwrap_or(0)),
        Message::SaveConfiguration,
        "Back to Device Selection",
        "Save Changes",
        configuration_presets,
        selected_preset,
        new_preset_name,
        show_preset_manager,
    )
}

pub fn view_edit_completion(success: bool) -> Element<'static, Message> {
    let title = if success {
        text("Success!")
            .size(30)
            .color(Color::from_rgb(0.0, 0.8, 0.0))
    } else {
        text("Error!")
            .size(30)
            .color(Color::from_rgb(0.8, 0.0, 0.0))
    };

    let message = if success {
        text("The configuration was successfully saved.")
    } else {
        text("There was an error saving the configuration.")
    };

    // Add a spacer to push the button to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    let back_button = button("Back to Main Menu")
        .on_press(Message::BackToMainMenu)
        .padding(10);

    let content = column![title, message, spacer, back_button,]
        .spacing(20)
        .padding(20)
        .width(Length::Fill)
        .align_x(Alignment::Center);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Shrink)
        .center_y(Length::Shrink)
        .into()
}
