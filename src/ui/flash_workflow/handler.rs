use super::{FlashState, FlashMessage, FlashWorkflowState};
use crate::models::CancelToken;
use crate::utils::repo::ImageRepo;
use iced::Task;
use std::sync::Arc;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut FlashState,
    image_repo: &Arc<ImageRepo>,
    cancel_token: &CancelToken,
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

        // Add more message handlers as needed
        _ => {
            debug!("Unhandled flash message: {:?}", message);
            Task::none()
        }
    }
}