//! The tray icon, its menu, and the mode they control.
//!
//! Everything here runs on the event loop's thread. That is not just tidiness:
//! on Windows the keep-awake request belongs to the thread that made it (see
//! `awake.rs`), and this is the one thread guaranteed to live as long as the
//! process.

use crate::awake::{Keeper, Level};
use crate::config::{Config, Mode};
use crate::icon::{self, Face};
use crate::log::{lerror, linfo, lwarn};
use crate::util::{
    duration_label, humanize, local_clock_at, local_seconds_of_day, next_whole_hours, now_unix,
};
use crate::{autostart, theme};

use std::time::{Duration, Instant};
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const ID_OFF: &str = "off";
const ID_INDEFINITE: &str = "indefinite";
const ID_FOR: &str = "for:";
const ID_UNTIL: &str = "until:";
const ID_SCREEN: &str = "screen";
const ID_AUTOSTART: &str = "autostart";
const ID_QUIT: &str = "quit";
const ID_HEADER: &str = "header";

/// How often the loop wakes when nothing is due: often enough to keep the
/// countdown in the menu honest to the minute and to notice the taskbar theme
/// changing, rare enough to be invisible in Task Manager.
const TICK: Duration = Duration::from_secs(30);

/// How many whole hours "Keep awake until" offers.
const UNTIL_CHOICES: u32 = 8;

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
    /// Last face and header painted, so the tick only touches the tray when
    /// something visible actually changed.
    face: Face,
    header: String,
    /// The local hour the "Keep awake until" list was built in. When it moves
    /// on, the first entry is in the past and the list is rebuilt.
    menu_hour: u64,
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
            face: Face::Off,
            header: String::new(),
            menu_hour: 0,
        };

        // A timed session that ran out while we were closed comes back off.
        if let Mode::Until { at } = app.cfg.mode {
            if at <= now_unix() {
                app.cfg.mode = Mode::Off;
            }
        }
        app.apply();
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
            UserEvent::Menu(id) => match id.as_str() {
                ID_QUIT => {
                    linfo!("quit requested from the tray");
                    return Flow::Exit;
                }
                ID_OFF => self.set_mode(Mode::Off),
                ID_INDEFINITE => self.set_mode(Mode::Indefinite),
                ID_SCREEN => {
                    self.cfg.keep_screen_on = !self.cfg.keep_screen_on;
                    self.apply();
                }
                ID_AUTOSTART => {
                    let want = !autostart::is_enabled();
                    match autostart::set(want) {
                        Ok(()) => linfo!("run at login {}", if want { "on" } else { "off" }),
                        Err(e) => lerror!("{e}"),
                    }
                    self.rebuild_menu();
                }
                other => {
                    if let Some(mins) = other
                        .strip_prefix(ID_FOR)
                        .and_then(|m| m.parse::<u64>().ok())
                    {
                        self.set_mode(Mode::Until {
                            at: now_unix() + mins * 60,
                        });
                    } else if let Some(at) = other
                        .strip_prefix(ID_UNTIL)
                        .and_then(|t| t.parse::<u64>().ok())
                    {
                        // A choice that went stale while the menu sat open is
                        // ignored rather than turned into an instant expiry.
                        if at > now_unix() {
                            self.set_mode(Mode::Until { at });
                        }
                    }
                }
            },
        }
        Flow::Continue
    }

    /// Called on every wake. Ends an expired session and repaints whatever
    /// the passage of time has changed.
    pub fn tick(&mut self) {
        if let Mode::Until { at } = self.cfg.mode {
            if at <= now_unix() {
                linfo!("timed session ended");
                self.set_mode(Mode::Off);
                return;
            }
        }
        let dark = theme::taskbar_is_dark();
        if dark != self.dark_taskbar {
            self.dark_taskbar = dark;
            self.paint_icon(true);
        }
        if self.header_text() != self.header || local_seconds_of_day() / 3600 != self.menu_hour {
            self.rebuild_menu();
        }
    }

    /// When the event loop should next wake: the end of a timed session if
    /// that is sooner than the regular tick.
    pub fn next_wake(&self) -> Instant {
        let tick = Instant::now() + TICK;
        match self.cfg.mode {
            Mode::Until { at } => {
                let left = at.saturating_sub(now_unix());
                tick.min(Instant::now() + Duration::from_secs(left))
            }
            _ => tick,
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        self.cfg.mode = mode;
        self.apply();
    }

    /// Push the current config to the OS, the tray, and the disk.
    fn apply(&mut self) {
        let level = match self.cfg.mode {
            Mode::Off => Level::Off,
            _ if self.cfg.keep_screen_on => Level::SystemAndScreen,
            _ => Level::System,
        };
        match self.keeper.set(level) {
            Ok(()) => linfo!("mode={:?} level={:?}", self.cfg.mode, level),
            Err(e) => {
                lerror!("could not apply {level:?}: {e}");
                self.cfg.mode = Mode::Off;
            }
        }
        self.paint_icon(false);
        self.rebuild_menu();
        if let Err(e) = self.cfg.save() {
            lwarn!("could not save config: {e}");
        }
    }

    fn face(&self) -> Face {
        match self.cfg.mode {
            Mode::Off => Face::Off,
            _ if self.cfg.keep_screen_on => Face::AwakeScreen,
            _ => Face::Awake,
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
        let screen = if self.cfg.keep_screen_on {
            ", screen on"
        } else {
            ""
        };
        match self.cfg.mode {
            Mode::Off => "Off \u{2014} normal power plan".into(),
            Mode::Indefinite => format!("Awake{screen}"),
            Mode::Until { at } => format!(
                "Awake{screen} until {} \u{2014} {} left",
                local_clock_at(at),
                humanize(at.saturating_sub(now_unix()))
            ),
        }
    }

    fn rebuild_menu(&mut self) {
        self.header = self.header_text();
        let sod = local_seconds_of_day();
        self.menu_hour = sod / 3600;
        let _ = self
            .tray
            .set_tooltip(Some(format!("Cafe \u{2014} {}", self.header)));

        let menu = Menu::new();
        let _ = menu.append(&MenuItem::with_id(ID_HEADER, &self.header, false, None));
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

        let timed = matches!(mode, Mode::Until { .. });
        let durations = Submenu::new("Keep awake for", true);
        for &mins in &self.cfg.durations {
            let _ = durations.append(&MenuItem::with_id(
                format!("{ID_FOR}{mins}"),
                duration_label(mins),
                true,
                None,
            ));
        }
        // A check on the submenu itself would be ideal but is not something
        // menus offer; a check item inside it says "you are on a timer".
        if timed {
            let _ = durations.prepend_items(&[
                &CheckMenuItem::with_id(ID_HEADER, "Timer running", false, true, None),
                &PredefinedMenuItem::separator(),
            ]);
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

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_SCREEN,
            "Keep screen on",
            true,
            self.cfg.keep_screen_on,
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
        let _ = menu.append(&MenuItem::with_id(ID_QUIT, "Quit Cafe", true, None));

        self.tray.set_menu(Some(Box::new(menu)));
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
