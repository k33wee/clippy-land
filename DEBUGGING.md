# Debugging and issue reports

This guide explains how to collect useful logs for Clippy Land and how to open a good bug report.

## Before opening an issue

Please include:

- install method: Cosmic Store / Flatpak / `.deb` / RPM / source install
- Clippy Land version
- COSMIC / Pop!_OS version if known
- whether the issue happens on:
  - direct panel click
  - `--toggle`
  - both
- whether the issue is new or worked in an older release
- screenshots or screen recordings if the issue is visual

If the problem is performance-related, please also include logs from the debug wrapper below.

## Collect panel debug logs

Clippy Land ships a debug wrapper that enables `CLIPPY_LAND_DEBUG_TIMING=1` and writes panel-spawned applet logs to a file.

Default log path:

```bash
${XDG_STATE_HOME:-$HOME/.local/state}/clippy-land/panel-debug.log
```

You can override the log file with:

```bash
CLIPPY_LAND_DEBUG_LOG_FILE=/path/to/custom.log
```

## Source or custom-prefix installs

Enable the installed debug wrapper:

```bash
just prefix="$HOME/.local" enable-debug-wrapper
pkill -9 cosmic-panel
```

Disable it again later:

```bash
just prefix="$HOME/.local" disable-debug-wrapper
pkill -9 cosmic-panel
```

## Native package installs (`.deb`, RPM, etc.)

The wrapper is installed alongside the normal binary as:

```bash
/usr/bin/cosmic-applet-clippy-land-debug.sh
```

To use it in COSMIC panel:

1. Copy the desktop entry to:
   `~/.local/share/applications/io.github.k33wee.clippy-land.desktop`
2. Change `Exec=` to:

```bash
/usr/bin/cosmic-applet-clippy-land-debug.sh
```

3. Restart the panel:

```bash
pkill -9 cosmic-panel
```

## Flatpak installs

The debug wrapper is available inside the sandbox as:

```bash
flatpak run --command=cosmic-applet-clippy-land-debug.sh io.github.k33wee.clippy-land
```

To make the panel applet use it, create a user-local desktop-entry override and set:

```bash
Exec=flatpak run --command=cosmic-applet-clippy-land-debug.sh io.github.k33wee.clippy-land
```

Then restart the panel:

```bash
pkill -9 cosmic-panel
```

## Reproduce the problem

After enabling the wrapper:

1. restart COSMIC panel
2. reproduce the issue
3. test both paths when relevant:
   - click the applet in the panel
   - run `cosmic-applet-clippy-land --toggle` or `flatpak run ... --toggle`
4. attach the log file to the issue

## Most useful log lines

- `ipc D-Bus toggle service ready`
- `ipc D-Bus toggle completed in ...`
- `ipc D-Bus toggle delivered to applet`
- `popup requested via ...`
- `popup window opened via ...`
- `first popup redraw observed ...`
- startup timing lines beginning with `startup` or `init stage:`

## How to write a good issue

Suggested template:

```md
### Install method
Flatpak / .deb / RPM / source

### Version
vX.Y.Z

### What happens
Describe the bug briefly.

### How to reproduce
1. ...
2. ...
3. ...

### Does it happen on panel click, --toggle, or both?

### Expected behavior

### Logs
Attach `${XDG_STATE_HOME:-$HOME/.local/state}/clippy-land/panel-debug.log`

### Screenshots / recording
If relevant
```

## Notes

- Timing logs are enabled by the debug wrapper via `CLIPPY_LAND_DEBUG_TIMING=1`.
- The wrapper is meant for diagnostics; normal installs keep using the standard launcher by default.
