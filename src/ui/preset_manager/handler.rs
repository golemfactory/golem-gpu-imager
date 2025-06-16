use super::{PresetManagerState, PresetManagerMessage, PresetEditorMessage, PresetEditor};
use crate::models::ConfigurationPreset;
use crate::utils::PresetManager;
use iced::Task;
use tracing::{debug, error, info};

pub fn handle_message(
    state: &mut PresetManagerState,
    preset_manager: &mut Option<PresetManager>,
    message: PresetManagerMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        PresetManagerMessage::ToggleManager => {
            state.show_manager = !state.show_manager;
            debug!("Toggled preset manager: show={}", state.show_manager);
            Task::none()
        }
        
        PresetManagerMessage::SelectPreset(index) => {
            if index < state.presets.len() {
                state.selected_preset = Some(index);
                debug!("Selected preset: {}", index);
            }
            Task::none()
        }
        
        PresetManagerMessage::DeletePreset(index) => {
            if index < state.presets.len() {
                let preset_name = state.presets[index].name.clone();
                state.presets.remove(index);
                
                // Update preset manager if available
                if let Some(manager) = preset_manager {
                    let _ = manager.delete_preset(index);
                }
                
                // Update selected index if needed
                if let Some(selected) = state.selected_preset {
                    if selected == index {
                        state.selected_preset = None;
                    } else if selected > index {
                        state.selected_preset = Some(selected - 1);
                    }
                }
                
                info!("Deleted preset: {}", preset_name);
            }
            Task::none()
        }
        
        PresetManagerMessage::SetDefaultPreset(index) => {
            if index < state.presets.len() {
                // Clear all default flags first
                for preset in &mut state.presets {
                    preset.is_default = false;
                }
                
                // Set the selected one as default
                state.presets[index].is_default = true;
                
                // Update preset manager if available
                if let Some(manager) = preset_manager {
                    let _ = manager.set_default_preset(index);
                }
                
                info!("Set default preset: {}", state.presets[index].name);
            }
            Task::none()
        }
        
        PresetManagerMessage::SetPresetName(name) => {
            state.new_preset_name = name;
            Task::none()
        }
        
        PresetManagerMessage::SaveAsPreset => {
            if !state.new_preset_name.trim().is_empty() {
                // This would need current configuration from the calling context
                // For now, create a placeholder
                let new_preset = ConfigurationPreset {
                    name: state.new_preset_name.clone(),
                    payment_network: crate::models::PaymentNetwork::Testnet,
                    subnet: "public".to_string(),
                    network_type: crate::models::NetworkType::Central,
                    wallet_address: String::new(),
                    is_default: false,
                };
                
                let preset_name = new_preset.name.clone();
                state.presets.push(new_preset.clone());
                
                // Update preset manager if available
                if let Some(manager) = preset_manager {
                    let _ = manager.add_preset(new_preset);
                }
                
                state.new_preset_name.clear();
                info!("Created new preset: {}", preset_name);
            }
            Task::none()
        }
        
        PresetManagerMessage::Editor(editor_message) => {
            handle_editor_message(state, preset_manager, editor_message)
        }
        
        PresetManagerMessage::SavePresetsToStorage => {
            // The preset manager automatically saves when presets are modified
            // This is a no-op for now since save_presets is private
            debug!("Save presets to storage requested");
            Task::none()
        }
        
        PresetManagerMessage::LoadPresetsFromStorage => {
            if let Some(manager) = preset_manager {
                state.presets = manager.get_presets().clone();
                info!("Presets loaded from storage");
            }
            Task::none()
        }
        
        PresetManagerMessage::BackToMainMenu => {
            // This should trigger the main app to switch back to StartScreen mode
            Task::done(crate::ui::messages::Message::BackToMainMenu)
        }
        
        _ => {
            debug!("Unhandled preset manager message: {:?}", message);
            Task::none()
        }
    }
}

fn handle_editor_message(
    state: &mut PresetManagerState,
    preset_manager: &mut Option<PresetManager>,
    message: PresetEditorMessage,
) -> Task<crate::ui::messages::Message> {
    match message {
        PresetEditorMessage::Start(index) => {
            if let Some(preset) = state.presets.get(index) {
                state.editor = Some(PresetEditor::new(index, preset));
            }
            Task::none()
        }
        
        PresetEditorMessage::Cancel => {
            state.editor = None;
            Task::none()
        }
        
        PresetEditorMessage::Save => {
            if let Some(editor) = &state.editor {
                if editor.is_valid() {
                    let updated_preset = editor.to_preset();
                    let index = editor.preset_index;
                    
                    if index < state.presets.len() {
                        state.presets[index] = updated_preset.clone();
                        
                        // Update in preset manager if available
                        if let Some(manager) = preset_manager {
                            let _ = manager.update_preset(index, updated_preset);
                        }
                    }
                    
                    state.editor = None;
                    info!("Saved preset changes");
                }
            }
            Task::none()
        }
        
        PresetEditorMessage::UpdateName(name) => {
            if let Some(editor) = &mut state.editor {
                editor.name = name;
            }
            Task::none()
        }
        
        PresetEditorMessage::UpdatePaymentNetwork(network) => {
            if let Some(editor) = &mut state.editor {
                editor.payment_network = network;
            }
            Task::none()
        }
        
        PresetEditorMessage::UpdateSubnet(subnet) => {
            if let Some(editor) = &mut state.editor {
                editor.subnet = subnet;
            }
            Task::none()
        }
        
        PresetEditorMessage::UpdateNetworkType(network_type) => {
            if let Some(editor) = &mut state.editor {
                editor.network_type = network_type;
            }
            Task::none()
        }
        
        PresetEditorMessage::UpdateWalletAddress(address) => {
            if let Some(editor) = &mut state.editor {
                editor.wallet_address = address;
            }
            Task::none()
        }
    }
}