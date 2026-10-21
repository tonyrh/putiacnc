# putiacnc-core

The calculation core of PutiaCNC, in Rust: project model, geometry and GRBL G-code generation.
It is a port of the browser app's `build()` and produces the same output for the same project.

## What's in it

| File | Purpose |
|---|---|
| `src/model.rs` | Data model (serde). Reads the project files the browser app exports (`"app": "putia-cnc"`), no conversion needed. |
| `src/clip.rs` | Safe wrapper around Clipper 6.4 (the same library and version the browser app uses): even-odd union, round-join offset, cleaning, area. |
| `src/toolpath.rs` | Profile (outside / inside / on line), pocket, V-carve, tabs, ramping, tool shapes, tool changes, units, work zero / Z zero, time estimate. |
| `src/bin/putia-gcode.rs` | Command line: `putia-gcode project.json [--imperial] [-o out.nc]` |
| `tests/golden.rs` | Golden tests against G-code produced by the browser app. |
| `tests/data/` | Eight projects exported from the browser app, with its G-code and time estimate. |

Not ported yet: SVG import, the 2D editor, the material-removal simulation and the 3D preview.

## Build and test

Needs Rust 1.75 or newer and a C++ compiler (Clipper is C++, built by the `cc` crate).

    cargo test
    cargo run --release --bin putia-gcode -- tests/data/tabs.project.json -o tabs.nc

If `cargo` fails with "Permission denied" running a build script, your project folder is on a
no-exec mount. Build elsewhere: `CARGO_TARGET_DIR=/tmp/putia-target cargo test`.

## Golden tests

Each scenario in `tests/data` has `NAME.project.json` (export from the browser app),
`NAME.expected.nc` (what the browser app generated) and `NAME.meta.json` (units, time estimate).
Five scenarios match line for line. The two with tiny rings in the centre of a circle
(V-carve, two operations) are compared geometrically: both Clipper ports keep slightly different
vertices on the same curve (up to about 0.1 mm), and the tests allow that. The imperial scenario
differs by 0.0001 in. on two lines because JavaScript's `Math.hypot` and Rust's `hypot` can differ in the last bit.

To regenerate or add scenarios, export a project from the app (Project card, export icon), and add
its G-code and a meta file in the same format.

## Notes

- Lengths are millimetres, feeds are mm/min, loop points are integers in 1/1000 mm (Clipper's scale).
- G-code number formatting reproduces JavaScript's `toFixed` and `String(number)` exactly.
- Offsets that collapse a shape are very slow in Clipper. Repeated offsets (pocket rings, V-carve rings)
  therefore offset the previous ring by one step instead of offsetting the original shape by a growing distance.
