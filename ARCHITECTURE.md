# How PutiaCNC is put together

Two Rust crates in one workspace.

| Crate | Folder | What it is |
|---|---|---|
| `putiacnc-core` | `src/` | Everything that is not a window: the project model, geometry, toolpaths, G-code, checks, undo history, selection rules. Binary `putia-gcode` (project JSON in, GRBL G-code out). |
| `putiacnc-gui` | `gui/src/` | The egui/eframe window. Binary `putiacnc`. |

The rule that keeps this manageable: **anything that decides something lives in the core crate, with a test.**
The GUI draws, reads the mouse and keyboard, and calls core functions. If you find yourself writing an `if` that
changes the project inside a drawing function, it probably belongs in core.

## Data

- A project is `ProjectFile { data: ProjectData, .. }` (`src/model.rs`), saved as JSON. The structs mirror the file
  format of the old browser app, so old projects still load.
- Lengths are millimetres; feeds are mm/min. Loop points are integers in 1/1000 mm.
- `ProjectData` holds the stock, `loops` (every drawn shape), `groups` (placed text and imports), `ops` (operations),
  `tools`, `clamps`, the optional 3D `relief`, and the machine.
- Two kinds of group exist on a shape (`LoopData`): `grp` (a placed text or an import) and `ug` (made with Edit > Group).
  They are selected, moved and copied as one unit; `selection::unit_of` is the single place that decides this.

## Core crate (`src/`)

| Module | Job |
|---|---|
| `model` | The data structures above, with serde defaults for old files. |
| `shapes` | Making and changing shapes, per-operation defaults, `add_operation` / `add_inlay`, `GroupCopies` (fresh group ids when copying), tabs bookkeeping. |
| `clip`, `arcs`, `trim`, `cleanup` | Polygon offsetting and booleans (Clipper), arc fitting, the Trim tool, the Clean up command. |
| `svg`, `dxf`, `bitmap`, `text` | Importers and the font-to-outline code. |
| `toolpath` | `generate(&ProjectData, Units) -> Output`: profile, pocket, V-carve, drill, inlay, tabs, lead-in, G-code lines and a time estimate. |
| `relief` | The 3D model roughing and finishing passes. |
| `sim` | Walks the toolpath to give the 3D preview its material removal and timing. |
| `checks` | Reads the generated toolpath and reports what would go wrong: clamp collisions, depth beyond the stock, and so on. |
| `history` | Undo and redo as project snapshots, each tagged Drawing / Project / Both, so the toolbar and the Edit menu can step differently. |
| `selection` | Selection units, group and ungroup. |
| `machines`, `toollib` | Machine definitions and the tool library. |

## GUI crate (`gui/src/`)

`main.rs` owns the window and the frame loop: it lays out the panels and calls into the modules below.
`app/` is the shared state (`App`) and what changes it.

| Module | Job |
|---|---|
| `app/mod.rs` | The `App` struct, the drag states, `Mode`, and the tests that exercise `App` as a whole. |
| `app/session.rs`, `files.rs`, `generate.rs`, `edit.rs` | New / load / preferences; opening, importing, saving, exporting; background toolpath generation and the checks; delete, duplicate, offset, combine, undo steps, snapping. |
| `canvas/` | The 2D drawing: `mod.rs` (the big `canvas()` function and keys), `palette.rs` (tool palette, options bar, zoom card), `handles.rs`, `tabs.rs`, `nodes.rs`, `trim.rs`, `rulers.rs`, `draw.rs`. |
| `panels/` | `menu.rs`, `bottom.rs` (the bottom bar), `dialogs.rs` (status bar, confirm), `project.rs` (project and operations panels), `stock.rs` (stock and clamps cards). |
| `view3d.rs`, `viewcube.rs`, `preview.rs`, `model3d.rs` | The 3D preview, its cube card, and the mesh code. |
| `zoomcard.rs` | The zoom card (2D and 3D). |
| `text.rs`, `array.rs`, `offset.rs`, `combine.rs`, `corners.rs`, `align.rs`, `trace.rs`, `cleanup.rs` | The dialogs and popovers of the tools of the same name. |
| `toolmgr.rs`, `machmgr.rs`, `toolpick.rs` | Tool and machine managers, the tool picker. |
| `checkui.rs`, `osnap.rs`, `dynin.rs` | Showing check results, object snap, typing exact values while drawing. |
| `icons.rs`, `theme.rs`, `widgets.rs`, `csd.rs`, `about.rs`, `update.rs`, `prefs.rs`, `shortcuts.rs` | Drawn icons, colours, shared widgets, the window header, About, update check, preferences, the shortcut list. |

## How a change reaches the machine

1. An edit records a snapshot in `App.hist`, changes `project.data`, and calls `App::touch()`.
2. `touch()` marks the toolpath stale. `generate.rs` starts a worker thread that runs `toolpath::generate`
   on a copy of the project; only the latest request wins, and a result for an old project is dropped.
3. When the result arrives, `checks::run` is started in another thread on the same data.
4. The G-code panel, the 3D preview and Export all use that `Output`. Saving or exporting never uses an old one.

## Adding a feature

- Put the decision in core with a unit test (`#[cfg(test)]` in the module, or a file in `tests/`).
- If it changes G-code, add or update a golden test (`tests/golden.rs`, see `docs/testing.md`).
- Keep the window code to drawing and input. New dialogs get their own file in `gui/src/`.
- Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` (CI does both).
