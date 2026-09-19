#!/usr/bin/env bash
# generate-cargo-sources.sh
# Generates cargo-sources.json from Cargo.lock using flatpak-cargo-generator.
# The tool is cloned once to ~/.local/share/flatpak-builder-tools and reused on
# every subsequent run. Requires: git, uv

set -euo pipefail

TOOLS_DIR="${HOME}/.local/share/flatpak-builder-tools"
GENERATOR="${TOOLS_DIR}/cargo/flatpak-cargo-generator.py"
TOOLS_REPO="https://github.com/flatpak/flatpak-builder-tools.git"
OUTPUT="${1:-cargo-sources.json}"

# ── 1. Ensure uv is available ────────────────────────────────────────────────
if ! command -v uv &>/dev/null; then
    echo "Error: uv is not installed. Install it from https://docs.astral.sh/uv/getting-started/installation/" >&2
    exit 1
fi

# ── 2. Ensure the tool is present ────────────────────────────────────────────
if [[ ! -f "${GENERATOR}" ]]; then
    echo "→ Cloning flatpak-builder-tools to ${TOOLS_DIR} ..."
    git clone --depth=1 "${TOOLS_REPO}" "${TOOLS_DIR}"
else
    echo "→ flatpak-cargo-generator found at ${GENERATOR}"
fi

# ── 3. Run the generator ─────────────────────────────────────────────────────
if [[ ! -f "Cargo.lock" ]]; then
    echo "Error: Cargo.lock not found. Run this script from your project root." >&2
    exit 1
fi

echo "→ Generating ${OUTPUT} from Cargo.lock ..."
uv run --with aiohttp --with tomlkit "${GENERATOR}" Cargo.lock -o "${OUTPUT}"
python3 scripts/check-cargo-sources.py --rewrite "${OUTPUT}"
echo "✔ Done — ${OUTPUT} updated"
