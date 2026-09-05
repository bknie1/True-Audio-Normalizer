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
    movie_item: CheckMenuItem,
    music_item: CheckMenuItem,
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
        // Platform-appropriate capture list.
        #[cfg(target_os = "windows")]
        let (capture_names, loopback, capture_title) =
            (tan_live::list_outputs(), true, "Capture (what's playing)");
        #[cfg(not(target_os = "windows"))]
        let (capture_names, loopback, capture_title) =
            (tan_live::list_inputs(), false, "Capture (input / monitor)");

        let output_names = tan_live::list_outputs();

        let cfg = EngineConfig {
            loopback,
            capture: None,
            output: None,
            profile: ProfileKind::Movie,
            latency_ms: 200,
        };

        let header = MenuItem::with_id(MenuId::new("header"), "TAN - True Audio Normalizer", false, None);
        let enabled_item = CheckMenuItem::with_id(MenuId::new("enabled"), "Enabled", true, true, None);

        let movie_item = CheckMenuItem::with_id(MenuId::new("profile:movie"), "Movie", true, true, None);
        let music_item = CheckMenuItem::with_id(MenuId::new("profile:music"), "Music", true, false, None);
        let profile_menu = Submenu::with_items(
            "Profile",
            true,
            &[&movie_item, &music_item],
        )
        .expect("profile submenu");

        // Capture device submenu: a "System default" entry plus each device.
        let mut capture_items: Vec<CheckMenuItem> = Vec::new();
        let mut capture_specs: Vec<Option<String>> = Vec::new();
        capture_items.push(CheckMenuItem::with_id(MenuId::new("cap:default"), "System default", true, true, None));
        capture_specs.push(None);
        for (i, name) in capture_names.iter().enumerate() {
            capture_items.push(CheckMenuItem::with_id(MenuId::new(format!("cap:{i}")), name, true, false, None));
            capture_specs.push(Some(i.to_string()));
        }
        let capture_menu = Submenu::with_items(
            capture_title,
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
            output_specs.push(Some(i.to_string()));
        }
        let output_menu = Submenu::with_items(
            "Output (play TAN to)",
            true,
            &output_items.iter().map(|i| i as &dyn tray_icon::menu::IsMenuItem).collect::<Vec<_>>(),
        )
        .expect("output submenu");

        let copy_item = MenuItem::with_id(MenuId::new("copy"), "Copy diagnostics to clipboard", true, None);
        let quit_item = MenuItem::with_id(MenuId::new("quit"), "Quit", true, None);

        let menu = Menu::new();
        menu.append_items(&[
            &header,
            &PredefinedMenuItem::separator(),
            &enabled_item,
            &profile_menu,
            &capture_menu,
            &output_menu,
            &PredefinedMenuItem::separator(),
            &copy_item,
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
            movie_item,
            music_item,
            last_status: String::new(),
            capture_items,
            capture_specs,
            output_items,
            output_specs,
        };
        app.apply();
        app
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
            "enabled" => {
                self.enabled = self.enabled_item.is_checked();
                self.apply();
            }
            "profile:movie" => {
                self.cfg.profile = ProfileKind::Movie;
                self.movie_item.set_checked(true);
                self.music_item.set_checked(false);
                self.apply();
            }
            "profile:music" => {
                self.cfg.profile = ProfileKind::Music;
                self.movie_item.set_checked(false);
                self.music_item.set_checked(true);
                self.apply();
            }
            other if other.starts_with("cap:") => {
                if let Some(idx) = self.capture_items.iter().position(|it| it.id().0 == other) {
                    for (i, it) in self.capture_items.iter().enumerate() {
                        it.set_checked(i == idx);
                    }
                    self.cfg.capture = self.capture_specs[idx].clone();
                    self.apply();
                }
            }
            other if other.starts_with("out:") => {
                if let Some(idx) = self.output_items.iter().position(|it| it.id().0 == other) {
                    for (i, it) in self.output_items.iter().enumerate() {
                        it.set_checked(i == idx);
                    }
                    self.cfg.output = self.output_specs[idx].clone();
                    self.apply();
                }
            }
            _ => {}
        }
    }
}

/// A simple solid TAN-orange tray icon, generated so there's no asset to ship.
fn make_icon() -> Icon {
    let (w, h) = (32u32, 32u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..(w * h) {
        rgba.extend_from_slice(&[0xe0, 0x76, 0x4a, 0xff]); // #e0764a
    }
    Icon::from_rgba(rgba, w, h).expect("valid icon")
}
