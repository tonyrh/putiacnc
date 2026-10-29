# Testing

## Automated

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs both on every push to `main` and every pull request.

- **Unit tests** sit next to the code they test, in both crates.
- **Golden tests** (`tests/golden.rs`): eight projects in `tests/data` are turned into G-code and compared with
  snapshots of this generator's own reviewed output (coordinates within 0.0011 mm, same lines, same time estimate).
  They catch unintended changes. After a change you meant to make, run `UPDATE_SNAPSHOTS=1 cargo test --test golden`,
  read `git diff tests/data` by eye, and commit what you accept. Whether the G-code is *right* is the job of
  `verify_programs.rs`, which re-reads the text with an independent checker.
- **Replay** (`tests/replay.rs`, `tests/common`): the G-code text is played back the way the 3D preview does it (steps with their line, arcs cut into short pieces, stops, waits, tools) and held to what the generator says: cutting length, time, end point, the passes and their lowest Z, the second tool, the levelling lap. Every random project goes through the same check.
- **Random projects** (`tests/random_projects.rs`): projects built from a seed (stock, tools, one to three operations
  of every kind including inlay, shapes with holes, a random machine) go through the generator and the independent
  checker, in mm and inches, with arcs on and off. A project must not panic, must pass the checker, must agree with the
  generator on cutting length and depth, must end at safe Z over X0 Y0, and must not cut below the stock without a note.
  20 seeds run with the normal tests; `cargo test --release --test random_projects -- --ignored --nocapture`
  runs 5000 (`COUNT=n` to change). A failure prints its seed and writes the project to the temp folder; `SEED=n` with
  the `one_seed` test (ignored) shows it in detail.
- **Feature tests** in `tests/`: `air.rs` (air cuts), `arcs.rs`, `checks_*.rs` (the toolpath checks), `lead.rs`,
  `limits.rs`, `relief.rs`, `tabs.rs`.
- Hostile values (`tests/hostile.rs`): takes the sample projects and replaces numbers anywhere in them (zero, negative, tiny, 1e9, 1e300, a string, null), as a damaged or hand-edited file could. Whatever loads must never panic, abort, hang (30 s) or write NaN/inf into the G-code or the times, and the same project must give the same program twice and after save and load. 150 cases run with `cargo test`; `COUNT=3000 SEED=1000 cargo test --release --test hostile -- --ignored` runs more (3000 take about 35 s). The first runs found: a C++ exception from Clipper that aborted the whole program for points beyond 4.6e18, a 24 GB allocation from a pass depth of 1e-9, a 11 GB cutter table for a 1000 km tool, an integer overflow in number rounding, and infinite times from a machine speed of 0; each is a plain test in the same file.
- Random actions (`gui/src/fuzz.rs`): random sequences of what a person does in the window (draw, delete, duplicate, offset, combine, group, array, corners, clean up, align, add and remove operations, change the stock, undo, redo, toolbar undo) with checks after every step: no selection, tab, group or operation reference left pointing at something that is gone; the toolpath still generates with finite numbers; a frame of the whole window draws without a panic and without egui reporting two widgets with one ID; and undoing every step gives back the first project, redoing the same number gives back the last. A failing sequence is shrunk to the few steps needed. `cargo test fuzz` runs 6; `COUNT=500 SEED=1 cargo test --release fuzz -- --ignored` runs more (500 take about 2 minutes; a project that takes over 5 s to generate prints a `SLOW` line, and with `FUZZ_DUMP=/some/folder` also saves it there; this is how the 83-second offset below was found). It found: Clipper's offset taking 83 s on 19,000 points from many overlapping copies (the offset now cleans its input when it is big: 1 s), Edit > Redo bringing a shape back into an operation that had been removed, and the toolbar undo taking back a whole step that changed both drawing and project (a later menu undo then put the removed operation back).
- **GUI tests** (`gui/src`) build an `App` without a window and drive it: undo scopes, groups, operations keeping
  their settings, the background generator.

Anything that decides something should be testable without a window. See `ARCHITECTURE.md`.

## Looking at the window without a screen

`scripts/shot.sh` starts a virtual screen (once), opens the app in a given state in a throwaway config folder, waits,
takes the picture and closes the app. It needs `xvfb` and `imagemagick`, and a built `putiacnc`
(`cargo build -p putiacnc-gui`).

```
scripts/shot.sh -o 2d.png tests/data/outside_holes.project.json
scripts/shot.sh -t dark -e PUTIACNC_VIEW=3d -e PUTIACNC_PLAY=0.6 -c 396,44,1084,300 -o 3d.png my.project.json
scripts/shot.sh -w 1354x800 -e PUTIACNC_SHOW_OFFSET=1 -o smallest.png my.project.json
scripts/shot.sh -h          # all the options
```

`-c X,Y,W,H` keeps one part of the screen (the canvas starts at x = 394, below the tab strip at y = 44); `-e` sets the
variables below, and can be repeated. The same variables work when you start the program by hand:
`PUTIACNC_WIN=1920x985 PUTIACNC_THEME=dark cargo run -p putiacnc-gui -- project.json`.

| Variable | Effect |
|---|---|
| `PUTIACNC_CONFIG_DIR` | Where preferences and libraries are read and written. `shot.sh` uses a throwaway folder. |
| `PUTIACNC_WIN=WxH` | Window size (the smallest allowed is about 1354 wide). |
| `PUTIACNC_THEME=light\|dark` | Overrides the desktop theme. |
| `PUTIACNC_VIEW=3d` | Opens the 3D preview. |
| `PUTIACNC_PLAY=0..1` (or `-n`: at the n-th stop for the operator), `PUTIACNC_SPEED=n`, `PUTIACNC_T_FILE=path` | Playback position, speed index, or a file whose number sets the time. |
| `PUTIACNC_PART=n`, `PUTIACNC_GCODE_PART=n` | The 3D view or the G-code tab shows plug piece n (1 = the first; 0 = the main stock). |
| `PUTIACNC_ZOOM=f` | Scales the 3D camera distance. |
| `PUTIACNC_HIDE=left,right,machine,toolpaths` | Hides the side panels, the machine area outline, or the toolpath lines (any mix). |
| `PUTIACNC_SELECT=1,4,7` / `PUTIACNC_SELECT_ALL` | Selects shapes by index (no expansion to their group). |
| `PUTIACNC_SHOW_*` (`ABOUT`, `ALIGN`, `ARRAY`, `CHECKS`, `CLEAN`, `COMBINE`, `CONFIRM`, `CORNERS`, `ERR`, `MACHINES`, `OFFSET`, `SHORTCUTS`, `TEXT`, `TOOLS`, `UPDATE`) | Opens that dialog or popover. |
| `PUTIACNC_SPEED_UNIT=n` | The speed unit of the tool library window (and of the custom feeds block), with the lengths view it belongs to (0-2 mm, 3-5 inches): 0 mm/min, 1 mm/s, 2 m/min, 3 in/min, 4 in/s, 5 ft/min. |
| `PUTIACNC_TAB_GCODE`, `PUTIACNC_TAB_NOTES`, `PUTIACNC_COLLAPSE=1,2` | Right panel tab; folds operations by number. |
| `PUTIACNC_IMPORT2=file`, `PUTIACNC_TRACE_MODE=centre` | Opens a bitmap in the Import bitmap dialog (centre line instead of outlines with the second). |
| `PUTIACNC_OP=n` | Operation n (by position) is active with its shapes selected; all the others are folded. |
| `PUTIACNC_NODE_EDIT=i`, `PUTIACNC_TAB_EDIT`, `PUTIACNC_ROTATE` | Starts point editing, tab placing or rotate mode. |
| `PUTIACNC_TAB_EDIT=1` | The first operation gets manual tabs and is in tab placement (shows the Done button). |
| `PUTIACNC_DESELECT` | Nothing is selected, after the `PUTIACNC_SHOW_*` tool windows are opened (to see their empty state). |
| `PUTIACNC_SHOW_ERR[=warning|info]` | Shows a sample message in the banner: an error by default, or a warning or information. |
| `PUTIACNC_DROPUP=snap_menu\|guides_menu` | Opens that drop-up menu of the bottom bar. |
| `PUTIACNC_APPLY_NOTHING=array\|combine\|corners` | Opens that tool on a case where Apply changes nothing and presses Apply once, to see where its message goes. |
| `PUTIACNC_PLACE_TEXT=..`, `PUTIACNC_EDIT_TEXT`, `PUTIACNC_POLYGON=ring\|point`, `PUTIACNC_DYN=..`, `PUTIACNC_IMPORT2=file`, `PUTIACNC_RELIEF`, `PUTIACNC_TOOL_QUERY=..` | Sets up a tool mid-use. |
| `PUTIACNC_CHECK_SHOW=n`, `PUTIACNC_DEBUG_CHECKS` | Shows check result `n` in the 3D view; prints a summary of the checks. |
| `PUTIACNC_PRINT_VERSION_INFO` | Prints the "Copy system info" text and carries on. |
| `PUTIACNC_UPDATE_URL` | Where the update check looks (point it at a local file to test). |

### Sweeping every state, size and theme

`scripts/shot-sweep.py` takes a picture of about 23 states (the design with and without a selection, folded operations, tabs, the 3D
preview in three moments, the G-code and Notes tabs, the banners, every dialog, hidden panels) at the smallest window size and at
1500x900, in the light theme (`--full`: also 1920x985 and the dark theme, about 15 minutes), and checks each picture by itself: the
window is drawn, the panels and the canvas are not empty, nothing is drawn outside the window, and egui's red duplicate-ID marker is
not on screen. It also compares every picture with the baseline of an earlier run (`shot-sweep/baseline`, not in git) and lists the ones
where more than 0.3 % of the pixels changed, with a before | after | changes picture in `shot-sweep/diff/`. Run it with `--update` when
the program looks the way you want, and again after a change to the layout: only what moved shows up. The status bar is left out of the
comparison (it prints the generation time). A difference is where to look, not a failure; a layout that looks wrong in a way the checks
cannot see still needs your eyes.

Things to know:
- Shift and Ctrl clicks are hard to send through Xvfb, so use `PUTIACNC_SELECT` instead.
- The toolpath checks need up to a minute on a big project before their markers appear.
- Anything that needs a real GPU, a trackpad (pinch-zoom) or macOS cursors cannot be tested this way.

The `Screenshots` workflow (started by hand from the Actions tab) does the same on Linux, macOS and Windows.

`tests/verify_programs.rs` reads the generated G-code back with `verify::trace` (see `src/verify.rs`) for every sample project in many variants and fails on any warning or when the cutting length or depth read from the text differs from the generator's own figures. Add a variant there when a new option changes the text.

## Before a release

1. Open a real project, generate the G-code, and look at the 3D preview from start to finish.
2. Cut something small with every operation type that changed since the last release, on the real machine.
3. For anything near clamps or tool changes, check the first move of each operation in a sender with the spindle off.
