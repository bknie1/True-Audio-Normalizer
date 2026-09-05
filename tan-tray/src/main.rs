//! TAN in the system tray: a small cross-platform (Windows / macOS / Linux)
//! menu-bar app around the shared live engine in `tan_live`. Toggle TAN on and
//! off, pick the capture and playback devices, and switch the movie/music
//! profile - all without a terminal.
//!
//! Capturing "what's playing" differs by platform: on Windows we use WASAPI
//! loopback on an output device; on macOS and Linux there is no loopback, so
//! the capture list is input devices and you point it at a monitor / virtual
//! source (a PulseAudio/PipeWire "Monitor of ..." on Linux, or a loopback
//! device like BlackHole on macOS). The menu adapts its capture list to match.
//!
//! Hide the console window on Windows release builds (this is a tray app).
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use tan_live::{start, EngineConfig, ProfileKind, RunningEngine};

fn main() {
    let event_loop = EventLoopBuilder::new().build();
    let menu_rx = MenuEvent::receiver();
    let mut app: Option<App> = None;

    event_loop.run(move |event, _target, control_flow| {
        // Wake periodically to drain the tray's menu-event channel; it does not
        // wake the tao loop on its own.
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(150));

        // Build the tray after the event loop has initialized - required on
        // macOS/Linux, harmless on Windows.
        if let Event::NewEvents(StartCause::Init) = event {
            app = Some(App::new());
        }

        if let Some(app) = app.as_mut() {
            while let Ok(ev) = menu_rx.try_recv() {
                app.on_menu(&ev.id, control_flow);
            }
        }
    });
}

struct App {
    tray: TrayIcon,
    cfg: EngineConfig,
    engine: Option<RunningEngine>,
    enabled: bool,

    enabled_item: CheckMenuItem,
    profile_items: Vec<CheckMenuItem>,
    profile_kinds: Vec<ProfileKind>,
    /// Last engine wiring or error, included in copied diagnostics.
    last_status: String,

    // Parallel arrays: menu item, and the device spec it selects (None = default).
    capture_items: Vec<CheckMenuItem>,
    capture_specs: Vec<Option<String>>,
    output_items: Vec<CheckMenuItem>,
    output_specs: Vec<Option<String>>,
}

impl App {
    fn new() -> Self {
        // Platform-appropriate capture list: Windows captures an output device
        // via loopback; macOS/Linux capture an input/monitor source.
        #[cfg(target_os = "windows")]
        let (capture_names, loopback) = (tan_live::list_outputs(), true);
        #[cfg(not(target_os = "windows"))]
        let (capture_names, loopback) = (tan_live::list_inputs(), false);

        let output_names = tan_live::list_outputs();

        let cfg = EngineConfig {
            loopback,
            capture: None,
            output: None,
            profile: ProfileKind::Universal,
            latency_ms: 200,
        };

        let header = MenuItem::with_id(MenuId::new("header"), "TAN - True Audio Normalizer", false, None);
        let enabled_item = CheckMenuItem::with_id(MenuId::new("enabled"), "Enabled", true, true, None);

        // Profile submenu built from the engine's list, so adding a preset in
        // tan-live surfaces here automatically. Universal is the default.
        let profile_kinds: Vec<ProfileKind> = ProfileKind::all().to_vec();
        let profile_items: Vec<CheckMenuItem> = profile_kinds
            .iter()
            .map(|p| {
                CheckMenuItem::with_id(
                    MenuId::new(format!("profile:{}", p.key())),
                    p.label(),
                    true,
                    *p == cfg.profile,
                    None,
                )
            })
            .collect();
        let profile_menu = Submenu::with_items(
            "Profile",
            true,
            &profile_items.iter().map(|i| i as &dyn tray_icon::menu::IsMenuItem).collect::<Vec<_>>(),
        )
        .expect("profile submenu");

        // Capture device submenu: a "System default" entry plus each device.
        let mut capture_items: Vec<CheckMenuItem> = Vec::new();
        let mut capture_specs: Vec<Option<String>> = Vec::new();
        capture_items.push(CheckMenuItem::with_id(MenuId::new("cap:default"), "System default", true, true, None));
        capture_specs.push(None);
        for (i, name) in capture_names.iter().enumerate() {
            capture_items.push(CheckMenuItem::with_id(MenuId::new(format!("cap:{i}")), name, true, false, None));
            // Store the device NAME (resolve() matches by substring); stable
            // across restarts and reordering, unlike an index.
            capture_specs.push(Some(name.clone()));
        }
        let capture_menu = Submenu::with_items(
            "Input",
            true,
            &capture_items.iter().map(|i| i as &dyn tray_icon::menu::IsMenuItem).collect::<Vec<_>>(),
        )
        .expect("capture submenu");

        // Output device submenu.
        let mut output_items: Vec<CheckMenuItem> = Vec::new();
        let mut output_specs: Vec<Option<String>> = Vec::new();
        output_items.push(CheckMenuItem::with_id(MenuId::new("out:default"), "System default", true, true, None));
        output_specs.push(None);
        for (i, name) in output_names.iter().enumerate() {
            output_items.push(CheckMenuItem::with_id(MenuId::new(format!("out:{i}")), name, true, false, None));
            output_specs.push(Some(name.clone()));
        }
        let output_menu = Submenu::with_items(
            "Output",
            true,
            &output_items.iter().map(|i| i as &dyn tray_icon::menu::IsMenuItem).collect::<Vec<_>>(),
        )
        .expect("output submenu");

        let export_item = MenuItem::with_id(MenuId::new("export"), "Export settings...", true, None);
        let import_item = MenuItem::with_id(MenuId::new("import"), "Import settings...", true, None);
        let copy_item = MenuItem::with_id(MenuId::new("copy"), "Copy diagnostics", true, None);
        let quit_item = MenuItem::with_id(MenuId::new("quit"), "Quit", true, None);

        let settings_menu = Submenu::with_items(
            "Settings",
            true,
            &[&export_item, &import_item, &copy_item],
        )
        .expect("settings submenu");

        let menu = Menu::new();
        menu.append_items(&[
            &header,
            &PredefinedMenuItem::separator(),
            &enabled_item,
            &profile_menu,
            &capture_menu,
            &output_menu,
            &PredefinedMenuItem::separator(),
            &settings_menu,
            &quit_item,
        ])
        .expect("build menu");

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("TAN - starting...")
            .with_icon(make_icon())
            .build()
            .expect("build tray icon");

        let mut app = App {
            tray,
            cfg,
            engine: None,
            enabled: true,
            enabled_item,
            profile_items,
            profile_kinds,
            last_status: String::new(),
            capture_items,
            capture_specs,
            output_items,
            output_specs,
        };
        app.restore_saved(); // reapply last session's choices before starting
        app.apply();
        app
    }

    /// Load the last-used settings and reflect them in the menu, without
    /// starting the engine (the caller does that).
    fn restore_saved(&mut self) {
        let saved = load_saved();
        self.apply_saved(saved);
    }

    /// Apply a parsed settings set to state and the menu (no engine restart).
    /// Missing/absent devices fall back to System default; `loopback` is
    /// platform-fixed and never restored.
    fn apply_saved(&mut self, saved: Saved) {
        if let Some(p) = saved.profile {
            if let Some(kind) = ProfileKind::from_key(&p) {
                self.cfg.profile = kind;
            }
        }
        self.sync_profile_checks();

        if let Some(cap) = saved.capture {
            let spec = if cap.is_empty() { None } else { Some(cap) };
            let idx = self.capture_specs.iter().position(|s| *s == spec).unwrap_or(0);
            self.cfg.capture = self.capture_specs[idx].clone();
            for (i, it) in self.capture_items.iter().enumerate() {
                it.set_checked(i == idx);
            }
        }
        if let Some(out) = saved.output {
            let spec = if out.is_empty() { None } else { Some(out) };
            let idx = self.output_specs.iter().position(|s| *s == spec).unwrap_or(0);
            self.cfg.output = self.output_specs[idx].clone();
            for (i, it) in self.output_items.iter().enumerate() {
                it.set_checked(i == idx);
            }
        }
        if let Some(en) = saved.enabled {
            self.enabled = en;
            self.enabled_item.set_checked(en);
        }
    }

    /// Tick the menu item for the active profile, untick the rest.
    fn sync_profile_checks(&self) {
        for (item, kind) in self.profile_items.iter().zip(self.profile_kinds.iter()) {
            item.set_checked(*kind == self.cfg.profile);
        }
    }

    /// The settings serialized as `key=value` text (used for save and export).
    fn config_body(&self) -> String {
        format!(
            "profile={}\nloopback={}\ncapture={}\noutput={}\nenabled={}\n",
            self.cfg.profile.key(),
            self.cfg.loopback,
            self.cfg.capture.clone().unwrap_or_default(),
            self.cfg.output.clone().unwrap_or_default(),
            self.enabled,
        )
    }

    /// Persist the current settings so the next launch restores them.
    fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, self.config_body());
    }

    /// Export the current settings to a file the user picks.
    fn export_settings(&self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("tan-settings.txt")
            .add_filter("TAN settings", &["txt"])
            .save_file()
        {
            match std::fs::write(&path, self.config_body()) {
                Ok(()) => self.set_tooltip(&format!("TAN - settings exported to {}", path.display())),
                Err(e) => self.set_tooltip(&format!("TAN - export failed: {e}")),
            }
        }
    }

    /// Import settings from a file the user picks, then apply and start.
    fn import_settings(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TAN settings", &["txt"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.apply_saved(parse_saved(&text));
                    self.save();
                    self.apply();
                }
                Err(e) => self.set_tooltip(&format!("TAN - import failed: {e}")),
            }
        }
    }

    /// (Re)start or stop the engine to match current state, and reflect the
    /// result in the tray tooltip.
    fn apply(&mut self) {
        self.engine = None; // dropping the handle stops both audio streams
        if !self.enabled {
            self.last_status = "paused".to_string();
            self.set_tooltip("TAN - paused");
            return;
        }
        match start(&self.cfg) {
            Ok(engine) => {
                let info = engine.info.clone();
                self.engine = Some(engine);
                self.last_status = format!("on: {info}");
                self.set_tooltip(&format!("TAN - on\n{info}"));
            }
            Err(err) => {
                self.enabled = false;
                self.enabled_item.set_checked(false);
                self.last_status = format!("error: {err}");
                self.set_tooltip(&format!("TAN - error\n{err}"));
                eprintln!("TAN could not start: {err}");
            }
        }
    }

    /// Assemble a diagnostics report and put it on the clipboard.
    fn copy_diagnostics(&mut self) {
        let report = format!(
            "{}\nprofile: {}\nloopback: {}\ncapture spec: {:?}\noutput spec: {:?}\nlast status: {}\n\n{}",
            "== TAN tray diagnostics ==",
            self.cfg.profile.label(),
            self.cfg.loopback,
            self.cfg.capture,
            self.cfg.output,
            self.last_status,
            tan_live::diagnostics(),
        );
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(report)) {
            Ok(()) => self.set_tooltip("TAN - diagnostics copied to clipboard"),
            Err(e) => self.set_tooltip(&format!("TAN - clipboard error: {e}")),
        }
    }

    fn set_tooltip(&self, text: &str) {
        let _ = self.tray.set_tooltip(Some(text));
    }

    fn on_menu(&mut self, id: &MenuId, control_flow: &mut ControlFlow) {
        let id = id.0.as_str();
        match id {
            "quit" => {
                self.engine = None;
                *control_flow = ControlFlow::Exit;
            }
            "copy" => {
                self.copy_diagnostics();
            }
            "export" => {
                self.export_settings();
            }
            "import" => {
                self.import_settings();
            }
            "enabled" => {
                self.enabled = self.enabled_item.is_checked();
                self.apply();
                self.save();
            }
            other if other.starts_with("profile:") => {
                let key = &other["profile:".len()..];
                if let Some(kind) = ProfileKind::from_key(key) {
                    self.cfg.profile = kind;
                    self.sync_profile_checks();
                    self.apply();
                    self.save();
                }
            }
            other if other.starts_with("cap:") => {
                if let Some(idx) = self.capture_items.iter().position(|it| it.id().0 == other) {
                    for (i, it) in self.capture_items.iter().enumerate() {
                        it.set_checked(i == idx);
                    }
                    self.cfg.capture = self.capture_specs[idx].clone();
                    self.apply();
                    self.save();
                }
            }
            other if other.starts_with("out:") => {
                if let Some(idx) = self.output_items.iter().position(|it| it.id().0 == other) {
                    for (i, it) in self.output_items.iter().enumerate() {
                        it.set_checked(i == idx);
                    }
                    self.cfg.output = self.output_specs[idx].clone();
                    self.apply();
                    self.save();
                }
            }
            _ => {}
        }
    }
}

/// Where the tray remembers its settings: %APPDATA%\TAN\config.txt on Windows,
/// $XDG_CONFIG_HOME/TAN or ~/.config/TAN elsewhere.
fn config_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("TAN").join("config.txt"))
}

#[derive(Default)]
struct Saved {
    profile: Option<String>,
    capture: Option<String>,
    output: Option<String>,
    enabled: Option<bool>,
}

/// Load and parse the config file; absent or unreadable is fine (defaults).
fn load_saved() -> Saved {
    let Some(path) = config_path() else { return Saved::default() };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_saved(&text),
        Err(_) => Saved::default(),
    }
}

/// Parse the simple `key=value` config text.
fn parse_saved(text: &str) -> Saved {
    let mut s = Saved::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "profile" => s.profile = Some(v),
            "capture" => s.capture = Some(v),
            "output" => s.output = Some(v),
            "enabled" => s.enabled = Some(v == "true"),
            _ => {}
        }
    }
    s
}

/// The TAN badge icon, drawn in code from the same geometry as docs/icon.svg
/// (a rounded orange badge with seven cream waveform bars) so there's no asset
/// to ship and no rasterizer dependency. Rendered at 64x64 with a little edge
/// feathering; the tray scales it down.
fn make_icon() -> Icon {
    let (rgba, s) = icon_rgba();
    Icon::from_rgba(rgba, s, s).expect("valid icon")
}

fn icon_rgba() -> (Vec<u8>, u32) {
    const S: usize = 64;
    let k = S as f32 / 128.0; // the SVG uses a 0..128 viewBox

    // Badge gradient (SVG #badge): #c86a3d -> #9c451f, top-left to bottom-right.
    let g0 = [0xc8u8, 0x6a, 0x3d];
    let g1 = [0x9cu8, 0x45, 0x1f];
    // Bars: (x-center, y-top, y-bottom, color) in 128-space; stroke-width 8.
    let bars: [(f32, f32, f32, [u8; 3]); 7] = [
        (22.0, 58.0, 70.0, [0xff, 0xd1, 0x66]),
        (36.0, 34.0, 94.0, [0xff, 0xdd, 0x8c]),
        (50.0, 50.0, 78.0, [0xff, 0xe4, 0xa8]),
        (64.0, 24.0, 104.0, [0xff, 0xec, 0xc4]),
        (78.0, 48.0, 80.0, [0xfb, 0xee, 0xe0]),
        (92.0, 42.0, 86.0, [0xf7, 0xf6, 0xf3]),
        (106.0, 44.0, 84.0, [0xf7, 0xf6, 0xf3]),
    ];
    let radius = 26.0 * k; // corner radius
    let hw = 4.0 * k; // bar half-width (stroke-width 8 / 2)
    let edge = S as f32;

    // Signed coverage of a rounded-rect at (px,py): 1 inside, 0 outside, with a
    // ~1px feathered border.
    let rrect_cov = |px: f32, py: f32| -> f32 {
        let inset = 0.5;
        let (x0, y0, x1, y1) = (inset, inset, edge - inset, edge - inset);
        // distance outside the rounded rect (0 if inside)
        let cx = px.clamp(x0 + radius, x1 - radius);
        let cy = py.clamp(y0 + radius, y1 - radius);
        let dx = (px - cx).abs().max(0.0);
        let dy = (py - cy).abs().max(0.0);
        // inside the straight edges?
        let outside = if px < x0 || px > x1 || py < y0 || py > y1 {
            // in a corner zone, use radial distance
            (((dx).powi(2) + (dy).powi(2)).sqrt() - radius).max(0.0)
        } else {
            let corner = ((dx).powi(2) + (dy).powi(2)).sqrt() - radius;
            corner.max(0.0)
        };
        (1.0 - outside).clamp(0.0, 1.0)
    };

    // Distance from a point to a vertical segment (for capsule-shaped bars).
    let seg_dist = |px: f32, py: f32, cx: f32, top: f32, bot: f32| -> f32 {
        let dx = px - cx;
        if py < top {
            (dx * dx + (py - top) * (py - top)).sqrt()
        } else if py > bot {
            (dx * dx + (py - bot) * (py - bot)).sqrt()
        } else {
            dx.abs()
        }
    };

    let mut rgba = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let cov = rrect_cov(px, py);
            if cov <= 0.0 {
                continue; // transparent outside the badge
            }
            // Base badge gradient along the diagonal.
            let t = ((px + py) / (2.0 * edge)).clamp(0.0, 1.0);
            let mut col = [
                (g0[0] as f32 + (g1[0] as f32 - g0[0] as f32) * t) as u8,
                (g0[1] as f32 + (g1[1] as f32 - g0[1] as f32) * t) as u8,
                (g0[2] as f32 + (g1[2] as f32 - g0[2] as f32) * t) as u8,
            ];
            // Overlay bars (nearest-covered wins), with soft edges.
            for &(bx, bt, bb, bc) in &bars {
                let d = seg_dist(px, py, bx * k, bt * k, bb * k);
                let a = (hw + 0.5 - d).clamp(0.0, 1.0); // 1 inside, feathered edge
                if a > 0.0 {
                    col = [
                        (col[0] as f32 * (1.0 - a) + bc[0] as f32 * a) as u8,
                        (col[1] as f32 * (1.0 - a) + bc[1] as f32 * a) as u8,
                        (col[2] as f32 * (1.0 - a) + bc[2] as f32 * a) as u8,
                    ];
                }
            }
            let i = (y * S + x) * 4;
            rgba[i] = col[0];
            rgba[i + 1] = col[1];
            rgba[i + 2] = col[2];
            rgba[i + 3] = (cov * 255.0) as u8;
        }
    }
    (rgba, S as u32)
}

#[cfg(test)]
mod tests {
    use super::{icon_rgba, parse_saved};

    #[test]
    fn parses_config_and_empty_means_default_device() {
        let s = parse_saved(
            "profile=music\nloopback=true\ncapture=Sonar - Gaming\noutput=\nenabled=false\n",
        );
        assert_eq!(s.profile.as_deref(), Some("music"));
        assert_eq!(s.capture.as_deref(), Some("Sonar - Gaming"));
        assert_eq!(s.output.as_deref(), Some("")); // empty -> System default on restore
        assert_eq!(s.enabled, Some(false));
    }

    #[test]
    fn missing_keys_stay_none() {
        let s = parse_saved("profile=movie\n# a comment line\ngarbage\n");
        assert_eq!(s.profile.as_deref(), Some("movie"));
        assert!(s.capture.is_none());
        assert!(s.enabled.is_none());
    }

    #[test]
    fn icon_is_shaped_not_a_square() {
        let (rgba, s) = icon_rgba();
        let s = s as usize;
        let at = |x: usize, y: usize| {
            let i = (y * s + x) * 4;
            (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3])
        };
        // Corner is outside the rounded badge -> transparent.
        assert_eq!(at(0, 0).3, 0, "top-left corner should be transparent");
        // Center is inside the badge -> opaque.
        assert_eq!(at(s / 2, s / 2).3, 255, "center should be opaque");
        // The tall middle bar (#ffecc4) crosses the center column; expect a
        // light, cream-ish pixel there, not the dark badge orange.
        let (r, g, b, a) = at(s / 2, s / 2);
        assert_eq!(a, 255);
        assert!(r > 200 && g > 180 && b > 150, "center bar should be cream, got {r},{g},{b}");
    }
}
