#!/usr/bin/env bash
# Normalize the desktop-file ID for GLib/D-Bus and preserve the Tauri payload.
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
source_deb="${CARGO_TARGET_DIR:-$project_dir/target}/release/bundle/deb/JARVIS Monitor_1.0.0_amd64.deb"
temp_dir="$(mktemp -d /tmp/jarvis-package.XXXXXXXX)"
trap 'rm -rf -- "$temp_dir"' EXIT
dpkg-deb -R "$source_deb" "$temp_dir/package"
entry_dir="$temp_dir/package/usr/share/applications"
mv -- "$entry_dir/JARVIS Monitor.desktop" "$entry_dir/local.jarvis.Monitor.desktop"
sed -i '/^Terminal=false$/a DBusActivatable=false' "$entry_dir/local.jarvis.Monitor.desktop"
desktop-file-validate "$entry_dir/local.jarvis.Monitor.desktop"
(
  cd "$temp_dir/package"
  find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums
)
mkdir -p "$project_dir/release"
dpkg-deb --build --root-owner-group "$temp_dir/package" "$project_dir/release/jarvis-monitor_1.0.0_amd64.deb"
(
  cd "$project_dir/release"
  sha256sum jarvis-monitor_1.0.0_amd64.deb > SHA256SUMS
)
