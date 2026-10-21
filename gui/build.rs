// Embeds the app icon in the Windows executable (Explorer, taskbar, Alt-Tab).
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../assets/putiacnc.ico");
        res.set("ProductName", "PutiaCNC");
        res.set("FileDescription", "PutiaCNC");
        if let Err(e) = res.compile() { println!("cargo:warning=could not embed the icon: {e}"); }
    }
    println!("cargo:rerun-if-changed=../assets/putiacnc.ico");
}
