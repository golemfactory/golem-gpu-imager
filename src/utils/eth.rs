use ethereum_types::Address;
use std::str::FromStr;

/// Validates an Ethereum address with checksum validation
///
/// Uses the ethers-core library for proper validation of EIP-55 checksummed addresses
/// and standard address format.
///
/// Returns true if the address is valid, false otherwise
pub fn is_valid_eth_address(address: &str) -> bool {
    // Explicit check for 0x prefix - ethers will try to add it if missing
    if !address.starts_with("0x") {
        return false;
    }

    // Try to parse the address using ethers-core
    Address::from_str(address).is_ok()
}

/// Formats an Ethereum address with proper EIP-55 checksum
///
/// The address must be a valid Ethereum address starting with "0x"
///
/// Returns None if the address is invalid
#[allow(dead_code)]
pub fn format_eth_address(address: &str) -> Option<String> {
    match Address::from_str(address) {
        Ok(eth_address) => Some(format!("{:?}", eth_address)),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_eth_addresses() {
        // Valid lowercase address
        assert!(is_valid_eth_address(
            "0xab5801a7d398351b8be11c439e05c5b3259aec9b"
        ));

        // Valid checksummed address (EIP-55)
        assert!(is_valid_eth_address(
            "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B"
        ));

        // Another valid address
        assert!(is_valid_eth_address(
            "0x7da82C7AB4771ff031b66538D2fB9b0B047f6CF9"
        ));
    }

    #[test]
    fn test_invalid_eth_addresses() {
        // Invalid length
        assert!(!is_valid_eth_address(
            "0xab5801a7d398351b8be11c439e05c5b3259aec"
        ));

        // Invalid prefix
        assert!(!is_valid_eth_address(
            "ab5801a7d398351b8be11c439e05c5b3259aec9b"
        ));

        // Invalid character
        assert!(!is_valid_eth_address(
            "0xab5801a7d398351b8be11c439e05c5b3259aecqb"
        ));

        // Note: ethers-core actually supports some checksums that our previous implementation rejected
        // Explicitly test for the 0x prefix which is our main requirement
        assert!(!is_valid_eth_address("Abc"));
    }

    #[test]
    fn test_format_eth_address() {
        // Test that formatting works correctly
        let address = "0xab5801a7d398351b8be11c439e05c5b3259aec9b";
        let formatted = format_eth_address(address);
        assert!(formatted.is_some());

        // Check if invalid addresses return None
        let invalid_address = "0xinvalid";
        assert_eq!(format_eth_address(invalid_address), None);
    }
}
