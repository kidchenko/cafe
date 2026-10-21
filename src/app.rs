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
use crate::{autostart, facts, notify, procs, theme};

use std::time::{Duration, Instant, SystemTime};
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const ID_OFF: &str = "off";
const ID_INDEFINITE: &str = "indefinite";
const ID_FOR: &str = "for:";
const ID_UNTIL: &str = "until:";
const ID_APP_ADD: &str = "app+:";
const ID_APP_REMOVE: &str = "app-:";
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

/// How many whole hours "Keep awake until" offers.
const UNTIL_CHOICES: u32 = 8;
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
    menu_hour: u64,
    windowed: Vec<String>,

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
        let tray = TrayIconBuilder::new()
            .with_tooltip("Cafe")
            .with_icon(tray_image(Face::Off, dark_taskbar)?)
            // macOS menu bar items open their menu on click; that is what every
            // other one does. On Windows left click toggles and the menu lives
            // on the right button, like PowerToys Awake.
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .with_icon_as_template(cfg!(target_os = "macos"))
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
            ID_OFF => self.set_mode(Mode::Off),
            ID_INDEFINITE => self.set_mode(Mode::Indefinite),
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
                } else if let Some(name) = other.strip_prefix(ID_APP_ADD) {
                    linfo!("watching {name}");
                    self.cfg.apps.push(name.to_string());
                    self.observe();
                    self.apply(true);
                } else if let Some(name) = other.strip_prefix(ID_APP_REMOVE) {
                    linfo!("no longer watching {name}");
                    self.cfg.apps.retain(|a| a != name);
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
                        "Keep-awake ended",
                        "Your timer ran out. Your normal power plan applies again.",
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
        if self.header_text() != self.header
            || local_seconds_of_day() / 3600 != self.menu_hour
            || windowed != self.windowed
        {
            self.rebuild_menu();
        }
    }

    /// When the event loop should next wake: the end of a timed session if
    /// that is sooner than the regular tick.
    pub fn next_wake(&self) -> Instant {
        let every = if self.cfg.apps.is_empty() {
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
                    &format!(
                        "Cafe won't keep your {MACHINE} awake on battery below {}%.",
                        self.cfg.battery_floor
                    ),
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
                let _ = self.tray.set_icon(Some(img));
                #[cfg(target_os = "macos")]
                self.tray.set_icon_as_template(true);
            }
            Err(e) => lerror!("{e}"),
        }
    }

    fn header_text(&self) -> String {
        if self.low {
            return format!("Off \u{2014} battery below {}%", self.cfg.battery_floor);
        }
        let screen = if self.cfg.keep_screen_on {
            ", screen on"
        } else {
            ""
        };
        match (self.cfg.mode, &self.trigger) {
            (Mode::Off, None) => "Off \u{2014} normal power plan".into(),
            (Mode::Off, Some(app)) => format!("Awake{screen} while {app} runs"),
            (Mode::Indefinite, _) => format!("Awake{screen}"),
            (Mode::Until { at }, _) => format!(
                "Awake{screen} until {} \u{2014} {} left",
                local_clock_at(at),
                humanize(at.saturating_sub(now_unix()))
            ),
        }
    }

    /// Windowed apps not already on the watch list.
    fn add_candidates(&self) -> Vec<String> {
        let watched: Vec<String> = self.cfg.apps.iter().map(|a| procs::normalise(a)).collect();
        procs::windowed()
            .into_iter()
            .filter(|n| !watched.contains(&procs::normalise(n)))
            .collect()
    }

    fn rebuild_menu(&mut self) {
        self.header = self.header_text();
        let sod = local_seconds_of_day();
        self.menu_hour = sod / 3600;
        self.windowed = self.add_candidates();
        let _ = self
            .tray
            .set_tooltip(Some(format!("Cafe \u{2014} {}", self.header)));

        let menu = Menu::new();
        let _ = menu.append(&MenuItem::with_id(ID_INERT, &self.header, false, None));
        let _ = menu.append(&PredefinedMenuItem::separator());

        let mode = self.cfg.mode;
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_OFF,
            "Off (use power plan)",
            true,
            mode == Mode::Off,
            None,
        ));
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_INDEFINITE,
            "Keep awake indefinitely",
            true,
            mode == Mode::Indefinite,
            None,
        ));

        let durations = Submenu::new("Keep awake for", true);
        // A check on the submenu itself would be ideal but is not something
        // menus offer; a check item inside it says "you are on a timer".
        if matches!(mode, Mode::Until { .. }) {
            let _ = durations.append_items(&[
                &CheckMenuItem::with_id(ID_INERT, "Timer running", false, true, None),
                &PredefinedMenuItem::separator(),
            ]);
        }
        for &mins in &self.cfg.durations {
            let _ = durations.append(&MenuItem::with_id(
                format!("{ID_FOR}{mins}"),
                duration_label(mins),
                true,
                None,
            ));
        }
        let _ = menu.append(&durations);

        let until = Submenu::new("Keep awake until", true);
        for (at, label) in next_whole_hours(now_unix(), sod, UNTIL_CHOICES) {
            let _ = until.append(&MenuItem::with_id(
                format!("{ID_UNTIL}{at}"),
                label,
                true,
                None,
            ));
        }
        let _ = menu.append(&until);
        let _ = menu.append(&self.apps_menu());

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_SCREEN,
            "Keep screen on",
            true,
            self.cfg.keep_screen_on,
            None,
        ));
        let _ = menu.append(&self.battery_menu());
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_NOTIFY,
            "Notifications",
            true,
            self.cfg.notify,
            None,
        ));
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_FACTS,
            "Coffee facts",
            true,
            self.cfg.facts,
            None,
        ));
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_AUTOSTART,
            "Run at login",
            true,
            autostart::is_enabled(),
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        if self.cfg.facts {
            let _ = menu.append(&MenuItem::with_id(
                ID_FACT,
                format!("\u{2615}  {}", self.fact()),
                true,
                None,
            ));
            let _ = menu.append(&PredefinedMenuItem::separator());
        }
        let _ = menu.append(&MenuItem::with_id(ID_QUIT, "Quit Cafe", true, None));

        self.tray.set_menu(Some(Box::new(menu)));
    }

    /// Watched apps (checked; clicking one removes it), then the windowed apps
    /// that could be added, then a way out to the config for anything without a
    /// window, like a build tool or ffmpeg.
    fn apps_menu(&self) -> Submenu {
        let sub = Submenu::new("Keep awake while app runs", true);
        if self.cfg.apps.is_empty() {
            let _ = sub.append(&MenuItem::with_id(ID_INERT, "No apps yet", false, None));
        }
        for app in &self.cfg.apps {
            let running = self.trigger.as_deref() == Some(app.as_str());
            let label = if running {
                format!("{app}  (running)")
            } else {
                app.clone()
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

        let add = Submenu::new("Add running app", !self.windowed.is_empty());
        for name in &self.windowed {
            let _ = add.append(&MenuItem::with_id(
                format!("{ID_APP_ADD}{name}"),
                name,
                true,
                None,
            ));
        }
        let _ = sub.append(&add);
        let _ = sub.append(&MenuItem::with_id(
            ID_EDIT,
            "Edit app list\u{2026}",
            true,
            None,
        ));
        sub
    }

    fn battery_menu(&self) -> Submenu {
        let has_battery = self.battery.is_some();
        let title = match self.battery {
            Some(b) => format!("Stop on battery below  ({}%)", b.percent),
            None => "Stop on battery below".to_string(),
        };
        let sub = Submenu::new(title, has_battery);
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
