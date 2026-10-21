//! Persisted settings, as JSON in `%APPDATA%\Cafe\config.json` on Windows and
//! `~/Library/Application Support/Cafe/config.json` on macOS.
//!
//! The current mode is saved along with the preferences, so Cafe comes back
//! the way it was left: a timed session that has not run out yet resumes with
//! the time it had left, and one that ran out while the app was closed comes
//! back off.

use crate::log::{lerror, linfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Mode {
    /// The normal power plan applies.
    Off,
    /// Awake until switched off.
    Indefinite,
    /// Awake until this Unix time, then back to `Off`.
    Until { at: u64 },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub mode: Mode,
    /// Also keep the display on. Off means only the system stays awake and the
    /// screen may still dim and turn off on its normal schedule.
    pub keep_screen_on: bool,
    /// The "Keep awake for" choices, in minutes.
    pub durations: Vec<u32>,
    /// Stop keeping awake when running on battery below this percentage.
    /// 0 means never. Has no effect on a machine without a battery.
    pub battery_floor: u8,
    /// Keep awake while any of these processes is running, whatever the mode.
    /// Matched case-insensitively against the executable name, with or without
    /// `.exe`.
    pub apps: Vec<String>,
    /// Notify when a timed session ends or the battery floor stops Cafe.
    pub notify: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            keep_screen_on: true,
            durations: vec![15, 30, 60, 120, 240, 480],
            // On by default: a laptop kept awake on battery overnight is the
            // one failure this app can cause rather than prevent.
            battery_floor: 20,
            apps: Vec::new(),
            notify: true,
        }
    }
}

impl Config {
    #[cfg(windows)]
    pub fn dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Cafe"))
    }

    #[cfg(target_os = "macos")]
    pub fn dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Cafe"))
    }

    pub fn path() -> Option<PathBuf> {
        Self::dir().map(|d| d.join("config.json"))
    }

    /// Last-modified time of the file, to notice a hand edit cheaply.
    pub fn modified() -> Option<std::time::SystemTime> {
        std::fs::metadata(Self::path()?).ok()?.modified().ok()
    }

    /// Read and parse without falling back to defaults: `None` for missing
    /// or unparsable, so an editor caught mid-save changes nothing.
    pub fn reload() -> Option<Self> {
        Self::parse(&std::fs::read_to_string(Self::path()?).ok()?).ok()
    }

    fn parse(text: &str) -> Result<Self, serde_json::Error> {
        // Notepad and PowerShell both like to write a BOM; serde_json does not.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        serde_json::from_str::<Config>(text).map(Config::sanitised)
    }

    /// Never fails: a missing or corrupt file yields defaults.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            linfo!("no config at {}; using defaults", path.display());
            return Self::default();
        };
        match Self::parse(&text) {
            Ok(c) => {
                linfo!(
                    "config loaded: mode={:?} screen={}",
                    c.mode,
                    c.keep_screen_on
                );
                c
            }
            Err(e) => {
                lerror!(
                    "config at {} is unreadable ({e}); using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Temp file + rename, so an interrupted save never leaves half a file.
    pub fn save(&self) -> std::io::Result<()> {
        let (Some(dir), Some(path)) = (Self::dir(), Self::path()) else {
            return Err(std::io::Error::other("no config directory in environment"));
        };
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)
    }

    pub fn sanitised(mut self) -> Self {
        // A minute to a week; anything else is a typo.
        self.durations.retain(|m| (1..=7 * 24 * 60).contains(m));
        self.durations.sort_unstable();
        self.durations.dedup();
        if self.durations.is_empty() {
            self.durations = Self::default().durations;
        }
        self.battery_floor = self.battery_floor.min(95);
        let mut seen = Vec::new();
        self.apps.retain(|a| {
            let key = crate::procs::normalise(a);
            !key.is_empty() && !seen.contains(&key) && {
                seen.push(key);
                true
            }
        });
        for a in &mut self.apps {
            *a = a.trim().to_string();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_mode() {
        for mode in [
            Mode::Off,
            Mode::Indefinite,
            Mode::Until { at: 1_700_000_000 },
        ] {
            let c = Config {
                mode,
                ..Config::default()
            };
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(Config::parse(&json).unwrap(), c);
        }
    }

    #[test]
    fn tolerates_bom_and_missing_fields() {
        let c = Config::parse("\u{feff}{\"keepScreenOn\": false}").unwrap();
        assert_eq!(c.mode, Mode::Off);
        assert!(!c.keep_screen_on);
        assert!(!c.durations.is_empty());
    }

    #[test]
    fn cleans_app_list_and_battery_floor() {
        let c = Config::parse(
            r#"{"apps": ["OBS64.exe", " obs64 ", "", "ffmpeg"], "batteryFloor": 200}"#,
        )
        .unwrap();
        assert_eq!(c.apps, vec!["OBS64.exe", "ffmpeg"]);
        assert_eq!(c.battery_floor, 95);
    }

    #[test]
    fn drops_nonsense_durations() {
        let c = Config::parse("{\"durations\": [0, 30, 30, 99999999]}").unwrap();
        assert_eq!(c.durations, vec![30]);
        let c = Config::parse("{\"durations\": [0]}").unwrap();
        assert_eq!(c.durations, Config::default().durations);
    }
}
