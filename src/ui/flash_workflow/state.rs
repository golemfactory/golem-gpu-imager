pub use crate::models::CancelToken;

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

pub use crate::models::ImageMetadata;

#[derive(Debug, Clone)]
pub struct OsImageGroup {
    pub channel_name: String,         // Channel name (release, testing, etc.)
    pub description: String,          // Channel description
    pub latest_version: OsImage,      // Latest version (prominently displayed)
    pub older_versions: Vec<OsImage>, // Older versions (in expandable section)
    pub expanded: bool,               // Whether older versions are shown
}

#[derive(Debug, Clone)]
pub enum FlashWorkflowState {
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
    ConfigureSettings,
    WritingImage(f32),       // Progress 0.0 - 1.0 for image writing
    VerifyingImage(f32),     // Progress 0.0 - 1.0 for image verification
    Completion(bool),        // Success or failure
}

#[derive(Debug, Clone)]
pub struct FlashState {
    pub workflow_state: FlashWorkflowState,
    pub os_images: Vec<OsImage>,
    pub os_image_groups: Vec<OsImageGroup>,
    pub selected_os_image: Option<usize>,
    pub selected_os_image_group: Option<(usize, usize)>,
    pub selected_device: Option<usize>,
    pub downloads_in_progress: Vec<(String, f32)>, // (version_id, progress)
    pub cancel_token: CancelToken, // Cancellation token for this workflow's operations
}

impl FlashState {
    pub fn new() -> Self {
        Self {
            workflow_state: FlashWorkflowState::SelectOsImage,
            os_images: Vec::new(),
            os_image_groups: Vec::new(),
            selected_os_image: None,
            selected_os_image_group: None,
            selected_device: None,
            downloads_in_progress: Vec::new(),
            cancel_token: CancelToken::new(),
        }
    }
}
