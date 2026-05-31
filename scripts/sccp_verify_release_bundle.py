#!/usr/bin/env python3
"""Verify a published SCCP release-note attachment bundle."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BUNDLE_SCRIPT = ROOT / "scripts" / "sccp_release_bundle.py"
REPORT_SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
SCHEMA = "sccp-release-bundle-v1"
CORRIDOR_COMPLETION_SENTINEL = "SCCP production corridor completed."
CORRIDOR_DRY_RUN_SENTINEL = "SCCP production corridor dry run completed."
SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_DOMAIN_SOL = 3
SCCP_DOMAIN_TON = 4
SCCP_DOMAIN_TRON = 5
SCCP_DOMAIN_SORA_KUSAMA = 6
SCCP_DOMAIN_SORA_POLKADOT = 7
SCCP_DOMAIN_SORA2 = 8
ALL_LANES_REQUIRED_DOMAINS = (
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
)
ALL_LANES_CHAIN_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "eth",
    SCCP_DOMAIN_BSC: "bsc",
    SCCP_DOMAIN_SOL: "sol",
    SCCP_DOMAIN_TON: "ton",
    SCCP_DOMAIN_TRON: "tron",
    SCCP_DOMAIN_SORA_KUSAMA: "sora-kusama",
    SCCP_DOMAIN_SORA_POLKADOT: "sora-polkadot",
    SCCP_DOMAIN_SORA2: "sora2",
}
SOLANA_BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
SOLANA_BASE58_INDEX = {
    symbol: index for index, symbol in enumerate(SOLANA_BASE58_ALPHABET)
}
REQUIRED_ARTIFACT_PATHS = (
    "sccp-release-readiness.md",
    "sccp-release-readiness.json",
    "sccp-all-lanes-summary.json",
    "sccp-release-notes-attachment.md",
)
ARTIFACT_KEYS = {"path", "bytes", "sha256"}
MANIFEST_KEYS = {
    "schema",
    "production_ready",
    "release_checklist_ready",
    "corridor_ready",
    "blockers",
    "artifacts",
}
READINESS_REPORT_KEYS = {
    "production_ready",
    "evidence",
    "release_checklist",
    "corridor",
    "blockers",
    "inputs",
    "input_artifacts",
    "cryptographic_evidence",
    "user_prover_submission_surfaces",
}
CRYPTOGRAPHIC_EVIDENCE_KEYS = {
    "domain",
    "chain",
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
    "destination_binding_hash",
    "route_allowlist_hash",
    "route_canary_evidence_hash",
    "route_canary_evidence_source",
    "route_canary_evidence_bound",
}
USER_PROVER_SUBMISSION_SURFACE_KEYS = {
    "lanes",
    "proof_backend",
    "sdk_helpers",
    "on_chain_submission",
    "required_phases",
    "validation_status",
    "validation_blockers",
}
RELEASE_CHECKLIST_KEYS = {"ready", "items"}
RELEASE_CHECKLIST_ITEM_KEYS = {"id", "title", "ready", "blockers"}
CORRIDOR_KEYS = {
    "production_ready",
    "phases",
    "evidence_artifacts",
    "require_phase_evidence",
    "blockers",
}
ALL_LANES_SUMMARY_KEYS = {
    "production_ready",
    "required_domains",
    "lanes",
    "blockers",
    "release_checklist",
}
ALL_LANES_LANE_KEYS = {
    "domain",
    "chain",
    "records",
    "production_ready",
    "source_record_hashes",
    "source_adapter_gate",
    "destination_binding",
    "route_allowlist",
    "blockers",
}
ALL_LANES_RECORD_KEYS = {
    "source_verifier_material",
    "source_adapter_deployment",
    "destination_rollout",
    "route_allowlist",
}
ALL_LANES_SOURCE_RECORD_HASH_KEYS = {
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
}
ALL_LANES_SOURCE_ADAPTER_GATE_KEYS = {
    "required",
    "ready",
    "gate_hash",
    "audit_hashes",
    "blockers",
}
ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN = {
    SCCP_DOMAIN_SOL: {
        "solana_tower_replay_verifier_hash",
        "solana_full_accountsdb_lattice_verifier_hash",
        "solana_bank_fork_choice_verifier_hash",
        "solana_full_light_client_gate_hash",
    },
    SCCP_DOMAIN_TON: {
        "ton_masterchain_config_verifier_hash",
        "ton_validator_set_transition_verifier_hash",
        "ton_shard_accounts_dictionary_verifier_hash",
        "ton_full_light_client_gate_hash",
    },
    SCCP_DOMAIN_TRON: {"tron_dpos_source_gate_hash"},
}
ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS = {
    "destination_binding_hash",
    "destination_binding_key",
    "expected_destination_binding_hash",
    "expected_destination_binding_hash_matches",
    "recomputed",
}
ALL_LANES_DESTINATION_BINDING_OPTIONAL_KEYS = {
    "destination_bridge_address",
    "destination_network_id",
}
ALL_LANES_DESTINATION_BINDING_KEYS = (
    ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS
    | ALL_LANES_DESTINATION_BINDING_OPTIONAL_KEYS
)
ALL_LANES_EVM_DESTINATION_DOMAINS = {SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC}
ALL_LANES_STATIC_DESTINATION_DOMAINS = {
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
}
ALL_LANES_ROUTE_ALLOWLIST_KEYS = {
    "route_allowlist_hash",
    "expected_route_allowlist_hash",
    "expected_route_allowlist_hash_matches",
    "route_canary",
}
ALL_LANES_ROUTE_CANARY_COMMON_KEYS = {
    "status",
    "evidence_hash",
    "evidence_source",
    "route_allowlist_hash",
    "destination_binding_hash",
    "evidence_bound",
}
ALL_LANES_EVM_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "transaction_hash",
    "log_index",
    "call_data_sha256",
    "message_id",
    "payload_hash",
    "target_domain",
    "statement_hash",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "proof_version",
    "proof_source_domain",
    "message_proof_used",
}
ALL_LANES_TRON_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "transaction_id",
    "transaction_owner_address",
    "log_index",
    "message_id",
    "call_data_sha256",
    "payload_hash",
    "target_domain",
    "statement_hash",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "proof_version",
    "proof_source_domain",
    "message_proof_used",
    "raw_data_owner_matches_transaction",
    "signature_sha256",
    "signature_recovered_address",
    "signature_recovers_to_owner",
}
ALL_LANES_SOLANA_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "solana_programdata_address",
    "solana_programdata_slot",
}
ALL_LANES_TON_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "ton_account_state_hash",
    "ton_last_transaction_hash",
    "ton_last_transaction_lt",
}
ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "substrate_finalized_head",
    "substrate_runtime_code_hash",
    "substrate_runtime_spec_version",
    "substrate_runtime_transaction_version",
}
ALL_LANES_ROUTE_CANARY_KEYS_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: ALL_LANES_EVM_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_BSC: ALL_LANES_EVM_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SOL: ALL_LANES_SOLANA_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_TON: ALL_LANES_TON_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_TRON: ALL_LANES_TRON_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA_KUSAMA: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA_POLKADOT: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA2: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
}
ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_BSC: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_SOL: "solana_live_programdata_snapshot",
    SCCP_DOMAIN_TON: "ton_live_account_snapshot",
    SCCP_DOMAIN_TRON: "tron_message_proof_accepted_transaction",
    SCCP_DOMAIN_SORA_KUSAMA: "substrate_finalized_runtime_snapshot",
    SCCP_DOMAIN_SORA_POLKADOT: "substrate_finalized_runtime_snapshot",
    SCCP_DOMAIN_SORA2: "substrate_finalized_runtime_snapshot",
}


class DuplicateJsonKeyError(ValueError):
    """Raised when a public JSON root contains a duplicate object key."""

    def __init__(self, key: str) -> None:
        super().__init__(key)
        self.key = key


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for key, value in pairs:
        if key in payload:
            raise DuplicateJsonKeyError(key)
        payload[key] = value
    return payload


def _load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_keys,
    )


def _canonical_json_text(payload: Any) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def _canonical_json_file_errors(label: str, path: Path, payload: Any) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"cannot load {label} JSON for canonical serialization check: {exc}"]
    if text != _canonical_json_text(payload):
        return [f"{label} JSON is not canonical release-bundle serialization"]
    return []


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _is_canonical_sha256_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(symbol in "0123456789abcdef" for symbol in value)
    )


def _load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _report_module() -> Any:
    return _load_module("_sccp_release_readiness_report", REPORT_SCRIPT)


def _bundle_module() -> Any:
    return _load_module("_sccp_release_bundle", BUNDLE_SCRIPT)


def _canonical_artifact_path(artifact: dict[str, Any]) -> tuple[str | None, list[str]]:
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return None, ["manifest artifact has no path"]
    if "\\" in artifact_path:
        return None, [f"manifest artifact path is not canonical: {artifact_path}"]
    path = PurePosixPath(artifact_path)
    if path.is_absolute() or ".." in path.parts:
        return None, [f"manifest artifact path escapes bundle: {artifact_path}"]
    if artifact_path != path.as_posix():
        return None, [f"manifest artifact path is not canonical: {artifact_path}"]
    return artifact_path, []


def _canonical_report_input_path_errors(value: Any) -> list[str]:
    if not isinstance(value, str) or not value:
        return ["readiness report inputs item must be a non-empty string"]
    if "\\" in value:
        return [f"readiness report inputs path is not canonical: {value}"]
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        return [f"readiness report inputs path escapes bundle: {value}"]
    if value != path.as_posix():
        return [f"readiness report inputs path is not canonical: {value}"]
    return []


def _canonical_report_artifact_path_errors(label: str, value: str) -> list[str]:
    if "\\" in value:
        return [f"{label} artifact path is not canonical: {value}"]
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        return [f"{label} artifact path escapes bundle: {value}"]
    if value != path.as_posix():
        return [f"{label} artifact path is not canonical: {value}"]
    return []


def _copied_input_layout_errors(label: str, index: int, value: Any) -> list[str]:
    if not isinstance(value, str) or _canonical_report_input_path_errors(value):
        return []
    expected_prefix = f"{index:02d}-"
    path = PurePosixPath(value)
    if (
        len(path.parts) != 2
        or path.parts[0] != "evidence"
        or not path.name.startswith(expected_prefix)
        or path.name == expected_prefix
        or not path.name.endswith(".toml")
    ):
        return [
            f"{label} path must use copied evidence layout "
            f"evidence/{expected_prefix}*.toml: {value}"
        ]
    return []


def _expected_phase_artifact_path(phase: str) -> str:
    return f"corridor/{phase}.log"


def _artifact_errors(bundle_dir: Path, artifact: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    artifact_path, path_errors = _canonical_artifact_path(artifact)
    for key in sorted(set(artifact) - ARTIFACT_KEYS):
        if artifact_path is None:
            errors.append(f"manifest artifact contains unknown field: {key}")
        else:
            errors.append(
                f"manifest artifact {artifact_path} contains unknown field: {key}"
            )
    if path_errors:
        return [*errors, *path_errors]
    assert artifact_path is not None
    path = bundle_dir.joinpath(*PurePosixPath(artifact_path).parts)
    current = bundle_dir
    for part in PurePosixPath(artifact_path).parts:
        current = current / part
        if current.is_symlink():
            return [f"bundle artifact path uses symlink: {artifact_path}"]
    if not path.is_file():
        return [f"missing bundle artifact: {artifact_path}"]
    expected_bytes = artifact.get("bytes")
    expected_hash = artifact.get("sha256")
    actual_bytes = path.stat().st_size
    actual_hash = _sha256(path)
    if type(expected_bytes) is not int or expected_bytes < 0:
        errors.append(f"{artifact_path} bytes must be a non-negative integer")
    elif expected_bytes != actual_bytes:
        errors.append(
            f"{artifact_path} byte length mismatch: expected {expected_bytes}, got {actual_bytes}"
        )
    if not _is_canonical_sha256_text(expected_hash):
        errors.append(f"{artifact_path} sha256 must be a canonical SHA-256 hex string")
    elif expected_hash != actual_hash:
        errors.append(
            f"{artifact_path} sha256 mismatch: expected {expected_hash}, got {actual_hash}"
        )
    return errors


def _manifest_artifacts_by_path(
    artifacts: list[Any],
    errors: list[str],
) -> dict[str, dict[str, Any]]:
    by_path: dict[str, dict[str, Any]] = {}
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if path_errors or artifact_path is None:
            continue
        if artifact_path in by_path:
            errors.append(f"duplicate manifest artifact path: {artifact_path}")
            continue
        by_path[artifact_path] = artifact
    return by_path


def _bundle_file_paths(bundle_dir: Path, errors: list[str]) -> set[str]:
    paths: set[str] = set()
    try:
        candidates = sorted(bundle_dir.rglob("*"))
    except OSError as exc:
        errors.append(f"cannot enumerate bundle files: {exc}")
        return paths
    for candidate in candidates:
        try:
            relative = candidate.relative_to(bundle_dir).as_posix()
        except ValueError:
            errors.append(f"bundle file escapes bundle root: {candidate}")
            continue
        if candidate.is_symlink():
            errors.append(f"bundle contains symlink: {relative}")
            continue
        if not candidate.is_file():
            continue
        relative_path = PurePosixPath(relative)
        if (
            "\\" in relative
            or relative_path.is_absolute()
            or ".." in relative_path.parts
        ):
            errors.append(f"bundle contains non-canonical file path: {relative}")
            continue
        paths.add(relative)
    return paths


def _check_report_artifact(
    errors: list[str],
    manifest_artifacts: dict[str, dict[str, Any]],
    artifact: Any,
    *,
    label: str,
) -> None:
    if not isinstance(artifact, dict):
        errors.append(f"{label} artifact is not an object")
        return
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        errors.append(f"{label} artifact has no path")
        return
    for key in sorted(set(artifact) - ARTIFACT_KEYS):
        errors.append(
            f"{label} artifact {artifact_path} contains unknown field: {key}"
        )
    expected_bytes = artifact.get("bytes")
    if type(expected_bytes) is not int or expected_bytes < 0:
        errors.append(
            f"{label} artifact bytes must be a non-negative integer for {artifact_path}"
        )
    expected_hash = artifact.get("sha256")
    if not _is_canonical_sha256_text(expected_hash):
        errors.append(
            f"{label} artifact sha256 must be a canonical SHA-256 hex string "
            f"for {artifact_path}"
        )
    path_errors = _canonical_report_artifact_path_errors(label, artifact_path)
    if path_errors:
        errors.extend(path_errors)
        return
    manifest_artifact = manifest_artifacts.get(artifact_path)
    if manifest_artifact is None:
        errors.append(f"{label} artifact is missing from manifest: {artifact_path}")
        return
    for field in ("bytes", "sha256"):
        if manifest_artifact.get(field) != artifact.get(field):
            errors.append(
                f"{label} artifact {field} mismatch for {artifact_path}: "
                f"manifest={manifest_artifact.get(field)!r}, "
                f"report={artifact.get(field)!r}"
            )


def _phase_transcript_errors(
    bundle_dir: Path,
    phase: str,
    artifact: Any,
) -> list[str]:
    if not isinstance(artifact, dict):
        return []
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return []
    canonical_path, path_errors = _canonical_artifact_path(artifact)
    if path_errors or canonical_path is None:
        return []
    path = bundle_dir.joinpath(*PurePosixPath(canonical_path).parts)
    try:
        transcript = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return [f"readiness report phase {phase} evidence artifact is not UTF-8 text"]
    except OSError as exc:
        return [
            f"readiness report phase {phase} evidence artifact cannot be read: {exc}"
        ]
    phase_marker = f"==> SCCP production corridor: {phase}"
    errors: list[str] = []
    if CORRIDOR_DRY_RUN_SENTINEL in transcript:
        errors.append(
            f"readiness report phase {phase} evidence artifact is a dry-run transcript"
        )
    if phase_marker not in transcript:
        errors.append(
            f"readiness report phase {phase} evidence artifact is missing the phase marker"
        )
    if CORRIDOR_COMPLETION_SENTINEL not in transcript:
        errors.append(
            "readiness report phase "
            f"{phase} evidence artifact is missing the completion sentinel"
        )
    return errors


def _expected_cryptographic_evidence(evidence: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for lane in evidence.get("lanes", []):
        if not isinstance(lane, dict):
            continue
        source_hashes = lane.get("source_record_hashes")
        if not isinstance(source_hashes, dict):
            source_hashes = {}
        destination_binding = lane.get("destination_binding")
        if not isinstance(destination_binding, dict):
            destination_binding = {}
        route_allowlist = lane.get("route_allowlist")
        if not isinstance(route_allowlist, dict):
            route_allowlist = {}
        route_canary = route_allowlist.get("route_canary")
        if not isinstance(route_canary, dict):
            route_canary = {}
        rows.append(
            {
                "domain": lane.get("domain"),
                "chain": lane.get("chain"),
                "source_verifier_material_hash": source_hashes.get(
                    "source_verifier_material_hash"
                ),
                "source_adapter_engine_deployment_hash": source_hashes.get(
                    "source_adapter_engine_deployment_hash"
                ),
                "destination_binding_hash": destination_binding.get(
                    "destination_binding_hash"
                ),
                "route_allowlist_hash": route_allowlist.get("route_allowlist_hash"),
                "route_canary_evidence_hash": route_canary.get("evidence_hash"),
                "route_canary_evidence_source": route_canary.get("evidence_source"),
                "route_canary_evidence_bound": bool(route_canary.get("evidence_bound")),
            }
        )
    return rows


def _expected_readiness_markdown(report: dict[str, Any]) -> str:
    module = _report_module()
    ordered_report = copy.deepcopy(report)
    corridor = ordered_report.get("corridor")
    if isinstance(corridor, dict):
        phase_order = module._corridor_phases()
        for field in ("phases", "evidence_artifacts"):
            values = corridor.get(field)
            if not isinstance(values, dict):
                continue
            ordered = {
                phase: values[phase]
                for phase in phase_order
                if phase in values
            }
            for phase in sorted(set(values) - set(ordered)):
                ordered[phase] = values[phase]
            corridor[field] = ordered
    return module._render_markdown(ordered_report, max_blockers_per_lane=4)


def _expected_release_notes_attachment(
    report: dict[str, Any],
    artifacts: list[Any],
) -> str:
    attachment_artifacts = [
        artifact
        for artifact in artifacts
        if (
            isinstance(artifact, dict)
            and artifact.get("path") != "sccp-release-notes-attachment.md"
        )
    ]
    module = _bundle_module()
    return module._release_notes_attachment(report, attachment_artifacts)


def _manifest_artifact_paths_in_order(artifacts: list[Any]) -> list[str]:
    paths: list[str] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.append(artifact_path)
    return paths


def _expected_manifest_artifact_order(report: dict[str, Any]) -> list[str]:
    paths = [
        "sccp-release-readiness.md",
        "sccp-release-readiness.json",
        "sccp-all-lanes-summary.json",
        *_expected_input_paths(report),
    ]

    corridor = report.get("corridor")
    if isinstance(corridor, dict):
        phases = corridor.get("phases")
        phase_artifacts = corridor.get("evidence_artifacts")
        if isinstance(phases, dict) and isinstance(phase_artifacts, dict):
            for phase in _report_module()._corridor_phases():
                if phases.get(phase) != "passed":
                    continue
                artifact = phase_artifacts.get(phase)
                if not isinstance(artifact, dict):
                    continue
                artifact_path, path_errors = _canonical_artifact_path(artifact)
                if not path_errors and artifact_path is not None:
                    paths.append(artifact_path)

    paths.append("sccp-release-notes-attachment.md")
    return paths


def _expected_submission_surfaces(report: dict[str, Any]) -> list[dict[str, Any]]:
    corridor = report.get("corridor")
    phase_status = {}
    if isinstance(corridor, dict) and isinstance(corridor.get("phases"), dict):
        phase_status = corridor["phases"]
    module = _report_module()
    return module._submission_surfaces(phase_status)


def _corridor_phase_errors(corridor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    try:
        expected_phases = _report_module()._corridor_phases()
    except Exception as exc:
        return [f"cannot load expected production corridor phases: {exc}"]
    phases = corridor.get("phases")
    if not isinstance(phases, dict):
        return ["readiness report corridor phases is not an object"]
    phase_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(phase_artifacts, dict):
        phase_artifacts = {}

    expected_set = set(expected_phases)
    for phase in sorted(set(phases) - expected_set):
        errors.append(f"readiness report corridor has unknown phase status: {phase}")
    for phase in expected_phases:
        if phase not in phases:
            errors.append(f"readiness report corridor missing phase status: {phase}")
            continue
        status = phases[phase]
        if status != "passed":
            errors.append(
                f"readiness report corridor phase {phase} is not passed: {status!r}"
            )
        artifact = phase_artifacts.get(phase)
        if not isinstance(artifact, dict):
            errors.append(
                "readiness report corridor phase "
                f"{phase} has no hashed evidence artifact"
            )
            continue
        expected_path = _expected_phase_artifact_path(phase)
        if artifact.get("path") != expected_path:
            errors.append(
                "readiness report phase "
                f"{phase} evidence artifact path must be {expected_path}"
            )
    if corridor.get("blockers"):
        errors.append("readiness report production corridor contains blockers")
    return errors


def _expected_release_checklist(report: dict[str, Any]) -> dict[str, Any]:
    evidence = report.get("evidence")
    if not isinstance(evidence, dict):
        return {"ready": False, "items": []}
    checklist = evidence.get("release_checklist")
    if isinstance(checklist, dict):
        return checklist
    return {
        "ready": bool(evidence.get("production_ready")),
        "items": [],
    }


def _boolean_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if type(payload.get(field)) is not bool:
        return [f"{label} {field} must be a boolean"]
    return []


def _list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if not isinstance(payload.get(field), list):
        return [f"{label} {field} must be a list"]
    return []


def _is_canonical_fixed_hex_text(value: Any, *, byte_length: int) -> bool:
    if not isinstance(value, str):
        return False
    if len(value) != 2 + byte_length * 2 or not value.startswith("0x"):
        return False
    text = value[2:]
    return all(symbol in "0123456789abcdef" for symbol in text)


def _is_canonical_hex32_text(value: Any) -> bool:
    return _is_canonical_fixed_hex_text(value, byte_length=32)


def _is_canonical_decimal_text(value: Any, *, positive: bool) -> bool:
    if not isinstance(value, str) or not value:
        return False
    if not all(symbol in "0123456789" for symbol in value):
        return False
    if len(value) > 1 and value.startswith("0"):
        return False
    if positive and value == "0":
        return False
    return True


def _u32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        return [f"{label} {field} must be a u32 integer"]
    return []


def _expected_u32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    expected: int,
    expected_label: str,
) -> list[str]:
    errors = _u32_field_errors(label, payload, field)
    if errors:
        return errors
    if field in payload and payload.get(field) != expected:
        return [f"{label} {field} must be {expected_label}"]
    return []


def _true_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    errors = _boolean_field_errors(label, payload, field)
    if errors:
        return errors
    if field in payload and payload.get(field) is not True:
        return [f"{label} {field} must be true"]
    return []


def _cryptographic_evidence_row_schema_errors(row: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for key in sorted(CRYPTOGRAPHIC_EVIDENCE_KEYS - set(row)):
        errors.append(f"readiness report cryptographic evidence row missing field: {key}")
    if "domain" in row and type(row.get("domain")) is not int:
        errors.append("readiness report cryptographic evidence row domain must be an integer")
    if "chain" in row and (
        not isinstance(row.get("chain"), str) or not row.get("chain")
    ):
        errors.append(
            "readiness report cryptographic evidence row chain must be a non-empty string"
        )
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        errors.extend(
            _nonzero_fixed_hex_field_errors(
                "readiness report cryptographic evidence row",
                row,
                field,
                byte_length=32,
                type_label="bytes32",
            )
        )
    if "route_canary_evidence_source" in row and (
        not isinstance(row.get("route_canary_evidence_source"), str)
        or not row.get("route_canary_evidence_source")
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_evidence_source must be a non-empty string"
        )
    if "route_canary_evidence_bound" in row and (
        type(row.get("route_canary_evidence_bound")) is not bool
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_evidence_bound must be a boolean"
        )
    return errors


def _cryptographic_evidence_lane_binding_errors(
    crypto: list[Any],
    lanes: Any,
) -> list[str]:
    errors: list[str] = []
    if not isinstance(lanes, list):
        return errors
    seen_domains: set[int] = set()
    for index, row in enumerate(crypto):
        if not isinstance(row, dict):
            continue
        domain = row.get("domain")
        if type(domain) is int:
            if domain in seen_domains:
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{index} duplicates domain {domain}"
                )
            seen_domains.add(domain)
        if index >= len(lanes) or not isinstance(lanes[index], dict):
            continue
        lane = lanes[index]
        lane_domain = lane.get("domain")
        if type(domain) is int and type(lane_domain) is int and domain != lane_domain:
            errors.append(
                "readiness report cryptographic evidence row "
                f"{index} domain must match lane domain"
            )
        chain = row.get("chain")
        lane_chain = lane.get("chain")
        if (
            isinstance(chain, str)
            and chain
            and isinstance(lane_chain, str)
            and lane_chain
            and chain != lane_chain
        ):
            errors.append(
                "readiness report cryptographic evidence row "
                f"{index} chain must match lane chain"
            )
        field_bindings = (
            (
                "source_verifier_material_hash",
                ("source_record_hashes", "source_verifier_material_hash"),
            ),
            (
                "source_adapter_engine_deployment_hash",
                ("source_record_hashes", "source_adapter_engine_deployment_hash"),
            ),
            ("destination_binding_hash", ("destination_binding", "destination_binding_hash")),
            ("route_allowlist_hash", ("route_allowlist", "route_allowlist_hash")),
            (
                "route_canary_evidence_hash",
                ("route_allowlist", "route_canary", "evidence_hash"),
            ),
            (
                "route_canary_evidence_source",
                ("route_allowlist", "route_canary", "evidence_source"),
            ),
            (
                "route_canary_evidence_bound",
                ("route_allowlist", "route_canary", "evidence_bound"),
            ),
        )
        for field, lane_path in field_bindings:
            if field not in row:
                continue
            expected: Any = lane
            for segment in lane_path:
                if not isinstance(expected, dict) or segment not in expected:
                    expected = None
                    break
                expected = expected[segment]
            if expected is not None and row.get(field) != expected:
                lane_field = ".".join(lane_path)
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{index} {field} must match embedded lane {lane_field}"
                )
    return errors


def _non_empty_string_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if not isinstance(payload.get(field), str) or not payload.get(field):
        return [f"{label} {field} must be a non-empty string"]
    return []


def _string_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, list) or (
        not allow_empty and not value
    ) or any(not isinstance(item, str) or not item for item in value):
        return [f"{label} {field} must be a list of non-empty strings"]
    return []


def _integer_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, list) or (
        not allow_empty and not value
    ) or any(type(item) is not int for item in value):
        return [f"{label} {field} must be a list of integers"]
    return []


def _submission_surface_row_schema_errors(row: dict[str, Any]) -> list[str]:
    label = "readiness report user prover submission surface row"
    errors: list[str] = []
    for field in ("lanes", "proof_backend", "sdk_helpers", "on_chain_submission"):
        errors.extend(_non_empty_string_field_errors(label, row, field))
    errors.extend(
        _string_list_field_errors(label, row, "required_phases", allow_empty=False)
    )
    if "validation_status" in row and row.get("validation_status") not in {
        "passed",
        "blocked",
    }:
        errors.append(
            "readiness report user prover submission surface row "
            "validation_status must be passed or blocked"
        )
    if row.get("validation_status") == "blocked":
        errors.append(
            "readiness report user prover submission surface row "
            "validation_status must be passed"
        )
    errors.extend(
        _string_list_field_errors(label, row, "validation_blockers", allow_empty=True)
    )
    blockers = row.get("validation_blockers")
    if isinstance(blockers, list) and blockers:
        errors.append(
            "readiness report user prover submission surface row "
            "validation_blockers must be empty"
        )
    return errors


def _exact_object_key_errors(
    label: str,
    payload: dict[str, Any],
    allowed_keys: set[str],
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(payload) - allowed_keys):
        errors.append(f"{label} contains unknown field: {key}")
    for key in sorted(allowed_keys - set(payload)):
        errors.append(f"{label} missing field: {key}")
    return errors


def _fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    if field not in payload:
        return []
    if not _is_canonical_fixed_hex_text(payload.get(field), byte_length=byte_length):
        return [f"{label} {field} must be a canonical {type_label} hex string"]
    return []


def _nonzero_fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    errors = _fixed_hex_field_errors(
        label,
        payload,
        field,
        byte_length=byte_length,
        type_label=type_label,
    )
    if errors or field not in payload:
        return errors
    value = payload.get(field)
    if isinstance(value, str) and all(char == "0" for char in value[2:]):
        return [
            f"{label} {field} must be a non-zero canonical {type_label} hex string"
        ]
    return []


def _empty_or_nonzero_fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    if field not in payload:
        return []
    if payload.get(field) == "":
        return []
    errors = _nonzero_fixed_hex_field_errors(
        label,
        payload,
        field,
        byte_length=byte_length,
        type_label=type_label,
    )
    if errors:
        return [
            f"{label} {field} must be empty or a non-zero canonical "
            f"{type_label} hex string"
        ]
    return []


def _source_adapter_gate_coherence_errors(
    label: str,
    lane: dict[str, Any],
    source_gate: dict[str, Any],
) -> list[str]:
    domain = lane.get("domain")
    required = source_gate.get("required")
    ready = source_gate.get("ready")
    gate_hash = source_gate.get("gate_hash")
    audit_hashes = source_gate.get("audit_hashes")
    blockers = source_gate.get("blockers")

    if type(required) is not bool:
        return []
    errors: list[str] = []
    expected_audit_keys = (
        ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.get(domain)
        if type(domain) is int
        else None
    )
    if expected_audit_keys is None:
        if required:
            errors.append(f"{label} required must be false for this lane domain")
            return errors
        if ready is not True:
            errors.append(f"{label} ready must be true when gate is not required")
        if isinstance(audit_hashes, dict) and audit_hashes:
            errors.append(
                f"{label} audit_hashes must be empty when gate is not required"
            )
        if gate_hash not in (None, ""):
            errors.append(f"{label} gate_hash must be empty when gate is not required")
        if isinstance(blockers, list) and blockers:
            errors.append(f"{label} blockers must be empty when gate is not required")
        return errors

    if not required:
        errors.append(f"{label} required must be true for this lane domain")
        return errors

    if isinstance(audit_hashes, dict):
        for key in sorted(set(audit_hashes) - expected_audit_keys):
            errors.append(f"{label} audit_hashes contains unexpected field: {key}")
        for key in sorted(expected_audit_keys - set(audit_hashes)):
            errors.append(f"{label} audit_hashes missing field: {key}")

    if type(ready) is not bool:
        return errors
    if not isinstance(blockers, list):
        return errors
    if not ready:
        errors.append(f"{label} ready must be true when gate is required")
    if blockers:
        errors.append(f"{label} blockers must be empty when gate is required")
    return errors


def _distinct_nonzero_hex_field_errors(
    label: str,
    fields: tuple[tuple[str, Any], ...],
    *,
    byte_length: int,
) -> list[str]:
    errors: list[str] = []
    seen: dict[str, str] = {}
    for field, value in fields:
        if (
            not _is_canonical_fixed_hex_text(value, byte_length=byte_length)
            or not isinstance(value, str)
            or all(char == "0" for char in value[2:])
        ):
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            errors.append(f"{label} {field} must not reuse {previous_field}")
            continue
        seen[value] = field
    return errors


def _decimal_text_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    positive: bool,
) -> list[str]:
    if field not in payload:
        return []
    if not _is_canonical_decimal_text(payload.get(field), positive=positive):
        qualifier = "positive " if positive else ""
        return [f"{label} {field} must be a canonical {qualifier}decimal string"]
    return []


def _tron_address_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not _is_canonical_tron_address_text(value):
        return [
            f"{label} {field} must be a non-zero canonical 0x41-prefixed "
            "21-byte hex string"
        ]
    return []


def _solana_pubkey_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, str):
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    try:
        raw = _decode_solana_base58(value)
    except ValueError:
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    if len(raw) != 32 or not any(raw):
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    return []


def _decode_solana_base58(value: str) -> bytes:
    if value != value.strip() or not value:
        raise ValueError("not canonical base58")
    numeric = 0
    for symbol in value:
        digit = SOLANA_BASE58_INDEX.get(symbol)
        if digit is None:
            raise ValueError("not canonical base58")
        numeric = numeric * 58 + digit
    leading_zeros = len(value) - len(value.lstrip("1"))
    payload = (
        b""
        if numeric == 0
        else numeric.to_bytes((numeric.bit_length() + 7) // 8, "big")
    )
    return (b"\x00" * leading_zeros) + payload


def _is_canonical_tron_address_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and _is_canonical_fixed_hex_text(value, byte_length=21)
        and value.startswith("0x41")
        and any(byte != "0" for byte in value[4:])
    )


def _matching_text_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    expected: Any,
    expected_field: str,
) -> list[str]:
    if field not in payload or not isinstance(payload.get(field), str):
        return []
    if not isinstance(expected, str):
        return []
    if payload.get(field) != expected:
        return [f"{label} {field} must match {expected_field}"]
    return []


def _canonical_nonzero_fixed_hex_value(value: Any, *, byte_length: int) -> str | None:
    if not _is_canonical_fixed_hex_text(value, byte_length=byte_length):
        return None
    assert isinstance(value, str)
    if all(char == "0" for char in value[2:]):
        return None
    return value


def _route_canary_common_hash_role_errors(
    label: str,
    lane: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    source_hashes = lane.get("source_record_hashes")
    route_allowlist = lane.get("route_allowlist")
    destination_binding = lane.get("destination_binding")
    fields: list[tuple[str, Any]] = []
    if isinstance(source_hashes, dict):
        fields.extend(
            (
                (field, source_hashes.get(field))
                for field in (
                    "source_verifier_material_hash",
                    "source_adapter_engine_deployment_hash",
                )
            )
        )
    if isinstance(route_allowlist, dict):
        fields.append(
            ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
        )
    if isinstance(destination_binding, dict):
        fields.append(
            (
                "destination_binding_hash",
                destination_binding.get("destination_binding_hash"),
            )
        )
    fields.append(("evidence_hash", route_canary.get("evidence_hash")))
    return _distinct_nonzero_hex_field_errors(
        f"{label} hash role",
        tuple(fields),
        byte_length=32,
    )


def _all_lanes_lane_label(label: str, index: int, lane: dict[str, Any]) -> str:
    domain = lane.get("domain")
    if type(domain) is int:
        return f"{label} lane domain {domain}"
    if isinstance(lane.get("chain"), str) and lane.get("chain"):
        return f"{label} lane {lane['chain']}"
    return f"{label} lane {index}"


def _all_lanes_route_canary_cross_lane_errors(
    label: str,
    lanes: Any,
) -> list[str]:
    if not isinstance(lanes, list):
        return []

    errors: list[str] = []
    governed_hashes: dict[str, tuple[str, str]] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            for field in (
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
            ):
                value = _canonical_nonzero_fixed_hex_value(
                    source_hashes.get(field),
                    byte_length=32,
                )
                if value is not None:
                    governed_hashes.setdefault(value, (lane_label, field))
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            value = _canonical_nonzero_fixed_hex_value(
                destination_binding.get("destination_binding_hash"),
                byte_length=32,
            )
            if value is not None:
                governed_hashes.setdefault(
                    value,
                    (lane_label, "destination_binding_hash"),
                )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            value = _canonical_nonzero_fixed_hex_value(
                route_allowlist.get("route_allowlist_hash"),
                byte_length=32,
            )
            if value is not None:
                governed_hashes.setdefault(value, (lane_label, "route_allowlist_hash"))

    seen_canaries: dict[str, str] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        route_allowlist = lane.get("route_allowlist")
        if not isinstance(route_allowlist, dict):
            continue
        route_canary = route_allowlist.get("route_canary")
        if not isinstance(route_canary, dict):
            continue
        evidence_hash = _canonical_nonzero_fixed_hex_value(
            route_canary.get("evidence_hash"),
            byte_length=32,
        )
        if evidence_hash is None:
            continue
        canary_label = f"{lane_label} route_allowlist route_canary"
        previous_canary_label = seen_canaries.get(evidence_hash)
        if previous_canary_label is not None:
            errors.append(
                f"{canary_label} evidence_hash must be distinct from "
                f"{previous_canary_label} route_canary evidence_hash"
            )
        else:
            seen_canaries[evidence_hash] = f"{lane_label} route_allowlist"
        governed = governed_hashes.get(evidence_hash)
        if governed is None:
            continue
        governed_lane_label, governed_field = governed
        if governed_lane_label == lane_label:
            continue
        errors.append(
            f"{canary_label} evidence_hash must not reuse {governed_field} "
            f"from {governed_lane_label}"
        )
    return errors


def _all_lanes_route_canary_schema_errors(
    label: str,
    lane: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    domain = lane.get("domain")
    expected_keys = ALL_LANES_ROUTE_CANARY_KEYS_BY_DOMAIN.get(
        domain,
        ALL_LANES_ROUTE_CANARY_COMMON_KEYS,
    )
    errors.extend(_exact_object_key_errors(label, route_canary, expected_keys))
    for field in (
        "evidence_hash",
        "route_allowlist_hash",
        "destination_binding_hash",
    ):
        errors.extend(
            _nonzero_fixed_hex_field_errors(
                label,
                route_canary,
                field,
                byte_length=32,
                type_label="bytes32",
            )
        )
    for field in ("status", "evidence_source"):
        errors.extend(_non_empty_string_field_errors(label, route_canary, field))
    if isinstance(route_canary.get("status"), str) and (
        route_canary.get("status") != "passed"
    ):
        errors.append(f"{label} status must be passed")
    expected_source = ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN.get(domain)
    if (
        expected_source is not None
        and isinstance(route_canary.get("evidence_source"), str)
        and route_canary.get("evidence_source") != expected_source
    ):
        errors.append(f"{label} evidence_source must be {expected_source}")
    errors.extend(_true_field_errors(label, route_canary, "evidence_bound"))
    route_allowlist = lane.get("route_allowlist")
    if isinstance(route_allowlist, dict):
        expected_route_hash = route_allowlist.get("route_allowlist_hash")
        if (
            isinstance(expected_route_hash, str)
            and isinstance(route_canary.get("route_allowlist_hash"), str)
            and route_canary.get("route_allowlist_hash") != expected_route_hash
        ):
            errors.append(
                f"{label} route_allowlist_hash must match lane "
                "route_allowlist_hash"
            )
    destination_binding = lane.get("destination_binding")
    if isinstance(destination_binding, dict):
        expected_destination_hash = destination_binding.get("destination_binding_hash")
        if (
            isinstance(expected_destination_hash, str)
            and isinstance(route_canary.get("destination_binding_hash"), str)
            and route_canary.get("destination_binding_hash") != expected_destination_hash
        ):
            errors.append(
                f"{label} destination_binding_hash must match lane "
                "destination_binding_hash"
            )
    errors.extend(_route_canary_common_hash_role_errors(label, lane, route_canary))

    if domain in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC):
        for field in (
            "transaction_hash",
            "call_data_sha256",
            "message_id",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
        ):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        evm_transcript_hash_fields = (
            "transaction_hash",
            "call_data_sha256",
            "message_id",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_block_hash",
            "evidence_hash",
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} transcript hash",
                tuple(
                    (field, route_canary.get(field))
                    for field in evm_transcript_hash_fields
                ),
                byte_length=32,
            )
        )
        governed_hash_fields: list[tuple[str, Any]] = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in evm_transcript_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
        errors.extend(_u32_field_errors(label, route_canary, "log_index"))
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "target_domain",
                domain,
                "the lane domain",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_version",
                1,
                "1",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_source_domain",
                SCCP_DOMAIN_SORA,
                "SORA",
            )
        )
        errors.extend(_true_field_errors(label, route_canary, "message_proof_used"))
    elif domain == SCCP_DOMAIN_TRON:
        for field in (
            "transaction_id",
            "message_id",
            "call_data_sha256",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "signature_sha256",
        ):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        for field in ("transaction_owner_address", "signature_recovered_address"):
            errors.extend(_tron_address_field_errors(label, route_canary, field))
        transaction_owner_address = route_canary.get("transaction_owner_address")
        signature_recovered_address = route_canary.get("signature_recovered_address")
        if (
            _is_canonical_tron_address_text(transaction_owner_address)
            and _is_canonical_tron_address_text(signature_recovered_address)
            and signature_recovered_address != transaction_owner_address
        ):
            errors.append(
                f"{label} signature_recovered_address must match "
                "transaction_owner_address"
            )
        tron_transcript_hash_fields = (
            "transaction_id",
            "message_id",
            "call_data_sha256",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_block_hash",
            "signature_sha256",
            "evidence_hash",
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} transcript hash",
                tuple(
                    (field, route_canary.get(field))
                    for field in tron_transcript_hash_fields
                ),
                byte_length=32,
            )
        )
        governed_hash_fields: list[tuple[str, Any]] = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in tron_transcript_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
        errors.extend(_u32_field_errors(label, route_canary, "log_index"))
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "target_domain",
                SCCP_DOMAIN_TRON,
                "TRON",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_version",
                1,
                "1",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_source_domain",
                SCCP_DOMAIN_SORA,
                "SORA",
            )
        )
        for field in (
            "message_proof_used",
            "raw_data_owner_matches_transaction",
            "signature_recovers_to_owner",
        ):
            errors.extend(_true_field_errors(label, route_canary, field))
    elif domain == SCCP_DOMAIN_SOL:
        errors.extend(
            _solana_pubkey_field_errors(
                label,
                route_canary,
                "solana_programdata_address",
            )
        )
        errors.extend(
            _decimal_text_field_errors(
                label,
                route_canary,
                "solana_programdata_slot",
                positive=True,
            )
        )
    elif domain == SCCP_DOMAIN_TON:
        for field in ("ton_account_state_hash", "ton_last_transaction_hash"):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        errors.extend(
            _decimal_text_field_errors(
                label,
                route_canary,
                "ton_last_transaction_lt",
                positive=True,
            )
        )
        ton_hash_fields = (
            "ton_account_state_hash",
            "ton_last_transaction_hash",
            "evidence_hash",
        )
        governed_hash_fields = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in ton_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
    elif domain in (
        SCCP_DOMAIN_SORA_KUSAMA,
        SCCP_DOMAIN_SORA_POLKADOT,
        SCCP_DOMAIN_SORA2,
    ):
        for field in ("substrate_finalized_head", "substrate_runtime_code_hash"):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        for field in (
            "substrate_runtime_spec_version",
            "substrate_runtime_transaction_version",
        ):
            errors.extend(
                _decimal_text_field_errors(
                    label,
                    route_canary,
                    field,
                    positive=False,
                )
            )
        substrate_hash_fields = (
            "substrate_finalized_head",
            "substrate_runtime_code_hash",
            "evidence_hash",
        )
        governed_hash_fields = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in substrate_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
    return errors


def _all_lanes_lane_schema_errors(label: str, lanes: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(lanes, list):
        return errors
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            errors.append(f"{label} lane {index} is not an object")
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        for key in sorted(set(lane) - ALL_LANES_LANE_KEYS):
            errors.append(f"{lane_label} contains unknown field: {key}")
        for key in sorted(ALL_LANES_LANE_KEYS - set(lane)):
            errors.append(f"{lane_label} missing field: {key}")
        if "domain" in lane and type(lane.get("domain")) is not int:
            errors.append(f"{lane_label} domain must be an integer")
        domain = lane.get("domain")
        expected_chain = (
            ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
            if type(domain) is int
            else None
        )
        if type(domain) is int and expected_chain is None:
            errors.append(f"{lane_label} domain must be a production remote domain")
        if "chain" in lane and (
            not isinstance(lane.get("chain"), str) or not lane.get("chain")
        ):
            errors.append(f"{lane_label} chain must be a non-empty string")
        elif expected_chain is not None and lane.get("chain") != expected_chain:
            errors.append(f"{lane_label} chain must be {expected_chain}")
        errors.extend(_true_field_errors(lane_label, lane, "production_ready"))
        for field in (
            "records",
            "source_record_hashes",
            "source_adapter_gate",
            "destination_binding",
            "route_allowlist",
        ):
            if field in lane and not isinstance(lane.get(field), dict):
                errors.append(f"{lane_label} {field} is not an object")
        errors.extend(
            _string_list_field_errors(lane_label, lane, "blockers", allow_empty=True)
        )
        blockers = lane.get("blockers")
        if isinstance(blockers, list) and blockers:
            errors.append(f"{lane_label} blockers must be empty")
        records = lane.get("records")
        if isinstance(records, dict):
            records_label = f"{lane_label} records"
            errors.extend(
                _exact_object_key_errors(
                    records_label,
                    records,
                    ALL_LANES_RECORD_KEYS,
                )
            )
            for field in ALL_LANES_RECORD_KEYS:
                if field in records:
                    errors.extend(_true_field_errors(records_label, records, field))
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            source_hashes_label = f"{lane_label} source_record_hashes"
            errors.extend(
                _exact_object_key_errors(
                    source_hashes_label,
                    source_hashes,
                    ALL_LANES_SOURCE_RECORD_HASH_KEYS,
                )
            )
            for field in ALL_LANES_SOURCE_RECORD_HASH_KEYS:
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        source_hashes_label,
                        source_hashes,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
        source_gate = lane.get("source_adapter_gate")
        if isinstance(source_gate, dict):
            source_gate_label = f"{lane_label} source_adapter_gate"
            errors.extend(
                _exact_object_key_errors(
                    source_gate_label,
                    source_gate,
                    ALL_LANES_SOURCE_ADAPTER_GATE_KEYS,
                )
            )
            errors.extend(
                _boolean_field_errors(source_gate_label, source_gate, "required")
            )
            errors.extend(
                _boolean_field_errors(source_gate_label, source_gate, "ready")
            )
            errors.extend(
                _empty_or_nonzero_fixed_hex_field_errors(
                    source_gate_label,
                    source_gate,
                    "gate_hash",
                    byte_length=32,
                    type_label="bytes32",
                )
            )
            gate_hash = source_gate.get("gate_hash")
            audit_hashes = source_gate.get("audit_hashes")
            if "audit_hashes" in source_gate and not isinstance(
                audit_hashes,
                dict,
            ):
                errors.append(f"{source_gate_label} audit_hashes is not an object")
            elif isinstance(audit_hashes, dict):
                for field, value in sorted(audit_hashes.items()):
                    if not isinstance(field, str) or not field:
                        errors.append(
                            f"{source_gate_label} audit_hashes contains an empty key"
                        )
                    elif not _is_canonical_fixed_hex_text(value, byte_length=32) or (
                        isinstance(value, str) and all(char == "0" for char in value[2:])
                    ):
                        errors.append(
                            f"{source_gate_label} audit_hashes {field} must be a "
                            "non-zero canonical bytes32 hex string"
                        )
            if source_gate.get("required") is True:
                if (
                    not _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
                    or (
                        isinstance(gate_hash, str)
                        and all(char == "0" for char in gate_hash[2:])
                    )
                ):
                    errors.append(
                        f"{source_gate_label} gate_hash must be a non-zero "
                        "canonical bytes32 hex string when required"
                    )
                if isinstance(audit_hashes, dict):
                    if not audit_hashes:
                        errors.append(
                            f"{source_gate_label} audit_hashes must not be empty "
                            "when required"
                        )
                    elif (
                        _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
                        and isinstance(gate_hash, str)
                        and any(char != "0" for char in gate_hash[2:])
                        and not any(
                            gate_hash == value for value in audit_hashes.values()
                        )
                    ):
                        errors.append(
                            f"{source_gate_label} gate_hash must match one "
                            "audit_hashes value"
                        )
            errors.extend(
                _string_list_field_errors(
                    source_gate_label,
                    source_gate,
                    "blockers",
                    allow_empty=True,
                )
            )
            blockers = source_gate.get("blockers")
            if (
                source_gate.get("ready") is True
                and isinstance(blockers, list)
                and blockers
            ):
                errors.append(f"{source_gate_label} blockers must be empty when ready")
            errors.extend(
                _source_adapter_gate_coherence_errors(
                    source_gate_label,
                    lane,
                    source_gate,
                )
            )
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            destination_label = f"{lane_label} destination_binding"
            for key in sorted(
                set(destination_binding) - ALL_LANES_DESTINATION_BINDING_KEYS
            ):
                errors.append(f"{destination_label} contains unknown field: {key}")
            for key in sorted(
                ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS
                - set(destination_binding)
            ):
                errors.append(f"{destination_label} missing field: {key}")
            errors.extend(
                _non_empty_string_field_errors(
                    destination_label,
                    destination_binding,
                    "destination_binding_key",
                )
            )
            for field in (
                "destination_binding_hash",
                "expected_destination_binding_hash",
                "destination_network_id",
            ):
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        destination_label,
                        destination_binding,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    destination_label,
                    destination_binding,
                    "destination_bridge_address",
                    byte_length=20,
                    type_label="20-byte",
                )
            )
            if domain in ALL_LANES_EVM_DESTINATION_DOMAINS:
                for field in ("destination_network_id", "destination_bridge_address"):
                    if field not in destination_binding:
                        errors.append(
                            f"{destination_label} {field} is required for "
                            "EVM-family lanes"
                        )
            elif domain == SCCP_DOMAIN_TRON:
                if "destination_network_id" not in destination_binding:
                    errors.append(
                        f"{destination_label} destination_network_id is required "
                        "for TRON lanes"
                    )
                if "destination_bridge_address" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_bridge_address is only "
                        "valid for EVM-family lanes"
                    )
            elif domain in ALL_LANES_STATIC_DESTINATION_DOMAINS:
                if "destination_network_id" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_network_id is only valid "
                        "for EVM-family or TRON lanes"
                    )
                if "destination_bridge_address" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_bridge_address is only "
                        "valid for EVM-family lanes"
                    )
            errors.extend(
                _matching_text_field_errors(
                    destination_label,
                    destination_binding,
                    "expected_destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                    "destination_binding_hash",
                )
            )
            errors.extend(
                _true_field_errors(
                    destination_label,
                    destination_binding,
                    "expected_destination_binding_hash_matches",
                )
            )
            errors.extend(
                _true_field_errors(
                    destination_label,
                    destination_binding,
                    "recomputed",
                )
            )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            route_label = f"{lane_label} route_allowlist"
            errors.extend(
                _exact_object_key_errors(
                    route_label,
                    route_allowlist,
                    ALL_LANES_ROUTE_ALLOWLIST_KEYS,
                )
            )
            for field in ("route_allowlist_hash", "expected_route_allowlist_hash"):
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        route_label,
                        route_allowlist,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
            errors.extend(
                _matching_text_field_errors(
                    route_label,
                    route_allowlist,
                    "expected_route_allowlist_hash",
                    route_allowlist.get("route_allowlist_hash"),
                    "route_allowlist_hash",
                )
            )
            errors.extend(
                _true_field_errors(
                    route_label,
                    route_allowlist,
                    "expected_route_allowlist_hash_matches",
                )
            )
            route_canary = route_allowlist.get("route_canary")
            if not isinstance(route_canary, dict):
                errors.append(f"{route_label} route_canary is not an object")
            else:
                canary_label = f"{route_label} route_canary"
                errors.extend(
                    _all_lanes_route_canary_schema_errors(
                        canary_label,
                        lane,
                        route_canary,
                    )
                )
                errors.extend(
                    _matching_text_field_errors(
                        canary_label,
                        route_canary,
                        "route_allowlist_hash",
                        route_allowlist.get("route_allowlist_hash"),
                        "lane route_allowlist_hash",
                    )
                )
                if isinstance(destination_binding, dict):
                    errors.extend(
                        _matching_text_field_errors(
                            canary_label,
                            route_canary,
                            "destination_binding_hash",
                            destination_binding.get("destination_binding_hash"),
                            "lane destination_binding_hash",
                        )
                    )
    return errors


def _all_lanes_summary_schema_errors(
    label: str,
    summary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(summary) - ALL_LANES_SUMMARY_KEYS):
        errors.append(f"{label} contains unknown field: {key}")
    for key in sorted(ALL_LANES_SUMMARY_KEYS - set(summary)):
        errors.append(f"{label} missing field: {key}")
    errors.extend(_boolean_field_errors(label, summary, "production_ready"))
    errors.extend(
        _integer_list_field_errors(
            label,
            summary,
            "required_domains",
            allow_empty=False,
        )
    )
    errors.extend(_list_field_errors(label, summary, "lanes"))
    errors.extend(_string_list_field_errors(label, summary, "blockers", allow_empty=True))
    blockers = summary.get("blockers")
    if isinstance(blockers, list) and blockers:
        errors.append(f"{label} blockers must be empty")
    errors.extend(_all_lanes_lane_schema_errors(label, summary.get("lanes")))
    errors.extend(_all_lanes_route_canary_cross_lane_errors(label, summary.get("lanes")))
    required_domains = summary.get("required_domains")
    lanes = summary.get("lanes")
    if (
        isinstance(required_domains, list)
        and all(type(domain) is int for domain in required_domains)
        and isinstance(lanes, list)
        and all(
            isinstance(lane, dict) and type(lane.get("domain")) is int
            for lane in lanes
        )
    ):
        lane_domains = [lane["domain"] for lane in lanes]
        if len(set(required_domains)) != len(required_domains):
            errors.append(f"{label} required_domains contains duplicate domains")
        if len(set(lane_domains)) != len(lane_domains):
            errors.append(f"{label} lanes contain duplicate domains")
        expected_domains = list(ALL_LANES_REQUIRED_DOMAINS)
        if required_domains != expected_domains:
            errors.append(
                f"{label} required_domains must be the production remote domains"
            )
        if lane_domains != expected_domains:
            errors.append(f"{label} lane domains must be the production remote domains")
        if required_domains != lane_domains:
            errors.append(f"{label} required_domains must match lane domains")
    if "release_checklist" in summary and not isinstance(
        summary.get("release_checklist"),
        dict,
    ):
        errors.append(f"{label} release_checklist is not an object")
    return errors


def _release_checklist_schema_errors(
    label: str,
    checklist: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(checklist) - RELEASE_CHECKLIST_KEYS):
        errors.append(f"{label} release_checklist contains unknown field: {key}")
    for key in sorted(RELEASE_CHECKLIST_KEYS - set(checklist)):
        errors.append(f"{label} release_checklist missing field: {key}")
    errors.extend(_true_field_errors(f"{label} release_checklist", checklist, "ready"))
    items = checklist.get("items")
    if not isinstance(items, list):
        errors.append(f"{label} release_checklist items is not a list")
        return errors
    for item in items:
        if not isinstance(item, dict):
            errors.append(f"{label} release_checklist item is not an object")
            continue
        item_id = item.get("id")
        item_label = (
            f"{label} release_checklist item {item_id}"
            if isinstance(item_id, str) and item_id
            else f"{label} release_checklist item"
        )
        for key in sorted(set(item) - RELEASE_CHECKLIST_ITEM_KEYS):
            errors.append(f"{item_label} contains unknown field: {key}")
        for key in sorted(RELEASE_CHECKLIST_ITEM_KEYS - set(item)):
            errors.append(f"{item_label} missing field: {key}")
        errors.extend(_non_empty_string_field_errors(item_label, item, "id"))
        errors.extend(_non_empty_string_field_errors(item_label, item, "title"))
        errors.extend(_true_field_errors(item_label, item, "ready"))
        errors.extend(
            _string_list_field_errors(item_label, item, "blockers", allow_empty=True)
        )
        blockers = item.get("blockers")
        if isinstance(blockers, list) and blockers:
            errors.append(f"{item_label} blockers must be empty")
    return errors


def _corridor_schema_errors(corridor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(corridor) - CORRIDOR_KEYS):
        errors.append(f"readiness report corridor contains unknown field: {key}")
    for key in sorted(CORRIDOR_KEYS - set(corridor)):
        errors.append(f"readiness report corridor missing field: {key}")
    if type(corridor.get("production_ready")) is not bool:
        errors.append("readiness report corridor production_ready is not a boolean")
    if type(corridor.get("require_phase_evidence")) is not bool:
        errors.append("readiness report corridor require_phase_evidence is not a boolean")
    if not isinstance(corridor.get("phases"), dict):
        errors.append("readiness report corridor phases is not an object")
    if not isinstance(corridor.get("evidence_artifacts"), dict):
        errors.append("readiness report corridor evidence_artifacts is not an object")
    errors.extend(
        _string_list_field_errors(
            "readiness report corridor",
            corridor,
            "blockers",
            allow_empty=True,
        )
    )
    blockers = corridor.get("blockers")
    if isinstance(blockers, list) and blockers:
        errors.append("readiness report corridor blockers must be empty")
    return errors


def _expected_input_paths(report: dict[str, Any]) -> list[str]:
    paths: list[str] = []
    input_artifacts = report.get("input_artifacts")
    if not isinstance(input_artifacts, list):
        return paths
    for artifact in input_artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.append(artifact_path)
    return paths


def _input_provenance_schema_errors(report: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    inputs = report.get("inputs")
    if not isinstance(inputs, list) or not inputs:
        errors.append(
            "readiness report inputs must be a non-empty list of canonical paths"
        )
    else:
        seen_inputs: set[str] = set()
        for index, item in enumerate(inputs):
            item_errors = _canonical_report_input_path_errors(item)
            if item_errors:
                errors.extend(item_errors)
            errors.extend(
                _copied_input_layout_errors("readiness report inputs", index, item)
            )
            if isinstance(item, str) and item in seen_inputs:
                errors.append(
                    f"readiness report inputs contains duplicate path: {item}"
                )
            if isinstance(item, str):
                seen_inputs.add(item)

    input_artifacts = report.get("input_artifacts")
    if isinstance(input_artifacts, list):
        seen_artifacts: set[str] = set()
        for index, artifact in enumerate(input_artifacts):
            if not isinstance(artifact, dict):
                continue
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if path_errors or artifact_path is None:
                continue
            errors.extend(
                _copied_input_layout_errors(
                    "readiness report input_artifacts",
                    index,
                    artifact_path,
                )
            )
            if artifact_path in seen_artifacts:
                errors.append(
                    "readiness report input_artifacts contains duplicate path: "
                    f"{artifact_path}"
                )
            seen_artifacts.add(artifact_path)
    return errors


def _bundle_artifact_path(bundle_dir: Path, artifact: dict[str, Any]) -> Path | None:
    artifact_path, path_errors = _canonical_artifact_path(artifact)
    if path_errors or artifact_path is None:
        return None
    return bundle_dir.joinpath(*PurePosixPath(artifact_path).parts)


def _copied_input_summary(
    bundle_dir: Path,
    report: dict[str, Any],
    errors: list[str],
) -> dict[str, Any] | None:
    input_paths: list[Path] = []
    input_artifacts = report.get("input_artifacts")
    if not isinstance(input_artifacts, list) or not input_artifacts:
        errors.append("readiness report input_artifacts must be a non-empty list")
        return None
    for index, artifact in enumerate(input_artifacts):
        if not isinstance(artifact, dict):
            errors.append(f"readiness report input artifact {index} is not an object")
            return None
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if path_errors or artifact_path is None:
            errors.extend(
                f"readiness report input artifact {index}: {error}"
                for error in path_errors
            )
            return None
        path = _bundle_artifact_path(bundle_dir, artifact)
        if path is not None:
            input_paths.append(path)
    if not input_paths:
        errors.append("readiness report has no usable copied evidence inputs")
        return None
    module = _bundle_module()
    return module._all_lanes_summary(input_paths)


def _referenced_report_artifact_paths(report: dict[str, Any]) -> set[str]:
    paths = set(REQUIRED_ARTIFACT_PATHS)
    input_artifacts = report.get("input_artifacts")
    if isinstance(input_artifacts, list):
        for artifact in input_artifacts:
            if not isinstance(artifact, dict):
                continue
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if not path_errors and artifact_path is not None:
                paths.add(artifact_path)

    corridor = report.get("corridor")
    if not isinstance(corridor, dict):
        return paths
    phase_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(phase_artifacts, dict):
        return paths
    phases = corridor.get("phases")
    if not isinstance(phases, dict):
        return paths
    for phase, status in phases.items():
        if status != "passed":
            continue
        artifact = phase_artifacts.get(phase)
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.add(artifact_path)
    return paths


def verify_bundle(bundle_dir: Path) -> dict[str, Any]:
    """Return a verification summary for an SCCP release bundle."""

    errors: list[str] = []
    manifest_path = bundle_dir / "manifest.json"
    manifest_sha256: str | None = None
    if manifest_path.is_symlink():
        return {
            "verified": False,
            "errors": ["manifest is a symlink: manifest.json"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    if not manifest_path.is_file():
        return {
            "verified": False,
            "errors": [f"missing manifest: {manifest_path}"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    try:
        manifest_sha256 = _sha256(manifest_path)
    except OSError:
        manifest_sha256 = None
    try:
        manifest = _load_json(manifest_path)
    except json.JSONDecodeError as exc:
        return {
            "verified": False,
            "errors": [f"manifest is not valid JSON: {exc}"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    except DuplicateJsonKeyError as exc:
        return {
            "verified": False,
            "errors": [f"manifest JSON contains duplicate key: {exc.key}"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    if not isinstance(manifest, dict):
        return {
            "verified": False,
            "errors": ["manifest is not a JSON object"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    errors.extend(_canonical_json_file_errors("manifest", manifest_path, manifest))

    for key in sorted(set(manifest) - MANIFEST_KEYS):
        errors.append(f"manifest contains unknown top-level field: {key}")
    for key in sorted(MANIFEST_KEYS - set(manifest)):
        errors.append(f"manifest missing top-level field: {key}")
    if manifest.get("schema") != SCHEMA:
        errors.append(f"unexpected manifest schema: {manifest.get('schema')}")
    errors.extend(_boolean_field_errors("manifest", manifest, "production_ready"))
    errors.extend(
        _boolean_field_errors("manifest", manifest, "release_checklist_ready")
    )
    errors.extend(_boolean_field_errors("manifest", manifest, "corridor_ready"))
    errors.extend(_string_list_field_errors("manifest", manifest, "blockers", allow_empty=True))
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append("manifest artifacts must be a non-empty list")
        artifacts = []
    manifest_artifacts = _manifest_artifacts_by_path(artifacts, errors)
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            errors.append("manifest artifact entry is not an object")
            continue
        errors.extend(_artifact_errors(bundle_dir, artifact))
    bundle_paths = _bundle_file_paths(bundle_dir, errors)
    expected_paths = set(manifest_artifacts) | {"manifest.json"}
    for unexpected in sorted(bundle_paths - expected_paths):
        errors.append(f"bundle contains unmanifested artifact: {unexpected}")
    for missing in sorted(expected_paths - bundle_paths):
        errors.append(f"bundle is missing expected artifact file: {missing}")

    for required_path in REQUIRED_ARTIFACT_PATHS:
        if required_path not in manifest_artifacts:
            errors.append(f"manifest missing required artifact: {required_path}")

    report_md_path = bundle_dir / "sccp-release-readiness.md"
    report_path = bundle_dir / "sccp-release-readiness.json"
    summary_path = bundle_dir / "sccp-all-lanes-summary.json"
    notes_path = bundle_dir / "sccp-release-notes-attachment.md"
    try:
        report = _load_json(report_path)
    except DuplicateJsonKeyError as exc:
        report = {}
        errors.append(f"readiness report JSON contains duplicate key: {exc.key}")
    except (OSError, json.JSONDecodeError) as exc:
        report = {}
        errors.append(f"cannot load readiness report JSON: {exc}")
    if not isinstance(report, dict) or not report:
        errors.append("readiness report JSON must be a non-empty object")
        report = {}
    else:
        errors.extend(
            _canonical_json_file_errors("readiness report", report_path, report)
        )
    for key in sorted(set(report) - READINESS_REPORT_KEYS):
        errors.append(
            f"readiness report contains unknown top-level field: {key}"
        )
    if report:
        for key in sorted(READINESS_REPORT_KEYS - set(report)):
            errors.append(f"readiness report missing top-level field: {key}")
    try:
        summary = _load_json(summary_path)
    except DuplicateJsonKeyError as exc:
        summary = {}
        errors.append(f"all-lanes summary JSON contains duplicate key: {exc.key}")
    except (OSError, json.JSONDecodeError) as exc:
        summary = {}
        errors.append(f"cannot load all-lanes summary JSON: {exc}")
    if not isinstance(summary, dict) or not summary:
        errors.append("all-lanes summary JSON must be a non-empty object")
        summary = {}
    else:
        errors.extend(
            _canonical_json_file_errors("all-lanes summary", summary_path, summary)
        )
    report_evidence: dict[str, Any] = {}
    report_release_checklist: dict[str, Any] = {}
    report_corridor: dict[str, Any] = {}
    summary_release_checklist: dict[str, Any] = {}
    if report:
        errors.extend(
            _boolean_field_errors("readiness report", report, "production_ready")
        )
        errors.extend(
            _string_list_field_errors(
                "readiness report",
                report,
                "blockers",
                allow_empty=True,
            )
        )
        raw_evidence = report.get("evidence")
        if not isinstance(raw_evidence, dict):
            errors.append("readiness report evidence is not an object")
        else:
            report_evidence = raw_evidence
            errors.extend(
                _all_lanes_summary_schema_errors(
                    "readiness report embedded evidence",
                    report_evidence,
                )
            )
        raw_release_checklist = report.get("release_checklist")
        if not isinstance(raw_release_checklist, dict):
            errors.append("readiness report release_checklist is not an object")
        else:
            report_release_checklist = raw_release_checklist
            errors.extend(
                _release_checklist_schema_errors(
                    "readiness report",
                    report_release_checklist,
                )
            )
        raw_corridor = report.get("corridor")
        if not isinstance(raw_corridor, dict):
            errors.append("readiness report corridor is not an object")
        else:
            report_corridor = raw_corridor
            errors.extend(_corridor_schema_errors(report_corridor))
    if summary:
        errors.extend(
            _all_lanes_summary_schema_errors("all-lanes summary", summary)
        )
        raw_summary_checklist = summary.get("release_checklist")
        if not isinstance(raw_summary_checklist, dict):
            errors.append("all-lanes summary release_checklist is not an object")
        else:
            summary_release_checklist = raw_summary_checklist
            errors.extend(
                _release_checklist_schema_errors(
                    "all-lanes summary",
                    summary_release_checklist,
                )
            )
    if report:
        referenced_paths = _referenced_report_artifact_paths(report)
        for unexpected in sorted(set(manifest_artifacts) - referenced_paths):
            errors.append(
                "manifest contains artifact not referenced by readiness report: "
                f"{unexpected}"
            )
        for missing in sorted(referenced_paths - set(manifest_artifacts)):
            errors.append(
                "manifest missing readiness report referenced artifact: "
                f"{missing}"
            )
        try:
            expected_order = _expected_manifest_artifact_order(report)
        except Exception as exc:
            errors.append(f"cannot compute canonical manifest artifact order: {exc}")
        else:
            if _manifest_artifact_paths_in_order(artifacts) != expected_order:
                errors.append(
                    "manifest artifact order does not match canonical "
                    "release bundle order"
                )
    if report:
        try:
            report_markdown = report_md_path.read_text(encoding="utf-8")
        except OSError as exc:
            errors.append(f"cannot load readiness report Markdown: {exc}")
        else:
            try:
                expected_markdown = _expected_readiness_markdown(report)
            except Exception as exc:
                errors.append(f"cannot render readiness report Markdown: {exc}")
            else:
                if report_markdown != expected_markdown:
                    errors.append(
                        "readiness report Markdown does not match readiness report JSON"
                    )

    if report and not report.get("production_ready"):
        errors.append("readiness report is not production_ready")
    if report and report.get("blockers"):
        errors.append("readiness report contains blockers")
    if report:
        errors.extend(_input_provenance_schema_errors(report))
        report_inputs = report.get("inputs")
        if isinstance(report_inputs, list) and report_inputs != _expected_input_paths(report):
            errors.append(
                "readiness report inputs do not match copied input artifacts"
            )
    if report and not report_evidence.get("production_ready"):
        errors.append("readiness report embedded evidence is not production_ready")
    if report and not report_release_checklist.get("ready"):
        errors.append("readiness report release_checklist is not ready")
    if report and report_release_checklist != _expected_release_checklist(report):
        errors.append(
            "readiness report release_checklist does not match embedded evidence"
        )
    if report and not report_corridor.get("production_ready"):
        errors.append("readiness report production corridor is not ready")
    if report and report_corridor.get("require_phase_evidence") is not True:
        errors.append("readiness report does not require hashed phase evidence")
    if report:
        errors.extend(_corridor_phase_errors(report_corridor))
    if summary and not summary.get("production_ready"):
        errors.append("all-lanes summary is not production_ready")
    if summary and not summary_release_checklist.get("ready"):
        errors.append("all-lanes summary release_checklist is not ready")
    if report and summary and report_evidence != summary:
        errors.append("all-lanes summary does not match readiness report evidence")
    if report:
        try:
            copied_summary = _copied_input_summary(bundle_dir, report, errors)
        except Exception as exc:
            copied_summary = None
            errors.append(
                f"cannot recompute all-lanes summary from copied evidence: {exc}"
            )
        if copied_summary is not None:
            if summary and copied_summary != summary:
                errors.append(
                    "all-lanes summary does not match copied evidence inputs"
                )
            if report_evidence != copied_summary:
                errors.append(
                    "readiness report evidence does not match copied evidence inputs"
                )
    if report:
        report_input_artifacts = report.get("input_artifacts")
        if not isinstance(report_input_artifacts, list):
            report_input_artifacts = []
        for artifact in report_input_artifacts:
            _check_report_artifact(
                errors,
                manifest_artifacts,
                artifact,
                label="readiness report input",
            )
        corridor = report_corridor
        phase_artifacts = corridor.get("evidence_artifacts", {})
        if not isinstance(phase_artifacts, dict):
            errors.append("readiness report corridor evidence_artifacts is not an object")
            phase_artifacts = {}
        phases = corridor.get("phases", {})
        if isinstance(phases, dict):
            for phase in sorted(set(phase_artifacts) - set(phases)):
                errors.append(
                    "readiness report corridor has evidence artifact for "
                    f"unknown phase: {phase}"
                )
            for phase, status in phases.items():
                if status != "passed":
                    continue
                _check_report_artifact(
                    errors,
                    manifest_artifacts,
                    phase_artifacts.get(phase),
                    label=f"readiness report phase {phase}",
                )
                errors.extend(
                    _phase_transcript_errors(
                        bundle_dir,
                        phase,
                        phase_artifacts.get(phase),
                    )
                )
        else:
            errors.append("readiness report corridor phases is not an object")
        crypto = report.get("cryptographic_evidence")
        lanes = report_evidence.get("lanes", [])
        if not isinstance(crypto, list) or not crypto:
            errors.append("readiness report cryptographic_evidence is missing")
        elif isinstance(lanes, list) and len(crypto) != len(lanes):
            errors.append("readiness report cryptographic_evidence does not cover every lane")
        if isinstance(crypto, list):
            errors.extend(_cryptographic_evidence_lane_binding_errors(crypto, lanes))
            expected_crypto = _expected_cryptographic_evidence(report_evidence)
            if crypto != expected_crypto:
                errors.append(
                    "readiness report cryptographic_evidence does not match embedded lane evidence"
                )
        surfaces = report.get("user_prover_submission_surfaces")
        if not isinstance(surfaces, list) or not surfaces:
            errors.append("readiness report user_prover_submission_surfaces is missing")
        else:
            for row in surfaces:
                if not isinstance(row, dict):
                    errors.append(
                        "readiness report user prover submission surface row "
                        "is not an object"
                    )
                    continue
                for key in sorted(set(row) - USER_PROVER_SUBMISSION_SURFACE_KEYS):
                    errors.append(
                        "readiness report user prover submission surface row "
                        f"contains unknown field: {key}"
                    )
                for key in sorted(USER_PROVER_SUBMISSION_SURFACE_KEYS - set(row)):
                    errors.append(
                        "readiness report user prover submission surface row "
                        f"missing field: {key}"
                    )
                errors.extend(_submission_surface_row_schema_errors(row))
            try:
                expected_surfaces = _expected_submission_surfaces(report)
            except Exception as exc:
                errors.append(
                    f"cannot render user prover submission surfaces: {exc}"
                )
            else:
                if surfaces != expected_surfaces:
                    errors.append(
                        "readiness report user_prover_submission_surfaces "
                        "does not match corridor phases"
                    )
        if isinstance(crypto, list):
            for row in crypto:
                if not isinstance(row, dict):
                    errors.append("readiness report cryptographic evidence row is not an object")
                    continue
                for key in sorted(set(row) - CRYPTOGRAPHIC_EVIDENCE_KEYS):
                    errors.append(
                        "readiness report cryptographic evidence row contains "
                        f"unknown field: {key}"
                    )
                errors.extend(_cryptographic_evidence_row_schema_errors(row))
                if row.get("route_canary_evidence_bound") is not True:
                    errors.append(
                        "readiness report cryptographic evidence row has unbound route canary"
                    )
                for field in (
                    "source_verifier_material_hash",
                    "source_adapter_engine_deployment_hash",
                    "destination_binding_hash",
                    "route_allowlist_hash",
                    "route_canary_evidence_hash",
                    "route_canary_evidence_source",
                ):
                    if not row.get(field):
                        errors.append(
                            "readiness report cryptographic evidence row missing "
                            f"{field}"
                        )
    if manifest.get("production_ready") is not True:
        errors.append("manifest production_ready is not true")
    if manifest.get("release_checklist_ready") is not True:
        errors.append("manifest release_checklist_ready is not true")
    if manifest.get("corridor_ready") is not True:
        errors.append("manifest corridor_ready is not true")
    if manifest.get("blockers"):
        errors.append("manifest contains blockers")
    if report:
        if manifest.get("production_ready") != report.get("production_ready"):
            errors.append(
                "manifest production_ready does not match readiness report"
            )
        if manifest.get("blockers") != report.get("blockers"):
            errors.append("manifest blockers do not match readiness report blockers")
        if report_release_checklist:
            if manifest.get("release_checklist_ready") != report_release_checklist.get(
                "ready"
            ):
                errors.append(
                    "manifest release_checklist_ready does not match "
                    "readiness report release_checklist"
                )
        if report_corridor:
            if manifest.get("corridor_ready") != report_corridor.get(
                "production_ready"
            ):
                errors.append(
                    "manifest corridor_ready does not match readiness report corridor"
                )
    if summary:
        if manifest.get("production_ready") != summary.get("production_ready"):
            errors.append("manifest production_ready does not match all-lanes summary")
        if summary_release_checklist:
            if manifest.get("release_checklist_ready") != summary_release_checklist.get(
                "ready"
            ):
                errors.append(
                    "manifest release_checklist_ready does not match "
                    "all-lanes summary release_checklist"
                )

    try:
        notes = notes_path.read_text(encoding="utf-8")
    except OSError as exc:
        notes = ""
        errors.append(f"cannot load release-notes attachment: {exc}")
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        path = artifact.get("path")
        digest = artifact.get("sha256")
        if path == "sccp-release-notes-attachment.md":
            continue
        if isinstance(path, str) and path not in notes:
            errors.append(f"release notes attachment does not list {path}")
        if isinstance(digest, str) and digest not in notes:
            errors.append(f"release notes attachment does not list hash for {path}")
    if "manifest.json" not in notes:
        errors.append("release notes attachment does not list manifest.json")
    if report and notes:
        try:
            expected_notes = _expected_release_notes_attachment(report, artifacts)
        except Exception as exc:
            errors.append(f"cannot render release-notes attachment: {exc}")
        else:
            if notes != expected_notes:
                errors.append(
                    "release notes attachment does not match manifest and report"
                )

    return {
        "verified": not errors,
        "errors": errors,
        "artifacts": artifacts,
        "manifest_sha256": manifest_sha256,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify a generated SCCP release-note attachment bundle.",
    )
    parser.add_argument("bundle_dir", type=Path, help="Bundle directory to verify.")
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print the verification summary as JSON.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    summary = verify_bundle(args.bundle_dir)
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    elif summary["verified"]:
        print(f"SCCP release bundle verified: {args.bundle_dir}")
    else:
        print(f"SCCP release bundle verification failed: {args.bundle_dir}")
        for error in summary["errors"]:
            print(f"- {error}")
    return 0 if summary["verified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
