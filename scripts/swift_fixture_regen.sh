#!/usr/bin/env bash
set -euo pipefail

# Regenerates Swift Norito fixtures by copying the canonical artifacts produced
# for the Android SDK. This keeps both SDKs on the same set of fixtures until
# the shared Rust exporter (`scripts/export_norito_fixtures`) grows Swift-specific
# outputs. The script mirrors `.norito` and supporting JSON files into
# `IrohaSwift/Fixtures` (configurable via env vars) so Swift tests and dashboards
# can compare against the latest canonical set.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${SWIFT_FIXTURE_SOURCE:-${REPO_ROOT}/java/iroha_android/src/test/resources}"
TARGET_DIR="${SWIFT_FIXTURE_OUT:-${REPO_ROOT}/IrohaSwift/Fixtures}"
STATE_FILE="${SWIFT_FIXTURE_STATE_FILE:-${REPO_ROOT}/artifacts/swift_fixture_regen_state.json}"
ROTATION_OWNER="${SWIFT_FIXTURE_ROTATION_OWNER:-}"
CADENCE_LABEL="${SWIFT_FIXTURE_CADENCE:-weekly-wed-1700utc}"
CADENCE_INTERVAL_HOURS="${SWIFT_FIXTURE_CADENCE_INTERVAL_HOURS:-48}"
ALERT_CONTACT="${SWIFT_FIXTURE_ALERT_CONTACT:-}"
ALERT_CHANNEL="${SWIFT_FIXTURE_ALERT_CHANNEL:-}"
ODD_WEEK_OWNER="${SWIFT_FIXTURE_ODD_WEEK_OWNER:-android-foundations}"
EVEN_WEEK_OWNER="${SWIFT_FIXTURE_EVEN_WEEK_OWNER:-swift-lead}"
EVENT_TRIGGER="${SWIFT_FIXTURE_EVENT_TRIGGER:-}"
EVENT_REASON="${SWIFT_FIXTURE_EVENT_REASON:-}"
WINDOW_HOURS="${SWIFT_FIXTURE_WINDOW_HOURS:-24}"
ARCHIVE_PATH="${SWIFT_FIXTURE_ARCHIVE:-}"
PROVENANCE_OUT="${SWIFT_FIXTURE_PROVENANCE_OUT:-${REPO_ROOT}/artifacts/swift_fixture_provenance.json}"
PROVENANCE_APPROVER="${SWIFT_FIXTURE_APPROVER:-}"
PROVENANCE_TICKET="${SWIFT_FIXTURE_TICKET:-}"
PROVENANCE_NOTES="${SWIFT_FIXTURE_NOTES:-}"
PROVENANCE_GIT_REF="${SWIFT_FIXTURE_GIT_REF:-}"
ARCHIVE_SHA=""
ARCHIVE_KIND=""
ARCHIVE_PATH_ABS=""
SOURCE_KIND="directory"
_TEMP_ARCHIVE_DIR=""

if [[ -n "${ARCHIVE_PATH}" ]]; then
  if [[ ! -f "${ARCHIVE_PATH}" ]]; then
    echo "[swift-fixtures] archive not found: ${ARCHIVE_PATH}" >&2
    exit 1
  fi
  SOURCE_KIND="archive"
  ARCHIVE_PATH_ABS="$(python3 -c 'import os, sys; print(os.path.abspath(sys.argv[1]))' "${ARCHIVE_PATH}")"
  _TEMP_ARCHIVE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/swift-fixtures-archive.XXXXXX")"
  cleanup_archive() {
    rm -rf "${_TEMP_ARCHIVE_DIR}"
  }
  trap cleanup_archive EXIT
  EXTRACT_DIR="${_TEMP_ARCHIVE_DIR}/archive"
  META_FILE="${_TEMP_ARCHIVE_DIR}/metadata.json"
  mkdir -p "${EXTRACT_DIR}"
  echo "[swift-fixtures] extracting archive ${ARCHIVE_PATH_ABS} into ${EXTRACT_DIR}"
  python3 -m scripts.swift_fixture_archive \
    --archive "${ARCHIVE_PATH_ABS}" \
    --out-dir "${EXTRACT_DIR}" \
    --meta-out "${META_FILE}"
  mapfile -t archive_info < <(python3 - "${META_FILE}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    data = json.load(handle)
print(data["root"])
print(data.get("sha256", ""))
print(data.get("kind", ""))
PY
)
  if [[ "${#archive_info[@]}" -lt 1 ]]; then
    echo "[swift-fixtures] failed to parse archive metadata from ${META_FILE}" >&2
    exit 1
  fi
  SOURCE_DIR="${archive_info[0]}"
  ARCHIVE_SHA="${archive_info[1]:-}"
  ARCHIVE_KIND="${archive_info[2]:-}"
  echo "[swift-fixtures] archive extracted; fixture root: ${SOURCE_DIR}"
else
  ARCHIVE_PATH_ABS=""
fi

if [[ ! -d "${SOURCE_DIR}" ]]; then
  echo "[swift-fixtures] source directory not found: ${SOURCE_DIR}" >&2
  exit 1
fi

mkdir -p "${TARGET_DIR}"

echo "[swift-fixtures] syncing fixtures from ${SOURCE_DIR} to ${TARGET_DIR}"
rsync -a --delete --prune-empty-dirs \
  --include '*/' \
  --include '*.norito' \
  --include '*transaction_payload*.json' \
  --include '*transaction_fixtures*.manifest.json' \
  --include '*trigger_instructions*.json' \
  --exclude '*' \
  "${SOURCE_DIR}/" "${TARGET_DIR}/"

changes=$(cd "${REPO_ROOT}" && git status --short -- "${TARGET_DIR#${REPO_ROOT}/}") || true
if [[ -z "${changes}" ]]; then
  echo "[swift-fixtures] no changes detected"
else
  echo "[swift-fixtures] updated files:" >&2
  echo "${changes}" >&2
fi

mkdir -p "$(dirname "${STATE_FILE}")"
state_summary="$(
  python3 - "${STATE_FILE}" "${SOURCE_DIR}" "${TARGET_DIR}" "${CADENCE_LABEL}" \
    "${ROTATION_OWNER}" "${ODD_WEEK_OWNER}" "${EVEN_WEEK_OWNER}" \
    "${EVENT_TRIGGER}" "${WINDOW_HOURS}" "${EVENT_REASON}" \
    "${SOURCE_KIND}" "${ARCHIVE_PATH_ABS}" "${ARCHIVE_SHA}" "${ARCHIVE_KIND}" \
    "${CADENCE_INTERVAL_HOURS}" "${ALERT_CONTACT}" "${ALERT_CHANNEL}" <<'PY'
from __future__ import annotations

import json
import math
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


def isoformat_utc(dt: datetime) -> str:
    """Return an ISO-8601 string with Z suffix and second precision."""
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_bool(value: str) -> bool:
    return value.lower() in {"1", "true", "yes", "on", "event"} if value else False


def rolling_slot(now: datetime, *, interval_hours: float) -> datetime:
    """Return the most recent rolling interval slot."""
    anchor = datetime(2026, 1, 1, tzinfo=timezone.utc)
    seconds = interval_hours * 3600.0
    if seconds <= 0:
        return now
    offset = (now - anchor).total_seconds()
    steps = math.floor(offset / seconds)
    return anchor + timedelta(seconds=steps * seconds)


def weekly_slot(now: datetime, *, weekday: int, hour: int, minute: int = 0) -> datetime:
    """Return the most recent weekly slot for the requested weekday/time."""
    now_utc = now.astimezone(timezone.utc)
    iso_year, iso_week, _ = now_utc.isocalendar()
    monday = datetime.strptime(f"{iso_year} {iso_week} 1", "%G %V %u").replace(tzinfo=timezone.utc)
    slot = monday + timedelta(days=weekday - 1, hours=hour, minutes=minute)
    while slot > now_utc:
        slot -= timedelta(days=7)
    return slot


def fallback_slot(now: datetime) -> datetime:
    """Return the latest Monday/Thursday 17:00 UTC slot."""
    monday_slot = weekly_slot(now, weekday=1, hour=17)
    thursday_slot = weekly_slot(now, weekday=4, hour=17)
    return max(monday_slot, thursday_slot)


def resolve_slot(now: datetime, label: str, *, interval_hours: float) -> tuple[str, datetime]:
    normalized = (label or "weekly-wed-1700utc").strip().lower()
    if normalized == "rolling-48h":
        return normalized, rolling_slot(now, interval_hours=interval_hours or 48.0)
    if normalized == "fallback-mon-thu-utc":
        return normalized, fallback_slot(now)
    if normalized == "weekly-wed-1700utc":
        return normalized, weekly_slot(now, weekday=3, hour=17)
    # Default to Wednesday cadence for unrecognised labels.
    return "weekly-wed-1700utc", weekly_slot(now, weekday=3, hour=17)


def resolve_next_slot(slot: datetime, label: str, *, interval_hours: float) -> datetime:
    if label == "rolling-48h":
        return slot + timedelta(hours=interval_hours or 48.0)
    if label == "fallback-mon-thu-utc":
        if slot.weekday() == 0:  # Monday
            return slot + timedelta(days=3)
        # Thursday slot rolls over to next Monday.
        return slot + timedelta(days=4)
    return slot + timedelta(days=7)


state_path = Path(sys.argv[1])
source_dir = sys.argv[2]
target_dir = sys.argv[3]
cadence_label = sys.argv[4]
provided_owner = sys.argv[5]
odd_owner = sys.argv[6]
even_owner = sys.argv[7]
event_flag = parse_bool(sys.argv[8])
window_hours = float(sys.argv[9]) if sys.argv[9] else 24.0
event_reason = sys.argv[10].strip()
source_kind = (sys.argv[11] if len(sys.argv) > 11 else "directory").strip().lower() or "directory"
archive_path = sys.argv[12].strip() if len(sys.argv) > 12 else ""
archive_sha = sys.argv[13].strip() if len(sys.argv) > 13 else ""
archive_kind = sys.argv[14].strip() if len(sys.argv) > 14 else ""
cadence_interval_hours = float(sys.argv[15]) if len(sys.argv) > 15 and sys.argv[15] else 48.0
alert_contact = sys.argv[16].strip() if len(sys.argv) > 16 else ""
alert_channel = sys.argv[17].strip() if len(sys.argv) > 17 else ""

now = datetime.now(timezone.utc)
normalized_label, slot = resolve_slot(now, cadence_label, interval_hours=cadence_interval_hours)
next_slot = resolve_next_slot(slot, normalized_label, interval_hours=cadence_interval_hours)
window_end = slot + timedelta(hours=window_hours)
iso_year, iso_week, _ = slot.isocalendar()
auto_owner = odd_owner if iso_week % 2 else even_owner
owner_source = "env" if provided_owner else "auto"
rotation_owner = provided_owner.strip() if provided_owner.strip() else auto_owner
trigger = "event" if event_flag else "scheduled"
event_reason_json = event_reason or None

state_path.parent.mkdir(parents=True, exist_ok=True)
state = {
    "generated_at": isoformat_utc(now),
    "rotation_owner": rotation_owner,
    "rotation_owner_auto": auto_owner,
    "rotation_owner_source": owner_source,
    "cadence": normalized_label,
    "cadence_window_hours": window_hours,
    "cadence_interval_hours": cadence_interval_hours,
    "trigger": trigger,
    "event_reason": event_reason_json,
    "source": source_dir,
    "target": target_dir,
    "window": {
        "slot_start": isoformat_utc(slot),
        "slot_end": isoformat_utc(window_end),
        "next_slot": isoformat_utc(next_slot),
        "iso_year": iso_year,
        "iso_week": iso_week,
        "weekday": 3,
        "label": "Wednesday 17:00 UTC",
    },
    "roster": {
        "odd_weeks": odd_owner,
        "even_weeks": even_owner,
    },
    "source_kind": source_kind,
}
if archive_path:
    archive_info = {"path": archive_path}
    if archive_sha:
        archive_info["sha256"] = archive_sha
    if archive_kind:
        archive_info["kind"] = archive_kind
    state["source_archive"] = archive_info
if alert_contact or alert_channel:
    rotation = {}
    if alert_contact:
        rotation["contact"] = alert_contact
    if alert_channel:
        rotation["channel"] = alert_channel
    state["alert_rotation"] = rotation
state_path.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")

lines = [
    f"[swift-fixtures] wrote cadence state to {state_path}",
    (
        "[swift-fixtures] rotation owner: "
        f"{rotation_owner} (auto={auto_owner}, source={owner_source}) | "
        f"trigger: {trigger} | slot_start: {state['window']['slot_start']} | "
        f"next_slot: {state['window']['next_slot']}"
    ),
]
if event_reason_json:
    lines.append(f"[swift-fixtures] event reason: {event_reason_json}")
if archive_path:
    sha_display = archive_sha or "unknown"
    kind_display = archive_kind or "unknown"
    lines.append(
        f"[swift-fixtures] source archive: {archive_path} "
        f"(sha256={sha_display}, kind={kind_display})"
    )
print("\n".join(lines))
PY
)"
echo "${state_summary}"

if [[ -n "${PROVENANCE_OUT}" ]]; then
  echo "[swift-fixtures] writing provenance manifest to ${PROVENANCE_OUT}"
  python3 "${REPO_ROOT}/scripts/swift_fixture_provenance.py" \
    --root "${TARGET_DIR}" \
    --state-file "${STATE_FILE}" \
    --out "${PROVENANCE_OUT}" \
    --git-ref "${PROVENANCE_GIT_REF}" \
    --approver "${PROVENANCE_APPROVER}" \
    --change-ticket "${PROVENANCE_TICKET}" \
    --source-kind "${SOURCE_KIND}" \
    --archive-sha "${ARCHIVE_SHA}" \
    --archive-kind "${ARCHIVE_KIND}" \
    --notes "${PROVENANCE_NOTES}"
fi

echo "[swift-fixtures] done"
