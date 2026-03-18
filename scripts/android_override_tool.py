#!/usr/bin/env python3
"""
Helpers for managing Android telemetry redaction overrides.

The CLI mirrors the workflow captured in the Android support playbook:
- `apply` issues a temporary override token, writes a manifest artefact,
  and records the request in the Markdown audit log.
- `revoke` clears the override by stamping the revocation time in the log.

Both sub-commands operate exclusively on local artefacts. Uploading the
manifest or applying the override on Torii is handled by the operator.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import secrets
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple


DEFAULT_DURATION_SECONDS = 60 * 60  # 1 hour
TOKEN_BYTES = 16  # 128-bit random token expressed as hex
TABLE_HEADER_PREFIX = "| Ticket ID"
DEFAULT_ACTOR_ROLE = "support"
ACTOR_ROLE_CHOICES = {
    "support",
    "sre",
    "docs",
    "compliance",
    "program",
    "other",
}


class OverrideError(RuntimeError):
    """Domain error raised for invalid override operations."""


def _isoformat(ts: dt.datetime) -> str:
    """Return a zulu ISO-8601 string."""
    ts = ts.astimezone(dt.timezone.utc).replace(microsecond=0)
    return ts.isoformat().replace("+00:00", "Z")


def _load_request(path: Path) -> Dict[str, Any]:
    """Load an override request file.

    The request is expected to be JSON; if parsing fails we support a simple
    `key=value` fallback to avoid blocking emergency overrides.
    """
    if not path.is_file():
        raise OverrideError(f"request file does not exist: {path}")

    raw = path.read_text(encoding="utf-8").strip()
    if not raw:
        raise OverrideError(f"request file is empty: {path}")

    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        data: Dict[str, Any] = {}
        for line in raw.splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            if "=" in stripped:
                key, value = stripped.split("=", 1)
            elif ":" in stripped:
                key, value = stripped.split(":", 1)
            else:
                raise OverrideError(
                    f"could not parse line in request file: {line!r}"
                )
            data[key.strip()] = value.strip()
        if not data:
            raise OverrideError(f"request file format not recognised: {path}")
        return data


def _generate_token(token: Optional[str] = None) -> str:
    if token:
        return token
    return secrets.token_hex(TOKEN_BYTES)


def _hash_token(token: str) -> str:
    digest = hashlib.blake2b(token.encode("utf-8"), digest_size=32)
    return digest.hexdigest()


def _sanitize_cell(value: str) -> str:
    return value.replace("|", "\\|")


def _desanitize_cell(value: str) -> str:
    return value.replace("\\|", "|").strip()


def _locate_table(lines: Sequence[str]) -> Tuple[int, int]:
    """Return (start_index, end_index) for the audit table within ``lines``."""
    start = -1
    for idx, line in enumerate(lines):
        if line.strip().startswith(TABLE_HEADER_PREFIX):
            start = idx
            break
    if start == -1:
        raise OverrideError("telemetry override log is missing the table header")

    end = start + 1
    while end < len(lines) and lines[end].strip().startswith("|"):
        end += 1
    return start, end


def _append_row(
    lines: List[str],
    row: List[str],
) -> None:
    start, end = _locate_table(lines)
    row_line = "| " + " | ".join(_sanitize_cell(col) for col in row) + " |\n"
    lines.insert(end, row_line)


def _iterate_table_rows(lines: Sequence[str]) -> List[List[str]]:
    """Return parsed table rows (excluding the header/separator)."""
    start, end = _locate_table(lines)
    rows: List[List[str]] = []
    for idx in range(start + 2, end):
        line = lines[idx].strip()
        if not line or not line.startswith("|"):
            continue
        if line.startswith("|-----------"):
            continue
        parts = [
            _desanitize_cell(segment)
            for segment in line.strip().strip("|").split("|")
        ]
        if len(parts) >= 7:
            rows.append(parts[:7])
    return rows


def _update_row_revoked(
    lines: List[str],
    token_hash: str,
    revoked_ts: str,
) -> Dict[str, str]:
    for idx, line in enumerate(lines):
        if not line.strip().startswith("|"):
            continue
        if line.strip().startswith(TABLE_HEADER_PREFIX):
            continue
        if line.strip().startswith("|-----------"):
            continue
        if token_hash not in lines[idx]:
            continue
        parts = [segment.strip() for segment in line.strip().strip("|").split("|")]
        if len(parts) < 7:
            continue
        if parts[3] == token_hash:
            parts[5] = revoked_ts
            lines[idx] = "| " + " | ".join(_sanitize_cell(col) for col in parts) + " |\n"
            return {
                "ticket_id": parts[0],
                "requested_at": parts[1],
                "approver": parts[2],
                "token_hash": parts[3],
                "expires_at": parts[4],
                "revoked_at": parts[5],
                "notes": parts[6],
            }
    raise OverrideError(f"token hash not found in audit log: {token_hash}")


def _normalise_actor_role(value: Optional[str]) -> str:
    """Return the telemetry-safe actor role bucket."""
    if value is None:
        return DEFAULT_ACTOR_ROLE
    normalized = value.strip().lower().replace("-", "_").replace(" ", "_")
    if normalized not in ACTOR_ROLE_CHOICES:
        raise OverrideError(
            f"actor role must be one of {sorted(ACTOR_ROLE_CHOICES)} (got {value!r})"
        )
    return normalized


def _record_override_event(event_log: Optional[Path], payload: Dict[str, Any]) -> None:
    """Append an override event to the NDJSON log if requested."""
    if event_log is None:
        return
    event_log.parent.mkdir(parents=True, exist_ok=True)
    enriched = dict(payload)
    enriched.setdefault("signal", "android.telemetry.redaction.override")
    enriched.setdefault("logged_at", _isoformat(dt.datetime.now(dt.timezone.utc)))
    with event_log.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(enriched, sort_keys=True))
        handle.write("\n")


def apply_override(
    *,
    request_path: Path,
    log_path: Path,
    output_path: Path,
    reason: Optional[str] = None,
    duration_seconds: Optional[int] = None,
    ticket_id: Optional[str] = None,
    approver: Optional[str] = None,
    now: Optional[dt.datetime] = None,
    token: Optional[str] = None,
    actor_role: Optional[str] = None,
    event_log: Optional[Path] = None,
) -> Dict[str, Any]:
    request = _load_request(request_path)

    ticket = ticket_id or request.get("ticket_id")
    if not ticket:
        raise OverrideError("ticket id is required (missing --ticket and request.ticket_id)")

    approver_name = approver or request.get("approver")
    if not approver_name:
        raise OverrideError("approver is required (missing --approver and request.approver)")

    duration = duration_seconds or request.get("duration_seconds") or DEFAULT_DURATION_SECONDS
    try:
        duration_int = int(duration)
    except (TypeError, ValueError) as exc:
        raise OverrideError(f"duration must be an integer (seconds): {duration!r}") from exc
    if duration_int <= 0:
        raise OverrideError("duration must be positive")

    issued_at = now or dt.datetime.now(dt.timezone.utc)
    expires_at = issued_at + dt.timedelta(seconds=duration_int)

    token_value = _generate_token(token)
    token_hash = _hash_token(token_value)

    requester = request.get("requested_by")
    request_reason = request.get("reason")
    effective_reason = (reason or request_reason or "Android telemetry override").strip()
    actor_role_masked = _normalise_actor_role(actor_role or request.get("actor_role"))

    notes_parts = [effective_reason]
    if requester:
        notes_parts.append(f"requested by {requester}")
    notes_parts.append(f"request {request_path.as_posix()}")
    notes = "; ".join(part for part in notes_parts if part)

    manifest = {
        "kind": "android.telemetry.override",
        "ticket_id": ticket,
        "reason": effective_reason,
        "token": token_value,
        "issued_at": _isoformat(issued_at),
        "expires_at": _isoformat(expires_at),
        "approver": approver_name,
    }
    if requester:
        manifest["requested_by"] = requester
    additional = {key: value for key, value in request.items() if key not in manifest}
    if additional:
        manifest["request_metadata"] = additional

    output_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if not log_path.is_file():
        raise OverrideError(f"audit log not found: {log_path}")
    lines = log_path.read_text(encoding="utf-8").splitlines(keepends=True)

    # Ensure the token hash does not already exist.
    _, end = _locate_table(lines)
    for idx in range(end):
        if token_hash in lines[idx]:
            raise OverrideError("generated token collides with existing entry; retry")

    _append_row(
        lines,
        [
            ticket,
            _isoformat(issued_at),
            approver_name,
            token_hash,
            _isoformat(expires_at),
            "-",
            notes,
        ],
    )

    log_path.write_text("".join(lines), encoding="utf-8")

    _record_override_event(
        event_log,
        {
            "action": "apply",
            "actor_role_masked": actor_role_masked,
            "override_id": token_hash,
            "ticket_id": ticket,
            "reason": effective_reason,
            "issued_at": _isoformat(issued_at),
            "expires_at": _isoformat(expires_at),
            "approver": approver_name,
        },
    )

    return {
        "token": token_value,
        "token_hash": token_hash,
        "issued_at": _isoformat(issued_at),
        "expires_at": _isoformat(expires_at),
        "ticket_id": ticket,
        "approver": approver_name,
        "notes": notes,
        "manifest_path": str(output_path),
        "actor_role": actor_role_masked,
    }


def revoke_override(
    *,
    token: str,
    log_path: Path,
    now: Optional[dt.datetime] = None,
    actor_role: Optional[str] = None,
    event_log: Optional[Path] = None,
) -> Dict[str, Any]:
    if not token:
        raise OverrideError("token is required")
    if not log_path.is_file():
        raise OverrideError(f"audit log not found: {log_path}")

    revoked_at = now or dt.datetime.now(dt.timezone.utc)
    revoked_str = _isoformat(revoked_at)
    token_hash = _hash_token(token)

    lines = log_path.read_text(encoding="utf-8").splitlines(keepends=True)
    row = _update_row_revoked(lines, token_hash, revoked_str)
    log_path.write_text("".join(lines), encoding="utf-8")

    actor_role_masked = _normalise_actor_role(actor_role)
    _record_override_event(
        event_log,
        {
            "action": "revoke",
            "actor_role_masked": actor_role_masked,
            "override_id": token_hash,
            "ticket_id": row["ticket_id"],
            "revoked_at": revoked_str,
            "expires_at": row["expires_at"],
        },
    )

    return {
        "token_hash": token_hash,
        "revoked_at": revoked_str,
        "ticket_id": row["ticket_id"],
        "actor_role": actor_role_masked,
    }


def export_override_digest(
    *,
    log_path: Path,
    output_path: Path,
    now: Optional[dt.datetime] = None,
) -> Dict[str, Any]:
    """Write a sanitised JSON digest summarising the override log."""
    if not log_path.is_file():
        raise OverrideError(f"audit log not found: {log_path}")

    lines = log_path.read_text(encoding="utf-8").splitlines()
    rows = _iterate_table_rows(lines)

    entries = []
    active = 0
    for row in rows:
        ticket_id, requested_at, approver, token_hash, expires_at, revoked_at, notes = row
        revoked = revoked_at if revoked_at not in {"-", ""} else None
        status = "active" if revoked is None else "revoked"
        if status == "active":
            active += 1
        entries.append(
            {
                "ticket_id": ticket_id,
                "requested_at": requested_at,
                "approver": approver,
                "token_hash": token_hash,
                "expires_at": expires_at,
                "revoked_at": revoked,
                "status": status,
                "notes": notes,
            }
        )

    generated_at = _isoformat(now or dt.datetime.now(dt.timezone.utc))
    digest = {
        "generated_at": generated_at,
        "source_log": str(log_path),
        "log_sha256": hashlib.sha256(log_path.read_bytes()).hexdigest(),
        "entry_count": len(entries),
        "active_overrides": active,
        "entries": entries,
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(digest, indent=2) + "\n", encoding="utf-8")
    return digest


def _parse_timestamp(value: str) -> dt.datetime:
    """Parse an ISO-8601 timestamp (accepts trailing Z)."""
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        ts = dt.datetime.fromisoformat(value)
    except ValueError as exc:  # pragma: no cover - defensive
        raise OverrideError(f"invalid timestamp: {value}") from exc
    if ts.tzinfo is None:
        raise OverrideError("timestamp must include a timezone offset")
    return ts


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Android telemetry override helper")
    parser.set_defaults(func=None)

    parser.add_argument(
        "--log",
        type=Path,
        default=Path("docs/source/sdk/android/telemetry_override_log.md"),
        help="Path to the Markdown audit log (default: %(default)s)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("telemetry_redaction_override.to"),
        help="Output path for the override manifest (default: %(default)s)",
    )
    parser.add_argument(
        "--event-log",
        type=Path,
        help="Optional NDJSON log to append android.telemetry.redaction.override events",
    )

    subparsers = parser.add_subparsers(dest="command")

    apply_parser = subparsers.add_parser("apply", help="Issue a telemetry override token")
    apply_parser.add_argument("--request", required=True, type=Path, help="Path to the signed override request (.ko)")
    apply_parser.add_argument("--reason", help="Override reason to record in the manifest/log")
    apply_parser.add_argument("--duration", type=int, help="Override duration in seconds")
    apply_parser.add_argument("--ticket", help="Ticket identifier (falls back to request.ticket_id)")
    apply_parser.add_argument("--approver", help="Approver name (falls back to request.approver)")
    apply_parser.add_argument(
        "--actor-role",
        choices=sorted(ACTOR_ROLE_CHOICES),
        help=f"Categorise the actor issuing the override (default: {DEFAULT_ACTOR_ROLE})",
    )
    apply_parser.set_defaults(func=_cmd_apply)

    revoke_parser = subparsers.add_parser("revoke", help="Revoke a telemetry override token")
    revoke_parser.add_argument("--token", required=True, help="Override token issued by `apply`")
    revoke_parser.add_argument(
        "--actor-role",
        choices=sorted(ACTOR_ROLE_CHOICES),
        help=f"Categorise the actor revoking the override (default: {DEFAULT_ACTOR_ROLE})",
    )
    revoke_parser.set_defaults(func=_cmd_revoke)

    digest_parser = subparsers.add_parser("digest", help="Export a sanitised override digest JSON")
    digest_parser.add_argument("--out", type=Path, help="Output path for the digest JSON")
    digest_parser.add_argument(
        "--timestamp",
        help="Override generated_at timestamp (ISO-8601, e.g. 2026-02-12T15:30:00Z)",
    )
    digest_parser.set_defaults(func=_cmd_digest)

    return parser


def _cmd_apply(args: argparse.Namespace) -> int:
    result = apply_override(
        request_path=args.request,
        log_path=args.log,
        output_path=args.output,
        reason=args.reason,
        duration_seconds=args.duration,
        ticket_id=args.ticket,
        approver=args.approver,
        actor_role=args.actor_role,
        event_log=args.event_log,
    )
    stamp = result["issued_at"]
    token = result["token"]
    print(f"[{stamp}] Override token issued: {token}")
    print(f"[{stamp}] Manifest written to {result['manifest_path']}")
    print(f"[{stamp}] Recorded entry in {args.log}")
    return 0


def _cmd_revoke(args: argparse.Namespace) -> int:
    result = revoke_override(
        token=args.token,
        log_path=args.log,
        actor_role=args.actor_role,
        event_log=args.event_log,
    )
    print(f"[{result['revoked_at']}] Override revoked; telemetry restored to hashed state")
    return 0


def _cmd_digest(args: argparse.Namespace) -> int:
    timestamp = _parse_timestamp(args.timestamp) if args.timestamp else None
    if args.out:
        output_path = args.out
    else:
        stamp = (timestamp or dt.datetime.now(dt.timezone.utc)).strftime("%Y%m%dT%H%M%SZ")
        output_path = Path("docs/source/sdk/android/readiness/override_logs") / f"override_digest_{stamp}.json"

    result = export_override_digest(log_path=args.log, output_path=output_path, now=timestamp)
    print(f"[{result['generated_at']}] Override digest written to {output_path}")
    return 0


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if not getattr(args, "func", None):
        parser.print_help(file=sys.stderr)
        return 1

    try:
        return args.func(args)
    except OverrideError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
