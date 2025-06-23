
#[derive(Debug, Clone)]
pub enum EditMessage {
    SelectExistingDevice(usize),
    GotoEditConfiguration,
    DeviceConfigurationLoaded(crate::disk::GolemConfig),
    DeviceConfigurationLoadFailed(String),
    SaveConfiguration,
    ConfigurationSaved,
    ConfigurationSaveFailed,
    BackToMainMenu,
    BackToDeviceSelection,
    EditAnother,
    RefreshDevices,
}
