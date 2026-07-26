#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/sm_perf.sh [--write-baseline] [--tolerance FRACTION]
       [--baseline PATH] [--criterion-dir PATH] [--mode MODE]
       [--compare-baseline PATH] [--compare-tolerance FRACTION]
       [--compare-label LABEL] [--capture-json PATH]
       [--cpu-label STRING]

Runs the SM Criterion benchmark suite (SM2/SM3/SM4 vs baseline algorithms) with
the repo-standard settings, then verifies performance against the recorded
medians. Pass --write-baseline to refresh
crates/iroha_crypto/benches/sm_perf_baseline.json after manual review. Use
--tolerance to override the default 25% guard, --baseline to point at an
alternate baseline JSON, and --criterion-dir if Criterion output lives
elsewhere. Provide SM_PERF_CPU_LABEL to stamp the baseline with a CPU label
when writing. --mode toggles the acceleration path:

  auto       - Allow runtime detection (default, CRYPTO_SM_INTRINSICS=auto).
  scalar     - Force scalar fallback via CRYPTO_SM_INTRINSICS=force-disable.
  neon       - Require NEON support but honour runtime detection.
  neon-force - Build with sm-neon-force and force intrinsics on
               (CRYPTO_SM_INTRINSICS=force-enable).
When no comparison baseline is specified the script will attempt to compare
against the scalar baseline on acceleration modes and enforce the configured
comparison tolerance.

Add --capture-json PATH to emit a structured summary for aggregation; combine it
with SM_PERF_CAPTURE_LABEL to stamp host-specific identifiers. When capturing or
writing baselines set --cpu-label (or SM_PERF_CPU_LABEL) so metadata records the
exact host.
EOF
}

format_percent() {
    local value=${1:-0}
    awk -v pct="$value" 'BEGIN { printf "%.2f%%", pct * 100 }'
}

write_baseline=no
tolerance=0.25
baseline=crates/iroha_crypto/benches/sm_perf_baseline.json
criterion_dir=target/criterion
mode=auto
baseline_set=no
compare_baseline=
compare_baseline_set=no
compare_label=
compare_tolerance=0.10
compare_tolerance_set=no
capture_json=
cpu_label=
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
            baseline_set=yes
            shift 2
            ;;
        --criterion-dir)
            criterion_dir=${2:?"--criterion-dir requires a path"}
            shift 2
            ;;
        --mode)
            mode=${2:?"--mode requires a value"}
            shift 2
            ;;
        --compare-baseline)
            compare_baseline=${2:?"--compare-baseline requires a path"}
            compare_baseline_set=yes
            shift 2
            ;;
        --compare-label)
            compare_label=${2:?"--compare-label requires a value"}
            shift 2
            ;;
        --compare-tolerance)
            compare_tolerance=${2:?"--compare-tolerance requires a value"}
            compare_tolerance_set=yes
            shift 2
            ;;
        --capture-json)
            capture_json=${2:?"--capture-json requires a path"}
            shift 2
            ;;
        --cpu-label)
            cpu_label=${2:?"--cpu-label requires a value"}
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

features=(sm sm-neon)
bench_env=()

case "$mode" in
    auto)
        bench_env+=(CRYPTO_SM_INTRINSICS=auto)
        ;;
    scalar)
        bench_env+=(CRYPTO_SM_INTRINSICS=force-disable)
        ;;
    neon)
        bench_env+=(CRYPTO_SM_INTRINSICS=auto)
        ;;
    neon-force)
        features+=(sm-neon-force)
        bench_env+=(CRYPTO_SM_INTRINSICS=force-enable)
        ;;
    *)
        echo "unknown mode: $mode" >&2
        usage >&2
        exit 1
        ;;
esac

feature_flag=$(IFS=' '; echo "${features[*]}")

if [[ -n "$cpu_label" ]]; then
    export SM_PERF_CPU_LABEL="$cpu_label"
fi

if host_triple=$(rustc -Vv 2>/dev/null | awk '/^host:/ {print $2}'); then
    host_arch=${host_triple%%-*}
    host_os_raw=${host_triple#*-}
else
    host_arch=$(uname -m)
    host_os_raw=$(uname -s)
fi
host_arch=${host_arch//arm64/aarch64}
case "$host_os_raw" in
    apple-darwin*|*-apple-darwin*) host_os=macos ;;
    unknown-linux-gnu*|*-unknown-linux-gnu*) host_os=unknown_linux_gnu ;;
    *linux*) host_os=linux ;;
    *darwin*) host_os=macos ;;
    *)
        host_os=$(printf '%s' "$host_os_raw" | tr '[:upper:]' '[:lower:]')
        host_os=${host_os//-/_}
        host_os=${host_os//./_}
        ;;
esac
mode_token=${mode//-/_}

if [[ "$baseline_set" == no ]]; then
    candidates=(
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_${host_os}_${mode_token}.json"
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_${mode_token}.json"
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_os}_${mode_token}.json"
    )
    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            baseline="$candidate"
            break
        fi
    done
fi

if [[ "$compare_baseline_set" == no && "$mode" != "scalar" ]]; then
    candidates=(
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_${host_os}_scalar.json"
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_scalar.json"
        "crates/iroha_crypto/benches/sm_perf_baseline_${host_os}_scalar.json"
    )
    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            compare_baseline="$candidate"
            break
        fi
    done
    if [[ -n "$compare_baseline" ]]; then
        if [[ "$compare_tolerance_set" == no ]]; then
            if [[ "$host_arch" == "aarch64" ]]; then
                compare_tolerance=5.25
            else
                compare_tolerance=0.25
            fi
        fi
        if [[ -z "$compare_label" ]]; then
            compare_label="scalar (${host_arch}/${host_os}) baseline"
        fi
    fi
fi

echo "sm_perf: running mode '$mode' with baseline '$baseline'"
if [[ -n "$compare_baseline" ]]; then
    formatted_compare_tol=$(format_percent "$compare_tolerance")
    echo "sm_perf: comparison baseline '$compare_baseline' (tolerance ${formatted_compare_tol}${compare_label:+, label \"$compare_label\"})"
fi

if [[ -n "$capture_json" ]]; then
    capture_dir=$(dirname -- "$capture_json")
    mkdir -p "$capture_dir"
fi

if ((${#bench_env[@]} > 0)); then
    env "${bench_env[@]}" cargo bench -p iroha_crypto --bench sm_perf --features "$feature_flag" -- --noplot
else
    cargo bench -p iroha_crypto --bench sm_perf --features "$feature_flag" -- --noplot
fi

resolve_criterion_dir() {
    local candidates=(
        "$criterion_dir"
        "crates/iroha_crypto/$criterion_dir"
        "crates/iroha_crypto/target-codex/criterion"
        "target-codex/criterion"
    )
    for dir in "${candidates[@]}"; do
        if [[ -d "$dir" ]]; then
            printf '%s' "$dir"
            return
        fi
    done
    printf '%s' "$criterion_dir"
}

detected_criterion_dir=$(resolve_criterion_dir)

summary_args=(
    --criterion-dir "$detected_criterion_dir"
    --baseline "$baseline"
    --tolerance "$tolerance"
)

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    summary_args+=(--summary-only)
fi

if [[ "$write_baseline" == yes ]]; then
    summary_args+=(--write-baseline "$baseline")
fi
if [[ -n "$compare_baseline" ]]; then
    summary_args+=(--compare-baseline "$compare_baseline")
    summary_args+=(--compare-tolerance "$compare_tolerance")
    if [[ -n "$compare_label" ]]; then
        summary_args+=(--compare-label "$compare_label")
    fi
fi

if [[ -n "$capture_json" ]]; then
    summary_args+=(--capture-json "$capture_json")
    summary_args+=(--capture-mode "$mode")
    if [[ -n "${SM_PERF_CAPTURE_LABEL:-}" ]]; then
        summary_args+=(--capture-label "${SM_PERF_CAPTURE_LABEL}")
    fi
fi

if ((${#bench_env[@]} > 0)); then
    env "${bench_env[@]}" cargo run -p iroha_crypto --bin sm_perf_check --features "$feature_flag" -- "${summary_args[@]}"
else
    cargo run -p iroha_crypto --bin sm_perf_check --features "$feature_flag" -- "${summary_args[@]}"
fi
