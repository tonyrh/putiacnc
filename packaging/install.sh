#!/usr/bin/env bash
# Installs a source build of PutiaCNC for the current user (Linux): the program, its menu entry
# and its icons, so launchers, taskbars and window switchers show the icon (on Wayland they find it
# through the app id "putiacnc", not through the window).
#
#   packaging/install.sh               build in release mode if needed and install into ~/.local
#   packaging/install.sh --uninstall   remove what it installed
#   PREFIX=/usr/local packaging/install.sh   install somewhere else (needs write access)
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="${CARGO_TARGET_DIR:-target}"
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin/putiacnc"
APPS="$PREFIX/share/applications"
ICONS="$PREFIX/share/icons/hicolor"
SIZES="16 24 32 48 64 128 256 512"

refresh() {
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$ICONS" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$BIN" "$APPS/putiacnc.desktop"
    for s in $SIZES; do rm -f "$ICONS/${s}x${s}/apps/putiacnc.png"; done
    refresh
    echo "PutiaCNC removed from $PREFIX"
    exit 0
fi

if [ ! -x "$TARGET/release/putiacnc" ] || [ -n "$(find gui/src src Cargo.toml gui/Cargo.toml -newer "$TARGET/release/putiacnc" -print -quit 2>/dev/null)" ]; then
    cargo build --release -p putiacnc-gui
fi

install -Dm755 "$TARGET/release/putiacnc" "$BIN"
# the menu entry points at the installed program, so it works even when ~/.local/bin is not on PATH
sed "s|^Exec=.*|Exec=$BIN %f|" packaging/putiacnc.desktop | install -Dm644 /dev/stdin "$APPS/putiacnc.desktop"
for s in $SIZES; do install -Dm644 "assets/icons/putiacnc-$s.png" "$ICONS/${s}x${s}/apps/putiacnc.png"; done
refresh
echo "Installed PutiaCNC into $PREFIX (program, menu entry and icons)."
echo "Restart your launcher or panel if the icon does not show up yet."
