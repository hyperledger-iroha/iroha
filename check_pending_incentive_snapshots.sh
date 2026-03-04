#!/bin/sh
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: check_pending_incentive_snapshots.sh [--dir DIR] [--relay HEX] [--details]

Validate incentive snapshot artefacts emitted by the SoraNet relay runtime.

Options:
  --dir DIR     Directory to scan (defaults to artifacts/incentives).
  --relay HEX   Restrict the report to a specific relay id (hex without 0x).
  --details     Print the per-file summary after the aggregated report.
USAGE
}

SPOOL_DIR="artifacts/incentives"
FILTER_RELAY=""
PRINT_DETAILS=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dir)
      [ "$#" -ge 2 ] || { echo "missing value for --dir" >&2; usage; exit 2; }
      SPOOL_DIR="$2"
      shift 2
      ;;
    --dir=*)
      SPOOL_DIR="${1#--dir=}"
      shift
      ;;
    --relay)
      [ "$#" -ge 2 ] || { echo "missing value for --relay" >&2; usage; exit 2; }
      FILTER_RELAY="${2##0x}"
      FILTER_RELAY="${FILTER_RELAY,,}"
      shift 2
      ;;
    --relay=*)
      FILTER_RELAY="${1#--relay=}"
      FILTER_RELAY="${FILTER_RELAY##0x}"
      FILTER_RELAY="${FILTER_RELAY,,}"
      shift
      ;;
    --details)
      PRINT_DETAILS=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [ ! -d "$SPOOL_DIR" ]; then
  echo "no incentive snapshots found (directory '$SPOOL_DIR' missing)" >&2
  exit 1
fi

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required for snapshot inspection" >&2
  exit 1
}

export SPOOL_DIR="$SPOOL_DIR"
export FILTER_RELAY="$FILTER_RELAY"
export PRINT_DETAILS="$PRINT_DETAILS"

python3 <<'PYTHON'
from __future__ import annotations

import json
import os
import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple

spool_dir = Path(os.environ.get("SPOOL_DIR", "artifacts/incentives"))
filter_relay = os.environ.get("FILTER_RELAY", "")
print_details = os.environ.get("PRINT_DETAILS", "0") == "1"

pattern = re.compile(
    r"relay-(?P<relay>[0-9a-fA-F]+)-epoch-(?P<epoch>\d+)-(?P<stamp>\d+)(?:-(?P<dup>\d+))?\.to$"
)


@dataclass
class Snapshot:
    path: Path
    relay: str
    epoch: int
    reason: str
    timestamp: int
    duplicate_index: Optional[int]


def classify_reason(payload: bytes) -> str:
    text = payload.decode("utf-8", errors="ignore")
    if "measurement" in text:
        return "measurement"
    if "uptime" in text:
        return "uptime"
    return "unknown"


snapshots: List[Snapshot] = []

for entry in sorted(spool_dir.glob("relay-*-epoch-*")):
    if not entry.is_file():
        continue
    match = pattern.match(entry.name)
    if not match:
        continue
    relay = match.group("relay").lower()
    if filter_relay and relay != filter_relay:
        continue
    epoch = int(match.group("epoch"))
    stamp = int(match.group("stamp"))
    duplicate = match.group("dup")
    payload = entry.read_bytes()
    reason = classify_reason(payload)
    snapshots.append(
        Snapshot(
            path=entry,
            relay=relay,
            epoch=epoch,
            reason=reason,
            timestamp=stamp,
            duplicate_index=int(duplicate) if duplicate else None,
        )
    )

if not snapshots:
    print("No incentive snapshots found for the selected filter.")
    sys.exit(1)

summary: Dict[str, Dict[int, Dict[str, List[Snapshot]]]] = defaultdict(
    lambda: defaultdict(lambda: defaultdict(list))
)

for snap in snapshots:
    summary[snap.relay][snap.epoch][snap.reason].append(snap)

missing = []
for relay, epochs in summary.items():
    for epoch, reasons in epochs.items():
        if "uptime" not in reasons:
            missing.append((relay, epoch, "uptime"))
        if "measurement" not in reasons:
            missing.append((relay, epoch, "measurement"))

print(json.dumps({
    "spool_dir": str(spool_dir),
    "relays": {
        relay: {
            str(epoch): sorted(reasons.keys())
            for epoch, reasons in sorted(epochs.items())
        }
        for relay, epochs in sorted(summary.items())
    },
    "missing": [
        {"relay": relay, "epoch": epoch, "expected": expected}
        for relay, epoch, expected in missing
    ],
}, indent=2))

if print_details:
    print("\nDetailed snapshots:")
    for snap in snapshots:
        dup = f"-{snap.duplicate_index}" if snap.duplicate_index is not None else ""
        print(
            f"relay={snap.relay} epoch={snap.epoch} reason={snap.reason} "
            f"timestamp={snap.timestamp}{dup} path={snap.path}"
        )

if missing:
    sys.exit(2)
PYTHON

PYTHON=$?
if [ "$PYTHON" -eq 0 ]; then
  exit 0
else
  exit "$PYTHON"
fi
