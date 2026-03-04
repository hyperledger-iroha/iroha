#!/usr/bin/env bash

set -euo pipefail

LOG_FILE=${1:-ops/drill-log.md}

if [[ ! -f "${LOG_FILE}" ]]; then
    echo "Error: drill log not found at ${LOG_FILE}" >&2
    exit 1
fi

python3 - <<'PYCODE' "${LOG_FILE}"
import sys
from pathlib import Path

ALLOWED_STATUSES = {"pass", "fail", "follow-up", "scheduled"}
EXPECTED_COLUMNS = [
    "Date",
    "Scenario",
    "Status",
    "Incident Commander",
    "Scribe",
    "Start (UTC)",
    "End (UTC)",
    "Notes",
    "Follow-up / Incident Link",
]

log_path = Path(sys.argv[1])
lines = log_path.read_text(encoding="utf-8").splitlines()
table_rows = [line.strip() for line in lines if line.strip().startswith("|")]

if len(table_rows) < 2:
    raise SystemExit("drill log must contain header and separator rows.")

header = table_rows[0]
columns = [col.strip() for col in header.strip("|").split("|")]
if columns != EXPECTED_COLUMNS:
    raise SystemExit(
        "drill log header mismatch.\n"
        f"Expected: {EXPECTED_COLUMNS}\n"
        f"Found:    {columns}"
    )

entries = table_rows[2:]
for idx, row in enumerate(entries, start=1):
    parts = [col.strip() for col in row.strip("|").split("|")]
    if len(parts) != len(EXPECTED_COLUMNS):
        raise SystemExit(
            f"row {idx} has {len(parts)} columns; expected {len(EXPECTED_COLUMNS)}.\n"
            f"Row content: {row}"
        )
    status = parts[2].lower()
    if status not in ALLOWED_STATUSES:
        raise SystemExit(
            f"row {idx} has invalid status '{parts[2]}'. "
            f"Allowed: {', '.join(sorted(ALLOWED_STATUSES))}"
        )

print(f"Drill log OK: {len(entries)} entries validated.")
PYCODE
