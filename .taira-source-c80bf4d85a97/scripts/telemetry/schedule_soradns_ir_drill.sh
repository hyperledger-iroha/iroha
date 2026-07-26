#!/usr/bin/env bash

set -euo pipefail

print_usage() {
  cat <<'USAGE'
Usage: schedule_soradns_ir_drill.sh [options]

Automatically appends a "scheduled" entry to ops/drill-log.md for the next
SoraDNS transparency incident response drill. By default the drill is scheduled
for the first Thursday of the next quarter at 14:00 UTC and uses the
Transparency Oncall roster.

Options:
  --date YYYY-MM-DD    Override the scheduled date (UTC).
  --start HH:MMZ       Override the scheduled start time (default: 14:00Z).
  --log PATH           Override drill log path (default: ops/drill-log.md).
  --ic NAME            Incident commander (default: Transparency Oncall).
  --scribe NAME        Scribe (default: Ops Bot).
  --notes TEXT         Additional notes to append.
  --help               Show this message.
USAGE
}

DATE_OVERRIDE=""
START_TIME="14:00Z"
LOG_PATH="ops/drill-log.md"
IC_NAME="Transparency Oncall"
SCRIBE_NAME="Ops Bot"
NOTES="Quarterly transparency rehearsal (scheduled automatically)."

require_option_value() {
  local option="$1"
  if [[ $# -lt 2 || -z "$2" || "$2" == --* ]]; then
    echo "Error: ${option} requires a value." >&2
    exit 1
  fi
}

derive_schedule_fields() {
  local date_override="$1"
  local start_time="$2"
  python3 - "${date_override}" "${start_time}" <<'PY'
from __future__ import annotations
from datetime import date, datetime, timedelta, timezone
import re
import sys

date_override, start_time = sys.argv[1:]
date_re = re.compile(r"^\d{4}-\d{2}-\d{2}$")
time_re = re.compile(r"^\d{2}:\d{2}Z$")


def fail(message: str) -> None:
    raise SystemExit(f"Error: {message}")


def parse_schedule_date(raw: str) -> date:
    if raw:
        if not date_re.match(raw):
            fail(f"--date must use YYYY-MM-DD; got {raw!r}.")
        try:
            return date.fromisoformat(raw)
        except ValueError:
            fail(f"--date is not a valid calendar date: {raw!r}.")

    today = datetime.now(timezone.utc).date()
    next_quarter_month = ((today.month - 1) // 3 + 1) * 3 + 1
    year = today.year
    if next_quarter_month > 12:
        next_quarter_month -= 12
        year += 1

    scheduled = date(year, next_quarter_month, 1)
    # Thursday = 3
    while scheduled.weekday() != 3:
        scheduled += timedelta(days=1)
    return scheduled


def validate_start_time(raw: str) -> None:
    if not time_re.match(raw):
        fail(f"--start must use HH:MMZ; got {raw!r}.")
    hour, minute = map(int, raw[:-1].split(":"))
    if hour > 23 or minute > 59:
        fail(f"--start is not a valid UTC time: {raw!r}.")


validate_start_time(start_time)
scheduled_date = parse_schedule_date(date_override)
quarter = (scheduled_date.month - 1) // 3 + 1
print(scheduled_date.isoformat(), f"Q{quarter} {scheduled_date.year}")
PY
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --date)
      require_option_value "$1" "${2-}"
      DATE_OVERRIDE=$2
      shift 2
      ;;
    --start)
      require_option_value "$1" "${2-}"
      START_TIME=$2
      shift 2
      ;;
    --log)
      require_option_value "$1" "${2-}"
      LOG_PATH=$2
      shift 2
      ;;
    --ic)
      require_option_value "$1" "${2-}"
      IC_NAME=$2
      shift 2
      ;;
    --scribe)
      require_option_value "$1" "${2-}"
      SCRIBE_NAME=$2
      shift 2
      ;;
    --notes)
      require_option_value "$1" "${2-}"
      NOTES=$2
      shift 2
      ;;
    --help|-h)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      print_usage
      exit 1
      ;;
  esac
done

schedule_fields="$(derive_schedule_fields "${DATE_OVERRIDE}" "${START_TIME}")"
read -r DATE_OVERRIDE QUARTER_LABEL <<< "${schedule_fields}"

SCENARIO="SoraDNS transparency IR drill (${QUARTER_LABEL})"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_SCRIPT="${SCRIPT_DIR}/log_sorafs_drill.sh"

if [[ ! -x "${LOG_SCRIPT}" ]]; then
  echo "Error: helper script log_sorafs_drill.sh not found." >&2
  exit 1
fi

"${LOG_SCRIPT}" \
  --scenario "${SCENARIO}" \
  --status scheduled \
  --date "${DATE_OVERRIDE}" \
  --start "${START_TIME}" \
  --ic "${IC_NAME}" \
  --scribe "${SCRIBE_NAME}" \
  --notes "${NOTES}" \
  --log "${LOG_PATH}"

echo "Scheduled SoraDNS IR drill (${SCENARIO}) on ${DATE_OVERRIDE} at ${START_TIME}"
