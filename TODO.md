# TODO

Things we agreed to do, or decided to wait on. Newest thinking first; cross items off when done.

## Needs the maintainer
- [ ] Try the recent changes on real projects: operation eye, clamp fold and hide, warning signs in 3D, tool dialogs closing on Esc / right-click.
- [ ] Air cut on the real GRBL 1.1 machine (nothing has been cut yet).

## Small
- [x] Mark clamp collisions on the 2D drawing too (a clamp the toolpath runs into turns solid red with a "!" badge).
- [ ] Count the tool holder / collet diameter in the clamp check (today only the cutter radius counts).
- [ ] A way to move a shape into or out of an existing operation (for example a right-click menu "Operation ▸ …"). Wait until the current flow has been used for a while.
- [ ] Deleting a shape to redraw it loses the operation's settings if it was the last shape; think about a way to keep them.
- [ ] Text dialog can run off a short window; the tool palette overflows in very small windows.

## Bigger
- [x] Text: automatic kerning (font `kern` / `GPOS` pairs), letter / word spacing in % (done); fit to width done.
- [ ] Text: manual kerning handles on the canvas.
- [x] Text along an arc at any angle (done).
- [x] Text arc: warn when the text is longer than the circle (done).
- [ ] Maybe, only if asked: text along any picked path (rigid letters first, warped later; placed once, not live-linked). Decided not worth the cost for now.
- [ ] Rest machining: clear only what the previous, larger tool left (pockets first, then relief).
- [ ] Bezier and arc node editing (0.4.0).
- [ ] Other ideas from the feature comparison: G-code sender, more post-processors, dogbone corners, helical entry, nesting.

## Release 0.3.2
- [x] README screenshots refreshed (`docs/screenshot-2d.png`, `docs/screenshot-3d.png`; project built by `examples/docproject.rs`).
- [ ] Refresh `docs/demo.gif` and `docs/demo.mp4` to match.
- [ ] Maintainer tags and publishes.

## Later
- [ ] macOS notarization.
- [ ] Arrow-key navigation in the tool picker.
