#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/sm_perf_quarterly.sh [--output-root DIR] [--cpu-label LABEL] [--owner NAME] [--quarter YYYY-QN]

Generate a quarterly SM performance capture plan under the given output root
and wrap the matrix helper with owner + quarter metadata. The script emits:
  - capture_commands.sh   — runnable commands for each mode (scalar/auto/neon-force)
  - capture_plan.json     — matrix metadata from sm_perf_capture_helper.sh
  - quarterly_plan.json   — capture_plan.json plus owner/quarter host metadata

Examples:
  scripts/sm_perf_quarterly.sh --cpu-label neoverse-n2-b01 --owner "Perf WG"
  scripts/sm_perf_quarterly.sh --output-root artifacts/sm_perf/lab --quarter 2026-Q3
EOF
}

output_root="artifacts/sm_perf"
cpu_label=""
owner="${SM_PERF_OWNER:-}"
quarter_override=""

while (($#)); do
    case "$1" in
        --output-root)
            output_root=${2:?"--output-root requires a path"}
            shift 2
            ;;
        --cpu-label)
            cpu_label=${2:?"--cpu-label requires a value"}
            shift 2
            ;;
        --owner)
            owner=${2:?"--owner requires a value"}
            shift 2
            ;;
        --quarter)
            quarter_override=${2:?"--quarter requires a value like 2026-Q3"}
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "$owner" ]]; then
    owner=$(git config user.name 2>/dev/null || true)
fi
if [[ -z "$owner" ]]; then
    owner="unassigned"
fi

if [[ -n "$quarter_override" ]]; then
    quarter_label=$quarter_override
else
    year=$(date +%Y)
    month=$(date +%m)
    q=$(( (10#$month - 1) / 3 + 1 ))
    quarter_label="${year}-Q${q}"
fi

hostname_token=$(hostname -s 2>/dev/null || hostname 2>/dev/null || echo "host")
host_label=${cpu_label:-$hostname_token}
host_label=${host_label// /-}
host_label=${host_label//[^A-Za-z0-9._-]/-}

run_dir="${output_root%/}/${quarter_label}/${host_label}"
mkdir -p "$run_dir"

echo "Generating capture plan in ${run_dir} (owner=${owner}, quarter=${quarter_label})"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
helper="${script_dir}/sm_perf_capture_helper.sh"

if [[ ! -x "$helper" ]]; then
    echo "missing helper at ${helper}" >&2
    exit 1
fi

helper_args=(--matrix --output "$run_dir")
if [[ -n "$cpu_label" ]]; then
    helper_args+=(--cpu-label "$cpu_label")
fi

"$helper" "${helper_args[@]}"

capture_plan="${run_dir%/}/capture_plan.json"
commands_path="${run_dir%/}/capture_commands.sh"

if [[ ! -f "$capture_plan" ]]; then
    echo "capture_plan.json not found at ${capture_plan}; helper may have failed" >&2
    exit 1
fi

python3 - "$capture_plan" "$owner" "$quarter_label" "$run_dir" "$host_label" "$commands_path" <<'PY'
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

plan_path = Path(sys.argv[1])
owner = sys.argv[2]
quarter = sys.argv[3]
run_dir = sys.argv[4]
host_label = sys.argv[5]
commands_path = sys.argv[6]

with plan_path.open("r", encoding="utf-8") as handle:
    plan = json.load(handle)

plan["quarterly_owner"] = owner
plan["quarter"] = quarter
plan["run_directory"] = run_dir
plan["host_label"] = host_label
plan["scheduled_at"] = datetime.now(timezone.utc).isoformat()
plan["commands_script"] = commands_path

output_path = plan_path.with_name("quarterly_plan.json")
with output_path.open("w", encoding="utf-8") as handle:
    json.dump(plan, handle, indent=2)
    handle.write("\n")

print(f"Wrote quarterly plan to {output_path}")
PY

echo "Run the generated commands in ${commands_path} on the target host and promote the baselines afterward."
