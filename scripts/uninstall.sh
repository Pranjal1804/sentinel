#!/usr/bin/env bash
set -euo pipefail
dest="${XDG_DATA_HOME:-$HOME/.local/share}/sentinel"
if [[ ! -d "$dest" ]]; then echo 'No user-level SENTINEL bundle directory found.'; exit 0; fi
read -r -p "Remove only the user-level SENTINEL bundle at $dest? [y/N] " answer
if [[ "$answer" =~ ^[Yy]$ ]]; then rm -rf -- "$dest"; echo 'Removed the SENTINEL bundle. User data and system configuration were left untouched.'; else echo 'Uninstall cancelled.'; fi
