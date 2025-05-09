use crate::models::{AppMode, EditState, FlashState, Message, OsImage, StorageDevice};
use crate::ui;
use crate::utils::repo::{DownloadStatus, ImageRepo, Version};
use futures_util::FutureExt;
use iced::task::Sipper;
use iced::{Alignment, Element, Length, Task};
use std::sync::Arc;

pub struct GolemGpuImager {
    pub mode: AppMode,
    pub os_images: Vec<OsImage>,
    pub storage_devices: Vec<StorageDevice>,
    pub selected_os_image: Option<usize>,
    pub selected_device: Option<usize>,
    pub image_repo: Arc<ImageRepo>,
    pub is_loading_repo: bool,
    pub downloads_in_progress: Vec<(String, f32)>, // (version_id, progress)
}

impl GolemGpuImager {
    pub fn new() -> Self {
        let image_repo = Arc::new(ImageRepo::new());

        Self {
            mode: AppMode::StartScreen,
            os_images: vec![], // Will be populated from repo
            storage_devices: vec![
                StorageDevice {
                    name: "Kingston 32GB".to_string(),
                    path: "/dev/sdb".to_string(),
                    size: "32GB".to_string(),
                },
                StorageDevice {
                    name: "SanDisk 64GB".to_string(),
                    path: "/dev/sdc".to_string(),
                    size: "64GB".to_string(),
                },
            ],
            selected_os_image: None,
            selected_device: None,
            image_repo,
            is_loading_repo: false,
            downloads_in_progress: Vec::new(),
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
                let devices = rs_drivelist::drive_list().unwrap();
                self.storage_devices = devices
                    .into_iter()
                    .filter(|d| d.isRemovable && !d.isVirtual)
                    .map(|d| StorageDevice {
                        name: d.description,
                        path: d.device,
                        size: format!("{:.2} GB", d.size as f64 / 1000.0 / 1000.0 / 1000.0),
                    })
                    .collect()
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
                    let repo = Arc::clone(&self.image_repo);

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

                // Update the UI to go to select target device
                if let Some(selected_idx) = self.selected_os_image {
                    if let Some(image) = self.os_images.get(selected_idx) {
                        if image.version == version_id {
                            // Move to device selection after download completes
                            self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);
                        }
                    }
                }
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
                println!("Download failed for {}: {}", version_id, error);
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
                if !self.is_loading_repo {
                    return self.load_repo_data();
                }
            }
            Message::GotoConfigureSettings => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    // Initialize with default values
                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                        payment_network: crate::models::PaymentNetwork::Testnet,
                        subnet: "public".to_string(),
                        network_type: crate::models::NetworkType::Hybrid,
                        wallet_address: "".to_string(),
                        is_wallet_valid: false,
                    });
                }
            }
            Message::SetPaymentNetwork(network) => {
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
            }
            Message::SetSubnet(new_subnet) => {
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
            }
            Message::SetNetworkType(network_type) => {
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
            }
            Message::SetWalletAddress(new_address) => {
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
            }
            Message::SelectTargetDevice(index) => {
                self.selected_device = Some(index);
                // After device selection, move to configuration
                self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network: crate::models::PaymentNetwork::Testnet,
                    subnet: "public".to_string(),
                    network_type: crate::models::NetworkType::Hybrid,
                    wallet_address: "".to_string(),
                    is_wallet_valid: false,
                });
            }
            Message::WriteImage => {
                // Start the actual writing process based on the configuration
                if let AppMode::FlashNewImage(FlashState::ConfigureSettings {
                    payment_network,
                    subnet,
                    network_type,
                    wallet_address,
                    is_wallet_valid,
                }) = &self.mode
                {
                    // Check if wallet address is valid before proceeding
                    if !wallet_address.is_empty() && !is_wallet_valid {
                        // Show error or return (we'll just return for now, but ideally
                        // there should be some error shown to the user)
                        println!(
                            "Cannot proceed, wallet address is invalid: {}",
                            wallet_address
                        );
                        return Task::none();
                    }

                    // Here you would apply the configuration (payment_network, subnet, network_type, wallet_address)
                    // to the image before flashing
                    println!(
                        "Starting flash with config: {:?} {:?} {} {}",
                        payment_network, network_type, subnet, wallet_address
                    );

                    // Start the write process
                    self.mode = AppMode::FlashNewImage(FlashState::WritingProcess(0.0));

                    // For now, we'll simulate completion after a moment
                    // This would be replaced by actual flashing with progress updates
                    self.mode = AppMode::FlashNewImage(FlashState::Completion(true));
                }
            }
            Message::CancelWrite => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);
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
            }
            Message::SaveConfiguration => {
                if let AppMode::EditExistingDisk(_) = &self.mode {
                    // In a real app, we would save the configuration here
                    self.mode = AppMode::EditExistingDisk(EditState::Completion(true));
                }
            }
            Message::BackToMainMenu => {
                self.mode = AppMode::StartScreen;
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
                FlashState::SelectTargetDevice => {
                    ui::flash::view_select_target_device(&self.storage_devices)
                }
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
                ),
                FlashState::WritingProcess(progress) => ui::flash::view_writing_process(*progress),
                FlashState::Completion(success) => ui::flash::view_flash_completion(*success),
            },
            AppMode::EditExistingDisk(state) => match state {
                EditState::SelectDevice => {
                    ui::view_select_existing_device(self.selected_device, &self.storage_devices)
                }
                EditState::EditConfiguration => ui::view_edit_configuration(self.selected_device),
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
        iced::Subscription::none()
    }
}
