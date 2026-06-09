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
        raise ValueError(
            "native EVM Groth16 prover bundle JSON contains duplicate key: "
            f"{exc.key}"
        ) from exc
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
    "sdk_helpers",
    "on_chain_submission",
    "required_phases",
    "validation_status",
)


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


def _release_report_preflight_errors(report: Any, *, label: str) -> list[str]:
    errors: list[str] = []
    payload = _require_report_mapping(report, label, errors)
    if errors:
        return errors
    _require_report_fields(payload, label, ("production_ready", "blockers"), errors)
    return errors


def _artifact_row_errors(row: Any, label: str) -> list[str]:
    errors: list[str] = []
    artifact = _require_report_mapping(row, label, errors)
    if not errors:
        _require_report_fields(artifact, label, ("path", "bytes", "sha256"), errors)
    return errors


def _release_report_bundle_errors(report: Any, *, label: str) -> list[str]:
    errors = _release_report_preflight_errors(report, label=label)
    payload = report if isinstance(report, dict) else {}
    if not isinstance(payload, dict):
        return errors

    _require_report_fields(
        payload,
        label,
        (
            "input_artifacts",
            "release_checklist",
            "corridor",
            "cryptographic_evidence",
            "user_prover_submission_surfaces",
            "native_evm_prover_bundle",
            "source_inventory",
            "evidence",
        ),
        errors,
    )

    for index, artifact in enumerate(
        _require_report_list(payload.get("input_artifacts"), f"{label}.input_artifacts", errors)
    ):
        errors.extend(_artifact_row_errors(artifact, f"{label}.input_artifacts[{index}]"))

    checklist = _require_report_mapping(
        payload.get("release_checklist"),
        f"{label}.release_checklist",
        errors,
    )
    if checklist:
        _require_report_fields(
            checklist,
            f"{label}.release_checklist",
            ("ready", "items"),
            errors,
        )
        for index, item in enumerate(
            _require_report_list(
                checklist.get("items"),
                f"{label}.release_checklist.items",
                errors,
            )
        ):
            item_payload = _require_report_mapping(
                item,
                f"{label}.release_checklist.items[{index}]",
                errors,
            )
            if item_payload:
                _require_report_fields(
                    item_payload,
                    f"{label}.release_checklist.items[{index}]",
                    ("id", "ready"),
                    errors,
                )

    corridor = _require_report_mapping(payload.get("corridor"), f"{label}.corridor", errors)
    if corridor:
        _require_report_fields(
            corridor,
            f"{label}.corridor",
            ("production_ready", "phases", "evidence_artifacts"),
            errors,
        )
        _require_report_mapping(corridor.get("phases"), f"{label}.corridor.phases", errors)
        evidence_artifacts = _require_report_mapping(
            corridor.get("evidence_artifacts"),
            f"{label}.corridor.evidence_artifacts",
            errors,
        )
        for phase, artifact in evidence_artifacts.items():
            if artifact is not None:
                errors.extend(
                    _artifact_row_errors(
                        artifact,
                        f"{label}.corridor.evidence_artifacts[{phase!r}]",
                    )
                )

    for index, row in enumerate(
        _require_report_list(
            payload.get("cryptographic_evidence"),
            f"{label}.cryptographic_evidence",
            errors,
        )
    ):
        row_payload = _require_report_mapping(
            row,
            f"{label}.cryptographic_evidence[{index}]",
            errors,
        )
        if row_payload:
            _require_report_fields(
                row_payload,
                f"{label}.cryptographic_evidence[{index}]",
                CRYPTOGRAPHIC_EVIDENCE_ROW_FIELDS,
                errors,
            )

    for index, surface in enumerate(
        _require_report_list(
            payload.get("user_prover_submission_surfaces"),
            f"{label}.user_prover_submission_surfaces",
            errors,
        )
    ):
        surface_payload = _require_report_mapping(
            surface,
            f"{label}.user_prover_submission_surfaces[{index}]",
            errors,
        )
        if surface_payload:
            _require_report_fields(
                surface_payload,
                f"{label}.user_prover_submission_surfaces[{index}]",
                USER_PROVER_SUBMISSION_SURFACE_FIELDS,
                errors,
            )
            _require_report_list(
                surface_payload.get("required_phases"),
                f"{label}.user_prover_submission_surfaces[{index}].required_phases",
                errors,
            )

    _require_report_mapping(
        payload.get("native_evm_prover_bundle"),
        f"{label}.native_evm_prover_bundle",
        errors,
    )
    source_inventory = _require_report_mapping(
        payload.get("source_inventory"),
        f"{label}.source_inventory",
        errors,
    )
    for gate, inventory in source_inventory.items():
        _require_report_mapping(inventory, f"{label}.source_inventory[{gate!r}]", errors)

    evidence = _require_report_mapping(payload.get("evidence"), f"{label}.evidence", errors)
    if evidence:
        for index, lane in enumerate(
            _require_report_list(evidence.get("lanes"), f"{label}.evidence.lanes", errors)
        ):
            lane_payload = _require_report_mapping(
                lane,
                f"{label}.evidence.lanes[{index}]",
                errors,
            )
            if lane_payload:
                _require_report_fields(
                    lane_payload,
                    f"{label}.evidence.lanes[{index}]",
                    ("domain", "chain", "production_ready", "records"),
                    errors,
                )
                _require_report_mapping(
                    lane_payload.get("records"),
                    f"{label}.evidence.lanes[{index}].records",
                    errors,
                )
    return errors


def _reject_malformed_release_report(errors: list[str]) -> None:
    if errors:
        details = "\n".join(f"- {error}" for error in errors)
        raise ValueError("malformed SCCP release readiness report:\n" + details)


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
            _release_report_bundle_errors(report, label="bundled report")
        )
        summary = _all_lanes_summary(copied_evidence)

        report_md = args.output_dir / "sccp-release-readiness.md"
        report_json = args.output_dir / "sccp-release-readiness.json"
        summary_json = args.output_dir / "sccp-all-lanes-summary.json"
        notes_md = args.output_dir / "sccp-release-notes-attachment.md"
        manifest_json = args.output_dir / "manifest.json"

        report_md.write_text(
            report_module._render_markdown(report, max_blockers_per_lane=4),
            encoding="utf-8",
        )
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
        notes_md.write_text(
            _release_notes_attachment(report, attachment_artifacts),
            encoding="utf-8",
        )
        all_artifact_paths = [*attachment_paths, notes_md]
        manifest = _release_bundle_manifest(args.output_dir, report, all_artifact_paths)
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
