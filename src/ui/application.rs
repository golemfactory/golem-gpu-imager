use crate::models::{
    AppMode, ConfigurationPreset, EditState, FlashState, Message, NetworkType, OsImage,
    PaymentNetwork, StorageDevice,
};
use crate::ui;
use crate::utils::disks::WriteProgress;
use crate::utils::repo::{DownloadStatus, ImageRepo, Version};
use crate::utils::{Disk, PresetManager};
use anyhow::anyhow;
use futures_util::TryStreamExt;
use iced::{Alignment, Element, Length, Task};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct GolemGpuImager {
    pub mode: AppMode,
    pub os_images: Vec<OsImage>,
    pub storage_devices: Vec<StorageDevice>,
    pub selected_os_image: Option<usize>,
    pub selected_device: Option<usize>,
    pub image_repo: Arc<ImageRepo>,
    pub is_loading_repo: bool,
    pub downloads_in_progress: Vec<(String, f32)>, // (version_id, progress)
    pub configuration_presets: Vec<ConfigurationPreset>,
    pub selected_preset: Option<usize>,
    pub new_preset_name: String,
    pub show_preset_manager: bool,
    pub preset_manager: Option<PresetManager>,
    pub locked_disk: Option<Disk>,
    pub error_message: Option<String>,
}

impl GolemGpuImager {
    pub fn new() -> Self {
        let image_repo = Arc::new(ImageRepo::new());

        // Initialize the PresetManager
        let preset_manager = match PresetManager::new() {
            Ok(mut manager) => {
                // Initialize with default presets if needed
                let _ = manager.init_with_defaults();
                Some(manager)
            }
            Err(e) => {
                error!("Failed to initialize preset manager: {}", e);
                None
            }
        };

        // Get presets from the preset manager, or use defaults if manager initialization failed
        let configuration_presets = match &preset_manager {
            Some(manager) => manager.get_presets().clone(),
            None => {
                // Fallback to hardcoded defaults if preset manager failed
                vec![
                    ConfigurationPreset {
                        name: "Testnet Development".to_string(),
                        payment_network: PaymentNetwork::Testnet,
                        subnet: "public".to_string(),
                        network_type: NetworkType::Central,
                        wallet_address: "".to_string(),
                        is_default: true,
                    },
                    ConfigurationPreset {
                        name: "Mainnet Production".to_string(),
                        payment_network: PaymentNetwork::Mainnet,
                        subnet: "public".to_string(),
                        network_type: NetworkType::Central,
                        wallet_address: "".to_string(),
                        is_default: false,
                    },
                ]
            }
        };

        Self {
            mode: AppMode::StartScreen,
            os_images: vec![],       // Will be populated from repo
            storage_devices: vec![], // Will be populated when needed
            selected_os_image: None,
            selected_device: None,
            image_repo,
            is_loading_repo: false,
            downloads_in_progress: Vec::new(),
            configuration_presets,
            selected_preset: None,
            new_preset_name: String::new(),
            show_preset_manager: false,
            preset_manager,
            locked_disk: None,
            error_message: None,
        }
    }

    pub fn load_repo_data(&mut self) -> Task<Message> {
        self.is_loading_repo = true;

        let _repo = Arc::clone(&self.image_repo);

        Task::perform(
            async move {
                let mut repo_instance = ImageRepo::new(); // Create a mutable instance to fetch data

                // First fetch the metadata
                let metadata_result = repo_instance.fetch_metadata().await;

                if let Ok(metadata) = metadata_result {
                    // Clone the metadata to avoid borrow issues
                    let metadata_cloned = metadata.clone();

                    // Convert repo data to OsImage format
                    let mut os_images = Vec::new();

                    for channel in &metadata_cloned.channels {
                        if let Some(newest) = channel
                            .versions
                            .iter()
                            .max_by(|a, b| a.created.cmp(&b.created))
                        {
                            let description = match channel.name.as_str() {
                                "release" => "Stable release version",
                                "testing" => "Testing version with latest features",
                                "unstable" => "Development version with latest changes",
                                "susteen" => "Enterprise support version",
                                _ => "GPU OS version",
                            };

                            // Check if the image is downloaded
                            let downloaded = repo_instance.is_image_downloaded(newest);

                            // Get path if downloaded
                            let path_str = if downloaded {
                                Some(
                                    repo_instance
                                        .get_image_path(newest)
                                        .to_string_lossy()
                                        .to_string(),
                                )
                            } else {
                                None
                            };

                            os_images.push(OsImage {
                                name: channel.name.clone(),
                                version: newest.id.clone(),
                                description: description.to_string(),
                                downloaded,
                                path: path_str,
                                created: newest.created.clone(),
                                sha256: newest.sha256.clone(),
                            });
                        }
                    }

                    return Some(os_images);
                }

                None
            },
            |result| match result {
                Some(os_images) => Message::RepoDataLoaded(os_images),
                None => Message::RepoLoadFailed,
            },
        )
    }

    pub fn title(&self) -> String {
        String::from("Golem GPU Imager")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FlashNewImage => {
                self.mode = AppMode::FlashNewImage(FlashState::SelectOsImage);
                self.selected_os_image = None;
                self.selected_device = None;

                // Load repository data if we haven't yet
                if self.os_images.is_empty() && !self.is_loading_repo {
                    return self.load_repo_data();
                }
            }
            Message::EditExistingDisk => {
                self.mode = AppMode::EditExistingDisk(EditState::SelectDevice);
                self.selected_device = None;
                // Clear any previous error messages
                self.error_message = None;

                // Get list of removable storage devices
                info!("Getting available storage devices");
                match rs_drivelist::drive_list() {
                    Ok(devices) => {
                        // Filter to only include removable, non-virtual devices
                        self.storage_devices = devices
                            .into_iter()
                            .filter(|d| d.isRemovable && !d.isVirtual)
                            .map(|d| StorageDevice {
                                name: d.description,
                                path: d.device,
                                size: format!("{:.2} GB", d.size as f64 / 1000.0 / 1000.0 / 1000.0),
                            })
                            .collect();

                        debug!("Found {} available devices", self.storage_devices.len());
                    }
                    Err(e) => {
                        error!("Failed to get drive list: {}", e);
                        // In case of error, provide an empty list
                        self.storage_devices = vec![];
                        self.error_message =
                            Some(format!("Failed to detect storage devices: {}", e));
                    }
                }
            }
            Message::SelectOsImage(index) => {
                self.selected_os_image = Some(index);
            }
            Message::DownloadOsImage(index) => {
                if let Some(image) = self.os_images.get(index) {
                    self.selected_os_image = Some(index);

                    // Set state to downloading image with progress display
                    self.mode = AppMode::FlashNewImage(FlashState::DownloadingImage {
                        version_id: image.version.clone(),
                        progress: 0.0,
                        channel: image.name.clone(),
                        created_date: image.created.clone(),
                    });

                    // Add to downloads in progress
                    self.downloads_in_progress
                        .push((image.version.clone(), 0.0));

                    // Start actual download
                    let version_id = image.version.clone();
                    let channel_name = image.name.clone();
                    let repo: Arc<ImageRepo> = Arc::clone(&self.image_repo);

                    // Create Version struct for download
                    let version = Version {
                        id: version_id.clone(),
                        path: format!("golem-gpu-live-{}-{}.img.xz", channel_name, version_id),
                        sha256: image.sha256.clone(),
                        created: image.created.clone(),
                    };

                    // Create a task that sips from the download straw
                    let version_id_1 = version_id.clone();
                    let version_id_2 = version_id.clone();
                    return Task::sip(
                        repo.start_download(&channel_name, version),
                        move |status| match status {
                            DownloadStatus::NotStarted { .. } => {
                                Message::DownloadProgress(version_id_2.clone(), 0f32)
                            }
                            DownloadStatus::InProgress {
                                progress,
                                bytes_downloaded,
                                total_bytes,
                            } => Message::DownloadProgress(version_id_2.clone(), progress),
                            DownloadStatus::Completed { path } => {
                                Message::DownloadCompleted(version_id_2.clone())
                            }
                            DownloadStatus::Failed { error } => {
                                Message::DownloadFailed(version_id_2.clone(), error)
                            }
                        },
                        move |done| {
                            if let Err(e) = done {
                                Message::DownloadFailed(version_id_1, "Download failed".to_string())
                            } else {
                                Message::DownloadCompleted(version_id_1)
                            }
                        },
                    );
                }
            }
            Message::DownloadProgress(version_id, progress) => {
                // Update progress in downloads list
                if let Some(index) = self
                    .downloads_in_progress
                    .iter()
                    .position(|(id, _)| id == &version_id)
                {
                    self.downloads_in_progress[index].1 = progress;
                }

                // Update UI if we're in downloading state with this version
                if let AppMode::FlashNewImage(FlashState::DownloadingImage {
                    version_id: current_id,
                    channel,
                    created_date,
                    ..
                }) = &self.mode
                {
                    if current_id == &version_id {
                        self.mode = AppMode::FlashNewImage(FlashState::DownloadingImage {
                            version_id: version_id.clone(),
                            progress,
                            channel: channel.clone(),
                            created_date: created_date.clone(),
                        });
                    }
                }
            }
            Message::DownloadCompleted(version_id) => {
                // Remove from downloads in progress
                self.downloads_in_progress
                    .retain(|(id, _)| id != &version_id);

                // Mark the OS image as downloaded
                if let Some(index) = self
                    .os_images
                    .iter()
                    .position(|img| img.version == version_id)
                {
                    if let Some(image) = self.os_images.get_mut(index) {
                        image.downloaded = true;

                        // Get the file path
                        let repo_version = Version {
                            id: image.version.clone(),
                            path: format!("golem-gpu-live-{}-{}.img.xz", image.name, image.version),
                            sha256: image.sha256.clone(),
                            created: image.created.clone(),
                        };

                        let repo = ImageRepo::new(); // Create temporary instance
                        let path = repo.get_image_path(&repo_version);
                        image.path = Some(path.to_string_lossy().to_string());
                    }
                }

                // Refresh the list of available storage devices
                info!("Refreshing available storage devices");
                match rs_drivelist::drive_list() {
                    Ok(devices) => {
                        // Filter to only include removable, non-virtual devices
                        self.storage_devices = devices
                            .into_iter()
                            .filter(|d| d.isRemovable && !d.isVirtual)
                            .map(|d| StorageDevice {
                                name: d.description,
                                path: d.device,
                                size: format!("{:.2} GB", d.size as f64 / 1000.0 / 1000.0 / 1000.0),
                            })
                            .collect();

                        debug!("Found {} available devices", self.storage_devices.len());

                        // Clear any previous device selection
                        self.selected_device = None;
                    }
                    Err(e) => {
                        error!("Failed to get drive list: {}", e);
                        // In case of error, provide an empty list
                        self.storage_devices = vec![];
                    }
                }

                // ALWAYS go to the device selection screen after this message
                // Either after a download completes or when clicking next from image selection
                debug!("Moving to device selection screen");
                self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);
            }
            Message::DownloadFailed(version_id, error) => {
                // Remove from downloads in progress
                self.downloads_in_progress
                    .retain(|(id, _)| id != &version_id);

                // Update UI state if needed
                if let Some(selected_idx) = self.selected_os_image {
                    if let Some(image) = self.os_images.get(selected_idx) {
                        if image.version == version_id {
                            if let AppMode::FlashNewImage(_) = &mut self.mode {
                                // Return to selection screen
                                self.mode = AppMode::FlashNewImage(FlashState::SelectOsImage);
                            }
                        }
                    }
                }

                // Display error in UI or log it
                warn!("Download failed for {}: {}", version_id, error);
            }
            Message::RepoDataLoaded(os_images) => {
                self.is_loading_repo = false;
                self.os_images = os_images;
            }
            Message::RepoLoadFailed => {
                self.is_loading_repo = false;
                // Could display an error message here
            }
            Message::RefreshRepoData => {
                if let AppMode::FlashNewImage(FlashState::SelectTargetDevice) = &self.mode {
                    // Refresh the list of available storage devices
                    info!("Refreshing available storage devices");
                    match rs_drivelist::drive_list() {
                        Ok(devices) => {
                            // Filter to only include removable, non-virtual devices
                            self.storage_devices = devices
                                .into_iter()
                                .filter(|d| d.isRemovable && !d.isVirtual)
                                .map(|d| StorageDevice {
                                    name: d.description,
                                    path: d.device,
                                    size: format!(
                                        "{:.2} GB",
                                        d.size as f64 / 1000.0 / 1000.0 / 1000.0
                                    ),
                                })
                                .collect();

                            debug!("Found {} available devices", self.storage_devices.len());

                            // Clear any previous device selection
                            self.selected_device = None;
                        }
                        Err(e) => {
                            error!("Failed to get drive list: {}", e);
                            // In case of error, provide an empty list
                            self.storage_devices = vec![];
                        }
                    }
                    return Task::none();
                }

                // Default behavior - refresh repository data
                if !self.is_loading_repo {
                    return self.load_repo_data();
                }
            }
            Message::GotoConfigureSettings => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    // Verify we have a device selected
                    // OS image must have been selected before we even get to the device selection screen
                    if self.selected_device.is_none() {
                        error!("No device selected");
                        return Task::none();
                    }

                    info!(
                        "Proceeding to configuration with device {}",
                        self.selected_device.unwrap()
                    );

                    // Check if we have a default preset
                    if let Some(default_preset) = self.get_default_preset() {
                        let is_wallet_valid = if default_preset.wallet_address.is_empty() {
                            false
                        } else {
                            crate::utils::eth::is_valid_eth_address(&default_preset.wallet_address)
                        };

                        // Use the default preset values
                        self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                            payment_network: default_preset.payment_network,
                            subnet: default_preset.subnet.clone(),
                            network_type: default_preset.network_type,
                            wallet_address: default_preset.wallet_address.clone(),
                            is_wallet_valid,
                        });

                        // Find and select the default preset
                        self.selected_preset =
                            self.configuration_presets.iter().position(|p| p.is_default);
                    } else {
                        // Initialize with default values if no preset
                        self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                            payment_network: crate::models::PaymentNetwork::Testnet,
                            subnet: "public".to_string(),
                            network_type: crate::models::NetworkType::Hybrid,
                            wallet_address: "".to_string(),
                            is_wallet_valid: false,
                        });
                        self.selected_preset = None;
                    }
                }
            }
            Message::SetPaymentNetwork(network) => {
                // Handle payment network update in flash mode
                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                        payment_network: network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
                // Handle payment network update in edit mode
                else if let AppMode::EditExistingDisk(EditState::EditConfiguration {
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
            }
            Message::SetSubnet(new_subnet) => {
                // Handle subnet update in flash mode
                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                        payment_network: *payment_network,
                        subnet: new_subnet,
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
                // Handle subnet update in edit mode
                else if let AppMode::EditExistingDisk(EditState::EditConfiguration {
                    payment_network,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: *payment_network,
                        subnet: new_subnet,
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
            }
            Message::SetNetworkType(network_type) => {
                // Handle network type update in flash mode
                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network,
                    subnet,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
                // Handle network type update in edit mode
                else if let AppMode::EditExistingDisk(EditState::EditConfiguration {
                    payment_network,
                    subnet,
                    wallet_address,
                    is_wallet_valid,
                    ..
                }) = &self.mode
                {
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type,
                        wallet_address: wallet_address.clone(),
                        is_wallet_valid: *is_wallet_valid,
                    });
                }
            }
            Message::SetWalletAddress(new_address) => {
                // Handle wallet address update in flash mode
                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network,
                    subnet,
                    network_type,
                    ..
                }) = &self.mode
                {
                    // Validate the Ethereum address
                    let is_valid = if new_address.is_empty() {
                        false
                    } else {
                        crate::utils::eth::is_valid_eth_address(&new_address)
                    };

                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: new_address,
                        is_wallet_valid: is_valid,
                    });
                }
                // Handle wallet address update in edit mode
                else if let AppMode::EditExistingDisk(EditState::EditConfiguration {
                    payment_network,
                    subnet,
                    network_type,
                    ..
                }) = &self.mode
                {
                    // Validate the Ethereum address
                    let is_valid = if new_address.is_empty() {
                        false
                    } else {
                        crate::utils::eth::is_valid_eth_address(&new_address)
                    };

                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: new_address,
                        is_wallet_valid: is_valid,
                    });
                }
            }
            Message::SelectTargetDevice(index) => {
                // Set the selected device index
                self.selected_device = Some(index);
                debug!("Selected device index: {}", index);

                // Stay on the device selection screen - we'll move to configuration
                // only when the user clicks the Write button
                self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);
            }
            Message::WriteImage => {
                // Start the actual writing process based on the configuration
                // First make sure we have both an image and device selected
                if self.selected_os_image.is_none() {
                    error!("No OS image selected for writing");
                    return Task::none();
                }

                if self.selected_device.is_none() {
                    error!("No target device selected for writing");
                    return Task::none();
                }

                // Extract needed data from the current mode
                let config_data = if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                }) = &self.mode
                {
                    // Check if wallet address is valid before proceeding
                    if !wallet_address.is_empty() && !*is_wallet_valid {
                        // Show error or return (we'll just return for now, but ideally
                        // there should be some error shown to the user)
                        warn!(
                            "Cannot proceed, wallet address is invalid: {}",
                            wallet_address
                        );
                        return Task::none();
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
                    if let (Some(image_idx), Some(device_idx)) =
                        (self.selected_os_image, self.selected_device)
                    {
                        if let (Some(image), Some(device)) = (
                            self.os_images.get(image_idx),
                            self.storage_devices.get(device_idx),
                        ) {
                            // Make sure the image is downloaded
                            if let Some(image_path) = &image.path {
                                // Start the write process with initial 0% progress
                                self.mode = AppMode::FlashNewImage(FlashState::WritingProcess(0.0));

                                // Get device path and image path
                                let device_path = device.path.clone();
                                let image_path_val = image_path.clone();

                                info!(
                                    "Starting flash with config: {:?} {:?} {} {} to device {}",
                                    payment_network_val,
                                    network_type_val,
                                    subnet_val,
                                    wallet_address_val,
                                    device_path
                                );

                                return Task::future(async move {
                                    info!("Starting disk image write to {}", device_path);
                                    Disk::lock_path(&device_path).await
                                })
                                .and_then(move |mut disk| {
                                    // Now write the image and handle progress
                                    let write_task = Task::sip(
                                        disk.write_image(&image_path_val),
                                        |message| match message {
                                            WriteProgress::Start => Message::WriteProgress(0.0),
                                            WriteProgress::Write(bytes) => {
                                                Message::WriteProgress(bytes as f32 / 1000.0)
                                            }
                                            WriteProgress::Finish => Message::WriteProgress(100.0),
                                        },
                                        |result| match result {
                                            Ok(WriteProgress::Finish) => {
                                                Message::WriteImageCompleted
                                            }
                                            Ok(_) => todo!(),
                                            Err(e) => Message::WriteImageFailed(e.to_string()),
                                        },
                                    );

                                    write_task
                                });
                            } else {
                                // Image not downloaded
                                error!("Cannot write - image not downloaded: {}", image.name);
                                self.mode = AppMode::FlashNewImage(FlashState::Completion(false));
                            }
                        } else {
                            // Invalid indices
                            error!("Invalid OS image or device indices");
                            self.mode = AppMode::FlashNewImage(FlashState::Completion(false));
                        }
                    } else {
                        // No indices
                        error!("No OS image or device selected");
                        self.mode = AppMode::FlashNewImage(FlashState::Completion(false));
                    }
                }
            }
            Message::CancelWrite => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    // Cancel the writing operation and go back to device selection
                    info!("User cancelled write operation");
                    self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);

                    // Release any disk resources
                    self.locked_disk = None;
                }
            }
            Message::WriteProgress(progress) => {
                // Update the writing progress in the UI
                if let AppMode::FlashNewImage(FlashState::WritingProcess(_)) = &self.mode {
                    // Update the UI with the new progress value
                    debug!("Write progress: {:.1}%", progress * 100.0);
                    self.mode = AppMode::FlashNewImage(FlashState::WritingProcess(progress));
                }
            }
            Message::WriteImageCompleted => {
                // First check if we need to apply configuration
                if let AppMode::FlashNewImage(FlashState::WritingProcess(_)) = &self.mode {
                    // Extract the device path from the selected device
                    if let Some(device_idx) = self.selected_device {
                        if let Some(device) = self.storage_devices.get(device_idx) {
                            // Get the device path and config values, similar to WriteImage message handler
                            let device_path = device.path.clone();

                            // Extract needed data from the previous mode
                            let config_data =
                                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                                    payment_network,
                                    subnet,
                                    network_type,
                                    wallet_address,
                                    ..
                                }) = &self.mode
                                {
                                    Some((
                                        *payment_network,
                                        *network_type,
                                        subnet.clone(),
                                        wallet_address.clone(),
                                    ))
                                } else {
                                    None
                                };

                            if let Some((payment_network, network_type, subnet, wallet_address)) =
                                config_data
                            {
                                // Create a task to apply configuration
                                info!("Image write completed, applying configuration");

                                // Set the mode to completion now, the task will run in the background
                                self.mode = AppMode::FlashNewImage(FlashState::Completion(true));

                                // Release the current disk handle
                                self.locked_disk = None;

                                return Task::perform(
                                    async move {
                                        // Try to lock the device again to apply configuration
                                        match Disk::lock_path(&device_path).await {
                                            Ok(mut disk) => {
                                                // Config partition UUID
                                                let config_partition_uuid =
                                                    "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

                                                // Format configuration partition if needed before writing
                                                // This ensures the partition is accessible and properly formatted
                                                let format_result = disk.find_or_create_partition(config_partition_uuid, true);
                                                if let Err(e) = format_result {
                                                    warn!("Failed to prepare configuration partition: {}", e);
                                                    return;
                                                }
                                                
                                                // Try to apply configuration
                                                let result = disk.write_configuration(
                                                    config_partition_uuid,
                                                    payment_network,
                                                    network_type,
                                                    &subnet,
                                                    &wallet_address,
                                                );

                                                if let Err(e) = result {
                                                    warn!("Applied configuration failed: {}", e);
                                                } else {
                                                    info!("Applied configuration successfully");
                                                }
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "Failed to lock device for configuration: {}",
                                                    e
                                                );
                                            }
                                        }

                                        // No need to return anything, the UI is already updated
                                    },
                                    |_| Message::Exit, // This message is ignored since UI is already updated
                                );
                            }
                        }
                    }
                }

                // Default behavior if we can't apply configuration
                info!("Image write completed successfully");
                self.mode = AppMode::FlashNewImage(FlashState::Completion(true));

                // Release any disk resources
                self.locked_disk = None;
            }
            Message::WriteImageFailed(error_msg) => {
                // Set the completion state to failure and log the error
                error!("Image write failed: {}", error_msg);
                self.mode = AppMode::FlashNewImage(FlashState::Completion(false));

                // Release any disk resources
                self.locked_disk = None;
            }
            Message::DeviceLockedForWriting(disk, image_path) => {
                // We've locked the device and are ready to write the image
                info!("Device locked for writing image: {}", image_path);

                // Start writing with 0% progress
                self.mode = AppMode::FlashNewImage(FlashState::WritingProcess(0.0));

                // Setup a global variable to store progress for subscriptions to access
                // in a real app this would be better handled with a proper state management system
                use std::sync::Arc;
                use std::sync::atomic::{AtomicU32, Ordering};

                // Create a static atomic to store progress (as integer percentage)
                static WRITE_PROGRESS: once_cell::sync::Lazy<Arc<AtomicU32>> =
                    once_cell::sync::Lazy::new(|| Arc::new(AtomicU32::new(0)));

                // Reset progress to 0
                WRITE_PROGRESS.store(0, Ordering::SeqCst);

                // Create a task to perform the actual write
                let image_path_clone = image_path.clone();

                todo!()
            }

            Message::PollWriteProgress => {
                // Check if we're in writing mode
                if let AppMode::FlashNewImage(FlashState::WritingProcess(_)) = &self.mode {
                    // Get the current progress from our static atomic
                    use std::sync::atomic::Ordering;

                    // Access the static progress atomic
                    static WRITE_PROGRESS: once_cell::sync::Lazy<
                        std::sync::Arc<std::sync::atomic::AtomicU32>,
                    > = once_cell::sync::Lazy::new(|| {
                        std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0))
                    });

                    // Read the current progress
                    let progress_int = WRITE_PROGRESS.load(Ordering::SeqCst);

                    // Convert from integer percentage (0-10000) back to float (0.0-1.0)
                    let progress = progress_int as f32 / 10000.0;

                    // Only update UI if progress has actually changed
                    if let AppMode::FlashNewImage(FlashState::WritingProcess(current_progress)) =
                        &self.mode
                    {
                        if progress > *current_progress && progress <= 1.0 {
                            debug!("Updating UI progress: {:.2}%", progress * 100.0);

                            // Update mode with new progress
                            self.mode =
                                AppMode::FlashNewImage(FlashState::WritingProcess(progress));
                        }
                    }
                }
            }
            Message::FlashAnother => {
                self.mode = AppMode::FlashNewImage(FlashState::SelectOsImage);
                self.selected_os_image = None;
                self.selected_device = None;
            }
            Message::Exit => {
                self.mode = AppMode::StartScreen;
                self.selected_os_image = None;
                self.selected_device = None;
            }
            Message::SelectExistingDevice(index) => {
                self.selected_device = Some(index);
                // Update the mode to navigate back to device selection
                if let AppMode::EditExistingDisk(_) = &self.mode {
                    // Release any locked disk when going back to device selection
                    self.locked_disk = None;
                    // Clear any error messages
                    self.error_message = None;
                    self.mode = AppMode::EditExistingDisk(EditState::SelectDevice);
                }
            }
            Message::GotoEditConfiguration => {
                if let AppMode::EditExistingDisk(_) = &self.mode {
                    // Get the selected device path
                    if let Some(device_index) = self.selected_device {
                        if let Some(device) = self.storage_devices.get(device_index) {
                            // Clear any previous error messages
                            self.error_message = None;

                            // Attempt to lock the device
                            let device_path = device.path.clone();

                            // Return a task that will lock the device and update the state
                            return Task::perform(
                                async move {
                                    // Try to acquire exclusive lock on the device
                                    match Disk::lock_path(&device_path).await {
                                        Ok(disk) => (Some(disk), None),
                                        Err(err) => {
                                            // Format a more user-friendly error message
                                            let error_msg = format!(
                                                "Failed to lock device {}: {}",
                                                device_path, err
                                            );
                                            error!("{}", error_msg);
                                            (None, Some(error_msg))
                                        }
                                    }
                                },
                                move |(disk, error)| {
                                    if let Some(disk) = disk {
                                        // Successfully locked the device, proceed with configuration
                                        Message::DeviceLocked(Some(disk))
                                    } else if let Some(error_msg) = error {
                                        // Show error message to the user
                                        Message::ShowError(error_msg)
                                    } else {
                                        // This should never happen but handle it just in case
                                        Message::ShowError(
                                            "Failed to lock device: Unknown error".to_string(),
                                        )
                                    }
                                },
                            );
                        }
                    }
                }
            }

            Message::DeviceLocked(disk) => {
                // Store the locked disk
                self.locked_disk = disk;

                // If this message is a result of the SaveConfiguration task, check current mode
                if let AppMode::EditExistingDisk(EditState::EditConfiguration { .. }) = &self.mode {
                    if self.locked_disk.is_some() {
                        // We got the disk back after writing configuration, proceed to completion
                        self.mode = AppMode::EditExistingDisk(EditState::Completion(true));
                    } else {
                        // Disk was somehow lost, show error
                        self.error_message =
                            Some("Failed to maintain disk lock after configuration".to_string());
                        self.mode = AppMode::EditExistingDisk(EditState::Completion(false));
                    }
                    return Task::none();
                }

                // This is initial device locking, not a result of SaveConfiguration
                // First try to read configuration from the disk
                let config_partition_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";
                let mut config_from_disk = None;

                if let Some(disk) = &mut self.locked_disk {
                    // Try to read the configuration from the disk
                    // First, use find_or_create_partition to format the configuration partition if needed
                    match disk.find_or_create_partition(config_partition_uuid, true) {
                        Ok(fs) => {
                            info!("Successfully accessed configuration partition");
                            // Now try to read the configuration
                            match disk.read_configuration(config_partition_uuid) {
                                Ok(config) => {
                                    info!("Successfully read configuration from disk");
                                    config_from_disk = Some(config);
                                }
                                Err(e) => {
                                    // Not a fatal error - we'll just use default values
                                    warn!("Failed to read configuration from disk: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            // Failed to access the partition even with formatting
                            warn!("Failed to access configuration partition: {}", e);
                        }
                    }
                }

                // If we successfully read the configuration from disk, use it
                if let Some(config) = config_from_disk {
                    let is_wallet_valid = if config.wallet_address.is_empty() {
                        false
                    } else {
                        crate::utils::eth::is_valid_eth_address(&config.wallet_address)
                    };

                    // Use the configuration values from the disk
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: config.payment_network,
                        subnet: config.subnet,
                        network_type: config.network_type,
                        wallet_address: config.wallet_address,
                        is_wallet_valid,
                    });

                    // Reset selected preset as we're loading from disk
                    self.selected_preset = None;
                }
                // Otherwise check if we have a default preset
                else if let Some(default_preset) = self.get_default_preset() {
                    let is_wallet_valid = if default_preset.wallet_address.is_empty() {
                        false
                    } else {
                        crate::utils::eth::is_valid_eth_address(&default_preset.wallet_address)
                    };

                    // Use the default preset values
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: default_preset.payment_network,
                        subnet: default_preset.subnet.clone(),
                        network_type: default_preset.network_type,
                        wallet_address: default_preset.wallet_address.clone(),
                        is_wallet_valid,
                    });

                    // Find and select the default preset
                    self.selected_preset =
                        self.configuration_presets.iter().position(|p| p.is_default);
                } else {
                    // Initialize with default values
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                        payment_network: crate::models::PaymentNetwork::Testnet,
                        subnet: "public".to_string(),
                        network_type: crate::models::NetworkType::Hybrid,
                        wallet_address: "".to_string(),
                        is_wallet_valid: false,
                    });
                    self.selected_preset = None;
                }
            }
            Message::SaveConfiguration => {
                if let AppMode::EditExistingDisk(EditState::EditConfiguration {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                }) = &self.mode
                {
                    // Check if wallet address is valid before proceeding
                    if !wallet_address.is_empty() && !is_wallet_valid {
                        // Show error to the user
                        let error_msg =
                            format!("Cannot save, wallet address is invalid: {}", wallet_address);
                        error!("{}", error_msg);
                        self.error_message = Some(error_msg);
                        return Task::none();
                    }

                    // Check if we have a locked disk
                    if self.locked_disk.is_none() {
                        let error_msg = "Error: No locked disk available for writing. Device may have been disconnected.";
                        error!("{}", error_msg);
                        self.error_message = Some(error_msg.to_string());
                        self.mode = AppMode::EditExistingDisk(EditState::Completion(false));
                        return Task::none();
                    }

                    // Make clones of all data needed for the async task
                    let payment_network = *payment_network;
                    let network_type = *network_type;
                    let subnet = subnet.clone();
                    let wallet_address = wallet_address.clone();

                    // Create a handle to the locked disk that can be sent to the async task
                    // We need to do this because we can't send the locked_disk directly (it doesn't implement Clone)
                    let mut locked_disk = self.locked_disk.take();

                    // Use the config partition UUID
                    // In a real application, this UUID would be a constant or configuration value
                    let config_partition_uuid = "33b921b8-edc5-46a0-8baa-d0b7ad84fc71";

                    // Write the configuration in a separate task
                    return Task::perform(
                        async move {
                            // If we don't have a locked disk, we can't write the configuration
                            if locked_disk.is_none() {
                                return (false, Some("No locked disk available".to_string()), None);
                            }

                            let mut disk = locked_disk.unwrap();

                            info!(
                                "Writing config to disk: {:?} {:?} {} {}",
                                payment_network, network_type, subnet, wallet_address
                            );

                            // First make sure the partition is properly formatted
                            let format_result = disk.find_or_create_partition(config_partition_uuid, true);
                            if let Err(e) = format_result {
                                let error_msg = format!("Failed to prepare configuration partition: {}", e);
                                error!("{}", error_msg);
                                return (false, Some(error_msg), Some(disk));
                            }
                            
                            // Use the write_configuration function to write to the disk
                            match disk.write_configuration(
                                config_partition_uuid,
                                payment_network,
                                network_type,
                                &subnet,
                                &wallet_address,
                            ) {
                                Ok(_) => {
                                    // Configuration was successfully written
                                    (true, None, Some(disk))
                                }
                                Err(e) => {
                                    // There was an error writing the configuration
                                    let error_msg = format!("Failed to write configuration: {}", e);
                                    (false, Some(error_msg), Some(disk))
                                }
                            }
                        },
                        |(success, error_msg, disk)| {
                            // Return the disk to self.locked_disk if available
                            if let Some(d) = disk {
                                Message::DeviceLocked(Some(d))
                            } else if success {
                                // Configuration saved successfully
                                Message::ConfigurationSaved
                            } else {
                                // Configuration save failed
                                if let Some(msg) = error_msg {
                                    Message::ShowError(msg)
                                } else {
                                    Message::ConfigurationSaveFailed
                                }
                            }
                        },
                    );
                }

                return Task::none();
            }

            Message::ConfigurationSaved => {
                // Release the lock on successful save
                self.locked_disk = None;
                self.mode = AppMode::EditExistingDisk(EditState::Completion(true));
            }

            Message::ConfigurationSaveFailed => {
                // Keep the lock on failure so user can retry
                self.mode = AppMode::EditExistingDisk(EditState::Completion(false));
            }

            Message::ShowError(error_msg) => {
                // Store the error message for display
                self.error_message = Some(error_msg);

                // Go back to device selection mode
                self.mode = AppMode::EditExistingDisk(EditState::SelectDevice);
            }
            Message::BackToMainMenu => {
                // Release any locked disk when going back to main menu
                self.locked_disk = None;
                // Clear any error messages
                self.error_message = None;
                self.mode = AppMode::StartScreen;
            }
            // Handle preset-related messages
            Message::SaveAsPreset => {
                if !self.new_preset_name.is_empty() {
                    if let Some(preset) =
                        self.create_preset_from_current_config(self.new_preset_name.clone())
                    {
                        // Clone the preset before any modifications
                        let mut new_preset = preset.clone();

                        // If this is the first preset, set it as default
                        let is_first = self.configuration_presets.is_empty();
                        if is_first {
                            new_preset.is_default = true;
                        }

                        // Add the preset to the PresetManager first to persist it
                        if let Some(manager) = &mut self.preset_manager {
                            if let Err(e) = manager.add_preset(new_preset.clone()) {
                                error!("Failed to add preset to manager: {}", e);
                            }

                            // Refresh our local copy from the manager
                            self.configuration_presets = manager.get_presets().clone();
                        } else {
                            // Fall back to just adding to our local copy if no manager
                            self.configuration_presets.push(new_preset);
                        }

                        // Select the new preset
                        self.selected_preset = Some(self.configuration_presets.len() - 1);

                        // Clear the new preset name
                        self.new_preset_name = String::new();
                    }
                }
            }
            Message::SelectPreset(index) => {
                self.apply_preset(index);
            }
            Message::DeletePreset(index) => {
                self.delete_preset(index);
            }
            Message::SetDefaultPreset(index) => {
                self.set_default_preset(index);
            }
            Message::EditPresetName(index, name) => {
                if index < self.configuration_presets.len() && !name.is_empty() {
                    self.configuration_presets[index].name = name;
                }
            }
            Message::SetPresetName(name) => {
                self.new_preset_name = name;
            }
            Message::SavePresetsToStorage => {
                // This would be implemented with actual persistence
                info!(
                    "Saved {} presets to storage",
                    self.configuration_presets.len()
                );
            }
            Message::LoadPresetsFromStorage => {
                // This would be implemented with actual persistence
                info!("Loaded presets from storage");
            }
            Message::TogglePresetManager => {
                // Toggle preset management UI visibility
                self.show_preset_manager = !self.show_preset_manager;
            }
            Message::BackToSelectOsImage => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    self.mode = AppMode::FlashNewImage(FlashState::SelectOsImage);
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        // If we're loading repository data, show a loading indicator
        if self.is_loading_repo {
            return self.view_loading();
        }

        match &self.mode {
            AppMode::StartScreen => ui::view_start_screen(),
            AppMode::FlashNewImage(state) => match state {
                FlashState::SelectOsImage => {
                    if self.os_images.is_empty() {
                        self.view_no_images()
                    } else {
                        ui::view_select_os_image(&self.os_images, self.selected_os_image)
                    }
                }
                FlashState::DownloadingImage {
                    version_id,
                    progress,
                    channel,
                    created_date,
                } => {
                    ui::flash::view_downloading_image(version_id, *progress, channel, created_date)
                }
                FlashState::SelectTargetDevice => ui::flash::view_select_target_device(
                    &self.storage_devices,
                    self.selected_device,
                ),
                FlashState::ConfigureSettings {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                } => ui::flash::view_configure_settings(
                    *payment_network,
                    subnet.clone(),
                    *network_type,
                    wallet_address.clone(),
                    *is_wallet_valid,
                    &self.configuration_presets,
                    self.selected_preset,
                    &self.new_preset_name,
                    self.show_preset_manager,
                ),
                FlashState::WritingProcess(progress) => ui::flash::view_writing_process(*progress),
                FlashState::Completion(success) => ui::flash::view_flash_completion(*success),
            },
            AppMode::EditExistingDisk(state) => match state {
                EditState::SelectDevice => {
                    // Pass the error message to the view if one exists
                    let error_ref = self.error_message.as_deref();
                    ui::view_select_existing_device(
                        self.selected_device,
                        &self.storage_devices,
                        error_ref,
                    )
                }
                EditState::EditConfiguration {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                } => ui::view_edit_configuration(
                    *payment_network,
                    subnet.clone(),
                    *network_type,
                    wallet_address.clone(),
                    *is_wallet_valid,
                    self.selected_device,
                    &self.configuration_presets,
                    self.selected_preset,
                    &self.new_preset_name,
                    self.show_preset_manager,
                ),
                EditState::Completion(success) => ui::view_edit_completion(*success),
            },
        }
    }

    fn view_loading(&self) -> Element<'_, Message> {
        use iced::widget::{column, container, text};

        let content = column![
            text("Loading repository data...").size(24),
            text("Please wait").size(16)
        ]
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .spacing(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_no_images(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, text};

        let content = column![
            text("No OS images found").size(24),
            text("Unable to fetch repository data or no images available").size(16),
            button(text("Refresh")).on_press(Message::RefreshRepoData),
            button(text("Back to Main Menu")).on_press(Message::BackToMainMenu)
        ]
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .spacing(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    // Required to implement Application trait in main.rs
    pub fn subscription(&self) -> iced::Subscription<Message> {
        // If we're in writing process mode, periodically send progress updates
        if let AppMode::FlashNewImage(FlashState::WritingProcess(_)) = &self.mode {
            // Create a timer subscription that periodically updates the progress bar
            iced::time::every(std::time::Duration::from_millis(200))
                .map(|_| Message::PollWriteProgress)
        } else {
            iced::Subscription::none()
        }
    }
}

impl GolemGpuImager {
    // Get the default preset if one exists
    fn get_default_preset(&self) -> Option<&ConfigurationPreset> {
        // Try using the PresetManager first
        if let Some(manager) = &self.preset_manager {
            return manager.get_default_preset();
        }

        // Fall back to the in-memory configuration_presets if PresetManager is not available
        self.configuration_presets
            .iter()
            .find(|preset| preset.is_default)
    }

    // Create a preset from current configuration
    fn create_preset_from_current_config(&self, name: String) -> Option<ConfigurationPreset> {
        match &self.mode {
            AppMode::FlashNewImage(FlashState::ConfigureSettings {
                payment_network,
                subnet,
                network_type,
                wallet_address,
                is_wallet_valid,
            }) => {
                if *is_wallet_valid || wallet_address.is_empty() {
                    Some(ConfigurationPreset {
                        name,
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_default: false,
                    })
                } else {
                    None // Don't create a preset with invalid wallet
                }
            }
            AppMode::EditExistingDisk(EditState::EditConfiguration {
                payment_network,
                subnet,
                network_type,
                wallet_address,
                is_wallet_valid,
            }) => {
                if *is_wallet_valid || wallet_address.is_empty() {
                    Some(ConfigurationPreset {
                        name,
                        payment_network: *payment_network,
                        subnet: subnet.clone(),
                        network_type: *network_type,
                        wallet_address: wallet_address.clone(),
                        is_default: false,
                    })
                } else {
                    None // Don't create a preset with invalid wallet
                }
            }
            _ => None,
        }
    }

    // Apply a preset configuration
    fn apply_preset(&mut self, preset_index: usize) {
        if preset_index >= self.configuration_presets.len() {
            return;
        }

        let preset = &self.configuration_presets[preset_index];

        // Determine wallet validity
        let is_wallet_valid = if preset.wallet_address.is_empty() {
            false
        } else {
            crate::utils::eth::is_valid_eth_address(&preset.wallet_address)
        };

        // Apply preset based on current mode
        match &mut self.mode {
            AppMode::FlashNewImage(FlashState::ConfigureSettings { .. }) => {
                self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network: preset.payment_network,
                    subnet: preset.subnet.clone(),
                    network_type: preset.network_type,
                    wallet_address: preset.wallet_address.clone(),
                    is_wallet_valid,
                });
            }
            AppMode::EditExistingDisk(EditState::EditConfiguration { .. }) => {
                self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration {
                    payment_network: preset.payment_network,
                    subnet: preset.subnet.clone(),
                    network_type: preset.network_type,
                    wallet_address: preset.wallet_address.clone(),
                    is_wallet_valid,
                });
            }
            _ => {
                // Not in a configuration mode, nothing to apply
            }
        }

        self.selected_preset = Some(preset_index);
    }

    // Set a preset as the default
    fn set_default_preset(&mut self, preset_index: usize) {
        if preset_index >= self.configuration_presets.len() {
            return;
        }

        // Clear all default flags
        for preset in &mut self.configuration_presets {
            preset.is_default = false;
        }

        // Set the selected preset as default
        self.configuration_presets[preset_index].is_default = true;

        // Update the preset in the PresetManager and persist it
        if let Some(manager) = &mut self.preset_manager {
            let mut preset = self.configuration_presets[preset_index].clone();
            preset.is_default = true;
            if let Err(e) = manager.set_default_preset(preset_index) {
                error!("Failed to set default preset in manager: {}", e);
            }
        }
    }

    // Delete a preset
    fn delete_preset(&mut self, preset_index: usize) {
        if preset_index >= self.configuration_presets.len() {
            return;
        }

        // Delete the preset from the PresetManager first
        if let Some(manager) = &mut self.preset_manager {
            if let Err(e) = manager.delete_preset(preset_index) {
                error!("Failed to delete preset from manager: {}", e);
            }
        }

        // Remove the preset from our local copy
        let was_default = self.configuration_presets[preset_index].is_default;
        self.configuration_presets.remove(preset_index);

        // Adjust selected preset if necessary
        if let Some(selected) = self.selected_preset {
            if selected == preset_index {
                self.selected_preset = None;
            } else if selected > preset_index {
                self.selected_preset = Some(selected - 1);
            }
        }

        // If we deleted the default preset and there are still presets,
        // make the first one default
        if was_default && !self.configuration_presets.is_empty() {
            self.configuration_presets[0].is_default = true;

            // Update the default preset in the PresetManager
            if let Some(manager) = &mut self.preset_manager {
                if let Err(e) = manager.set_default_preset(0) {
                    error!("Failed to set new default preset after deletion: {}", e);
                }
            }
        }
    }
}
