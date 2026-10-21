# Changelog

## [Unreleased]

- Keep awake while an app runs: pick from running apps, or list process names
  in the config.
- Stop on battery below a floor (default 20%), with the current charge shown in
  the menu.
- Notifications when a timer ends or the battery floor stops Cafe, with a
  toggle in the menu.
- Hand edits to config.json are picked up without a restart.
- Coffee facts: a daily "did you know?" in the menu, plus at most one
  notification a day when a session starts. Toggle with "Coffee facts".
- Landing page in `docs/`.

## [0.0.1] - 2026-10-20

First release.

- Tray app for Windows and macOS that keeps the machine awake.
- Modes: off (normal power plan), indefinitely, for a set duration, or until a
  time of day.
- "Keep screen on" toggle.
- On Windows, left-click toggles. Launching the app a second time also toggles.
- Run at login.
- Remembers its mode across restarts, including timed sessions that still have
  time left.
- Windows installer (Inno Setup), Chocolatey and winget manifests, a universal
  macOS DMG, and a Homebrew cask.
