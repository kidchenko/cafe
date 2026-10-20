//! The one thing Cafe actually does: ask the OS not to sleep.
//!
//! Both platforms offer a request that lasts until it is withdrawn, which is
//! exactly the shape wanted here. Neither touches the power plan itself: when
//! the request is dropped the machine goes back to whatever the user's normal
//! settings say, and if Cafe crashes or is killed the OS drops it for us. There
//! is nothing to "restore", which is the whole reason to do it this way rather
//! than editing power settings.
//!
//! | level            | Windows                               | macOS                          |
//! |------------------|---------------------------------------|--------------------------------|
//! | `Off`            | `ES_CONTINUOUS` alone (clears)         | release the assertion          |
//! | `System`         | `+ ES_SYSTEM_REQUIRED`                 | `PreventUserIdleSystemSleep`   |
//! | `SystemAndScreen`| `+ ES_SYSTEM_REQUIRED + ES_DISPLAY_REQUIRED` | `PreventUserIdleDisplaySleep` |
//!
//! Neither prevents sleep the user asks for explicitly (Start > Sleep, closing
//! a MacBook lid on battery). That is the OS's call, and the right one.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Off,
    System,
    SystemAndScreen,
}

pub use imp::Keeper;

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod imp {
    use super::Level;
    use windows::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };

    /// `SetThreadExecutionState` is per *thread*: the request lives as long as
    /// the calling thread does. `Keeper` is created and driven from the event
    /// loop's thread, which lives for the whole process, so that is the thread
    /// that owns the request. Do not move it to a short-lived worker.
    pub struct Keeper {
        level: Level,
    }

    impl Keeper {
        pub fn new() -> Self {
            Self { level: Level::Off }
        }

        pub fn set(&mut self, level: Level) -> Result<(), String> {
            let flags = match level {
                Level::Off => ES_CONTINUOUS,
                Level::System => ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
                Level::SystemAndScreen => ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED,
            };
            // Returns the previous state, or 0 on failure.
            let prev = unsafe { SetThreadExecutionState(flags) };
            if prev.0 == 0 {
                return Err("SetThreadExecutionState failed".into());
            }
            self.level = level;
            Ok(())
        }
    }

    impl Drop for Keeper {
        fn drop(&mut self) {
            // Process exit clears it anyway; this just makes Quit explicit.
            if self.level != Level::Off {
                let _ = self.set(Level::Off);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod imp {
    use super::Level;
    use std::ffi::{c_char, c_void, CString};

    type CFStringRef = *const c_void;
    type IOPMAssertionID = u32;
    type IOReturn = i32;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;
    const K_IO_RETURN_SUCCESS: IOReturn = 0;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut IOPMAssertionID,
        ) -> IOReturn;
        fn IOPMAssertionRelease(id: IOPMAssertionID) -> IOReturn;
    }

    /// An owned CFString, released on drop.
    struct CfString(CFStringRef);

    impl CfString {
        fn new(s: &str) -> Option<Self> {
            let c = CString::new(s).ok()?;
            let r = unsafe {
                CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
            };
            (!r.is_null()).then_some(Self(r))
        }
    }

    impl Drop for CfString {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }

    /// Holds at most one IOPM assertion. Visible to the user in
    /// `pmset -g assertions` under the name "Cafe", which is the first place
    /// anyone looks when a Mac will not sleep.
    pub struct Keeper {
        level: Level,
        id: Option<IOPMAssertionID>,
    }

    impl Keeper {
        pub fn new() -> Self {
            Self {
                level: Level::Off,
                id: None,
            }
        }

        pub fn set(&mut self, level: Level) -> Result<(), String> {
            if level == self.level {
                return Ok(());
            }
            self.release();
            let kind = match level {
                Level::Off => {
                    self.level = Level::Off;
                    return Ok(());
                }
                Level::System => "PreventUserIdleSystemSleep",
                // Keeping the display awake implies keeping the system awake.
                Level::SystemAndScreen => "PreventUserIdleDisplaySleep",
            };
            let (Some(kind), Some(name)) = (CfString::new(kind), CfString::new("Cafe")) else {
                return Err("could not build CFString".into());
            };
            let mut id: IOPMAssertionID = 0;
            let rc = unsafe {
                IOPMAssertionCreateWithName(kind.0, K_IOPM_ASSERTION_LEVEL_ON, name.0, &mut id)
            };
            if rc != K_IO_RETURN_SUCCESS {
                self.level = Level::Off;
                return Err(format!("IOPMAssertionCreateWithName failed ({rc:#x})"));
            }
            self.id = Some(id);
            self.level = level;
            Ok(())
        }

        fn release(&mut self) {
            if let Some(id) = self.id.take() {
                unsafe {
                    IOPMAssertionRelease(id);
                }
            }
        }
    }

    impl Drop for Keeper {
        fn drop(&mut self) {
            self.release();
        }
    }
}
