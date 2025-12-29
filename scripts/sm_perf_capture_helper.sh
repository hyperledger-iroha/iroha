#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/sm_perf_capture_helper.sh [--matrix] [--output DIR] [--cpu-label LABEL]

Prints the commands required to capture SM performance baselines for the
current machine. When --matrix is supplied the helper also writes a
capture_commands.sh script and a capture_plan.json manifest under the
specified --output directory so lab runs can be scheduled and reproduced.
Use --cpu-label to stamp captures with a human-friendly identifier (e.g.
"neoverse-n2-b01" or "m3pro-rosetta") that will flow into the plan/commands.
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

matrix_mode=no
output_dir=
cpu_label=
while (($#)); do
    case "$1" in
        --matrix)
            matrix_mode=yes
            shift
            ;;
        --output)
            output_dir=${2:?"--output requires a path"}
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

if [[ "$matrix_mode" == yes && -z "$output_dir" ]]; then
    echo "--output is required when --matrix is specified" >&2
    exit 1
fi

# Derive host triple so we suggest the correct baseline filenames.
if host_triple=$(rustc -Vv 2>/dev/null | awk '/^host:/ {print $2}'); then
    host_arch=${host_triple%%-*}
    host_os=${host_triple#*-}
    host_triple_raw=$host_triple
else
    host_arch=$(uname -m)
    host_os=$(uname -s)
    host_triple_raw="${host_arch}-${host_os}"
fi

host_arch=${host_arch//arm64/aarch64}

case "$host_os" in
    apple-darwin*|*-apple-darwin*)
        host_os_token=macos
        ;;
    unknown-linux-gnu*|*-unknown-linux-gnu*)
        host_os_token=unknown_linux_gnu
        ;;
    *linux*)
        host_os_token=linux
        ;;
    *darwin*)
        host_os_token=macos
        ;;
    *)
        host_os_token=$(printf '%s' "$host_os" | tr '[:upper:]' '[:lower:]')
        host_os_token=${host_os_token//-/_}
        host_os_token=${host_os_token//./_}
        ;;
esac

# Compute recommended capture modes and cargo feature sets.
declare -a modes
declare -a capture_modes=()
declare -a capture_baselines=()
declare -a capture_raw_paths=()
declare -a capture_env_strings=()

modes=(scalar auto)
has_neon=no
if [[ "$host_arch" == "aarch64" ]]; then
    has_neon=yes
    modes+=(neon-force)
fi

cat <<EOF
======================================================================
SM performance baseline capture helper
======================================================================
Detected host triple : ${host_arch}-${host_os_token}

This helper prints the commands required to record Criterion medians for
the current machine so SM-4c.1 (cross-architecture perf gating) can be
completed. Run the snippets from the repository root on the target
hardware. Each mode will:
  1. Execute the Criterion benchmark suite.
  2. Write a baseline JSON to crates/iroha_crypto/benches/.

Before running the commands:
  * Set an informative CPU label (e.g. SM_PERF_CPU_LABEL="m3-pro-16c").
    ${cpu_label:+(will inject SM_PERF_CPU_LABEL="${cpu_label}")}
  * Ensure the workspace is clean and that rustup toolchain matches CI.
  * Consider clearing previous Criterion state: rm -rf target/criterion/sm_perf*

EOF

for mode in "${modes[@]}"; do
    features="sm"
    env_prefix=""
    case "$mode" in
        scalar)
            if [[ "$has_neon" == yes ]]; then
                features="sm sm-neon"
            fi
            env_prefix="CRYPTO_SM_INTRINSICS=force-disable"
            ;;
        auto)
            if [[ "$has_neon" == yes ]]; then
                features="sm sm-neon"
            fi
            env_prefix="CRYPTO_SM_INTRINSICS=auto"
            ;;
        neon-force)
            features="sm sm-neon sm-neon-force"
            env_prefix="CRYPTO_SM_INTRINSICS=force-enable"
            ;;
    esac
    baseline_path="crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_${host_os_token}_${mode//-/_}.json"

    cat <<EOF
----------------------------------------------------------------------
Mode: ${mode}
Baseline path: ${baseline_path}
EOF

    if [[ -n "${env_prefix}" ]]; then
        printf 'Env flags   : %s\n' "${env_prefix}"
    fi

    printf 'Cargo features: %s\n\n' "${features}"

    cat <<EOF
# 1) Run the Criterion suite for this mode
${env_prefix:+${env_prefix} }cargo bench -p iroha_crypto --bench sm_perf --features "${features}" -- --noplot

# 2) Record the medians into the architecture-specific baseline
${env_prefix:+${env_prefix} }cargo run -p iroha_crypto --bin sm_perf_check --features "${features}" -- \
  --criterion-dir target/criterion \
  --baseline crates/iroha_crypto/benches/sm_perf_baseline.json \
  --write-baseline "${baseline_path}" \
  --tolerance 0.25

# Optional: compare against an existing scalar baseline once captured
# ${env_prefix:+${env_prefix} }cargo run -p iroha_crypto --bin sm_perf_check --features "${features}" -- \\
#   --criterion-dir target/criterion \\
#   --baseline "${baseline_path}" \\
#   --compare-baseline crates/iroha_crypto/benches/sm_perf_baseline_${host_arch}_${host_os_token}_scalar.json \\
#   --compare-tolerance 0.10


EOF

    if [[ "$matrix_mode" == yes ]]; then
        sanitized_mode=${mode//-/_}
        raw_path="${output_dir%/}/raw/${sanitized_mode}.json"
        capture_modes+=("$mode")
        capture_baselines+=("$baseline_path")
        capture_raw_paths+=("$raw_path")
        env_entry="SM_PERF_CAPTURE_LABEL=${mode}"
        if [[ -n "${cpu_label}" ]]; then
            env_entry+=";SM_PERF_CPU_LABEL=${cpu_label}"
        fi
        if [[ -n "${env_prefix}" ]]; then
            env_entry+=";${env_prefix}"
        fi
        capture_env_strings+=("$env_entry")
    fi
done

if [[ "$matrix_mode" == yes ]]; then
    mkdir -p "${output_dir%/}/raw"
    commands_path="${output_dir%/}/capture_commands.sh"
cat >"$commands_path" <<EOF
#!/usr/bin/env bash
set -euo pipefail

cd "$REPO_ROOT"

echo "Remember to export SM_PERF_CPU_LABEL=<label-for-host> before running captures."
${cpu_label:+export SM_PERF_CPU_LABEL="${cpu_label}"}

EOF

    for idx in "${!capture_modes[@]}"; do
        mode="${capture_modes[$idx]}"
        raw="${capture_raw_paths[$idx]}"
        env_chain="${capture_env_strings[$idx]}"
        IFS=';' read -r -a env_parts <<< "$env_chain"
        env_prefix_cmd=""
        for part in "${env_parts[@]}"; do
            [[ -z "$part" ]] && continue
            if [[ -z "$env_prefix_cmd" ]]; then
                env_prefix_cmd="$part"
            else
                env_prefix_cmd+=" $part"
            fi
        done
        printf '%s scripts/sm_perf.sh --mode %s --capture-json %q\n\n' \
            "$env_prefix_cmd" "$mode" "$raw" >>"$commands_path"
    done
    chmod +x "$commands_path"

    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    raw_dir="${output_dir%/}/raw"
    plan_path="${output_dir%/}/capture_plan.json"
    plan_entries=()
    for idx in "${!capture_modes[@]}"; do
        plan_entries+=("${capture_modes[$idx]}|${capture_baselines[$idx]}|${capture_raw_paths[$idx]}|${capture_env_strings[$idx]}")
    done
    plan_payload=$(printf '%s\n' "${plan_entries[@]}")
    PLAN_ENTRIES="$plan_payload" python3 - "$plan_path" "$timestamp" "$host_arch" "$host_os_token" "$host_triple_raw" "${output_dir%/}" "$raw_dir" "$commands_path" <<'PY'
import json
import os
import sys

plan_path, timestamp, host_arch, host_os, host_triple, output_dir, raw_dir, commands_path = sys.argv[1:9]
entries = []
payload = os.environ.get("PLAN_ENTRIES", "")
for line in payload.splitlines():
    if not line.strip():
        continue
    try:
        mode, baseline, raw, env_chain = line.split("|", 3)
    except ValueError:
        continue
    env_values = [segment for segment in env_chain.split(";") if segment]
    entries.append(
        {
            "mode": mode,
            "baseline_path": baseline,
            "raw_output": raw,
            "env": env_values,
            "command": [
                "scripts/sm_perf.sh",
                "--mode",
                mode,
                "--capture-json",
                raw,
            ],
        }
    )

plan = {
    "generated_at": timestamp,
    "host": {
        "arch": host_arch,
        "os_token": host_os,
        "detected_triple": host_triple,
        "cpu_label": os.environ.get("SM_PERF_CPU_LABEL", ""),
    },
    "output_dir": output_dir,
    "raw_dir": raw_dir,
    "commands_script": commands_path,
    "captures": entries,
}

with open(plan_path, "w", encoding="utf-8") as handle:
    json.dump(plan, handle, indent=2)
    handle.write("\n")
PY

    echo
    echo "Matrix assets written to ${output_dir%/}"
    echo "  Commands: $commands_path"
    echo "  Plan:     $plan_path"
fi

cat <<'EOF'
Notes:
  * Capture the scalar baseline first so the optional comparison step works.
  * After generating the JSON files, commit them together with the benchmark
    logs and update docs/source/crypto/sm_program.md with the recorded medians.
  * If the platform lacks NEON (e.g. x86_64), skip the neon-force commands.

Happy benchmarking!
EOF
