# Windows Installer

This directory contains the Windows MSI installer configuration for Golem GPU Imager.

## Prerequisites

- WiX Toolset v3.11+ (for MSI generation)
- PowerShell (for build scripts)

## Files

- `installer.wxs` - WiX configuration file defining the MSI package
- `build-msi.bat` - Windows batch script to build the MSI
- `build-msi.ps1` - PowerShell script to build the MSI  
- `build-msi.sh` - Cross-platform shell script to build the MSI

## Building

### Using Batch Script
```cmd
build-msi.bat
```

### Using PowerShell
```powershell
./build-msi.ps1
```

### Using Shell Script
```bash
./build-msi.sh
```

## Output

The generated MSI file will be created in the project root directory.