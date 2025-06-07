/// Windows-specific privilege elevation utilities
/// 
/// This module provides functionality to detect if the current process
/// is running with administrator privileges and request elevation if needed.

#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{BOOL, FALSE, HANDLE, HWND},
    Security::{
        CheckTokenMembership, CreateWellKnownSid, GetTokenInformation, 
        TokenElevation, WinBuiltinAdministratorsSid, TOKEN_ELEVATION, TOKEN_QUERY
    },
    System::{
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::{
        Shell::{ShellExecuteW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

/// Check if the current process is running with administrator privileges
#[cfg(windows)]
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = 0;
        
        // Get the access token for the current process
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == FALSE {
            return false;
        }
        
        // Check if the token is elevated
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length = 0u32;
        
        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );
        
        // Close the token handle
        windows_sys::Win32::Foundation::CloseHandle(token);
        
        result != FALSE && elevation.TokenIsElevated != 0
    }
}

/// Check if the current user is a member of the Administrators group
#[cfg(windows)]
pub fn is_admin_user() -> bool {
    unsafe {
        // Create the Administrators group SID
        let mut admin_sid = ptr::null_mut();
        let mut sid_size = 0u32;
        
        // Get the size needed for the SID
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            ptr::null_mut(),
            admin_sid,
            &mut sid_size,
        );
        
        // Allocate memory for the SID
        let mut sid_buffer = vec![0u8; sid_size as usize];
        let admin_sid = sid_buffer.as_mut_ptr() as *mut _;
        
        // Create the actual SID
        if CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            ptr::null_mut(),
            admin_sid,
            &mut sid_size,
        ) == FALSE {
            return false;
        }
        
        // Check if the current user is a member of the Administrators group
        let mut is_member: BOOL = FALSE;
        CheckTokenMembership(0, admin_sid, &mut is_member) != FALSE && is_member != FALSE
    }
}

/// Request elevation by restarting the application with "Run as administrator"
#[cfg(windows)]
pub fn request_elevation() -> Result<(), String> {
    // Get the current executable path
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;
    
    let exe_path_str = exe_path.to_string_lossy();
    
    // Convert to wide string for Windows API
    let mut exe_path_wide: Vec<u16> = exe_path_str.encode_utf16().collect();
    exe_path_wide.push(0); // Null terminate
    
    let verb = "runas\0".encode_utf16().collect::<Vec<u16>>();
    
    unsafe {
        let result = ShellExecuteW(
            0 as HWND,
            verb.as_ptr(),
            exe_path_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
        
        // ShellExecuteW returns a value > 32 on success
        if result as i32 <= 32 {
            return Err(format!("Failed to request elevation. Error code: {}", result as i32));
        }
    }
    
    // If elevation request was successful, exit the current process
    std::process::exit(0);
}

/// Check elevation status and optionally request elevation if needed
#[cfg(windows)]
pub fn ensure_elevation(force_request: bool) -> Result<bool, String> {
    if is_elevated() {
        return Ok(true);
    }
    
    if !is_admin_user() {
        return Err("Current user is not a member of the Administrators group. Please run as an administrator.".to_string());
    }
    
    if force_request {
        request_elevation()?;
        // This line should never be reached if elevation succeeds
        return Err("Elevation request failed".to_string());
    }
    
    Ok(false)
}

/// Display elevation status information
pub fn get_elevation_status() -> String {
    #[cfg(windows)]
    {
        let elevated = is_elevated();
        let admin_user = is_admin_user();
        
        match (elevated, admin_user) {
            (true, true) => "Running with administrator privileges".to_string(),
            (false, true) => "User is admin but process is not elevated. Use 'Run as administrator' to elevate.".to_string(),
            (false, false) => "User is not an administrator. Please log in as an administrator.".to_string(),
            (true, false) => "Process is elevated but user check failed (unusual)".to_string(),
        }
    }
    
    #[cfg(not(windows))]
    {
        // On non-Windows systems, check if running as root
        if unsafe { libc::geteuid() } == 0 {
            "Running as root".to_string()
        } else {
            "Not running as root. Some operations may require sudo.".to_string()
        }
    }
}

/// Non-Windows stub implementations
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    // On Unix-like systems, check if running as root
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(windows))]
pub fn is_admin_user() -> bool {
    // On Unix-like systems, always return true as sudo can provide elevation
    true
}

#[cfg(not(windows))]
pub fn request_elevation() -> Result<(), String> {
    Err("Elevation request not supported on this platform. Use sudo to run as root.".to_string())
}

#[cfg(not(windows))]
pub fn ensure_elevation(_force_request: bool) -> Result<bool, String> {
    Ok(is_elevated())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_elevation_status() {
        let status = get_elevation_status();
        println!("Elevation status: {}", status);
        // Just verify that we can get a status string without panicking
        assert!(!status.is_empty());
    }
    
    #[test]
    #[cfg(windows)]
    fn test_admin_checks() {
        // These tests will vary based on how the tests are run
        let elevated = is_elevated();
        let admin_user = is_admin_user();
        
        println!("Is elevated: {}", elevated);
        println!("Is admin user: {}", admin_user);
        
        // If we're elevated, we should also be an admin user
        if elevated {
            assert!(admin_user, "If process is elevated, user should be admin");
        }
    }
}