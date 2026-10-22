# Changelog

## [Unreleased]

- Keep awake while an app is open. Pick from the open apps, or list process
  names in the config.
- Stop on battery below a floor (default 20%), with the current charge shown in
  the menu.
- Notifications when a timer ends or the battery floor stops Cafe, with a
  toggle in the menu.
- Hand edits to config.json are picked up without a restart.
- Coffee facts. A new one in the menu every day, plus at most one notification
  a day when you turn Cafe on. Turn them off with "Coffee facts".
- Landing page in `docs/`.
- Picking apps to watch is easier. The list shows apps by their real names
  (OBS Studio, not obs64), leaves out background helpers, and **Choose an
  app…** adds one that isn't open yet, or a command-line program.
- A shorter menu. "Off" and "Keep awake" are one switch, the two timer menus
  are one **Timer** menu, and the preferences live under **Settings**.

## [0.0.1] - 2026-10-20

First release.

- Tray app for Windows and macOS that keeps the machine awake.
- Modes: off, keep awake, keep awake for a set time, or until a time of day.
- "Keep screen on" toggle.
- On Windows, left-click toggles. Launching the app a second time also toggles,
  including reopening Cafe.app from Finder or Spotlight on macOS.
- Run at login. On macOS 13+ it is a regular login item in System Settings.
- Remembers its mode across restarts, including timed sessions that still have
  time left.
- Windows installer (Inno Setup), Chocolatey and winget manifests, a universal
  macOS DMG, and a Homebrew cask.
