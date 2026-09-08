"""Verify canonical Xcode tool dispatch retains ranlib's argv-zero mode."""

from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "build_norito_xcframework.sh"


def index_helper():
    """Read the actual shell helper so tests exercise the shipped invocation."""
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"^rebuild_apple_archive_index\(\) \{\n.*?^\}", source, re.M | re.S
    )
    if match is None:
        raise AssertionError("Apple archive-index helper is missing")
    return match.group(0)


def python_dispatch():
    """Extract the fixed Python command used by the shell helper."""
    match = re.search(r"^\s*'(import subprocess,sys;[^']+)'", index_helper(), re.M)
    if match is None:
        raise AssertionError("Canonical ranlib dispatch is missing")
    return compile(match.group(1), str(SCRIPT), "exec")


class AppleArchiveIndexDispatchTests(unittest.TestCase):
    def test_preserves_mode_and_canonical_executable_without_shell_expansion(self):
        archive = "/tmp/archive with spaces;$(unchanged).a"
        executable = "/selected/Xcode/toolchain/libtool"
        with mock.patch.object(sys, "argv", ["-c", executable, archive]):
            with mock.patch("subprocess.run") as run:
                exec(python_dispatch(), {})
        run.assert_called_once_with(
            ["ranlib", "-D", archive], executable=executable, check=True
        )

    def test_tool_failure_propagates(self):
        failure = subprocess.CalledProcessError(1, ["ranlib", "-D", "bad.a"])
        with mock.patch.object(sys, "argv", ["-c", "/selected/libtool", "bad.a"]):
            with mock.patch("subprocess.run", side_effect=failure):
                with self.assertRaises(subprocess.CalledProcessError) as caught:
                    exec(python_dispatch(), {})
        self.assertIs(caught.exception, failure)

    @unittest.skipUnless(sys.platform == "darwin", "requires selected Apple tools")
    def test_actual_canonical_libtool_rebuilds_deterministic_empty_archive_index(self):
        env = os.environ.copy()
        developer = "/Applications/Xcode.app/Contents/Developer"
        if Path(developer).is_dir():
            env["DEVELOPER_DIR"] = developer
        selected = subprocess.run(
            ["/usr/bin/xcrun", "--find", "ranlib"],
            env=env, text=True, capture_output=True, check=False,
        )
        if selected.returncode != 0:
            self.skipTest("Xcode ranlib is unavailable")
        canonical = Path(selected.stdout.strip()).resolve(strict=True)
        with tempfile.TemporaryDirectory(prefix="apple-index-dispatch-") as temporary:
            archive = Path(temporary) / "empty archive.a"
            archive.write_bytes(b"!<arch>\n")
            env.update(
                RANLIB_BINARY=str(canonical), PYTHON_BINARY=sys.executable,
                USER_HOME_DIR=str(Path.home()), MOBILE_TMPDIR=temporary,
                XCODE_DEVELOPER_DIR=env.get("DEVELOPER_DIR", ""),
            )
            command = [
                "/bin/bash", "-c",
                index_helper() + '\nrebuild_apple_archive_index "$1"',
                "index-test", str(archive),
            ]
            subprocess.run(command, env=env, check=True, capture_output=True)
            first = archive.read_bytes()
            self.assertIn(b"__.SYMDEF", first)
            subprocess.run(command, env=env, check=True, capture_output=True)
            self.assertEqual(archive.read_bytes(), first)


if __name__ == "__main__":
    unittest.main()
