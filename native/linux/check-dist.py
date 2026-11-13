#!/usr/bin/env python3
"""Check the Linux distribution boundary, not overall license compliance."""
import os
from pathlib import Path
import re
import subprocess
import sys


SYSTEM_LIBRARY = re.compile(
    r"lib(?:gtk|gdk|adwaita|glib|gio|gobject|gmodule|gthread).*\.(?:so(?:\..*)?|a)$"
)


def check_dist(root):
    for path in root.rglob("*"):
        if SYSTEM_LIBRARY.fullmatch(path.name):
            raise ValueError(f"システムライブラリは同梱できません: {path}")

    required = {
        "tuipwa": ("libgio-2.0.so.", "libgobject-2.0.so.", "libglib-2.0.so."),
        "ghostty-host": (
            "libgtk-4.so.", "libadwaita-1.so.", "libgio-2.0.so.",
            "libgobject-2.0.so.", "libglib-2.0.so.",
        ),
    }
    for binary, libraries in required.items():
        result = subprocess.run(
            ["readelf", "--dynamic", str(root / "bin" / binary)],
            check=True, text=True, capture_output=True,
            env={**os.environ, "LC_ALL": "C"},
        )
        needed = re.findall(r"\(NEEDED\).*\[([^\]]+)\]", result.stdout)
        for library in libraries:
            if not any(name.startswith(library) for name in needed):
                raise ValueError(f"{binary}: 動的依存が見つかりません: {library}")

    for name in (
        "LICENSE", "THIRD_PARTY_NOTICES.md", "share/licenses/ghostty/LICENSE",
        "share/licenses/rust-dependencies.tsv", "share/licenses/system/LGPL-2.1.txt",
    ):
        if not (root / name).is_file() or (root / name).stat().st_size == 0:
            raise ValueError(f"ライセンス通知がありません: {root / name}")


if __name__ == "__main__":
    try:
        check_dist(Path(sys.argv[1] if len(sys.argv) > 1 else "dist"))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
    print("配布物検査 OK: GTK 系は動的リンク・非同梱、ライセンス通知あり")
