#!/usr/bin/env python3
"""
Aggregate Android partner SLA session data.

This helper walks `docs/source/sdk/android/partner_sla_sessions/`, loads each
`session.json`, and writes a concise summary capturing partner counts,
acknowledgements, blackout windows, and open action items. The output can be
used directly as the roadmap/status evidence for AND8 coordination.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Mapping, MutableMapping, Sequence, Set


DEFAULT_ROOT = Path("docs/source/sdk/android/partner_sla_sessions")
DEFAULT_RELEASE_PACKET = "docs/source/sdk/android/readiness/archive/ga-2027-10/release_calendar_ga-2027-10.md"
DEFAULT_ACK_LOG = "docs/source/sdk/android/readiness/archive/ga-2027-10/ack_log_ga-2027-10.md"
DEFAULT_ACTION_LOG = "docs/source/sdk/android/readiness/partner_sla_action_log.md"
DEFAULT_SUPPORT_MATRIX = "docs/source/sdk/android/android_support_playbook.md#section-7-5-cutover-and-support-matrix"
DEFAULT_FORUM_BUNDLE = "docs/source/sdk/android/readiness/partner_enablement_forum/2026-03/"


@dataclass(frozen=True)
class ActionItem:
    description: str
    owner: str
    due: str | None
    partner: str


@dataclass(frozen=True)
class Session:
    partner: str
    date: str
    acknowledged: bool
    acknowledgement_ref: str | None
    blackout_windows: Sequence[str]
    action_items: Sequence[ActionItem]


def _load_session(path: Path) -> Session:
    try:
        payload: MutableMapping[str, object] = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to parse session JSON {path}: {exc}") from exc
    try:
        partner = str(payload["partner"])
        date = str(payload["date"])
        acknowledged = bool(payload.get("acknowledged", False))
        acknowledgement_ref = payload.get("acknowledgement_ref")
    except (KeyError, TypeError) as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] session {path} missing required fields") from exc

    blackout_windows_raw = payload.get("blackout_windows", [])
    if not isinstance(blackout_windows_raw, list):
        blackout_windows = []
    else:
        blackout_windows = [str(entry) for entry in blackout_windows_raw]

    action_items_payload = payload.get("action_items", [])
    action_items: List[ActionItem] = []
    if isinstance(action_items_payload, list):
        for entry in action_items_payload:
            if not isinstance(entry, dict):
                continue
            description = str(entry.get("description", "")).strip()
            owner = str(entry.get("owner", "")).strip()
            due_raw = entry.get("due")
            due = str(due_raw).strip() if due_raw is not None else None
            if description and owner:
                action_items.append(ActionItem(description=description, owner=owner, due=due, partner=partner))
    return Session(
        partner=partner,
        date=date,
        acknowledged=acknowledged,
        acknowledgement_ref=str(acknowledgement_ref) if acknowledgement_ref else None,
        blackout_windows=blackout_windows,
        action_items=action_items,
    )


def load_sessions(root: Path) -> Sequence[Session]:
    if not root.exists():
        raise SystemExit(f"[error] session root does not exist: {root}")
    sessions: List[Session] = []
    for path in sorted(root.glob("*/*/session.json")):
        sessions.append(_load_session(path))
    if not sessions:
        raise SystemExit(f"[error] no session.json files found under {root}")
    return sessions


def _dedupe(items: Iterable[str]) -> List[str]:
    seen: Set[str] = set()
    output: List[str] = []
    for item in items:
        if item not in seen:
            seen.add(item)
            output.append(item)
    return output


def build_summary(
    sessions: Sequence[Session],
    *,
    release_packet: str,
    ack_log: str,
    action_log: str,
    support_matrix: str,
    forum_bundle: str,
) -> Mapping[str, object]:
    partners = _dedupe(sorted(session.partner for session in sessions))
    acknowledged = _dedupe(sorted(session.partner for session in sessions if session.acknowledged))
    pending = [partner for partner in partners if partner not in set(acknowledged)]
    blackouts = _dedupe(sorted(window for session in sessions for window in session.blackout_windows))
    actions = _dedupe_actions(sessions)
    return {
        "release_packet": release_packet,
        "ack_log": ack_log,
        "action_log": action_log,
        "support_matrix": support_matrix,
        "forum_bundle": forum_bundle,
        "partners": partners,
        "sessions": len(sessions),
        "acknowledged_partners": acknowledged,
        "pending_acknowledgements": pending,
        "blackout_windows": blackouts,
        "open_actions": actions,
    }


def _dedupe_actions(sessions: Sequence[Session]) -> List[Mapping[str, str]]:
    actions: List[Mapping[str, str]] = []
    seen: Set[tuple[str, str, str | None, str]] = set()
    for session in sessions:
        for item in session.action_items:
            key = (item.partner, item.description, item.owner, item.due)
            if key in seen:
                continue
            seen.add(key)
            actions.append(
                {
                    "partner": item.partner,
                    "description": item.description,
                    "owner": item.owner,
                    "due": item.due or "unspecified",
                }
            )
    actions.sort(key=lambda entry: (entry["due"], entry["partner"], entry["description"]))
    return actions


def render_summary_text(summary: Mapping[str, object]) -> str:
    partners = summary.get("partners", [])
    acknowledged = summary.get("acknowledged_partners", [])
    pending = summary.get("pending_acknowledgements", [])
    blackouts = summary.get("blackout_windows", [])
    open_actions = summary.get("open_actions", [])
    lines = [
        "Partner SLA Summary",
        "===================",
        f"Release calendar packet: {summary.get('release_packet', '-')}",
        f"Acknowledgement log: {summary.get('ack_log', '-')}",
        f"Action tracker: {summary.get('action_log', '-')}",
        "",
        f"Total partners: {len(partners)} ({', '.join(partners) if partners else '—'})",
        f"Sessions captured: {summary.get('sessions', 0)}",
        f"Acknowledged partners: {len(acknowledged)} ({', '.join(acknowledged) if acknowledged else '—'})",
        f"Pending acknowledgements: {len(pending)} ({', '.join(pending) if pending else '—'})",
        "Known blackout windows: " + (", ".join(blackouts) if blackouts else "—"),
        "",
        "Open action items:",
    ]
    if open_actions:
        for action in open_actions:
            lines.append(
                f"- [{action['partner']}] {action['description']} (owner: {action['owner']}, due: {action['due']})"
            )
    else:
        lines.append("- None")
    lines.extend(
        [
            "",
            "Cutover matrix & forum bundle",
            "-----------------------------",
            f"- Cutover/support matrix: {summary.get('support_matrix', '-')}",
            f"- Partner enablement forum bundle: {summary.get('forum_bundle', '-')}",
        ]
    )
    return "\n".join(lines) + "\n"


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Aggregate partner SLA session data for Android AND8 readiness.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT, help="Root directory containing partner session folders.")
    parser.add_argument("--release-packet", default=DEFAULT_RELEASE_PACKET, help="Path to the release calendar packet.")
    parser.add_argument("--ack-log", default=DEFAULT_ACK_LOG, help="Path to the acknowledgement log.")
    parser.add_argument("--action-log", default=DEFAULT_ACTION_LOG, help="Path to the action tracker.")
    parser.add_argument("--support-matrix", default=DEFAULT_SUPPORT_MATRIX, help="Path to the cutover/support matrix.")
    parser.add_argument("--forum-bundle", default=DEFAULT_FORUM_BUNDLE, help="Path to the partner enablement forum bundle.")
    parser.add_argument("--text-out", type=Path, help="Write the rendered text summary to the given path.")
    parser.add_argument("--json-out", type=Path, help="Write a machine-readable JSON summary to the given path.")
    args = parser.parse_args(argv)

    sessions = load_sessions(args.root)
    summary = build_summary(
        sessions,
        release_packet=args.release_packet,
        ack_log=args.ack_log,
        action_log=args.action_log,
        support_matrix=args.support_matrix,
        forum_bundle=args.forum_bundle,
    )
    text = render_summary_text(summary)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    if args.text_out:
        args.text_out.parent.mkdir(parents=True, exist_ok=True)
        args.text_out.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)

    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
