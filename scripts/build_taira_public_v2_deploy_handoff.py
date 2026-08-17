#!/usr/bin/env python3
"""Build one source-bound public-v2 post-deploy prerequisite handoff.

The public entry point is intentionally disabled until the independent native
deployment-evidence authority is provisioned.  The private structural builder
exists so that the closed artifact contract can be reviewed and tested without
mistaking local JSON construction for release authority.

``deploy_handoff_manifest_sha256`` names the precursor deploy-evidence
inventory.  That inventory covers the applied reset report, reset manifest,
and four native receipts, but not itself or the separately written final
handoff.  This avoids a self-referential digest while preserving an exact input
closure.
"""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
from typing import Any, NoReturn

try:
    from . import build_taira_public_v2_prerequisite_handoff as prerequisite
    from . import check_taira_public_v2_24h_soak_evidence as checker
    from . import seal_taira_release_controllers as controllers
    from . import taira_constants
except ImportError:
    import build_taira_public_v2_prerequisite_handoff as prerequisite
    import check_taira_public_v2_24h_soak_evidence as checker
    import seal_taira_release_controllers as controllers
    import taira_constants


EVIDENCE_KIND = "public-soak-deploy-evidence"
EVIDENCE_SCHEMA = "iroha.taira.release_handoff"
EVIDENCE_INVENTORY = controllers.HANDOFF_MANIFEST
APPLIED_REPORT = "deploy-applied-v1.json"
RESET_MANIFEST = "reset-manifest.json"
NATIVE_RECEIPT_SCHEMA = (
    "iroha.taira.public-v2-deploy-native-verifier-receipt.v1"
)
NATIVE_RECEIPT_PROTOCOL = "iroha-taira-public-v2-deploy-native-verifier-v1"
NATIVE_RECEIPTS = tuple(
    f"{validator}-native-verifier-receipt-v1.json"
    for validator in checker.VALIDATORS
)
EVIDENCE_FILES = (APPLIED_REPORT, *NATIVE_RECEIPTS, RESET_MANIFEST)
MAX_EVIDENCE_FILE_BYTES = 8 * 1024 * 1024
MAX_ATTESTATION_BYTES = 4 * 1024 * 1024
MAX_HANDOFF_BYTES = checker.MAX_HANDOFF_BYTES
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")

DEPLOY_NATIVE_EVIDENCE_AUTHORITY_BARRIER = (
    "missing preprovisioned independent public-v2 deploy native-evidence "
    "authority: post-deploy handoff production is disabled before caller path "
    "inspection"
)

APPLIED_REPORT_FIELDS = {
    "absent_old_children",
    "admission_archive_sha256",
    "admission_receipt_consumed",
    "admission_receipt_id",
    "applied",
    "binary",
    "binary_sha256",
    "boi_artifact_inventory_sha256",
    "boi_qualification_receipt_id",
    "boi_qualified_inventory_sha256",
    "bundle",
    "chain_id",
    "config_set_sha256",
    "deployment_completed_at_unix_ms",
    "dpn_validator_release_commit",
    "end_block_hash",
    "end_height",
    "genesis_block_hash",
    "network_id",
    "network_name",
    "nexus_topology",
    "peer_count",
    "protocol_version",
    "receipt_signers",
    "restart_duration_ms",
    "restart_generation",
    "restart_proof",
    "signed_genesis_sha256",
    "source_commit",
    "start_height",
    "supervisor",
    "supervisor_sha256",
    "topology_sha256",
}
BASE_SIGNER_FIELDS = {
    "binary_stat_seal",
    "config_sha256",
    "lifecycle_binding_sha256",
    "node_id",
    "public_key",
    "runtime_binding_sha256",
}
NATIVE_RECEIPT_FIELDS = {
    "controller",
    "deploy_receipt_sha256",
    "deployment",
    "network",
    "protocol",
    "receipt_signer",
    "reset_manifest_sha256",
    "schema",
    "schema_version",
    "source",
    "validator_id",
    "verification_result",
    "verifier_binary_sha256",
    "verifier_source_sha256",
}
NATIVE_CONTROLLER_FIELDS = {
    "controller_host_id",
    "controller_installation_id",
    "controller_sha256",
}
NATIVE_NETWORK_FIELDS = {
    "chain_id",
    "genesis_block_hash",
    "network_id",
    "network_name",
    "protocol_version",
}
NATIVE_DEPLOYMENT_FIELDS = {
    "config_set_sha256",
    "deployment_completed_at_unix_ms",
    "end_block_hash",
    "end_height",
    "restart_generation",
    "signed_genesis_sha256",
    "start_height",
    "topology_sha256",
    "validator_binary_sha256",
}


class DeployHandoffError(RuntimeError):
    """The supplied post-deploy evidence is not one exact trusted closure."""


@dataclass(frozen=True)
class DeployControllerTrust:
    """Replayed installed macOS deploy-controller identity."""

    attestation: prerequisite.CapturedFile
    authenticated: Mapping[str, object]
    controller_sha256: str
    host_id: str
    installation_id: str
    source_commit: str


@dataclass(frozen=True)
class DeployEvidence:
    """Stable captures from one exact precursor evidence root."""

    root: Path
    root_identity: tuple[int, ...]
    inventory: prerequisite.CapturedFile
    files: Mapping[str, prerequisite.CapturedFile]


def _fail(message: str) -> NoReturn:
    raise DeployHandoffError(message)


def require_deploy_native_evidence_authority_provisioned() -> NoReturn:
    """Refuse before caller path I/O until the independent authority exists."""

    _fail(DEPLOY_NATIVE_EVIDENCE_AUTHORITY_BARRIER)


def _sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be one nonzero lowercase 40-hex commit")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or any(ord(char) < 0x20 for char in value):
        _fail(f"{label} must be one nonempty printable string")
    return value


def _exact(value: object, fields: set[str] | frozenset[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    actual = set(value)
    expected = set(fields)
    if actual != expected:
        _fail(
            f"{label} fields differ: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )
    return value


def _reject_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _reject_constant(value: str) -> NoReturn:
    _fail(f"JSON contains non-finite number {value!r}")


def _decode_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise DeployHandoffError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def _compact_json(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise DeployHandoffError(f"handoff is not canonically encodable: {error}") from error


def _pretty_json(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                indent=2,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise DeployHandoffError(
            f"reset manifest is not canonically encodable: {error}"
        ) from error


def _report_json(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=True, allow_nan=False, sort_keys=True) + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise DeployHandoffError(f"deploy report is not canonically encodable: {error}") from error


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _absolute_directory(path: Path, label: str) -> tuple[Path, tuple[int, ...]]:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must use one absolute normalized path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as error:
        raise DeployHandoffError(f"cannot inspect {label}: {path}") from error
    if (
        resolved != path
        or not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_uid not in {0, os.geteuid()}
        or info.st_mode & 0o022
    ):
        _fail(f"{label} is not one owner-controlled real directory")
    return path, _directory_identity(info)


def _capture_evidence_root(path: Path) -> DeployEvidence:
    root, root_identity = _absolute_directory(path, "deploy evidence root")
    expected_names = {EVIDENCE_INVENTORY, *EVIDENCE_FILES}
    if set(os.listdir(root)) != expected_names:
        _fail("deploy evidence root inventory is not exact")
    inventory = prerequisite._capture_file(
        root / EVIDENCE_INVENTORY,
        "deploy evidence inventory",
        MAX_EVIDENCE_FILE_BYTES,
    )
    inventory_value = _exact(
        _decode_json(inventory.payload, "deploy evidence inventory"),
        {"files", "kind", "schema", "schema_version"},
        "deploy evidence inventory",
    )
    if inventory.payload != _compact_json(inventory_value):
        _fail("deploy evidence inventory is not canonical compact JSON")
    if (
        inventory_value["kind"] != EVIDENCE_KIND
        or inventory_value["schema"] != EVIDENCE_SCHEMA
        or type(inventory_value["schema_version"]) is not int
        or inventory_value["schema_version"] != 1
    ):
        _fail("deploy evidence inventory identity is wrong")
    rows = inventory_value["files"]
    if not isinstance(rows, list) or len(rows) != len(EVIDENCE_FILES):
        _fail("deploy evidence inventory row count is not exact")
    files: dict[str, prerequisite.CapturedFile] = {}
    row_names: list[str] = []
    identities = [(inventory.identity.device, inventory.identity.inode)]
    for row in rows:
        row = _exact(row, {"path", "sha256", "size"}, "deploy evidence row")
        name = row["path"]
        if not isinstance(name, str) or name not in EVIDENCE_FILES:
            _fail("deploy evidence row path is not one exact leaf")
        captured = prerequisite._capture_file(
            root / name,
            f"deploy evidence {name}",
            MAX_EVIDENCE_FILE_BYTES,
        )
        if (
            _sha256(row["sha256"], f"deploy evidence {name} digest")
            != captured.sha256
            or _integer(row["size"], f"deploy evidence {name} size", minimum=1)
            != captured.identity.size
        ):
            _fail(f"deploy evidence inventory does not bind {name}")
        files[name] = captured
        row_names.append(name)
        identities.append((captured.identity.device, captured.identity.inode))
    if row_names != sorted(EVIDENCE_FILES):
        _fail("deploy evidence inventory rows are not exact and sorted")
    if len(identities) != len(set(identities)):
        _fail("deploy evidence files contain a filesystem alias")
    if _directory_identity(root.lstat()) != root_identity:
        _fail("deploy evidence root changed while capturing")
    return DeployEvidence(root, root_identity, inventory, files)


def _replay_evidence(evidence: DeployEvidence) -> None:
    if (
        set(os.listdir(evidence.root)) != {EVIDENCE_INVENTORY, *EVIDENCE_FILES}
        or _directory_identity(evidence.root.lstat()) != evidence.root_identity
    ):
        _fail("deploy evidence root changed during production")
    prerequisite._replay_file(
        evidence.inventory,
        "deploy evidence inventory",
        MAX_EVIDENCE_FILE_BYTES,
    )
    for name in EVIDENCE_FILES:
        prerequisite._replay_file(
            evidence.files[name],
            f"deploy evidence {name}",
            MAX_EVIDENCE_FILE_BYTES,
        )


def _authenticate_controller_attestation(path: Path) -> DeployControllerTrust:
    captured = prerequisite._capture_file(
        path,
        "deploy controller attestation",
        MAX_ATTESTATION_BYTES,
        private=True,
    )
    value = _decode_json(captured.payload, "deploy controller attestation")
    if captured.payload != controllers.canonical_json_bytes(value):
        _fail("deploy controller attestation is not canonical compact JSON")
    controller_sha256 = _sha256(
        value.get("controller_digest"), "deploy controller digest"
    )
    launcher_sha256 = _sha256(
        value.get("launcher_sha256"), "deploy controller launcher digest"
    )
    source_commit = _commit(value.get("source_commit"), "deploy controller source")
    for field in ("controller_version", "platform", "role"):
        _text(value.get(field), f"deploy controller {field}")
    try:
        host_id = checker._identity_text(
            value.get("host_id"), "deploy controller host ID"
        )
        installation_id = checker._identity_text(
            value.get("installation_id"), "deploy controller installation ID"
        )
    except checker.EvidenceError as error:
        raise DeployHandoffError(str(error)) from error
    uid = _integer(value.get("uid"), "deploy controller UID")
    try:
        replay = controllers._attest(
            expected_launcher_sha256=launcher_sha256,
            expected_controller_digest=controller_sha256,
            expected_version=str(value["controller_version"]),
            expected_host_id=host_id,
            expected_installation_id=installation_id,
            expected_uid=str(uid),
            source_commit=source_commit,
            platform_name=str(value["platform"]),
            role=str(value["role"]),
        )
    except controllers.ControllerSealError as error:
        raise DeployHandoffError(
            f"installed deploy-controller attestation failed replay: {error}"
        ) from error
    if replay != value:
        _fail("deploy controller attestation differs from current installed state")
    if value["platform"] != "macos" or value["role"] != "macos-deploy":
        _fail("controller attestation is not the macos-deploy authority")
    prerequisite._replay_file(
        captured,
        "deploy controller attestation",
        MAX_ATTESTATION_BYTES,
        private=True,
    )
    return DeployControllerTrust(
        captured,
        value,
        controller_sha256,
        host_id,
        installation_id,
        source_commit,
    )


def _replay_controller_trust(trust: DeployControllerTrust) -> None:
    if _authenticate_controller_attestation(trust.attestation.path) != trust:
        _fail("installed deploy-controller trust changed during production")


def _source(value: object, label: str) -> dict[str, object]:
    source = _exact(value, checker.SOURCE_FIELDS, label)
    return {
        "cargo_lock_sha256": _sha256(source["cargo_lock_sha256"], f"{label} Cargo.lock"),
        "commit": _commit(source["commit"], f"{label} commit"),
        "dpn_validator_release_commit": _commit(
            source["dpn_validator_release_commit"], f"{label} DPN commit"
        ),
        "workspace_source_manifest_sha256": _sha256(
            source["workspace_source_manifest_sha256"], f"{label} source manifest"
        ),
    }


def _load_handoff(
    path: Path,
    kind: str,
    fields: set[str],
) -> tuple[prerequisite.CapturedFile, dict[str, object], dict[str, object]]:
    captured = prerequisite._capture_file(path, f"{kind} handoff", MAX_HANDOFF_BYTES)
    document = _exact(
        _decode_json(captured.payload, f"{kind} handoff"),
        checker.HANDOFF_DOCUMENT_FIELDS,
        f"{kind} handoff",
    )
    if captured.payload != _compact_json(document):
        _fail(f"{kind} handoff is not canonical compact JSON")
    if (
        document["schema"] != checker.HANDOFF_SCHEMA
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 1
        or document["kind"] != kind
    ):
        _fail(f"{kind} handoff identity is wrong")
    source = _source(document["source"], f"{kind} handoff source")
    identity = _exact(document["identity"], fields, f"{kind} handoff identity")
    for field in fields:
        _sha256(identity[field], f"{kind} {field}")
    return captured, source, identity


def _marked_from_raw(value: object, label: str) -> dict[str, object]:
    digest = _sha256(value, label)
    if int(digest[-2:], 16) & 1 != 1:
        _fail(f"{label} lacks its Iroha marker bit")
    return {
        "algorithm": checker.IROHA_HASH_ALGORITHM,
        "type": checker.BLOCK_HASH_TYPE,
        "value": digest,
    }


def _public_signer(
    value: object, validator: str, binary_sha256: str, restart: str
) -> dict[str, object]:
    signer = _exact(value, BASE_SIGNER_FIELDS, f"deploy signer {validator}")
    try:
        key = dict(checker._public_key(signer["public_key"], f"deploy key {validator}"))
        expected_node = checker._receipt_node_id(key)
    except checker.EvidenceError as error:
        raise DeployHandoffError(str(error)) from error
    node_id = _text(signer["node_id"], f"deploy node ID {validator}")
    if node_id != expected_node:
        _fail(f"deploy node ID {validator} is not derived from its public key")
    seal = signer["binary_stat_seal"]
    if not isinstance(seal, list) or len(seal) != 5:
        _fail(f"deploy binary stat seal {validator} is not exact")
    normalized = [
        _integer(
            item,
            f"deploy binary stat seal {validator}[{index}]",
            minimum=1 if index < 3 else 0,
        )
        for index, item in enumerate(seal)
    ]
    config_sha256 = _sha256(signer["config_sha256"], f"deploy config {validator}")
    runtime = _sha256(signer["runtime_binding_sha256"], f"deploy runtime {validator}")
    if runtime != checker._runtime_binding_sha256(
        binary_sha256, normalized, config_sha256, restart
    ):
        _fail(f"deploy runtime binding {validator} is not derived")
    lifecycle = _sha256(
        signer["lifecycle_binding_sha256"], f"deploy lifecycle {validator}"
    )
    if lifecycle != checker._lifecycle_binding_sha256(
        runtime, restart, validator, node_id
    ):
        _fail(f"deploy lifecycle binding {validator} is not derived")
    return {
        "binary_stat_seal": normalized,
        "config_sha256": config_sha256,
        "lifecycle_binding_sha256": lifecycle,
        "node_id": node_id,
        "public_key": key,
        "runtime_binding_sha256": runtime,
    }


def _load_applied_report(
    captured: prerequisite.CapturedFile,
) -> tuple[dict[str, object], dict[str, dict[str, object]]]:
    report = _exact(
        _decode_json(captured.payload, "applied deploy report"),
        APPLIED_REPORT_FIELDS,
        "applied deploy report",
    )
    if captured.payload != _report_json(report):
        _fail("applied deploy report is not the deploy controller's canonical JSON")
    if (
        report["applied"] is not True
        or report["admission_receipt_consumed"] is not True
        or report["restart_proof"] != "passed"
        or report["peer_count"] != len(checker.VALIDATORS)
    ):
        _fail("applied deploy report is not one successful consumed reset")
    for field in (
        "admission_archive_sha256",
        "admission_receipt_id",
        "binary_sha256",
        "boi_artifact_inventory_sha256",
        "boi_qualification_receipt_id",
        "boi_qualified_inventory_sha256",
        "config_set_sha256",
        "restart_generation",
        "signed_genesis_sha256",
        "supervisor_sha256",
        "topology_sha256",
    ):
        _sha256(report[field], f"applied report {field}")
    _commit(report["source_commit"], "applied report source commit")
    _commit(report["dpn_validator_release_commit"], "applied report DPN commit")
    for field in ("binary", "bundle", "supervisor"):
        path = Path(_text(report[field], f"applied report {field} path"))
        if not path.is_absolute() or Path(os.path.abspath(path)) != path:
            _fail(f"applied report {field} path is not absolute and normalized")
    start = _integer(report["start_height"], "deploy start height", minimum=1)
    end = _integer(report["end_height"], "deploy end height", minimum=1)
    if end <= start:
        _fail("applied deploy report did not prove advancement")
    _sha256(report["end_block_hash"], "deploy end block hash")
    _integer(
        report["deployment_completed_at_unix_ms"],
        "deployment completion time",
        minimum=1,
    )
    _integer(report["restart_duration_ms"], "restart duration")
    absent = report["absent_old_children"]
    if (
        not isinstance(absent, list)
        or any(not isinstance(item, str) or not item for item in absent)
        or absent != sorted(set(absent))
    ):
        _fail("applied report absent-child list is not exact")
    if (
        report["network_name"] != taira_constants.NETWORK_NAME
        or report["chain_id"] != taira_constants.CHAIN_ID
        or report["network_id"] != taira_constants.NETWORK_ID
        or type(report["protocol_version"]) is not int
        or report["protocol_version"] != checker.PROTOCOL_VERSION
    ):
        _fail("applied report is not exact public Taira revision 4")
    genesis = _sha256(report["genesis_block_hash"], "deployed genesis block hash")
    if int(genesis[-2:], 16) & 1 == 0:
        _fail("deployed genesis block hash lacks its Iroha marker bit")
    topology = _text(report["nexus_topology"], "deployed topology")
    topology_value = _decode_json(topology.encode("ascii"), "deployed topology")
    topology_bytes = _compact_json(topology_value)[:-1]
    if topology_bytes.decode("ascii") != topology:
        _fail("deployed topology is not canonical compact JSON")
    if hashlib.sha256(topology_bytes).hexdigest() != report["topology_sha256"]:
        _fail("deployed topology digest is not derived from its exact JSON")
    raw_signers = _exact(
        report["receipt_signers"], set(checker.VALIDATORS), "deploy signer map"
    )
    signers = {
        validator: _public_signer(
            raw_signers[validator],
            validator,
            str(report["binary_sha256"]),
            str(report["restart_generation"]),
        )
        for validator in checker.VALIDATORS
    }
    if len({str(row["node_id"]) for row in signers.values()}) != len(signers):
        _fail("deploy receipt signer node IDs are aliased")
    return report, signers


def _load_reset_manifest(
    captured: prerequisite.CapturedFile,
    report: Mapping[str, object],
    source: Mapping[str, object],
    signers: Mapping[str, Mapping[str, object]],
) -> dict[str, object]:
    manifest = _decode_json(captured.payload, "reset manifest")
    if captured.payload != _pretty_json(manifest):
        _fail("reset manifest is not canonical deterministic JSON")
    if (
        manifest.get("schema") != "taira-exact2f-reset-bundle"
        or manifest.get("peer_count") != len(checker.VALIDATORS)
        or manifest.get("chain_id") != report["chain_id"]
        or manifest.get("source_commit") != source["commit"]
        or manifest.get("dpn_validator_release_commit")
        != source["dpn_validator_release_commit"]
        or manifest.get("cargo_lock_sha256") != source["cargo_lock_sha256"]
        or manifest.get("workspace_source_manifest_sha256")
        != source["workspace_source_manifest_sha256"]
        or manifest.get("irohad_sha256") != report["binary_sha256"]
        or manifest.get("signed_genesis_sha256") != report["signed_genesis_sha256"]
        or manifest.get("genesis_expected_hash") != report["genesis_block_hash"]
    ):
        _fail("reset manifest differs from the applied deployment identity")
    configs = manifest.get("configs")
    if not isinstance(configs, dict) or tuple(configs) != checker.VALIDATORS:
        _fail("reset manifest config map is not the exact ordered validator set")
    normalized_configs = {
        validator: _sha256(configs[validator], f"reset config {validator}")
        for validator in checker.VALIDATORS
    }
    config_bytes = json.dumps(
        normalized_configs,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("ascii")
    if hashlib.sha256(config_bytes).hexdigest() != report["config_set_sha256"]:
        _fail("deployed config-set digest is not derived from the reset manifest")
    reset_signers = _exact(
        manifest.get("receipt_signers"),
        set(checker.VALIDATORS),
        "reset receipt signer map",
    )
    for validator in checker.VALIDATORS:
        row = _exact(
            reset_signers[validator], {"node_id", "public_key"}, f"reset signer {validator}"
        )
        if (
            row["node_id"] != signers[validator]["node_id"]
            or row["public_key"] != signers[validator]["public_key"]
            or normalized_configs[validator] != signers[validator]["config_sha256"]
        ):
            _fail(f"reset signer/config differs from applied deployment: {validator}")
    return manifest


def _validate_native_receipts(
    evidence: DeployEvidence,
    report: Mapping[str, object],
    source: Mapping[str, object],
    signers: Mapping[str, Mapping[str, object]],
    trust: DeployControllerTrust,
) -> dict[str, dict[str, object]]:
    controller = {
        "controller_host_id": trust.host_id,
        "controller_installation_id": trust.installation_id,
        "controller_sha256": trust.controller_sha256,
    }
    network = {
        "chain_id": report["chain_id"],
        "genesis_block_hash": _marked_from_raw(
            report["genesis_block_hash"], "native deployed genesis block hash"
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
        "end_block_hash": _marked_from_raw(
            report["end_block_hash"], "native deploy end block hash"
        ),
        "end_height": report["end_height"],
        "restart_generation": report["restart_generation"],
        "signed_genesis_sha256": report["signed_genesis_sha256"],
        "start_height": report["start_height"],
        "topology_sha256": report["topology_sha256"],
        "validator_binary_sha256": report["binary_sha256"],
    }
    if set(controller) != NATIVE_CONTROLLER_FIELDS or set(network) != NATIVE_NETWORK_FIELDS:
        _fail("internal native-verifier identity projection differs")
    if set(deployment) != NATIVE_DEPLOYMENT_FIELDS:
        _fail("internal native-verifier deployment projection differs")
    result: dict[str, dict[str, object]] = {}
    receipt_digests: set[str] = set()
    verifier_identities: set[tuple[str, str]] = set()
    for validator, name in zip(checker.VALIDATORS, NATIVE_RECEIPTS, strict=True):
        captured = evidence.files[name]
        receipt = _exact(
            _decode_json(captured.payload, f"native receipt {validator}"),
            NATIVE_RECEIPT_FIELDS,
            f"native receipt {validator}",
        )
        if captured.payload != _compact_json(receipt):
            _fail(f"native receipt {validator} is not canonical compact JSON")
        verifier_binary = _sha256(
            receipt["verifier_binary_sha256"], f"native verifier binary {validator}"
        )
        verifier_source = _sha256(
            receipt["verifier_source_sha256"], f"native verifier source {validator}"
        )
        if (
            receipt["schema"] != NATIVE_RECEIPT_SCHEMA
            or type(receipt["schema_version"]) is not int
            or receipt["schema_version"] != 1
            or receipt["protocol"] != NATIVE_RECEIPT_PROTOCOL
            or receipt["validator_id"] != validator
            or receipt["deploy_receipt_sha256"]
            != evidence.files[APPLIED_REPORT].sha256
            or receipt["reset_manifest_sha256"]
            != evidence.files[RESET_MANIFEST].sha256
            or not checker._json_exact_equal(receipt["source"], source)
            or not checker._json_exact_equal(receipt["controller"], controller)
            or not checker._json_exact_equal(receipt["network"], network)
            or not checker._json_exact_equal(receipt["deployment"], deployment)
            or not checker._json_exact_equal(
                receipt["receipt_signer"], signers[validator]
            )
            or receipt["verification_result"] != "verified"
        ):
            _fail(f"native receipt {validator} does not bind the exact deployment")
        if captured.sha256 in receipt_digests:
            _fail("native deploy verifier receipt was reused")
        receipt_digests.add(captured.sha256)
        verifier_identities.add((verifier_binary, verifier_source))
        result[validator] = {
            **signers[validator],
            "native_verifier_binary_sha256": verifier_binary,
            "native_verifier_receipt_sha256": captured.sha256,
            "native_verifier_receipt_size_bytes": captured.identity.size,
            "native_verifier_source_sha256": verifier_source,
            "verification_result": "verified",
        }
    if len(verifier_identities) != 1:
        _fail("four native receipts do not share one pinned verifier identity")
    return result


def _build_structural_deploy_handoff(
    evidence_root: Path,
    candidate_handoff: Path,
    publication_handoff: Path,
    controller_attestation: Path,
    output: Path,
) -> dict[str, object]:
    """Validate exact precursor bytes and publish one compact deploy handoff."""

    trust = _authenticate_controller_attestation(controller_attestation)
    evidence = _capture_evidence_root(evidence_root)
    candidate_file, source, candidate = _load_handoff(
        candidate_handoff, "candidate", checker.CANDIDATE_IDENTITY_FIELDS
    )
    publication_file, publication_source, publication = _load_handoff(
        publication_handoff, "publication", checker.PUBLICATION_IDENTITY_FIELDS
    )
    if publication_source != source or trust.source_commit != source["commit"]:
        _fail("candidate/publication/deploy-controller source identity differs")
    if (
        publication["qualification_receipt_id"]
        != candidate["qualification_receipt_id"]
        or publication["candidate_handoff_sha256"] != candidate_file.sha256
        or publication["handoff_inventory_sha256"]
        != candidate["handoff_inventory_sha256"]
        or publication["admission_archive_sha256"]
        != candidate["admission_archive_sha256"]
        or publication["validator_binary_sha256"]
        != candidate["validator_binary_sha256"]
    ):
        _fail("publication handoff does not consume the exact candidate")
    report, base_signers = _load_applied_report(evidence.files[APPLIED_REPORT])
    if (
        report["source_commit"] != source["commit"]
        or report["dpn_validator_release_commit"]
        != source["dpn_validator_release_commit"]
        or report["admission_archive_sha256"]
        != candidate["admission_archive_sha256"]
        or report["binary_sha256"] != candidate["validator_binary_sha256"]
    ):
        _fail("applied deploy report differs from the admitted candidate")
    _load_reset_manifest(
        evidence.files[RESET_MANIFEST], report, source, base_signers
    )
    receipt_signers = _validate_native_receipts(
        evidence, report, source, base_signers, trust
    )
    identity: dict[str, object] = {
        "admission_archive_sha256": report["admission_archive_sha256"],
        "admission_receipt_id": report["admission_receipt_id"],
        "candidate_handoff_sha256": candidate_file.sha256,
        "chain_id": report["chain_id"],
        "config_set_sha256": report["config_set_sha256"],
        "controller_host_id": trust.host_id,
        "controller_installation_id": trust.installation_id,
        "controller_sha256": trust.controller_sha256,
        "deploy_handoff_manifest_sha256": evidence.inventory.sha256,
        "deploy_receipt_sha256": evidence.files[APPLIED_REPORT].sha256,
        "deployment_completed_at_unix_ms": report[
            "deployment_completed_at_unix_ms"
        ],
        "end_block_hash": _marked_from_raw(
            report["end_block_hash"], "deploy end block hash"
        ),
        "end_height": report["end_height"],
        "genesis_block_hash": _marked_from_raw(
            report["genesis_block_hash"], "deployed genesis block hash"
        ),
        "handoff_inventory_sha256": candidate["handoff_inventory_sha256"],
        "network_id": report["network_id"],
        "network_name": report["network_name"],
        "protocol_version": report["protocol_version"],
        "publication_handoff_sha256": publication_file.sha256,
        "publication_receipt_sha256": publication["publication_receipt_sha256"],
        "published_primary_oci_manifest_sha256": publication[
            "published_primary_oci_manifest_sha256"
        ],
        "qualification_receipt_id": candidate["qualification_receipt_id"],
        "receipt_signers": receipt_signers,
        "restart_generation": report["restart_generation"],
        "signed_genesis_sha256": report["signed_genesis_sha256"],
        "start_height": report["start_height"],
        "supervisor_sha256": report["supervisor_sha256"],
        "topology_sha256": report["topology_sha256"],
        "validator_binary_sha256": report["binary_sha256"],
    }
    if set(identity) != checker.DEPLOY_IDENTITY_FIELDS:
        _fail("deploy handoff identity fields differ from the soak checker")
    document = {
        "identity": identity,
        "kind": "deploy",
        "schema": checker.HANDOFF_SCHEMA,
        "schema_version": 1,
        "source": source,
    }
    payload = _compact_json(document)
    identities = [
        (candidate_file.identity.device, candidate_file.identity.inode),
        (publication_file.identity.device, publication_file.identity.inode),
        (trust.attestation.identity.device, trust.attestation.identity.inode),
        (evidence.inventory.identity.device, evidence.inventory.identity.inode),
        *(
            (captured.identity.device, captured.identity.inode)
            for captured in evidence.files.values()
        ),
    ]
    if len(identities) != len(set(identities)):
        _fail("deploy handoff inputs contain a filesystem alias")
    output = prerequisite._absolute(output, "deploy handoff output", exists=False)
    if output == evidence.root or evidence.root in output.parents:
        _fail("deploy handoff output must not modify the precursor evidence root")
    _replay_evidence(evidence)
    prerequisite._replay_file(candidate_file, "candidate handoff", MAX_HANDOFF_BYTES)
    prerequisite._replay_file(
        publication_file, "publication handoff", MAX_HANDOFF_BYTES
    )
    _replay_controller_trust(trust)
    prerequisite._write_atomic_no_replace(output, payload)
    return document


def build_deploy_handoff(
    evidence_root: Path,
    candidate_handoff: Path,
    publication_handoff: Path,
    controller_attestation: Path,
    output: Path,
) -> dict[str, object]:
    """Build one deploy handoff only after the native authority is provisioned."""

    # This barrier must remain ahead of every caller-controlled path operation.
    require_deploy_native_evidence_authority_provisioned()
    return _build_structural_deploy_handoff(
        evidence_root,
        candidate_handoff,
        publication_handoff,
        controller_attestation,
        output,
    )


def build_parser() -> argparse.ArgumentParser:
    """Build the path-only post-deploy handoff command line."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--deploy-evidence-root", type=Path, required=True)
    parser.add_argument("--candidate-handoff", type=Path, required=True)
    parser.add_argument("--publication-handoff", type=Path, required=True)
    parser.add_argument("--deploy-controller-attestation", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the disabled public producer and emit one secret-free summary."""

    args = build_parser().parse_args(argv)
    try:
        document = build_deploy_handoff(
            args.deploy_evidence_root,
            args.candidate_handoff,
            args.publication_handoff,
            args.deploy_controller_attestation,
            args.output,
        )
    except (
        DeployHandoffError,
        OSError,
        ValueError,
        prerequisite.PrerequisiteHandoffError,
        controllers.ControllerSealError,
    ) as error:
        print(f"Taira public-v2 deploy handoff refused: {error}", file=sys.stderr)
        return 1
    summary = {
        "kind": document["kind"],
        "output": str(args.output),
        "sha256": hashlib.sha256(_compact_json(document)).hexdigest(),
    }
    sys.stdout.buffer.write(_compact_json(summary))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
