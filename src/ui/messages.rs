use crate::ui::{
    configuration::ConfigurationMessage, device_selection::DeviceMessage,
    edit_workflow::EditMessage, flash_workflow::FlashMessage, preset_manager::PresetManagerMessage,
};

#[derive(Debug, Clone)]
pub enum Message {
    // App-level messages
    FlashNewImage,
    EditExistingDisk,
    ManagePresets,
    BackToMainMenu,
    Exit,
    ShowError(String),

    // Repository management
    RepoDataLoaded(Vec<crate::ui::flash_workflow::OsImage>),
    RepoGroupDataLoaded(
        Vec<crate::ui::flash_workflow::OsImage>,
        Vec<crate::ui::flash_workflow::OsImageGroup>,
    ),
    RepoLoadFailed,
    RefreshRepoData,

    // Elevation management (Windows)
    RequestElevation,
    CheckElevationStatus,

    // Preset management
    SaveAsPreset(crate::models::ConfigurationPreset),
    SelectPreset(usize),

    // Configuration settings
    InitializeFlashConfiguration,

    // Module-specific message variants
    Flash(FlashMessage),
    Edit(EditMessage),
    PresetManager(PresetManagerMessage),
    DeviceSelection(DeviceMessage),
    Configuration(ConfigurationMessage),
}
