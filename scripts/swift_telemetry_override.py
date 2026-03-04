#!/usr/bin/env python3
"""Manage Swift telemetry redaction overrides stored in a local JSON ledger."""

from __future__ import annotations

import argparse
import json
import sys
import uuid
from dataclasses import dataclass, asdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import List, Optional

DEFAULT_STORE = Path("artifacts/swift_telemetry_overrides.json")


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _parse_iso(value: str) -> datetime:
    try:
        if value.endswith("Z"):
            value = value[:-1] + "+00:00"
        return datetime.fromisoformat(value).astimezone(timezone.utc)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"invalid ISO-8601 timestamp: {value}") from exc


@dataclass
class OverrideEntry:
    id: str
    actor_role_masked: str
    reason: str
    created_at: str
    expires_at: str
    revoked_at: Optional[str] = None

    @property
    def active(self) -> bool:
        if self.revoked_at is not None:
            return False
        try:
            expires = _parse_iso(self.expires_at)
        except argparse.ArgumentTypeError:
            return False
        return expires > _now()


def _load_store(path: Path) -> List[OverrideEntry]:
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON in override store {path}: {exc}") from exc
    if not isinstance(data, list):
        raise RuntimeError(f"override store {path} must be a list")
    entries: List[OverrideEntry] = []
    for item in data:
        if not isinstance(item, dict):
            continue
        entries.append(
            OverrideEntry(
                id=str(item.get("id", "")),
                actor_role_masked=str(item.get("actor_role_masked", "")),
                reason=str(item.get("reason", "")),
                created_at=str(item.get("created_at", "")),
                expires_at=str(item.get("expires_at", "")),
                revoked_at=item.get("revoked_at"),
            )
        )
    return entries


def _write_store(path: Path, entries: List[OverrideEntry]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = [asdict(entry) for entry in entries]
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def action_list(args: argparse.Namespace) -> None:
    entries = _load_store(args.store)
    if args.json:
        print(json.dumps([asdict(entry) | {"active": entry.active} for entry in entries], indent=2))
        return
    if not entries:
        print("(no overrides)")
        return
    for entry in entries:
        status = "active" if entry.active else "inactive"
        revoked = f", revoked_at={entry.revoked_at}" if entry.revoked_at else ""
        print(
            f"{entry.id} [{status}] role={entry.actor_role_masked} reason={entry.reason} "
            f"created={entry.created_at} expires={entry.expires_at}{revoked}"
        )


def action_create(args: argparse.Namespace) -> None:
    entries = _load_store(args.store)
    now = _now()
    expires_at: datetime
    if args.expires_at:
        expires_at = _parse_iso(args.expires_at)
    else:
        expires_at = now + timedelta(hours=args.expires_in_hours)
    override = OverrideEntry(
        id=args.id or str(uuid.uuid4()),
        actor_role_masked=args.actor_role,
        reason=args.reason,
        created_at=_iso(now),
        expires_at=_iso(expires_at),
    )
    entries.append(override)
    _write_store(args.store, entries)
    print(json.dumps(asdict(override), indent=2))


def action_revoke(args: argparse.Namespace) -> None:
    entries = _load_store(args.store)
    updated = False
    for entry in entries:
        if entry.id == args.id and entry.revoked_at is None:
            entry.revoked_at = _iso(_now())
            updated = True
            break
    if not updated:
        raise SystemExit(f"override {args.id} not found or already revoked")
    _write_store(args.store, entries)
    print(f"override {args.id} revoked")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Manage Swift telemetry redaction overrides.")
    parser.add_argument(
        "--store",
        type=Path,
        default=DEFAULT_STORE,
        help=f"Override store path (default: {DEFAULT_STORE})",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    cmd_ls = subparsers.add_parser("list", help="List overrides in the store.")
    cmd_ls.add_argument("--json", action="store_true", help="Emit JSON instead of text.")
    cmd_ls.set_defaults(func=action_list)

    cmd_create = subparsers.add_parser("create", help="Create a new override entry.")
    cmd_create.add_argument("--id", help="Optional override identifier (defaults to UUID4).")
    cmd_create.add_argument(
        "--actor-role",
        required=True,
        help="Masked actor role (support/sre/audit/etc.).",
    )
    cmd_create.add_argument("--reason", required=True, help="Reason for the override.")
    expires_group = cmd_create.add_mutually_exclusive_group()
    expires_group.add_argument(
        "--expires-in-hours",
        type=float,
        default=24,
        help="Hours until the override expires (default: 24).",
    )
    expires_group.add_argument(
        "--expires-at",
        type=str,
        help="Explicit expiration timestamp (ISO-8601).",
    )
    cmd_create.set_defaults(func=action_create)

    cmd_revoke = subparsers.add_parser("revoke", help="Revoke an override by ID.")
    cmd_revoke.add_argument("--id", required=True, help="Override identifier.")
    cmd_revoke.set_defaults(func=action_revoke)

    return parser


def main(argv: List[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    args.func(args)  # type: ignore[attr-defined]
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
