#!/usr/bin/env python3
"""
Assemble the transparency release manifest and checksum file.

This script consumes the artefacts produced by the ingest/build/sanitize
pipeline and generates:

1. `checksums.sha256` — deterministic SHA-256 digests for every artefact.
2. `transparency_manifest.json` — provenance manifest referencing the
   quarter, artefact list, optional SoraFS CID, and operator notes.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from types import ModuleType
from typing import Iterable, Optional

HEAD_REQUEST_ACTION = "MinistryTransparencyHeadUpdateV1"
DEFAULT_IPNS_KEY = "ministry-transparency.latest"
DEFAULT_HEAD_UPDATER_PATH = Path(__file__).with_name("publisher_head_updater.py")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def blake2b_hex(path: Path, *, digest_size: int = 32) -> str:
    """Compute a BLAKE2b digest for the given file."""

    digest = hashlib.blake2b(digest_size=digest_size)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_kv(entry: str) -> tuple[str, Path]:
    if "=" not in entry:
        raise argparse.ArgumentTypeError(
            f"expected label=path format for --artifact, got '{entry}'"
        )
    label, raw_path = entry.split("=", 1)
    if not label:
        raise argparse.ArgumentTypeError("artifact label cannot be empty")
    return label, Path(raw_path).resolve()


def build_artifact_list(args: argparse.Namespace) -> list[tuple[str, Path]]:
    artefacts: list[tuple[str, Path]] = [
        ("sanitized_metrics", Path(args.sanitized).resolve()),
        ("dp_report", Path(args.dp_report).resolve()),
    ]
    optional_fields = [
        ("ingest_snapshot", args.ingest),
        ("metrics_json", args.metrics),
        ("metrics_manifest", args.raw_manifest),
        ("summary", args.summary),
    ]
    for label, value in optional_fields:
        if value:
            artefacts.append((label, Path(value).resolve()))
    if args.artifact:
        artefacts.extend(args.artifact)
    return artefacts


def write_checksums(
    artefacts: Iterable[tuple[str, Path]], output_dir: Path
) -> tuple[Path, list[dict[str, str]]]:
    lines = []
    manifest_entries = []
    for label, path in artefacts:
        if not path.exists():
            raise FileNotFoundError(f"artifact '{label}' missing at {path}")
        digest = sha256(path)
        try:
            rel_path = path.relative_to(output_dir)
        except ValueError:
            rel_path = path
        lines.append(f"{digest}  {rel_path}\n")
        manifest_entries.append(
            {"label": label, "path": str(rel_path), "sha256": digest}
        )
    checksums_path = output_dir / "checksums.sha256"
    checksums_path.write_text("".join(lines), encoding="utf-8")
    return checksums_path, manifest_entries


def write_manifest(
    args: argparse.Namespace, entries: list[dict[str, str]], checksums: Path, generated_at: str
) -> Path:
    manifest = {
        "version": 1,
        "quarter": args.quarter,
        "generated_at": generated_at,
        "sorafs_cid": args.sorafs_cid,
        "note": args.note,
        "artifacts": entries,
        "checksums_file": str(checksums.relative_to(args.output_dir)),
    }
    manifest_path = args.output_dir / "transparency_manifest.json"
    manifest_path.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"[transparency_release] wrote manifest to {manifest_path}")
    return manifest_path


def write_head_update_request(
    *,
    queue_dir: Path,
    quarter: str,
    generated_at: str,
    generated_at_slug: str,
    sorafs_cid_hex: str,
    ipns_key: str,
    manifest_path: Path,
    checksums_path: Path,
    action_path: Path,
    dashboards_git_sha: Optional[str],
    note: Optional[str],
) -> Path:
    """Emit a publisher head-update request for the transparency release."""

    queue_dir.mkdir(parents=True, exist_ok=True)
    quarter_slug = quarter.replace("/", "-")
    filename = f"{quarter_slug}_{generated_at_slug}_head_request.json"
    request_path = queue_dir / filename

    request = {
        "action": HEAD_REQUEST_ACTION,
        "version": 1,
        "quarter": quarter,
        "generated_at": generated_at,
        "sorafs_cid_hex": sorafs_cid_hex,
        "ipns_key": ipns_key,
        "release_dir": f"ministry/releases/{quarter}",
        "manifest_path": str(manifest_path),
        "checksums_path": str(checksums_path),
        "release_action": str(action_path),
    }
    if dashboards_git_sha:
        request["dashboards_git_sha"] = dashboards_git_sha
    if note:
        request["note"] = note

    request_path.write_text(
        json.dumps(request, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"[transparency_release] wrote head request to {request_path}")
    return request_path


def load_head_updater_module(module_path: Path) -> ModuleType:
    """Dynamically import the publisher head updater helper."""

    module_path = module_path.resolve()
    spec = importlib.util.spec_from_file_location(
        "ministry_publisher_head_updater", module_path
    )
    if spec is None or spec.loader is None:
        raise ImportError(f"unable to load head updater from {module_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[attr-defined]
    return module


def process_head_request(
    *,
    request_path: Path,
    governance_dir: Path,
    updater_module_path: Path,
    ipns_template: Optional[str],
    dry_run: bool,
) -> Path:
    """Process a single head-update file via the publisher helper."""

    module = load_head_updater_module(updater_module_path)
    try:
        head_update_config_cls = module.HeadUpdateConfig
        handle_request_file = module.handle_request_file
    except AttributeError as exc:  # pragma: no cover - defensive guard
        raise RuntimeError(
            "publisher head updater is missing the expected helpers"
        ) from exc

    queue_dir = request_path.parent
    processed_dir = queue_dir / "processed"
    publisher_dir = governance_dir / "publisher"
    state_dir = publisher_dir / "ipns_heads"
    log_path = publisher_dir / "head_updates.log"

    config = head_update_config_cls(
        queue_dir=queue_dir,
        processed_dir=processed_dir,
        state_dir=state_dir,
        log_path=log_path,
        ipns_template=ipns_template,
        dry_run=dry_run,
    )
    return handle_request_file(request_path, config)


def load_root_cid_from_summary(summary_path: Path) -> str:
    """Extract the first root CID from a sorafs_cli summary JSON."""

    data = json.loads(summary_path.read_text(encoding="utf-8"))
    root_cids = data.get("root_cids_hex")
    if not isinstance(root_cids, list) or not root_cids:
        raise ValueError(f"{summary_path} is missing `root_cids_hex`")
    first = root_cids[0]
    if not isinstance(first, str) or not first.strip():
        raise ValueError(f"{summary_path} contains an invalid `root_cids_hex` entry")
    return first.strip().lower()


def detect_git_rev() -> Optional[str]:
    """Best-effort `git rev-parse HEAD` helper."""

    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip() or None


def write_governance_payload(
    *,
    output_path: Path,
    quarter: str,
    generated_at: str,
    manifest_path: Path,
    manifest_digest_hex: str,
    checksums_path: Path,
    sorafs_cid_hex: str,
    dashboards_git_sha: Optional[str],
    note: Optional[str],
) -> Path:
    payload = {
        "action": "TransparencyReleaseV1",
        "version": 1,
        "quarter": quarter,
        "generated_at": generated_at,
        "manifest_digest_blake2b_256": manifest_digest_hex,
        "manifest_path": str(manifest_path),
        "checksums_path": str(checksums_path),
        "sorafs_cid_hex": sorafs_cid_hex,
        "dashboards_git_sha": dashboards_git_sha,
        "note": note,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"[transparency_release] wrote governance payload to {output_path}")
    return output_path


def anchor_release(action_path: Path, governance_dir: Path) -> None:
    """Invoke the xtask anchor helper to drop release artefacts into the governance DAG dir."""

    print(
        "[transparency_release] anchoring release via "
        "`cargo xtask ministry-transparency anchor`"
    )
    subprocess.run(
        [
            "cargo",
            "xtask",
            "ministry-transparency",
            "anchor",
            "--action",
            str(action_path),
            "--governance-dir",
            str(governance_dir),
        ],
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Assemble transparency release manifest")
    parser.add_argument("--quarter", required=True, help="Quarter identifier (e.g. 2026-Q3)")
    parser.add_argument(
        "--output-dir",
        required=True,
        help="Directory where manifest + checksum files will be written",
    )
    parser.add_argument("--ingest", help="Path to the ingest snapshot JSON")
    parser.add_argument("--metrics", help="Path to the metrics JSON emitted by `xtask build`")
    parser.add_argument("--raw-manifest", help="Path to the `TransparencyManifest` JSON")
    parser.add_argument("--summary", help="Path to the Markdown/PDF executive summary")
    parser.add_argument("--sanitized", required=True, help="Path to sanitized metrics JSON")
    parser.add_argument("--dp-report", required=True, help="Path to DP audit report JSON")
    parser.add_argument(
        "--artifact",
        action="append",
        type=parse_kv,
        metavar="label=path",
        help="Extra artifact to include (can be repeated)",
    )
    parser.add_argument("--sorafs-cid", help="Optional SoraFS CID for the published bundle")
    parser.add_argument(
        "--sorafs-summary",
        help="Path to sorafs_cli summary JSON (car pack/proof verify) to extract the SoraFS CID",
    )
    parser.add_argument("--note", help="Optional operator note recorded in the manifest")
    parser.add_argument(
        "--governance-out",
        help="Path for the TransparencyReleaseV1 governance payload (defaults to output-dir)",
    )
    parser.add_argument(
        "--dashboards-git-sha",
        help="Override dashboards git SHA recorded in the governance payload (defaults to HEAD)",
    )
    parser.add_argument(
        "--governance-dir",
        help="Optional governance DAG directory; when provided the script will run "
        "`cargo xtask ministry-transparency anchor` to drop the Norito payload + JSON summary",
    )
    parser.add_argument(
        "--skip-anchor",
        action="store_true",
        help="Skip the `cargo xtask ... anchor` invocation (useful for dry runs).",
    )
    parser.add_argument(
        "--ipns-key",
        default=DEFAULT_IPNS_KEY,
        help=(
            "IPNS key alias recorded in publisher head-update requests "
            "(defaults to %(default)s)"
        ),
    )
    parser.add_argument(
        "--auto-head-update",
        action="store_true",
        help=(
            "Immediately process the emitted head-request JSON via "
            "publisher_head_updater.py when --governance-dir is provided."
        ),
    )
    parser.add_argument(
        "--head-updater-path",
        default=str(DEFAULT_HEAD_UPDATER_PATH),
        help="Path to publisher_head_updater.py (used with --auto-head-update)",
    )
    parser.add_argument(
        "--head-update-ipns-template",
        help=(
            "Optional IPNS publish command template forwarded to the head "
            "updater when --auto-head-update is enabled."
        ),
    )
    parser.add_argument(
        "--head-update-dry-run",
        action="store_true",
        help="Propagate dry-run behaviour to the auto head-update step.",
    )
    args = parser.parse_args()

    args.output_dir = Path(args.output_dir).resolve()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    if args.auto_head_update and not args.governance_dir:
        parser.error("--auto-head-update requires --governance-dir")

    head_updater_path = Path(args.head_updater_path).resolve()

    auto_cid = None
    if args.sorafs_summary:
        summary_path = Path(args.sorafs_summary).resolve()
        auto_cid = load_root_cid_from_summary(summary_path)
    provided_cid = args.sorafs_cid.lower() if args.sorafs_cid else None
    if provided_cid and auto_cid and provided_cid != auto_cid:
        raise SystemExit(
            f"SoraFS CID mismatch: provided {provided_cid} but summary reported {auto_cid}"
        )
    sorafs_cid = provided_cid or auto_cid
    if sorafs_cid is None:
        raise SystemExit(
            "Unable to determine SoraFS CID. Provide --sorafs-cid or point "
            "--sorafs-summary at a sorafs_cli car/proof summary."
        )
    args.sorafs_cid = sorafs_cid

    artefacts = build_artifact_list(args)
    checksums_path, manifest_entries = write_checksums(artefacts, args.output_dir)
    now = dt.datetime.now(dt.timezone.utc)
    generated_at = now.isoformat()
    generated_at_slug = now.strftime("%Y%m%dT%H%M%SZ")
    manifest_path = write_manifest(args, manifest_entries, checksums_path, generated_at)

    dashboards_git_sha = args.dashboards_git_sha or detect_git_rev()
    governance_path = (
        Path(args.governance_out).resolve()
        if args.governance_out
        else args.output_dir / "transparency_release_action.json"
    )
    manifest_digest_hex = blake2b_hex(manifest_path)
    action_path = write_governance_payload(
        output_path=governance_path,
        quarter=args.quarter,
        generated_at=generated_at,
        manifest_path=manifest_path,
        manifest_digest_hex=manifest_digest_hex,
        checksums_path=checksums_path,
        sorafs_cid_hex=sorafs_cid,
        dashboards_git_sha=dashboards_git_sha,
        note=args.note,
    )

    governance_dir: Optional[Path] = None
    head_request_path = None
    if args.governance_dir:
        governance_dir = Path(args.governance_dir).resolve()
        if args.skip_anchor:
            print("[transparency_release] skipping anchor (--skip-anchor enabled)")
        else:
            anchor_release(action_path, governance_dir)
        head_queue_dir = (
            governance_dir / "publisher" / "head_requests" / "ministry_transparency"
        )
        head_request_path = write_head_update_request(
            queue_dir=head_queue_dir,
            quarter=args.quarter,
            generated_at=generated_at,
            generated_at_slug=generated_at_slug,
            sorafs_cid_hex=sorafs_cid,
            ipns_key=args.ipns_key,
            manifest_path=manifest_path,
            checksums_path=checksums_path,
            action_path=action_path,
            dashboards_git_sha=dashboards_git_sha,
            note=args.note,
        )
        print(
            "[transparency_release] queued publisher head update at "
            f"{head_request_path}"
        )

        if args.auto_head_update:
            processed_request = process_head_request(
                request_path=head_request_path,
                governance_dir=governance_dir,
                updater_module_path=head_updater_path,
                ipns_template=args.head_update_ipns_template,
                dry_run=args.head_update_dry_run,
            )
            if args.head_update_dry_run:
                print(
                    "[transparency_release] auto head-update dry-run complete; "
                    "request left in queue"
                )
            else:
                print(
                    "[transparency_release] processed publisher head update "
                    f"via {processed_request}"
                )


if __name__ == "__main__":
    main()
