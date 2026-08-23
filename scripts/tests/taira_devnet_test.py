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
        self.doctor_fails = False
        self.leave_peer_running_on_stop = False
        self.process_commands: dict[int, str] = {}
        self.start_env: dict[str, str] | None = None
        self.mcp_protocol_version = "taira-test-protocol-v1"
        self.requests: list[tuple[str, object | None]] = []
        self.api_port = module.DEFAULT_API_PORT
        self.help_options = {
            option
            for _binary, _subcommands, options in (
                *module.CLI_SURFACES,
                *module.INROU_CLI_SURFACES,
            )
            for option in options
        } | {"--public-root", "--json"}
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
            return subprocess.CompletedProcess(
                values,
                0,
                "\n".join(sorted(self.help_options)),
                "",
            )
        if "localnet" in values:
            target = Path(values[values.index("--out-dir") + 1])
            api_port = int(values[values.index("--base-api-port") + 1])
            self.api_port = api_port
            target.mkdir(mode=0o700)
            for name in ("start.sh", "stop.sh"):
                executable(target / name, b"#!/usr/bin/env bash\n")
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
        elif "inrou-stage" in values:
            stage = Path(values[values.index("--stage-dir") + 1])
            (stage / "manifests").mkdir(parents=True, mode=0o700)
            (stage / "payloads" / "guest" / "aarch64").mkdir(
                parents=True, mode=0o700
            )
            (stage / module.INROU_STAGE_RECEIPT_FILE).write_text(
                '{"schema_version":1}\n', encoding="utf-8"
            )
            (stage / module.INROU_STAGE_BUNDLE_PAYLOAD).write_bytes(b"bundle")
            (stage / module.INROU_STAGE_BUNDLE_MANIFEST).write_bytes(b"bundle-manifest")
            (stage / module.INROU_STAGE_GUEST_MANIFEST).write_bytes(b"guest-manifest")
            (stage / module.INROU_STAGE_GUEST_PAYLOAD / "aarch64" / "kernel").write_bytes(
                b"kernel"
            )
            stage.chmod(0o700)
            for staged_file in (
                stage / module.INROU_STAGE_RECEIPT_FILE,
                stage / module.INROU_STAGE_BUNDLE_PAYLOAD,
                stage / module.INROU_STAGE_BUNDLE_MANIFEST,
                stage / module.INROU_STAGE_GUEST_MANIFEST,
            ):
                staged_file.chmod(0o600)
        elif values[0] == "/bin/bash" and values[1].endswith("/start.sh"):
            target = Path(str(kwargs["cwd"]))
            self.start_env = dict(kwargs["env"])
            for index in range(module.PEER_COUNT):
                pid = 10_000 + index
                (target / f"peer{index}.pid").write_text(f"{pid}\n", encoding="utf-8")
                self.process_commands[pid] = (
                    f"/fake/iroha3d_taira --sora --config {target / f'peer{index}.toml'}"
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
            return subprocess.CompletedProcess(values, 0, self.ping_stdout, "")
        elif "status" in values:
            return subprocess.CompletedProcess(values, 0, self.status_stdout, "")
        elif "doctor" in values and self.doctor_fails:
            raise module.DevnetError("full doctor failed")
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
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        for name in ("kagami", "iroha3d_taira", "iroha", "sorafs-node"):
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

    def inrou_canary_workspace(self) -> Path:
        """Create the exact three-file runtime-only canary interface."""

        workspace = self.root / "inrou-canary"
        workspace.mkdir()
        (workspace / module.INROU_CANARY_CONTAINER_FILE).write_text(
            "{}\n", encoding="utf-8"
        )
        (workspace / module.INROU_CANARY_SERVICE_FILE).write_text(
            "{}\n", encoding="utf-8"
        )
        (workspace / module.INROU_CANARY_BUNDLE_FILE).write_bytes(b"bundle")
        return workspace

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
        self.assertFalse(report["inrou_canary"])
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
        self.assertIsNotNone(runtime.start_env)
        self.assertEqual(runtime.start_env["IROHA_LOCALNET_FAUCET_RESERVE_RETRIES"], "0")
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

    def test_default_up_excludes_unused_sorafs_and_inrou_gates(self) -> None:
        (self.bin_dir / "sorafs-node").unlink()
        runtime = FakeRuntime()

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertFalse(report["inrou_canary"])
        help_commands = [
            command for command in runtime.commands if "--help" in command
        ]
        self.assertFalse(
            any(command[0].endswith("sorafs-node") for command in help_commands)
        )
        self.assertFalse(any("inrou-stage" in command for command in help_commands))
        self.assertFalse(any("inrou-canary" in command for command in help_commands))

    def test_inrou_opt_in_requires_and_preflights_optional_toolchain(self) -> None:
        workspace = self.inrou_canary_workspace()
        (self.bin_dir / "sorafs-node").unlink()
        runtime = FakeRuntime()
        args = self.up_args("--inrou-canary-dir", str(workspace))

        with self.assertRaisesRegex(module.DevnetError, "sorafs-node"):
            module.up(args, run=runtime.run, request=runtime.request)

        self.assertEqual(runtime.commands, [])
        executable(self.bin_dir / "sorafs-node")

        report = module.up(args, run=runtime.run, request=runtime.request)

        self.assertTrue(report["inrou_canary"])
        help_commands = [
            command for command in runtime.commands if "--help" in command
        ]
        self.assertIn((str(self.bin_dir / "sorafs-node"), "--help"), help_commands)
        self.assertIn(
            (str(self.bin_dir / "iroha"), "taira", "inrou-stage", "--help"),
            help_commands,
        )
        self.assertIn(
            (str(self.bin_dir / "iroha"), "taira", "inrou-canary", "--help"),
            help_commands,
        )
        self.assertFalse(any("doctor" in command for command in help_commands))

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

    def test_up_waits_for_committed_genesis_before_signed_smoke(self) -> None:
        runtime = FakeRuntime()
        runtime.height = 0
        args = self.up_args()
        args.timeout_seconds = 0.01

        with mock.patch.object(module.time, "sleep", return_value=None):
            with self.assertRaisesRegex(module.DevnetError, "required_above=0"):
                module.up(args, run=runtime.run, request=runtime.request)

        self.assertFalse(any("--no-wait" in command for command in runtime.commands))

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
            module.DEFAULT_BLOCK_CADENCE_MS,
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
        (target / "stop.sh").write_text("#!/bin/sh\n", encoding="utf-8")

        args = module.parser().parse_args(["--dir", str(state), "down"])
        report = module.down(args, run=FakeRuntime().run)

        self.assertTrue(report["stopped"])
        self.assertTrue(report["runtime_signers_deleted"])

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

    def test_down_does_not_run_generated_stop_before_exact_process_ownership(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        runtime.process_commands[10_000] = (
            f"/fake/iroha3d_taira --sora --config {target / 'peer0.toml'}.backup"
        )
        stop_count = sum(
            command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
            for command in runtime.commands
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "down"]
        )

        with self.assertRaisesRegex(module.DevnetError, "not the sole running process"):
            module.down(args, run=runtime.run)

        self.assertEqual(
            sum(
                command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
                for command in runtime.commands
            ),
            stop_count,
        )
        self.assertTrue((target / "peer0.pid").is_file())

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

    def test_unavailable_operator_status_does_not_replace_portable_smoke(self) -> None:
        runtime = FakeRuntime()
        runtime.sumeragi_status_http = 401

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(report["terminal_status"], "Applied")

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

    def test_full_public_doctor_is_opt_in(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace()
        report = module.up(
            self.up_args(
                "--full-doctor",
                "--inrou-canary-dir",
                str(workspace),
            ),
            run=runtime.run,
            request=runtime.request,
        )
        stages = [
            command
            for command in runtime.commands
            if "inrou-stage" in command and "--stage-dir" in command
        ]
        canaries = [
            command
            for command in runtime.commands
            if "inrou-canary" in command and "--stage-dir" in command
        ]
        ingests = [command for command in runtime.commands if "ingest" in command]
        doctor = [
            command
            for command in runtime.commands
            if "doctor" in command and "--public-root" in command
        ]
        self.assertTrue(report["inrou_canary"])
        self.assertIsNotNone(report["inrou_stage"])
        self.assertEqual(len(stages), 1)
        self.assertEqual(len(canaries), 1)
        self.assertEqual(len(ingests), module.PEER_COUNT * 2)
        self.assertEqual(len(doctor), 1)
        self.assertTrue(
            all(
                f"--max-capacity-bytes={module.TAIRA_SORAFS_MAX_CAPACITY_BYTES}"
                in command
                for command in ingests
            )
        )
        self.assertEqual(
            {
                next(value for value in command if value.startswith("--data-dir="))
                for command in ingests
            },
            {
                "--data-dir="
                + str(
                    (
                        self.root
                        / "state"
                        / "network"
                        / "state"
                        / f"peer{index}"
                        / "sorafs"
                    ).resolve()
                )
                for index in range(module.PEER_COUNT)
            },
        )
        stage = stages[0]
        canary = canaries[0]
        self.assertEqual(
            stage[stage.index("--container") + 1],
            str((workspace / module.INROU_CANARY_CONTAINER_FILE).resolve()),
        )
        self.assertEqual(
            stage[stage.index("--service") + 1],
            str((workspace / module.INROU_CANARY_SERVICE_FILE).resolve()),
        )
        self.assertEqual(
            stage[stage.index("--bundle-file") + 1],
            str((workspace / module.INROU_CANARY_BUNDLE_FILE).resolve()),
        )
        self.assertEqual(
            canary[canary.index("--stage-dir") + 1],
            stage[stage.index("--stage-dir") + 1],
        )
        self.assertIn("--fee-payer", canary)
        self.assertEqual(doctor[0][doctor[0].index("--public-root") + 1], "http://127.0.0.1:29080/")
        ping_index = next(
            index
            for index, command in enumerate(runtime.commands)
            if "ping" in command and "--no-wait" in command
        )
        status_index = next(
            index
            for index, command in enumerate(runtime.commands)
            if "status" in command and "--wait" in command
        )
        stage_index = runtime.commands.index(stage)
        ingest_indexes = [runtime.commands.index(command) for command in ingests]
        start_index = next(
            index
            for index, command in enumerate(runtime.commands)
            if command[0] == "/bin/bash" and command[1].endswith("start.sh")
        )
        canary_index = runtime.commands.index(canary)
        doctor_index = runtime.commands.index(doctor[0])
        self.assertLess(stage_index, min(ingest_indexes))
        self.assertLess(max(ingest_indexes), start_index)
        self.assertLess(ping_index, status_index)
        self.assertLess(status_index, canary_index)
        self.assertLess(canary_index, doctor_index)

    def test_full_doctor_without_inrou_workspace_fails_before_commands(self) -> None:
        runtime = FakeRuntime()

        with self.assertRaisesRegex(
            module.DevnetError, "--full-doctor requires --inrou-canary-dir"
        ):
            module.up(
                self.up_args("--full-doctor"),
                run=runtime.run,
                request=runtime.request,
            )

        self.assertEqual(runtime.commands, [])
        self.assertFalse((self.root / "state").exists())

    def test_inrou_workspace_requires_all_three_regular_files(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace()
        (workspace / module.INROU_CANARY_BUNDLE_FILE).unlink()

        with self.assertRaisesRegex(module.DevnetError, "missing regular file"):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=runtime.run,
                request=runtime.request,
            )

        self.assertEqual(runtime.commands, [])

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

    def test_build_command_selects_only_the_requested_toolchain(self) -> None:
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

        inrou_command = module.cargo_build_command(
            "local-release",
            Path("/tmp/taira-target"),
            include_inrou=True,
        )
        self.assertEqual(inrou_command.count("--bin"), 4)
        self.assertIn("sorafs_node", inrou_command)
        self.assertIn("sorafs-node", inrou_command)
        self.assertEqual(
            inrou_command[: len(command)],
            command,
        )

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
                "CARGO_INCREMENTAL": "1",
                "RUSTC_WRAPPER": "sccache",
                "TAIRA_TEST_ENV_RETAINED": "yes",
            },
        ):
            module.binary_paths(args, run)

        build_env = calls[0]["env"]
        self.assertIsInstance(build_env, dict)
        assert isinstance(build_env, dict)
        self.assertNotIn("CARGO_INCREMENTAL", build_env)
        self.assertNotIn("RUSTC_WRAPPER", build_env)
        self.assertEqual(build_env["TAIRA_TEST_ENV_RETAINED"], "yes")

    def test_compiled_surface_preflight_precedes_destructive_replacement(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        sentinel = target / "preserve-before-preflight"
        sentinel.write_text("live cohort\n", encoding="utf-8")
        runtime.help_options.remove("--terminal-status")
        stop_count = sum(
            command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
            for command in runtime.commands
        )

        with self.assertRaisesRegex(module.DevnetError, "compiled CLI surface"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(sentinel.read_text(encoding="utf-8"), "live cohort\n")
        self.assertEqual(
            sum(
                command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
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

    def test_command_timeout_is_reported_without_a_traceback(self) -> None:
        timeout = subprocess.TimeoutExpired(["cargo", "build"], 7)
        with mock.patch.object(module.subprocess, "run", side_effect=timeout):
            with self.assertRaisesRegex(module.DevnetError, "cargo timed out after 7s"):
                module.run_command(["cargo", "build"], timeout=7)

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
