#!/usr/bin/env bash
set -euo pipefail
if ! grep -q 'ID=ubuntu' /etc/os-release; then echo 'SENTINEL currently targets Ubuntu.' >&2; exit 1; fi
version=$( . /etc/os-release; printf '%s' "$VERSION_ID" )
arch=$(dpkg --print-architecture)
printf 'Ubuntu %s (%s)\n' "$version" "$arch"
case "$version" in 22.04|24.04) echo 'Supported Ubuntu release.';; *) echo 'Compatibility is not confirmed for this Ubuntu release.';; esac
"$(dirname "$0")/check-dependencies.sh"
if ! pkg-config --exists webkit2gtk-4.1 gtk+-3.0 sqlite3 2>/dev/null; then
  echo 'Native build libraries are missing. Nothing was installed.'
  echo 'Install manually if desired: build-essential, curl, wget, file, libssl-dev, libsqlite3-dev, libayatana-appindicator3-dev, librsvg2-dev, libwebkit2gtk-4.1-dev, and libgtk-3-dev.'
  read -r -p 'Install these build packages with apt now? [y/N] ' answer
  if [[ "$answer" =~ ^[Yy]$ ]]; then sudo apt-get install build-essential curl wget file libssl-dev libsqlite3-dev libayatana-appindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev libgtk-3-dev; else echo 'Skipped package installation.'; fi
fi
