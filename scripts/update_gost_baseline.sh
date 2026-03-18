#!/usr/bin/env bash
# Regenerate the GOST perf baseline after validating new measurements.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/update_gost_baseline.sh [--tolerance FRACTION] [--baseline PATH]

Runs the standard GOST bench + check workflow, rebases the baseline JSON, updates
status/roadmap references, and lists the median table so you can copy it into the
release notes/commit message.
EOF
}

tolerance=0.20
baseline=crates/iroha_crypto/benches/gost_perf_baseline.json
while (($#)); do
    case "$1" in
        --tolerance)
            tolerance=${2:?'--tolerance needs a value'}
            shift 2
            ;;
        --baseline)
            baseline=${2:?'--baseline needs a value'}
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

# Run benches and export new baseline
./scripts/gost_bench.sh --write-baseline --tolerance "$tolerance" --baseline "$baseline"

# Show the updated table for easy copy/paste
python3 - <<'PY'
import json
from pathlib import Path
baseline = Path("crates/iroha_crypto/benches/gost_perf_baseline.json")
algorithms = json.loads(baseline.read_text())["algorithms"]
print("### Updated GOST baseline\n")
print("| Algorithm | Median (µs) |")
print("|-----------|-------------|")
for name, value in algorithms.items():
    print(f"| {name} | {value:.2f} |")
PY
