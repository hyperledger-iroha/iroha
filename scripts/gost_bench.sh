#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/gost_bench.sh [--write-baseline] [--tolerance FRACTION] [--baseline PATH]

Runs the GOST Criterion benchmark (Ed25519, Secp256k1, TC26 curves) with the
repo-standard settings, then verifies performance against the recorded medians.
Pass --write-baseline to refresh crates/iroha_crypto/benches/gost_perf_baseline.json
with the new results (after manual review).
Use --tolerance to override the default 20% guard, and --baseline to point at
an alternate baseline JSON.
EOF
}

write_baseline=no
tolerance=0.20
baseline=crates/iroha_crypto/benches/gost_perf_baseline.json
while (($#)); do
    case "$1" in
        --write-baseline)
            write_baseline=yes
            shift
            ;;
        --tolerance)
            tolerance=${2:?"--tolerance requires a value"}
            shift 2
            ;;
        --baseline)
            baseline=${2:?"--baseline requires a path"}
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

cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot

summary_args=(
    --criterion-dir target/criterion
    --baseline "$baseline"
    --tolerance "$tolerance"
)

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    summary_args+=(--summary-only)
fi

if [[ "$write_baseline" == yes ]]; then
    summary_args+=(--write-baseline "$baseline")
fi

cargo run -p iroha_crypto --bin gost_perf_check --features gost -- "${summary_args[@]}"
