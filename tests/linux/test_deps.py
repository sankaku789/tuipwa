"""Build dependency diagnostics; no system packages are required by these tests."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "check_deps", Path(__file__).resolve().parents[2] / "native/linux/check-deps.py"
)
deps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deps)


class DependencyTests(unittest.TestCase):
    def setUp(self):
        self.env = patch.dict(os.environ, {"CC": "cc", "CXX": "c++"})
        self.env.start()
        self.addCleanup(self.env.stop)

    @patch.object(deps.shutil, "which", return_value="/usr/bin/tool")
    @patch.object(deps.subprocess, "run", return_value=subprocess.CompletedProcess([], 0))
    def test_all_present(self, run, which):
        self.assertEqual(deps.missing_dependencies(), [])
        self.assertEqual(run.call_count, len(deps.LIBRARIES))

    @patch.object(deps.shutil, "which", return_value=None)
    @patch.object(deps.subprocess, "run")
    def test_missing_pkg_config_skips_library_probes(self, run, which):
        missing = deps.missing_dependencies()
        self.assertIn("コマンド: pkg-config — pkg-config", missing)
        self.assertIn("コマンド: cargo — Cargo", missing)
        run.assert_not_called()

    @patch.object(deps.shutil, "which", side_effect=lambda name: None if name == "curl" else name)
    @patch.object(deps.subprocess, "run")
    def test_reports_commands_and_libraries_together(self, run, which):
        run.side_effect = lambda args, **kwargs: subprocess.CompletedProcess(
            args, 1 if args[-1] in ("gtk4", "gio-2.0") else 0
        )
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            self.assertEqual(deps.main(), 1)
        for expected in ("curl", "GTK 4", "gio-2.0", "-dev / -devel", "PKG_CONFIG_PATH"):
            self.assertIn(expected, output.getvalue())

    @patch.object(deps.shutil, "which", return_value="/usr/bin/tool")
    @patch.object(deps.subprocess, "run", return_value=subprocess.CompletedProcess([], 0))
    def test_compiler_overrides(self, run, which):
        with patch.dict(os.environ, {"CC": "clang -O2", "CXX": "clang++"}):
            self.assertEqual(deps.missing_dependencies(), [])
        which.assert_any_call("clang")
        which.assert_any_call("clang++")


if __name__ == "__main__":
    unittest.main()
