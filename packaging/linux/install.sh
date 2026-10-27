#!/usr/bin/env bash
# OpenBrush
# Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
# Author: Aryan Sinha <techyminati>
# Licensed under the CipherOS License 2.0. See LICENSE for the full terms.
#
# Installs OpenBrush for the current user (no root needed):
#   ~/.local/bin/openbrush, plus a menu entry and icon.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
install -Dm755 "$HERE/openbrush" "$PREFIX/bin/openbrush"
install -Dm644 "$HERE/openbrush.desktop" "$PREFIX/share/applications/openbrush.desktop"
install -Dm644 "$HERE/openbrush.png" "$PREFIX/share/icons/hicolor/256x256/apps/openbrush.png"
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" || true
echo "OpenBrush installed to $PREFIX/bin/openbrush"
