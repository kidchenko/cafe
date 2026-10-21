//! Running processes, for "keep awake while an app runs".
//!
//! Two questions, both answered by a fresh snapshot on the tick:
//! - [`running`]: every process name, to match the watch list against.
//! - [`windowed`]: the apps a person would recognise, for the "Add running app"
//!   menu. Background services make up most of a process list and none of them
//!   are what anyone means by "while OBS is open".
//!
//! Names are compared after [`normalise`], so `OBS64.exe`, `obs64` and
//! `obs64.EXE` in the config are all the same entry.

use std::collections::HashSet;

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

/// Normalised names of every process this user can see.
pub use imp::running;

/// Display names of apps with a window, sorted and de-duplicated, minus the
/// shell's own furniture.
pub fn windowed() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names: Vec<String> = imp::windowed()
        .into_iter()
        .filter(|n| !IGNORE.contains(&normalise(n).as_str()))
        .filter(|n| seen.insert(normalise(n)))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
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
    use super::normalise;
    use std::collections::{HashMap, HashSet};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
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

    pub fn windowed() -> Vec<String> {
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
        let names = snapshot();
        pids.iter()
            .filter_map(|p| names.get(p))
            .map(|n| n.strip_suffix(".exe").unwrap_or(n).to_string())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod imp {
    use super::normalise;
    use std::collections::HashSet;
    use std::ffi::{c_int, c_void};

    // libproc, part of libSystem. Declared here rather than taken from `libc`
    // so the build does not depend on which libc release exposes them.
    extern "C" {
        fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
        fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
    }

    const PATH_MAX: usize = 4096;

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

    /// An app bundle's main executable is what a person thinks of as "the
    /// app"; helpers live under Contents/Frameworks or are named so.
    pub fn windowed() -> Vec<String> {
        paths()
            .iter()
            .filter(|p| p.contains(".app/Contents/MacOS/") && !p.starts_with("/System/"))
            .map(|p| basename(p).to_string())
            .filter(|n| !n.contains("Helper"))
            .collect()
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
