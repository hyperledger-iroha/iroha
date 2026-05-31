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
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ALL_LANES_SCRIPT = ROOT / "scripts" / "sccp_all_lanes_evidence.py"
REPORT_SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"


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


def _artifact(path: Path, root: Path) -> dict[str, Any]:
    payload = path.read_bytes()
    return {
        "path": path.relative_to(root).as_posix(),
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


def _relative_to_bundle(output_dir: Path, path: Path) -> Path:
    return path.relative_to(output_dir)


def _build_bundle_report(
    report_module: Any,
    output_dir: Path,
    evidence_paths: list[Path],
    phase_results: list[str],
    phase_evidence_args: list[str],
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
        return report_module._build_report(
            relative_evidence,
            phase_results,
            relative_phase_evidence,
            require_phase_evidence=True,
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
            force=args.force,
        )
        preflight_report = report_module._build_report(
            args.toml,
            args.phase_result,
            _phase_evidence_args(phase_sources),
            require_phase_evidence=True,
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
        report = _build_bundle_report(
            report_module,
            args.output_dir,
            copied_evidence,
            args.phase_result,
            copied_phase_args,
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
    except (
        OSError,
        RuntimeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        parser.exit(2, f"{parser.prog}: error: {exc}\n")

    print(f"Wrote SCCP release bundle to {args.output_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
