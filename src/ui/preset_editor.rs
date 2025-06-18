use crate::models::ConfigurationPreset;
use crate::ui::preset_manager::{PresetEditor, PresetEditorMessage};
use crate::utils::PresetManager;

pub struct PresetEditorHandler;

impl PresetEditorHandler {
    pub fn handle_message(
        editor: &mut Option<PresetEditor>,
        presets: &mut Vec<ConfigurationPreset>,
        preset_manager: &mut Option<PresetManager>,
        message: PresetEditorMessage,
    ) {
        match message {
            PresetEditorMessage::Start(index) => {
                if let Some(preset) = presets.get(index) {
                    *editor = Some(PresetEditor::new(index, preset));
                }
            }
            PresetEditorMessage::Cancel => {
                *editor = None;
            }
            PresetEditorMessage::Save => {
                if let Some(editor_instance) = editor {
                    if editor_instance.is_valid() {
                        let updated_preset = editor_instance.to_preset();
                        let index = editor_instance.editing_index.unwrap_or(0);

                        if index < presets.len() {
                            presets[index] = updated_preset.clone();

                            // Update in preset manager if available
                            if let Some(manager) = preset_manager {
                                let _ = manager.update_preset(index, updated_preset);
                            }
                        }

                        *editor = None;
                    }
                }
            }
            PresetEditorMessage::UpdateName(name) => {
                if let Some(editor_instance) = editor {
                    editor_instance.name = name;
                }
            }
            PresetEditorMessage::UpdatePaymentNetwork(network) => {
                if let Some(editor_instance) = editor {
                    editor_instance.payment_network = network;
                }
            }
            PresetEditorMessage::UpdateSubnet(subnet) => {
                if let Some(editor_instance) = editor {
                    editor_instance.subnet = subnet;
                }
            }
            PresetEditorMessage::UpdateNetworkType(network_type) => {
                if let Some(editor_instance) = editor {
                    editor_instance.network_type = network_type;
                }
            }
            PresetEditorMessage::UpdateWalletAddress(address) => {
                if let Some(editor_instance) = editor {
                    editor_instance.wallet_address = address;
                }
            }
        }
    }
}
