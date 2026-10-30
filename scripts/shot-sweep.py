#!/usr/bin/env python3
"""Screenshots of many states, sizes and themes, with automatic checks (Linux; needs xvfb, imagemagick and python3-pil).

  scripts/shot-sweep.py                  # quick: light theme, two sizes, every state (about 5 minutes)
  scripts/shot-sweep.py --full           # both themes and three sizes (about 15 minutes)
  scripts/shot-sweep.py --update         # keep this run as the baseline (shot-sweep/baseline)
  scripts/shot-sweep.py --only 3d,gcode  # only the states whose name contains one of these words
  scripts/shot-sweep.py -b path/to/putiacnc

What it checks in every picture:
  * the window is drawn: not one flat colour, the side panels and the canvas are not empty;
  * nothing is painted outside the window, and the bottom bar is there;
  * egui's own error marker (a red 🔥 box around two widgets that share an ID) is not on screen;
  * against the baseline of an earlier run: the share of pixels that changed. Anything over 0.3 % is listed, with a picture of
    before | after | changes in shot-sweep/diff/ so you can look at it.
(The status bar is left out of that comparison: it says how many milliseconds the toolpath took.)
A change is not a failure of the program: it tells you where to look. Update the baseline when the new look is what you want.
"""
import argparse, os, shutil, subprocess, sys, time

try:
    from PIL import Image, ImageChops
except ImportError:
    sys.exit("python3-pil is missing (pip install pillow)")

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DATA = os.path.join(ROOT, "tests", "data")
OUT = os.path.join(ROOT, "shot-sweep")

# name, project, extra environment, seconds to wait
STATES = [
    ("design", "outside_holes", {}, 4),
    ("design-selected", "outside_holes", {"PUTIACNC_SELECT_ALL": "1"}, 4),
    ("design-ops-folded", "two_ops_toolchange", {"PUTIACNC_COLLAPSE": "1,2"}, 4),
    ("design-pocket-island", "pocket_island", {"PUTIACNC_OP": "1"}, 4),
    ("design-tabs", "tabs", {"PUTIACNC_TAB_EDIT": "1"}, 4),
    ("3d", "outside_holes", {"PUTIACNC_VIEW": "3d", "PUTIACNC_PLAY": "0.5"}, 7),
    ("3d-vbit", "vbit_profile", {"PUTIACNC_VIEW": "3d", "PUTIACNC_PLAY": "1"}, 7),
    ("3d-paused", "two_ops_toolchange", {"PUTIACNC_VIEW": "3d", "PUTIACNC_PLAY": "-1"}, 7),
    ("3d-holder-hit", "holder_clamp.json", {"PUTIACNC_VIEW": "3d", "PUTIACNC_PLAY": "0.24"}, 7),
    ("tab-gcode", "two_ops_toolchange", {"PUTIACNC_TAB_GCODE": "1"}, 5),
    ("tab-notes", "outside_holes", {"PUTIACNC_TAB_NOTES": "1"}, 4),
    ("banner-error", "outside_holes", {"PUTIACNC_SHOW_ERR": "1"}, 4),
    ("banner-warning", "outside_holes", {"PUTIACNC_SHOW_ERR": "warning"}, 4),
    ("dialog-machines", "outside_holes", {"PUTIACNC_SHOW_MACHINES": "1"}, 4),
    ("dialog-tools", "outside_holes", {"PUTIACNC_SHOW_TOOLS": "1"}, 4),
    ("dialog-shortcuts", "outside_holes", {"PUTIACNC_SHOW_SHORTCUTS": "1"}, 4),
    ("dialog-about", "outside_holes", {"PUTIACNC_SHOW_ABOUT": "1"}, 4),
    ("dialog-array", "outside_holes", {"PUTIACNC_SHOW_ARRAY": "1", "PUTIACNC_SELECT": "0"}, 4),
    ("dialog-combine", "outside_holes", {"PUTIACNC_SHOW_COMBINE": "1", "PUTIACNC_SELECT": "0,1"}, 4),
    ("dialog-corners", "outside_holes", {"PUTIACNC_SHOW_CORNERS": "1", "PUTIACNC_SELECT": "0"}, 4),
    ("dialog-offset", "outside_holes", {"PUTIACNC_SHOW_OFFSET": "1", "PUTIACNC_SELECT": "0"}, 4),
    ("dialog-clean", "outside_holes", {"PUTIACNC_SHOW_CLEAN": "1"}, 4),
    ("dialog-checks", "outside_holes", {"PUTIACNC_SHOW_CHECKS": "1"}, 6),
    ("hidden-panels", "outside_holes", {"PUTIACNC_HIDE": "left,right"}, 4),
]

QUICK_SIZES = ["1354x640", "1500x900"]
FULL_SIZES = ["1354x640", "1500x900", "1920x985"]


def window(path, size):
    w, h = map(int, size.split("x"))
    return Image.open(path).convert("RGB").crop((0, 0, w, h)), w, h


def flat(img, box):
    """Share of pixels in `box` that equal the most common colour."""
    c = img.crop(box)
    colours = c.getcolors(c.size[0] * c.size[1])
    return max(n for n, _ in colours) / (c.size[0] * c.size[1])


def problems(path, size, state):
    img, w, h = window(path, size)
    bad = []
    full = Image.open(path).convert("RGB")
    # outside the window: the virtual screen is black there. Inside, the bottom row is the status bar, never black.
    if flat(img, (0, 0, w, h)) > 0.97:
        bad.append("the window is one flat colour (the program did not draw, or was not up in time)")
        return bad
    bottom = img.crop((0, h - 24, w, h))
    if flat(bottom, (0, 0, w, 24)) > 0.995 and bottom.getpixel((w // 2, 12)) == (0, 0, 0):
        bad.append("the bottom bar is black (the window is smaller than asked, or not drawn)")
    left_panel = (8, 120, 380, h - 60)
    if "hidden-panels" not in state and flat(img, left_panel) > 0.985:
        bad.append("the left panel is empty")
    # the Notes tab of a project without notes is empty on purpose
    if "hidden-panels" not in state and "tab-notes" not in state:
        right_panel = (w - 420, 120, w - 20, h - 60)
        if flat(img, right_panel) > 0.985:
            bad.append("the right panel is empty")
    if "3d" not in state:
        canvas = (410, 100, w - 450, h - 70)
        if flat(img, canvas) > 0.998 and not state.startswith("dialog"):
            bad.append("the canvas is empty")
    # egui marks two widgets with the same ID by a red box and a "🔥" text; count strictly red pixels (pure error red)
    px = img.load()
    red = 0
    step = 1
    for y in range(0, h, step):
        for x in range(0, w, step):
            r, g, b = px[x, y]
            if r > 235 and g < 40 and b < 40:
                red += 1
    if red > 40 and "banner-error" not in state and "dialog-checks" not in state:
        bad.append(f"{red} pure-red pixels (egui's duplicate-ID marker is red)")
    if full.size[0] > w and full.crop((w, 0, full.size[0], min(h, full.size[1]))).getbbox() is not None:
        # anything non-black right of the window means the window is bigger than asked
        extra = full.crop((w + 2, 0, min(full.size[0], w + 40), min(h, full.size[1])))
        if extra.getbbox() is not None:
            bad.append("something is drawn to the right of the window")
    return bad


def changed_share(a, b):
    if a.size != b.size:
        return 1.0, None
    d = ImageChops.difference(a, b).convert("L").point(lambda v: 255 if v > 24 else 0)
    # the status bar says how many milliseconds the toolpath took: not part of the comparison
    d.paste(0, (0, a.size[1] - 26, a.size[0], a.size[1]))
    n = d.histogram()[255]
    return n / (a.size[0] * a.size[1]), d


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--only", default="")
    ap.add_argument("-b", "--binary", default="")
    ap.add_argument("--threshold", type=float, default=0.003)
    a = ap.parse_args()
    binary = a.binary or os.path.join(os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target")), "debug", "putiacnc")
    if not os.access(binary, os.X_OK):
        sys.exit(f"No program at {binary}. Build it first: cargo build -p putiacnc-gui")
    sizes = FULL_SIZES if a.full else QUICK_SIZES
    themes = ["light", "dark"] if a.full else ["light"]
    only = [w for w in a.only.split(",") if w]
    states = [s for s in STATES if not only or any(w in s[0] for w in only)]
    for d in ("out", "diff"):
        shutil.rmtree(os.path.join(OUT, d), ignore_errors=True)
        os.makedirs(os.path.join(OUT, d))
    base = os.path.join(OUT, "baseline")
    os.makedirs(base, exist_ok=True)
    total = len(states) * len(sizes) * len(themes)
    print(f"{total} pictures ({len(states)} states x {len(sizes)} sizes x {len(themes)} themes)")
    t0, k = time.time(), 0
    wrong, changed, new = [], [], []
    for theme in themes:
        for size in sizes:
            for name, proj, env, wait in states:
                k += 1
                tag = f"{name}_{size}_{theme}"
                out = os.path.join(OUT, "out", tag + ".png")
                cmd = ["sh", os.path.join(HERE, "shot.sh"), "-w", size, "-t", theme, "-s", str(wait), "-b", binary, "-o", out]
                for key, val in env.items():
                    cmd += ["-e", f"{key}={val}"]
                cmd.append(os.path.join(DATA, proj if proj.endswith(".json") else proj + ".project.json"))
                r = subprocess.run(cmd, capture_output=True, text=True)
                if r.returncode != 0 or not os.path.exists(out):
                    wrong.append((tag, [f"the screenshot failed: {r.stderr.strip()[:200]}"]))
                    continue
                bad = problems(out, size, name)
                img, w, h = window(out, size)
                img.save(out)  # keep only the window
                if bad:
                    wrong.append((tag, bad))
                bpath = os.path.join(base, tag + ".png")
                if os.path.exists(bpath):
                    old = Image.open(bpath).convert("RGB")
                    share, diff = changed_share(old, img)
                    if share > a.threshold:
                        changed.append((tag, share))
                        side = Image.new("RGB", (w * 3 + 20, h), (255, 0, 255))
                        side.paste(old, (0, 0)); side.paste(img, (w + 10, 0))
                        if diff is not None:
                            side.paste(diff.convert("RGB"), (2 * w + 20, 0))
                        side.save(os.path.join(OUT, "diff", tag + ".png"))
                else:
                    new.append(tag)
                if k % 10 == 0:
                    print(f"  {k}/{total} ({int(time.time() - t0)} s)", flush=True)
    if a.update:
        for f in os.listdir(os.path.join(OUT, "out")):
            shutil.copy(os.path.join(OUT, "out", f), os.path.join(base, f))
        print(f"baseline updated ({total} pictures)")
    print()
    if wrong:
        print(f"PROBLEMS in {len(wrong)} picture(s):")
        for tag, bad in wrong:
            print(f"  {tag}: " + "; ".join(bad))
    else:
        print("no problems found in the pictures themselves")
    if changed:
        print(f"\nCHANGED against the baseline ({len(changed)}), look at shot-sweep/diff/:")
        for tag, share in sorted(changed, key=lambda x: -x[1]):
            print(f"  {share * 100:5.1f} %  {tag}")
    elif not new:
        print("nothing changed against the baseline")
    if new and not a.update:
        print(f"\n{len(new)} picture(s) have no baseline yet (run with --update to keep this run as the baseline)")
    sys.exit(1 if wrong else 0)


if __name__ == "__main__":
    main()
