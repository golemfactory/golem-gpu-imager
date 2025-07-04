#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageMetadata {
    pub compressed_hash: String,   // Original SHA256 from repo
    pub uncompressed_hash: String, // SHA256 of decompressed data
    pub uncompressed_size: u64,    // Size of decompressed image
    pub created_at: String,        // When metadata was calculated
}

#[derive(Debug, Clone)]
pub struct OsImage {
    pub name: String,                    // Channel name
    pub version: String,                 // Version id
    pub description: String,             // Human-readable description
    pub downloaded: bool,                // Whether the image is already downloaded
    pub path: Option<String>,            // Path to the image file if downloaded
    pub created: String,                 // Creation date from metadata
    pub sha256: String,                  // SHA256 hash for verification
    pub is_latest: bool,                 // Whether this is the latest version in the channel
    pub metadata: Option<ImageMetadata>, // Uncompressed image metadata
}

#[derive(Debug, Clone)]
pub struct OsImageGroup {
    pub channel_name: String,         // Channel name (release, testing, etc.)
    pub description: String,          // Channel description
    pub latest_version: OsImage,      // Latest version (prominently displayed)
    pub older_versions: Vec<OsImage>, // Older versions (in expandable section)
    pub expanded: bool,               // Whether older versions are shown
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigurationPreset {
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
}

// Implement Display trait so pick_list can properly show the preset
impl std::fmt::Display for ConfigurationPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_default {
            write!(f, "{} (Default)", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresetEditor {
    pub preset_index: usize,
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
}

impl PresetEditor {
    pub fn new(preset_index: usize, preset: &ConfigurationPreset) -> Self {
        Self {
            preset_index,
            name: preset.name.clone(),
            payment_network: preset.payment_network,
            subnet: preset.subnet.clone(),
            network_type: preset.network_type,
            wallet_address: preset.wallet_address.clone(),
            is_default: preset.is_default,
        }
    }

    pub fn to_preset(&self) -> ConfigurationPreset {
        ConfigurationPreset {
            name: self.name.clone(),
            payment_network: self.payment_network,
            subnet: self.subnet.clone(),
            network_type: self.network_type,
            wallet_address: self.wallet_address.clone(),
            is_default: self.is_default,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && !self.subnet.trim().is_empty()
    }
}

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
pub struct StorageDevice {
    pub name: String,
    pub path: String,
    pub size: String,
}

// A simple cancel token for aborting operations
#[derive(Debug, Clone)]
pub struct CancelToken {
    // Whether the operation should be cancelled
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub enum AppMode {
    StartScreen,
    FlashNewImage(FlashState),
    EditExistingDisk(EditState),
    ManagePresets,
}

pub enum FlashState {
    SelectOsImage,
    ProcessingImage {
        version_id: String,
        download_progress: f32,
        metadata_progress: f32,
        overall_progress: f32,
        channel: String,
        created_date: String,
        phase: crate::utils::streaming_hash_calculator::ProcessingPhase,
        uncompressed_size: Option<u64>,
    },
    SelectTargetDevice,
    ConfigureSettings {
        payment_network: PaymentNetwork,
        subnet: String,
        network_type: NetworkType,
        wallet_address: String,
        is_wallet_valid: bool,
    },
    ClearingPartitions(f32), // Progress 0.0 - 1.0 for partition clearing
    WritingImage(f32),       // Progress 0.0 - 1.0 for image writing
    VerifyingImage(f32),     // Progress 0.0 - 1.0 for image verification
    WritingConfig(f32),      // Progress 0.0 - 1.0 for config writing
    WritingProcess(f32),     // Legacy - for backward compatibility
    Completion(bool),        // Success or failure
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PaymentNetwork {
    Testnet,
    Mainnet,
}

// Implement Display trait for PaymentNetwork so combo_box can display it properly
impl std::fmt::Display for PaymentNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentNetwork::Testnet => write!(f, "Testnet"),
            PaymentNetwork::Mainnet => write!(f, "Mainnet"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkType {
    Hybrid,
    Central,
}

// Implement Display trait for NetworkType so combo_box can display it properly
impl std::fmt::Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkType::Hybrid => write!(f, "Hybrid"),
            NetworkType::Central => write!(f, "Central"),
        }
    }
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
    ManagePresets,
    SelectOsImage(usize),
    DownloadOsImage(usize),
    AnalyzeOsImage(usize),                 // Analyze metadata for downloaded image
    SelectOsImageFromGroup(usize, usize), // Group index, version index (0 = latest, 1+ = older)
    DownloadOsImageFromGroup(usize, usize), // Group index, version index
    AnalyzeOsImageFromGroup(usize, usize), // Group index, version index - analyze downloaded image
    ToggleVersionHistory(usize),          // Toggle expanded state for a group
    ProcessingProgress(String, crate::utils::streaming_hash_calculator::ProcessingProgress), // Version ID and unified progress
    ProcessingCompleted(String, ImageMetadata), // Version ID and final metadata
    ProcessingFailed(String, String),           // Version ID and error message
    GotoConfigureSettings,                   // Go to image configuration screen
    GotoSelectTargetDevice,                  // Go to storage device selection screen
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
    GotoEditConfiguration, // Go to edit configuration screen
    SaveConfiguration,
    BackToMainMenu,
    RepoDataLoaded(Vec<OsImage>),
    RepoGroupDataLoaded(Vec<OsImage>, Vec<OsImageGroup>), // Legacy images + new grouped images
    RepoLoadFailed,
    RefreshRepoData,
    RefreshDevices, // Refresh the list of available storage devices
    // Configuration preset management
    SaveAsPreset,                  // Save current configuration as a new preset
    SelectPreset(usize),           // Select a preset by index
    DeletePreset(usize),           // Delete a preset by index
    SetDefaultPreset(usize),       // Set a preset as default
    EditPresetName(usize, String), // Edit a preset name
    PresetEditor(PresetEditorMessage), // All preset editor operations
    SavePresetsToStorage,          // Save presets to persistent storage
    LoadPresetsFromStorage,        // Load presets from persistent storage
    SetPresetName(String),         // Set name for new preset
    TogglePresetManager,           // Toggle preset management UI visibility
    BackToSelectOsImage,           // Go back to the OS image selection screen
    DeviceLocked(Option<crate::disk::Disk>), // Device has been locked for editing
    ConfigurationSaved,            // Configuration has been saved to device
    ConfigurationSaveFailed,       // Failed to save configuration to device
    ShowError(String),             // Show an error message to the user
    DeviceLockedForWriting(crate::disk::Disk, String), // Device locked for writing with image path
    ClearPartitionsProgress(f32),  // Update partition clearing progress
    ClearPartitionsCompleted,      // Partition clearing completed successfully
    ClearPartitionsFailed(String), // Partition clearing failed with error message
    WriteImageProgress(f32),       // Update the image writing progress
    VerificationProgress(f32),     // Update the verification progress
    WriteImageCompleted,           // Image write completed successfully
    WriteImageFailed(String),      // Image write failed with error message
    WriteConfigProgress(f32),      // Update the config writing progress
    WriteConfigCompleted,          // Config write completed successfully
    WriteConfigFailed(String),     // Config write failed with error message
    PollWriteProgress,             // Poll for progress updates from the subscription
    RequestElevation,              // Request administrator elevation (Windows only)
    CheckElevationStatus,          // Check current elevation status
}
