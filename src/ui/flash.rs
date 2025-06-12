use iced::alignment::Horizontal;
use iced::widget::{
    Column, Container, button, column, container, pick_list, progress_bar, row, scrollable, svg,
    text,
};
use iced::{Alignment, Color, Element, Length};
use iced::{Border, Theme};

use crate::models::{Message, NetworkType, OsImage, OsImageGroup, PaymentNetwork, StorageDevice};
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

        let mut image_info_items = vec![
            text(&image.name).size(20).into(),
            text(format!("Version: {}", image.version)).size(15).into(),
            text(&image.description).size(14).into(),
        ];

        // Add metadata information if available
        if let Some(metadata) = &image.metadata {
            let uncompressed_size_gb =
                metadata.uncompressed_size as f64 / (1024.0 * 1024.0 * 1024.0);
            image_info_items.push(
                row![
                    icons::analytics(),
                    text(format!("Uncompressed: {:.2} GB", uncompressed_size_gb))
                        .size(12)
                        .color(Color::from_rgb(0.0, 0.5, 0.8)),
                    icons::verified(),
                    text("Verified")
                        .size(12)
                        .color(Color::from_rgb(0.0, 0.6, 0.0))
                ]
                .spacing(5)
                .align_y(Alignment::Center)
                .into(),
            );
        }

        image_info_items.push(
            text(format!("Created: {}", image.created))
                .size(12)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
                .into(),
        );

        let image_info = column(image_info_items).spacing(8).width(Length::Fill);

        let action_button = if !image.downloaded {
            // State 1: Not downloaded
            button(
                row![icons::get_app(), text("Download")]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::DownloadOsImage(i))
            .padding(10)
            .style(button::secondary)
        } else if image.metadata.is_none() {
            // State 2: Downloaded but needs analysis
            button(
                row![icons::analytics(), text("Analyze")]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::AnalyzeOsImage(i))
            .padding(10)
            .style(button::secondary)
        } else {
            // State 3: Ready to select
            button(
                row![
                    if is_selected { icons::check_circle() } else { icons::check() },
                    text(if is_selected { "Selected" } else { "Select" })
                ]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::SelectOsImage(i))
            .padding(10)
            .style(if is_selected {
                |_theme: &Theme, _status| {
                    button::Style {
                        background: Some(crate::style::PRIMARY.into()),
                        text_color: Color::WHITE,
                        border: Border {
                            color: crate::style::PRIMARY,
                            width: 2.0,
                            radius: 5.0.into(),
                        },
                        shadow: iced::Shadow {
                            color: crate::style::PRIMARY.scale_alpha(0.3),
                            offset: iced::Vector::new(0.0, 2.0),
                            blur_radius: 4.0,
                        },
                        ..button::Style::default()
                    }
                }
            } else {
                button::primary
            })
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
            crate::style::selected_os_image_container
        } else {
            crate::style::bordered_box
        })
        .into()
    }))
    .spacing(15)
    .width(Length::Fill)
    .padding(iced::Padding::new(0.0).right(10.0)); // Add right padding to prevent scrollbar collision

    // Make scrollable in case we have many images
    let scrollable_content = scrollable(os_image_list).height(Length::Fill);

    // Navigation buttons
    let next_button = if selected_os_image.is_some() {
        button(
            container(row!["Select Target Device", icons::navigate_next()]).center_x(Length::Fill),
        )
        .on_press(Message::GotoSelectTargetDevice)
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

pub fn view_select_os_image_groups<'a>(
    os_image_groups: &'a [OsImageGroup],
    selected_os_image_group: Option<(usize, usize)>,
) -> Element<'a, Message> {
    // Page header
    let header = container(text("Select OS Image").size(28))
        .width(Length::Fill)
        .padding(15)
        .style(crate::style::bordered_box);

    // Create OS image group cards
    let os_image_list = column(
        os_image_groups
            .iter()
            .enumerate()
            .map(|(group_idx, group)| {
                let is_selected_group = selected_os_image_group.map(|(g, _)| g) == Some(group_idx);
                let selected_version_idx = if is_selected_group {
                    selected_os_image_group.map(|(_, v)| v)
                } else {
                    None
                };

                // Latest version card (always shown)
                let latest_is_selected = selected_version_idx == Some(0);
                let mut image_info_items = vec![
                    row![
                        text(&group.channel_name).size(20),
                        text("(Latest)")
                            .size(14)
                            .color(Color::from_rgb(0.0, 0.6, 0.0))
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
                    text(format!("Version: {}", group.latest_version.version))
                        .size(15)
                        .into(),
                    text(&group.description).size(14).into(),
                ];

                // Add metadata information if available
                if let Some(metadata) = &group.latest_version.metadata {
                    let uncompressed_size_gb =
                        metadata.uncompressed_size as f64 / (1024.0 * 1024.0 * 1024.0);
                    image_info_items.push(
                        row![
                            icons::analytics(),
                            text(format!("Uncompressed: {:.2} GB", uncompressed_size_gb))
                                .size(12)
                                .color(Color::from_rgb(0.0, 0.5, 0.8)),
                            icons::verified(),
                            text("Verified")
                                .size(12)
                                .color(Color::from_rgb(0.0, 0.6, 0.0))
                        ]
                        .spacing(5)
                        .align_y(Alignment::Center)
                        .into(),
                    );
                }

                image_info_items.push(
                    text(format!("Created: {}", group.latest_version.created))
                        .size(12)
                        .color(Color::from_rgb(0.5, 0.5, 0.5))
                        .into(),
                );

                let latest_image_info = column(image_info_items).spacing(8).width(Length::Fill);

                let latest_action_button = if !group.latest_version.downloaded {
                    // State 1: Not downloaded
                    button(
                        row![icons::get_app(), text("Download")]
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .on_press(Message::DownloadOsImageFromGroup(group_idx, 0))
                    .padding(10)
                    .style(button::secondary)
                } else if group.latest_version.metadata.is_none() {
                    // State 2: Downloaded but needs analysis
                    button(
                        row![icons::analytics(), text("Analyze")]
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .on_press(Message::AnalyzeOsImageFromGroup(group_idx, 0))
                    .padding(10)
                    .style(button::secondary)
                } else {
                    // State 3: Ready to select
                    button(
                        row![
                            if latest_is_selected { icons::check_circle() } else { icons::check() },
                            text(if latest_is_selected { "Selected" } else { "Select" })
                        ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .on_press(Message::SelectOsImageFromGroup(group_idx, 0))
                    .padding(10)
                    .style(if latest_is_selected {
                        |_theme: &Theme, _status| {
                            button::Style {
                                background: Some(crate::style::PRIMARY.into()),
                                text_color: Color::WHITE,
                                border: Border {
                                    color: crate::style::PRIMARY,
                                    width: 2.0,
                                    radius: 5.0.into(),
                                },
                                shadow: iced::Shadow {
                                    color: crate::style::PRIMARY.scale_alpha(0.3),
                                    offset: iced::Vector::new(0.0, 2.0),
                                    blur_radius: 4.0,
                                },
                                ..button::Style::default()
                            }
                        }
                    } else {
                        button::primary
                    })
                };

                // Create latest version container
                let latest_container = container(
                    row![latest_image_info, latest_action_button]
                        .spacing(15)
                        .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding(15)
                .style(if latest_is_selected {
                    crate::style::selected_os_image_container
                } else {
                    crate::style::bordered_box
                });

                // Version history expansion section
                let mut version_items = vec![latest_container.into()];

                if !group.older_versions.is_empty() {
                    // Expand/collapse toggle button
                    let (toggle_icon, toggle_text) = if group.expanded {
                        (
                            icons::expand_less(),
                            format!("Hide {} older versions", group.older_versions.len()),
                        )
                    } else {
                        (
                            icons::expand_more(),
                            format!("Show {} older versions", group.older_versions.len()),
                        )
                    };

                    let toggle_button = button(
                        row![toggle_icon, text(toggle_text).size(14)]
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .on_press(Message::ToggleVersionHistory(group_idx))
                    .padding(8)
                    .style(button::text);

                    let toggle_container = container(toggle_button)
                        .width(Length::Fill)
                        .padding([0, 15]);

                    version_items.push(toggle_container.into());

                    // Older versions (shown when expanded)
                    if group.expanded {
                        let older_versions_list =
                            column(group.older_versions.iter().enumerate().map(
                                |(version_idx, older_image)| {
                                    let actual_version_idx = version_idx + 1; // +1 because 0 is latest
                                    let is_selected =
                                        selected_version_idx == Some(actual_version_idx);

                                    let mut older_info_items = vec![
                                        text(format!("Version: {}", older_image.version))
                                            .size(15)
                                            .into(),
                                    ];

                                    // Add metadata information if available for older versions
                                    if let Some(metadata) = &older_image.metadata {
                                        let uncompressed_size_gb = metadata.uncompressed_size
                                            as f64
                                            / (1024.0 * 1024.0 * 1024.0);
                                        older_info_items.push(
                                            row![
                                                icons::analytics(),
                                                text(format!(
                                                    "Uncompressed: {:.2} GB",
                                                    uncompressed_size_gb
                                                ))
                                                .size(11)
                                                .color(Color::from_rgb(0.0, 0.5, 0.8)),
                                                icons::verified(),
                                                text("Verified")
                                                    .size(11)
                                                    .color(Color::from_rgb(0.0, 0.6, 0.0))
                                            ]
                                            .spacing(4)
                                            .align_y(Alignment::Center)
                                            .into(),
                                        );
                                    }

                                    older_info_items.push(
                                        text(format!("Created: {}", older_image.created))
                                            .size(12)
                                            .color(Color::from_rgb(0.5, 0.5, 0.5))
                                            .into(),
                                    );

                                    let older_image_info =
                                        column(older_info_items).spacing(5).width(Length::Fill);

                                    let older_action_button = if !older_image.downloaded {
                                        // State 1: Not downloaded
                                        button(
                                            row![icons::get_app(), text("Download")]
                                                .spacing(5)
                                                .align_y(Alignment::Center),
                                        )
                                        .on_press(Message::DownloadOsImageFromGroup(
                                            group_idx,
                                            actual_version_idx,
                                        ))
                                        .padding(8)
                                        .style(button::secondary)
                                    } else if older_image.metadata.is_none() {
                                        // State 2: Downloaded but needs analysis
                                        button(
                                            row![icons::analytics(), text("Analyze")]
                                                .spacing(5)
                                                .align_y(Alignment::Center),
                                        )
                                        .on_press(Message::AnalyzeOsImageFromGroup(
                                            group_idx,
                                            actual_version_idx,
                                        ))
                                        .padding(8)
                                        .style(button::secondary)
                                    } else {
                                        // State 3: Ready to select
                                        button(
                                            row![
                                                if is_selected { icons::check_circle() } else { icons::check() },
                                                text(if is_selected { "Selected" } else { "Select" })
                                            ]
                                                .spacing(5)
                                                .align_y(Alignment::Center),
                                        )
                                        .on_press(Message::SelectOsImageFromGroup(
                                            group_idx,
                                            actual_version_idx,
                                        ))
                                        .padding(8)
                                        .style(
                                            if is_selected {
                                                |_theme: &Theme, _status| {
                                                    button::Style {
                                                        background: Some(crate::style::PRIMARY.into()),
                                                        text_color: Color::WHITE,
                                                        border: Border {
                                                            color: crate::style::PRIMARY,
                                                            width: 2.0,
                                                            radius: 5.0.into(),
                                                        },
                                                        shadow: iced::Shadow {
                                                            color: crate::style::PRIMARY.scale_alpha(0.3),
                                                            offset: iced::Vector::new(0.0, 2.0),
                                                            blur_radius: 4.0,
                                                        },
                                                        ..button::Style::default()
                                                    }
                                                }
                                            } else {
                                                button::secondary
                                            },
                                        )
                                    };

                                    container(
                                        row![older_image_info, older_action_button]
                                            .spacing(15)
                                            .align_y(Alignment::Center),
                                    )
                                    .width(Length::Fill)
                                    .padding(10)
                                    .style(if is_selected {
                                        crate::style::selected_os_image_container
                                    } else {
                                        |theme: &Theme| {
                                            let palette = theme.extended_palette();
                                            container::Style {
                                                background: Some(
                                                    palette.background.weakest.color.into(),
                                                ),
                                                border: Border {
                                                    width: 1.0,
                                                    radius: 3.0.into(),
                                                    color: palette.background.strong.color,
                                                },
                                                ..container::Style::default()
                                            }
                                        }
                                    })
                                    .into()
                                },
                            ))
                            .spacing(5)
                            .width(Length::Fill);

                        let older_versions_container = container(older_versions_list)
                            .width(Length::Fill)
                            .padding(25); // Indent older versions

                        version_items.push(older_versions_container.into());
                    }
                }

                // Combine all version items into a group
                container(column(version_items).spacing(5))
                    .width(Length::Fill)
                    .padding(5)
                    .style(crate::style::bordered_box)
                    .into()
            }),
    )
    .spacing(15)
    .width(Length::Fill)
    .padding(iced::Padding::new(0.0).right(10.0)); // Add right padding to prevent scrollbar collision

    // Make scrollable in case we have many images
    let scrollable_content = scrollable(os_image_list).height(Length::Fill);

    // Navigation buttons
    let has_selection = selected_os_image_group.is_some();
    let next_button = if has_selection {
        button(
            container(row!["Select Target Device", icons::navigate_next()]).center_x(Length::Fill),
        )
        .on_press(Message::GotoSelectTargetDevice)
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

pub fn view_processing_image(
    version_id: &str,
    download_progress: f32,
    metadata_progress: f32,
    overall_progress: f32,
    channel: &str,
    created_date: &str,
    phase: &crate::utils::streaming_hash_calculator::ProcessingPhase,
    uncompressed_size: Option<u64>,
) -> Element<'static, Message> {
    use crate::utils::streaming_hash_calculator::ProcessingPhase;
    
    let title = match phase {
        ProcessingPhase::Download => text("Downloading and Verifying OS Image"),
        ProcessingPhase::Metadata => text("Analyzing Downloaded Image"),
        ProcessingPhase::Complete => text("Processing Complete"),
    }
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

    // Two-stage progress indicator
    let download_stage = if download_progress >= 1.0 {
        row![
            icons::check_circle().style(|_| iced::widget::text::Style {
                color: Some(crate::style::SUCCESS),
                ..iced::widget::text::Style::default()
            }),
            text("Download Complete").style(|_| iced::widget::text::Style {
                color: Some(crate::style::SUCCESS),
                ..iced::widget::text::Style::default()
            }),
        ]
    } else {
        row![
            icons::timer(),
            text("Downloading..."),
            text(format!("{}%", (download_progress * 100.0) as i32)).size(16),
        ]
    }
    .spacing(8)
    .align_y(Alignment::Center);

    let metadata_stage = match phase {
        ProcessingPhase::Download => {
            row![
                icons::timer().style(|_| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..iced::widget::text::Style::default()
                }),
                text("Waiting for download...").style(|_| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..iced::widget::text::Style::default()
                }),
            ]
        }
        ProcessingPhase::Metadata => {
            row![
                icons::timer(),
                text("Calculating SHA256 hash and size..."),
                text(format!("{}%", (metadata_progress * 100.0) as i32)).size(16),
            ]
        }
        ProcessingPhase::Complete => {
            row![
                icons::check_circle().style(|_| iced::widget::text::Style {
                    color: Some(crate::style::SUCCESS),
                    ..iced::widget::text::Style::default()
                }),
                text("Analysis Complete").style(|_| iced::widget::text::Style {
                    color: Some(crate::style::SUCCESS),
                    ..iced::widget::text::Style::default()
                }),
            ]
        }
    }
    .spacing(8)
    .align_y(Alignment::Center);

    let stages = column![download_stage, metadata_stage]
        .spacing(15)
        .width(Length::Fill)
        .align_x(Alignment::Center);

    // Progress indicator - use overall progress
    let progress_percentage = (overall_progress * 100.0) as i32;
    let progress_text = text(format!("{}%", progress_percentage)).size(25);
    let progress_bar = match phase {
        ProcessingPhase::Download => progress_bar(0.0..=1.0, overall_progress).style(progress_bar::secondary),
        ProcessingPhase::Metadata | ProcessingPhase::Complete => progress_bar(0.0..=1.0, overall_progress).style(progress_bar::primary),
    };

    // Size information if available
    let size_info = if let Some(size) = uncompressed_size {
        let size_gb = size as f64 / (1024.0 * 1024.0 * 1024.0);
        text(format!("Uncompressed size: {:.2} GB", size_gb))
            .size(16)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::from_rgb(0.0, 0.6, 0.0)),
                ..iced::widget::text::Style::default()
            })
    } else {
        match phase {
            ProcessingPhase::Download => text("Calculating size during processing...")
                .size(16)
                .style(|_| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..iced::widget::text::Style::default()
                }),
            _ => text("Calculating uncompressed size...")
                .size(16)
                .style(|_| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..iced::widget::text::Style::default()
                })
        }
    };

    // Status message
    let status_message = match phase {
        ProcessingPhase::Download => text("Download and verification in progress, please wait...").size(16),
        ProcessingPhase::Metadata => text("Analyzing image data, please wait...").size(16),
        ProcessingPhase::Complete => text("Processing completed successfully!").size(16),
    };

    // Optional cancel button
    let cancel_button = button(
        row![
            icons::cancel(), 
            text(match phase {
                ProcessingPhase::Download => "Cancel Download",
                ProcessingPhase::Metadata => "Cancel Analysis", 
                ProcessingPhase::Complete => "Cancel",
            })
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(Message::CancelWrite)
    .padding(10);

    let content = column![
        title,
        image_details,
        container(column![])
            .height(Length::Fill)
            .width(Length::Fill),
        stages,
        progress_text,
        progress_bar,
        size_info,
        status_message,
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

            let select_button = button(if is_selected {
                row![icons::check_circle(), text("Selected")]
                    .spacing(5)
                    .align_y(Alignment::Center)
            } else {
                row![icons::check(), text("Select")]
                    .spacing(5)
                    .align_y(Alignment::Center)
            })
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

pub fn view_writing_process(progress: f32, title: &'static str) -> Element<'static, Message> {
    // Page header with a more welcoming title with improved contrast
    let header =
        container(
            text(title)
                .size(28)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(iced::Color::WHITE), // Full white for maximum contrast
                    ..iced::widget::text::Style::default()
                }),
        )
        .width(Length::Fill)
        .padding(15)
        .style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(crate::style::PRIMARY.into()),
                border: iced::Border {
                    width: 1.0,
                    radius: 5.0.into(),
                    color: palette.primary.strong.color,
                },
                ..container::Style::default()
            }
        });

    // Create an icon to represent the writing process
    let writing_icon = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(80)
        .height(80);

    // Calculate more precise progress information
    let progress_percentage = (progress * 100.0) as i32;

    // Calculate approximate megabytes processed (assuming 16GB image size)
    // Adjust this value based on your actual image size
    const TOTAL_MB: u32 = 16 * 1024; // 16GB in MB

    // Progress now represents both read and written operations combined
    // so we calculate IO processed as a percentage of total work (read+write)
    let mb_processed = (progress * TOTAL_MB as f32) as u32;
    let mb_total = TOTAL_MB;

    // Create a nice styled progress bar with a pulse animation for low progress
    // This gives better feedback when progress seems stalled
    let progress_value = if progress < 0.02 {
        progress_bar(0.0..=1.0, progress).style(progress_bar::primary)
    } else {
        progress_bar(0.0..=1.0, progress).style(progress_bar::secondary)
    };

    // Display progress percentage with larger text and MB processed
    let progress_text = row![
        text(format!("{}%", progress_percentage)).size(28),
        text(format!("({} MB / {} MB)", mb_processed, mb_total)).size(16)
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Simple progress description
    let step_text = "Writing Image Data";
    let step_description = "Copying image data to device...";

    // Create a simple progress indicator
    let step_header = text(step_text).size(18).style(text::primary);
    let step_detail = text(step_description).size(14);

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
            text(format!(
                "Estimated time remaining: {} min {} sec",
                minutes, seconds
            ))
            .size(12)
        } else {
            text(format!(
                "Estimated time remaining: {} seconds",
                estimated_seconds_left
            ))
            .size(12)
        }
    } else if progress >= 0.98 {
        text("Finishing up, almost done...").size(12)
    } else {
        text("Calculating estimated time remaining...").size(12)
    };

    // Information container with improved visual hierarchy and spacing
    let info_container = container(
        row![
            writing_icon,
            column![
                text("Installing Golem GPU OS").size(20),
                row![progress_text],
                progress_value,
                row![step_header.width(Length::Fill), time_remaining],
                step_detail,
            ]
            .spacing(5)
            .width(Length::Fill)
        ]
        .spacing(15)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
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

    // Warning text about not disconnecting the device
    let warning_text = text("Please do not disconnect your device during the installation")
        .size(12)
        .style(|_theme: &Theme| text::Style {
            color: Some(crate::style::WARNING),
            ..text::Style::default()
        });

    // Cancel button with improved styling
    let cancel_button = button(
        row![icons::cancel(), text("Cancel Installation").size(14)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .on_press(Message::CancelWrite)
    .padding(8)
    .width(180)
    .style(button::danger);

    // Button container with warning
    let button_container = container(
        column![warning_text, cancel_button]
            .spacing(10)
            .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(10);

    // Main content with improved spacing
    let content = column![
        header,
        container(column![
            Container::new(Column::new()).height(15), // Top spacing
            info_container,
            spacer,
            button_container,
        ])
        .padding(15)
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::Fill);

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn view_flash_completion(success: bool, error_message: Option<&str>) -> Element<'_, Message> {
    // Page header with success/error status with improved styling
    let header_text = if success {
        "Installation Successful"
    } else {
        "Installation Failed"
    };
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

    let status_title = row![status_icon_styled, status_text,]
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

    // Error message container (only shown if error_message is Some and not success)
    let error_container = if !success {
        if let Some(error) = error_message {
            // Truncate very long error messages and format them better
            let formatted_error = if error.len() > 500 {
                // For very long errors, show first part and indicate truncation
                format!("{}... (error truncated, see logs for full details)", &error[..500])
            } else {
                error.to_string()
            };
            
            Some(container(
                scrollable(
                    column![
                        row![
                            icons::error().color(Color::from_rgb(0.8, 0.0, 0.0)),
                            text("Error Details:").size(16).color(Color::from_rgb(0.8, 0.0, 0.0))
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        
                        text(formatted_error)
                            .size(14)
                            .color(Color::from_rgb(0.7, 0.0, 0.0))
                    ]
                    .spacing(10)
                )
            )
            .width(Length::Fill)
            .height(Length::Fixed(120.0)) // Fixed height with scrolling
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
            }))
        } else {
            None
        }
    } else {
        None
    };

    // Add next steps for success case
    let next_steps_content = if success {
        column![
            text("Next Steps:").size(18).style(text::primary),
            row![
                icons::checkmark(),
                text("Insert the device into your target system")
            ]
            .spacing(5),
            row![
                icons::checkmark(),
                text("Boot your system from this device")
            ]
            .spacing(5),
            row![
                icons::checkmark(),
                text("The Golem GPU node will start automatically")
            ]
            .spacing(5),
        ]
        .spacing(10)
    } else {
        column![
            text("Troubleshooting Tips:").size(18).style(text::primary),
            row![
                icons::info(),
                text("Ensure the device is not write-protected")
            ]
            .spacing(5),
            row![
                icons::info(),
                text("Try using a different USB port or device")
            ]
            .spacing(5),
            row![icons::info(), text("Check if the device needs formatting")].spacing(5),
        ]
        .spacing(10)
    };

    // Wrap in container for styling
    let next_steps = container(next_steps_content)
        .padding(15)
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            border: Border {
                radius: 5.0.into(),
                width: 1.0,
                color: theme.extended_palette().background.strong.color,
            },
            ..container::Style::default()
        });

    // Information container with improved visual hierarchy
    let success_clone = success;
    let mut info_column = column![
        status_icon,
        status_title,
        column![].height(15), // Small spacer
        status_message,
    ];

    // Add error message if present
    if let Some(error_widget) = error_container {
        info_column = info_column.push(column![].height(15)); // Add spacer
        info_column = info_column.push(error_widget);
    }

    info_column = info_column.push(column![].height(25)); // Larger spacer
    info_column = info_column.push(next_steps);

    let info_container = container(
        info_column
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
        row![icons::install(), text("Flash Another Device").size(16)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .on_press(Message::FlashAnother)
    .padding(12)
    .width(220)
    .style(button::primary);

    let exit_button = button(
        row![icons::house(), text("Return to Home").size(16)]
            .spacing(8)
            .align_y(Alignment::Center),
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
