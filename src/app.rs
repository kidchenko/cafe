//! The tray icon, its menu, and the decision they feed: awake or not.
//!
//! Everything here runs on the event loop's thread. That is not just tidiness:
//! on Windows the keep-awake request belongs to the thread that made it (see
//! `awake.rs`), and this is the one thread guaranteed to live as long as the
//! process.
//!
//! The machine is kept awake when either
//! - the mode says so (indefinitely, or a timer that has not run out), or
//! - an app on the watch list is running,
//!
//! unless it is on battery below the configured floor, which overrides both.

use crate::awake::{Keeper, Level};
use crate::config::{Config, Mode};
use crate::icon::{self, Face};
use crate::log::{lerror, linfo, lwarn};
use crate::power::{self, Battery};
use crate::util::{
    duration_label, humanize, local_clock_at, local_seconds_of_day, next_whole_hours, now_unix,
};
use crate::{art, autostart, facts, notify, picker, procs, theme};

use std::time::{Duration, Instant, SystemTime};
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::accelerator::Accelerator;
use tray_icon::menu::{
    CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// "Keep awake" when off, "Turn off" otherwise: the same switch as a left
/// click on Windows or a second launch.
const ID_TOGGLE: &str = "toggle";
const ID_FOR: &str = "for:";
const ID_UNTIL: &str = "until:";
const ID_APP_ADD: &str = "app+:";
const ID_APP_REMOVE: &str = "app-:";
const ID_CHOOSE: &str = "choose-app";
const ID_EDIT: &str = "edit-config";
const ID_FLOOR: &str = "floor:";
const ID_SCREEN: &str = "screen";
const ID_NOTIFY: &str = "notify";
const ID_FACTS: &str = "facts";
const ID_FACT: &str = "fact";
const ID_AUTOSTART: &str = "autostart";
const ID_QUIT: &str = "quit";
/// Shared by every read-only row; nothing dispatches on it.
const ID_INERT: &str = "inert";

/// How often the loop wakes when nothing is due: often enough to keep the
/// countdown honest to the minute and to catch the battery crossing its floor,
/// rare enough to be invisible in Task Manager.
const TICK: Duration = Duration::from_secs(30);
/// Faster while apps are watched, so opening one is noticed promptly. A
/// process snapshot is a few hundred microseconds; this is still nothing.
const TICK_WATCHING: Duration = Duration::from_secs(10);
/// While a rebuild waits for the menu to close.
const MENU_RETRY: Duration = Duration::from_secs(1);

/// How many whole hours the timer offers after its durations.
const UNTIL_CHOICES: u32 = 6;
const FLOORS: [u8; 5] = [0, 10, 20, 30, 50];

const MACHINE: &str = if cfg!(target_os = "macos") {
    "Mac"
} else {
    "PC"
};

pub enum UserEvent {
    Menu(String),
    /// Left click on the tray icon (Windows) or a second launch of the app.
    Toggle,
}

#[derive(PartialEq, Eq)]
pub enum Flow {
    Continue,
    Exit,
}

pub struct App {
    cfg: Config,
    keeper: Keeper,
    tray: TrayIcon,
    dark_taskbar: bool,
    level: Level,

    // What the last look at the world found.
    battery: Option<Battery>,
    /// On battery below the floor.
    low: bool,
    /// The watched app currently holding the machine awake.
    trigger: Option<String>,
    /// Whether "battery low" has been announced, so it is said once per dip
    /// rather than every tick.
    low_announced: bool,
    /// How many facts past today's the menu has been clicked on to.
    fact_offset: usize,

    // What is on screen, so a tick only repaints what changed.
    face: Face,
    header: String,
    /// The status line at the top of the menu, kept so its text can change
    /// while the menu is open.
    header_item: Option<MenuItem>,
    /// A rebuild was put off because the menu was open.
    menu_stale: bool,
    menu_hour: u64,
    windowed: Vec<procs::App>,

    /// The config file's mtime as of our last read or write: anything newer is
    /// a hand edit.
    cfg_mtime: Option<SystemTime>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Result<Self, String> {
        forward_menu_events(&proxy);
        #[cfg(windows)]
        forward_tray_events(&proxy);

        let dark_taskbar = theme::taskbar_is_dark();
        let icon = tray_image(Face::Off, dark_taskbar)?;
        // On macOS the icon is a template: AppKit tints it for the menu bar.
        #[cfg(target_os = "macos")]
        let builder = TrayIconBuilder::new().with_icon_templated(icon);
        #[cfg(windows)]
        let builder = TrayIconBuilder::new().with_icon(icon);
        let tray = builder
            .with_tooltip("Cafe")
            // macOS menu bar items open their menu on click; that is what every
            // other one does. On Windows left click toggles and the menu lives
            // on the right button, like PowerToys Awake.
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .build()
            .map_err(|e| format!("could not create the tray icon: {e}"))?;

        let mut app = Self {
            cfg: Config::load(),
            keeper: Keeper::new(),
            tray,
            dark_taskbar,
            level: Level::Off,
            battery: None,
            low: false,
            trigger: None,
            low_announced: false,
            fact_offset: 0,
            face: Face::Off,
            header: String::new(),
            header_item: None,
            menu_stale: false,
            menu_hour: 0,
            windowed: Vec::new(),
            cfg_mtime: Config::modified(),
        };

        // A timed session that ran out while we were closed comes back off.
        if let Mode::Until { at } = app.cfg.mode {
            if at <= now_unix() {
                app.cfg.mode = Mode::Off;
            }
        }
        app.observe();
        app.apply(true);
        Ok(app)
    }

    pub fn handle(&mut self, event: UserEvent) -> Flow {
        match event {
            UserEvent::Toggle => {
                let next = match self.cfg.mode {
                    Mode::Off => Mode::Indefinite,
                    _ => Mode::Off,
                };
                self.set_mode(next);
            }
            UserEvent::Menu(id) => return self.menu(&id),
        }
        Flow::Continue
    }

    fn menu(&mut self, id: &str) -> Flow {
        match id {
            ID_QUIT => {
                linfo!("quit requested from the tray");
                return Flow::Exit;
            }
            ID_TOGGLE => return self.handle(UserEvent::Toggle),
            ID_SCREEN => {
                self.cfg.keep_screen_on = !self.cfg.keep_screen_on;
                self.apply(true);
            }
            ID_NOTIFY => {
                self.cfg.notify = !self.cfg.notify;
                self.apply(true);
            }
            ID_AUTOSTART => {
                let want = !autostart::is_enabled();
                match autostart::set(want) {
                    Ok(()) => linfo!("run at login {}", if want { "on" } else { "off" }),
                    Err(e) => lerror!("{e}"),
                }
                self.rebuild_menu();
            }
            ID_EDIT => open_config(),
            ID_CHOOSE => {
                if let Some(app) = picker::choose_app() {
                    self.watch(&app);
                }
            }
            ID_FACTS => {
                self.cfg.facts = !self.cfg.facts;
                self.apply(true);
            }
            ID_FACT => {
                // Asked for, so shown even with notifications off.
                notify::show("Did you know?", self.fact(), None);
                self.fact_offset += 1;
                self.rebuild_menu();
            }
            other => {
                if let Some(mins) = parse(other, ID_FOR) {
                    self.set_mode(Mode::Until {
                        at: now_unix() + mins * 60,
                    });
                } else if let Some(at) = parse(other, ID_UNTIL) {
                    // A choice that went stale while the menu sat open is
                    // ignored rather than turned into an instant expiry.
                    if at > now_unix() {
                        self.set_mode(Mode::Until { at });
                    }
                } else if let Some(floor) = parse(other, ID_FLOOR) {
                    self.cfg.battery_floor = floor as u8;
                    self.low_announced = false;
                    self.observe();
                    self.apply(true);
                } else if let Some(exe) = other.strip_prefix(ID_APP_ADD) {
                    if let Some(app) = self.windowed.iter().find(|a| a.exe == exe).cloned() {
                        self.watch(&app);
                    }
                } else if let Some(exe) = other.strip_prefix(ID_APP_REMOVE) {
                    linfo!("no longer watching {exe}");
                    self.cfg.unwatch(exe);
                    self.observe();
                    self.apply(true);
                }
            }
        }
        Flow::Continue
    }

    /// Called on every wake: end an expired timer, pick up a hand-edited
    /// config, look at the battery and the process list, and repaint whatever
    /// changed.
    pub fn tick(&mut self) {
        if let Mode::Until { at } = self.cfg.mode {
            if at <= now_unix() {
                linfo!("timed session ended");
                self.cfg.mode = Mode::Off;
                if self.cfg.notify {
                    notify::show(
                        "Time's up",
                        &format!("Your {MACHINE} can go to sleep again."),
                        None,
                    );
                }
                self.observe();
                self.apply(true);
                return;
            }
        }

        self.reload_if_edited();

        let mode = self.cfg.mode;
        self.observe();
        if self.cfg.mode != mode {
            // The battery floor ended a session.
            self.apply(true);
            return;
        }
        if self.wanted_level() != self.level {
            self.apply(false);
            return;
        }

        let dark = theme::taskbar_is_dark();
        if dark != self.dark_taskbar {
            self.dark_taskbar = dark;
            self.paint_icon(true);
        }
        let windowed = self.add_candidates();
        if self.menu_stale
            || local_seconds_of_day() / 3600 != self.menu_hour
            || windowed != self.windowed
        {
            self.rebuild_menu();
        } else {
            // The countdown changes every minute; that alone never needs a
            // new menu.
            self.update_header();
        }
    }

    /// When the event loop should next wake: the end of a timed session if
    /// that is sooner than the regular tick.
    pub fn next_wake(&self) -> Instant {
        // Check back soon for the menu closing, so it's never opened stale.
        let every = if self.menu_stale {
            MENU_RETRY
        } else if self.cfg.apps.is_empty() {
            TICK
        } else {
            TICK_WATCHING
        };
        let tick = Instant::now() + every;
        match self.cfg.mode {
            Mode::Until { at } => {
                let left = at.saturating_sub(now_unix());
                tick.min(Instant::now() + Duration::from_secs(left))
            }
            _ => tick,
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        let was_off = self.cfg.mode == Mode::Off;
        self.cfg.mode = mode;
        // Asking for a session while already under the floor is answered,
        // not silently undone on the next tick.
        self.low_announced = false;
        self.observe();
        if was_off && self.cfg.mode != Mode::Off {
            self.daily_fact();
        }
        self.apply(true);
    }

    fn fact(&self) -> &'static str {
        facts::for_day(facts::today(), self.fact_offset)
    }

    /// The once-a-day fact, on the first session started that day. Saved by
    /// the caller's `apply(true)`.
    fn daily_fact(&mut self) {
        let today = facts::today();
        if !self.cfg.facts || !self.cfg.notify || self.cfg.last_fact_day == today {
            return;
        }
        self.cfg.last_fact_day = today;
        notify::show("Did you know?", self.fact(), None);
    }

    /// Refresh battery and process state, and enforce the battery floor on
    /// the mode. Touches nothing outside `self`.
    fn observe(&mut self) {
        self.battery = power::battery();
        self.low = power::below_floor(self.battery, self.cfg.battery_floor);

        let running = if self.cfg.apps.is_empty() {
            None
        } else {
            procs::first_running(&self.cfg.apps, &procs::running()).map(str::to_string)
        };
        if running != self.trigger {
            match (&self.trigger, &running) {
                (_, Some(app)) => linfo!("{app} is running"),
                (Some(app), None) => linfo!("{app} is no longer running"),
                (None, None) => {}
            }
        }
        self.trigger = running;

        if !self.low {
            self.low_announced = false;
            return;
        }
        let would_be_awake = self.cfg.mode != Mode::Off || self.trigger.is_some();
        if self.cfg.mode != Mode::Off {
            linfo!(
                "battery below {}%: ending {:?}",
                self.cfg.battery_floor,
                self.cfg.mode
            );
            self.cfg.mode = Mode::Off;
        }
        if would_be_awake && !self.low_announced {
            self.low_announced = true;
            if self.cfg.notify {
                let percent = self.battery.map(|b| b.percent).unwrap_or(0);
                notify::show(
                    &format!("Battery at {percent}%"),
                    &format!("Cafe let go so your {MACHINE} can sleep and save power."),
                    None,
                );
            }
        }
    }

    fn awake(&self) -> bool {
        !self.low && (self.cfg.mode != Mode::Off || self.trigger.is_some())
    }

    fn wanted_level(&self) -> Level {
        match (self.awake(), self.cfg.keep_screen_on) {
            (false, _) => Level::Off,
            (true, true) => Level::SystemAndScreen,
            (true, false) => Level::System,
        }
    }

    /// Push the current state to the OS and the tray, and to disk if `save`.
    fn apply(&mut self, save: bool) {
        let level = self.wanted_level();
        match self.keeper.set(level) {
            Ok(()) => {
                if level != self.level {
                    linfo!(
                        "mode={:?} trigger={:?} level={:?}",
                        self.cfg.mode,
                        self.trigger,
                        level
                    );
                }
                self.level = level;
            }
            Err(e) => {
                lerror!("could not apply {level:?}: {e}");
                self.cfg.mode = Mode::Off;
            }
        }
        self.paint_icon(false);
        self.rebuild_menu();
        if save {
            match self.cfg.save() {
                Ok(()) => self.cfg_mtime = Config::modified(),
                Err(e) => lwarn!("could not save config: {e}"),
            }
        }
    }

    /// Pick up a hand edit of config.json ("Edit app list…"). The mode is
    /// ours, not the file's: a stale mode in an editor buffer must not
    /// restart a session the user just ended.
    fn reload_if_edited(&mut self) {
        let mtime = Config::modified();
        if mtime.is_none() || mtime == self.cfg_mtime {
            return;
        }
        self.cfg_mtime = mtime;
        if let Some(mut fresh) = Config::reload() {
            fresh.mode = self.cfg.mode;
            if fresh != self.cfg {
                linfo!("config edited on disk; apps={:?}", fresh.apps);
                self.cfg = fresh;
                self.rebuild_menu();
            }
        }
    }

    fn face(&self) -> Face {
        match self.wanted_level() {
            Level::Off => Face::Off,
            Level::System => Face::Awake,
            Level::SystemAndScreen => Face::AwakeScreen,
        }
    }

    fn paint_icon(&mut self, force: bool) {
        let face = self.face();
        if face == self.face && !force {
            return;
        }
        self.face = face;
        match tray_image(face, self.dark_taskbar) {
            Ok(img) => {
                #[cfg(target_os = "macos")]
                let _ = self.tray.set_icon_templated(Some(img));
                #[cfg(windows)]
                let _ = self.tray.set_icon(Some(img));
            }
            Err(e) => lerror!("{e}"),
        }
    }

    fn header_text(&self) -> String {
        if self.low {
            return format!("Off while the battery is under {}%", self.cfg.battery_floor);
        }
        let screen = if self.cfg.keep_screen_on {
            ", screen on"
        } else {
            ""
        };
        match (self.cfg.mode, &self.trigger) {
            (Mode::Off, None) => format!("Off, your {MACHINE} sleeps as usual"),
            (Mode::Off, Some(app)) => {
                format!("Awake while {} is open{screen}", self.cfg.app_label(app))
            }
            (Mode::Indefinite, _) => format!("Awake{screen}"),
            (Mode::Until { at }, _) => format!(
                "Awake until {} ({} left){screen}",
                local_clock_at(at),
                humanize(at.saturating_sub(now_unix()))
            ),
        }
    }

    fn watch(&mut self, app: &procs::App) {
        linfo!("watching {} ({})", app.exe, app.name);
        self.cfg.watch(app);
        self.observe();
        self.apply(true);
    }

    /// Windowed apps not already on the watch list.
    fn add_candidates(&self) -> Vec<procs::App> {
        let watched: Vec<String> = self.cfg.apps.iter().map(|a| procs::normalise(a)).collect();
        procs::windowed()
            .into_iter()
            .filter(|a| !watched.contains(&procs::normalise(&a.exe)))
            .collect()
    }

    /// The status line and tooltip, changed in place.
    fn update_header(&mut self) {
        let header = self.header_text();
        if header == self.header {
            return;
        }
        if let Some(item) = &self.header_item {
            item.set_text(&header);
        }
        let _ = self.tray.set_tooltip(Some(format!("Cafe\n{header}")));
        self.header = header;
    }

    /// Build the menu afresh. Replacing a menu that is open closes it on
    /// macOS, so while it's open only the status line changes, and the
    /// rebuild happens once it closes.
    fn rebuild_menu(&mut self) {
        if menu_is_open() {
            self.update_header();
            self.menu_stale = true;
            return;
        }
        self.menu_stale = false;
        self.header = self.header_text();
        let sod = local_seconds_of_day();
        self.menu_hour = sod / 3600;
        self.windowed = self.add_candidates();
        let _ = self
            .tray
            .set_tooltip(Some(format!("Cafe\n{}", self.header)));

        let menu = Menu::new();
        let header = MenuItem::with_id(ID_INERT, &self.header, false, None);
        let _ = menu.append(&header);
        self.header_item = Some(header);
        let _ = menu.append(&PredefinedMenuItem::separator());

        // Actions first, then the one setting people flip often, then the
        // rest out of the way under Settings.
        let (toggle, glyph) = if self.cfg.mode == Mode::Off {
            ("Keep awake", "coffee")
        } else {
            ("Turn off", "circle-stop")
        };
        let _ = menu.append(&icon_item(ID_TOGGLE, toggle, glyph, None));
        let _ = menu.append(&self.timer_menu(sod));
        let _ = menu.append(&self.apps_menu());

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_SCREEN,
            "Keep screen on",
            true,
            self.cfg.keep_screen_on,
            None,
        ));
        let _ = menu.append(&self.settings_menu());
        let _ = menu.append(&PredefinedMenuItem::separator());
        if self.cfg.facts {
            let _ = menu.append(&icon_item(ID_FACT, self.fact(), "lightbulb", None));
        }
        let _ = menu.append(&icon_item(ID_QUIT, "Quit Cafe", "power", shortcut('q')));

        self.tray.set_menu(Some(Box::new(menu)));
    }

    /// Durations, then the next few whole hours. The status line at the top
    /// already says when a running timer ends.
    fn timer_menu(&self, sod: u64) -> Submenu {
        let sub = icon_submenu("Timer", "timer", true);
        for &mins in &self.cfg.durations {
            let _ = sub.append(&MenuItem::with_id(
                format!("{ID_FOR}{mins}"),
                duration_label(mins),
                true,
                None,
            ));
        }
        let _ = sub.append(&PredefinedMenuItem::separator());
        for (at, label) in next_whole_hours(now_unix(), sod, UNTIL_CHOICES) {
            let _ = sub.append(&MenuItem::with_id(
                format!("{ID_UNTIL}{at}"),
                format!("Until {label}"),
                true,
                None,
            ));
        }
        sub
    }

    /// Things set once and left alone.
    fn settings_menu(&self) -> Submenu {
        let sub = icon_submenu("Settings", "settings", true);
        let _ = sub.append(&self.battery_menu());
        let _ = sub.append(&CheckMenuItem::with_id(
            ID_NOTIFY,
            "Notifications",
            true,
            self.cfg.notify,
            None,
        ));
        let _ = sub.append(&CheckMenuItem::with_id(
            ID_FACTS,
            "Coffee facts",
            true,
            self.cfg.facts,
            None,
        ));
        let _ = sub.append(&CheckMenuItem::with_id(
            ID_AUTOSTART,
            "Run at login",
            true,
            autostart::is_enabled(),
            None,
        ));
        let _ = sub.append(&PredefinedMenuItem::separator());
        let _ = sub.append(&icon_item(
            ID_EDIT,
            "Edit settings file\u{2026}",
            "file-cog",
            shortcut(','),
        ));
        let _ = sub.append(&PredefinedMenuItem::separator());
        let _ = sub.append(&MenuItem::with_id(
            ID_INERT,
            concat!("Cafe ", env!("CARGO_PKG_VERSION")),
            false,
            None,
        ));
        sub
    }

    /// Watched apps (checked; clicking one removes it), then the windowed apps
    /// that could be added, then the file picker for anything else, like an
    /// app that isn't open or a build tool.
    fn apps_menu(&self) -> Submenu {
        let sub = icon_submenu("While an app is open", "app-window", true);
        if self.cfg.apps.is_empty() {
            let _ = sub.append(&MenuItem::with_id(ID_INERT, "No apps yet", false, None));
        }
        for app in &self.cfg.apps {
            let running = self.trigger.as_deref() == Some(app.as_str());
            let name = self.cfg.app_label(app);
            let label = if running {
                format!("{name} (open now)")
            } else {
                name
            };
            let _ = sub.append(&CheckMenuItem::with_id(
                format!("{ID_APP_REMOVE}{app}"),
                label,
                true,
                true,
                None,
            ));
        }
        let _ = sub.append(&PredefinedMenuItem::separator());

        let add = Submenu::new("Add an open app", !self.windowed.is_empty());
        for app in &self.windowed {
            let _ = add.append(&MenuItem::with_id(
                format!("{ID_APP_ADD}{}", app.exe),
                &app.name,
                true,
                None,
            ));
        }
        let _ = sub.append(&add);
        let _ = sub.append(&icon_item(
            ID_CHOOSE,
            "Choose an app\u{2026}",
            "folder-open",
            None,
        ));
        sub
    }

    fn battery_menu(&self) -> Submenu {
        let has_battery = self.battery.is_some();
        let title = match self.battery {
            Some(b) => format!("Stop on battery below ({}% now)", b.percent),
            None => "Stop on battery below".to_string(),
        };
        let sub = icon_submenu(title, "battery-low", has_battery);
        for floor in FLOORS {
            let label = if floor == 0 {
                "Never".to_string()
            } else {
                format!("{floor}%")
            };
            let _ = sub.append(&CheckMenuItem::with_id(
                format!("{ID_FLOOR}{floor}"),
                label,
                true,
                self.cfg.battery_floor == floor,
                None,
            ));
        }
        sub
    }
}

/// Whether a menu is open right now. On macOS the run loop is in event
/// tracking mode for exactly as long as a menu is being shown.
#[cfg(target_os = "macos")]
fn menu_is_open() -> bool {
    use objc2_app_kit::NSEventTrackingRunLoopMode;
    use objc2_foundation::NSRunLoop;
    let mode = NSRunLoop::currentRunLoop().currentMode();
    mode.is_some_and(|m| *m == *unsafe { NSEventTrackingRunLoopMode })
}

/// Windows has no such signal. Its tray menu runs a modal loop of its own,
/// and this problem hasn't been seen there.
#[cfg(windows)]
fn menu_is_open() -> bool {
    false
}

/// A menu row with a Lucide glyph beside it.
fn icon_item(id: &str, text: &str, glyph: &str, accel: Option<Accelerator>) -> IconMenuItem {
    let item = IconMenuItem::with_id(id, text, true, None, accel);
    #[cfg(target_os = "macos")]
    item.set_icon_templated(art::glyph(glyph));
    #[cfg(windows)]
    item.set_icon(art::glyph(glyph));
    item
}

/// A submenu with a Lucide glyph beside it.
fn icon_submenu(text: impl AsRef<str>, glyph: &str, enabled: bool) -> Submenu {
    let sub = Submenu::new(text, enabled);
    #[cfg(target_os = "macos")]
    sub.set_icon_templated(art::glyph(glyph));
    #[cfg(windows)]
    sub.set_icon(art::glyph(glyph));
    sub
}

/// ⌘ and a key, on macOS. Windows tray menus can't take keyboard shortcuts.
fn shortcut(key: char) -> Option<Accelerator> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    use tray_icon::menu::accelerator::{Code, Modifiers};
    let code = match key {
        'q' => Code::KeyQ,
        ',' => Code::Comma,
        _ => return None,
    };
    Some(Accelerator::new(Modifiers::META, code))
}

fn parse(id: &str, prefix: &str) -> Option<u64> {
    id.strip_prefix(prefix)?.parse().ok()
}

/// Open config.json in a text editor, creating it first if need be.
fn open_config() {
    let Some(path) = Config::path() else { return };
    if !path.exists() {
        let _ = Config::load().save();
    }
    // Notepad / TextEdit rather than the .json association, which on many
    // machines is a browser or nothing at all.
    #[cfg(windows)]
    let spawned = std::process::Command::new("notepad.exe").arg(&path).spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("/usr/bin/open")
        .arg("-t")
        .arg(&path)
        .spawn();
    if let Err(e) = spawned {
        lwarn!("could not open {}: {e}", path.display());
    }
}

fn tray_image(face: Face, dark: bool) -> Result<Icon, String> {
    Icon::from_rgba(icon::render(face, dark, icon::EDGE), icon::EDGE, icon::EDGE)
        .map_err(|e| format!("could not build the tray image: {e}"))
}

fn forward_menu_events(proxy: &EventLoopProxy<UserEvent>) {
    let proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(e.id.0));
    }));
}

#[cfg(windows)]
fn forward_tray_events(proxy: &EventLoopProxy<UserEvent>) {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
    let proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        // `Click` fires for press and release; acting on both would toggle
        // twice and appear to do nothing.
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            let _ = proxy.send_event(UserEvent::Toggle);
        }
    }));
}
