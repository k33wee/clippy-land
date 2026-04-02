#!/bin/bash
set -e

# Get metadata from Cargo.toml
NAME=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].name')
VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
APPID=io.github.k33wee.clippy-land

cargo build --release

# generate vendored crates and a matching cargo config
mkdir -p .cargo
cargo vendor > .cargo/config.toml

# verify offline metadata works
cargo metadata --offline --format-version 1 >/dev/null

# Uninstall old version
flatpak uninstall $APPID -y || true

# Build and install the new version
flatpak-builder --force-clean --repo=repo build-dir $APPID.json

flatpak build-bundle repo clippy-land_$VERSION.flatpak io.github.k33wee.clippy-land

echo "Created clippy-land_$VERSION.flatpak"
