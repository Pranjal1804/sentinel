#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
bundle=$(find src-tauri/target/release/bundle -type f \( -name '*.deb' -o -name '*.AppImage' \) -print -quit 2>/dev/null || true)
if [[ -z "$bundle" ]]; then echo 'Build the release first: scripts/build-release.sh' >&2; exit 1; fi
dest="${XDG_DATA_HOME:-$HOME/.local/share}/sentinel"
mkdir -p "$dest"
cp "$bundle" "$dest/$(basename "$bundle")"
printf 'Copied %s to %s\n' "$bundle" "$dest"
echo 'For a .deb, install it explicitly with: sudo apt install <path>'
echo 'For an AppImage, make it executable and launch it from the copied location.'
