/// Validation utilities for configuration fields

/// Validates if a string is a valid SSH public key in OpenSSH format
pub fn is_valid_ssh_public_key(key: &str) -> bool {
    let trimmed = key.trim();

    // Empty keys are considered valid (optional field)
    if trimmed.is_empty() {
        return true;
    }

    // SSH keys typically start with algorithm name
    let valid_algorithms = [
        "ssh-rsa",
        "ssh-dss",
        "ssh-ed25519",
        "ecdsa-sha2-nistp256",
        "ecdsa-sha2-nistp384",
        "ecdsa-sha2-nistp521",
    ];

    // Split by whitespace and check structure
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    // SSH key should have at least 2 parts: algorithm and key data
    if parts.len() < 2 {
        return false;
    }

    // Check if it starts with a valid algorithm
    if !valid_algorithms.contains(&parts[0]) {
        return false;
    }

    // The key data should be base64 encoded and reasonably long
    let key_data = parts[1];
    if key_data.len() < 50 {
        return false;
    }

    // Basic base64 character check
    key_data
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}

/// Validates multiple SSH public keys (one per line or comma separated)
pub fn validate_ssh_keys(keys_input: &str) -> Vec<String> {
    let mut errors = Vec::new();

    if keys_input.trim().is_empty() {
        return errors; // Empty input is valid
    }

    // Try to parse as newline-separated first, then comma-separated
    let keys: Vec<&str> = if keys_input.contains('\n') {
        keys_input.lines().collect()
    } else {
        keys_input.split(',').collect()
    };

    for (i, key) in keys.iter().enumerate() {
        let trimmed_key = key.trim();
        if !trimmed_key.is_empty() && !is_valid_ssh_public_key(trimmed_key) {
            errors.push(format!("SSH key {} is invalid", i + 1));
        }
    }

    errors
}

/// Validates if a string is a valid URL
pub fn is_valid_url(url: &str) -> bool {
    let trimmed = url.trim();

    // Empty URLs are considered valid (optional field)
    if trimmed.is_empty() {
        return true;
    }

    // Basic URL validation - should start with http:// or https://
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return false;
    }

    // Should have some content after the protocol
    if trimmed.len() < 10 {
        return false;
    }

    // Basic character validation - no spaces, should contain domain-like structure
    if trimmed.contains(' ') {
        return false;
    }

    // Should contain at least one dot after the protocol (for domain)
    let after_protocol = if trimmed.starts_with("https://") {
        &trimmed[8..]
    } else {
        &trimmed[7..]
    };

    after_protocol.contains('.')
}

/// Validates if a string is a valid central net host URL in the format:
/// <host>:<port> or <hex-key>@<host>:<port>
/// Uses the same regex pattern as the server: ^(([0-9a-z]{56})@)?([^:]*)(:[0-9]{1,4})?$
pub fn is_valid_central_net_host(url: &str) -> bool {
    let trimmed = url.trim();
    
    // Empty URLs are considered valid (optional field)
    if trimmed.is_empty() {
        return true;
    }
    
    // Use the same regex pattern as the server
    let re = regex::Regex::new(r"^(([0-9a-z]{56})@)?([^:]*)(:[0-9]{1,4})?$").unwrap();
    
    if let Some(captures) = re.captures(trimmed) {
        // Extract host - must not be empty
        let host = captures.get(3).map(|m| m.as_str()).unwrap_or("");
        if host.is_empty() {
            return false;
        }
        
        // If port is specified, validate it
        if let Some(port_match) = captures.get(4) {
            let port_str = &port_match.as_str()[1..]; // Remove the ':' prefix
            if let Ok(port_num) = port_str.parse::<u16>() {
                port_num > 0
            } else {
                false
            }
        } else {
            // No port specified, which is valid (server defaults to 7464)
            true
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ssh_keys() {
        assert!(is_valid_ssh_public_key(""));
        assert!(is_valid_ssh_public_key(
            "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC7vbqajDhA3+oF8tP1oFqZ"
        ));
        assert!(is_valid_ssh_public_key(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG4rT3vTt99Ox5kndS4HmgTrKBT8SKzhK4rhGkEVGlCI user@example.com"
        ));
    }

    #[test]
    fn test_invalid_ssh_keys() {
        assert!(!is_valid_ssh_public_key("invalid-key"));
        assert!(!is_valid_ssh_public_key("ssh-rsa"));
        assert!(!is_valid_ssh_public_key("ssh-rsa short"));
        assert!(!is_valid_ssh_public_key(
            "invalid-algo AAAAB3NzaC1yc2EAAAADAQABAAABgQC7vbqajDhA3+oF8tP1oFqZ"
        ));
    }

    #[test]
    fn test_valid_urls() {
        assert!(is_valid_url(""));
        assert!(is_valid_url("https://example.com"));
        assert!(is_valid_url("http://test.example.org/path"));
        assert!(is_valid_url("https://api.example.com:8080/v1"));
    }

    #[test]
    fn test_invalid_urls() {
        assert!(!is_valid_url("not-a-url"));
        assert!(!is_valid_url("ftp://example.com"));
        assert!(!is_valid_url("https://"));
        assert!(!is_valid_url("http://no spaces allowed.com"));
        assert!(!is_valid_url("https://nodot"));
    }

    #[test]
    fn test_valid_central_net_host() {
        // Empty URL should be valid
        assert!(is_valid_central_net_host(""));
        
        // Valid host:port formats
        assert!(is_valid_central_net_host("a.com:5000"));
        assert!(is_valid_central_net_host("10.0.0.1:5000"));
        assert!(is_valid_central_net_host("192.168.1.1:8080"));
        assert!(is_valid_central_net_host("localhost:3000"));
        assert!(is_valid_central_net_host("example.com:443"));
        assert!(is_valid_central_net_host("18.185.178.4:7464"));
        
        // Valid host without port (defaults to 7464)
        assert!(is_valid_central_net_host("a.com"));
        assert!(is_valid_central_net_host("10.0.0.1"));
        assert!(is_valid_central_net_host("localhost"));
        
        // Valid 56-character hex key formats
        assert!(is_valid_central_net_host("393479950594e7c676ba121033a677a1316f722460827e217c82d2b3@18.185.178.4:7464"));
        assert!(is_valid_central_net_host("abcdef1234567890abcdef1234567890abcdef1234567890abcdef@da.com:5000"));
        assert!(is_valid_central_net_host("393479950594e7c676ba121033a677a1316f722460827e217c82d2b3@18.185.178.4"));
        
        // Valid port ranges (1-9999 based on regex)
        assert!(is_valid_central_net_host("host:1"));
        assert!(is_valid_central_net_host("host:9999"));
    }

    #[test]
    fn test_invalid_central_net_host() {
        // Invalid formats - empty host
        assert!(!is_valid_central_net_host(":5000"));
        assert!(!is_valid_central_net_host("@:5000"));
        
        // Invalid port formats
        assert!(!is_valid_central_net_host("host:"));
        assert!(!is_valid_central_net_host("host:port"));
        assert!(!is_valid_central_net_host("host:0"));
        assert!(!is_valid_central_net_host("host:10000")); // Port too high for 4-digit regex
        
        // Invalid hex key formats - wrong length (should be 56, not 64)
        assert!(!is_valid_central_net_host("shortkey@host:5000"));
        assert!(!is_valid_central_net_host("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890@host:5000")); // 64 chars
        
        // Invalid hex key characters (uppercase not allowed)
        assert!(!is_valid_central_net_host("393479950594E7C676BA121033A677A1316F722460827E217C82D2B3@host:5000"));
        
        // Invalid hex key characters (non-hex)
        assert!(!is_valid_central_net_host("gggg79950594e7c676ba121033a677a1316f722460827e217c82d2b3@host:5000"));
        
        // Multiple @ symbols not allowed by regex
        assert!(!is_valid_central_net_host("key@host@more:5000"));
        
        // Invalid characters in host based on regex
        assert!(!is_valid_central_net_host("393479950594e7c676ba121033a677a1316f722460827e217c82d2b3@:5000"));
    }
}
