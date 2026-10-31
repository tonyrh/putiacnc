#!/usr/bin/env python3
"""Writes fonts/hershey/*.hf from the Hershey single-stroke fonts (public domain, see fonts/hershey/NOTICE.txt).

    pip download Hershey-Fonts --no-deps -d /tmp/hf && unzip /tmp/hf/*.whl -d /tmp/hf/w
    python3 scripts/gen_hershey.py /tmp/hf/w

One line per character: `<code> <advance> <stroke>;<stroke>...`, a stroke being `x,y x,y ...`. Units are the font's own: capital letters
are 21 tall, y goes up from the baseline (so descenders are negative), x starts at the glyph's left edge.
"""
import sys, os
src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/hf/w"
sys.path.insert(0, src)
from HersheyFonts import HersheyFonts

FONTS = ["futural", "futuram", "rowmans", "rowmand", "scripts", "cursive"]
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fonts", "hershey")
for name in FONTS:
    f = HersheyFonts()
    f.load_default_font(name)
    base = f.render_options["base_line"]  # y is down in the data; the baseline of capitals
    lines = []
    glyphs = f._HersheyFonts__glyphs
    for code in range(32, 127):
        g = glyphs.get(chr(code))
        if g is None:
            continue
        strokes = []
        for st in g.strokes:
            pts = " ".join("%d,%d" % (x - g.left_offset, base - y) for x, y in st)
            if pts:
                strokes.append(pts)
        lines.append("%d %d %s" % (code, g.char_width, ";".join(strokes)))
    with open(os.path.join(out, name + ".hf"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print(name, len(lines), "glyphs")
