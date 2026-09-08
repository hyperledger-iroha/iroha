#!/usr/bin/env python3
"""Native retry identity, custody and failure-handling tests without SSH or live secrets."""

import copy
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "taira_retry.py"
SPEC = importlib.util.spec_from_file_location("taira_retry", SCRIPT)
retry = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(retry)
CAPACITY = SCRIPT.with_name("taira_disk_capacity.py")
SPEC_CAPACITY = importlib.util.spec_from_file_location("retry_capacity_test", CAPACITY)
capacity = importlib.util.module_from_spec(SPEC_CAPACITY)
SPEC_CAPACITY.loader.exec_module(capacity)


def artifact_receipts():
    artifacts = [
        {"name": name, "size": 10, "sha256": "b" * 64}
        for name in ("iroha", "iroha3d_taira", "sorafs-node", "kagami")
    ]
    build = {
        "commit": "a" * 40,
        "tree": "c" * 40,
        "source_unchanged": True,
        "toolchain_unchanged": True,
        "target": "aarch64-unknown-linux-gnu",
        "profile": "release",
        "jobs": 6,
        "deployed": False,
        "release_qualified": False,
        "artifacts": copy.deepcopy(artifacts),
    }
    binary = {
        "commit": build["commit"],
        "destination": "/runtime/artifacts/bin",
        "all_hashes_verified": True,
        "activated": False,
        "artifacts": artifacts,
    }
    source = {
        "commit": build["commit"],
        "tree": build["tree"],
        "clean": True,
        "signature_verified": True,
        "object_inventory_verified": True,
        "activated": False,
        "runtime_files_transferred": False,
        "history_included": False,
        "source_root": "/source",
    }
    return build, binary, source


def full_plan():
    footprint = {"bytes": 10, "inodes": 1}
    return capacity.cohost_peak_plan(
        coordinator_path="/runtime",
        upload_path="/srv/taira",
        service_path="/srv/taira",
        store_paths=[f"/var/lib/taira/validator-{i}/store" for i in range(4)],
        runtime_paths=[f"/var/lib/taira/validator-{i}/runtime" for i in range(4)],
        artifacts=footprint,
        stage=footprint,
        per_store=footprint,
        per_replica_runtime=footprint,
        headroom=[
            {
                "path": "/runtime",
                "label": "filesystem headroom",
                "bytes": 2 * 1024**3,
                "inodes": 1024,
            }
        ],
    )


def measured_inputs():
    stage = "/private/runtime/retained/inrou-stage"
    names = {
        "receipt.json": 2896,
        "container.json": 1398,
        "service.json": 2891,
        "manifests/aarch64.to": 502,
        "manifests/discovery.to": 502,
        "manifests/bundle.to": 502,
        "payloads/bundle.bin": 4082,
        "payloads/guest/aarch64/initrd.img": 13923072,
        "payloads/guest/aarch64/vmlinux": 27236288,
        "payloads/guest/aarch64/rootfs.ext4": 1610612736,
        "payloads/discovery/index.json": 729,
    }
    roles = [
        {"slug": f"taira-validator-{index}", "role": role, "bytes": 128}
        for index in range(1, 5)
        for role in (
            "iroha3d",
            "iroha_cli",
            "sorafs_node",
            "config",
            "genesis",
            "genesis_hash",
        )
    ]
    roles += [
        {"slug": "taira-edge", "role": role, "bytes": 128}
        for role in ("iroha_cli", "edge_config")
    ]
    inputs = {
        "schema": "taira.public-capacity-inputs.v1",
        "secret_contents_read": False,
        "filesystem": {"fragment_bytes": 4096},
        "artifacts": roles,
        "stage_files": [
            {"path": stage + "/" + name, "bytes": size} for name, size in names.items()
        ],
        "inventory_inrou_stage_bytes": sum(names.values()),
        "native_sf1_manifest_bindings": {
            name: "f" * 64 for name in ("bundle", "guest", "discovery")
        },
    }
    runtime = {
        "schema": "taira.public-runtime-capacity-inputs.v1",
        "secret_contents_read": False,
        "source_stage": stage,
        "service_artifacts": [],
        "bundle_compressed_bytes": 4082,
        "bundle_archive_decoded_bytes": 16384,
        "bundle_members": [{"path": "app/server.py", "kind": "file", "bytes": 14608}],
        "lease_volumes": [
            {"volume_name": "root_disk", "max_total_bytes": 1610612736},
            {"volume_name": "app_data", "max_total_bytes": 67108864},
        ],
        "container_resources": {"ephemeral_storage_bytes": 16777216},
    }
    return inputs, runtime


def derived_capacity(inputs=None, runtime=None, build=None):
    original_inputs, original_runtime = measured_inputs()
    return capacity.derive_capacity(
        inputs or original_inputs,
        runtime or original_runtime,
        build or artifact_receipts()[0],
        expected_commit="a" * 40,
        coordinator_path="/private/runtime/journal-v1",
        upload_path="/srv/taira",
        service_path="/srv/taira",
        store_paths=[
            f"/var/lib/taira/taira-validator-{i}/sorafs-data" for i in range(1, 5)
        ],
        runtime_paths=[
            f"/var/lib/taira/taira-validator-{i}/inrou-data" for i in range(1, 5)
        ],
        guest_headroom_path="/private/runtime",
        backing_path="/approved/vm",
    )


class RetryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.root.chmod(0o700)

    def tearDown(self):
        self.temporary.cleanup()

    def record(self, name, value):
        path = self.root / name
        retry.write_public(path, value)
        return path

    def ssh_route(self, host, known_hosts, *, proxy=None, forwarding=None):
        argv = ["/usr/bin/ssh", "-T", "-F", "/dev/null", "-i", "/private/ssh-identity"]
        for value in (
            "BatchMode=yes",
            "NumberOfPasswordPrompts=0",
            "IdentitiesOnly=yes",
            "IdentityAgent=none",
            "StrictHostKeyChecking=yes",
            "ForwardAgent=no",
            "ClearAllForwardings=yes",
            "GlobalKnownHostsFile=/dev/null",
            "UpdateHostKeys=no",
            "VerifyHostKeyDNS=no",
            "UserKnownHostsFile=" + str(known_hosts),
        ):
            argv += ["-o", value]
        if proxy is not None:
            argv += ["-o", "ProxyCommand=" + retry.shlex.join(proxy)]
        if forwarding is not None:
            argv += ["-W", forwarding, host]
        else:
            argv += [host, "/usr/bin/python3 -I -"]
        return argv

    def test_ssh_pins_exact_host_key_files_for_both_fixed_hops(self):
        guest_keys = self.record("guest_known_hosts", {"public": "guest"})
        mac_keys = self.record("mac_known_hosts", {"public": "backing"})
        proxy = self.ssh_route(
            "administrator@208.83.1.62", mac_keys, forwarding="192.168.64.3:22"
        )
        argv = self.ssh_route("root@192.168.64.3", guest_keys, proxy=proxy)
        pins = [
            {
                "path": str(path),
                "sha256": retry.hashlib.sha256(path.read_bytes()).hexdigest(),
            }
            for path in (guest_keys, mac_keys)
        ]
        self.assertEqual(retry.validate_ssh({"argv": argv, "pins": pins}), argv)
        # The established jump route predates IdentityAgent=none; IdentitiesOnly
        # and its explicit identity still constrain it to that admitted key.
        proxy.remove("IdentityAgent=none")
        proxy.pop(proxy.index("IdentitiesOnly=yes") + 1)
        argv = self.ssh_route("root@192.168.64.3", guest_keys, proxy=proxy)
        self.assertEqual(retry.validate_ssh({"argv": argv, "pins": pins}), argv)

    def test_ssh_rejects_unrelated_or_missing_pins_before_reading_any_bytes(self):
        argv = self.ssh_route("root@192.168.64.3", "/public/known_hosts")
        valid = {"path": "/public/known_hosts", "sha256": "a" * 64}
        secret = {"path": "/private/ssh-identity", "sha256": "b" * 64}
        for pins in ([secret], [valid, secret], [], [valid, valid]):
            with (
                self.subTest(pins=[pin["path"] for pin in pins]),
                mock.patch.object(retry, "public_record") as read,
            ):
                with self.assertRaises(retry.RetryError):
                    retry.validate_ssh({"argv": argv, "pins": pins})
                read.assert_not_called()

    def test_ssh_rejects_shell_proxy_wrong_forwarding_and_option_override(self):
        proxy = self.ssh_route(
            "administrator@208.83.1.62",
            "/public/mac_hosts",
            forwarding="192.168.64.3:22",
        )
        good = self.ssh_route("root@192.168.64.3", "/public/guest_hosts", proxy=proxy)
        wrong_target = copy.deepcopy(good)
        wrong_target[
            wrong_target.index(
                next(arg for arg in wrong_target if arg.startswith("ProxyCommand="))
            )
        ] = "ProxyCommand=" + retry.shlex.join(proxy).replace(
            "192.168.64.3:22", "192.168.64.4:22"
        )
        shell = copy.deepcopy(good)
        shell[
            shell.index(next(arg for arg in shell if arg.startswith("ProxyCommand=")))
        ] += "; touch /bad"
        override = good[:-2] + ["-o", "stricthostkeychecking=no"] + good[-2:]
        local_command = good[:-2] + ["-o", "LocalCommand=touch /bad"] + good[-2:]
        for argv in (wrong_target, shell, override, local_command):
            with self.assertRaises(retry.RetryError):
                retry.ssh_host_key_paths(argv)

    def test_duplicate_json_and_record_replacement_reject(self):
        with self.assertRaises(retry.RetryError):
            retry.decode(b'{"field":1,"field":2}')
        path = self.record("record.json", {"public": True})
        with self.assertRaises(FileExistsError):
            retry.write_public(path, {"public": False})
        self.assertEqual(retry.decode(retry.public_record(path)), {"public": True})

    def test_public_record_digest_mode_symlink_and_fifo_reject(self):
        path = self.record("record.json", {"public": True})
        with self.assertRaises(retry.RetryError):
            retry.public_record(path, "0" * 64)
        path.chmod(0o644)
        with self.assertRaises(retry.RetryError):
            retry.public_record(path, private=True)
        link = self.root / "link"
        link.symlink_to(path)
        with self.assertRaises(retry.RetryError):
            retry.public_record(link)
        fifo = self.root / "fifo"
        os.mkfifo(fifo, 0o600)
        with self.assertRaises(retry.RetryError):
            retry.public_record(fifo)

    def test_fresh_inventory_changes_only_attempt_and_nonce(self):
        previous = {
            "deployment_id": "retained",
            "authorization_nonce": "0" * 32,
            "revision": {"commit": "a" * 40},
            "validators": [{"artifact": "same-config"}],
        }
        expected = copy.deepcopy(previous)
        actual = retry.fresh_inventory(
            previous, "retry-1788850000000000000-1234abcd", "1" * 32
        )
        expected.update(
            deployment_id="taira-retry-1788850000000000000-1234abcd",
            authorization_nonce="1" * 32,
        )
        self.assertEqual(actual, expected)
        self.assertEqual(previous["deployment_id"], "retained")
        actual["validators"][0]["artifact"] = "changed"
        self.assertEqual(previous["validators"][0]["artifact"], "same-config")
        with self.assertRaises(retry.RetryError):
            retry.fresh_inventory(
                previous, "retry-1788850000000000000-1234abcd", "0" * 32
            )

    def test_actual_artifact_identity_is_separate_from_attempt(self):
        build, binary, source = artifact_receipts()
        self.assertEqual(
            retry.validate_artifact_receipts(build, binary, source), build["commit"]
        )
        for change in ("binary", "source", "build"):
            b, a, s = copy.deepcopy((build, binary, source))
            if change == "binary":
                a["artifacts"][0]["sha256"] = "d" * 64
            elif change == "source":
                s["commit"] = "e" * 40
            else:
                b["exit_code"] = 0
            with self.assertRaises(retry.RetryError):
                retry.validate_artifact_receipts(b, a, s)

    def test_config_paths_remain_exact_for_same_artifact_retry(self):
        build, binary, source = artifact_receipts()
        row = {
            "role": "iroha_cli",
            "sha256": "b" * 64,
            "local_path": "/runtime/artifacts/bin/iroha",
        }
        inventory = {
            "revision": {"commit": build["commit"], "source_root": "/source"},
            "validators": [{"artifacts": [row]}],
            "edge": {"artifacts": [row]},
        }
        retry.require_same_inventory_artifacts(inventory, binary, source)
        row["local_path"] = "/runtime/new/bin/iroha"
        with self.assertRaises(retry.RetryError):
            retry.require_same_inventory_artifacts(inventory, binary, source)

    def test_complete_edge_rollback_is_supported_and_incomplete_edge_rejects(self):
        inventory = {"revision": {"commit": "a" * 40}, "deployment_id": "actual75"}
        value = {
            "deployment_id": "actual75",
            "status": "rolled_back",
            "phase": "rolled_back",
            "next_step": 9,
            "touched_validators": list(retry.RETIRE_SLUGS[:-1]),
            "edge_touched": True,
            "edge_rollback_complete": True,
            "rollback_next_validator": 4,
            "rollback_failures": [],
            "recovery_intent": None,
        }
        with (
            mock.patch.object(retry, "RETIRE_COMMIT", "a" * 40),
            mock.patch.object(retry, "RETIRE_RETAINED_DEPLOYMENT", "actual75"),
        ):
            retry._retire_validate_terminal(inventory, value)
            for field, changed in (
                ("edge_rollback_complete", False),
                ("rollback_next_validator", 3),
                ("recovery_intent", {}),
                ("rollback_failures", ["failed"]),
                ("phase", "rolling_back"),
            ):
                with (
                    self.subTest(field=field),
                    self.assertRaises(retry._retire_RebindError),
                ):
                    retry._retire_validate_terminal(
                        inventory, dict(value, **{field: changed})
                    )

    def test_host_progress_must_include_completed_edge(self):
        context = {
            "inventory_sha256": "a",
            "authorization_sha256": "b",
            "nonce": "c",
            "edge_touched": True,
        }
        progress = {
            "schema": "iroha.taira.public-reset.host-progress.v1",
            "inventory_sha256": "a",
            "authorization_sha256": "b",
            "authorization_nonce": "c",
            "prepared_action": None,
            "rolling_back": True,
            "sealed": False,
            "touched_hosts": list(retry.RETIRE_SLUGS),
            "rolled_back_hosts": list(retry.RETIRE_SLUGS),
        }
        retry._retire_validate_progress(progress, context)
        progress["rolled_back_hosts"] = list(retry.RETIRE_SLUGS[:-1])
        with self.assertRaises(retry._retire_RebindError):
            retry._retire_validate_progress(progress, context)

    def test_same_cli_retirement_accepts_exact_manifest(self):
        _, manifest, _ = artifact_receipts()
        fake = SimpleNamespace(fd=123, close=lambda: None)
        args = SimpleNamespace(expected_commit="a" * 40, new_cli_sha256="b" * 64)
        with (
            mock.patch.object(
                retry, "RETIRE_BINARY_MANIFEST", Path("/runtime/manifest.json")
            ),
            mock.patch.object(retry, "RETIRE_BINS", Path(manifest["destination"])),
            mock.patch.object(retry, "RETIRE_COMMIT", args.expected_commit),
            mock.patch.object(retry, "RETIRE_CLI_SHA", args.new_cli_sha256),
            mock.patch.object(
                retry, "_retire_read_public", return_value=json.dumps(manifest).encode()
            ),
            mock.patch.object(retry, "_retire_binary", return_value=fake),
            mock.patch.object(
                retry.os, "fstat", return_value=SimpleNamespace(st_size=10)
            ),
            mock.patch.object(retry, "_retire_retained_context", return_value={}),
        ):
            value = retry._retire_context_from_retained({}, args)
        self.assertEqual(value["expected_commit"], "a" * 40)
        self.assertEqual(value["new_dispatcher_sha256"], "b" * 64)

    def test_capacity_observation_reads_only_public_stage_content_not_configs(self):
        stage = self.root / "stage"
        (stage / "payloads").mkdir(parents=True)
        config = self.root / "peer.toml"
        config.write_bytes(b"private_config_fixture_must_never_be_read")
        config.chmod(0o600)
        (stage / "container.json").write_text(
            json.dumps({"resources": {"ephemeral_storage_bytes": 16}})
        )
        (stage / "service.json").write_text(
            json.dumps({"lease_volumes": [], "artifacts": []})
        )
        archive = io.BytesIO()
        with retry.tarfile.open(fileobj=archive, mode="w:") as tar:
            info = retry.tarfile.TarInfo("app/main.py")
            info.size = 4
            tar.addfile(info, io.BytesIO(b"pass"))
        (stage / "payloads/bundle.bin").write_bytes(
            retry.gzip.compress(archive.getvalue())
        )
        (stage / "manifests").mkdir()
        bindings = {}
        for role, name in (
            ("bundle", "bundle.to"),
            ("guest", "aarch64.to"),
            ("discovery", "discovery.to"),
        ):
            path = stage / "manifests" / name
            path.write_bytes(("public manifest fixture " + role).encode())
            bindings[role + "_manifest_sha256"] = retry.hashlib.sha256(
                path.read_bytes()
            ).hexdigest()
        inventory = {
            "revision": {"commit": "a" * 40},
            "validators": [
                {
                    "slug": "taira-validator-1",
                    "artifacts": [
                        {
                            "role": "config",
                            "local_path": str(config),
                            "size": config.stat().st_size,
                        }
                    ],
                }
            ],
            "edge": {"slug": "taira-edge", "artifacts": []},
            "inrou_canary": {
                **bindings,
                "stage_bytes": sum(
                    path.stat().st_size for path in stage.rglob("*") if path.is_file()
                ),
            },
        }
        actual_lstat = Path.lstat

        def root_metadata(path):
            info = actual_lstat(path)
            result = {
                name: getattr(info, name)
                for name in dir(info)
                if name.startswith("st_")
            }
            result["st_uid"] = 0
            return SimpleNamespace(**result)

        allowed = {
            stage / "container.json",
            stage / "service.json",
            stage / "payloads/bundle.bin",
        }
        allowed.update((stage / "manifests").iterdir())
        reads = []

        def read_public(path, *args, **kwargs):
            path = Path(path)
            self.assertIn(path, allowed)
            reads.append(path)
            return path.read_bytes()

        with (
            mock.patch.object(Path, "lstat", root_metadata),
            mock.patch.object(retry, "public_record", side_effect=read_public),
        ):
            inputs, runtime = retry.measured_capacity_inputs(inventory, stage)
        self.assertEqual(set(reads), allowed)
        self.assertEqual(inputs["artifacts"][0]["bytes"], config.stat().st_size)
        self.assertEqual(
            runtime["bundle_members"],
            [{"path": "app/main.py", "kind": "file", "bytes": 4}],
        )
        self.assertNotIn("private_config_fixture", json.dumps((inputs, runtime)))

    def test_measured_capacity_includes_four_runtimes_and_physical_reserve(self):
        result = derived_capacity()
        self.assertEqual(
            result["derivation"]["per_replica_runtime"],
            {"bytes": 3347934329, "inodes": 139},
        )
        rows = result["guest_plan"]["allocations"]
        self.assertEqual(
            sum(
                row["bytes"]
                for row in rows
                if row["label"].startswith("runtime replica ")
            ),
            13391737316,
        )
        self.assertEqual(
            sum(row["bytes"] for row in result["backing_plan"]["allocations"]),
            sum(row["bytes"] for row in rows) + 2 * 1024**3,
        )
        inputs, runtime = measured_inputs()
        runtime["lease_volumes"][1]["max_total_bytes"] += 1024**2
        self.assertEqual(
            derived_capacity(inputs, runtime)["derivation"]["required_bytes"]
            - result["derivation"]["required_bytes"],
            4 * 1024**2,
        )

    def test_measured_capacity_uses_actual_binary_role_weights_and_config_bound(self):
        old = derived_capacity()
        build = artifact_receipts()[0]
        next(row for row in build["artifacts"] if row["name"] == "iroha3d_taira")[
            "size"
        ] += 1024
        next(row for row in build["artifacts"] if row["name"] == "iroha")["size"] += (
            2048
        )
        new = derived_capacity(build=build)
        self.assertEqual(
            new["derivation"]["required_bytes"] - old["derivation"]["required_bytes"],
            3 * (4 * 1024 + 5 * 2048),
        )
        configs = [
            row
            for row in new["derivation"]["artifact_role_sizes"]
            if row["role"] == "config"
        ]
        self.assertEqual([row["bytes"] for row in configs], [1024**2] * 4)

    def test_measured_capacity_rejects_unknown_stage_service_and_missing_role(self):
        for variant in ("stage", "service", "role", "profile"):
            inputs, runtime = measured_inputs()
            if variant == "stage":
                inputs["stage_files"].pop()
            if variant == "service":
                runtime["service_artifacts"] = [{"unmodeled": True}]
            if variant == "role":
                inputs["artifacts"].pop()
            if variant == "profile":
                inputs.pop("native_sf1_manifest_bindings")
            with (
                self.subTest(variant=variant),
                self.assertRaises(capacity.CapacityError),
            ):
                derived_capacity(inputs, runtime)

    def test_runtime_paths_come_from_native_assembly_not_new_attempt_names(self):
        prep = "/private/runtime/prep"
        arguments = {
            "--runtime-client-config": [prep + "/runtime-client.toml"],
            "--validator-client-config": [
                prep + f"/validator-{i}-client.toml" for i in range(1, 5)
            ],
            "--inrou-stage-dir": [prep + "/inrou-stage"],
            "--onboarding-token": [prep + "/network/runtime/onboarding.token"],
            "--known-hosts": ["/private/runtime/known-hosts"],
        }
        plan = {
            "runtime_root": "/private/runtime",
            "previous_inventory": "/private/runtime/assembly/inventory.json",
        }
        _, binary, _ = artifact_receipts()
        result = retry.derive_runtime_paths(
            plan, binary, {"revision": {"commit": "a" * 40}}, arguments
        )
        self.assertEqual(result["attempts_root"], "/private/runtime/retry-v1")
        self.assertEqual(result["prep_root"], prep)
        self.assertEqual(
            result["binary_manifest"], "/runtime/artifacts/verified-manifest.json"
        )
        self.assertEqual(
            result["local_args_path"],
            "/private/runtime/assembly/native-local-args.json",
        )
        arguments["--onboarding-token"] = ["/private/unrelated/token"]
        with self.assertRaises(retry.RetryError):
            retry.derive_runtime_paths(
                plan, binary, {"revision": {"commit": "a" * 40}}, arguments
            )

    def test_full_capacity_requires_all_four_runtime_footprints(self):
        module = {"validate_plan": capacity.validate_plan}
        plan = full_plan()
        retry.validate_full_capacity(module, plan)
        plan["allocations"] = [
            row for row in plan["allocations"] if row["label"] != "runtime replica 4"
        ]
        with self.assertRaises(retry.RetryError):
            retry.validate_full_capacity(module, plan)

    def test_capacity_false_and_missing_headroom_fail_before_mutation(self):
        with self.assertRaises(retry.RetryError):
            retry.check_capacity(
                {"evaluate": lambda _: {"passed": False, "errors": ["ENOSPC"]}},
                {},
                "apply",
            )
        plan = full_plan()
        plan["allocations"][-1]["bytes"] = 1
        with self.assertRaises(retry.RetryError):
            retry.validate_full_capacity(
                {"validate_plan": capacity.validate_plan}, plan
            )

    def test_errno_projection_keeps_operation_and_never_key_or_header_values(self):
        raw = (
            b"phase=preseed failed to write staged chunk4596: No space left on device (os error 28)\n"
            b"private_key=NOT-A-REAL-KEY authorization: bearer NOT-A-REAL-TOKEN"
        )
        result = retry.safe_native_error(raw)
        self.assertEqual(result["errno_name"], "ENOSPC")
        self.assertEqual(result["native_phase"], "preseed")
        self.assertEqual(result["operation"], "write staged chunk")
        self.assertNotIn("NOT-A-REAL", json.dumps(result))

    def test_native_heartbeat_projects_only_phase_and_counts(self):
        path = self.record(
            "journal.json",
            {
                "schema": "iroha.taira.public-reset.journal.v1",
                "phase": "preseed",
                "next_step": 5,
                "touched_validators": ["one", "two", "three", "four"],
                "edge_touched": False,
                "authorization_nonce": "MUST_NOT_LEAK",
                "failure_summary": "SECRET_TEST_VALUE",
            },
        )
        with mock.patch.object(
            retry,
            "public_record",
            side_effect=lambda path, **kwargs: Path(path).read_bytes(),
        ):
            result = retry.native_journal_progress(path)
        self.assertEqual(
            result,
            {
                "native_phase": "preseed",
                "next_step": 5,
                "touched_validator_count": 4,
                "edge_touched": False,
            },
        )
        self.assertNotIn("MUST_NOT_LEAK", json.dumps(result))
        self.assertNotIn("SECRET_TEST_VALUE", json.dumps(result))

    def test_public_validation_requires_same_source_network_and_curated_mcp(self):
        _, binary, _ = artifact_receipts()
        seed = self.root / "seed"
        seed.mkdir()
        retry.write_public(
            seed / "seed-authority-receipt.json", {"network_id": "native-network"}
        )
        for variant in ("valid", "wrong-source", "wrong-network", "mcp-error"):
            output = self.root / variant
            calls = []

            def native(argv, directory, *, phase, env, **kwargs):
                self.assertEqual(env, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
                self.assertNotIn("--config", argv)
                directory.mkdir(mode=0o700)
                calls.append(argv)
                if argv[0] != "/usr/bin/curl":
                    retry.write_public(
                        directory / "stdout",
                        {
                            "command": "taira_doctor",
                            "public_root": "https://taira.sora.org",
                            "status": "ok",
                            "failures": [],
                            "checks": [{"ok": True}],
                        },
                    )
                    return
                self.assertIn("--noproxy", argv)
                response = {
                    "status": {
                        "build": {
                            "git_commit_sha": "0" * 40
                            if variant == "wrong-source"
                            else binary["commit"],
                            "target_triple": "aarch64-unknown-linux-gnu",
                        }
                    },
                    "tip": 12,
                    "network": {
                        "network_id": "other"
                        if variant == "wrong-network"
                        else "native-network",
                        "chain_discriminant": 369,
                    },
                    "mcp": {
                        "jsonrpc": "2.0",
                        "id": 3,
                        "result": {
                            "isError": variant == "mcp-error",
                            "structuredContent": {"status": 200},
                        },
                    },
                }[directory.name]
                (directory / "stdout").write_bytes(b"200")
                retry.write_public(directory / "body.json", response)
                if directory.name == "mcp":
                    body = json.loads(argv[argv.index("--data-binary") + 1])
                    self.assertEqual(body["params"]["name"], "iroha.health")
                    self.assertEqual(body["params"]["arguments"], {})

            with (
                mock.patch.object(retry, "CONTINUITY_OUT", seed),
                mock.patch.object(retry, "run_native", side_effect=native),
                mock.patch.object(
                    retry,
                    "public_record",
                    side_effect=lambda path, **kwargs: Path(path).read_bytes(),
                ),
            ):
                if variant == "valid":
                    result = retry.public_validation(binary, {}, output)
                    self.assertTrue(result["public_mcp_health_passed"])
                    self.assertFalse(result["application_validation_completed"])
                else:
                    with self.assertRaises(retry.RetryError):
                        retry.public_validation(binary, {}, output)
            self.assertEqual(len(calls), 5)

    def test_native_error_tail_is_bounded_and_public_only(self):
        path = self.root / "stderr"
        path.write_bytes(b"x" * 100000 + b" write staged chunk 4596 (os error 28)")
        path.chmod(0o600)
        self.assertEqual(retry.native_error(path)["chunk"], 4596)

    def test_native_command_is_submitted_once_across_progress_waits(self):
        process = mock.Mock()
        process.wait.side_effect = [subprocess.TimeoutExpired("native", 30), 0]
        with (
            mock.patch.object(retry.subprocess, "Popen", return_value=process) as popen,
            mock.patch.object(retry, "emit"),
        ):
            result = retry.run_native(
                ["/native", "apply"], self.root / "native", phase="apply"
            )
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(popen.call_count, 1)
        self.assertEqual(process.wait.call_count, 2)

    def test_native_failure_preserves_errno_receipt_and_never_resubmits(self):
        def create(*args, **kwargs):
            kwargs["stderr"].write(b"write staged chunk 4596 (os error 28)")
            return SimpleNamespace(wait=lambda timeout: 1)

        with (
            mock.patch.object(retry.subprocess, "Popen", side_effect=create) as popen,
            mock.patch.object(retry, "emit"),
            self.assertRaises(retry.RetryError),
        ):
            retry.run_native(["/native", "apply"], self.root / "native", phase="apply")
        value = json.loads((self.root / "native/result.json").read_bytes())
        self.assertEqual(value["errno_name"], "ENOSPC")
        self.assertFalse(value["automatic_replay"])
        self.assertEqual(popen.call_count, 1)

    def test_future_import_module_payload_does_not_run_cli_main(self):
        source = b'from __future__ import annotations\ndef evaluate(value): return value\nif __name__ == "__main__": raise AssertionError("CLI main")\n'
        payload = retry.remote_payload(
            source, "evaluate", {"passed": True}, print_result=True
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            exec(compile(payload, "<test>", "exec"), {})
        self.assertEqual(json.loads(output.getvalue()), {"passed": True})

    def test_signer_is_an_inherited_read_only_fd_without_python_read(self):
        path = self.root / "signer"
        path.write_bytes(b"PUBLIC-NONKEY-FIXTURE")
        path.chmod(0o600)
        real_fstat = os.fstat

        def info(fd):
            original = real_fstat(fd)
            return SimpleNamespace(st_mode=original.st_mode, st_uid=0, st_nlink=1)

        captured = []

        def native(argv, output, *, phase, pass_fds):
            fd = pass_fds[0]
            self.assertEqual(
                retry.fcntl.fcntl(fd, retry.fcntl.F_GETFL) & os.O_ACCMODE, os.O_RDONLY
            )
            self.assertEqual(argv[argv.index("--signing-key-fd") + 1], fd)
            captured.append(fd)
            return {"passed": True}

        with (
            mock.patch.object(retry.os, "fstat", side_effect=info),
            mock.patch.object(retry, "run_native", side_effect=native),
            mock.patch.object(
                retry.os, "read", side_effect=AssertionError("Python secret read")
            ),
            mock.patch.object(
                retry.os, "pread", side_effect=AssertionError("Python secret pread")
            ),
        ):
            retry.authorize_native(
                ["/native"],
                self.root,
                [],
                {"signing_key": str(path), "trusted_public_key": "/trust"},
                self.root / "native",
            )
        with self.assertRaises(OSError):
            os.fstat(captured[0])

    def test_progress_relay_drops_unstructured_secret_output(self):
        path = self.root / "stdout"
        path.write_bytes(
            b"authorization: NOT-A-REAL-TOKEN\n"
            + json.dumps(
                {
                    "schema": retry.PROGRESS_SCHEMA,
                    "phase": "apply",
                    "status": "failed",
                    "errno_name": "ENOSPC",
                    "unexpected_secret": "NOT-A-REAL-TOKEN",
                }
            ).encode()
            + b"\n"
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            cursor, pending, phase = retry.relay_progress(path, 0, b"", "start")
        self.assertEqual(phase, "apply")
        self.assertEqual(cursor, path.stat().st_size)
        self.assertEqual(pending, b"")
        self.assertNotIn("NOT-A-REAL-TOKEN", output.getvalue())
        self.assertIn("ENOSPC", output.getvalue())

    def test_pending_mutation_is_not_inferred_as_rolled_back(self):
        root = self.root / "journal"
        (root / "rolled-back").mkdir(parents=True)
        with self.assertRaisesRegex(retry.RetryError, "pending mutation"):
            retry.find_terminal(root, "pending")

    def test_local_arguments_are_exact_ordered_paths(self):
        flags = (
            ("--runtime-client-config", 1),
            ("--validator-client-config", 4),
            ("--onboarding-token", 1),
            ("--inrou-stage-dir", 1),
            ("--validator-unit", 4),
            ("--edge-unit", 1),
            ("--known-hosts", 1),
        )
        args = []
        for flag, count in flags:
            args += [flag, *["/runtime/input-" + str(index) for index in range(count)]]
        actual, grouped = retry.local_arguments(json.dumps(args).encode())
        self.assertEqual(actual, args)
        self.assertEqual(len(grouped["--validator-client-config"]), 4)
        args[0] = "--private-key"
        with self.assertRaises(retry.RetryError):
            retry.local_arguments(json.dumps(args).encode())

    def test_phase_timing_and_failure_record_are_exclusive(self):
        with mock.patch.object(retry, "emit"):
            self.assertEqual(retry.call_phase("retire", lambda: 7, self.root), 7)
            with self.assertRaises(ZeroDivisionError):
                retry.call_phase("seed-post", lambda: 1 / 0, self.root)
        self.assertTrue(
            json.loads((self.root / "retire-phase.json").read_bytes())["passed"]
        )
        self.assertFalse(
            json.loads((self.root / "seed-post-phase.json").read_bytes())["passed"]
        )


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.attempts = self.root / "attempts"
        self.attempts.mkdir(mode=0o700)
        build, self.binary, self.source = artifact_receipts()
        self.inventory = {
            "revision": {"commit": build["commit"], "source_root": "/source"},
            "deployment_id": "retained",
            "authorization_nonce": "0" * 32,
            "next_genesis_hash": "c" * 64,
            "validators": [],
        }
        for index in range(1, 5):
            self.inventory["validators"].append(
                {
                    "slug": f"taira-validator-{index}",
                    "systemd_unit": f"iroha3d-taira-validator-{index}.service",
                    "systemd_unit_sha256": "d" * 64,
                    "artifacts": [
                        {
                            "role": "iroha_cli",
                            "local_path": "/runtime/artifacts/bin/iroha",
                            "sha256": "b" * 64,
                        }
                    ],
                }
            )
        self.inventory["edge"] = {
            "systemd_unit_sha256": "e" * 64,
            "artifacts": [dict(self.inventory["validators"][0]["artifacts"][0])],
        }

        def record(name, value):
            path = self.root / name
            path.write_text(json.dumps(value))
            return str(path)

        args = []
        for flag, count in (
            ("--runtime-client-config", 1),
            ("--validator-client-config", 4),
            ("--onboarding-token", 1),
            ("--inrou-stage-dir", 1),
            ("--validator-unit", 4),
            ("--edge-unit", 1),
            ("--known-hosts", 1),
        ):
            paths = [
                f"/public-fixture/{flag.removeprefix('--')}-{i}" for i in range(count)
            ]
            if flag == "--validator-unit":
                paths = [
                    f"/units/{row['systemd_unit']}"
                    for row in self.inventory["validators"]
                ]
            if flag == "--known-hosts":
                paths = ["/public-fixture/known_hosts"]
            args.extend([flag, *paths])
        reference = {"path": "/public-fixture/helper.py", "sha256": "f" * 64}
        self.plan = {
            "runtime_root": str(self.root),
            "attempts_root": str(self.attempts),
            "previous_inventory": record("inventory.json", self.inventory),
            "previous_terminal": str(
                self.root / "journal-v1/rolled-back" / ("a" * 64 + ".json")
            ),
            "local_args_path": record("args.json", args),
            "binary_manifest": record("binary.json", self.binary),
            "source_manifest": record("source.json", self.source),
            "trusted_public_key": record("trust.json", {"public_test_fixture": True}),
            "signing_key": "/public-fixture/signer",
            "ssh_identity": "/public-fixture/ssh_identity",
            "known_hosts": "/public-fixture/known_hosts",
            "prep_root": "/public-fixture/prep",
            "guard_support": reference,
            "unit_renderer": reference,
            "local_node": reference,
            "expected_mac": "00:00:00:00:00:00",
            "capacity_plan": full_plan(),
        }
        self.request = {
            "plan": self.plan,
            "binary": self.binary,
            "source": self.source,
            "binary_sha256": "1" * 64,
            "source_sha256": "2" * 64,
        }
        self.calls = []
        self.fail_phase = None
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.stack.enter_context(
            mock.patch.object(
                retry,
                "public_record",
                side_effect=lambda path, *a, **k: Path(path).read_bytes(),
            )
        )
        self.stack.enter_context(
            mock.patch.object(retry, "_retire_load_support", return_value={})
        )
        self.stack.enter_context(
            mock.patch.object(retry, "_retire_context_from_retained", return_value={})
        )
        self.stack.enter_context(
            mock.patch.object(
                retry, "_retire_locks", side_effect=lambda *a: contextlib.nullcontext()
            )
        )
        self.stack.enter_context(mock.patch.object(retry, "_retire_retained_state"))
        self.stack.enter_context(
            mock.patch.object(retry, "_retire_root_identity", return_value={})
        )
        self.stack.enter_context(
            mock.patch.object(
                retry, "_retire_apply", return_value={"guard_bytes_unchanged": True}
            )
        )
        self.stack.enter_context(mock.patch.object(retry, "emit"))
        self.stack.enter_context(
            mock.patch.object(retry, "run_native", side_effect=self.native)
        )
        self.stack.enter_context(
            mock.patch.object(retry, "authorize_native", side_effect=self.authorize)
        )
        self.stack.enter_context(
            mock.patch.object(retry, "_continuity_capture", side_effect=self.seed_pre)
        )
        self.stack.enter_context(
            mock.patch.object(
                retry, "_continuity_reconcile", side_effect=self.seed_post
            )
        )
        self.stack.enter_context(
            mock.patch.object(
                retry,
                "completed_execution_lock",
                side_effect=lambda *a: contextlib.nullcontext(),
            )
        )
        self.stack.enter_context(
            mock.patch.object(retry, "public_validation", return_value={"passed": True})
        )
        self.stack.enter_context(mock.patch.object(retry, "_boot_main"))
        self.stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
        self.capacity = {"evaluate": lambda _: {"passed": True, "errors": []}}

    def tearDown(self):
        self.temporary.cleanup()

    def native(
        self, argv, directory, *, phase, pass_fds=(), env=None, journal_path=None
    ):
        self.calls.append(phase)
        directory.mkdir(mode=0o700)
        if phase == self.fail_phase:
            self.fail_phase = None
            raise retry.RetryError("public injected native failure")
        if phase == "assemble":
            draft = json.loads(
                Path(argv[argv.index("--inventory-draft") + 1]).read_bytes()
            )
            retry.write_public(Path(argv[argv.index("--output") + 1]), draft)
        if phase == "preflight":
            inventory_path = Path(argv[argv.index("--inventory") + 1])
            inventory = json.loads(inventory_path.read_bytes())
            retry.write_public(
                directory / "stdout",
                {
                    "schema": "iroha.taira.public-reset.report.v1",
                    "command": "preflight",
                    "status": "ok",
                    "deployment_id": inventory["deployment_id"],
                    "revision": inventory["revision"]["commit"],
                    "inventory_sha256": retry.hashlib.sha256(
                        inventory_path.read_bytes()
                    ).hexdigest(),
                    "authorization_sha256": "9" * 64,
                },
            )
        if phase == "apply":
            attempt = directory.parent.parent
            inventory = json.loads((attempt / "assembly/inventory.json").read_bytes())
            frontier = json.loads((attempt / "apply-started.json").read_bytes())
            completed = {
                "schema": "iroha.taira.public-reset.journal.v1",
                "deployment_id": inventory["deployment_id"],
                "inventory_sha256": frontier["inventory_sha256"],
                "authorization_sha256": frontier["authorization_sha256"],
                "authorization_nonce": inventory["authorization_nonce"],
                "status": "completed",
                "phase": "completed",
                "next_step": 15,
                "recovery_intent": None,
                "touched_validators": [row["slug"] for row in inventory["validators"]],
                "edge_touched": True,
                "edge_rollback_complete": False,
                "rollback_next_validator": 0,
                "failure_summary": "",
                "rollback_failures": [],
            }
            for kind, value in (
                ("completed", completed),
                (
                    "deployment-proven",
                    dict(completed, status="sealing", phase="seal", next_step=13),
                ),
            ):
                target = self.root / "journal-v1" / kind
                target.mkdir(parents=True, exist_ok=True, mode=0o700)
                retry.write_public(target / ("9" * 64 + ".json"), value)
        return {"phase": phase, "exit_code": 0}

    def authorize(self, cli, assembly, args, plan, directory):
        self.calls.append("authorize")
        retry.write_public(
            assembly / "authorization.json", {"public_signature_fixture": True}
        )

    def seed_pre(self, args):
        retry.CONTINUITY_OUT.mkdir(mode=0o700)
        retry.write_public(
            retry.CONTINUITY_OUT / "prestart.json", {"public_fixture": True}
        )

    def seed_post(self, args):
        if self.fail_phase == "seed-post":
            self.fail_phase = None
            retry.write_public(
                retry.CONTINUITY_OUT / "startup-evidence.json",
                {"partial_public_observation": True},
            )
            raise retry.RetryError("injected postcondition failure")
        retry.write_public(
            retry.CONTINUITY_OUT / "seed-authority-receipt.json",
            {"public_fixture": True},
        )

    def test_complete_workflow_submits_one_apply_after_durable_frontier(self):
        result = retry.guest_locked(self.request, self.capacity, self.attempts)
        attempt = Path(result["private_attempt"])
        self.assertTrue(result["passed"])
        self.assertTrue((attempt / "apply-started.json").exists())
        self.assertEqual(self.calls, ["assemble", "authorize", "preflight", "apply"])
        self.assertEqual(result["completed"], list(retry.PHASES))

    def test_preapply_failure_resumes_same_identity_and_preserves_evidence(self):
        self.fail_phase = "assemble"
        with self.assertRaises(retry.RetryError):
            retry.guest_locked(self.request, self.capacity, self.attempts)
        pointer = json.loads((self.attempts / "latest.json").read_bytes())
        attempt = self.attempts / pointer["attempt_id"]
        operation = (attempt / "operation.json").read_bytes()
        self.assertFalse((attempt / "apply-started.json").exists())
        result = retry.guest_locked(self.request, self.capacity, self.attempts)
        self.assertEqual(result["attempt_id"], pointer["attempt_id"])
        self.assertEqual((attempt / "operation.json").read_bytes(), operation)
        self.assertEqual(len(list(attempt.glob("preapply-evidence-*"))), 1)
        self.assertEqual(self.calls.count("apply"), 1)

    def test_completed_apply_resumes_postconditions_without_new_nonce_or_native_calls(
        self,
    ):
        self.fail_phase = "seed-post"
        with self.assertRaises(retry.RetryError):
            retry.guest_locked(self.request, self.capacity, self.attempts)
        pointer = json.loads((self.attempts / "latest.json").read_bytes())
        attempt = self.attempts / pointer["attempt_id"]
        operation = (attempt / "operation.json").read_bytes()
        native_calls = list(self.calls)
        self.plan["capacity_plan"] = retry.postcondition_capacity_plans(
            str(self.root), "/backing"
        )["guest_plan"]
        result = retry.guest_locked(self.request, self.capacity, self.attempts)
        self.assertTrue(result["postconditions_only"])
        self.assertEqual(result["attempt_id"], pointer["attempt_id"])
        self.assertEqual((attempt / "operation.json").read_bytes(), operation)
        self.assertEqual(self.calls, native_calls)
        self.assertEqual(retry._retire_apply.call_count, 1)
        self.assertEqual(len(list(attempt.glob("postcondition-evidence-*"))), 1)
        self.assertTrue((attempt / "seed-continuity/prestart.json").exists())

    def test_completed_receipt_mismatch_blocks_postcondition_capacity_exemption(self):
        self.fail_phase = "seed-post"
        with self.assertRaises(retry.RetryError):
            retry.guest_locked(self.request, self.capacity, self.attempts)
        target = self.root / "journal-v1/completed" / ("9" * 64 + ".json")
        value = json.loads(target.read_bytes())
        value["inventory_sha256"] = "0" * 64
        target.write_text(json.dumps(value))
        with self.assertRaisesRegex(retry.RetryError, "exact completed deployment"):
            retry.previous_attempt(self.plan)
        self.assertEqual(self.calls.count("apply"), 1)

    def test_apply_frontier_blocks_resubmission_without_actual_terminal(self):
        self.fail_phase = "apply"
        with self.assertRaises(retry.RetryError):
            retry.guest_locked(self.request, self.capacity, self.attempts)
        with self.assertRaisesRegex(retry.RetryError, "pending mutation"):
            retry.guest_locked(self.request, self.capacity, self.attempts)
        self.assertEqual(self.calls.count("apply"), 1)


if __name__ == "__main__":
    unittest.main()
