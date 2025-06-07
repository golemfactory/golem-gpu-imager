#!/bin/bash
set -e

# Build Debian package for Golem GPU Imager

PACKAGE_NAME="golem-gpu-imager"
ARCH="amd64"

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Extract version from Cargo.toml
cd "$PROJECT_ROOT"
VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml"
    exit 1
fi

BUILD_DIR="$PROJECT_ROOT/target/debian"
PACKAGE_DIR="$BUILD_DIR/${PACKAGE_NAME}_${VERSION}_${ARCH}"

echo "Building Debian package for $PACKAGE_NAME v$VERSION"

# Clean previous builds
rm -rf "$BUILD_DIR"
mkdir -p "$PACKAGE_DIR/DEBIAN"
mkdir -p "$PACKAGE_DIR/usr/bin"
mkdir -p "$PACKAGE_DIR/usr/share/applications"
mkdir -p "$PACKAGE_DIR/usr/share/pixmaps"
mkdir -p "$PACKAGE_DIR/usr/share/doc/$PACKAGE_NAME"

# Build the application in release mode
echo "Building application..."
cd "$PROJECT_ROOT"
cargo build --release

# Copy the binary
cp "$PROJECT_ROOT/target/release/$PACKAGE_NAME" "$PACKAGE_DIR/usr/bin/"

# Copy control file and update version
sed "s/Version: .*/Version: $VERSION/" "$PROJECT_ROOT/installers/linux/control" > "$PACKAGE_DIR/DEBIAN/control"

# Create desktop entry
cat > "$PACKAGE_DIR/usr/share/applications/$PACKAGE_NAME.desktop" << EOF
[Desktop Entry]
Name=Golem GPU Imager
Comment=GPU Image Management Tool for Golem Network
Exec=/usr/bin/$PACKAGE_NAME
Icon=$PACKAGE_NAME
Terminal=false
Type=Application
Categories=System;Utility;
Keywords=golem;gpu;image;flash;usb;
StartupNotify=true
EOF

# Copy icon
cp "$PROJECT_ROOT/resources/icon.png" "$PACKAGE_DIR/usr/share/pixmaps/$PACKAGE_NAME.png"

# Create copyright file
cat > "$PACKAGE_DIR/usr/share/doc/$PACKAGE_NAME/copyright" << EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: golem-gpu-imager
Upstream-Contact: Golem Factory <contact@golemfactory.com>
Source: https://github.com/golemfactory/golem-gpu-imager

Files: *
Copyright: 2024 Golem Factory
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 SOFTWARE.
EOF

# Create changelog
cat > "$PACKAGE_DIR/usr/share/doc/$PACKAGE_NAME/changelog.Debian" << EOF
$PACKAGE_NAME ($VERSION) unstable; urgency=medium

  * Initial Debian package release
  * GPU image management functionality
  * Ethereum wallet configuration
  * Storage device flashing capabilities

 -- Golem Factory <contact@golemfactory.com>  $(date -R)
EOF

# Compress changelog
gzip -9 "$PACKAGE_DIR/usr/share/doc/$PACKAGE_NAME/changelog.Debian"

# Set correct permissions
chmod 755 "$PACKAGE_DIR/usr/bin/$PACKAGE_NAME"
chmod 644 "$PACKAGE_DIR/usr/share/applications/$PACKAGE_NAME.desktop"
chmod 644 "$PACKAGE_DIR/usr/share/pixmaps/$PACKAGE_NAME.png"

# Calculate installed size
INSTALLED_SIZE=$(du -sk "$PACKAGE_DIR" | cut -f1)
sed -i "s/Installed-Size: .*/Installed-Size: $INSTALLED_SIZE/" "$PACKAGE_DIR/DEBIAN/control"

# Build the package
echo "Creating Debian package..."
dpkg-deb --build "$PACKAGE_DIR"

# Move the package to a more accessible location
mv "$PACKAGE_DIR.deb" "$PROJECT_ROOT/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"

echo "Debian package created: ${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
echo "Install with: sudo dpkg -i ${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"