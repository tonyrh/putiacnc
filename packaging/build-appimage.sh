#!/bin/sh
# Builds putiacnc-<version>-x86_64.AppImage. Needs appimagetool (https://github.com/AppImage/appimagetool)
# on PATH or in $APPIMAGETOOL. Run from the repository root.
set -e
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' gui/Cargo.toml | head -1)
cargo build --release -p putiacnc-gui
T="${CARGO_TARGET_DIR:-target}"
A="$T/putiacnc.AppDir"
rm -rf "$A"
mkdir -p "$A/usr/bin" "$A/usr/share/licenses/putiacnc" "$A/usr/share/applications" "$A/usr/share/icons/hicolor/256x256/apps"
cp "$T/release/putiacnc" "$A/usr/bin/putiacnc" && strip "$A/usr/bin/putiacnc"
cp LICENSE THIRD-PARTY-LICENSES.txt "$A/usr/share/licenses/putiacnc/"
cp packaging/putiacnc.desktop "$A/putiacnc.desktop"
cp packaging/putiacnc.desktop "$A/usr/share/applications/"
mkdir -p "$A/usr/share/metainfo" && cp packaging/putiacnc.appdata.xml "$A/usr/share/metainfo/"
cp assets/icons/putiacnc-256.png "$A/putiacnc.png"
cp assets/icons/putiacnc-256.png "$A/usr/share/icons/hicolor/256x256/apps/putiacnc.png"
# The windowing libraries the program loads at run time (winit opens them with dlopen, so ldd does not list them).
# Bundled so that the AppImage starts on a system that lacks them; a few are on every desktop, these often are not.
mkdir -p "$A/usr/lib"
for lib in libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1; do
  path=$(ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ { print $NF; exit }')
  [ -n "$path" ] || { echo "error: $lib not found on this system, cannot bundle it" >&2; exit 1; }
  cp -L "$path" "$A/usr/lib/$lib"
done
printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexport LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\nexec "$HERE/usr/bin/putiacnc" "$@"\n' > "$A/AppRun"
chmod +x "$A/AppRun"
ARCH=x86_64 "${APPIMAGETOOL:-appimagetool}" "$A" "putiacnc-$VER-x86_64.AppImage"
