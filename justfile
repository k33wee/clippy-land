set dotenv-load := false

name := 'cosmic-applet-clippy-land'
appid := 'io.github.k33wee.clippy-land'
prefix := '/usr'

# configurable paths
bin_dir := env_var_or_default("BIN_DIR", "~/.local/bin")
app_dir := env_var_or_default("APP_DIR", "~/.local/share/applications")
icon_dir := env_var_or_default("ICON_DIR", "~/.local/share/icons/hicolor/scalable/apps")
metainfo_dir := env_var_or_default("METAINFO_DIR", "~/.local/share/metainfo")

# default recipe
_default:
    @just --list

# Build release binary
build *args:
    cargo build --release {{args}}

# Alias for Flatpak compatibility
build-release *args:
    just build {{args}}

# Install for current user
install:
    install -Dm755 target/release/{{name}} {{bin_dir}}/{{name}}
    install -Dm755 resources/{{name}}.sh {{bin_dir}}/{{name}}.sh
    install -Dm644 resources/{{appid}}.desktop {{app_dir}}/{{appid}}.desktop
    install -Dm644 resources/{{appid}}.metainfo.xml {{metainfo_dir}}/{{appid}}.metainfo.xml
    install -Dm644 resources/icon.svg {{icon_dir}}/{{appid}}-symbolic.svg
    install -Dm644 LICENSE {{bin_dir}}/../share/licenses/{{appid}}/LICENSE
    update-desktop-database {{app_dir}} || true
    gtk-update-icon-cache -f ~/.local/share/icons/hicolor || true

# Uninstall for current user
uninstall:
    rm -f {{bin_dir}}/{{name}}
    rm -f {{bin_dir}}/{{name}}.sh
    rm -f {{app_dir}}/{{appid}}.desktop
    rm -f {{metainfo_dir}}/{{appid}}.metainfo.xml
    rm -f {{icon_dir}}/{{appid}}-symbolic.svg
    rm -f {{bin_dir}}/../share/licenses/{{appid}}/LICENSE
    update-desktop-database {{app_dir}} || true
    gtk-update-icon-cache -f ~/.local/share/icons/hicolor || true

# Clean build artifacts
clean:
    cargo clean
