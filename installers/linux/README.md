# Linux Installer

This directory contains the Debian package configuration for Golem GPU Imager on Ubuntu/Debian systems.

## Prerequisites

- dpkg-deb (usually pre-installed on Debian/Ubuntu)
- Rust toolchain (for building the application)

## Files

- `control` - Debian package control file with metadata
- `build-deb.sh` - Script to build the Debian package
- `README.md` - This documentation

## Building

### Build Debian Package
```bash
./build-deb.sh
```

This will:
1. Build the application in release mode
2. Create the Debian package structure
3. Generate the `.deb` file in the project root

## Installation

### Install the Package
```bash
sudo dpkg -i golem-gpu-imager_<version>_amd64.deb
```

### Install Dependencies (if needed)
```bash
sudo apt-get install -f
```

## Dependencies

The package depends on:
- libc6 (>= 2.31)
- libgcc-s1 (>= 3.0) 
- libgtk-3-0 (for GUI)
- udisks2 (for disk management)

## Uninstallation

```bash
sudo dpkg -r golem-gpu-imager
```