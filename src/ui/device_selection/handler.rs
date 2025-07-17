use super::{DeviceMessage, DeviceSelectionState, StorageDevice};
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

            Task::perform(
                async {
                    // Run the blocking rs_drivelist call in a blocking task
                    tokio::task::spawn_blocking(|| {
                        info!("Getting available storage devices");
                        match rs_drivelist::drive_list() {
                            Ok(devices) => {
                                // Filter to only include removable, non-virtual devices
                                let storage_devices: Vec<StorageDevice> = devices
                                    .into_iter()
                                    .filter(|d| d.isRemovable && !d.isVirtual)
                                    .map(|d| StorageDevice {
                                        name: d.description,
                                        path: r"\\.\F:".to_string(),
                                        size: format!(
                                            "{:.2} GB",
                                            d.size as f64 / 1000.0 / 1000.0 / 1000.0
                                        ),
                                        is_card: d.isCard,
                                        is_usb: d.isUSB,
                                        is_scsi: d.isSCSI,
                                        is_removable: d.isRemovable,
                                    })
                                    .collect();

                                debug!("Found {} available devices", storage_devices.len());
                                Ok(storage_devices)
                            }
                            Err(e) => {
                                error!("Failed to get drive list: {}", e);
                                Err(format!("Failed to detect storage devices: {}", e))
                            }
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Task failed: {}", e)))
                },
                |result| match result {
                    Ok(devices) => crate::ui::messages::Message::DeviceSelection(
                        DeviceMessage::DevicesLoaded(devices),
                    ),
                    Err(error) => crate::ui::messages::Message::DeviceSelection(
                        DeviceMessage::DeviceLoadFailed(error),
                    ),
                },
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
