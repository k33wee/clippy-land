#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

APP_BIN="${APP_BIN:-$ROOT_DIR/target/debug/cosmic-applet-clippy-land}"
LOG_FILE="${LOG_FILE:-/tmp/clippy-land-e2e-$$.log}"
export CLIPPY_LAND_BUS_NAME="${CLIPPY_LAND_BUS_NAME:-io.github.k33wee.ClippyLand.E2e$$}"

POLL_SETTLE_SECONDS="${POLL_SETTLE_SECONDS:-1.2}"
TOGGLE_SETTLE_SECONDS="${TOGGLE_SETTLE_SECONDS:-0.6}"
KEY_SETTLE_SECONDS="${KEY_SETTLE_SECONDS:-0.25}"

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'missing required command: %s\n' "$1" >&2
        exit 1
    fi
}

cleanup() {
    local exit_code=$?
    set +e
    if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
        "$APP_BIN" --toggle >/dev/null 2>&1 || true
        sleep "$TOGGLE_SETTLE_SECONDS"
        kill "$APP_PID" >/dev/null 2>&1 || true
        wait "$APP_PID" >/dev/null 2>&1 || true
    fi
    rm -f "$TMP_RED_PNG" "$TMP_BLUE_PNG"
    if [[ $exit_code -ne 0 ]]; then
        printf '\nE2E failed. App log: %s\n' "$LOG_FILE" >&2
    fi
    exit "$exit_code"
}

assert_eq() {
    local expected="$1"
    local actual="$2"
    local label="$3"
    if [[ "$expected" != "$actual" ]]; then
        printf 'assertion failed (%s)\nexpected: %s\nactual:   %s\n' "$label" "$expected" "$actual" >&2
        return 1
    fi
}

toggle_popup() {
    "$APP_BIN" --toggle
    sleep "$TOGGLE_SETTLE_SECONDS"
}

key() {
    wtype -k "$1"
    sleep "$KEY_SETTLE_SECONDS"
}

copy_text() {
    printf '%s' "$1" | wl-copy -n
}

clipboard_text() {
    wl-paste -n
}

copy_image_png() {
    local path="$1"
    wl-copy -t image/png < "$path"
}

clipboard_image_sha256() {
    wl-paste -t image/png | sha256sum | cut -d' ' -f1
}

make_png_from_base64() {
    local b64="$1"
    local output="$2"
    printf '%s' "$b64" | base64 -d > "$output"
}

require_cmd cargo
require_cmd wl-copy
require_cmd wl-paste
require_cmd wtype
require_cmd sha256sum
require_cmd base64

if [[ -z "${WAYLAND_DISPLAY:-}" || -z "${XDG_RUNTIME_DIR:-}" ]]; then
    printf 'WAYLAND_DISPLAY and XDG_RUNTIME_DIR must be set for Wayland E2E tests.\n' >&2
    exit 1
fi

TMP_RED_PNG="/tmp/clippy-land-e2e-red-$$.png"
TMP_BLUE_PNG="/tmp/clippy-land-e2e-blue-$$.png"
RED_B64='iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8Dwn4GBgYGJAQoAHxcCAk+Uzr4AAAAASUVORK5CYII='
BLUE_B64='iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGNkYPj/n4GBgYGJAQoAHRkCAjRcHicAAAAASUVORK5CYII='

make_png_from_base64 "$RED_B64" "$TMP_RED_PNG"
make_png_from_base64 "$BLUE_B64" "$TMP_BLUE_PNG"

trap cleanup EXIT

printf 'Building debug binary...\n'
cargo build >/dev/null

printf 'Launching app instance for E2E...\n'
"$APP_BIN" --no-standalone >"$LOG_FILE" 2>&1 &
APP_PID=$!

sleep 1

# Ensure app instance is alive before starting scenarios.
if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
    printf 'app instance exited before scenarios started\n' >&2
    exit 1
fi

printf 'Scenario 1: text selection/copy via toggle + keyboard...\n'
copy_text 'e2e-text-first'
sleep "$POLL_SETTLE_SECONDS"
copy_text 'e2e-text-second'
sleep "$POLL_SETTLE_SECONDS"

toggle_popup
key Down
key Return
sleep "$POLL_SETTLE_SECONDS"

actual_text="$(clipboard_text)"
assert_eq 'e2e-text-second' "$actual_text" 'text entry recopy should match selected row'

key Escape
sleep "$TOGGLE_SETTLE_SECONDS"

printf 'Scenario 2: image selection/copy via toggle + keyboard...\n'
copy_image_png "$TMP_RED_PNG"
sleep "$POLL_SETTLE_SECONDS"
copy_image_png "$TMP_BLUE_PNG"
sleep "$POLL_SETTLE_SECONDS"

expected_img_sha="$(sha256sum "$TMP_BLUE_PNG" | cut -d' ' -f1)"

toggle_popup
key Down
key Return
sleep "$POLL_SETTLE_SECONDS"

actual_img_sha="$(clipboard_image_sha256)"
assert_eq "$expected_img_sha" "$actual_img_sha" 'image entry recopy should match selected row'

key Escape
sleep "$TOGGLE_SETTLE_SECONDS"

printf 'E2E success: toggle-driven clipboard flows are working.\n'
