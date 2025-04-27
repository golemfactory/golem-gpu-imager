use iced::alignment::{Horizontal, Vertical};
use iced::theme::{self, Theme};
use iced::widget::{Column, Container, Row, button, column, container, progress_bar, row, scrollable, svg, text};
use iced::{Alignment, Background, Border, Center, Color, Padding, Renderer};
use iced::{Application, Element, Length, Settings, executor};

// Include the logo SVG data
const LOGO_SVG: &[u8] = include_bytes!("assets/logo.svg");

pub fn main() -> iced::Result {
    iced::application(
        GolemGpuImager::new,
        GolemGpuImager::update,
        GolemGpuImager::view,
    )
    .title(GolemGpuImager::title)
    .window_size(iced::Size::new(480f32, 640f32))
    .centered()
    .run()
}

#[derive(Debug, Clone)]
enum Message {
    FlashNewImage,
    EditExistingDisk,
    SelectOsImage(usize),
    DownloadOsImage(usize),
    ConfigureSettings,
    SelectTargetDevice(usize),
    WriteImage,
    CancelWrite,
    FlashAnother,
    Exit,
    SelectExistingDevice(usize),
    SaveConfiguration,
    BackToMainMenu,
}

#[derive(Debug, Clone)]
struct OsImage {
    name: String,
    version: String,
    description: String,
    downloaded: bool,
}

#[derive(Debug, Clone)]
struct StorageDevice {
    name: String,
    path: String,
    size: String,
}

enum AppMode {
    StartScreen,
    FlashNewImage(FlashState),
    EditExistingDisk(EditState),
}

enum FlashState {
    SelectOsImage,
    ConfigureSettings,
    SelectTargetDevice,
    WritingProcess(f32), // Progress 0.0 - 1.0
    Completion(bool),    // Success or failure
}

enum EditState {
    SelectDevice,
    EditConfiguration,
    Completion(bool), // Success or failure
}

struct GolemGpuImager {
    mode: AppMode,
    os_images: Vec<OsImage>,
    storage_devices: Vec<StorageDevice>,
    selected_os_image: Option<usize>,
    selected_device: Option<usize>,
}

impl GolemGpuImager {
    fn new() -> Self {
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

    fn title(&self) -> String {
        String::from("Golem GPU Imager")
    }

    fn update(&mut self, message: Message) {
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

    fn view(&self) -> Element<Message> {
        match &self.mode {
            AppMode::StartScreen => self.view_start_screen(),
            AppMode::FlashNewImage(state) => match state {
                FlashState::SelectOsImage => self.view_select_os_image(),
                FlashState::ConfigureSettings => self.view_configure_settings(),
                FlashState::SelectTargetDevice => self.view_select_target_device(),
                FlashState::WritingProcess(progress) => self.view_writing_process(*progress),
                FlashState::Completion(success) => self.view_flash_completion(*success),
            },
            AppMode::EditExistingDisk(state) => match state {
                EditState::SelectDevice => self.view_select_existing_device(),
                EditState::EditConfiguration => self.view_edit_configuration(),
                EditState::Completion(success) => self.view_edit_completion(*success),
            },
        }
    }
}

impl GolemGpuImager {
    fn view_start_screen(&self) -> Element<Message> {
        // Create the logo widget from the included SVG data
        let logo = svg::Svg::new(svg::Handle::from_memory(LOGO_SVG))
            .width(200)
            .height(200);

        let title = text("Golem GPU Imager")
            .size(40)
            .width(Length::Fill)
            .align_x(Horizontal::Center);

        let description = text(
            "A utility to flash OS images onto Golem GPU devices or edit existing configurations.",
        )
        .size(20)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center);

        let flash_button = button("Flash New Image")
            .width(200)
            .padding(10)
            .on_press(Message::FlashNewImage);

        let edit_button = button("Edit Existing Disk")
            .width(200)
            .padding(10)
            .on_press(Message::EditExistingDisk);

        let content = column![logo, title, description, flash_button, edit_button,]
            .width(Length::Fill)
            .spacing(20)
            .align_x(Horizontal::Center)
            .padding(40);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_select_os_image(&self) -> Element<Message> {
        let title = text("Select OS Image")
            .size(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center);

        let os_image_list = column(self.os_images.iter().enumerate().map(|(i, image)| {
            let image_info = column![
                text(&image.name).size(20),
                text(format!("Version: {}", image.version)).size(16),
                text(&image.description).size(16),
            ]
            .spacing(5)
            .width(Length::Fill);

            let action_button = if image.downloaded {
                button("Select")
                    .on_press(Message::SelectOsImage(i))
                    .padding(10)
            } else {
                button("Download")
                    .on_press(Message::DownloadOsImage(i))
                    .padding(10)
            };

            row![image_info, action_button,]
                .spacing(20)
                .padding(10)
                .width(Length::Fill)
                .into()
        }))
        .spacing(10)
        .width(Length::Fill);

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let next_button = if self.selected_os_image.is_some() {
            button("Next: Configure Settings")
                .on_press(Message::ConfigureSettings)
                .padding(10)
        } else {
            button("Next: Configure Settings").padding(10)
        };

        let back_button = button("Back to Main Menu")
            .on_press(Message::BackToMainMenu)
            .padding(10);

        let buttons = row![back_button, next_button,]
            .spacing(10)
            .width(Length::Fill)
            .align_y(Vertical::Center);

        let content = column![
            title,
            os_image_list,
            spacer, // This spacer will push the buttons to the bottom
            buttons,
        ]
        .spacing(20)
        .padding(20)
        .width(Length::Fill);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_configure_settings(&self) -> Element<Message> {
        let title = text("Configure OS Settings")
            .size(30)
            .width(Length::Fill)
            .align_x(Horizontal::Center);

        // In a real app, we would have form fields here for hostname, user/password, network settings, etc.
        // For this example, we'll just use placeholder text

        let hostname = text("Hostname: golem-gpu").size(16);
        let password = text("Password: ********").size(16);
        let network = text("Network: DHCP").size(16);
        let ssh = text("SSH: Enabled").size(16);

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let back_button = button("Back to OS Selection")
            .on_press(Message::SelectOsImage(self.selected_os_image.unwrap_or(0)))
            .padding(10);

        let next_button = button("Next: Select Target Device")
            .on_press(Message::SelectTargetDevice(0))
            .padding(10);

        let buttons = row![back_button, next_button,]
            .spacing(10)
            .width(Length::Fill)
            .align_y(Alignment::Center);

        let content = column![title, hostname, password, network, ssh, spacer, buttons,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_select_target_device(&self) -> Element<Message> {
        let title = text("Select Target Device")
            .size(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center);

        let warning = text("Warning: All data on the selected device will be erased!")
            .size(16)
            .color(Color::from_rgb(1.0, 0.0, 0.0));

        let device_list = column(self.storage_devices.iter().enumerate().map(|(i, device)| {
            let device_info = column![
                text(&device.name).size(20),
                text(format!("Path: {}", device.path)).size(16),
                text(format!("Size: {}", device.size)).size(16),
            ]
            .spacing(5)
            .width(Length::Fill);

            let select_button = button("Select")
                .on_press(Message::SelectTargetDevice(i))
                .padding(10);

            row![device_info, select_button,]
                .spacing(20)
                .padding(10)
                .width(Length::Fill)
                .into()
        }))
        .spacing(10)
        .width(Length::Fill);

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let back_button = button("Back to Configure Settings")
            .on_press(Message::ConfigureSettings)
            .padding(10);

        let write_button = button("Write Image")
            .on_press(Message::WriteImage)
            .padding(10);

        let buttons = row![back_button, write_button,]
            .spacing(10)
            .width(Length::Fill)
            .align_y(Alignment::Center);

        let content = column![title, warning, device_list, spacer, buttons,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_writing_process(&self, progress: f32) -> Element<Message> {
        let title = text("Writing Image")
            .size(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center);

        let progress_text = text(format!("Progress: {}%", (progress * 100.0) as i32)).size(16);

        // In a real app, we would have a progress bar here
        // For now, we'll just use text

        // Add a spacer to push the button to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let cancel_button = button("Cancel").on_press(Message::CancelWrite).padding(10);

        let content = column![title, progress_text, spacer, cancel_button,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_flash_completion(&self, success: bool) -> Element<Message> {
        let title = if success {
            text("Success!")
                .size(30)
                .color(Color::from_rgb(0.0, 0.8, 0.0))
        } else {
            text("Error!")
                .size(30)
                .color(Color::from_rgb(0.8, 0.0, 0.0))
        };

        let message = if success {
            text("The image was successfully written to the device.")
        } else {
            text("There was an error writing the image to the device.")
        };

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let flash_another_button = button("Flash Another Device")
            .on_press(Message::FlashAnother)
            .padding(10);

        let exit_button = button("Exit").on_press(Message::Exit).padding(10);

        let buttons = row![flash_another_button, exit_button,]
            .spacing(10)
            .width(Length::Fill)
            .align_y(Alignment::Center);

        let content = column![title, message, spacer, buttons,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Shrink)
            .center_y(Length::Shrink)
            .into()
    }

    fn view_select_existing_device(&self) -> Element<Message> {
        let title = text("Select Existing Device")
            .size(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center);

        let device_list = column(self.storage_devices.iter().enumerate().map(|(i, device)| {
            let device_info = column![
                text(&device.name).size(20),
                text(format!("Path: {}", device.path)).size(16),
                text(format!("Size: {}", device.size)).size(16),
            ]
            .spacing(5)
            .width(Length::Fill);

            let select_button = button("Select")
                .on_press(Message::SelectExistingDevice(i))
                .padding(10);

            row![device_info, select_button,]
                .spacing(20)
                .padding(10)
                .width(Length::Fill)
                .into()
        }))
        .spacing(10)
        .width(Length::Fill);

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let back_button = button("Back to Main Menu")
            .on_press(Message::BackToMainMenu)
            .padding(10);

        let content = column![title, device_list, spacer, back_button,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Shrink)
            .into()
    }

    fn view_edit_configuration(&self) -> Element<Message> {
        let title = text("Edit Configuration")
            .size(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center);

        // In a real app, we would have form fields here for hostname, network settings, etc.
        // For this example, we'll just use placeholder text

        let hostname = text("Hostname: golem-gpu").size(16);
        let network = text("Network: DHCP").size(16);
        let ssh = text("SSH: Enabled").size(16);

        // Add a spacer to push buttons to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let back_button = button("Back to Device Selection")
            .on_press(Message::SelectExistingDevice(
                self.selected_device.unwrap_or(0),
            ))
            .padding(10);

        let save_button = button("Save Changes")
            .on_press(Message::SaveConfiguration)
            .padding(10);

        let buttons = row![back_button, save_button,]
            .spacing(10)
            .width(Length::Fill)
            .align_y(Alignment::Center);

        let content = column![title, hostname, network, ssh, spacer, buttons,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Shrink)
            .into()
    }

    fn view_edit_completion(&self, success: bool) -> Element<Message> {
        let title = if success {
            text("Success!")
                .size(30)
                .color(Color::from_rgb(0.0, 0.8, 0.0))
        } else {
            text("Error!")
                .size(30)
                .color(Color::from_rgb(0.8, 0.0, 0.0))
        };

        let message = if success {
            text("The configuration was successfully saved.")
        } else {
            text("There was an error saving the configuration.")
        };
        
        // Add a spacer to push the button to the bottom
        let spacer = Container::new(Column::new())
            .height(Length::Fill)
            .width(Length::Fill);

        let back_button = button("Back to Main Menu")
            .on_press(Message::BackToMainMenu)
            .padding(10);

        let content = column![title, message, spacer, back_button,]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Shrink)
            .center_y(Length::Shrink)
            .into()
    }
}
