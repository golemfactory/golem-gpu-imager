use crate::models::{NetworkType, PaymentNetwork, ImageMetadata};

#[derive(Debug, Clone)]
pub enum FlashMessage {
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
    GotoSelectTargetDevice,                  // Go to storage device selection screen
    GotoConfigureSettings,                   // Go to image configuration screen
    SetPaymentNetwork(PaymentNetwork),
    SetSubnet(String),
    SetNetworkType(NetworkType),
    SetWalletAddress(String),
    SelectTargetDevice(usize),
    WriteImage,
    CancelWrite,
    FlashAnother,
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
    BackToSelectOsImage,           // Go back to the OS image selection screen
    BackToMainMenu,                // Navigation: go back to main menu
    Exit,                          // App action: exit application
    RefreshRepoData,               // App action: refresh repository data
}