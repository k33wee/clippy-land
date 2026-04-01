# Prerequisites

- Install Flatpak and flatpak-builder (version >= 1.4.2)
- Install Rust toolchain (for cargo vendor)
- Install Flatpak SDKs: `org.freedesktop.Sdk` and `org.freedesktop.Platform` (rust-stable extension recommended)

## Quick start (fresh clone)

```bash
# generate vendored crates and a matching cargo config
mkdir -p .cargo
cargo vendor > .cargo/config.toml

# verify offline metadata works
cargo metadata --offline --format-version 1 >/dev/null

# Uninstall old version
flatpak uninstall io.github.k33wee.clippy-land -y || true

# Build and install the new version
flatpak-builder --force-clean --repo=repo build-dir io.github.k33wee.clippy-land.json

# Install the new version with builder
flatpak --user remote-delete local-repo || true
flatpak --user remote-add --no-gpg-verify local-repo file://$PWD/repo
flatpak --user install local-repo io.github.k33wee.clippy-land

# Alternatively, create a bundle and install that
flatpak build-bundle repo clippy-land.flatpak io.github.k33wee.clippy-land
flatpak install clippy-land.flatpak
```
