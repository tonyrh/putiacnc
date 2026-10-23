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
        note = ("CMT %s, %s, %s. Cutting length %s mm, overall %s mm, shank %s mm.%s %s Dimensions from CMT's website; feeds and speeds are conservative starting values, not CMT's."
                % (code, f["title"], f["mat"], fmt(cl), fmt(ol), fmt(sh), extra, f["use"]))
        add(name="%s %s %s (CMT %s)" % (label(d, sys), f["title"], name_z, code), sys=sys, shape=f["shape"], angle=60.0, flutes=int(total),
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

out = {"app": "putia-cnc-tools", "v": 1, "name": "CMT CNC bits", "tools": tools}
json.dump(out, _sys.stdout, ensure_ascii=False, indent=1)
_sys.stdout.write("\n")
