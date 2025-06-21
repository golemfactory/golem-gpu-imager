use crate::ui::configuration::ConfigurationMessage;

#[derive(Debug, Clone)]
pub enum PresetEditorMessage {
    Start(usize),
    Cancel,
    Save,
    UpdateName(String),
    Configuration(ConfigurationMessage), // Delegate all configuration changes to the configuration module
}

#[derive(Debug, Clone)]
pub enum PresetManagerMessage {
    SaveAsPreset(crate::models::ConfigurationPreset), // Save current configuration as a new preset
    SelectPreset(usize),           // Select a preset by index
    DeletePreset(usize),           // Delete a preset by index
    SetDefaultPreset(usize),       // Set a preset as default
    Editor(PresetEditorMessage),   // All preset editor operations
    SavePresetsToStorage,          // Save presets to persistent storage
    LoadPresetsFromStorage,        // Load presets from persistent storage
    SetPresetName(String),         // Set name for new preset
    ToggleManager,                 // Toggle preset management UI visibility
    BackToMainMenu,                // Return to main menu
    SetNewPresetName(String),      // Set name for new preset
    CreatePreset,                  // Create new preset
    EditPreset(usize),             // Edit existing preset
    SavePreset,                    // Save preset being edited
    CancelEdit,                    // Cancel editing
    DuplicatePreset(usize),        // Duplicate an existing preset
}
