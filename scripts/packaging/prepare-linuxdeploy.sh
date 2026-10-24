#!/usr/bin/env bash
set -euo pipefail

# CLI 2.11.1's default linuxdeploy predates LINUXDEPLOY_EXCLUDED_LIBRARIES.
# Use Tauri's pinned replacement before bundling (and updater signing).
tools_dir="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
work_dir="$(mktemp -d)"
trap 'rm -rf -- "$work_dir"' EXIT

curl --fail --location --retry 3 \
  --output "$work_dir/linuxdeploy-x86_64.AppImage" \
  https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy-07333c6/linuxdeploy-x86_64.AppImage
printf '%s  %s\n' \
  '36a2d7e274d12e1050d0e9ecfe11d339ed54720b2bec464c286d53f8b07f5c62' \
  "$work_dir/linuxdeploy-x86_64.AppImage" | sha256sum --check --status

bash "$(dirname "$0")/test-linuxdeploy.sh" "$work_dir/linuxdeploy-x86_64.AppImage"
install -Dm755 "$work_dir/linuxdeploy-x86_64.AppImage" "$tools_dir/linuxdeploy-x86_64.AppImage"
