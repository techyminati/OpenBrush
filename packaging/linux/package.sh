#!/usr/bin/env bash
# OpenBrush
# Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
# Author: Aryan Sinha <techyminati>
# Licensed under the CipherOS License 2.0. See LICENSE for the full terms.
#
# Builds OpenBrush and packs it with a desktop entry, icon and installer.
# Usage: packaging/linux/package.sh [output-dir]
set -euo pipefail
OUT="${1:-dist}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

cargo build --release --locked
STAGE="$(mktemp -d)/OpenBrush"
mkdir -p "$STAGE"
cp target/release/openbrush "$STAGE/"
cp packaging/linux/openbrush.desktop packaging/linux/install.sh "$STAGE/"
cp assets/app-icon/icon-256.png "$STAGE/openbrush.png"
cp LICENSE THIRD_PARTY_NOTICES.md "$STAGE/"
chmod +x "$STAGE/openbrush" "$STAGE/install.sh"
mkdir -p "$OUT"
TARBALL="$OUT/OpenBrush-linux-x86_64.tar.gz"
tar -C "$(dirname "$STAGE")" -czf "$TARBALL" OpenBrush
echo "$TARBALL"
