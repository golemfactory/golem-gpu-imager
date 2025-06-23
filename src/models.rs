// Forward declarations for module messages
// These will be defined in their respective modules

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageMetadata {
    pub compressed_hash: String,   // Original SHA256 from repo
    pub uncompressed_hash: String, // SHA256 of decompressed data
    pub uncompressed_size: u64,    // Size of decompressed image
    pub created_at: String,        // When metadata was calculated
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigurationPreset {
    pub name: String,
    pub payment_network: PaymentNetwork,
    pub subnet: String,
    pub network_type: NetworkType,
    pub wallet_address: String,
    pub is_default: bool,
    #[serde(default)]
    pub non_interactive_install: bool,
    #[serde(default)]
    pub ssh_keys: Vec<String>,
    #[serde(default)]
    pub configuration_server: Option<String>,
    #[serde(default)]
    pub metrics_server: Option<String>,
    #[serde(default)]
    pub central_net_host: Option<String>,
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

#[derive(Debug, Clone)]
pub enum AppMode {
    StartScreen,
    FlashNewImage,
    EditExistingDisk,
    ManagePresets,
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
