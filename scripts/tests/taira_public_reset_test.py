"""Safety tests for the offline public Taira reset handoff validator."""

from __future__ import annotations

import ast
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "taira_public_reset.py"
SPEC = importlib.util.spec_from_file_location("taira_public_reset", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
sys.path.insert(0, str(MODULE_PATH.parent))
try:
    SPEC.loader.exec_module(MODULE)
finally:
    sys.path.remove(str(MODULE_PATH.parent))

GIT_HEAD, GIT_TREE, GIT_ENTRIES = MODULE._git_snapshot(REPO_ROOT)
TRACKED_TREE_SHA256 = hashlib.sha256(
    MODULE.SOURCE_DOMAIN + MODULE.canonical_json_bytes(list(GIT_ENTRIES))
).hexdigest()


def sha256_bytes(body: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``body``."""

    return hashlib.sha256(body).hexdigest()


def write_file(path: Path, body: bytes, mode: int) -> Path:
    """Create one fixture file with an exact mode."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    path.chmod(mode)
    return path


def artifact(
    role: str,
    source: Path,
    install: str,
    mode: int,
    maximum: int | None = None,
) -> dict[str, object]:
    """Build one staged-artifact inventory entry."""

    body = source.read_bytes()
    return {
        "role": role,
        "source_path": str(source),
        "install_path": install,
        "sha256": sha256_bytes(body),
        "mode": f"0{mode:o}",
        "max_bytes": len(body) if maximum is None else maximum,
    }


class InventoryFixture:
    """Create a complete valid external operator inventory."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.now = int(time.time())
        self.previous = "a" * 64
        self.genesis = "b" * 64
        self.commit = GIT_HEAD
        self.head_tree = GIT_TREE
        self.nonce = "d" * 64
        self.not_before = self.now - 30
        self.expires_at = self.now + 600
        self.source: dict[str, object] = {}
        self.refresh_source()

        artifact_root = root / "artifacts"
        common = {
            "binary": write_file(
                artifact_root / "iroha3d_taira", b"validator-binary\n", 0o755
            ),
            "genesis": write_file(artifact_root / "genesis.json", b"{}\n", 0o644),
            "genesis_hash": write_file(
                artifact_root / "genesis.sha256", f"{self.genesis}\n".encode(), 0o644
            ),
            "iroha_cli": write_file(artifact_root / "iroha", b"client-binary\n", 0o755),
        }
        self.validators: list[dict[str, object]] = []
        for index, validator_id in enumerate(MODULE.SLUGS, start=1):
            service_root = f"/srv/taira/{validator_id}"
            state_root = f"/var/lib/taira/{validator_id}"
            config = write_file(
                artifact_root / f"{validator_id}.toml",
                f"validator = {index}\n".encode(),
                0o640,
            )
            artifacts = [
                artifact(
                    "binary", common["binary"], f"{service_root}/bin/iroha3d_taira",
                    0o755, MODULE.VALIDATOR_ARTIFACT_SPECS["binary"][3],
                ),
                artifact(
                    "config", config, f"{service_root}/config/config.toml", 0o640,
                    MODULE.VALIDATOR_ARTIFACT_SPECS["config"][3],
                ),
                artifact(
                    "genesis", common["genesis"], f"{service_root}/genesis/genesis.json",
                    0o644, MODULE.VALIDATOR_ARTIFACT_SPECS["genesis"][3],
                ),
                artifact(
                    "genesis_hash",
                    common["genesis_hash"],
                    f"{service_root}/genesis/genesis.sha256",
                    0o644,
                    MODULE.VALIDATOR_ARTIFACT_SPECS["genesis_hash"][3],
                ),
                artifact(
                    "iroha_cli", common["iroha_cli"], f"{service_root}/bin/iroha",
                    0o755, MODULE.VALIDATOR_ARTIFACT_SPECS["iroha_cli"][3],
                ),
            ]
            platform = {
                "system": "Linux",
                "machine": "aarch64",
                "kvm_device_path": "/dev/kvm",
                "kvm_api_version": 12,
                "mountinfo_path": "/proc/self/mountinfo",
            }
            service = {
                "manager_path": "/usr/bin/systemctl",
                "unit": f"{validator_id}.service",
                "unit_path": f"/etc/systemd/system/{validator_id}.service",
                "unit_sha256": sha256_bytes(f"unit-{index}".encode()),
                "service_root": service_root,
                "service_guard_path": f"{service_root}/.taira-service-root",
                "state_root": state_root,
                "state_guard_path": f"{state_root}/.taira-state-root",
                "state_lock_path": f"{state_root}/.taira-state-lock",
            }
            self.validators.append(
                {
                    "id": validator_id,
                    "host_identity_sha256": f"{index}" * 64,
                    "platform": platform,
                    "service": service,
                    "artifacts": artifacts,
                    "preflight_attestation": {},
                }
            )

        edge_config = write_file(
            artifact_root / "taira.sora.org.conf",
            b"# operator-rendered edge config\n",
            0o640,
        )
        edge_service = {
            "manager_path": "/usr/bin/systemctl",
            "unit": "nginx.service",
            "unit_path": "/etc/systemd/system/nginx.service",
            "unit_sha256": sha256_bytes(b"nginx-unit"),
            "service_root": "/etc/nginx/conf.d",
            "service_guard_path": "/etc/nginx/conf.d/.taira-edge-root",
            "operator_uid": os.geteuid(),
            "temporary_root": "/etc/nginx/conf.d/.taira-staging",
            "target_config_path": "/etc/nginx/conf.d/taira.conf",
        }
        self.edge: dict[str, object] = {
            "id": "taira-public-edge",
            "host_identity_sha256": "e" * 64,
            "platform": {"system": "Linux", "machine": "x86_64"},
            "service": edge_service,
            "nginx": {"path": "/usr/sbin/nginx", "sha256": sha256_bytes(b"nginx")},
            "config": artifact(
                "edge_config", edge_config, str(edge_service["target_config_path"]),
                0o640, MODULE.EDGE_CONFIG_MAX_BYTES,
            ),
            "authority": {
                "public_root": "https://taira.sora.org",
                "tls_authority": "taira-tls-operator",
                "dns_authority": "taira-dns-operator",
                "public_validator": MODULE.SLUGS[0],
                "colocation_validator": None,
            },
            "validator_routes": [
                {
                    "validator_id": validator_id,
                    "hostname": f"{validator_id}.sora.org",
                    "upstream": f"10.0.0.{index}:8080",
                }
                for index, validator_id in enumerate(MODULE.SLUGS, start=1)
            ],
            "preflight_attestation": {},
        }
        self.inventory: dict[str, object] = {
            "schema": MODULE.INVENTORY_SCHEMA,
            "schema_version": MODULE.SCHEMA_VERSION,
            "deployment_id": "taira-public-reset-2026-08-24",
            "chain_id": MODULE.CHAIN_ID,
            "chain_discriminant": MODULE.CHAIN_DISCRIMINANT,
            "previous_genesis_hash": self.previous,
            "genesis_hash": self.genesis,
            "source": self.source,
            "mutation_window": {
                "not_before_unix_seconds": self.not_before,
                "expires_at_unix_seconds": self.expires_at,
                "approval_nonce_sha256": self.nonce,
            },
            "validators": self.validators,
            "edge_authority": self.edge,
            "artifact_closure_sha256": "f" * 64,
        }
        self.inventory_path = root / "inventory.json"
        self.reseal()

    def write_json(self, path: Path, value: object) -> str:
        """Write canonical owner-only JSON and return its digest."""

        body = MODULE.canonical_json_bytes(value)
        write_file(path, body, 0o600)
        return sha256_bytes(body)

    def refresh_source(self) -> None:
        """Regenerate the pinned complete stage-0 tracked-source manifest."""

        manifest = {
            "schema": MODULE.SOURCE_SCHEMA,
            "schema_version": MODULE.SCHEMA_VERSION,
            "branch": MODULE.GIT_BRANCH,
            "head_commit_sha1": self.commit,
            "head_tree_sha1": self.head_tree,
            "tracked_tree_sha256": TRACKED_TREE_SHA256,
            "tracked_files": list(GIT_ENTRIES),
        }
        manifest_path = self.root / "source-manifest.json"
        manifest_sha = self.write_json(manifest_path, manifest)
        cargo = (REPO_ROOT / "Cargo.lock").read_bytes()
        planner = MODULE_PATH.read_bytes()
        constants_path = REPO_ROOT / "scripts" / "taira_constants.py"
        constants = constants_path.read_bytes()
        self.source.clear()
        self.source.update(
            {
                "root": str(REPO_ROOT),
                "manifest_path": str(manifest_path),
                "manifest_sha256": manifest_sha,
                "branch": MODULE.GIT_BRANCH,
                "head_commit_sha1": self.commit,
                "head_tree_sha1": self.head_tree,
                "tracked_tree_sha256": TRACKED_TREE_SHA256,
                "cargo_lock_sha256": sha256_bytes(cargo),
                "planner_relative_path": "scripts/taira_public_reset.py",
                "planner_sha256": sha256_bytes(planner),
                "constants_relative_path": "scripts/taira_constants.py",
                "constants_sha256": sha256_bytes(constants),
            }
        )

    def _validator_attestation(self, validator: dict[str, object], index: int) -> dict[str, object]:
        artifacts = validator["artifacts"]
        assert isinstance(artifacts, list)
        artifact_set = sha256_bytes(
            MODULE.HOST_ARTIFACT_DOMAIN + MODULE.canonical_json_bytes(artifacts)
        )
        return {
            "schema": MODULE.VALIDATOR_ATTESTATION_SCHEMA,
            "schema_version": MODULE.SCHEMA_VERSION,
            "deployment_id": self.inventory["deployment_id"],
            "host_id": validator["id"],
            "host_identity_sha256": validator["host_identity_sha256"],
            "platform": validator["platform"],
            "service": validator["service"],
            "artifact_set_sha256": artifact_set,
            "genesis_hash": self.inventory["genesis_hash"],
            "source_tree_sha256": self.source["tracked_tree_sha256"],
            "daemon_config_validated": True,
            "inrou_startup_qualified": True,
            "inrou_identity": {
                "name": "iroha-inrou-0",
                "slot": 0,
                "uid": 70_000,
                "gid": 70_000,
                "home": "/nonexistent",
                "shell": "/usr/sbin/nologin",
                "locked": True,
                "primary_group_members": [],
                "nss_supplementary_groups": [],
                "nss_sources": ["files"],
            },
            "attestation_authority": "operator-preflight",
            "expires_at_unix_seconds": self.expires_at - 30,
        }

    def _edge_attestation(self) -> dict[str, object]:
        config = self.edge["config"]
        authority = self.edge["authority"]
        nginx = self.edge["nginx"]
        assert isinstance(config, dict) and isinstance(authority, dict) and isinstance(nginx, dict)
        return {
            "schema": MODULE.EDGE_ATTESTATION_SCHEMA,
            "schema_version": MODULE.SCHEMA_VERSION,
            "deployment_id": self.inventory["deployment_id"],
            "edge_id": self.edge["id"],
            "host_identity_sha256": self.edge["host_identity_sha256"],
            "platform": self.edge["platform"],
            "service": self.edge["service"],
            "nginx_sha256": nginx["sha256"],
            "config_sha256": config["sha256"],
            "genesis_hash": self.inventory["genesis_hash"],
            "source_tree_sha256": self.source["tracked_tree_sha256"],
            "public_validator": authority["public_validator"],
            "target_parent_direct": True,
            "target_parent_owner_uid": self.edge["service"]["operator_uid"],
            "target_parent_non_group_writable": True,
            "target_leaf_direct_regular": True,
            "target_leaf_nlink": 1,
            "staged_nginx_validated": True,
            "rollback_armed_through_reload": True,
            "attestation_authority": "operator-edge-preflight",
            "expires_at_unix_seconds": self.expires_at - 30,
        }

    @staticmethod
    def _validator_closure(validator: dict[str, object], attestation_sha: str) -> dict[str, object]:
        artifacts = validator["artifacts"]
        assert isinstance(artifacts, list)
        return {
            "id": validator["id"],
            "host_identity_sha256": validator["host_identity_sha256"],
            "platform": validator["platform"],
            "service": validator["service"],
            "artifacts": artifacts,
            "artifact_set_sha256": sha256_bytes(
                MODULE.HOST_ARTIFACT_DOMAIN + MODULE.canonical_json_bytes(artifacts)
            ),
            "preflight_attestation_sha256": attestation_sha,
        }

    def reseal(self) -> None:
        """Regenerate all pinned attestations and the immutable closure."""

        validator_closures = []
        for index, validator in enumerate(self.validators, start=1):
            attestation = self._validator_attestation(validator, index)
            path = self.root / f"validator-{index}-preflight.json"
            digest = self.write_json(path, attestation)
            validator["preflight_attestation"] = {"path": str(path), "sha256": digest}
            validator_closures.append(self._validator_closure(validator, digest))

        edge_attestation = self._edge_attestation()
        edge_path = self.root / "edge-preflight.json"
        edge_attestation_sha = self.write_json(edge_path, edge_attestation)
        self.edge["preflight_attestation"] = {
            "path": str(edge_path),
            "sha256": edge_attestation_sha,
        }
        edge_closure = {
            "id": self.edge["id"],
            "host_identity_sha256": self.edge["host_identity_sha256"],
            "platform": self.edge["platform"],
            "service": self.edge["service"],
            "nginx": self.edge["nginx"],
            "config": self.edge["config"],
            "authority": self.edge["authority"],
            "validator_routes": self.edge["validator_routes"],
            "preflight_attestation_sha256": edge_attestation_sha,
        }
        closure = {
            "schema": "iroha.taira.public-reset.artifact-closure.v1",
            "schema_version": MODULE.SCHEMA_VERSION,
            "deployment_id": self.inventory["deployment_id"],
            "chain_id": self.inventory["chain_id"],
            "chain_discriminant": self.inventory["chain_discriminant"],
            "previous_genesis_hash": self.inventory["previous_genesis_hash"],
            "genesis_hash": self.inventory["genesis_hash"],
            "source_commit": self.source["head_commit_sha1"],
            "source_tree_sha256": self.source["tracked_tree_sha256"],
            "source_manifest_sha256": self.source["manifest_sha256"],
            "validators": validator_closures,
            "edge_authority": edge_closure,
        }
        self.inventory["artifact_closure_sha256"] = sha256_bytes(
            MODULE.ARTIFACT_DOMAIN + MODULE.canonical_json_bytes(closure)
        )
        self.write_inventory()

    def write_inventory(self) -> None:
        """Write the current in-memory inventory without repairing it."""

        self.write_json(self.inventory_path, self.inventory)

    def replace_pinned_attestation(self, ref: dict[str, object], value: dict[str, object]) -> None:
        """Replace one attestation and update only its direct inventory pin."""

        path = Path(str(ref["path"]))
        ref["sha256"] = self.write_json(path, value)
        self.write_inventory()


class PublicResetInventoryTests(unittest.TestCase):
    """Exercise the exact external inventory and closure contract."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.fixture = InventoryFixture(Path(self.temporary.name).resolve())

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def assert_refused(self, pattern: str | None = None) -> None:
        """Assert that the current fixture fails closed."""

        if pattern is None:
            with self.assertRaises(MODULE.PublicResetError):
                MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        else:
            with self.assertRaisesRegex(MODULE.PublicResetError, pattern):
                MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)

    def test_complete_inventory_produces_redacted_offline_handoff(self) -> None:
        inventory = MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        self.assertEqual(tuple(validator.id for validator in inventory.validators), MODULE.SLUGS)
        report = MODULE.handoff_report(inventory, confirmation_validated=False)
        self.assertEqual(report["status"], "offline-evidence-validated")
        self.assertFalse(report["confirmation_validated"])
        self.assertFalse(report["authorization_granted"])
        self.assertFalse(report["executor_available"])
        self.assertFalse(report["live_preflight_performed"])
        self.assertFalse(report["mutation_possible"])
        encoded = MODULE.canonical_json_bytes(report).decode()
        self.assertNotIn(str(self.fixture.root), encoded)
        self.assertNotIn(self.fixture.nonce, encoded)
        self.assertIn("<approval_nonce_sha256>", encoded)

    def test_preflight_is_the_only_read_only_command_spelling(self) -> None:
        parser = MODULE.build_parser()
        parsed = parser.parse_args(
            ["preflight", "--inventory", str(self.fixture.inventory_path)]
        )
        self.assertEqual(parsed.command, "preflight")
        for retired in ("plan", "dry-run"):
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as error:
                parser.parse_args(
                    [retired, "--inventory", str(self.fixture.inventory_path)]
                )
            self.assertEqual(error.exception.code, 2)
            self.assertIn("invalid choice", stderr.getvalue())

    def test_confirmation_is_bound_to_exact_inventory_and_nonce(self) -> None:
        inventory = MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        with self.assertRaisesRegex(MODULE.PublicResetError, "does not bind"):
            MODULE.confirm(inventory, "confirm-public-taira-reset")
        report = MODULE.confirm(inventory, inventory.confirmation)
        self.assertTrue(report["confirmation_validated"])
        self.assertFalse(report["authorization_granted"])
        self.assertEqual(report["status"], "confirmed-offline-handoff")
        self.assertNotIn("confirmation_format", report)

    def test_requires_exactly_four_canonical_distinct_hosts(self) -> None:
        self.fixture.validators.pop()
        self.fixture.write_inventory()
        self.assert_refused("exactly four")

        replacement = InventoryFixture(self.fixture.root / "duplicate")
        replacement.validators[1]["host_identity_sha256"] = replacement.validators[0][
            "host_identity_sha256"
        ]
        replacement.reseal()
        with self.assertRaisesRegex(MODULE.PublicResetError, "must be distinct"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_requires_linux_aarch64_kvm12_and_explicit_paths(self) -> None:
        platform = self.fixture.validators[0]["platform"]
        assert isinstance(platform, dict)
        platform["machine"] = "x86_64"
        self.fixture.write_inventory()
        self.assert_refused("Linux/aarch64")

        replacement = InventoryFixture(self.fixture.root / "kvm")
        platform = replacement.validators[0]["platform"]
        assert isinstance(platform, dict)
        platform["kvm_api_version"] = 11
        replacement.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "KVM API version 12"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

        for case, field, value, pattern in (
            ("device", "kvm_device_path", "/dev/operator-kvm", "/dev/kvm"),
            ("mountinfo", "mountinfo_path", "/tmp/mountinfo", "/proc/self/mountinfo"),
        ):
            exact = InventoryFixture(self.fixture.root / f"platform-{case}")
            exact_platform = exact.validators[0]["platform"]
            assert isinstance(exact_platform, dict)
            exact_platform[field] = value
            exact.write_inventory()
            with self.assertRaisesRegex(MODULE.PublicResetError, pattern):
                MODULE.load_inventory(exact.inventory_path, now=exact.now)

        missing = InventoryFixture(self.fixture.root / "paths")
        service = missing.validators[0]["service"]
        assert isinstance(service, dict)
        del service["state_root"]
        missing.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "fields differ"):
            MODULE.load_inventory(missing.inventory_path, now=missing.now)

    def test_rejects_overlapping_or_control_colliding_roots(self) -> None:
        service = self.fixture.validators[0]["service"]
        assert isinstance(service, dict)
        service["state_root"] = f"{service['service_root']}/state"
        self.fixture.write_inventory()
        self.assert_refused("must be disjoint")

        replacement = InventoryFixture(self.fixture.root / "collision")
        service = replacement.validators[0]["service"]
        artifacts = replacement.validators[0]["artifacts"]
        assert isinstance(service, dict) and isinstance(artifacts, list)
        artifacts[0]["install_path"] = service["service_guard_path"]
        replacement.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "collide"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_rejects_inventory_selected_deep_roots_and_units(self) -> None:
        service = self.fixture.validators[0]["service"]
        assert isinstance(service, dict)
        service["service_root"] = "/etc/ssl/private/taira-validator-1"
        service["service_guard_path"] = (
            "/etc/ssl/private/taira-validator-1/.taira-service-root"
        )
        service["unit"] = "operator-selected.service"
        service["unit_path"] = "/etc/systemd/system/operator-selected.service"
        self.fixture.write_inventory()
        self.assert_refused("exact canonical Taira paths and unit")

    def test_source_closure_binds_actual_head_complete_index_and_cargo_lock(self) -> None:
        self.fixture.source["head_commit_sha1"] = "c" * 40
        self.fixture.write_inventory()
        self.assert_refused("current optimizations checkout")

        incomplete = InventoryFixture(self.fixture.root / "incomplete")
        manifest_path = Path(str(incomplete.source["manifest_path"]))
        manifest = json.loads(manifest_path.read_text())
        manifest["tracked_files"].pop()
        manifest["tracked_tree_sha256"] = sha256_bytes(
            MODULE.SOURCE_DOMAIN
            + MODULE.canonical_json_bytes(manifest["tracked_files"])
        )
        incomplete.source["tracked_tree_sha256"] = manifest["tracked_tree_sha256"]
        incomplete.source["manifest_sha256"] = incomplete.write_json(
            manifest_path, manifest
        )
        incomplete.reseal()
        with self.assertRaisesRegex(MODULE.PublicResetError, "current Git index"):
            MODULE.load_inventory(incomplete.inventory_path, now=incomplete.now)

        cargo = InventoryFixture(self.fixture.root / "cargo")
        cargo.source["cargo_lock_sha256"] = "9" * 64
        cargo.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "cargo_lock SHA-256"):
            MODULE.load_inventory(cargo.inventory_path, now=cargo.now)

    def test_source_and_artifact_tampering_are_detected(self) -> None:
        manifest_path = Path(str(self.fixture.source["manifest_path"]))
        manifest = json.loads(manifest_path.read_text())
        manifest["tracked_files"][0]["blob_sha1"] = "9" * 40
        manifest["tracked_tree_sha256"] = sha256_bytes(
            MODULE.SOURCE_DOMAIN + MODULE.canonical_json_bytes(manifest["tracked_files"])
        )
        self.fixture.source["tracked_tree_sha256"] = manifest["tracked_tree_sha256"]
        self.fixture.source["manifest_sha256"] = self.fixture.write_json(manifest_path, manifest)
        self.fixture.reseal()
        self.assert_refused("current Git index")

        replacement = InventoryFixture(self.fixture.root / "artifact")
        binary = Path(str(replacement.validators[0]["artifacts"][0]["source_path"]))
        binary.write_bytes(b"tampered\n")
        binary.chmod(0o755)
        with self.assertRaisesRegex(MODULE.PublicResetError, "SHA-256 disagrees"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_artifact_modes_and_per_host_config_closure_are_exact(self) -> None:
        config = Path(str(self.fixture.validators[0]["artifacts"][1]["source_path"]))
        config.chmod(0o600)
        self.assert_refused("mode disagrees")

        replacement = InventoryFixture(self.fixture.root / "configs")
        first = replacement.validators[0]["artifacts"][1]
        second = replacement.validators[1]["artifacts"][1]
        second_path = Path(str(second["source_path"]))
        second_path.write_bytes(Path(str(first["source_path"])).read_bytes())
        second_path.chmod(0o640)
        second["sha256"] = first["sha256"]
        replacement.reseal()
        with self.assertRaisesRegex(MODULE.PublicResetError, "distinct config"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_artifact_roles_sources_installs_modes_and_bounds_are_closed(self) -> None:
        unknown = InventoryFixture(self.fixture.root / "unknown-role")
        unknown.validators[0]["artifacts"][0]["role"] = "operator_binary"
        unknown.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "five canonical V1 roles"):
            MODULE.load_inventory(unknown.inventory_path, now=unknown.now)

        extra = InventoryFixture(self.fixture.root / "extra-role")
        extra.validators[0]["artifacts"].append(
            dict(extra.validators[0]["artifacts"][0], role="extra")
        )
        extra.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "exactly the five V1 roles"):
            MODULE.load_inventory(extra.inventory_path, now=extra.now)

        wrong_install = InventoryFixture(self.fixture.root / "wrong-install")
        wrong_install.validators[0]["artifacts"][0]["install_path"] = (
            "/srv/taira/taira-validator-1/bin/operator-daemon"
        )
        wrong_install.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "source/install contract"):
            MODULE.load_inventory(wrong_install.inventory_path, now=wrong_install.now)

        wrong_mode = InventoryFixture(self.fixture.root / "wrong-mode")
        config = wrong_mode.validators[0]["artifacts"][1]
        Path(str(config["source_path"])).chmod(0o644)
        config["mode"] = "0644"
        wrong_mode.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "source/install contract"):
            MODULE.load_inventory(wrong_mode.inventory_path, now=wrong_mode.now)

        wrong_bound = InventoryFixture(self.fixture.root / "wrong-bound")
        wrong_bound.validators[0]["artifacts"][1]["max_bytes"] += 1
        wrong_bound.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "source/install contract"):
            MODULE.load_inventory(wrong_bound.inventory_path, now=wrong_bound.now)

        wrong_source = InventoryFixture(self.fixture.root / "wrong-source")
        config = wrong_source.validators[0]["artifacts"][1]
        bad_path = write_file(
            wrong_source.root / "artifacts" / "operator.toml",
            Path(str(config["source_path"])).read_bytes(),
            0o640,
        )
        config["source_path"] = str(bad_path)
        wrong_source.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "source/install contract"):
            MODULE.load_inventory(wrong_source.inventory_path, now=wrong_source.now)

    def test_common_artifacts_and_genesis_hash_file_must_agree(self) -> None:
        replacement_binary = write_file(
            self.fixture.root / "other" / "iroha3d_taira",
            b"other-validator-binary\n",
            0o755,
        )
        changed = self.fixture.validators[1]["artifacts"][0]
        install = str(changed["install_path"])
        changed.update(
            artifact(
                "binary", replacement_binary, install, 0o755,
                MODULE.VALIDATOR_ARTIFACT_SPECS["binary"][3],
            )
        )
        self.fixture.reseal()
        self.assert_refused("share the same binary")

        replacement = InventoryFixture(self.fixture.root / "genesis-hash")
        wrong = write_file(
            replacement.root / "wrong" / "genesis.sha256", b"f" * 64 + b"\n", 0o644
        )
        for validator in replacement.validators:
            entry = validator["artifacts"][3]
            install = str(entry["install_path"])
            entry.update(
                artifact(
                    "genesis_hash", wrong, install, 0o644,
                    MODULE.VALIDATOR_ARTIFACT_SPECS["genesis_hash"][3],
                )
            )
        replacement.reseal()
        with self.assertRaisesRegex(MODULE.PublicResetError, "does not contain"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_upstreams_use_canonical_nonlocal_renderer_rules(self) -> None:
        self.assertEqual(
            MODULE._canonical_upstream("10.0.0.1:8080", "upstream"),
            "10.0.0.1:8080",
        )
        self.assertEqual(
            MODULE._canonical_upstream("node.example:443", "upstream"),
            "node.example:443",
        )
        self.assertEqual(
            MODULE._canonical_upstream("[2001:db8::1]:8080", "upstream"),
            "[2001:db8::1]:8080",
        )
        invalid = (
            "localhost:8080",
            "api.localhost:8080",
            "127.0.0.1:8080",
            "0.0.0.0:8080",
            "010.0.0.1:8080",
            "999.0.0.1:8080",
            "[::]:8080",
            "[0:0:0:0:0:0:0:1]:8080",
            "[::ffff:7f00:1]:8080",
            "[2001:0db8::1]:8080",
            "NODE.example:8080",
            "node.example.:8080",
            "node..example:8080",
            "node.example:08080",
            ":8080",
        )
        for value in invalid:
            with self.subTest(value=value), self.assertRaises(MODULE.PublicResetError):
                MODULE._canonical_upstream(value, "upstream")

        routes = self.fixture.edge["validator_routes"]
        assert isinstance(routes, list)
        routes[0]["upstream"] = "localhost:8080"
        self.fixture.reseal()
        self.assert_refused("localhost")

    def test_edge_authority_routes_and_attested_target_safety_are_mandatory(self) -> None:
        routes = self.fixture.edge["validator_routes"]
        assert isinstance(routes, list)
        routes[1]["upstream"] = routes[0]["upstream"]
        self.fixture.reseal()
        self.assert_refused("must each be distinct")

        replacement = InventoryFixture(self.fixture.root / "edge")
        ref = replacement.edge["preflight_attestation"]
        assert isinstance(ref, dict)
        attestation = json.loads(Path(str(ref["path"])).read_text())
        attestation["target_parent_non_group_writable"] = False
        replacement.replace_pinned_attestation(ref, attestation)
        with self.assertRaisesRegex(MODULE.PublicResetError, "safe cutover"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

        nested = InventoryFixture(self.fixture.root / "nested-edge-target")
        service = nested.edge["service"]
        config = nested.edge["config"]
        assert isinstance(service, dict) and isinstance(config, dict)
        service["target_config_path"] = "/etc/nginx/conf.d/includes/taira.conf"
        config["install_path"] = service["target_config_path"]
        nested.reseal()
        with self.assertRaisesRegex(MODULE.PublicResetError, "direct child"):
            MODULE.load_inventory(nested.inventory_path, now=nested.now)

    def test_inrou_slot_zero_and_fresh_window_are_mandatory(self) -> None:
        ref = self.fixture.validators[0]["preflight_attestation"]
        assert isinstance(ref, dict)
        attestation = json.loads(Path(str(ref["path"])).read_text())
        attestation["inrou_identity"]["slot"] = 1
        self.fixture.replace_pinned_attestation(ref, attestation)
        self.assert_refused("files-only iroha-inrou-0")

        for case, field, value in (
            ("uid", "uid", 5001),
            ("home", "home", "/var/lib/iroha"),
            ("nss", "nss_sources", ["files", "sss"]),
            ("group", "primary_group_members", ["other-user"]),
        ):
            replacement = InventoryFixture(self.fixture.root / f"inrou-{case}")
            replacement_ref = replacement.validators[0]["preflight_attestation"]
            assert isinstance(replacement_ref, dict)
            replacement_attestation = json.loads(
                Path(str(replacement_ref["path"])).read_text()
            )
            replacement_attestation["inrou_identity"][field] = value
            replacement.replace_pinned_attestation(
                replacement_ref, replacement_attestation
            )
            with self.assertRaisesRegex(
                MODULE.PublicResetError, "files-only iroha-inrou-0"
            ):
                MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

        replacement = InventoryFixture(self.fixture.root / "expired")
        with self.assertRaisesRegex(MODULE.PublicResetError, "not currently active"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.expires_at)

    def test_unknown_secret_bearing_fields_and_wrong_chain_are_rejected(self) -> None:
        self.fixture.inventory["private_key"] = "must-never-be-accepted"
        self.fixture.write_inventory()
        self.assert_refused("forbidden secret-bearing field")

        replacement = InventoryFixture(self.fixture.root / "chain")
        replacement.inventory["chain_discriminant"] = 0
        replacement.write_inventory()
        with self.assertRaisesRegex(MODULE.PublicResetError, "canonical Taira"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

    def test_inventory_must_be_owner_only_single_link_canonical_json(self) -> None:
        self.fixture.inventory_path.chmod(0o644)
        self.assert_refused("owner-only")

        replacement = InventoryFixture(self.fixture.root / "duplicate-json")
        body = replacement.inventory_path.read_bytes()
        body = body.replace(b'"schema":', b'"schema":"duplicate","schema":', 1)
        write_file(replacement.inventory_path, body, 0o600)
        with self.assertRaisesRegex(MODULE.PublicResetError, "duplicate field"):
            MODULE.load_inventory(replacement.inventory_path, now=replacement.now)

        symlink_fixture = InventoryFixture(self.fixture.root / "symlink")
        link = symlink_fixture.root / "inventory-link.json"
        link.symlink_to(symlink_fixture.inventory_path)
        with self.assertRaisesRegex(MODULE.PublicResetError, "symlink ancestry"):
            MODULE.load_inventory(link, now=symlink_fixture.now)

        with self.assertRaisesRegex(MODULE.PublicResetError, "canonical absolute"):
            MODULE.load_inventory(Path("inventory.json"), now=self.fixture.now)

        ancestry_fixture = InventoryFixture(self.fixture.root / "real-ancestry")
        alias = self.fixture.root / "alias-ancestry"
        alias.symlink_to(ancestry_fixture.root, target_is_directory=True)
        with self.assertRaisesRegex(MODULE.PublicResetError, "symlink ancestry"):
            MODULE.load_inventory(
                alias / ancestry_fixture.inventory_path.name,
                now=ancestry_fixture.now,
            )

        permissive = InventoryFixture(self.fixture.root / "permissive-ancestry")
        permissive.root.chmod(0o770)
        with self.assertRaisesRegex(MODULE.PublicResetError, "non-group-writable"):
            MODULE.load_inventory(permissive.inventory_path, now=permissive.now)


class PublicResetExecutionBarrierTests(unittest.TestCase):
    """Prove that the public-reset utility has no executable mutation path."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.fixture = InventoryFixture(Path(self.temporary.name).resolve())

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_module_imports_no_transport_or_process_runner_and_opens_read_only(self) -> None:
        source = MODULE_PATH.read_text()
        tree = ast.parse(source)
        imports: set[str] = set()
        imported_dangerous_aliases: set[str] = set()
        bare_dangerous = {
            "open", "run", "call", "check_call", "check_output", "Popen",
            "urlopen", "create_connection", "connect", "unlink", "rmdir",
            "remove", "removedirs", "rename", "replace", "system", "popen",
            "spawn", "exec", "eval", "kill",
        }
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imports.update(alias.name.split(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom):
                if node.module:
                    imports.add(node.module.split(".")[0])
                imported_dangerous_aliases.update(
                    alias.asname or alias.name
                    for alias in node.names
                    if alias.name in bare_dangerous
                )
        forbidden_modules = {
            "subprocess", "socket", "urllib", "http", "ftplib", "telnetlib", "shutil"
        }
        self.assertTrue(forbidden_modules.isdisjoint(imports))
        forbidden_calls = {
            "unlink", "rmdir", "remove", "removedirs", "rename", "replace",
            "system", "popen", "spawn", "exec", "kill",
        }
        called_attributes = {
            node.func.attr
            for node in ast.walk(tree)
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
        }
        self.assertTrue(forbidden_calls.isdisjoint(called_attributes))
        called_names = {
            node.func.id
            for node in ast.walk(tree)
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
        }
        self.assertTrue(bare_dangerous.isdisjoint(called_names))
        self.assertTrue(imported_dangerous_aliases.isdisjoint(called_names))

        alias_probe = ast.parse(
            "from subprocess import run as innocent_name\ninnocent_name(['false'])\n"
        )
        probe_import = next(
            node for node in ast.walk(alias_probe) if isinstance(node, ast.ImportFrom)
        )
        probe_call = next(
            node for node in ast.walk(alias_probe) if isinstance(node, ast.Call)
        )
        probe_aliases = {
            alias.asname or alias.name
            for alias in probe_import.names
            if alias.name in bare_dangerous
        }
        self.assertIn(probe_call.func.id, probe_aliases)
        for write_flag in ("O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"):
            self.assertNotIn(write_flag, source)

        observed_flags: list[int] = []
        real_open = MODULE.os.open

        def guarded_open(path: object, flags: int, *args: object) -> int:
            observed_flags.append(flags)
            self.assertEqual(flags & os.O_ACCMODE, os.O_RDONLY)
            return real_open(path, flags, *args)

        with mock.patch.object(MODULE.os, "open", side_effect=guarded_open):
            MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        self.assertTrue(observed_flags)

    def test_apply_always_refuses_even_with_exact_confirmation(self) -> None:
        inventory = MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            result = MODULE.main(
                [
                    "apply",
                    "--inventory",
                    str(self.fixture.inventory_path),
                    "--confirm-public-mutation",
                    inventory.confirmation,
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn("public reset execution is unavailable", stderr.getvalue())

    def test_confirm_changes_no_input_file(self) -> None:
        inventory = MODULE.load_inventory(self.fixture.inventory_path, now=self.fixture.now)
        before = {
            path: sha256_bytes(path.read_bytes())
            for path in self.fixture.root.rglob("*")
            if path.is_file()
        }
        MODULE.confirm(inventory, inventory.confirmation)
        after = {
            path: sha256_bytes(path.read_bytes())
            for path in self.fixture.root.rglob("*")
            if path.is_file()
        }
        self.assertEqual(before, after)


if __name__ == "__main__":
    unittest.main()
