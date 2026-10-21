//! Battery state, for the "stop on battery below N%" floor.
//!
//! Read on the tick rather than subscribed to: both platforms can push power
//! notifications, but a 30-second poll of a struct the OS already maintains is
//! cheaper in code than either subscription and plenty fast for a percentage
//! that moves a point every few minutes.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Battery {
    /// Running from the battery rather than a charger.
    pub discharging: bool,
    pub percent: u8,
}

/// `None` when there is no battery (a desktop) or its state is unknown.
pub use imp::battery;

/// Should a keep-awake request be dropped because of the battery?
pub fn below_floor(battery: Option<Battery>, floor: u8) -> bool {
    match battery {
        Some(b) => floor > 0 && b.discharging && b.percent < floor,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod imp {
    use super::Battery;
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    const NO_SYSTEM_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;

    pub fn battery() -> Option<Battery> {
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s) }.ok()?;
        if s.BatteryFlag == UNKNOWN
            || s.BatteryFlag & NO_SYSTEM_BATTERY != 0
            || s.BatteryLifePercent == UNKNOWN
        {
            return None;
        }
        Some(Battery {
            // 0 = offline (on battery), 1 = online, 255 = unknown. Unknown is
            // treated as plugged in: never stop on a guess.
            discharging: s.ACLineStatus == 0,
            percent: s.BatteryLifePercent.min(100),
        })
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod imp {
    use super::Battery;
    use std::ffi::{c_char, c_void, CString};

    type CFTypeRef = *const c_void;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT32_TYPE: isize = 3;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: CFTypeRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFTypeRef;
        fn CFRelease(cf: CFTypeRef);
        fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> CFTypeRef;
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, the_type: isize, value: *mut c_void) -> u8;
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
        fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFTypeRef;
        fn IOPSGetPowerSourceDescription(blob: CFTypeRef, ps: CFTypeRef) -> CFTypeRef;
    }

    /// A CF object we own, released on drop.
    struct Owned(CFTypeRef);

    impl Owned {
        fn new(r: CFTypeRef) -> Option<Self> {
            (!r.is_null()).then_some(Self(r))
        }

        fn string(s: &str) -> Option<Self> {
            let c = CString::new(s).ok()?;
            Self::new(unsafe {
                CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
            })
        }
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }

    /// Borrowed lookups into a power-source description dictionary.
    struct Desc(CFTypeRef);

    impl Desc {
        fn get(&self, key: &str) -> Option<CFTypeRef> {
            let k = Owned::string(key)?;
            let v = unsafe { CFDictionaryGetValue(self.0, k.0) };
            (!v.is_null()).then_some(v)
        }

        fn is(&self, key: &str, expected: &str) -> bool {
            match (self.get(key), Owned::string(expected)) {
                (Some(v), Some(e)) => unsafe { CFEqual(v, e.0) != 0 },
                _ => false,
            }
        }

        fn int(&self, key: &str) -> Option<i32> {
            let v = self.get(key)?;
            let mut out: i32 = 0;
            let ok = unsafe {
                CFNumberGetValue(
                    v,
                    K_CF_NUMBER_SINT32_TYPE,
                    &mut out as *mut i32 as *mut c_void,
                )
            };
            (ok != 0).then_some(out)
        }
    }

    pub fn battery() -> Option<Battery> {
        let blob = Owned::new(unsafe { IOPSCopyPowerSourcesInfo() })?;
        let list = Owned::new(unsafe { IOPSCopyPowerSourcesList(blob.0) })?;
        let count = unsafe { CFArrayGetCount(list.0) };
        for i in 0..count {
            let ps = unsafe { CFArrayGetValueAtIndex(list.0, i) };
            let desc = unsafe { IOPSGetPowerSourceDescription(blob.0, ps) };
            if desc.is_null() {
                continue;
            }
            let d = Desc(desc);
            // A UPS also shows up as a power source; only the built-in battery
            // is the one that drains when the lid is open.
            if !d.is("Type", "InternalBattery") {
                continue;
            }
            let current = d.int("Current Capacity")?;
            let max = d.int("Max Capacity").filter(|m| *m > 0)?;
            return Some(Battery {
                discharging: d.is("Power Source State", "Battery Power"),
                percent: (current * 100 / max).clamp(0, 100) as u8,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_only_applies_when_discharging_below_it() {
        let b = |discharging, percent| {
            Some(Battery {
                discharging,
                percent,
            })
        };
        assert!(below_floor(b(true, 15), 20));
        assert!(!below_floor(b(true, 20), 20));
        assert!(!below_floor(b(false, 5), 20), "plugged in never stops");
        assert!(!below_floor(b(true, 5), 0), "0 means never");
        assert!(!below_floor(None, 20), "no battery never stops");
    }
}
