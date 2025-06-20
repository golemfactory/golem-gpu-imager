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
}
