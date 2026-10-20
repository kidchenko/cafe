# Changelog

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
