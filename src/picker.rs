//! "Choose an app…": the system's own file picker, for adding an app that
//! isn't open right now, or a program without a window.
//!
//! Both run modally on the event loop's thread, which is where menu clicks
//! are handled anyway; the tray simply waits until the dialog closes.

use crate::procs::{self, App};

pub use imp::choose_app;

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod imp {
    use super::{procs, App};
    use crate::util::{pcwstr, wide};
    use std::path::Path;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_DONTADDTORECENT, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    pub fn choose_app() -> Option<App> {
        // Pairs of label and pattern, each NUL-terminated, then one more NUL.
        let filter: Vec<u16> = "Programs (*.exe)\0*.exe\0\0".encode_utf16().collect();
        let title = wide("Choose an app to keep your PC awake while it's open");
        let start = wide(&std::env::var("ProgramFiles").unwrap_or_default());
        let mut file = vec![0u16; 1024];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(file.as_mut_ptr()),
            nMaxFile: file.len() as u32,
            lpstrInitialDir: pcwstr(&start),
            lpstrTitle: pcwstr(&title),
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT,
            ..Default::default()
        };
        if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
            return None;
        }
        let end = file.iter().position(|&c| c == 0)?;
        procs::describe(Path::new(&String::from_utf16_lossy(&file[..end])))
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod imp {
    use super::{procs, App};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{ns_string, NSURL};
    use std::path::Path;

    pub fn choose_app() -> Option<App> {
        let mtm = MainThreadMarker::new()?;
        // A menu bar app is never the active one, and a panel opened by an
        // inactive app comes up behind whatever is in front.
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);

        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(false);
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(ns_string!("/Applications"))));
        panel.setMessage(Some(ns_string!(
            "Choose an app to keep your Mac awake while it's open."
        )));
        panel.setPrompt(Some(ns_string!("Add")));
        if panel.runModal() != NSModalResponseOK {
            return None;
        }
        let path = panel.URL()?.path()?.to_string();
        procs::describe(Path::new(&path))
    }
}
