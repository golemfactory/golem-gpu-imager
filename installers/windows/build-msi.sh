#!/bin/bash
# Shell script to build MSI installer for Golem GPU Imager using Docker
# Requirements: Docker must be installed and running

set -e

CONFIGURATION=${1:-release}
OUTPUT_DIR="dist"
DOCKER_IMAGE="jkroepke/wixtoolset:latest"

echo "Building Golem GPU Imager MSI Installer using Docker..."

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo "Error: Docker not found. Please install Docker first."
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo "Error: Docker daemon is not running. Please start Docker."
    exit 1
fi

# Check if the executable exists
EXE_PATH="target/x86_64-pc-windows-gnu/release/golem-gpu-imager.exe"
if [ ! -f "$EXE_PATH" ]; then
    echo "Error: Executable not found at $EXE_PATH"
    echo "Please build the project first with: cargo build --release --target x86_64-pc-windows-gnu"
    exit 1
fi

# Create output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# Extract version from Cargo.toml
VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml"
    exit 1
fi

echo "Version: $VERSION"
echo "Building installer using Docker..."

# Pull the latest WiX Docker image
echo "Pulling Docker image..."
docker pull "$DOCKER_IMAGE"

# Run WiX toolset in Docker container (WiX v4 syntax)
echo "Building MSI with WiX v4..."
MSI_NAME="GolemGpuImager-$VERSION-x64.msi"
docker run --rm \
    -v "$(pwd):/work" \
    -w /work \
    "$DOCKER_IMAGE" \
    build installer.wxs -arch x64 -out "$OUTPUT_DIR/$MSI_NAME"

# Clean up intermediate files
rm -f "$OUTPUT_DIR/installer.wixobj"

echo "Successfully created: $OUTPUT_DIR/$MSI_NAME"

# Display file info
if [ -f "$OUTPUT_DIR/$MSI_NAME" ]; then
    FILE_SIZE=$(du -h "$OUTPUT_DIR/$MSI_NAME" | cut -f1)
    echo "File size: $FILE_SIZE"
    echo "Created: $(date)"
else
    echo "Error: MSI file was not created successfully"
    exit 1
fi

echo "MSI installer build completed successfully!"