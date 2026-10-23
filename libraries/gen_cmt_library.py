#!/usr/bin/env python3
"""Builds cmt-cnc-bits.json, a PutiaCNC tool library (File format "putia-cnc-tools") of CMT bits that suit a CNC router.

The dimensions were copied from the product tables on cmtorangetools.com (October 2026). The feeds and speeds are NOT
CMT's: they are conservative starting values worked out from the diameter, like the app's own starter tools.
Run:  python3 gen_cmt_library.py > cmt-cnc-bits.json
"""
import json, re, time
import sys as _sys
from fractions import Fraction

def inch(s):
    """'1-1/2' -> 1.5, '3/8' -> 0.375, '2' -> 2.0"""
    s = s.strip()
    if '-' in s and '/' in s:
        a, b = s.split('-'); return float(a) + float(Fraction(b))
    return float(Fraction(s))

# family: (title, shape, chip direction text, flutes per cutting zone, total flutes, material text, use text, rows)
# rows: (code, D, cutting length, overall length, shank [, extra])  -- inches as strings when the table is in inches
FAMILIES = []

def fam(**k): FAMILIES.append(k)

fam(key="191", title="upcut spiral", shape="flat", z=2, total=2, mat="solid carbide (HWM)", inch=True,
    use="Upcut: chips are ejected upward; a clean underside. Contour cutting, end trimming and panel sizing in solid wood, wood composites, plastics and laminates.",
    rows=[("191.001.11","1/8","1/2","2","1/4"),("191.003.11","5/32","1/2","2","1/4"),("191.005.11","3/16","3/4","2","1/4"),
          ("191.007.11","1/4","3/4","2","1/4"),("191.008.11","1/4","1","2-1/2","1/4"),("191.501.11","5/16","1","3","1/2"),
          ("191.503.11","3/8","1-1/4","3-1/4","1/2"),("191.505.11","1/2","1-1/4","3","1/2"),("191.506.11","1/2","1-1/2","3-1/2","1/2"),
          ("191.507.11","1/2","2","4","1/2")])
fam(key="192", title="downcut spiral", shape="flat", z=2, total=2, mat="solid carbide (HWM)", inch=True,
    use="Downcut: chips are pushed down; an excellent finish on the upper face. Cutting, copying and panel sizing in solid wood, composites, plastics and laminates.",
    rows=[("192.001.11","1/8","1/2","2","1/4"),("192.003.11","5/32","1/2","2","1/4"),("192.005.11","3/16","3/4","2","1/4"),
          ("192.007.11","1/4","3/4","2","1/4"),("192.008.11","1/4","1","2-1/2","1/4"),("192.501.11","5/16","1","3","1/2"),
          ("192.503.11","3/8","1-1/4","3-1/4","1/2"),("192.505.11","1/2","1-1/4","3","1/2"),("192.506.11","1/2","1-1/2","3-1/2","1/2"),
          ("192.507.11","1/2","2","4","1/2")])
fam(key="193", title="3-flute upcut spiral", shape="flat", z=3, total=3, mat="solid carbide (HWM)", inch=False,
    use="Three flutes, upcut: a clean underside at high feed. Contour cutting, end trimming and panel sizing in solid wood, composites, plastics and laminates.",
    rows=[("193.081.11",8,32,80,8),("193.100.11",10,32,80,10),("193.101.11",10,42,90,10),("193.120.11",12,35,83,12),("193.121.11",12,42,90,12),
          ("193.122.11",12,52,100,12),("193.140.11",14,58,110,14),("193.160.11",16,55,110,16),("193.161.11",16,35,90,16),("193.162.11",16,95,150,16),
          ("193.165.11",16,72,120,16),("193.180.11",18,55,110,18),("193.200.11",20,60,120,20),("193.201.11",20,70,120,20),("193.202.11",20,102,165,20)])
fam(key="198", title="single-flute upcut spiral", shape="flat", z=1, total=1, mat="solid carbide (HWM)", inch=True,
    use="One flute, upcut: big chip clearance for plastics and soft materials; a clean underside.",
    rows=[("198.001.11","1/8","1/2","2","1/4"),("198.005.11","3/16","5/8","2","1/4"),("198.007.11","1/4","3/4","2","1/4"),
          ("198.008.11","1/4","1","2-1/2","1/4"),("198.504.11","3/8","1-1/8","3","3/8")])
fam(key="198d", title="single-flute downcut spiral", shape="flat", z=1, total=1, mat="solid carbide (HWM)", inch=False,
    use="One flute, downcut: chips pushed down, an excellent finish on the upper face.",
    rows=[("198.660.11",6,27,60,6)])
fam(key="190", title="up & downcut compression spiral", shape="flat", z=None, total=None, mat="solid carbide (HWM)", inch=False,
    use="Compression (an upcut and a downcut section): a clean finish on both faces of plywood, MDF and laminated boards. Cut at least as deep as the downcut section so the full thickness is compressed.",
    rows=[("190.040.11",4,15,50,4,"1+1",7),("190.050.11",5,22,60,5,"1+1",8),("190.060.11",6,22,60,6,"1+1",8),
          ("190.008.11",6.35,22.6,63.5,6.35,"2+2",7),("190.080.11",8,32,80,8,"2+2",7),("190.513.11",9.52,22.2,76.2,9.53,"2+2",4.8),
          ("190.504.11",9.53,28.6,76.2,9.52,"2+2",7),("190.813.11",9.53,25.4,76.2,9.52,"3+3",5.2),("190.100.11",10,32,80,10,"2+2",7),
          ("190.101.11",10,42,90,10,"2+2",7),("190.120.11",12,42,90,12,"2+2",7),("190.121.11",12,52,100,12,"2+2",7),
          ("190.320.11",12,25.4,83,12,"3+3",5.2),("190.515.11",12.7,22.2,76.2,12.7,"2+2",5.2),("190.517.11",12.7,34.9,88.9,12.7,"2+2",5.2),
          ("190.505.11",12.7,25.4,76.2,12.7,"2+2",12),("190.506.11",12.7,28.6,76.2,12.7,"2+2",12),("190.507.11",12.7,34.9,88.9,12.7,"2+2",12),
          ("190.508.11",12.7,41.3,101.6,12.7,"2+2",12),("190.815.11",12.7,28.5,76.2,12.7,"3+3",6),("190.160.11",16,55,110,16,"2+2",24),
          ("190.180.11",18,55,110,18,"2+2",30)])
fam(key="195", title="3-flute upcut spiral with chipbreaker", shape="flat", z=3, total=3, mat="solid carbide (HWM)", inch=True,
    use="Three flutes with a chipbreaker (maximum tooth depth 0.3 mm), upcut: for roughing and fast contour cutting in solid wood, composites, plastics and laminates.",
    rows=[("195.506.11","1/2","1-1/2","3-1/2","1/2")])
fam(key="196", title="3-flute downcut spiral with chipbreaker", shape="flat", z=3, total=3, mat="solid carbide (HWM)", inch=True,
    use="Three flutes with a chipbreaker (maximum tooth depth 0.3 mm), downcut: a good upper face with fast contour cutting.",
    rows=[("196.506.11","1/2","1-1/2","3-1/2","1/2")])
fam(key="199", title="round nose upcut spiral", shape="ball", z=2, total=2, mat="solid carbide (HWM)", inch=False,
    use="Full-radius ball end, upcut: 3D relief, rounded pockets and fluting. Radius = half the diameter.",
    rows=[("199.001.11",3.18,12.7,50.8,6.35,1.6),("199.060.11",6,27,70,6,3),("199.008.11",6.35,25.4,63.5,6.35,3.2),("199.081.11",8,32,80,8,4),
          ("199.504.11",9.53,28.6,76,9.52,4.8),("199.100.11",10,32,80,10,5),("199.120.11",12,35,83,12,6),("199.505.11",12.7,31.8,76.2,12.7,6.4),
          ("199.509.11",15.88,57.15,109.5,15.87,7.9),("199.160.11",16,55,110,16,8),("199.511.11",19.05,57.15,109.5,19.05,9.5)])
fam(key="197", title="4-flute upcut spiral with chipbreaker", shape="flat", z=4, total=4, mat="solid carbide (HWM)", inch=False,
    use="Four flutes with a chipbreaker (maximum tooth depth 0.1 mm), upcut: fast roughing and contour cutting in solid wood, composites and plastics.",
    rows=[("197.121.11",12,42,90,12),("197.140.11",14,50,110,14),("197.160.11",16,55,110,16),("197.161.11",16,35,90,16),
          ("197.180.11",18,55,110,18),("197.200.11",20,60,120,20),("197.201.11",20,70,120,20)])
fam(key="188", title="single-flute spiral for aluminium", shape="flat", z=1, total=1, mat="5% cobalt HSS", inch=False, slow=True,
    use="One polished upcut flute with a positive rake: aluminium and soft non-ferrous metals, plastics. Maximum 12000 rpm per CMT. Use a lubricant or air blast.",
    rows=[("188.030.51",3,12,60,8),("188.040.51",4,12,60,8),("188.041.51",4,40,100,8),("188.050.51",5,14,60,8),("188.051.51",5,40,100,8),
          ("188.060.51",6,14,60,8),("188.061.51",6,40,100,8),("188.070.51",7,14,60,8),("188.080.51",8,14,80,8),("188.081.51",8,40,100,8),
          ("188.090.51",9,14,80,8),("188.100.51",10,14,80,8),("188.120.51",12,14,80,8)])
# 914 round nose (ball end) bits for a router: grouped by geometry, the shank variants go in the notes
RN = [("714.032.11",3.18,1.6,9.5,50.8,6),("814.032.11",3.18,1.6,9.5,50.8,6.35),("914.032.11",3.18,1.6,9.5,50.8,8),
      ("914.060.11",6,3,12.7,50.8,8),("714.060.11",6,3,12.7,50.8,6),("814.064.11",6.35,3.2,12.7,50.8,6.35),("814.564.11",6.35,3.2,15.9,63.5,12.7),
      ("814.595.11",9.53,4.8,25.4,66.7,12.7),("814.095.11",9.53,4.8,6.4,50.8,6.35),("714.095.11",9.53,4.8,6.4,50.8,6),("914.095.11",9.53,4.8,6.4,50.8,8),
      ("914.127.11",12.7,6.4,9.5,50.8,8),("914.627.11",12.7,6.4,31.8,73,12),("714.127.11",12.7,6.4,9.5,50.8,6),("814.127.11",12.7,6.4,9.5,50.8,6.35),
      ("814.627.11",12.7,6.4,31.8,73,12.7),("814.660.11",15.88,7.9,31.8,73,12.7),("814.160.11",15.88,7.9,9.5,50.8,6.35),("714.160.11",15.88,7.9,9.5,50.8,6),
      ("914.160.11",15.88,7.9,9.5,50.8,8),("914.190.11",19.05,9.5,11.3,50.8,8),("914.690.11",19.05,9.5,31.8,73,12),("714.190.11",19.05,9.5,11.3,50.8,6),
      ("814.190.11",19.05,9.5,11.3,50.8,6.35),("814.690.11",19.05,9.5,31.8,73,12.7),("814.721.11",22,11,25.4,63.5,12.7),("914.221.11",22,11,25.4,63.5,8),
      ("914.254.11",25.4,12.7,15.9,58.8,8),("914.754.11",25.4,12.7,31.8,73,12),("814.754.11",25.4,12.7,31.8,73,12.7),("814.817.11",31.75,15.9,19.1,58.8,12.7),
      ("914.817.11",31.75,15.9,19.1,58.8,12)]
# 815 V-grooving bits, 90 degrees (inch table) and the 47/64 pair
VG = [("815.064.11","1/4","5/16","1-1/2","1/4",90,"HWM"),("815.095.11","3/8","1/2","1-3/4","1/4",90,"HW"),("815.127.11","1/2","1/2","1-3/4","1/4",90,"HW"),
      ("815.660.11","5/8","1/2","2-1/2","1/2",90,"HW"),("815.690.11","3/4","5/8","2-1/2","1/2",90,"HW"),("815.817.11","1-1/4","3/4","2-1/2","1/2",90,"HW"),
      ("815.880.11","1-1/2","1-1/8","2-3/4","1/2",90,"HW"),("815.001.11","47/64","19/64","2-3/8","1/4",90,"HW"),("815.002.11","47/64","1/8","2-3/8","1/4",135,"HW")]

# The same bits sold with DLCS chrome coating (".41" instead of ".11"): identical geometry, so they are merged into
# the uncoated tool instead of becoming duplicates.
DLCS = {"192": ["192.007","192.008","192.503","192.505","192.506","192.507"],
        "190": ["190.080","190.504","190.513","190.813","190.100","190.101","190.120","190.121","190.320","190.505","190.506",
                "190.507","190.508","190.815","190.515","190.517"]}
DLCS_CODES = {c + ".11": c + ".41" for v in DLCS.values() for c in v}

MM = 25.4
def is_inch(d_mm):
    q = d_mm / MM * 64
    return abs(d_mm - round(d_mm)) > 1e-6 and abs(q - round(q)) < 0.02
def fmt(v):
    s = ("%.2f" % v).rstrip('0').rstrip('.')
    return s
def frac(d_mm):
    inch = d_mm / MM
    for den in (2,4,8,16,32,64):
        n = round(inch * den)
        if n >= 1 and abs(inch - n/den) < 0.0004:
            g = Fraction(n, den); w, r = divmod(g.numerator, g.denominator)
            if r == 0: return str(w)
            return ("%d-" % w if w else "") + "%d/%d" % (r, g.denominator)
    return fmt(d_mm)
def label(d_mm, sys): return (frac(d_mm) + "″") if sys == "in" else "Ø" + fmt(d_mm)

def speeds(d, zeff, shape, slow=False):
    rpm = 12000 if d <= 10 else (10000 if d <= 16 else 8000)
    if slow: rpm = min(rpm, 10000)
    k = 0.8 if shape in ("ball", "arc") else 1.0
    chip = 0.8 * k * min(max(0.012 * d, 0.004), 0.12) * (0.6 if slow else 1.0)
    feed = max(100.0, min(3000.0, round(rpm * zeff * chip / 10) * 10))
    plunge = max(60.0, round(feed * 0.3 / 10) * 10)
    pd = round(min(max(0.35 * d, 0.3), 8.0), 1)
    so = 15.0 if shape == "ball" else 40.0
    return rpm, feed, plunge, so, pd

tools, seq = [], [0]
def add(**t):
    seq[0] += 1
    t["id"] = "cmt%03d" % seq[0]
    t.setdefault("tip", 0.0)
    tools.append(t)

for f in FAMILIES:
    for r in f["rows"]:
        code = r[0]
        if f["inch"]:
            d, cl, ol, sh = (inch(x) * MM for x in r[1:5])
        else:
            d, cl, ol, sh = (float(x) for x in r[1:5])
        extra = ""
        if f["key"] == "190":
            zs = r[5]; a, b = (int(x) for x in zs.split("+")); zeff = a; total = a + b
            extra = " I1 (as listed by CMT) %s mm." % fmt(r[6])
            name_z = "Z" + zs
        else:
            zeff = f["z"]; total = f["total"]; name_z = "Z%d" % total
        if f["key"] == "199": extra = " Radius %s mm." % fmt(r[5])
        sys = "in" if (f["inch"] or is_inch(d)) else "mm"
        rpm, feed, plunge, so, pd = speeds(d, zeff, f["shape"], f.get("slow", False))
        if code in DLCS_CODES:
            extra += " Also sold with DLCS chrome coating for longer life: CMT %s (same dimensions)." % DLCS_CODES[code]
        note = ("CMT %s, %s, %s. Cutting length %s mm, overall %s mm, shank %s mm.%s %s Dimensions from CMT's website; feeds and speeds are conservative starting values, not CMT's."
                % (code, f["title"], f["mat"], fmt(cl), fmt(ol), fmt(sh), extra, f["use"]))
        short = {"up & downcut compression spiral": "compression spiral", "3-flute upcut spiral with chipbreaker": "upcut chipbreaker",
                 "3-flute downcut spiral with chipbreaker": "downcut chipbreaker", "4-flute upcut spiral with chipbreaker": "upcut chipbreaker",
                 "single-flute spiral for aluminium": "aluminium spiral", "single-flute downcut spiral": "downcut spiral",
                 "single-flute upcut spiral": "upcut spiral", "3-flute upcut spiral": "upcut spiral"}.get(f["title"], f["title"])
        add(name="%s %s %s (CMT %s)" % (label(d, sys), short, name_z, code), sys=sys, shape=f["shape"], angle=60.0, flutes=int(total),
            dia=round(d, 3), rpm=rpm, feed=feed, plunge=plunge, so=so, pd=pd, notes=note)

# round nose bits: one tool per geometry, listing every shank and code
groups = {}
for code, d, rad, cl, ol, sh in RN:
    groups.setdefault((d, rad, cl, ol), []).append((code, sh))
for (d, rad, cl, ol), v in groups.items():
    sys = "in" if is_inch(d) else "mm"
    rpm, feed, plunge, so, pd = speeds(d, 2, "ball")
    codes = ", ".join("%s (shank %s mm)" % (c, fmt(s)) for c, s in v)
    note = ("CMT round nose router bit, radius %s mm, 2 flutes. Cutting length %s mm, overall %s mm. Codes: %s. Engraving, fluting and rounded grooves in wood and wood products; use several passes in cove cuts. "
            "Dimensions from CMT's website; feeds and speeds are conservative starting values, not CMT's." % (fmt(rad), fmt(cl), fmt(ol), codes))
    add(name="%s round nose, R%s (CMT %s)" % (label(d, sys), fmt(rad), v[0][0]), sys=sys, shape="ball", angle=60.0, flutes=2, dia=round(d, 3), rpm=rpm, feed=feed, plunge=plunge, so=so, pd=pd, notes=note)

for code, d, cl, ol, sh, ang, mat in VG:
    d_mm = inch(d) * MM
    rpm, feed, plunge, so, pd = speeds(d_mm, 2, "vbit")
    plunge = min(plunge, 250.0); feed = min(feed, 1200.0)
    note = ("CMT %s V-grooving bit, %d°, carbide-tipped (%s), 2 flutes. Cutting length %s mm, overall %s mm, shank %s mm. Chamfers (45° each side), V-grooves, folding grooves and lettering. "
            "Dimensions from CMT's website; feeds and speeds are conservative starting values, not CMT's."
            % (code, ang, mat, fmt(inch(cl) * MM), fmt(inch(ol) * MM), fmt(inch(sh) * MM)))
    add(name="%d° V-bit %s (CMT %s)" % (ang, label(d_mm, "in"), code), sys="in", shape="vbit", angle=float(ang), flutes=2, dia=round(d_mm, 3), rpm=rpm, feed=feed, plunge=plunge, so=40.0, pd=2.0, notes=note)

# 663 V-groove / folding / signmaking cutters with replaceable knives (mm; D, cutting length I, overall L, angle A, shank 20, Z)
VG663 = [("663.103.11",34,40,115,45,1),("663.102.11",44,38,115,60,1),("663.101.11",52,25,100,91,1),("663.110.11",60,21,95,110,1),
         ("663.120.11",87,24,95,120,2),("663.130.11",91,20,95,130,2),("663.150.11",96.5,12.7,95,150,2)]
for code, d, cl, ol, ang, z in VG663:
    rpm = 12000 if d <= 40 else (10000 if d <= 55 else (8000 if d <= 65 else 6000))
    feed = 1000.0 if d <= 55 else 800.0
    note = ("CMT %s V-groove / folding / signmaking cutter with replaceable knives, %d\u00b0, %d flute%s. Cutting length %s mm, overall %s mm, shank 20 mm. Large cutter: "
            "check that your spindle and collet suit a 20 mm shank and keep the speed down. The page gives no units; read as millimetres. Dimensions from CMT's website; "
            "feeds and speeds are conservative starting values, not CMT's." % (code, ang, z, "" if z == 1 else "s", fmt(cl), fmt(ol)))
    add(name="%d\u00b0 V-groove \u00d8%s, %d fl (CMT %s)" % (ang, fmt(d), z, code), sys="mm", shape="vbit", angle=float(ang), flutes=z, dia=d, rpm=rpm, feed=feed,
        plunge=200.0, so=40.0, pd=2.0, notes=note)

# 663 spoilboard surfacing cutters with insert knives (mm; D, I, L, S). The page says Z3 or Z4 without saying which row is which.
SB = [("663.007.11",34.9,10,60,8),("663.015.11",38.1,12,60,12.7),("663.005.11",38.1,12,60,12),("663.004.11",60,12,80,12),
      ("663.014.11",60,12,80,12.7),("663.003.11",80,12,90,20),("663.006.11",100,12,90,20)]
sb = {}
for code, d, cl, ol, sh in SB: sb.setdefault((d, cl, ol), []).append((code, sh))
for (d, cl, ol), v in sb.items():
    rpm = 12000 if d <= 40 else (10000 if d <= 62 else 8000)
    sys_ = "in" if is_inch(d) else "mm"
    codes = ", ".join("%s (shank %s mm)" % (c, fmt(sh)) for c, sh in v)
    note = ("CMT spoilboard surfacing cutter with replaceable insert knives, 3 or 4 cutting edges (CMT does not say which per item; 3 assumed). Cutting length %s mm, overall %s mm. Codes: %s. "
            "Flattening spoilboards and slabs: take shallow passes with a stepover of about 70%% of the diameter. The page gives no units; read as millimetres. "
            "Dimensions from CMT's website; feeds and speeds are conservative starting values, not CMT's." % (fmt(cl), fmt(ol), codes))
    add(name="%s surfacing cutter, %s mm (CMT %s)" % (label(d, sys_), fmt(cl), v[0][0]), sys=sys_, shape="flat", angle=60.0, flutes=3, dia=d, rpm=rpm,
        feed=2000.0 if d < 50 else 1800.0, plunge=300.0, so=70.0, pd=1.0, notes=note)

# Contractor (hand-router) bits, 8 mm shank, carbide-tipped. Units are not stated on the page; read as millimetres.
NOTE_K = "Dimensions from CMT's website (the page states no units; read as millimetres); feeds and speeds are conservative starting values, not CMT's."
STRAIGHT = [("K911-030",3,8,51,1),("K911-040",4,11,51,1),("K911-050",5,12.7,51,1),("K911-060",6,16,51,1),("K912-060",6,25.4,57,2),
            ("K912-080",8,32,62,2),("K911-080",8,20,51,2),("K911-100",10,20,51,2),("K912-100",10,32,62,2),("K912-120",12,32,62,2),
            ("K911-120",12,20,51,2),("K911-140",14,25.4,56,2),("K911-150",15,25.4,56,2),("K911-160",16,25.4,56,2),("K911-180",18,25,56,2),
            ("K911-200",20,25,56,2),("K911-220",22,25.4,56,2),("K911-240",24,25.4,56,2),("K911-250",25,25.4,57,2)]
for code, d, cl, ol, z in STRAIGHT:
    rpm = 12000 if d <= 12 else (10000 if d <= 20 else 8000)
    rpm_, feed, plunge, so, pd = speeds(d, z, "flat")
    feed = min(feed, 1500.0)
    note = ("CMT %s straight router bit, carbide-tipped (HW), %d flute%s, 8 mm shank. Cutting length %s mm, overall %s mm. Contractor bit made for hand routers: fine for light "
            "CNC work in wood and wood composites, but not a solid-carbide spiral; use shallow passes. %s" % (code, z, "" if z == 1 else "s", fmt(cl), fmt(ol), NOTE_K))
    add(name="\u00d8%s straight Z%d, %s mm cut (CMT %s)" % (fmt(d), z, fmt(cl), code), sys="mm", shape="flat", angle=60.0, flutes=z, dia=d, rpm=rpm,
        feed=feed, plunge=plunge, so=so, pd=min(pd, cl * 0.5), notes=note)

for code, d, cl, ol, z in [("K914-060",6,9.5,40,2),("K914-095",9.5,9.5,40,2),("K914-127",12.7,12.7,40,2),("K914-160",16,12.7,45,2),("K914-190",19,12.7,46,2)]:
    rpm_, feed, plunge, so, pd = speeds(d, z, "ball")
    note = ("CMT %s round nose router bit, carbide-tipped, radius %s mm, 2 flutes, 8 mm shank. Cutting length %s mm, overall %s mm. Contractor bit made for hand routers. %s"
            % (code, fmt(d / 2), fmt(cl), fmt(ol), NOTE_K))
    add(name="\u00d8%s round nose, 8 mm shank (CMT %s)" % (fmt(d), code), sys="mm", shape="ball", angle=60.0, flutes=z, dia=d, rpm=min(rpm_, 12000),
        feed=min(feed, 1500.0), plunge=plunge, so=so, pd=pd, notes=note)

for code, d, cl, ol, ang in [("K958-110",11,14,45,60),("K915-127",12.7,12.7,45,90),("K915-160",16,16,45,90),("K915-317",31.8,20,60,90)]:
    d_mm = d
    sys_ = "in" if is_inch(d) else "mm"
    rpm_, feed, plunge, so, pd = speeds(d, 2, "vbit")
    note = ("CMT %s V-grooving router bit, carbide-tipped (HW), %d\u00b0, 2 flutes. Cutting length %s mm, overall %s mm. Contractor bit made for hand routers; the page lists no shank size. %s"
            % (code, ang, fmt(cl), fmt(ol), NOTE_K))
    add(name="%d\u00b0 V-bit %s (CMT %s)" % (ang, label(d, sys_), code), sys=sys_, shape="vbit", angle=float(ang), flutes=2, dia=d, rpm=min(rpm_, 12000),
        feed=min(feed, 1200.0), plunge=min(plunge, 250.0), so=40.0, pd=2.0, notes=note)

# Diablo (from a saved product page): DR20104, 1/2" 90 degree carbide V-groove bit, 2 cutters, max 24000 rpm, carbide height 7/16",
# overall length 1-3/4", shank 1/4". Lists "CNC Machine" among its tools.
d_mm = inch("1/2") * MM
rpm_, feed, plunge, so, pd = speeds(d_mm, 2, "vbit")
add(name="90\u00b0 V-bit 1/2\u2033, 7/16\u2033 cut (Diablo DR20104)", sys="in", shape="vbit", angle=90.0, flutes=2, dia=round(d_mm, 3), rpm=12000, feed=min(feed, 1200.0),
    plunge=min(plunge, 250.0), so=40.0, pd=2.0,
    notes=("Diablo DR20104 1/2\u2033 carbide V-groove bit, 90\u00b0 (45\u00b0 per side), 2 cutters, TiCo carbide with non-stick coating. Carbide height 7/16\u2033 (%s mm), overall length 1-3/4\u2033 (%s mm), "
           "shank 1/4\u2033. Diablo lists a maximum of 24000 rpm and CNC machines among its uses; for engraving, signmaking and veining. Very close to CMT 815.127.11 but with a slightly shorter cutting edge. "
           "Dimensions from Diablo's product page; feeds and speeds are conservative starting values, not Diablo's.") % (fmt(inch("7/16") * MM), fmt(inch("1-3/4") * MM)))

# Diablo straight bits, 2 cutters, TiCo carbide (D, carbide height, overall length, shank; inches). The diablotools.com product pages show no
# dimensions to a fetch tool, so these come from a retailer's listing (burnstools.com) of the same item numbers.
for code, D, H, L, S in [("DR04100","1/8","3/8","1-3/4","1/4"),("DR04108","1/4","1","2-1/2","1/4"),("DR04124","3/8","1","2-1/2","1/4"),
                         ("DR04132","1/2","1","2-1/8","1/4"),("DR12156","3/4","1-1/2","3","1/2")]:
    d_mm = inch(D) * MM
    rpm_, feed, plunge, so, pd = speeds(d_mm, 2, "flat")
    add(name="%s\u2033 straight Z2, %s\u2033 cut (Diablo %s)" % (D, H, code), sys="in", shape="flat", angle=60.0, flutes=2, dia=round(d_mm, 3),
        rpm=12000 if d_mm <= 12.7 else 10000, feed=min(feed, 1500.0), plunge=plunge, so=so, pd=min(pd, inch(H) * MM * 0.5),
        notes=("Diablo %s double-flute straight router bit, TiCo Hi-Density carbide, 2 cutters. Carbide height %s\u2033 (%s mm), overall length %s\u2033 (%s mm), shank %s\u2033 (%s mm). "
               "Hand-router bit: fine for light CNC work in wood and composites, with shallow passes. Dimensions from a retailer's listing of the Diablo item number; "
               "feeds and speeds are conservative starting values, not Diablo's.")
        % (code, H, fmt(inch(H) * MM), L, fmt(inch(L) * MM), S, fmt(inch(S) * MM)))

# Diablo round nose bits (retailer listing of the item numbers): D, R, carbide height, overall length, shank (inches), 2 flutes
for code, D, R, H, L, S in [("DR18106","3/8","3/16","3/8","1-13/16","1/4"),("DR18108","1/2","1/4","3/8","1-13/16","1/4"),("DR18112","3/4","3/8","7/16","2","1/4")]:
    d_mm = inch(D) * MM
    rpm_, feed, plunge, so, pd = speeds(d_mm, 2, "ball")
    add(name="%s\u2033 round nose, %s\u2033 cut (Diablo %s)" % (D, H, code), sys="in", shape="ball", angle=60.0, flutes=2, dia=round(d_mm, 3),
        rpm=12000 if d_mm <= 12.7 else 10000, feed=min(feed, 1500.0), plunge=plunge, so=so, pd=pd,
        notes=("Diablo %s round nose router bit, TiCo Hi-Density carbide, 2 flutes, radius %s\u2033 (%s mm). Carbide height %s\u2033 (%s mm), overall length %s\u2033 (%s mm), shank %s\u2033. "
               "Hand-router bit. Dimensions from a retailer's listing of the Diablo item number; feeds and speeds are conservative starting values, not Diablo's.")
        % (code, R, fmt(inch(R) * MM), H, fmt(inch(H) * MM), L, fmt(inch(L) * MM), S))

# remove duplicates: the same kind of bit with the same diameter, flutes, angle and cutting length is one tool
def cutlen(t):
    m = re.search(r"Cutting length ([0-9.]+) mm", t["notes"]) or re.search(r"Carbide height [^()]*\(([0-9.]+) mm\)", t["notes"])
    return m.group(1) if m else ""
def kind(t):
    if t["shape"] == "vbit": return "vbit"
    if t["shape"] == "ball": return "spiral round nose" if "spiral" in t["name"] else "router round nose"
    return kind_(t)
def kind_(t): return re.sub(r"^.*?\s(?=[a-zA-Z])", "", re.sub(r"\s*\(CMT.*$", "", t["name"]), count=1)
unique = []
for t in tools:
    k = (t["shape"], round(t["dia"], 1), t["flutes"], round(t["angle"], 1), kind(t))
    cl = float(cutlen(t) or 0)
    dup = next((u for u, uk, ucl in unique if uk == k and abs(ucl - cl) < 0.35), None)
    if dup is not None:
        codes = re.findall(r"(?:CMT|Diablo) ([A-Z]{1,2}\d{2,3}[.-]?\d{3}(?:\.\d{2})?|DR\d{5})", t["notes"])
        dup["notes"] += " Same bit also listed as %s." % ", ".join(codes)
        print("merged", t["name"], "->", dup["name"], file=_sys.stderr)
        continue
    unique.append((t, k, cl))
unique = [u[0] for u in unique]
tools[:] = unique
for i, t in enumerate(tools, 1): t["id"] = "cmt%03d" % i

out = {"app": "putia-cnc-tools", "v": 1, "name": "CNC router bits (CMT, Diablo)", "tools": tools}
json.dump(out, _sys.stdout, ensure_ascii=False, indent=1)
_sys.stdout.write("\n")
