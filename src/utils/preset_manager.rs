use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::models::{ConfigurationPreset, NetworkType, PaymentNetwork};
#[cfg(feature = "profile-susteen")]
use reqwest;

/// Struct to hold configuration presets and manage their persistence
pub struct PresetManager {
    presets: Vec<ConfigurationPreset>,
    config_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct PresetsToml {
    presets: Vec<ConfigurationPreset>,
}

impl PresetManager {
    /// Create a new PresetManager instance
    pub fn new() -> Result<Self, String> {
        // Get the project directories
        let project_dirs = ProjectDirs::from("com", "golem", "golem-gpu-imager")
            .ok_or_else(|| "Failed to determine project directories".to_string())?;

        // Get the config directory
        let config_dir = project_dirs.config_dir().to_path_buf();

        // Create the config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Initialize with empty presets
        Ok(Self {
            presets: Vec::new(),
            config_dir,
        })
    }

    /// Initialize with default presets if no presets exist
    #[cfg(not(any(feature = "enterprise", feature = "profile-susteen")))]
    pub fn init_with_defaults(&mut self) -> Result<(), String> {
        // If presets file exists, load it
        if self.presets_file_exists() {
            self.load_presets()?;
        }

        // If no presets were loaded, create defaults
        if self.presets.is_empty() {
            self.create_default_presets();
        }

        Ok(())
    }

    /// Get the list of all presets
    pub fn get_presets(&self) -> &Vec<ConfigurationPreset> {
        &self.presets
    }

    /// Get the default preset (if exists)
    pub fn get_default_preset(&self) -> Option<&ConfigurationPreset> {
        self.presets.iter().find(|p| p.is_default)
    }

    /// Add a new preset
    #[cfg(not(feature = "profile-susteen"))]
    pub fn add_preset(&mut self, preset: ConfigurationPreset) -> Result<(), String> {
        // If this is the first preset, make it default
        let is_first = self.presets.is_empty();

        // If the preset is being set as default, unset default on all other presets
        if preset.is_default || is_first {
            for p in &mut self.presets {
                p.is_default = false;
            }
        }

        // Ensure first preset is default
        let mut new_preset = preset;
        if is_first {
            new_preset.is_default = true;
        }

        self.presets.push(new_preset);
        self.save_presets()?;

        Ok(())
    }

    /// Update an existing preset
    #[allow(dead_code)]
    #[cfg(not(feature = "profile-susteen"))]
    pub fn update_preset(
        &mut self,
        index: usize,
        preset: ConfigurationPreset,
    ) -> Result<(), String> {
        if index >= self.presets.len() {
            return Err("Preset index out of bounds".to_string());
        }

        // If the preset is being set as default, unset default on all other presets
        if preset.is_default {
            for p in &mut self.presets {
                p.is_default = false;
            }
        }

        self.presets[index] = preset;
        self.save_presets()?;

        Ok(())
    }

    /// Set a preset as default
    #[cfg(not(feature = "profile-susteen"))]
    pub fn set_default_preset(&mut self, index: usize) -> Result<(), String> {
        if index >= self.presets.len() {
            return Err("Preset index out of bounds".to_string());
        }

        // Unset default on all presets
        for p in &mut self.presets {
            p.is_default = false;
        }

        // Set the selected preset as default
        self.presets[index].is_default = true;

        self.save_presets()?;

        Ok(())
    }

    /// Delete a preset
    #[cfg(not(feature = "profile-susteen"))]
    pub fn delete_preset(&mut self, index: usize) -> Result<(), String> {
        if index >= self.presets.len() {
            return Err("Preset index out of bounds".to_string());
        }

        let was_default = self.presets[index].is_default;

        // Remove the preset
        self.presets.remove(index);

        // If the deleted preset was default and we still have presets, set the first one as default
        if was_default && !self.presets.is_empty() {
            self.presets[0].is_default = true;
        }

        self.save_presets()?;

        Ok(())
    }

    /// Create default presets
    #[allow(dead_code)]
    fn create_default_presets(&mut self) {
        self.presets = vec![
            ConfigurationPreset {
                name: "Testnet Development".to_string(),
                payment_network: PaymentNetwork::Testnet,
                subnet: "public".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "".to_string(),
                is_default: true,
                non_interactive_install: false,
                ssh_keys: Vec::new(),
                configuration_server: None,
                metrics_server: None,
                central_net_host: None,
            },
            ConfigurationPreset {
                name: "Mainnet Production".to_string(),
                payment_network: PaymentNetwork::Mainnet,
                subnet: "production".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "".to_string(),
                is_default: false,
                non_interactive_install: false,
                ssh_keys: Vec::new(),
                configuration_server: None,
                metrics_server: None,
                central_net_host: None,
            },
            ConfigurationPreset {
                name: "Susteen Support".to_string(),
                payment_network: PaymentNetwork::Testnet,
                subnet: "susteen".to_string(),
                network_type: NetworkType::Central,
                wallet_address: "0x206bfe4F439a83b65A5B9c2C3B1cc6cB49054cc4".to_string(),
                is_default: false,
                non_interactive_install: true,
                ssh_keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPeI8LZGexCdqXozb+gPKnZCQLr7AlXqRCgJpM9eS/y3 reqc@pop-os".to_string()],
                configuration_server: Some("http://63.176.129.155/config.toml".to_string()),
                metrics_server: Some("http://63.176.129.155:9091".to_string()),
                central_net_host: None,
            },
        ];

        // Save the default presets to disk
        let _ = self.save_presets();
    }

    /// Load presets from the configuration file
    fn load_presets(&mut self) -> Result<(), String> {
        let presets_path = self.get_presets_path();

        // Check if the file exists
        if !presets_path.exists() {
            return Ok(());
        }

        // Read the file content
        let mut file =
            File::open(&presets_path).map_err(|e| format!("Failed to open presets file: {}", e))?;

        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| format!("Failed to read presets file: {}", e))?;

        // Parse the TOML content
        let presets_toml: PresetsToml =
            toml::from_str(&content).map_err(|e| format!("Failed to parse presets TOML: {}", e))?;

        // Update the presets
        self.presets = presets_toml.presets;

        Ok(())
    }

    /// Save presets to the configuration file
    #[cfg(not(feature = "profile-susteen"))]
    fn save_presets(&self) -> Result<(), String> {
        let presets_path = self.get_presets_path();

        // Ensure the config directory exists
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Create the presets TOML structure
        let presets_toml = PresetsToml {
            presets: self.presets.clone(),
        };

        // Serialize to TOML
        let toml_content = toml::to_string(&presets_toml)
            .map_err(|e| format!("Failed to serialize presets to TOML: {}", e))?;

        // Write to the file
        let mut file = File::create(&presets_path)
            .map_err(|e| format!("Failed to create presets file: {}", e))?;

        file.write_all(toml_content.as_bytes())
            .map_err(|e| format!("Failed to write presets to file: {}", e))?;

        Ok(())
    }

    /// Check if the presets file exists
    fn presets_file_exists(&self) -> bool {
        self.get_presets_path().exists()
    }

    /// Get the path to the presets file
    fn get_presets_path(&self) -> PathBuf {
        self.config_dir.join("presets.toml")
    }
}

#[cfg(feature = "enterprise")]
impl PresetManager {
    // This function would only be compiled when the "enterprise" feature is enabled
    fn create_enterprise_presets(&mut self) {
        self.presets = vec![ConfigurationPreset {
            name: "Enterprise Standard".to_string(),
            payment_network: PaymentNetwork::Mainnet,
            subnet: "enterprise".to_string(),
            network_type: NetworkType::Central,
            wallet_address: "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string(),
            is_default: true,
        }];

        // Save the enterprise presets to disk
        let _ = self.save_presets();
    }

    // Override init_with_defaults to use enterprise presets
    #[cfg(feature = "enterprise")]
    pub fn init_with_defaults(&mut self) -> Result<(), String> {
        // If presets file exists, load it
        if self.presets_file_exists() {
            self.load_presets()?;
        }

        // If no presets were loaded, create enterprise defaults
        if self.presets.is_empty() {
            self.create_enterprise_presets();
        }

        Ok(())
    }
}

#[cfg(feature = "profile-susteen")]
impl PresetManager {
    /// Create susteen preset from remote configuration
    async fn create_susteen_preset() -> Result<ConfigurationPreset, String> {
        let config_url = "http://63.176.129.155/config.toml";
        
        // Fetch configuration from remote URL
        let response = reqwest::get(config_url)
            .await
            .map_err(|e| format!("Failed to fetch configuration from {}: {}", config_url, e))?;
        
        if !response.status().is_success() {
            return Err(format!("HTTP error {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
        }
        
        let config_text = response.text()
            .await
            .map_err(|e| format!("Failed to read configuration text: {}", e))?;
        
        // Parse the TOML configuration
        let config: toml::Value = toml::from_str(&config_text)
            .map_err(|e| format!("Failed to parse configuration TOML: {}", e))?;
        
        // Extract configuration values with defaults
        // Extract from env section for environment variables
        let env_section = config.get("env");
        
        let payment_network = env_section
            .and_then(|env| env.get("YA_PAYMENT_NETWORK_GROUP"))
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "mainnet" => PaymentNetwork::Mainnet,
                _ => PaymentNetwork::Testnet,
            })
            .unwrap_or(PaymentNetwork::Testnet);
        
        let network_type = env_section
            .and_then(|env| env.get("YA_NET_TYPE"))
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "hybrid" => NetworkType::Hybrid,
                _ => NetworkType::Central,
            })
            .unwrap_or(NetworkType::Central);
        
        let subnet = env_section
            .and_then(|env| env.get("SUBNET"))
            .and_then(|v| v.as_str())
            .unwrap_or("susteen")
            .to_string();
        
        // Extract from top-level fields
        let wallet_address = config.get("glm_account")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let non_interactive_install = config.get("non_interactive_install")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        
        let ssh_keys = config.get("ssh_keys")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        
        let configuration_server = config.get("configuration_server")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        let metrics_server = env_section
            .and_then(|env| env.get("YAGNA_METRICS_URL"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        let central_net_host = env_section
            .and_then(|env| env.get("CENTRAL_NET_HOST"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        Ok(ConfigurationPreset {
            name: "susteen".to_string(),
            payment_network,
            subnet,
            network_type,
            wallet_address,
            is_default: true,
            non_interactive_install,
            ssh_keys,
            configuration_server,
            metrics_server,
            central_net_host,
        })
    }
    
    /// Create susteen presets from remote configuration (virtual, not saved to disk)
    async fn create_susteen_presets(&mut self) -> Result<(), String> {
        let susteen_preset = Self::create_susteen_preset().await?;
        self.presets = vec![susteen_preset];
        
        // Do not save to disk - keep this preset virtual/ephemeral
        Ok(())
    }
    
    /// Override init_with_defaults to use susteen preset
    pub fn init_with_defaults(&mut self) -> Result<(), String> {
        // In profile-susteen mode, never load from disk - always create fresh virtual preset
        // Use tokio runtime to run async function
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        
        rt.block_on(async {
            self.create_susteen_presets().await
        })?;
        
        Ok(())
    }
    
    /// Override save_presets to be a no-op in profile-susteen mode
    fn save_presets(&self) -> Result<(), String> {
        // In profile-susteen mode, never save to disk - presets are virtual/ephemeral
        Ok(())
    }
    
    /// Override add_preset to prevent disk persistence
    pub fn add_preset(&mut self, _preset: ConfigurationPreset) -> Result<(), String> {
        // In profile-susteen mode, don't allow adding presets
        Err("Cannot add presets in profile-susteen mode".to_string())
    }
    
    /// Override update_preset to prevent disk persistence
    pub fn update_preset(&mut self, _index: usize, _preset: ConfigurationPreset) -> Result<(), String> {
        // In profile-susteen mode, don't allow updating presets
        Err("Cannot update presets in profile-susteen mode".to_string())
    }
    
    /// Override set_default_preset to prevent disk persistence
    pub fn set_default_preset(&mut self, _index: usize) -> Result<(), String> {
        // In profile-susteen mode, don't allow changing default (susteen is always default)
        Err("Cannot change default preset in profile-susteen mode".to_string())
    }
    
    /// Override delete_preset to prevent disk persistence
    pub fn delete_preset(&mut self, _index: usize) -> Result<(), String> {
        // In profile-susteen mode, don't allow deleting presets
        Err("Cannot delete presets in profile-susteen mode".to_string())
    }
}
