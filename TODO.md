# TODO

Things we agreed to do, or decided to wait on. Newest thinking first; cross items off when done.

## Needs the maintainer
- [ ] Try the recent changes on real projects: operation eye, clamp fold and hide, warning signs in 3D, tool dialogs closing on Esc / right-click.
- [ ] Air cut on the real GRBL 1.1 machine (nothing has been cut yet).

## Small
- [ ] Mark clamp collisions on the 2D drawing too (today the warning signs are in the 3D preview only).
- [ ] Count the tool holder / collet diameter in the clamp check (today only the cutter radius counts).
- [ ] A way to move a shape into or out of an existing operation (for example a right-click menu "Operation ▸ …"). Wait until the current flow has been used for a while.
- [ ] Deleting a shape to redraw it loses the operation's settings if it was the last shape; think about a way to keep them.
- [ ] Text dialog can run off a short window; the tool palette overflows in very small windows.

## Bigger
- [ ] Text: automatic kerning (font `kern` / `GPOS` pairs), letter / word spacing in %, fit to width.
- [ ] Text: manual kerning handles on the canvas.
- [ ] Text along any picked path (circle layouts become presets of the same engine); rigid letters first, warped letters later.
- [ ] Rest machining: clear only what the previous, larger tool left (pockets first, then relief).
- [ ] Bezier and arc node editing (0.4.0).
- [ ] Other ideas from the feature comparison: G-code sender, more post-processors, dogbone corners, helical entry, nesting.

## Release 0.3.2
- [ ] Refresh the README and docs screenshots (`docs/screenshot-*.png`, `docs/demo.gif`, `docs/demo.mp4`): panel order, dialog positions, badges, Preview, warning signs.
- [ ] Maintainer tags and publishes.

## Later
- [ ] macOS notarization.
- [ ] Arrow-key navigation in the tool picker.
