//! Small helpers shared across modules.

/// Seconds since the Unix epoch, saturating at 0 on a pre-1970 clock.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// "45s", "12m", "1h 05m". Minutes are rounded up so a session never reads
/// "0m left" while it is still running.
pub fn humanize(secs: u64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs.div_ceil(60);
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}h {:02}m", mins / 60, mins % 60)
    }
}

/// "30 minutes", "1 hour", "1.5 hours", for the duration menu.
pub fn duration_label(mins: u32) -> String {
    match mins {
        1 => "1 minute".into(),
        m if m < 60 => format!("{m} minutes"),
        60 => "1 hour".into(),
        m if m % 60 == 0 => format!("{} hours", m / 60),
        m => format!("{:.1} hours", m as f32 / 60.0),
    }
}

/// Seconds since local midnight, from the OS clock and its idea of the zone
/// and DST, so no date crate is needed.
#[cfg(windows)]
pub fn local_seconds_of_day() -> u64 {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    t.wHour as u64 * 3600 + t.wMinute as u64 * 60 + t.wSecond as u64
}

#[cfg(target_os = "macos")]
pub fn local_seconds_of_day() -> u64 {
    let secs = now_unix() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&secs, &mut tm);
    }
    tm.tm_hour as u64 * 3600 + tm.tm_min as u64 * 60 + tm.tm_sec as u64
}

/// The next `count` whole hours as (unix time, "17:00") pairs, given the
/// current Unix time and seconds since local midnight. Pure, so the menu's
/// arithmetic can be tested without a clock.
pub fn next_whole_hours(now: u64, local_sod: u64, count: u32) -> Vec<(u64, String)> {
    let to_next = 3600 - local_sod % 3600;
    (0..count as u64)
        .map(|i| {
            let delta = to_next + i * 3600;
            (now + delta, clock_label(local_sod + delta))
        })
        .collect()
}

/// "HH:MM" for a time `secs` after local midnight, wrapping past 24h.
pub fn clock_label(secs: u64) -> String {
    let s = secs % 86_400;
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// "HH:MM" for a Unix time, read off the local clock. Ignores a DST change
/// between now and then, which at worst mislabels a session by an hour on two
/// nights a year.
pub fn local_clock_at(at: u64) -> String {
    let now = now_unix();
    let sod = local_seconds_of_day();
    clock_label(if at >= now {
        sod + (at - now)
    } else {
        sod + 86_400 - (now - at) % 86_400
    })
}

/// Full path to the running executable, quoted for the Run key.
#[cfg(windows)]
pub fn exe_path_quoted() -> Option<String> {
    let p = std::env::current_exe().ok()?;
    Some(format!("\"{}\"", p.display()))
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use windows::core::PCWSTR;

    /// Null-terminated UTF-16. Keep the buffer alive while the PCWSTR is used.
    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn pcwstr(buf: &[u16]) -> PCWSTR {
        PCWSTR(buf.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_rounds_minutes_up() {
        assert_eq!(humanize(5), "5s");
        assert_eq!(humanize(60), "1m");
        assert_eq!(humanize(61), "2m");
        assert_eq!(humanize(3600), "1h 00m");
        assert_eq!(humanize(3600 + 5 * 60), "1h 05m");
    }

    #[test]
    fn next_hours_from_mid_hour() {
        // 16:20:00 local.
        let hours = next_whole_hours(1_000_000, 16 * 3600 + 20 * 60, 3);
        assert_eq!(hours[0], (1_000_000 + 40 * 60, "17:00".to_string()));
        assert_eq!(hours[1].1, "18:00");
        assert_eq!(hours[2].0 - hours[0].0, 7200);
    }

    #[test]
    fn next_hours_wrap_past_midnight() {
        let hours = next_whole_hours(0, 23 * 3600 + 59 * 60, 2);
        assert_eq!(hours[0], (60, "00:00".to_string()));
        assert_eq!(hours[1].1, "01:00");
    }

    #[test]
    fn on_the_hour_skips_to_the_next() {
        let hours = next_whole_hours(0, 9 * 3600, 1);
        assert_eq!(hours[0], (3600, "10:00".to_string()));
    }

    #[test]
    fn duration_labels() {
        assert_eq!(duration_label(30), "30 minutes");
        assert_eq!(duration_label(60), "1 hour");
        assert_eq!(duration_label(120), "2 hours");
        assert_eq!(duration_label(90), "1.5 hours");
    }
}
