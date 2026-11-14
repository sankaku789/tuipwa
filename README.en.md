<img src="assets/icon.svg" width="64" height="64" alt="">

# TuiPWA

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[日本語](README.md) | English

A Linux x64 launcher that makes TUI programs feel like standalone desktop apps. TuiPWA uses [Ghostty](https://ghostty.org/) as a shared host, opening a separate, tab-free window for each app.

- Launch registered apps from your desktop application menu.
- Each window has its own PTY and closes when its program exits.

## Installation

### 1. Install Rust / Cargo

Follow the [official Rust installation instructions](https://www.rust-lang.org/tools/install) to install the stable toolchain (with edition 2024 support). Check that `rustc --version` and `cargo --version` work.

### 2. Install system tools and libraries

The table below lists **tool and library names, not package names**. Use your OS package manager to install the corresponding packages.

**Libraries need development files, including headers and pkg-config metadata, not just runtime packages.** Where packaged separately, development packages often have a `-dev` or `-devel` suffix.

| Purpose | What to install |
|---|---|
| Compilation and build tools | C/C++ compiler, Make, CMake, Ninja, pkg-config, Python 3, binutils |
| Source download, extraction, and patching | Git, curl, CA certificates, patch, xz |
| Windows and UI | GTK 4, libadwaita, gtk4-layer-shell |
| GLib infrastructure and resource generation | GLib / GIO / GObject, GObject Introspection, Blueprint compiler, gettext |
| Fonts and text rendering | fontconfig, FreeType, HarfBuzz |
| Images, compression, regular expressions, and XML | libpng, zlib, Oniguruma, libxml2 |
| GPU rendering | SPIRV-Cross, EGL / OpenGL (a graphics driver suitable for your system is also required) |
| Wayland / X11 and keyboard input | Wayland, wayland-protocols, libxkbcommon, X11, libXcursor, libXi, libXrandr |
| Icons and display fonts | Adwaita / hicolor icon themes, system fonts |
| Refreshing Open With candidates (when used) | `update-desktop-database` from desktop-file-utils |

See [dependencies and tools](CONTRIBUTING.md#依存ライブラリツール) (Japanese) for details on their roles and build-time handling.

### 3. Build and install TuiPWA

Before downloading or compiling, `make` checks for common missing commands and development libraries. To run only this check, use `make check-deps` inside the repository. Missing items are listed together and the build stops (this does not guarantee version compatibility or a working GUI environment). Diagnostics are currently in Japanese.

The following are downloaded automatically during the build; **do not install them manually**. The first build requires an internet connection.

- Zig: `make` downloads the required version.
- Dedicated Ghostty host: `make` downloads, patches, and builds the source. A separate installation of standard Ghostty is not required.
- Rust crates: Cargo downloads them according to `Cargo.lock`.

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
| `tuipwa run <id> -- <file>...` | Open files in a registered app |
| `tuipwa list` | List registered apps |
| `tuipwa show <id>` | Show registration details |
| `tuipwa update <id> [options]` | Update only the specified fields |
| `tuipwa remove <id>` | Remove a registration, not the TUI program itself |
| `tuipwa validate <manifest.json>` | Validate a manifest |

### Open files in a registered app

```sh
tuipwa run nano -- ./memo.txt "./notes with spaces.txt"
```

Specify exactly one registration ID before `--`, followed by local file paths. Files are appended to the registered arguments for this launch only. Relative paths become absolute paths based on the **directory from which you invoke the command**. Files do not need to exist yet. The app's working directory and saved registration remain unchanged.

Everything after `--` is treated as a file path. Register app options using `add` / `update` instead. URLs are not interpreted, and arguments are not reinterpreted by a shell.

### Register as an Open With candidate

```sh
tuipwa add nano                         # Detect MIME types from existing .desktop entries
# Specify types explicitly, or update an existing registration
tuipwa update nano --mime-type text/plain --mime-type text/markdown
tuipwa show nano
```

New registrations look for an existing `.desktop` entry with a matching executable in the XDG application directories and inherit its `MimeType` declaration. This also applies to `add --manifest`. Detected types register the app as an Open With candidate in KDE and other desktops. No app-name lookup table is used and no inspected commands are executed.

If detection fails or you want to override it, specify supported file types using `--mime-type`. Wrapper commands and executable paths with complex escaping are not inferred. Multiple selected local files are passed to a single app launch. Normal menu launches without files still work. **Default application settings are not changed.**

On `update`, `--mime-type` replaces the supported list; omitting it keeps the list. Use `tuipwa update nano --clear-mime-types` to disable this support. If neither explicit nor detected types are available, no file-opening support is declared. Updating existing registrations does not rerun detection, preserving manual settings and disabled support. Repeat the option for each type rather than using wildcards or a semicolon-separated list.

The candidate cache is refreshed using `update-desktop-database`. If the tool is missing or fails, a warning is shown, but the registration is saved. Run the command shown in the warning to refresh it later. Desktop-specific caches can also affect when candidates appear.

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
