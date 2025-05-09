#[derive(Debug, Clone)]
pub struct OsImage {
    pub name: String,         // Channel name
    pub version: String,      // Version id
    pub description: String,  // Human-readable description
    pub downloaded: bool,     // Whether the image is already downloaded
    pub path: Option<String>, // Path to the image file if downloaded
    pub created: String,      // Creation date from metadata
    pub sha256: String,       // SHA256 hash for verification
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigurationPreset {
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
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
    DownloadingImage {
        version_id: String,
        progress: f32,
        channel: String,
        created_date: String,
    },
    SelectTargetDevice,
    ConfigureSettings {
        payment_network: PaymentNetwork,
        subnet: String,
        network_type: NetworkType,
        wallet_address: String,
        is_wallet_valid: bool,
    },
    WritingProcess(f32), // Progress 0.0 - 1.0
    Completion(bool),    // Success or failure
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaymentNetwork {
    Testnet,
    Mainnet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkType {
    Hybrid,
    Central,
}

pub enum EditState {
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
pub enum Message {
    FlashNewImage,
    EditExistingDisk,
    SelectOsImage(usize),
    DownloadOsImage(usize),
    DownloadProgress(String, f32),  // Version ID and progress (0.0-1.0)
    DownloadCompleted(String),      // Version ID of completed download
    DownloadFailed(String, String), // Version ID and error message
    GotoConfigureSettings,          // Go to image configuration screen
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    SelectTargetDevice(usize),
    WriteImage,
    CancelWrite,
    FlashAnother,
    Exit,
    SelectExistingDevice(usize),
    GotoEditConfiguration,          // Go to edit configuration screen
    SaveConfiguration,
    BackToMainMenu,
    RepoDataLoaded(Vec<OsImage>),
    RepoLoadFailed,
    RefreshRepoData,
    // Configuration preset management
    SaveAsPreset,                   // Save current configuration as a new preset
    SelectPreset(usize),            // Select a preset by index
    DeletePreset(usize),            // Delete a preset by index
    SetDefaultPreset(usize),        // Set a preset as default
    EditPresetName(usize, String),  // Edit a preset name
    SavePresetsToStorage,           // Save presets to persistent storage
    LoadPresetsFromStorage,         // Load presets from persistent storage
    SetPresetName(String),          // Set name for new preset
}
