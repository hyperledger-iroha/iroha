"""Focused tests for the disposable Kagami-backed Taira devnet command."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "taira_devnet.py"
SPEC = importlib.util.spec_from_file_location("taira_devnet", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
module = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = module
SPEC.loader.exec_module(module)


def executable(path: Path, body: bytes = b"current binary\n") -> Path:
    """Create one non-empty executable used by the fake toolchain."""

    path.write_bytes(body)
    path.chmod(0o700)
    return path


class FakeRuntime:
    """Model the subprocess and HTTP surface consumed by the command."""

    def __init__(self) -> None:
        self.commands: list[tuple[str, ...]] = []
        self.height = 1
        self.unhealthy_peer: int | None = None
        self.doctor_fails = False
        self.leave_peer_running_on_stop = False
        self.process_commands: dict[int, str] = {}

    def run(
        self,
        command: list[str] | tuple[str, ...],
        **kwargs: object,
    ) -> subprocess.CompletedProcess[str]:
        values = tuple(str(value) for value in command)
        self.commands.append(values)
        if "localnet" in values:
            target = Path(values[values.index("--out-dir") + 1])
            api_port = int(values[values.index("--base-api-port") + 1])
            target.mkdir(mode=0o700)
            for name in ("start.sh", "stop.sh"):
                executable(target / name, b"#!/usr/bin/env bash\n")
            for index in range(module.PEER_COUNT):
                (target / f"peer{index}.toml").write_text(
                    f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                    f'address = "addr:127.0.0.1:{api_port + index}#ABCD"\n',
                    encoding="utf-8",
                )
            network_id = "hash:" + "A" * 64 + "#ABCD"
            (target / "genesis.expected_hash").write_text(network_id + "\n", encoding="utf-8")
            (target / "client.toml").write_text(
                f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                f'network_id = "{network_id}"\n'
                f'torii_url = "http://127.0.0.1:{api_port}/"\n',
                encoding="utf-8",
            )
        elif values[0] == "/bin/bash" and values[1].endswith("/start.sh"):
            target = Path(str(kwargs["cwd"]))
            for index in range(module.PEER_COUNT):
                pid = 10_000 + index
                (target / f"peer{index}.pid").write_text(f"{pid}\n", encoding="utf-8")
                self.process_commands[pid] = (
                    f"/fake/iroha3d --sora --config {target / f'peer{index}.toml'}"
                )
        elif values[0] == "/bin/bash" and values[1].endswith("/stop.sh"):
            target = Path(str(kwargs["cwd"]))
            first_retained = self.leave_peer_running_on_stop
            for index in range(module.PEER_COUNT):
                if first_retained and index == 0:
                    continue
                (target / f"peer{index}.pid").unlink(missing_ok=True)
                self.process_commands.pop(10_000 + index, None)
        elif values == ("ps", "-axww", "-o", "pid=,command="):
            stdout = "".join(
                f"{pid} {command_line}\n"
                for pid, command_line in self.process_commands.items()
            )
            return subprocess.CompletedProcess(values, 0, stdout, "")
        elif "ping" in values:
            self.height += 1
        elif "doctor" in values and self.doctor_fails:
            raise module.DevnetError("full doctor failed")
        return subprocess.CompletedProcess(values, 0, "", "")

    def request(self, url: str, payload: object | None) -> tuple[int, object | None]:
        if url.endswith("v1/mcp"):
            if payload is None:
                return 200, {
                    "enabled": True,
                    "protocolVersion": module.MCP_PROTOCOL_VERSION,
                }
            assert isinstance(payload, dict)
            if payload.get("method") == "initialize":
                return 200, {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"protocolVersion": module.MCP_PROTOCOL_VERSION},
                }
            if payload.get("method") == "tools/list":
                return 200, {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "result": {"tools": [{"name": "iroha.health"}]},
                }
            raise AssertionError(f"unexpected MCP payload: {payload}")
        for index in range(module.PEER_COUNT):
            if f":{module.DEFAULT_API_PORT + index}/" not in url:
                continue
            if index == self.unhealthy_peer and url.endswith("readyz"):
                return 503, None
            if url.endswith(("health", "readyz")):
                return 200, None
            if url.endswith("status/blocks"):
                return 200, self.height
        raise AssertionError(f"unexpected URL: {url}")


class TairaDevnetTests(unittest.TestCase):
    """Exercise the small orchestration contract without real peers."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        for name in ("kagami", "iroha3d", "iroha"):
            executable(self.bin_dir / name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def up_args(self, *extra: str):
        """Parse a no-build ``up`` command for this test directory."""

        return module.parser().parse_args(
            [
                "--dir",
                str(self.root / "state"),
                "up",
                "--no-build",
                "--bin-dir",
                str(self.bin_dir),
                "--timeout-seconds",
                "1",
                *extra,
            ]
        )

    def test_up_is_fresh_exact_four_and_proves_signed_finality(self) -> None:
        runtime = FakeRuntime()

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(report["baseline_height"], 1)
        self.assertEqual(report["final_height"], 2)
        kagami = next(command for command in runtime.commands if "localnet" in command)
        self.assertIn("--fresh-random-keys", kagami)
        self.assertEqual(kagami[kagami.index("--peers") + 1], "4")
        self.assertEqual(kagami[kagami.index("--sora-profile") + 1], "nexus")
        self.assertEqual(kagami[kagami.index("--consensus-mode") + 1], "npos")
        self.assertEqual(kagami[kagami.index("--chain-id") + 1], module.DEFAULT_CHAIN_ID)
        self.assertEqual(kagami[kagami.index("--bind-host") + 1], "127.0.0.1")
        self.assertEqual(kagami[kagami.index("--public-host") + 1], "127.0.0.1")
        config_checks = [
            command for command in runtime.commands if "--check-config" in command
        ]
        self.assertEqual(len(config_checks), 4)
        self.assertTrue(
            all(command.count(str(self.bin_dir / "iroha3d")) == 1 for command in config_checks)
        )
        self.assertEqual(sum("ping" in command for command in runtime.commands), 1)
        self.assertEqual(sum("doctor" in command for command in runtime.commands), 0)
        ping = next(command for command in runtime.commands if "ping" in command)
        self.assertIn("--machine", ping)
        self.assertIn("--fee-payer", ping)
        self.assertIn("tx", ping)

    def test_fresh_generation_has_no_hidden_wall_clock_deadline(self) -> None:
        calls: list[dict[str, object]] = []

        def run(
            command: list[str] | tuple[str, ...],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            calls.append(kwargs)
            return subprocess.CompletedProcess(command, 0, "", "")

        module.generate_network(
            self.root / "network",
            self.bin_dir / "kagami",
            module.DEFAULT_API_PORT,
            module.DEFAULT_P2P_PORT,
            run,
        )

        self.assertEqual(len(calls), 1)
        self.assertIsNone(calls[0]["timeout"])
        self.assertIs(calls[0]["capture_output"], False)

    def test_failed_readiness_stops_failed_cohort_without_activation_state(self) -> None:
        runtime = FakeRuntime()
        runtime.unhealthy_peer = 2
        args = self.up_args()
        args.timeout_seconds = 0.01

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "did not converge"):
                module.up(args, run=runtime.run, request=runtime.request)

        stop_calls = [command for command in runtime.commands if command[0] == "/bin/bash"]
        self.assertTrue(stop_calls[-1][1].endswith("network/stop.sh"))
        state = self.root / "state"
        self.assertEqual((state / module.MARKER).read_text(encoding="utf-8"), module.MARKER_BODY)
        self.assertFalse((state / "current.json").exists())
        self.assertFalse((state / "generations").exists())

    def test_interrupted_startup_stops_the_generated_cohort(self) -> None:
        runtime = FakeRuntime()

        def interrupt(_url: str, _payload: object | None) -> tuple[int, object | None]:
            raise KeyboardInterrupt

        with self.assertRaisesRegex(module.DevnetError, "startup was interrupted"):
            module.up(self.up_args(), run=runtime.run, request=interrupt)

        stop_calls = [command for command in runtime.commands if command[0] == "/bin/bash"]
        self.assertTrue(stop_calls[-1][1].endswith("network/stop.sh"))

    def test_check_is_read_only_and_down_needs_no_release_confirmation(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        ping_count = sum("ping" in command for command in runtime.commands)
        state = self.root / "state"

        check_args = module.parser().parse_args(
            ["--dir", str(state), "check", "--timeout-seconds", "1"]
        )
        report = module.check(check_args, run=runtime.run, request=runtime.request)
        self.assertEqual(report["height"], 2)
        self.assertEqual(sum("ping" in command for command in runtime.commands), ping_count)

        down_args = module.parser().parse_args(["--dir", str(state), "down"])
        self.assertTrue(module.down(down_args, run=runtime.run)["stopped"])

    def test_down_and_replacement_fail_closed_on_residual_peer(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        runtime.leave_peer_running_on_stop = True
        state = self.root / "state"
        down_args = module.parser().parse_args(["--dir", str(state), "down"])

        with self.assertRaisesRegex(module.DevnetError, "left peer PID files"):
            module.down(down_args, run=runtime.run)
        with self.assertRaisesRegex(module.DevnetError, "left peer PID files"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertTrue((state / "network" / "peer0.pid").is_file())
        self.assertTrue((state / "network" / "peer0.toml").is_file())

    def test_down_rejects_marker_only_state(self) -> None:
        state = self.root / "state"
        module.managed_root(state, create=True)
        args = module.parser().parse_args(["--dir", str(state), "down"])

        with self.assertRaisesRegex(module.DevnetError, "run `up` first"):
            module.down(args, run=FakeRuntime().run)

    def test_up_preserves_incomplete_network_with_residual_pid_evidence(self) -> None:
        state = module.managed_root(self.root / "state", create=True)
        target = state / "network"
        target.mkdir()
        (target / "peer0.pid").write_text("12345\n", encoding="utf-8")

        with self.assertRaisesRegex(module.DevnetError, "left peer PID files"):
            module.up(self.up_args(), run=FakeRuntime().run, request=FakeRuntime().request)

        self.assertEqual((target / "peer0.pid").read_text(encoding="utf-8"), "12345\n")

    def test_check_rejects_a_marker_without_a_generated_bundle(self) -> None:
        state = self.root / "state"
        module.managed_root(state, create=True)
        args = module.parser().parse_args(
            ["--dir", str(state), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "run `up` first"):
            module.check(args, request=FakeRuntime().request)

    def test_check_rejects_healthy_listeners_not_owned_by_bundle_pids(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        runtime.process_commands.clear()
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "not the sole running process"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_bundle_identity_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        client = self.root / "state" / "network" / "client.toml"
        client.write_text(
            client.read_text(encoding="utf-8").replace(module.DEFAULT_CHAIN_ID, "wrong-chain"),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "not for canonical Taira"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_full_public_doctor_is_opt_in(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args("--full-doctor"), run=runtime.run, request=runtime.request)
        doctor = [command for command in runtime.commands if "doctor" in command]
        self.assertEqual(len(doctor), 1)
        self.assertEqual(doctor[0][doctor[0].index("--public-root") + 1], "http://127.0.0.1:29080/")

    def test_managed_directory_refuses_foreign_contents(self) -> None:
        foreign = self.root / "foreign"
        foreign.mkdir()
        (foreign / "keep").write_text("mine\n", encoding="utf-8")

        with self.assertRaisesRegex(module.DevnetError, "unmarked non-empty"):
            module.managed_root(foreign, create=True)

        self.assertEqual((foreign / "keep").read_text(encoding="utf-8"), "mine\n")

    def test_managed_directory_canonicalizes_symlinked_ancestry(self) -> None:
        real = self.root / "real"
        real.mkdir()
        alias = self.root / "alias"
        alias.symlink_to(real, target_is_directory=True)

        managed = module.managed_root(alias / "state", create=True)

        self.assertEqual(managed, (real / "state").resolve())
        self.assertEqual(
            (real / "state" / module.MARKER).read_text(encoding="utf-8"),
            module.MARKER_BODY,
        )

    def test_up_and_down_reject_a_symlinked_network_directory(self) -> None:
        state = module.managed_root(self.root / "state", create=True)
        foreign = self.root / "foreign"
        foreign.mkdir()
        executable(foreign / "stop.sh", b"#!/usr/bin/env bash\n")
        (state / "network").symlink_to(foreign, target_is_directory=True)
        runtime = FakeRuntime()

        down_args = module.parser().parse_args(["--dir", str(state), "down"])
        with self.assertRaisesRegex(module.DevnetError, "symlinked network directory"):
            module.down(down_args, run=runtime.run)
        with self.assertRaisesRegex(module.DevnetError, "symlinked network directory"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(runtime.commands, [])

    def test_build_command_has_no_retired_release_features(self) -> None:
        command = module.cargo_build_command("local-release", Path("/tmp/taira-target"))
        self.assertEqual(command[0], str(REPO_ROOT / "scripts" / "cargo_fast.sh"))
        self.assertIn("--stable-local-metadata", command)
        self.assertEqual(command[command.index("--target-dir") + 1], "/tmp/taira-target")
        self.assertEqual(command.count("--bin"), 3)
        rendered = " ".join(command)
        self.assertNotIn("external-software-signer-bin", rendered)
        self.assertIn("--locked", command)
        self.assertNotIn("--features", command)

    def test_http_request_accepts_plain_text_health_response(self) -> None:
        class PlainResponse:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args: object) -> None:
                return None

            @staticmethod
            def read(_limit: int = -1) -> bytes:
                return b"Healthy"

        with mock.patch.object(module.urllib.request, "urlopen", return_value=PlainResponse()):
            status, payload = module.http_request("http://127.0.0.1:29080/health")

        self.assertEqual(status, 200)
        self.assertEqual(payload, "Healthy")

    def test_http_request_rejects_an_oversized_response(self) -> None:
        class OversizedResponse:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args: object) -> None:
                return None

            @staticmethod
            def read(limit: int = -1) -> bytes:
                assert limit == module.MAX_HTTP_RESPONSE_BYTES + 1
                return b"x" * limit

        with mock.patch.object(
            module.urllib.request,
            "urlopen",
            return_value=OversizedResponse(),
        ):
            with self.assertRaisesRegex(module.DevnetError, "HTTP response exceeds"):
                module.http_request("http://127.0.0.1:29080/health")

    def test_managed_directory_rejects_an_oversized_marker(self) -> None:
        state = self.root / "state"
        state.mkdir()
        (state / module.MARKER).write_bytes(b"x" * (module.MAX_MARKER_BYTES + 1))

        with self.assertRaisesRegex(module.DevnetError, "devnet marker exceeds"):
            module.managed_root(state, create=False)

    def test_failure_log_tail_reads_only_a_bounded_suffix(self) -> None:
        target = self.root / "network"
        target.mkdir()
        log = target / "peer0.log"
        log.write_bytes(
            b"discard-this-prefix" + b"x" * module.MAX_LOG_TAIL_BYTES + b"\nlast-a\nlast-b\n"
        )
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr):
            module.dump_logs(target)

        rendered = stderr.getvalue()
        self.assertNotIn("discard-this-prefix", rendered)
        self.assertIn("last-a", rendered)
        self.assertIn("last-b", rendered)

    def test_command_timeout_is_reported_without_a_traceback(self) -> None:
        timeout = subprocess.TimeoutExpired(["cargo", "build"], 7)
        with mock.patch.object(module.subprocess, "run", side_effect=timeout):
            with self.assertRaisesRegex(module.DevnetError, "cargo timed out after 7s"):
                module.run_command(["cargo", "build"], timeout=7)

    def test_mcp_rejects_stale_negotiated_protocol(self) -> None:
        def stale_request(_url: str, payload: object | None) -> tuple[int, object]:
            if payload is None:
                return 200, {
                    "enabled": True,
                    "protocolVersion": module.MCP_PROTOCOL_VERSION,
                }
            return 200, {
                "jsonrpc": "2.0",
                "id": 1,
                "result": {"protocolVersion": "stale"},
            }

        with self.assertRaisesRegex(module.DevnetError, "MCP initialize failed"):
            module.check_mcp("http://127.0.0.1:29080/", stale_request)

    def test_help_exposes_only_up_check_and_down(self) -> None:
        completed = subprocess.run(
            [sys.executable, str(MODULE_PATH), "--help"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0)
        self.assertIn("{up,check,down}", completed.stdout)
        self.assertNotIn("promote", completed.stdout.lower())
        self.assertNotIn("publish", completed.stdout.lower())

    def test_retired_taira_orchestration_does_not_reappear(self) -> None:
        def names(directory: Path, pattern: str = "*taira*") -> set[str]:
            return {entry.name for entry in directory.glob(pattern) if entry.is_file()}

        self.assertEqual(
            names(REPO_ROOT / "scripts"),
            {"render_taira_edge_nginx_conf.py", "taira_constants.py", "taira_devnet.py"},
        )
        self.assertEqual(
            names(REPO_ROOT / "scripts" / "tests"),
            {"render_taira_edge_nginx_conf_test.py", "taira_devnet_test.py"},
        )
        self.assertEqual(names(REPO_ROOT / ".github" / "workflows"), set())
        self.assertEqual(names(REPO_ROOT / "ci"), set())
        self.assertEqual(
            names(REPO_ROOT / "crates" / "iroha_cli" / "src" / "bin"),
            {"taira_fee_sponsor_program.rs"},
        )
        self.assertEqual(
            names(REPO_ROOT / "crates" / "irohad" / "src" / "bin"),
            {"taira_bootle_lantern_broker.rs"},
        )
        self.assertEqual(
            names(REPO_ROOT / "crates" / "iroha_test_network" / "src" / "bin"),
            set(),
        )


if __name__ == "__main__":
    unittest.main()
