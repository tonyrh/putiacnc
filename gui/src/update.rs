//! Update check: asks GitHub for the latest release and, when it is newer than this build,
//! offers to download it. An AppImage can replace itself; other builds open the release page.
//! Nothing is contacted unless the user chooses "Check for updates" (or turns on the startup check).
use crate::app::App;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

const REPO: &str = "tonyrh/putiacnc";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    pub notes: String,
    /// Download for this platform: (file name, url).
    pub asset: Option<(String, String)>,
}

pub enum Status {
    Idle,
    Checking,
    UpToDate,
    NoReleases,
    Available(Release),
    Downloading(Release),
    Installed(Release),
    Failed(String),
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<(), String>),
}

pub struct Updater {
    pub status: Status,
    pub open: bool,
    pub auto: bool,
    rx: Option<Receiver<Msg>>,
    /// Startup check: stay quiet unless there is something to show.
    silent: bool,
}

impl Updater {
    pub fn new() -> Self {
        Updater { status: Status::Idle, open: false, auto: load_auto(), rx: None, silent: false }
    }

    pub fn check(&mut self, silent: bool) {
        if matches!(self.status, Status::Checking | Status::Downloading(_)) { return; }
        self.status = Status::Checking;
        self.silent = silent;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || { let _ = tx.send(Msg::Checked(fetch_latest())); });
    }

    fn download(&mut self, r: Release) {
        let Some((_, url)) = r.asset.clone() else { return };
        let Some(target) = self_update_target() else { return };
        self.status = Status::Downloading(r);
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || { let _ = tx.send(Msg::Downloaded(install(&url, &target))); });
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(Msg::Checked(res)) => {
                self.rx = None;
                self.status = match res {
                    Ok(None) => Status::NoReleases,
                    Ok(Some(r)) if is_newer(&r.version, CURRENT) => { self.open = true; Status::Available(r) }
                    Ok(Some(_)) => Status::UpToDate,
                    Err(e) => { if !self.silent { self.open = true; } Status::Failed(e) }
                };
                ctx.request_repaint();
            }
            Ok(Msg::Downloaded(res)) => {
                self.rx = None;
                let rel = match std::mem::replace(&mut self.status, Status::Idle) { Status::Downloading(r) => Some(r), _ => None };
                self.status = match (res, rel) {
                    (Ok(()), Some(r)) => Status::Installed(r),
                    (Err(e), _) => Status::Failed(e),
                    _ => Status::Idle,
                };
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(150)),
            Err(TryRecvError::Disconnected) => { self.rx = None; self.status = Status::Failed("The update check stopped unexpectedly.".into()); }
        }
    }
}

// ---------------------------------------------------------------------------- settings

fn settings_path() -> Option<PathBuf> { crate::toolmgr::config_dir().map(|d| d.join("settings.json")) }

fn load_auto() -> bool {
    settings_path().and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("update_check").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

fn save_auto(on: bool) {
    if let Some(p) = settings_path() {
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        let _ = std::fs::write(p, serde_json::json!({ "update_check": on }).to_string());
    }
}

// ---------------------------------------------------------------------------- network

fn agent(read_secs: u64) -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| format!("TLS is not available: {e}"))?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(read_secs))
        .user_agent(&format!("PutiaCNC/{CURRENT}"))
        .build())
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = std::env::var("PUTIACNC_UPDATE_URL").unwrap_or_else(|_| format!("https://api.github.com/repos/{REPO}/releases/latest"));
    match agent(20)?.get(&url).set("Accept", "application/vnd.github+json").call() {
        Ok(resp) => {
            let body = resp.into_string().map_err(|e| format!("Could not read the answer: {e}"))?;
            parse_release(&body).map(Some)
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(403, _)) | Err(ureq::Error::Status(429, _)) => Err("GitHub is limiting requests right now. Try again later.".into()),
        Err(ureq::Error::Status(c, _)) => Err(format!("GitHub answered with an error ({c}).")),
        Err(e) => Err(format!("Could not reach GitHub: {e}")),
    }
}

/// Path of the running AppImage, when this is one (the only build that can replace itself).
fn self_update_target() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") { return None; }
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file())
}

pub fn can_self_update(r: &Release) -> bool {
    self_update_target().is_some() && r.asset.as_ref().map_or(false, |(n, _)| n.ends_with(".AppImage"))
}

fn install(url: &str, target: &std::path::Path) -> Result<(), String> {
    use std::io::Read;
    let resp = agent(120)?.get(url).call().map_err(|e| format!("Download failed: {e}"))?;
    let tmp = target.with_extension("AppImage.new");
    let res = (|| -> Result<(), String> {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("Cannot write next to the AppImage: {e}"))?;
        let n = std::io::copy(&mut resp.into_reader().take(500 * 1024 * 1024), &mut f).map_err(|e| format!("Download failed: {e}"))?;
        if n < 1024 * 1024 { return Err("The downloaded file is too small to be the app.".into()); }
        drop(f);
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?; }
        std::fs::rename(&tmp, target).map_err(|e| format!("Could not replace the AppImage: {e}"))
    })();
    if res.is_err() { let _ = std::fs::remove_file(&tmp); }
    res
}

// ---------------------------------------------------------------------------- pure helpers

fn version_parts(s: &str) -> Vec<u64> {
    s.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).map_while(|p| p.parse::<u64>().ok()).collect()
}

/// True when `a` is a higher version than `b` ("v0.2.0" > "0.1.9").
pub fn is_newer(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (version_parts(a), version_parts(b));
    let n = x.len().max(y.len());
    x.resize(n, 0); y.resize(n, 0);
    x > y
}

pub fn pick_asset(assets: &[(String, String)], os: &str) -> Option<(String, String)> {
    let suffixes: &[&str] = match os { "windows" => &[".msi", ".exe", ".zip"], "macos" => &[".dmg", ".zip"], _ => &[".AppImage"] };
    suffixes.iter().find_map(|s| assets.iter().find(|(n, _)| n.to_lowercase().ends_with(&s.to_lowercase()))).cloned()
}

pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| "GitHub sent an answer I could not read.".to_string())?;
    let tag = v.get("tag_name").and_then(|t| t.as_str()).ok_or("The latest release has no version tag.")?;
    let assets: Vec<(String, String)> = v.get("assets").and_then(|a| a.as_array()).map(|a| {
        a.iter().filter_map(|x| Some((x.get("name")?.as_str()?.to_string(), x.get("browser_download_url")?.as_str()?.to_string()))).collect()
    }).unwrap_or_default();
    Ok(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: v.get("html_url").and_then(|u| u.as_str()).map(String::from).unwrap_or_else(|| format!("https://github.com/{REPO}/releases")),
        notes: v.get("body").and_then(|b| b.as_str()).unwrap_or("").trim().to_string(),
        asset: pick_asset(&assets, std::env::consts::OS),
    })
}

// ---------------------------------------------------------------------------- window

pub fn window(ctx: &egui::Context, app: &mut App) {
    let up = &mut app.updater;
    if !up.open { return; }
    let mut open = true;
    let (mut do_check, mut do_download, mut close) = (false, None::<Release>, false);
    let mut auto = up.auto;
    egui::Window::new("Check for updates").open(&mut open).collapsible(false).resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([420.0, 0.0]).show(ctx, |ui| {
        ui.label(format!("You are using PutiaCNC {CURRENT}."));
        ui.add_space(6.0);
        match &up.status {
            Status::Idle => { ui.weak("Not checked yet."); }
            Status::Checking => { ui.horizontal(|ui| { ui.spinner(); ui.label("Checking for updates…"); }); }
            Status::UpToDate => { ui.label("You have the latest version."); }
            Status::NoReleases => { ui.label("No releases have been published yet."); }
            Status::Failed(e) => { ui.colored_label(egui::Color32::from_rgb(200, 90, 40), e.as_str()); }
            Status::Downloading(r) => { ui.horizontal(|ui| { ui.spinner(); ui.label(format!("Downloading {}…", r.version)); }); }
            Status::Installed(r) => { ui.label(format!("Version {} is installed. Restart PutiaCNC to use it.", r.version)); }
            Status::Available(r) => {
                ui.strong(format!("Version {} is available.", r.version));
                if !r.notes.is_empty() {
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| { ui.add(egui::Label::new(r.notes.as_str()).wrap(true)); });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if can_self_update(r) {
                        if ui.button("Download and install").on_hover_text("Replaces this AppImage with the new version").clicked() { do_download = Some(r.clone()); }
                    } else if let Some((name, url)) = &r.asset {
                        if ui.button("Download").on_hover_text(name.as_str()).clicked() { let _ = open::that(url); }
                    }
                    if ui.button("Release page").clicked() { let _ = open::that(&r.page); }
                });
            }
        }
        ui.add_space(8.0);
        ui.separator();
        ui.checkbox(&mut auto, "Check for updates when PutiaCNC starts");
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let busy = matches!(up.status, Status::Checking | Status::Downloading(_));
            if ui.add_enabled(!busy, egui::Button::new("Check again")).clicked() { do_check = true; }
            if ui.button("Close").clicked() { close = true; }
        });
    });
    if auto != up.auto { up.auto = auto; save_auto(auto); }
    if do_check { up.check(false); }
    if let Some(r) = do_download { up.download(r); }
    if !open || close { up.open = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1"));
    }

    #[test]
    fn release_json() {
        let j = r#"{"tag_name":"v0.2.0","html_url":"https://github.com/x/y/releases/tag/v0.2.0","body":" Fixes \n","assets":[
            {"name":"PutiaCNC-0.2.0-x86_64.AppImage","browser_download_url":"https://e/a.AppImage"},
            {"name":"PutiaCNC-0.2.0.zip","browser_download_url":"https://e/a.zip"}]}"#;
        let r = parse_release(j).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.notes, "Fixes");
        let a = [("a.zip".to_string(), "z".to_string()), ("b.AppImage".to_string(), "i".to_string())];
        assert_eq!(pick_asset(&a, "linux").unwrap().1, "i");
        assert_eq!(pick_asset(&a, "windows").unwrap().1, "z");
        assert!(pick_asset(&a, "macos").is_some());
        assert!(parse_release("{}").is_err());
        assert!(parse_release("nope").is_err());
    }

    /// Serves `body` once on a local port and returns its URL.
    fn serve(body: Vec<u8>) -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/f.AppImage", l.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
        url
    }

    #[test]
    fn install_replaces_the_file() {
        let dir = std::env::temp_dir().join(format!("putia-upd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("PutiaCNC.AppImage");
        std::fs::write(&target, b"old").unwrap();
        install(&serve(vec![7u8; 2 * 1024 * 1024]), &target).unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().len(), 2 * 1024 * 1024);
        assert!(!dir.join("PutiaCNC.AppImage.new").exists());
        // too small a file is refused and the old one is kept
        std::fs::write(&target, b"old").unwrap();
        assert!(install(&serve(vec![1u8; 100]), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
