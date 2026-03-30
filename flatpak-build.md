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

# then build
flatpak-builder --force-clean build-dir flatpak_schema.json

# (Re)create remote and install:
flatpak --user remote-delete local-repo || true
flatpak --user remote-add --no-gpg-verify local-repo file://$PWD/repo
flatpak --user install local-repo com.keewee.CosmicAppletClippyLand

# Run:
flatpak run com.keewee.CosmicAppletClippyLand
```
