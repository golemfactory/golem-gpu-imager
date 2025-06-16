use iced::widget::{
    Column, button, column, container, row, scrollable, text, text_input, pick_list,
};
use iced::{Alignment, Element, Length};
use crate::models::{ConfigurationPreset, PaymentNetwork, NetworkType};
use crate::ui::preset_manager::{PresetManagerState, PresetManagerMessage, PresetEditorMessage};
use crate::style;
use crate::ui::icons;

/// Main preset manager view function
pub fn view_preset_manager<'a>(state: &'a PresetManagerState) -> Element<'a, PresetManagerMessage> {
    let header = container(
        row![
            text("Preset Management").size(28),
            button(
                row![
                    icons::navigate_before(),
                    text("Back to Main Menu")
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .on_press(PresetManagerMessage::BackToMainMenu)
        ]
        .spacing(20)
        .align_y(Alignment::Center)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box);

    let content = if let Some(editor) = &state.editor {
        view_preset_editor(editor)
    } else {
        view_preset_list(&state.presets, state.selected_preset, &state.new_preset_name)
    };

    column![header, content]
        .spacing(20)
        .padding(20)
        .into()
}

/// View for displaying the list of presets
pub fn view_preset_list<'a>(
    presets: &'a [ConfigurationPreset], 
    selected_preset: Option<usize>,
    new_preset_name: &'a str
) -> Element<'a, PresetManagerMessage> {
    let mut preset_items = Column::new().spacing(10);

    // Add new preset form
    let new_preset_form = container(
        column![
            text("Create New Preset").size(20),
            row![
                text_input("Enter preset name...", new_preset_name)
                    .on_input(PresetManagerMessage::SetPresetName)
                    .width(Length::FillPortion(3)),
                button("Save as Preset")
                    .on_press(PresetManagerMessage::SaveAsPreset)
                    .width(Length::FillPortion(1))
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ]
        .spacing(10)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box);

    preset_items = preset_items.push(new_preset_form);

    // Add presets header
    if !presets.is_empty() {
        let presets_header = container(
            text("Existing Presets").size(20)
        )
        .width(Length::Fill)
        .padding(15);
        
        preset_items = preset_items.push(presets_header);
    }

    // Add each preset as a card
    for (index, preset) in presets.iter().enumerate() {
        let is_selected = selected_preset == Some(index);
        
        let preset_card = create_preset_card(preset, index, is_selected);
        preset_items = preset_items.push(preset_card);
    }

    // Show empty state if no presets
    if presets.is_empty() {
        let empty_message = container(
            column![
                icons::storage(),
                text("No Presets Available").size(18),
                text("Create your first preset using the form above").size(14)
            ]
            .spacing(10)
            .align_x(Alignment::Center)
        )
        .width(Length::Fill)
        .padding(40)
        .style(style::bordered_box);
        
        preset_items = preset_items.push(empty_message);
    }

    scrollable(preset_items).height(Length::Fill).into()
}

/// Create a card for a single preset
fn create_preset_card<'a>(
    preset: &'a ConfigurationPreset, 
    index: usize, 
    is_selected: bool
) -> Element<'a, PresetManagerMessage> {
    let selection_indicator = if is_selected {
        container(text("●").color([0.2, 0.6, 1.0]))
    } else {
        container(text("○"))
    };

    let default_badge = if preset.is_default {
        Some(
            container(text("DEFAULT"))
                .padding([2, 8])
                .style(|theme: &iced::Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(palette.success.base.color.into()),
                        text_color: Some(palette.success.base.text),
                        border: iced::Border {
                            radius: 12.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })
        )
    } else {
        None
    };

    let name_and_badge = if let Some(badge) = default_badge {
        row![
            text(&preset.name).size(16),
            badge
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    } else {
        row![text(&preset.name).size(16)]
    };

    let preset_info = column![
        name_and_badge,
        text(format!("Network: {:?}", preset.payment_network)).size(12),
        text(format!("Subnet: {}", preset.subnet)).size(12),
        text(format!("Type: {:?}", preset.network_type)).size(12),
        if !preset.wallet_address.is_empty() {
            text(format!("Wallet: {}...", &preset.wallet_address[..preset.wallet_address.len().min(20)])).size(12)
        } else {
            text("Wallet: Not set").size(12)
        }
    ]
    .spacing(4);

    let actions = row![
        button("Select")
            .on_press(PresetManagerMessage::SelectPreset(index)),
        button("Edit")
            .on_press(PresetManagerMessage::Editor(PresetEditorMessage::Start(index))),
        if !preset.is_default {
            button("Set Default")
                .on_press(PresetManagerMessage::SetDefaultPreset(index))
        } else {
            button("Default").style(button::secondary)
        },
        button("Delete")
            .on_press(PresetManagerMessage::DeletePreset(index))
            .style(button::danger)
    ]
    .spacing(8);

    let content = row![
        selection_indicator.width(Length::Fixed(20.0)),
        preset_info.width(Length::Fill),
        actions.width(Length::Shrink)
    ]
    .spacing(15)
    .align_y(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding(15)
        .style(if is_selected {
            |theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.primary.weak.color.into()),
                    border: iced::Border {
                        width: 1.0,
                        radius: 5.0.into(),
                        color: palette.primary.strong.color,
                    },
                    ..Default::default()
                }
            }
        } else {
            style::bordered_box
        })
        .into()
}

/// View for editing a preset
pub fn view_preset_editor<'a>(editor: &'a crate::ui::preset_manager::PresetEditor) -> Element<'a, PresetManagerMessage> {
    let header = container(
        row![
            text("Edit Preset").size(24),
            button("Cancel")
                .on_press(PresetManagerMessage::Editor(PresetEditorMessage::Cancel))
        ]
        .spacing(20)
        .align_y(Alignment::Center)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box);

    let form = container(
        column![
            // Name field
            column![
                text("Preset Name").size(14),
                text_input("Enter preset name", &editor.name)
                    .on_input(|name| PresetManagerMessage::Editor(PresetEditorMessage::UpdateName(name)))
            ]
            .spacing(5),

            // Payment Network field
            column![
                text("Payment Network").size(14),
                pick_list(
                    vec![PaymentNetwork::Testnet, PaymentNetwork::Mainnet],
                    Some(editor.payment_network),
                    |network| PresetManagerMessage::Editor(PresetEditorMessage::UpdatePaymentNetwork(network))
                )
            ]
            .spacing(5),

            // Subnet field
            column![
                text("Subnet").size(14),
                text_input("Enter subnet", &editor.subnet)
                    .on_input(|subnet| PresetManagerMessage::Editor(PresetEditorMessage::UpdateSubnet(subnet)))
            ]
            .spacing(5),

            // Network Type field
            column![
                text("Network Type").size(14),
                pick_list(
                    vec![NetworkType::Central, NetworkType::Hybrid],
                    Some(editor.network_type),
                    |network_type| PresetManagerMessage::Editor(PresetEditorMessage::UpdateNetworkType(network_type))
                )
            ]
            .spacing(5),

            // Wallet Address field
            column![
                text("Wallet Address").size(14),
                text_input("Enter wallet address", &editor.wallet_address)
                    .on_input(|address| PresetManagerMessage::Editor(PresetEditorMessage::UpdateWalletAddress(address)))
            ]
            .spacing(5),

            // Action buttons
            row![
                button("Save Changes")
                    .on_press(PresetManagerMessage::Editor(PresetEditorMessage::Save))
                    .style(button::primary),
                button("Cancel")
                    .on_press(PresetManagerMessage::Editor(PresetEditorMessage::Cancel))
            ]
            .spacing(10)
        ]
        .spacing(15)
    )
    .width(Length::Fill)
    .padding(20)
    .style(style::bordered_box);

    column![header, form]
        .spacing(20)
        .into()
}