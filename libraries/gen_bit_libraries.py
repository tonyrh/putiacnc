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
BRAND = "cmt"
def add(**t):
    seq[0] += 1
    t["id"] = "cmt%03d" % seq[0]
    t.setdefault("tip", 0.0)
    t["brand"] = BRAND
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

BRAND = "diablo"
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

BRAND = "amana"
# Amana Tool metric bits. The category pages show no tables to a fetch tool, but each product page title carries all the dimensions
# ("57322 ... 5mm Dia x 30mm x 5mm Shank x 64mm Long ..."), so these come from titles returned by search. This is a PARTIAL sample, not the whole range.
# (code, kind, shape, D, cut, overall or None, shank, flutes, extra text)
AMANA = [("57300","'O' single-flute aluminium cutting upcut spiral","flat",2,6,64,6,1,"Aluminium: use a lubricant or air blast."),
         ("51498","'O' single-flute aluminium cutting upcut spiral","flat",8,25,64,8,1,"Aluminium: use a lubricant or air blast."),
         ("57311","'O' single-flute plastic cutting upcut spiral","flat",2,8,64,6,1,"For plastics and acrylic."),
         ("57322","'O' single-flute plastic cutting upcut spiral","flat",5,30,64,5,1,"For plastics and acrylic."),
         ("57327","'O' single-flute plastic cutting upcut spiral","flat",10,30,76,10,1,"For plastics and acrylic."),
         ("46453","solid carbide upcut spiral ball nose, radius 1.5 mm","ball",3,12,50,6,2,""),
         ("46454","solid carbide upcut spiral ball nose, radius 2 mm","ball",4,12,50,4,2,""),
         ("46456","solid carbide upcut spiral ball nose, radius 3 mm","ball",6,22,63,6,2,""),
         ("46458","solid carbide upcut spiral ball nose, radius 6 mm","ball",12,29,75,12,2,""),
         ("45213","carbide-tipped straight plunge","flat",6,25,None,6,2,"The title does not give the flute count; 2 assumed."),
         ("45215","carbide-tipped straight plunge","flat",8,25,None,8,2,"The title does not give the flute count; 2 assumed.")]
for code, what, shape, d, cl, ol, sh, z, extra in AMANA:
    rpm_, feed, plunge, so, pd = speeds(d, z, shape, slow=("aluminium" in what))
    if "plastic" in what: feed = min(feed, 1200.0)
    if shape == "flat" and z == 1 and d <= 3: pd = min(pd, 0.8)
    note = ("Amana Tool %s %s. Cutting length %s mm%s, shank %s mm. %s Dimensions from the product title on amanatool.com; feeds and speeds are conservative starting values, not Amana's."
            % (code, what, fmt(cl), (", overall %s mm" % fmt(ol)) if ol else "", fmt(sh), extra))
    nm = {"flat": "straight" if "straight" in what else ("aluminium spiral" if "aluminium" in what else "plastic spiral"), "ball": "ball nose"}[shape]
    add(name="\u00d8%s %s Z%d, %s mm cut (Amana %s)" % (fmt(d), nm, z, fmt(cl), code), sys="mm", shape=shape, angle=60.0, flutes=z, dia=d, rpm=min(rpm_, 12000),
        feed=min(feed, 1500.0), plunge=plunge, so=so, pd=pd, notes=note)
add(name="90\u00b0 V-bit \u00d812.7, 6 mm cut (Amana 45704M)", sys="mm", shape="vbit", angle=90.0, flutes=2, dia=12.7, rpm=12000, feed=1000.0, plunge=250.0, so=40.0, pd=2.0,
    notes="Amana Tool 45704M carbide-tipped V-groove bit, 90\u00b0, \u00d812.7 mm, cutting length 6 mm, shank 6 mm. The title does not give the flute count; 2 assumed. Dimensions from the product title on amanatool.com; feeds and speeds are conservative starting values, not Amana's.")
add(name="\u00d828.5 surfacing cutter, 12.5 mm (Amana 45523)", sys="mm", shape="flat", angle=60.0, flutes=2, dia=28.5, rpm=12000, feed=1800.0, plunge=300.0, so=70.0, pd=1.0,
    notes="Amana Tool 45523 carbide-tipped spoilboard surfacing / flycutter bit, \u00d828.5 mm, cutting length 12.5 mm, shank 6 mm. The title does not give the flute count; 2 assumed. Take shallow passes with about 70% stepover. Dimensions from the product title on amanatool.com; feeds and speeds are conservative starting values, not Amana's.")

# Amana point cutting roundover bits (the page you gave lists these): convex arc profile, arc radius R, diameter D, cutting length, shank.
# 56126 and 56121 are the same bit with a 1/4" or a 1/2" shank, so they are one tool. The titles give no flute count; 2 assumed.
PCR = [("56123-M","Carbide tipped","6","12","13",None,"6"),("56125-M","Solid carbide","3","6","8",None,"6"),
       ("56125","Solid carbide","1/8","1/4","5/16","in","1/4"),("56126","Carbide tipped","3/16","3/8","19/64","in","1/4"),
       ("56128","Carbide tipped","19/64","19/32","1/2","in","1/4"),("56142","Solid carbide","1/16","9/64","7/32","in","1/4"),
       ("56143","Solid carbide","3/32","13/64","1/4","in","1/4"),("56141","Solid carbide","1/32","5/64","3/16","in","1/4")]
for code, mat, R, D, H, unit, S in PCR:
    inch_ = unit == "in"
    r_mm, d_mm, h_mm, s_mm = [(inch(x) * MM if inch_ else float(x)) for x in (R, D, H, S)]
    rpm_, feed, plunge, so, pd = speeds(d_mm, 2, "arc")
    sys_ = "in" if inch_ else "mm"
    fmtv = (lambda v: frac(v) + "\u2033") if inch_ else (lambda v: "\u00d8" + fmt(v))
    extra = " Also made with a 1/2\u2033 shank as Amana 56121 (cutting length 5/16\u2033); not a separate tool." if code == "56126" else ""
    t = add(name="R%s point cutting roundover, %s (Amana %s)" % (fmt(r_mm) + " mm" if not inch_ else frac(r_mm) + "\u2033", fmtv(d_mm), code), sys=sys_, shape="arc", angle=60.0, flutes=2,
            dia=round(d_mm, 3), rpm=min(rpm_, 12000), feed=min(feed, 1200.0), plunge=min(plunge, 250.0), so=40.0, pd=max(0.3, min(pd, r_mm)),
            notes=("Amana Tool %s %s point cutting roundover, convex arc radius %s mm, diameter %s mm, cutting length %s mm, shank %s mm. For rounding edges and relief carving; "
                   "the title does not give the flute count (2 assumed).%s Dimensions from the product title on amanatool.com; feeds and speeds are conservative starting values, not Amana's.")
            % (code, mat.lower(), fmt(r_mm), fmt(d_mm), fmt(h_mm), fmt(s_mm), extra))
    tools[-1]["arc"] = round(r_mm, 3)

BRAND = "bosch"
# ---- Bosch Professional PRO router bits (product pages on bosch-professional.com, variant tables; all mm) ----------------------------------
# (article, D1, cutting length L, total length G, shank, flutes)
BOSCH_STRAIGHT = [("2608628438",3.2,7.7,51,6,1),("2608628439",4.8,12.4,51,6,1),
    ("2608628372",8,25,56,8,2),("2608628373",10,25,56,8,2),("2608628374",12,31.5,62,8,2),("2608628375",14,19.6,51,8,2),("2608628379",6,15.7,48,8,2),
    ("2608628381",8,19.6,51,8,2),("2608628383",10,19.6,51,8,2),("2608628385",12,19.6,51,8,2),("2608628387",15,19.6,51,8,2),("2608628388",16,19.6,51,8,2),
    ("2608628389",18,24.6,56,8,2),("2608628390",20,24.6,56,8,2),("2608628392",25,19.6,51,8,2),("2608628440",6,15.7,48,6,2),("2608628441",8,19.6,51,6,2),
    ("2608628442",9.5,19.6,51,6,2),("2608628443",12.7,19.6,51,6,2),("2608628444",19,19.6,51,6,2)]
BOSCH_VGROOVE = [("2608628406",11,13.7,45,8,60,2),("2608628407",16,15.7,45,8,90,2),("2608628445",6.4,9.2,45,6,90,1),("2608628446",9.5,12.4,45,6,90,2),("2608628447",12.7,12.4,45,6,90,2)]
# core box = round nose: (article, R1, D1, L, G, shank). The page does not give flutes; 2 assumed.
BOSCH_CORE = [("2608628368",6,12,9.2,40,8),("2608628370",10,20,12.4,46,8),("2608628450",6.3,12.7,9.2,40,6),("2608628451",4.7,9.5,9.2,40,6),("2608628452",8,15.9,12.4,45,6),
    ("2608628453",12.7,25.4,15.6,48,6),("2608628460",3.2,6.4,9.2,40,6),("2608628473",12,24,15.9,57,12),("2608628418",6.4,12.7,9.2,40,6.35),("2608628425",3.2,6.4,9.2,40,6.35),
    ("2608628632",4.8,9.5,9.2,40,6.35),("2608628633",7.9,15.9,12.4,45,6.35),("2608628634",12.7,25.4,15.6,48,6.35)]
NOTE_B = "Dimensions from Bosch's product page; feeds and speeds are conservative starting values, not Bosch's. Hand-router bit (Bosch lists up to 35000 rpm): fine for light CNC work."
_bg = {}
for art, d, cl, ol, sh, z in BOSCH_STRAIGHT: _bg.setdefault((d, cl, ol, z), []).append((art, sh))
for (d, cl, ol, z), v in _bg.items():
    rpm_, feed, plunge, so, pd = speeds(d, z, "flat")
    codes = ", ".join("%s (shank %s mm)" % (a, fmt(sh)) for a, sh in v)
    add(name="\u00d8%s straight Z%d, %s mm cut (Bosch %s)" % (fmt(d), z, fmt(cl), v[0][0]), sys="mm", shape="flat", angle=60.0, flutes=z, dia=d, rpm=min(rpm_, 12000),
        feed=min(feed, 1500.0), plunge=plunge, so=so, pd=min(pd, cl * 0.5),
        notes="Bosch PRO straight router bit, %d flute%s, carbide-tipped. Cutting length %s mm, overall %s mm. Articles: %s. %s" % (z, "" if z == 1 else "s", fmt(cl), fmt(ol), codes, NOTE_B))
for art, d, cl, ol, sh, ang, z in BOSCH_VGROOVE:
    rpm_, feed, plunge, so, pd = speeds(d, z, "vbit")
    add(name="%d\u00b0 V-bit \u00d8%s (Bosch %s)" % (ang, fmt(d), art), sys="mm", shape="vbit", angle=float(ang), flutes=z, dia=d, rpm=min(rpm_, 12000), feed=min(feed, 1200.0),
        plunge=min(plunge, 250.0), so=40.0, pd=2.0,
        notes="Bosch PRO V-groove bit, %d\u00b0, %d flute%s. Cutting length %s mm, overall %s mm, shank %s mm. Article %s. %s" % (ang, z, "" if z == 1 else "s", fmt(cl), fmt(ol), fmt(sh), art, NOTE_B))
_bc = {}
for art, r_, d, cl, ol, sh in BOSCH_CORE: _bc.setdefault((round(d, 1), cl, ol), []).append((art, sh, r_, d))
for (dk, cl, ol), v in _bc.items():
    d = v[0][3]; r_ = v[0][2]
    rpm_, feed, plunge, so, pd = speeds(d, 2, "ball")
    codes = ", ".join("%s (shank %s mm)" % (a, fmt(sh)) for a, sh, _, _ in v)
    add(name="\u00d8%s round nose (core box), %s mm cut (Bosch %s)" % (fmt(d), fmt(cl), v[0][0]), sys="mm", shape="ball", angle=60.0, flutes=2, dia=d, rpm=min(rpm_, 12000),
        feed=min(feed, 1500.0), plunge=plunge, so=so, pd=pd,
        notes="Bosch PRO core box (round nose) router bit, radius %s mm. Cutting length %s mm, overall %s mm. Articles: %s. The page does not give the flute count (2 assumed). %s" % (fmt(r_), fmt(cl), fmt(ol), codes, NOTE_B))

BRAND = "amana"
# ---- Amana Tool CNC booklet (rows read from the PDF by amana_extract.py) ---------------------------------------------------------------
import os
_rows = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "amana_booklet.json")))

def cutlen(t):
    m = re.search(r"Cutting length ([0-9.]+) mm", t["notes"]) or re.search(r"Carbide height [^()]*\(([0-9.]+) mm\)", t["notes"])
    return m.group(1) if m else ""
def _shank(t):
    m = re.search(r"shank ([0-9.]+) mm", t["notes"]); return float(m.group(1)) if m else None
def _direction(t):
    n = t["name"]
    return "down" if "downcut" in n else ("up" if ("upcut" in n or "aluminium" in n or "plastic" in n) else "")
def _class(t):
    n = t["name"]
    if "compression" in n or "Compression" in t["notes"][:80]: return "comp"
    if t["shape"] == "ball": return "ball"
    if t["shape"] == "flat" and ("spiral" in n or "chipbreaker" in n): return "spiral"
    return ""
def _same_geo(t, cls, D, B, d, Z, sys_flutes_ok=True):
    if _class(t) != cls or abs(t["dia"] - D) > 0.15 or t["flutes"] != Z: return False
    cl = cutlen(t)
    if not cl or abs(float(cl) - B) > 0.35: return False
    sh = _shank(t)
    return sh is None or abs(sh - d) <= 0.1
def _stripcode(c): return c.split("-")[0]

groups = {}
def _taper_ok(r):
    """Carving bits with a taper: the table's a\u00b0 is per side. Under 5\u00b0 per side the app's 10\u00b0 minimum included angle cannot hold it; a flat-bottom taper has no shape."""
    return r.get("A") is not None and r["A"] >= 5.0 and bool(r["R"])
for r in _rows:
    A = r.get("A")
    if not r.get("up") and not r.get("comp") and not r.get("down"): continue   # no tool number: a drawing label or a table I could not read
    if r["fam"] in ("compression", "spiral") and r["page"] in (89, 90) and r["R"]: continue   # corner-radius rows on the compression pages: shape not given
    if A is not None and 0.2 < A < 5.0: continue    # slight tapers (1 to 3.6 degrees per side) do not fit the app's tool shapes
    if A is not None and A >= 5.0 and not r["R"]: continue
    if r["fam"] == "carving" and A is not None and A >= 5.0: r = dict(r, taper=True)
    if r["D"] < 0.7 and not r.get("taper"): continue
    if r["D"] < 1.5 and not r.get("taper"): continue                       # micro bits are not worth a tool-library slot
    cls = "taper" if r.get("taper") else ("comp" if r["fam"] == "compression" else (("ball" if abs(r["R"] * 2 - r["D"]) < 0.05 else "bull") if r["R"] else "spiral"))
    key = (r["fam"], cls, round(r["D"], 2), round(r["B"], 1), round(r["d"], 1), round(r["L"], 0), r["Z"], round(r["R"] or 0, 2), round(r["B1"] or 0, 1), round(r.get("A") or 0, 1))
    g = groups.setdefault(key, dict(r=r, up=[], down=[], comp=[]))
    for k in ("up", "down", "comp"):
        for c in r[k]:
            if c not in g[k]: g[k].append(c)
n_amana = n_merged = 0
for key, g in groups.items():
    r = g["r"]; fam, cls = key[0], key[1]
    D, B, d, L, Z = r["D"], r["B"], r["d"], r["L"], r["Z"]
    allcodes = g["up"] + g["down"] + g["comp"]
    # already in the library by code (the title-derived Amana entries), or the same geometry from CMT: note the code there
    def known():
        for t in tools:
            if t["brand"] == "amana" and any(_stripcode(c) in t["notes"] for c in allcodes): return t
        return None
    def match(direction):
        for t in tools:
            if t["brand"] == "amana" and _same_geo(t, cls, D, B, d, Z) and (cls in ("comp", "ball") or _direction(t) in (direction, "")): return t
        return None
    kt = known()
    if kt is not None:
        n_merged += 1; continue
    pending = {"up": g["up"], "down": g["down"], "comp": g["comp"]}
    for dirn in ("up", "down", "comp"):
        if pending[dirn] and cls != "comp" or (cls == "comp" and dirn == "comp" and pending[dirn]):
            t = match("up" if dirn == "comp" else dirn)
            if t is not None and (cls != "comp" or True) and not (cls == "spiral" and fam == "plastic" and "plastic" not in t["name"] and "aluminium" not in t["name"] and False):
                t["notes"] += " Amana equivalent: %s (booklet page %d)." % (", ".join(pending[dirn]), r["page"]); pending[dirn] = []; n_merged += 1
    left = [k for k in ("up", "down", "comp") if pending[k]]
    if not left: continue
    codes = pending["up"] + pending["down"] + pending["comp"]
    inch_ = is_inch(D) and is_inch(d) if False else (abs(D / MM * 64 - round(D / MM * 64)) < 0.03 and abs(D - round(D)) > 1e-6)
    sys_ = "in" if inch_ else "mm"
    cutl = (frac(B) + "\u2033") if inch_ else fmt(B) + " mm"
    if cls == "comp": dword = "compression spiral"; shape = "flat"
    elif cls == "ball": dword = {"alu": "aluminium ball nose", "foam": "foam ball nose", "carving": "carving ball nose", "plastic": "plastic ball nose"}.get(fam, "ball nose"); shape = "ball"
    elif cls == "bull": dword = "corner-radius spiral"; shape = "arc"
    elif cls == "taper": dword = "tapered carving"; shape = "taper"
    else:
        dw = "up/downcut" if (pending["up"] and pending["down"]) else ("upcut" if pending["up"] else "downcut")
        if fam == "plunge":
            what = "straight plunge"; dw = ""
        elif fam == "surfacing":
            what = "surfacing"; dw = ""
        elif fam == "plastic" and r["page"] == 34:
            what = "plastic straight O-flute"; dw = ""
        elif fam == "plastic":
            what = "plastic O-flute" if Z == 1 else ("plastic double O-flute" if r["page"] == 31 else "plastic straight-grind")
        elif fam == "alu":
            what = "aluminium O-flute spiral" if Z == 1 else "aluminium spiral"
        elif fam == "foam": what = "foam cutting spiral"
        elif fam == "carving": what = "carving spiral"
        else:
            what = "chipbreaker spiral" if (r["page"] in (90, 91, 92) and Z == 3) else "spiral"
        dword = (dw + " " + what).strip(); shape = "flat"
    zeff = Z
    rpm_, feed, plunge, so, pd = speeds(D, zeff, "ball" if shape == "arc" else shape, slow=(fam in ("plastic", "alu")))
    if D <= 6.5 and fam not in ("plastic", "alu"): pd = min(pd, D * 0.5)
    first = codes[0]
    code_list = []
    if pending["up"]: code_list.append("up " + ", ".join(pending["up"]))
    if pending["down"]: code_list.append("down " + ", ".join(pending["down"]))
    if pending["comp"]: code_list.append(", ".join(pending["comp"]))
    mat = "carbide tipped" if ((fam == "plunge" and r["page"] in (70, 71, 73)) or fam == "surfacing") else "solid carbide"
    note = (("Amana Tool %s %s, " + mat + ", %d flute%s. Cutting length %s mm, overall %s mm, shank %s mm.%s Codes: %s%s. "
            "Dimensions from Amana's CNC booklet (page %d); feeds and speeds are conservative starting values, not Amana's. A -K suffix is the Spektra-coated version.")
            % (first, dword, Z, "" if Z == 1 else "s", fmt(B), fmt(L), fmt(d),
               ((" Compression section B1 %s mm." % fmt(r["B1"])) if (cls == "comp" and r["B1"]) else "") + (" The booklet page does not state the flute count; 2 assumed." if r["page"] == 71 else "") + (" Hand-router style bit: shallow passes." if fam == "plunge" else ""),
               "; ".join(code_list), ("; radius %s mm" % fmt(r["R"])) if r["R"] else "", r["page"]))
    short = lambda w: w.replace("chipbreaker spiral", "chipbreaker").replace("plastic straight-grind", "plastic").replace("plastic double O-flute", "plastic dbl O").replace("up/downcut", "up/down").replace("aluminium O-flute spiral", "alu O-flute").replace("aluminium spiral", "alu spiral").replace("aluminium ball nose", "alu ball nose").replace("foam cutting spiral", "foam spiral").replace("corner-radius spiral", "corner radius")
    base = "%s %s Z%d, %s" % (label(D, sys_), dword, Z, cutl)
    if len(base) + 14 > 60: base = "%s %s Z%d, %s" % (label(D, sys_), short(dword), Z, cutl)
    if len(base) + 4 + 14 <= 60: base += " cut"
    add(name=base + " (Amana %s)" % first.split("-")[0],
        sys=sys_, shape=shape, angle=60.0, flutes=Z, dia=round(D, 3), rpm=min(rpm_, 12000), feed=min(feed, 2000.0), plunge=plunge, so=so, pd=pd, notes=note + (" Cutting aluminium: use a lubricant or an air blast, and light passes." if fam == "alu" else ""))
    if shape == "arc": tools[-1]["arc"] = round(r["R"], 3)
    if shape == "taper":
        # the table gives the ball (tip) diameter D and the angle per side: the widest point is D + 2 B tan(a)
        import math
        t = tools[-1]; a = r["A"]
        t["dia"] = round(D + 2 * B * math.tan(math.radians(a)), 3); t["angle"] = round(2 * a, 2); t["tip"] = round(r["R"], 3)
        t["feed"] = min(t["feed"], 900.0); t["pd"] = min(t["pd"], 1.0); t["so"] = 15.0
        t["notes"] += " Tapered ball nose: ball diameter %s mm, %s\u00b0 per side (%s\u00b0 included); the diameter shown is the widest point of the cutting length, worked out from the table." % (fmt(D), fmt(a), fmt(2 * a))
    n_amana += 1
print("amana: %d new tools, %d rows merged into existing tools" % (n_amana, n_merged), file=_sys.stderr)

# remove duplicates: the same kind of bit with the same diameter, flutes, angle and cutting length is one tool
def cutlen(t):
    m = re.search(r"Cutting length ([0-9.]+) mm", t["notes"]) or re.search(r"Carbide height [^()]*\(([0-9.]+) mm\)", t["notes"])
    return m.group(1) if m else ""
def kind(t):
    n = re.sub(r"\s*\((?:CMT|Diablo|Amana|Bosch|Freud|Whiteside)[^)]*\)$", "", t["name"])
    base = re.sub(r"^.*?\s(?=[a-zA-Z])", "", n, count=1)
    base = re.split(r",| Z\d| \u00d8", base)[0].strip()
    if t["shape"] == "vbit": return "vbit"
    if t["shape"] == "ball": return "spiral round nose" if "spiral" in n else "router round nose"
    if "chipbreaker" in n: return "chipbreaker " + re.sub(r"\d+", "", base)
    if "plastic" in n: return "plastic " + ("straight-grind" if ("straight" in n or "double" not in n and "O-flute" not in n) else "O-flute") + (" double" if "double" in n else "")
    if "straight" in n: return "straight"
    if "surfacing" in n: return "surfacing"
    if "compression" in n: return "compression"
    return re.sub(r"\d+", "", base)
BRAND = "freud"
# ---- Freud General Purpose catalog 2020 (rows read from the PDF by freud_extract.py; the CNC-suitable families only) ------------------------
_fr = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "freud_catalog.json")))
def _fin(mm): return frac(mm) + "\u2033"
def _fnote(r, what, extra=""):
    return ("Freud %s %s, carbide%s. Cutting length %s mm (%s), overall length %s mm, shank %s mm. %sFrom Freud's 2020 General Purpose catalog; feeds and speeds are "
            "conservative starting values, not Freud's.") % (r["code"], what, " (solid carbide)" if r["stars"] else " tipped", fmt(r["H"]), _fin(r["H"]), fmt(r["L"]), fmt(r["S"]), extra)
for r in _fr:
    f, D, code = r["fam"], r["D"], r["code"]
    if D < 1.5 or r["stars"] == "**": continue          # ** = made for the Porter-Cable dovetail jig
    sfx = " (Freud %s)" % code
    sys_ = "mm" if abs(D * 10 - round(D * 10)) < 1e-6 and not is_inch(D) else "in"
    lab = label(D, sys_) if sys_ == "in" else "\u00d8%s mm" % fmt(D)
    if f in ("straight2", "straight1"):
        z = 2 if f == "straight2" else 1
        if code == "04-144": continue                   # the table shows 1-13/16 in for this one, which is probably a misprint
        shape, nm, what = "flat", "%s straight Z%d, %s cut" % (lab, z, _fin(r["H"])), "%s-flute straight router bit" % ("double" if z == 2 else "single")
        extra = "Undersized for plywood. " if code in ("04-111", "04-127", "04-128", "12-114", "12-135", "12-150") or "undersized" in code else ""
        if code in ("04-111", "04-127", "04-128", "12-114", "12-135", "12-150"): extra = "Undersized plywood bit (cuts a slot slightly narrower than the nominal diameter). "
        rpm_, feed, plunge, so, pd = speeds(D, z, shape, slow=(z == 1))
        add(name=nm + sfx, sys=sys_, shape=shape, angle=60.0, flutes=z, dia=round(D, 3), rpm=min(rpm_, 12000 if D <= 25 else 10000), feed=min(feed, 1800.0), plunge=plunge, so=so, pd=min(pd, r["H"] * 0.5),
            notes=_fnote(r, what, extra + ("Single flute: more chip room for fast feeds. " if z == 1 else "") + "A hand-router bit; fine for light CNC work with shallow passes. "))
    elif f == "spiral":
        up = code.startswith("75"); sub = int(code[3:]); z = 3 if sub // 100 in (3, 6) else 2; cb = sub // 100 in (5, 6)
        rpm_, feed, plunge, so, pd = speeds(D, z, "flat")
        add(name="%s %s spiral Z%d%s%s" % (lab, "upcut" if up else "downcut", z, " chipbreaker" if cb else "", sfx), sys=sys_, shape="flat", angle=60.0, flutes=z, dia=round(D, 3), rpm=min(rpm_, 12000), feed=min(feed, 2500.0),
            plunge=plunge, so=so, pd=min(pd, r["H"] * 0.5),
            notes=_fnote(r, "premier solid carbide %s spiral" % ("up" if up else "down"), ("Chipbreaker flutes: leave a rougher but faster cut in thick stock. " if cb else "") +
                         ("Up spiral clears chips out of the cut; use it for slots and pockets. " if up else "Down spiral presses chips into the cut for a clean top edge on veneer and laminate. ")))
    elif f in ("oflute_spiral", "oflute_straight"):
        z = int(r["N"]); up = code.startswith("73") or code == "75-025"; down = code.startswith("74")
        rpm_, feed, plunge, so, pd = speeds(D, z, "flat", slow=True)
        add(name="%s O-flute %s%s%s" % (lab, "downcut " if down else ("upcut " if f == "oflute_spiral" else ""), "plastic cutter Z%d" % z, sfx), sys=sys_, shape="flat", angle=60.0, flutes=z, dia=round(D, 3), rpm=min(rpm_, 12000),
            feed=min(feed, 1500.0), plunge=plunge, so=so, pd=min(pd, r["H"] * 0.5),
            notes=_fnote(r, "premier solid carbide O-flute %s" % ("straight bit" if f == "oflute_straight" else ("down spiral" if down else "up spiral")), "O-flute geometry with a polished edge for plastics, acrylic and solid surface; also cuts wood and aluminium with lubricant. "))
    elif f in ("compression", "mortise_comp"):
        z = int(r["N"]) if f == "mortise_comp" else {1: 1, 2: 2, 3: 3, 7: 4}[int(code[3])]
        if f == "compression" and code == "77-610": continue
        rpm_, feed, plunge, so, pd = speeds(D, max(z, 2), "flat")
        add(name="%s compression Z%d%s" % (lab, z, sfx), sys=sys_, shape="flat", angle=60.0, flutes=z, dia=round(D, 3), rpm=min(rpm_, 12000), feed=min(feed, 2500.0), plunge=plunge, so=so, pd=min(pd, r["H"] * 0.5),
            notes=_fnote(r, "premier solid carbide %scompression spiral" % ("mortise " if f == "mortise_comp" else ""), "Upcut section %s mm (%s) at the tip, downcut above it: clean top and bottom faces on plywood, laminate and solid wood; cut deeper than the upcut section. " % (fmt(r["U"]), _fin(r["U"]))))
    elif f == "roundnose":
        R = r["R"]; rpm_, feed, plunge, so, pd = speeds(D, 2, "ball")
        shp = "ball" if abs(R * 2 - D) < 0.02 else "arc"
        if shp == "arc": continue
        add(name="%s round nose Z2, R%s%s" % (lab, fmt(R), sfx), sys=sys_, shape="ball", angle=60.0, flutes=2, dia=round(D, 3), rpm=min(rpm_, 12000 if D <= 25 else 10000), feed=min(feed, 1500.0), plunge=plunge, so=so, pd=min(pd, R),
            notes=_fnote(r, "round nose router bit (radius %s mm)" % fmt(R), "Grooves, fluting and cove cuts. Hand-router bit. "))
    elif f == "vgroove":
        ang = r["A"]; rpm_, feed, plunge, so, pd = speeds(D, 2, "vbit")
        add(name="%d\u00b0 V-groove %s%s" % (ang, lab, sfx), sys=sys_, shape="vbit", angle=float(ang), flutes=2, dia=round(D, 3), rpm=min(rpm_, 12000 if D <= 25 else 10000), feed=min(feed, 1200.0), plunge=min(plunge, 250.0), so=40.0, pd=2.0,
            notes=_fnote(r, "%d\u00b0 V-grooving bit" % ang, "V-grooves, chamfers and sign lettering. 2 cutters assumed. "))
    elif f == "radiusv":
        R = r["R"]; rpm_, feed, plunge, so, pd = speeds(D, 2, "ball")
        add(name="%s radius V-groove Z2, R%s%s" % (lab, fmt(R), sfx), sys=sys_, shape="ball", angle=60.0, flutes=2, dia=round(D, 3), rpm=min(rpm_, 12000), feed=min(feed, 1500.0), plunge=plunge, so=so, pd=min(pd, R),
            notes=_fnote(r, "radius V-grooving (core box) bit, radius %s mm" % fmt(R), "Rounded grooves for lettering, fluting and signs. 2 cutters assumed. "))
    elif f == "carving":
        if code.startswith("72"): continue              # tapered ball tip: the table gives the tip diameter, which the tool form can't take as it stands
        ang = r["A"]; rpm_, feed, plunge, so, pd = speeds(D, 2, "vbit")
        add(name="%d\u00b0 detail carving %s%s" % (ang, lab, sfx), sys=sys_, shape="vbit", angle=float(ang), flutes=2, dia=round(D, 3), rpm=12000, feed=min(feed, 900.0), plunge=min(plunge, 200.0), so=30.0, pd=1.0,
            notes=_fnote(r, "%d\u00b0 detail carving bit" % ang, "Fine V-carving and lettering details (the angle is the included angle, checked against the diameter and height). 2 cutters assumed. "))
    elif f == "balltip":
        R = r["R"]; rpm_, feed, plunge, so, pd = speeds(D, 2, "ball")
        add(name="%s up-cut ball nose Z2%s" % (lab, sfx), sys=sys_, shape="ball", angle=60.0, flutes=2, dia=round(D, 3), rpm=min(rpm_, 12000), feed=min(feed, 1500.0), plunge=plunge, so=15.0, pd=min(pd, R),
            notes=_fnote(r, "up-cut spiral ball nose bit (radius %s mm)" % fmt(R), "3D finishing and rounded pockets. 2 flutes assumed. "))
# two Freud rows are listed without a diameter-matching table and are left out: the single 1/8 in radius-tip bit 72-210 and the 72-100 (no radius given)

BRAND = "whiteside"
# ---- Whiteside "CNC router bits" brochure (1-14-19), all inches, 1/4 in shank unless noted ----------------------------------------------------
def _w(code, name, shape, D, cl, ol, shank="1/4", z=2, what="", extra="", angle=60.0, tip=0.0, arc=0.0, slow=False, feed_cap=1800.0, pd_cap=None, so=None):
    d = inch(D) * MM; c = inch(cl) * MM
    rpm_, feed, plunge, so_, pd = speeds(d, z, "ball" if shape in ("ball", "arc", "taper") else ("vbit" if shape in ("vbit", "engrave") else "flat"), slow)
    if shape in ("vbit", "engrave"): feed_cap = min(feed_cap, 1000.0); pd = min(pd, 1.5)
    if pd_cap: pd = min(pd, pd_cap)
    add(name="%s (Whiteside %s)" % (name, code), sys="in", shape=shape, angle=float(angle), flutes=z, dia=round(d, 3), rpm=min(rpm_, 12000 if d <= 25.5 else 10000), feed=min(feed, feed_cap),
        plunge=plunge, so=so if so else so_, pd=round(min(pd, max(c * 0.5, 0.3)), 2),
        notes=("Whiteside %s %s. Cutting length %s mm (%s\u2033), overall length %s mm (%s\u2033), shank %s\u2033. %sFrom Whiteside's CNC router bit brochure; feeds and speeds are "
               "conservative starting values, not Whiteside's.") % (code, what, fmt(c), cl, fmt(inch(ol) * MM), ol, shank, extra))
    if tip: tools[-1]["tip"] = round(tip, 3)
    if arc: tools[-1]["arc"] = round(arc, 3)
FL = "Number of flutes not given in the brochure; 2 assumed. "
for code, D, cl, ol in [("RU1600","1/8","1/2","2"),("RU1800","3/16","3/4","2-1/2"),("RU2075","1/4","3/4","2-1/2")]:
    _w(code, "%s\u2033 upcut spiral Z2" % frac(inch(D) * MM), "flat", D, cl, ol, what="solid carbide up cut spiral", extra="Chips are ejected up and out of the cut; best at plunging. " + FL)
for code, D, cl, ol in [("RD1600","1/8","1/2","2"),("RD1800","3/16","3/4","2-1/2"),("RD2075","1/4","3/4","2-1/2")]:
    _w(code, "%s\u2033 downcut spiral Z2" % frac(inch(D) * MM), "flat", D, cl, ol, what="solid carbide down cut spiral", extra="Chips are pushed down; controls fraying on the top surface. " + FL)
for code, R, D, cl in [("RU1800RN","3/32","3/16","3/4"),("RU2075RN","1/8","1/4","3/4")]:
    _w(code, "%s\u2033 ball nose spiral Z2" % frac(inch(D) * MM), "ball", D, cl, "2-1/2", what="solid carbide ball nose up cut spiral (radius %s\u2033)" % R, extra=FL, feed_cap=1500.0)
_w("RU2075CN", "1/4\u2033 cove nose spiral Z2, R1/16", "arc", "1/4", "3/4", "2-1/2", what="solid carbide cove nose spiral (corner radius 1/16\u2033)", extra="Flat bottom with rounded corners, for U-shaped grooves. " + FL, arc=inch("1/16") * MM, feed_cap=1500.0)
for code, D, cl, ol in [("SC062","1/16","3/16","1-1/2"),("SC33","1/8","1/4","2")]:
    _w(code, "%s\u2033 flat bottom veining" % frac(inch(D) * MM), "flat", D, cl, ol, what="flat bottom veining bit", extra="Fine flat-bottomed grooves for lettering and signs. " + FL, feed_cap=900.0)
for code, R, D, cl, ol in [("1370","1/8","7/16","5/16","2-1/8"),("1372","1/4","3/4","5/8","2-3/8")]:
    _w(code, "%s\u2033 bowl & tray bit, R%s" % (frac(inch(D) * MM), R), "arc", D, cl, ol, what="bowl and tray bit (bottom radius %s\u2033)" % R, extra="Flat bottom with a large corner radius, for bowls, trays and dished pockets. " + FL, arc=inch(R) * MM, feed_cap=1500.0)
for code, R, D, cl, ol, sh in [("SC39","1/16","1/8","1/4","2","1/4"),("SC40","3/32","3/16","3/8","2","1/4"),("SC41","1/8","1/4","1/2","2","1/4"),("1403","3/16","3/8","1/2","2","1/4"),
                               ("1404","1/4","1/2","5/8","2-1/4","1/4"),("1405","5/16","5/8","3/8","2","1/4"),("1406","3/8","3/4","7/16","2","1/4")]:
    _w(code, "%s\u2033 round nose, R%s" % (frac(inch(D) * MM), R), "ball", D, cl, ol, what="round nose bit (radius %s\u2033)" % R, extra=FL, feed_cap=1500.0)
for code, D, pl, ol, ang in [("1500","1/4","1/8","1-1/2",90),("1502","1/2","1/4","2",90),("1541","1/4","7/32","2",60),("1550","1/2","7/16","2",60)]:
    _w(code, "%d\u00b0 V-groove %s\u2033" % (ang, frac(inch(D) * MM)), "vbit", D, pl, ol, what="%d\u00b0 vee groove bit" % ang, extra="Point length %s\u2033 (the cutting depth). V-grooves, chamfers and lettering. " % pl + FL, angle=ang)
for code, ang, tipd, label_ in [("SC70", 30, 0, "pencil point"), ("SC71", 30, "1/32", "1/32\u2033 flat point"), ("SC75", 45, 0, "pencil point"), ("SC76", 45, "1/32", "1/32\u2033 flat point"), ("SC54", 15, "1/16", "1/16\u2033 flat point")]:
    t = (inch(tipd) * MM / 2) if tipd else 0.0
    _w(code, "%d\u00b0 engraving, %s" % (ang, label_), "engrave", "1/4", "1/4", "2" if ang != 15 else "2-1/2", what="%d\u00b0 included-angle engraving bit, %s" % (ang, label_),
       extra="The brochure gives no cutting diameter or length; 1/4\u2033 (the shank size) is assumed for both. Flat points are treated as a tip radius. " + FL, angle=ang, tip=t, feed_cap=700.0, pd_cap=0.5)
_w("SC50", "22\u00b0 carving liner", "vbit", "0.243", "5/8", "2-1/2", what="carving liner, 22\u00b0 included angle, pencil point",
   extra="Point length 5/8\u2033; the diameter (0.243\u2033) is worked out from the angle and point length, as the brochure does not give it. " + FL, angle=22.0, feed_cap=700.0, pd_cap=0.5)
_w("SC64", "11\u00b0 conical ball nose, 1/16\u2033 ball", "taper", "1/4", "1/4", "2-1/2", what="conical ball nose spiral, 11\u00b0 included angle, ball diameter 1/16\u2033 (tip radius 1/32\u2033)",
   extra="The brochure gives no cutting diameter or length; 1/4\u2033 is assumed for both. " + FL, angle=11.0, tip=inch("1/32") * MM, feed_cap=700.0, pd_cap=0.5)
for code, D, cl, sh, z in [("6210","1","1/4","1/4",3),("6220","2","1/4","1/2",4)]:
    _w(code, "%s\u2033 spoilboard cutter, %d wings" % (frac(inch(D) * MM), z), "flat", D, cl, cl, shank=sh, z=z, what="carbide-tipped spoilboard cutter, %d wings" % z, extra="Overall length is not given; the cutting length is shown. For flattening the spoilboard. ", so=70.0, feed_cap=1800.0)

unique = []
for t in tools:
    tipped = bool(re.search(r"carbide[- ]tipped|\(HW\)", t["notes"], re.I))     # a carbide-tipped bit and a solid carbide one are different tools
    k = (t["brand"], t["shape"], round(t["dia"], 1), t["flutes"], round(t["angle"], 1), kind(t), round(t.get("tip", 0.0), 2), tipped)
    cl = float(cutlen(t) or 0)
    dup = next((u for u, uk, ucl in unique if uk == k and abs(ucl - cl) < 0.5), None)
    if dup is not None:
        codes = re.findall(r"\((?:CMT|Diablo|Amana|Bosch|Freud|Whiteside) ([^)]+)\)", t["name"])
        sh = re.findall(r"shank ([0-9.]+) mm|([0-9.]+) mm shank", t["notes"])
        sh = [a or b for a, b in sh][:1]
        dup["notes"] += " Same bit also listed as %s%s." % (", ".join(codes), (" (shank %s mm)" % sh[0]) if sh else "")
        print("merged", t["name"], "->", dup["name"], file=_sys.stderr)
        continue
    unique.append((t, k, cl))
unique = [u[0] for u in unique]
tools[:] = unique
BRAND = "generic"
# ---- Generic library: standard, brand-neutral sizes (no maker's data; every value is a conservative starting point) ----------------------
G_MM = [1, 2, 3, 4, 6, 8, 10, 12]
G_IN = [("1/8", 3.175), ("3/16", 4.7625), ("1/4", 6.35), ("3/8", 9.525), ("1/2", 12.7)]
def _gsize(d, sys_): return (d, sys_, ("%s\u2033" % [k for k, v in G_IN if abs(v - d) < 1e-3][0]) if sys_ == "in" else "%s mm" % fmt(d))
sizes = [(d, "mm", "%s mm" % fmt(d)) for d in G_MM] + [(v, "in", "%s\u2033" % k) for k, v in G_IN]
def gadd(name, sys_, shape, d, z, notes, angle=60.0, slow=False, tip=0.0, feed_cap=2000.0, pd_cap=None):
    rpm, feed, plunge, so, pd = speeds(d, z, shape, slow)
    if pd_cap: pd = min(pd, pd_cap)
    add(name=name, sys=sys_, shape=shape, angle=angle, flutes=z, dia=round(d, 3), rpm=min(rpm, 12000), feed=min(feed, feed_cap), plunge=plunge, so=so, pd=pd, notes=notes)
    if tip: tools[-1]["tip"] = tip
for d, sys_, lab in sizes:
    if d >= 2: gadd("%s upcut end mill" % lab, sys_, "flat", d, 2, "2-flute upcut spiral: general purpose for wood, MDF and plastics; clears chips upward.")
    if d >= 3: gadd("%s downcut end mill" % lab, sys_, "flat", d, 2, "2-flute downcut spiral: a clean top edge on plywood and laminates; it packs chips into the cut.")
    if d >= 6 and d <= 12.7: gadd("%s compression bit" % lab, sys_, "flat", d, 2, "Compression spiral: upcut at the bottom, downcut at the top, for clean faces on both sides of plywood and laminates. Cut deeper than its downcut section.")
    if d >= 1: gadd("%s flat end mill, single flute" % lab if d < 3 else "%s single-flute end mill" % lab, sys_, "flat", d, 1, "Single flute: big chip clearance for plastics, acrylic and aluminium (use a lubricant or air blast on metal).", slow=True)
    if d >= 2: gadd("%s ball nose" % lab, sys_, "ball", d, 2, "Relief carving, 3D finishing and rounded grooves; a small stepover leaves a smoother surface.")
for ang, d, sys_, lab in [(30, 6.35, "in", "1/4\u2033"), (60, 6.35, "in", "1/4\u2033"), (90, 6.35, "in", "1/4\u2033"), (90, 12.7, "in", "1/2\u2033"), (120, 12.7, "in", "1/2\u2033"),
                          (60, 6.0, "mm", "6 mm"), (90, 6.0, "mm", "6 mm"), (90, 12.0, "mm", "12 mm"), (120, 12.0, "mm", "12 mm")]:
    gadd("%d\u00b0 V-bit (%s)" % (ang, lab), sys_, "vbit", d, 2, "V-carving, chamfers and lettering; the depth of cut sets the line width.", angle=float(ang), feed_cap=1200.0, pd_cap=2.0)
for ang, d, sys_, lab, tip in [(20, 3.175, "in", "1/8\u2033", 0.1), (30, 3.175, "in", "1/8\u2033", 0.1), (60, 6.35, "in", "1/4\u2033", 0.2), (15, 3.0, "mm", "3 mm", 0.1), (30, 3.0, "mm", "3 mm", 0.1)]:
    gadd("%d\u00b0 engraving bit, %s shank" % (ang, lab), sys_, "engrave", d, 1, "Fine engraving and small lettering; the cutting tip is very small. Shallow passes only.", angle=float(ang), tip=tip, feed_cap=800.0, pd_cap=0.5)
for d, sys_, lab, taper in [(6.35, "in", "1/4\u2033", 10.0), (6.0, "mm", "6 mm", 10.0)]:
    gadd("%s tapered ball nose, 10\u00b0" % lab, sys_, "taper", d, 2, "Tapered ball nose for fine 3D carving. The tip radius is a typical value; set it to your bit's real tip.", angle=taper, tip=0.5, feed_cap=900.0, pd_cap=1.0)
for d, sys_, lab in [(25.4, "in", "1\u2033"), (38.1, "in", "1-1/2\u2033"), (50.0, "mm", "50 mm"), (76.2, "in", "3\u2033")]:
    rpm = 12000 if d <= 40 else (10000 if d <= 55 else 8000)
    add(name="%s surfacing bit (spoilboard)" % lab, sys=sys_, shape="flat", angle=60.0, flutes=3, dia=d, rpm=rpm, feed=1800.0, plunge=300.0, so=70.0, pd=1.0,
        notes="Large-diameter surfacing / fly-cutter bit for flattening the spoilboard or slabs: shallow passes with about 70% stepover, lowest sensible speed for the size.")
for d, sys_, lab, r in [(6.35, "in", "1/4\u2033", 1.5875), (9.525, "in", "3/8\u2033", 3.175)]:
    rpm_, feed, plunge, so, pd = speeds(d, 2, "arc")
    add(name="%s round-over bit, R%s" % (lab, fmt(r)), sys=sys_, shape="arc", angle=60.0, flutes=2, dia=d, rpm=min(rpm_, 12000), feed=min(feed, 1200.0), plunge=min(plunge, 250.0), so=40.0, pd=min(pd, r),
        notes="Convex round-over / bull-nose profile bit: rounds edges and carves relief with an arc of the given radius.")
    tools[-1]["arc"] = r

LIBS = [("cmt", "cmt", "CMT router bits", "CMT Orange Tools: CNC solid-carbide spirals, round nose, V-groove, surfacing and contractor bits."),
        ("amana", "amana", "Amana Tool CNC bits", "Amana Tool: solid-carbide spirals, compression, ball nose, plastic cutters and round-over bits."),
        ("bosch", "bosch", "Bosch PRO router bits", "Bosch Professional: straight, V-groove and core box (round nose) bits."),
        ("generic", "generic", "Generic CNC bits", "Brand-neutral set: standard sizes of end mills, compression, ball nose, V-bits, engraving, taper and surfacing bits."),
        ("freud", "freud", "Freud router bits", "Freud: straight, spiral, compression, O-flute, round nose, V-groove and carving bits from the General Purpose catalog."),
        ("whiteside", "whiteside", "Whiteside CNC bits", "Whiteside: solid-carbide spirals, ball and round nose, V-groove, engraving, bowl and spoilboard bits from its CNC brochure."),
        ("diablo", "diablo", "Diablo router bits", "Diablo: a few straight, round nose and V-groove bits.")]
here = os.path.dirname(os.path.abspath(__file__))
for brand, fname, title, _ in LIBS:
    own = [dict(t) for t in tools if t["brand"] == brand]
    for i, t in enumerate(own, 1):
        t["id"] = "%s%03d" % (brand[:3], i); t.pop("brand", None)
    with open(os.path.join(here, "%s.json" % fname), "w", encoding="utf-8") as f:
        json.dump({"app": "putia-cnc-tools", "v": 1, "name": title, "tools": own}, f, ensure_ascii=False, indent=1); f.write("\n")
    print("%-8s %d tools" % (brand, len(own)), file=_sys.stderr)
