"""Focused fail-closed tests for the macOS Taira FD198 supervisor."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SUPERVISOR = Path(__file__).resolve().parents[1] / "taira_fd198_supervisor.py"


class TairaFd198SupervisorTest(unittest.TestCase):
    def setUp(self) -> None:
        # macOS exposes /var as a compatibility symlink.  Production Taira
        # paths live below /Users and the supervisor intentionally rejects all
        # symlink components, so keep this fixture under the real checkout.
        self._temporary = tempfile.TemporaryDirectory(dir=SUPERVISOR.parent)
        self.addCleanup(self._temporary.cleanup)
        self.root = Path(self._temporary.name)
        self.private = self.root / "private"
        self.private.mkdir(mode=0o700)
        self.private.chmod(0o700)
        self.source = self.private / "runtime-signer.private_key"
        self.source.write_bytes(b"x" * 71)
        self.source.chmod(0o600)
        self.launch = self.private / "runtime-signer.fd198"
        self.config = self.root / "config.toml"
        self.config.write_text('chain = "test"\n', encoding="utf-8")
        self.genesis = self.root / "genesis.json"
        self.genesis.write_text("{}\n", encoding="utf-8")
        self.marker = self.root / "marker.json"
        self.fake_daemon = self.root / "iroha3d_taira"
        self.fake_daemon.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, sys, time\n"
            "payload = bytearray()\n"
            "check_config = '--check-config' in sys.argv[1:]\n"
            "if check_config:\n"
            "    payload.extend(os.pread(198, 71, 0))\n"
            "else:\n"
            "    while len(payload) < 71:\n"
            "        chunk = os.read(198, 71 - len(payload))\n"
            "        if not chunk: break\n"
            "        payload.extend(chunk)\n"
            "ok = len(payload) == 71 and os.get_inheritable(198)\n"
            "links = os.fstat(198).st_nlink\n"
            "if not check_config:\n"
            "    os.lseek(198, 0, os.SEEK_SET)\n"
            "    os.write(198, b'\\0' * 71)\n"
            "    os.fsync(198)\n"
            "    os.ftruncate(198, 0)\n"
            "    os.fsync(198)\n"
            "time.sleep(float(os.environ.get('TAIRA_SUPERVISOR_TEST_SLEEP', '0')))\n"
            "payload[:] = b'\\0' * len(payload)\n"
            "with open(os.environ['TAIRA_SUPERVISOR_TEST_MARKER'], 'w', encoding='utf-8') as f:\n"
            "    json.dump({'argv': sys.argv[1:], 'fd198': ok, 'links': links}, f)\n"
            "configured = int(os.environ.get('TAIRA_SUPERVISOR_TEST_EXIT_CODE', '0'))\n"
            "raise SystemExit(configured if ok else 4)\n",
            encoding="utf-8",
        )
        self.fake_daemon.chmod(0o700)

    def _command(self, action: str) -> list[str]:
        command = [
            sys.executable,
            "-I",
            "-B",
            str(SUPERVISOR),
            action,
        ]
        if action in ("check-config", "run"):
            command.extend(
                [
                    "--binary",
                    str(self.fake_daemon),
                    "--config",
                    str(self.config),
                    "--genesis-manifest",
                    str(self.genesis),
                ]
            )
        if action == "check-config":
            command.extend(["--config-blake3", "A" * 64])
        command.extend(
            [
                "--signer-source",
                str(self.source),
                "--signer-launch",
                str(self.launch),
            ]
        )
        return command

    def _run(
        self, action: str, environment_updates: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["TAIRA_SUPERVISOR_TEST_MARKER"] = str(self.marker)
        environment.update(environment_updates or {})
        return subprocess.run(
            self._command(action),
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

    def test_run_preserves_source_and_daemon_consumes_distinct_launch_copy(self) -> None:
        result = self._run("run")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.source.stat().st_size, 71)
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o600)
        self.assertEqual(self.launch.stat().st_size, 0)
        self.assertFalse(self.source.samefile(self.launch))
        marker = json.loads(self.marker.read_text(encoding="utf-8"))
        self.assertTrue(marker["fd198"])
        self.assertEqual(marker["links"], 1)
        self.assertEqual(
            marker["argv"],
            [
                "--sora",
                "--config",
                str(self.config),
                "--genesis-manifest-json",
                str(self.genesis),
            ],
        )

    def test_check_config_gets_fd198_and_disposable_copy_is_destroyed(self) -> None:
        source_before = self.source.read_bytes()
        result = self._run("check-config")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.source.read_bytes(), source_before)
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o600)
        self.assertFalse(self.launch.exists())
        marker = json.loads(self.marker.read_text(encoding="utf-8"))
        self.assertTrue(marker["fd198"])
        self.assertEqual(marker["links"], 0)
        self.assertEqual(
            marker["argv"],
            [
                "--sora",
                "--check-config",
                "--config",
                str(self.config),
                "--config-blake3",
                "a" * 64,
                "--genesis-manifest-json",
                str(self.genesis),
            ],
        )

    def test_check_config_propagates_failure_after_destroying_copy(self) -> None:
        source_before = self.source.read_bytes()
        result = self._run(
            "check-config", {"TAIRA_SUPERVISOR_TEST_EXIT_CODE": "23"}
        )

        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertEqual(self.source.read_bytes(), source_before)
        self.assertFalse(self.launch.exists())
        self.assertTrue(self.marker.exists())

    def test_check_config_timeout_destroys_copy(self) -> None:
        command = self._command("check-config")
        command.extend(("--timeout-seconds", "0.05"))
        environment = os.environ.copy()
        environment["TAIRA_SUPERVISOR_TEST_MARKER"] = str(self.marker)
        environment["TAIRA_SUPERVISOR_TEST_SLEEP"] = "2"
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("config check timed out", result.stderr)
        self.assertEqual(self.source.read_bytes(), b"x" * 71)
        self.assertFalse(self.launch.exists())
        self.assertFalse(self.marker.exists())

    def test_invalid_config_digest_fails_before_staging(self) -> None:
        command = self._command("check-config")
        digest_index = command.index("--config-blake3") + 1
        command[digest_index] = "not-a-digest"
        result = subprocess.run(command, check=False, capture_output=True, text=True)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exactly 64 hex digits", result.stderr)
        self.assertEqual(self.source.read_bytes(), b"x" * 71)
        self.assertFalse(self.launch.exists())
        self.assertFalse(self.marker.exists())

    def test_exec_failure_destroys_staged_copy(self) -> None:
        self.fake_daemon.write_text("#!/definitely/missing/interpreter\n", encoding="utf-8")
        self.fake_daemon.chmod(0o700)
        result = self._run("run")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot execute", result.stderr)
        self.assertEqual(self.source.read_bytes(), b"x" * 71)
        self.assertFalse(self.launch.exists())
        self.assertFalse(self.marker.exists())

    def test_source_descriptor_collision_at_fd198_is_relocated(self) -> None:
        # Fill every descriptor below 198 in a disposable driver, then exec the
        # supervisor.  Its source open must land on 198; the supervisor must
        # relocate that source before replacing 198 with the launch copy.
        driver = (
            "import json, os, sys\n"
            "command = json.loads(sys.argv[1])\n"
            "for target in range(3, 198):\n"
            "    descriptor = os.open('/dev/null', os.O_RDONLY)\n"
            "    if descriptor != target:\n"
            "        os.dup2(descriptor, target)\n"
            "        os.close(descriptor)\n"
            "    os.set_inheritable(target, True)\n"
            "os.execve(command[0], command, os.environ.copy())\n"
        )
        environment = os.environ.copy()
        environment["TAIRA_SUPERVISOR_TEST_MARKER"] = str(self.marker)
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-B",
                "-c",
                driver,
                json.dumps(self._command("run")),
            ],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(self.marker.read_text(encoding="utf-8"))["fd198"])
        self.assertEqual(self.source.stat().st_size, 71)
        self.assertEqual(self.launch.stat().st_size, 0)

    def test_validate_is_metadata_only_and_does_not_create_launch_copy(self) -> None:
        before = self.source.stat()
        result = self._run("validate")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.launch.exists())
        after = self.source.stat()
        self.assertEqual(after.st_size, 71)
        self.assertEqual(after.st_ino, before.st_ino)
        self.assertEqual(after.st_mtime_ns, before.st_mtime_ns)

    def test_weak_source_mode_fails_without_staging(self) -> None:
        self.source.chmod(0o640)
        result = self._run("run")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("owner-0600", result.stderr)
        self.assertFalse(self.launch.exists())
        self.assertFalse(self.marker.exists())

    def test_source_symlink_fails_without_reading_target(self) -> None:
        target = self.private / "target"
        self.source.rename(target)
        self.source.symlink_to(target)
        result = self._run("validate")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlink", result.stderr)
        self.assertEqual(target.stat().st_size, 71)
        self.assertFalse(self.launch.exists())

    def test_untrusted_stale_launch_file_is_never_replaced(self) -> None:
        self.launch.write_bytes(b"bad")
        self.launch.chmod(0o600)
        before = self.launch.read_bytes()
        result = self._run("run")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("untrusted stale", result.stderr)
        self.assertEqual(self.launch.read_bytes(), before)
        self.assertEqual(self.source.stat().st_size, 71)
        self.assertFalse(self.marker.exists())

    def test_trusted_stale_launch_copy_is_wiped_before_replacement(self) -> None:
        self.launch.write_bytes(b"s" * 71)
        self.launch.chmod(0o600)
        stale_descriptor = os.open(self.launch, os.O_RDONLY)
        self.addCleanup(os.close, stale_descriptor)

        result = self._run("run")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(os.fstat(stale_descriptor).st_size, 0)
        self.assertEqual(os.pread(stale_descriptor, 1, 0), b"")
        self.assertEqual(self.source.read_bytes(), b"x" * 71)
        self.assertEqual(self.launch.stat().st_size, 0)

    def test_persistent_source_cannot_be_reused_as_launch_file(self) -> None:
        command = self._command("validate")
        command[-1] = str(self.source)
        result = subprocess.run(command, check=False, capture_output=True, text=True)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must be distinct", result.stderr)
        self.assertEqual(self.source.stat().st_size, 71)


if __name__ == "__main__":
    unittest.main()
