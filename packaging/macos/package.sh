#!/usr/bin/env bash
# OpenBrush
# Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
# Author: Aryan Sinha <techyminati>
# Licensed under the CipherOS License 2.0. See LICENSE for the full terms.
#
# Builds a universal (Apple Silicon + Intel) OpenBrush.app and wraps it in a .dmg.
# Usage: packaging/macos/package.sh [version] [output-dir]
set -euo pipefail

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
OUT="${2:-dist}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin

WORK="$(mktemp -d)"
APP="$WORK/OpenBrush.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create \
    target/aarch64-apple-darwin/release/openbrush \
    target/x86_64-apple-darwin/release/openbrush \
    -output "$APP/Contents/MacOS/openbrush"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# Icon set from the pre-rendered PNGs.
ICONSET="$WORK/OpenBrush.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    cp "assets/app-icon/icon-$size.png" "$ICONSET/icon_${size}x${size}.png"
    cp "assets/app-icon/icon-$((size * 2)).png" "$ICONSET/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/OpenBrush.icns"

# Ad-hoc signature so Apple Silicon will launch it (it is not notarised).
codesign --force --deep --sign - "$APP"

mkdir -p "$OUT"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cp LICENSE "$STAGE/LICENSE.txt"
DMG="$OUT/OpenBrush-macos-universal.dmg"
rm -f "$DMG"
hdiutil create -volname "OpenBrush $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$WORK"
echo "$DMG"
