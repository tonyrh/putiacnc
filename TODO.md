# TODO

Things we agreed to do, or decided to wait on. Newest thinking first; cross items off when done.

## Out of scope

PutiaCNC creates G-code; it does not run the machine. Everything that talks to the machine is left to the sender the user already likes (UGS, gSender, CNCjs, bCNC, the controller's own screen), so these are decided against, not postponed:
- A G-code sender (streaming over serial, jogging, pause and stop, position display).
- Probing and homing routines. The machine profile's own start and end G-code boxes let a user paste their own.

## Needs the maintainer
- [x] 3D preview with a very big toolpath (140,000 moves and more): 30 ms a frame at 140,000 moves and 115 ms at 770,000 (see docs/testing.md, `gui/src/perf.rs`). Decided not to draw the toolpath on the GPU: it cannot be tested without a GPU, needs hand-written GL for the glow backend, and only matters for rare huge jobs. Skipping steps shorter than a few pixels did not help either.
- [ ] Try the recent changes on real projects: operation eye, clamp fold and hide, warning signs in 3D, tool dialogs closing on Esc / right-click.
- [ ] Air cut on the real GRBL 1.1 machine (nothing has been cut yet).

## Small
- [ ] macOS (the app runs there; checked by the maintainer): fix a few mouse cursor icons (list to come from the maintainer), then a native title bar (planned for about next week; today the window draws its own bar with buttons at the top right).
- [x] Mark clamp collisions on the 2D drawing too (a clamp the toolpath runs into turns solid red with the same warning sign as the 3D preview).
- [x] Count the tool holder in the clamp check: Holder dia. in the machine settings (behind a "Tool holder" switch, off by default), Stick-out in the tool manager (0 = unknown, check off). Also drawn in the 3D preview, which turns the clamp and the touching part red during the replay, with a banner. Not done: the holder against the stock itself (a deep cut with a short stick-out) and the shank diameter in the clamp check.
- [x] Move shapes into an existing operation: "Use selected shapes" (shown in the active operation, and in any operation without shapes). Maybe later: a right-click menu.
- [x] An operation that loses its last shape stays with its settings; "Use selected shapes" in the operation puts shapes (also from other operations) into it. This also covers moving shapes between operations.
- [x] Dialog buttons: a window that does its job once (Box generator, Fingers, Combine) has OK (does it and closes) and Cancel; one that is used again and again (Offset, Array, Fillet and chamfer) has Apply (does it, stays open) and Close; Clean up keeps Clean up and Close. The dialog audit checks the labels, the row and the order.

## Bigger
- [x] Setup assistant, quick tour, three sample projects (name plate, finger-joint box, 3D relief) that fit the user's machine and tools (0.4.0 to 0.4.3).
- [x] Setup sheet (File > Setup sheet…): a printable HTML page with the stock and zero, the tools in order, the operations with times, a checklist, the warnings and the notes (0.4.2).
- [x] Surface the stock or the spoilboard (Generate > Surface the stock…): a Pocket over a rectangle a cutter radius larger than the stock, with a wide cutter (0.4.2).
- [x] The message when a pocket or inside profile makes no toolpath names the reason (tool wider than the shape, with the widths and a tool that fits; open lines).
- [x] Rest machining: when the main cutter is too big to cut anything, the Corners cutter cuts the whole pocket, with an information note.
- [x] Relief finishing lines along X, Y or both.
- [x] Finishing pass with stock to leave for Profile and Pocket (the Finish row).
- [x] Rest machining for 3D relief: a smaller ball nose (the Detail row of the Finishing step) finishes only what the finishing ball could not reach. V-carve does not need it (one bit cuts every width).
- [ ] Check the setup assistant and the quick tour on macOS and Windows (only tested on Linux so far).
- [x] Grouping (Edit > Group, Ctrl+G; Ungroup, Ctrl+Shift+G): done. Groups are flat (grouping a group with something else merges them); a copy of a group is a group of its own.
- [x] Layers (name, colour, eye, lock; hidden layers still get cut, as in Aspire): done. Still to do: DXF and SVG layers by name on import (a layer for each layer of the file), and a marker in the operations list for an operation that cuts hidden shapes (the summary says it now).
- [ ] V-carve with a flat depth (as Aspire): the V-bit carves the edges only down to a "Flat depth", and a second, flat tool ("Clear with") clears the wider area at that depth, so large letters get a flat bottom with sharp V walls. Today the depth only caps the V trench (and stops at the tool's full diameter), and nothing clears the floor. Build on the second tool and the pocket and rest machining that inlay already uses; test the overlap between the two so no ridge is left. Maybe also: follow inside corners to a sharp point at the bottom (the offset method rounds them slightly).
- [ ] Maybe later: dimension lines (linear, radius, diameter, angle; display only, attached to their points). Left out for now: operations already group shapes by what is cut.
- [x] Text: automatic kerning (font `kern` / `GPOS` pairs), letter / word spacing in % (done); fit to width done.
- [ ] Text: manual kerning handles on the canvas.
- [x] Text along an arc at any angle (done).
- [x] Text arc: warn when the text is longer than the circle (done).
- [ ] Maybe, only if asked: text along any picked path (rigid letters first, warped later; placed once, not live-linked). Decided not worth the cost for now.
- [x] Rest machining for pockets: a smaller flat end mill (the Corners row of a Pocket operation) clears only what the main cutter could not reach (corners and parts narrower than it).
- [ ] Bezier and arc node editing (0.4.0).
- [ ] Marlin output (a third G-code style next to GRBL and Mach3): `;` comments instead of `( )`, no `M2` (end with `M5` and a lift), tool change and spindle `S` scaling as the machine's build needs, arcs optional. First ask the maintainer: is it a CNC build with a spindle, are `G2`/`G3` enabled, how are tools changed. Not checked against Marlin itself yet.
- [ ] More G-code styles, in this order: Marlin (item above), then Smoothieware and Duet / RepRapFirmware, then a generic Fanuc-style output (`G54`-`G59`, `G43`, `M6` with `T`, `%` markers; the Haas manuals are free). GRBL-compatible controllers (grblHAL, FluidNC, Carbide Motion, OpenBuilds Blackbox) probably already work: only machine presets. Every new style is marked "untested on a machine" until someone has tried it.
- [x] (Done: the machine catalogue has 20 machines; the rest are listed under "More machines" below.) Machine presets for popular hobby machines: search the web for the current specs (work area, controller, spindle speed range) of Genmitsu 3018 / 4040 / ProVerXL, Shapeoko, X-Carve, OpenBuilds (WorkBee, LEAD, ACRO), Sienci LongMill, BobsCNC, Snapmaker (CNC mode), Carvera, Onefinity; draft the preset values for the maintainer to check before adding any. Take numbers from the vendors' own pages, not from other programs' files. The usable area can differ from the stated one (clamps, homing offsets), so presets are starting points.
- [x] Inlay plug on a separate piece of stock: done ("Cut from a separate piece" in the plug operation: size, thickness, position, XY zero, material; two G-code files, "name.nc" and "name-plug.nc"; G-code tab switches between them; 2D and 3D show and simulate the piece). The toolpath checks run on the piece too (warnings are marked "plug piece" and Show in 3D switches to it). Replacing the main stock: a piece that takes the main stock's place (same size and zero, the clamps apply) is the default. A piece that stays beside is checked against the clamps at the place where it is drawn. A new piece looks like a contrasting wood.
- [ ] Other ideas from the feature comparison: more post-processors, nesting that uses the wasted space of an open pocket (nesting by outline, holes cut through included, exists: Modify → Nest on the stock). (Dogbone and T-bone corners exist: Modify → Fillet and chamfer, and in the box generator.)
- [x] Export split: Export G-code asks how to split the program: one file (as now, the default), one file per tool, or one file per operation. Each file is complete on its own (header with spindle on and a first move to safe Z, a safe end, a summary of its own operation, tool and time); names `<project>-<number>-<operation>.nc`; the dialog lists the files with their times before saving. A tool change or "Pause afterwards" at a file boundary becomes the end of that file, not an `M0`; every file after a tool change repeats the "touch off Z0 again" reminder. Operations with the same tool stay separate files. The 3D preview and the time estimate still describe the whole job; plug pieces stay separate files, with the split inside each piece. Mockup agreed 2026-10-30; build after the air cut on the GRBL machine shows how `M0` behaves.
- [x] Plotter mode, first step: a pen or marker as the tool, to draw outlines on paper. A tool shape "pen" (line width, draw speed, pen-down speed) and a Draw operation (pen down at Z0 or a little below for a spring-loaded holder, lifted a few mm between strokes, nearest stroke first, one pass, no spindle). Draw stays separate from cutting: a pen only works in Draw, a cutter never in Draw. The G-code never starts the spindle, the checker and the 3D simulation know it, and there is a pen test project for a real machine.
- [x] Single-line text: six built-in single-stroke (Hershey) fonts in the text tool (Sans, Sans bold, Roman, Roman bold, Script, Cursive), listed first as "Single line · …". Letters are open strokes (an o closes on itself) that a Draw operation follows with the pen; Italian and Latin-1 accents are put together from the plain letter and a mark. Straight, stacked, arc, spacing, fit to width and shadow all work; kerning does not apply. Credit in `fonts/hershey/NOTICE.txt`.
- [x] Hatch fills: a Draw operation can fill closed shapes with parallel lines (angle, spacing = the pen width unless set, cross-hatch, outline on or off). Even-odd, so a shape inside another is a hole. Each line is its own stroke, so the pen lifts between lines.
- [x] Joining strokes: the lines of a hatch fill are joined into a zigzag (when the way across lies inside the shapes), "Join gaps under N mm" keeps the pen down between strokes that start close to where the last one ended, and an edge two shapes share is drawn once (identical segments only; an edge that only partly overlaps another is drawn twice).
- [x] Pen colour per tool: a pen has an ink colour (colour button in the tool editor, saved as `color` "#rrggbb"); the 3D preview draws that pen's ink in it and the tool lists show a dot. It is for the screen only: the G-code does not change (the pen-change stop names the pen). The bundled pens have none (dark ink).
- [x] Repeat count (1 to 20 passes over the whole drawing, `(Repeat n of N)` comments) and a pen-down rest (`G4 P<seconds>` after every pen-down; none when the pen stays down for a joined stroke) on the Draw operation.
- [x] 2D pen width: the canvas draws a Draw operation's strokes as wide as the pen (at least one pixel, in the pen's ink colour if it has one, otherwise the operation's colour), so hatch spacing, overlaps and gaps show before drawing.
- [x] Centre-line bitmap trace with the pen: checked that a centre-line trace of a sketch is drawn one line per stroke by a Draw operation (test with `tests/data/sketch.png`). Known: where two strokes cross, the trace leaves a bridge of about a millimetre between two junctions instead of one point (invisible in ink; "Join gaps" keeps the pen down over it).
- [ ] Plotter mode, next: a servo or solenoid pen (`M3 S<angle>` / `M5` as a machine setting); try the pen test on the real machine and adjust the wording.

## Interface consistency (audit of 2026-10-26)
Done: "…" and "×" everywhere in the app's texts, sentence case in the menus. Still open:
- [x] Spelling: British "Centre" is the choice for the app's texts (decided by the maintainer; nothing to change).
- [x] Button names: the G-code panel button is now "Export G-code…", like the menu. (The "Clean up" button was already right.)
- [x] Messages: the banner has three styles: error (red), warning (orange, chosen so it is not mistaken for green), information (blue, with an "i"). Results of tools and imports are information or warnings; only real failures are errors.
- [x] Combine, Array, Offset and Corners close on a successful Apply (Close is now Cancel); they stay open when nothing changed.
- [x] When Apply changes nothing, the message of Combine, Array, Offset and Corners shows in the tool's own window instead of the banner, and goes when the window closes. (Notes about what was done, such as "N open shapes were left out", still go to the banner.)
- [x] Align, Offset, Corners (and Array, Combine) with an empty selection: checked by screenshot. Align closes itself; the others stay open with one clear line ("Select … first."), Apply greyed out and no preview. Nothing to change.
- [x] The controls converted to segmented switches without a screenshot hook (Ellipse / Circle, Radius / Diameter, Polygon / Star, Trim / Extend, Face outwards / Face the centre): tried by the maintainer, they work.
- [ ] Mac: how the all-scroll (four-way) move cursor looks.

## Release 0.3.2
- [x] README screenshots refreshed (`docs/screenshot-2d.png`, `docs/screenshot-3d.png`; project built by `examples/docproject.rs`).
- [x] Demo video replaced (`docs/demo.mp4` and the one embedded in the README); the unused GIF was removed. The sound was taken out of `docs/demo.mp4` (the website uses that file). The copy embedded in the README was uploaded again without sound.
- [x] Maintainer tags and publishes (0.3.2 is out).

## Project page
- [x] Project page is up at `https://tonyrh.github.io/putiacnc/` (GitHub Pages from `main` / `/docs`), verified in Google Search Console with the meta tag in `docs/index.html`, sitemap submitted, repo Website field set. Waiting for Google to index it.

## Later
- [ ] macOS notarization.
- [ ] Arrow-key navigation in the tool picker.
- [ ] Haiku port (idea, low priority): Rust has a Tier 3 `x86_64-unknown-haiku` target, but eframe/winit has no Haiku backend as far as I know, so the GUI would need an SDL-style windowing layer, and file dialogs and clipboard would need Haiku replacements. The core crate and `putia-gcode` might build as they are; untested.
- [ ] Raspberry Pi / ARM64 Linux (Pi 4 and 5, 64-bit OS): the core crate and `putia-gcode` should build as they are; the GUI needs OpenGL 3.3 or ES 3 (Mesa on the Pi 4/5) and may be slow with big toolpaths or the 3D preview. Untested (the sandbox could not download the aarch64 target). Options: try `cargo build --release` on a real Pi, and/or add a CI job that cross-builds `aarch64-unknown-linux-gnu` and publishes a release binary. Pi 3 and 32-bit armv7 are borderline.

## Long term ideas
Things that make a real job easier
- [x] Setup sheet (done in 0.4.2, File → Setup sheet): a printable page or PDF with the stock, work zero, tools in order and estimated time (both files and both zeros for the two-piece inlay).
- [x] Feeds and speeds by material: Material in the Stock section (softwood, hardwood, plywood, MDF, acrylic, aluminium, foam) and Rigidity in the machine editor (light, medium, heavy); "Suggest for <material>" in an operation's custom feeds fills in spindle speed, feed, plunge, stepover and depth per pass, and a line shows the chip load against the usual range. Numbers are typical hobby-router starting values (`src/feeds.rs`), not tool-maker data: to check against real cuts and adjust the table. Not yet: per-tool-maker chip-load data (CMT, Amana), surface-speed limits for big cutters beyond a simple rule, spindle power.
- [x] Surfacing / spoilboard flattening (done in 0.4.2, Generate → Surface the stock).
- [ ] Double-sided work: flip the piece with registration holes or dowels.

Toolpath quality
- [x] Better path ordering (fewer and shorter rapids): done, always on.
- [x] Dogbone corners (Fillet and chamfer window).
- [x] Helical entry for pockets and profiles.
- [x] Adaptive clearing, simple form (light stepover, rounded inner rings). A true constant-engagement toolpath (trochoidal loops following the remaining material) is not done.
- [x] Finger joints on rectangle edges (Modify menu).
- [x] Box generator (Generate menu).
- [x] T-bone corners and box dividers.
- [x] Box dividers in both directions (half-lap crossings).
- [ ] Box lids that slide or hinge.
- [ ] Adaptive clearing.
- [ ] Nesting.

Machine side
- [x] Preview against the real machine limits (acceleration) for more accurate time estimates: done, Acceleration and Corner deviation in the machine editor (off by default). Still to do: check it against the time a real job takes on the machine, and fill the acceleration into the machine catalogue where the makers publish it. Per-axis limits are done (Acceleration Y and Z in the machine editor); per-axis top speeds (`$110` to `$112`) are done too (Top speed X, Y and Z).

Reach and polish
- [ ] Italian translation and a colour-blind-safe theme.
- [ ] Distribution: a Windows installer, Flatpak, a notarized Mac build. (The AppImage, the macOS disk image and the portable Windows zip are done.)
- [ ] More project templates and an example gallery (three samples exist: Generate → Sample projects); a short manual with videos.
- [x] Shared tool and machine preset libraries on GitHub (the ready-made libraries are in `libraries/`; removals and corrections are taken as issues).
- [x] First-run setup assistant (units, machine, starter tools).
- [x] Machine catalogue from the makers' published specs (20 machines; setup assistant and Machines → From catalogue).
- [ ] More machines: Lunyee and TwoTrees (maker pages not found yet), Sienci LongMill (travel per size), OpenBuilds LEAD, Stepcraft. Needs app work first: Makera Carvera (Smoothieware-based firmware, M6 tool changes) and Onefinity (Redline controller, not GRBL).

- [ ] Low priority, not worth an upgrade on its own: in egui 0.27 a disabled widget takes the Tab focus for one frame and drops it, so Tab cannot get past a greyed-out button (the Operations panel's controls are out of reach by keyboard while no shapes are selected). Left as it is. If egui is upgraded for another reason, check whether the newer version fixes it and test Tab in the Operations panel with nothing selected and in a dialog with a disabled button. (A vendored egui with a one-line change in `create_widget`, skipping `interested_in_focus` for a disabled widget, would also fix it.)
