# macOS Installer

This directory is reserved for future macOS installer configuration for Golem GPU Imager.

## Planned Features

- macOS DMG package creation
- Code signing and notarization
- Homebrew formula
- App bundle configuration

## Status

Currently not implemented. The application can be built and run on macOS using:

```bash
cargo build --release
./target/release/golem-gpu-imager
```