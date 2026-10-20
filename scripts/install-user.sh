#!/usr/bin/env bash
# Install only this application for the current user. No root, daemon or autostart.
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
deb_file="${1:-$project_dir/release/jarvis-monitor_1.0.0_amd64.deb}"
app_dir="$HOME/.local/lib/jarvis-monitor/1.0.0"
entry_file="$HOME/.local/share/applications/local.jarvis.Monitor.desktop"
icon_file="$HOME/.local/share/icons/hicolor/128x128/apps/local.jarvis.monitor.png"
link_file="$HOME/.local/bin/jarvis-monitor"
for target in "$app_dir" "$entry_file" "$icon_file" "$link_file"; do
  if [[ -e "$target" || -L "$target" ]]; then
    printf 'Instalação interrompida para preservar arquivo existente: %s\n' "$target" >&2
    exit 1
  fi
done
[[ -f "$deb_file" ]] || { printf 'Pacote não encontrado: %s\n' "$deb_file" >&2; exit 1; }
temp_dir="$(mktemp -d /tmp/jarvis-install.XXXXXXXX)"
trap 'rm -rf -- "$temp_dir"' EXIT
dpkg-deb -x "$deb_file" "$temp_dir"
install -Dm755 "$temp_dir/usr/bin/jarvis-monitor" "$app_dir/jarvis-monitor"
install -Dm644 "$temp_dir/usr/share/icons/hicolor/128x128/apps/jarvis-monitor.png" "$icon_file"
mkdir -p -- "$(dirname -- "$entry_file")" "$(dirname -- "$link_file")"
ln -s -- "$app_dir/jarvis-monitor" "$link_file"
cat > "$entry_file" <<DESKTOP
[Desktop Entry]
Type=Application
Name=JARVIS Monitor
Comment=Monitor de hardware local somente leitura
Exec="$app_dir/jarvis-monitor"
Icon=$icon_file
Terminal=false
DBusActivatable=false
Categories=System;Monitor;
StartupWMClass=jarvis-monitor
DESKTOP
if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$HOME/.local/share/applications"
fi
printf 'Instalado para este usuário: %s\nMenu: JARVIS Monitor\n' "$app_dir/jarvis-monitor"
