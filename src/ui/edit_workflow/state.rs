use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum EditWorkflowState {
    SelectDevice,
    LoadingConfiguration, // Loading configuration from selected device
    EditConfiguration {
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
    },
    Completion(bool), // Success or failure
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub workflow_state: EditWorkflowState,
    pub selected_device: Option<usize>,
    pub locked_disk: Option<crate::disk::Disk>,
    pub error_message: Option<String>,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            workflow_state: EditWorkflowState::SelectDevice,
            selected_device: None,
            locked_disk: None,
            error_message: None,
        }
    }
}
