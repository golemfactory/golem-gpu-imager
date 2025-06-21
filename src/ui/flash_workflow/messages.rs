use crate::models::ImageMetadata;

#[derive(Debug, Clone)]
pub enum FlashMessage {
    SelectOsImage(usize),
    DownloadOsImage(usize),
    AnalyzeOsImage(usize), // Analyze metadata for downloaded image
    SelectOsImageFromGroup(usize, usize), // Group index, version index (0 = latest, 1+ = older)
    DownloadOsImageFromGroup(usize, usize), // Group index, version index
    AnalyzeOsImageFromGroup(usize, usize), // Group index, version index - analyze downloaded image
    ToggleVersionHistory(usize), // Toggle expanded state for a group
    ProcessingProgress(
        String,
        crate::utils::streaming_hash_calculator::ProcessingProgress,
    ), // Version ID and unified progress
    ProcessingCompleted(String, ImageMetadata), // Version ID and final metadata
    ProcessingFailed(String, String), // Version ID and error message
    GotoSelectTargetDevice, // Go to storage device selection screen
    GotoConfigureSettings, // Go to image configuration screen
    SelectTargetDevice(usize),
    RefreshTargetDevices, // Delegate device refresh to DeviceSelection module
    WriteImage,
    CancelWrite,
    FlashAnother,
    WriteImageProgress(f32),       // Update the image writing progress
    VerificationProgress(f32),     // Update the verification progress
    WriteImageCompleted,           // Image write completed successfully
    WriteImageFailed(String),      // Image write failed with error message
    BackToSelectOsImage,           // Go back to the OS image selection screen
    BackToSelectTargetDevice,      // Go back to target device selection screen
    BackToMainMenu,                // Navigation: go back to main menu
    Exit,                          // App action: exit application
    RefreshRepoData,               // App action: refresh repository data
}
