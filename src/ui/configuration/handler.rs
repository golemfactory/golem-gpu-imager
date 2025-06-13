use super::{ConfigurationState, ConfigurationMessage};
use iced::Task;
use tracing::debug;

pub fn handle_message(
    state: &mut ConfigurationState,
    presets: &[crate::ui::preset_manager::ConfigurationPreset],
    message: ConfigurationMessage,
) -> Task<crate::models::Message> {
    match message {
        ConfigurationMessage::SetPaymentNetwork(network) => {
            state.payment_network = network;
            debug!("Set payment network: {:?}", network);
            Task::none()
        }
        
        ConfigurationMessage::SetSubnet(subnet) => {
            state.subnet = subnet;
            debug!("Set subnet: {}", state.subnet);
            Task::none()
        }
        
        ConfigurationMessage::SetNetworkType(network_type) => {
            state.network_type = network_type;
            debug!("Set network type: {:?}", network_type);
            Task::none()
        }
        
        ConfigurationMessage::SetWalletAddress(address) => {
            state.wallet_address = address.clone();
            state.is_wallet_valid = address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            debug!("Set wallet address: {} (valid: {})", address, state.is_wallet_valid);
            Task::none()
        }
        
        ConfigurationMessage::LoadFromPreset(index) => {
            if let Some(preset) = presets.get(index) {
                *state = ConfigurationState::from_preset(preset);
                debug!("Loaded configuration from preset: {}", preset.name);
            }
            Task::none()
        }
        
        ConfigurationMessage::ValidateConfiguration => {
            // Revalidate wallet address
            state.is_wallet_valid = state.wallet_address.is_empty() || 
                crate::utils::eth::is_valid_eth_address(&state.wallet_address);
            debug!("Configuration validation: valid={}", state.is_valid());
            Task::none()
        }
    }
}