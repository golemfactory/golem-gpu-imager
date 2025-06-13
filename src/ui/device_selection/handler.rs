use super::{DeviceSelectionState, DeviceMessage, StorageDevice};
use iced::Task;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut DeviceSelectionState,
    message: DeviceMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        DeviceMessage::RefreshDevices => {
            state.is_refreshing = true;
            state.error_message = None;
            debug!("Starting device refresh");
            
            // This would typically enumerate devices
            // For now, simulate with a task
            Task::perform(
                async {
                    // Simulate device enumeration
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    
                    // Return mock devices for now
                    vec![
                        StorageDevice {
                            name: "USB Drive".to_string(),
                            path: "/dev/sdb".to_string(),
                            size: "32 GB".to_string(),
                        },
                        StorageDevice {
                            name: "SD Card".to_string(),
                            path: "/dev/sdc".to_string(),
                            size: "16 GB".to_string(),
                        },
                    ]
                },
                |devices| {
                    crate::ui::messages::Message::DeviceSelection(DeviceMessage::DevicesLoaded(devices))
                }
            )
        }
        
        DeviceMessage::DevicesLoaded(devices) => {
            state.devices = devices;
            state.is_refreshing = false;
            state.selected_device = None;
            info!("Loaded {} devices", state.devices.len());
            Task::none()
        }
        
        DeviceMessage::DeviceLoadFailed(error) => {
            state.is_refreshing = false;
            state.error_message = Some(error.clone());
            error!("Failed to load devices: {}", error);
            Task::none()
        }
        
        DeviceMessage::SelectDevice(index) => {
            if index < state.devices.len() {
                state.selected_device = Some(index);
                debug!("Selected device: {}", index);
            }
            Task::none()
        }
        
        DeviceMessage::ClearSelection => {
            state.selected_device = None;
            debug!("Cleared device selection");
            Task::none()
        }
    }
}