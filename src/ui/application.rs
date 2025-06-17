use crate::models::AppMode;
use crate::ui::{
    messages::Message,
    flash_workflow::{FlashState, FlashMessage},
    edit_workflow::EditState,
    preset_manager::PresetManagerState,
    device_selection::DeviceSelectionState,
    configuration::ConfigurationState,
};
use crate::utils::repo::ImageRepo;
use crate::utils::{PresetManager, image_metadata::MetadataManager};
use iced::{Element, Task};
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct GolemGpuImager {
    pub mode: AppMode,
    
    // Module states
    pub flash_workflow: Option<FlashState>,
    pub edit_workflow: Option<EditState>,
    pub preset_manager: PresetManagerState,
    pub device_selection: DeviceSelectionState,
    pub configuration: ConfigurationState,
    
    // Shared resources
    pub image_repo: Arc<ImageRepo>,
    pub elevation_status: String,
    pub is_elevated: bool,
    pub metadata_manager: Option<MetadataManager>,
    pub preset_manager_backend: Option<PresetManager>,
    pub is_loading_repo: bool,
    pub error_message: Option<String>,
}

impl GolemGpuImager {
    pub fn new() -> Self {
        let image_repo = Arc::new(ImageRepo::new());

        // Initialize the PresetManager backend
        let preset_manager_backend = match PresetManager::new() {
            Ok(mut manager) => {
                let _ = manager.init_with_defaults();
                Some(manager)
            }
            Err(e) => {
                error!("Failed to initialize preset manager: {}", e);
                None
            }
        };

        // Initialize preset manager state with defaults or from backend
        let preset_manager_state = match &preset_manager_backend {
            Some(manager) => {
                let mut state = PresetManagerState::new();
                state.presets = manager.get_presets().clone();
                state
            }
            None => PresetManagerState::with_defaults(),
        };

        let elevation_status = crate::utils::get_elevation_status();
        let is_elevated = crate::utils::is_elevated();

        // Initialize the MetadataManager
        let metadata_manager = match MetadataManager::new() {
            Ok(manager) => {
                info!("Successfully initialized metadata manager");
                Some(manager)
            }
            Err(e) => {
                error!("Failed to initialize metadata manager: {}", e);
                None
            }
        };

        Self {
            mode: AppMode::StartScreen,
            flash_workflow: None,
            edit_workflow: None,
            preset_manager: preset_manager_state,
            device_selection: DeviceSelectionState::new(),
            configuration: ConfigurationState::new(),
            image_repo,
            elevation_status,
            is_elevated,
            metadata_manager,
            preset_manager_backend,
            is_loading_repo: false,
            error_message: None,
        }
    }
}

impl GolemGpuImager {
    pub fn title(&self) -> String {
        format!("Golem GPU Imager v{}", env!("CARGO_PKG_VERSION"))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // App-level messages
            Message::FlashNewImage => {
                self.mode = AppMode::FlashNewImage;
                self.flash_workflow = Some(FlashState::new());
                
                // Load repository data and refresh devices for flash workflow
                Task::batch([
                    self.load_repo_data(),
                    Task::done(Message::DeviceSelection(
                        crate::ui::device_selection::DeviceMessage::RefreshDevices
                    ))
                ])
            }
            
            Message::EditExistingDisk => {
                self.mode = AppMode::EditExistingDisk;
                self.edit_workflow = Some(EditState::new());
                
                debug!("Entering edit existing disk mode - delegating device enumeration to DeviceSelection module");
                
                // Delegate device enumeration to the shared DeviceSelection module
                Task::done(Message::DeviceSelection(
                    crate::ui::device_selection::DeviceMessage::RefreshDevices
                ))
            }
            
            Message::ManagePresets => {
                self.mode = AppMode::ManagePresets;
                self.preset_manager.show_manager = true;
                Task::none()
            }
            
            Message::BackToMainMenu => {
                self.mode = AppMode::StartScreen;
                self.flash_workflow = None;
                self.edit_workflow = None;
                self.preset_manager.show_manager = false;
                self.preset_manager.editor = None;
                Task::none()
            }
            
            Message::Exit => {
                std::process::exit(0);
            }
            
            Message::ShowError(error) => {
                self.error_message = Some(error);
                Task::none()
            }
            
            // Repository management
            Message::RefreshRepoData => {
                self.load_repo_data()
            }
            
            Message::RepoDataLoaded(images) => {
                if let Some(flash_state) = &mut self.flash_workflow {
                    flash_state.os_images = images;
                }
                self.is_loading_repo = false;
                Task::none()
            }
            
            Message::RepoGroupDataLoaded(images, groups) => {
                if let Some(flash_state) = &mut self.flash_workflow {
                    flash_state.os_images = images;
                    flash_state.os_image_groups = groups;
                }
                self.is_loading_repo = false;
                Task::none()
            }
            
            Message::RepoLoadFailed => {
                self.is_loading_repo = false;
                self.error_message = Some("Failed to load repository data".to_string());
                Task::none()
            }
            
            // Delegate module-specific messages
            Message::Flash(flash_msg) => {
                if let Some(flash_state) = &mut self.flash_workflow {
                    crate::ui::flash_workflow::handler::handle_message(
                        flash_state,
                        &self.image_repo,
                        &self.device_selection,
                        flash_msg,
                    )
                } else {
                    Task::none()
                }
            }
            
            Message::Edit(edit_msg) => {
                if let Some(edit_state) = &mut self.edit_workflow {
                    crate::ui::edit_workflow::handler::handle_message(edit_state, &self.device_selection, edit_msg)
                } else {
                    Task::none()
                }
            }
            
            Message::PresetManager(preset_msg) => {
                crate::ui::preset_manager::handler::handle_message(
                    &mut self.preset_manager,
                    &mut self.preset_manager_backend,
                    preset_msg,
                )
            }
            
            Message::DeviceSelection(device_msg) => {
                crate::ui::device_selection::handler::handle_message(
                    &mut self.device_selection,
                    device_msg,
                )
            }
            
            Message::Configuration(config_msg) => {
                crate::ui::configuration::handler::handle_message(
                    &mut self.configuration,
                    &self.preset_manager.presets,
                    config_msg,
                )
            }
            
            // Elevation management
            Message::RequestElevation => {
                #[cfg(windows)]
                {
                    if let Err(e) = crate::utils::request_elevation() {
                        self.error_message = Some(format!("Failed to request elevation: {}", e));
                        error!("Failed to request elevation: {}", e);
                    }
                }
                #[cfg(not(windows))]
                {
                    self.error_message = Some("Elevation request is only supported on Windows. Please run with sudo on Unix systems.".to_string());
                }
                Task::none()
            }
            
            Message::CheckElevationStatus => {
                self.elevation_status = crate::utils::get_elevation_status();
                self.is_elevated = crate::utils::is_elevated();
                Task::none()
            }
            
            // Preset management messages - TODO: Implement properly
            Message::SaveAsPreset => {
                // TODO: Save current configuration as preset
                Task::none()
            }
            
            Message::SelectPreset(index) => {
                // TODO: Apply preset configuration to current workflow
                Task::none()
            }
            
            Message::DeletePreset(index) => {
                // TODO: Delete preset by index
                Task::none()
            }
            
            Message::SetDefaultPreset(index) => {
                // TODO: Set preset as default
                Task::none()
            }
            
            Message::SetPresetName(name) => {
                // TODO: Set new preset name
                Task::none()
            }
            
            // Configuration settings - delegate to current workflow
            Message::SetPaymentNetwork(network) => {
                match &mut self.flash_workflow {
                    Some(flash_state) => {
                        crate::ui::flash_workflow::handler::handle_message(
                            flash_state,
                            &self.image_repo,
                            &self.device_selection,
                            FlashMessage::SetPaymentNetwork(network),
                        )
                    }
                    None => Task::none()
                }
            }
            
            Message::SetNetworkType(network_type) => {
                match &mut self.flash_workflow {
                    Some(flash_state) => {
                        crate::ui::flash_workflow::handler::handle_message(
                            flash_state,
                            &self.image_repo,
                            &self.device_selection,
                            FlashMessage::SetNetworkType(network_type),
                        )
                    }
                    None => Task::none()
                }
            }
            
            Message::SetSubnet(subnet) => {
                match &mut self.flash_workflow {
                    Some(flash_state) => {
                        crate::ui::flash_workflow::handler::handle_message(
                            flash_state,
                            &self.image_repo,
                            &self.device_selection,
                            FlashMessage::SetSubnet(subnet),
                        )
                    }
                    None => Task::none()
                }
            }
            
            Message::SetWalletAddress(address) => {
                match &mut self.flash_workflow {
                    Some(flash_state) => {
                        crate::ui::flash_workflow::handler::handle_message(
                            flash_state,
                            &self.image_repo,
                            &self.device_selection,
                            FlashMessage::SetWalletAddress(address),
                        )
                    }
                    None => Task::none()
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.mode {
            AppMode::StartScreen => {
                crate::ui::start_screen::view_start_screen(
                    self.error_message.as_deref(),
                    self.is_elevated,
                    &self.elevation_status,
                )
            }
            AppMode::FlashNewImage => {
                if let Some(flash_state) = &self.flash_workflow {
                    crate::ui::flash_workflow::view(flash_state, &self.device_selection, self.is_loading_repo).map(Message::Flash)
                } else {
                    crate::ui::start_screen::view_start_screen(
                        self.error_message.as_deref(),
                        self.is_elevated,
                        &self.elevation_status,
                    )
                }
            }
            AppMode::EditExistingDisk => {
                if let Some(edit_state) = &self.edit_workflow {
                    crate::ui::edit_workflow::view(edit_state, &self.device_selection, &self.preset_manager)
                } else {
                    crate::ui::start_screen::view_start_screen(
                        self.error_message.as_deref(),
                        self.is_elevated,
                        &self.elevation_status,
                    )
                }
            }
            AppMode::ManagePresets => {
                crate::ui::preset_manager::view(&self.preset_manager).map(Message::PresetManager)
            }
        }
    }

    pub fn load_repo_data(&mut self) -> Task<Message> {
        self.is_loading_repo = true;
        
        let repo = Arc::clone(&self.image_repo);
        let metadata_manager = self.metadata_manager.clone();

        Task::perform(
            async move {
                match repo.fetch_metadata().await {
                    Ok(metadata) => {
                        // Helper function to load metadata for downloaded images only
                        let load_metadata_for_image = |sha256: &str| -> Option<crate::models::ImageMetadata> {
                            if let Some(ref manager) = metadata_manager {
                                debug!("Attempting to load metadata for SHA256: {}", sha256);
                                match manager.load_metadata(sha256) {
                                    Ok(Some(metadata)) => {
                                        info!("Successfully loaded metadata for SHA256: {} (uncompressed size: {} bytes)", 
                                            sha256, metadata.uncompressed_size);
                                        Some(metadata)
                                    },
                                    Ok(None) => {
                                        debug!("No metadata found for SHA256: {}", sha256);
                                        None
                                    },
                                    Err(e) => {
                                        error!("Failed to load metadata for SHA256 {}: {}", sha256, e);
                                        None
                                    }
                                }
                            } else {
                                error!("MetadataManager not available");
                                None
                            }
                        };
                        
                        // Convert repository metadata to UI structures
                        let mut os_images = Vec::new();
                        let mut os_image_groups = Vec::new();
                        
                        for channel in &metadata.channels {
                            if channel.versions.is_empty() {
                                continue;
                            }
                            
                            // Create OsImageGroup for this channel
                            // Find the actual latest version by creation date (most recent)
                            let latest_version = channel.versions.iter()
                                .max_by(|a, b| a.created.cmp(&b.created))
                                .unwrap_or(&channel.versions[0]);
                            let is_downloaded = repo.is_image_downloaded(latest_version);
                            debug!("Channel '{}' latest version '{}': downloaded={}, path={}", 
                                channel.name, latest_version.id, is_downloaded, latest_version.path);
                            let image_path = if is_downloaded {
                                let path = repo.get_image_path(latest_version).to_string_lossy().to_string();
                                debug!("Image path for downloaded version '{}': {}", latest_version.id, path);
                                Some(path)
                            } else {
                                None
                            };
                            
                            let latest_os_image = crate::ui::flash_workflow::OsImage {
                                name: channel.name.clone(),
                                version: latest_version.id.clone(),
                                description: format!("Latest {} release", channel.name),
                                downloaded: is_downloaded,
                                path: image_path,
                                created: latest_version.created.clone(),
                                sha256: latest_version.sha256.clone(),
                                is_latest: true,
                                metadata: load_metadata_for_image(&latest_version.sha256)
                            };
                            
                            // Create older versions (exclude the latest and sort by creation date, newest first)
                            let mut older_versions_sorted = channel.versions.iter()
                                .filter(|v| v.id != latest_version.id)
                                .collect::<Vec<_>>();
                            older_versions_sorted.sort_by(|a, b| b.created.cmp(&a.created));
                            
                            let older_versions: Vec<crate::ui::flash_workflow::OsImage> = older_versions_sorted.iter().map(|version| {
                                let is_downloaded = repo.is_image_downloaded(version);
                                let image_path = if is_downloaded {
                                    Some(repo.get_image_path(version).to_string_lossy().to_string())
                                } else {
                                    None
                                };
                                
                                crate::ui::flash_workflow::OsImage {
                                    name: channel.name.clone(),
                                    version: version.id.clone(),
                                    description: format!("{} version {}", channel.name, version.id),
                                    downloaded: is_downloaded,
                                    path: image_path,
                                    created: version.created.clone(),
                                    sha256: version.sha256.clone(),
                                    is_latest: false,
                                    metadata: load_metadata_for_image(&version.sha256)
                                }
                            }).collect();
                            
                            let group = crate::ui::flash_workflow::OsImageGroup {
                                channel_name: channel.name.clone(),
                                description: format!("{} channel images", channel.name),
                                latest_version: latest_os_image.clone(),
                                older_versions,
                                expanded: false,
                            };
                            
                            os_image_groups.push(group);
                            
                            // Also add individual images for flat list view (sorted by creation date, newest first)
                            let mut sorted_versions = channel.versions.clone();
                            sorted_versions.sort_by(|a, b| b.created.cmp(&a.created));
                            
                            for version in &sorted_versions {
                                let is_downloaded = repo.is_image_downloaded(version);
                                let image_path = if is_downloaded {
                                    Some(repo.get_image_path(version).to_string_lossy().to_string())
                                } else {
                                    None
                                };
                                
                                os_images.push(crate::ui::flash_workflow::OsImage {
                                    name: format!("{} {}", channel.name, version.id),
                                    version: version.id.clone(),
                                    description: format!("{} version {}", channel.name, version.id),
                                    downloaded: is_downloaded,
                                    path: image_path,
                                    created: version.created.clone(),
                                    sha256: version.sha256.clone(),
                                    is_latest: version == latest_version,
                                    metadata: load_metadata_for_image(&version.sha256)
                                });
                            }
                        }
                        
                        (os_images, os_image_groups)
                    }
                    Err(_) => {
                        // Return empty data on error
                        (vec![], vec![])
                    }
                }
            },
            |(images, groups)| Message::RepoGroupDataLoaded(images, groups)
        )
    }
}