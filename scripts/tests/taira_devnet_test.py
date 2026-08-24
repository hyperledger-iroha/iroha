"""Focused tests for the disposable Kagami-backed Taira devnet command."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
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
sys.path.insert(0, str(MODULE_PATH.parent))
try:
    SPEC.loader.exec_module(module)
finally:
    sys.path.remove(str(MODULE_PATH.parent))


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
        self.leave_peer_running_on_stop = False
        self.transient_command_loss_on_stop = False
        self.exit_before_kill_peer: int | None = None
        self.process_commands: dict[int, str] = {}
        self.exiting_process_polls: dict[int, int] = {}
        self.start_env: dict[str, str] | None = None
        self.start_pass_fds: tuple[int, ...] | None = None
        self.generation_pass_fds: tuple[int, ...] | None = None
        self.mcp_protocol_version = "taira-test-protocol-v1"
        self.requests: list[tuple[str, object | None]] = []
        self.api_port = module.DEFAULT_API_PORT
        self.help_options_by_surface = {
            (binary, subcommands): set(options)
            for binary, subcommands, options in module.CLI_SURFACES
        }
        self.sumeragi_status_http = 200
        self.restart_required_peer: int | None = None
        self.sumeragi_blocker_peer: int | None = None
        self.ping_stdout = json.dumps({"hash": "hash:" + "a" * 64 + "#ABCD"})
        self.status_stdout = json.dumps(
            {"hash": "a" * 64, "terminal_kind": "Applied"}
        )

    def run(
        self,
        command: list[str] | tuple[str, ...],
        **kwargs: object,
    ) -> subprocess.CompletedProcess[str]:
        values = tuple(str(value) for value in command)
        self.commands.append(values)
        if "--help" in values:
            surface = (Path(values[0]).name, values[1:-1])
            return subprocess.CompletedProcess(
                values,
                0,
                "\n".join(sorted(self.help_options_by_surface.get(surface, set()))),
                "",
            )
        if "localnet" in values:
            self.generation_pass_fds = tuple(kwargs.get("pass_fds", ()))
            target = Path(values[values.index("--out-dir") + 1])
            api_port = int(values[values.index("--base-api-port") + 1])
            self.api_port = api_port
            target.mkdir(mode=0o700)
            executable(
                target / "start.sh",
                b"#!/usr/bin/env bash\n"
                b"  if command -v python3 >/dev/null 2>&1; then\n"
                b"  fi\n"
                b'  echo "$peer_pid" > "$PIDFILE"\n',
            )
            executable(target / "stop.sh", b"#!/usr/bin/env bash\n")
            genesis_hash = "a" * 63 + "b"
            network_id = module.network_id_from_genesis_hash(genesis_hash)
            for index in range(module.PEER_COUNT):
                sorafs_dir = target / "state" / f"peer{index}" / "sorafs"
                (target / f"peer{index}.toml").write_text(
                    f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                    f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}\n"
                    f'[genesis]\nexpected_hash = "{network_id}"\n'
                    f'address = "addr:127.0.0.1:{api_port + index}#ABCD"\n'
                    "[nexus.storage]\n"
                    f"local_budget_bytes = {module.GENERATED_LOCALNET_NEXUS_STORAGE_BYTES}\n"
                    "[sorafs.storage]\n"
                    "enabled = false\n"
                    f'data_dir = "{sorafs_dir}"\n',
                    encoding="utf-8",
                )
            signer_directory = target / module.RUNTIME_SIGNER_DIRECTORY
            signer_directory.mkdir(parents=True, mode=0o700)
            for index in range(module.PEER_COUNT):
                signer = signer_directory / f"peer{index}.private_key"
                signer.write_bytes(b"x" * module.RUNTIME_SIGNER_FILE_BYTES)
                signer.chmod(0o600)
            for path, expected_size in module.generated_runtime_secret_paths(target):
                path.write_bytes(b"x" * expected_size)
                path.chmod(0o600)
            (target / "genesis.expected_hash").write_text(
                genesis_hash + "\n", encoding="utf-8"
            )
            (target / "client.toml").write_text(
                f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                f'network_id = "{network_id}"\n'
                f'torii_url = "http://127.0.0.1:{api_port}/"\n'
                f"[account]\nchain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}\n",
                encoding="utf-8",
            )
        elif "--check-config" in values:
            config = Path(values[values.index("--config") + 1])
            module.require_canonical_taira_storage_profiles(config.parent)
        elif values[0] == "/bin/bash" and values[1].endswith("/start.sh"):
            target = Path(str(kwargs["cwd"]))
            self.start_env = dict(kwargs["env"])
            self.start_pass_fds = tuple(kwargs.get("pass_fds", ()))
            for index in range(module.PEER_COUNT):
                pid = 10_000 + index
                (target / f"peer{index}.pid").write_text(f"{pid}\n", encoding="utf-8")
                self.process_commands[pid] = (
                    f"/fake/iroha3d_taira --sora --config {target / f'peer{index}.toml'}"
                )
        elif values[:2] == ("/bin/kill", "-TERM"):
            pid = int(values[2])
            if self.exit_before_kill_peer == pid:
                self.exit_before_kill_peer = None
                self.process_commands.pop(pid, None)
                raise module.DevnetError("kill failed: no such process")
            if not (self.leave_peer_running_on_stop and pid == 10_000):
                if self.transient_command_loss_on_stop:
                    self.process_commands[pid] = "(iroha3d_taira)"
                    self.exiting_process_polls[pid] = 1
                else:
                    self.process_commands.pop(pid, None)
        elif values == ("ps", "-axww", "-o", "pid=,command="):
            stdout = "".join(
                f"{pid} {command_line}\n"
                for pid, command_line in self.process_commands.items()
            )
            for pid in list(self.exiting_process_polls):
                remaining = self.exiting_process_polls[pid] - 1
                if remaining == 0:
                    self.exiting_process_polls.pop(pid)
                    self.process_commands.pop(pid, None)
                else:
                    self.exiting_process_polls[pid] = remaining
            return subprocess.CompletedProcess(values, 0, stdout, "")
        elif "ping" in values:
            self.height += 1
            return subprocess.CompletedProcess(values, 0, self.ping_stdout, "")
        elif "status" in values:
            return subprocess.CompletedProcess(values, 0, self.status_stdout, "")
        return subprocess.CompletedProcess(values, 0, "", "")

    def request(self, url: str, payload: object | None) -> tuple[int, object | None]:
        self.requests.append((url, payload))
        if url.endswith("v1/mcp"):
            if payload is None:
                return 200, {
                    "enabled": True,
                    "protocolVersion": self.mcp_protocol_version,
                }
            assert isinstance(payload, dict)
            if payload.get("method") == "initialize":
                params = payload.get("params")
                assert isinstance(params, dict)
                assert params.get("protocolVersion") == self.mcp_protocol_version
                return 200, {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"protocolVersion": self.mcp_protocol_version},
                }
            if payload.get("method") == "notifications/initialized":
                return 202, None
            if payload.get("method") == "tools/list":
                return 200, {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "result": {"tools": [{"name": "iroha.health"}]},
                }
            raise AssertionError(f"unexpected MCP payload: {payload}")
        for index in range(module.PEER_COUNT):
            if f":{self.api_port + index}/" not in url:
                continue
            if url.endswith("v1/sumeragi/status"):
                if self.sumeragi_status_http != 200:
                    return self.sumeragi_status_http, None
                blocker = (
                    {"blocker": "application_pending", "details": None}
                    if index == self.sumeragi_blocker_peer
                    else None
                )
                return 200, {
                    "protocol_version": 4,
                    "restart_required": index == self.restart_required_peer,
                    "liveness": {"blocker": blocker},
                }
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
        self.stability_patch = mock.patch.object(
            module, "POST_SMOKE_STABILITY_SECONDS", 0.0
        )
        self.stability_patch.start()
        self.addCleanup(self.stability_patch.stop)
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        for name in ("kagami", "iroha3d_taira", "iroha"):
            executable(self.bin_dir / name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_first_release_taira_identity_is_exact(self) -> None:
        self.assertEqual(
            module.DEFAULT_CHAIN_ID,
            "fc56984b-2be7-431d-840e-21514d1883f0",
        )
        self.assertEqual(module.DEFAULT_CHAIN_DISCRIMINANT, 369)

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

    def generated_network(self, name: str) -> tuple[FakeRuntime, Path]:
        """Ask the fake Kagami runtime for one unmodified generated network."""

        runtime = FakeRuntime()
        target = (self.root / name).resolve(strict=False)
        module.generate_network(
            target,
            self.bin_dir / "kagami",
            module.DEFAULT_API_PORT,
            module.DEFAULT_P2P_PORT,
            module.DEFAULT_BLOCK_CADENCE_MS,
            runtime.run,
        )
        return runtime, target

    def test_up_is_fresh_exact_four_and_proves_signed_finality(self) -> None:
        runtime = FakeRuntime()

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(report["baseline_height"], 1)
        self.assertEqual(report["final_height"], 2)
        self.assertEqual(report["transaction_hash"], "a" * 64)
        self.assertEqual(report["terminal_status"], "Applied")
        self.assertNotIn("inrou_canary", report)
        self.assertNotIn("inrou_stage", report)
        kagami = next(
            command
            for command in runtime.commands
            if "localnet" in command and "--out-dir" in command
        )
        self.assertIn("--fresh-random-keys", kagami)
        self.assertEqual(kagami[kagami.index("--peers") + 1], "4")
        self.assertEqual(kagami[kagami.index("--sora-profile") + 1], "nexus")
        self.assertEqual(kagami[kagami.index("--consensus-mode") + 1], "npos")
        self.assertEqual(
            kagami[kagami.index("--block-cadence-ms") + 1],
            str(module.DEFAULT_BLOCK_CADENCE_MS),
        )
        self.assertEqual(kagami[kagami.index("--chain-id") + 1], module.DEFAULT_CHAIN_ID)
        self.assertEqual(kagami[kagami.index("--bind-host") + 1], "127.0.0.1")
        self.assertEqual(kagami[kagami.index("--public-host") + 1], "127.0.0.1")
        self.assertIsNotNone(runtime.generation_pass_fds)
        self.assertEqual(len(runtime.generation_pass_fds), 1)
        config_checks = [
            command for command in runtime.commands if "--check-config" in command
        ]
        self.assertEqual(len(config_checks), 4)
        self.assertTrue(
            all(
                command.count(str(self.bin_dir / "iroha3d_taira")) == 1
                for command in config_checks
            )
        )

        self.assertEqual(sum("--no-wait" in command for command in runtime.commands), 1)
        self.assertEqual(sum("--wait" in command for command in runtime.commands), 1)
        self.assertEqual(sum("doctor" in command for command in runtime.commands), 0)
        expected_weights = dict(module.TAIRA_NEXUS_STORAGE_WEIGHTS)
        for index in range(module.PEER_COUNT):
            config = self.root / "state" / "network" / f"peer{index}.toml"
            self.assertEqual(
                module.section_assignment(
                    config, "nexus.storage", "local_budget_bytes"
                ),
                str(module.TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES),
            )
            for key, value in expected_weights.items():
                self.assertEqual(
                    module.section_assignment(
                        config, "nexus.storage.disk_budget_weights", key
                    ),
                    str(value),
                )
            self.assertEqual(
                module.section_assignment(
                    config, "sorafs.storage", "max_capacity_bytes"
                ),
                str(module.TAIRA_SORAFS_MAX_CAPACITY_BYTES),
            )
            self.assertEqual(
                module.section_assignment(config, "sorafs.storage", "enabled"),
                "false",
            )
            self.assertEqual(
                Path(module.section_assignment(config, "sorafs.storage", "data_dir")),
                (
                    self.root
                    / "state"
                    / "network"
                    / "state"
                    / f"peer{index}"
                    / "sorafs"
                ).resolve(),
            )
        ping = next(
            command
            for command in runtime.commands
            if "ping" in command and "--no-wait" in command
        )
        self.assertIn("--machine", ping)
        self.assertIn("--fee-payer", ping)
        self.assertIn("tx", ping)
        self.assertIn("--no-wait", ping)
        status = next(command for command in runtime.commands if "--wait" in command)
        self.assertIn("--wait", status)
        self.assertEqual(status[status.index("--hash") + 1], "a" * 64)
        self.assertEqual(status[status.index("--terminal-status") + 1], "applied")
        start = next(command for command in runtime.commands if command[0] == "/bin/bash")
        self.assertTrue(start[1].endswith("network/start.sh"))
        start_script = (self.root / "state" / "network" / "start.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn('CUSTODYFILE="$DIR/peer${i}.launching"', start_script)
        self.assertIn("printf '%s\\n' \"$peer_pid\"", start_script)
        self.assertIn('mv -f "$PIDFILE_TMP" "$PIDFILE"', start_script)
        self.assertNotIn('echo "$peer_pid" > "$PIDFILE"', start_script)
        self.assertIsNotNone(runtime.start_env)
        self.assertEqual(runtime.start_env["IROHA_LOCALNET_FAUCET_RESERVE_RETRIES"], "0")
        self.assertIsNotNone(runtime.start_pass_fds)
        self.assertEqual(len(runtime.start_pass_fds), 1)
        mcp_methods = [
            payload.get("method")
            for url, payload in runtime.requests
            if url.endswith("v1/mcp") and isinstance(payload, dict)
        ]
        self.assertEqual(
            mcp_methods,
            ["initialize", "notifications/initialized", "tools/list"]
            * module.PEER_COUNT,
        )
        mcp_roots = {
            url.removesuffix("v1/mcp")
            for url, payload in runtime.requests
            if url.endswith("v1/mcp") and payload is None
        }
        self.assertEqual(mcp_roots, set(module.torii_roots(module.DEFAULT_API_PORT)))

    def test_default_up_preflights_only_shipping_surfaces(self) -> None:
        runtime = FakeRuntime()

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertNotIn("inrou_canary", report)
        self.assertNotIn("inrou_stage", report)
        help_commands = [
            command for command in runtime.commands if "--help" in command
        ]
        self.assertFalse(
            any(command[0].endswith("sorafs-node") for command in help_commands)
        )
        self.assertFalse(any("inrou-stage" in command for command in help_commands))
        self.assertFalse(any("inrou-canary" in command for command in help_commands))

    def test_storage_overlay_fails_closed_before_rewriting_any_peer(self) -> None:
        source_nexus = (
            "[nexus.storage]\n"
            f"local_budget_bytes = {module.GENERATED_LOCALNET_NEXUS_STORAGE_BYTES}\n"
        )
        source_sorafs = "[sorafs.storage]\nenabled = false\n"
        cases = (
            (
                "missing",
                lambda text: text.replace(source_nexus, "", 1),
                "must contain one \\[nexus.storage\\]",
            ),
            (
                "duplicate",
                lambda text: text + "\n" + source_sorafs,
                "must contain one \\[sorafs.storage\\]",
            ),
            (
                "unexpected-section",
                lambda text: text
                + "\n[nexus.storage.disk_budget_weights]\nkura_blocks_bps = 1\n",
                "unexpected storage sections",
            ),
            (
                "unexpected-assignment",
                lambda text: text.replace(
                    source_nexus,
                    source_nexus + "fallback_budget_bytes = 1\n",
                    1,
                ),
                "wrong assignment set",
            ),
        )
        for name, mutate, error in cases:
            with self.subTest(name=name):
                _, target = self.generated_network(f"generated-{name}")
                peer0 = target / "peer0.toml"
                peer3 = target / "peer3.toml"
                peer0_before = peer0.read_text(encoding="utf-8")
                peer3.write_text(
                    mutate(peer3.read_text(encoding="utf-8")),
                    encoding="utf-8",
                )

                with self.assertRaisesRegex(module.DevnetError, error):
                    module.apply_canonical_taira_storage_profiles(target)

                self.assertEqual(peer0.read_text(encoding="utf-8"), peer0_before)

    def test_canonical_storage_validator_rejects_capacity_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        config = self.root / "state" / "network" / "peer2.toml"
        contents = config.read_text(encoding="utf-8")
        config.write_text(
            contents.replace(
                f"max_capacity_bytes = {module.TAIRA_SORAFS_MAX_CAPACITY_BYTES}",
                f"max_capacity_bytes = {module.TAIRA_SORAFS_MAX_CAPACITY_BYTES + 1}",
                1,
            ),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "wrong computed SoraFS capacity"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_default_deadline_matches_the_generated_transaction_window(self) -> None:
        args = module.parser().parse_args(
            [
                "--dir",
                str(self.root / "state"),
                "up",
                "--no-build",
                "--bin-dir",
                str(self.bin_dir),
            ]
        )

        self.assertEqual(args.timeout_seconds, 300)
        self.assertEqual(args.generation_timeout_seconds, 2 * 60)

    def test_up_waits_for_committed_genesis_before_signed_smoke(self) -> None:
        runtime = FakeRuntime()
        runtime.height = 0
        args = self.up_args()
        args.timeout_seconds = 0.01

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "required_above=0"):
                module.up(args, run=runtime.run, request=runtime.request)

        self.assertFalse(any("--no-wait" in command for command in runtime.commands))

    def test_post_smoke_stability_rechecks_owned_cohort(self) -> None:
        runtime = FakeRuntime()
        diagnostics = 0

        def diagnose() -> None:
            nonlocal diagnostics
            diagnostics += 1
            if diagnostics == 2:
                raise module.DevnetError("peer exited after initial convergence")

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(
                module.DevnetError, "peer exited after initial convergence"
            ):
                module.verify_cluster_stability(
                    module.torii_roots(module.DEFAULT_API_PORT),
                    runtime.height,
                    1.0,
                    runtime.request,
                    diagnose=diagnose,
                )

        self.assertEqual(diagnostics, 2)

    def test_post_smoke_stability_rejects_per_peer_height_rollback(self) -> None:
        runtime = FakeRuntime()
        block_reads = 0

        def request(url: str, payload: object | None) -> tuple[int, object | None]:
            nonlocal block_reads
            if url.endswith("status/blocks"):
                sample = block_reads // module.PEER_COUNT
                block_reads += 1
                return 200, (5, 6, 5)[min(sample, 2)]
            return runtime.request(url, payload)

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "height rollback"):
                module.verify_cluster_stability(
                    module.torii_roots(module.DEFAULT_API_PORT),
                    5,
                    1.0,
                    request,
                )

        self.assertEqual(block_reads, 3 * module.PEER_COUNT)

    def test_fresh_generation_has_a_bounded_wall_clock_deadline(self) -> None:
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
            module.DEFAULT_BLOCK_CADENCE_MS,
            run,
        )

        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0]["timeout"], module.DEFAULT_GENERATION_TIMEOUT_SECONDS)
        self.assertIs(calls[0]["capture_output"], False)

    def test_failed_readiness_stops_failed_cohort_without_activation_state(self) -> None:
        runtime = FakeRuntime()
        runtime.unhealthy_peer = 2
        args = self.up_args()
        args.timeout_seconds = 0.01

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "did not converge"):
                module.up(args, run=runtime.run, request=runtime.request)

        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_000, 10_001, 10_002, 10_003})
        state = self.root / "state"
        self.assertEqual((state / module.MARKER).read_text(encoding="utf-8"), module.MARKER_BODY)
        self.assertFalse((state / "current.json").exists())
        self.assertFalse((state / "generations").exists())
        self.assertFalse((state / "network" / module.RUNTIME_SIGNER_DIRECTORY).exists())

    def test_interrupted_startup_stops_the_generated_cohort(self) -> None:
        runtime = FakeRuntime()

        def interrupt(_url: str, _payload: object | None) -> tuple[int, object | None]:
            raise KeyboardInterrupt

        with self.assertRaisesRegex(module.DevnetError, "startup was interrupted"):
            module.up(self.up_args(), run=runtime.run, request=interrupt)

        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_000, 10_001, 10_002, 10_003})

    def test_check_is_read_only_and_down_needs_no_release_confirmation(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        ping_count = sum("--no-wait" in command for command in runtime.commands)
        state = self.root / "state"

        check_args = module.parser().parse_args(
            ["--dir", str(state), "check", "--timeout-seconds", "1"]
        )
        report = module.check(check_args, run=runtime.run, request=runtime.request)
        self.assertEqual(report["height"], 2)
        self.assertEqual(sum("--no-wait" in command for command in runtime.commands), ping_count)

        for path in module.runtime_signer_launch_paths(state / "network"):
            path.write_bytes(b"")
            path.chmod(0o600)

        down_args = module.parser().parse_args(["--dir", str(state), "down"])
        down_report = module.down(down_args, run=runtime.run)
        self.assertTrue(down_report["stopped"])
        self.assertTrue(down_report["runtime_signers_deleted"])
        self.assertFalse((state / "network" / module.RUNTIME_SIGNER_DIRECTORY).exists())
        self.assertFalse((state / "network" / "runtime").exists())

    def test_runtime_cleanup_removes_control_secrets_after_peer_keys_are_absent(self) -> None:
        _runtime, target = self.generated_network("network")
        signer_directory = target / module.RUNTIME_SIGNER_DIRECTORY
        for path in signer_directory.iterdir():
            path.unlink()
        signer_directory.rmdir()

        module.delete_runtime_signer_files(target)

        self.assertFalse((target / "runtime").exists())

    def test_check_derives_custom_ports_from_the_generated_bundle(self) -> None:
        runtime = FakeRuntime()
        module.up(
            self.up_args("--base-api-port", "30120"),
            run=runtime.run,
            request=runtime.request,
        )
        state = self.root / "state"

        args = module.parser().parse_args(
            ["--dir", str(state), "check", "--timeout-seconds", "1"]
        )
        report = module.check(args, run=runtime.run, request=runtime.request)

        self.assertEqual(report["torii_roots"][0], "http://127.0.0.1:30120/")
        self.assertEqual(report["torii_roots"][-1], "http://127.0.0.1:30123/")

    def test_signed_smoke_rejects_untyped_or_unbound_terminal_receipts(self) -> None:
        cases = [
            ("not-json", None, "transaction receipt"),
            (
                None,
                json.dumps({"hash": "b" * 64, "terminal_kind": "Applied"}),
                "Applied pipeline finality",
            ),
            (
                None,
                json.dumps({"hash": "a" * 64, "terminal_kind": "Rejected"}),
                "Applied pipeline finality",
            ),
        ]
        for ping_stdout, status_stdout, message in cases:
            with self.subTest(message=message):
                runtime = FakeRuntime()
                if ping_stdout is not None:
                    runtime.ping_stdout = ping_stdout
                if status_stdout is not None:
                    runtime.status_stdout = status_stdout

                with self.assertRaisesRegex(module.DevnetError, message):
                    module.up(self.up_args(), run=runtime.run, request=runtime.request)

                self.assertEqual(runtime.process_commands, {})
                self.assertEqual(
                    list((self.root / "state" / "network").glob("peer*.pid")), []
                )

    def test_down_and_replacement_fail_closed_on_residual_peer(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        runtime.leave_peer_running_on_stop = True
        state = self.root / "state"
        down_args = module.parser().parse_args(["--dir", str(state), "down"])

        with mock.patch.object(module.time, "sleep", return_value=None):
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

    def test_down_accepts_an_already_absent_runtime_signer_directory(self) -> None:
        state = module.managed_root(self.root / "state", create=True)
        target = state / "network"
        target.mkdir()

        args = module.parser().parse_args(["--dir", str(state), "down"])
        report = module.down(args, run=FakeRuntime().run)

        self.assertTrue(report["stopped"])
        self.assertTrue(report["runtime_signers_deleted"])

    def test_down_cannot_overtake_an_active_mutating_command(self) -> None:
        state = module.managed_root(self.root / "state", create=True)
        args = module.parser().parse_args(["--dir", str(state), "down"])
        runtime = FakeRuntime()

        with module.mutation_lock(state):
            with self.assertRaisesRegex(
                module.DevnetError,
                "another Taira devnet mutation is already running",
            ):
                module.down(args, run=runtime.run)

        self.assertEqual(runtime.commands, [])

    def test_launcher_inherits_the_mutation_lock_until_it_exits(self) -> None:
        state = module.managed_root(self.root / "state", create=True)
        child: subprocess.Popen[bytes] | None = None
        try:
            with module.mutation_lock(state) as descriptor:
                child = subprocess.Popen(
                    [
                        sys.executable,
                        "-c",
                        "import sys; sys.stdin.buffer.read(1)",
                    ],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    pass_fds=(descriptor,),
                )

            with self.assertRaisesRegex(
                module.DevnetError,
                "another Taira devnet mutation is already running",
            ):
                with module.mutation_lock(state):
                    pass

            child.communicate(b"x", timeout=5)
            self.assertEqual(child.returncode, 0)
            with module.mutation_lock(state):
                pass
        finally:
            if child is not None and child.poll() is None:
                child.communicate(b"x", timeout=5)

    def test_check_and_down_do_not_require_the_retired_stop_script(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        state = self.root / "state"
        (state / "network" / "stop.sh").unlink()

        check_args = module.parser().parse_args(
            ["--dir", str(state), "check", "--timeout-seconds", "1"]
        )
        self.assertEqual(
            module.check(check_args, run=runtime.run, request=runtime.request)["height"],
            2,
        )
        down_args = module.parser().parse_args(["--dir", str(state), "down"])
        self.assertTrue(module.down(down_args, run=runtime.run)["stopped"])

    def test_down_recovers_each_verified_peer_from_a_partial_pid_cohort(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        state = self.root / "state"
        target = state / "network"
        (target / "peer1.pid").unlink()
        runtime.process_commands.pop(10_001)

        args = module.parser().parse_args(["--dir", str(state), "down"])
        report = module.down(args, run=runtime.run)

        self.assertTrue(report["stopped"])
        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_000, 10_002, 10_003})
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])

    def test_down_recovers_spawned_peer_before_atomic_pid_publication(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        state = self.root / "state"
        target = state / "network"
        (target / "peer1.pid").unlink()
        launch_path, pid_temporary = module.peer_launch_paths(target, 1)
        launch_path.touch(mode=0o600)
        pid_temporary.write_text("10001\n", encoding="utf-8")
        pid_temporary.chmod(0o600)

        args = module.parser().parse_args(["--dir", str(state), "down"])
        report = module.down(args, run=runtime.run)

        self.assertTrue(report["stopped"])
        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_000, 10_001, 10_002, 10_003})
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])
        self.assertFalse(launch_path.exists())
        self.assertFalse(pid_temporary.exists())

    def test_down_accepts_peer_exit_between_ownership_check_and_signal(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        runtime.exit_before_kill_peer = 10_000
        target = self.root / "state" / "network"

        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )
        report = module.down(args, run=runtime.run)

        self.assertTrue(report["stopped"])
        self.assertTrue(report["runtime_signers_deleted"])
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])

    def test_failed_up_retains_signers_when_teardown_is_not_proven(self) -> None:
        runtime = FakeRuntime()
        runtime.unhealthy_peer = 2
        runtime.leave_peer_running_on_stop = True
        args = self.up_args()
        args.timeout_seconds = 0.01

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "did not converge"):
                module.up(args, run=runtime.run, request=runtime.request)

        target = self.root / "state" / "network"
        self.assertTrue((target / module.RUNTIME_SIGNER_DIRECTORY).is_dir())
        self.assertTrue((target / "peer0.pid").is_file())

    def test_down_removes_stale_pid_evidence_for_an_already_stopped_peer(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        runtime.process_commands.pop(10_000)

        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )
        report = module.down(args, run=runtime.run)

        self.assertTrue(report["stopped"])
        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_001, 10_002, 10_003})
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])

    def test_down_waits_through_transient_exiting_process_argv_loss(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        runtime.transient_command_loss_on_stop = True
        target = self.root / "state" / "network"

        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )
        with mock.patch.object(module.time, "sleep", return_value=None):
            report = module.down(args, run=runtime.run)

        self.assertTrue(report["stopped"])
        self.assertTrue(report["runtime_signers_deleted"])
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(runtime.exiting_process_polls, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])

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

    def test_down_stops_verified_peers_but_retains_mismatched_pid_evidence(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        runtime.process_commands[10_000] = (
            f"/fake/iroha3d_taira --sora --config {target / 'peer0.toml'}.backup"
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )

        with self.assertRaisesRegex(module.DevnetError, "not the sole running process"):
            module.down(args, run=runtime.run)

        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_001, 10_002, 10_003})
        self.assertTrue((target / "peer0.pid").is_file())
        self.assertEqual(set(runtime.process_commands), {10_000})

    def test_down_stops_verified_peers_but_retains_malformed_pid_evidence(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        pid_path = target / "peer0.pid"
        pid_path.write_text("not-a-pid\n", encoding="utf-8")
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )

        with self.assertRaisesRegex(module.DevnetError, "PID file is malformed"):
            module.down(args, run=runtime.run)

        terminated = {
            int(command[2])
            for command in runtime.commands
            if command[:2] == ("/bin/kill", "-TERM")
        }
        self.assertEqual(terminated, {10_001, 10_002, 10_003})
        self.assertEqual(pid_path.read_text(encoding="utf-8"), "not-a-pid\n")
        self.assertEqual(set(runtime.process_commands), {10_000})

    def test_status_fail_stop_and_watchdog_blockers_are_terminal_when_exposed(self) -> None:
        cases = (("restart", 1, None), ("blocker", None, 2))
        for label, restart_peer, blocker_peer in cases:
            with self.subTest(label=label):
                runtime = FakeRuntime()
                runtime.restart_required_peer = restart_peer
                runtime.sumeragi_blocker_peer = blocker_peer
                message = "requires restart" if restart_peer is not None else "liveness blocker"

                with self.assertRaisesRegex(module.DevnetError, message):
                    module.up(self.up_args(), run=runtime.run, request=runtime.request)

                self.assertEqual(runtime.process_commands, {})

    def test_structured_watchdog_log_exposes_blocker_when_status_is_unavailable(self) -> None:
        target = self.root / "network"
        target.mkdir()
        record = {
            "level": "WARN",
            "fields": {
                "message": module.SUMERAGI_NO_PROGRESS_LOG_MESSAGE,
                "blocker": "SuccessorActivationPending",
                "height": 3,
            },
            "target": "iroha_core::sumeragi::status",
        }
        (target / "peer2.log").write_text(
            json.dumps(record) + "\n",
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            module.DevnetError,
            "peer2 log at height 3: SuccessorActivationPending",
        ):
            module.check_sumeragi_liveness_logs(target)

    def test_log_monitor_tracks_active_recovery_and_committed_successor(self) -> None:
        target = self.root / "network"
        target.mkdir()
        blocked = json.dumps(
            {
                "level": "WARN",
                "fields": {
                    "message": module.SUMERAGI_NO_PROGRESS_LOG_MESSAGE,
                    "blocker": "SuccessorActivationPending",
                    "height": 3,
                },
                "target": "iroha_core::sumeragi::status",
            }
        )
        path = target / "peer0.log"
        path.write_text(blocked + "\n", encoding="utf-8")

        with self.assertRaisesRegex(
            module.DevnetError,
            "peer0 log at height 3: SuccessorActivationPending",
        ):
            module.check_sumeragi_liveness_logs(
                target,
                committed_heights=[2, 2, 2, 2],
            )

        recovered = json.dumps(
            {
                "level": "INFO",
                "fields": {
                    "message": module.SUMERAGI_PROGRESS_RECOVERED_LOG_MESSAGE,
                    "recovered_blocker": "SuccessorActivationPending",
                    "height": 3,
                },
                "target": "iroha_core::sumeragi::status",
            }
        )
        with path.open("a", encoding="utf-8") as output:
            output.write(recovered + "\n")
        module.check_sumeragi_liveness_logs(
            target,
            committed_heights=[2, 2, 2, 2],
        )

        with path.open("a", encoding="utf-8") as output:
            output.write(blocked.replace('"height": 3', '"height": 1') + "\n")
        module.check_sumeragi_liveness_logs(
            target,
            committed_heights=[2, 2, 2, 2],
        )

    def test_fresh_log_cursor_rejects_a_new_blocker(self) -> None:
        target = self.root / "network"
        target.mkdir()
        path = target / "peer1.log"
        path.write_text("old startup line\n", encoding="utf-8")
        offsets = module.peer_log_offsets(target)
        record = {
            "fields": {
                "message": module.SUMERAGI_NO_PROGRESS_LOG_MESSAGE,
                "blocker": "BodyUnavailable",
                "height": 2,
            },
            "target": "iroha_core::sumeragi::status",
        }
        with path.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\n")

        with self.assertRaisesRegex(module.DevnetError, "peer1 log at height 2"):
            module.check_sumeragi_liveness_logs(target, offsets)

    def test_authoritative_clean_status_supersedes_a_large_historical_log(self) -> None:
        target = self.root / "network"
        target.mkdir()
        log = target / "peer0.log"
        with log.open("wb") as output:
            output.truncate(module.MAX_INITIAL_LOG_STATE_SCAN_BYTES + 1)

        offsets = module.check_sumeragi_liveness_logs(
            target,
            committed_heights=[2, 2, 2, 2],
            authoritative_status=[True, True, True, True],
        )

        self.assertEqual(offsets[0], module.MAX_INITIAL_LOG_STATE_SCAN_BYTES + 1)

    def test_cluster_wait_runs_owned_diagnostics_before_retryable_http(self) -> None:
        requests = 0

        def request(_url: str, _payload: object | None) -> tuple[int, None]:
            nonlocal requests
            requests += 1
            return 0, None

        def diagnose() -> None:
            raise module.DevnetError("owned peer exited")

        with self.assertRaisesRegex(module.DevnetError, "owned peer exited"):
            module.wait_for_cluster(
                module.torii_roots(module.DEFAULT_API_PORT),
                30,
                request,
                diagnose=diagnose,
            )

        self.assertEqual(requests, 0)

    def test_unavailable_operator_status_does_not_replace_portable_smoke(self) -> None:
        runtime = FakeRuntime()
        runtime.sumeragi_status_http = 401

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(report["terminal_status"], "Applied")

    def test_check_rejects_an_already_active_log_blocker_at_current_height(self) -> None:
        runtime = FakeRuntime()
        runtime.sumeragi_status_http = 401
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        record = {
            "fields": {
                "message": module.SUMERAGI_NO_PROGRESS_LOG_MESSAGE,
                "blocker": "SuccessorActivationPending",
                "height": runtime.height,
            },
            "target": "iroha_core::sumeragi::status",
        }
        (target / "peer3.log").write_text(
            json.dumps(record) + "\n",
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(
            module.DevnetError,
            "peer3 log at height 2: SuccessorActivationPending",
        ):
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

    def test_check_rejects_client_chain_discriminant_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        client = self.root / "state" / "network" / "client.toml"
        client.write_text(
            client.read_text(encoding="utf-8").replace(
                f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}",
                f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT + 1}",
            ),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(
            module.DevnetError, "wrong Taira chain discriminant"
        ):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_peer_chain_discriminant_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        peer = self.root / "state" / "network" / "peer2.toml"
        peer.write_text(
            peer.read_text(encoding="utf-8").replace(
                f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}",
                f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT + 1}",
            ),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(
            module.DevnetError, "wrong Taira chain discriminant"
        ):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_client_network_id_checksum_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        client = self.root / "state" / "network" / "client.toml"
        contents = client.read_text(encoding="utf-8")
        network_id = module.quoted_assignment(client, "network_id")
        replacement = network_id[:-1] + ("0" if network_id[-1] != "0" else "1")
        client.write_text(contents.replace(network_id, replacement), encoding="utf-8")
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "does not match its genesis hash"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_peer_genesis_identity_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        config = self.root / "state" / "network" / "peer2.toml"
        contents = config.read_text(encoding="utf-8")
        network_id = module.quoted_assignment(config, "expected_hash")
        foreign = module.network_id_from_genesis_hash("1" * 63 + "3")
        config.write_text(contents.replace(network_id, foreign), encoding="utf-8")
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "genesis hash does not match"):
            module.check(args, run=runtime.run, request=runtime.request)

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

    def test_build_command_selects_only_the_shipping_toolchain(self) -> None:
        command = module.cargo_build_command("local-release", Path("/tmp/taira-target"))
        self.assertEqual(command[0], str(REPO_ROOT / "scripts" / "cargo_fast.sh"))
        self.assertIn("--stable-local-metadata", command)
        self.assertIn("--no-sccache", command)
        self.assertEqual(command[command.index("--target-dir") + 1], "/tmp/taira-target")
        self.assertEqual(command.count("--bin"), 3)
        rendered = " ".join(command)
        self.assertIn("iroha3d_taira", rendered)
        self.assertNotIn("sorafs-node", rendered)
        self.assertNotIn("external-software-signer-bin", rendered)
        self.assertIn("--locked", command)
        self.assertNotIn("--features", command)
        self.assertNotIn("--jobs", command[: command.index("--")])

        runtime = FakeRuntime()
        state = module.managed_root(self.root / "state", create=True)
        network = state / "network"
        network.mkdir()
        sentinel = network / "keep"
        sentinel.write_text("running cohort\n", encoding="utf-8")
        args = module.parser().parse_args(
            ["--dir", str(state), "up", "--bin-dir", str(self.bin_dir)]
        )
        with self.assertRaisesRegex(module.DevnetError, "--bin-dir requires --no-build"):
            module.up(args, run=runtime.run, request=runtime.request)
        self.assertEqual(runtime.commands, [])
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "running cohort\n")

    def test_build_jobs_override_is_validated_and_passed_to_cargo_fast(self) -> None:
        self.assertEqual(module.positive_integer("12"), 12)
        for invalid in ("0", "-1", "+1", "1.0", "many"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(module.argparse.ArgumentTypeError):
                    module.positive_integer(invalid)
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                module.parser().parse_args(["up", "--jobs", "0"])

        target_dir = self.root / "target"
        bin_dir = target_dir / "local-release"
        bin_dir.mkdir(parents=True)
        for name in ("kagami", "iroha3d_taira", "iroha"):
            executable(bin_dir / name)
        args = module.parser().parse_args(
            [
                "--dir",
                str(self.root / "state"),
                "up",
                "--target-dir",
                str(target_dir),
                "--jobs",
                "6",
            ]
        )
        commands: list[tuple[str, ...]] = []

        def run(
            command: list[str] | tuple[str, ...],
            **_kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            commands.append(tuple(command))
            return subprocess.CompletedProcess(command, 0, "", "")

        module.binary_paths(args, run)

        build = commands[0]
        separator = build.index("--")
        self.assertEqual(build[build.index("--jobs") + 1], "6")
        self.assertLess(build.index("--jobs"), separator)

    def test_cargo_fast_no_sccache_build_removes_conflicting_environment(self) -> None:
        target_dir = self.root / "target"
        bin_dir = target_dir / "local-release"
        bin_dir.mkdir(parents=True)
        for name in ("kagami", "iroha3d_taira", "iroha"):
            executable(bin_dir / name)
        args = module.parser().parse_args(
            [
                "--dir",
                str(self.root / "state"),
                "up",
                "--target-dir",
                str(target_dir),
            ]
        )
        calls: list[dict[str, object]] = []

        def run(
            command: list[str] | tuple[str, ...],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            calls.append(kwargs)
            return subprocess.CompletedProcess(command, 0, "", "")

        with mock.patch.dict(
            os.environ,
            {
                "CARGO_BUILD_JOBS": "1",
                "CARGO_INCREMENTAL": "1",
                "RUSTC_WRAPPER": "sccache",
                "TAIRA_TEST_ENV_RETAINED": "yes",
            },
        ):
            module.binary_paths(args, run)

        build_env = calls[0]["env"]
        self.assertIsInstance(build_env, dict)
        assert isinstance(build_env, dict)
        self.assertIsNone(calls[0]["timeout"])
        self.assertNotIn("CARGO_BUILD_JOBS", build_env)
        self.assertNotIn("CARGO_INCREMENTAL", build_env)
        self.assertNotIn("RUSTC_WRAPPER", build_env)
        self.assertEqual(build_env["TAIRA_TEST_ENV_RETAINED"], "yes")

    def test_command_deadlines_require_finite_positive_seconds(self) -> None:
        self.assertEqual(module.finite_positive_float("0.25"), 0.25)
        for invalid in ("0", "-1", "inf", "-inf", "nan", "soon"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(module.argparse.ArgumentTypeError):
                    module.finite_positive_float(invalid)

        deadline_options = (
            ("up", "--generation-timeout-seconds"),
            ("up", "--timeout-seconds"),
            ("check", "--timeout-seconds"),
        )
        for command, option in deadline_options:
            with self.subTest(command=command, option=option):
                with contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit):
                        module.parser().parse_args([command, option, "inf"])

    def test_compiled_surface_preflight_precedes_destructive_replacement(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        sentinel = target / "preserve-before-preflight"
        sentinel.write_text("live cohort\n", encoding="utf-8")
        status_surface = ("iroha", ("tx", "status"))
        ping_surface = ("iroha", ("tx", "ping"))
        runtime.help_options_by_surface[status_surface].remove("--terminal-status")
        runtime.help_options_by_surface[ping_surface].add("--terminal-status")
        stop_count = sum(
            command[:2] == ("/bin/kill", "-TERM") for command in runtime.commands
        )

        with self.assertRaisesRegex(module.DevnetError, "compiled CLI surface"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(sentinel.read_text(encoding="utf-8"), "live cohort\n")
        self.assertEqual(
            sum(
                command[:2] == ("/bin/kill", "-TERM")
                for command in runtime.commands
            ),
            stop_count,
        )
        self.assertEqual(len(runtime.process_commands), module.PEER_COUNT)

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

        def open_plain(request, *, timeout: int):
            self.assertEqual(timeout, 3)
            self.assertEqual(request.get_header("Accept"), "text/plain")
            return PlainResponse()

        with mock.patch.object(module.urllib.request, "urlopen", side_effect=open_plain):
            status, payload = module.http_request("http://127.0.0.1:29080/health")

        self.assertEqual(status, 200)
        self.assertEqual(payload, "Healthy")

    def test_http_request_keeps_json_accept_for_torii_json_routes(self) -> None:
        class JsonResponse:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args: object) -> None:
                return None

            @staticmethod
            def read(_limit: int = -1) -> bytes:
                return b"2"

        def open_json(request, *, timeout: int):
            self.assertEqual(timeout, 3)
            self.assertEqual(request.get_header("Accept"), "application/json")
            return JsonResponse()

        with mock.patch.object(module.urllib.request, "urlopen", side_effect=open_json):
            status, payload = module.http_request("http://127.0.0.1:29080/status/blocks")

        self.assertEqual(status, 200)
        self.assertEqual(payload, 2)

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

    def test_failed_up_dumps_logs_before_peer_teardown(self) -> None:
        runtime = FakeRuntime()
        runtime.unhealthy_peer = 2
        args = self.up_args()
        args.timeout_seconds = 0.01
        events: list[str] = []
        original_stop_network = module.stop_network

        def record_logs(_target: Path) -> None:
            self.assertTrue(runtime.process_commands)
            events.append("logs")

        def record_stop(*stop_args: object, **stop_kwargs: object) -> bool:
            if runtime.process_commands:
                events.append("stop")
            return original_stop_network(*stop_args, **stop_kwargs)

        with (
            mock.patch.object(module, "dump_logs", side_effect=record_logs),
            mock.patch.object(module, "stop_network", side_effect=record_stop),
            mock.patch.object(module.time, "sleep", return_value=None),
        ):
            with self.assertRaisesRegex(module.DevnetError, "did not converge"):
                module.up(args, run=runtime.run, request=runtime.request)

        self.assertEqual(events, ["logs", "stop"])
        self.assertEqual(runtime.process_commands, {})

    def test_post_start_command_spawn_failure_stops_the_owned_cohort(self) -> None:
        runtime = FakeRuntime()

        def run(
            command: list[str] | tuple[str, ...],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            values = tuple(str(value) for value in command)
            if "ping" in values:
                return module.run_command(
                    [str(self.root / "missing-after-start")],
                    timeout=1,
                )
            return runtime.run(command, **kwargs)

        with self.assertRaisesRegex(
            module.DevnetError,
            "could not start missing-after-start",
        ):
            module.up(self.up_args(), run=run, request=runtime.request)

        target = self.root / "state" / "network"
        self.assertEqual(runtime.process_commands, {})
        self.assertEqual(list(target.glob("peer*.pid")), [])
        self.assertFalse((target / module.RUNTIME_SIGNER_DIRECTORY).exists())

    def test_interruption_reports_when_the_cohort_cannot_be_proven_stopped(self) -> None:
        runtime = FakeRuntime()
        runtime.leave_peer_running_on_stop = True

        def run(
            command: list[str] | tuple[str, ...],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            values = tuple(str(value) for value in command)
            if "ping" in values and "--no-wait" in values:
                raise KeyboardInterrupt
            return runtime.run(command, **kwargs)

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(
                module.DevnetError,
                "teardown could not be proven; retained peers may still be live",
            ):
                module.up(self.up_args(), run=run, request=runtime.request)

        target = self.root / "state" / "network"
        self.assertEqual(set(runtime.process_commands), {10_000})
        self.assertTrue((target / "peer0.pid").is_file())
        self.assertTrue((target / module.RUNTIME_SIGNER_DIRECTORY).is_dir())

    def test_bounded_command_timeout_kills_only_its_private_process_group(self) -> None:
        timeout = subprocess.TimeoutExpired(["helper", "work"], 7)
        process = mock.Mock()
        process.pid = 43_210
        process.communicate.side_effect = [timeout, ("", "")]

        with (
            mock.patch.object(module.subprocess, "Popen", return_value=process) as popen,
            mock.patch.object(module.os, "getpgid", return_value=process.pid),
            mock.patch.object(module.os, "killpg") as killpg,
        ):
            with self.assertRaisesRegex(module.DevnetError, "helper timed out after 7s"):
                module.run_command(["helper", "work"], timeout=7)

        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        killpg.assert_called_once_with(process.pid, module.signal.SIGKILL)
        process.wait.assert_called_once_with()

    def test_bounded_command_heartbeat_preempts_and_kills_only_its_child(self) -> None:
        process = mock.Mock()
        process.pid = 43_211
        process.communicate.return_value = ("", "")

        def heartbeat() -> None:
            raise module.DevnetError("owned peer published a liveness blocker")

        with (
            mock.patch.object(module.subprocess, "Popen", return_value=process),
            mock.patch.object(module.os, "getpgid", return_value=process.pid),
            mock.patch.object(module.os, "killpg") as killpg,
        ):
            with self.assertRaisesRegex(module.DevnetError, "liveness blocker"):
                module.run_command(
                    ["helper", "work"],
                    timeout=7,
                    heartbeat=heartbeat,
                )

        killpg.assert_called_once_with(process.pid, module.signal.SIGKILL)
        process.wait.assert_called_once_with()

    def test_cargo_timeout_is_rejected_without_starting_or_signaling_cargo(self) -> None:
        with (
            mock.patch.object(module.subprocess, "Popen") as popen,
            mock.patch.object(module.os, "killpg") as killpg,
        ):
            with self.assertRaisesRegex(
                module.DevnetError, "Cargo and rustc commands must run without"
            ):
                module.run_command(
                    [str(REPO_ROOT / "scripts" / "cargo_fast.sh"), "--", "build"],
                    timeout=7,
                )

        popen.assert_not_called()
        killpg.assert_not_called()

    def test_mcp_rejects_stale_protocol_and_nonaccepted_notification(self) -> None:
        def stale_request(_url: str, payload: object | None) -> tuple[int, object]:
            if payload is None:
                return 200, {
                    "enabled": True,
                    "protocolVersion": "advertised-version",
                }
            return 200, {
                "jsonrpc": "2.0",
                "id": 1,
                "result": {"protocolVersion": "stale"},
            }

        with self.assertRaisesRegex(module.DevnetError, "MCP initialize failed"):
            module.check_mcp("http://127.0.0.1:29080/", stale_request)

        def rejected_notification(
            _url: str, payload: object | None
        ) -> tuple[int, object | None]:
            if payload is None:
                return 200, {
                    "enabled": True,
                    "protocolVersion": "advertised-version",
                }
            assert isinstance(payload, dict)
            if payload.get("method") == "initialize":
                return 200, {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"protocolVersion": "advertised-version"},
                }
            if payload.get("method") == "notifications/initialized":
                return 200, None
            raise AssertionError(f"unexpected MCP payload: {payload}")

        with self.assertRaisesRegex(
            module.DevnetError, "MCP initialized notification failed"
        ):
            module.check_mcp("http://127.0.0.1:29080/", rejected_notification)

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
        up_help = subprocess.run(
            [sys.executable, str(MODULE_PATH), "up", "--help"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(up_help.returncode, 0)
        self.assertNotIn("--inrou-canary-dir", up_help.stdout)
        self.assertNotIn("--full-doctor", up_help.stdout)
        self.assertNotIn("--build-timeout-seconds", up_help.stdout)
        check_help = subprocess.run(
            [sys.executable, str(MODULE_PATH), "check", "--help"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(check_help.returncode, 0)
        self.assertNotIn("--full-doctor", check_help.stdout)
        self.assertNotIn("--iroha", check_help.stdout)

    def test_retired_taira_orchestration_does_not_reappear(self) -> None:
        def names(directory: Path, pattern: str = "*taira*") -> set[str]:
            return {entry.name for entry in directory.glob(pattern) if entry.is_file()}

        self.assertEqual(
            names(REPO_ROOT / "scripts"),
            {"render_taira_edge_nginx_conf.py", "taira_constants.py", "taira_devnet.py"},
        )
        self.assertEqual(
            names(REPO_ROOT / "scripts" / "tests"),
            {
                "render_taira_edge_nginx_conf_test.py",
                "taira_devnet_test.py",
                "taira_inrou_canary_identity_source_test.py",
            },
        )
        config_root = REPO_ROOT / "configs" / "soranexus" / "taira"
        self.assertEqual(
            names(config_root, "*.sh"),
            {
                "install_taira_edge_nginx_conf.sh",
                "install_taira_edge_nginx_conf_mock_test.sh",
            },
        )
        self.assertEqual(names(config_root, "*.py"), set())
        self.assertFalse((REPO_ROOT / "defaults" / "kagami" / "iroha3-taira").exists())
        self.assertFalse(
            (
                REPO_ROOT
                / "crates"
                / "iroha_kagami"
                / "examples"
                / "taira_kaigi_localnet.rs"
            ).exists()
        )
        self.assertEqual(names(REPO_ROOT / ".github" / "workflows"), set())
        self.assertEqual(names(REPO_ROOT / "ci"), set())
        self.assertEqual(
            names(REPO_ROOT / "crates" / "iroha_cli" / "src" / "bin"),
            {"taira_fee_sponsor_program.rs"},
        )
        self.assertEqual(
            names(REPO_ROOT / "crates" / "irohad" / "src" / "bin"),
            {"iroha3d_taira.rs", "taira_bootle_lantern_broker.rs"},
        )
        self.assertEqual(
            names(REPO_ROOT / "crates" / "iroha_test_network" / "src" / "bin"),
            set(),
        )


if __name__ == "__main__":
    unittest.main()
