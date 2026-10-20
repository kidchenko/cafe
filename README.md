<div align="center">

<img src="assets/icon-256.png" width="112" height="112" alt="Cafe">

# Cafe

Keep your Mac or PC awake, from the tray.<br>
For downloads, renders, presentations, long builds, or just reading.

<sub><i>café</i> — Portuguese for <i>coffee</i></sub>

</div>

Cafe is a coffee cup in the tray. Click it and your computer stays awake; click
again and your normal power plan takes over. It does what PowerToys Awake and
Amphetamine do, on both Windows and macOS.

A single 0.5 MB executable, no runtime dependencies. Sits in the tray using
about 2 MB of private memory and no measurable CPU.

## The cup

| | |
|---|---|
| **Outline cup** | Off. Your normal power plan applies. |
| **Filled cup** | Awake. The system won't idle-sleep, but the screen may still turn off. |
| **Cup with steam** | Awake, screen on. Neither the system nor the display sleeps. |

On Windows, **left-click** toggles between off and awake, and **right-click**
opens the menu. On macOS, clicking opens the menu, like every other menu bar
item.

Running `cafe` again while it's already in the tray toggles the running copy
instead of starting a second one. Pin it, bind it to a hotkey, or run it from a
terminal. On macOS, opening Cafe.app again from Finder or Spotlight toggles it
too.

## The menu

| | |
|---|---|
| **Off (use power plan)** | Drops the request. The OS goes back to your own settings. On macOS it reads **Off (use Energy settings)**. |
| **Keep awake indefinitely** | Until you switch it off. |
| **Keep awake for ▸** | 15 min, 30 min, 1 h, 2 h, 4 h or 8 h, then back to off. The time left is at the top of the menu and in the tooltip. |
| **Keep awake until ▸** | One of the next eight whole hours (17:00, 18:00 and so on). |
| **Keep awake while app runs ▸** | Stays awake while any app on your list is running, whatever the mode. [More below.](#keep-awake-while-an-app-runs) |
| **Keep screen on** | Also keeps the display awake. |
| **Stop on battery below ▸** | Never, 10%, 20%, 30% or 50%. Default 20%. [More below.](#battery) |
| **Notifications** | When a timer runs out or the battery floor stops Cafe. |
| **Coffee facts** | A daily "did you know?" line in the menu. [More below.](#coffee-facts) |
| **Run at login** | Per-user, no admin. On macOS, move Cafe.app to Applications first. |

The mode is saved. A timer that hasn't run out yet picks up where it left off
after a restart; one that ran out while Cafe was closed comes back off.

### Keep awake while an app runs

For the things you'd otherwise forget to switch on and off around: OBS, a game,
a video call, a long build.

- **Add running app ▸** lists the apps that have a window right now. Pick one
  and it's on the list.
- **Edit app list…** opens the config file, for things without a window, like
  `ffmpeg`, `cargo` or `rsync`.
- Click a listed app to remove it.

Matching uses the executable name, ignoring case and `.exe`, so `OBS64.exe` and
`obs64` are the same entry. Cafe checks every 10 seconds while the list isn't
empty, and the menu header shows which app is holding it awake.

### Battery

On battery and below the floor, Cafe ends any session and ignores watched apps
until you plug in. It tells you once when that happens, and the menu shows the
current charge. On a desktop with no battery the option is greyed out.

It's on by default (20%) because a laptop kept awake on battery overnight is the
one problem Cafe can cause rather than prevent.

### Coffee facts

One fact a day sits near the bottom of the menu: where coffee comes from, what a
cortado is, why it's called mocha. Click it to read it in full and get the next
one. The first session you start each day also shows one as a notification, and
never more than one a day.

Forty facts, each one checked. Where the popular story is a legend, the fact
says so.

## Install

### Windows

```powershell
winget install kidchenko.Cafe
# or
choco install cafe
```

Or grab [**Cafe-Setup.exe**](https://github.com/kidchenko/cafe/releases/latest/download/Cafe-Setup.exe)
directly. Each release also ships a portable `cafe-x.y.z-x64.exe` that runs
without installing; see [Releases](https://github.com/kidchenko/cafe/releases).

The binary isn't code-signed, so SmartScreen warns on first run: click **More
info → Run anyway**.

Use the installer if you want notifications to say "Cafe". Windows takes a
notification's name from a Start menu shortcut, and only the installer creates
one; a portable copy shows them under PowerShell's name instead. The installer
also offers to start Cafe when you sign in.

### macOS

```bash
brew install --cask kidchenko/tap/cafe
```

Or download [**Cafe.dmg**](https://github.com/kidchenko/cafe/releases/latest/download/Cafe.dmg)
(Apple silicon and Intel), open it, and drag Cafe into **Applications**. It
lives in the menu bar with no Dock icon.

The build isn't notarized, so Gatekeeper blocks the first launch from the DMG.
The Homebrew cask clears this for you; otherwise, do it once:

- **System Settings → Privacy & Security → Open Anyway** next to Cafe; or
- `xattr -dr com.apple.quarantine /Applications/Cafe.app`

Move it into Applications *first*: the quarantine flag can't be cleared while
the app is still on the read-only disk image.

### Requirements

Windows 10 or later. macOS 11 or later.

## How it works

Cafe never edits your power plan. It asks the OS for a temporary request
instead, and the OS ends that request as soon as Cafe withdraws it, quits, or
crashes. There is nothing to restore and nothing to get stuck.

| | Awake | Screen on |
|---|---|---|
| **Windows** | `SetThreadExecutionState(ES_SYSTEM_REQUIRED)` | `+ ES_DISPLAY_REQUIRED` |
| **macOS** | IOKit assertion `PreventUserIdleSystemSleep` | `PreventUserIdleDisplaySleep` |

You can see the request yourself while Cafe is awake:

```powershell
powercfg /requests        # Windows, from an admin prompt: cafe.exe under SYSTEM (and DISPLAY)
```

```bash
pmset -g assertions       # macOS: an assertion named "Cafe"
```

Neither one blocks sleep you ask for yourself: Start → Sleep, the power button,
or closing a MacBook lid on battery. That is the OS's call, and the right one.

Battery state comes from `GetSystemPowerStatus` on Windows and IOKit's power
source list on macOS. Running apps come from a Toolhelp snapshot on Windows and
`libproc` on macOS. All of it is read on a timer; nothing hooks into other
processes, and nothing leaves the machine.

The tray and cup icons are drawn in code from signed distance functions
(`src/icon.rs`), so there are no image assets to keep in sync.

## Configuration

`%APPDATA%\Cafe\config.json` on Windows,
`~/Library/Application Support/Cafe/config.json` on macOS. The menu covers
everything, but the file is there if you prefer it. Edits are picked up within
a few seconds; no restart needed.

```json
{
  "mode": { "kind": "off" },
  "keepScreenOn": true,
  "durations": [15, 30, 60, 120, 240, 480],
  "batteryFloor": 20,
  "apps": ["obs64", "ffmpeg"],
  "notify": true,
  "facts": true
}
```

| Key | |
|---|---|
| `durations` | The "Keep awake for" choices, in minutes. 1 minute to 1 week. |
| `batteryFloor` | Percent. `0` means never stop. Capped at 95. |
| `apps` | Process names. Case and `.exe` don't matter. |
| `mode` | Cafe's own state; edits to it are ignored while Cafe is running. |

## Troubleshooting

Cafe logs to `cafe.log` next to the config. It records every mode change, every
watched app starting and stopping, and the battery floor kicking in:

```
INFO  mode=Indefinite trigger=None level=SystemAndScreen
INFO  obs64 is running
INFO  battery below 20%: ending Indefinite
INFO  toast shown: Battery at 19% / Cafe won't keep your PC awake on battery below 20%.
```

**My computer slept anyway.** Check the first line of the menu: it says why Cafe
is off. Then check `powercfg /requests` or `pmset -g assertions` while it's
awake. If Cafe is listed there and the machine still sleeps, something asked for
sleep explicitly (a lid close, a scheduled task, a power button), which Cafe
doesn't override by design.

**A watched app isn't keeping it awake.** Compare the name in the menu with the
process name in Task Manager's **Details** tab, or Activity Monitor. The
**Processes** tab shows a friendly name, which isn't what Cafe matches.

**Notifications say "Windows PowerShell".** You're running the portable exe;
see [Install](#windows). If there's no notification at all, check Focus Assist
or Do Not Disturb.

**Launching Cafe again does nothing visible.** It did something: the second
launch toggled the running copy, then exited. Look at the cup.

## Build

```powershell
winget install Rustlang.Rustup
choco install innosetup -y     # only for the installer
```

```powershell
.\build.ps1                    # tests + release binary + installer, in dist\
.\build.ps1 -Run               # ...then launch it
.\build.ps1 -Choco             # ...plus the Chocolatey package
.\build.ps1 -Icons             # regenerate assets\icon.ico and the PNGs
.\build.ps1 -SkipInstaller     # binary only
```

```bash
./build.sh                     # tests + release binary + dist/Cafe.app
./build.sh --run               # ...then launch it in the menu bar
./build.sh --universal --dmg   # what a release ships: arm64 + x86_64, in a .dmg
```

Needs only the Rust toolchain. The cup, the menu and the rules are shared Rust;
only the OS-specific pieces fork (execution state vs IOKit, Toolhelp vs
libproc, WinRT toasts vs `osascript`).

`cargo test` covers the rules that don't need a desktop: timers and clock
labels, the battery floor, app-name matching, config parsing, the facts list,
and the icon renderer.

The landing page is [`docs/index.html`](docs/index.html), a single static file
served by GitHub Pages from `/docs`.

## Releasing

1. Bump `version` in `Cargo.toml` and add a section to `CHANGELOG.md`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`

The `release` workflow builds the DMG on macOS and the installer on Windows,
publishes both to a GitHub Release, and rewrites the Chocolatey, winget and
Homebrew manifests with the new version and checksums:

- **Chocolatey** is pushed automatically when the `CHOCO_API_KEY` secret is set.
- **winget**: submit the `manifest-winget` artifact to
  [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), or use
  `wingetcreate submit`.
- **Homebrew**: copy the `manifest-homebrew` artifact to `Casks/cafe.rb` in
  `kidchenko/homebrew-tap`.

Running the workflow by hand from the Actions tab is a dry run: everything is
built and uploaded as artifacts, nothing is published.

## Not yet

- **Choose your coffee**: espresso, cappuccino, mocha and more as cup styles,
  plus a brewing animation while awake ([#1](https://github.com/kidchenko/cafe/issues/1)).
- **A signed, notarized macOS build**: would drop the Gatekeeper step and let
  notifications carry Cafe's own name.
- **Code signing on Windows**: would drop the SmartScreen warning.

## License

MIT.
