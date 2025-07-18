use super::{PresetEditor, PresetEditorMessage, PresetManagerMessage, PresetManagerState};
use crate::models::ConfigurationPreset;
use crate::utils::PresetManager;
use iced::Task;
use std::path::PathBuf;
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

                // Clear deletion confirmation
                state.deletion_confirmation = None;

                info!("Deleted preset: {}", preset_name);
            }
            Task::none()
        }

        PresetManagerMessage::ConfirmDeletePreset(index) => {
            if index < state.presets.len() {
                let preset_name = state.presets[index].name.clone();
                state.deletion_confirmation = Some((index, preset_name));
            }
            Task::none()
        }

        PresetManagerMessage::CancelDeleteConfirmation => {
            state.deletion_confirmation = None;
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

        PresetManagerMessage::SaveAsPreset(mut new_preset) => {
            // Handle both manual creation (with new_preset_name) and import (with preset.name)
            if !state.new_preset_name.trim().is_empty() {
                // Manual creation: Use the user-entered name
                new_preset.name = state.new_preset_name.clone();
                new_preset.is_default = false; // New presets are not default by default

                let preset_name = new_preset.name.clone();
                state.presets.push(new_preset.clone());

                // Update preset manager if available
                if let Some(manager) = preset_manager {
                    let _ = manager.add_preset(new_preset);
                }

                state.new_preset_name.clear();
                info!("Created new preset: {}", preset_name);
            } else if !new_preset.name.trim().is_empty() {
                // Import: Use the preset's existing name
                new_preset.is_default = false; // Imported presets are not default by default

                let preset_name = new_preset.name.clone();
                state.presets.push(new_preset.clone());

                // Update preset manager if available
                if let Some(manager) = preset_manager {
                    let _ = manager.add_preset(new_preset);
                }

                info!("Added imported preset: {}", preset_name);
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

        PresetManagerMessage::SetNewPresetName(name) => {
            state.new_preset_name = name;
            Task::none()
        }

        PresetManagerMessage::CreatePreset => {
            if !state.new_preset_name.trim().is_empty() {
                state.editor = Some(PresetEditor::new_preset());
                if let Some(editor) = &mut state.editor {
                    editor.name = state.new_preset_name.clone();
                }
                state.new_preset_name.clear();
            }
            Task::none()
        }

        PresetManagerMessage::EditPreset(index) => {
            if let Some(preset) = state.presets.get(index) {
                state.editor = Some(PresetEditor::new(index, preset));
            }
            Task::none()
        }

        PresetManagerMessage::SavePreset => {
            if let Some(editor) = &state.editor {
                if editor.is_valid() {
                    let preset = editor.to_preset();

                    if let Some(index) = editor.editing_index {
                        // Update existing preset
                        if index < state.presets.len() {
                            let preset_name = preset.name.clone();
                            state.presets[index] = preset.clone();
                            if let Some(manager) = preset_manager {
                                let _ = manager.update_preset(index, preset);
                            }
                            info!("Updated preset: {}", preset_name);
                        }
                    } else {
                        // Create new preset
                        state.presets.push(preset.clone());
                        if let Some(manager) = preset_manager {
                            let _ = manager.add_preset(preset.clone());
                        }
                        info!("Created new preset: {}", preset.name);
                    }

                    state.editor = None;
                }
            }
            Task::none()
        }

        PresetManagerMessage::CancelEdit => {
            state.editor = None;
            Task::none()
        }

        // These messages are no longer used - configuration changes are handled
        // through PresetEditorMessage::Configuration(ConfigurationMessage)
        PresetManagerMessage::DuplicatePreset(index) => {
            if let Some(preset) = state.presets.get(index) {
                let mut duplicated = preset.clone();
                duplicated.name = format!("{} Copy", preset.name);
                duplicated.is_default = false; // Duplicates are never default
                state.presets.push(duplicated.clone());

                if let Some(manager) = preset_manager {
                    let _ = manager.add_preset(duplicated.clone());
                }

                info!("Duplicated preset: {}", duplicated.name);
            }
            Task::none()
        }

        PresetManagerMessage::ExportPreset(index) => {
            if let Some(preset) = state.presets.get(index) {
                let preset_name = preset.name.clone();
                debug!("Starting export for preset: {}", preset_name);

                Task::perform(export_preset_dialog(preset_name), move |path_opt| {
                    if let Some(path) = path_opt {
                        crate::ui::messages::Message::PresetManager(
                            PresetManagerMessage::ExportPresetToFile(index, path),
                        )
                    } else {
                        crate::ui::messages::Message::PresetManager(
                            PresetManagerMessage::CancelEdit,
                        ) // No-op message
                    }
                })
            } else {
                Task::none()
            }
        }

        PresetManagerMessage::ImportPreset => {
            debug!("Starting preset import");
            Task::perform(import_preset_dialog(), |path_opt| {
                if let Some(path) = path_opt {
                    crate::ui::messages::Message::PresetManager(
                        PresetManagerMessage::ImportPresetFromFile(path),
                    )
                } else {
                    crate::ui::messages::Message::PresetManager(PresetManagerMessage::CancelEdit) // No-op message
                }
            })
        }

        PresetManagerMessage::ExportPresetToFile(index, path) => {
            if let Some(preset) = state.presets.get(index) {
                Task::perform(save_preset_to_file(preset.clone(), path), |result| {
                    match result {
                        Ok(_) => {
                            info!("Preset exported successfully");
                            crate::ui::messages::Message::PresetManager(
                                PresetManagerMessage::CancelEdit,
                            ) // No-op message
                        }
                        Err(e) => {
                            error!("Failed to export preset: {}", e);
                            crate::ui::messages::Message::PresetManager(
                                PresetManagerMessage::CancelEdit,
                            ) // No-op message
                        }
                    }
                })
            } else {
                Task::none()
            }
        }

        PresetManagerMessage::ImportPresetFromFile(path) => {
            Task::perform(load_preset_from_file(path), |result| {
                match result {
                    Ok(preset) => {
                        info!("Preset imported successfully: {}", preset.name);
                        crate::ui::messages::Message::PresetManager(
                            PresetManagerMessage::SaveAsPreset(preset),
                        )
                    }
                    Err(e) => {
                        error!("Failed to import preset: {}", e);
                        crate::ui::messages::Message::PresetManager(
                            PresetManagerMessage::CancelEdit,
                        ) // No-op message
                    }
                }
            })
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

                    if let Some(index) = editor.editing_index {
                        if index < state.presets.len() {
                            state.presets[index] = updated_preset.clone();

                            // Update in preset manager if available
                            if let Some(manager) = preset_manager {
                                let _ = manager.update_preset(index, updated_preset);
                            }
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

        PresetEditorMessage::Configuration(config_msg) => {
            if let Some(editor) = &mut state.editor {
                // Delegate configuration changes to the configuration handler
                let task = crate::ui::configuration::handle_message(
                    &mut editor.configuration,
                    &state.presets,
                    config_msg,
                );
                
                // Map the result messages to the correct context
                task.map(|msg| match msg {
                    crate::ui::messages::Message::Configuration(config_msg) => {
                        crate::ui::messages::Message::PresetManager(
                            PresetManagerMessage::Editor(
                                PresetEditorMessage::Configuration(config_msg)
                            )
                        )
                    }
                    other => other,
                })
            } else {
                Task::none()
            }
        }
    }
}

// Preset export/import file format
#[derive(serde::Serialize, serde::Deserialize)]
struct PresetFileFormat {
    version: String,
    exported_at: String,
    preset: ConfigurationPreset,
}

// Async functions for file operations
async fn export_preset_dialog(preset_name: String) -> Option<PathBuf> {
    let suggested_filename = format!("{}.json", preset_name.replace(' ', "_"));

    rfd::AsyncFileDialog::new()
        .set_title("Export Preset")
        .set_file_name(&suggested_filename)
        .add_filter("JSON files", &["json"])
        .save_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

async fn import_preset_dialog() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Import Preset")
        .add_filter("JSON files", &["json"])
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

async fn save_preset_to_file(
    preset: ConfigurationPreset,
    path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let preset_file = PresetFileFormat {
        version: "1.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        preset,
    };

    let json_content = serde_json::to_string_pretty(&preset_file)?;
    tokio::fs::write(&path, json_content).await?;

    Ok(())
}

async fn load_preset_from_file(
    path: PathBuf,
) -> Result<ConfigurationPreset, Box<dyn std::error::Error + Send + Sync>> {
    let file_content = tokio::fs::read_to_string(&path).await?;
    let preset_file: PresetFileFormat = serde_json::from_str(&file_content)?;

    // Handle name conflicts by appending " (Imported)"
    let mut preset = preset_file.preset;
    if !preset.name.ends_with(" (Imported)") {
        preset.name = format!("{} (Imported)", preset.name);
    }
    preset.is_default = false; // Imported presets are never default

    Ok(preset)
}
