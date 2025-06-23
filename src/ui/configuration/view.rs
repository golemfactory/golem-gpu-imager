use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Length};

use crate::models::{NetworkType, PaymentNetwork};
use crate::style;
use crate::ui::{icons, messages::Message};
use super::{ConfigurationMessage, ConfigurationState};

/// Main configuration view - reusable across all contexts
pub fn view_configuration<'a, F>(
    state: &'a ConfigurationState,
    title: &'a str,
    description: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let header = view_header(title, description);
    let form = view_configuration_form(state, message_factory);
    
    column![header, form]
        .spacing(20)
        .width(Length::Fill)
        .into()
}

/// Configuration header component
pub fn view_header<'a>(title: &'a str, description: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).size(28),
            text(description).size(16),
        ]
        .spacing(5)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box)
    .into()
}

/// Main configuration form component
pub fn view_configuration_form<'a, F>(
    state: &'a ConfigurationState,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let basic_form = view_basic_configuration(state, message_factory);
    let advanced_form = view_advanced_configuration(state, message_factory);
    
    container(
        column![
            basic_form,
            advanced_form,
        ]
        .spacing(20)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box)
    .into()
}

/// Basic configuration fields
pub fn view_basic_configuration<'a, F>(
    state: &'a ConfigurationState,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        // Non-interactive install checkbox
        column![
            checkbox("Non-Interactive Mode (Headless)", state.non_interactive_install)
                .on_toggle(move |checked| message_factory(ConfigurationMessage::SetNonInteractiveInstall(checked)))
                .size(16),
            text("First OS start will not ask anything - will select available GPUs and data partition without user interaction")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
        ]
        .spacing(5),
        
        // Payment Network
        view_payment_network_field(state.payment_network, message_factory),
        
        // Network Type
        view_network_type_field(state.network_type, message_factory),
        
        // Subnet
        view_subnet_field(&state.subnet, message_factory),
        
        // Wallet Address
        view_wallet_address_field(&state.wallet_address, state.is_wallet_valid, message_factory),
        
        // SSH Keys
        view_ssh_keys_field(&state.ssh_keys, message_factory),
        
        // Configuration Server
        view_configuration_server_field(&state.configuration_server, message_factory),
    ]
    .spacing(20)
    .into()
}

/// Advanced configuration options with accordion
pub fn view_advanced_configuration<'a, F>(
    state: &'a ConfigurationState,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let toggle_button = view_advanced_options_toggle(state.advanced_options_expanded, message_factory);
    
    let advanced_fields = if state.advanced_options_expanded {
        column![
            view_metrics_server_field(&state.metrics_server, message_factory),
            view_central_net_host_field(&state.central_net_host, message_factory),
        ]
        .spacing(20)
    } else {
        column![]
    };
    
    column![toggle_button, advanced_fields]
        .spacing(15)
        .into()
}

/// Advanced options toggle button
pub fn view_advanced_options_toggle<'a, F>(
    expanded: bool,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let expand_icon = if expanded {
        icons::expand_less()
    } else {
        icons::expand_more()
    };
    
    let toggle_text = if expanded {
        "Hide Advanced Options"
    } else {
        "Show Advanced Options"
    };
    
    container(
        button(
            row![expand_icon, text(toggle_text).size(16)]
                .spacing(8)
                .align_y(Alignment::Center)
        )
        .on_press(message_factory(ConfigurationMessage::ToggleAdvancedOptions))
        .padding(8)
        .style(button::text)
    )
    .width(Length::Fill)
    .style(style::bordered_box)
    .into()
}

/// Payment network field component
pub fn view_payment_network_field<'a, F>(
    payment_network: PaymentNetwork,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Payment Network").size(16),
        pick_list(
            &[PaymentNetwork::Testnet, PaymentNetwork::Mainnet][..],
            Some(payment_network),
            move |network| message_factory(ConfigurationMessage::SetPaymentNetwork(network))
        )
        .width(Length::Fill)
        .style(style::pick_list_style),
        text(match payment_network {
            PaymentNetwork::Testnet => "Use testnet GLM tokens for development and testing",
            PaymentNetwork::Mainnet => "Use real GLM tokens for production workloads",
        })
        .size(12)
        .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Network type field component
pub fn view_network_type_field<'a, F>(
    network_type: NetworkType,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Network Type").size(16),
        pick_list(
            &[NetworkType::Central, NetworkType::Hybrid][..],
            Some(network_type),
            move |network_type| message_factory(ConfigurationMessage::SetNetworkType(network_type))
        )
        .width(Length::Fill)
        .style(style::pick_list_style),
        text(match network_type {
            NetworkType::Central => "Connect through central network infrastructure",
            NetworkType::Hybrid => "Mix of central and peer-to-peer connections",
        })
        .size(12)
        .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Subnet field component
pub fn view_subnet_field<'a, F>(
    subnet: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Subnet").size(16),
        text_input("Enter subnet name (e.g., 'public')", subnet)
            .on_input(move |subnet| message_factory(ConfigurationMessage::SetSubnet(subnet)))
            .width(Length::Fill)
            .style(style::default_text_input),
        text("Specify which subnet to connect to on the Golem Network")
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Wallet address field component with validation
pub fn view_wallet_address_field<'a, F>(
    wallet_address: &'a str,
    is_valid: bool,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    let validation_message = if !wallet_address.is_empty() {
        if is_valid {
            container(
                row![
                    icons::check_circle().color(style::SUCCESS),
                    text("Valid Ethereum address").color(style::SUCCESS)
                ]
                .spacing(5)
                .align_y(Alignment::Center)
            )
            .style(style::valid_message_container)
        } else {
            container(
                row![
                    icons::error().color(style::ERROR),
                    text("Invalid Ethereum address format").color(style::ERROR)
                ]
                .spacing(5)
                .align_y(Alignment::Center)
            )
            .style(style::invalid_message_container)
        }
    } else {
        container(
            text("Leave empty to use the node's default wallet")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6))
        )
    };
    
    column![
        text("Wallet Address (Optional)").size(16),
        text_input("Enter Ethereum wallet address (0x...)", wallet_address)
            .on_input(move |address| message_factory(ConfigurationMessage::SetWalletAddress(address)))
            .width(Length::Fill)
            .style(if wallet_address.is_empty() {
                style::default_text_input
            } else if is_valid {
                style::valid_wallet_input
            } else {
                style::invalid_wallet_input
            }),
        validation_message,
    ]
    .spacing(5)
    .into()
}

/// SSH keys field component
pub fn view_ssh_keys_field<'a, F>(
    ssh_keys: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("SSH Public Keys").size(16),
        text_input("Enter SSH public keys (one per line or comma separated)", ssh_keys)
            .on_input(move |keys| message_factory(ConfigurationMessage::SetSSHKeys(keys)))
            .width(Length::Fill)
            .style(style::default_text_input),
        text("Public keys in OpenSSH format for user 'golem' - leave empty if not needed")
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Configuration server field component
pub fn view_configuration_server_field<'a, F>(
    configuration_server: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Configuration Server (Optional)").size(16),
        text_input("Enter configuration server URL", configuration_server)
            .on_input(move |server| message_factory(ConfigurationMessage::SetConfigurationServer(server)))
            .width(Length::Fill)
            .style(style::default_text_input),
        text("URL to server where to look for configuration updates")
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Metrics server field component
pub fn view_metrics_server_field<'a, F>(
    metrics_server: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Metrics Server").size(16),
        text_input("Enter metrics server URL", metrics_server)
            .on_input(move |server| message_factory(ConfigurationMessage::SetMetricsServer(server)))
            .width(Length::Fill)
            .style(style::default_text_input),
        text("URL to metrics server push endpoint (default: https://metrics.golem.network:9092/)")
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Central net host field component
pub fn view_central_net_host_field<'a, F>(
    central_net_host: &'a str,
    message_factory: F,
) -> Element<'a, Message>
where
    F: Fn(ConfigurationMessage) -> Message + Copy + 'a,
{
    column![
        text("Central Net Host").size(16),
        text_input("Enter central net server address", central_net_host)
            .on_input(move |host| message_factory(ConfigurationMessage::SetCentralNetHost(host)))
            .width(Length::Fill)
            .style(style::default_text_input),
        text("Central network coordination server address (leave empty by default)")
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
    ]
    .spacing(5)
    .into()
}

/// Navigation buttons component
pub fn view_navigation<'a>(
    back_action: Message,
    next_action: Option<Message>,
    back_label: &'a str,
    next_label: &'a str,
    can_proceed: bool,
) -> Element<'a, Message> {
    let back_button = button(
        row![icons::navigate_before(), text(back_label)]
            .spacing(5)
            .align_y(Alignment::Center)
    )
    .on_press(back_action)
    .padding(8)
    .width(220)
    .style(button::secondary);
    
    let next_button = if let Some(action) = next_action {
        if can_proceed {
            button(
                container(
                    row![text(next_label), icons::navigate_next()]
                        .spacing(5)
                        .align_y(Alignment::Center)
                )
                .center_x(Length::Fill)
            )
            .on_press(action)
            .padding(8)
            .width(220)
            .style(button::primary)
        } else {
            button(
                container(
                    row![
                        text("Complete configuration to continue"),
                        icons::navigate_next()
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center)
                )
                .center_x(Length::Fill)
            )
            .padding(8)
            .width(220)
            .style(button::secondary)
        }
    } else {
        button(text(""))
            .padding(8)
            .width(220)
            .style(button::secondary)
    };
    
    container(
        row![back_button, next_button]
            .spacing(15)
            .width(Length::Fill)
            .align_y(Alignment::Center)
    )
    .width(Length::Fill)
    .padding(15)
    .style(style::bordered_box)
    .into()
}

/// Workflow configuration editor with preset management
pub fn view_configuration_editor<'a, F>(
    configuration_state: &'a ConfigurationState,
    title: &'a str,
    description: &'a str,
    back_action: Message,
    next_action: Option<Message>,
    back_label: &'a str,
    next_label: &'a str,
    configuration_presets: &'a [crate::models::ConfigurationPreset],
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
    configuration_presets: &'a [crate::models::ConfigurationPreset],
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