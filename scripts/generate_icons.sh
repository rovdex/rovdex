#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_PNG="${1:-$ROOT_DIR/assets/icons/source.png}"
ICON_DIR="$ROOT_DIR/assets/icons"
ICONSET_DIR="$ICON_DIR/Rovdex.iconset"
ICNS_PATH="$ICON_DIR/Rovdex.icns"
ICO_PATH="$ICON_DIR/Rovdex.ico"

mkdir -p "$ICON_DIR"
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

swift "$ROOT_DIR/scripts/generate_icons.swift" "$SOURCE_PNG" "$ICONSET_DIR"
iconutil -c icns "$ICONSET_DIR" -o "$ICNS_PATH"

python3 - <<'PY' "$ICONSET_DIR" "$ICO_PATH"
import os, struct, sys
iconset_dir, output_path = sys.argv[1], sys.argv[2]
png_names = [
    ("icon_16x16.png", 16),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
]
entries = []
payload = bytearray()
for name, size in png_names:
    path = os.path.join(iconset_dir, name)
    with open(path, "rb") as f:
        data = f.read()
    width = 0 if size >= 256 else size
    height = 0 if size >= 256 else size
    entries.append((width, height, len(data), 6 + 16 * len(png_names) + len(payload)))
    payload.extend(data)

with open(output_path, "wb") as f:
    f.write(struct.pack("<HHH", 0, 1, len(entries)))
    for width, height, size, offset in entries:
        f.write(struct.pack("<BBBBHHII", width, height, 0, 0, 1, 32, size, offset))
    f.write(payload)
PY

echo "Generated:"
echo "  $ICNS_PATH"
echo "  $ICO_PATH"
