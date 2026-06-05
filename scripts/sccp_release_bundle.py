#!/usr/bin/env python3
"""Build hash-bound SCCP public release-note attachments."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
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


def _artifact(path: Path, root: Path) -> dict[str, Any]:
    payload = path.read_bytes()
    artifact_path = path.relative_to(root).as_posix()
    control_character = _path_control_character(artifact_path)
    if control_character is not None:
        raise ValueError(
            "release artifact path contains control character "
            f"{control_character}: {artifact_path!r}"
        )
    return {
        "path": artifact_path,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def _copy_file(source: Path, destination: Path) -> Path:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    return destination


def _safe_name(path: Path, index: int) -> str:
    name = path.name.replace("/", "_").replace("\\", "_")
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
    if phase_evidence_dir is not None:
        for phase in phases:
            sources[phase] = _phase_log_from_dir(phase_evidence_dir, phase)
    for raw in phase_evidence:
        name, path = _parse_phase_evidence_arg(raw)
        if name == "all":
            for phase in phases:
                sources[phase] = path
            continue
        if name not in phases:
            raise argparse.ArgumentTypeError(f"unknown SCCP corridor phase: {name}")
        sources[name] = path
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
    control_character = _path_control_character(value)
    if control_character is not None:
        raise ValueError(
            "native EVM Groth16 prover bundle "
            f"{label} path contains control character {control_character}: {value!r}"
        )
    if "\\" in value:
        raise ValueError(
            f"native EVM Groth16 prover bundle {label} path must use POSIX separators"
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

    def add_path(raw_path: Any, label: str) -> None:
        relative_path = _native_evm_manifest_relative_path(raw_path, label)
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


def _release_notes_attachment(
    report: dict[str, Any],
    artifacts: list[dict[str, Any]],
) -> str:
    status = "READY" if report["production_ready"] else "NOT READY"
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
    if report["blockers"]:
        lines.extend(["", "## Blocking Items", ""])
        lines.extend(f"- {blocker}" for blocker in report["blockers"])
    return "\n".join(lines) + "\n"


def _bundle_artifacts(output_dir: Path, paths: list[Path]) -> list[dict[str, Any]]:
    return [_artifact(path, output_dir) for path in paths]


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


def _validate_output_dir(
    output_dir: Path,
    *,
    input_paths: list[Path],
    phase_sources: dict[str, Path],
    native_evm_prover_bundle: Path | None,
    force: bool,
) -> None:
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
    if not force:
        return

    protected_paths = [*input_paths, *phase_sources.values()]
    if native_evm_prover_bundle is not None:
        protected_paths.append(native_evm_prover_bundle)
        protected_paths.extend(
            source
            for _, source in _native_evm_prover_payload_sources(
                native_evm_prover_bundle
            )
        )
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
        if not preflight_report["production_ready"] and not args.allow_not_ready:
            blockers = "\n".join(f"- {blocker}" for blocker in preflight_report["blockers"])
            parser.exit(1, f"SCCP release bundle is not production ready:\n{blockers}\n")

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
        manifest = {
            "schema": "sccp-release-bundle-v1",
            "production_ready": bool(report["production_ready"]),
            "release_checklist_ready": bool(report["release_checklist"]["ready"]),
            "corridor_ready": bool(report["corridor"]["production_ready"]),
            "blockers": report["blockers"],
            "artifacts": _bundle_artifacts(args.output_dir, all_artifact_paths),
        }
        _write_json(manifest_json, manifest)
        verification_summary: dict[str, Any] | None = None
        if report["production_ready"]:
            verification_summary = _verify_generated_bundle(args.output_dir)
    except (
        OSError,
        RuntimeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        parser.exit(2, f"{parser.prog}: error: {exc}\n")

    print(f"Wrote SCCP release bundle to {args.output_dir}")
    if report["production_ready"]:
        print(f"Verified SCCP release bundle at {args.output_dir}")
        if verification_summary is not None:
            print(
                "SCCP release bundle manifest_sha256: "
                f"{verification_summary['manifest_sha256']}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
