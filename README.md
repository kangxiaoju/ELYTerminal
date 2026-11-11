# ELYTerminal

[![Release](https://img.shields.io/github/v/release/ELY-Open-Source/ELYTerminal?style=flat-square&color=24292f)](https://github.com/ELY-Open-Source/ELYTerminal/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-3da639?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.95-dea584?style=flat-square)](https://www.rust-lang.org)
[![GPUI Fast](https://img.shields.io/badge/GPUI_Fast-0.1.14-5b4cff?style=flat-square)](https://github.com/longbridge/gpui-fast)
[![Ely](https://img.shields.io/badge/Ely-components-6d28d9?style=flat-square)](https://github.com/ELY-Open-Source/Ely-GPUI-Components)
[![alacritty](https://img.shields.io/badge/alacritty-0.26-f46d01?style=flat-square)](https://github.com/alacritty/alacritty)

English / [中文](README_zh.md)

A terminal for macOS, Windows and Linux, in Rust on GPUI and Ely components. Sessions sit on the left, the terminal in the middle, Git or Files on the right. Every setting has a control in the settings window.

## Install

A [release on GitHub](https://github.com/ELY-Open-Source/ELYTerminal/releases) carries builds for each platform.

- macOS 11 or later, Apple silicon: open `ELYTerminal-VERSION.dmg` and drag ELYTerminal into Applications. The app is signed with Developer ID and notarized. With Homebrew: `brew install --cask ely-open-source/tap/elyterminal`.
- Windows 10 or later, x86_64: unzip `ELYTerminal-VERSION-windows-x86_64.zip` and run `elyterminal.exe`, the zip's other files kept beside it. With winget, once Microsoft's catalog holds it: `winget install ELYOpenSource.ELYTerminal`. It is unsigned; SmartScreen and Defender are told apart in [Windows](docs/en_us/install.md#windows).
- Linux, x86_64: [packages](docs/en_us/install.md).

## What it does

- Sessions sit on the left, the terminal in the middle, Git or Files on the right.
- Shell integration for zsh, bash, fish and PowerShell, so Files and Git follow the session's folder.
- Profiles beside +: your own command lines, and the shells this system holds, WSL distributions and Git Bash among them.
- Ely peeks over the window's edge and follows the sessions.
- Images through kitty's graphics protocol, animations too, and the clipboard through kitty's OSC 5522, which asks you before a program reads what it holds, unless you allowed that program for the session or it answers a paste.
- Background opacity and blur on macOS, Windows, Wayland and X11, and Mica on Windows 11.

The rest is in [docs/en_us/features.md](docs/en_us/features.md).

## Keys

Settings › Terminal lists them all, as this platform binds them. The table is in [docs/en_us/keys.md](docs/en_us/keys.md).

## Settings

The settings window saves each change at once to `settings.json` in the platform's config folder:

- macOS: `~/Library/Application Support/ELYTerminal/settings.json`
- Linux: `$XDG_CONFIG_HOME/ELYTerminal/settings.json`, else `~/.config/ELYTerminal/settings.json`
- Windows: `%APPDATA%\ELYTerminal\settings.json`

A file that does not parse, or holds a value out of range, stops the start with an error naming the file and the field. The app never overwrites it.

Settings › Appearance › Language shows the interface in English or 简体中文.

Settings › Terminal checks for a newer release at start and once a day, or at once with Check now; a newer one shows a Download button that opens its page. The check runs `curl`, which macOS and Windows ship.

## Build

Rust 1.95.0, which `rust-toolchain.toml` pins; rustup installs it on the first build.

```sh
cargo run --release
```

Release scripts, Linux libraries and the log are in [docs/en_us/develop.md](docs/en_us/develop.md).

This branch uses GPUI Fast directly, without GPUI Kit. Embedded hosts must resolve
the same fixed Fast source as Ely and the terminal; enabling Fast only in the host
does not migrate its dependencies. Native platform defaults are disabled, with
`font-kit`, `wayland` and `x11` selected explicitly. The existing PTY, parser,
host-owned settings and visibility APIs are unchanged.

GPUI Fast 0.1.14's upstream Apple renderer requires macOS 12 and SDK 26; existing
release downloads above describe their own older build, not verification of this
migration branch. macOS and Windows native validation must be recorded separately.

Embedded hosts own the `TerminalView` entity and close a session by releasing it.
The native input handler keeps only a weak entity reference: deferred platform
window teardown must not keep the terminal or its PTY alive after host shutdown.

Terminal messages (including status notifications, command completion and clipboard
prompts) use the terminal's own English and Simplified Chinese catalogs. Embedded
hosts keep their existing Ely `I18n` global and custom messages: every terminal
lookup reads its current locale (`en-US` or `zh-CN`), so host locale changes apply
immediately without replacing the host catalog or installing standalone settings
observers. Hosts must initialize their `I18n` before rendering terminal UI.

On Windows, embedded initialization also reads the system wheel settings before
terminal input is handled. These settings are shared process-wide: repeated
embedded initialization or later standalone window creation reuses them, while
an initial Windows settings-query failure is returned to the caller.

This fork integrates upstream 0.2.2 at `ba664ef6ef0cdfcb35dc6b65d538cc58e7ecd313`
while retaining host-owned initialization, theme and settings, weak input ownership,
PTY foreground probes and `RecordsChanged` events. Its new profile, Files panel and
appearance controls use the same host-locale terminal catalogs as existing controls.
Embedded hosts can continue running an external PTY bridge through `term::spawn`
and observing agent records through `TerminalView::records` without starting the
standalone application's services. Hosts that patch Ely must provide the upstream
`TreeDrag` and `SegmentedControl::on_context` APIs and resolve one Ely instance.

Hosts can call `TerminalView::set_notification_policy(lifecycle, system)` before
events arrive. Disabling lifecycle notices delegates status/command completion to
the host while retaining custom OSC notifications; disabling system notices
honors a host's system preference. Status records and protocol queries always
remain active. `set_front(false, window)` cancels pending graphics animation and
cursor blinking without stopping terminal output or the child process.

Hosts that run the PTY bridge themselves can wait for the grid to catch up with a count of raw
output bytes: `TerminalView::set_output_barrier(bytes, cx)` emits
`TerminalEvent::OutputBarrier(bytes)` once the parser has taken the PTY's first `bytes` bytes (at
once if it already has; one target at a time). The count is the tap's read count committed at the
parser's wakeup, not bytes merely read, so nothing is injected into the stream and nothing is held
back. Inside a synchronized update (`?2026`) the grid shows the last complete screen. Nothing is
emitted unless the host calls it. `Pty::output_processed` and `PtyEvent::OutputProcessed` expose the
same count.

`TerminalView::set_resize_settle(Some(quiet))` is an opt-in for hosts whose programs redraw their
whole history after a resize. After the grid is resized the view keeps drawing the last whole screen
until output has rested for `quiet`, at least 500 ms have passed and at most 1 s after the latest
size; a key, mouse press or wheel turn ends it at once. Parsing, the PTY and the program are not
held back, and `None` (the default) holds no frame and starts no timer.

## License

MIT, in `LICENSE`. Box drawing and key encoding follow Ghostty's code, MIT, in `LICENSE-GHOSTTY`. The theme files come from iTerm2-Color-Schemes, MIT, with its license in `assets/themes`. JetBrains Mono and its Nerd Font build are under the SIL Open Font License, in `assets/fonts`. The compound text decoder's tables come from libX11, under SunSoft's and Bruno Haible's notice, in `vendor/xim-ctext/tables/LICENSE`. Microsoft's ConPTY, `conpty.dll` and `OpenConsole.exe`, is MIT, in `vendor/conpty/LICENSE`.
