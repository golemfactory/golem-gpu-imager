use iced::alignment::Horizontal;
use iced::widget::{Column, Container, button, column, container, progress_bar, row, scrollable, svg, text, pick_list};
use iced::{Alignment, Color, Element, Length};
use iced::{Border, Theme};

use crate::models::{Message, NetworkType, OsImage, PaymentNetwork, StorageDevice};
use crate::style;
use crate::ui::{LOGO_SVG, icons};

pub fn view_select_os_image<'a>(
    os_images: &'a [OsImage],
    selected_os_image: Option<usize>,
) -> Element<'a, Message> {
    // Page header
    let header = container(text("Select OS Image").size(28))
        .width(Length::Fill)
        .padding(15)
        .style(crate::style::bordered_box);

    // Create OS image cards
    let os_image_list = column(os_images.iter().enumerate().map(|(i, image)| {
        let is_selected = selected_os_image == Some(i);

        let image_info = column![
            text(&image.name).size(20),
            text(format!("Version: {}", image.version)).size(15),
            text(&image.description).size(14),
        ]
        .spacing(8)
        .width(Length::Fill);

        let action_button = if image.downloaded {
            button("Select")
                .on_press(Message::SelectOsImage(i))
                .padding(10)
                .style(if is_selected {
                    button::success
                } else {
                    button::primary
                })
        } else {
            button(text("Download"))
                .on_press(Message::DownloadOsImage(i))
                .padding(10)
                .style(button::secondary)
        };

        // Create a container for each OS image item
        container(
            row![image_info, action_button]
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
    .spacing(15)
    .width(Length::Fill);

    // Make scrollable in case we have many images
    let scrollable_content = scrollable(os_image_list).height(Length::Fill);

    // Navigation buttons
    let next_button = if selected_os_image.is_some() {
        button(container(row!["Configure Settings", icons::navigate_next()]).center_x(Length::Fill))
            .on_press(Message::GotoConfigureSettings)
            .padding(12)
            .width(220)
            .style(button::primary)
    } else {
        button("Next: Configure Settings")
            .padding(12)
            .width(220)
            .style(button::primary)
    };

    let back_button = button(iced::widget::row![icons::navigate_before(), "Back"])
        .on_press(Message::BackToMainMenu)
        .padding(12)
        .width(100)
        .style(button::secondary);

    let navigation = container(
        row![back_button, next_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    // Main content
    let content = column![header, scrollable_content, navigation,].width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::style::main_box)
        .into()
}

pub fn view_downloading_image(
    version_id: &str,
    progress: f32,
    channel: &str,
    created_date: &str,
) -> Element<'static, Message> {
    let title = text("Downloading OS Image")
        .size(30)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    // Image details at the top
    let image_details = column![
        text(format!("Channel: {}", channel)).size(18),
        text(format!("Version: {}", version_id)).size(18),
        text(format!("Created: {}", created_date)).size(16),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_x(Alignment::Center);

    // Progress indicator
    let progress_percentage = (progress * 100.0) as i32;
    let progress_text = text(format!("{}%", progress_percentage)).size(25);
    let progress_bar = progress_bar(0.0..=1.0, progress).style(progress_bar::secondary);

    // Optional cancel button
    let cancel_button = button("Cancel Download")
        .on_press(Message::CancelWrite)
        .padding(10);

    let content = column![
        title,
        image_details,
        container(column![])
            .height(Length::Fill)
            .width(Length::Fill),
        progress_text,
        progress_bar,
        text("Download in progress, please wait...").size(16),
        container(column![])
            .height(Length::Fill)
            .width(Length::Fill),
        cancel_button,
    ]
    .spacing(20)
    .padding(20)
    .width(Length::Fill)
    .align_x(Alignment::Center);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

/// Shared configuration UI component used in both flash and edit workflows
pub fn view_configuration_editor<'a>(
    payment_network: PaymentNetwork,
    subnet: String,
    network_type: NetworkType,
    wallet_address: String,
    is_wallet_valid: bool,
    title_text: &'a str,
    description_text: &'a str,
    back_action: Message,
    next_action: Message,
    back_label: &'a str,
    next_label: &'a str,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
    show_preset_manager: bool,
) -> Element<'a, Message> {
    // Create the title - simple text by default
    let title = text(title_text).size(30);

    // Create simplified settings UI
    let description = text(description_text).size(16);

    // Network selection
    let network_label = text("Payment Network").size(18);

    let network_pl = pick_list(
        &[PaymentNetwork::Testnet, PaymentNetwork::Mainnet][..],
        Some(payment_network),
        Message::SetPaymentNetwork
    )
    .style(crate::style::pick_list_style);

    

    // Subnet field with text input
    let subnet_label = text("Subnet").size(18);
    let subnet_input = iced::widget::text_input("Enter subnet", &subnet)
        .on_input(Message::SetSubnet)
        .padding(10);

    // Wallet address field with text input and validation indicator
    let wallet_label = text("Ethereum Wallet Address").size(18);

    // Choose style based on validation state and create input with appropriate styling
    let wallet_input = if !wallet_address.is_empty() {
        if is_wallet_valid {
            iced::widget::text_input("Enter ETH wallet address", &wallet_address)
                .on_input(Message::SetWalletAddress)
                .padding(10)
                .style(crate::style::valid_wallet_input)
        } else {
            iced::widget::text_input("Enter ETH wallet address", &wallet_address)
                .on_input(Message::SetWalletAddress)
                .padding(10)
                .style(crate::style::invalid_wallet_input)
        }
    } else {
        // Default styling for empty input
        iced::widget::text_input("Enter ETH wallet address", &wallet_address)
            .on_input(Message::SetWalletAddress)
            .padding(10)
            .style(crate::style::default_text_input)
    };

    // Add validation message if address is not empty
    let validation_message = if !wallet_address.is_empty() {
        if is_wallet_valid {
            container(row![icons::checkmark(), text("Valid Ethereum address").size(14)].spacing(5))
                .style(crate::style::valid_message_container)
        } else {
            container(
                row![
                    icons::error(),
                    text("Invalid Ethereum address format").size(14)
                ]
                .spacing(5),
            )
            .style(crate::style::invalid_message_container)
        }
    } else {
        container(row![text("").size(14)])
    };

    // Network type using pick list
    let type_label = text("Network Type").size(18);

    let type_pl = pick_list(
        &[NetworkType::Hybrid, NetworkType::Central][..],
        Some(network_type),
        Message::SetNetworkType
    )
    .style(crate::style::pick_list_style);

    // Navigation buttons
    let back_button = button(row![icons::navigate_before(), text(back_label)].spacing(5).align_y(Alignment::Center))
        .on_press(back_action)
        .padding(10)
        .style(button::secondary);

    // Use different icons based on the label
    let next_icon = if next_label.contains("Save") {
        icons::save()
    } else {
        icons::send()
    };

    let next_button = button(row![text(next_label), next_icon].spacing(5).align_y(Alignment::Center))
        .on_press(next_action)
        .padding(10)
        .style(button::primary);

    let navigation = row![back_button, next_button]
        .spacing(15)
        .width(Length::Fill)
        .align_y(Alignment::Center);

    // We'll create all UI components inline for better type inference

    // Main configuration content
    let config_content = column![
        network_label,
        network_pl.width(Length::Fill),
        subnet_label,
        subnet_input,
        wallet_label,
        wallet_input,
        validation_message,
        type_label,
        type_pl.width(Length::Fill),
    ]
    .spacing(10)
    .width(Length::Fill);

    // Either show the normal configuration UI or the preset manager UI
    if show_preset_manager {
        // Create a preset management UI that allows detailed management of presets
        let preset_list = if configuration_presets.is_empty() {
            container(text("No presets defined. Create your first preset.").size(16))
                .width(Length::Fill)
                .padding(20)
                .center_x(Length::Fill)
        } else {
            let mut preset_rows = column![];

            // Create an entry for each preset with full management options
            for (idx, preset) in configuration_presets.iter().enumerate() {
                let is_selected = selected_preset == Some(idx);
                let is_default = preset.is_default;

                // Create a row for each preset with name, actions, and info
                let preset_row = container(
                    row![
                        // Star icon for default preset
                        if is_default { icons::star() } else { icons::star_border() },

                        // Preset name
                        text(&preset.name).size(16).width(Length::Fill),

                        // Network info
                        column![
                            text(format!("Network: {:?}", preset.payment_network)).size(14),
                            text(format!("Type: {:?}", preset.network_type)).size(14),
                        ].width(Length::Fill),

                        // Action buttons
                        button(
                            row![icons::delete(), text("Delete")]
                                .spacing(5)
                                .align_y(Alignment::Center)
                        )
                        .on_press(Message::DeletePreset(idx))
                        .style(button::danger)
                        .padding(8),

                        button(
                            row![icons::star(), text("Set Default")]
                                .spacing(5)
                                .align_y(Alignment::Center)
                        )
                        .on_press(Message::SetDefaultPreset(idx))
                        .style(if is_default { button::success } else { button::secondary })
                        .padding(8),

                        button(
                            row![icons::tune(), text("Load")]
                                .spacing(5)
                                .align_y(Alignment::Center)
                        )
                        .on_press(Message::SelectPreset(idx))
                        .style(if is_selected { button::primary } else { button::secondary })
                        .padding(8)
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                )
                .padding(10)
                .style(if is_selected {
                    crate::style::selected_container
                } else {
                    crate::style::bordered_box
                })
                .width(Length::Fill);

                preset_rows = column![preset_rows, preset_row];
            }

            container(
                column![
                    row![
                        icons::settings(),
                        text("Manage Configuration Presets").size(20)
                    ].spacing(5).align_y(Alignment::Center),
                    container(preset_rows)
                        .padding(10)
                        .style(crate::style::bordered_box)
                ]
                .spacing(10)
            )
            .width(Length::Fill)
        };

        // Add new preset form
        let new_preset_form = container(
            column![
                text("Create New Preset").size(18),
                row![
                    iced::widget::text_input("Enter preset name", new_preset_name)
                        .on_input(Message::SetPresetName)
                        .padding(10)
                        .width(Length::Fill),
                    button(
                        row![icons::save(), text("Save")]
                            .spacing(5)
                            .align_y(Alignment::Center)
                    )
                    .on_press(Message::SaveAsPreset)
                    .style(button::primary)
                    .padding(10),
                ]
                .spacing(10)
            ]
            .spacing(10)
        )
        .padding(15)
        .style(crate::style::bordered_box)
        .width(Length::Fill);

        // Return button
        let back_to_config = button(
            row![
                icons::navigate_before(),
                text("Back to Configuration")
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .on_press(Message::TogglePresetManager)
        .padding(10)
        .style(button::secondary);

        // Layout for preset manager view
        column![
            title,
            text("Manage your saved configuration presets").size(16),
            preset_list,
            new_preset_form,
            back_to_config
        ]
        .spacing(20)
        .padding(20)
        .into()
    } else {
        // Main layout with clean vertical organization (normal view)
        column![
            title,
            description,

            // Preset picker at the top using pick_list
            container(
                row![
                    text("Preset:").size(16),

                    if !configuration_presets.is_empty() {
                        // Get the currently selected preset, if any
                        let selected = selected_preset.and_then(|idx| {
                            if idx < configuration_presets.len() {
                                Some(&configuration_presets[idx])
                            } else {
                                None
                            }
                        });

                        // Create a pick_list for selecting presets
                        let preset_picker = pick_list(
                            configuration_presets,
                            selected,
                            |preset| {
                                // Find the index of the selected preset
                                let idx = configuration_presets.iter()
                                    .position(|p| p.name == preset.name)
                                    .unwrap_or(0);

                                Message::SelectPreset(idx)
                            }
                        )
                        .width(Length::Fill)
                        .style(crate::style::pick_list_style);

                        let row_with_icon = row![
                            icons::tune(),
                            preset_picker
                        ].spacing(5).align_y(Alignment::Center).width(Length::Fill);

                        let preset_container: Element<'_, Message> = container(row_with_icon)
                            .width(Length::Fill)
                            .into();

                        preset_container
                    } else {
                        // Show a disabled text input when no presets are available
                        container(
                            text("No presets available").size(16)
                        )
                        .width(Length::Fill)
                        .padding(8)
                        .style(crate::style::bordered_box)
                        .into()
                    },

                    button(
                        row![
                            icons::settings(),
                            text("Manage").size(14)
                        ].spacing(5).align_y(Alignment::Center)
                    )
                    .on_press(Message::TogglePresetManager)
                    .padding(8)
                    .style(button::secondary)
                ]
                .spacing(10)
                .align_y(Alignment::Center)
            )
            .padding(10)
            .style(crate::style::bordered_box)
            .width(Length::Fill),

            // Main configuration content
            config_content,

            // Save as preset UI
            container(
                row![
                    icons::save(),
                    text("Save Current Settings as Preset").size(16),
                    iced::widget::text_input("Enter preset name", new_preset_name)
                        .on_input(Message::SetPresetName)
                        .padding(8)
                        .width(Length::Fill),
                    button("Save")
                        .on_press(Message::SaveAsPreset)
                        .style(button::primary)
                        .padding(8)
                ]
                .spacing(10)
                .align_y(Alignment::Center)
            )
            .padding(10)
            .style(crate::style::bordered_box)
            .width(Length::Fill),

            // Navigation buttons
            navigation
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
}

pub fn view_configure_settings<'a>(
    payment_network: PaymentNetwork,
    subnet: String,
    network_type: NetworkType,
    wallet_address: String,
    is_wallet_valid: bool,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
    show_preset_manager: bool,
) -> Element<'a, Message> {
    view_configuration_editor(
        payment_network,
        subnet,
        network_type,
        wallet_address,
        is_wallet_valid,
        "Configure OS Image",
        "Configure the OS image with the following options:",
        Message::BackToSelectOsImage,
        Message::WriteImage,
        "Back",
        "Start Writing",
        configuration_presets,
        selected_preset,
        new_preset_name,
        show_preset_manager,
    )
}

pub fn view_select_target_device<'a>(storage_devices: &'a [StorageDevice]) -> Element<'a, Message> {
    let title = text("Select Target Device")
        .size(30)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let warning = text("Warning: All data on the selected device will be erased!")
        .size(16)
        .color(Color::from_rgb(1.0, 0.0, 0.0));

    let device_list = column(storage_devices.iter().enumerate().map(|(i, device)| {
        let device_info = column![
            text(&device.name).size(20),
            text(format!("Path: {}", device.path)).size(16),
            text(format!("Size: {}", device.size)).size(16),
        ]
        .spacing(5)
        .width(Length::Fill);

        let select_button = button("Select")
            .on_press(Message::SelectTargetDevice(i))
            .padding(10);

        row![device_info, select_button,]
            .spacing(20)
            .padding(10)
            .width(Length::Fill)
            .into()
    }))
    .spacing(10)
    .width(Length::Fill);

    // Add a spacer to push buttons to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    let back_button = button(row![icons::navigate_before(), "Back to Configure Settings"].spacing(5).align_y(Alignment::Center))
        .on_press(Message::GotoConfigureSettings)
        .padding(10)
        .style(button::secondary);

    let write_button = button(row![text("Write Image"), icons::send()].spacing(5).align_y(Alignment::Center))
        .on_press(Message::WriteImage)
        .padding(10)
        .style(button::primary);

    let buttons = row![back_button, write_button,]
        .spacing(10)
        .width(Length::Fill)
        .align_y(Alignment::Center);

    let content = column![title, warning, device_list, spacer, buttons]
        .spacing(20)
        .padding(20)
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

pub fn view_writing_process(progress: f32) -> Element<'static, Message> {
    // Page header
    let header = container(text("Writing Image").size(28).style(text::primary))
        .width(Length::Fill)
        .padding(15)
        .style(container::secondary);

    // Create an icon to represent the writing process
    let writing_icon = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(100)
        .height(100);

    // Create a nice styled progress bar
    let progress_value = progress_bar(0.0..=1.0, progress).style(progress_bar::secondary);

    // Display progress percentage
    let progress_percentage = (progress * 100.0) as i32;
    let progress_text = text(format!("{}%", progress_percentage)).size(25);

    // Description text
    let step_text = text(match progress_percentage {
        0..=33 => "Preparing disk...",
        34..=66 => "Writing image data...",
        _ => "Verifying written data...",
    })
    .size(16);

    // Information container
    let info_container = container(
        column![
            writing_icon,
            text("Writing Golem GPU OS to device").size(20),
            row![progress_text].padding(10),
            step_text,
            column![].height(20), // Small spacer
            progress_value,
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(20)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();

        container::Style::default()
            .background(palette.background.weak.color)
            .border(Border {
                radius: 8.0.into(),
                width: 1.0,
                color: palette.primary.base.color,
            })
    });

    // Add a spacer to push the button to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    // Cancel button
    let cancel_button = button(text("Cancel").align_x(Horizontal::Center).size(16))
        .on_press(Message::CancelWrite)
        .padding(12)
        .width(120)
        .style(button::danger);

    // Button container
    let button_container = container(cancel_button)
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .padding(15);

    // Main content
    let content = column![
        header,
        container(column![
            Container::new(Column::new()).height(40), // Top spacing
            info_container,
            spacer,
            button_container,
        ])
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::Fill);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn view_flash_completion(success: bool) -> Element<'static, Message> {
    // Page header with success/error status
    let header = container(text(if success { "Success" } else { "Error" }).size(28))
        .width(Length::Fill)
        .padding(15)
        .style(if success {
            container::success
        } else {
            container::danger
        });

    // Create an icon to represent the status
    let status_icon = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(100)
        .height(100);

    // Status title
    let status_title = text(if success {
        "Operation Completed Successfully!"
    } else {
        "Operation Failed"
    })
    .size(26)
    .style(if success { text::success } else { text::danger });

    // Status message
    let status_message = text(
        if success {
            "The Golem GPU OS image was successfully written to the device.\nYour device is now ready to use."
        } else {
            "There was an error writing the image to the device.\nPlease check your device and try again."
        }
    )
    .size(16);

    // Information container
    let success_clone = success;
    let info_container = container(
        column![
            status_icon,
            status_title,
            column![].height(15), // Small spacer
            status_message,
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(30)
    .style(move |theme: &Theme| {
        container::secondary(theme).border(Border {
            radius: 8.0.into(),
            width: 2.0,
            color: if success_clone {
                style::SUCCESS.scale_alpha(0.5)
            } else {
                style::ERROR.scale_alpha(0.5)
            },
        })
    });

    // Add a spacer to push the buttons to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    // Create styled buttons
    let flash_another_button = button(
        text("Flash Another Device")
            .align_x(Horizontal::Center)
            .size(16),
    )
    .on_press(Message::FlashAnother)
    .padding(12)
    .width(200)
    .style(button::primary);

    let exit_button = button(text("Exit").align_x(Horizontal::Center).size(16))
        .on_press(Message::Exit)
        .padding(12)
        .width(100)
        .style(button::secondary);

    // Button container
    let buttons_container = container(
        row![flash_another_button, exit_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
    .style(container::dark);

    // Main content
    let content = column![
        header,
        container(column![
            Container::new(Column::new()).height(40), // Top spacing
            info_container,
            spacer,
        ])
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill),
        buttons_container,
    ]
    .width(Length::Fill);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
