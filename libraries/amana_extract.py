#!/usr/bin/env python3
"""Reads the solid-carbide spiral tables of Amana Tool's CNC booklet (PDF) into amana_booklet.json.

Only the families that suit a CNC router tool library are read: spiral plunge (up/down), compression spirals, ball nose spirals,
3-flute chipbreaker spirals, the single-flute 'O' flute plastic cutters, aluminium cutting spirals and end mills, foam cutting spirals and
2D/3D carving spirals (pages 8-13, 22, 23). Run:  python3 amana_extract.py Amana_Tool__CNC_Booklet.pdf
Needs pdftotext (poppler). Row values are inches or mm as printed; everything is converted to mm here.
"""
import json, re, subprocess, sys
PDF = sys.argv[1] if len(sys.argv) > 1 else "Amana_Tool__CNC_Booklet.pdf"

def page(p):
    return subprocess.run(["pdftotext", "-layout", "-f", str(p), "-l", str(p), PDF, "-"], capture_output=True, text=True).stdout.replace("\t", " ")

def dim(tok):
    t = tok.strip().replace("″", '"')
    if t in ("—", "-"): return None
    m = re.fullmatch(r"(\d+(?:\.\d+)?)mm", t)
    if m: return float(m.group(1))
    m = re.fullmatch(r"(\d+)-(\d+)/(\d+)\"?", t)
    if m: return (int(m.group(1)) + int(m.group(2)) / int(m.group(3))) * 25.4
    m = re.fullmatch(r"(\d+)/(\d+)\"?", t)
    if m: return int(m.group(1)) / int(m.group(2)) * 25.4
    m = re.fullmatch(r"(\d*\.\d+)\"?", t)
    if m: return float(m.group(1)) * 25.4
    m = re.fullmatch(r"(\d+)", t)
    if m and int(m.group(1)) <= 10: return int(m.group(1)) * 25.4
    return None

CODE = re.compile(r"(?<![\d.])([345]\d{4})((?:-[A-Z]{1,3}\d?)*)")
# (pdf page, what the section is). 'flutes' is the default when the table has no Flute column.
PAGES = {81: "spiral", 82: "spiral", 85: "spiral", 83: "compression", 84: "compression", 91: "spiral", 92: "spiral", 31: "plastic", 32: "plastic", 33: "plastic",
         8: "alu", 10: "alu", 11: "alu", 12: "alu", 13: "alu", 22: "carving", 23: "foam",
         18: "carving", 19: "carving", 70: "plunge", 71: "plunge", 72: "plunge", 73: "plunge", 74: "plunge",
         34: "plastic", 36: "plastic", 86: "compression", 87: "compression", 88: "compression", 89: "compression", 90: "spiral", 99: "surfacing"}
# pages whose table has no Flute column and no "• Single Flute" bullet: the section is single-flute 'O' flute
DEFAULT_Z = {8: 1, 34: 1, 71: 2}   # page 71 (45200 series) does not say; Amana lists these as 2-flute bits and the page note talks of "a two-flute bit"

def scan():
    rows = []
    for p, fam in PAGES.items():
        hdr = None; upx = dnx = None; sflute = DEFAULT_Z.get(p); title = ""
        last = None
        for ln in page(p).splitlines():
            if "HIGH SPEED STEEL" in ln: break            # the HSS tables below the carbide ones are not read
            s = ln.strip()
            m = re.search(r"•\s*(Single|\d)\s*(?:&\s*(\d)\s*)?Flute", s)
            if m: sflute = 1 if m.group(1) == "Single" else int(m.group(1))
            if re.match(r"^[A-Z][A-Z0-9 /&\-,'’‘\.()]+$", s) and len(s) > 10 and "TOOL" not in s: title = s
            if "Up-Cut" in ln and "Down-Cut" in ln: upx, dnx = ln.index("Up-Cut"), ln.index("Down-Cut")
            if re.search(r"Ø\s*D", ln) and "Tool No" in ln:
                hdr = []
                for t in ln.split():
                    if t in ("ØD", "B", "B1", "Ød", "L", "R"): hdr.append(t)
                    elif t == "a°": hdr.append("A")
                    elif "Flute" in t: hdr.append("Flute"); 
                continue
            if not hdr: continue
            toks = ln.split()
            # a code the PDF pushed onto the next line (the small pack icons break the row): attach it to the row above
            if last is not None and not any(dim(t) is not None and t not in ("2", "5") for t in toks[:3]):
                extra = [(m.start(), m.group(1), m.group(2)) for m in CODE.finditer(ln)]
                if extra and len(toks) <= 8:
                    for pos, c, suf in extra:
                        full = c + suf
                        if "LH" in suf or full in last["up"] + last["down"] + last["comp"]: continue
                        if last["fam"] == "compression": last["comp"].append(full)
                        elif upx is not None and dnx is not None: (last["up"] if abs(pos - upx) <= abs(pos - dnx) else last["down"]).append(full)
                        else: last["up"].append(full)
                    continue
            if len(toks) < len(hdr): continue
            v = {}; ok = True
            for k, t in zip(hdr, toks):
                if k == "Flute":
                    if not re.fullmatch(r"[1-4]", t): ok = False; break
                    v[k] = int(t)
                elif k == "A":
                    m = re.fullmatch(r"(\d+(?:\.\d+)?)\u00b0", t)
                    if not m: ok = False; break
                    v[k] = float(m.group(1))
                else:
                    d = dim(t)
                    if d is None and k != "B1": ok = False; break
                    v[k] = d
            if not ok: continue
            end = ln.find(toks[len(hdr) - 1]) + len(toks[len(hdr) - 1])
            codes = [(m.start(), m.group(1), m.group(2)) for m in CODE.finditer(ln, end)]
            if not codes: continue
            z = v.get("Flute", sflute)
            if z is None: continue
            up, down, comp = [], [], []
            for pos, c, suf in codes:
                full = c + suf
                if "LH" in suf or c == "45582": continue          # 45582 is a drawing label, not a product
                if fam == "compression": comp.append(full)
                elif fam == "plastic" and not (upx is not None and dnx is not None): up.append(full)
                elif upx is not None and dnx is not None:
                    (up if abs(pos - upx) <= abs(pos - dnx) else down).append(full)
                else: up.append(full)
            if not (up or down or comp): continue
            last = dict(page=p, fam=fam, title=title, D=v["ØD"], B=v["B"], B1=v.get("B1"), d=v["Ød"], L=v["L"], R=v.get("R"), A=v.get("A"), Z=z, up=up, down=down, comp=comp)
            rows.append(last)
    return rows

if __name__ == "__main__":
    rows = scan()
    json.dump(rows, open("amana_booklet.json", "w"), indent=0)
    from collections import Counter
    print(len(rows), Counter((r["page"], r["fam"]) for r in rows))
