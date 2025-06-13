use crate::disk::Disk;
use crate::models::{AppMode, CancelToken, Message};
use crate::ui::{
    flash_workflow::{FlashState, FlashMessage},
    edit_workflow::{EditState, EditMessage},
    preset_manager::{PresetManagerState, PresetManagerMessage},
    device_selection::{DeviceSelectionState, DeviceMessage},
    configuration::{ConfigurationState, ConfigurationMessage},
};
use crate::utils::repo::ImageRepo;
use crate::utils::{PresetManager, image_metadata::MetadataManager};
use iced::{Element, Task};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

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
    pub cancel_token: CancelToken,
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
            cancel_token: CancelToken::new(),
            elevation_status,
            is_elevated,
            metadata_manager,
            preset_manager_backend,
            is_loading_repo: false,
            error_message: None,
        }
    }
}

impl iced::Application for GolemGpuImager {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Task<Message>) {
        let mut app = Self::new();
        let load_task = app.load_repo_data();
        (app, load_task)
    }

    fn title(&self) -> String {
        format!("Golem GPU Imager v{}", env!("CARGO_PKG_VERSION"))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // App-level messages
            Message::FlashNewImage => {
                self.mode = AppMode::FlashNewImage;
                self.flash_workflow = Some(FlashState::new());
                Task::none()
            }
            
            Message::EditExistingDisk => {
                self.mode = AppMode::EditExistingDisk;
                self.edit_workflow = Some(EditState::new());
                Task::none()
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
                        &self.cancel_token,
                        flash_msg,
                    )
                } else {
                    Task::none()
                }
            }
            
            Message::Edit(edit_msg) => {
                if let Some(edit_state) = &mut self.edit_workflow {
                    crate::ui::edit_workflow::handler::handle_message(edit_state, edit_msg)
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
                // This would trigger elevation request
                debug!("Elevation requested");
                Task::none()
            }
            
            Message::CheckElevationStatus => {
                self.elevation_status = crate::utils::get_elevation_status();
                self.is_elevated = crate::utils::is_elevated();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.mode {
            AppMode::StartScreen => {
                crate::ui::start_screen::view_start_screen()
            }
            AppMode::FlashNewImage => {
                if let Some(flash_state) = &self.flash_workflow {
                    // Return appropriate view based on flash workflow state
                    // This would need to be implemented with proper view delegation
                    crate::ui::start_screen::view_start_screen() // Placeholder
                } else {
                    crate::ui::start_screen::view_start_screen()
                }
            }
            AppMode::EditExistingDisk => {
                if let Some(edit_state) = &self.edit_workflow {
                    // Return appropriate view based on edit workflow state
                    // This would need to be implemented with proper view delegation
                    crate::ui::start_screen::view_start_screen() // Placeholder
                } else {
                    crate::ui::start_screen::view_start_screen()
                }
            }
            AppMode::ManagePresets => {
                // Return preset manager view
                crate::ui::start_screen::view_start_screen() // Placeholder
            }
        }
    }
}

impl GolemGpuImager {
    pub fn load_repo_data(&mut self) -> Task<Message> {
        self.is_loading_repo = true;
        
        let _repo = Arc::clone(&self.image_repo);
        let metadata_manager = self.metadata_manager.clone();

        Task::perform(
            async move {
                // Simulate repository loading
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                
                // Return mock data for now
                vec![]
            },
            |images| Message::RepoDataLoaded(images)
        )
    }
}