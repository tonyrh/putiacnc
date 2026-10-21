# PutiaCNC

A CNC toolpath studio for GRBL machines, written in Rust. Draw or import shapes, assign operations
(profile, pocket, V-carve), preview the result in 2D and in a simulated 3D view of the machined
stock, and export G-code. It is a native port of the browser app and reads and writes the same
project files (`"app": "putia-cnc"`), so projects move between the two without conversion.

The workspace has two crates:

- **`putiacnc-core`** (this folder): project model, geometry, SVG import, the material-removal
  simulation and G-code generation. It has no GUI dependency and also builds a command-line tool.
- **`putiacnc-gui`** (`gui/`): the desktop app, built with egui/eframe (OpenGL).

## The desktop app

Features:

- 2D design canvas: rectangles, ellipses, lines and polygons, SVG import, select / move / resize,
  snapping, undo and redo, align to the stock and to each other, distribute.
- Operations: profile (outside, inside, on the line), pocket and V-carve, each with its own tool,
  depth, pass depth (defaults to the tool's value), cut direction, ramp-in, air cut and holding tabs
  (automatic or placed by hand on the canvas). Warns when the depth exceeds the stock thickness.
- Libraries for tools and machines (work area limits), with imperial and metric tools.
- 3D preview: GPU ray-marched height field of the stock with the toolpath, rapids and cutter
  marker, and a play bar to replay the job.
- G-code tab with a time estimate and warnings, optional output in inches; project notes.
- Unsaved-changes prompts, native file dialogs, light and dark themes.

### Build and run

Needs Rust 1.75 or newer and a C++ compiler (Clipper is C++, built by the `cc` crate). On Linux
the window needs the usual X11 or Wayland and OpenGL libraries.

    cargo run --release -p putiacnc-gui
    cargo run --release -p putiacnc-gui -- path/to/project.json

The binary is called `putiacnc`. Tool and machine libraries are stored per user:
`~/.config/putiacnc` on Linux (or `$XDG_CONFIG_HOME/putiacnc`), `%APPDATA%\PutiaCNC` on Windows,
`~/Library/Application Support/PutiaCNC` on macOS. Set `PUTIACNC_CONFIG_DIR` to use another folder.

The app icon is in `assets/` (`putiacnc.svg` plus PNGs from 16 to 512 px).

## Command line

    cargo run --release --bin putia-gcode -- project.json [--imperial] [-o out.nc]

---

# putiacnc-core

The calculation core of PutiaCNC, in Rust: project model, geometry and GRBL G-code generation.
It is a port of the browser app's `build()` and produces the same output for the same project.

## What's in it

| File | Purpose |
|---|---|
| `src/model.rs` | Data model (serde). Reads the project files the browser app exports (`"app": "putia-cnc"`), no conversion needed. |
| `src/clip.rs` | Safe wrapper around Clipper 6.4 (the same library and version the browser app uses): even-odd union, round-join offset, cleaning, area. |
| `src/toolpath.rs` | Profile (outside / inside / on line), pocket, V-carve, tabs, ramping, tool shapes, tool changes, units, work zero / Z zero, time estimate. |
| `src/svg.rs` | SVG import into closed and open shapes. |
| `src/sim.rs` | Height-field material-removal simulation used by the 3D preview. |
| `src/bin/putia-gcode.rs` | Command line: `putia-gcode project.json [--imperial] [-o out.nc]` |
| `tests/golden.rs` | Golden tests against G-code produced by the browser app. |
| `tests/data/` | Eight projects exported from the browser app, with its G-code and time estimate. |

Also in the core: SVG import (`src/svg.rs`), shape helpers (`src/shapes.rs`), tool and machine libraries (`src/toollib.rs`, `src/machines.rs`) and the height-field simulation behind the 3D preview (`src/sim.rs`).

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

## License

PutiaCNC is free software, released under the GNU General Public License v3.0 or later. See `LICENSE`.
