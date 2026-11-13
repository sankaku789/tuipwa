<img src="assets/icon.svg" width="64" height="64" alt="">

# TuiPWA

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[日本語](README.md) | English

A Linux x64 launcher that makes TUI programs feel like standalone desktop apps. TuiPWA uses [Ghostty](https://ghostty.org/) as a shared host, opening a separate, tab-free window for each app.

- Launch registered apps from your desktop application menu.
- Each window has its own PTY and closes when its program exits.

## Installation

Required dependencies (including development files for libraries):

- [Rust / Cargo](https://www.rust-lang.org/tools/install) (stable)
- C/C++ compiler, Make, CMake, Ninja, pkg-config, Git, curl, CA certificates, patch, Python 3, binutils, xz
- GTK 4, libadwaita, gtk4-layer-shell, GLib / GIO / GObject, GObject Introspection, Blueprint, gettext
- fontconfig, FreeType, HarfBuzz, libpng, zlib, Oniguruma, libxml2, SPIRV-Cross
- Wayland, wayland-protocols, libxkbcommon, X11, libXcursor, libXi, libXrandr, EGL / OpenGL
- Adwaita / hicolor icon themes and system fonts

Zig is downloaded automatically during the build.

```sh
git clone https://github.com/sankaku789/tuipwa.git
cd tuipwa
make install
```

The CLI is normally installed in `~/.cargo/bin`, and the host in `~/.local/share/tuipwa/native/`. You can run `tuipwa` once Cargo's `bin/` directory is on your PATH. Do not use sudo for `make install`.

The patched Ghostty host is built and installed automatically. GTK and other system libraries are provided by your OS, not bundled. The first build can take some time.

## Usage

Install the TUI programs you want to run, then register and launch them:

```sh
tuipwa add btop
tuipwa run btop
```

Registered apps also appear in your desktop application menu. `run`, `show`, `update`, and `remove` take a registration ID, which defaults to the executable name.

### Commands

| Command | Action |
|---|---|
| `tuipwa add <command>` | Register an app |
| `tuipwa run <id> [id ...]` | Launch registered apps |
| `tuipwa list` | List registered apps |
| `tuipwa show <id>` | Show registration details |
| `tuipwa update <id> [options]` | Update only the specified fields |
| `tuipwa remove <id>` | Remove a registration, not the TUI program itself |
| `tuipwa validate <manifest.json>` | Validate a manifest |

### Name, arguments, and working directory

```sh
tuipwa add vim --id notes --name "Notes" --cwd ~/Documents -- ~/notes.txt
tuipwa run notes
tuipwa show notes
```

Use `--id` for the registration ID, `--name` for the menu label, `--title` for the window title, and `--cwd` for the working directory. Arguments after `--` are passed directly to the TUI without shell reinterpretation.

### Icons and categories

```sh
tuipwa update btop --color "#2563EB" --category System --category Monitor
tuipwa add vim --id editor --icon ./editor.png
```

`--color` sets the background of a generated icon. `--icon` selects an existing PNG/ICO file; the two options cannot be combined. When neither is given to `add`, the icon is selected automatically. The default category is `Utility`.

Duplicate IDs are not allowed. Use `update` for an existing registration.

### Update or remove a registration

```sh
tuipwa update notes --name "My Notes"
tuipwa update notes -- ~/Documents/todo.txt       # Replace arguments
tuipwa update notes --clear-args                 # Clear arguments
tuipwa update btop --color "#0F766E"
tuipwa update btop --auto-icon                    # Restore a generated icon
tuipwa remove notes
```

Updates take effect the next time the app is launched.

### Register from JSON

```sh
tuipwa validate examples/nano.json
tuipwa add --manifest examples/nano.json
```

Run `tuipwa --help` for all options. See [manifest v1](docs/MANIFEST.md) (Japanese) for the JSON format.

### Help language

Help uses `LC_ALL`, then `LC_MESSAGES`, then `LANG`, skipping empty values. Japanese locales select Japanese; all other locales, including an unset locale, select English.

```sh
env LC_ALL=C tuipwa --help                  # English
env LC_ALL=ja_JP.UTF-8 tuipwa --help       # Japanese
```

This language selection applies to help text only. Other output and error messages are not fully translated.

## Development and specifications

- [CONTRIBUTING.md](CONTRIBUTING.md) (Japanese) — dev builds, tests, dependencies, internals, distribution, and desktop checks
- [manifest v1](docs/MANIFEST.md) (Japanese) — registration JSON specification

## License

TuiPWA is licensed under the [MIT License](LICENSE). Ghostty, its dependencies, and fonts retain their respective licenses. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
