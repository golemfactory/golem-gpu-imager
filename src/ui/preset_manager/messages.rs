use crate::models::{NetworkType, PaymentNetwork};

#[derive(Debug, Clone)]
pub enum PresetEditorMessage {
    Start(usize),
    Cancel,
    Save,
    UpdateName(String),
    UpdatePaymentNetwork(PaymentNetwork),
    UpdateSubnet(String),
    UpdateNetworkType(NetworkType),
    UpdateWalletAddress(String),
}

#[derive(Debug, Clone)]
pub enum PresetManagerMessage {
    SaveAsPreset,                  // Save current configuration as a new preset
    SelectPreset(usize),           // Select a preset by index
    DeletePreset(usize),           // Delete a preset by index
    SetDefaultPreset(usize),       // Set a preset as default
    EditPresetName(usize, String), // Edit a preset name
    Editor(PresetEditorMessage),   // All preset editor operations
    SavePresetsToStorage,          // Save presets to persistent storage
    LoadPresetsFromStorage,        // Load presets from persistent storage
    SetPresetName(String),         // Set name for new preset
    ToggleManager,                 // Toggle preset management UI visibility
    BackToMainMenu,                // Return to main menu
    
    // New messages for preset manager UI
    SetNewPresetName(String),      // Set name for new preset
    CreatePreset,                  // Create new preset
    EditPreset(usize),             // Edit existing preset
    SavePreset,                    // Save preset being edited
    CancelEdit,                    // Cancel editing
    SetPaymentNetwork(PaymentNetwork), // Set payment network in editor
    SetNetworkType(NetworkType),   // Set network type in editor
    SetSubnet(String),             // Set subnet in editor
    SetWalletAddress(String),      // Set wallet address in editor
    
    // Additional actions
    DuplicatePreset(usize),        // Duplicate an existing preset
}