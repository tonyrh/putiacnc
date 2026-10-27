# TODO

Things we agreed to do, or decided to wait on. Newest thinking first; cross items off when done.

## Needs the maintainer
- [ ] Try the recent changes on real projects: operation eye, clamp fold and hide, warning signs in 3D, tool dialogs closing on Esc / right-click.
- [ ] Air cut on the real GRBL 1.1 machine (nothing has been cut yet).

## Small
- [ ] macOS (the app runs there; checked by the maintainer): fix a few mouse cursor icons (list to come from the maintainer), then a native title bar (planned for about next week; today the window draws its own bar with buttons at the top right).
- [x] Mark clamp collisions on the 2D drawing too (a clamp the toolpath runs into turns solid red with the same warning sign as the 3D preview).
- [ ] Count the tool holder in the clamp check: Holder dia. in the machine settings, Stickout in the tool manager (0 = unknown, check off).
- [x] Move shapes into an existing operation: "Use selected shapes" (shown in the active operation, and in any operation without shapes). Maybe later: a right-click menu.
- [x] An operation that loses its last shape stays with its settings; "Use selected shapes" in the operation puts shapes (also from other operations) into it. This also covers moving shapes between operations.
- [ ] Text dialog can run off a short window; the tool palette overflows in very small windows.

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
- [ ] Other ideas from the feature comparison: G-code sender, more post-processors, dogbone corners, helical entry, nesting.

## Interface consistency (audit of 2026-10-26)
Done: "…" and "×" everywhere in the app's texts, sentence case in the menus. Still open:
- [x] Spelling: British "Centre" is the choice for the app's texts (decided by the maintainer; nothing to change).
- [x] Button names: the G-code panel button is now "Export G-code…", like the menu. (The "Clean up" button was already right.)
- [x] Messages: the banner has three styles: error (red), warning (orange, chosen so it is not mistaken for green), information (blue, with an "i"). Results of tools and imports are information or warnings; only real failures are errors.
- [ ] Colour-blindness check (maintainer has a colour vision defect): find places where colour alone carries the meaning (clamp collisions red in 2D and 3D, operation eye, toolpath colours) and give each a second cue.
- [x] Combine, Array, Offset and Corners close on a successful Apply (Close is now Cancel); they stay open when nothing changed.
- [x] When Apply changes nothing, the message of Combine, Array, Offset and Corners shows in the tool's own window instead of the banner, and goes when the window closes. (Notes about what was done, such as "N open shapes were left out", still go to the banner.)
- [ ] Align, Offset and Corners: check what they show with an empty selection (no leftover preview, Apply disabled, a clear line saying what to select). (The duplicated "Select … first." line is fixed in Array, Offset, Corners and Combine.)
- [x] The controls converted to segmented switches without a screenshot hook (Ellipse / Circle, Radius / Diameter, Polygon / Star, Trim / Extend, Face outwards / Face the centre): tried by the maintainer, they work.
- [ ] Mac: how the all-scroll (four-way) move cursor looks.

## Release 0.3.2
- [x] README screenshots refreshed (`docs/screenshot-2d.png`, `docs/screenshot-3d.png`; project built by `examples/docproject.rs`).
- [x] Demo video replaced (`docs/demo.mp4` and the one embedded in the README); the unused GIF was removed.
- [ ] Maintainer tags and publishes.

## Later
- [ ] macOS notarization.
- [ ] Arrow-key navigation in the tool picker.
