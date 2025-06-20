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
    DeletePreset(usize),
    SetDefaultPreset(usize),
    SetPresetName(String),

    // Configuration settings
    SetPaymentNetwork(crate::models::PaymentNetwork),
    SetNetworkType(crate::models::NetworkType),
    SetSubnet(String),
    SetWalletAddress(String),
    InitializeFlashConfiguration,

    // Module-specific message variants
    Flash(FlashMessage),
    Edit(EditMessage),
    PresetManager(PresetManagerMessage),
    DeviceSelection(DeviceMessage),
    Configuration(ConfigurationMessage),
}

// Convert from the old models::Message to new ui::messages::Message
impl From<crate::models::Message> for Message {
    fn from(msg: crate::models::Message) -> Self {
        match msg {
            crate::models::Message::FlashNewImage => Message::FlashNewImage,
            crate::models::Message::EditExistingDisk => Message::EditExistingDisk,
            crate::models::Message::ManagePresets => Message::ManagePresets,
            crate::models::Message::BackToMainMenu => Message::BackToMainMenu,
            crate::models::Message::Exit => Message::Exit,
            crate::models::Message::ShowError(err) => Message::ShowError(err),
            crate::models::Message::RefreshRepoData => Message::RefreshRepoData,
            crate::models::Message::RepoLoadFailed => Message::RepoLoadFailed,
            crate::models::Message::RequestElevation => Message::RequestElevation,
            crate::models::Message::CheckElevationStatus => Message::CheckElevationStatus,
            // For data-carrying messages, we need placeholder conversion
            crate::models::Message::RepoDataLoaded(_) => Message::RepoLoadFailed,
            crate::models::Message::RepoGroupDataLoaded(_) => Message::RepoLoadFailed,
        }
    }
}
