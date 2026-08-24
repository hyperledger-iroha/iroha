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
import types
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
network_id_from_genesis_hash = sys.modules["taira_constants"].network_id_from_genesis_hash

REAL_REQUIRE_INROU_QUALIFICATION_HOST = module.require_inrou_qualification_host
REAL_REQUIRE_SAFE_CLEANUP_TARGET = module.require_safe_cleanup_target


def executable(path: Path, body: bytes = b"current binary\n") -> Path:
    """Create one non-empty executable used by the fake toolchain."""

    path.write_bytes(body)
    path.chmod(0o700)
    return path


class FakeRuntime:
    """Model the subprocess and HTTP surface consumed by the command."""

    def __init__(self) -> None:
        self.commands: list[tuple[str, ...]] = []
        self.git_branch = module.TAIRA_QUALIFICATION_BRANCH
        self.git_head = "f" * 40
        self.git_diff = ""
        self.git_untracked = ""
        self.validator_git_head = self.git_head
        self.client_git_head = self.git_head
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
            for _binary, _subcommands, options in module.CLI_SURFACES
            for option in options
        } | {
            option
            for _binary, _subcommands, options in module.INROU_CANARY_CLI_SURFACES
            for option in options
        } | {"--public-root", "--json"}
        self.sumeragi_status_http = 200
        self.restart_required_peer: int | None = None
        self.sumeragi_blocker_peer: int | None = None
        self.ping_stdout = json.dumps({"hash": "hash:" + "a" * 64 + "#ABCD"})
        self.status_stdout = json.dumps(
            {"hash": "a" * 64, "terminal_kind": "Applied"}
        )
        self.inrou_canary_stdout = json.dumps(
            {
                "command": "taira_inrou_canary",
                "status": "ok",
                "public_root": "http://127.0.0.1:29080",
                "checks": [
                    {
                        "name": "inrou_authoritative_status",
                        "http_status": 200,
                        "ok": True,
                        "detail": "active_adverts=4, hosted_replicas=4",
                    },
                    {
                        "name": "inrou_public_routes",
                        "http_status": 200,
                        "ok": True,
                        "detail": (
                            "observed deterministic identities for replica slots "
                            "1, 2, 3, and 4"
                        ),
                    },
                ],
                "warnings": [],
                "failures": [],
                "service_name": "taira_inrou_canary",
                "service_version": module.INROU_CANARY_DEPLOY_VERSION_V1,
                "mutation_mode": "deploy",
                "route_host": module.INROU_CANARY_ROUTE_HOST_V1,
                "route_path": module.INROU_CANARY_HEALTH_PATH_V1,
                "active_host_adverts": 4,
                "hosted_replica_count": 4,
                "bundle_hash": "a" * 64,
                "bundle_content_cid": "b" + "a" * 58,
                "bundle_manifest_digest_hex": "1" * 64,
                "guest_content_cid": "b" + "b" * 58,
                "guest_manifest_digest_hex": "2" * 64,
                "submitted_tx_hash": "hash:" + "C" * 64 + "#ABCD",
                "mutation_response_digest": "b" * 64,
                "replica_identities": [
                    {
                        "replica_slot": slot,
                        "identity": f"taira_inrou_canary:replica:{slot}",
                        "response_sha256": f"{slot:064x}",
                    }
                    for slot in range(1, module.PEER_COUNT + 1)
                ],
            }
        )

    def run(
        self,
        command: list[str] | tuple[str, ...],
        **kwargs: object,
    ) -> subprocess.CompletedProcess[str]:
        values = tuple(str(value) for value in command)
        self.commands.append(values)
        if values == ("git", "branch", "--show-current"):
            return subprocess.CompletedProcess(values, 0, self.git_branch + "\n", "")
        if values == ("git", "rev-parse", "HEAD"):
            return subprocess.CompletedProcess(values, 0, self.git_head + "\n", "")
        if values == (
            "git",
            "diff",
            "--binary",
            "--no-ext-diff",
            "HEAD",
            "--",
            ".",
        ):
            return subprocess.CompletedProcess(values, 0, self.git_diff, "")
        if values == (
            "git",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
        ):
            return subprocess.CompletedProcess(values, 0, self.git_untracked, "")
        if Path(values[0]).name == "rustc" and values[1:] == ("-vV",):
            return subprocess.CompletedProcess(
                values,
                0,
                (
                    "rustc 1.93.1 (test)\n"
                    "binary: rustc\n"
                    "commit-hash: " + "f" * 40 + "\n"
                    "host: aarch64-unknown-linux-gnu\n"
                    "release: 1.93.1\n"
                    "LLVM version: 21.1.0\n"
                ),
                "",
            )
        if Path(values[0]).name == "cargo_fast.sh":
            target_dir = Path(values[values.index("--target-dir") + 1])
            target_triple = values[values.index("--target") + 1]
            profile = values[values.index("--profile") + 1]
            bin_dir = target_dir / target_triple / profile
            bin_dir.mkdir(parents=True, exist_ok=True)
            for index, value in enumerate(values[:-1]):
                if value == "--bin":
                    executable(bin_dir / values[index + 1])
            return subprocess.CompletedProcess(values, 0, "", "")
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
            network_id = network_id_from_genesis_hash(genesis_hash)
            for index in range(module.PEER_COUNT):
                sorafs_dir = target / "state" / f"peer{index}" / "sorafs"
                runtime_dir = (
                    target / "state" / f"peer{index}" / "soracloud_runtime"
                )
                (target / f"peer{index}.toml").write_text(
                    f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                    f"chain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}\n"
                    '[genesis]\nexpected_hash_file = "genesis.expected_hash"\n'
                    f'address = "addr:127.0.0.1:{api_port + index}#ABCD"\n'
                    "[nexus.storage]\n"
                    f"local_budget_bytes = {module.GENERATED_LOCALNET_NEXUS_STORAGE_BYTES}\n"
                    "[sorafs.storage]\n"
                    "enabled = false\n"
                    f'data_dir = "{sorafs_dir}"\n'
                    "[soracloud_runtime]\n"
                    f'state_dir = "{runtime_dir}"\n'
                    "production_mode = true\n"
                    "[soracloud_runtime.egress]\n"
                    "default_allow = false\n"
                    "allowed_hosts = []\n"
                    f"rate_per_minute = {module.GENERATED_TAIRA_EGRESS_RATE_PER_MINUTE}\n"
                    f"max_bytes_per_minute = {module.GENERATED_TAIRA_EGRESS_MAX_BYTES_PER_MINUTE}\n",
                    encoding="utf-8",
                )
            signer_directory = target / module.RUNTIME_SIGNER_DIRECTORY
            signer_directory.mkdir(parents=True, mode=0o700)
            for index in range(module.PEER_COUNT):
                signer = signer_directory / f"peer{index}.private_key"
                signer.write_bytes(b"x" * module.RUNTIME_SIGNER_FILE_BYTES)
                signer.chmod(0o600)
            (target / "genesis.expected_hash").write_text(
                network_id + "\n", encoding="utf-8"
            )
            (target / "client.toml").write_text(
                f'chain = "{module.DEFAULT_CHAIN_ID}"\n'
                'network_id_file = "genesis.expected_hash"\n'
                f'torii_url = "http://127.0.0.1:{api_port}/"\n'
                f"[account]\nchain_discriminant = {module.DEFAULT_CHAIN_DISCRIMINANT}\n",
                encoding="utf-8",
            )
        elif "--check-config" in values:
            config = Path(values[values.index("--config") + 1])
            module.require_canonical_taira_profiles(config.parent)
        elif "inrou-stage" in values:
            stage = Path(values[values.index("--stage-dir") + 1])
            manifests = stage / "manifests"
            guest = stage / module.INROU_STAGE_GUEST_PAYLOAD / "aarch64"
            manifests.mkdir(parents=True, mode=0o700)
            guest.mkdir(parents=True, mode=0o700)
            for directory in (
                stage,
                manifests,
                stage / "payloads",
                stage / module.INROU_STAGE_GUEST_PAYLOAD,
                guest,
            ):
                directory.chmod(0o700)
            staged_files = {
                stage / module.INROU_STAGE_RECEIPT_FILE: b'{"schema_version":1}\n',
                stage / module.INROU_STAGE_CONTAINER_FILE: b"{}\n",
                stage / module.INROU_STAGE_SERVICE_FILE: b"{}\n",
                stage / module.INROU_STAGE_BUNDLE_PAYLOAD: b"bundle",
                stage / module.INROU_STAGE_BUNDLE_MANIFEST: b"bundle-manifest",
                stage / module.INROU_STAGE_GUEST_MANIFEST: b"guest-manifest",
                guest / "kernel": b"kernel",
            }
            for path, payload in staged_files.items():
                path.write_bytes(payload)
                path.chmod(0o600)
            return subprocess.CompletedProcess(
                values,
                0,
                json.dumps({"command": "taira_inrou_stage", "status": "ok"}),
                "",
            )
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
        elif "tools" in values and "version" in values:
            return subprocess.CompletedProcess(
                values,
                0,
                json.dumps(
                    {
                        "client_git_sha": self.client_git_head,
                        "client_version": "test",
                        "server_version": "test",
                    }
                ),
                "",
            )
        elif "status" in values:
            return subprocess.CompletedProcess(values, 0, self.status_stdout, "")
        elif "inrou-canary" in values:
            self.height += 1
            return subprocess.CompletedProcess(
                values, 0, self.inrou_canary_stdout, ""
            )
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
            if url.endswith("/status"):
                return 200, {
                    "build": {
                        "git_commit_sha": self.validator_git_head,
                        "target_triple": "aarch64-unknown-linux-gnu",
                    }
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
        self.root = Path(self.temporary.name).resolve()
        self.target_dir = self.root / "target"
        self.rust_target = "aarch64-unknown-linux-gnu"
        self.bin_dir = (
            self.target_dir / self.rust_target / module.TAIRA_BUILD_PROFILE
        )
        self.bin_dir.mkdir(parents=True)
        for name in ("kagami", "iroha3d_taira", "iroha", "sorafs-node"):
            executable(self.bin_dir / name)
        self.host_preflight = mock.patch.object(
            module, "require_inrou_qualification_host", return_value=None
        )
        self.host_preflight_mock = self.host_preflight.start()
        self.cleanup_preflight = mock.patch.object(
            module,
            "require_safe_cleanup_target",
            side_effect=lambda _root, target: (
                (target.stat().st_dev, target.stat().st_ino, 0)
                if target.exists()
                else None
            ),
        )
        self.cleanup_preflight.start()

    def tearDown(self) -> None:
        self.cleanup_preflight.stop()
        self.host_preflight.stop()
        self.temporary.cleanup()

    def test_first_release_taira_identity_is_exact(self) -> None:
        self.assertEqual(module.DEFAULT_DIR, Path("/var/lib/iroha-taira-devnet"))
        self.assertEqual(module.parser().parse_args(["check"]).dir, module.DEFAULT_DIR)
        self.assertEqual(
            module.DEFAULT_CHAIN_ID,
            "fc56984b-2be7-431d-840e-21514d1883f0",
        )
        self.assertEqual(module.DEFAULT_CHAIN_DISCRIMINANT, 369)
        self.assertEqual(
            [module.taira_inrou_identity(index) for index in range(module.PEER_COUNT)],
            [
                ("iroha-inrou-0", 70_000, 70_000),
                ("iroha-inrou-1", 70_001, 70_001),
                ("iroha-inrou-2", 70_002, 70_002),
                ("iroha-inrou-3", 70_003, 70_003),
            ],
        )

    def test_run_command_converts_spawn_oserror_to_devnet_error(self) -> None:
        with mock.patch.object(
            module.subprocess,
            "run",
            side_effect=OSError("spawn denied"),
        ):
            with self.assertRaisesRegex(
                module.DevnetError,
                "cannot execute missing-tool: spawn denied",
            ):
                module.run_command(["missing-tool"])

    def test_inrou_host_preflight_requires_linux_aarch64_root_and_kvm(self) -> None:
        REAL_REQUIRE_INROU_QUALIFICATION_HOST(
            system="Linux",
            machine="aarch64",
            effective_uid=0,
            kvm_probe=lambda path: (
                self.assertEqual(path, Path("/dev/kvm"))
                or module.LINUX_KVM_API_VERSION
            ),
            identity_probe=lambda: None,
        )
        cases = (
            ("Darwin", "arm64", 0, module.LINUX_KVM_API_VERSION, "requires Linux"),
            ("Linux", "x86_64", 0, module.LINUX_KVM_API_VERSION, "Linux AArch64"),
            ("Linux", "aarch64", 501, module.LINUX_KVM_API_VERSION, "uid 0"),
            ("Linux", "aarch64", 0, module.LINUX_KVM_API_VERSION - 1, "API version"),
        )
        for system, machine, effective_uid, api_version, error in cases:
            with self.subTest(error=error):
                with self.assertRaisesRegex(module.DevnetError, error):
                    REAL_REQUIRE_INROU_QUALIFICATION_HOST(
                        system=system,
                        machine=machine,
                        effective_uid=effective_uid,
                        kvm_probe=lambda _path, version=api_version: version,
                        identity_probe=lambda: None,
                    )
        with self.assertRaisesRegex(module.DevnetError, "cannot use /dev/kvm"):
            REAL_REQUIRE_INROU_QUALIFICATION_HOST(
                system="Linux",
                machine="aarch64",
                effective_uid=0,
                kvm_probe=lambda _path: (_ for _ in ()).throw(PermissionError("denied")),
                identity_probe=lambda: None,
            )

    def test_inrou_host_preflight_requires_exact_local_nss_identities(self) -> None:
        users = {
            f"iroha-inrou-{slot}": types.SimpleNamespace(
                pw_name=f"iroha-inrou-{slot}",
                pw_uid=70_000 + slot,
                pw_gid=70_000 + slot,
                pw_dir="/nonexistent",
                pw_shell="/usr/sbin/nologin",
            )
            for slot in range(module.PEER_COUNT)
        }
        groups = {
            f"iroha-inrou-{slot}": types.SimpleNamespace(
                gr_name=f"iroha-inrou-{slot}",
                gr_gid=70_000 + slot,
                gr_mem=[],
            )
            for slot in range(module.PEER_COUNT)
        }

        def user_by_id(identifier: int):
            return users[f"iroha-inrou-{identifier - 70_000}"]

        def group_by_id(identifier: int):
            return groups[f"iroha-inrou-{identifier - 70_000}"]

        with (
            mock.patch.object(module.pwd, "getpwnam", side_effect=users.__getitem__),
            mock.patch.object(module.pwd, "getpwuid", side_effect=user_by_id),
            mock.patch.object(module.grp, "getgrnam", side_effect=groups.__getitem__),
            mock.patch.object(module.grp, "getgrgid", side_effect=group_by_id),
            mock.patch.object(module.grp, "getgrall", return_value=list(groups.values())),
        ):
            module.require_canonical_inrou_nss_identities()

            users["iroha-inrou-2"] = types.SimpleNamespace(
                pw_name="iroha-inrou-2",
                pw_uid=70_002,
                pw_gid=70_002,
                pw_dir="/home/legacy-inrou",
                pw_shell="/bin/bash",
            )
            with self.assertRaisesRegex(module.DevnetError, "home /nonexistent"):
                module.require_canonical_inrou_nss_identities()

        kvm_probes: list[Path] = []
        with self.assertRaisesRegex(module.DevnetError, "NSS identity drift"):
            REAL_REQUIRE_INROU_QUALIFICATION_HOST(
                system="Linux",
                machine="aarch64",
                effective_uid=0,
                identity_probe=lambda: (_ for _ in ()).throw(
                    module.DevnetError("NSS identity drift")
                ),
                kvm_probe=lambda path: kvm_probes.append(path) or 12,
            )
        self.assertEqual(kvm_probes, [])

    def test_nonqualified_host_is_rejected_before_bundle_or_build(self) -> None:
        runtime = FakeRuntime()
        self.host_preflight_mock.side_effect = module.DevnetError(
            "Taira Inrou V1 qualification requires Linux AArch64"
        )

        with self.assertRaisesRegex(module.DevnetError, "Linux AArch64"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(runtime.commands, [])
        self.assertFalse((self.root / "state").exists())

    def test_up_rejects_target_dir_equal_to_or_inside_network_before_work(
        self,
    ) -> None:
        for case, relative_target in (
            ("equal", Path(".")),
            ("inside", Path("nested-target")),
        ):
            with self.subTest(case=case):
                state = module.managed_root(self.root / f"overlap-{case}", create=True)
                network = state / "network"
                network.mkdir(mode=0o700)
                sentinel = network / "preserve"
                sentinel.write_bytes(b"running cohort\n")
                target_dir = network / relative_target
                bin_dir = (
                    target_dir / self.rust_target / module.TAIRA_BUILD_PROFILE
                )
                bin_dir.mkdir(parents=True, mode=0o700)
                binaries = [
                    executable(bin_dir / name, b"sentinel binary\n")
                    for name in ("kagami", "iroha3d_taira", "iroha")
                ]
                args = self.up_args()
                args.dir = state
                args.target_dir = target_dir
                runtime = FakeRuntime()

                with self.assertRaisesRegex(module.DevnetError, "must not overlap"):
                    module.up(args, run=runtime.run, request=runtime.request)

                self.assertEqual(sentinel.read_bytes(), b"running cohort\n")
                self.assertTrue(
                    all(
                        path.read_bytes() == b"sentinel binary\n"
                        for path in binaries
                    )
                )
                self.assertEqual(runtime.commands, [])

    def test_up_rejects_managed_root_as_target_dir_before_work(self) -> None:
        state = module.managed_root(self.root / "root-overlap", create=True)
        network = state / "network"
        network.mkdir(mode=0o700)
        sentinel = network / "preserve"
        sentinel.write_bytes(b"running cohort\n")
        bin_dir = state / self.rust_target / module.TAIRA_BUILD_PROFILE
        bin_dir.mkdir(parents=True, mode=0o700)
        binaries = [
            executable(bin_dir / name, b"sentinel binary\n")
            for name in ("kagami", "iroha3d_taira", "iroha")
        ]
        args = self.up_args()
        args.dir = state
        args.target_dir = state
        runtime = FakeRuntime()

        with self.assertRaisesRegex(module.DevnetError, "must not overlap"):
            module.up(args, run=runtime.run, request=runtime.request)

        self.assertEqual(sentinel.read_bytes(), b"running cohort\n")
        self.assertTrue(
            all(path.read_bytes() == b"sentinel binary\n" for path in binaries)
        )
        self.assertEqual(runtime.commands, [])

    def test_up_requires_optimizations_branch_before_build(self) -> None:
        runtime = FakeRuntime()
        runtime.git_branch = "main"

        with self.assertRaisesRegex(module.DevnetError, "requires branch `optimizations`"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertFalse(
            any(Path(command[0]).name == "cargo_fast.sh" for command in runtime.commands)
        )
        self.assertFalse((self.root / "state" / "network").exists())

    def test_up_rejects_source_change_during_build_before_reset(self) -> None:
        runtime = FakeRuntime()
        diff_command = (
            "git",
            "diff",
            "--binary",
            "--no-ext-diff",
            "HEAD",
            "--",
            ".",
        )
        diff_calls = 0

        def run(command, **kwargs):
            nonlocal diff_calls
            if tuple(str(value) for value in command) == diff_command:
                diff_calls += 1
                if diff_calls == 2:
                    runtime.git_diff = "changed during build"
            return runtime.run(command, **kwargs)

        with self.assertRaisesRegex(module.DevnetError, "changed while building"):
            module.up(self.up_args(), run=run, request=runtime.request)

        self.assertFalse((self.root / "state" / "network").exists())

    def test_source_observation_frames_each_untracked_record_unambiguously(self) -> None:
        runtime = FakeRuntime()
        first = self.root / "a"
        second = self.root / "b"
        first.write_bytes(b"prefix")
        second.write_bytes(b"suffix")
        first.chmod(0o600)
        second.chmod(0o600)
        mode = str(first.stat().st_mode & 0o777).encode("ascii")
        embedded_second_record = (
            b"\0untracked\0b\0" + mode + b"\0file\0" + second.read_bytes()
        )

        with mock.patch.object(module, "REPO_ROOT", self.root):
            first.write_bytes(b"prefix" + embedded_second_record)
            first.chmod(0o600)
            runtime.git_untracked = "a\0"
            one_file_digest = module.current_source_observation(runtime.run)[
                "observed_nonignored_worktree_sha256"
            ]

            first.write_bytes(b"prefix")
            first.chmod(0o600)
            runtime.git_untracked = "a\0b\0"
            two_file_digest = module.current_source_observation(runtime.run)[
                "observed_nonignored_worktree_sha256"
            ]

        self.assertNotEqual(one_file_digest, two_file_digest)

    def test_untracked_source_hash_rejects_atomic_path_replacement(self) -> None:
        source = self.root / "source"
        replacement = self.root / "replacement"
        source.write_bytes(b"source-bytes")
        replacement.write_bytes(b"other-bytes!")
        source.chmod(0o600)
        replacement.chmod(0o600)
        metadata = source.lstat()
        real_fstat = module.os.fstat
        fstat_calls = 0

        def replace_after_open(descriptor: int):
            nonlocal fstat_calls
            fstat_calls += 1
            if fstat_calls == 1:
                replacement.replace(source)
            return real_fstat(descriptor)

        with mock.patch.object(module.os, "fstat", side_effect=replace_after_open):
            with self.assertRaises(module.DevnetError):
                module._untracked_source_content(source, metadata)

    def test_source_observation_converts_vanished_untracked_path_to_devnet_error(
        self,
    ) -> None:
        runtime = FakeRuntime()
        runtime.git_untracked = "vanished\0"

        with mock.patch.object(module, "REPO_ROOT", self.root):
            with self.assertRaises(module.DevnetError):
                module.current_source_observation(runtime.run)

    def test_source_observation_supports_non_utf8_git_paths_stably(self) -> None:
        runtime = FakeRuntime()
        raw_name = b"untracked-\xff"
        relative = os.fsdecode(raw_name)
        self.assertEqual(relative.encode("utf-8", errors="surrogateescape"), raw_name)
        backing = self.root / "surrogate-path-backing"
        backing.write_bytes(b"non-UTF-8 path contents\n")
        backing.chmod(0o600)
        metadata = backing.lstat()
        real_lstat = type(self.root).lstat

        def lstat_surrogate(path):
            if relative in str(path):
                return metadata
            return real_lstat(path)

        runtime.git_untracked = relative + "\0"

        with (
            mock.patch.object(module, "REPO_ROOT", self.root),
            mock.patch.object(
                type(self.root),
                "lstat",
                autospec=True,
                side_effect=lstat_surrogate,
            ),
            mock.patch.object(
                module,
                "_untracked_source_content",
                return_value=(b"file", b"\x9a" * 32),
            ) as content,
        ):
            first = module.current_source_observation(runtime.run)
            second = module.current_source_observation(runtime.run)

        self.assertEqual(first, second)
        self.assertRegex(
            first["observed_nonignored_worktree_sha256"], r"^[0-9a-f]{64}$"
        )
        self.assertEqual(first["cargo_source_consumption"], "not_proven")
        self.assertEqual(content.call_count, 2)
        for call in content.call_args_list:
            self.assertIn(relative, str(call.args[0]))

    def test_up_rejects_live_validator_build_identity_drift(self) -> None:
        runtime = FakeRuntime()
        runtime.validator_git_head = "e" * 40

        with self.assertRaisesRegex(module.DevnetError, "validator build identity"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertFalse(runtime.process_commands)

    def test_up_rejects_live_cli_build_identity_drift(self) -> None:
        runtime = FakeRuntime()
        runtime.client_git_head = "e" * 40

        with self.assertRaisesRegex(module.DevnetError, "CLI build identity"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertFalse(runtime.process_commands)

    def test_up_rejects_toolchain_change_during_qualification(self) -> None:
        runtime = FakeRuntime()

        def run(command, **kwargs):
            completed = runtime.run(command, **kwargs)
            values = tuple(str(value) for value in command)
            if "tools" in values and "version" in values:
                executable(self.bin_dir / "iroha", b"changed binary\n")
            return completed

        with self.assertRaisesRegex(module.DevnetError, "toolchain changed"):
            module.up(self.up_args(), run=run, request=runtime.request)

        self.assertFalse(runtime.process_commands)

    def test_up_cleans_cohort_when_final_toolchain_binary_disappears(self) -> None:
        runtime = FakeRuntime()
        real_fstat = module.os.fstat
        fstat_calls = 0

        def disappear_after_final_read(descriptor: int):
            nonlocal fstat_calls
            result = real_fstat(descriptor)
            fstat_calls += 1
            if fstat_calls == 8:
                (self.bin_dir / "kagami").unlink()
            return result

        with mock.patch.object(
            module.os,
            "fstat",
            side_effect=disappear_after_final_read,
        ):
            with self.assertRaisesRegex(
                module.DevnetError,
                "cannot re-inspect qualifying executable",
            ):
                module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(fstat_calls, 8)
        self.assertFalse(runtime.process_commands)
        self.assertTrue(
            any(
                command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
                for command in runtime.commands
            )
        )

    def test_up_cleans_cohort_when_final_toolchain_read_fails(self) -> None:
        runtime = FakeRuntime()
        real_fdopen = module.os.fdopen
        fdopen_calls = 0

        class FailingReadStream:
            def __init__(self, stream) -> None:
                self.stream = stream

            def __enter__(self):
                self.stream.__enter__()
                return self

            def __exit__(self, *args):
                return self.stream.__exit__(*args)

            def read(self, _size: int = -1) -> bytes:
                raise OSError("injected binary read failure")

            def fileno(self) -> int:
                return self.stream.fileno()

        def fail_final_read(descriptor: int, *args, **kwargs):
            nonlocal fdopen_calls
            stream = real_fdopen(descriptor, *args, **kwargs)
            if not args or args[0] != "rb":
                return stream
            fdopen_calls += 1
            if fdopen_calls == 4:
                return FailingReadStream(stream)
            return stream

        with mock.patch.object(module.os, "fdopen", side_effect=fail_final_read):
            with self.assertRaisesRegex(
                module.DevnetError,
                "cannot hash qualifying executable",
            ):
                module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(fdopen_calls, 4)
        self.assertFalse(runtime.process_commands)
        self.assertTrue(
            any(
                command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
                for command in runtime.commands
            )
        )

    def test_canonical_network_id_rejects_pre_release_and_malformed_text(self) -> None:
        genesis_hash = "a" * 63 + "b"
        canonical = network_id_from_genesis_hash(genesis_hash)
        self.assertEqual(module.canonical_network_id(canonical), canonical)
        malformed = (
            genesis_hash,
            canonical.lower(),
            canonical[:-1] + ("0" if canonical[-1] != "0" else "1"),
            "hash:" + "0" * 63 + "2#F56D",
        )
        for value in malformed:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    module.canonical_network_id(value)
        with self.assertRaises(ValueError):
            network_id_from_genesis_hash("0" * 63 + "2")

    def up_args(self, *extra: str):
        """Parse one current-workspace ``up`` command for this test directory."""

        return module.parser().parse_args(
            [
                "--dir",
                str(self.root / "state"),
                "up",
                "--target-dir",
                str(self.target_dir),
                "--timeout-seconds",
                "1",
                *extra,
            ]
        )

    def inrou_canary_workspace(self, *, name: str = "inrou-canary") -> Path:
        """Create the owner-only fixed input surface consumed by the fake stager."""

        workspace = self.root / name
        workspace.mkdir(mode=0o700)
        workspace.chmod(0o700)
        guest = workspace / module.INROU_CANARY_GUEST_DIRECTORY
        guest.mkdir(parents=True, mode=0o700)
        (workspace / "inrou").chmod(0o700)
        guest.chmod(0o700)
        fixtures = {
            module.INROU_CANARY_CONTAINER_FILE: b"{}\n",
            module.INROU_CANARY_SERVICE_FILE: b"{}\n",
            module.INROU_CANARY_BUNDLE_FILE: b"bundle",
        }
        for name, payload in fixtures.items():
            path = workspace / name
            path.write_bytes(payload)
            path.chmod(0o600)
        for guest_name in module.INROU_CANARY_GUEST_FILES:
            path = guest / guest_name
            path.write_bytes(guest_name.encode("utf-8"))
            path.chmod(0o600)
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
        self.assertNotIn("inrou_backend", report)
        self.assertEqual(report["configured_inrou_vm_capacity_per_peer"], 1)
        self.assertEqual(report["inrou_startup_boundary_qualified_peers"], 4)
        self.assertNotIn("inrou_vm_capacity_per_peer", report)
        self.assertNotIn("inrou_qualified_peers", report)
        self.assertNotIn("inrou_configured_vm_capacity_per_peer", report)
        self.assertEqual(report["inrou_canary"], {"status": "not_requested"})
        self.assertEqual(
            report["inrou_guest_workload_qualification"], "not_requested"
        )
        self.assertIsNone(report["inrou_canary_input_content_sha256"])
        self.assertNotIn("source", report)
        source_observation = report["source_observation"]
        self.assertEqual(
            source_observation["branch"], module.TAIRA_QUALIFICATION_BRANCH
        )
        self.assertEqual(source_observation["git_head"], runtime.git_head)
        self.assertEqual(source_observation["target_triple"], self.rust_target)
        self.assertEqual(
            source_observation["observation_scope"],
            "git_head_tracked_diff_nonignored_untracked",
        )
        self.assertRegex(
            source_observation["observed_nonignored_worktree_sha256"],
            r"^[0-9a-f]{64}$",
        )
        self.assertEqual(
            source_observation["cargo_source_consumption"], "not_proven"
        )
        self.assertEqual(
            source_observation["stability_checks"],
            "matched_before_after_build_and_qualification",
        )
        self.assertEqual(
            set(report["toolchain"]), {"kagami", "iroha3d_taira", "iroha"}
        )
        for evidence in report["toolchain"].values():
            self.assertEqual(evidence["bytes"], len(b"current binary\n"))
            self.assertRegex(evidence["sha256"], r"^[0-9a-f]{64}$")
        self.host_preflight_mock.assert_called_once_with()
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
        identities: set[tuple[int, int]] = set()
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
            identity_name, uid, gid = module.taira_inrou_identity(index)
            self.assertEqual(identity_name, f"iroha-inrou-{index}")
            self.assertEqual(
                module.section_assignment(config, "soracloud_runtime.inrou", "enabled"),
                "true",
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "portable_vm_uid"
                ),
                str(uid),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "portable_vm_gid"
                ),
                str(gid),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "max_cpu_millis"
                ),
                str(module.TAIRA_INROU_MAX_CPU_MILLIS),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "max_memory_bytes"
                ),
                str(module.TAIRA_INROU_MAX_MEMORY_BYTES),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "max_storage_bytes"
                ),
                str(module.TAIRA_INROU_MAX_STORAGE_BYTES),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "start_grace_ms"
                ),
                str(module.TAIRA_INROU_START_GRACE_MS),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.inrou", "stop_grace_ms"
                ),
                str(module.TAIRA_INROU_STOP_GRACE_MS),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.egress", "rate_per_minute"
                ),
                str(module.TAIRA_INROU_EGRESS_RATE_PER_MINUTE),
            )
            self.assertEqual(
                module.section_assignment(
                    config, "soracloud_runtime.egress", "max_bytes_per_minute"
                ),
                str(module.TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE),
            )
            contents = config.read_text(encoding="utf-8")
            self.assertNotIn("backends =", contents)
            self.assertNotIn("max_concurrent_vms =", contents)
            identities.add((uid, gid))
        self.assertEqual(len(identities), module.PEER_COUNT)
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

    def test_default_up_preflights_only_shipping_surfaces(self) -> None:
        runtime = FakeRuntime()
        (self.bin_dir / "sorafs-node").unlink()

        report = module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(report["inrou_canary"], {"status": "not_requested"})
        self.assertEqual(
            report["inrou_guest_workload_qualification"], "not_requested"
        )
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
                    module.apply_canonical_taira_profiles(target)

                self.assertEqual(peer0.read_text(encoding="utf-8"), peer0_before)

    def test_profile_overlay_rejects_retained_inrou_table_before_rewriting(self) -> None:
        _, target = self.generated_network("generated-retained-inrou")
        peer0 = target / "peer0.toml"
        peer3 = target / "peer3.toml"
        peer0_before = peer0.read_text(encoding="utf-8")
        peer3.write_text(
            peer3.read_text(encoding="utf-8")
            + "\n[soracloud_runtime.inrou]\nenabled = false\n",
            encoding="utf-8",
        )

        with self.assertRaisesRegex(module.DevnetError, "retained an Inrou selector"):
            module.apply_canonical_taira_profiles(target)

        self.assertEqual(peer0.read_text(encoding="utf-8"), peer0_before)

    def test_canonical_profile_rejects_identity_and_selector_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        config = self.root / "state" / "network" / "peer2.toml"
        original = config.read_text(encoding="utf-8")
        config.write_text(
            original.replace("portable_vm_uid = 70002", "portable_vm_uid = 70001", 1),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(module.DevnetError, "wrong PortableVM V1 profile"):
            module.require_canonical_taira_profiles(config.parent)
        config.write_text(
            original.replace("start_grace_ms = 30000", "start_grace_ms = 99", 1),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(module.DevnetError, "wrong PortableVM V1 profile"):
            module.require_canonical_taira_profiles(config.parent)
        config.write_text(
            original.replace("enabled = true", "enabled = true\nbackends = [\"portable_vm\"]", 1),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(module.DevnetError, "wrong assignment set"):
            module.require_canonical_taira_profiles(config.parent)

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
                "--target-dir",
                str(self.target_dir),
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

        with self.assertRaises(module.DevnetError) as raised:
            module.up(self.up_args(), run=runtime.run, request=interrupt)

        message = str(raised.exception)
        self.assertIn("startup was interrupted", message)
        self.assertIn("teardown was attempted", message)
        self.assertNotIn("cohort stopped", message)
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
        self.assertEqual(report["configured_inrou_vm_capacity_per_peer"], 1)
        self.assertEqual(report["configured_peers"], module.PEER_COUNT)
        self.assertFalse(any("qualified" in key for key in report))
        self.assertNotIn("inrou_vm_capacity_per_peer", report)
        self.assertNotIn("inrou_qualified_peers", report)
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

    def test_failing_operator_status_is_not_swallowed(self) -> None:
        runtime = FakeRuntime()
        runtime.sumeragi_status_http = 503

        with self.assertRaisesRegex(module.DevnetError, "status route failed"):
            module.up(self.up_args(), run=runtime.run, request=runtime.request)

        self.assertEqual(runtime.process_commands, {})

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

    def test_bundle_identity_rejects_raw_crc_and_record_framing_drift(self) -> None:
        _, target = self.generated_network("identity-record-cases")
        identity_path = target / "genesis.expected_hash"
        canonical = identity_path.read_text(encoding="utf-8").removesuffix("\n")
        cases = (
            ("raw", "a" * 63 + "b" + "\n", "is invalid"),
            (
                "bad checksum",
                canonical[:-1] + ("0" if canonical[-1] != "0" else "1") + "\n",
                "is invalid",
            ),
            ("missing newline", canonical, "lacks a final newline"),
            ("multiple records", f"{canonical}\n{canonical}\n", "exactly one record"),
        )
        for label, record, message in cases:
            with self.subTest(label=label):
                identity_path.write_text(record, encoding="utf-8")
                with self.assertRaisesRegex(module.DevnetError, message):
                    module.require_bundle_identity(
                        target, module.torii_roots(module.DEFAULT_API_PORT)
                    )

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

    def test_check_rejects_client_network_identity_file_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        client = self.root / "state" / "network" / "client.toml"
        contents = client.read_text(encoding="utf-8")
        client.write_text(
            contents.replace("genesis.expected_hash", "foreign.expected_hash"),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "network identity file does not match"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_duplicate_inline_client_network_identity(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        client = self.root / "state" / "network" / "client.toml"
        network_id = (self.root / "state" / "network" / "genesis.expected_hash").read_text(
            encoding="utf-8"
        ).removesuffix("\n")
        contents = client.read_text(encoding="utf-8")
        client.write_text(
            contents.replace(
                "[account]\n", f'network_id = "{network_id}"\n[account]\n', 1
            ),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "must not contain `network_id`"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_peer_genesis_identity_file_drift(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        config = self.root / "state" / "network" / "peer2.toml"
        contents = config.read_text(encoding="utf-8")
        config.write_text(
            contents.replace("genesis.expected_hash", "foreign.expected_hash"),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "genesis identity file does not match"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_check_rejects_duplicate_inline_peer_genesis_identity(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        config = target / "peer2.toml"
        network_id = (target / "genesis.expected_hash").read_text(
            encoding="utf-8"
        ).removesuffix("\n")
        contents = config.read_text(encoding="utf-8")
        config.write_text(
            contents.replace(
                "[genesis]\n", f'[genesis]\nexpected_hash = "{network_id}"\n', 1
            ),
            encoding="utf-8",
        )
        args = module.parser().parse_args(
            ["--dir", str(self.root / "state"), "check", "--timeout-seconds", "1"]
        )

        with self.assertRaisesRegex(module.DevnetError, "must not contain `expected_hash`"):
            module.check(args, run=runtime.run, request=runtime.request)

    def test_full_public_doctor_is_opt_in(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace()
        report = module.up(
            self.up_args(
                "--inrou-canary-dir",
                str(workspace),
                "--full-doctor",
            ),
            run=runtime.run,
            request=runtime.request,
        )
        stages = [
            command
            for command in runtime.commands
            if "inrou-stage" in command and "--help" not in command
        ]
        canaries = [
            command
            for command in runtime.commands
            if "inrou-canary" in command and "--help" not in command
        ]
        ingests = [command for command in runtime.commands if "ingest" in command]
        doctor = [
            command
            for command in runtime.commands
            if "doctor" in command and "--public-root" in command and "--help" not in command
        ]
        self.assertEqual(report["inrou_canary"], json.loads(runtime.inrou_canary_stdout))
        self.assertEqual(report["inrou_guest_workload_qualification"], "verified")
        self.assertEqual(report["final_height"], 3)
        self.assertNotIn("inrou_stage", report)
        self.assertEqual(len(stages), 1)
        self.assertEqual(len(canaries), 1)
        self.assertEqual(len(ingests), module.PEER_COUNT * 2)
        self.assertEqual(len(doctor), 1)
        stage = stages[0]
        canary = canaries[0]
        snapshot = (
            self.root
            / "state"
            / "network"
            / module.INROU_CANARY_INPUT_SNAPSHOT_DIRECTORY
        )
        self.assertEqual(stage[stage.index("--mode") + 1], "deploy")
        self.assertEqual(canary[canary.index("--mode") + 1], "deploy")
        self.assertEqual(
            stage[stage.index("--container") + 1],
            str(snapshot / module.INROU_CANARY_CONTAINER_FILE),
        )
        self.assertEqual(
            stage[stage.index("--service") + 1],
            str(snapshot / module.INROU_CANARY_SERVICE_FILE),
        )
        self.assertEqual(
            stage[stage.index("--bundle-file") + 1],
            str(snapshot / module.INROU_CANARY_BUNDLE_FILE),
        )
        self.assertRegex(
            report["inrou_canary_input_content_sha256"], r"^[0-9a-f]{64}$"
        )
        self.assertEqual(
            canary[canary.index("--stage-dir") + 1],
            stage[stage.index("--stage-dir") + 1],
        )
        self.assertIn("--fee-payer", canary)
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
        self.assertEqual(
            doctor[0][doctor[0].index("--public-root") + 1],
            "http://127.0.0.1:29080",
        )
        stage_index = runtime.commands.index(stage)
        ingest_indexes = [runtime.commands.index(command) for command in ingests]
        start_index = next(
            index
            for index, command in enumerate(runtime.commands)
            if command[0] == "/bin/bash" and command[1].endswith("start.sh")
        )
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
        canary_index = runtime.commands.index(canary)
        doctor_index = runtime.commands.index(doctor[0])
        self.assertLess(stage_index, min(ingest_indexes))
        self.assertLess(max(ingest_indexes), start_index)
        self.assertLess(start_index, ping_index)
        self.assertLess(ping_index, status_index)
        self.assertLess(status_index, canary_index)
        self.assertLess(canary_index, doctor_index)

    def test_inrou_canary_does_not_enable_full_doctor(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace()

        report = module.up(
            self.up_args("--inrou-canary-dir", str(workspace)),
            run=runtime.run,
            request=runtime.request,
        )

        self.assertEqual(report["inrou_canary"]["status"], "ok")
        self.assertEqual(report["inrou_guest_workload_qualification"], "verified")
        self.assertRegex(
            report["inrou_canary"]["submitted_tx_hash"],
            r"^hash:[0-9A-F]{64}#[0-9A-F]{4}$",
        )
        self.assertEqual(
            len(report["inrou_canary"]["replica_identities"]),
            module.PEER_COUNT,
        )
        self.assertEqual(
            set(report["toolchain"]),
            {"kagami", "iroha3d_taira", "iroha", "sorafs-node"},
        )
        self.assertFalse(
            any("doctor" in command and "--help" not in command for command in runtime.commands)
        )

    def test_full_doctor_without_canary_remains_independent(self) -> None:
        runtime = FakeRuntime()

        report = module.up(
            self.up_args("--full-doctor"),
            run=runtime.run,
            request=runtime.request,
        )

        self.assertEqual(report["inrou_canary"], {"status": "not_requested"})
        self.assertEqual(
            report["inrou_guest_workload_qualification"], "not_requested"
        )
        self.assertFalse(any("inrou-stage" in command for command in runtime.commands))
        self.assertFalse(any("inrou-canary" in command for command in runtime.commands))
        doctors = [
            command
            for command in runtime.commands
            if "doctor" in command and "--help" not in command
        ]
        self.assertEqual(len(doctors), 1)

    def test_inrou_workspace_rejects_missing_or_permissive_inputs_before_mutation(self) -> None:
        cases = (
            "missing",
            "missing-guest",
            "permissive-file",
            "permissive-directory",
        )
        for case in cases:
            with self.subTest(case=case):
                runtime = FakeRuntime()
                workspace = self.inrou_canary_workspace(name=f"inrou-{case}")
                if case == "missing":
                    (workspace / module.INROU_CANARY_BUNDLE_FILE).unlink()
                elif case == "missing-guest":
                    (
                        workspace
                        / module.INROU_CANARY_GUEST_DIRECTORY
                        / module.INROU_CANARY_GUEST_FILES[-1]
                    ).unlink()
                elif case == "permissive-file":
                    (workspace / module.INROU_CANARY_BUNDLE_FILE).chmod(0o640)
                else:
                    workspace.chmod(0o750)

                with self.assertRaises(module.DevnetError):
                    module.up(
                        self.up_args("--inrou-canary-dir", str(workspace)),
                        run=runtime.run,
                        request=runtime.request,
                    )

                self.assertEqual(runtime.commands, [])
                self.assertFalse((self.root / "state").exists())

        workspace = self.inrou_canary_workspace(name="inrou-foreign-owner")
        runtime = FakeRuntime()
        with mock.patch.object(module.os, "geteuid", return_value=os.geteuid() + 1):
            with self.assertRaisesRegex(module.DevnetError, "owned by root or uid"):
                module.up(
                    self.up_args("--inrou-canary-dir", str(workspace)),
                    run=runtime.run,
                    request=runtime.request,
                )
        self.assertEqual(runtime.commands, [])
        self.assertFalse((self.root / "state").exists())

    def test_inrou_workspace_rejects_symlink_and_devnet_overlap(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace(name="inrou-symlink")
        bundle = workspace / module.INROU_CANARY_BUNDLE_FILE
        real_bundle = self.root / "real-bundle"
        bundle.rename(real_bundle)
        bundle.symlink_to(real_bundle)

        with self.assertRaisesRegex(module.DevnetError, "direct regular file"):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=runtime.run,
                request=runtime.request,
            )
        self.assertEqual(runtime.commands, [])

        state = self.root / "state"
        state.mkdir(mode=0o700)
        state.chmod(0o700)
        nested = self.inrou_canary_workspace(name="state/inrou-nested")
        with self.assertRaisesRegex(module.DevnetError, "must be disjoint"):
            module.up(
                self.up_args("--inrou-canary-dir", str(nested)),
                run=runtime.run,
                request=runtime.request,
            )
        self.assertEqual(runtime.commands, [])
        self.assertFalse((state / module.MARKER).exists())

    def test_inrou_workspace_rejects_writable_ancestor_and_target_or_repo_overlap(
        self,
    ) -> None:
        writable_parent = self.root / "writable-canary-parent"
        writable_parent.mkdir(mode=0o700)
        workspace = self.inrou_canary_workspace(
            name="writable-canary-parent/workspace"
        )
        writable_parent.chmod(0o777)
        runtime = FakeRuntime()

        with self.assertRaisesRegex(module.DevnetError, "non-writable by group/other"):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=runtime.run,
                request=runtime.request,
            )
        self.assertEqual(runtime.commands, [])

        writable_parent.chmod(0o700)
        for label, target_dir, repo_root in (
            ("qualification target", workspace / "cargo-target", module.REPO_ROOT),
            ("repository", self.target_dir, workspace.parent),
        ):
            with self.subTest(label=label):
                args = self.up_args("--inrou-canary-dir", str(workspace))
                args.target_dir = target_dir
                runtime = FakeRuntime()
                with mock.patch.object(module, "REPO_ROOT", repo_root):
                    with self.assertRaisesRegex(module.DevnetError, "must be disjoint"):
                        module.up(args, run=runtime.run, request=runtime.request)
                self.assertEqual(runtime.commands, [])

    def test_inrou_workspace_path_swap_during_build_preserves_existing_cohort(
        self,
    ) -> None:
        state = module.managed_root(self.root / "state", create=True)
        existing = state / "network"
        existing.mkdir(mode=0o700)
        sentinel = existing / "preserve"
        sentinel.write_bytes(b"existing cohort\n")
        workspace = self.inrou_canary_workspace()
        bundle = workspace / module.INROU_CANARY_BUNDLE_FILE
        replacement = self.root / "replacement-bundle"
        replacement.write_bytes(b"forged")
        replacement.chmod(0o600)
        runtime = FakeRuntime()
        swapped = False

        def run(command, **kwargs):
            nonlocal swapped
            completed = runtime.run(command, **kwargs)
            if Path(str(command[0])).name == "cargo_fast.sh" and not swapped:
                replacement.replace(bundle)
                swapped = True
            return completed

        with self.assertRaisesRegex(
            module.DevnetError,
            "workspace changed before the disposable cohort was replaced",
        ):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=run,
                request=runtime.request,
            )

        self.assertTrue(swapped)
        self.assertEqual(sentinel.read_bytes(), b"existing cohort\n")
        self.assertFalse(
            any(
                "localnet" in command and "--out-dir" in command
                for command in runtime.commands
            )
        )

    def test_inrou_snapshot_rejects_path_swap_after_workspace_observation(self) -> None:
        workspace = self.inrou_canary_workspace(name="snapshot-swap-workspace")
        observed = module.require_inrou_canary_workspace(workspace)
        bundle = workspace / module.INROU_CANARY_BUNDLE_FILE
        replacement = self.root / "snapshot-swap-replacement"
        replacement.write_bytes(bundle.read_bytes())
        replacement.chmod(0o600)
        replacement.replace(bundle)
        target = self.root / "snapshot-target"
        target.mkdir(mode=0o700)

        with self.assertRaisesRegex(module.DevnetError, "changed identity before staging"):
            module.snapshot_inrou_canary_workspace(target, observed)

    def test_inrou_canary_rejects_noncanonical_compiled_receipt_and_stops(self) -> None:
        runtime = FakeRuntime()
        workspace = self.inrou_canary_workspace()
        receipt = json.loads(runtime.inrou_canary_stdout)
        receipt["replica_identities"][3]["replica_slot"] = 3
        runtime.inrou_canary_stdout = json.dumps(receipt)

        with self.assertRaisesRegex(module.DevnetError, "non-canonical replica identity"):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=runtime.run,
                request=runtime.request,
            )

        canary_index = next(
            index
            for index, command in enumerate(runtime.commands)
            if "inrou-canary" in command and "--help" not in command
        )
        stop_index = max(
            index
            for index, command in enumerate(runtime.commands)
            if command[0] == "/bin/bash" and command[1].endswith("stop.sh")
        )
        self.assertLess(canary_index, stop_index)

    def test_inrou_canary_receipt_rejects_status_or_route_drift(self) -> None:
        baseline = json.loads(FakeRuntime().inrou_canary_stdout)
        cases = (
            (
                "host-count",
                lambda receipt: receipt.__setitem__("active_host_adverts", 3),
                "active_host_adverts=4",
            ),
            (
                "missing-route-check",
                lambda receipt: receipt["checks"].pop(),
                "malformed checks",
            ),
            (
                "missing-submitted-mutation",
                lambda receipt: receipt.__setitem__("submitted_tx_hash", None),
                "malformed submitted_tx_hash",
            ),
            (
                "lowercase-submitted-mutation",
                lambda receipt: receipt.__setitem__(
                    "submitted_tx_hash",
                    receipt["submitted_tx_hash"].replace("C", "c"),
                ),
                "malformed submitted_tx_hash",
            ),
        )
        for name, mutate, error in cases:
            with self.subTest(name=name):
                receipt = json.loads(json.dumps(baseline))
                mutate(receipt)
                completed = subprocess.CompletedProcess(
                    ["iroha", "taira", "inrou-canary"],
                    0,
                    json.dumps(receipt),
                    "",
                )
                with self.assertRaisesRegex(module.DevnetError, error):
                    module.canonical_inrou_canary_outcome(
                        completed,
                        "http://127.0.0.1:29080",
                    )

    def test_inrou_canary_receipt_v1_rejects_unknown_and_legacy_variants(self) -> None:
        baseline = json.loads(FakeRuntime().inrou_canary_stdout)
        cases = (
            (
                "unknown-top-level",
                lambda receipt: receipt.__setitem__("legacy", True),
                "exact V1 schema",
            ),
            (
                "legacy-version",
                lambda receipt: receipt.__setitem__("service_version", "0.9.0"),
                "exact V1 deploy success",
            ),
            (
                "upgrade-version",
                lambda receipt: receipt.__setitem__("service_version", "1.0.1"),
                "exact V1 deploy success",
            ),
            (
                "noncanonical-public-root",
                lambda receipt: receipt.__setitem__(
                    "public_root", "http://127.0.0.1:29080/"
                ),
                "exact V1 deploy success",
            ),
            (
                "unknown-check-field",
                lambda receipt: receipt["checks"][0].__setitem__("legacy", True),
                "check violates the V1 schema",
            ),
            (
                "unknown-replica-field",
                lambda receipt: receipt["replica_identities"][0].__setitem__(
                    "legacy", True
                ),
                "malformed replica identity",
            ),
            (
                "reordered-replicas",
                lambda receipt: receipt["replica_identities"].reverse(),
                "non-canonical replica identity",
            ),
        )
        for name, mutate, error in cases:
            with self.subTest(name=name):
                receipt = json.loads(json.dumps(baseline))
                mutate(receipt)
                completed = subprocess.CompletedProcess(
                    ["iroha", "taira", "inrou-canary"],
                    0,
                    json.dumps(receipt),
                    "",
                )
                with self.assertRaisesRegex(module.DevnetError, error):
                    module.canonical_inrou_canary_outcome(
                        completed,
                        "http://127.0.0.1:29080",
                    )

    def test_managed_directory_refuses_foreign_contents(self) -> None:
        foreign = self.root / "foreign"
        foreign.mkdir()
        (foreign / "keep").write_text("mine\n", encoding="utf-8")

        with self.assertRaisesRegex(module.DevnetError, "unmarked non-empty"):
            module.managed_root(foreign, create=True)

        self.assertEqual((foreign / "keep").read_text(encoding="utf-8"), "mine\n")

    def test_managed_root_rejects_writable_parent_before_creating_state(self) -> None:
        parent = self.root / "writable-devnet-parent"
        parent.mkdir(mode=0o700)
        parent.chmod(0o777)
        for case, precreate in (("missing", False), ("existing", True)):
            with self.subTest(case=case):
                root = parent / case
                if precreate:
                    root.mkdir(mode=0o700)

                with self.assertRaisesRegex(module.DevnetError, "devnet parent"):
                    module.managed_root(root, create=True)

                self.assertEqual(root.exists(), precreate)
                self.assertFalse((root / module.MARKER).exists())
                self.assertFalse((root / "network").exists())

    def test_managed_directory_refuses_foreign_owner_before_marking(self) -> None:
        foreign = self.root / "foreign-owner"
        foreign.mkdir()

        with mock.patch.object(module.os, "geteuid", return_value=os.geteuid() + 1):
            with self.assertRaisesRegex(module.DevnetError, "owned by effective uid"):
                module.managed_root(foreign, create=True)

        self.assertEqual(list(foreign.iterdir()), [])

    def test_managed_directory_rejects_symlinked_ancestry(self) -> None:
        real = self.root / "real"
        real.mkdir()
        alias = self.root / "alias"
        alias.symlink_to(real, target_is_directory=True)

        with self.assertRaisesRegex(module.DevnetError, "non-direct devnet directory"):
            module.managed_root(alias / "state", create=True)
        self.assertFalse((real / "state").exists())

    def test_privileged_cleanup_rejects_foreign_owner_and_mount_crossing(self) -> None:
        root = module.managed_root(self.root / "cleanup-state", create=True)
        target = root / "network"
        target.mkdir()
        expected_owner = os.geteuid()
        with mock.patch.object(
            module.shutil.rmtree,
            "avoids_symlink_attacks",
            True,
        ):
            identity = REAL_REQUIRE_SAFE_CLEANUP_TARGET(
                root,
                target,
                expected_owner=expected_owner,
            )
            self.assertEqual(identity[:2], (target.stat().st_dev, target.stat().st_ino))
            with self.assertRaisesRegex(module.DevnetError, "owned by uid"):
                REAL_REQUIRE_SAFE_CLEANUP_TARGET(
                    root,
                    target,
                    expected_owner=expected_owner + 1,
                )

            mounted = target / "mounted"
            mounted.mkdir()
            real_ismount = module.os.path.ismount
            with mock.patch.object(
                module.os.path,
                "ismount",
                side_effect=lambda path: Path(path) == mounted or real_ismount(path),
            ):
                with self.assertRaisesRegex(module.DevnetError, "mount boundary"):
                    REAL_REQUIRE_SAFE_CLEANUP_TARGET(
                        root,
                        target,
                        expected_owner=expected_owner,
                    )

    def test_reset_network_rejects_managed_root_identity_swap_before_stop(self) -> None:
        root = module.managed_root(self.root / "root-swap-state", create=True)
        original_network = root / "network"
        original_network.mkdir(mode=0o700)
        executable(original_network / "stop.sh", b"#!/usr/bin/env bash\n")
        original_sentinel = original_network / "preserve"
        original_sentinel.write_bytes(b"original cohort\n")
        metadata = root.lstat()
        expected_identity = (metadata.st_dev, metadata.st_ino, metadata.st_uid)

        displaced_root = self.root / "displaced-root"
        root.rename(displaced_root)
        root.mkdir(mode=0o700)
        replacement_network = root / "network"
        replacement_network.mkdir(mode=0o700)
        executable(replacement_network / "stop.sh", b"#!/usr/bin/env bash\n")
        replacement_sentinel = replacement_network / "preserve"
        replacement_sentinel.write_bytes(b"replacement cohort\n")
        runtime = FakeRuntime()

        with self.assertRaisesRegex(module.DevnetError, "root changed"):
            module.reset_network(root, runtime.run, expected_identity)

        self.assertEqual(runtime.commands, [])
        self.assertEqual(
            (displaced_root / "network" / "preserve").read_bytes(),
            b"original cohort\n",
        )
        self.assertEqual(replacement_sentinel.read_bytes(), b"replacement cohort\n")

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
        command = module.cargo_build_command(
            "local-release",
            Path("/tmp/taira-target"),
            self.rust_target,
        )
        self.assertEqual(command[0], str(REPO_ROOT / "scripts" / "cargo_fast.sh"))
        self.assertNotIn("--stable-local-metadata", command)
        self.assertIn("--no-sccache", command)
        self.assertEqual(command[command.index("--target-dir") + 1], "/tmp/taira-target")
        self.assertEqual(command[command.index("--target") + 1], self.rust_target)
        self.assertEqual(command.count("--bin"), 3)
        rendered = " ".join(command)
        self.assertIn("iroha3d_taira", rendered)
        self.assertNotIn("sorafs-node", rendered)
        self.assertNotIn("external-software-signer-bin", rendered)
        self.assertIn("--locked", command)
        self.assertNotIn("--features", command)
        canary_command = module.cargo_build_command(
            "local-release",
            Path("/tmp/taira-target"),
            self.rust_target,
            include_inrou_canary=True,
        )
        self.assertEqual(canary_command.count("--bin"), 4)
        self.assertIn("sorafs-node", canary_command)

        for retired in ("--no-build", "--bin-dir"):
            with contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    module.parser().parse_args(["up", retired])

    def test_rustc_host_target_preserves_the_rustup_proxy_path(self) -> None:
        toolchain = executable(self.root / "rustup-toolchain")
        proxy = self.root / "rustc"
        proxy.symlink_to(toolchain)
        commands: list[tuple[str, ...]] = []

        def run(command, **_kwargs):
            values = tuple(str(value) for value in command)
            commands.append(values)
            return subprocess.CompletedProcess(
                values,
                0,
                "rustc 1.93.1 (test)\nhost: aarch64-unknown-linux-gnu\n",
                "",
            )

        with mock.patch.object(module.shutil, "which", return_value=str(proxy)):
            rustc, target_triple = module.rustc_host_target(run)

        self.assertEqual(rustc, proxy)
        self.assertEqual(target_triple, self.rust_target)
        self.assertEqual(commands, [(str(proxy), "-vV")])

    def test_binary_paths_rejects_symlinked_target_triple_before_build(self) -> None:
        target_dir = self.root / "symlink-triple-target"
        target_dir.mkdir()
        foreign_triple = self.root / "foreign-triple"
        foreign_bin_dir = foreign_triple / module.TAIRA_BUILD_PROFILE
        foreign_bin_dir.mkdir(parents=True)
        binaries = [
            executable(foreign_bin_dir / name, b"sentinel binary\n")
            for name in ("kagami", "iroha3d_taira", "iroha")
        ]
        (target_dir / self.rust_target).symlink_to(
            foreign_triple, target_is_directory=True
        )
        args = self.up_args()
        args.target_dir = target_dir
        runtime = FakeRuntime()

        with self.assertRaises(module.DevnetError):
            module.binary_paths(args, runtime.run)

        self.assertTrue(all(path.read_bytes() == b"sentinel binary\n" for path in binaries))
        self.assertFalse(
            any(Path(command[0]).name == "cargo_fast.sh" for command in runtime.commands)
        )

    def test_binary_paths_rejects_existing_target_under_writable_parent(self) -> None:
        writable_parent = self.root / "writable-parent"
        writable_parent.mkdir(mode=0o700)
        writable_parent.chmod(0o777)
        target_dir = writable_parent / "target"
        bin_dir = target_dir / self.rust_target / module.TAIRA_BUILD_PROFILE
        bin_dir.mkdir(parents=True, mode=0o700)
        binaries = [
            executable(bin_dir / name, b"sentinel binary\n")
            for name in ("kagami", "iroha3d_taira", "iroha")
        ]
        args = self.up_args()
        args.target_dir = target_dir
        runtime = FakeRuntime()

        with self.assertRaises(module.DevnetError):
            module.binary_paths(args, runtime.run)

        self.assertTrue(
            all(path.read_bytes() == b"sentinel binary\n" for path in binaries)
        )
        self.assertEqual(runtime.commands, [])

    def test_binary_paths_rejects_symlinked_profile_parent_before_build(self) -> None:
        target_dir = self.root / "symlink-profile-target"
        triple_dir = target_dir / self.rust_target
        triple_dir.mkdir(parents=True)
        foreign_bin_dir = self.root / "foreign-profile"
        foreign_bin_dir.mkdir()
        binaries = [
            executable(foreign_bin_dir / name, b"sentinel binary\n")
            for name in ("kagami", "iroha3d_taira", "iroha")
        ]
        (triple_dir / module.TAIRA_BUILD_PROFILE).symlink_to(
            foreign_bin_dir, target_is_directory=True
        )
        args = self.up_args()
        args.target_dir = target_dir
        runtime = FakeRuntime()

        with self.assertRaises(module.DevnetError):
            module.binary_paths(args, runtime.run)

        self.assertTrue(all(path.read_bytes() == b"sentinel binary\n" for path in binaries))
        self.assertFalse(
            any(Path(command[0]).name == "cargo_fast.sh" for command in runtime.commands)
        )

    def test_cargo_fast_no_sccache_build_removes_conflicting_environment(self) -> None:
        target_dir = self.root / "target"
        bin_dir = target_dir / self.rust_target / module.TAIRA_BUILD_PROFILE
        bin_dir.mkdir(parents=True, exist_ok=True)
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
        runtime = FakeRuntime()
        calls: list[tuple[tuple[str, ...], dict[str, object]]] = []

        def run(
            command: list[str] | tuple[str, ...],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[str]:
            calls.append((tuple(str(value) for value in command), kwargs))
            return runtime.run(command, **kwargs)

        with mock.patch.dict(
            os.environ,
            {
                "CARGO_BUILD_TARGET": "stale-target",
                "CARGO_INCREMENTAL": "1",
                "CARGO_TARGET_DIR": "/tmp/stale-cargo-target",
                "RUSTC": "stale-rustc",
                "RUSTC_WRAPPER": "sccache",
                "RUSTC_WORKSPACE_WRAPPER": "stale-workspace-wrapper",
                "VERGEN_GIT_SHA": "stale-build",
                "IROHA_GIT_COMMIT_HASH": "0" * 40,
                "TAIRA_TEST_ENV_RETAINED": "yes",
            },
        ):
            paths = module.binary_paths(args, run)

        self.assertEqual(
            paths[:3],
            tuple(bin_dir / name for name in ("kagami", "iroha3d_taira", "iroha")),
        )
        self.assertIsNone(paths[3])
        self.assertEqual(paths[4], self.rust_target)
        build_command, build_kwargs = next(
            (command, kwargs) for command, kwargs in calls if "env" in kwargs
        )
        build_env = build_kwargs["env"]
        self.assertIsInstance(build_env, dict)
        assert isinstance(build_env, dict)
        self.assertNotIn("CARGO_BUILD_TARGET", build_env)
        self.assertNotIn("CARGO_INCREMENTAL", build_env)
        self.assertNotIn("CARGO_TARGET_DIR", build_env)
        self.assertNotIn("RUSTC_WRAPPER", build_env)
        self.assertNotIn("RUSTC_WORKSPACE_WRAPPER", build_env)
        self.assertNotIn("VERGEN_GIT_SHA", build_env)
        self.assertNotIn("IROHA_GIT_COMMIT_HASH", build_env)
        rustc_command = next(
            command
            for command, _kwargs in calls
            if Path(command[0]).name == "rustc" and command[1:] == ("-vV",)
        )
        self.assertEqual(build_env["RUSTC"], rustc_command[0])
        self.assertNotEqual(build_env["RUSTC"], "stale-rustc")
        self.assertEqual(build_env["TAIRA_TEST_ENV_RETAINED"], "yes")
        self.assertEqual(
            build_command[build_command.index("--target") + 1], self.rust_target
        )

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

    def test_canary_surface_drift_preserves_the_running_cohort(self) -> None:
        runtime = FakeRuntime()
        module.up(self.up_args(), run=runtime.run, request=runtime.request)
        target = self.root / "state" / "network"
        sentinel = target / "preserve-before-canary-preflight"
        sentinel.write_text("live cohort\n", encoding="utf-8")
        workspace = self.inrou_canary_workspace()
        runtime.help_options.remove("--timeout-secs")
        stop_count = sum(
            command[0] == "/bin/bash" and command[1].endswith("/stop.sh")
            for command in runtime.commands
        )

        with self.assertRaisesRegex(module.DevnetError, "compiled CLI surface"):
            module.up(
                self.up_args("--inrou-canary-dir", str(workspace)),
                run=runtime.run,
                request=runtime.request,
            )

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
        up_help = subprocess.run(
            [sys.executable, str(MODULE_PATH), "up", "--help"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(up_help.returncode, 0)
        self.assertIn("--inrou-canary-dir", up_help.stdout)

    def test_retired_taira_orchestration_does_not_reappear(self) -> None:
        def names(directory: Path, pattern: str = "*taira*") -> set[str]:
            return {entry.name for entry in directory.glob(pattern) if entry.is_file()}

        self.assertEqual(
            names(REPO_ROOT / "scripts"),
            {
                "render_taira_edge_nginx_conf.py",
                "taira_constants.py",
                "taira_devnet.py",
                "taira_public_reset.py",
            },
        )
        self.assertEqual(
            names(REPO_ROOT / "scripts" / "tests"),
            {
                "render_taira_edge_nginx_conf_test.py",
                "taira_devnet_test.py",
                "taira_inrou_canary_identity_source_test.py",
                "taira_public_reset_test.py",
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
