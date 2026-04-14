#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
APP_NAME="Rovdex"
ICNS_PATH="$ROOT_DIR/assets/icons/Rovdex.icns"
BUNDLE_ID="com.rovdex.app"
VERSION="$(awk '
  $0 ~ /^\[workspace.package\]/ { in_block=1; next }
  in_block && $0 ~ /^\[/ { in_block=0 }
  in_block && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
' "$ROOT_DIR/Cargo.toml")"

usage() {
  cat <<'EOF'
Usage:
  scripts/package.sh [macos|windows] [target-triple]

Examples:
  scripts/package.sh macos
  scripts/package.sh windows x86_64-pc-windows-msvc

Notes:
  - macOS packages are emitted as .dmg containing Rovdex.app
  - Windows packages are emitted as .exe
  - The script expects the required Rust target and linker toolchain to already exist
EOF
}

platform="${1:-macos}"
requested_target="${2:-}"

case "$platform" in
  macos)
    target="${requested_target:-$(rustc -Vv | awk '/host:/ {print $2}')}"
    bin_name="$APP_NAME"
    archive_ext="dmg"
    case "$target" in
      aarch64-apple-darwin) archive_name="${APP_NAME}-macOS-arm64.dmg" ;;
      x86_64-apple-darwin) archive_name="${APP_NAME}-macOS-x64.dmg" ;;
      *) archive_name="${APP_NAME}-${target}.dmg" ;;
    esac
    ;;
  windows)
    target="${requested_target:-x86_64-pc-windows-msvc}"
    bin_name="${APP_NAME}.exe"
    archive_ext="exe"
    case "$target" in
      x86_64-pc-windows-msvc) archive_name="${APP_NAME}-Windows-x64.exe" ;;
      aarch64-pc-windows-msvc) archive_name="${APP_NAME}-Windows-arm64.exe" ;;
      *) archive_name="${APP_NAME}-${target}.exe" ;;
    esac
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    echo "unsupported platform: $platform" >&2
    usage
    exit 1
    ;;
esac

cd "$ROOT_DIR"
mkdir -p "$DIST_DIR"

echo "Building target: $target"
cargo build --release -p rovdex-cli --target "$target"

stage_dir="$DIST_DIR/package-${target}"
rm -rf "$stage_dir"
mkdir -p "$stage_dir"

source_bin="$ROOT_DIR/target/$target/release/rovdex-cli"
if [[ "$platform" == "windows" ]]; then
  source_bin="$ROOT_DIR/target/$target/release/rovdex-cli.exe"
fi

if [[ ! -f "$source_bin" ]]; then
  echo "expected binary not found: $source_bin" >&2
  exit 1
fi

if [[ "$platform" == "macos" && ! -f "$ICNS_PATH" ]]; then
  echo "missing macOS icon: $ICNS_PATH" >&2
  echo "run: ./scripts/generate_icons.sh" >&2
  exit 1
fi

if [[ "$platform" == "macos" ]]; then
  app_dir="$stage_dir/${APP_NAME}.app"
  contents_dir="$app_dir/Contents"
  macos_dir="$contents_dir/MacOS"
  resources_dir="$contents_dir/Resources"
  mkdir -p "$macos_dir" "$resources_dir"
  cp "$source_bin" "$macos_dir/$APP_NAME"
  chmod +x "$macos_dir/$APP_NAME"
  cat > "$contents_dir/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>${APP_NAME}.icns</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleSignature</key>
  <string>ROVD</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF
  cp "$ICNS_PATH" "$resources_dir/${APP_NAME}.icns"
  cp "$ROOT_DIR/README.md" "$stage_dir/README.md"
  cp "$ROOT_DIR/LICENSE" "$stage_dir/LICENSE"
  ln -s /Applications "$stage_dir/Applications"
  rm -f "$DIST_DIR/$archive_name"
  hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$stage_dir" \
    -ov \
    -format UDZO \
    "$DIST_DIR/$archive_name"
else
  cp "$source_bin" "$stage_dir/$bin_name"
  cp "$ROOT_DIR/README.md" "$stage_dir/README.md"
  cp "$ROOT_DIR/LICENSE" "$stage_dir/LICENSE"
  cp "$source_bin" "$DIST_DIR/$archive_name"
fi

echo "Created package: $DIST_DIR/$archive_name"
