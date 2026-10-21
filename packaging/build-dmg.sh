#!/bin/sh
# Builds PutiaCNC-<version>-macos.dmg (universal: Apple silicon + Intel). Run on a Mac, from the repo root.
# Needs Xcode command line tools and rustup. The app is ad-hoc signed, not notarized (see README).
set -e
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' gui/Cargo.toml | head -1)
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p putiacnc-gui --target aarch64-apple-darwin
cargo build --release -p putiacnc-gui --target x86_64-apple-darwin

APP=target/dmg/PutiaCNC.app
rm -rf target/dmg && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/putiacnc" \
  target/aarch64-apple-darwin/release/putiacnc target/x86_64-apple-darwin/release/putiacnc
strip "$APP/Contents/MacOS/putiacnc"

# icon: .icns from the PNGs in assets/icons
IS=target/putiacnc.iconset
rm -rf "$IS" && mkdir -p "$IS"
for s in 16 32 128 256 512; do cp "assets/icons/putiacnc-$s.png" "$IS/icon_${s}x${s}.png"; done
cp assets/icons/putiacnc-32.png  "$IS/icon_16x16@2x.png"
cp assets/icons/putiacnc-64.png  "$IS/icon_32x32@2x.png"
cp assets/icons/putiacnc-256.png "$IS/icon_128x128@2x.png"
cp assets/icons/putiacnc-512.png "$IS/icon_256x256@2x.png"
iconutil -c icns "$IS" -o "$APP/Contents/Resources/putiacnc.icns"

cp LICENSE THIRD-PARTY-LICENSES.txt "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>PutiaCNC</string>
  <key>CFBundleDisplayName</key><string>PutiaCNC</string>
  <key>CFBundleIdentifier</key><string>io.github.tonyrh.putiacnc</string>
  <key>CFBundleExecutable</key><string>putiacnc</string>
  <key>CFBundleIconFile</key><string>putiacnc</string>
  <key>CFBundleVersion</key><string>$VER</string>
  <key>CFBundleShortVersionString</key><string>$VER</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
</dict></plist>
PLIST
codesign --force --deep --sign - "$APP"

# disk image: the app, an Applications shortcut, and the licence files
cp LICENSE THIRD-PARTY-LICENSES.txt target/dmg/
ln -s /Applications target/dmg/Applications
OUT="PutiaCNC-$VER-macos.dmg"
rm -f "$OUT"
hdiutil create -volname "PutiaCNC $VER" -srcfolder target/dmg -ov -format UDZO "$OUT"
echo "Built $OUT"
