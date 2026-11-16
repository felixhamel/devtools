#!/usr/bin/env bash
# Builds DevUtils.app and a zip of it for releases, from the release binary.
#
#   scripts/bundle-macos.sh             # dist/DevUtils.app + dist/DevUtils-<version>-macos-<arch>.zip
#   scripts/bundle-macos.sh --install   # ... then copy DevUtils.app to /Applications
#
# The app is signed ad hoc (required on Apple Silicon), not with a Developer ID,
# so it is not notarized: a copy downloaded from the internet must be allowed
# once (right-click > Open, or `xattr -dr com.apple.quarantine DevUtils.app`).
set -euo pipefail

BUNDLE_ID="io.github.felixhamel.devtools"
root="$(cd "$(dirname "$0")/.." && pwd)"
app_dir="$root/app"
dist="$root/dist"
bundle="$dist/DevUtils.app"
icon_src="$app_dir/assets/app-icon.png"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$app_dir/Cargo.toml" | head -1)"
arch="$(uname -m)"

cargo build --release --manifest-path "$app_dir/Cargo.toml"

rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"

# Icon: every size macOS uses, generated from the source PNG.
iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$icon_src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$icon_src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/AppIcon.icns"
rm -rf "$(dirname "$iconset")"

cp "$app_dir/target/release/devutils" "$bundle/Contents/MacOS/devutils"
cat > "$bundle/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DevUtils</string>
  <key>CFBundleDisplayName</key><string>DevUtils</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>devutils</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
plutil -lint -s "$bundle/Contents/Info.plist"

codesign --force --sign - "$bundle"
codesign --verify --strict "$bundle"

zip="$dist/DevUtils-$version-macos-$arch.zip"
rm -f "$zip"
# ditto keeps the bundle's metadata and signature intact, unlike plain zip.
ditto -c -k --keepParent "$bundle" "$zip"
echo "Built $bundle"
echo "Built $zip"

if [[ "${1:-}" == "--install" ]]; then
  target="/Applications/DevUtils.app"
  if [[ -e "$target" ]]; then
    existing="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$target/Contents/Info.plist" 2>/dev/null || true)"
    if [[ "$existing" != "$BUNDLE_ID" ]]; then
      echo "Refusing to replace $target: it is a different app ($existing)." >&2
      exit 1
    fi
    rm -rf "$target"
  fi
  ditto "$bundle" "$target"
  echo "Installed $target"
fi
