use iced::Element;

use crate::models::{AppMode, EditState, FlashState, Message, OsImage, StorageDevice};
use crate::ui;

pub struct GolemGpuImager {
    pub mode: AppMode,
    pub os_images: Vec<OsImage>,
    pub storage_devices: Vec<StorageDevice>,
    pub selected_os_image: Option<usize>,
    pub selected_device: Option<usize>,
}

impl GolemGpuImager {
    pub fn new() -> Self {
        Self {
            mode: AppMode::StartScreen,
            os_images: vec![
                OsImage {
                    name: "Golem GPU OS".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Official Golem GPU operating system".to_string(),
                    downloaded: false,
                },
                OsImage {
                    name: "Golem GPU OS Dev".to_string(),
                    version: "1.1.0-dev".to_string(),
                    description: "Development version with latest features".to_string(),
                    downloaded: true,
                },
            ],
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
        }
    }

    pub fn title(&self) -> String {
        String::from("Golem GPU Imager")
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::FlashNewImage => {
                self.mode = AppMode::FlashNewImage(FlashState::SelectOsImage);
                self.selected_os_image = None;
                self.selected_device = None;
            }
            Message::EditExistingDisk => {
                self.mode = AppMode::EditExistingDisk(EditState::SelectDevice);
                self.selected_device = None;
            }
            Message::SelectOsImage(index) => {
                self.selected_os_image = Some(index);
            }
            Message::DownloadOsImage(index) => {
                // In a real app, we would download the OS image here
                if let Some(image) = self.os_images.get_mut(index) {
                    image.downloaded = true;
                    self.selected_os_image = Some(index);
                }
            }
            Message::ConfigureSettings => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    self.mode = AppMode::FlashNewImage(FlashState::ConfigureSettings);
                }
            }
            Message::SelectTargetDevice(index) => {
                self.selected_device = Some(index);
                if let AppMode::FlashNewImage(_) = &self.mode {
                    self.mode = AppMode::FlashNewImage(FlashState::SelectTargetDevice);
                }
            }
            Message::WriteImage => {
                if let AppMode::FlashNewImage(_) = &self.mode {
                    self.mode = AppMode::FlashNewImage(FlashState::WritingProcess(0.0));
                    // In a real app, we would start the writing process here
                    // and update the progress periodically
                    // For now, we'll simulate completion after a moment
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
                if let AppMode::EditExistingDisk(_) = &self.mode {
                    self.mode = AppMode::EditExistingDisk(EditState::EditConfiguration);
                }
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
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.mode {
            AppMode::StartScreen => ui::view_start_screen(),
            AppMode::FlashNewImage(state) => match state {
                FlashState::SelectOsImage => ui::view_select_os_image(&self.os_images, self.selected_os_image),
                FlashState::ConfigureSettings => ui::view_configure_settings(self.selected_os_image),
                FlashState::SelectTargetDevice => ui::view_select_target_device(&self.storage_devices),
                FlashState::WritingProcess(progress) => ui::view_writing_process(*progress),
                FlashState::Completion(success) => ui::view_flash_completion(*success),
            },
            AppMode::EditExistingDisk(state) => match state {
                EditState::SelectDevice => ui::view_select_existing_device(&self.storage_devices),
                EditState::EditConfiguration => ui::view_edit_configuration(self.selected_device),
                EditState::Completion(success) => ui::view_edit_completion(*success),
            },
        }
    }
}