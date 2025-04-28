#[derive(Debug, Clone)]
pub struct OsImage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub downloaded: bool,
}

#[derive(Debug, Clone)]
pub struct StorageDevice {
    pub name: String,
    pub path: String,
    pub size: String,
}

pub enum AppMode {
    StartScreen,
    FlashNewImage(FlashState),
    EditExistingDisk(EditState),
}

pub enum FlashState {
    SelectOsImage,
    ConfigureSettings,
    SelectTargetDevice,
    WritingProcess(f32), // Progress 0.0 - 1.0
    Completion(bool),    // Success or failure
}

pub enum EditState {
    SelectDevice,
    EditConfiguration,
    Completion(bool), // Success or failure
}

#[derive(Debug, Clone)]
pub enum Message {
    FlashNewImage,
    EditExistingDisk,
    SelectOsImage(usize),
    DownloadOsImage(usize),
    ConfigureSettings,
    SelectTargetDevice(usize),
    WriteImage,
    CancelWrite,
    FlashAnother,
    Exit,
    SelectExistingDevice(usize),
    SaveConfiguration,
    BackToMainMenu,
}