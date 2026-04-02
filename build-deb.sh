#!/bin/bash
set -e

# Get metadata from Cargo.toml
NAME=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].name')
VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
DESC=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].description')
ARCH=amd64
APPID=io.github.k33wee.clippy-land

# Build the project
cargo build --release

# Prepare directories
rm -rf deb_build
mkdir -p deb_build/DEBIAN \
         deb_build/usr/bin \
         deb_build/usr/share/applications \
         deb_build/usr/share/icons/hicolor/scalable/apps \
         deb_build/usr/share/metainfo

# Copy files
cp target/release/$NAME deb_build/usr/bin/
chmod 755 deb_build/usr/bin/$NAME
cp resources/$APPID.desktop deb_build/usr/share/applications/$APPID.desktop
cp resources/icon.svg deb_build/usr/share/icons/hicolor/scalable/apps/$APPID.svg
cp resources/$APPID.metainfo.xml deb_build/usr/share/metainfo/$APPID.metainfo.xml

# Create control file
cat > deb_build/DEBIAN/control <<EOL
Package: $NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: k33wee <https://github.com/k33wee>
Description: $DESC
EOL

# Build the .deb
DEB_NAME="${NAME}_${VERSION}_${ARCH}.deb"
dpkg-deb --build deb_build "$DEB_NAME"
echo "Created $DEB_NAME"
