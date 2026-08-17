"""Focused tests for the fail-closed public-v2 deploy handoff producer."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from typing import Any

import pytest

from scripts import build_taira_public_v2_deploy_handoff as handoff
from scripts import build_taira_public_v2_prerequisite_handoff as prerequisite
from scripts import check_taira_public_v2_24h_soak_evidence as checker
from scripts import render_taira_validator_bundle as renderer


def digest(label: str) -> str:
    """Return one deterministic nonzero SHA-256-shaped fixture value."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def compact(value: object) -> bytes:
    """Encode canonical compact JSON."""

    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def pretty(value: object) -> bytes:
    """Encode the reset manifest's canonical indented JSON."""

    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n"
    ).encode("ascii")


def report_bytes(value: object) -> bytes:
    """Encode the deployment controller's canonical stdout report."""

    return (json.dumps(value, ensure_ascii=True, sort_keys=True) + "\n").encode(
        "ascii"
    )


def write(path: Path, payload: bytes, mode: int = 0o600) -> Path:
    """Write one owner-controlled fixture file."""

    path.write_bytes(payload)
    path.chmod(mode)
    return path


def checker_artifact(path: Path) -> checker.Artifact:
    """Capture one fixture as the public soak checker sees it."""

    payload = path.read_bytes()
    info = path.stat()
    return checker.Artifact(
        path,
        payload,
        hashlib.sha256(payload).hexdigest(),
        len(payload),
        info.st_dev,
        info.st_ino,
    )


def public_key(index: int) -> dict[str, str]:
    """Return one real compressed secp256k1 public-key projection."""

    payload = renderer._secp256k1_public_payload(index.to_bytes(32, "big"))
    return {"algorithm": "secp256k1", "payload_hex": payload.hex()}


@dataclass
class Harness:
    """Mutable paths and values for one complete structural fixture."""

    root: Path
    candidate: Path
    publication: Path
    attestation: Path
    output: Path
    trust: handoff.DeployControllerTrust
    source: dict[str, object]
    candidate_identity: dict[str, object]
    publication_identity: dict[str, object]
    report: dict[str, object]
    reset: dict[str, object]
    native: dict[str, dict[str, object]]

    def rewrite_inventory(self) -> None:
        """Rebuild the exact precursor inventory after a deliberate mutation."""

        rows = []
        for name in sorted(handoff.EVIDENCE_FILES):
            path = self.root / name
            payload = path.read_bytes()
            rows.append(
                {
                    "path": name,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "size": len(payload),
                }
            )
        write(
            self.root / handoff.EVIDENCE_INVENTORY,
            compact(
                {
                    "files": rows,
                    "kind": handoff.EVIDENCE_KIND,
                    "schema": handoff.EVIDENCE_SCHEMA,
                    "schema_version": 1,
                }
            ),
        )


def make_harness(tmp_path: Path) -> Harness:
    """Create one exact deploy report, reset manifest, and receipt closure."""

    root = tmp_path / "deploy-evidence"
    root.mkdir(mode=0o700)
    source = {
        "cargo_lock_sha256": digest("cargo-lock"),
        "commit": "1a" * 20,
        "dpn_validator_release_commit": "2b" * 20,
        "workspace_source_manifest_sha256": digest("workspace-source"),
    }
    binary_sha256 = digest("iroha3d")
    restart_generation = digest("restart-generation")
    configs = {
        validator: digest(f"config-{validator}") for validator in checker.VALIDATORS
    }
    config_set_sha256 = hashlib.sha256(
        json.dumps(
            configs, ensure_ascii=True, sort_keys=True, separators=(",", ":")
        ).encode("ascii")
    ).hexdigest()
    topology = {
        "canonical_lane_bindings": [],
        "canonical_physical_dataspaces": [],
        "observed_catalog_hash": "hash:" + "ab" * 32,
        "observed_lane_count": 7,
    }
    topology_text = json.dumps(
        topology, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    topology_sha256 = hashlib.sha256(topology_text.encode("ascii")).hexdigest()
    genesis_hash = "ab" * 31 + "01"
    end_hash = digest("deploy-end-block")
    base_signers: dict[str, dict[str, object]] = {}
    reset_signers: dict[str, dict[str, object]] = {}
    for index, validator in enumerate(checker.VALIDATORS, start=1):
        key = public_key(index)
        node_id = checker._receipt_node_id(key)
        seal = [101, 202, 303, 404, 505]
        runtime = checker._runtime_binding_sha256(
            binary_sha256, seal, configs[validator], restart_generation
        )
        base_signers[validator] = {
            "binary_stat_seal": seal,
            "config_sha256": configs[validator],
            "lifecycle_binding_sha256": checker._lifecycle_binding_sha256(
                runtime, restart_generation, validator, node_id
            ),
            "node_id": node_id,
            "public_key": key,
            "runtime_binding_sha256": runtime,
        }
        reset_signers[validator] = {"node_id": node_id, "public_key": key}
    candidate_identity = {
        "admission_archive_sha256": digest("admission-archive"),
        "admission_authority_manifest_sha256": digest("admission-authority"),
        "handoff_inventory_sha256": digest("candidate-root-inventory"),
        "qualification_receipt_id": digest("qualification-receipt"),
        "validator_binary_sha256": binary_sha256,
    }
    candidate_document = {
        "identity": candidate_identity,
        "kind": "candidate",
        "schema": checker.HANDOFF_SCHEMA,
        "schema_version": 1,
        "source": source,
    }
    candidate = write(tmp_path / "candidate.json", compact(candidate_document))
    candidate_sha256 = hashlib.sha256(candidate.read_bytes()).hexdigest()
    publication_identity = {
        "admission_archive_sha256": candidate_identity[
            "admission_archive_sha256"
        ],
        "candidate_handoff_sha256": candidate_sha256,
        "handoff_inventory_sha256": candidate_identity[
            "handoff_inventory_sha256"
        ],
        "publication_public_key_sha256": digest("publication-public-key"),
        "publication_receipt_sha256": digest("publication-receipt"),
        "publication_signature_sha256": digest("publication-signature"),
        "published_primary_oci_manifest_sha256": digest("primary-manifest"),
        "published_receipt_oci_manifest_sha256": digest("receipt-manifest"),
        "publisher_controller_sha256": digest("publisher-controller"),
        "qualification_receipt_id": candidate_identity[
            "qualification_receipt_id"
        ],
        "validator_binary_sha256": binary_sha256,
    }
    publication_document = {
        "identity": publication_identity,
        "kind": "publication",
        "schema": checker.HANDOFF_SCHEMA,
        "schema_version": 1,
        "source": source,
    }
    publication = write(
        tmp_path / "publication.json", compact(publication_document)
    )
    report: dict[str, object] = {
        "absent_old_children": [],
        "admission_archive_sha256": candidate_identity[
            "admission_archive_sha256"
        ],
        "admission_receipt_consumed": True,
        "admission_receipt_id": digest("admission-receipt"),
        "applied": True,
        "binary": "/Library/SORA/Taira/binaries/fixture/iroha3d",
        "binary_sha256": binary_sha256,
        "boi_artifact_inventory_sha256": digest("boi-artifacts"),
        "boi_qualification_receipt_id": digest("boi-qualification"),
        "boi_qualified_inventory_sha256": digest("boi-qualified"),
        "bundle": "/Library/SORA/Taira/reset/fixture",
        "chain_id": handoff.taira_constants.CHAIN_ID,
        "config_set_sha256": config_set_sha256,
        "deployment_completed_at_unix_ms": 1_900_000_000_000,
        "dpn_validator_release_commit": source[
            "dpn_validator_release_commit"
        ],
        "end_block_hash": end_hash,
        "end_height": 12,
        "genesis_block_hash": genesis_hash,
        "network_id": handoff.taira_constants.NETWORK_ID,
        "network_name": handoff.taira_constants.NETWORK_NAME,
        "nexus_topology": topology_text,
        "peer_count": 4,
        "protocol_version": checker.PROTOCOL_VERSION,
        "receipt_signers": base_signers,
        "restart_duration_ms": 300,
        "restart_generation": restart_generation,
        "restart_proof": "passed",
        "signed_genesis_sha256": digest("signed-genesis"),
        "source_commit": source["commit"],
        "start_height": 10,
        "supervisor": "/Library/SORA/Taira/supervisors/fixture/supervisor.py",
        "supervisor_sha256": digest("supervisor"),
        "topology_sha256": topology_sha256,
    }
    assert set(report) == handoff.APPLIED_REPORT_FIELDS
    applied_path = write(root / handoff.APPLIED_REPORT, report_bytes(report))
    reset: dict[str, object] = {
        "cargo_lock_sha256": source["cargo_lock_sha256"],
        "chain_id": report["chain_id"],
        "configs": configs,
        "dpn_validator_release_commit": source[
            "dpn_validator_release_commit"
        ],
        "genesis_expected_hash": genesis_hash,
        "irohad_sha256": binary_sha256,
        "peer_count": 4,
        "receipt_signers": reset_signers,
        "schema": "taira-exact2f-reset-bundle",
        "signed_genesis_sha256": report["signed_genesis_sha256"],
        "source_commit": source["commit"],
        "workspace_source_manifest_sha256": source[
            "workspace_source_manifest_sha256"
        ],
    }
    reset_path = write(root / handoff.RESET_MANIFEST, pretty(reset))
    attestation = write(tmp_path / "deploy-controller-attestation.json", b"{}\n")
    attestation_capture = prerequisite._capture_file(
        attestation,
        "fixture deploy controller attestation",
        handoff.MAX_ATTESTATION_BYTES,
        private=True,
    )
    trust = handoff.DeployControllerTrust(
        attestation_capture,
        {},
        digest("deploy-controller"),
        "taira-deploy-host",
        "taira-deploy-installation",
        str(source["commit"]),
    )
    native: dict[str, dict[str, object]] = {}
    controller = {
        "controller_host_id": trust.host_id,
        "controller_installation_id": trust.installation_id,
        "controller_sha256": trust.controller_sha256,
    }
    network = {
        "chain_id": report["chain_id"],
        "genesis_block_hash": handoff._marked_from_raw(
            genesis_hash, "fixture genesis"
        ),
        "network_id": report["network_id"],
        "network_name": report["network_name"],
        "protocol_version": report["protocol_version"],
    }
    deployment = {
        "config_set_sha256": report["config_set_sha256"],
        "deployment_completed_at_unix_ms": report[
            "deployment_completed_at_unix_ms"
        ],
        "end_block_hash": handoff._marked_from_raw(end_hash, "fixture end"),
        "end_height": report["end_height"],
        "restart_generation": report["restart_generation"],
        "signed_genesis_sha256": report["signed_genesis_sha256"],
        "start_height": report["start_height"],
        "topology_sha256": report["topology_sha256"],
        "validator_binary_sha256": report["binary_sha256"],
    }
    deploy_receipt_sha256 = hashlib.sha256(applied_path.read_bytes()).hexdigest()
    reset_manifest_sha256 = hashlib.sha256(reset_path.read_bytes()).hexdigest()
    for validator, name in zip(checker.VALIDATORS, handoff.NATIVE_RECEIPTS, strict=True):
        receipt = {
            "controller": controller,
            "deploy_receipt_sha256": deploy_receipt_sha256,
            "deployment": deployment,
            "network": network,
            "protocol": handoff.NATIVE_RECEIPT_PROTOCOL,
            "receipt_signer": base_signers[validator],
            "reset_manifest_sha256": reset_manifest_sha256,
            "schema": handoff.NATIVE_RECEIPT_SCHEMA,
            "schema_version": 1,
            "source": source,
            "validator_id": validator,
            "verification_result": "verified",
            "verifier_binary_sha256": digest("native-verifier-binary"),
            "verifier_source_sha256": digest("native-verifier-source"),
        }
        native[validator] = receipt
        write(root / name, compact(receipt))
    harness = Harness(
        root,
        candidate,
        publication,
        attestation,
        tmp_path / "deploy-handoff.json",
        trust,
        source,
        candidate_identity,
        publication_identity,
        report,
        reset,
        native,
    )
    harness.rewrite_inventory()
    return harness


@pytest.fixture
def harness(tmp_path: Path) -> Harness:
    """Return one complete structural producer fixture."""

    return make_harness(tmp_path)


def authorize_structural_builder(
    monkeypatch: pytest.MonkeyPatch, harness: Harness
) -> None:
    """Replace installed-controller replay with the captured fixture trust."""

    monkeypatch.setattr(
        handoff,
        "_authenticate_controller_attestation",
        lambda _path: harness.trust,
    )


def test_public_builder_refuses_before_caller_path_io(tmp_path: Path) -> None:
    missing = tmp_path / "missing"
    with pytest.raises(
        handoff.DeployHandoffError,
        match="missing preprovisioned independent public-v2 deploy",
    ):
        handoff.build_deploy_handoff(missing, missing, missing, missing, missing)


def test_structural_builder_emits_exact_checker_identity(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    document = handoff._build_structural_deploy_handoff(
        harness.root,
        harness.candidate,
        harness.publication,
        harness.attestation,
        harness.output,
    )

    assert set(document) == checker.HANDOFF_DOCUMENT_FIELDS
    assert document["kind"] == "deploy"
    identity = document["identity"]
    assert set(identity) == checker.DEPLOY_IDENTITY_FIELDS
    assert identity["deploy_receipt_sha256"] == hashlib.sha256(
        (harness.root / handoff.APPLIED_REPORT).read_bytes()
    ).hexdigest()
    assert identity["deploy_handoff_manifest_sha256"] == hashlib.sha256(
        (harness.root / handoff.EVIDENCE_INVENTORY).read_bytes()
    ).hexdigest()
    assert identity["genesis_block_hash"]["value"] == harness.report[
        "genesis_block_hash"
    ]
    assert harness.output.read_bytes() == compact(document)
    for validator in checker.VALIDATORS:
        signer = identity["receipt_signers"][validator]
        assert set(signer) == checker.RECEIPT_SIGNER_FIELDS
        assert signer["native_verifier_receipt_sha256"] == hashlib.sha256(
            (harness.root / f"{validator}-native-verifier-receipt-v1.json").read_bytes()
        ).hexdigest()

    artifacts = {
        "candidate": checker_artifact(harness.candidate),
        "publication": checker_artifact(harness.publication),
        "deploy": checker_artifact(harness.output),
    }
    references = {
        f"{kind}_handoff": {
            "kind": kind,
            "schema": checker.HANDOFF_SCHEMA,
            "sha256": artifact.sha256,
            "size_bytes": artifact.size,
            "source": harness.source,
        }
        for kind, artifact in artifacts.items()
    }
    handoffs, projection = checker._validate_prerequisites(
        references,
        source=harness.source,
        expected_binary_sha256=str(harness.report["binary_sha256"]),
        candidate_artifact=artifacts["candidate"],
        publication_artifact=artifacts["publication"],
        deploy_artifact=artifacts["deploy"],
        network={
            "chain_id": harness.report["chain_id"],
            "genesis_block_hash": identity["genesis_block_hash"],
            "name": harness.report["network_name"],
            "network_id": harness.report["network_id"],
            "protocol_version": harness.report["protocol_version"],
        },
        expected_native_binary_sha256=digest("native-verifier-binary"),
        expected_native_source_sha256=digest("native-verifier-source"),
    )
    assert handoffs["deploy"] == artifacts["deploy"].sha256
    assert projection["end_hash"] == harness.report["end_block_hash"]


def test_structural_builder_never_overwrites_output(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    write(harness.output, b"existing\n")

    with pytest.raises(prerequisite.PrerequisiteHandoffError, match="already exists"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )
    assert harness.output.read_bytes() == b"existing\n"


def test_missing_deployment_completion_is_not_inferred(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    harness.report.pop("deployment_completed_at_unix_ms")
    write(
        harness.root / handoff.APPLIED_REPORT,
        report_bytes(harness.report),
    )
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="fields differ"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_topology_digest_must_be_derived_from_applied_report(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    harness.report["topology_sha256"] = digest("forged-topology")
    write(harness.root / handoff.APPLIED_REPORT, report_bytes(harness.report))
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="topology digest"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_end_block_hash_must_carry_iroha_marker_bit(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    harness.report["end_block_hash"] = "ab" * 31 + "02"
    write(harness.root / handoff.APPLIED_REPORT, report_bytes(harness.report))
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="Iroha marker bit"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_config_set_digest_must_come_from_reset_manifest(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    configs = harness.reset["configs"]
    assert isinstance(configs, dict)
    configs[checker.VALIDATORS[0]] = digest("other-config")
    write(harness.root / handoff.RESET_MANIFEST, pretty(harness.reset))
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="config-set digest"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_native_receipt_must_bind_exact_controller_and_deployment(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    validator = checker.VALIDATORS[0]
    harness.native[validator]["controller"] = {
        "controller_host_id": "foreign-deploy-host",
        "controller_installation_id": harness.trust.installation_id,
        "controller_sha256": harness.trust.controller_sha256,
    }
    write(
        harness.root / handoff.NATIVE_RECEIPTS[0],
        compact(harness.native[validator]),
    )
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="exact deployment"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_native_receipt_rejects_boolean_height_splice(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    harness.report["start_height"] = 1
    write(harness.root / handoff.APPLIED_REPORT, report_bytes(harness.report))
    validator = checker.VALIDATORS[0]
    deployment = harness.native[validator]["deployment"]
    assert isinstance(deployment, dict)
    deployment["start_height"] = True
    write(
        harness.root / handoff.NATIVE_RECEIPTS[0],
        compact(harness.native[validator]),
    )
    harness.rewrite_inventory()

    with pytest.raises(handoff.DeployHandoffError, match="exact deployment"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_publication_splice_is_rejected(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    harness.publication_identity["candidate_handoff_sha256"] = digest(
        "foreign-candidate"
    )
    write(
        harness.publication,
        compact(
            {
                "identity": harness.publication_identity,
                "kind": "publication",
                "schema": checker.HANDOFF_SCHEMA,
                "schema_version": 1,
                "source": harness.source,
            }
        ),
    )

    with pytest.raises(handoff.DeployHandoffError, match="exact candidate"):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )


def test_controller_attestation_is_replayed_from_installed_macos_deploy(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    value: dict[str, Any] = {
        "controller_digest": digest("controller"),
        "controller_version": "1",
        "host_id": "taira-deploy-host",
        "installation_id": "taira-deploy-installation",
        "launcher_sha256": digest("launcher"),
        "platform": "macos",
        "role": "macos-deploy",
        "source_commit": "3c" * 20,
        "uid": 501,
    }
    path = write(tmp_path / "attestation.json", compact(value))
    calls: list[dict[str, object]] = []

    def replay(**kwargs: object) -> dict[str, Any]:
        calls.append(kwargs)
        return value

    monkeypatch.setattr(handoff.controllers, "_attest", replay)
    trust = handoff._authenticate_controller_attestation(path)

    assert trust.controller_sha256 == value["controller_digest"]
    assert trust.host_id == value["host_id"]
    assert calls == [
        {
            "expected_controller_digest": value["controller_digest"],
            "expected_host_id": value["host_id"],
            "expected_installation_id": value["installation_id"],
            "expected_launcher_sha256": value["launcher_sha256"],
            "expected_uid": "501",
            "expected_version": "1",
            "platform_name": "macos",
            "role": "macos-deploy",
            "source_commit": value["source_commit"],
        }
    ]


def test_evidence_inventory_cannot_alias_native_receipts(
    harness: Harness, monkeypatch: pytest.MonkeyPatch
) -> None:
    authorize_structural_builder(monkeypatch, harness)
    second = harness.root / handoff.NATIVE_RECEIPTS[1]
    second.unlink()
    second.hardlink_to(harness.root / handoff.NATIVE_RECEIPTS[0])
    harness.rewrite_inventory()

    with pytest.raises(
        (handoff.DeployHandoffError, prerequisite.PrerequisiteHandoffError),
        match="owner-controlled regular file|filesystem alias",
    ):
        handoff._build_structural_deploy_handoff(
            harness.root,
            harness.candidate,
            harness.publication,
            harness.attestation,
            harness.output,
        )
