# Cafe

<img src="assets/icon-256.png" width="96" align="right" alt="">

Keep your Mac or PC awake from the tray. It works like PowerToys Awake or
Amphetamine, and the same small binary runs on Windows and macOS.

| icon | state |
|------|-------|
| outline cup | **Off**: your normal power plan applies |
| filled cup | **Awake**: the system won't idle-sleep, but the screen may turn off |
| cup with steam | **Awake, screen on**: neither the system nor the display sleeps |

## Menu

- **Off (use power plan)**: drops the request, so the OS goes back to your own settings.
- **Keep awake indefinitely**
- **Keep awake for ▸** 15 min / 30 min / 1 h / 2 h / 4 h / 8 h, then back to Off.
  The time left shows at the top of the menu and in the tooltip.
- **Keep awake until ▸** one of the next eight whole hours (17:00, 18:00 and so on),
  then back to Off.
- **Keep awake while app runs ▸**: stays awake while any app on your list is
  running, whatever the mode. Add apps from **Add running app ▸**, which lists
  apps with a window. Use **Edit app list…** for anything without a window, like
  `ffmpeg` or `cargo`. Click a listed app to remove it.
- **Keep screen on**: also keeps the display awake.
- **Stop on battery below ▸** Never / 10% / 20% / 30% / 50% (default 20%). On
  battery below the floor, Cafe ends any session and ignores watched apps until
  you plug in. The current charge shows in the menu.
- **Notifications**: notifies you when a timer runs out or the battery floor
  stops Cafe.
- **Coffee facts**: a "did you know?" line near the bottom of the menu that
  changes daily. Click it to see the full fact and get the next one. The first
  session you start each day also shows one as a notification, and never more
  than one a day.
- **Run at login**
- **Quit Cafe**

On Windows, **left-click** the icon to toggle between Off and awake. On macOS,
clicking opens the menu.

**Launching Cafe again toggles it.** The second launch tells the running
instance to flip and then exits, so you can bind `cafe` to a hotkey or run it
from a terminal.

## Install

| | |
|-|-|
| Windows installer | [Cafe-Setup.exe](https://github.com/kidchenko/cafe/releases/latest/download/Cafe-Setup.exe) |
| winget | `winget install kidchenko.Cafe` |
| Chocolatey | `choco install cafe` |
| macOS | [Cafe.dmg](https://github.com/kidchenko/cafe/releases/latest/download/Cafe.dmg), or `brew install --cask kidchenko/tap/cafe` |

The macOS build is a universal binary (Apple silicon + Intel). It is ad-hoc
signed but not notarized, so the first time you open it from the DMG, use
right-click > Open. The Homebrew cask clears the quarantine flag for you.

## How it works

Cafe never edits your power plan. It asks the OS for a temporary request, and
the OS forgets that request once Cafe withdraws it, quits or crashes, so there
is nothing to restore.

- **Windows**: `SetThreadExecutionState` with `ES_SYSTEM_REQUIRED`, plus
  `ES_DISPLAY_REQUIRED` when the screen is kept on. You can see it with
  `powercfg /requests` (run from an elevated prompt).
- **macOS**: an IOKit power assertion, either `PreventUserIdleSystemSleep` or
  `PreventUserIdleDisplaySleep`. You can see it with `pmset -g assertions`.

Neither one blocks sleep that you ask for yourself, such as Start > Sleep or
closing a MacBook lid on battery.

## Files

| | Windows | macOS |
|-|---------|-------|
| config | `%APPDATA%\Cafe\config.json` | `~/Library/Application Support/Cafe/config.json` |
| log | `%APPDATA%\Cafe\cafe.log` | `~/Library/Application Support/Cafe/cafe.log` |

The current mode is saved. A timed session that hasn't run out yet resumes
after a restart. Settings you can edit in the config (Cafe picks up changes
within a few seconds, no restart needed):

| key | meaning |
|-----|---------|
| `apps` | process names to keep awake for, e.g. `["obs64", "ffmpeg"]`; matching ignores case and `.exe` |
| `batteryFloor` | percent, `0` = never stop |
| `durations` | the "Keep awake for" choices, in minutes |
| `notify` | `true` / `false` |
| `facts` | `true` / `false` |

On Windows, notifications show up as "Cafe" when installed with the installer.
A portable copy shows them under PowerShell's name instead, because Windows
only lets registered apps post notifications under their own name.

## Build

```powershell
.\build.ps1             # Windows: tests, cafe.exe, and the Inno Setup installer in dist\
.\build.ps1 -Choco      # ... plus the Chocolatey .nupkg
.\build.ps1 -Icons      # regenerate assets\icon.ico and the PNGs from the renderer
```

```sh
./build.sh --run                 # macOS: dist/Cafe.app, launched into the menu bar
./build.sh --universal --dmg     # what a release ships
```

The landing page is `docs/index.html`, a single static file. To serve it, turn
on GitHub Pages with "Deploy from a branch", `main`, `/docs`. Pages on a private
repo needs a paid plan.

The icons are drawn in code (`src/icon.rs`). `cafe --dump-icons <dir>` writes
them as raw RGBA, and `assets/make_icons.py` turns that into `icon.ico` and the
PNGs.

## Release

1. Bump `version` in `Cargo.toml` and add a section to `CHANGELOG.md`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`

The `release` workflow builds the DMG on macOS and the installer on Windows,
then publishes both to a GitHub Release. It also rewrites the Chocolatey,
winget and Homebrew manifests with the new version and checksums, and uploads
them as run artifacts. From there:

- **Chocolatey**: pushed automatically if the `CHOCO_API_KEY` secret is set.
- **winget**: submit the `manifest-winget` artifact to
  [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) under
  `manifests/k/kidchenko/Cafe/<version>/` (or use `wingetcreate submit`).
- **Homebrew**: copy the `manifest-homebrew` artifact to `Casks/cafe.rb` in
  `kidchenko/homebrew-tap`.

Running the workflow by hand from the Actions tab is a dry run: everything is
built and uploaded as artifacts, and nothing is published.

## License

MIT
