<div align="center">

<img src="assets/icon-256.png" width="112" height="112" alt="Cafe">

# Cafe

Keep your Mac or PC awake, from the tray.<br>
For downloads, renders, presentations, long builds, or just reading.

<sub><i>Café</i> is Portuguese for <i>coffee</i>.</sub>

</div>

Cafe is a coffee cup in the tray. Click it and your computer stays awake. Click
it again and your computer goes back to sleeping the way it normally does. It's
like PowerToys Awake or Amphetamine, and it runs on both Windows and macOS.

It's one small executable (about 0.5 MB) with nothing else to install, and it
uses about 2 MB of memory while it sits there.

## The cup

| | |
|---|---|
| **Outline cup** | Off. Your computer sleeps as usual. |
| **Filled cup** | Awake. The computer won't go to sleep, but the screen can still turn off. |
| **Cup with steam** | Awake with the screen on. Nothing goes to sleep. |

On Windows, **left-click** toggles between off and awake, and **right-click**
opens the menu. On macOS, clicking opens the menu, same as any other menu bar
icon.

Running `cafe` again while it's already in the tray toggles the running copy
instead of starting a second one. Pin it, bind it to a hotkey, or run it from a
terminal. On macOS, opening Cafe.app again from Finder or Spotlight toggles it
too.

## The menu

The top line says what Cafe is doing right now. Under it:

| | |
|---|---|
| **Keep awake** | Stays awake until you turn it off. While Cafe is on, this reads **Turn off**. |
| **Timer** | 15 min, 30 min, 1 h, 2 h, 4 h or 8 h, or until one of the next six whole hours (17:00, 18:00 and so on). Then it turns itself off. The menu and the tooltip show how long is left. |
| **While an app is open** | Stays awake whenever an app on your list is running. [More below.](#keep-awake-while-an-app-is-open) |
| **Keep screen on** | Keeps the display on too. |

**Settings** holds the things you set once:

| | |
|---|---|
| **Stop on battery below** | Never, 10%, 20%, 30% or 50%. The default is 20%. [More below.](#battery) |
| **Notifications** | Lets you know when a timer ends or a low battery stops Cafe. |
| **Coffee facts** | A coffee fact of the day at the bottom of the menu. [More below.](#coffee-facts) |
| **Run at login** | Just for your user, no admin needed. On macOS it shows up in System Settings > General > Login Items, and you can turn it off there too. Move Cafe.app to Applications first. |

Cafe remembers what it was doing. If you restart in the middle of a timer, the
timer carries on. If it ran out while Cafe was closed, Cafe starts up off.

### Keep awake while an app is open

Handy for things you'd forget to babysit: OBS, a game, a video call, a long
build.

- **Add an open app** lists the apps you have open right now, by the names you
  know them by (OBS Studio, Visual Studio Code). Pick one to add it.
- **Choose an app…** opens the usual file picker, so you can add an app that
  isn't open yet. You can pick a command-line program like `ffmpeg` or `rsync`
  here too.
- **Edit app list…** opens the config file, if you'd rather type names in.
- Click an app on the list to remove it.

Behind the scenes Cafe matches on the program's file name and ignores case and
`.exe`, so `OBS64.exe` and `obs64` count as the same app. While the list has anything on
it, Cafe checks every 10 seconds, and the top of the menu tells you which app
is keeping it awake.

### Battery

If you're on battery and the charge drops below your limit, Cafe turns itself
off and ignores your app list until you plug in again. You get one notification
when that happens, and the menu shows the current charge. On a desktop with no
battery, the option is greyed out.

It's on by default (at 20%) so a laptop left awake on battery doesn't run flat
overnight.

### Coffee facts

There's a new fact at the bottom of the menu every day, like where coffee
comes from, what a cortado is, or why it's called mocha. Click it to see the
next one. The first time you turn Cafe on each day, you also get one as a
notification. That's the only one you'll get that day.

There are forty of them, and each one has been fact-checked. If the popular story is just
a legend, the fact says so.

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

The app isn't code-signed yet, so SmartScreen warns you the first time you run
it. Click **More info**, then **Run anyway**.

If you want notifications to show up as "Cafe", use the installer. Windows
takes a notification's name from the Start menu shortcut, and only the
installer makes one. The portable copy's notifications show up as PowerShell.
The installer can also start Cafe when you sign in.

### macOS

```bash
brew install --cask kidchenko/tap/cafe
```

Or download [**Cafe.dmg**](https://github.com/kidchenko/cafe/releases/latest/download/Cafe.dmg)
(Apple silicon and Intel), open it, and drag Cafe into **Applications**. It
lives in the menu bar and doesn't show up in the Dock.

The app isn't notarized yet, so macOS blocks it the first time you open it.
Homebrew takes care of that for you. Otherwise, do one of these once:

- Go to **System Settings > Privacy & Security** and click **Open Anyway**
  next to Cafe.
- Run `xattr -dr com.apple.quarantine /Applications/Cafe.app`

Move it into Applications before you do this. It won't work while the app is
still inside the disk image.

### Requirements

Windows 10 or later. macOS 11 or later.

## How it works

Cafe never changes your power settings. It asks the OS to stay awake for now,
and the OS drops that request as soon as Cafe takes it back, quits or crashes.
So there's nothing to undo, and your computer can't get stuck awake.

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

Neither one stops sleep you ask for yourself, like Start > Sleep, the power
button, or closing a MacBook lid on battery. Cafe leaves those alone on
purpose.

Battery state comes from `GetSystemPowerStatus` on Windows and IOKit's power
source list on macOS. The list of running apps comes from a Toolhelp snapshot on
Windows and `libproc` on macOS. Cafe just reads these every so often. It doesn't
hook into other programs, and nothing leaves your computer.

The icons are drawn in code (`src/icon.rs`). The images in `assets/` and
`docs/img/` are generated from it with `.\build.ps1 -Icons`.

## Configuration

`%APPDATA%\Cafe\config.json` on Windows,
`~/Library/Application Support/Cafe/config.json` on macOS. You can do
everything from the menu, but you can edit the file if you'd rather. Cafe
notices changes within a few seconds, so there's no need to restart it.

```json
{
  "mode": { "kind": "off" },
  "keepScreenOn": true,
  "durations": [15, 30, 60, 120, 240, 480],
  "batteryFloor": 20,
  "apps": ["obs64", "ffmpeg"],
  "appNames": { "obs64": "OBS Studio" },
  "notify": true,
  "facts": true
}
```

| Key | |
|---|---|
| `durations` | The **Timer** choices, in minutes. 1 minute to 1 week. |
| `batteryFloor` | Percent. `0` means never stop. Capped at 95. |
| `apps` | Process names. Case and `.exe` don't matter. |
| `appNames` | What the menu calls each app, keyed by process name. Filled in for you when you add an app from the menu. |
| `mode` | What Cafe is doing right now. Cafe ignores edits to it while it's running. |

## Troubleshooting

Cafe writes a log to `cafe.log`, next to the config. It notes every mode
change, every app on your list opening and closing, and the battery limit
kicking in:

```
INFO  mode=Indefinite trigger=None level=SystemAndScreen
INFO  obs64 is running
INFO  battery below 20%: ending Indefinite
INFO  toast shown: Battery at 19% / Cafe let go so your PC can sleep and save power.
```

**My computer slept anyway.** Look at the first line of the menu, which says
why Cafe is off. Then run `powercfg /requests` or `pmset -g assertions` while
it's awake. If Cafe shows up there and the computer still sleeps, something
asked for sleep directly, like closing the lid, a scheduled task or the power
button. Cafe doesn't override those.

**An app on my list isn't keeping it awake.** Compare the name in the menu with
the one in Task Manager's **Details** tab, or in Activity Monitor. The
**Processes** tab shows a friendlier name, and that's not what Cafe looks for.

**Notifications say "Windows PowerShell".** You're running the portable exe.
See [Install](#windows). If you get no notifications at all, check Focus Assist
or Do Not Disturb.

**Opening Cafe again doesn't seem to do anything.** It did. The second copy
switched the running one on or off, then quit. Check the cup.

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

All you need is the Rust toolchain. The cup, the menu and the logic are shared.
Only the OS-specific parts are split: execution state vs IOKit, Toolhelp vs
libproc, and WinRT toasts vs `osascript`.

`cargo test` covers everything that doesn't need a desktop: timers and clock
labels, the battery limit, app-name matching, config parsing, the facts list
and the icon drawing.

The landing page is [`docs/index.html`](docs/index.html), a single static file
served by GitHub Pages from `/docs`.

## Releasing

1. Bump `version` in `Cargo.toml` and add a section to `CHANGELOG.md`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`

The `release` workflow builds the DMG on macOS and the installer on Windows,
puts both on a GitHub Release, and updates the Chocolatey, winget and Homebrew
manifests with the new version and checksums:

- **Chocolatey** is pushed automatically when the `CHOCO_API_KEY` secret is set.
- **winget**: submit the `manifest-winget` artifact to
  [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), or use
  `wingetcreate submit`.
- **Homebrew**: copy the `manifest-homebrew` artifact to `Casks/cafe.rb` in
  `kidchenko/homebrew-tap`.

If you run the workflow by hand from the Actions tab, it's a dry run. It builds
everything and uploads it as artifacts, but doesn't publish anything.

## Not yet

- **Pick your coffee.** Espresso, cappuccino, mocha and more as cup styles,
  plus a brewing animation while it's awake ([#1](https://github.com/kidchenko/cafe/issues/1)).
- **A signed, notarized Mac build.** No more Gatekeeper step, and
  notifications would show up as Cafe.
- **Code signing on Windows.** No more SmartScreen warning.

## License

MIT.
