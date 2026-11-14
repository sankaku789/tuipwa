#!/usr/bin/env python3
"""Report common missing build dependencies before downloading or compiling."""
import os
import shlex
import shutil
import subprocess
import sys


TOOLS = {
    "rustc": "Rust (stable / edition 2024)",
    "cargo": "Cargo",
    "cmake": "CMake",
    "ninja": "Ninja",
    "pkg-config": "pkg-config",
    "git": "Git",
    "curl": "curl",
    "patch": "patch",
    "tar": "tar",
    "xz": "xz",
    "sha256sum": "SHA-256 ツール",
    "readelf": "binutils",
    "blueprint-compiler": "Blueprint コンパイラ",
    "glib-compile-resources": "GLib リソース生成ツール",
    "msgfmt": "gettext",
}

# pkg-config identifiers, not distribution-specific package names.
LIBRARIES = {
    "gio-2.0": "GLib / GIO / GObject",
    "gtk4": "GTK 4",
    "libadwaita-1": "libadwaita",
    "fontconfig": "fontconfig",
    "freetype2": "FreeType",
    "libpng": "libpng",
    "zlib": "zlib",
    "oniguruma": "Oniguruma",
    "spirv-cross-c-shared": "SPIRV-Cross",
    "egl": "EGL",
}


def missing_dependencies():
    missing = []
    for variable, default, label in (("CC", "cc", "C コンパイラ"),
                                     ("CXX", "c++", "C++ コンパイラ")):
        command = shlex.split(os.environ.get(variable) or default)
        if not shutil.which(command[0]):
            missing.append(f"コマンド: {command[0]} — {label} ({variable})")
    for command, label in TOOLS.items():
        if not shutil.which(command):
            missing.append(f"コマンド: {command} — {label}")
    if shutil.which("pkg-config"):
        for module, label in LIBRARIES.items():
            result = subprocess.run(
                ["pkg-config", "--exists", module], capture_output=True,
            )
            if result.returncode:
                missing.append(f"開発ライブラリ: {label} (pkg-config: {module})")
    return missing


def main():
    missing = missing_dependencies()
    if missing:
        print("ビルドに必要な依存が見つかりません:", file=sys.stderr)
        for item in missing:
            print(f"  - {item}", file=sys.stderr)
        print(
            "\n対応するツール・開発用パッケージ（-dev / -devel 等）を導入してください。\n"
            "導入済みの場合は PATH / PKG_CONFIG_PATH を確認してください。\n"
            "導入手順: README.md の「インストール」\n"
            "再確認: make check-deps（ダウンロード・ビルドは行いません）",
            file=sys.stderr,
        )
        return 1
    print("依存チェック OK（バージョン互換性・実行環境はビルド・起動時に確認します）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
