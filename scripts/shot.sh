#!/bin/sh
# Take a screenshot of the window in a given state, on a virtual screen (Linux; needs xvfb and imagemagick).
#
#   scripts/shot.sh [options] [project.json]
#
#   -o FILE     where to write the picture (default shot.png)
#   -w WxH      window size (default 1920x985; the smallest the app allows is about 1354x640)
#   -t THEME    light or dark (default light)
#   -c X,Y,W,H  keep only this part of the screen (default: the whole window)
#   -s SECONDS  how long to wait before the picture (default 10; toolpath checks need up to 60)
#   -e VAR=VAL  an extra PUTIACNC_* setting, e.g. -e PUTIACNC_VIEW=3d (can be repeated; see docs/testing.md)
#   -b BINARY   the program (default $CARGO_TARGET_DIR/debug/putiacnc, else target/debug/putiacnc)
#
# Examples:
#   scripts/shot.sh -e PUTIACNC_VIEW=3d -e PUTIACNC_PLAY=0.6 -o 3d.png tests/data/outside_holes.project.json
#   scripts/shot.sh -t dark -w 1354x800 -c 396,600,534,200 -o bars.png my.project.json
set -e
out=shot.png; win=1920x985; theme=light; crop=""; wait=10; bin=""; envs=""
while getopts "o:w:t:c:s:e:b:h" o; do
  case $o in
    o) out=$OPTARG ;; w) win=$OPTARG ;; t) theme=$OPTARG ;; c) crop=$OPTARG ;; s) wait=$OPTARG ;;
    e) envs="$envs $OPTARG" ;; b) bin=$OPTARG ;;
    *) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  esac
done
shift $((OPTIND - 1))
proj=${1:-}
[ -n "$bin" ] || bin="${CARGO_TARGET_DIR:-target}/debug/putiacnc"
[ -x "$bin" ] || { echo "No program at $bin. Build it first: cargo build -p putiacnc-gui" >&2; exit 1; }
command -v import >/dev/null || { echo "ImageMagick's 'import' is missing." >&2; exit 1; }

# a virtual screen, started once and left running
export DISPLAY=${DISPLAY:-:99}
if ! xdpyinfo >/dev/null 2>&1; then
  command -v Xvfb >/dev/null || { echo "Xvfb is missing." >&2; exit 1; }
  rm -f "/tmp/.X${DISPLAY#:}-lock" "/tmp/.X11-unix/X${DISPLAY#:}"
  nohup Xvfb "$DISPLAY" -screen 0 1920x1080x24 >/dev/null 2>&1 &
  sleep 3
fi

# a throwaway config folder, so saved preferences never change the picture
cfg=${PUTIACNC_CONFIG_DIR:-$(mktemp -d)}
[ -n "$PUTIACNC_CONFIG_DIR" ] || trap 'rm -rf "$cfg"' EXIT

# shellcheck disable=SC2086
env PUTIACNC_CONFIG_DIR="$cfg" PUTIACNC_THEME="$theme" PUTIACNC_WIN="$win" $envs "$bin" $proj >/dev/null 2>&1 &
pid=$!
sleep "$wait"
if [ -n "$crop" ]; then
  IFS=, read -r x y w h <<EOF
$crop
EOF
  import -window root -crop "${w}x${h}+${x}+${y}" +repage "$out"
else
  import -window root "$out"
fi
kill $pid 2>/dev/null || true
wait $pid 2>/dev/null || true
echo "$out"
