#!/usr/bin/env bash
set -euo pipefail
printf 'SENTINEL build dependency check\n\n'
for tool in rustc cargo node npm pkg-config; do
  if command -v "$tool" >/dev/null 2>&1; then printf 'OK   %-14s %s\n' "$tool" "$("$tool" --version 2>/dev/null | head -n1)"; else printf 'MISS %s\n' "$tool"; fi
done
for package in webkit2gtk-4.1 gtk+-3.0 sqlite3; do
  if pkg-config --exists "$package" 2>/dev/null; then printf 'OK   %-14s %s\n' "$package" "$(pkg-config --modversion "$package")"; else printf 'MISS %s (required to build the native desktop app)\n' "$package"; fi
done
printf '\nOptional security tools:\n'
for tool in clamscan yara auditctl osqueryi rkhunter chkrootkit lynis smartctl sensors; do
  if command -v "$tool" >/dev/null 2>&1; then printf 'FOUND %s\n' "$tool"; else printf 'not found: %s\n' "$tool"; fi
done
