"""Distribution policy checks; run with python3 -m unittest discover -s tests/linux."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "check_dist", Path(__file__).resolve().parents[2] / "native/linux/check-dist.py"
)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)

NEEDED = "\n".join(
    f"(NEEDED) Shared library: [{name}]"
    for name in ("libgtk-4.so.1", "libadwaita-1.so.0", "libgio-2.0.so.0",
                 "libgobject-2.0.so.0", "libglib-2.0.so.0")
)


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("LICENSE", "THIRD_PARTY_NOTICES.md",
                     "share/licenses/ghostty/LICENSE",
                     "share/licenses/rust-dependencies.tsv",
                     "share/licenses/system/LGPL-2.1.txt"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("notice")
        self.reader = patch.object(policy.subprocess, "run", return_value=
                                   subprocess.CompletedProcess([], 0, stdout=NEEDED))
        self.reader.start()
        self.addCleanup(self.reader.stop)

    def test_system_dynamic_libraries_allowed(self):
        policy.check_dist(self.root)

    def test_bundled_libraries_rejected(self):
        for name in ("libgtk-4.so.1", "libadwaita-1.a", "libglib-2.0.so",
                     "libgio-2.0.so.0", "libgobject-2.0.so.0"):
            with self.subTest(name=name):
                path = self.root / name
                path.touch()
                with self.assertRaisesRegex(ValueError, "同梱できません"):
                    policy.check_dist(self.root)
                path.unlink()

    def test_missing_dynamic_dependency_rejected(self):
        with patch.object(policy.subprocess, "run", return_value=
                          subprocess.CompletedProcess([], 0, stdout="")):
            with self.assertRaisesRegex(ValueError, "動的依存"):
                policy.check_dist(self.root)

    def test_missing_notice_rejected(self):
        (self.root / "LICENSE").unlink()
        with self.assertRaisesRegex(ValueError, "ライセンス通知"):
            policy.check_dist(self.root)


if __name__ == "__main__":
    unittest.main()
