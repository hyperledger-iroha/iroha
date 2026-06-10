#!/usr/bin/env python3
"""Build hash-bound SCCP public release-note attachments."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import stat
import sys
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[1]
ALL_LANES_SCRIPT = ROOT / "scripts" / "sccp_all_lanes_evidence.py"
REPORT_SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
VERIFY_SCRIPT = ROOT / "scripts" / "sccp_verify_release_bundle.py"


class DuplicateJsonKeyError(ValueError):
    """Raised when a JSON object contains a duplicate key."""

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


def _load_json_without_duplicate_keys(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_keys,
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


def _all_lanes_module() -> Any:
    return _load_module("_sccp_all_lanes_evidence", ALL_LANES_SCRIPT)


def _verify_module() -> Any:
    return _load_module("_sccp_verify_release_bundle", VERIFY_SCRIPT)


def _path_control_character(path: str) -> str | None:
    for character in path:
        if ord(character) < 0x20 or ord(character) == 0x7F:
            return repr(character)
    return None


MARKDOWN_UNSAFE_PATH_CHARACTERS = frozenset("|`<>")
NATIVE_EVM_PROVER_FORBIDDEN_PATH_MARKERS = (
    "webassembly",
    "wasm",
    "snarkjs",
    "remoteprover",
    "remote-prover",
    "remote_prover",
    "remote prover",
    "prover-url",
    "prover_url",
    "proverendpoint",
    "prover-endpoint",
    "prover_endpoint",
    "prover endpoint",
)


def _path_markdown_unsafe_character(path: str) -> str | None:
    for character in path:
        if character in MARKDOWN_UNSAFE_PATH_CHARACTERS:
            return repr(character)
    return None


def _native_evm_prover_duplicate_json_key_error(key: Any) -> str:
    label = "native EVM Groth16 prover bundle"
    if not isinstance(key, str) or not key:
        return f"{label} JSON contains malformed duplicate key"
    if _path_control_character(key) is not None:
        return f"{label} JSON contains duplicate key with control character"
    if not key.isascii():
        return f"{label} JSON contains duplicate key with non-ASCII character"
    if key.strip() != key:
        return f"{label} JSON contains duplicate key with surrounding whitespace"
    if any(character.isspace() for character in key):
        return f"{label} JSON contains duplicate key with whitespace"
    if _path_markdown_unsafe_character(key) is not None:
        return f"{label} JSON contains duplicate key with Markdown-unsafe character"
    return f"{label} JSON contains duplicate key: {key}"


def _path_percent_encoded_traversal(path: str) -> str | None:
    decoded = path
    seen = {decoded}
    for _ in range(32):
        if "%" not in decoded:
            return None
        decoded = unquote(decoded)
        if decoded in seen:
            return None
        seen.add(decoded)
        decoded_path = PurePosixPath(decoded)
        if (
            decoded_path.is_absolute()
            or ".." in decoded_path.parts
            or "\\" in decoded
            or decoded != decoded_path.as_posix()
        ):
            return repr(path)
    if "%" in decoded:
        return repr(path)
    return None


def _artifact(path: Path, root: Path) -> dict[str, Any]:
    payload = path.read_bytes()
    artifact_path = path.relative_to(root).as_posix()
    if artifact_path.strip() != artifact_path:
        raise ValueError(
            "release artifact path must not contain surrounding whitespace: "
            f"{artifact_path!r}"
        )
    control_character = _path_control_character(artifact_path)
    if control_character is not None:
        raise ValueError(
            "release artifact path contains control character "
            f"{control_character}: {artifact_path!r}"
        )
    markdown_unsafe_character = _path_markdown_unsafe_character(artifact_path)
    if markdown_unsafe_character is not None:
        raise ValueError(
            "release artifact path contains Markdown-unsafe character "
            f"{markdown_unsafe_character}: {artifact_path!r}"
        )
    percent_traversal = _path_percent_encoded_traversal(artifact_path)
    if percent_traversal is not None:
        raise ValueError(
            "release artifact path contains percent-encoded traversal segment: "
            f"{percent_traversal}"
        )
    return {
        "path": artifact_path,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def _copy_file(source: Path, destination: Path) -> Path:
    if source.is_symlink():
        raise ValueError(f"release bundle source path must not be a symlink: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    return destination


def _safe_name(path: Path, index: int) -> str:
    name = path.name.replace("/", "_").replace("\\", "_")
    markdown_unsafe_character = _path_markdown_unsafe_character(name)
    if markdown_unsafe_character is not None:
        raise ValueError(
            "release bundle copied filename contains Markdown-unsafe character "
            f"{markdown_unsafe_character}: {name!r}"
        )
    return f"{index:02d}-{name}"


def _copy_evidence_inputs(paths: list[Path], output_dir: Path) -> list[Path]:
    evidence_dir = output_dir / "evidence"
    return [
        _copy_file(path, evidence_dir / _safe_name(path, index))
        for index, path in enumerate(paths)
    ]


def _parse_phase_evidence_arg(raw: str) -> tuple[str, Path]:
    if "=" not in raw:
        raise argparse.ArgumentTypeError(
            f"phase evidence must use NAME=PATH syntax: {raw}"
        )
    name, path_text = raw.split("=", 1)
    name = name.strip()
    if not name:
        raise argparse.ArgumentTypeError(f"phase evidence name is empty: {raw}")
    if not path_text:
        raise argparse.ArgumentTypeError(f"phase evidence path is empty: {raw}")
    return name, Path(path_text)


def _phase_log_from_dir(directory: Path, phase: str) -> Path:
    candidates = (
        directory / f"{phase}.log",
        directory / "dist" / "sccp-production-corridor" / f"{phase}.log",
        directory / f"sccp-production-corridor-{phase}" / f"{phase}.log",
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    expected = ", ".join(str(candidate) for candidate in candidates)
    raise FileNotFoundError(
        f"missing SCCP corridor evidence log for phase {phase}; checked {expected}"
    )


def _phase_evidence_sources(
    phases: list[str],
    phase_evidence: list[str],
    phase_evidence_dir: Path | None,
) -> dict[str, Path]:
    sources: dict[str, Path] = {}
    source_labels: dict[str, str] = {}

    def assign(phase: str, path: Path, label: str) -> None:
        previous = source_labels.get(phase)
        if previous is not None:
            raise argparse.ArgumentTypeError(
                f"duplicate SCCP corridor phase evidence for {phase}: "
                f"already set by {previous}, cannot set from {label}"
            )
        sources[phase] = path
        source_labels[phase] = label

    if phase_evidence_dir is not None:
        for phase in phases:
            assign(
                phase,
                _phase_log_from_dir(phase_evidence_dir, phase),
                "--phase-evidence-dir",
            )
    for raw in phase_evidence:
        name, path = _parse_phase_evidence_arg(raw)
        label = f"--phase-evidence {raw}"
        if name == "all":
            for phase in phases:
                assign(phase, path, label)
            continue
        if name not in phases:
            raise argparse.ArgumentTypeError(f"unknown SCCP corridor phase: {name}")
        assign(name, path, label)
    return sources


def _phase_evidence_args(sources: dict[str, Path]) -> list[str]:
    return [f"{phase}={path}" for phase, path in sorted(sources.items())]


def _copy_phase_evidence(
    phases: list[str],
    sources: dict[str, Path],
    output_dir: Path,
) -> tuple[list[str], list[Path]]:
    copied: list[Path] = []
    args: list[str] = []
    corridor_dir = output_dir / "corridor"
    for phase in phases:
        source = sources.get(phase)
        if source is None:
            continue
        destination = _copy_file(source, corridor_dir / f"{phase}.log")
        copied.append(destination)
        args.append(f"{phase}={destination}")
    return args, copied


def _native_evm_manifest_relative_path(value: Any, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value:
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must be a "
            "non-empty relative POSIX file path"
        )
    if value.strip() != value:
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must not contain "
            "surrounding whitespace"
        )
    control_character = _path_control_character(value)
    if control_character is not None:
        raise ValueError(
            "native EVM Groth16 prover bundle "
            f"{label} path contains control character {control_character}: {value!r}"
        )
    markdown_unsafe_character = _path_markdown_unsafe_character(value)
    if markdown_unsafe_character is not None:
        raise ValueError(
            "native EVM Groth16 prover bundle "
            f"{label} path contains Markdown-unsafe character "
            f"{markdown_unsafe_character}: {value!r}"
        )
    percent_traversal = _path_percent_encoded_traversal(value)
    if percent_traversal is not None:
        raise ValueError(
            "native EVM Groth16 prover bundle "
            f"{label} path contains percent-encoded traversal segment: "
            f"{percent_traversal}"
        )
    if ":" in value:
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must not contain URI schemes or drive prefixes"
        )
    if "\\" in value:
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must use POSIX separators"
        )
    normalized_value = value.lower()
    for marker in NATIVE_EVM_PROVER_FORBIDDEN_PATH_MARKERS:
        if marker in normalized_value:
            raise ValueError(
                f"native EVM Groth16 prover bundle {label} path contains forbidden prover dependency marker: {marker}"
            )
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or ".." in path.parts
        or not path.parts
        or value != path.as_posix()
    ):
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must be relative "
            "and stay under the manifest directory"
        )
    return path


def _native_evm_prover_payload_sources(
    source: Path | None,
) -> list[tuple[PurePosixPath, Path]]:
    if source is None:
        return []
    try:
        payload = _load_json_without_duplicate_keys(source)
    except DuplicateJsonKeyError as exc:
        raise ValueError(_native_evm_prover_duplicate_json_key_error(exc.key)) from exc
    if not isinstance(payload, dict):
        raise ValueError("native EVM Groth16 prover bundle must be a JSON object")

    paths: list[tuple[PurePosixPath, Path]] = []
    seen_roles_by_path: dict[str, str] = {}

    def add_path(raw_path: Any, label: str) -> None:
        relative_path = _native_evm_manifest_relative_path(raw_path, label)
        relative_text = relative_path.as_posix()
        previous_label = seen_roles_by_path.get(relative_text)
        if previous_label is not None:
            raise ValueError(
                f"native EVM Groth16 prover bundle {label} path must not reuse "
                f"{previous_label}: {relative_text}"
            )
        seen_roles_by_path[relative_text] = label
        artifact_path = source.parent.joinpath(*relative_path.parts)
        if artifact_path.resolve() == source.resolve():
            raise ValueError(
                f"native EVM Groth16 prover bundle {label} must not reference "
                "the manifest itself"
            )
        if not artifact_path.is_file():
            raise FileNotFoundError(
                "native EVM Groth16 prover bundle "
                f"{label} file is missing or is not a regular file: "
                f"{relative_path.as_posix()}"
            )
        paths.append((relative_path, artifact_path))

    for field in ("proof_artifact", "proving_key", "verifier_key"):
        if field in payload:
            add_path(payload[field], field)
    if "cross_sdk_fixture_parity_artifact" in payload:
        add_path(
            payload["cross_sdk_fixture_parity_artifact"],
            "cross_sdk_fixture_parity_artifact",
        )
    if "native_prover_self_test_artifact" in payload:
        add_path(
            payload["native_prover_self_test_artifact"],
            "native_prover_self_test_artifact",
        )

    native_sdk_artifacts = payload.get("native_sdk_artifacts")
    if isinstance(native_sdk_artifacts, list):
        for index, artifact in enumerate(native_sdk_artifacts):
            if not isinstance(artifact, dict) or "implementation_artifact" not in artifact:
                continue
            sdk = artifact.get("sdk")
            label = (
                f"{sdk} implementation_artifact"
                if isinstance(sdk, str) and sdk
                else f"native_sdk_artifacts[{index}].implementation_artifact"
            )
            add_path(artifact["implementation_artifact"], label)
    return paths


def _copy_native_evm_prover_bundle(
    source: Path | None,
    output_dir: Path,
) -> tuple[Path | None, list[Path]]:
    if source is None:
        return None, []
    copied_manifest = _copy_file(
        source,
        output_dir / "native-prover" / _safe_name(source, 0),
    )
    copied_payloads: list[Path] = []
    seen_payloads: set[str] = set()
    for relative_path, artifact_source in _native_evm_prover_payload_sources(source):
        relative_text = relative_path.as_posix()
        if relative_text in seen_payloads:
            continue
        seen_payloads.add(relative_text)
        destination = copied_manifest.parent.joinpath(*relative_path.parts)
        if destination == copied_manifest:
            raise ValueError(
                "native EVM Groth16 prover bundle payload path would overwrite "
                "the copied manifest"
            )
        copied_payloads.append(_copy_file(artifact_source, destination))
    return copied_manifest, copied_payloads


def _all_lanes_summary(paths: list[Path]) -> dict[str, Any]:
    module = _all_lanes_module()
    records = module.load_evidence_bundle(paths)
    return module.validate_evidence_bundle(records)


def _write_json(path: Path, payload: Any) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _markdown_string_list_items(value: Any, *, field_label: str) -> list[str]:
    if not isinstance(value, list):
        return [f"- `<invalid {field_label}>`"]
    if not value:
        return []
    if not all(isinstance(item, str) and item for item in value):
        return [f"- `<invalid {field_label}>`"]
    return [f"- {item}" for item in value]


CRYPTOGRAPHIC_EVIDENCE_ROW_FIELDS = (
    "domain",
    "chain",
    "evm_source_rpc_chain_id",
    "evm_source_block_tag",
    "evm_destination_rpc_chain_id",
    "evm_destination_block_tag",
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
    "destination_binding_hash",
    "source_adapter_gate_hash",
    "source_adapter_gate_required",
    "source_adapter_gate_audit_hashes",
    "route_allowlist_hash",
    "route_canary_evidence_hash",
    "route_canary_evidence_source",
    "route_canary_evidence_bound",
    "route_canary_transaction_hash",
    "route_canary_receipt_block_number",
    "route_canary_receipt_block_hash",
    "route_canary_receipt_block_finalized",
    "route_canary_block_receipts_root",
    "route_canary_message_id",
    "route_canary_block_number",
    "route_canary_block_timestamp",
)

USER_PROVER_SUBMISSION_SURFACE_FIELDS = (
    "lanes",
    "proof_backend",
    "sdk_helper_symbols",
    "sdk_helper_symbols_by_sdk",
    "sdk_helpers",
    "on_chain_submission",
    "required_phases",
    "validation_status",
    "validation_blockers",
)
READINESS_REPORT_BUNDLE_FIELDS = (
    "inputs",
    "input_artifacts",
    "release_checklist",
    "corridor",
    "cryptographic_evidence",
    "user_prover_submission_surfaces",
    "native_evm_prover_bundle",
    "source_inventory",
    "evidence",
)
READINESS_REPORT_ROOT_FIELDS = (
    "production_ready",
    "blockers",
    *READINESS_REPORT_BUNDLE_FIELDS,
)
RELEASE_CHECKLIST_FIELDS = ("ready", "items")
RELEASE_CHECKLIST_ITEM_FIELDS = ("id", "title", "ready", "blockers")
CORRIDOR_FIELDS = (
    "production_ready",
    "phases",
    "evidence_artifacts",
    "require_phase_evidence",
    "blockers",
)
ARTIFACT_FIELDS = ("path", "bytes", "sha256")
NATIVE_EVM_PROVER_BUNDLE_SUMMARY_FIELDS = (
    "required",
    "schema",
    "artifact",
    "bundle_id",
    "lanes",
    "proof_backend",
    "proof_artifact",
    "proof_artifact_hash",
    "proving_key",
    "proving_key_hash",
    "verifier_key",
    "verifier_key_hash",
    "destination_binding_hash",
    "audit_hashes",
    "cross_sdk_fixture_parity_artifact",
    "native_prover_self_test_artifact",
    "sdk_artifacts",
    "validation_status",
    "validation_blockers",
)
NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_FIELDS = (
    "sdk",
    "implementation",
    "implementation_hash",
    "implementation_artifact",
)
SOURCE_INVENTORY_FIELDS = ("validation_status", "validation_blockers")
ALL_LANES_SUMMARY_FIELDS = (
    "production_ready",
    "required_domains",
    "supported_launch_domains",
    "unsupported_launch_domains",
    "lanes",
    "blockers",
    "release_checklist",
)
ALL_LANES_LANE_FIELDS = (
    "domain",
    "chain",
    "records",
    "production_ready",
    "source_record_hashes",
    "source_adapter_gate",
    "evm_live_metadata",
    "destination_binding",
    "route_allowlist",
    "blockers",
)
ALL_LANES_RECORD_FIELDS = (
    "source_verifier_material",
    "source_adapter_deployment",
    "destination_rollout",
    "route_allowlist",
)

_SOURCE_INVENTORY_KNOWN_GATES: frozenset[str] | None = None
_CORRIDOR_PHASE_NAMES: frozenset[str] | None = None
_SUBMISSION_SURFACE_KNOWN_SDKS: frozenset[str] | None = None
_SUBMISSION_SURFACE_KNOWN_REQUIRED_PHASES: frozenset[str] | None = None
_ALL_LANES_NESTED_FIELD_SETS: dict[str, frozenset[str]] | None = None
_NATIVE_EVM_REQUIRED_IMPLEMENTATIONS: dict[str, str] | None = None
_NATIVE_EVM_REQUIRED_AUDIT_HASHES: frozenset[str] | None = None
_ACTIVE_LAUNCH_DOMAIN: int | None = None
_SCCP_DOMAIN_ETH: int | None = None
_ALL_LANES_EVM_DESTINATION_DOMAINS: frozenset[int] | None = None
_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN: dict[int, frozenset[str]] | None = None
_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN: dict[int, str] | None = None
_ROUTE_CANARY_SOURCE_BY_DOMAIN: dict[int, str] | None = None
_SCCP_DOMAIN_SORA: int | None = None
_SCCP_DOMAIN_SOL: int | None = None
_SCCP_DOMAIN_TON: int | None = None
_SCCP_DOMAIN_TRON: int | None = None


def _source_inventory_known_gates() -> frozenset[str]:
    global _SOURCE_INVENTORY_KNOWN_GATES
    if _SOURCE_INVENTORY_KNOWN_GATES is None:
        verifier = _verify_module()
        _SOURCE_INVENTORY_KNOWN_GATES = frozenset(
            verifier.SOURCE_INVENTORY_REQUIRED_GATES
        )
    return _SOURCE_INVENTORY_KNOWN_GATES


def _corridor_phase_names() -> frozenset[str]:
    global _CORRIDOR_PHASE_NAMES
    if _CORRIDOR_PHASE_NAMES is None:
        verifier = _verify_module()
        _CORRIDOR_PHASE_NAMES = frozenset(verifier.CORRIDOR_PHASES)
    return _CORRIDOR_PHASE_NAMES


def _submission_surface_known_sdks() -> frozenset[str]:
    global _SUBMISSION_SURFACE_KNOWN_SDKS
    if _SUBMISSION_SURFACE_KNOWN_SDKS is None:
        verifier = _verify_module()
        _SUBMISSION_SURFACE_KNOWN_SDKS = frozenset(
            (*verifier.USER_PROVER_SDK_PHASES, "dotnet-sdk")
        )
    return _SUBMISSION_SURFACE_KNOWN_SDKS


def _submission_surface_known_required_phases() -> frozenset[str]:
    global _SUBMISSION_SURFACE_KNOWN_REQUIRED_PHASES
    if _SUBMISSION_SURFACE_KNOWN_REQUIRED_PHASES is None:
        verifier = _verify_module()
        _SUBMISSION_SURFACE_KNOWN_REQUIRED_PHASES = frozenset(
            verifier.USER_PROVER_KNOWN_REQUIRED_PHASES
        )
    return _SUBMISSION_SURFACE_KNOWN_REQUIRED_PHASES


def _all_lanes_nested_field_sets() -> dict[str, frozenset[str]]:
    global _ALL_LANES_NESTED_FIELD_SETS
    if _ALL_LANES_NESTED_FIELD_SETS is None:
        verifier = _verify_module()
        _ALL_LANES_NESTED_FIELD_SETS = {
            "source_record_hashes": frozenset(
                verifier.ALL_LANES_SOURCE_RECORD_HASH_KEYS
            ),
            "source_adapter_gate": frozenset(
                verifier.ALL_LANES_SOURCE_ADAPTER_GATE_KEYS
            ),
            "evm_live_metadata": frozenset(verifier.ALL_LANES_EVM_LIVE_METADATA_KEYS),
            "destination_binding": frozenset(verifier.ALL_LANES_DESTINATION_BINDING_KEYS),
            "destination_binding_required": frozenset(
                verifier.ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS
            ),
            "route_allowlist": frozenset(verifier.ALL_LANES_ROUTE_ALLOWLIST_KEYS),
            "route_canary_common": frozenset(
                verifier.ALL_LANES_ROUTE_CANARY_COMMON_KEYS
            ),
        }
    return _ALL_LANES_NESTED_FIELD_SETS


def _all_lanes_route_canary_fields(domain: Any) -> frozenset[str]:
    verifier = _verify_module()
    if type(domain) is int:
        fields = verifier.ALL_LANES_ROUTE_CANARY_KEYS_BY_DOMAIN.get(domain)
        if fields is not None:
            return frozenset(fields)
    return _all_lanes_nested_field_sets()["route_canary_common"]


def _native_evm_required_implementations() -> dict[str, str]:
    global _NATIVE_EVM_REQUIRED_IMPLEMENTATIONS
    if _NATIVE_EVM_REQUIRED_IMPLEMENTATIONS is None:
        verifier = _verify_module()
        _NATIVE_EVM_REQUIRED_IMPLEMENTATIONS = dict(
            verifier.NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS
        )
    return _NATIVE_EVM_REQUIRED_IMPLEMENTATIONS


def _native_evm_required_audit_hashes() -> frozenset[str]:
    global _NATIVE_EVM_REQUIRED_AUDIT_HASHES
    if _NATIVE_EVM_REQUIRED_AUDIT_HASHES is None:
        verifier = _verify_module()
        _NATIVE_EVM_REQUIRED_AUDIT_HASHES = frozenset(
            verifier.NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES
        )
    return _NATIVE_EVM_REQUIRED_AUDIT_HASHES


def _active_launch_domain() -> int:
    global _ACTIVE_LAUNCH_DOMAIN
    if _ACTIVE_LAUNCH_DOMAIN is None:
        verifier = _verify_module()
        _ACTIVE_LAUNCH_DOMAIN = int(verifier.ACTIVE_LAUNCH_DOMAIN)
    return _ACTIVE_LAUNCH_DOMAIN


def _sccp_domain_eth() -> int:
    global _SCCP_DOMAIN_ETH
    if _SCCP_DOMAIN_ETH is None:
        verifier = _verify_module()
        _SCCP_DOMAIN_ETH = int(verifier.SCCP_DOMAIN_ETH)
    return _SCCP_DOMAIN_ETH


def _sccp_domain_sora() -> int:
    global _SCCP_DOMAIN_SORA
    if _SCCP_DOMAIN_SORA is None:
        verifier = _verify_module()
        _SCCP_DOMAIN_SORA = int(verifier.SCCP_DOMAIN_SORA)
    return _SCCP_DOMAIN_SORA


def _sccp_domain_sol() -> int:
    global _SCCP_DOMAIN_SOL
    if _SCCP_DOMAIN_SOL is None:
        verifier = _verify_module()
        _SCCP_DOMAIN_SOL = int(verifier.SCCP_DOMAIN_SOL)
    return _SCCP_DOMAIN_SOL


def _sccp_domain_ton() -> int:
    global _SCCP_DOMAIN_TON
    if _SCCP_DOMAIN_TON is None:
        verifier = _verify_module()
        _SCCP_DOMAIN_TON = int(verifier.SCCP_DOMAIN_TON)
    return _SCCP_DOMAIN_TON


def _sccp_domain_tron() -> int:
    global _SCCP_DOMAIN_TRON
    if _SCCP_DOMAIN_TRON is None:
        verifier = _verify_module()
        _SCCP_DOMAIN_TRON = int(verifier.SCCP_DOMAIN_TRON)
    return _SCCP_DOMAIN_TRON


def _all_lanes_evm_destination_domains() -> frozenset[int]:
    global _ALL_LANES_EVM_DESTINATION_DOMAINS
    if _ALL_LANES_EVM_DESTINATION_DOMAINS is None:
        verifier = _verify_module()
        _ALL_LANES_EVM_DESTINATION_DOMAINS = frozenset(
            int(domain) for domain in verifier.ALL_LANES_EVM_DESTINATION_DOMAINS
        )
    return _ALL_LANES_EVM_DESTINATION_DOMAINS


def _source_adapter_gate_audit_keys_by_domain() -> dict[int, frozenset[str]]:
    global _SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN
    if _SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN is None:
        verifier = _verify_module()
        _SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN = {
            int(domain): frozenset(keys)
            for domain, keys in verifier.ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.items()
        }
    return _SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN


def _source_adapter_gate_hash_key_by_domain() -> dict[int, str]:
    global _SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN
    if _SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN is None:
        verifier = _verify_module()
        _SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN = {
            int(domain): str(key)
            for domain, key in verifier.ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN.items()
        }
    return _SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN


def _route_canary_source_by_domain() -> dict[int, str]:
    global _ROUTE_CANARY_SOURCE_BY_DOMAIN
    if _ROUTE_CANARY_SOURCE_BY_DOMAIN is None:
        verifier = _verify_module()
        _ROUTE_CANARY_SOURCE_BY_DOMAIN = {
            int(domain): str(source)
            for domain, source in verifier.ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN.items()
        }
    return _ROUTE_CANARY_SOURCE_BY_DOMAIN


def _expected_evm_rpc_chain_id(domain: int, chain: Any) -> int:
    verifier = _verify_module()
    return int(verifier._expected_evm_rpc_chain_id(domain, chain))


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


def _is_canonical_tron_address_text(value: Any) -> bool:
    verifier = _verify_module()
    return bool(verifier._is_canonical_tron_address_text(value))


def _is_canonical_solana_pubkey_text(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    verifier = _verify_module()
    try:
        raw = verifier._decode_solana_base58(value)
    except ValueError:
        return False
    return len(raw) == 32 and any(raw)


def _source_adapter_gate_semantic_errors(
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
    audit_label = f"{label}.source_adapter_gate audit_hashes"
    expected_audit_keys = (
        _source_adapter_gate_audit_keys_by_domain().get(domain)
        if type(domain) is int
        else None
    )
    if expected_audit_keys is None:
        if required:
            errors.append(
                f"{label}.source_adapter_gate required must be false "
                "for this lane domain"
            )
            return errors
        if ready is not True:
            errors.append(
                f"{label}.source_adapter_gate ready must be true "
                "when gate is not required"
            )
        if isinstance(audit_hashes, dict) and audit_hashes:
            errors.append(
                f"{label}.source_adapter_gate audit_hashes must be empty "
                "when gate is not required"
            )
        if gate_hash not in (None, ""):
            errors.append(
                f"{label}.source_adapter_gate gate_hash must be empty "
                "when gate is not required"
            )
        if isinstance(blockers, list) and blockers:
            errors.append(
                f"{label}.source_adapter_gate blockers must be empty "
                "when gate is not required"
            )
        return errors

    if not required:
        errors.append(
            f"{label}.source_adapter_gate required must be true for this lane domain"
        )
        return errors

    if not _is_nonzero_bytes32_hex_text(gate_hash):
        errors.append(
            f"{label}.source_adapter_gate gate_hash must be a non-zero "
            "canonical bytes32 hex string when required"
        )
    if isinstance(audit_hashes, dict):
        semantic_audit_hashes = {
            key: value
            for key, value in audit_hashes.items()
            if _source_adapter_gate_audit_key_error(key, audit_label) is None
        }
        if not semantic_audit_hashes:
            errors.append(
                f"{label}.source_adapter_gate audit_hashes must not be empty "
                "when required"
            )
        for key in sorted(set(semantic_audit_hashes) - expected_audit_keys):
            errors.append(
                f"{label}.source_adapter_gate audit_hashes contains "
                f"unexpected field: {key}"
            )
        for key in sorted(expected_audit_keys - set(semantic_audit_hashes)):
            errors.append(
                f"{label}.source_adapter_gate audit_hashes missing field: {key}"
            )
        if _is_nonzero_bytes32_hex_text(gate_hash) and not any(
            gate_hash == value for value in semantic_audit_hashes.values()
        ):
            errors.append(
                f"{label}.source_adapter_gate gate_hash must match one "
                "audit_hashes value"
            )
        expected_gate_key = (
            _source_adapter_gate_hash_key_by_domain().get(domain)
            if type(domain) is int
            else None
        )
        expected_gate_hash = semantic_audit_hashes.get(expected_gate_key)
        if (
            expected_gate_key is not None
            and _is_nonzero_bytes32_hex_text(gate_hash)
            and _is_nonzero_bytes32_hex_text(expected_gate_hash)
            and gate_hash != expected_gate_hash
        ):
            errors.append(
                f"{label}.source_adapter_gate gate_hash must match "
                f"audit_hashes.{expected_gate_key}"
            )
    if type(ready) is bool and not ready:
        errors.append(
            f"{label}.source_adapter_gate ready must be true when gate is required"
        )
    if isinstance(blockers, list) and blockers:
        errors.append(
            f"{label}.source_adapter_gate blockers must be empty "
            "when gate is required"
        )
    return errors


def _route_canary_common_semantic_errors(
    label: str,
    lane: dict[str, Any],
    route_allowlist: dict[str, Any],
    destination_binding: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    canary_label = f"{label}.route_allowlist.route_canary"
    for field in (
        "evidence_hash",
        "route_allowlist_hash",
        "destination_binding_hash",
    ):
        if not _is_nonzero_bytes32_hex_text(route_canary.get(field)):
            errors.append(
                f"{canary_label} {field} must be a non-zero canonical "
                "bytes32 hex string"
            )

    if isinstance(route_canary.get("status"), str) and (
        route_canary.get("status") != "passed"
    ):
        errors.append(f"{canary_label} status must be passed")

    domain = lane.get("domain")
    expected_source = (
        _route_canary_source_by_domain().get(domain)
        if type(domain) is int
        else None
    )
    if (
        expected_source is not None
        and isinstance(route_canary.get("evidence_source"), str)
        and route_canary.get("evidence_source") != expected_source
    ):
        errors.append(f"{canary_label} evidence_source must be {expected_source}")

    if route_canary.get("evidence_bound") is not True:
        errors.append(f"{canary_label} evidence_bound must be true")

    expected_route_hash = route_allowlist.get("route_allowlist_hash")
    if (
        isinstance(expected_route_hash, str)
        and isinstance(route_canary.get("route_allowlist_hash"), str)
        and route_canary.get("route_allowlist_hash") != expected_route_hash
    ):
        errors.append(
            f"{canary_label} route_allowlist_hash must match lane "
            "route_allowlist_hash"
        )

    expected_destination_hash = destination_binding.get("destination_binding_hash")
    if (
        isinstance(expected_destination_hash, str)
        and isinstance(route_canary.get("destination_binding_hash"), str)
        and route_canary.get("destination_binding_hash") != expected_destination_hash
    ):
        errors.append(
            f"{canary_label} destination_binding_hash must match lane "
            "destination_binding_hash"
        )
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} hash role",
            (
                (
                    "source_verifier_material_hash",
                    lane.get("source_record_hashes", {}).get(
                        "source_verifier_material_hash"
                    )
                    if isinstance(lane.get("source_record_hashes"), dict)
                    else None,
                ),
                (
                    "source_adapter_engine_deployment_hash",
                    lane.get("source_record_hashes", {}).get(
                        "source_adapter_engine_deployment_hash"
                    )
                    if isinstance(lane.get("source_record_hashes"), dict)
                    else None,
                ),
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash")),
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                ),
                ("evidence_hash", route_canary.get("evidence_hash")),
            ),
        )
    )
    return errors


def _distinct_nonzero_bytes32_field_errors(
    label: str,
    fields: tuple[tuple[str, Any], ...],
) -> list[str]:
    errors: list[str] = []
    seen: dict[str, str] = {}
    for field, value in fields:
        if not _is_nonzero_bytes32_hex_text(value):
            continue
        assert isinstance(value, str)
        previous_field = seen.get(value)
        if previous_field is not None:
            errors.append(f"{label} {field} must not reuse {previous_field}")
            continue
        seen[value] = field
    return errors


def _route_canary_u32_errors(
    label: str,
    route_canary: dict[str, Any],
    field: str,
) -> list[str]:
    value = route_canary.get(field)
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        return [f"{label} {field} must be a u32 integer"]
    return []


def _route_canary_expected_u32_errors(
    label: str,
    route_canary: dict[str, Any],
    field: str,
    expected: int,
    expected_label: str,
) -> list[str]:
    errors = _route_canary_u32_errors(label, route_canary, field)
    if errors:
        return errors
    if route_canary.get(field) != expected:
        return [f"{label} {field} must be {expected_label}"]
    return []


def _route_canary_evm_semantic_errors(
    label: str,
    lane: dict[str, Any],
    route_allowlist: dict[str, Any],
    destination_binding: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    domain = lane.get("domain")
    if domain not in _all_lanes_evm_destination_domains():
        return []

    canary_label = f"{label}.route_allowlist.route_canary"
    errors: list[str] = []
    transcript_hash_fields = (
        "transaction_hash",
        "receipt_block_hash",
        "block_receipts_root",
        "call_data_sha256",
        "message_id",
        "payload_hash",
        "statement_hash",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
    )
    for field in transcript_hash_fields:
        if not _is_nonzero_bytes32_hex_text(route_canary.get(field)):
            errors.append(
                f"{canary_label} {field} must be a non-zero canonical "
                "bytes32 hex string"
            )

    transcript_roles = transcript_hash_fields + ("evidence_hash",)
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} transcript hash",
            tuple((field, route_canary.get(field)) for field in transcript_roles),
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
    governed_hash_fields.append(
        ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
    )
    governed_hash_fields.append(
        (
            "destination_binding_hash",
            destination_binding.get("destination_binding_hash"),
        )
    )
    governed_hash_fields.extend(
        (field, route_canary.get(field)) for field in transcript_roles
    )
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} hash role",
            tuple(governed_hash_fields),
        )
    )

    errors.extend(_route_canary_u32_errors(canary_label, route_canary, "log_index"))
    receipt_block_number = route_canary.get("receipt_block_number")
    if type(receipt_block_number) is not int or receipt_block_number <= 0:
        errors.append(f"{canary_label} receipt_block_number must be a positive integer")
    if type(domain) is int:
        errors.extend(
            _route_canary_expected_u32_errors(
                canary_label,
                route_canary,
                "target_domain",
                domain,
                "the lane domain",
            )
        )
    errors.extend(
        _route_canary_expected_u32_errors(
            canary_label,
            route_canary,
            "proof_version",
            1,
            "1",
        )
    )
    errors.extend(
        _route_canary_expected_u32_errors(
            canary_label,
            route_canary,
            "proof_source_domain",
            _sccp_domain_sora(),
            "SORA",
        )
    )
    for field in ("message_proof_used", "receipt_block_finalized"):
        if route_canary.get(field) is not True:
            errors.append(f"{canary_label} {field} must be true")
    return errors


def _route_canary_integer_errors(
    label: str,
    route_canary: dict[str, Any],
    field: str,
    *,
    positive: bool,
) -> list[str]:
    value = route_canary.get(field)
    if type(value) is not int or value < 0 or (positive and value == 0):
        qualifier = "positive " if positive else "non-negative "
        return [f"{label} {field} must be a {qualifier}integer"]
    return []


def _route_canary_tron_semantic_errors(
    label: str,
    lane: dict[str, Any],
    route_allowlist: dict[str, Any],
    destination_binding: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    if lane.get("domain") != _sccp_domain_tron():
        return []

    canary_label = f"{label}.route_allowlist.route_canary"
    errors: list[str] = []
    transcript_hash_fields = (
        "transaction_id",
        "message_id",
        "call_data_sha256",
        "payload_hash",
        "statement_hash",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
        "signature_sha256",
    )
    for field in transcript_hash_fields:
        if not _is_nonzero_bytes32_hex_text(route_canary.get(field)):
            errors.append(
                f"{canary_label} {field} must be a non-zero canonical "
                "bytes32 hex string"
            )

    for field in ("transaction_owner_address", "signature_recovered_address"):
        if not _is_canonical_tron_address_text(route_canary.get(field)):
            errors.append(
                f"{canary_label} {field} must be a non-zero canonical "
                "0x41-prefixed 21-byte hex string"
            )
    transaction_owner_address = route_canary.get("transaction_owner_address")
    signature_recovered_address = route_canary.get("signature_recovered_address")
    if (
        _is_canonical_tron_address_text(transaction_owner_address)
        and _is_canonical_tron_address_text(signature_recovered_address)
        and signature_recovered_address != transaction_owner_address
    ):
        errors.append(
            f"{canary_label} signature_recovered_address must match "
            "transaction_owner_address"
        )

    transcript_roles = transcript_hash_fields + ("evidence_hash",)
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} transcript hash",
            tuple((field, route_canary.get(field)) for field in transcript_roles),
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
    governed_hash_fields.append(
        ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
    )
    governed_hash_fields.append(
        (
            "destination_binding_hash",
            destination_binding.get("destination_binding_hash"),
        )
    )
    governed_hash_fields.extend(
        (field, route_canary.get(field)) for field in transcript_roles
    )
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} hash role",
            tuple(governed_hash_fields),
        )
    )

    errors.extend(
        _route_canary_integer_errors(
            canary_label,
            route_canary,
            "block_number",
            positive=True,
        )
    )
    errors.extend(
        _route_canary_integer_errors(
            canary_label,
            route_canary,
            "block_timestamp",
            positive=False,
        )
    )
    errors.extend(_route_canary_u32_errors(canary_label, route_canary, "log_index"))
    errors.extend(
        _route_canary_expected_u32_errors(
            canary_label,
            route_canary,
            "target_domain",
            _sccp_domain_tron(),
            "TRON",
        )
    )
    errors.extend(
        _route_canary_expected_u32_errors(
            canary_label,
            route_canary,
            "proof_version",
            1,
            "1",
        )
    )
    errors.extend(
        _route_canary_expected_u32_errors(
            canary_label,
            route_canary,
            "proof_source_domain",
            _sccp_domain_sora(),
            "SORA",
        )
    )
    for field in (
        "message_proof_used",
        "raw_data_owner_matches_transaction",
        "signature_recovers_to_owner",
    ):
        if route_canary.get(field) is not True:
            errors.append(f"{canary_label} {field} must be true")
    return errors


def _route_canary_solana_semantic_errors(
    label: str,
    lane: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    if lane.get("domain") != _sccp_domain_sol():
        return []

    canary_label = f"{label}.route_allowlist.route_canary"
    errors: list[str] = []
    if not _is_canonical_solana_pubkey_text(
        route_canary.get("solana_programdata_address")
    ):
        errors.append(
            f"{canary_label} solana_programdata_address must be a non-zero "
            "canonical base58 Solana address"
        )
    if not _is_canonical_decimal_text(
        route_canary.get("solana_programdata_slot"),
        positive=True,
    ):
        errors.append(
            f"{canary_label} solana_programdata_slot must be a canonical "
            "positive decimal string"
        )
    return errors


def _route_canary_ton_semantic_errors(
    label: str,
    lane: dict[str, Any],
    route_allowlist: dict[str, Any],
    destination_binding: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    if lane.get("domain") != _sccp_domain_ton():
        return []

    canary_label = f"{label}.route_allowlist.route_canary"
    errors: list[str] = []
    for field in ("ton_account_state_hash", "ton_last_transaction_hash"):
        if not _is_nonzero_bytes32_hex_text(route_canary.get(field)):
            errors.append(
                f"{canary_label} {field} must be a non-zero canonical "
                "bytes32 hex string"
            )
    if not _is_canonical_decimal_text(
        route_canary.get("ton_last_transaction_lt"),
        positive=True,
    ):
        errors.append(
            f"{canary_label} ton_last_transaction_lt must be a canonical "
            "positive decimal string"
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
    governed_hash_fields.append(
        ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
    )
    governed_hash_fields.append(
        (
            "destination_binding_hash",
            destination_binding.get("destination_binding_hash"),
        )
    )
    governed_hash_fields.extend(
        (field, route_canary.get(field))
        for field in (
            "ton_account_state_hash",
            "ton_last_transaction_hash",
            "evidence_hash",
        )
    )
    errors.extend(
        _distinct_nonzero_bytes32_field_errors(
            f"{canary_label} hash role",
            tuple(governed_hash_fields),
        )
    )
    return errors


def _all_lanes_route_canary_cross_lane_bundle_errors(
    label: str,
    lanes: list[Any],
) -> list[str]:
    governed_hashes: dict[str, tuple[str, str]] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = f"{label}.lanes[{index}]"
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            for field in (
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
            ):
                value = source_hashes.get(field)
                if _is_nonzero_bytes32_hex_text(value):
                    assert isinstance(value, str)
                    governed_hashes.setdefault(value, (lane_label, field))
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            value = destination_binding.get("destination_binding_hash")
            if _is_nonzero_bytes32_hex_text(value):
                assert isinstance(value, str)
                governed_hashes.setdefault(
                    value,
                    (lane_label, "destination_binding_hash"),
                )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            value = route_allowlist.get("route_allowlist_hash")
            if _is_nonzero_bytes32_hex_text(value):
                assert isinstance(value, str)
                governed_hashes.setdefault(value, (lane_label, "route_allowlist_hash"))

    errors: list[str] = []
    seen_canaries: dict[str, str] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = f"{label}.lanes[{index}]"
        route_allowlist = lane.get("route_allowlist")
        if not isinstance(route_allowlist, dict):
            continue
        route_canary = route_allowlist.get("route_canary")
        if not isinstance(route_canary, dict):
            continue
        evidence_hash = route_canary.get("evidence_hash")
        if not _is_nonzero_bytes32_hex_text(evidence_hash):
            continue
        assert isinstance(evidence_hash, str)
        canary_label = f"{lane_label}.route_allowlist.route_canary"
        previous_canary_label = seen_canaries.get(evidence_hash)
        if previous_canary_label is not None:
            errors.append(
                f"{canary_label} evidence_hash must be distinct from "
                f"{previous_canary_label} evidence_hash"
            )
        else:
            seen_canaries[evidence_hash] = canary_label

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


def _source_inventory_gate_key_error(gate: Any, label: str) -> str | None:
    if not isinstance(gate, str) or not gate:
        return f"{label} contains malformed unknown gate name"
    if _path_control_character(gate) is not None:
        return f"{label} contains unknown gate name with control character"
    if not gate.isascii():
        return f"{label} contains unknown gate name with non-ASCII character"
    if gate.strip() != gate:
        return f"{label} contains unknown gate name with surrounding whitespace"
    if any(character.isspace() for character in gate):
        return f"{label} contains unknown gate name with whitespace"
    if _path_markdown_unsafe_character(gate) is not None:
        return f"{label} contains unknown gate name with Markdown-unsafe character"
    return None


def _checklist_item_id_error(item_id: Any, label: str) -> str | None:
    if not isinstance(item_id, str) or not item_id:
        return f"{label} id must be a non-empty string"
    if _path_control_character(item_id) is not None:
        return f"{label} id contains control character"
    if not item_id.isascii():
        return f"{label} id contains non-ASCII character"
    if item_id.strip() != item_id:
        return f"{label} id contains surrounding whitespace"
    if any(character.isspace() for character in item_id):
        return f"{label} id contains whitespace"
    if _path_markdown_unsafe_character(item_id) is not None:
        return f"{label} id contains Markdown-unsafe character"
    return None


def _corridor_phase_key_error(phase: Any, label: str) -> str | None:
    if not isinstance(phase, str) or not phase:
        return f"{label} contains malformed phase"
    if _path_control_character(phase) is not None:
        return f"{label} contains phase with control character"
    if not phase.isascii():
        return f"{label} contains phase with non-ASCII character"
    if phase.strip() != phase:
        return f"{label} contains phase with surrounding whitespace"
    if any(character.isspace() for character in phase):
        return f"{label} contains phase with whitespace"
    if _path_markdown_unsafe_character(phase) is not None:
        return f"{label} contains phase with Markdown-unsafe character"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in phase)
        or phase.startswith("-")
        or phase.endswith("-")
    ):
        return f"{label} contains malformed phase"
    return None


def _submission_surface_sdk_key_error(sdk: Any, label: str) -> str | None:
    if not isinstance(sdk, str) or not sdk:
        return f"{label} contains malformed SDK key"
    if _path_control_character(sdk) is not None:
        return f"{label} contains SDK key with control character"
    if not sdk.isascii():
        return f"{label} contains SDK key with non-ASCII character"
    if sdk.strip() != sdk:
        return f"{label} contains SDK key with surrounding whitespace"
    if any(character.isspace() for character in sdk):
        return f"{label} contains SDK key with whitespace"
    if _path_markdown_unsafe_character(sdk) is not None:
        return f"{label} contains SDK key with Markdown-unsafe character"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in sdk)
        or sdk.startswith("-")
        or sdk.endswith("-")
    ):
        return f"{label} contains malformed SDK key"
    return None


def _require_report_mapping(
    value: Any,
    label: str,
    errors: list[str],
) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    errors.append(f"{label} must be an object")
    return {}


def _require_report_list(value: Any, label: str, errors: list[str]) -> list[Any]:
    if isinstance(value, list):
        return value
    errors.append(f"{label} must be a list")
    return []


def _require_report_fields(
    payload: dict[str, Any],
    label: str,
    fields: tuple[str, ...],
    errors: list[str],
) -> None:
    for field in fields:
        if field not in payload:
            errors.append(f"{label} missing field: {field}")


def _unknown_report_field_errors(
    payload: dict[str, Any],
    label: str,
    allowed_fields: tuple[str, ...],
) -> list[str]:
    return [
        f"{label} contains unknown field: {field}"
        for field in sorted(set(payload) - set(allowed_fields), key=str)
    ]


def _string_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    errors: list[str] = []
    value = payload.get(field)
    if not isinstance(value, list):
        return [f"{label} {field} must be a list of non-empty strings"]
    if not allow_empty and not value:
        errors.append(f"{label} {field} must not be empty")
    seen: set[str] = set()
    for item in value:
        if not isinstance(item, str) or not item:
            errors.append(f"{label} {field} must be a list of non-empty strings")
            continue
        if item.strip() != item:
            errors.append(
                f"{label} {field} must be a list of non-empty strings "
                "with no surrounding whitespace"
            )
        if item in seen:
            errors.append(f"{label} {field} must not contain duplicate strings")
        seen.add(item)
    return errors


def _non_empty_string_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, str) or not value or value.strip() != value:
        return [
            f"{label} {field} must be a non-empty string "
            "with no surrounding whitespace"
        ]
    return []


def _integer_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    value = payload.get(field)
    if not isinstance(value, list):
        return [f"{label} {field} must be a list of integers"]
    errors: list[str] = []
    if not allow_empty and not value:
        errors.append(f"{label} {field} must not be empty")
    seen: set[int] = set()
    for item in value:
        if type(item) is not int:
            errors.append(f"{label} {field} must be a list of integers")
            continue
        if item in seen:
            errors.append(f"{label} {field} must not contain duplicate integers")
        seen.add(item)
    return errors


def _is_canonical_sha256_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _is_canonical_bytes32_hex_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 66
        and value.startswith("0x")
        and all(character in "0123456789abcdef" for character in value[2:])
    )


def _is_nonzero_bytes32_hex_text(value: Any) -> bool:
    return _is_canonical_bytes32_hex_text(value) and any(
        character != "0" for character in value[2:]
    )


def _native_evm_field_name_error(
    key: Any,
    label: str,
    field_kind: str,
) -> str:
    if not isinstance(key, str) or not key:
        return f"{label} contains malformed {field_kind} field name"
    if _path_control_character(key) is not None:
        return f"{label} contains {field_kind} field name with control character"
    if not key.isascii():
        return f"{label} contains {field_kind} field name with non-ASCII character"
    if key.strip() != key:
        return f"{label} contains {field_kind} field name with surrounding whitespace"
    if any(character.isspace() for character in key):
        return f"{label} contains {field_kind} field name with whitespace"
    if _path_markdown_unsafe_character(key) is not None:
        return (
            f"{label} contains {field_kind} field name with Markdown-unsafe "
            "character"
        )
    return f"{label} contains {field_kind} field: {key}"


def _native_evm_unknown_field_errors(
    payload: dict[str, Any],
    label: str,
    allowed_fields: tuple[str, ...],
) -> list[str]:
    return [
        _native_evm_field_name_error(field, label, "unknown")
        for field in sorted(set(payload) - set(allowed_fields), key=str)
    ]


def _native_evm_sdk_key_error(sdk: Any, label: str) -> str | None:
    if not isinstance(sdk, str) or not sdk:
        return f"{label} sdk must be a lowercase SDK id"
    if _path_control_character(sdk) is not None:
        return f"{label} sdk contains control character"
    if not sdk.isascii():
        return f"{label} sdk must be printable ASCII"
    if sdk.strip() != sdk:
        return f"{label} sdk must not contain surrounding whitespace"
    if any(character.isspace() for character in sdk):
        return f"{label} sdk must not contain whitespace"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in sdk)
        or sdk.startswith("-")
        or sdk.endswith("-")
    ):
        return f"{label} sdk must be a lowercase SDK id"
    return None


def _source_adapter_gate_audit_key_error(key: Any, label: str) -> str | None:
    if not isinstance(key, str) or not key:
        return f"{label} contains malformed audit field name"
    if _path_control_character(key) is not None:
        return f"{label} contains audit field name with control character"
    if not key.isascii():
        return f"{label} contains audit field name with non-ASCII character"
    if key.strip() != key:
        return f"{label} contains audit field name with surrounding whitespace"
    if any(character.isspace() for character in key):
        return f"{label} contains audit field name with whitespace"
    if _path_markdown_unsafe_character(key) is not None:
        return f"{label} contains audit field name with Markdown-unsafe character"
    return None


def _unknown_public_field_error(field: Any, label: str) -> str:
    if not isinstance(field, str) or not field:
        return f"{label} contains malformed unknown field name"
    if _path_control_character(field) is not None:
        return f"{label} contains unknown field name with control character"
    if not field.isascii():
        return f"{label} contains unknown field name with non-ASCII character"
    if field.strip() != field:
        return f"{label} contains unknown field name with surrounding whitespace"
    if any(character.isspace() for character in field):
        return f"{label} contains unknown field name with whitespace"
    if _path_markdown_unsafe_character(field) is not None:
        return f"{label} contains unknown field name with Markdown-unsafe character"
    return f"{label} contains unknown field: {field}"


def _unknown_public_field_errors(
    payload: dict[str, Any],
    label: str,
    allowed_fields: set[str] | frozenset[str],
) -> list[str]:
    return [
        _unknown_public_field_error(field, label)
        for field in sorted(set(payload) - set(allowed_fields), key=str)
    ]


def _optional_bytes32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if value in ("", None):
        return []
    if not _is_canonical_bytes32_hex_text(value):
        return [
            f"{label} {field} must be empty, null, or a canonical bytes32 hex string"
        ]
    return []


def _optional_nonzero_bytes32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if value in ("", None):
        return []
    if not _is_canonical_bytes32_hex_text(value) or all(
        character == "0" for character in value[2:]
    ):
        return [
            f"{label} {field} must be empty, null, or a non-zero canonical "
            "bytes32 hex string"
        ]
    return []


def _optional_integer_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    positive: bool,
) -> list[str]:
    if field not in payload or payload.get(field) is None:
        return []
    value = payload.get(field)
    if (
        type(value) is not int
        or (positive and value <= 0)
        or (not positive and value < 0)
    ):
        kind = "positive" if positive else "non-negative"
        return [f"{label} {field} must be null or a {kind} integer"]
    return []


def _string_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if (
        not isinstance(value, str)
        or (not allow_empty and not value)
        or value.strip() != value
    ):
        empty_text = "non-empty " if not allow_empty else ""
        return [
            f"{label} {field} must be a {empty_text}string with no surrounding "
            "whitespace"
        ]
    return []


def _all_lanes_object(
    value: Any,
    label: str,
    allowed_fields: set[str] | frozenset[str],
    required_fields: set[str] | frozenset[str],
    errors: list[str],
) -> dict[str, Any]:
    payload = _require_report_mapping(value, label, errors)
    if not isinstance(value, dict):
        return {}
    errors.extend(_unknown_public_field_errors(payload, label, allowed_fields))
    _require_report_fields(payload, label, tuple(required_fields), errors)
    return payload


def _release_report_preflight_errors(report: Any, *, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(report, label, errors)
    if errors:
        return errors
    _require_report_fields(payload, label, ("production_ready", "blockers"), errors)
    if type(payload.get("production_ready")) is not bool:
        errors.append(f"{label} production_ready must be true or false")
    errors.extend(
        _string_list_field_errors(label, payload, "blockers", allow_empty=True)
    )
    blockers = payload.get("blockers")
    if payload.get("production_ready") is True and isinstance(blockers, list) and blockers:
        errors.append(f"{label} blockers must be empty when production_ready is true")
    return errors


def _artifact_row_errors(row: Any, label: str) -> list[str]:
    errors: list[str] = []
    artifact = _require_report_mapping(row, label, errors)
    if errors:
        return errors
    require_bundle_relative_path = label.startswith("bundled report.")
    errors.extend(_unknown_report_field_errors(artifact, label, ARTIFACT_FIELDS))
    _require_report_fields(artifact, label, ARTIFACT_FIELDS, errors)
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        errors.append(f"{label} path must be a non-empty string")
    else:
        if artifact_path.strip() != artifact_path:
            errors.append(
                f"{label} path must not contain surrounding whitespace: "
                f"{artifact_path!r}"
            )
        control_character = _path_control_character(artifact_path)
        if control_character is not None:
            errors.append(
                f"{label} path contains control character "
                f"{control_character}: {artifact_path!r}"
            )
        markdown_unsafe_character = _path_markdown_unsafe_character(artifact_path)
        if markdown_unsafe_character is not None:
            errors.append(
                f"{label} path contains Markdown-unsafe character "
                f"{markdown_unsafe_character}: {artifact_path!r}"
            )
        percent_traversal = _path_percent_encoded_traversal(artifact_path)
        if percent_traversal is not None:
            errors.append(
                f"{label} path contains percent-encoded traversal segment: "
                f"{percent_traversal}"
            )
        if require_bundle_relative_path:
            path = PurePosixPath(artifact_path)
            if (
                path.is_absolute()
                or ".." in path.parts
                or "\\" in artifact_path
                or artifact_path != path.as_posix()
            ):
                errors.append(f"{label} path is not canonical: {artifact_path}")
    bytes_value = artifact.get("bytes")
    if type(bytes_value) is not int or bytes_value < 0:
        errors.append(f"{label} bytes must be a non-negative integer")
    if not _is_canonical_sha256_text(artifact.get("sha256")):
        errors.append(f"{label} sha256 must be a canonical SHA-256 hex string")
    return errors


def _native_evm_artifact_summary_errors(row: Any, label: str) -> list[str]:
    errors: list[str] = []
    artifact = _require_report_mapping(row, label, errors)
    if errors:
        return errors
    require_bundle_relative_path = label.startswith("bundled report.")
    errors.extend(_native_evm_unknown_field_errors(artifact, label, ARTIFACT_FIELDS))
    _require_report_fields(artifact, label, ARTIFACT_FIELDS, errors)
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        errors.append(f"{label} path must be a non-empty string")
    else:
        if artifact_path.strip() != artifact_path:
            errors.append(
                f"{label} path must not contain surrounding whitespace: "
                f"{artifact_path!r}"
            )
        control_character = _path_control_character(artifact_path)
        if control_character is not None:
            errors.append(
                f"{label} path contains control character "
                f"{control_character}: {artifact_path!r}"
            )
        markdown_unsafe_character = _path_markdown_unsafe_character(artifact_path)
        if markdown_unsafe_character is not None:
            errors.append(
                f"{label} path contains Markdown-unsafe character "
                f"{markdown_unsafe_character}: {artifact_path!r}"
            )
        percent_traversal = _path_percent_encoded_traversal(artifact_path)
        if percent_traversal is not None:
            errors.append(
                f"{label} path contains percent-encoded traversal segment: "
                f"{percent_traversal}"
            )
        if require_bundle_relative_path:
            path = PurePosixPath(artifact_path)
            if (
                path.is_absolute()
                or ".." in path.parts
                or "\\" in artifact_path
                or artifact_path != path.as_posix()
            ):
                errors.append(f"{label} path is not canonical: {artifact_path}")
    bytes_value = artifact.get("bytes")
    if type(bytes_value) is not int or bytes_value < 0:
        errors.append(f"{label} bytes must be a non-negative integer")
    if not _is_canonical_sha256_text(artifact.get("sha256")):
        errors.append(f"{label} sha256 must be a canonical SHA-256 hex string")
    return errors


def _native_evm_artifact_hash_binding_errors(
    payload: dict[str, Any],
    label: str,
    artifact_field: str,
    hash_field: str,
) -> list[str]:
    artifact = payload.get(artifact_field)
    expected_hash = payload.get(hash_field)
    if not isinstance(artifact, dict):
        return []
    artifact_hash = artifact.get("sha256")
    if (
        _is_canonical_sha256_text(artifact_hash)
        and isinstance(expected_hash, str)
        and f"0x{artifact_hash}" != expected_hash
    ):
        return [f"{label} {artifact_field} sha256 must match {hash_field}"]
    return []


def _native_evm_summary_path_role_errors(
    label: str,
    payload: dict[str, Any],
) -> list[str]:
    roles: list[tuple[str, Any]] = [
        ("artifact", payload.get("artifact")),
        ("proof_artifact", payload.get("proof_artifact")),
        ("proving_key", payload.get("proving_key")),
        ("verifier_key", payload.get("verifier_key")),
        (
            "cross_sdk_fixture_parity_artifact",
            payload.get("cross_sdk_fixture_parity_artifact"),
        ),
        (
            "native_prover_self_test_artifact",
            payload.get("native_prover_self_test_artifact"),
        ),
    ]
    sdk_artifacts = payload.get("sdk_artifacts")
    if isinstance(sdk_artifacts, list):
        for index, row in enumerate(sdk_artifacts):
            if isinstance(row, dict):
                roles.append(
                    (
                        f"sdk_artifacts[{index}].implementation_artifact",
                        row.get("implementation_artifact"),
                    )
                )

    errors: list[str] = []
    seen: dict[str, str] = {}
    for role, artifact in roles:
        if not isinstance(artifact, dict):
            continue
        artifact_path = artifact.get("path")
        if not isinstance(artifact_path, str) or not artifact_path:
            continue
        path = PurePosixPath(artifact_path)
        if (
            artifact_path.strip() != artifact_path
            or _path_control_character(artifact_path) is not None
            or _path_markdown_unsafe_character(artifact_path) is not None
            or _path_percent_encoded_traversal(artifact_path) is not None
            or path.is_absolute()
            or ".." in path.parts
            or "\\" in artifact_path
            or artifact_path != path.as_posix()
        ):
            continue
        previous_role = seen.get(artifact_path)
        if previous_role is not None:
            errors.append(
                f"{label} {role} path must not reuse {previous_role}: "
                f"{artifact_path}"
            )
            continue
        seen[artifact_path] = role
    return errors


def _canonical_copied_input_path_errors(value: Any, label: str) -> list[str]:
    if not isinstance(value, str) or not value:
        return [f"{label} item must be a non-empty string"]
    if value.strip() != value:
        return [
            f"{label} path must not contain surrounding whitespace: {value!r}"
        ]
    control_character = _path_control_character(value)
    if control_character is not None:
        return [
            f"{label} path contains control character {control_character}: "
            f"{value!r}"
        ]
    markdown_unsafe_character = _path_markdown_unsafe_character(value)
    if markdown_unsafe_character is not None:
        return [
            f"{label} path contains Markdown-unsafe character "
            f"{markdown_unsafe_character}: {value!r}"
        ]
    percent_traversal = _path_percent_encoded_traversal(value)
    if percent_traversal is not None:
        return [
            f"{label} path contains percent-encoded traversal segment: "
            f"{percent_traversal}"
        ]
    if "\\" in value:
        return [f"{label} path is not canonical: {value}"]
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        return [f"{label} path escapes bundle: {value}"]
    if value != path.as_posix():
        return [f"{label} path is not canonical: {value}"]
    return []


def _copied_input_layout_errors(label: str, index: int, value: Any) -> list[str]:
    if not isinstance(value, str) or _canonical_copied_input_path_errors(value, label):
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


def _copied_input_provenance_bundle_errors(
    payload: dict[str, Any],
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []

    errors: list[str] = []
    input_paths: list[str] = []
    inputs = payload.get("inputs")
    inputs_label = f"{label}.inputs"
    if not isinstance(inputs, list) or not inputs:
        errors.append(f"{inputs_label} must be a non-empty list of canonical paths")
    else:
        seen_inputs: set[str] = set()
        for index, item in enumerate(inputs):
            errors.extend(_canonical_copied_input_path_errors(item, inputs_label))
            errors.extend(_copied_input_layout_errors(inputs_label, index, item))
            if isinstance(item, str):
                if item in seen_inputs:
                    errors.append(f"{inputs_label} contains duplicate path: {item}")
                seen_inputs.add(item)
                if not _canonical_copied_input_path_errors(item, inputs_label):
                    input_paths.append(item)

    artifact_paths: list[str] = []
    input_artifacts = payload.get("input_artifacts")
    artifacts_label = f"{label}.input_artifacts"
    if not isinstance(input_artifacts, list) or not input_artifacts:
        errors.append(f"{artifacts_label} must be a non-empty list")
    else:
        seen_artifacts: set[str] = set()
        for index, artifact in enumerate(input_artifacts):
            if not isinstance(artifact, dict):
                continue
            artifact_path = artifact.get("path")
            if not isinstance(artifact_path, str):
                continue
            if _canonical_copied_input_path_errors(artifact_path, artifacts_label):
                continue
            errors.extend(
                _copied_input_layout_errors(artifacts_label, index, artifact_path)
            )
            if artifact_path in seen_artifacts:
                errors.append(
                    f"{artifacts_label} contains duplicate path: {artifact_path}"
                )
            seen_artifacts.add(artifact_path)
            artifact_paths.append(artifact_path)

    if input_paths and artifact_paths and input_paths != artifact_paths:
        errors.append(f"{inputs_label} do not match copied input_artifacts")
    return errors


def _release_checklist_bundle_errors(
    checklist: Any,
    label: str,
    *,
    require_ready: bool,
) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(checklist, label, errors)
    if errors or not payload:
        return errors
    errors.extend(_unknown_report_field_errors(payload, label, RELEASE_CHECKLIST_FIELDS))
    _require_report_fields(payload, label, RELEASE_CHECKLIST_FIELDS, errors)
    if require_ready:
        if payload.get("ready") is not True:
            errors.append(f"{label} ready must be true")
    elif type(payload.get("ready")) is not bool:
        errors.append(f"{label} ready must be true or false")

    items = _require_report_list(payload.get("items"), f"{label}.items", errors)
    seen_item_ids: set[str] = set()
    for index, item in enumerate(items):
        item_base_label = f"{label}.items[{index}]"
        item_payload = _require_report_mapping(item, item_base_label, errors)
        if not isinstance(item, dict):
            continue
        item_id = item_payload.get("id")
        item_id_error = _checklist_item_id_error(item_id, item_base_label)
        if item_id_error is not None:
            errors.append(item_id_error)
            item_label = item_base_label
        else:
            assert isinstance(item_id, str)
            item_label = f"{label}.items[{item_id!r}]"
            if item_id in seen_item_ids:
                errors.append(f"{label} contains duplicate item id: {item_id}")
            seen_item_ids.add(item_id)
        errors.extend(
            _unknown_report_field_errors(
                item_payload,
                item_label,
                RELEASE_CHECKLIST_ITEM_FIELDS,
            )
        )
        _require_report_fields(
            item_payload,
            item_label,
            RELEASE_CHECKLIST_ITEM_FIELDS,
            errors,
        )
        title = item_payload.get("title")
        if not isinstance(title, str) or not title or title.strip() != title:
            errors.append(
                f"{item_label} title must be a non-empty string "
                "with no surrounding whitespace"
            )
        if require_ready:
            if item_payload.get("ready") is not True:
                errors.append(f"{item_label} ready must be true")
        elif type(item_payload.get("ready")) is not bool:
            errors.append(f"{item_label} ready must be true or false")
        errors.extend(
            _string_list_field_errors(
                item_label,
                item_payload,
                "blockers",
                allow_empty=True,
            )
        )
        blockers = item_payload.get("blockers")
        if require_ready and isinstance(blockers, list) and blockers:
            errors.append(f"{item_label} blockers must be empty")
    return errors


def _native_evm_prover_summary_errors(summary: Any, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(summary, label, errors)
    if errors or not payload:
        return errors

    errors.extend(
        _native_evm_unknown_field_errors(
            payload,
            label,
            NATIVE_EVM_PROVER_BUNDLE_SUMMARY_FIELDS,
        )
    )

    if "required" in payload and payload.get("required") is not True:
        errors.append(f"{label} required must be true")
    validation_status = payload.get("validation_status")
    if "validation_status" in payload and validation_status not in {"passed", "blocked"}:
        errors.append(f"{label} validation_status must be passed or blocked")
    elif validation_status != "passed":
        errors.append(f"{label} validation_status must be passed")
    if validation_status == "passed":
        _require_report_fields(
            payload,
            label,
            NATIVE_EVM_PROVER_BUNDLE_SUMMARY_FIELDS,
            errors,
        )
    for field in (
        "schema",
        "bundle_id",
        "lanes",
        "proof_backend",
    ):
        value = payload.get(field)
        if validation_status == "passed" or value not in ("", None):
            errors.extend(_non_empty_string_field_errors(label, payload, field))
    for field in (
        "proof_artifact_hash",
        "proving_key_hash",
        "verifier_key_hash",
        "destination_binding_hash",
    ):
        value = payload.get(field)
        if (
            field in payload
            and (validation_status == "passed" or value not in ("", None))
            and not _is_nonzero_bytes32_hex_text(value)
        ):
            errors.append(
                f"{label} {field} must be a canonical non-zero 32-byte hex value"
            )

    for artifact_field, hash_field in (
        ("artifact", ""),
        ("proof_artifact", "proof_artifact_hash"),
        ("proving_key", "proving_key_hash"),
        ("verifier_key", "verifier_key_hash"),
        ("cross_sdk_fixture_parity_artifact", ""),
        ("native_prover_self_test_artifact", ""),
    ):
        if artifact_field not in payload:
            continue
        artifact = payload.get(artifact_field)
        if validation_status != "passed" and artifact is None:
            continue
        artifact_label = f"{label}.{artifact_field}"
        if not isinstance(artifact, dict):
            errors.append(f"{artifact_label} must be an object")
            continue
        errors.extend(_native_evm_artifact_summary_errors(artifact, artifact_label))
        if hash_field:
            errors.extend(
                _native_evm_artifact_hash_binding_errors(
                    payload,
                    label,
                    artifact_field,
                    hash_field,
                )
            )

    audit_hashes = payload.get("audit_hashes")
    required_audit_hashes = _native_evm_required_audit_hashes()
    semantic_audit_hashes: dict[str, Any] = {}
    if "audit_hashes" in payload:
        if not isinstance(audit_hashes, dict):
            errors.append(f"{label}.audit_hashes must be a non-empty object")
        elif validation_status == "passed" and not audit_hashes:
            errors.append(f"{label}.audit_hashes must be a non-empty object")
        else:
            for key in sorted(audit_hashes, key=str):
                audit_label = f"{label}.audit_hashes"
                key_error = _source_adapter_gate_audit_key_error(key, audit_label)
                if key_error is not None:
                    errors.append(key_error)
                    continue
                if key not in required_audit_hashes:
                    errors.append(f"{audit_label} contains unexpected audit field: {key}")
                    continue
                semantic_audit_hashes[key] = audit_hashes[key]
            for key, value in sorted(semantic_audit_hashes.items()):
                if not _is_nonzero_bytes32_hex_text(value):
                    errors.append(
                        f"{label}.audit_hashes.{key} must be a canonical "
                        "non-zero 32-byte hex value"
                    )
    if validation_status == "passed" and isinstance(audit_hashes, dict):
        for key in sorted(required_audit_hashes - set(semantic_audit_hashes)):
            errors.append(f"{label}.audit_hashes missing field: {key}")

    reserved_audit_hash_roles: dict[str, Any] = {
        "proof_artifact_hash": payload.get("proof_artifact_hash"),
        "proving_key_hash": payload.get("proving_key_hash"),
        "verifier_key_hash": payload.get("verifier_key_hash"),
        "destination_binding_hash": payload.get("destination_binding_hash"),
    }

    parity_hash = semantic_audit_hashes.get("cross_sdk_fixture_parity")
    if isinstance(payload.get("cross_sdk_fixture_parity_artifact"), dict):
        artifact_hash = payload["cross_sdk_fixture_parity_artifact"].get("sha256")
        if (
            _is_canonical_sha256_text(artifact_hash)
            and isinstance(parity_hash, str)
            and f"0x{artifact_hash}" != parity_hash
        ):
            errors.append(
                f"{label} cross_sdk_fixture_parity_artifact sha256 must match "
                "audit_hashes.cross_sdk_fixture_parity"
            )
    self_test_hash = semantic_audit_hashes.get("native_prover_self_test")
    if isinstance(payload.get("native_prover_self_test_artifact"), dict):
        artifact_hash = payload["native_prover_self_test_artifact"].get("sha256")
        if (
            _is_canonical_sha256_text(artifact_hash)
            and isinstance(self_test_hash, str)
            and f"0x{artifact_hash}" != self_test_hash
        ):
            errors.append(
                f"{label} native_prover_self_test_artifact sha256 must match "
                "audit_hashes.native_prover_self_test"
            )

    if "validation_blockers" in payload:
        errors.extend(
            _string_list_field_errors(
                label,
                payload,
                "validation_blockers",
                allow_empty=True,
            )
        )
        validation_blockers = payload.get("validation_blockers")
        if (
            validation_status == "passed"
            and isinstance(validation_blockers, list)
            and validation_blockers
        ):
            errors.append(
                f"{label} validation_blockers must be empty when validation_status is passed"
            )
        if isinstance(validation_blockers, list) and validation_blockers:
            errors.append(f"{label} validation_blockers must be empty")

    seen_sdks: set[str] = set()
    if "sdk_artifacts" in payload:
        sdk_rows = _require_report_list(
            payload.get("sdk_artifacts"),
            f"{label}.sdk_artifacts",
            errors,
        )
        if validation_status == "passed" and not sdk_rows:
            errors.append(f"{label}.sdk_artifacts must be a non-empty list")
        for index, row in enumerate(sdk_rows):
            row_label = f"{label}.sdk_artifacts[{index}]"
            row_payload = _require_report_mapping(row, row_label, errors)
            if not isinstance(row, dict):
                continue
            errors.extend(
                _native_evm_unknown_field_errors(
                    row_payload,
                    row_label,
                    NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_FIELDS,
                )
            )
            _require_report_fields(
                row_payload,
                row_label,
                NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_FIELDS,
                errors,
            )
            sdk = row_payload.get("sdk")
            sdk_error = _native_evm_sdk_key_error(sdk, row_label)
            if sdk_error is not None:
                errors.append(sdk_error)
            else:
                assert isinstance(sdk, str)
                if sdk in seen_sdks:
                    errors.append(f"{label}.sdk_artifacts contains duplicate sdk: {sdk}")
                seen_sdks.add(sdk)
                expected_implementation = _native_evm_required_implementations().get(sdk)
                if expected_implementation is None:
                    errors.append(f"{label}.sdk_artifacts contains unknown sdk: {sdk}")
                elif row_payload.get("implementation") != expected_implementation:
                    errors.append(
                        f"{row_label} implementation must be {expected_implementation}"
                    )
            implementation = row_payload.get("implementation")
            if "implementation" in row_payload and (
                not isinstance(implementation, str)
                or not implementation
                or implementation.strip() != implementation
            ):
                errors.append(
                    f"{row_label} implementation must be a non-empty string "
                    "with no surrounding whitespace"
                )
            if "implementation_hash" in row_payload and not _is_nonzero_bytes32_hex_text(
                row_payload.get("implementation_hash")
            ):
                errors.append(
                    f"{row_label} implementation_hash must be a canonical "
                    "non-zero 32-byte hex value"
                )
            implementation_artifact = row_payload.get("implementation_artifact")
            if "implementation_artifact" in row_payload:
                artifact_label = f"{row_label}.implementation_artifact"
                if not isinstance(implementation_artifact, dict):
                    errors.append(f"{artifact_label} must be an object")
                else:
                    errors.extend(
                        _native_evm_artifact_summary_errors(
                            implementation_artifact,
                            artifact_label,
                        )
                    )
                    artifact_hash = implementation_artifact.get("sha256")
                    implementation_hash = row_payload.get("implementation_hash")
                    if (
                        _is_canonical_sha256_text(artifact_hash)
                        and isinstance(implementation_hash, str)
                        and f"0x{artifact_hash}" != implementation_hash
                    ):
                        errors.append(
                            f"{artifact_label} sha256 must match implementation_hash"
                        )
            if isinstance(sdk, str):
                reserved_audit_hash_roles[
                    f"sdk_artifacts[{index}].implementation_hash"
                ] = row_payload.get("implementation_hash")
        if validation_status == "passed":
            for sdk in sorted(
                set(_native_evm_required_implementations()) - seen_sdks
            ):
                errors.append(f"{label}.sdk_artifacts missing sdk: {sdk}")

    seen_audit_hashes: dict[str, str] = {}
    for key, value in sorted(semantic_audit_hashes.items()):
        if not _is_nonzero_bytes32_hex_text(value):
            continue
        previous_key = seen_audit_hashes.get(value)
        if previous_key is not None:
            errors.append(
                f"{label}.audit_hashes.{key} must not duplicate "
                f"audit_hashes.{previous_key}"
            )
        seen_audit_hashes[value] = key
        for role, role_hash in reserved_audit_hash_roles.items():
            if value == role_hash:
                errors.append(f"{label}.audit_hashes.{key} must not reuse {role}")

    errors.extend(_native_evm_summary_path_role_errors(label, payload))
    return errors


def _cryptographic_evidence_row_bundle_errors(row: Any, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(row, label, errors)
    if errors:
        return errors

    errors.extend(
        _unknown_report_field_errors(
            payload,
            label,
            CRYPTOGRAPHIC_EVIDENCE_ROW_FIELDS,
        )
    )
    _require_report_fields(payload, label, CRYPTOGRAPHIC_EVIDENCE_ROW_FIELDS, errors)
    if "domain" in payload and type(payload.get("domain")) is not int:
        errors.append(f"{label} domain must be an integer")
    chain = payload.get("chain")
    if "chain" in payload and (
        not isinstance(chain, str) or not chain or chain.strip() != chain
    ):
        errors.append(
            f"{label} chain must be a non-empty string with no surrounding whitespace"
        )
    for field in (
        "evm_source_rpc_chain_id",
        "evm_source_block_tag",
        "evm_destination_rpc_chain_id",
        "evm_destination_block_tag",
    ):
        if field in payload and not isinstance(payload.get(field), str):
            errors.append(f"{label} {field} must be a string")
    route_canary_source = payload.get("route_canary_evidence_source")
    if "route_canary_evidence_source" in payload and route_canary_source not in (
        "",
        None,
    ):
        if (
            not isinstance(route_canary_source, str)
            or route_canary_source.strip() != route_canary_source
        ):
            errors.append(
                f"{label} route_canary_evidence_source must be a non-empty string "
                "with no surrounding whitespace, empty, or null"
            )
    for field in ("route_canary_evidence_bound", "source_adapter_gate_required"):
        if field in payload and type(payload.get(field)) is not bool:
            errors.append(f"{label} {field} must be true or false")
    if (
        "route_canary_receipt_block_finalized" in payload
        and payload.get("route_canary_receipt_block_finalized") is not None
        and type(payload.get("route_canary_receipt_block_finalized")) is not bool
    ):
        errors.append(
            f"{label} route_canary_receipt_block_finalized must be true, false, or null"
        )
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "source_adapter_gate_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
        "route_canary_transaction_hash",
        "route_canary_receipt_block_hash",
        "route_canary_block_receipts_root",
        "route_canary_message_id",
    ):
        errors.extend(_optional_bytes32_field_errors(label, payload, field))
    errors.extend(
        _optional_integer_field_errors(
            label,
            payload,
            "route_canary_receipt_block_number",
            positive=True,
        )
    )
    errors.extend(
        _optional_integer_field_errors(
            label,
            payload,
            "route_canary_block_number",
            positive=True,
        )
    )
    errors.extend(
        _optional_integer_field_errors(
            label,
            payload,
            "route_canary_block_timestamp",
            positive=False,
        )
    )
    audit_hashes = payload.get("source_adapter_gate_audit_hashes")
    if "source_adapter_gate_audit_hashes" in payload:
        if not isinstance(audit_hashes, dict):
            errors.append(f"{label} source_adapter_gate_audit_hashes must be an object")
        else:
            audit_label = f"{label} source_adapter_gate_audit_hashes"
            for audit_key, audit_hash in sorted(
                audit_hashes.items(),
                key=lambda item: str(item[0]),
            ):
                key_error = _source_adapter_gate_audit_key_error(
                    audit_key,
                    audit_label,
                )
                if key_error is not None:
                    errors.append(key_error)
                    continue
                if not _is_canonical_bytes32_hex_text(audit_hash):
                    errors.append(
                        f"{audit_label} {audit_key} must be a canonical bytes32 hex string"
                    )
    return errors


def _cryptographic_evidence_lane_binding_bundle_errors(
    crypto_rows: list[Any],
    lanes: Any,
    *,
    crypto_label: str,
    lanes_label: str,
) -> list[str]:
    if not isinstance(lanes, list):
        return []

    errors: list[str] = []
    if len(crypto_rows) != len(lanes):
        errors.append(f"{crypto_label} must cover every embedded lane")

    seen_domains: set[int] = set()
    field_bindings = (
        ("evm_source_rpc_chain_id", ("evm_live_metadata", "source_rpc_chain_id")),
        ("evm_source_block_tag", ("evm_live_metadata", "source_block_tag")),
        (
            "evm_destination_rpc_chain_id",
            ("evm_live_metadata", "destination_rpc_chain_id"),
        ),
        ("evm_destination_block_tag", ("evm_live_metadata", "destination_block_tag")),
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
        (
            "route_canary_transaction_hash",
            ("route_allowlist", "route_canary", "transaction_hash"),
        ),
        (
            "route_canary_receipt_block_number",
            ("route_allowlist", "route_canary", "receipt_block_number"),
        ),
        (
            "route_canary_receipt_block_hash",
            ("route_allowlist", "route_canary", "receipt_block_hash"),
        ),
        (
            "route_canary_receipt_block_finalized",
            ("route_allowlist", "route_canary", "receipt_block_finalized"),
        ),
        (
            "route_canary_block_receipts_root",
            ("route_allowlist", "route_canary", "block_receipts_root"),
        ),
        ("route_canary_message_id", ("route_allowlist", "route_canary", "message_id")),
        ("route_canary_block_number", ("route_allowlist", "route_canary", "block_number")),
        (
            "route_canary_block_timestamp",
            ("route_allowlist", "route_canary", "block_timestamp"),
        ),
        ("source_adapter_gate_required", ("source_adapter_gate", "required")),
        ("source_adapter_gate_hash", ("source_adapter_gate", "gate_hash")),
        ("source_adapter_gate_audit_hashes", ("source_adapter_gate", "audit_hashes")),
    )

    missing = object()
    for index, row in enumerate(crypto_rows):
        if not isinstance(row, dict):
            continue
        row_label = f"{crypto_label}[{index}]"
        domain = row.get("domain")
        if type(domain) is int:
            if domain in seen_domains:
                errors.append(f"{row_label} duplicates domain {domain}")
            seen_domains.add(domain)
        if index >= len(lanes) or not isinstance(lanes[index], dict):
            errors.append(f"{row_label} has no embedded lane")
            continue
        lane = lanes[index]
        lane_label = f"{lanes_label}[{index}]"
        lane_domain = lane.get("domain")
        if type(domain) is int and type(lane_domain) is int and domain != lane_domain:
            errors.append(f"{row_label} domain must match {lane_label} domain")
        chain = row.get("chain")
        lane_chain = lane.get("chain")
        if (
            isinstance(chain, str)
            and chain
            and isinstance(lane_chain, str)
            and lane_chain
            and chain != lane_chain
        ):
            errors.append(f"{row_label} chain must match {lane_label} chain")

        for field, lane_path in field_bindings:
            if field not in row:
                continue
            expected: Any = lane
            for segment in lane_path:
                if not isinstance(expected, dict) or segment not in expected:
                    expected = missing
                    break
                expected = expected[segment]
            if expected is missing or row.get(field) == expected:
                continue
            lane_field = ".".join(lane_path)
            errors.append(f"{row_label} {field} must match {lane_label}.{lane_field}")
    return errors


def _submission_surface_row_bundle_errors(surface: Any, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(surface, label, errors)
    if errors:
        return errors

    errors.extend(
        _unknown_report_field_errors(
            payload,
            label,
            USER_PROVER_SUBMISSION_SURFACE_FIELDS,
        )
    )
    _require_report_fields(payload, label, USER_PROVER_SUBMISSION_SURFACE_FIELDS, errors)
    for field in ("lanes", "proof_backend", "sdk_helpers", "on_chain_submission"):
        value = payload.get(field)
        if field in payload and (
            not isinstance(value, str) or not value or value.strip() != value
        ):
            errors.append(
                f"{label} {field} must be a non-empty string "
                "with no surrounding whitespace"
            )

    errors.extend(
        _string_list_field_errors(
            label,
            payload,
            "sdk_helper_symbols",
            allow_empty=False,
        )
    )
    helper_symbols = payload.get("sdk_helper_symbols")
    if isinstance(helper_symbols, list) and all(
        isinstance(item, str) and item for item in helper_symbols
    ):
        expected_helpers = ", ".join(helper_symbols)
        if payload.get("sdk_helpers") != expected_helpers:
            errors.append(f"{label} sdk_helpers must match sdk_helper_symbols")

    helper_sets = payload.get("sdk_helper_symbols_by_sdk")
    if not isinstance(helper_sets, dict):
        errors.append(f"{label} sdk_helper_symbols_by_sdk must be an object")
    else:
        known_sdks = _submission_surface_known_sdks()
        semantic_helper_sets: dict[str, Any] = {}
        sdk_label = f"{label} sdk_helper_symbols_by_sdk"
        for sdk, helpers in sorted(helper_sets.items(), key=lambda item: str(item[0])):
            sdk_error = _submission_surface_sdk_key_error(sdk, sdk_label)
            if sdk_error is not None:
                errors.append(sdk_error)
                continue
            if sdk not in known_sdks:
                errors.append(f"{sdk_label} contains unknown SDK: {sdk}")
            semantic_helper_sets[sdk] = helpers
            row_label = f"{sdk_label}[{sdk}]"
            if not isinstance(helpers, list) or not helpers:
                errors.append(f"{row_label} must be a list of non-empty strings")
                continue
            if any(not isinstance(item, str) or not item for item in helpers):
                errors.append(f"{row_label} must be a list of non-empty strings")
                continue
            if any(item.strip() != item for item in helpers):
                errors.append(
                    f"{row_label} must be a list of non-empty strings "
                    "with no surrounding whitespace"
                )
                continue
            if len(helpers) != len(set(helpers)):
                errors.append(f"{row_label} contains duplicate symbols")
        js_helpers = semantic_helper_sets.get("js-sdk")
        if (
            isinstance(js_helpers, list)
            and isinstance(helper_symbols, list)
            and js_helpers != helper_symbols
        ):
            errors.append(
                f"{label} sdk_helper_symbols_by_sdk[js-sdk] must match "
                "sdk_helper_symbols"
            )

    errors.extend(
        _string_list_field_errors(
            label,
            payload,
            "required_phases",
            allow_empty=False,
        )
    )
    required_phases = payload.get("required_phases")
    if isinstance(required_phases, list) and all(
        isinstance(item, str) and item for item in required_phases
    ):
        semantic_phases: list[str] = []
        phase_label = f"{label} required_phases"
        known_phases = _submission_surface_known_required_phases()
        for phase in required_phases:
            phase_error = _corridor_phase_key_error(phase, phase_label)
            if phase_error is not None:
                errors.append(phase_error)
                continue
            semantic_phases.append(phase)
        if len(semantic_phases) != len(set(semantic_phases)):
            errors.append(f"{phase_label} contains duplicate phases")
        for phase in sorted(set(semantic_phases) - known_phases):
            errors.append(f"{phase_label} contains unknown phase: {phase}")

    validation_status = payload.get("validation_status")
    if "validation_status" in payload and validation_status not in {"passed", "blocked"}:
        errors.append(f"{label} validation_status must be passed or blocked")
    elif validation_status != "passed":
        errors.append(f"{label} validation_status must be passed")
    if "validation_blockers" in payload:
        errors.extend(
            _string_list_field_errors(
                label,
                payload,
                "validation_blockers",
                allow_empty=True,
            )
        )
        validation_blockers = payload.get("validation_blockers")
        if (
            validation_status == "passed"
            and isinstance(validation_blockers, list)
            and validation_blockers
        ):
            errors.append(
                f"{label} validation_blockers must be empty when validation_status is passed"
            )
        if isinstance(validation_blockers, list) and validation_blockers:
            errors.append(f"{label} validation_blockers must be empty")
    return errors


def _submission_surface_binding_bundle_errors(
    surfaces: list[Any],
    report: dict[str, Any],
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []

    errors: list[str] = []
    try:
        expected_surfaces = _verify_module()._expected_submission_surfaces(report)
    except Exception as exc:
        return [f"{label}.user_prover_submission_surfaces cannot be recomputed: {exc}"]

    expected_by_lanes = {
        surface["lanes"]: surface
        for surface in expected_surfaces
        if isinstance(surface, dict) and isinstance(surface.get("lanes"), str)
    }
    seen_lanes: set[str] = set()
    for surface in surfaces:
        if not isinstance(surface, dict):
            continue
        lanes = surface.get("lanes")
        if not isinstance(lanes, str) or not lanes:
            continue
        if lanes in seen_lanes:
            errors.append(
                f"{label}.user_prover_submission_surfaces contains duplicate "
                f"lanes row: {lanes}"
            )
        else:
            seen_lanes.add(lanes)
        expected = expected_by_lanes.get(lanes)
        if expected is None:
            errors.append(
                f"{label}.user_prover_submission_surfaces contains unknown "
                f"lanes row: {lanes}"
            )
            continue
        if surface.get("proof_backend") != expected.get("proof_backend"):
            errors.append(
                f"{label}.user_prover_submission_surfaces proof_backend mismatch "
                f"for lanes {lanes}: expected {expected.get('proof_backend')}, "
                f"got {surface.get('proof_backend')!r}"
            )
        helper_sets = surface.get("sdk_helper_symbols_by_sdk")
        expected_helper_sets = expected.get("sdk_helper_symbols_by_sdk")
        if isinstance(helper_sets, dict) and isinstance(expected_helper_sets, dict):
            for sdk, expected_helpers in expected_helper_sets.items():
                helpers = helper_sets.get(sdk)
                if not isinstance(helpers, list):
                    continue
                for helper in expected_helpers:
                    if helper not in helpers:
                        errors.append(
                            f"{label}.user_prover_submission_surfaces lanes "
                            f"{lanes} sdk_helper_symbols_by_sdk[{sdk}] missing "
                            f"required helper: {helper}"
                        )
            helper_symbols = surface.get("sdk_helper_symbols")
            expected_js_helpers = expected_helper_sets.get("js-sdk", ())
            if isinstance(helper_symbols, list):
                for helper in expected_js_helpers:
                    if helper not in helper_symbols:
                        errors.append(
                            f"{label}.user_prover_submission_surfaces lanes "
                            f"{lanes} sdk_helper_symbols missing required "
                            f"helper: {helper}"
                        )
    for expected_lanes in expected_by_lanes:
        if expected_lanes not in seen_lanes:
            errors.append(
                f"{label}.user_prover_submission_surfaces missing required "
                f"lanes row: {expected_lanes}"
            )
    if surfaces != expected_surfaces:
        errors.append(
            f"{label}.user_prover_submission_surfaces must match copied corridor phases"
        )
    return errors


def _native_evm_prover_binding_bundle_errors(
    summary: dict[str, Any],
    report: dict[str, Any],
    bundle_dir: Path | None,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    if bundle_dir is None:
        return [f"{label}.native_evm_prover_bundle cannot be recomputed: missing bundle directory"]
    evidence = report.get("evidence")
    if not isinstance(evidence, dict):
        evidence = {}
    try:
        expected_summary = _verify_module()._expected_native_evm_prover_bundle_status(
            bundle_dir,
            report,
            evidence,
        )
    except Exception as exc:
        return [f"{label}.native_evm_prover_bundle cannot be recomputed: {exc}"]

    errors = [
        f"bundled native EVM prover manifest blocker: {blocker}"
        for blocker in expected_summary.get("validation_blockers", [])
    ]
    if summary != expected_summary:
        errors.append(
            f"{label}.native_evm_prover_bundle does not match bundled native prover manifest"
        )
    return errors


def _copied_evidence_binding_bundle_errors(
    summary: dict[str, Any],
    report: dict[str, Any],
    bundle_dir: Path | None,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    if bundle_dir is None:
        return [f"{label}.evidence cannot be recomputed: missing bundle directory"]
    recompute_errors: list[str] = []
    try:
        copied_summary = _verify_module()._copied_input_summary(
            bundle_dir,
            report,
            recompute_errors,
        )
    except Exception as exc:
        return [f"{label}.evidence cannot be recomputed from copied inputs: {exc}"]

    errors = [
        f"{label}.evidence copied input blocker: {error}"
        for error in recompute_errors
    ]
    if copied_summary is not None and summary != copied_summary:
        errors.append(f"{label}.evidence does not match copied evidence inputs")
    return errors


def _release_checklist_binding_bundle_errors(
    checklist: dict[str, Any],
    report: dict[str, Any],
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    try:
        expected_checklist = _verify_module()._expected_release_checklist(report)
    except Exception as exc:
        return [f"{label}.release_checklist cannot be recomputed: {exc}"]
    if checklist != expected_checklist:
        return [f"{label}.release_checklist does not match embedded evidence"]
    return []


def _corridor_phase_transcript_bundle_errors(
    corridor: dict[str, Any],
    bundle_dir: Path | None,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    if bundle_dir is None:
        return [f"{label}.corridor phase evidence cannot be checked: missing bundle directory"]
    phases = corridor.get("phases")
    evidence_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(phases, dict) or not isinstance(evidence_artifacts, dict):
        return []
    known_phases = _corridor_phase_names()
    errors: list[str] = []
    verifier = _verify_module()
    for phase, status in phases.items():
        if status != "passed":
            continue
        phase_error = _corridor_phase_key_error(
            phase,
            f"{label}.corridor.phases",
        )
        if phase_error is not None or phase not in known_phases:
            continue
        errors.extend(
            verifier._phase_transcript_errors(
                bundle_dir,
                phase,
                evidence_artifacts.get(phase),
            )
        )
    return errors


def _bundled_artifact_integrity_errors(
    artifact: Any,
    bundle_dir: Path | None,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report."):
        return []
    if not isinstance(artifact, dict):
        return []
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return []
    if (
        artifact_path.strip() != artifact_path
        or _path_control_character(artifact_path) is not None
        or _path_markdown_unsafe_character(artifact_path) is not None
        or _path_percent_encoded_traversal(artifact_path) is not None
    ):
        return []
    relative_path = PurePosixPath(artifact_path)
    if (
        relative_path.is_absolute()
        or ".." in relative_path.parts
        or "\\" in artifact_path
        or artifact_path != relative_path.as_posix()
    ):
        return []
    if bundle_dir is None:
        return [f"{label} artifact cannot be checked: missing bundle directory"]

    path = bundle_dir.joinpath(*relative_path.parts)
    current = bundle_dir
    for part in relative_path.parts:
        current = current / part
        if current.is_symlink():
            return [f"{label} artifact path uses symlink: {artifact_path}"]
    if not path.is_file():
        return [f"{label} artifact file is missing: {artifact_path}"]

    errors: list[str] = []
    expected_bytes = artifact.get("bytes")
    if type(expected_bytes) is int and expected_bytes >= 0:
        actual_bytes = path.stat().st_size
        if expected_bytes != actual_bytes:
            errors.append(
                f"{label} artifact byte length mismatch for {artifact_path}: "
                f"expected {expected_bytes}, got {actual_bytes}"
            )
    expected_hash = artifact.get("sha256")
    if _is_canonical_sha256_text(expected_hash):
        actual_hash = hashlib.sha256(path.read_bytes()).hexdigest()
        if expected_hash != actual_hash:
            errors.append(
                f"{label} artifact sha256 mismatch for {artifact_path}: "
                f"expected {expected_hash}, got {actual_hash}"
            )
    return errors


def _release_report_artifact_integrity_bundle_errors(
    payload: dict[str, Any],
    bundle_dir: Path | None,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    errors: list[str] = []

    input_artifacts = payload.get("input_artifacts")
    if isinstance(input_artifacts, list):
        for index, artifact in enumerate(input_artifacts):
            errors.extend(
                _bundled_artifact_integrity_errors(
                    artifact,
                    bundle_dir,
                    f"{label}.input_artifacts[{index}]",
                )
            )

    corridor = payload.get("corridor")
    if isinstance(corridor, dict):
        phase_artifacts = corridor.get("evidence_artifacts")
        if isinstance(phase_artifacts, dict):
            for phase, artifact in phase_artifacts.items():
                phase_error = _corridor_phase_key_error(
                    phase,
                    f"{label}.corridor.evidence_artifacts",
                )
                artifact_label = (
                    f"{label}.corridor.evidence_artifacts[malformed phase]"
                    if phase_error is not None
                    else f"{label}.corridor.evidence_artifacts[{phase!r}]"
                )
                errors.extend(
                    _bundled_artifact_integrity_errors(
                        artifact,
                        bundle_dir,
                        artifact_label,
                    )
                )

    native_summary = payload.get("native_evm_prover_bundle")
    if isinstance(native_summary, dict):
        for field in (
            "artifact",
            "proof_artifact",
            "proving_key",
            "verifier_key",
            "cross_sdk_fixture_parity_artifact",
            "native_prover_self_test_artifact",
        ):
            errors.extend(
                _bundled_artifact_integrity_errors(
                    native_summary.get(field),
                    bundle_dir,
                    f"{label}.native_evm_prover_bundle.{field}",
                )
            )
        sdk_artifacts = native_summary.get("sdk_artifacts")
        if isinstance(sdk_artifacts, list):
            for index, row in enumerate(sdk_artifacts):
                if not isinstance(row, dict):
                    continue
                errors.extend(
                    _bundled_artifact_integrity_errors(
                        row.get("implementation_artifact"),
                        bundle_dir,
                        (
                            f"{label}.native_evm_prover_bundle."
                            f"sdk_artifacts[{index}].implementation_artifact"
                        ),
                    )
                )
    return errors


def _readiness_markdown_bundle_errors(
    report: dict[str, Any],
    markdown: Any,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    if not isinstance(markdown, str):
        return [f"{label}.markdown must be UTF-8 text"]
    verifier = _verify_module()
    errors = verifier._readiness_markdown_invariant_errors(report, markdown)
    try:
        expected_markdown = verifier._expected_readiness_markdown(report)
    except Exception as exc:
        errors.append(f"{label}.markdown cannot be rendered canonically: {exc}")
    else:
        if markdown != expected_markdown:
            errors.append(
                "readiness report Markdown does not match readiness report JSON"
            )
    return errors


def _all_lanes_nested_bundle_errors(
    lane: dict[str, Any],
    label: str,
) -> list[str]:
    errors: list[str] = []
    field_sets = _all_lanes_nested_field_sets()
    enforce_governed_hash_semantics = (
        lane.get("domain") == _active_launch_domain()
        or lane.get("production_ready") is True
    )
    governed_required_fields: frozenset[str] = (
        frozenset(field_sets["source_record_hashes"])
        if enforce_governed_hash_semantics
        else frozenset()
    )

    source_hashes = _all_lanes_object(
        lane.get("source_record_hashes"),
        f"{label}.source_record_hashes",
        field_sets["source_record_hashes"],
        governed_required_fields,
        errors,
    )
    for field in field_sets["source_record_hashes"]:
        errors.extend(
            _optional_nonzero_bytes32_field_errors(
                f"{label}.source_record_hashes",
                source_hashes,
                field,
            )
        )

    source_gate = _all_lanes_object(
        lane.get("source_adapter_gate"),
        f"{label}.source_adapter_gate",
        field_sets["source_adapter_gate"],
        field_sets["source_adapter_gate"] if enforce_governed_hash_semantics else frozenset(),
        errors,
    )
    for field in ("required", "ready"):
        if field in source_gate and type(source_gate.get(field)) is not bool:
            errors.append(f"{label}.source_adapter_gate {field} must be true or false")
    errors.extend(
        _optional_nonzero_bytes32_field_errors(
            f"{label}.source_adapter_gate",
            source_gate,
            "gate_hash",
        )
    )
    errors.extend(
        _string_list_field_errors(
            f"{label}.source_adapter_gate",
            source_gate,
            "blockers",
            allow_empty=True,
        )
    )
    audit_hashes = source_gate.get("audit_hashes")
    if "audit_hashes" in source_gate:
        audit_label = f"{label}.source_adapter_gate audit_hashes"
        if not isinstance(audit_hashes, dict):
            errors.append(f"{audit_label} must be an object")
        else:
            for audit_key, audit_hash in sorted(
                audit_hashes.items(),
                key=lambda item: str(item[0]),
            ):
                key_error = _source_adapter_gate_audit_key_error(
                    audit_key,
                    audit_label,
                )
                if key_error is not None:
                    errors.append(key_error)
                    continue
                if not _is_canonical_bytes32_hex_text(audit_hash) or all(
                    character == "0" for character in audit_hash[2:]
                ):
                    errors.append(
                        f"{audit_label} {audit_key} must be a non-zero canonical "
                        "bytes32 hex string"
                    )
    if (
        enforce_governed_hash_semantics
        and source_gate.get("ready") is True
        and isinstance(source_gate.get("blockers"), list)
        and source_gate.get("blockers")
    ):
        errors.append(f"{label}.source_adapter_gate blockers must be empty when ready")
    if enforce_governed_hash_semantics:
        errors.extend(_source_adapter_gate_semantic_errors(label, lane, source_gate))

    evm_live = _all_lanes_object(
        lane.get("evm_live_metadata"),
        f"{label}.evm_live_metadata",
        field_sets["evm_live_metadata"],
        field_sets["evm_live_metadata"] if enforce_governed_hash_semantics else frozenset(),
        errors,
    )
    for field in ("required", "ready"):
        if field in evm_live and type(evm_live.get(field)) is not bool:
            errors.append(f"{label}.evm_live_metadata {field} must be true or false")
    for field in (
        "source_rpc_chain_id",
        "source_block_tag",
        "destination_rpc_chain_id",
        "destination_block_tag",
    ):
        errors.extend(
            _string_field_errors(
                f"{label}.evm_live_metadata",
                evm_live,
                field,
                allow_empty=True,
            )
        )
    domain = lane.get("domain")
    if domain in _all_lanes_evm_destination_domains():
        if (
            enforce_governed_hash_semantics
            and "required" in evm_live
            and evm_live.get("required") is not True
        ):
            errors.append(f"{label}.evm_live_metadata required must be true")
        if (
            enforce_governed_hash_semantics
            and domain == _active_launch_domain()
            and "ready" in evm_live
            and evm_live.get("ready") is not True
        ):
            errors.append(f"{label}.evm_live_metadata ready must be true")
        expected_chain_id = _expected_evm_rpc_chain_id(domain, lane.get("chain"))
        for field in ("source_rpc_chain_id", "destination_rpc_chain_id"):
            value = evm_live.get(field)
            if enforce_governed_hash_semantics and not value:
                errors.append(f"{label}.evm_live_metadata {field} must be present")
            elif value not in ("", None) and isinstance(value, str) and (
                not _is_canonical_decimal_text(value, positive=True)
                or int(value, 10) != expected_chain_id
            ):
                errors.append(
                    f"{label}.evm_live_metadata {field} "
                    f"must be canonical chain id {expected_chain_id}"
                )
        for field in ("source_block_tag", "destination_block_tag"):
            if enforce_governed_hash_semantics and not evm_live.get(field):
                errors.append(f"{label}.evm_live_metadata {field} must be present")
        if domain == _sccp_domain_eth():
            for field in ("source_block_tag", "destination_block_tag"):
                if (
                    enforce_governed_hash_semantics
                    and evm_live.get(field) != "finalized"
                ):
                    errors.append(
                        f"{label}.evm_live_metadata {field} "
                        "must be finalized for Ethereum mainnet"
                    )
    else:
        if "required" in evm_live and evm_live.get("required") is not False:
            errors.append(
                f"{label}.evm_live_metadata required must be false for non-EVM lanes"
            )
        if "ready" in evm_live and evm_live.get("ready") is not True:
            errors.append(
                f"{label}.evm_live_metadata ready must be true for non-EVM lanes"
            )
        for field in (
            "source_rpc_chain_id",
            "source_block_tag",
            "destination_rpc_chain_id",
            "destination_block_tag",
        ):
            if evm_live.get(field) not in ("", None):
                errors.append(
                    f"{label}.evm_live_metadata {field} must be empty "
                    "for non-EVM lanes"
                )

    destination_binding = _all_lanes_object(
        lane.get("destination_binding"),
        f"{label}.destination_binding",
        field_sets["destination_binding"],
        (
            field_sets["destination_binding_required"]
            if enforce_governed_hash_semantics
            else frozenset()
        ),
        errors,
    )
    for field in (
        "destination_binding_hash",
        "expected_destination_binding_hash",
    ):
        errors.extend(
            _optional_nonzero_bytes32_field_errors(
                f"{label}.destination_binding",
                destination_binding,
                field,
            )
        )
    for field in ("destination_binding_key", "destination_bridge_address"):
        errors.extend(
            _string_field_errors(
                f"{label}.destination_binding",
                destination_binding,
                field,
                allow_empty=True,
            )
        )
    if (
        "destination_network_id" in destination_binding
        and destination_binding.get("destination_network_id") is not None
    ):
        errors.extend(
            _string_field_errors(
                f"{label}.destination_binding",
                destination_binding,
                "destination_network_id",
                allow_empty=True,
            )
        )
    for field in ("expected_destination_binding_hash_matches", "recomputed"):
        if (
            field in destination_binding
            and type(destination_binding.get(field)) is not bool
        ):
            errors.append(
                f"{label}.destination_binding {field} must be true or false"
            )
        elif (
            enforce_governed_hash_semantics
            and field in destination_binding
            and destination_binding.get(field) is not True
        ):
            errors.append(f"{label}.destination_binding {field} must be true")
    destination_hash = destination_binding.get("destination_binding_hash")
    expected_destination_hash = destination_binding.get(
        "expected_destination_binding_hash"
    )
    if (
        enforce_governed_hash_semantics
        and _is_nonzero_bytes32_hex_text(destination_hash)
        and _is_nonzero_bytes32_hex_text(expected_destination_hash)
        and expected_destination_hash != destination_hash
    ):
        errors.append(
            f"{label}.destination_binding expected_destination_binding_hash must match destination_binding_hash"
        )

    route_allowlist = _all_lanes_object(
        lane.get("route_allowlist"),
        f"{label}.route_allowlist",
        field_sets["route_allowlist"],
        field_sets["route_allowlist"] if enforce_governed_hash_semantics else frozenset(),
        errors,
    )
    for field in ("route_allowlist_hash", "expected_route_allowlist_hash"):
        errors.extend(
            _optional_nonzero_bytes32_field_errors(
                f"{label}.route_allowlist",
                route_allowlist,
                field,
            )
        )
    if (
        "expected_route_allowlist_hash_matches" in route_allowlist
        and type(route_allowlist.get("expected_route_allowlist_hash_matches"))
        is not bool
    ):
        errors.append(
            f"{label}.route_allowlist expected_route_allowlist_hash_matches "
            "must be true or false"
        )
    elif (
        enforce_governed_hash_semantics
        and "expected_route_allowlist_hash_matches" in route_allowlist
        and route_allowlist.get("expected_route_allowlist_hash_matches") is not True
    ):
        errors.append(
            f"{label}.route_allowlist expected_route_allowlist_hash_matches "
            "must be true"
        )
    route_hash = route_allowlist.get("route_allowlist_hash")
    expected_route_hash = route_allowlist.get("expected_route_allowlist_hash")
    if (
        enforce_governed_hash_semantics
        and _is_nonzero_bytes32_hex_text(route_hash)
        and _is_nonzero_bytes32_hex_text(expected_route_hash)
        and expected_route_hash != route_hash
    ):
        errors.append(
            f"{label}.route_allowlist expected_route_allowlist_hash must match route_allowlist_hash"
        )
    route_canary_value = route_allowlist.get("route_canary")
    if enforce_governed_hash_semantics or route_canary_value is not None:
        route_canary = _all_lanes_object(
            route_canary_value,
            f"{label}.route_allowlist.route_canary",
            _all_lanes_route_canary_fields(lane.get("domain")),
            (
                _all_lanes_route_canary_fields(lane.get("domain"))
                if enforce_governed_hash_semantics
                else frozenset()
            ),
            errors,
        )
    else:
        route_canary = {}
    for field in (
        "evidence_hash",
        "route_allowlist_hash",
        "destination_binding_hash",
        "transaction_hash",
        "receipt_block_hash",
        "block_receipts_root",
        "call_data_sha256",
        "message_id",
        "payload_hash",
        "statement_hash",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
        "transaction_id",
        "signature_sha256",
        "ton_account_state_hash",
        "ton_last_transaction_hash",
    ):
        errors.extend(
            _optional_nonzero_bytes32_field_errors(
                f"{label}.route_allowlist.route_canary",
                route_canary,
                field,
            )
        )
    for field in ("status", "evidence_source"):
        errors.extend(
            _string_field_errors(
                f"{label}.route_allowlist.route_canary",
                route_canary,
                field,
                allow_empty=False,
            )
        )
    for field in (
        "evidence_bound",
        "message_proof_used",
        "receipt_block_finalized",
        "raw_data_owner_matches_transaction",
        "signature_recovers_to_owner",
    ):
        if field in route_canary and type(route_canary.get(field)) is not bool:
            errors.append(
                f"{label}.route_allowlist.route_canary {field} must be true or false"
            )
    for field in (
        "log_index",
        "receipt_block_number",
        "target_domain",
        "proof_version",
        "proof_source_domain",
        "block_number",
        "block_timestamp",
    ):
        errors.extend(
            _optional_integer_field_errors(
                f"{label}.route_allowlist.route_canary",
                route_canary,
                field,
                positive=False,
            )
        )
    for field in (
        "transaction_owner_address",
        "signature_recovered_address",
        "solana_programdata_address",
    ):
        errors.extend(
            _string_field_errors(
                f"{label}.route_allowlist.route_canary",
                route_canary,
                field,
                allow_empty=False,
            )
        )
    if enforce_governed_hash_semantics:
        errors.extend(
            _route_canary_common_semantic_errors(
                label,
                lane,
                route_allowlist,
                destination_binding,
                route_canary,
            )
        )
        errors.extend(
            _route_canary_evm_semantic_errors(
                label,
                lane,
                route_allowlist,
                destination_binding,
                route_canary,
            )
        )
        errors.extend(
            _route_canary_tron_semantic_errors(
                label,
                lane,
                route_allowlist,
                destination_binding,
                route_canary,
            )
        )
        errors.extend(_route_canary_solana_semantic_errors(label, lane, route_canary))
        errors.extend(
            _route_canary_ton_semantic_errors(
                label,
                lane,
                route_allowlist,
                destination_binding,
                route_canary,
            )
        )
    return errors


def _all_lanes_summary_bundle_errors(summary: Any, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(summary, label, errors)
    if errors:
        return errors
    errors.extend(
        _unknown_report_field_errors(payload, label, ALL_LANES_SUMMARY_FIELDS)
    )
    _require_report_fields(payload, label, ALL_LANES_SUMMARY_FIELDS, errors)
    if type(payload.get("production_ready")) is not bool:
        errors.append(f"{label} production_ready must be true or false")
    errors.extend(
        _integer_list_field_errors(
            label,
            payload,
            "required_domains",
            allow_empty=False,
        )
    )
    errors.extend(
        _integer_list_field_errors(
            label,
            payload,
            "supported_launch_domains",
            allow_empty=False,
        )
    )
    errors.extend(
        _integer_list_field_errors(
            label,
            payload,
            "unsupported_launch_domains",
            allow_empty=True,
        )
    )
    errors.extend(_string_list_field_errors(label, payload, "blockers", allow_empty=True))
    blockers = payload.get("blockers")
    if (
        payload.get("production_ready") is True
        and isinstance(blockers, list)
        and blockers
    ):
        errors.append(f"{label} blockers must be empty when production_ready is true")

    checklist = _require_report_mapping(
        payload.get("release_checklist"),
        f"{label}.release_checklist",
        errors,
    )
    if checklist:
        errors.extend(
            _release_checklist_bundle_errors(
                checklist,
                f"{label}.release_checklist",
                require_ready=False,
            )
        )

    lanes = _require_report_list(payload.get("lanes"), f"{label}.lanes", errors)
    for index, lane in enumerate(lanes):
        lane_label = f"{label}.lanes[{index}]"
        lane_payload = _require_report_mapping(lane, lane_label, errors)
        if not isinstance(lane, dict):
            continue
        errors.extend(
            _unknown_report_field_errors(lane_payload, lane_label, ALL_LANES_LANE_FIELDS)
        )
        _require_report_fields(lane_payload, lane_label, ALL_LANES_LANE_FIELDS, errors)
        if "domain" in lane_payload and type(lane_payload.get("domain")) is not int:
            errors.append(f"{lane_label} domain must be an integer")
        if "chain" in lane_payload:
            chain = lane_payload.get("chain")
            if not isinstance(chain, str) or not chain or chain.strip() != chain:
                errors.append(
                    f"{lane_label} chain must be a non-empty string "
                    "with no surrounding whitespace"
                )
        if "production_ready" in lane_payload and type(
            lane_payload.get("production_ready")
        ) is not bool:
            errors.append(f"{lane_label} production_ready must be true or false")
        errors.extend(
            _string_list_field_errors(
                lane_label,
                lane_payload,
                "blockers",
                allow_empty=True,
            )
        )
        lane_blockers = lane_payload.get("blockers")
        if (
            lane_payload.get("production_ready") is True
            and isinstance(lane_blockers, list)
            and lane_blockers
        ):
            errors.append(
                f"{lane_label} blockers must be empty when production_ready is true"
            )

        records = _require_report_mapping(
            lane_payload.get("records"),
            f"{lane_label}.records",
            errors,
        )
        if records:
            errors.extend(
                _unknown_report_field_errors(
                    records,
                    f"{lane_label}.records",
                    ALL_LANES_RECORD_FIELDS,
                )
            )
            _require_report_fields(
                records,
                f"{lane_label}.records",
                ALL_LANES_RECORD_FIELDS,
                errors,
            )
            for field in ALL_LANES_RECORD_FIELDS:
                if field in records and type(records.get(field)) is not bool:
                    errors.append(
                        f"{lane_label}.records {field} must be true or false"
                    )

        errors.extend(_all_lanes_nested_bundle_errors(lane_payload, lane_label))
    errors.extend(_all_lanes_route_canary_cross_lane_bundle_errors(label, lanes))
    return errors


def _all_lanes_summary_output_bundle_errors(
    summary: Any,
    report: dict[str, Any],
    label: str,
) -> list[str]:
    errors = _all_lanes_summary_bundle_errors(summary, label)
    if errors:
        return errors
    if not isinstance(summary, dict):
        return errors
    report_evidence = report.get("evidence")
    if not isinstance(report_evidence, dict):
        errors.append(
            f"{label} cannot be compared: bundled report evidence is not an object"
        )
    elif summary != report_evidence:
        errors.append(f"{label} does not match bundled report evidence")
    return errors


def _release_report_bundle_errors(
    report: Any,
    *,
    label: str,
    bundle_dir: Path | None = None,
) -> list[str]:
    errors = _release_report_preflight_errors(report, label=label)
    payload = report if isinstance(report, dict) else {}
    if not isinstance(payload, dict):
        return errors

    errors.extend(
        _unknown_report_field_errors(payload, label, READINESS_REPORT_ROOT_FIELDS)
    )
    _require_report_fields(payload, label, READINESS_REPORT_BUNDLE_FIELDS, errors)

    _require_report_list(payload.get("inputs"), f"{label}.inputs", errors)
    for index, artifact in enumerate(
        _require_report_list(payload.get("input_artifacts"), f"{label}.input_artifacts", errors)
    ):
        errors.extend(_artifact_row_errors(artifact, f"{label}.input_artifacts[{index}]"))
    errors.extend(_copied_input_provenance_bundle_errors(payload, label))

    checklist = _require_report_mapping(
        payload.get("release_checklist"),
        f"{label}.release_checklist",
        errors,
    )
    if checklist:
        checklist_errors = _release_checklist_bundle_errors(
            checklist,
            f"{label}.release_checklist",
            require_ready=True,
        )
        errors.extend(checklist_errors)
    else:
        checklist_errors = []

    corridor = _require_report_mapping(payload.get("corridor"), f"{label}.corridor", errors)
    if corridor:
        errors.extend(
            _unknown_report_field_errors(
                corridor,
                f"{label}.corridor",
                CORRIDOR_FIELDS,
            )
        )
        _require_report_fields(
            corridor,
            f"{label}.corridor",
            CORRIDOR_FIELDS,
            errors,
        )
        if type(corridor.get("production_ready")) is not bool:
            errors.append(f"{label}.corridor production_ready must be true or false")
        elif corridor.get("production_ready") is not True:
            errors.append(f"{label} production corridor is not ready")
        if type(corridor.get("require_phase_evidence")) is not bool:
            errors.append(
                f"{label}.corridor require_phase_evidence must be true or false"
            )
        elif corridor.get("require_phase_evidence") is not True:
            errors.append(f"{label} does not require hashed phase evidence")
        errors.extend(
            _string_list_field_errors(
                f"{label}.corridor",
                corridor,
                "blockers",
                allow_empty=True,
            )
        )
        corridor_blockers = corridor.get("blockers")
        if (
            corridor.get("production_ready") is True
            and isinstance(corridor_blockers, list)
            and corridor_blockers
        ):
            errors.append(
                f"{label}.corridor blockers must be empty when production_ready is true"
            )
        phases = _require_report_mapping(
            corridor.get("phases"), f"{label}.corridor.phases", errors
        )
        known_corridor_phases = _corridor_phase_names()
        for phase in sorted(phases, key=str):
            phase_error = _corridor_phase_key_error(
                phase,
                f"{label}.corridor.phases",
            )
            if phase_error is not None:
                errors.append(phase_error)
            elif phase not in known_corridor_phases:
                errors.append(f"{label}.corridor has unknown phase status: {phase}")
            status = phases.get(phase)
            if status not in {"passed", "blocked"}:
                if phase_error is None:
                    errors.append(
                        f"{label}.corridor phase {phase} status must be passed or blocked"
                    )
                else:
                    errors.append(
                        f"{label}.corridor phase status must be passed or blocked"
                    )
            elif corridor.get("production_ready") is True and status != "passed":
                if phase_error is None:
                    errors.append(
                        f"{label}.corridor phase {phase} is not passed: {status!r}"
                    )
                else:
                    errors.append(
                        f"{label}.corridor phase is not passed: {status!r}"
                    )
        evidence_artifacts = _require_report_mapping(
            corridor.get("evidence_artifacts"),
            f"{label}.corridor.evidence_artifacts",
            errors,
        )
        for phase, artifact in evidence_artifacts.items():
            phase_error = _corridor_phase_key_error(
                phase,
                f"{label}.corridor.evidence_artifacts",
            )
            if phase_error is not None:
                errors.append(phase_error)
                artifact_label = f"{label}.corridor.evidence_artifacts[malformed phase]"
            else:
                artifact_label = f"{label}.corridor.evidence_artifacts[{phase!r}]"
                if phase not in known_corridor_phases:
                    errors.append(
                        f"{label}.corridor has evidence artifact for unknown phase: {phase}"
                    )
            if artifact is not None:
                errors.extend(
                    _artifact_row_errors(
                        artifact,
                        artifact_label,
                    )
                )
        if corridor.get("require_phase_evidence") is True:
            for phase, status in phases.items():
                if status != "passed":
                    continue
                phase_error = _corridor_phase_key_error(
                    phase,
                    f"{label}.corridor.phases",
                )
                if phase_error is not None:
                    continue
                artifact = evidence_artifacts.get(phase)
                if not isinstance(artifact, dict):
                    errors.append(
                        f"{label}.corridor phase {phase} has no hashed evidence artifact"
                    )
        errors.extend(
            _corridor_phase_transcript_bundle_errors(
                corridor,
                bundle_dir,
                label,
            )
        )

    crypto_rows = _require_report_list(
        payload.get("cryptographic_evidence"),
        f"{label}.cryptographic_evidence",
        errors,
    )
    for index, row in enumerate(crypto_rows):
        errors.extend(
            _cryptographic_evidence_row_bundle_errors(
                row,
                f"{label}.cryptographic_evidence[{index}]",
            )
        )

    submission_surfaces = _require_report_list(
        payload.get("user_prover_submission_surfaces"),
        f"{label}.user_prover_submission_surfaces",
        errors,
    )
    for index, surface in enumerate(submission_surfaces):
        errors.extend(
            _submission_surface_row_bundle_errors(
                surface,
                f"{label}.user_prover_submission_surfaces[{index}]",
            )
        )
    errors.extend(
        _submission_surface_binding_bundle_errors(
            submission_surfaces,
            payload,
            label,
        )
    )

    native_summary = payload.get("native_evm_prover_bundle")
    native_summary_errors = _native_evm_prover_summary_errors(
        native_summary,
        f"{label}.native_evm_prover_bundle",
    )
    errors.extend(native_summary_errors)
    if not native_summary_errors and isinstance(native_summary, dict):
        errors.extend(
            _native_evm_prover_binding_bundle_errors(
                native_summary,
                payload,
                bundle_dir,
                label,
            )
        )
    source_inventory = _require_report_mapping(
        payload.get("source_inventory"),
        f"{label}.source_inventory",
        errors,
    )
    source_inventory_label = f"{label}.source_inventory"
    if source_inventory:
        known_source_inventory_gates = _source_inventory_known_gates()
        for gate in sorted(source_inventory, key=str):
            gate_error = _source_inventory_gate_key_error(gate, source_inventory_label)
            if gate_error is not None:
                errors.append(gate_error)
            elif gate not in known_source_inventory_gates:
                errors.append(f"{source_inventory_label} contains unknown gate: {gate}")
            inventory = source_inventory[gate]
            inventory_label = f"{source_inventory_label}[{gate!r}]"
            inventory_payload = _require_report_mapping(inventory, inventory_label, errors)
            if not isinstance(inventory, dict):
                continue
            errors.extend(
                _unknown_report_field_errors(
                    inventory_payload,
                    inventory_label,
                    SOURCE_INVENTORY_FIELDS,
                )
            )
            _require_report_fields(
                inventory_payload,
                inventory_label,
                SOURCE_INVENTORY_FIELDS,
                errors,
            )
            validation_status = inventory_payload.get("validation_status")
            if validation_status not in {"passed", "blocked"}:
                errors.append(
                    f"{inventory_label} validation_status must be passed or blocked"
                )
            elif validation_status != "passed":
                errors.append(f"{inventory_label} validation_status must be passed")
            errors.extend(
                _string_list_field_errors(
                    inventory_label,
                    inventory_payload,
                    "validation_blockers",
                    allow_empty=True,
                )
            )
            validation_blockers = inventory_payload.get("validation_blockers")
            if isinstance(validation_blockers, list) and validation_blockers:
                errors.append(f"{inventory_label} validation_blockers must be empty")

    evidence_summary = payload.get("evidence")
    evidence_summary_errors = _all_lanes_summary_bundle_errors(
        evidence_summary,
        f"{label}.evidence",
    )
    errors.extend(evidence_summary_errors)
    if not evidence_summary_errors and isinstance(evidence_summary, dict):
        errors.extend(
            _copied_evidence_binding_bundle_errors(
                evidence_summary,
                payload,
                bundle_dir,
                label,
            )
        )
    if isinstance(evidence_summary, dict):
        errors.extend(
            _cryptographic_evidence_lane_binding_bundle_errors(
                crypto_rows,
                evidence_summary.get("lanes"),
                crypto_label=f"{label}.cryptographic_evidence",
                lanes_label=f"{label}.evidence.lanes",
            )
        )
    if (
        not checklist_errors
        and isinstance(checklist, dict)
        and not evidence_summary_errors
        and isinstance(evidence_summary, dict)
        and not native_summary_errors
    ):
        errors.extend(
            _release_checklist_binding_bundle_errors(
                checklist,
                payload,
                label,
            )
        )
    errors.extend(
        _release_report_artifact_integrity_bundle_errors(
            payload,
            bundle_dir,
            label,
        )
    )
    return errors


def _reject_malformed_release_report(errors: list[str]) -> None:
    if errors:
        details = "\n".join(f"- {error}" for error in errors)
        raise ValueError("malformed SCCP release readiness report:\n" + details)


def _release_notes_attachment_bundle_errors(
    report: dict[str, Any],
    artifacts: list[Any],
    notes: Any,
    label: str,
) -> list[str]:
    if not label.startswith("bundled report"):
        return []
    if not isinstance(notes, str):
        return [f"{label}.release_notes_attachment must be UTF-8 text"]
    verifier = _verify_module()
    errors = verifier._release_notes_attachment_invariant_errors(
        report,
        artifacts,
        notes,
    )
    try:
        expected_notes = verifier._expected_release_notes_attachment(report, artifacts)
    except Exception as exc:
        errors.append(f"{label}.release_notes_attachment cannot be rendered: {exc}")
    else:
        if notes != expected_notes:
            errors.append("release notes attachment does not match manifest and report")
    return errors


def _release_notes_attachment(
    report: dict[str, Any],
    artifacts: list[dict[str, Any]],
) -> str:
    status = "READY" if report["production_ready"] is True else "NOT READY"
    lines = [
        "# SCCP Public Release Notes Attachment",
        "",
        f"Status: {status}",
        "",
        (
            "Attach `manifest.json` plus every artifact below to the public release "
            "notes before production activation."
        ),
        "",
        (
            "`manifest.json` is the verifier root and is intentionally not listed "
            "in its own artifact table."
        ),
        "",
        "| Artifact | Bytes | SHA-256 |",
        "| --- | ---: | --- |",
    ]
    for artifact in artifacts:
        lines.append(
            "| `{path}` | {bytes} | `{sha256}` |".format(
                path=artifact["path"],
                bytes=artifact["bytes"],
                sha256=artifact["sha256"],
            )
        )
    blocker_lines = _markdown_string_list_items(
        report.get("blockers"),
        field_label="blockers",
    )
    if blocker_lines:
        lines.extend(["", "## Blocking Items", ""])
        lines.extend(blocker_lines)
    return "\n".join(lines) + "\n"


def _bundle_artifacts(output_dir: Path, paths: list[Path]) -> list[dict[str, Any]]:
    return [_artifact(path, output_dir) for path in paths]


def _release_bundle_manifest_errors(
    manifest: Any,
    output_dir: Path,
    report: dict[str, Any],
    summary: dict[str, Any],
) -> list[str]:
    verifier = _verify_module()
    if not isinstance(manifest, dict):
        return ["manifest is not a JSON object"]

    errors: list[str] = []
    for key in sorted(set(manifest) - verifier.MANIFEST_KEYS, key=str):
        errors.append(
            verifier._native_evm_prover_field_name_blocker(
                "manifest",
                key,
                "unknown top-level",
            )
        )
    for key in sorted(verifier.MANIFEST_KEYS - set(manifest)):
        errors.append(f"manifest missing top-level field: {key}")
    if manifest.get("schema") != verifier.SCHEMA:
        errors.append(f"unexpected manifest schema: {manifest.get('schema')}")
    errors.extend(verifier._boolean_field_errors("manifest", manifest, "production_ready"))
    errors.extend(
        verifier._boolean_field_errors("manifest", manifest, "release_checklist_ready")
    )
    errors.extend(verifier._boolean_field_errors("manifest", manifest, "corridor_ready"))
    errors.extend(
        verifier._string_list_field_errors(
            "manifest",
            manifest,
            "blockers",
            allow_empty=True,
        )
    )

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append("manifest artifacts must be a non-empty list")
        artifacts = []
    manifest_artifacts = verifier._manifest_artifacts_by_path(artifacts, errors)
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            errors.append("manifest artifact entry is not an object")
            continue
        errors.extend(verifier._artifact_errors(output_dir, artifact))

    expected_paths = set(manifest_artifacts)
    bundle_paths, bundle_directories = verifier._bundle_entry_paths(output_dir, errors)
    for unexpected in sorted(bundle_paths - expected_paths):
        errors.append(f"bundle contains unmanifested artifact: {unexpected}")
    for missing in sorted(expected_paths - bundle_paths):
        errors.append(f"bundle is missing expected artifact file: {missing}")
    expected_directories = verifier._expected_bundle_directories(expected_paths)
    for unexpected in sorted(bundle_directories - expected_directories):
        errors.append(f"bundle contains unmanifested directory: {unexpected}")
    for required_path in verifier.REQUIRED_ARTIFACT_PATHS:
        if required_path not in manifest_artifacts:
            errors.append(f"manifest missing required artifact: {required_path}")

    referenced_paths = verifier._referenced_report_artifact_paths(report)
    for unexpected in sorted(set(manifest_artifacts) - referenced_paths):
        errors.append(
            "manifest contains artifact not referenced by readiness report: "
            f"{unexpected}"
        )
    for missing in sorted(referenced_paths - set(manifest_artifacts)):
        errors.append(f"manifest missing readiness report referenced artifact: {missing}")
    try:
        expected_order = verifier._expected_manifest_artifact_order(report)
    except Exception as exc:
        errors.append(f"cannot compute canonical manifest artifact order: {exc}")
    else:
        if verifier._manifest_artifact_paths_in_order(artifacts) != expected_order:
            errors.append(
                "manifest artifact order does not match canonical release bundle order"
            )

    if manifest.get("production_ready") is not True:
        errors.append("manifest production_ready is not true")
    if manifest.get("release_checklist_ready") is not True:
        errors.append("manifest release_checklist_ready is not true")
    if manifest.get("corridor_ready") is not True:
        errors.append("manifest corridor_ready is not true")
    if manifest.get("blockers"):
        errors.append("manifest contains blockers")
    if manifest.get("production_ready") != report.get("production_ready"):
        errors.append("manifest production_ready does not match readiness report")
    if manifest.get("blockers") != report.get("blockers"):
        errors.append("manifest blockers do not match readiness report blockers")
    release_checklist = report.get("release_checklist")
    if isinstance(release_checklist, dict) and manifest.get(
        "release_checklist_ready"
    ) != release_checklist.get("ready"):
        errors.append(
            "manifest release_checklist_ready does not match "
            "readiness report release_checklist"
        )
    corridor = report.get("corridor")
    if isinstance(corridor, dict) and manifest.get("corridor_ready") != corridor.get(
        "production_ready"
    ):
        errors.append("manifest corridor_ready does not match readiness report corridor")
    summary_native_bundle = report.get("native_evm_prover_bundle")
    if not isinstance(summary_native_bundle, dict):
        summary_native_bundle = verifier._missing_native_evm_prover_bundle_status()
    summary_launch_checklist = verifier._active_launch_release_checklist(
        summary,
        summary_native_bundle,
    )
    summary_launch_ready = summary_launch_checklist.get("ready")
    if manifest.get("production_ready") != summary_launch_ready:
        errors.append(
            "manifest production_ready does not match all-lanes summary active "
            f"{verifier.ACTIVE_LAUNCH_DISPLAY} launch readiness"
        )
    if isinstance(summary.get("release_checklist"), dict) and manifest.get(
        "release_checklist_ready"
    ) != summary_launch_checklist.get("ready"):
        errors.append(
            "manifest release_checklist_ready does not match all-lanes summary "
            f"active {verifier.ACTIVE_LAUNCH_DISPLAY} release checklist"
        )
    return errors


def _release_bundle_manifest(
    output_dir: Path,
    report: dict[str, Any],
    artifact_paths: list[Path],
) -> dict[str, Any]:
    """Build the release manifest without truthy-coercing readiness fields."""

    return {
        "schema": "sccp-release-bundle-v1",
        "production_ready": report["production_ready"],
        "release_checklist_ready": report["release_checklist"]["ready"],
        "corridor_ready": report["corridor"]["production_ready"],
        "blockers": report["blockers"],
        "artifacts": _bundle_artifacts(output_dir, artifact_paths),
    }


def _verify_generated_bundle(output_dir: Path) -> dict[str, Any]:
    summary = _verify_module().verify_bundle(output_dir)
    if summary["verified"]:
        return summary
    errors = "\n".join(f"- {error}" for error in summary["errors"])
    raise RuntimeError(
        "generated SCCP release bundle failed strict verification:\n" + errors
    )


def _relative_to_bundle(output_dir: Path, path: Path) -> Path:
    return path.relative_to(output_dir)


def _build_bundle_report(
    report_module: Any,
    output_dir: Path,
    evidence_paths: list[Path],
    phase_results: list[str],
    phase_evidence_args: list[str],
    native_evm_prover_bundle: Path | None,
) -> dict[str, Any]:
    relative_evidence = [
        _relative_to_bundle(output_dir, path)
        for path in evidence_paths
    ]
    relative_phase_evidence: list[str] = []
    for raw in phase_evidence_args:
        phase, path_text = raw.split("=", 1)
        relative_phase_evidence.append(
            f"{phase}={_relative_to_bundle(output_dir, Path(path_text))}"
        )

    original_cwd = Path.cwd()
    try:
        os.chdir(output_dir)
        relative_native_bundle = (
            _relative_to_bundle(output_dir, native_evm_prover_bundle)
            if native_evm_prover_bundle is not None
            else None
        )
        return report_module._build_report(
            relative_evidence,
            phase_results,
            relative_phase_evidence,
            require_phase_evidence=True,
            native_evm_prover_bundle=relative_native_bundle,
        )
    finally:
        os.chdir(original_cwd)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Build a self-contained SCCP release-note attachment bundle with "
            "strict readiness reports, all-lanes summary JSON, copied evidence "
            "inputs, and hashed corridor logs."
        )
    )
    parser.add_argument(
        "toml",
        nargs="+",
        type=Path,
        help="TOML evidence snippet or full config containing [zk] SCCP records.",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        type=Path,
        help="Directory to create for the release bundle.",
    )
    parser.add_argument(
        "--phase-result",
        action="append",
        default=[],
        metavar="PHASE=STATUS",
        help="Production-corridor phase status; repeat or use all=passed.",
    )
    parser.add_argument(
        "--phase-evidence",
        action="append",
        default=[],
        metavar="PHASE=PATH",
        help="Production-corridor evidence log; repeat or use all=PATH.",
    )
    parser.add_argument(
        "--phase-evidence-dir",
        type=Path,
        help=(
            "Directory containing <phase>.log files, dist/sccp-production-corridor/"
            "<phase>.log files, or downloaded sccp-production-corridor-<phase>/"
            "<phase>.log artifact directories."
        ),
    )
    parser.add_argument(
        "--native-evm-prover-bundle",
        type=Path,
        help=(
            "Audited Ethereum mainnet no-WASM native EVM Groth16 prover bundle "
            "manifest to copy, hash, and validate."
        ),
    )
    parser.add_argument(
        "--allow-not-ready",
        action="store_true",
        help="Write a blocked bundle for review instead of failing on readiness blockers.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Replace an existing output directory.",
    )
    return parser


def _prepare_output_dir(path: Path, *, force: bool) -> None:
    if path.exists():
        if not force:
            raise FileExistsError(f"output directory already exists: {path}")
        shutil.rmtree(path)
    path.mkdir(parents=True)


def _path_contains(parent: Path, child: Path) -> bool:
    try:
        child.relative_to(parent)
    except ValueError:
        return False
    return True


def _reject_path_control_characters(path: Path, label: str) -> None:
    path_text = str(path)
    control_character = _path_control_character(path_text)
    if control_character is not None:
        raise ValueError(
            f"{label} contains control character {control_character}: "
            f"{path_text!r}"
        )


def _reject_path_markdown_unsafe_characters(path_text: str, label: str) -> None:
    markdown_unsafe_character = _path_markdown_unsafe_character(path_text)
    if markdown_unsafe_character is not None:
        raise ValueError(
            f"{label} contains Markdown-unsafe character "
            f"{markdown_unsafe_character}: {path_text!r}"
        )


def _reject_symlink_sources(paths: list[Path]) -> None:
    for path in paths:
        _reject_path_control_characters(path, "release bundle source path")
        _reject_path_markdown_unsafe_characters(
            path.name,
            "release bundle source filename",
        )
        if path.is_symlink():
            raise ValueError(
                f"release bundle source path must not be a symlink: {path}"
            )
        current = Path(path.anchor) if path.is_absolute() else Path(".")
        parts = path.parts[1:] if path.is_absolute() else path.parts
        for part in parts:
            current = current / part
            try:
                mode = current.lstat().st_mode
            except FileNotFoundError:
                break
            if stat.S_ISLNK(mode):
                if path.is_absolute() and current.parent == Path(path.anchor):
                    continue
                raise ValueError(
                    "release bundle source path ancestor must not be a symlink: "
                    f"{current}"
                )


def _evidence_input_identity(path: Path) -> tuple[object, ...]:
    try:
        status = path.stat()
    except FileNotFoundError:
        return ("path", path.resolve())
    return ("file", status.st_dev, status.st_ino)


def _reject_duplicate_evidence_inputs(paths: list[Path]) -> None:
    seen: dict[tuple[object, ...], Path] = {}
    for path in paths:
        identity = _evidence_input_identity(path)
        previous = seen.get(identity)
        if previous is not None:
            raise ValueError(
                "release bundle evidence input path is duplicated: "
                f"{path} duplicates {previous}"
            )
        seen[identity] = path


def _reject_symlinked_existing_output_path(path: Path) -> None:
    current = Path(path.anchor) if path.is_absolute() else Path(".")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    for part in parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            return
        if stat.S_ISLNK(mode):
            if path.is_absolute() and current.parent == Path(path.anchor):
                continue
            if current == path:
                raise ValueError(
                    f"release bundle output directory must not be a symlink: {current}"
                )
            raise ValueError(
                "release bundle output directory ancestor must not be a symlink: "
                f"{current}"
            )


def _validate_output_dir(
    output_dir: Path,
    *,
    input_paths: list[Path],
    phase_sources: dict[str, Path],
    native_evm_prover_bundle: Path | None,
    force: bool,
) -> None:
    _reject_path_control_characters(output_dir, "release bundle output directory")
    resolved_output = output_dir.resolve()
    forbidden_outputs = {
        Path("/").resolve(),
        Path.home().resolve(),
        ROOT.resolve(),
        Path.cwd().resolve(),
    }
    if resolved_output in forbidden_outputs:
        raise ValueError(f"refusing dangerous output directory: {output_dir}")
    if _path_contains(resolved_output, ROOT.resolve()):
        raise ValueError(
            f"refusing output directory that contains the repository root: {output_dir}"
        )
    _reject_symlinked_existing_output_path(output_dir)
    protected_paths = [*input_paths, *phase_sources.values()]
    if native_evm_prover_bundle is not None:
        protected_paths.append(native_evm_prover_bundle)
    _reject_symlink_sources(protected_paths)
    native_payload_sources = (
        _native_evm_prover_payload_sources(native_evm_prover_bundle)
        if native_evm_prover_bundle is not None
        else []
    )
    _reject_symlink_sources([source for _, source in native_payload_sources])
    if not force:
        return

    if native_evm_prover_bundle is not None:
        protected_paths.extend(source for _, source in native_payload_sources)
    for protected_path in protected_paths:
        resolved_protected = protected_path.resolve()
        if _path_contains(resolved_output, resolved_protected):
            raise ValueError(
                "refusing --force output directory that contains input evidence: "
                f"{output_dir} contains {protected_path}"
            )


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    report_module = _report_module()

    try:
        phases = report_module._corridor_phases()
        phase_sources = _phase_evidence_sources(
            phases,
            args.phase_evidence,
            args.phase_evidence_dir,
        )
        _validate_output_dir(
            args.output_dir,
            input_paths=args.toml,
            phase_sources=phase_sources,
            native_evm_prover_bundle=args.native_evm_prover_bundle,
            force=args.force,
        )
        _reject_duplicate_evidence_inputs(args.toml)
        preflight_report = report_module._build_report(
            args.toml,
            args.phase_result,
            _phase_evidence_args(phase_sources),
            require_phase_evidence=True,
            native_evm_prover_bundle=args.native_evm_prover_bundle,
        )
        _reject_malformed_release_report(
            _release_report_preflight_errors(preflight_report, label="preflight report")
        )
        if (
            preflight_report["production_ready"] is not True
            and not args.allow_not_ready
        ):
            blockers = "\n".join(
                _markdown_string_list_items(
                    preflight_report.get("blockers"),
                    field_label="blockers",
                )
            )
            parser.exit(1, f"SCCP release bundle is not production ready:\n{blockers}\n")
        _reject_malformed_release_report(
            _release_report_bundle_errors(preflight_report, label="preflight report")
        )

        _prepare_output_dir(args.output_dir, force=args.force)
        copied_evidence = _copy_evidence_inputs(args.toml, args.output_dir)
        copied_phase_args, copied_phase_logs = _copy_phase_evidence(
            phases,
            phase_sources,
            args.output_dir,
        )
        (
            copied_native_evm_prover_bundle,
            copied_native_evm_prover_payloads,
        ) = _copy_native_evm_prover_bundle(
            args.native_evm_prover_bundle,
            args.output_dir,
        )
        report = _build_bundle_report(
            report_module,
            args.output_dir,
            copied_evidence,
            args.phase_result,
            copied_phase_args,
            copied_native_evm_prover_bundle,
        )
        _reject_malformed_release_report(
            _release_report_bundle_errors(
                report,
                label="bundled report",
                bundle_dir=args.output_dir,
            )
        )
        summary = _all_lanes_summary(copied_evidence)
        _reject_malformed_release_report(
            _all_lanes_summary_output_bundle_errors(
                summary,
                report,
                "bundled summary",
            )
        )

        report_md = args.output_dir / "sccp-release-readiness.md"
        report_json = args.output_dir / "sccp-release-readiness.json"
        summary_json = args.output_dir / "sccp-all-lanes-summary.json"
        notes_md = args.output_dir / "sccp-release-notes-attachment.md"
        manifest_json = args.output_dir / "manifest.json"

        report_markdown = report_module._render_markdown(
            report,
            max_blockers_per_lane=4,
        )
        _reject_malformed_release_report(
            _readiness_markdown_bundle_errors(
                report,
                report_markdown,
                "bundled report",
            )
        )
        report_md.write_text(report_markdown, encoding="utf-8")
        _write_json(report_json, report)
        _write_json(summary_json, summary)

        attachment_paths = [
            report_md,
            report_json,
            summary_json,
            *copied_evidence,
            *(
                [copied_native_evm_prover_bundle]
                if copied_native_evm_prover_bundle is not None
                else []
            ),
            *copied_native_evm_prover_payloads,
            *copied_phase_logs,
        ]
        attachment_artifacts = _bundle_artifacts(args.output_dir, attachment_paths)
        release_notes = _release_notes_attachment(report, attachment_artifacts)
        _reject_malformed_release_report(
            _release_notes_attachment_bundle_errors(
                report,
                attachment_artifacts,
                release_notes,
                "bundled report",
            )
        )
        notes_md.write_text(release_notes, encoding="utf-8")
        all_artifact_paths = [*attachment_paths, notes_md]
        manifest = _release_bundle_manifest(args.output_dir, report, all_artifact_paths)
        _reject_malformed_release_report(
            _release_bundle_manifest_errors(
                manifest,
                args.output_dir,
                report,
                summary,
            )
        )
        _write_json(manifest_json, manifest)
        verification_summary: dict[str, Any] | None = None
        if report["production_ready"] is True:
            verification_summary = _verify_generated_bundle(args.output_dir)
    except (
        OSError,
        RuntimeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        parser.exit(2, f"{parser.prog}: error: {exc}\n")

    print(f"Wrote SCCP release bundle to {args.output_dir}")
    if report["production_ready"] is True:
        print(f"Verified SCCP release bundle at {args.output_dir}")
        if verification_summary is not None:
            print(
                "SCCP release bundle manifest_sha256: "
                f"{verification_summary['manifest_sha256']}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
