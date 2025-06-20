use iced::widget::{
    Column, Container, button, checkbox, column, container, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Alignment, Color, Element, Length};
use iced::{Border, Theme};

use crate::models::{NetworkType, PaymentNetwork};
use crate::style;
use crate::ui::{icons, messages::Message, preset_manager::PresetEditorMessage};

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    SetPaymentNetwork(PaymentNetwork),
    SetNetworkType(NetworkType),
    SetSubnet(String),
    SetWalletAddress(String),
    SelectPreset(usize),
    SetNonInteractiveInstall(bool),
    SetSSHKeys(String),
    SetConfigurationServer(String),
    SetMetricsServer(String),
    SetCentralNetHost(String),
}

/// Shared configuration UI component used in both flash and edit workflows
pub fn view_configuration_editor<'a, F>(
    payment_network: PaymentNetwork,
    subnet: String,
    network_type: NetworkType,
    wallet_address: String,
    is_wallet_valid: bool,
    non_interactive_install: bool,
    ssh_keys: String,
    configuration_server: String,
    metrics_server: String,
    central_net_host: String,
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
    preset_editor: Option<&'a crate::ui::preset_manager::PresetEditor>,
    preset_back_action: Message,
    preset_manager_action: Message,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigMessage) -> Message + Copy + 'a,
{
    // Show preset editor if active
    if let Some(editor) = preset_editor {
        return view_preset_editor(editor);
    }

    // Page header
    let header =
        container(column![text(title_text).size(28), text(description_text).size(16),].spacing(5))
            .width(Length::Fill)
            .padding(15)
            .style(crate::style::bordered_box);

    // Configuration preset selection section
    let preset_section = if !configuration_presets.is_empty() {
        let preset_list = pick_list(
            configuration_presets,
            selected_preset.and_then(|i| configuration_presets.get(i)),
            move |preset| {
                // Find the index of the selected preset and apply it
                if let Some(index) = configuration_presets
                    .iter()
                    .position(|p| p.name == preset.name)
                {
                    message_factory(ConfigMessage::SelectPreset(index))
                } else {
                    Message::ShowError("Preset not found".to_string())
                }
            },
        )
        .placeholder("Select a configuration preset...")
        .width(Length::Fill)
        .style(crate::style::pick_list_style);

        let preset_manager_button = button(
            row![icons::settings(), text("Manage Presets")]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(preset_manager_action.clone())
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
        .style(crate::style::bordered_box)
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
                .on_press(preset_manager_action.clone())
                .padding(8)
                .style(button::primary)
            ]
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(15)
        .style(crate::style::bordered_box)
    };

    // Configuration form section
    let form_section = container(
        column![
            // Non-interactive install (headless mode) - at the top as requested
            column![
                checkbox("Non-Interactive Mode (Headless)", non_interactive_install)
                    .on_toggle(move |checked| message_factory(ConfigMessage::SetNonInteractiveInstall(checked)))
                    .size(16),
                text("First OS start will not ask anything - will select available GPUs and data partition without user interaction")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Payment Network selection
            column![
                text("Payment Network").size(16),
                pick_list(
                    &[PaymentNetwork::Testnet, PaymentNetwork::Mainnet][..],
                    Some(payment_network),
                    move |network| message_factory(ConfigMessage::SetPaymentNetwork(network))
                )
                .width(Length::Fill)
                .style(crate::style::pick_list_style),
                text(match payment_network {
                    PaymentNetwork::Testnet => "Use testnet GLM tokens for development and testing",
                    PaymentNetwork::Mainnet => "Use real GLM tokens for production workloads",
                })
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Network Type selection
            column![
                text("Network Type").size(16),
                pick_list(
                    &[NetworkType::Central, NetworkType::Hybrid][..],
                    Some(network_type),
                    move |network_type| message_factory(ConfigMessage::SetNetworkType(
                        network_type
                    ))
                )
                .width(Length::Fill)
                .style(crate::style::pick_list_style),
                text(match network_type {
                    NetworkType::Central => "Connect through central network infrastructure",
                    NetworkType::Hybrid => "Mix of central and peer-to-peer connections",
                })
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Subnet configuration
            column![
                text("Subnet").size(16),
                text_input("Enter subnet name (e.g., 'public')", &subnet)
                    .on_input(move |subnet| message_factory(ConfigMessage::SetSubnet(subnet)))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("Specify which subnet to connect to on the Golem Network")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Wallet address input with validation
            column![
                text("Wallet Address (Optional)").size(16),
                text_input("Enter Ethereum wallet address (0x...)", &wallet_address)
                    .on_input(
                        move |address| message_factory(ConfigMessage::SetWalletAddress(address))
                    )
                    .width(Length::Fill)
                    .style(if wallet_address.is_empty() {
                        crate::style::default_text_input
                    } else if is_wallet_valid {
                        crate::style::valid_wallet_input
                    } else {
                        crate::style::invalid_wallet_input
                    }),
                // Validation message
                if !wallet_address.is_empty() {
                    if is_wallet_valid {
                        container(
                            row![
                                icons::check_circle().color(crate::style::SUCCESS),
                                text("Valid Ethereum address").color(crate::style::SUCCESS)
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                        )
                        .style(crate::style::valid_message_container)
                    } else {
                        container(
                            row![
                                icons::error().color(crate::style::ERROR),
                                text("Invalid Ethereum address format").color(crate::style::ERROR)
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                        )
                        .style(crate::style::invalid_message_container)
                    }
                } else {
                    container(
                        text("Leave empty to use the node's default wallet")
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.6, 0.6)),
                    )
                }
            ]
            .spacing(5),
            // SSH Keys configuration
            column![
                text("SSH Public Keys").size(16),
                text_input("Enter SSH public keys (one per line or comma separated)", &ssh_keys)
                    .on_input(move |keys| message_factory(ConfigMessage::SetSSHKeys(keys)))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("Public keys in OpenSSH format for user 'golem' - leave empty if not needed")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Configuration Server
            column![
                text("Configuration Server (Optional)").size(16),
                text_input("Enter configuration server URL", &configuration_server)
                    .on_input(move |server| message_factory(ConfigMessage::SetConfigurationServer(server)))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("URL to server where to look for configuration updates")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Advanced Options Section
            text("Advanced Options").size(18).color(Color::from_rgb(0.8, 0.8, 0.8)),
            // Metrics Server
            column![
                text("Metrics Server").size(16),
                text_input("Enter metrics server URL", &metrics_server)
                    .on_input(move |server| message_factory(ConfigMessage::SetMetricsServer(server)))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("URL to metrics server push endpoint (default: https://metrics.golem.network:9092/)")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Central Net Host
            column![
                text("Central Net Host").size(16),
                text_input("Enter central net server address", &central_net_host)
                    .on_input(move |host| message_factory(ConfigMessage::SetCentralNetHost(host)))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("Central network coordination server address (leave empty by default)")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    // Save as preset section
    let save_preset_section = if !new_preset_name.trim().is_empty() {
        container(
            column![
                text("Save Current Configuration").size(16),
                row![
                    text_input("Preset name", new_preset_name)
                        .on_input(|name| Message::SetPresetName(name))
                        .width(Length::Fill)
                        .style(crate::style::default_text_input),
                    button(
                        row![icons::save(), text("Save")]
                            .spacing(5)
                            .align_y(Alignment::Center)
                    )
                    .on_press(Message::SaveAsPreset(crate::models::ConfigurationPreset {
                        name: new_preset_name.to_string(),
                        payment_network,
                        subnet: subnet.clone(),
                        network_type,
                        wallet_address: wallet_address.clone(),
                        non_interactive_install,
                        ssh_keys: ssh_keys.split('\n').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                        configuration_server: if configuration_server.trim().is_empty() { None } else { Some(configuration_server.clone()) },
                        metrics_server: if metrics_server.trim().is_empty() { None } else { Some(metrics_server.clone()) },
                        central_net_host: if central_net_host.trim().is_empty() { None } else { Some(central_net_host.clone()) },
                        is_default: false,
                    }))
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
        .style(crate::style::bordered_box)
    } else {
        container(column![])
    };

    // Navigation buttons
    let can_proceed = !subnet.trim().is_empty() && (wallet_address.is_empty() || is_wallet_valid);

    let next_button = if can_proceed && !next_label.is_empty() {
        button(
            container(
                row![text(next_label), icons::navigate_next()]
                    .spacing(5)
                    .align_y(Alignment::Center),
            )
            .center_x(Length::Fill),
        )
        .on_press(next_action)
        .padding(8)
        .width(220)
        .style(button::primary)
    } else if next_label.is_empty() {
        // No next button needed, use an empty button
        button(text(""))
            .padding(8)
            .width(220)
            .style(button::secondary)
    } else {
        button(
            container(
                row![
                    text("Complete configuration to continue"),
                    icons::navigate_next()
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .center_x(Length::Fill),
        )
        .padding(8)
        .width(220)
        .style(button::secondary)
    };

    let back_button = button(
        row![icons::navigate_before(), text(back_label)]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(back_action)
    .padding(8)
    .width(220)
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

    // Main content - make scrollable if needed
    let content = column![
        header,
        scrollable(
            column![preset_section, form_section, save_preset_section,]
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
        .style(crate::style::main_box)
        .into()
}

/// Shared preset editor component
pub fn view_preset_editor<'a>(
    editor: &'a crate::ui::preset_manager::PresetEditor,
) -> Element<'a, Message> {
    // Page header
    let header = container(
        column![
            text(format!("Edit Preset: {}", editor.name)).size(28),
            text("Modify the configuration settings for this preset").size(16),
        ]
        .spacing(5),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    // Preset editor form
    let form_section = container(
        column![
            // Preset name
            column![
                text("Preset Name").size(16),
                text_input("Enter preset name", &editor.name)
                    .on_input(|name| Message::PresetManager(
                        crate::ui::preset_manager::PresetManagerMessage::Editor(
                            crate::ui::preset_manager::PresetEditorMessage::UpdateName(name)
                        )
                    ))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
            ]
            .spacing(5),
            // Payment Network selection
            column![
                text("Payment Network").size(16),
                pick_list(
                    &[PaymentNetwork::Testnet, PaymentNetwork::Mainnet][..],
                    Some(editor.payment_network),
                    |network| Message::PresetManager(
                        crate::ui::preset_manager::PresetManagerMessage::Editor(
                            crate::ui::preset_manager::PresetEditorMessage::UpdatePaymentNetwork(
                                network
                            )
                        )
                    )
                )
                .width(Length::Fill)
                .style(crate::style::pick_list_style),
                text(match editor.payment_network {
                    PaymentNetwork::Testnet => "Use testnet GLM tokens for development and testing",
                    PaymentNetwork::Mainnet => "Use real GLM tokens for production workloads",
                })
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Network Type selection
            column![
                text("Network Type").size(16),
                pick_list(
                    &[NetworkType::Central, NetworkType::Hybrid][..],
                    Some(editor.network_type),
                    |network_type| Message::PresetManager(
                        crate::ui::preset_manager::PresetManagerMessage::Editor(
                            crate::ui::preset_manager::PresetEditorMessage::UpdateNetworkType(
                                network_type
                            )
                        )
                    )
                )
                .width(Length::Fill)
                .style(crate::style::pick_list_style),
                text(match editor.network_type {
                    NetworkType::Central => "Connect through central network infrastructure",
                    NetworkType::Hybrid => "Mix of central and peer-to-peer connections",
                })
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Subnet configuration
            column![
                text("Subnet").size(16),
                text_input("Enter subnet name (e.g., 'public')", &editor.subnet)
                    .on_input(|subnet| Message::PresetManager(
                        crate::ui::preset_manager::PresetManagerMessage::Editor(
                            crate::ui::preset_manager::PresetEditorMessage::UpdateSubnet(subnet)
                        )
                    ))
                    .width(Length::Fill)
                    .style(crate::style::default_text_input),
                text("Specify which subnet to connect to on the Golem Network")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
            // Wallet address input
            column![
                text("Wallet Address (Optional)").size(16),
                text_input(
                    "Enter Ethereum wallet address (0x...)",
                    &editor.wallet_address
                )
                .on_input(|address| Message::PresetManager(
                    crate::ui::preset_manager::PresetManagerMessage::Editor(
                        crate::ui::preset_manager::PresetEditorMessage::UpdateWalletAddress(
                            address
                        )
                    )
                ))
                .width(Length::Fill)
                .style(if editor.wallet_address.is_empty() {
                    crate::style::default_text_input
                } else if crate::utils::eth::is_valid_eth_address(&editor.wallet_address) {
                    crate::style::valid_wallet_input
                } else {
                    crate::style::invalid_wallet_input
                }),
                // Validation message
                if !editor.wallet_address.is_empty() {
                    if crate::utils::eth::is_valid_eth_address(&editor.wallet_address) {
                        container(
                            row![
                                icons::check_circle().color(crate::style::SUCCESS),
                                text("Valid Ethereum address").color(crate::style::SUCCESS)
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                        )
                        .style(crate::style::valid_message_container)
                    } else {
                        container(
                            row![
                                icons::error().color(crate::style::ERROR),
                                text("Invalid Ethereum address format").color(crate::style::ERROR)
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                        )
                        .style(crate::style::invalid_message_container)
                    }
                } else {
                    container(
                        text("Leave empty to use the node's default wallet")
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.6, 0.6)),
                    )
                }
            ]
            .spacing(5),
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    // Navigation buttons
    let can_save = !editor.name.trim().is_empty()
        && !editor.subnet.trim().is_empty()
        && (editor.wallet_address.is_empty()
            || crate::utils::eth::is_valid_eth_address(&editor.wallet_address));

    let save_button = if can_save {
        button(
            row![icons::save(), text("Save Preset")]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .on_press(Message::PresetManager(
            crate::ui::preset_manager::PresetManagerMessage::Editor(
                crate::ui::preset_manager::PresetEditorMessage::Save,
            ),
        ))
        .padding(8)
        .width(150)
        .style(button::primary)
    } else {
        button(
            row![icons::save(), text("Complete all fields")]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .padding(8)
        .width(150)
        .style(button::secondary)
    };

    let cancel_button = button(
        row![icons::cancel(), text("Cancel")]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .on_press(Message::PresetManager(
        crate::ui::preset_manager::PresetManagerMessage::Editor(
            crate::ui::preset_manager::PresetEditorMessage::Cancel,
        ),
    ))
    .padding(8)
    .width(150)
    .style(button::secondary);

    let navigation = container(
        row![cancel_button, save_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
    .style(crate::style::bordered_box);

    // Main content
    let content = column![
        header,
        scrollable(form_section).height(Length::Fill),
        navigation,
    ]
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::style::main_box)
        .into()
}
