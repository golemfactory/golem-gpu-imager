use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum EditWorkflowState {
    SelectDevice,
    LoadingConfiguration, // Loading configuration from selected device
    EditConfiguration,    // Configuration editing (uses centralized ConfigurationState)
    Completion(bool),     // Success or failure
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
