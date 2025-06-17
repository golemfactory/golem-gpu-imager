use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Border, Color, Element, Length};
use crate::models::{ConfigurationPreset, NetworkType, PaymentNetwork};
use crate::ui::icons;
use crate::style;
use super::{PresetEditor, PresetManagerMessage};

/// Main preset manager view
pub fn view_preset_manager<'a>(
    presets: &'a [ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
    editor: Option<&'a PresetEditor>,
) -> Element<'a, PresetManagerMessage> {
    let title = text("Manage Configuration Presets")
        .size(28)
        .width(Length::Fill);

    let content = if let Some(preset_editor) = editor {
        // Show preset editor
        view_preset_editor(preset_editor)
    } else {
        // Show preset list
        view_preset_list(presets, selected_preset, new_preset_name)
    };

    let back_button = button(
        row![icons::navigate_before(), "Back to Main Menu"]
            .spacing(5)
            .align_y(Alignment::Center)
    )
    .on_press(PresetManagerMessage::BackToMainMenu)
    .padding(8)
    .style(button::secondary);

    column![
        title,
        content,
        container(back_button).width(Length::Fill).padding([15, 0])
    ]
    .spacing(20)
    .padding(20)
    .into()
}

/// Preset list view with grid layout
fn view_preset_list<'a>(
    presets: &'a [ConfigurationPreset],
    selected_preset: Option<usize>,
    new_preset_name: &'a str,
) -> Element<'a, PresetManagerMessage> {
    // Simple header with title and count
    let header = container(
        row![
            text("Configuration Presets").size(24),
            container(
                text(format!("{} presets", presets.len()))
                    .size(14)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            )
            .width(Length::Fill)
            .align_x(Alignment::End)
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill)
    )
    .padding(10)
    .width(Length::Fill);
    
    // Simple create section
    let quick_create = container(
        row![
            text("Create New Preset").size(16),
            container(
                text_input("New preset name...", new_preset_name)
                    .on_input(PresetManagerMessage::SetNewPresetName)
                    .padding(8)
                    .width(Length::Fill)
            )
            .width(Length::Fill),
            button("Create")
                .on_press(PresetManagerMessage::CreatePreset)
                .padding(8)
                .style(button::primary)
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    )
    .style(style::bordered_box)
    .padding(15)
    .width(Length::Fill);

    let presets_section: Element<'a, PresetManagerMessage> = if presets.is_empty() {
        container(
            column![
                icons::star_border().size(32).color(Color::from_rgb(0.6, 0.6, 0.6)),
                text("No presets found").size(16),
                text("Create your first preset above").size(12)
                    .color(Color::from_rgb(0.7, 0.7, 0.7))
            ]
            .spacing(10)
            .align_x(Alignment::Center)
        )
        .padding(30)
        .width(Length::Fill)
        .into()
    } else {
        // Grid layout for preset cards
        let all_presets: Vec<(usize, &ConfigurationPreset)> = presets
            .iter()
            .enumerate()
            .collect();
        let preset_grid = create_preset_grid(all_presets, selected_preset);
        
        container(preset_grid)
            .padding(5)
            .width(Length::Fill)
            .into()
    };

    scrollable(
        column![header, quick_create, presets_section]
            .spacing(20)
            .width(Length::Fill)
    )
    .height(Length::Fill)
    .into()
}

/// Create responsive grid layout for preset cards
fn create_preset_grid<'a>(
    filtered_presets: Vec<(usize, &'a ConfigurationPreset)>,
    selected_preset: Option<usize>,
) -> Element<'a, PresetManagerMessage> {
    // Create rows of cards (3 cards per row)
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    
    for (original_index, preset) in filtered_presets {
        let card = create_compact_preset_card(preset, original_index, selected_preset == Some(original_index));
        current_row.push(card);
        
        if current_row.len() == 3 {
            let row_element = row(current_row)
                .spacing(12)
                .width(Length::Fill);
            rows.push(row_element.into());
            current_row = Vec::new();
        }
    }
    
    // Add remaining cards in the last row
    if !current_row.is_empty() {
        // Pad with empty space to maintain alignment
        while current_row.len() < 3 {
            current_row.push(container("").width(Length::Fill).into());
        }
        let row_element = row(current_row)
            .spacing(12)
            .width(Length::Fill);
        rows.push(row_element.into());
    }
    
    column(rows)
        .spacing(12)
        .width(Length::Fill)
        .into()
}

/// Create compact preset card for grid layout
fn create_compact_preset_card<'a>(
    preset: &'a ConfigurationPreset,
    index: usize,
    is_selected: bool,
) -> Element<'a, PresetManagerMessage> {
    // Header with name and default badge
    let header = row![
        column![
            text(&preset.name)
                .size(15)
                .color(if is_selected {
                    Color::from_rgb(0.1, 0.1, 0.1)
                } else {
                    Color::from_rgb(0.9, 0.9, 0.9)
                }),
            if preset.is_default {
                container(
                    row![
                        icons::star().size(12),
                        text("DEFAULT").size(10)
                    ]
                    .spacing(3)
                    .align_y(Alignment::Center)
                )
                .padding(6)
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(1.0, 0.8, 0.0).into()),
                    border: Border {
                        radius: 8.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    text_color: Some(Color::from_rgb(0.4, 0.2, 0.0)),
                    ..container::Style::default()
                })
            } else {
                container("")
                    .height(Length::Fixed(0.0))
            }
        ]
        .spacing(4)
        .width(Length::Fill),
        // Network type badge
        container(
            text(match preset.payment_network {
                PaymentNetwork::Testnet => "TEST",
                PaymentNetwork::Mainnet => "MAIN",
            })
            .size(10)
        )
        .padding(6)
        .style(match preset.payment_network {
            PaymentNetwork::Testnet => style::testnet_badge,
            PaymentNetwork::Mainnet => style::mainnet_badge,
        })
    ]
    .align_y(Alignment::Start);
    
    // Compact details
    let details = column![
        text(format!("{:?} • {}", preset.network_type, preset.subnet))
            .size(11)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
        if !preset.wallet_address.is_empty() {
            text(format!("{}...{}", 
                &preset.wallet_address[..6.min(preset.wallet_address.len())],
                if preset.wallet_address.len() > 12 {
                    &preset.wallet_address[preset.wallet_address.len()-6..]
                } else { "" }
            ))
            .size(10)
            .color(Color::from_rgb(0.5, 0.5, 0.5))
        } else {
            text("No wallet set")
                .size(10)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
        }
    ]
    .spacing(2);
    
    // Compact action buttons
    let actions = row![
        button(icons::edit())
            .on_press(PresetManagerMessage::EditPreset(index))
            .padding(6)
            .style(button::secondary),
        button(icons::save())
            .on_press(PresetManagerMessage::DuplicatePreset(index))
            .padding(6)
            .style(button::secondary),
        if !preset.is_default {
            button(icons::star_border())
                .on_press(PresetManagerMessage::SetDefaultPreset(index))
                .padding(6)
                .style(button::primary)
        } else {
            button(icons::star())
                .padding(6)
                .style(button::success)
        },
        button(icons::delete())
            .on_press(PresetManagerMessage::DeletePreset(index))
            .padding(6)
            .style(button::danger)
    ]
    .spacing(4);
    
    let content = column![
        header,
        details,
        container(actions)
            .width(Length::Fill)
            .padding(8)
    ]
    .spacing(8)
    .width(Length::Fill);
    
    container(content)
        .style(if is_selected {
            style::selected_compact_preset_card
        } else {
            style::compact_preset_card
        })
        .padding(12)
        .width(Length::Fill)
        .into()
}

/// Enhanced preset editor view
fn view_preset_editor<'a>(editor: &'a PresetEditor) -> Element<'a, PresetManagerMessage> {
    let title = text(if editor.editing_index.is_some() {
        "Edit Preset"
    } else {
        "Create New Preset"
    })
    .size(20);

    let name_input = column![
        text("Preset Name").size(14),
        text_input("Enter preset name...", &editor.name)
            .on_input(PresetManagerMessage::SetPresetName)
            .padding(8)
            .width(Length::Fill)
    ]
    .spacing(5);

    let payment_network_input = column![
        text("Payment Network").size(14),
        pick_list(
            &[PaymentNetwork::Testnet, PaymentNetwork::Mainnet][..],
            Some(editor.payment_network),
            PresetManagerMessage::SetPaymentNetwork
        )
        .padding(8)
        .width(Length::Fill)
    ]
    .spacing(5);

    let network_type_input = column![
        text("Network Type").size(14),
        pick_list(
            &[NetworkType::Central, NetworkType::Hybrid][..],
            Some(editor.network_type),
            PresetManagerMessage::SetNetworkType
        )
        .padding(8)
        .width(Length::Fill)
    ]
    .spacing(5);

    let subnet_input = column![
        text("Subnet").size(14),
        text_input("Enter subnet...", &editor.subnet)
            .on_input(PresetManagerMessage::SetSubnet)
            .padding(8)
            .width(Length::Fill)
    ]
    .spacing(5);

    let wallet_input = column![
        text("Wallet Address").size(14),
        text_input("Enter wallet address...", &editor.wallet_address)
            .on_input(PresetManagerMessage::SetWalletAddress)
            .padding(8)
            .width(Length::Fill)
    ]
    .spacing(5);

    let default_checkbox = row![
        text("Set as default preset").size(14),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let actions = row![
        button("Cancel")
            .on_press(PresetManagerMessage::CancelEdit)
            .padding(8)
            .style(button::secondary),
        button(if editor.editing_index.is_some() {
            "Update Preset"
        } else {
            "Save Preset"
        })
        .on_press(PresetManagerMessage::SavePreset)
        .padding(8)
        .style(button::primary)
    ]
    .spacing(10);

    let form = column![
        name_input,
        payment_network_input,
        network_type_input,
        subnet_input,
        wallet_input,
        default_checkbox,
        container(actions).width(Length::Fill)
    ]
    .spacing(15)
    .width(Length::Fill);

    container(
        column![title, form]
            .spacing(20)
    )
    .style(style::bordered_box)
    .padding(20)
    .width(Length::Fill)
    .into()
}