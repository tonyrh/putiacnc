# TODO

Things we agreed to do, or decided to wait on. Newest thinking first; cross items off when done.

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

## Bigger
- [x] Grouping (Edit > Group, Ctrl+G; Ungroup, Ctrl+Shift+G): done. Groups are flat (grouping a group with something else merges them); a copy of a group is a group of its own.
- [ ] Maybe later: layers (name, colour, eye, lock; hidden layers still get cut, as in Aspire) and dimension lines (linear, radius, diameter, angle; display only, attached to their points). Left out for now: operations already group shapes by what is cut.
- [x] Text: automatic kerning (font `kern` / `GPOS` pairs), letter / word spacing in % (done); fit to width done.
- [ ] Text: manual kerning handles on the canvas.
- [x] Text along an arc at any angle (done).
- [x] Text arc: warn when the text is longer than the circle (done).
- [ ] Maybe, only if asked: text along any picked path (rigid letters first, warped later; placed once, not live-linked). Decided not worth the cost for now.
- [ ] Rest machining: clear only what the previous, larger tool left (pockets first, then relief).
- [ ] Bezier and arc node editing (0.4.0).
- [ ] Marlin output (a third G-code style next to GRBL and Mach3): `;` comments instead of `( )`, no `M2` (end with `M5` and a lift), tool change and spindle `S` scaling as the machine's build needs, arcs optional. First ask the maintainer: is it a CNC build with a spindle, are `G2`/`G3` enabled, how are tools changed. Not checked against Marlin itself yet.
- [ ] More G-code styles, in this order: Marlin (item above), then Smoothieware and Duet / RepRapFirmware, then a generic Fanuc-style output (`G54`-`G59`, `G43`, `M6` with `T`, `%` markers; the Haas manuals are free). GRBL-compatible controllers (grblHAL, FluidNC, Carbide Motion, OpenBuilds Blackbox) probably already work: only machine presets. Every new style is marked "untested on a machine" until someone has tried it.
- [ ] Machine presets for popular hobby machines: search the web for the current specs (work area, controller, spindle speed range) of Genmitsu 3018 / 4040 / ProVerXL, Shapeoko, X-Carve, OpenBuilds (WorkBee, LEAD, ACRO), Sienci LongMill, BobsCNC, Snapmaker (CNC mode), Carvera, Onefinity; draft the preset values for the maintainer to check before adding any. Take numbers from the vendors' own pages, not from other programs' files. The usable area can differ from the stated one (clamps, homing offsets), so presets are starting points.
- [x] Inlay plug on a separate piece of stock: done ("Cut from a separate piece" in the plug operation: size, thickness, position, XY zero, material; two G-code files, "name.nc" and "name-plug.nc"; G-code tab switches between them; 2D and 3D show and simulate the piece). The toolpath checks run on the piece too (warnings are marked "plug piece" and Show in 3D switches to it). Replacing the main stock: a piece that takes the main stock's place (same size and zero, the clamps apply) is the default. A piece that stays beside is checked against the clamps at the place where it is drawn. A new piece looks like a contrasting wood.
- [ ] Other ideas from the feature comparison: G-code sender, more post-processors, dogbone corners, helical entry, nesting.
- [x] Export split: Export G-code asks how to split the program: one file (as now, the default), one file per tool, or one file per operation. Each file is complete on its own (header with spindle on and a first move to safe Z, a safe end, a summary of its own operation, tool and time); names `<project>-<number>-<operation>.nc`; the dialog lists the files with their times before saving. A tool change or "Pause afterwards" at a file boundary becomes the end of that file, not an `M0`; every file after a tool change repeats the "touch off Z0 again" reminder. Operations with the same tool stay separate files. The 3D preview and the time estimate still describe the whole job; plug pieces stay separate files, with the split inside each piece. Mockup agreed 2026-10-30; build after the air cut on the GRBL machine shows how `M0` behaves.
- [x] Plotter mode, first step: a pen or marker as the tool, to draw outlines on paper. A tool shape "pen" (line width, draw speed, pen-down speed) and a Draw operation (pen down at Z0 or a little below for a spring-loaded holder, lifted a few mm between strokes, nearest stroke first, one pass, no spindle). Draw stays separate from cutting: a pen only works in Draw, a cutter never in Draw. The G-code never starts the spindle, the checker and the 3D simulation know it, and there is a pen test project for a real machine.
- [x] Single-line text: six built-in single-stroke (Hershey) fonts in the text tool (Sans, Sans bold, Roman, Roman bold, Script, Cursive), listed first as "Single line · …". Letters are open strokes (an o closes on itself) that a Draw operation follows with the pen; Italian and Latin-1 accents are put together from the plain letter and a mark. Straight, stacked, arc, spacing, fit to width and shadow all work; kerning does not apply. Credit in `fonts/hershey/NOTICE.txt`.
- [x] Hatch fills: a Draw operation can fill closed shapes with parallel lines (angle, spacing = the pen width unless set, cross-hatch, outline on or off). Even-odd, so a shape inside another is a hole. Each line is its own stroke, so the pen lifts between lines.
- [x] Joining strokes: the lines of a hatch fill are joined into a zigzag (when the way across lies inside the shapes), "Join gaps under N mm" keeps the pen down between strokes that start close to where the last one ended, and an edge two shapes share is drawn once (identical segments only; an edge that only partly overlaps another is drawn twice).
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
- [x] Demo video replaced (`docs/demo.mp4` and the one embedded in the README); the unused GIF was removed.
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
- [ ] Setup sheet: a printable page or PDF with the stock, work zero, tools in order and estimated time (both files and both zeros for the two-piece inlay).
- [ ] Feeds and speeds by material: chip-load rules from tool and material (ties in with the CMT tool data). The per-operation custom feeds and speeds are in; this would fill them in.
- [ ] Surfacing / spoilboard flattening wizard.
- [ ] Double-sided work: flip the piece with registration holes or dowels.

Toolpath quality
- [x] Better path ordering (fewer and shorter rapids): done, always on.
- [ ] Dogbone / T-bone corners and a finger / box-joint generator.
- [ ] Adaptive clearing.
- [ ] Nesting.

Machine side
- [ ] Send G-code to a GRBL machine over serial (jog, set zero, run, pause). Probing snippets are an easier first step.
- [ ] Preview against the real machine limits (acceleration) for more accurate time estimates.

Reach and polish
- [ ] Italian translation and a colour-blind-safe theme.
- [ ] Distribution: Windows installer, AppImage / Flatpak, signed Mac build.
- [ ] Project templates and an example gallery; a short manual with videos.
- [ ] Shared tool and machine preset libraries on GitHub.
