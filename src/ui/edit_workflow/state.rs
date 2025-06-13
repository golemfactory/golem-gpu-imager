use crate::models::{NetworkType, PaymentNetwork};
use crate::ui::device_selection::StorageDevice;

#[derive(Debug, Clone)]
pub enum EditWorkflowState {
    SelectDevice,
    EditConfiguration {
        payment_network: PaymentNetwork,
        subnet: String,
        network_type: NetworkType,
        wallet_address: String,
        is_wallet_valid: bool,
    },
    Completion(bool), // Success or failure
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub workflow_state: EditWorkflowState,
    pub storage_devices: Vec<StorageDevice>,
    pub selected_device: Option<usize>,
    pub locked_disk: Option<crate::disk::Disk>,
    pub error_message: Option<String>,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            workflow_state: EditWorkflowState::SelectDevice,
            storage_devices: Vec::new(),
            selected_device: None,
            locked_disk: None,
            error_message: None,
        }
    }
}