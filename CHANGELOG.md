# Changelog

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
