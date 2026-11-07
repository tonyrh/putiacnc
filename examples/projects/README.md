# Example projects

Open them with File → Open in PutiaCNC.

- `oval-sign.project.json`: an oval plate (260 × 170 × 15 mm wood). "PutiaCNC" is V-carved in the middle (60° V-bit), and "toolpath studio" and "free open source software" follow the inside of the oval (30° engraving bit). A fine border line is engraved, and the oval is cut out last with four holding tabs. The text is stored as outlines, so it looks the same without the fonts (FreeSerif Bold and Poppins Light); `cargo run --release --example ovalsign` rebuilds it.
