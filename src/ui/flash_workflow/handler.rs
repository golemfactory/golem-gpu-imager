use super::{FlashState, FlashMessage, FlashWorkflowState};
use crate::models::CancelToken;
use crate::utils::repo::ImageRepo;
use crate::disk::{Disk, WriteProgress};
use iced::Task;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub fn handle_message(
    state: &mut FlashState,
    image_repo: &Arc<ImageRepo>,
    cancel_token: &CancelToken,
    device_selection: &crate::ui::device_selection::DeviceSelectionState,
    message: FlashMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        FlashMessage::SelectOsImage(index) => {
            if let Some(image) = state.os_images.get(index) {
                state.selected_os_image = Some(index);
                debug!("Selected OS image: {}", image.name);
            }
            Task::none()
        }
        
        FlashMessage::SelectOsImageFromGroup(group_index, version_index) => {
            if let Some(group) = state.os_image_groups.get(group_index) {
                let image = if version_index == 0 {
                    &group.latest_version
                } else if let Some(older_image) = group.older_versions.get(version_index - 1) {
                    older_image
                } else {
                    return Task::none();
                };

                state.selected_os_image_group = Some((group_index, version_index));
                debug!("Selected OS image from group: {} version {}", image.name, image.version);
            }
            Task::none()
        }
        
        FlashMessage::ToggleVersionHistory(group_index) => {
            if let Some(group) = state.os_image_groups.get_mut(group_index) {
                group.expanded = !group.expanded;
                debug!("Toggled version history for group {}: expanded={}", group_index, group.expanded);
            }
            Task::none()
        }
        
        FlashMessage::GotoSelectTargetDevice => {
            state.workflow_state = FlashWorkflowState::SelectTargetDevice;
            debug!("Entering target device selection - delegating device refresh to DeviceSelection module");
            Task::done(crate::ui::messages::Message::DeviceSelection(
                crate::ui::device_selection::DeviceMessage::RefreshDevices
            ))
        }
        
        FlashMessage::GotoConfigureSettings => {
            state.workflow_state = FlashWorkflowState::ConfigureSettings {
                payment_network: crate::models::PaymentNetwork::Testnet,
                subnet: "public".to_string(),
                network_type: crate::models::NetworkType::Central,
                wallet_address: String::new(),
                is_wallet_valid: true,
            };
            Task::none()
        }
        
        FlashMessage::SetPaymentNetwork(network) => {
            if let FlashWorkflowState::ConfigureSettings { payment_network, .. } = &mut state.workflow_state {
                *payment_network = network;
            }
            Task::none()
        }
        
        FlashMessage::SetSubnet(subnet) => {
            if let FlashWorkflowState::ConfigureSettings { subnet: current_subnet, .. } = &mut state.workflow_state {
                *current_subnet = subnet;
            }
            Task::none()
        }
        
        FlashMessage::SetNetworkType(network_type) => {
            if let FlashWorkflowState::ConfigureSettings { network_type: current_type, .. } = &mut state.workflow_state {
                *current_type = network_type;
            }
            Task::none()
        }
        
        FlashMessage::SetWalletAddress(address) => {
            if let FlashWorkflowState::ConfigureSettings { wallet_address, is_wallet_valid, .. } = &mut state.workflow_state {
                *wallet_address = address.clone();
                *is_wallet_valid = address.is_empty() || crate::utils::eth::is_valid_eth_address(&address);
            }
            Task::none()
        }
        
        FlashMessage::SelectTargetDevice(index) => {
            state.selected_device = Some(index);
            debug!("Selected target device: {}", index);
            Task::none()
        }
        
        FlashMessage::RefreshTargetDevices => {
            debug!("Delegating target device refresh to DeviceSelection module");
            Task::done(crate::ui::messages::Message::DeviceSelection(
                crate::ui::device_selection::DeviceMessage::RefreshDevices
            ))
        }
        
        FlashMessage::ProcessingProgress(version_id, progress) => {
            // Update download progress for specific version
            if let Some(download) = state.downloads_in_progress.iter_mut().find(|(id, _)| id == &version_id) {
                download.1 = progress.overall_progress;
            }
            
            // Update state if this is the currently processing image
            if let FlashWorkflowState::ProcessingImage { version_id: current_id, .. } = &mut state.workflow_state {
                if current_id == &version_id {
                    *current_id = version_id;
                    // Update the state with progress details
                    if let FlashWorkflowState::ProcessingImage { 
                        download_progress, 
                        metadata_progress, 
                        overall_progress, 
                        phase,
                        uncompressed_size,
                        .. 
                    } = &mut state.workflow_state {
                        *download_progress = progress.download_progress;
                        *metadata_progress = progress.metadata_progress;
                        *overall_progress = progress.overall_progress;
                        *phase = progress.phase.clone();
                        if let Some(size) = progress.uncompressed_size {
                            *uncompressed_size = Some(size);
                        }
                    }
                }
            }
            Task::none()
        }
        
        FlashMessage::ProcessingCompleted(version_id, metadata) => {
            // Remove from downloads in progress
            state.downloads_in_progress.retain(|(id, _)| id != &version_id);
            
            // Update the image metadata
            if let Some(image) = state.os_images.iter_mut().find(|img| img.version == version_id) {
                image.metadata = Some(metadata.clone());
            }
            
            // Also update in groups
            for group in &mut state.os_image_groups {
                if group.latest_version.version == version_id {
                    group.latest_version.metadata = Some(metadata.clone());
                }
                for older_version in &mut group.older_versions {
                    if older_version.version == version_id {
                        older_version.metadata = Some(metadata.clone());
                    }
                }
            }
            
            // Go back to image selection
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            info!("Processing completed for version: {}", version_id);
            Task::none()
        }
        
        FlashMessage::ProcessingFailed(version_id, error) => {
            // Remove from downloads in progress
            state.downloads_in_progress.retain(|(id, _)| id != &version_id);
            
            // Go back to image selection
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            error!("Processing failed for version {}: {}", version_id, error);
            Task::done(crate::ui::messages::Message::ShowError(format!("Failed to process image: {}", error)))
        }
        
        FlashMessage::BackToSelectOsImage => {
            state.workflow_state = FlashWorkflowState::SelectOsImage;
            Task::none()
        }
        
        FlashMessage::BackToSelectTargetDevice => {
            state.workflow_state = FlashWorkflowState::SelectTargetDevice;
            Task::none()
        }
        
        FlashMessage::FlashAnother => {
            *state = FlashState::new();
            Task::none()
        }
        
        // App-level navigation messages that need to be forwarded
        FlashMessage::BackToMainMenu => {
            Task::done(crate::ui::messages::Message::BackToMainMenu)
        }
        
        FlashMessage::Exit => {
            Task::done(crate::ui::messages::Message::Exit)
        }
        
        FlashMessage::RefreshRepoData => {
            Task::done(crate::ui::messages::Message::RefreshRepoData)
        }
        
        FlashMessage::DownloadOsImage(image_index) => {
            debug!("Starting download for OS image at index: {}", image_index);
            
            if let Some(os_image) = state.os_images.get(image_index) {
                // Create a Version struct that matches what ImageRepo expects
                let repo_version = crate::utils::repo::Version {
                    id: os_image.version.clone(),
                    path: format!("golem-gpu-live-{}.img.xz", os_image.version),
                    sha256: os_image.sha256.clone(),
                    created: os_image.created.clone(),
                };

                // Start the download using ImageRepo
                let repo_clone = Arc::clone(image_repo);
                let cancel_token_clone = cancel_token.clone();
                
                let channel_name = "release".to_string(); // Default channel for flat list
                
                // Change workflow state to processing
                state.workflow_state = FlashWorkflowState::ProcessingImage {
                    version_id: os_image.version.clone(),
                    download_progress: 0.0,
                    metadata_progress: 0.0,
                    overall_progress: 0.0,
                    channel: channel_name.clone(),
                    created_date: os_image.created.clone(),
                    phase: crate::utils::streaming_hash_calculator::ProcessingPhase::Download,
                    uncompressed_size: None,
                };
                
                // Add to downloads in progress
                state.downloads_in_progress.push((os_image.version.clone(), 0.0));
                
                info!("Starting download for {} version {}", channel_name, os_image.version);
                
                let version_id_1 = os_image.version.clone();
                let version_id_2 = os_image.version.clone();
                
                return Task::sip(
                    repo_clone.start_download(&channel_name, repo_version, cancel_token_clone),
                    move |status| match status {
                        crate::utils::repo::DownloadStatus::NotStarted => {
                            crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(
                                version_id_2.clone(), 
                                crate::utils::streaming_hash_calculator::ProcessingProgress::new_download(0, 0)
                            ))
                        }
                        crate::utils::repo::DownloadStatus::Processing(progress) => {
                            crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(
                                version_id_2.clone(), 
                                progress
                            ))
                        }
                        crate::utils::repo::DownloadStatus::Completed { metadata, .. } => {
                            crate::ui::messages::Message::Flash(FlashMessage::ProcessingCompleted(
                                version_id_2.clone(), 
                                metadata
                            ))
                        }
                        crate::utils::repo::DownloadStatus::Failed { error } => {
                            crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                                version_id_2.clone(), 
                                error
                            ))
                        }
                    },
                    move |result| {
                        if let Err(e) = result {
                            crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                                version_id_1, 
                                e.to_string()
                            ))
                        } else {
                            // This shouldn't happen in the new flow, but handle gracefully
                            crate::ui::messages::Message::Flash(FlashMessage::BackToSelectOsImage)
                        }
                    }
                );
            }
            
            Task::none()
        }

        FlashMessage::AnalyzeOsImage(image_index) => {
            debug!("Starting analysis for OS image at index: {}", image_index);
            
            if let Some(os_image) = state.os_images.get(image_index) {
                if os_image.downloaded && os_image.metadata.is_none() {
                    state.selected_os_image = Some(image_index);
                    
                    info!("Starting metadata analysis for existing image: {}", os_image.version);

                    // Set state to processing with metadata phase (skip download)
                    state.workflow_state = FlashWorkflowState::ProcessingImage {
                        version_id: os_image.version.clone(),
                        download_progress: 1.0, // Download already complete
                        metadata_progress: 0.0,
                        overall_progress: 0.5,  // Start at 50% (download phase done)
                        channel: os_image.name.clone(),
                        created_date: os_image.created.clone(),
                        phase: crate::utils::streaming_hash_calculator::ProcessingPhase::Metadata,
                        uncompressed_size: None,
                    };

                    // Add to downloads in progress for UI tracking
                    state.downloads_in_progress.push((os_image.version.clone(), 0.5)); // Start at 50% progress

                    // Get the image path for analysis
                    if let Some(ref image_path) = os_image.path {
                        let version_id = os_image.version.clone();
                        let cancel_token_clone = cancel_token.clone();
                        let version_id_1 = version_id.clone();
                        let version_id_2 = version_id.clone();
                        
                        return Task::sip(
                            {
                                use crate::utils::metadata_calculator::calculate_image_metadata;
                                use std::path::Path;

                                let path = Path::new(image_path);
                                let compressed_hash = os_image.sha256.clone();
                                
                                calculate_image_metadata(path, compressed_hash, cancel_token_clone)
                            },
                            move |progress| {
                                // Convert MetadataProgress to ProcessingProgress for consistency
                                use crate::utils::streaming_hash_calculator::ProcessingProgress;
                                
                                let processing_progress = match progress {
                                    crate::utils::metadata_calculator::MetadataProgress::Start => {
                                        ProcessingProgress::new_metadata(0.0, None, None, None)
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Processing { progress, .. } => {
                                        ProcessingProgress::new_metadata(progress, None, None, None)
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Completed { metadata } => {
                                        return crate::ui::messages::Message::Flash(FlashMessage::ProcessingCompleted(version_id_2.clone(), metadata));
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Failed { error } => {
                                        return crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(version_id_2.clone(), error));
                                    },
                                };
                                
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(version_id_2.clone(), processing_progress))
                            },
                            move |result| match result {
                                Ok(_) => {
                                    // The completion should have been handled by the progress handler
                                    crate::ui::messages::Message::Flash(FlashMessage::BackToSelectOsImage)
                                },
                                Err(e) => crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(version_id_1, format!("Metadata analysis failed: {}", e))),
                            }
                        );
                    } else {
                        // This should not happen for downloaded images, but handle gracefully
                        return Task::done(crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                            os_image.version.clone(),
                            "Image path not found for downloaded image".to_string(),
                        )));
                    }
                }
            }
            
            Task::none()
        }

        FlashMessage::DownloadOsImageFromGroup(group_index, version_index) => {
            debug!("Starting download for OS image from group {} at version {}", group_index, version_index);
            
            // Get the version information from the group
            if let Some(group) = state.os_image_groups.get(group_index) {
                let version = if version_index == 0 {
                    // Latest version (index 0)
                    Some(&group.latest_version)
                } else if let Some(older_version) = group.older_versions.get(version_index - 1) {
                    // Older version (index > 0)
                    Some(older_version)
                } else {
                    None
                };

                if let Some(os_image) = version {
                    // Create a Version struct that matches what ImageRepo expects
                    let repo_version = crate::utils::repo::Version {
                        id: os_image.version.clone(),
                        path: format!("golem-gpu-live-{}-{}.img.xz", group.channel_name, os_image.version),
                        sha256: os_image.sha256.clone(),
                        created: os_image.created.clone(),
                    };

                    // Start the download using ImageRepo
                    let repo_clone = Arc::clone(image_repo);
                    let cancel_token_clone = cancel_token.clone();
                    
                    let channel_name = group.channel_name.clone();
                    
                    // Change workflow state to processing
                    state.workflow_state = FlashWorkflowState::ProcessingImage {
                        version_id: os_image.version.clone(),
                        download_progress: 0.0,
                        metadata_progress: 0.0,
                        overall_progress: 0.0,
                        channel: channel_name.clone(),
                        created_date: os_image.created.clone(),
                        phase: crate::utils::streaming_hash_calculator::ProcessingPhase::Download,
                        uncompressed_size: None,
                    };
                    
                    // Add to downloads in progress
                    state.downloads_in_progress.push((os_image.version.clone(), 0.0));
                    
                    info!("Starting download for {} version {}", channel_name, os_image.version);
                    
                    let version_id_1 = os_image.version.clone();
                    let version_id_2 = os_image.version.clone();
                    
                    return Task::sip(
                        repo_clone.start_download(&channel_name, repo_version, cancel_token_clone),
                        move |status| match status {
                            crate::utils::repo::DownloadStatus::NotStarted => {
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(
                                    version_id_2.clone(), 
                                    crate::utils::streaming_hash_calculator::ProcessingProgress::new_download(0, 0)
                                ))
                            }
                            crate::utils::repo::DownloadStatus::Processing(progress) => {
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(
                                    version_id_2.clone(), 
                                    progress
                                ))
                            }
                            crate::utils::repo::DownloadStatus::Completed { metadata, .. } => {
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingCompleted(
                                    version_id_2.clone(), 
                                    metadata
                                ))
                            }
                            crate::utils::repo::DownloadStatus::Failed { error } => {
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                                    version_id_2.clone(), 
                                    error
                                ))
                            }
                        },
                        move |result| {
                            if let Err(e) = result {
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                                    version_id_1, 
                                    e.to_string()
                                ))
                            } else {
                                // This shouldn't happen in the new flow, but handle gracefully
                                crate::ui::messages::Message::Flash(FlashMessage::BackToSelectOsImage)
                            }
                        }
                    );
                }
            }
            
            Task::none()
        }

        FlashMessage::AnalyzeOsImageFromGroup(group_index, version_index) => {
            debug!("Starting analysis for OS image from group {} at version {}", group_index, version_index);
            
            if let Some(group) = state.os_image_groups.get(group_index) {
                let image = if version_index == 0 {
                    &group.latest_version
                } else if let Some(older_image) = group.older_versions.get(version_index - 1) {
                    older_image
                } else {
                    return Task::none();
                };

                if image.downloaded && image.metadata.is_none() {
                    state.selected_os_image_group = Some((group_index, version_index));
                    // Also try to set legacy selection for backward compatibility
                    if let Some(legacy_index) = state.os_images.iter().position(|img| {
                        img.name == image.name && img.version == image.version
                    }) {
                        state.selected_os_image = Some(legacy_index);
                    }
                    
                    info!("Starting metadata analysis for existing image: {}", image.version);

                    // Set state to processing with metadata phase (skip download)
                    state.workflow_state = FlashWorkflowState::ProcessingImage {
                        version_id: image.version.clone(),
                        download_progress: 1.0, // Download already complete
                        metadata_progress: 0.0,
                        overall_progress: 0.5,  // Start at 50% (download phase done)
                        channel: image.name.clone(),
                        created_date: image.created.clone(),
                        phase: crate::utils::streaming_hash_calculator::ProcessingPhase::Metadata,
                        uncompressed_size: None,
                    };

                    // Add to downloads in progress for UI tracking
                    state.downloads_in_progress.push((image.version.clone(), 0.5)); // Start at 50% progress

                    // Get the image path for analysis
                    if let Some(ref image_path) = image.path {
                        let version_id = image.version.clone();
                        let cancel_token_clone = cancel_token.clone();
                        let version_id_1 = version_id.clone();
                        let version_id_2 = version_id.clone();
                        
                        return Task::sip(
                            {
                                use crate::utils::metadata_calculator::calculate_image_metadata;
                                use std::path::Path;

                                let path = Path::new(image_path);
                                let compressed_hash = image.sha256.clone();
                                
                                calculate_image_metadata(path, compressed_hash, cancel_token_clone)
                            },
                            move |progress| {
                                // Convert MetadataProgress to ProcessingProgress for consistency
                                use crate::utils::streaming_hash_calculator::ProcessingProgress;
                                
                                let processing_progress = match progress {
                                    crate::utils::metadata_calculator::MetadataProgress::Start => {
                                        ProcessingProgress::new_metadata(0.0, None, None, None)
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Processing { progress, .. } => {
                                        ProcessingProgress::new_metadata(progress, None, None, None)
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Completed { metadata } => {
                                        return crate::ui::messages::Message::Flash(FlashMessage::ProcessingCompleted(version_id_2.clone(), metadata));
                                    },
                                    crate::utils::metadata_calculator::MetadataProgress::Failed { error } => {
                                        return crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(version_id_2.clone(), error));
                                    },
                                };
                                
                                crate::ui::messages::Message::Flash(FlashMessage::ProcessingProgress(version_id_2.clone(), processing_progress))
                            },
                            move |result| match result {
                                Ok(_) => {
                                    // The completion should have been handled by the progress handler
                                    crate::ui::messages::Message::Flash(FlashMessage::BackToSelectOsImage)
                                },
                                Err(e) => crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(version_id_1, format!("Metadata analysis failed: {}", e))),
                            }
                        );
                    } else {
                        // This should not happen for downloaded images, but handle gracefully
                        return Task::done(crate::ui::messages::Message::Flash(FlashMessage::ProcessingFailed(
                            image.version.clone(),
                            "Image path not found for downloaded image".to_string(),
                        )));
                    }
                }
            }
            
            Task::none()
        }

        FlashMessage::WriteImage => {
            debug!("Starting image write process");
            
            // Make sure we have both an image and device selected
            let selected_image_option = if let Some(image_idx) = state.selected_os_image {
                state.os_images.get(image_idx).cloned()
            } else if let Some((group_idx, version_idx)) = state.selected_os_image_group {
                if let Some(group) = state.os_image_groups.get(group_idx) {
                    if version_idx == 0 {
                        Some(group.latest_version.clone())
                    } else if let Some(older_image) = group.older_versions.get(version_idx - 1) {
                        Some(older_image.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            
            if selected_image_option.is_none() {
                error!("No OS image selected for writing");
                return Task::done(crate::ui::messages::Message::ShowError("No OS image selected for writing".to_string()));
            }

            if state.selected_device.is_none() {
                error!("No target device selected for writing");
                return Task::done(crate::ui::messages::Message::ShowError("No target device selected for writing".to_string()));
            }

            // Extract needed data from the current workflow state
            let config_data = if let FlashWorkflowState::ConfigureSettings {
                payment_network,
                subnet,
                network_type,
                wallet_address,
                is_wallet_valid,
            } = &state.workflow_state
            {
                // Check if wallet address is valid before proceeding
                if !wallet_address.is_empty() && !*is_wallet_valid {
                    warn!("Cannot proceed, wallet address is invalid: {}", wallet_address);
                    return Task::done(crate::ui::messages::Message::ShowError("Invalid wallet address".to_string()));
                }

                // Collect the data we need for the task
                Some((
                    *payment_network,
                    *network_type,
                    subnet.clone(),
                    wallet_address.clone(),
                ))
            } else {
                None
            };

            // Only proceed if we have valid configuration data
            if let Some((
                payment_network_val,
                network_type_val,
                subnet_val,
                wallet_address_val,
            )) = config_data
            {
                // Get the selected OS image and device
                if let (Some(image), Some(device_idx)) = (selected_image_option, state.selected_device) {
                    if let Some(device) = device_selection.devices.get(device_idx) {
                        // Make sure the image is downloaded
                        if let Some(image_path) = &image.path {
                            // Start the write process with initial 0% progress for image writing
                            state.workflow_state = FlashWorkflowState::WritingImage(0.0);

                            // Get device path, image path, and metadata
                            let device_path = device.path.clone();
                            let image_path_val = image_path.clone();
                            let image_metadata = image.metadata.clone();
                            // Create a clone of the cancel token that we can pass to the task
                            let cancel_token_clone = cancel_token.clone();

                            // Extract configuration before creating async closure
                            let config = Some(crate::disk::ImageConfiguration::new(
                                payment_network_val,
                                network_type_val,
                                subnet_val.clone(),
                                wallet_address_val.clone(),
                            ));

                            info!(
                                "Starting flash with config: {:?} {:?} {} {} to device {}",
                                payment_network_val,
                                network_type_val,
                                subnet_val,
                                wallet_address_val,
                                device_path
                            );

                            // Pass the cancel token clone into the future task
                            return Task::future(async move {
                                info!("Starting disk image write to {}", device_path);
                                // Store the device_path for use throughout the process
                                // When writing an image, set edit_mode to false to allow disk cleaning
                                let locked_disk = Disk::lock_path(&device_path, false).await;
                                // Log whether we successfully locked the disk
                                match &locked_disk {
                                    Ok(_) => info!("Successfully locked disk: {}", device_path),
                                    Err(e) => {
                                        error!("Failed to lock disk {}: {}", device_path, e)
                                    }
                                }
                                locked_disk
                            })
                            .and_then(move |disk| {
                                // Now write the image and handle progress
                                // Note: write_image now takes ownership of disk
                                // Clone the cancel token again for this specific closure
                                let task_cancel_token = cancel_token_clone.clone();

                                let write_task = match &image_metadata {
                                    Some(metadata) => Task::sip(
                                        disk.write_image(
                                            &image_path_val,
                                            metadata.clone(),
                                            task_cancel_token,
                                            config.clone(),
                                        ),
                                    |message| match message {
                                        WriteProgress::Start => {
                                            crate::ui::messages::Message::Flash(FlashMessage::WriteImageProgress(0.0))
                                        }
                                        WriteProgress::ClearingPartitions { progress } => {
                                            crate::ui::messages::Message::Flash(FlashMessage::ClearPartitionsProgress(progress))
                                        }
                                        WriteProgress::Write {
                                            total_written,
                                            total_size,
                                        } => {
                                            // Calculate progress based on actual metadata size or fallback to 16GB
                                            let size_for_calculation = if total_size > 0 {
                                                total_size as f32
                                            } else {
                                                16.0 * 1024.0 * 1024.0 * 1024.0 // 16GB fallback
                                            };

                                            // Calculate progress percentage (0.0-1.0)
                                            let progress =
                                                total_written as f32 / size_for_calculation;

                                            // Clamp to make sure we don't go over 100%
                                            let clamped_progress = progress.min(1.0);

                                            crate::ui::messages::Message::Flash(FlashMessage::WriteImageProgress(clamped_progress))
                                        }
                                        WriteProgress::Verifying {
                                            verified_bytes,
                                            total_size,
                                        } => {
                                            // Calculate verification progress (0.0-1.0)
                                            let progress = if total_size > 0 {
                                                verified_bytes as f32 / total_size as f32
                                            } else {
                                                0.0
                                            };

                                            // Use a separate message for verification progress
                                            crate::ui::messages::Message::Flash(FlashMessage::VerificationProgress(progress.min(1.0)))
                                        }
                                        WriteProgress::Finish => {
                                            crate::ui::messages::Message::Flash(FlashMessage::WriteImageProgress(1.0))
                                        }
                                    },
                                    |result| match result {
                                        Ok(WriteProgress::Finish) => {
                                            // When image writing is complete, we'll need to reacquire the disk
                                            // because write_image now consumes the disk
                                            crate::ui::messages::Message::Flash(FlashMessage::WriteImageCompleted)
                                        }
                                        Ok(_) => {
                                            crate::ui::messages::Message::Flash(FlashMessage::WriteImageCompleted)
                                        }
                                        Err(e) => crate::ui::messages::Message::Flash(FlashMessage::WriteImageFailed(format!("{:?}", e))),
                                    },
                                ),
                                None => {
                                    // This should never happen in practice, but handle gracefully
                                    Task::done(crate::ui::messages::Message::Flash(FlashMessage::WriteImageFailed("Image metadata is required for writing".to_string())))
                                }
                            };

                                write_task
                            });
                        } else {
                            // Image not downloaded
                            error!("Cannot write - image not downloaded: {}", image.name);
                            state.workflow_state = FlashWorkflowState::Completion(false);
                            Task::done(crate::ui::messages::Message::ShowError("Image not downloaded".to_string()))
                        }
                    } else {
                        // Invalid device index
                        error!("Invalid device index");
                        state.workflow_state = FlashWorkflowState::Completion(false);
                        Task::done(crate::ui::messages::Message::ShowError("Invalid device index".to_string()))
                    }
                } else {
                    // No image or device selected
                    error!("No OS image or device selected");
                    state.workflow_state = FlashWorkflowState::Completion(false);
                    Task::done(crate::ui::messages::Message::ShowError("No OS image or device selected".to_string()))
                }
            } else {
                // No configuration data
                error!("No configuration data available");
                state.workflow_state = FlashWorkflowState::Completion(false);
                Task::done(crate::ui::messages::Message::ShowError("No configuration data available".to_string()))
            }
        }
        
        FlashMessage::ClearPartitionsCompleted => {
            debug!("Partition clearing completed, starting image write");
            state.workflow_state = FlashWorkflowState::WritingImage(0.0);
            Task::none()
        }
        
        FlashMessage::WriteImageCompleted => {
            // Reset the cancel token for future operations
            debug!("Image writing completed, flashing successful");
            state.workflow_state = FlashWorkflowState::Completion(true);
            Task::none()
        }
        
        FlashMessage::WriteConfigCompleted => {
            debug!("Configuration writing completed, flashing successful");
            state.workflow_state = FlashWorkflowState::Completion(true);
            Task::none()
        }
        
        FlashMessage::ClearPartitionsFailed(error) => {
            error!("Partition clearing failed: {}", error);
            state.workflow_state = FlashWorkflowState::Completion(false);
            Task::done(crate::ui::messages::Message::ShowError(format!("Failed to clear partitions: {}", error)))
        }
        
        FlashMessage::WriteImageFailed(error) => {
            error!("Image writing failed: {}", error);
            state.workflow_state = FlashWorkflowState::Completion(false);
            Task::done(crate::ui::messages::Message::ShowError(format!("Failed to write image: {}", error)))
        }
        
        FlashMessage::WriteConfigFailed(error) => {
            error!("Configuration writing failed: {}", error);
            state.workflow_state = FlashWorkflowState::Completion(false);
            Task::done(crate::ui::messages::Message::ShowError(format!("Failed to write configuration: {}", error)))
        }
        
        FlashMessage::ClearPartitionsProgress(progress) => {
            if let FlashWorkflowState::ClearingPartitions(_) = &mut state.workflow_state {
                debug!("Partition clearing progress: {:.1}%", progress * 100.0);
                state.workflow_state = FlashWorkflowState::ClearingPartitions(progress);
            }
            Task::none()
        }
        
        FlashMessage::WriteImageProgress(progress) => {
            if let FlashWorkflowState::WritingImage(_) = &mut state.workflow_state {
                debug!("Image write progress: {:.1}%", progress * 100.0);
                state.workflow_state = FlashWorkflowState::WritingImage(progress);
            }
            Task::none()
        }
        
        FlashMessage::VerificationProgress(progress) => {
            match &mut state.workflow_state {
                FlashWorkflowState::WritingImage(_) => {
                    // When we receive verification progress, transition to verifying state
                    debug!("Verification progress: {:.1}%", progress * 100.0);
                    state.workflow_state = FlashWorkflowState::VerifyingImage(progress);
                }
                FlashWorkflowState::VerifyingImage(_) => {
                    // Update verification progress
                    debug!("Verification progress: {:.1}%", progress * 100.0);
                    state.workflow_state = FlashWorkflowState::VerifyingImage(progress);
                }
                _ => {}
            }
            Task::none()
        }
        
        FlashMessage::WriteConfigProgress(progress) => {
            if let FlashWorkflowState::WritingConfig(_) = &mut state.workflow_state {
                debug!("Config write progress: {:.1}%", progress * 100.0);
                state.workflow_state = FlashWorkflowState::WritingConfig(progress);
            }
            Task::none()
        }

        // Add more message handlers as needed
        _ => {
            debug!("Unhandled flash message: {:?}", message);
            Task::none()
        }
    }
}