/// Configuration for image writing and partition setup
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ImageConfiguration {
    // Main TOML configuration fields
    pub accepted_terms: bool,
    pub glm_account: String,
    pub glm_per_hour: String,
    pub glm_node_name: Option<String>,
    pub non_interactive_install: bool,
    pub ssh_keys: Vec<String>,
    pub configuration_server: Option<String>,
    
    // Environment variables (stored in [env] section of TOML)
    pub payment_network: crate::models::PaymentNetwork,
    pub network_type: crate::models::NetworkType,
    pub subnet: String,
    pub central_net_host: Option<String>,
    pub metrics_server: Option<String>,
    pub metrics_job_name: Option<String>,
    pub metrics_group: Option<String>,
}

impl ImageConfiguration {
    /// Create a new ImageConfiguration with default values
    pub fn new(
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: String,
        glm_account: String,
    ) -> Self {
        Self {
            accepted_terms: true,
            glm_account,
            glm_per_hour: "0.25".to_string(),
            glm_node_name: None,
            non_interactive_install: false,
            ssh_keys: Vec::new(),
            configuration_server: None,
            payment_network,
            network_type,
            subnet,
            central_net_host: None,
            metrics_server: None,
            metrics_job_name: None,
            metrics_group: None,
        }
    }

    /// Create a new ImageConfiguration with all options
    pub fn new_with_options(
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: String,
        glm_account: String,
        non_interactive_install: bool,
        ssh_keys: String,
        configuration_server: String,
        metrics_server: String,
        central_net_host: String,
    ) -> Self {
        // Parse SSH keys from string (comma or newline separated)
        let ssh_keys_vec: Vec<String> = if ssh_keys.trim().is_empty() {
            Vec::new()
        } else if ssh_keys.contains('\n') {
            ssh_keys
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            ssh_keys
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };

        Self {
            accepted_terms: true,
            glm_account,
            glm_per_hour: "0.25".to_string(),
            glm_node_name: None,
            non_interactive_install,
            ssh_keys: ssh_keys_vec,
            configuration_server: if configuration_server.trim().is_empty() {
                None
            } else {
                Some(configuration_server)
            },
            payment_network,
            network_type,
            subnet,
            central_net_host: if central_net_host.trim().is_empty() {
                None
            } else {
                Some(central_net_host)
            },
            metrics_server: if metrics_server.trim().is_empty() {
                None
            } else {
                Some(metrics_server)
            },
            metrics_job_name: None,
            metrics_group: None,
        }
    }

    /// Ensure accepted_terms is set to true for new installations
    pub fn ensure_accepted_terms(&mut self) {
        self.accepted_terms = true;
    }

    /// Create ImageConfiguration from ENV variables
    pub fn from_env_variables(
        payment_network: crate::models::PaymentNetwork,
        network_type: crate::models::NetworkType,
        subnet: String,
        glm_account: String,
        non_interactive_install: bool,
        ssh_keys_vec: Vec<String>,
        configuration_server: String,
        central_net_host: String,
        metrics_server: String,
    ) -> Self {
        Self {
            accepted_terms: true,
            glm_account,
            glm_per_hour: "0.25".to_string(),
            glm_node_name: None,
            non_interactive_install,
            ssh_keys: ssh_keys_vec,
            configuration_server: if configuration_server.trim().is_empty() {
                None
            } else {
                Some(configuration_server)
            },
            payment_network,
            network_type,
            subnet,
            central_net_host: if central_net_host.trim().is_empty() {
                None
            } else {
                Some(central_net_host)
            },
            metrics_server: if metrics_server.trim().is_empty() {
                None
            } else {
                Some(metrics_server)
            },
            metrics_job_name: None,
            metrics_group: None,
        }
    }

    /// Parse configuration from golemwz.toml content with [env] section support
    pub fn from_toml_content(content: &str) -> Result<Self> {
        use toml::Value;
        
        let parsed: Value = toml::from_str(content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))?;
        
        let mut config = Self::default();
        
        // Parse main configuration fields
        if let Some(accepted_terms) = parsed.get("accepted_terms") {
            if let Some(terms_bool) = accepted_terms.as_bool() {
                config.accepted_terms = terms_bool;
            }
        }
        
        if let Some(glm_account) = parsed.get("glm_account") {
            if let Some(account_str) = glm_account.as_str() {
                config.glm_account = account_str.to_string();
            }
        }
        
        if let Some(glm_per_hour) = parsed.get("glm_per_hour") {
            if let Some(rate_str) = glm_per_hour.as_str() {
                config.glm_per_hour = rate_str.to_string();
            }
        }
        
        if let Some(glm_node_name) = parsed.get("glm_node_name") {
            if let Some(name_str) = glm_node_name.as_str() {
                config.glm_node_name = Some(name_str.to_string());
            }
        }
        
        if let Some(non_interactive) = parsed.get("non_interactive_install") {
            if let Some(interactive_bool) = non_interactive.as_bool() {
                config.non_interactive_install = interactive_bool;
            }
        }
        
        if let Some(ssh_keys) = parsed.get("ssh_keys") {
            if let Some(keys_array) = ssh_keys.as_array() {
                config.ssh_keys = keys_array
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
        
        if let Some(config_server) = parsed.get("configuration_server") {
            if let Some(server_str) = config_server.as_str() {
                if !server_str.trim().is_empty() {
                    config.configuration_server = Some(server_str.to_string());
                }
            }
        }
        
        // Parse [env] section for environment variables
        if let Some(env_section) = parsed.get("env") {
            if let Some(env_table) = env_section.as_table() {
                // Network configuration
                if let Some(net_type) = env_table.get("YA_NET_TYPE") {
                    if let Some(type_str) = net_type.as_str() {
                        config.network_type = match type_str.to_lowercase().as_str() {
                            "hybrid" => crate::models::NetworkType::Hybrid,
                            _ => crate::models::NetworkType::Central,
                        };
                    }
                }
                
                if let Some(subnet) = env_table.get("SUBNET") {
                    if let Some(subnet_str) = subnet.as_str() {
                        config.subnet = subnet_str.to_string();
                    }
                }
                
                if let Some(payment_net) = env_table.get("YA_PAYMENT_NETWORK_GROUP") {
                    if let Some(payment_str) = payment_net.as_str() {
                        config.payment_network = match payment_str.to_lowercase().as_str() {
                            "mainnet" => crate::models::PaymentNetwork::Mainnet,
                            _ => crate::models::PaymentNetwork::Testnet,
                        };
                    }
                }
                
                if let Some(central_host) = env_table.get("CENTRAL_NET_HOST") {
                    if let Some(host_str) = central_host.as_str() {
                        if !host_str.trim().is_empty() {
                            config.central_net_host = Some(host_str.to_string());
                        }
                    }
                }
                
                // Metrics configuration
                if let Some(metrics_url) = env_table.get("YAGNA_METRICS_URL") {
                    if let Some(url_str) = metrics_url.as_str() {
                        if !url_str.trim().is_empty() {
                            config.metrics_server = Some(url_str.to_string());
                        }
                    }
                }
                
                if let Some(job_name) = env_table.get("YAGNA_METRICS_JOB_NAME") {
                    if let Some(name_str) = job_name.as_str() {
                        if !name_str.trim().is_empty() {
                            config.metrics_job_name = Some(name_str.to_string());
                        }
                    }
                }
                
                if let Some(group) = env_table.get("YAGNA_METRICS_GROUP") {
                    if let Some(group_str) = group.as_str() {
                        if !group_str.trim().is_empty() {
                            config.metrics_group = Some(group_str.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(config)
    }
    
    /// Parse configuration from ENV content
    pub fn from_env_content(content: &str) -> Result<Self> {
        let mut config = Self::default();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                
                match key {
                    "YA_NET_TYPE" => {
                        config.network_type = match value.to_lowercase().as_str() {
                            "hybrid" => crate::models::NetworkType::Hybrid,
                            _ => crate::models::NetworkType::Central,
                        };
                    }
                    "SUBNET" => {
                        config.subnet = value.to_string();
                    }
                    "YA_PAYMENT_NETWORK_GROUP" => {
                        config.payment_network = match value.to_lowercase().as_str() {
                            "mainnet" => crate::models::PaymentNetwork::Mainnet,
                            _ => crate::models::PaymentNetwork::Testnet,
                        };
                    }
                    "CENTRAL_NET_HOST" => {
                        if !value.is_empty() {
                            config.central_net_host = Some(value.to_string());
                        }
                    }
                    "YAGNA_METRICS_URL" => {
                        if !value.is_empty() {
                            config.metrics_server = Some(value.to_string());
                        }
                    }
                    "YAGNA_METRICS_JOB_NAME" => {
                        if !value.is_empty() {
                            config.metrics_job_name = Some(value.to_string());
                        }
                    }
                    "YAGNA_METRICS_GROUP" => {
                        if !value.is_empty() {
                            config.metrics_group = Some(value.to_string());
                        }
                    }
                    _ => {
                        // Ignore unknown keys
                    }
                }
            }
        }
        
        Ok(config)
    }
    
    /// Parse configuration from both TOML and ENV content, with TOML as single source of truth
    pub fn from_config_files(toml_content: &str, env_content: &str) -> Result<Self> {
        // Start with TOML (single source of truth)
        let mut config = Self::from_toml_content(toml_content)?;
        
        // Only use ENV for fields not present in TOML
        let env_config = Self::from_env_content(env_content)?;
        
        // Fallback to ENV only for missing environment variables not in TOML [env] section
        if config.central_net_host.is_none() {
            config.central_net_host = env_config.central_net_host;
        }
        if config.metrics_server.is_none() {
            config.metrics_server = env_config.metrics_server;
        }
        if config.metrics_job_name.is_none() {
            config.metrics_job_name = env_config.metrics_job_name;
        }
        if config.metrics_group.is_none() {
            config.metrics_group = env_config.metrics_group;
        }
        
        Ok(config)
    }
    
    /// Generate golemwz.toml content with unified structure
    pub fn to_toml_content(&self) -> String {
        let mut content = String::new();
        
        // Main configuration section
        content.push_str("# Golem Configuration\n");
        content.push_str(&format!("accepted_terms = {}\n", self.accepted_terms));
        content.push_str(&format!("glm_account = \"{}\"\n", self.glm_account));
        content.push_str(&format!("glm_per_hour = \"{}\"\n", self.glm_per_hour));
        
        if let Some(ref node_name) = self.glm_node_name {
            content.push_str(&format!("glm_node_name = \"{}\"\n", node_name));
        }
        
        content.push_str(&format!("non_interactive_install = {}\n", self.non_interactive_install));
        
        // SSH keys array
        if !self.ssh_keys.is_empty() {
            let ssh_keys_str = self.ssh_keys
                .iter()
                .map(|key| format!("\"{}\"", key))
                .collect::<Vec<_>>()
                .join(", ");
            content.push_str(&format!("ssh_keys = [{}]\n", ssh_keys_str));
        } else {
            content.push_str("ssh_keys = []\n");
        }
        
        if let Some(ref config_server) = self.configuration_server {
            content.push_str(&format!("configuration_server = \"{}\"\n", config_server));
        }
        
        // Environment variables section
        content.push_str("\n# Environment Variables\n");
        content.push_str("[env]\n");
        
        let network_type_str = match self.network_type {
            crate::models::NetworkType::Hybrid => "hybrid",
            crate::models::NetworkType::Central => "central",
        };
        
        let payment_network_str = match self.payment_network {
            crate::models::PaymentNetwork::Testnet => "testnet",
            crate::models::PaymentNetwork::Mainnet => "mainnet",
        };
        
        content.push_str(&format!("YA_NET_TYPE = \"{}\"\n", network_type_str));
        content.push_str(&format!("SUBNET = \"{}\"\n", self.subnet));
        content.push_str(&format!("YA_PAYMENT_NETWORK_GROUP = \"{}\"\n", payment_network_str));
        
        if let Some(ref host) = self.central_net_host {
            content.push_str(&format!("CENTRAL_NET_HOST = \"{}\"\n", host));
        }
        
        if let Some(ref server) = self.metrics_server {
            content.push_str(&format!("YAGNA_METRICS_URL = \"{}\"\n", server));
        } else {
            content.push_str("YAGNA_METRICS_URL = \"https://metrics.golem.network:9092/\"\n");
        }
        
        if let Some(ref job_name) = self.metrics_job_name {
            content.push_str(&format!("YAGNA_METRICS_JOB_NAME = \"{}\"\n", job_name));
        } else {
            content.push_str("YAGNA_METRICS_JOB_NAME = \"community.1\"\n");
        }
        
        if let Some(ref group) = self.metrics_group {
            content.push_str(&format!("YAGNA_METRICS_GROUP = \"{}\"\n", group));
        } else {
            content.push_str("YAGNA_METRICS_GROUP = \"\"\n");
        }
        
        content
    }
    
    /// Generate golem.env content (extracted from [env] section)
    pub fn to_env_content(&self) -> String {
        let mut content = String::new();
        
        // Core network configuration
        let network_type_str = match self.network_type {
            crate::models::NetworkType::Hybrid => "hybrid",
            crate::models::NetworkType::Central => "central",
        };
        
        let payment_network_str = match self.payment_network {
            crate::models::PaymentNetwork::Testnet => "testnet",
            crate::models::PaymentNetwork::Mainnet => "mainnet",
        };
        
        content.push_str(&format!("YA_NET_TYPE={}\n", network_type_str));
        content.push_str(&format!("SUBNET={}\n", self.subnet));
        content.push_str(&format!("YA_PAYMENT_NETWORK_GROUP={}\n", payment_network_str));
        
        // Optional network configuration
        if let Some(ref host) = self.central_net_host {
            content.push_str(&format!("CENTRAL_NET_HOST={}\n", host));
        }
        
        // Metrics configuration
        if let Some(ref server) = self.metrics_server {
            content.push_str(&format!("YAGNA_METRICS_URL={}\n", server));
        } else {
            content.push_str("YAGNA_METRICS_URL=https://metrics.golem.network:9092/\n");
        }
        
        if let Some(ref job_name) = self.metrics_job_name {
            content.push_str(&format!("YAGNA_METRICS_JOB_NAME={}\n", job_name));
        } else {
            content.push_str("YAGNA_METRICS_JOB_NAME=community.1\n");
        }
        
        if let Some(ref group) = self.metrics_group {
            content.push_str(&format!("YAGNA_METRICS_GROUP={}\n", group));
        } else {
            content.push_str("YAGNA_METRICS_GROUP=\n");
        }
        
        content
    }
    
    /// Generate both configuration files as a tuple (toml_content, env_content)
    pub fn generate_config_files(&self) -> (String, String) {
        (self.to_toml_content(), self.to_env_content())
    }
}

impl Default for ImageConfiguration {
    fn default() -> Self {
        Self {
            accepted_terms: true,
            glm_account: String::new(),
            glm_per_hour: "0.25".to_string(),
            glm_node_name: None,
            non_interactive_install: false,
            ssh_keys: Vec::new(),
            configuration_server: None,
            payment_network: crate::models::PaymentNetwork::Testnet,
            network_type: crate::models::NetworkType::Central,
            subnet: "public".to_string(),
            central_net_host: None,
            metrics_server: None,
            metrics_job_name: None,
            metrics_group: None,
        }
    }
}

impl From<ImageConfiguration> for crate::disk::GolemConfig {
    fn from(config: ImageConfiguration) -> Self {
        Self {
            payment_network: config.payment_network,
            network_type: config.network_type,
            subnet: config.subnet,
            wallet_address: config.glm_account,
            glm_per_hour: config.glm_per_hour,
            non_interactive_install: config.non_interactive_install,
            ssh_keys: config.ssh_keys,
            configuration_server: config.configuration_server,
            metrics_server: config.metrics_server,
            central_net_host: config.central_net_host,
        }
    }
}

impl From<super::GolemConfig> for ImageConfiguration {
    fn from(config: super::GolemConfig) -> Self {
        Self {
            accepted_terms: true,
            glm_account: config.wallet_address,
            glm_per_hour: config.glm_per_hour,
            glm_node_name: None,
            non_interactive_install: config.non_interactive_install,
            ssh_keys: config.ssh_keys,
            configuration_server: config.configuration_server,
            payment_network: config.payment_network,
            network_type: config.network_type,
            subnet: config.subnet,
            central_net_host: config.central_net_host,
            metrics_server: config.metrics_server,
            metrics_job_name: None,
            metrics_group: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NetworkType, PaymentNetwork};

    #[test]
    fn test_image_configuration_from_toml_content() {
        let toml_content = r#"
accepted_terms = true
glm_account = "0x1234567890abcdef1234567890abcdef12345678"
glm_per_hour = "0.5"
glm_node_name = "test-node"
non_interactive_install = true
ssh_keys = ["ssh-rsa AAAAB3..."]
configuration_server = "https://config.example.com"

[env]
YA_NET_TYPE = "hybrid"
SUBNET = "devnet-beta"
YA_PAYMENT_NETWORK_GROUP = "mainnet"
CENTRAL_NET_HOST = "central.example.com"
YAGNA_METRICS_URL = "https://metrics.example.com"
YAGNA_METRICS_JOB_NAME = "community.1"
YAGNA_METRICS_GROUP = "gpu-provider"
"#;

        let config = ImageConfiguration::from_toml_content(toml_content).unwrap();
        
        // Main TOML fields
        assert_eq!(config.accepted_terms, true);
        assert_eq!(config.glm_account, "0x1234567890abcdef1234567890abcdef12345678");
        assert_eq!(config.glm_per_hour, "0.5");
        assert_eq!(config.glm_node_name, Some("test-node".to_string()));
        assert_eq!(config.non_interactive_install, true);
        assert_eq!(config.ssh_keys, vec!["ssh-rsa AAAAB3..."]);
        assert_eq!(config.configuration_server, Some("https://config.example.com".to_string()));
        
        // Environment variables from [env] section
        assert_eq!(config.payment_network, PaymentNetwork::Mainnet);
        assert_eq!(config.network_type, NetworkType::Hybrid);
        assert_eq!(config.subnet, "devnet-beta");
        assert_eq!(config.central_net_host, Some("central.example.com".to_string()));
        assert_eq!(config.metrics_server, Some("https://metrics.example.com".to_string()));
        assert_eq!(config.metrics_job_name, Some("community.1".to_string()));
        assert_eq!(config.metrics_group, Some("gpu-provider".to_string()));
    }

    #[test]
    fn test_image_configuration_from_env_content() {
        let env_content = r#"
YA_NET_TYPE=hybrid
SUBNET=devnet-beta
YA_PAYMENT_NETWORK_GROUP=mainnet
YAGNA_METRICS_URL=https://metrics.example.com
CENTRAL_NET_HOST=central.example.com
YAGNA_METRICS_JOB_NAME=community.1
YAGNA_METRICS_GROUP=gpu-provider
"#;

        let config = ImageConfiguration::from_env_content(env_content).unwrap();
        assert_eq!(config.network_type, NetworkType::Hybrid);
        assert_eq!(config.subnet, "devnet-beta");
        assert_eq!(config.payment_network, PaymentNetwork::Mainnet);
        assert_eq!(config.metrics_server, Some("https://metrics.example.com".to_string()));
        assert_eq!(config.central_net_host, Some("central.example.com".to_string()));
        assert_eq!(config.metrics_job_name, Some("community.1".to_string()));
        assert_eq!(config.metrics_group, Some("gpu-provider".to_string()));
    }

    #[test]
    fn test_image_configuration_from_config_files() {
        let toml_content = r#"
accepted_terms = true
glm_account = "0x1234567890abcdef1234567890abcdef12345678"
glm_per_hour = "0.75"
glm_node_name = "test-node"

[env]
YA_NET_TYPE = "hybrid"
SUBNET = "mainnet"
YA_PAYMENT_NETWORK_GROUP = "mainnet"
"#;

        let env_content = r#"
YA_NET_TYPE=central
SUBNET=testnet
YA_PAYMENT_NETWORK_GROUP=testnet
"#;

        let config = ImageConfiguration::from_config_files(toml_content, env_content).unwrap();
        
        // TOML values should be used (single source of truth)
        assert_eq!(config.accepted_terms, true);
        assert_eq!(config.glm_account, "0x1234567890abcdef1234567890abcdef12345678");
        assert_eq!(config.glm_per_hour, "0.75");
        assert_eq!(config.glm_node_name, Some("test-node".to_string()));
        
        // TOML [env] section should take precedence over standalone env file
        assert_eq!(config.network_type, NetworkType::Hybrid);
        assert_eq!(config.subnet, "mainnet");
        assert_eq!(config.payment_network, PaymentNetwork::Mainnet);
    }

    #[test]
    fn test_image_configuration_to_toml_content() {
        let config = ImageConfiguration {
            accepted_terms: true,
            glm_account: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            glm_per_hour: "0.5".to_string(),
            glm_node_name: Some("test-node".to_string()),
            payment_network: PaymentNetwork::Mainnet,
            network_type: NetworkType::Hybrid,
            subnet: "devnet-beta".to_string(),
            ..Default::default()
        };

        let toml_content = config.to_toml_content();
        
        // Check main TOML fields
        assert!(toml_content.contains("accepted_terms = true"));
        assert!(toml_content.contains("glm_account = \"0x1234567890abcdef1234567890abcdef12345678\""));
        assert!(toml_content.contains("glm_per_hour = \"0.5\""));
        assert!(toml_content.contains("glm_node_name = \"test-node\""));
        
        // Check [env] section
        assert!(toml_content.contains("[env]"));
        assert!(toml_content.contains("YA_PAYMENT_NETWORK_GROUP = \"mainnet\""));
        assert!(toml_content.contains("YA_NET_TYPE = \"hybrid\""));
        assert!(toml_content.contains("SUBNET = \"devnet-beta\""));
        
        // Check comment
        assert!(toml_content.contains("# Golem Configuration"));
    }

    #[test]
    fn test_image_configuration_to_env_content() {
        let config = ImageConfiguration {
            payment_network: PaymentNetwork::Mainnet,
            network_type: NetworkType::Hybrid,
            subnet: "devnet-beta".to_string(),
            non_interactive_install: true,
            ssh_keys: vec!["ssh-rsa AAAAB3...".to_string(), "ssh-ed25519 AAAAC3...".to_string()],
            configuration_server: Some("https://config.example.com".to_string()),
            metrics_server: Some("https://metrics.example.com".to_string()),
            central_net_host: Some("central.example.com".to_string()),
            metrics_job_name: Some("community.1".to_string()),
            metrics_group: Some("gpu-provider".to_string()),
            ..Default::default()
        };

        let env_content = config.to_env_content();
        assert!(env_content.contains("YA_NET_TYPE=hybrid"));
        assert!(env_content.contains("SUBNET=devnet-beta"));
        assert!(env_content.contains("YA_PAYMENT_NETWORK_GROUP=mainnet"));
        assert!(env_content.contains("YAGNA_METRICS_URL=https://metrics.example.com"));
        assert!(env_content.contains("CENTRAL_NET_HOST=central.example.com"));
        assert!(env_content.contains("YAGNA_METRICS_JOB_NAME=community.1"));
        assert!(env_content.contains("YAGNA_METRICS_GROUP=gpu-provider"));
    }

    #[test]
    fn test_image_configuration_generate_config_files() {
        let config = ImageConfiguration {
            accepted_terms: true,
            payment_network: PaymentNetwork::Mainnet,
            network_type: NetworkType::Hybrid,
            subnet: "mainnet".to_string(),
            glm_account: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            glm_per_hour: "1.0".to_string(),
            glm_node_name: Some("test-node".to_string()),
            non_interactive_install: true,
            ssh_keys: vec!["ssh-rsa AAAAB3...".to_string()],
            configuration_server: Some("https://config.example.com".to_string()),
            metrics_server: Some("https://metrics.example.com".to_string()),
            central_net_host: Some("central.example.com".to_string()),
            metrics_job_name: Some("community.1".to_string()),
            metrics_group: Some("gpu-provider".to_string()),
        };

        let (toml_content, env_content) = config.generate_config_files();
        
        // Check TOML content - should contain main fields and [env] section
        assert!(toml_content.contains("accepted_terms = true"));
        assert!(toml_content.contains("glm_account = \"0x1234567890abcdef1234567890abcdef12345678\""));
        assert!(toml_content.contains("glm_per_hour = \"1.0\""));
        assert!(toml_content.contains("glm_node_name = \"test-node\""));
        assert!(toml_content.contains("[env]"));
        assert!(toml_content.contains("YA_PAYMENT_NETWORK_GROUP = \"mainnet\""));
        assert!(toml_content.contains("YA_NET_TYPE = \"hybrid\""));
        
        // Check ENV content - extracted from [env] section
        assert!(env_content.contains("YA_NET_TYPE=hybrid"));
        assert!(env_content.contains("SUBNET=mainnet"));
        assert!(env_content.contains("YA_PAYMENT_NETWORK_GROUP=mainnet"));
        assert!(env_content.contains("YAGNA_METRICS_URL=https://metrics.example.com"));
        assert!(env_content.contains("CENTRAL_NET_HOST=central.example.com"));
        assert!(env_content.contains("YAGNA_METRICS_JOB_NAME=community.1"));
        assert!(env_content.contains("YAGNA_METRICS_GROUP=gpu-provider"));
    }

    #[test]
    fn test_image_configuration_default() {
        let config = ImageConfiguration::default();
        
        // Main TOML fields
        assert_eq!(config.accepted_terms, true);
        assert_eq!(config.glm_account, "");
        assert_eq!(config.glm_per_hour, "0.25");
        assert_eq!(config.glm_node_name, None);
        assert_eq!(config.non_interactive_install, false);
        assert!(config.ssh_keys.is_empty());
        assert_eq!(config.configuration_server, None);
        
        // Environment variables
        assert_eq!(config.payment_network, PaymentNetwork::Testnet);
        assert_eq!(config.network_type, NetworkType::Central);
        assert_eq!(config.subnet, "public");
        assert_eq!(config.central_net_host, None);
        assert_eq!(config.metrics_server, None);
        assert_eq!(config.metrics_job_name, None);
        assert_eq!(config.metrics_group, None);
    }

    #[test]
    fn test_ensure_accepted_terms() {
        // Test with a configuration that starts with accepted_terms = false
        let mut config = ImageConfiguration {
            accepted_terms: false, // Start with false
            glm_account: "test_account".to_string(),
            ..Default::default()
        };
        
        // Ensure accepted_terms is false initially
        assert_eq!(config.accepted_terms, false);
        
        // Call ensure_accepted_terms
        config.ensure_accepted_terms();
        
        // Verify it's now true
        assert_eq!(config.accepted_terms, true);
        
        // Test with a configuration that already has accepted_terms = true
        let mut config2 = ImageConfiguration {
            accepted_terms: true,
            glm_account: "test_account2".to_string(),
            ..Default::default()  
        };
        
        // Call ensure_accepted_terms (should remain true)
        config2.ensure_accepted_terms();
        
        // Verify it's still true
        assert_eq!(config2.accepted_terms, true);
    }

    #[test]
    fn test_conversion_between_image_config_and_golem_config() {
        let image_config = ImageConfiguration {
            accepted_terms: true,
            payment_network: PaymentNetwork::Mainnet,
            network_type: NetworkType::Hybrid,
            subnet: "test-subnet".to_string(),
            glm_account: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            glm_per_hour: "0.5".to_string(),
            glm_node_name: Some("test-node".to_string()),
            non_interactive_install: true,
            ssh_keys: vec!["ssh-rsa AAAAB3...".to_string()],
            configuration_server: Some("https://config.example.com".to_string()),
            metrics_server: Some("https://metrics.example.com".to_string()),
            central_net_host: Some("central.example.com".to_string()),
            metrics_job_name: Some("community.1".to_string()),
            metrics_group: Some("gpu-provider".to_string()),
        };

        // Convert to GolemConfig
        let golem_config: crate::disk::GolemConfig = image_config.clone().into();
        assert_eq!(golem_config.payment_network, image_config.payment_network);
        assert_eq!(golem_config.network_type, image_config.network_type);
        assert_eq!(golem_config.subnet, image_config.subnet);
        assert_eq!(golem_config.wallet_address, image_config.glm_account);
        assert_eq!(golem_config.glm_per_hour, image_config.glm_per_hour);
        assert_eq!(golem_config.non_interactive_install, image_config.non_interactive_install);
        assert_eq!(golem_config.ssh_keys, image_config.ssh_keys);
        assert_eq!(golem_config.configuration_server, image_config.configuration_server);
        assert_eq!(golem_config.metrics_server, image_config.metrics_server);
        assert_eq!(golem_config.central_net_host, image_config.central_net_host);

        // Convert back to ImageConfiguration
        let converted_back: ImageConfiguration = golem_config.into();
        assert_eq!(converted_back.payment_network, image_config.payment_network);
        assert_eq!(converted_back.network_type, image_config.network_type);
        assert_eq!(converted_back.subnet, image_config.subnet);
        assert_eq!(converted_back.glm_account, image_config.glm_account);
        assert_eq!(converted_back.glm_per_hour, image_config.glm_per_hour);
        assert_eq!(converted_back.non_interactive_install, image_config.non_interactive_install);
        assert_eq!(converted_back.ssh_keys, image_config.ssh_keys);
        assert_eq!(converted_back.configuration_server, image_config.configuration_server);
        assert_eq!(converted_back.metrics_server, image_config.metrics_server);
        assert_eq!(converted_back.central_net_host, image_config.central_net_host);
        // Note: metrics_job_name and metrics_group are not in GolemConfig, so they reset to None
        assert_eq!(converted_back.metrics_job_name, None);
        assert_eq!(converted_back.metrics_group, None);
    }

    #[test]
    fn test_invalid_toml_parsing() {
        let invalid_toml = "invalid toml content [[[";
        let result = ImageConfiguration::from_toml_content(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_content_parsing() {
        // Empty TOML should use defaults
        let config = ImageConfiguration::from_toml_content("").unwrap();
        assert_eq!(config.glm_account, "");
        assert_eq!(config.glm_per_hour, "0.25");
        assert_eq!(config.accepted_terms, true);

        // Empty ENV should use defaults
        let config = ImageConfiguration::from_env_content("").unwrap();
        assert_eq!(config.network_type, NetworkType::Central);
        assert_eq!(config.subnet, "public");
        assert_eq!(config.payment_network, PaymentNetwork::Testnet);
    }

    #[test]
    fn test_env_parsing_with_comments() {
        let env_content = r#"
# This is a comment
YA_NET_TYPE=hybrid
# Another comment
SUBNET=test-subnet
YA_PAYMENT_NETWORK_GROUP=mainnet
"#;

        let config = ImageConfiguration::from_env_content(env_content).unwrap();
        assert_eq!(config.network_type, NetworkType::Hybrid);
        assert_eq!(config.subnet, "test-subnet");
        assert_eq!(config.payment_network, PaymentNetwork::Mainnet);
    }
}