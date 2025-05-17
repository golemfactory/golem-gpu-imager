use iced::alignment::Horizontal;
use iced::widget::{
    Column, Container, button, column, container, pick_list, progress_bar, row, scrollable, svg,
    text,
};
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
        button(
            container(row!["Select Target Device", icons::navigate_next()]).center_x(Length::Fill),
        )
        .on_press(Message::DownloadCompleted(
            selected_os_image
                .map(|i| os_images[i].version.clone())
                .unwrap_or_default(),
        ))
        .padding(12)
        .width(220)
        .style(button::primary)
    } else {
        button("Next: Select Target Device")
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
        Message::SetPaymentNetwork,
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
        Message::SetNetworkType,
    )
    .style(crate::style::pick_list_style);

    // Navigation buttons
    let back_button = button(
        row![icons::navigate_before(), text(back_label)]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(back_action)
    .padding(10)
    .style(button::secondary);

    // Use different icons based on the label
    let next_icon = if next_label.contains("Save") {
        icons::save()
    } else {
        icons::send()
    };

    let next_button = button(
        row![text(next_label), next_icon]
            .spacing(5)
            .align_y(Alignment::Center),
    )
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
                        if is_default {
                            icons::star()
                        } else {
                            icons::star_border()
                        },
                        // Preset name
                        text(&preset.name).size(16).width(Length::Fill),
                        // Network info
                        column![
                            text(format!("Network: {:?}", preset.payment_network)).size(14),
                            text(format!("Type: {:?}", preset.network_type)).size(14),
                        ]
                        .width(Length::Fill),
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
                        .style(if is_default {
                            button::success
                        } else {
                            button::secondary
                        })
                        .padding(8),
                        button(
                            row![icons::tune(), text("Load")]
                                .spacing(5)
                                .align_y(Alignment::Center)
                        )
                        .on_press(Message::SelectPreset(idx))
                        .style(if is_selected {
                            button::primary
                        } else {
                            button::secondary
                        })
                        .padding(8)
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
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
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                    container(preset_rows)
                        .padding(10)
                        .style(crate::style::bordered_box)
                ]
                .spacing(10),
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
            .spacing(10),
        )
        .padding(15)
        .style(crate::style::bordered_box)
        .width(Length::Fill);

        // Return button
        let back_to_config = button(
            row![icons::navigate_before(), text("Back to Configuration")]
                .spacing(8)
                .align_y(Alignment::Center),
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
                        let preset_picker = pick_list(configuration_presets, selected, |preset| {
                            // Find the index of the selected preset
                            let idx = configuration_presets
                                .iter()
                                .position(|p| p.name == preset.name)
                                .unwrap_or(0);

                            Message::SelectPreset(idx)
                        })
                        .width(Length::Fill)
                        .style(crate::style::pick_list_style);

                        let row_with_icon = row![icons::tune(), preset_picker]
                            .spacing(5)
                            .align_y(Alignment::Center)
                            .width(Length::Fill);

                        let preset_container: Element<'_, Message> =
                            container(row_with_icon).width(Length::Fill).into();

                        preset_container
                    } else {
                        // Show a disabled text input when no presets are available
                        container(text("No presets available").size(16))
                            .width(Length::Fill)
                            .padding(8)
                            .style(crate::style::bordered_box)
                            .into()
                    },
                    button(
                        row![icons::settings(), text("Manage").size(14)]
                            .spacing(5)
                            .align_y(Alignment::Center)
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
        "Write Image to Device",
        configuration_presets,
        selected_preset,
        new_preset_name,
        show_preset_manager,
    )
}

pub fn view_select_target_device<'a>(
    storage_devices: &'a [StorageDevice],
    selected_device: Option<usize>,
) -> Element<'a, Message> {
    let title = text("Select Target Device")
        .size(30)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let warning = text("Warning: All data on the selected device will be erased!")
        .size(16)
        .color(Color::from_rgb(1.0, 0.0, 0.0));

    // Device list or message if no devices found
    let device_list: Element<'a, Message> = if storage_devices.is_empty() {
        // Show message when no devices are available
        container(
            column![
                text("No storage devices found").size(20),
                text("Please connect a USB drive or SD card and try again").size(16),
                button(
                    row![icons::refresh(), text("Refresh Devices").size(16)]
                        .spacing(8)
                        .align_y(Alignment::Center)
                )
                .on_press(Message::RefreshRepoData) // Reuse this message to trigger a refresh
                .padding(12)
                .style(button::primary)
            ]
            .spacing(20)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(30)
        .style(crate::style::bordered_box)
        .into()
    } else {
        // Show actual device list
        column(storage_devices.iter().enumerate().map(|(i, device)| {
            let is_selected = selected_device == Some(i);

            let device_info = column![
                text(&device.name).size(20),
                text(format!("Path: {}", device.path)).size(16),
                text(format!("Size: {}", device.size)).size(16),
            ]
            .spacing(5)
            .width(Length::Fill);

            let select_button = button(if is_selected { "Selected" } else { "Select" })
                .on_press(Message::SelectTargetDevice(i))
                .padding(10)
                .style(if is_selected {
                    button::success
                } else {
                    button::secondary
                });

            container(
                row![device_info, select_button,]
                    .spacing(20)
                    .padding(10)
                    .width(Length::Fill)
                    .align_y(Alignment::Center),
            )
            .style(if is_selected {
                container::success
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

    // Add a spacer to push buttons to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    let back_button = button(
        row![icons::navigate_before(), "Back to Configure Settings"]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(Message::GotoConfigureSettings)
    .padding(10)
    .style(button::secondary);

    // Only enable the next button if a device is selected
    let next_button = if selected_device.is_some() {
        button(
            row![text("Next: Configure Settings"), icons::navigate_next()]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(Message::GotoConfigureSettings)
        .padding(10)
        .style(button::primary)
    } else {
        button(
            row![text("Select a device to continue"), icons::navigate_next()]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .padding(10)
        // Use a custom style for disabled buttons
        .style(|theme, _| {
            let palette = theme.extended_palette();

            button::Style {
                background: Some(palette.background.weak.color.into()),
                text_color: palette.background.strong.text,
                border: iced::Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                shadow: iced::Shadow::default(),
                ..button::Style::default()
            }
        })
    };

    let buttons = row![back_button, next_button,]
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
    // Page header with a more welcoming title
    let header = container(text("Writing Image to Device").size(30).style(text::primary))
        .width(Length::Fill)
        .padding(20)
        .style(container::secondary);

    // Create an icon to represent the writing process
    let writing_icon = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(120)
        .height(120);

    // Calculate more precise progress information
    let progress_percentage = (progress * 100.0) as i32;
    
    // Create a nice styled progress bar with a pulse animation for low progress
    // This gives better feedback when progress seems stalled
    let progress_value = if progress < 0.02 {
        progress_bar(0.0..=1.0, progress)
            .style(progress_bar::primary)
    } else {
        progress_bar(0.0..=1.0, progress)
            .style(progress_bar::secondary)
    };

    // Display progress percentage with larger text
    let progress_text = text(format!("{}%", progress_percentage)).size(36);

    // Enhanced description text - show different steps based on progress with more detail
    let (step_text, step_description) = match progress_percentage {
        0..=5 => (
            "Initializing Write Process",
            "Preparing disk and validating image data..."
        ),
        6..=15 => (
            "Preparing Disk",
            "Creating partition table and file system structure..."
        ),
        16..=30 => (
            "Writing Boot Sectors",
            "Installing bootloader and system configuration..."
        ),
        31..=75 => (
            "Writing OS Image",
            "Transferring main system files to device..."
        ),
        76..=90 => (
            "Writing Configuration",
            "Applying your custom settings to the device..."
        ),
        91..=99 => (
            "Finalizing",
            "Verifying data integrity and completing installation..."
        ),
        _ => (
            "Completing Installation",
            "Almost done! Finishing up the final steps..."
        ),
    };

    // Create a progress step indicator with more visual impact
    let step_header = text(step_text).size(20).style(text::primary);
    let step_detail = text(step_description).size(16);

    // Estimated time remaining calculation (simple approximation)
    // Assume a complete write takes about 10 minutes (600 seconds)
    let estimated_seconds_left = if progress > 0.05 {
        ((1.0 - progress) * 600.0) as i32
    } else {
        // Don't show estimate when just starting
        0
    };
    
    let time_remaining = if progress > 0.05 && progress < 0.98 {
        if estimated_seconds_left > 60 {
            let minutes = estimated_seconds_left / 60;
            let seconds = estimated_seconds_left % 60;
            text(format!("Estimated time remaining: {} min {} sec", minutes, seconds))
                .size(14)
        } else {
            text(format!("Estimated time remaining: {} seconds", estimated_seconds_left))
                .size(14)
        }
    } else if progress >= 0.98 {
        text("Finishing up, almost done...")
            .size(14)
    } else {
        text("Calculating estimated time remaining...")
            .size(14)
    };

    // Information container with improved visual hierarchy and spacing
    let info_container = container(
        column![
            writing_icon,
            text("Installing Golem GPU OS").size(24),
            column![].height(10), // Small spacer
            row![progress_text].padding(10),
            step_header,
            step_detail,
            column![].height(15), // Spacer
            progress_value,
            column![].height(10), // Small spacer
            time_remaining,
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(30)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();

        container::Style::default()
            .background(palette.background.weak.color)
            .border(Border {
                radius: 12.0.into(),
                width: 2.0,
                color: palette.primary.base.color,
            })
    });

    // Add a spacer to push the button to the bottom
    let spacer = Container::new(Column::new())
        .height(Length::Fill)
        .width(Length::Fill);

    // Warning text about not disconnecting the device
    let warning_text = text("Please do not disconnect your device during the installation")
        .size(14)
        .style(|theme: &Theme| text::Style {
            color: Some(crate::style::WARNING),
            ..text::Style::default()
        });

    // Cancel button with improved styling
    let cancel_button = button(
            row![
                icons::cancel(),
                text("Cancel Installation").size(16)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .on_press(Message::CancelWrite)
        .padding(12)
        .width(200)
        .style(button::danger);

    // Button container with warning
    let button_container = container(
        column![
            warning_text,
            cancel_button
        ]
        .spacing(15)
        .align_x(Alignment::Center)
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(15);

    // Main content with improved spacing
    let content = column![
        header,
        container(column![
            Container::new(Column::new()).height(30), // Top spacing
            info_container,
            spacer,
            button_container,
        ])
        .padding(25)
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
    // Page header with success/error status with improved styling
    let header_text = if success { "Installation Successful" } else { "Installation Failed" };
    let header = container(text(header_text).size(32))
        .width(Length::Fill)
        .padding(20)
        .style(if success {
            container::success
        } else {
            container::danger
        });

    // Create a more appropriate icon based on success/failure
    let status_icon = if success {
        svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
            .width(120)
            .height(120)
    } else {
        svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
            .width(120)
            .height(120)
    };

    // Status title with icon
    let status_icon_styled = if success { 
        icons::check_circle().style(text::success) 
    } else { 
        icons::error().style(text::danger) 
    };
    
    let status_text = text(if success {
        "Operation Completed Successfully!"
    } else {
        "Operation Failed"
    })
    .size(26)
    .style(if success { text::success } else { text::danger });
    
    let status_title = row![
        status_icon_styled,
        status_text,
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Status message with more detailed information
    let status_message = text(if success {
        "The Golem GPU OS image was successfully written to the device.\n\
        Your device is now configured and ready to use with Golem Network.\n\
        You can safely remove the device and boot your system with it."
    } else {
        "There was an error writing the image to the device.\n\
        This could be due to a write-protected device, insufficient permissions,\n\
        or hardware issues. Please check your device and try again."
    })
    .size(16);

    // Add next steps for success case
    let next_steps_content = if success {
        column![
            text("Next Steps:").size(18).style(text::primary),
            row![icons::checkmark(), text("Insert the device into your target system")].spacing(5),
            row![icons::checkmark(), text("Boot your system from this device")].spacing(5),
            row![icons::checkmark(), text("The Golem GPU node will start automatically")].spacing(5),
        ]
        .spacing(10)
    } else {
        column![
            text("Troubleshooting Tips:").size(18).style(text::primary),
            row![icons::info(), text("Ensure the device is not write-protected")].spacing(5),
            row![icons::info(), text("Try using a different USB port or device")].spacing(5),
            row![icons::info(), text("Check if the device needs formatting")].spacing(5),
        ]
        .spacing(10)
    };
    
    // Wrap in container for styling
    let next_steps = container(next_steps_content)
        .padding(15)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                border: Border {
                    radius: 5.0.into(),
                    width: 1.0,
                    color: theme.extended_palette().background.strong.color,
                },
                ..container::Style::default()
            }
        });

    // Information container with improved visual hierarchy
    let success_clone = success;
    let info_container = container(
        column![
            status_icon,
            status_title,
            column![].height(15), // Small spacer
            status_message,
            column![].height(25), // Larger spacer
            next_steps,
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(30)
    .style(move |theme: &Theme| {
        container::secondary(theme).border(Border {
            radius: 12.0.into(),
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

    // Create styled buttons with icons for better UX
    let flash_another_button = button(
        row![
            icons::install(), 
            text("Flash Another Device").size(16)
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    )
    .on_press(Message::FlashAnother)
    .padding(12)
    .width(220)
    .style(button::primary);

    let exit_button = button(
        row![
            icons::house(),
            text("Return to Home").size(16)
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    )
    .on_press(Message::Exit)
    .padding(12)
    .width(180)
    .style(button::secondary);

    // Button container with improved styling
    let buttons_container = container(
        row![flash_another_button, exit_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(20)
    .style(container::dark);

    // Main content with improved spacing and layout
    let content = column![
        header,
        container(column![
            Container::new(Column::new()).height(30), // Top spacing
            info_container,
            spacer,
        ])
        .padding(25)
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
