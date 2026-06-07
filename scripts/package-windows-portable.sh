#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(node -p "require('$ROOT/package.json').version")"
TARGET="$ROOT/src-tauri/target/x86_64-pc-windows-msvc/release"
DIST="$ROOT/dist"
STAGING="$DIST/portable-windows"
ZIP="$DIST/Paker-${VERSION}-windows-portable.zip"
INSTALLER_SRC="$TARGET/bundle/nsis/Paker_${VERSION}_x64-setup.exe"

mkdir -p "$STAGING" "$DIST/windows"

cp "$TARGET/paker.exe" "$STAGING/paker.exe"
touch "$STAGING/portable.txt"
rm -f "$ZIP"
(cd "$STAGING" && zip -j -q "$ZIP" paker.exe portable.txt)

cp "$TARGET/paker.exe" "$DIST/windows/paker.exe"
if [[ -f "$INSTALLER_SRC" ]]; then
  cp "$INSTALLER_SRC" "$DIST/windows/"
fi

echo "Portable zip: $ZIP"
echo "Installer:    $DIST/windows/Paker_${VERSION}_x64-setup.exe"
echo "Raw exe:      $DIST/windows/paker.exe"
