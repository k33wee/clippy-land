set dotenv-load := false

name := 'cosmic-applet-clippy-land'
appid := 'com.keewee.CosmicAppletClippyLand'

# configurable paths
bin_dir := env_var_or_default("BIN_DIR", "~/.local/bin")
app_dir := env_var_or_default("APP_DIR", "~/.local/share/applications")
icon_dir := env_var_or_default("ICON_DIR", "~/.local/share/icons/hicolor/scalable/apps")
metainfo_dir := env_var_or_default("METAINFO_DIR", "~/.local/share/metainfo")
# Desktop entries don't expand "~"; the install recipe expands it before writing Exec=.
exec_path := env_var_or_default("EXEC_PATH", bin_dir + "/" + name)

# default recipe
_default:
    @just --list

# Build release binary
build:
    cargo build --release

# Install for current user
install: build
    install -Dm755 target/release/{{name}} {{bin_dir}}/{{name}}
    install -Dm644 resources/com.keewee.CosmicAppletClippyLand.desktop {{app_dir}}/{{appid}}.desktop
    # Keep the same Exec field code as the shipped desktop file, but ensure ~/ is expanded.
    sh -c 'exec="{{exec_path}}"; exec=$(printf %s "$exec" | sed "s|^~/|$HOME/|"); sed -i "s|^Exec=.*|Exec=$exec %F|" "$1"' sh {{app_dir}}/{{appid}}.desktop
    install -Dm644 resources/app.metainfo.xml {{metainfo_dir}}/{{appid}}.metainfo.xml
    install -Dm644 resources/icon.svg {{icon_dir}}/{{appid}}.svg
    update-desktop-database {{app_dir}} || true
    gtk-update-icon-cache -f ~/.local/share/icons/hicolor || true

# Uninstall for current user
uninstall:
    rm -f {{bin_dir}}/{{name}}
    rm -f {{app_dir}}/{{appid}}.desktop
    rm -f {{metainfo_dir}}/{{appid}}.metainfo.xml
    rm -f {{icon_dir}}/{{appid}}.svg
    update-desktop-database {{app_dir}} || true
    gtk-update-icon-cache -f ~/.local/share/icons/hicolor || true

# Clean build artifacts
clean:
    cargo clean
