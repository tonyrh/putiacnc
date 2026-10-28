# Testing

## Automated

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs both on every push to `main` and every pull request.

- **Unit tests** sit next to the code they test, in both crates.
- **Golden tests** (`tests/golden.rs`): ten projects in `tests/data` are turned into G-code and compared with the
  output of the original browser app, line by line (coordinates within a small tolerance, same line count, same
  time estimate). They pin the toolpath maths. If one fails after a change you meant to make, check the diff by
  eye before touching the expected file.
- **Feature tests** in `tests/`: `air.rs` (air cuts), `arcs.rs`, `checks_*.rs` (the toolpath checks), `lead.rs`,
  `limits.rs`, `relief.rs`, `tabs.rs`.
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
| `PUTIACNC_PLAY=0..1`, `PUTIACNC_SPEED=n`, `PUTIACNC_T_FILE=path` | Playback position, speed index, or a file whose number sets the time. |
| `PUTIACNC_PART=n`, `PUTIACNC_GCODE_PART=n` | The 3D view or the G-code tab shows plug piece n (1 = the first; 0 = the main stock). |
| `PUTIACNC_ZOOM=f` | Scales the 3D camera distance. |
| `PUTIACNC_HIDE=left,right,machine,toolpaths` | Hides the side panels, the machine area outline, or the toolpath lines (any mix). |
| `PUTIACNC_SELECT=1,4,7` / `PUTIACNC_SELECT_ALL` | Selects shapes by index (no expansion to their group). |
| `PUTIACNC_SHOW_*` (`ABOUT`, `ALIGN`, `ARRAY`, `CHECKS`, `CLEAN`, `COMBINE`, `CONFIRM`, `CORNERS`, `ERR`, `MACHINES`, `OFFSET`, `SHORTCUTS`, `TEXT`, `TOOLS`, `UPDATE`) | Opens that dialog or popover. |
| `PUTIACNC_SPEED_UNIT=n` | The speed unit of the tool library window (and of the custom feeds block): 0 mm/min, 1 mm/s, 2 m/min, 3 in/min, 4 in/s, 5 ft/min. |
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

Things to know:
- Shift and Ctrl clicks are hard to send through Xvfb, so use `PUTIACNC_SELECT` instead.
- The toolpath checks need up to a minute on a big project before their markers appear.
- Anything that needs a real GPU, a trackpad (pinch-zoom) or macOS cursors cannot be tested this way.

The `Screenshots` workflow (started by hand from the Actions tab) does the same on Linux, macOS and Windows.

## Before a release

1. Open a real project, generate the G-code, and look at the 3D preview from start to finish.
2. Cut something small with every operation type that changed since the last release, on the real machine.
3. For anything near clamps or tool changes, check the first move of each operation in a sender with the spindle off.
