"""Focused fail-closed tests for the authenticated Taira reset17 controller."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


CONTROLLER_PATH = Path(__file__).with_name("testnet_reset17_authenticated_reset.py")
SPEC = importlib.util.spec_from_file_location("taira_reset17_controller", CONTROLLER_PATH)
assert SPEC is not None and SPEC.loader is not None
controller = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = controller
SPEC.loader.exec_module(controller)


class Reset17ControllerTest(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(dir=CONTROLLER_PATH.parent)
        self.addCleanup(self._temporary.cleanup)
        self.root = Path(self._temporary.name)
        self.control = self.root / "control"
        self.control.mkdir(mode=0o700)
        self.control.chmod(0o700)
        self.library = self.root / "Library"
        self.library.mkdir(mode=0o700)
        self.launch_agents = self.library / "LaunchAgents"
        self.launch_agents.mkdir(mode=0o700)

    def _file_record(
        self, relative: str, install: str, payload: bytes = b"public\n", mode: int = 0o444
    ) -> controller.FileRecord:
        return controller.FileRecord(
            path=Path(relative),
            sha256=hashlib.sha256(payload).hexdigest(),
            size=len(payload),
            mode=mode,
            install_relative=Path(install),
        )

    def _validator(self, index: int) -> controller.ValidatorCandidate:
        label = f"org.sora.taira.user.validator-{index}"
        identities = {
            role: controller.PrivateFileIdentity(
                path=self.control / "private" / f"validator-{index}" / role,
                device=1,
                inode=index * 10 + offset,
                uid=os.geteuid(),
                mode=0o600,
                links=1,
                size=71 if role == "soracloud_runtime_signer" else 32,
                modified_ns=1,
                changed_ns=1,
            )
            for offset, role in enumerate(sorted(controller.PRIVATE_ROLES))
        }
        return controller.ValidatorCandidate(
            index=index,
            label=label,
            data_root=self.control / "data" / "reset17" / f"validator-{index}",
            torii_url=f"http://127.0.0.1:{29079 + index}",
            p2p_port=33469 + index,
            config=self._file_record(
                f"config-{index}", f"config/validator-{index}.toml"
            ),
            launch_agent=self._file_record(
                f"plist-{index}", f"launch-agents/{label}.plist"
            ),
            private_files={role: identity.path for role, identity in identities.items()},
            private_identities=identities,
            signer_public_key_hex=f"{index:064x}",
            signer_handle=f"software://taira/inrou/{index:064x}",
            signer_authority=f"validator-{index}@taira",
            signer_policy_digest_hex=f"{index + 10:064x}",
            signer_launch_path=(
                self.control
                / "run"
                / "reset17"
                / f"validator-{index}"
                / "runtime-signer.fd198"
            ),
            validator_public_key=f"validator-key-{index}",
            trusted_peer_public_keys=tuple(
                f"validator-key-{peer}" for peer in range(1, 5)
            ),
            trusted_peer_endpoints=tuple(
                (
                    f"validator-key-{peer}",
                    f"127.0.0.1:{33469 + peer}",
                )
                for peer in range(1, 5)
            ),
            trusted_pop_public_keys=tuple(
                f"validator-key-{peer}" for peer in range(1, 5)
            ),
        )

    def _candidate(self) -> controller.Candidate:
        return controller.Candidate(
            bundle=self.root,
            manifest_path=self.root / "manifest.json",
            signature_path=self.root / "manifest.sig",
            allowed_signers_path=self.root / "allowed_signers",
            manifest_sha256="1" * 64,
            signature_sha256="2" * 64,
            allowed_signers_sha256="3" * 64,
            raw={},
            release_id="reset17-unit",
            source_commit="4" * 40,
            release_dir=self.control / "releases" / "reset17-unit",
            launch_agents_dir=self.launch_agents,
            control_root=self.control,
            python_path=Path(sys.executable),
            python_sha256="5" * 64,
            python_size=1,
            require_single_data_volume=True,
            artifacts={
                "fixture": self._file_record(
                    "artifact", "bin/artifact", payload=b"binary", mode=0o555
                )
            },
            validators=tuple(self._validator(index) for index in range(1, 5)),
            network_id="6" * 64,
        )

    def _write_validator_config(
        self,
        candidate: controller.Candidate,
        validator: controller.ValidatorCandidate,
        *,
        lane_index: int = controller.BPNG_LANE_ID,
        lane_alias: str = controller.BPNG_LANE_ALIAS,
        lane_dataspace: str = controller.BPNG_PHYSICAL_DATASPACE_ALIAS,
        dataspace_id: int = controller.BPNG_PHYSICAL_DATASPACE_ID,
        dataspace_alias: str = controller.BPNG_PHYSICAL_DATASPACE_ALIAS,
        route_lane: int = controller.BPNG_LANE_ID,
        route_dataspace: str = controller.BPNG_PHYSICAL_DATASPACE_ALIAS,
        peer_keys: tuple[str, ...] | None = None,
        pop_keys: tuple[str, ...] | None = None,
    ) -> tuple[Path, dict[str, object], Path]:
        peer_keys = peer_keys or tuple(f"validator-key-{index}" for index in range(1, 5))
        pop_keys = pop_keys or tuple(f"validator-key-{index}" for index in range(1, 5))
        signed_genesis = candidate.control_root / "signed-genesis.scale"
        runtime_signer: dict[str, object] = {
            "handle": validator.signer_handle,
            "authority": validator.signer_authority,
            "public_key_hex": validator.signer_public_key_hex,
            "policy_digest_hex": validator.signer_policy_digest_hex,
        }
        trusted_peers = json.dumps(
            [f"{key}@127.0.0.1:{33470 + offset}" for offset, key in enumerate(peer_keys)]
        )
        trusted_pop = "\n".join(
            (
                "[[trusted_peers_pop]]\n"
                f"public_key = {json.dumps(key)}\n"
                'pop_hex = "aa"'
            )
            for key in pop_keys
        )
        private = validator.private_files
        payload = f"""
chain = {json.dumps(controller.TAIRA_CHAIN_ID)}
chain_discriminant = {controller.TAIRA_CHAIN_DISCRIMINANT}
public_key = {json.dumps(validator.validator_public_key)}
private_key_file = {json.dumps(str(private['validator_signer']))}
soranet_transport_private_key_file = {json.dumps(str(private['soranet_transport']))}
trusted_peers = {trusted_peers}

{trusted_pop}

[sumeragi]
role = "validator"

[torii]
address = "127.0.0.1:{29079 + validator.index}"

[torii.faucet]
private_key_file = {json.dumps(str(private['faucet_signer']))}

[network]
address = "127.0.0.1:{validator.p2p_port}"

[streaming]
identity_private_key_file = {json.dumps(str(private['streaming_identity']))}

[genesis]
file = {json.dumps(str(signed_genesis))}

[kura]
store_dir = {json.dumps(str(validator.data_root / 'kura'))}

[snapshot]
store_dir = {json.dumps(str(validator.data_root / 'snapshots'))}

[nexus.storage]
local_budget_bytes = {controller.TAIRA_NEXUS_LOCAL_BUDGET_BYTES}

[nexus.storage.disk_budget_weights]
kura_blocks_bps = 5500
wsv_snapshots_bps = 2000
sorafs_bps = 2000
soranet_spool_bps = 250
soravpn_spool_bps = 250

[[nexus.lane_catalog]]
index = {lane_index}
alias = {json.dumps(lane_alias)}
dataspace = {json.dumps(lane_dataspace)}

[[nexus.dataspace_catalog]]
id = {dataspace_id}
alias = {json.dumps(dataspace_alias)}

[[nexus.routing_policy.rules]]
lane = {route_lane}
dataspace = {json.dumps(route_dataspace)}
[nexus.routing_policy.rules.matcher]
account = "*@bpng"

[[nexus.routing_policy.rules]]
lane = {route_lane}
dataspace = {json.dumps(route_dataspace)}
[nexus.routing_policy.rules.matcher]
account = "*@mibank.bpng"

[sorafs.storage]
enabled = false
max_capacity_bytes = {controller.TAIRA_SORAFS_CAPACITY_BYTES}

[soracloud_runtime]
production_mode = true
state_dir = {json.dumps(str(validator.data_root / 'soracloud'))}

[soracloud_runtime.inrou]
enabled = false
backends = []

[soracloud_runtime.submission.signer]
handle = {json.dumps(validator.signer_handle)}
authority = {json.dumps(validator.signer_authority)}
algorithm = "ed25519"
public_key_hex = {json.dumps(validator.signer_public_key_hex)}
revision = {controller.TAIRA_RUNTIME_SIGNER_REVISION}
policy_digest_hex = {json.dumps(validator.signer_policy_digest_hex)}
""".lstrip()
        path = validator.config.source(candidate.bundle)
        path.write_text(payload, encoding="utf-8")
        return path, runtime_signer, signed_genesis

    @staticmethod
    def _health_sample(
        heights: tuple[int, int, int, int],
        queues: tuple[int, int, int, int] = (0, 0, 0, 0),
    ) -> dict[str, object]:
        validators = [
            {"index": index, "height": height, "queue_size": queue}
            for index, (height, queue) in enumerate(zip(heights, queues), start=1)
        ]
        return {
            "validators": validators,
            "minimum_height": min(heights),
            "maximum_height": max(heights),
        }

    def test_bpng_profile_requires_public_alias_raw_id_domain_and_route(self) -> None:
        valid = {
            "asset_alias": "kina#bpng",
            "asset_definition": "839FV3NJC8NfgWQvghXU2hEFQm9a",
            "asset_domain": "bpng.bpng",
            "scale": 2,
            "lane_id": 3,
            "lane_alias": "dpn",
            "physical_dataspace_id": 10,
            "physical_dataspace_alias": "bpng",
        }
        controller._validate_bpng(valid)
        for key, replacement in (
            ("asset_alias", "digital_kina#bpng"),
            ("asset_definition", "kina#bpng"),
            ("asset_domain", "bpng"),
            ("scale", 18),
            ("lane_alias", "payments"),
        ):
            mutated = dict(valid)
            mutated[key] = replacement
            with self.subTest(key=key), self.assertRaises(controller.Reset17Error):
                controller._validate_bpng(mutated)

    def test_validator_config_binds_exact_bpng_lane_dataspace_and_routes(self) -> None:
        candidate = self._candidate()
        validator = candidate.validators[0]
        _path, signer, signed_genesis = self._write_validator_config(
            candidate, validator
        )
        public_key, trusted_peers, trusted_endpoints, trusted_pop = (
            controller._validate_config(
                validator.config,
                candidate.bundle,
                validator.data_root,
                validator.torii_url,
                validator.p2p_port,
                validator.private_files,
                signer,
                signed_genesis,
            )
        )
        expected_keys = tuple(f"validator-key-{index}" for index in range(1, 5))
        self.assertEqual(public_key, validator.validator_public_key)
        self.assertEqual(trusted_peers, expected_keys)
        self.assertEqual(
            trusted_endpoints,
            tuple(
                (key, f"127.0.0.1:{33470 + offset}")
                for offset, key in enumerate(expected_keys)
            ),
        )
        self.assertEqual(trusted_pop, expected_keys)

        mutations = (
            ({"lane_alias": "payments"}, "BPNG DPN lane binding"),
            ({"dataspace_id": 11}, "physical BPNG dataspace"),
            ({"route_lane": 2}, "route BPNG/MiBank to lane 3"),
            ({"route_dataspace": "other"}, "route BPNG/MiBank to lane 3"),
        )
        for arguments, message in mutations:
            with self.subTest(arguments=arguments):
                self._write_validator_config(candidate, validator, **arguments)
                with self.assertRaisesRegex(controller.Reset17Error, message):
                    controller._validate_config(
                        validator.config,
                        candidate.bundle,
                        validator.data_root,
                        validator.torii_url,
                        validator.p2p_port,
                        validator.private_files,
                        signer,
                        signed_genesis,
                    )

    def test_validator_config_rejects_duplicate_peer_or_pop_topology(self) -> None:
        candidate = self._candidate()
        validator = candidate.validators[0]
        canonical = tuple(f"validator-key-{index}" for index in range(1, 5))
        duplicate = (canonical[0], canonical[0], canonical[2], canonical[3])
        for argument in ("peer_keys", "pop_keys"):
            with self.subTest(argument=argument):
                _path, signer, signed_genesis = self._write_validator_config(
                    candidate, validator, **{argument: duplicate}
                )
                with self.assertRaisesRegex(
                    controller.Reset17Error, "topology contains duplicates"
                ):
                    controller._validate_config(
                        validator.config,
                        candidate.bundle,
                        validator.data_root,
                        validator.torii_url,
                        validator.p2p_port,
                        validator.private_files,
                        signer,
                        signed_genesis,
                    )

    def test_candidate_rejects_correct_peer_keys_bound_to_a_foreign_endpoint(self) -> None:
        release_id = "reset17-topology"
        release_dir = self.control / "releases" / release_id
        executable = {
            "iroha3d_taira",
            "kagami",
            "iroha",
            "taira_operator_status",
        }

        def record(name: str, install: str, mode: int) -> dict[str, object]:
            return {
                "path": name,
                "sha256": hashlib.sha256(name.encode("utf-8")).hexdigest(),
                "size": 1,
                "mode": mode,
                "install_relative": install,
            }

        artifacts = {
            name: record(
                f"artifact-{name}",
                f"artifacts/{name}",
                0o555 if name in executable else 0o444,
            )
            for name in controller.REQUIRED_ARTIFACTS
        }
        validators = [
            {
                "index": index,
                "label": f"org.sora.taira.user.validator-{index}",
                "data_root": str(
                    self.control / "data" / controller.GENERATION / f"validator-{index}"
                ),
                "torii_url": f"http://127.0.0.1:{29079 + index}",
                "p2p_port": 33469 + index,
                "config": record(
                    f"config-{index}", f"config/validator-{index}.toml", 0o444
                ),
                "launch_agent": record(
                    f"plist-{index}",
                    (
                        "launch-agents/"
                        f"org.sora.taira.user.validator-{index}.plist"
                    ),
                    0o444,
                ),
                "private_files": {},
                "runtime_signer": {},
            }
            for index in range(1, 5)
        ]
        raw = {
            "schema": controller.MANIFEST_SCHEMA,
            "generation": controller.GENERATION,
            "release_id": release_id,
            "network_id": "6" * 64,
            "source": {},
            "protocols": {},
            "bpng": {},
            "deployment": {},
            "artifacts": artifacts,
            "validators": validators,
        }
        payload = controller.canonical_json_bytes(raw)
        fixture_validators = {
            index: self._validator(index) for index in range(1, 5)
        }

        def private_files(
            _value: object, index: int
        ) -> tuple[
            dict[str, Path], dict[str, controller.PrivateFileIdentity]
        ]:
            validator = fixture_validators[index]
            return dict(validator.private_files), dict(validator.private_identities)

        def signer(
            _value: object, _private_files: object, index: int
        ) -> dict[str, object]:
            validator = fixture_validators[index]
            return {
                "source_path": validator.private_files["soracloud_runtime_signer"],
                "launch_path": validator.signer_launch_path,
                "public_key_hex": validator.signer_public_key_hex,
                "handle": validator.signer_handle,
                "authority": validator.signer_authority,
                "algorithm": "ed25519",
                "revision": controller.TAIRA_RUNTIME_SIGNER_REVISION,
                "policy_digest_hex": validator.signer_policy_digest_hex,
            }

        expected_roster = tuple(f"validator-key-{index}" for index in range(1, 5))

        def config_topology(
            config_record: controller.FileRecord, *_arguments: object
        ) -> tuple[
            str,
            tuple[str, ...],
            tuple[tuple[str, str], ...],
            tuple[str, ...],
        ]:
            index = int(config_record.install_relative.stem.rsplit("-", 1)[1])
            endpoints = tuple(
                (key, f"127.0.0.1:{33470 + offset}")
                for offset, key in enumerate(expected_roster)
            )
            if index == 4:
                endpoints = endpoints[:-1] + (
                    (expected_roster[-1], "127.0.0.1:44444"),
                )
            return (
                f"validator-key-{index}",
                expected_roster,
                endpoints,
                expected_roster,
            )

        deployment = {
            "control_root": self.control,
            "release_dir": release_dir,
            "launch_agents_dir": self.launch_agents,
            "python_path": Path(sys.executable),
            "python_sha256": "5" * 64,
            "python_size": 1,
            "require_single_data_volume": True,
        }
        with mock.patch.object(
            controller,
            "verify_manifest_signature",
            return_value=(payload, "2" * 64),
        ), mock.patch.object(
            controller, "_parse_source", return_value="4" * 40
        ), mock.patch.object(
            controller, "_validate_protocols"
        ), mock.patch.object(
            controller, "_validate_bpng"
        ), mock.patch.object(
            controller, "_parse_deployment", return_value=deployment
        ), mock.patch.object(
            controller, "_verify_external_public_file"
        ), mock.patch.object(
            controller, "_verify_file"
        ), mock.patch.object(
            controller, "_verify_macho_arm64"
        ), mock.patch.object(
            controller, "_parse_private_files", side_effect=private_files
        ), mock.patch.object(
            controller, "_parse_runtime_signer", side_effect=signer
        ), mock.patch.object(
            controller, "_validate_owner_private_ancestors"
        ), mock.patch.object(
            controller, "_validate_config", side_effect=config_topology
        ), self.assertRaisesRegex(
            controller.Reset17Error, "one exact four-peer topology"
        ):
            controller.load_candidate(
                bundle=self.root,
                manifest_path=self.root / "manifest.json",
                signature_path=self.root / "manifest.sig",
                allowed_signers_path=self.root / "allowed-signers",
                expected_manifest_sha256="1" * 64,
                expected_allowed_signers_sha256="3" * 64,
                expected_source_commit="4" * 40,
                expected_control_root=self.control,
                expected_launch_agents_dir=self.launch_agents,
                run_offline_checks=False,
            )

    def test_paths_are_lexically_normal_and_containment_is_strict(self) -> None:
        for value in ("/tmp/a/./b", "/tmp/a//b", "/tmp/a/../b"):
            with self.subTest(value=value), self.assertRaises(controller.Reset17Error):
                controller._absolute_path(value, "path")
        for value in ("a/./b", "a//b", "a/../b"):
            with self.subTest(value=value), self.assertRaises(controller.Reset17Error):
                controller._relative_path(value, "path")
        self.assertFalse(controller._is_below(self.control, self.control))
        self.assertTrue(controller._is_below(self.control / "child", self.control))

    def test_source_build_commands_are_exact_and_ordered(self) -> None:
        source = {
            "commit": "1" * 40,
            "tree": "2" * 40,
            "parent": "3" * 40,
            "commit_signer_fingerprint": controller.EXPECTED_COMMIT_SIGNER_FINGERPRINT,
            "source_date_epoch": 1,
            "cargo_target_dir": "/private/tmp/taira-reset17-authenticated-target",
            "rustc_version": "rustc fixture",
            "cargo_version": "cargo fixture",
            "build_commands": [list(command) for command in controller.EXPECTED_BUILD_COMMANDS],
        }
        self.assertEqual(controller._parse_source(source, "1" * 40), "1" * 40)
        source["build_commands"].append(["cargo", "build"])
        with self.assertRaisesRegex(controller.Reset17Error, "exact reviewed sequence"):
            controller._parse_source(source, "1" * 40)

    def test_signature_verifier_uses_pinned_open_descriptors(self) -> None:
        manifest = controller.canonical_json_bytes({"schema": "fixture"})
        allowed = b"release ssh-ed25519 AAAAfixture\n"
        signature = b"-----BEGIN SSH SIGNATURE-----\nfixture\n-----END SSH SIGNATURE-----\n"
        paths = []
        for name, payload in (
            ("manifest.json", manifest),
            ("allowed", allowed),
            ("signature", signature),
        ):
            path = self.root / name
            path.write_bytes(payload)
            paths.append(path)

        def fake_run(command: tuple[str, ...], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(str(command[4]).startswith("/dev/fd/"))
            self.assertTrue(str(command[-1]).startswith("/dev/fd/"))
            allowed_fd, signature_fd = kwargs["pass_fds"]  # type: ignore[index]
            self.assertEqual(os.read(allowed_fd, len(allowed)), allowed)
            self.assertEqual(os.read(signature_fd, len(signature)), signature)
            return subprocess.CompletedProcess(command, 0, b"", b"")

        with mock.patch.object(controller.subprocess, "run", side_effect=fake_run):
            observed, signature_digest = controller.verify_manifest_signature(
                paths[0],
                paths[2],
                paths[1],
                hashlib.sha256(manifest).hexdigest(),
                hashlib.sha256(allowed).hexdigest(),
            )
        self.assertEqual(observed, manifest)
        self.assertEqual(signature_digest, hashlib.sha256(signature).hexdigest())

    def test_runtime_signer_rejects_boolean_revision(self) -> None:
        private = self.root / "private"
        private.mkdir(mode=0o700)
        source = private / "signer"
        source.write_bytes(b"x" * 71)
        source.chmod(0o600)
        signer = {
            "source_path": str(source),
            "launch_path": str(private / "launch"),
            "public_key_hex": "a" * 64,
            "handle": f"software://taira/inrou/{'a' * 64}",
            "authority": "validator@taira",
            "algorithm": "ed25519",
            "revision": True,
            "policy_digest_hex": "b" * 64,
        }
        with self.assertRaisesRegex(controller.Reset17Error, "must be an integer"):
            controller._parse_runtime_signer(
                signer, {"soracloud_runtime_signer": source}, 1
            )

    def test_storage_gate_reserves_four_budgets_and_exact_copy_bytes(self) -> None:
        candidate = self._candidate()
        capacity = 400 * 1024**3
        copy_bytes = sum(
            record.size for _role, record in controller._candidate_public_records(candidate)
        ) + sum(item.launch_agent.size for item in candidate.validators)
        reserve = max(controller.MIN_FREE_RESERVE_BYTES, capacity // 10)
        required = (
            4 * controller.TAIRA_NEXUS_LOCAL_BUDGET_BYTES + reserve + copy_bytes
        )

        with mock.patch.object(
            controller,
            "_volume_identity",
            return_value=(7, capacity, required, self.control),
        ):
            planned = controller.storage_plan(candidate, [])
        self.assertEqual(planned["data_device"], 7)
        self.assertEqual(planned["groups"][0]["required_available_bytes"], required)

        with mock.patch.object(
            controller,
            "_volume_identity",
            return_value=(7, capacity, required - 1, self.control),
        ), self.assertRaisesRegex(controller.Reset17Error, "short by 1 bytes"):
            controller.storage_plan(candidate, [])

    def test_health_wait_requires_convergence_empty_queues_and_new_height(self) -> None:
        candidate = self._candidate()
        nonconverged = self._health_sample((10, 7, 7, 7))
        queued = self._health_sample((10, 10, 10, 10), (1, 0, 0, 0))
        initial = self._health_sample((10, 10, 10, 10))
        progressed = self._health_sample((11, 11, 11, 11))
        with mock.patch.object(
            controller,
            "_status_snapshot",
            side_effect=(nonconverged, queued, initial, progressed),
        ), mock.patch.object(
            controller.time, "monotonic", side_effect=(0, 0, 1, 2, 3)
        ), mock.patch.object(controller.time, "sleep") as sleep:
            result = controller.wait_for_four_peer_progress(candidate, 15)
        self.assertEqual(result, {"initial": initial, "progressed": progressed})
        self.assertEqual(sleep.call_count, 3)

    def test_health_wait_rejects_a_converged_but_stalled_cluster(self) -> None:
        candidate = self._candidate()
        stalled = self._health_sample((12, 12, 12, 12))
        with mock.patch.object(
            controller, "_status_snapshot", side_effect=(stalled, stalled, stalled)
        ), mock.patch.object(
            controller.time, "monotonic", side_effect=(0, 0, 5, 10, 15)
        ), mock.patch.object(controller.time, "sleep") as sleep:
            with self.assertRaisesRegex(
                controller.Reset17Error, "did not converge and progress"
            ):
                controller.wait_for_four_peer_progress(candidate, 15)
        self.assertEqual(sleep.call_count, 3)

    def test_status_snapshot_requires_both_readiness_endpoints(self) -> None:
        candidate = self._candidate()

        def endpoint(
            _origin: str, path: str, _timeout_seconds: float
        ) -> tuple[int, bytes]:
            if path == "/readyz":
                return 503, b"not ready"
            return 200, b"{}"

        with mock.patch.object(controller, "_torii_get", side_effect=endpoint):
            with self.assertRaisesRegex(
                controller.Reset17Error, "readiness endpoint is unavailable"
            ):
                controller._status_snapshot(candidate)

    def test_status_snapshot_rejects_restart_and_liveness_blockers(self) -> None:
        candidate = self._candidate()
        for blocker in (
            {"restart_required": True},
            {"liveness_blocker": "consensus admission is closed"},
        ):
            with self.subTest(blocker=blocker):
                status = {
                    "blocks": 8,
                    "queue_size": 0,
                    "network_id": candidate.network_id,
                    **blocker,
                }

                def endpoint(
                    _origin: str, path: str, _timeout_seconds: float
                ) -> tuple[int, bytes]:
                    return (
                        (200, json.dumps(status).encode("utf-8"))
                        if path == "/status"
                        else (200, b"ready")
                    )

                with mock.patch.object(controller, "_torii_get", side_effect=endpoint):
                    with self.assertRaisesRegex(
                        controller.Reset17Error, "reports a liveness blocker"
                    ):
                        controller._status_snapshot(candidate)

    def test_plan_is_deterministic_and_carries_kina_alias(self) -> None:
        candidate = self._candidate()
        capacity = 500 * 1024**3
        with mock.patch.object(
            controller,
            "_volume_identity",
            return_value=(9, capacity, capacity, self.control),
        ):
            first = controller.plan_bytes(candidate)
            second = controller.plan_bytes(candidate)
        self.assertEqual(first, second)
        parsed = json.loads(first)
        self.assertEqual(parsed["bpng"]["asset_alias"], "kina#bpng")
        self.assertEqual(parsed["bpng"]["asset_domain"], "bpng.bpng")

    def test_plan_tamper_is_rejected_even_when_tampered_digest_is_pinned(self) -> None:
        candidate = self._candidate()
        capacity = 500 * 1024**3
        plan_path = self.root / "plan.json"
        with mock.patch.object(
            controller,
            "_volume_identity",
            return_value=(9, capacity, capacity, self.control),
        ):
            plan = json.loads(controller.plan_bytes(candidate))
            plan["bpng"]["asset_alias"] = "other#bpng"
            tampered = controller.canonical_json_bytes(plan)
            plan_path.write_bytes(tampered)
            with self.assertRaisesRegex(controller.Reset17Error, "stale or does not match"):
                controller._read_and_check_plan(
                    candidate, plan_path, hashlib.sha256(tampered).hexdigest()
                )

    def test_predecessor_snapshot_is_immutable_and_reused_without_requery(self) -> None:
        candidate = self._candidate()
        old_payloads: dict[int, bytes] = {}
        for validator in candidate.validators:
            payload = f"predecessor-{validator.index}\n".encode("utf-8")
            old_payloads[validator.index] = payload
            target = candidate.launch_agents_dir / f"{validator.label}.plist"
            target.write_bytes(payload)
            target.chmod(0o644)
        loaded = {
            candidate.validators[0].label: True,
            candidate.validators[1].label: False,
            candidate.validators[2].label: True,
            candidate.validators[3].label: False,
        }
        with mock.patch.object(
            controller, "_service_is_loaded", side_effect=lambda label: loaded[label]
        ) as service_query:
            predecessor = controller.predecessor_plan(candidate)
        self.assertEqual(service_query.call_count, 4)
        for entry in predecessor:
            payload = old_payloads[entry["index"]]
            self.assertTrue(entry["present"])
            self.assertEqual(entry["sha256"], hashlib.sha256(payload).hexdigest())
            self.assertEqual(entry["loaded"], loaded[entry["label"]])

        controller._prepare_runtime_directories(candidate)
        controller._persist_predecessor_snapshot(candidate, predecessor)
        snapshot_path = controller._predecessor_snapshot_path(candidate)
        self.assertEqual(os.stat(snapshot_path).st_mode & 0o777, 0o400)
        snapshot = json.loads(snapshot_path.read_bytes())
        self.assertEqual(snapshot["schema"], controller.PREDECESSOR_SCHEMA)
        self.assertEqual(snapshot["predecessor"], predecessor)

        with mock.patch.object(controller, "_service_is_loaded") as service_query:
            reused = controller.predecessor_plan(candidate)
        service_query.assert_not_called()
        self.assertEqual(reused, predecessor)

    def test_rollback_restores_predecessors_removes_new_files_and_reloads_exactly(self) -> None:
        candidate = self._candidate()
        backup_root = (
            candidate.control_root
            / "backups"
            / candidate.release_id
            / "launch-agents"
        )
        backup_root.mkdir(parents=True, mode=0o700)
        installs: list[controller.LaunchAgentInstall] = []
        old_payloads: dict[int, bytes] = {}
        for validator in candidate.validators:
            target = candidate.launch_agents_dir / f"{validator.label}.plist"
            backup = backup_root / f"{validator.label}.plist.predecessor"
            target.write_bytes(f"reset17-{validator.index}\n".encode("utf-8"))
            target.chmod(0o644)
            had_predecessor = validator.index in (1, 2)
            if had_predecessor:
                old = f"reset16-{validator.index}\n".encode("utf-8")
                old_payloads[validator.index] = old
                backup.write_bytes(old)
                backup.chmod(0o644)
            installs.append(
                controller.LaunchAgentInstall(
                    validator=validator,
                    target=target,
                    backup=backup,
                    had_predecessor=had_predecessor,
                    was_already_desired=validator.index == 4,
                    predecessor_was_loaded=validator.index == 1,
                )
            )

        with mock.patch.object(controller, "_run_launchctl") as launchctl:
            self.assertTrue(controller._restore_launch_agents(candidate, installs))
        for index in (1, 2):
            self.assertEqual(installs[index - 1].target.read_bytes(), old_payloads[index])
        self.assertFalse(installs[2].target.exists())
        self.assertEqual(installs[3].target.read_bytes(), b"reset17-4\n")
        bootstraps = [
            call
            for call in launchctl.call_args_list
            if call.args[0][0] == "bootstrap"
        ]
        self.assertEqual(len(bootstraps), 1)
        self.assertEqual(bootstraps[0].args[0][-1], str(installs[0].target))

    def test_retry_rollback_reloads_an_already_desired_service_that_was_loaded(self) -> None:
        candidate = self._candidate()
        validator = candidate.validators[0]
        target = candidate.launch_agents_dir / f"{validator.label}.plist"
        target.write_bytes(b"public\n")
        target.chmod(0o644)
        with mock.patch.object(controller, "_service_is_loaded", return_value=True):
            predecessor = controller.predecessor_plan(candidate)
        entry = predecessor[0]
        self.assertFalse(entry["present"])
        self.assertTrue(entry["loaded"])

        install = controller.LaunchAgentInstall(
            validator=validator,
            target=target,
            backup=(
                candidate.control_root
                / "backups"
                / candidate.release_id
                / "launch-agents"
                / f"{validator.label}.plist.predecessor"
            ),
            had_predecessor=False,
            was_already_desired=True,
            predecessor_was_loaded=True,
        )
        with mock.patch.object(controller, "_run_launchctl") as launchctl:
            self.assertTrue(controller._restore_launch_agents(candidate, [install]))
        self.assertEqual(target.read_bytes(), b"public\n")
        bootstraps = [
            call
            for call in launchctl.call_args_list
            if call.args[0][0] == "bootstrap"
        ]
        self.assertEqual(len(bootstraps), 1)
        self.assertEqual(bootstraps[0].args[0][-1], str(target))

    def test_rollback_marks_raw_filesystem_error_incomplete_and_continues(self) -> None:
        candidate = self._candidate()
        backup_root = (
            candidate.control_root
            / "backups"
            / candidate.release_id
            / "launch-agents"
        )
        backup_root.mkdir(parents=True, mode=0o700)
        installs: list[controller.LaunchAgentInstall] = []
        expected: dict[Path, bytes] = {}
        for validator in candidate.validators[:2]:
            target = candidate.launch_agents_dir / f"{validator.label}.plist"
            backup = backup_root / f"{validator.label}.plist.predecessor"
            target.write_bytes(f"reset17-{validator.index}\n".encode("utf-8"))
            target.chmod(0o644)
            payload = f"reset16-{validator.index}\n".encode("utf-8")
            backup.write_bytes(payload)
            backup.chmod(0o644)
            expected[target] = payload
            installs.append(
                controller.LaunchAgentInstall(
                    validator=validator,
                    target=target,
                    backup=backup,
                    had_predecessor=True,
                    was_already_desired=False,
                    predecessor_was_loaded=False,
                )
            )

        original_replace = os.replace

        def replace_with_first_failure(source: Path, destination: Path) -> None:
            if destination == installs[0].target:
                raise OSError("injected rollback replace fault")
            original_replace(source, destination)

        with mock.patch.object(controller, "_run_launchctl"), mock.patch.object(
            controller.os, "replace", side_effect=replace_with_first_failure
        ) as replace:
            self.assertFalse(controller._restore_launch_agents(candidate, installs))
        self.assertEqual(replace.call_count, 2)
        self.assertEqual(installs[0].target.read_bytes(), b"reset17-1\n")
        self.assertEqual(installs[1].target.read_bytes(), expected[installs[1].target])

    def test_apply_preflight_failure_causes_no_deployment_mutation(self) -> None:
        candidate = self._candidate()
        digest = "a" * 64
        mutations = (
            "_stage_immutable_release",
            "_prepare_runtime_directories",
            "_persist_predecessor_snapshot",
            "_install_launch_agents",
            "_bootout_services",
            "_bootstrap_services",
        )
        with contextlib.ExitStack() as stack:
            mutation_mocks = [
                stack.enter_context(mock.patch.object(controller, name))
                for name in mutations
            ]
            stack.enter_context(
                mock.patch.object(
                    controller,
                    "_read_and_check_plan",
                    side_effect=controller.Reset17Error(
                        "injected preflight rejection"
                    ),
                )
            )
            with self.assertRaisesRegex(controller.Reset17Error, "injected preflight"):
                controller.apply_candidate(
                    candidate,
                    plan_path=self.root / "plan.json",
                    expected_plan_sha256=digest,
                    confirmation=f"{controller.PLAN_CONFIRMATION_PREFIX}{digest}",
                    result_path=(
                        candidate.control_root
                        / "results"
                        / f"{candidate.release_id}.json"
                    ),
                    health_timeout_seconds=15,
                )
        for mutation in mutation_mocks:
            mutation.assert_not_called()

    def test_apply_fault_after_install_invokes_rollback_and_failure_receipt(self) -> None:
        candidate = self._candidate()
        digest = "b" * 64
        installed = mock.sentinel.installed

        def inject_install(
            _candidate: controller.Candidate,
            _predecessors: object,
            installs: list[object],
        ) -> list[object]:
            installs.append(installed)
            return installs

        with mock.patch.object(
            controller, "_read_and_check_plan", return_value=({"predecessor": []}, b"")
        ), mock.patch.object(controller, "_stage_immutable_release"), mock.patch.object(
            controller, "_prepare_runtime_directories"
        ), mock.patch.object(
            controller, "_persist_predecessor_snapshot"
        ), mock.patch.object(
            controller, "_install_launch_agents", side_effect=inject_install
        ), mock.patch.object(
            controller, "_bootout_services"
        ), mock.patch.object(
            controller,
            "_bootstrap_services",
            side_effect=controller.Reset17Error("injected bootstrap fault"),
        ), mock.patch.object(
            controller, "_restore_launch_agents", return_value=True
        ) as restore, mock.patch.object(
            controller, "_write_failure_receipt"
        ) as failure_receipt, self.assertRaisesRegex(
            controller.Reset17Error, "injected bootstrap fault"
        ):
            controller.apply_candidate(
                candidate,
                plan_path=self.root / "plan.json",
                expected_plan_sha256=digest,
                confirmation=f"{controller.PLAN_CONFIRMATION_PREFIX}{digest}",
                result_path=(
                    candidate.control_root
                    / "results"
                    / f"{candidate.release_id}.json"
                ),
                health_timeout_seconds=15,
            )
        restore.assert_called_once_with(candidate, [installed])
        failure_receipt.assert_called_once_with(candidate, digest, True)

    def test_success_receipt_write_failure_rolls_back_and_records_failure(self) -> None:
        candidate = self._candidate()
        digest = "c" * 64
        installed = mock.sentinel.installed

        def inject_install(
            _candidate: controller.Candidate,
            _predecessors: object,
            installs: list[object],
        ) -> list[object]:
            installs.append(installed)
            return installs

        health = {
            "initial": self._health_sample((20, 20, 20, 20)),
            "progressed": self._health_sample((21, 21, 21, 21)),
        }
        with mock.patch.object(
            controller, "_read_and_check_plan", return_value=({"predecessor": []}, b"")
        ), mock.patch.object(controller, "_stage_immutable_release"), mock.patch.object(
            controller, "_prepare_runtime_directories"
        ), mock.patch.object(
            controller, "_persist_predecessor_snapshot"
        ), mock.patch.object(
            controller, "_install_launch_agents", side_effect=inject_install
        ), mock.patch.object(
            controller, "_bootout_services"
        ), mock.patch.object(
            controller, "_bootstrap_services"
        ), mock.patch.object(
            controller, "wait_for_four_peer_progress", return_value=health
        ), mock.patch.object(
            controller,
            "_write_canonical_file",
            side_effect=controller.Reset17Error("injected receipt fsync fault"),
        ), mock.patch.object(
            controller, "_restore_launch_agents", return_value=True
        ) as restore, mock.patch.object(
            controller, "_write_failure_receipt"
        ) as failure_receipt, self.assertRaisesRegex(
            controller.Reset17Error, "injected receipt fsync fault"
        ):
            controller.apply_candidate(
                candidate,
                plan_path=self.root / "plan.json",
                expected_plan_sha256=digest,
                confirmation=f"{controller.PLAN_CONFIRMATION_PREFIX}{digest}",
                result_path=(
                    candidate.control_root
                    / "results"
                    / f"{candidate.release_id}.json"
                ),
                health_timeout_seconds=15,
            )
        restore.assert_called_once_with(candidate, [installed])
        failure_receipt.assert_called_once_with(candidate, digest, True)

    def test_wrong_apply_confirmation_causes_no_deployment_mutation(self) -> None:
        candidate = self._candidate()
        with mock.patch.object(controller, "_stage_immutable_release") as stage:
            with self.assertRaisesRegex(controller.Reset17Error, "confirmation"):
                controller.apply_candidate(
                    candidate,
                    plan_path=self.root / "plan.json",
                    expected_plan_sha256="a" * 64,
                    confirmation="wrong",
                    result_path=(
                        candidate.control_root
                        / "results"
                        / f"{candidate.release_id}.json"
                    ),
                    health_timeout_seconds=15,
                )
        stage.assert_not_called()

    def test_launch_agent_rejects_unexpected_keys(self) -> None:
        candidate = self._candidate()
        validator = candidate.validators[0]
        payload = plistlib.dumps({"Label": validator.label, "Sockets": {}})
        source = validator.launch_agent.source(candidate.bundle)
        source.write_bytes(payload)
        with self.assertRaisesRegex(controller.Reset17Error, "missing or unexpected"):
            controller._validate_launch_agent(
                validator.launch_agent, candidate.bundle, validator, candidate
            )

    def test_cli_exposes_no_force_skip_or_no_health_bypass(self) -> None:
        help_text = controller.build_parser().format_help()
        self.assertNotIn("--force", help_text)
        self.assertNotIn("--skip", help_text)
        self.assertNotIn("--no-health", help_text)


if __name__ == "__main__":
    unittest.main()
