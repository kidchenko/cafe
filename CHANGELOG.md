# Changelog

## Unreleased

- Fixed: keeping awake while an app is open was slow to notice the app
  opening or quitting, up to 10 seconds. On macOS it now reacts at once;
  everywhere else, and for programs that aren't apps, Cafe checks every
  5 seconds.

## [0.0.3] - 2026-10-25

- Changed: a tidier menu. Fewer icons and separators, Settings sits with
  Quit, and Check for updates… is in Settings under the version.
  On macOS, icons and checkmarks share one column, so every label lines up.
- Added: while a timer runs, the cup in the menu bar or tray shows a small
  clock.
- Added: Check for updates… in Settings opens Cafe's web page, which tells
  you whether a newer version is out. Cafe itself still never goes online.

## [0.0.2] - 2026-10-22

- Fixed: on macOS 26, the menu bar menu closed the first time you hovered over
  or clicked a submenu after launching Cafe.
- Fixed: while a timer was running, an open menu could close when the minute
  changed. The countdown now updates in place.

## [0.0.1] - 2026-10-22

First release.

- A coffee cup in the tray (Windows) or menu bar (macOS) that keeps your
  computer awake.
- Keep awake until you turn it off, for a set time, or until a time of day,
  with the screen on too if you want.
- Keep awake while an app is open. Pick from the apps you have open, choose
  one with the file picker, or list process names in the config.
- Stop on battery below a limit you choose (20% by default).
- Notifications when a timer ends or a low battery stops Cafe.
- Coffee facts: a new one in the menu every day.
- Run at login. On macOS 13+ it's a regular login item in System Settings.
- On Windows, left-click the cup to switch it on or off. Opening Cafe again
  does the same on both systems, including from Finder or Spotlight on macOS.
- Remembers what it was doing across restarts, including a timer that still
  has time left. Picks up hand edits to config.json without a restart.
- Windows installer, Chocolatey and winget manifests, a universal macOS DMG
  (Apple silicon and Intel), and a Homebrew cask.
