// No console window: this is a tray app. No-op off Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

//! Cafe keeps the machine awake from the tray, like PowerToys Awake or
//! Caffeine/Amphetamine, on both Windows and macOS.
//!
//! | module            | responsibility                                     |
//! |-------------------|----------------------------------------------------|
//! | `awake`           | asks the OS not to sleep (and stops asking)         |
//! | `art`             | the menu's glyphs, from Lucide                      |
//! | `config`          | the persisted mode and preferences                  |
//! | `icon`            | draws the cup                                       |
//! | `power`           | battery level, for the stop-on-battery floor        |
//! | `procs`           | running apps, for keep-awake-while-an-app-is-open   |
//! | `picker`          | the "Choose an app…" file dialog                    |
//! | `notify`          | notifications when Cafe stops on its own            |
//! | `app`             | the tray icon, its menu, and the timer              |
//! | `single_instance` | one copy at a time; relaunching toggles             |
//! | `autostart`       | run at login                                        |

mod app;
mod art;
mod autostart;
mod awake;
mod config;
mod facts;
mod icon;
mod log;
mod notify;
mod picker;
mod power;
mod procs;
mod single_instance;
mod theme;
mod util;

use app::{App, Flow, UserEvent};
use log::{lerror, linfo};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

fn main() {
    // Development aid: writes the icons as raw RGBA and exits. See icon::dump.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--dump-icons") {
        let dir = args
            .get(i + 1)
            .map(String::as_str)
            .unwrap_or("target/icons");
        std::process::exit(match icon::dump(dir) {
            Ok(()) => 0,
            Err(_) => 1,
        });
    }

    log::init();
    linfo!(
        "--- Cafe {} starting (pid {}) ---",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    );

    // A second launch toggles the running instance and exits.
    let Some(signal) = single_instance::acquire() else {
        linfo!("another instance owns the tray; asked it to toggle, exiting");
        return;
    };

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Menu-bar agent, no Dock icon. tao resets the activation policy as it
    // starts, overriding LSUIElement, so it has to be asked for here too.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_activate_ignoring_other_apps(false);
    }
    let proxy = event_loop.create_proxy();

    let signal_proxy = proxy.clone();
    signal.watch(move || {
        let _ = signal_proxy.send_event(UserEvent::Toggle);
    });

    // The tray is built once the loop is running: on macOS the status item
    // cannot be created before the application has finished launching.
    let mut app: Option<App> = None;

    event_loop.run(move |event, _target, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                drop_launch_activation();
                match App::new(proxy.clone()) {
                    Ok(a) => app = Some(a),
                    Err(e) => {
                        lerror!("{e}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
            }
            Event::UserEvent(e) => {
                if let Some(a) = app.as_mut() {
                    if a.handle(e) == Flow::Exit {
                        // Dropping the app drops the keeper, which withdraws
                        // the request before the process goes.
                        app = None;
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
            }
            // macOS: opening Cafe.app again (Finder, Spotlight, `open`) does not
            // start a second process, so the socket never hears about it.
            // LaunchServices sends the running one a reopen instead; treat it
            // like any other relaunch.
            Event::Reopen { .. } => {
                if let Some(a) = app.as_mut() {
                    linfo!("reopened by LaunchServices");
                    let _ = a.handle(UserEvent::Toggle);
                }
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                if let Some(a) = app.as_mut() {
                    a.tick();
                }
            }
            _ => {}
        }

        if let Some(a) = app.as_ref() {
            *control_flow = ControlFlow::WaitUntil(a.next_wake());
        }
    });
}

/// tao asks macOS to activate the app as it finishes launching. A menu bar
/// app has nothing to bring forward, and on macOS 26 the request stays
/// pending until the first click on the menu bar icon, then completes while
/// the menu is open and closes it. Withdraw it before anything is shown.
#[cfg(target_os = "macos")]
fn drop_launch_activation() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).deactivate();
    }
}
