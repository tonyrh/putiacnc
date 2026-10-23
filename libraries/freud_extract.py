#!/usr/bin/env python3
"""Reads the router bit tables of the Freud General Purpose catalog (2020) into freud_catalog.json.
Usage: python3 freud_extract.py Freud_GPCatalog_2020_LOWRES.pdf     (PDF page = printed page + 5)
Only the families that suit a CNC router are read; bearing bits, trim bits, profile bits and sets are skipped."""
import json, re, subprocess, sys
from fractions import Fraction

def val(s):
    s = s.strip().replace("”", '"')
    if s.endswith("mm"): return float(s[:-2])
    s = s.rstrip('"').strip()
    m = re.fullmatch(r"(\d+)[- ](\d+/\d+)", s)
    if m: return (int(m.group(1)) + float(Fraction(m.group(2)))) * 25.4
    return float(Fraction(s)) * 25.4

V = r'(\d+(?:[- ]\d+/\d+|/\d+)?(?:mm|["”]))'
def page(p):
    return subprocess.run(["pdftotext", "-layout", "-f", str(p), "-l", str(p), sys.argv[1], "-"], capture_output=True, text=True).stdout

# (pdf pages, family, regex of the columns after the item number)
SPECS = [
 ((94, 95), "straight2", [V, V, V, V], ["D", "H", "S", "L"]),
 ((102, 103), "straight1", [V, V, V, V], ["D", "H", "S", "L"]),
 ((107, 108, 109), "spiral", [V, V, V, V], ["D", "H", "S", "L"]),
 ((110,), "oflute_spiral", [V, V, r"(\d)", V, V], ["D", "H", "N", "S", "L"]),
 ((111,), "oflute_straight", [V, V, r"(\d)", V, V], ["D", "H", "N", "S", "L"]),
 ((113, 114, 115), "compression", [V, V, V, V, V], ["D", "H", "S", "L", "U"]),
 ((116,), "mortise_comp", [V, V, r"(\d)", V, V, V], ["D", "H", "N", "S", "L", "U"]),
 ((128,), "roundnose", [V, V, V, V, V], ["D", "H", "R", "S", "L"]),
 ((132,), "vgroove", [V, V, r"(\d+)°", V, V], ["D", "H", "A", "S", "L"]),
 ((138,), "radiusv", [V, V, V, V, V], ["D", "H", "R", "S", "L"]),
 ((139,), "carving", [V, V, r"(\d+)°", V, V], ["D", "H", "A", "S", "L"]),
 ((141,), "balltip", [V, V, V, V, V], ["D", "H", "R", "S", "L"]),
]
out = []
for pages, fam, cols, names in SPECS:
    for p in pages:
        for line in page(p).splitlines():
            m = re.match(r"\s*(\d\d-\d{3})(\*{0,3})\s+(.*)", line)
            if not m: continue
            code, stars, rest = m.groups()
            pat = r"^" + r"\s+".join(cols)
            mm = re.match(pat, rest.replace("”", '"'))
            if not mm: continue
            row = {"code": code, "fam": fam, "page": p, "stars": stars}
            for n, g in zip(names, mm.groups()):
                row[n] = float(g) if n in ("N", "A") else round(val(g), 3)
            out.append(row)
# the V-groove table has rows with a top bearing (20-5xx) and a round nose table with top bearing (18-5xx): drop them
out = [r for r in out if not re.match(r"(20-5|18-5)", r["code"])]
json.dump(out, open("freud_catalog.json", "w"), indent=1)
from collections import Counter
print(len(out), Counter(r["fam"] for r in out), file=sys.stderr)
