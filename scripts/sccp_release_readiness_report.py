#!/usr/bin/env python3
"""Render SCCP release-readiness notes from evidence and validation results."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ALL_LANES_SCRIPT = ROOT / "scripts" / "sccp_all_lanes_evidence.py"
CORRIDOR_SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
CORRIDOR_COMPLETION_SENTINEL = "SCCP production corridor completed."
CORRIDOR_DRY_RUN_SENTINEL = "SCCP production corridor dry run completed."
USER_PROVER_SDK_PHASES = (
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
)
USER_PROVER_SUBMISSION_SURFACES: tuple[dict[str, Any], ...] = (
    {
        "lanes": "eth,bsc",
        "proof_backend": "evm-groth16-bn254-v1",
        "sdk_helpers": (
            "buildEvmSccpProofRequest, canonicalEvmSccpReceiptProofBytes, "
            "evmSccpReceiptProofHash, canonicalBscSccpReceiptProofBytes, "
            "bscSccpReceiptProofHash, EvmSccpProver, "
            "buildEvmSccpSubmission, buildEvmSccpBridgeProofSubmitPayload"
        ),
        "on_chain_submission": (
            "Torii bridge-proof submit payload with BN254 Groth16 "
            "proof_bytes_hex for the EVM verifier contract"
        ),
        "required_phases": (*USER_PROVER_SDK_PHASES, "contract-smoke"),
    },
    {
        "lanes": "tron",
        "proof_backend": "tron-groth16-bn254-v1",
        "sdk_helpers": (
            "buildTronSccpProofRequest, canonicalTronSccpReceiptProofBytes, "
            "canonicalTronSccpReceiptStateProofBytes, "
            "canonicalTronSccpTransactionSourceProofBytes, "
            "tronSccpTransactionSourceProofHash, TronSccpProver, "
            "buildTronSccpSubmission, buildTronSccpBridgeProofSubmitPayload"
        ),
        "on_chain_submission": (
            "Torii bridge-proof submit payload with BN254 Groth16 "
            "proof_bytes_hex for the TRON verifier contract"
        ),
        "required_phases": (*USER_PROVER_SDK_PHASES, "contract-smoke"),
    },
    {
        "lanes": "sol",
        "proof_backend": "solana-program-v1",
        "sdk_helpers": (
            "buildSolanaSccpProofRequest, "
            "buildSolanaSccpAccountsLtHashProofRequest, "
            "buildSolanaSccpFullLightClientAuditProofRequests, "
            "SolanaSccpSourceStateProver, SolanaSccpProver, "
            "buildSolanaSccpSubmission"
        ),
        "on_chain_submission": "Solana verifier-program instruction envelope",
        "required_phases": USER_PROVER_SDK_PHASES,
    },
    {
        "lanes": "ton",
        "proof_backend": "ton-contract-v1",
        "sdk_helpers": (
            "buildTonSccpProofRequest, buildTonShardStateProofRequest, "
            "buildTonSccpFullLightClientAuditProofRequests, "
            "TonSccpSourceStateProver, TonSccpProver, "
            "buildTonSccpSubmission"
        ),
        "on_chain_submission": "TON internal message body BOC",
        "required_phases": USER_PROVER_SDK_PHASES,
    },
    {
        "lanes": "substrate",
        "proof_backend": "substrate-runtime-v1",
        "sdk_helpers": (
            "buildSubstrateSccpProofRequest, "
            "buildSubstrateSccpRuntimeStorageProofRequest, "
            "SubstrateSccpProver, "
            "buildSubstrateSccpSubmission"
        ),
        "on_chain_submission": "Substrate runtime call envelope",
        "required_phases": USER_PROVER_SDK_PHASES,
    },
)


def _load_all_lanes_module() -> Any:
    spec = importlib.util.spec_from_file_location(
        "_sccp_all_lanes_evidence",
        ALL_LANES_SCRIPT,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {ALL_LANES_SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _corridor_phases() -> list[str]:
    completed = subprocess.run(
        ["bash", str(CORRIDOR_SCRIPT), "--list"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    phases = [
        line.strip()
        for line in completed.stdout.splitlines()
        if line.startswith("  ")
    ]
    if not phases:
        raise RuntimeError("SCCP production corridor runner did not list phases")
    return phases


def _normalize_phase_status(value: str) -> str:
    normalized = value.strip().lower()
    if normalized in {"pass", "passed", "ok", "success", "successful", "green"}:
        return "passed"
    if normalized in {"fail", "failed", "failure", "red"}:
        return "failed"
    if normalized in {"skip", "skipped"}:
        return "skipped"
    if normalized in {"missing", "unknown", "pending", "not-run", "not_run"}:
        return "missing"
    raise argparse.ArgumentTypeError(
        f"phase result status must be passed, failed, skipped, or missing: {value}"
    )


def _parse_phase_results(values: list[str], phases: list[str]) -> dict[str, str]:
    results = {phase: "missing" for phase in phases}
    for raw in values:
        if "=" not in raw:
            raise argparse.ArgumentTypeError(
                f"phase result must use NAME=STATUS syntax: {raw}"
            )
        name, status = raw.split("=", 1)
        name = name.strip()
        normalized = _normalize_phase_status(status)
        if name == "all":
            results = {phase: normalized for phase in phases}
            continue
        if name not in results:
            raise argparse.ArgumentTypeError(f"unknown SCCP corridor phase: {name}")
        results[name] = normalized
    return results


def _artifact(path: Path) -> dict[str, Any]:
    if path.is_symlink():
        raise ValueError(f"release artifact path must not be a symlink: {path}")
    payload = path.read_bytes()
    return {
        "path": str(path),
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


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


def _phase_transcript_errors(phase: str, artifact: dict[str, Any]) -> list[str]:
    path = Path(str(artifact["path"]))
    try:
        transcript = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return ["evidence artifact is not UTF-8 text"]
    phase_marker = f"==> SCCP production corridor: {phase}"
    errors: list[str] = []
    if CORRIDOR_DRY_RUN_SENTINEL in transcript:
        errors.append("evidence artifact is a dry-run transcript")
    if phase_marker not in transcript:
        errors.append("evidence artifact is missing the phase marker")
    if CORRIDOR_COMPLETION_SENTINEL not in transcript:
        errors.append("evidence artifact is missing the completion sentinel")
    return errors


def _parse_phase_evidence(
    values: list[str],
    phases: list[str],
    phase_status: dict[str, str],
    phase_evidence_dir: Path | None,
) -> dict[str, dict[str, Any]]:
    artifacts: dict[str, dict[str, Any]] = {}
    if phase_evidence_dir is not None:
        for phase in phases:
            if phase_status.get(phase) == "passed":
                artifacts[phase] = _artifact(
                    _phase_log_from_dir(phase_evidence_dir, phase)
                )
    for raw in values:
        if "=" not in raw:
            raise argparse.ArgumentTypeError(
                f"phase evidence must use NAME=PATH syntax: {raw}"
            )
        name, path_text = raw.split("=", 1)
        name = name.strip()
        if not path_text:
            raise argparse.ArgumentTypeError(
                f"phase evidence path must not be empty: {raw}"
            )
        artifact = _artifact(Path(path_text))
        if name == "all":
            for phase in phases:
                artifacts[phase] = artifact
            continue
        if name not in phases:
            raise argparse.ArgumentTypeError(f"unknown SCCP corridor phase: {name}")
        artifacts[name] = artifact
    return artifacts


def _load_evidence_summary(paths: list[Path]) -> dict[str, Any]:
    module = _load_all_lanes_module()
    records = module.load_evidence_bundle(paths)
    return module.validate_evidence_bundle(records)


def _input_artifacts(paths: list[Path]) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in paths:
        artifacts.append(_artifact(path))
    return artifacts


def _submission_surfaces(phase_status: dict[str, str]) -> list[dict[str, Any]]:
    surfaces: list[dict[str, Any]] = []
    for base in USER_PROVER_SUBMISSION_SURFACES:
        surface = dict(base)
        required_phases = list(surface["required_phases"])
        blockers = [
            f"{phase} is {phase_status.get(phase, 'missing')}"
            for phase in required_phases
            if phase_status.get(phase) != "passed"
        ]
        surface["required_phases"] = required_phases
        surface["validation_status"] = "passed" if not blockers else "blocked"
        surface["validation_blockers"] = blockers
        surfaces.append(surface)
    return surfaces


def _build_report(
    paths: list[Path],
    phase_results: list[str],
    phase_evidence: list[str],
    *,
    require_phase_evidence: bool,
    phase_evidence_dir: Path | None = None,
) -> dict[str, Any]:
    phases = _corridor_phases()
    phase_status = _parse_phase_results(phase_results, phases)
    phase_artifacts = _parse_phase_evidence(
        phase_evidence,
        phases,
        phase_status,
        phase_evidence_dir,
    )
    input_artifacts = _input_artifacts(paths)
    evidence = _load_evidence_summary(paths)
    release_checklist = evidence.get(
        "release_checklist",
        {
            "ready": bool(evidence["production_ready"]),
            "items": [],
        },
    )
    failed_phases = [
        phase for phase, status in phase_status.items() if status != "passed"
    ]
    missing_phase_evidence = [
        phase
        for phase, status in phase_status.items()
        if require_phase_evidence and status == "passed" and phase not in phase_artifacts
    ]
    invalid_phase_evidence: dict[str, list[str]] = {
        phase: errors
        for phase, artifact in phase_artifacts.items()
        if phase_status.get(phase) == "passed"
        for errors in [_phase_transcript_errors(phase, artifact)]
        if errors
    }
    corridor_ready = (
        not failed_phases
        and not missing_phase_evidence
        and not invalid_phase_evidence
    )
    production_ready = bool(release_checklist["ready"]) and corridor_ready
    blockers = list(evidence["blockers"])
    blockers.extend(
        f"production corridor phase {phase} is {phase_status[phase]}"
        for phase in failed_phases
    )
    blockers.extend(
        f"production corridor phase {phase} has no hashed evidence artifact"
        for phase in missing_phase_evidence
    )
    blockers.extend(
        f"production corridor phase {phase} {error}"
        for phase, errors in invalid_phase_evidence.items()
        for error in errors
    )
    return {
        "production_ready": production_ready,
        "evidence": evidence,
        "release_checklist": release_checklist,
        "corridor": {
            "production_ready": corridor_ready,
            "phases": phase_status,
            "evidence_artifacts": phase_artifacts,
            "require_phase_evidence": require_phase_evidence,
            "blockers": [
                f"{phase} is {phase_status[phase]}" for phase in failed_phases
            ]
            + [
                f"{phase} has no hashed evidence artifact"
                for phase in missing_phase_evidence
            ]
            + [
                f"{phase} {error}"
                for phase, errors in invalid_phase_evidence.items()
                for error in errors
            ],
        },
        "blockers": blockers,
        "inputs": [str(path) for path in paths],
        "input_artifacts": input_artifacts,
        "cryptographic_evidence": _cryptographic_evidence(evidence),
        "user_prover_submission_surfaces": _submission_surfaces(phase_status),
    }


def _record_flags(records: dict[str, bool]) -> str:
    labels = {
        "source_verifier_material": "source",
        "source_adapter_deployment": "deploy",
        "destination_rollout": "dest",
        "route_allowlist": "route",
    }
    return ", ".join(
        f"{label}={'yes' if records.get(field) else 'no'}"
        for field, label in labels.items()
    )


def _cryptographic_evidence(evidence: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for lane in evidence.get("lanes", []):
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


def _hash_cell(value: Any) -> str:
    if isinstance(value, str) and value:
        return f"`{value}`"
    return "-"


def _render_markdown(report: dict[str, Any], *, max_blockers_per_lane: int) -> str:
    status = "READY" if report["production_ready"] else "NOT READY"
    lines = [
        "# SCCP Release Readiness Report",
        "",
        f"Status: {status}",
        "",
        "## Evidence Inputs",
        "",
    ]
    lines.append("| Path | Bytes | SHA-256 |")
    lines.append("| --- | ---: | --- |")
    for artifact in report["input_artifacts"]:
        lines.append(
            "| `{path}` | {bytes} | `{sha256}` |".format(
                path=artifact["path"],
                bytes=artifact["bytes"],
                sha256=artifact["sha256"],
            )
        )
    lines.extend(["", "## Production Corridor", ""])
    lines.append("| Phase | Status | Evidence Artifact | Evidence SHA-256 |")
    lines.append("| --- | --- | --- | --- |")
    for phase, phase_status in report["corridor"]["phases"].items():
        artifact = report["corridor"]["evidence_artifacts"].get(phase)
        artifact_path = f"`{artifact['path']}`" if artifact else "-"
        artifact_hash = f"`{artifact['sha256']}`" if artifact else "-"
        lines.append(
            f"| `{phase}` | {phase_status} | {artifact_path} | {artifact_hash} |"
        )

    lines.extend(["", "## Release Checklist", ""])
    lines.append("| Gate | Status | Blockers |")
    lines.append("| --- | --- | --- |")
    for item in report["release_checklist"]["items"]:
        item_status = "ready" if item["ready"] else "blocked"
        blockers = item["blockers"][:max_blockers_per_lane]
        blocker_text = "<br>".join(blockers) if blockers else "-"
        if len(item["blockers"]) > max_blockers_per_lane:
            remaining = len(item["blockers"]) - max_blockers_per_lane
            blocker_text += f"<br>... {remaining} more"
        lines.append(f"| `{item['id']}` | {item_status} | {blocker_text} |")

    lines.extend(["", "## Cryptographic Evidence", ""])
    lines.append(
        "| Domain | Chain | Source Material | Source Deployment | "
        "Destination Binding | Route Allowlist | Route Canary | Canary Source |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for row in report["cryptographic_evidence"]:
        canary_source = row["route_canary_evidence_source"] or "-"
        if not row["route_canary_evidence_bound"]:
            canary_source = f"{canary_source} (unbound)"
        lines.append(
            "| {domain} | `{chain}` | {source} | {deploy} | {dest} | "
            "{route} | {canary} | `{canary_source}` |".format(
                domain=row["domain"],
                chain=row["chain"],
                source=_hash_cell(row["source_verifier_material_hash"]),
                deploy=_hash_cell(row["source_adapter_engine_deployment_hash"]),
                dest=_hash_cell(row["destination_binding_hash"]),
                route=_hash_cell(row["route_allowlist_hash"]),
                canary=_hash_cell(row["route_canary_evidence_hash"]),
                canary_source=canary_source,
            )
        )

    lines.extend(["", "## User Prover Submission Surfaces", ""])
    lines.append(
        "| Lanes | Proof Backend | SDK Helpers | On-chain Submission | "
        "Required Phases | Validation |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- |")
    for surface in report["user_prover_submission_surfaces"]:
        required_phases = ", ".join(
            f"`{phase}`" for phase in surface["required_phases"]
        )
        validation = surface["validation_status"]
        if surface["validation_blockers"]:
            validation += ": " + "<br>".join(surface["validation_blockers"])
        lines.append(
            "| `{lanes}` | `{proof_backend}` | {sdk_helpers} | {submission} | "
            "{required_phases} | {validation} |".format(
                lanes=surface["lanes"],
                proof_backend=surface["proof_backend"],
                sdk_helpers=surface["sdk_helpers"],
                submission=surface["on_chain_submission"],
                required_phases=required_phases,
                validation=validation,
            )
        )

    lines.extend(["", "## Lane Readiness", ""])
    lines.append("| Domain | Chain | Status | Records | Blockers |")
    lines.append("| --- | --- | --- | --- | --- |")
    for lane in report["evidence"]["lanes"]:
        lane_status = "ready" if lane["production_ready"] else "blocked"
        blockers = lane["blockers"][:max_blockers_per_lane]
        blocker_text = "<br>".join(blockers) if blockers else "-"
        if len(lane["blockers"]) > max_blockers_per_lane:
            remaining = len(lane["blockers"]) - max_blockers_per_lane
            blocker_text += f"<br>... {remaining} more"
        lines.append(
            "| {domain} | `{chain}` | {status} | {records} | {blockers} |".format(
                domain=lane["domain"],
                chain=lane["chain"],
                status=lane_status,
                records=_record_flags(lane["records"]),
                blockers=blocker_text,
            )
        )

    lines.extend(["", "## Blocking Items", ""])
    if report["blockers"]:
        for blocker in report["blockers"]:
            lines.append(f"- {blocker}")
    else:
        lines.append("- None")

    lines.extend(
        [
            "",
            "## Required Release Evidence",
            "",
            "- A passing `bash scripts/check_sccp_production_corridor.sh` run, recorded with `--require-phase-evidence` and one hashed `--phase-evidence` artifact for every passed phase.",
            "- A complete all-lanes evidence bundle containing source verifier material, source-adapter deployment, destination rollout, route allowlist, and route canary records for every advertised SCCP remote domain.",
            "- Governed live deployment evidence for immutable destination verifiers and source-chain verifier engines; offline placeholder or template-derived hashes keep the report blocked.",
            "- Public release notes must attach this report and the all-lanes JSON summary before production activation.",
        ]
    )
    return "\n".join(lines) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Render a public SCCP release-readiness report from all-lanes "
            "evidence and production-corridor validation results."
        )
    )
    parser.add_argument(
        "toml",
        nargs="+",
        type=Path,
        help="TOML evidence snippet or full config containing [zk] SCCP records.",
    )
    parser.add_argument(
        "--phase-result",
        action="append",
        default=[],
        metavar="PHASE=STATUS",
        help=(
            "Production-corridor phase status. Repeat for each phase, or use "
            "all=passed after a full corridor run."
        ),
    )
    parser.add_argument(
        "--phase-evidence",
        action="append",
        default=[],
        metavar="PHASE=PATH",
        help=(
            "Hash a production-corridor run artifact for one phase, or use "
            "all=PATH to bind the same full-run log to every phase."
        ),
    )
    parser.add_argument(
        "--phase-evidence-dir",
        type=Path,
        help=(
            "Directory containing hashed production-corridor phase logs. The "
            "report accepts <dir>/<phase>.log, "
            "<dir>/dist/sccp-production-corridor/<phase>.log, or downloaded "
            "CI artifact folders named sccp-production-corridor-<phase>."
        ),
    )
    parser.add_argument(
        "--require-phase-evidence",
        action="store_true",
        help=(
            "Keep the report blocked unless every passed corridor phase has a "
            "hashed --phase-evidence artifact."
        ),
    )
    parser.add_argument(
        "--format",
        choices=("markdown", "json"),
        default="markdown",
        help="Report output format.",
    )
    parser.add_argument(
        "--max-blockers-per-lane",
        type=int,
        default=4,
        help="Maximum lane blockers to show in the markdown table.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the report to this path instead of stdout.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.max_blockers_per_lane < 1:
        parser.error("--max-blockers-per-lane must be positive")

    try:
        report = _build_report(
            args.toml,
            args.phase_result,
            args.phase_evidence,
            require_phase_evidence=args.require_phase_evidence,
            phase_evidence_dir=args.phase_evidence_dir,
        )
    except (OSError, RuntimeError, ValueError, argparse.ArgumentTypeError) as exc:
        parser.exit(2, f"{parser.prog}: error: {exc}\n")

    if args.format == "json":
        output = json.dumps(report, indent=2, sort_keys=True) + "\n"
    else:
        output = _render_markdown(
            report,
            max_blockers_per_lane=args.max_blockers_per_lane,
        )

    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")
    return 0 if report["production_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
