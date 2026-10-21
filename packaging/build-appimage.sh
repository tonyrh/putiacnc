#!/bin/sh
# Builds PutiaCNC-<version>-x86_64.AppImage. Needs appimagetool (https://github.com/AppImage/appimagetool)
# on PATH or in $APPIMAGETOOL. Run from the repository root.
set -e
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' gui/Cargo.toml | head -1)
cargo build --release -p putiacnc-gui
A=target/PutiaCNC.AppDir
rm -rf "$A"
mkdir -p "$A/usr/bin" "$A/usr/share/licenses/putiacnc" "$A/usr/share/applications" "$A/usr/share/icons/hicolor/256x256/apps"
cp target/release/putiacnc "$A/usr/bin/putiacnc" && strip "$A/usr/bin/putiacnc"
cp LICENSE THIRD-PARTY-LICENSES.txt "$A/usr/share/licenses/putiacnc/"
cp packaging/putiacnc.desktop "$A/putiacnc.desktop"
cp packaging/putiacnc.desktop "$A/usr/share/applications/"
cp assets/icons/putiacnc-256.png "$A/putiacnc.png"
cp assets/icons/putiacnc-256.png "$A/usr/share/icons/hicolor/256x256/apps/putiacnc.png"
printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexec "$HERE/usr/bin/putiacnc" "$@"\n' > "$A/AppRun"
chmod +x "$A/AppRun"
ARCH=x86_64 "${APPIMAGETOOL:-appimagetool}" "$A" "PutiaCNC-$VER-x86_64.AppImage"
