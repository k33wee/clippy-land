#!/bin/sh
# Bridge the host COSMIC theme config into the Flatpak sandbox.
# cosmic-config reads from $XDG_CONFIG_HOME/cosmic/, but Flatpak redirects
# $XDG_CONFIG_HOME to a sandboxed path. The host ~/.config/cosmic is accessible
# at ~/.config/cosmic (via --filesystem=xdg-config/cosmic), so we symlink the
# sandbox config dir to the real one before launching the applet.

COSMIC_HOST="$HOME/.config/cosmic"
COSMIC_SANDBOX="$XDG_CONFIG_HOME/cosmic"

if [ -d "$COSMIC_HOST" ] && [ ! -e "$COSMIC_SANDBOX" ]; then
    mkdir -p "$XDG_CONFIG_HOME"
    ln -s "$COSMIC_HOST" "$COSMIC_SANDBOX"
fi

exec /app/bin/cosmic-applet-clippy-land "$@"
