# Windows Administrator Privileges

Golem GPU Imager requires administrator privileges on Windows to perform disk operations and hardware access.

## Why Administrator Privileges Are Required

1. **Direct Disk Access**: Writing to storage devices requires low-level system access
2. **Volume Locking**: Preventing other processes from accessing the disk during write operations
3. **GPT Manipulation**: Modifying partition tables requires system-level permissions
4. **Hardware Enumeration**: Detecting and accessing storage devices

## Implementation

### 1. Programmatic Elevation (Runtime Detection & Request)
The application includes runtime elevation detection and request functionality:

```rust
// Check if running with admin privileges
if utils::is_elevated() {
    // Proceed with disk operations
} else {
    // Show warning or request elevation
    utils::request_elevation()?; // Restarts app with "Run as administrator"
}
```

**Features:**
- **Detection**: Checks if the current process has administrator privileges
- **User Check**: Verifies if the current user is a member of the Administrators group  
- **Runtime Request**: Can request elevation by restarting the application with UAC prompt
- **Status Display**: Shows current elevation status in logs and UI
- **Cross-platform**: Graceful handling on non-Windows systems

**When to use:**
- Applications that sometimes need admin privileges but can run without them
- Better user experience - doesn't always show UAC prompt
- Allows graceful degradation of functionality

### 2. Application Manifest
The application includes a Windows manifest that requests administrator privileges:

```xml
<requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
```

This is embedded at build time via `build.rs` and ensures the application will:
- Show a UAC prompt when launched
- Run with elevated privileges automatically
- Have access to system-level disk operations

### 3. Application Startup
The application automatically checks elevation status at startup:

```rust
// Check elevation status on Windows
let elevation_status = utils::get_elevation_status();
tracing::info!("Privilege status: {}", elevation_status);

if !utils::is_elevated() {
    tracing::warn!("Application is not running with administrator privileges. Some operations may fail.");
}
```

**Status messages:**
- ✅ "Running with administrator privileges" 
- ⚠️ "User is admin but process is not elevated. Use 'Run as administrator' to elevate."
- ❌ "User is not an administrator. Please log in as an administrator."

### 4. WiX Installer Configuration
The MSI installer is configured to:
- **Install for all users (requires admin)**: `Scope="perMachine"`
- **Create admin-aware shortcuts**: Both Start Menu and Desktop shortcuts include `System.AppUserModel.RunAs=true`
- **WiX v4 compatible**: Uses modern WiX v4 syntax for elevation requirements

### 5. Shortcut Behavior
All shortcuts created by the installer will:
- Display "Run as Administrator" in descriptions
- Automatically request elevation when clicked
- Show the UAC shield icon on Windows

## User Experience

### First Launch
1. User double-clicks the application or shortcut
2. Windows UAC prompt appears: "Do you want to allow this app to make changes to your device?"
3. User clicks "Yes" to grant administrator privileges
4. Application launches with full system access

### Subsequent Launches
- UAC prompt will appear each time the application is launched
- This is by design for security reasons
- Users can create a scheduled task to bypass UAC if desired (advanced users)

## Development Notes

### Testing Without Admin Rights
For development/testing purposes, you can temporarily modify the manifest to use:
```xml
<requestedExecutionLevel level="asInvoker" uiAccess="false" />
```

However, disk operations will likely fail without elevation.

### Alternative Approaches
If admin requirements become problematic, consider:
1. **Service-based architecture**: Run a Windows service with admin rights
2. **Separate elevation tool**: Launch a separate elevated process for disk operations
3. **PowerShell integration**: Use elevated PowerShell scripts for disk operations

## Troubleshooting

### UAC Prompt Not Appearing
- Check that the manifest is properly embedded (run `cargo build`)
- Verify Windows UAC is enabled in Control Panel
- Try running from command prompt as administrator

### "Access Denied" Errors
- Ensure UAC prompt was accepted
- Check that antivirus isn't blocking the application
- Verify the application is running as administrator (Task Manager shows "Administrator" in process name)

### Performance Considerations
- Admin elevation adds ~1-2 seconds to startup time
- No runtime performance impact once elevated
- Consider caching elevation for better UX in future versions