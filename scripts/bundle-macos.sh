#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'The macOS bundle must be built on macOS.' >&2
  exit 1
fi
cargo build --release --locked
bundle="dist/Lingo.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/lingo "$bundle/Contents/MacOS/Lingo"
cp assets/Lingo.icns "$bundle/Contents/Resources/Lingo.icns"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Lingo</string>
  <key>CFBundleDisplayName</key><string>Lingo</string>
  <key>CFBundleIdentifier</key><string>io.github.finn-fengming.lingo</string>
  <key>CFBundleExecutable</key><string>Lingo</string>
  <key>CFBundleIconFile</key><string>Lingo</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAccessibilityUsageDescription</key><string>Lingo reads and replaces text that you explicitly select for translation.</string>
</dict></plist>
PLIST
# Ad-hoc signing makes a stable local bundle; distribution still needs Developer ID/notarization.
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
echo "Built $(pwd)/$bundle"
