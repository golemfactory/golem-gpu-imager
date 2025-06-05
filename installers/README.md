# Installers

This directory contains platform-specific installer configurations for Golem GPU Imager.

## Structure

```
installers/
├── windows/          # Windows MSI installer
│   ├── installer.wxs
│   ├── build-msi.bat
│   ├── build-msi.ps1
│   ├── build-msi.sh
│   └── README.md
├── linux/            # Debian/Ubuntu package
│   ├── control
│   ├── build-deb.sh
│   └── README.md
├── macos/            # macOS installer (future)
│   └── README.md
└── README.md         # This file
```

## Platform Support

### Windows
- **Format**: MSI (Windows Installer)
- **Requirements**: WiX Toolset v3.11+
- **Status**: ✅ Available

### Linux (Debian/Ubuntu)
- **Format**: DEB package
- **Requirements**: dpkg-deb, Rust toolchain
- **Status**: ✅ Available

### macOS
- **Format**: DMG (planned)
- **Requirements**: TBD
- **Status**: 🚧 Planned

## Building All Platforms

For automated builds across platforms, refer to the GitHub Actions workflow in `.github/workflows/`.

## Quick Start

1. Navigate to the appropriate platform directory
2. Follow the README instructions for that platform
3. Run the build script for your target platform

Each platform directory contains detailed instructions for building and distributing the installer.