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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --date)
      DATE_OVERRIDE=$2
      shift 2
      ;;
    --start)
      START_TIME=$2
      shift 2
      ;;
    --log)
      LOG_PATH=$2
      shift 2
      ;;
    --ic)
      IC_NAME=$2
      shift 2
      ;;
    --scribe)
      SCRIBE_NAME=$2
      shift 2
      ;;
    --notes)
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

if [[ -z "${DATE_OVERRIDE}" ]]; then
  read -r DATE_OVERRIDE QUARTER_LABEL < <(
    python3 - <<'PY'
from datetime import date, timedelta

today = date.today()
next_quarter_month = ((today.month - 1) // 3 + 1) * 3 + 1
year = today.year
if next_quarter_month > 12:
    next_quarter_month -= 12
    year += 1

first_day = date(year, next_quarter_month, 1)
# Thursday = 3
while first_day.weekday() != 3:
    first_day += timedelta(days=1)

quarter = (next_quarter_month - 1) // 3 + 1
print(first_day.isoformat(), f"Q{quarter} {year}")
PY
  )
else
  # Derive quarter label from custom date
  QUARTER_LABEL=$(CUSTOM_DATE="${DATE_OVERRIDE}" python3 - <<'PY'
from datetime import datetime
import os

raw = os.environ["CUSTOM_DATE"]
dt = datetime.strptime(raw, "%Y-%m-%d")
quarter = (dt.month - 1) // 3 + 1
print(f"Q{quarter} {dt.year}")
PY
fi

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
