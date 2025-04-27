# Golem GPU Imager

A tool for downloading, setting up, and writing OS images for Golem GPU devices.

## Overview

Golem GPU Imager is a utility similar to the Raspberry Pi Imager, designed specifically for Golem GPU devices. It simplifies the process of getting your Golem GPU up and running with the correct operating system.

## Features

- Browse and download official Golem GPU OS images
- Configure OS settings before writing
- Write images to SD cards and USB devices
- Verify written images for integrity
- Simple and intuitive interface

## Installation

```bash
cargo install golem-gpu-imager
```

## Usage

```bash
golem-gpu-imager
```

## Building from Source

```bash
git clone https://github.com/golem/golem-gpu-imager.git
cd golem-gpu-imager
cargo build --release
```

## License

[MIT](LICENSE)