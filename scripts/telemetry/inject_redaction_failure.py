#!/usr/bin/env python3
"""
Inject or clear Android telemetry redaction failures for AND7 rehearsals.

By default the helper mutates the local cache at
``artifacts/android/telemetry/status.json`` so teams can simulate alerts even
without access to the staging cluster. When ``--status-url`` is provided the
script POSTs a JSON payload to the specified endpoint instead, making it easy
to hook up to a live admin surface.

Example invocations::

    # Inject a mismatch for alice@wonderland and update the local cache
    scripts/telemetry/inject_redaction_failure.py --authority alice@wonderland

    # Clear recorded failures (after verifying alert recovery)
    scripts/telemetry/inject_redaction_failure.py --clear

    # Hit a staging endpoint instead of the cache
    scripts/telemetry/inject_redaction_failure.py \
        --status-url https://android-telemetry-stg/admin/redaction/failure

    # Show the current mismatch queue without mutating the cache
    scripts/telemetry/inject_redaction_failure.py --status
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, Optional, TextIO
from urllib import error, request

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from _redaction_status import (  # pylint: disable=import-error
    DEFAULT_STATUS_PATH,
    clear_failures,
    default_status,
    fetch_status_from_url,
    normalise_status,
    now_iso,
    read_status_file,
    update_failure_entries,
    write_status_file,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inject or clear Android telemetry redaction failures."
    )
    parser.add_argument(
        "--status-json",
        help="Path to the local status cache (defaults to "
        "ANDROID_TELEMETRY_STATUS_JSON or "
        f"{DEFAULT_STATUS_PATH})",
    )
    parser.add_argument(
        "--status-url",
        help="POST destination for remote injections. "
        "Set ANDROID_TELEMETRY_STATUS_URL to avoid passing this every time.",
    )
    parser.add_argument(
        "--authority",
        default="alice@wonderland",
        help="Authority to flag (default: alice@wonderland).",
    )
    parser.add_argument(
        "--reason",
        default="hash_mismatch",
        help="Failure reason label (default: hash_mismatch).",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=1,
        help="How many mismatches to add (default: 1).",
    )
    parser.add_argument(
        "--note",
        help="Optional note to record alongside the failure entry.",
    )
    parser.add_argument(
        "--hashed-authorities",
        type=int,
        help="Override hashed authority count when creating the cache.",
    )
    parser.add_argument(
        "--clear",
        action="store_true",
        help="Reset mismatch counters and remove recorded failures.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the would-be payload without mutating files/endpoints.",
    )
    parser.add_argument(
        "--status",
        action="store_true",
        help="Print the current mismatch queue and exit without mutating "
        "the cache or remote endpoint.",
    )
    return parser.parse_args()


def resolve_status_path(args: argparse.Namespace) -> Path:
    return Path(
        args.status_json
        or os.environ.get("ANDROID_TELEMETRY_STATUS_JSON", DEFAULT_STATUS_PATH)
    )


def inject_remote(args: argparse.Namespace) -> None:
    url = args.status_url or os.environ.get("ANDROID_TELEMETRY_STATUS_URL")
    if not url:
        raise SystemExit(
            "[error] --status-url or ANDROID_TELEMETRY_STATUS_URL is required "
            "when not using the local cache."
        )
    payload: Dict[str, Any] = {
        "authority": args.authority,
        "reason": args.reason,
        "count": args.count,
        "timestamp": now_iso(),
    }
    if args.note:
        payload["note"] = args.note
    if args.clear:
        payload["clear"] = True
    body = json.dumps(payload).encode("utf-8")
    if args.dry_run:
        print(f"[dry-run] POST {url} payload={json.dumps(payload)}")
        return
    req = request.Request(url, data=body, method="POST")
    req.add_header("Content-Type", "application/json")
    try:
        with request.urlopen(req, timeout=5.0) as response:  # nosec B310
            print(
                f"[{now_iso()}] POST {url} -> {response.status} {response.reason}"
            )
            print(response.read().decode("utf-8"))
    except error.HTTPError as exc:  # pragma: no cover - network guard
        raise SystemExit(
            f"[error] remote injection failed ({exc.code} {exc.reason})"
        ) from exc
    except error.URLError as exc:  # pragma: no cover - network guard
        raise SystemExit(f"[error] remote injection failed: {exc}") from exc


def inject_local(args: argparse.Namespace) -> None:
    status_path = resolve_status_path(args)
    try:
        data = read_status_file(status_path, create=not args.dry_run)
    except FileNotFoundError:
        data = default_status()
    if args.clear:
        updated = clear_failures(data)
        action = "Cleared"
    else:
        updated = update_failure_entries(
            data,
            authority=args.authority,
            reason=args.reason,
            count=max(1, args.count),
            note=args.note,
        )
        action = f"Injected mismatch for authority `{args.authority}`"
    if args.hashed_authorities is not None:
        updated["hashed_authorities"] = max(
            args.hashed_authorities, updated.get("mismatched_authorities", 0)
        )
    if args.dry_run:
        print(json.dumps(updated, indent=2))
        return
    write_status_file(status_path, updated)
    print(f"[{now_iso()}] {action} (status cache: {status_path})")


def print_status_summary(
    payload: Dict[str, Any], *, output: Optional[TextIO] = None
) -> None:
    """Render a short summary of the cached status payload."""

    output = output or sys.stdout
    status = normalise_status(payload)
    timestamp = status.get("timestamp", now_iso())
    hashed = int(status.get("hashed_authorities", 0) or 0)
    mismatched = int(status.get("mismatched_authorities", 0) or 0)
    salt = status.get("salt") or {}
    salt_bits = []
    if salt.get("epoch"):
        salt_bits.append(f"salt_epoch={salt['epoch']}")
    if salt.get("rotation_id"):
        salt_bits.append(f"salt_rotation={salt['rotation_id']}")
    salt_suffix = "  " + "  ".join(salt_bits) if salt_bits else ""
    print(
        f"STATUS  timestamp={timestamp}  hashed_authorities={hashed}  "
        f"mismatched={mismatched}{salt_suffix}",
        file=output,
    )
    failures = status.get("recent_failures") or []
    if not failures:
        print("  no_recent_failures=true", file=output)
    else:
        for entry in failures:
            authority = entry.get("authority", "unknown")
            reason = entry.get("reason", "unknown")
            count = int(entry.get("count", 1) or 1)
            ts = entry.get("timestamp")
            note = entry.get("note")
            parts = [
                f"FAILURE authority={authority}",
                f"reason={reason}",
                f"count={count}",
            ]
            if ts:
                parts.append(f"reported_at={ts}")
            if note:
                parts.append(f"note={note}")
            print("  " + "  ".join(parts), file=output)


def main() -> None:
    args = parse_args()
    if args.status:
        try:
            env_url = os.environ.get("ANDROID_TELEMETRY_STATUS_URL")
            if args.status_url or env_url:
                url = args.status_url or env_url
                payload = fetch_status_from_url(url)
            else:
                payload = read_status_file(resolve_status_path(args), create=False)
        except FileNotFoundError as exc:
            raise SystemExit(
                f"[error] status cache {exc.filename} not found. "
                "Run the status checker or inject helper first."
            ) from exc
        except Exception as exc:  # pragma: no cover - defensive guard
            raise SystemExit(f"[error] failed to load status payload: {exc}") from exc
        print_status_summary(payload)
        return

    if args.status_url or os.environ.get("ANDROID_TELEMETRY_STATUS_URL"):
        inject_remote(args)
    else:
        inject_local(args)


if __name__ == "__main__":
    main()
