//! Running processes, for "keep awake while an app is open".
//!
//! Two questions, both answered by a fresh snapshot on the tick:
//! - [`running`]: every process name, to match the watch list against.
//! - [`windowed`]: the apps a person would recognise, for the "Add an open
//!   app" menu. Background services make up most of a process list and none
//!   of them are what anyone means by "while OBS is open".
//!
//! An app has two names. The executable name is what gets matched and stored
//! (`obs64`, `wezterm-gui`); the display name is what the menu shows (OBS
//! Studio, WezTerm). [`describe`] gets both from a path, which is also how
//! the app picker turns a chosen file into an entry.
//!
//! Executable names are compared after [`normalise`], so `OBS64.exe`, `obs64`
//! and `obs64.EXE` in the config are all the same entry.

use std::collections::HashSet;

/// An app as the menu offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    /// What a person calls it: "OBS Studio", "WezTerm".
    pub name: String,
    /// The executable name Cafe matches on: "obs64", "wezterm-gui".
    pub exe: String,
}

/// Lower-case, trimmed, without a trailing `.exe`.
pub fn normalise(name: &str) -> String {
    let n = name.trim().to_lowercase();
    n.strip_suffix(".exe").map(str::to_string).unwrap_or(n)
}

/// The first watched app found running, as written in the config.
pub fn first_running<'a>(watch: &'a [String], running: &HashSet<String>) -> Option<&'a str> {
    watch
        .iter()
        .find(|w| running.contains(&normalise(w)))
        .map(String::as_str)
}

/// Normalised names of every process this user can see; and the two names of
/// the app or program at a path.
pub use imp::{describe, running};

/// Call `f` as soon as an app launches or quits, so a watched app is noticed
/// at once rather than on the next tick. macOS only: Windows has no cheap
/// equivalent, and there the tick is all there is.
#[cfg(target_os = "macos")]
pub use imp::on_launch_or_quit;

/// Apps with a window, sorted by display name and de-duplicated, minus the
/// shell's own furniture.
pub fn windowed() -> Vec<App> {
    let mut seen = HashSet::new();
    let mut apps: Vec<App> = imp::windowed()
        .into_iter()
        .filter(|a| !IGNORE.contains(&normalise(&a.exe).as_str()))
        .filter(|a| seen.insert(normalise(&a.exe)))
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Windowed processes that are part of the desktop rather than apps anyone
/// would want to watch.
const IGNORE: &[&str] = &[
    "cafe",
    "explorer",
    "applicationframehost",
    "textinputhost",
    "systemsettings",
    "shellexperiencehost",
    "startmenuexperiencehost",
    "searchhost",
    "lockapp",
    "finder",
    "dock",
    "systemuiserver",
    "controlcenter",
    "notificationcenter",
    "loginwindow",
];

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod imp {
    use super::{normalise, App};
    use crate::util::{pcwstr, wide};
    use std::collections::{HashMap, HashSet};
    use std::ffi::c_void;
    use std::path::Path;
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
        GW_OWNER,
    };

    /// pid -> executable name, e.g. "obs64.exe".
    fn snapshot() -> HashMap<u32, String> {
        let mut out = HashMap::new();
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return out;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = unsafe { Process32FirstW(snap, &mut entry) }.is_ok();
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            out.insert(
                entry.th32ProcessID,
                String::from_utf16_lossy(&entry.szExeFile[..len]),
            );
            ok = unsafe { Process32NextW(snap, &mut entry) }.is_ok();
        }
        unsafe {
            let _ = CloseHandle(snap);
        }
        out
    }

    pub fn running() -> HashSet<String> {
        snapshot().values().map(|n| normalise(n)).collect()
    }

    pub fn windowed() -> Vec<App> {
        unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let pids = &mut *(lparam.0 as *mut HashSet<u32>);
            // Visible, titled, top-level windows: what shows in Alt+Tab.
            let owned = GetWindow(hwnd, GW_OWNER)
                .map(|h| !h.is_invalid())
                .unwrap_or(false);
            if IsWindowVisible(hwnd).as_bool() && !owned && GetWindowTextLengthW(hwnd) > 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                pids.insert(pid);
            }
            BOOL(1)
        }

        let mut pids: HashSet<u32> = HashSet::new();
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut pids as *mut _ as isize));
        }
        pids.iter()
            .filter_map(|&pid| image_path(pid))
            .filter_map(|p| describe(Path::new(&p)))
            .collect()
    }

    /// Full path of a process's executable. Works for elevated processes too,
    /// with the limited query right.
    fn image_path(pid: u32) -> Option<String> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(process);
            ok.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// The executable's name, and the description from its version resource
    /// for display: what Task Manager shows, e.g. "Visual Studio Code" for
    /// Code.exe. Falls back to the file name.
    pub fn describe(path: &Path) -> Option<App> {
        let exe = path.file_stem()?.to_string_lossy().into_owned();
        let name = file_description(path)
            .filter(|d| !d.is_empty() && !d.to_lowercase().ends_with(".exe"))
            .unwrap_or_else(|| exe.clone());
        Some(App { name, exe })
    }

    fn file_description(path: &Path) -> Option<String> {
        let file = wide(&path.to_string_lossy());
        unsafe {
            let size = GetFileVersionInfoSizeW(pcwstr(&file), None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(pcwstr(&file), None, size, data.as_mut_ptr() as *mut c_void)
                .ok()?;
            let block = data.as_ptr() as *const c_void;

            // Strings are stored per language; use the first one the file
            // declares, as Explorer does.
            let mut ptr: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let query = wide(r"\VarFileInfo\Translation");
            if !VerQueryValueW(block, pcwstr(&query), &mut ptr, &mut len).as_bool() || len < 4 {
                return None;
            }
            let lang = *(ptr as *const u16);
            let codepage = *(ptr as *const u16).add(1);

            let query = wide(&format!(
                r"\StringFileInfo\{lang:04x}{codepage:04x}\FileDescription"
            ));
            if !VerQueryValueW(block, pcwstr(&query), &mut ptr, &mut len).as_bool() || len == 0 {
                return None;
            }
            let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            Some(String::from_utf16_lossy(&text[..end]).trim().to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod imp {
    use super::{normalise, App};
    use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};
    use objc2_foundation::{NSBundle, NSFileManager, NSString};
    use std::collections::HashSet;
    use std::ffi::{c_int, c_void};
    use std::path::Path;

    // libproc, part of libSystem. Declared here rather than taken from `libc`
    // so the build does not depend on which libc release exposes them.
    extern "C" {
        fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
        fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
    }

    const PATH_MAX: usize = 4096;

    pub fn on_launch_or_quit(f: impl Fn() + 'static) {
        use block2::RcBlock;
        use objc2_app_kit::{
            NSWorkspaceDidLaunchApplicationNotification,
            NSWorkspaceDidTerminateApplicationNotification,
        };
        use objc2_foundation::NSNotification;
        use std::ptr::NonNull;
        use std::rc::Rc;

        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let f = Rc::new(f);
        let names = unsafe {
            [
                NSWorkspaceDidLaunchApplicationNotification,
                NSWorkspaceDidTerminateApplicationNotification,
            ]
        };
        for name in names {
            let f = f.clone();
            let block = RcBlock::new(move |_: NonNull<NSNotification>| f());
            // Posted on the main thread, so no queue. The centre keeps the
            // block for as long as the process lives, which is as long as
            // Cafe wants it.
            let _ = unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            };
        }
    }

    /// Executable paths of every process this user may inspect. Other users'
    /// processes (root daemons) fail `proc_pidpath` and are simply skipped.
    fn paths() -> Vec<String> {
        let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Vec::new();
        }
        // Headroom for processes started between the two calls.
        let mut pids = vec![0 as c_int; count as usize + 64];
        let n = unsafe {
            proc_listallpids(
                pids.as_mut_ptr() as *mut c_void,
                (pids.len() * std::mem::size_of::<c_int>()) as c_int,
            )
        };
        let mut buf = vec![0u8; PATH_MAX];
        pids.iter()
            .take(n.max(0) as usize)
            .filter_map(|&pid| {
                let len =
                    unsafe { proc_pidpath(pid, buf.as_mut_ptr() as *mut c_void, PATH_MAX as u32) };
                (len > 0).then(|| String::from_utf8_lossy(&buf[..len as usize]).into_owned())
            })
            .collect()
    }

    fn basename(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    pub fn running() -> HashSet<String> {
        paths().iter().map(|p| normalise(basename(p))).collect()
    }

    /// The apps in the Dock: AppKit's "regular" activation policy. That
    /// leaves out menu bar agents, background helpers and system services,
    /// which a process list cannot tell apart from apps.
    pub fn windowed() -> Vec<App> {
        NSWorkspace::sharedWorkspace()
            .runningApplications()
            .iter()
            .filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular)
            .filter_map(|a| {
                let exe = a.executableURL()?.lastPathComponent()?.to_string();
                let name = a
                    .localizedName()
                    .map_or_else(|| exe.clone(), |n| n.to_string());
                Some(App { name, exe })
            })
            .collect()
    }

    /// An app bundle's executable and Finder name, or for a plain program
    /// (say, a build tool) its file name for both.
    pub fn describe(path: &Path) -> Option<App> {
        let ns_path = NSString::from_str(&path.to_string_lossy());
        let exe = match NSBundle::bundleWithPath(&ns_path).and_then(|b| b.executableURL()) {
            Some(url) => url.lastPathComponent()?.to_string(),
            None => path.file_name()?.to_string_lossy().into_owned(),
        };
        let name = NSFileManager::defaultManager()
            .displayNameAtPath(&ns_path)
            .to_string();
        let name = name.strip_suffix(".app").unwrap_or(&name).to_string();
        Some(App { name, exe })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_ignores_case_and_exe() {
        assert_eq!(normalise(" OBS64.EXE "), "obs64");
        assert_eq!(normalise("ffmpeg"), "ffmpeg");
        assert_eq!(normalise("zoom.us"), "zoom.us");
    }

    #[test]
    fn first_running_keeps_config_spelling() {
        let running: HashSet<String> = ["obs64".to_string(), "code".to_string()].into();
        let watch = vec!["ffmpeg".to_string(), "OBS64.exe".to_string()];
        assert_eq!(first_running(&watch, &running), Some("OBS64.exe"));
        assert_eq!(first_running(&[], &running), None);
    }

    #[test]
    fn sees_this_test_process() {
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().into_owned();
        assert!(running().contains(&normalise(&name)));
    }
}
