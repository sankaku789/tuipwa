# Third-Party Notices

TuiPWA's own code, documentation, assets, and Ghostty patches are covered by the MIT license in `LICENSE`. Third-party components retain their original copyrights and licenses.

## Bundled components

Paths below are relative to the distribution directory.

| Component | License and notices |
|---|---|
| [Ghostty](https://github.com/ghostty-org/ghostty) 1.4.0 | MIT; see `share/licenses/ghostty/LICENSE`. Includes TuiPWA patches for per-window identity, title/cwd whitespace preservation, and unused-host shutdown. |
| Ghostty dependencies, resources, and fonts | Respective licenses, including font OFL terms; collected notices are in `share/licenses/upstream/`. |
| Rust dependencies | Versions and license declarations are listed in `share/licenses/rust-dependencies.tsv`; collected notices are in `share/licenses/rust/`. The inventory may also include build-only dependencies. |

## System dependencies (not bundled)

[GTK](https://gitlab.gnome.org/GNOME/gtk), [libadwaita](https://gitlab.gnome.org/GNOME/libadwaita), [GLib/GIO/GObject](https://gitlab.gnome.org/GNOME/glib), and other system libraries are dynamically linked and installed through the operating system. GTK, libadwaita, and GLib are subject to LGPL-2.1-or-later and applicable component-specific terms.

The LGPL 2.1 text is included in `share/licenses/system/LGPL-2.1.txt` (repository copy: `licenses/LGPL-2.1.txt`). Consult your distribution's packages and source packages for version-specific notices and corresponding source. TuiPWA imposes no additional restrictions on replacing these libraries or reverse engineering to debug modifications to them.

System fonts used for TuiPWA icon generation are not copied into the distribution. Fonts embedded by Ghostty retain their own licenses.

Collected notices do not constitute a complete license compliance audit. Review the applicable terms before redistribution.
