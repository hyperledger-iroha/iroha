#!/usr/bin/env python3
"""
Process MinistryTransparencyHeadUpdateV1 requests and update the local publisher state.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import shlex
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

HEAD_REQUEST_ACTION = "MinistryTransparencyHeadUpdateV1"


@dataclass(frozen=True)
class HeadUpdateConfig:
    queue_dir: Path
    processed_dir: Path
    state_dir: Path
    log_path: Path
    ipns_template: Optional[str]
    dry_run: bool


def load_request(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("action") != HEAD_REQUEST_ACTION:
        raise ValueError(
            f"{path} is not a {HEAD_REQUEST_ACTION} request (action={data.get('action')})"
        )
    required = [
        "quarter",
        "generated_at",
        "sorafs_cid_hex",
        "ipns_key",
        "release_dir",
        "manifest_path",
        "checksums_path",
        "release_action",
    ]
    for field in required:
        value = data.get(field)
        if value is None or (isinstance(value, str) and not value.strip()):
            raise ValueError(f"{path} is missing required field `{field}`")
    return data


def maybe_run_ipns_command(template: str, request: dict, *, dry_run: bool) -> None:
    mapping = {
        "ipns_key": request["ipns_key"],
        "cid": request["sorafs_cid_hex"],
        "sorafs_cid": request["sorafs_cid_hex"],
        "sorafs_cid_hex": request["sorafs_cid_hex"],
        "release_dir": request["release_dir"],
        "manifest_path": request["manifest_path"],
        "checksums_path": request["checksums_path"],
        "release_action": request["release_action"],
    }
    command = template.format(**mapping)
    args = shlex.split(command)
    if dry_run:
        print(f"[head_updater] dry-run: would run {' '.join(args)}")
        return
    subprocess.run(args, check=True)


def ensure_parent(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def rotate_request_file(source: Path, processed_dir: Path) -> Path:
    processed_dir.mkdir(parents=True, exist_ok=True)
    candidate = processed_dir / source.name
    counter = 1
    while candidate.exists():
        candidate = processed_dir / f"{source.stem}_{counter}{source.suffix}"
        counter += 1
    source.rename(candidate)
    return candidate


def write_state(
    request: dict,
    *,
    state_dir: Path,
    processed_at: dt.datetime,
) -> Path:
    state_dir.mkdir(parents=True, exist_ok=True)
    state_payload = {
        "ipns_key": request["ipns_key"],
        "quarter": request["quarter"],
        "generated_at": request["generated_at"],
        "processed_at": processed_at.isoformat(),
        "sorafs_cid_hex": request["sorafs_cid_hex"],
        "release_dir": request["release_dir"],
        "manifest_path": request["manifest_path"],
        "checksums_path": request["checksums_path"],
        "release_action": request["release_action"],
        "dashboards_git_sha": request.get("dashboards_git_sha"),
        "note": request.get("note"),
    }
    state_path = state_dir / f"{request['ipns_key']}.json"
    ensure_parent(state_path)
    state_path.write_text(
        json.dumps(state_payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return state_path


def append_log(log_path: Path, entry: str) -> None:
    ensure_parent(log_path)
    with log_path.open("a", encoding="utf-8") as handle:
        handle.write(entry)


def handle_request_file(
    path: Path, config: HeadUpdateConfig, *, now: Optional[dt.datetime] = None
) -> Path:
    request = load_request(path)
    timestamp = now or dt.datetime.now(dt.timezone.utc)

    if config.ipns_template:
        maybe_run_ipns_command(config.ipns_template, request, dry_run=config.dry_run)

    if config.dry_run:
        print(
            f"[head_updater] dry-run: would mark {path.name} as processed for "
            f"{request['ipns_key']} (quarter {request['quarter']})"
        )
        return path

    state_path = write_state(
        request,
        state_dir=config.state_dir,
        processed_at=timestamp,
    )
    log_entry = (
        f"{timestamp.isoformat()} ipns_key={request['ipns_key']} "
        f"quarter={request['quarter']} cid={request['sorafs_cid_hex']} "
        f"state={state_path}\n"
    )
    append_log(config.log_path, log_entry)
    processed_path = rotate_request_file(path, config.processed_dir)
    return processed_path


def build_config(args: argparse.Namespace) -> HeadUpdateConfig:
    governance_dir = Path(args.governance_dir).resolve()
    queue_dir = (
        Path(args.queue_dir).resolve()
        if args.queue_dir
        else governance_dir
        / "publisher"
        / "head_requests"
        / "ministry_transparency"
    )
    processed_dir = (
        Path(args.processed_dir).resolve()
        if args.processed_dir
        else queue_dir / "processed"
    )
    state_dir = (
        Path(args.state_dir).resolve()
        if args.state_dir
        else governance_dir / "publisher" / "ipns_heads"
    )
    log_path = (
        Path(args.log_path).resolve()
        if args.log_path
        else governance_dir / "publisher" / "head_updates.log"
    )
    return HeadUpdateConfig(
        queue_dir=queue_dir,
        processed_dir=processed_dir,
        state_dir=state_dir,
        log_path=log_path,
        ipns_template=args.ipns_template,
        dry_run=args.dry_run,
    )


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Process pending Ministry transparency head-update requests."
    )
    parser.add_argument(
        "--governance-dir",
        required=True,
        help="Root governance directory that houses publisher artifacts.",
    )
    parser.add_argument(
        "--queue-dir",
        help="Override the head request directory. Defaults to "
        "<governance-dir>/publisher/head_requests/ministry_transparency.",
    )
    parser.add_argument(
        "--processed-dir",
        help="Override the processed request directory (default: <queue>/processed).",
    )
    parser.add_argument(
        "--state-dir",
        help="Override the directory storing ipns head state (default: "
        "<governance-dir>/publisher/ipns_heads).",
    )
    parser.add_argument(
        "--log-path",
        help="Override the log file path "
        "(default: <governance-dir>/publisher/head_updates.log).",
    )
    parser.add_argument(
        "--ipns-template",
        help=(
            "Optional command template used to publish IPNS updates. "
            "Use placeholders like {ipns_key}, {cid}, {release_dir}."
        ),
    )
    parser.add_argument(
        "--max-requests",
        type=int,
        default=0,
        help="Maximum number of requests to process (0 = all).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate requests without moving files or updating state/IPNS.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)
    config = build_config(args)

    if not config.queue_dir.exists():
        print(f"[head_updater] queue directory {config.queue_dir} does not exist", file=sys.stderr)
        return 1

    pending = sorted(
        path
        for path in config.queue_dir.iterdir()
        if path.is_file() and path.suffix == ".json"
    )
    if not pending:
        print("[head_updater] no pending requests")
        return 0

    processed = 0
    failures = 0
    for path in pending:
        if args.max_requests and processed >= args.max_requests:
            break
        try:
            handle_request_file(path, config)
            processed += 1
        except Exception as exc:  # pylint: disable=broad-except
            failures += 1
            print(f"[head_updater] failed to process {path}: {exc}", file=sys.stderr)

    if processed:
        print(f"[head_updater] processed {processed} request(s)")
    if failures:
        print(f"[head_updater] {failures} request(s) failed", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
