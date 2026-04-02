# clippy-land

COSMIC panel applet for keeping a history of recently copied text and images.

This applet polls the Wayland clipboard and updates the history when
the contents change.

![applet example](./resources/example.png)

Main features:

- Keep a history of the latest **30** clipboard entries (text + images)
- Re-copy an entry with a single click
- Remove individual entries from the history
- Clear all entries from history with one click
- Pin important entries to the top of the history (5 pinned entries max)
- Move between entries with arrow keys (up/down or k/j to navigate between entries, left/right or h/l to move to pin or delete buttons. You need to interact with the applet at least once to enable keyboard navigation)

## Dependencies

- Wayland clipboard support (via `wl-clipboard-rs`)
- Build dependencies for libcosmic:

```bash
sudo apt install cargo cmake just libexpat1-dev libfontconfig-dev libfreetype-dev libxkbcommon-dev pkgconf
```

## Build

```bash
cargo build --release
```

## Build/Install with just

```bash
# build
just build

# install for current user
just install
```

## Install with custom paths

```
# (supported env vars: BIN_DIR, APP_DIR, ICON_DIR, METAINFO_DIR, EXEC_PATH)
# note: `just install` expands a leading `~/` in EXEC_PATH before writing the desktop `Exec=` line

BIN_DIR=~/.local/bin \
APP_DIR=~/.local/share/applications \
ICON_DIR=~/.local/share/icons/hicolor/scalable/apps \
METAINFO_DIR=~/.local/share/metainfo \
EXEC_PATH=~/.local/bin/cosmic-applet-clippy-land \
just install

# uninstall
just uninstall
```

## Install with Flatpak

1. Add the required remotes (if not already added):

```bash
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
```

2. Download the latest `.flatpak` from the [releases page](https://github.com/k33wee/clippy-land/releases/).
3. In a terminal, navigate to the directory where you downloaded the `.flatpak` file and run:

```bash
flatpak install --user ./clippy-land_<version>.flatpak
```

This will install the applet for your user. If you encounter missing runtime errors, ensure the remotes above are added and try again.

## Install for Debian/Ubuntu

Download the latest .deb from the [releases page](https://github.com/k33wee/clippy-land/releases/):
In a terminal, navigate to the directory where you downloaded the .deb file and run:

```bash
sudo apt install ./cosmic-applet-clippy-land_<version>_amd64.deb
```

## Install for Fedora

Thanks to [lorduskordus](https://github.com/lorduskordus) there is now an RPM package on COPR.

- [copr.fedorainfracloud.org/coprs/kordus/cosmic-applets](https://copr.fedorainfracloud.org/coprs/kordus/cosmic-applets)

Fedora Workstation

```sh
sudo dnf copr enable kordus/cosmic-applets
sudo dnf install cosmic-applet-clippy-land
```

Fedora Atomic

```sh
sudo wget -O /etc/yum.repos.d/_copr:copr.fedorainfracloud.org:kordus:cosmic-applets.repo https://copr.fedorainfracloud.org/coprs/kordus/cosmic-applets/repo/fedora/kordus-cosmic-applets.repo
rpm-ostree install cosmic-applet-clippy-land
```

## Usage

Open **COSMIC Settings → Desktop → Panel → Applets** and add “Clippy Land” to your panel.
You might need to log out and back in to see the applet in the list of available applets.

## Notes

- App ID is currently `io.github.k33wee.clippy-land`.

## Translations

Thanks to our community contributors, Clippy Land is available in the following languages:

- **Italian** ([k33wee](https://github.com/k33wee))
- **English** ([k33wee](https://github.com/k33wee))
- **Portuguese** ([GuilhermeTerriaga](https://github.com/GuilhermeTerriaga))
- **Czech** ([lorduskordus](https://github.com/lorduskordus))
