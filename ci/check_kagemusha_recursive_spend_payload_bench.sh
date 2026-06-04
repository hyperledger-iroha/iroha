#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-}"
ARTIFACTS_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ARTIFACTS_DIR:-${ROOT_DIR}/artifacts/kagemusha_recursive_spend_payload_bench}"
CRITERION_OUT="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_CRITERION_OUT:-${ROOT_DIR}/target/criterion}"

SAMPLE_SIZE="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_SAMPLE_SIZE:-10}"
WARM_UP_TIME="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_WARM_UP_TIME:-0.1}"
MEASUREMENT_TIME="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_MEASUREMENT_TIME:-0.2}"
MAX_BYTES="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_MAX_BYTES:-2048}"
MAX_TRANSITION_PROFILE_BYTES="${KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_MAX_BYTES:-3072}"
MAX_RESERVED_LINEAGE_BYTES="${KAGEMUSHA_RECURSIVE_SPEND_RESERVED_LINEAGE_PAYLOAD_MAX_BYTES:-8192}"
MAX_RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES="${KAGEMUSHA_RECURSIVE_SPEND_RESERVED_LINEAGE_TRANSITION_PROFILE_MAX_BYTES:-4096}"
EXPECTED_HOPS="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_HOPS:-1,2,3,5,8,13,21,34,55,64}"
SKIP_BENCH="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_SKIP_BENCH:-0}"

if [[ "${MODE}" == "--self-test" || "${MODE}" == --negative-control-* ]]; then
  TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-payload-bench.XXXXXX")"
  trap 'rm -rf "${TMP_DIR}"' EXIT
  NEGATIVE_CRITERION_OUT="${TMP_DIR}/criterion"
  NEGATIVE_ARTIFACTS_DIR="${TMP_DIR}/artifacts"

  python3 - "$NEGATIVE_CRITERION_OUT" "$EXPECTED_HOPS" "$MODE" <<'PY'
import sys
from pathlib import Path

criterion_out = Path(sys.argv[1])
expected_hops = [int(part) for part in sys.argv[2].split(",") if part]
mode = sys.argv[3]

payload_baseline = 1751
transition_profile_baseline = 2094
reserved_lineage_payload_baseline = 3847
reserved_lineage_transition_profile_baseline = 2817

def write_benchmark(group, hop, size):
    path = criterion_out / group / f"{hop}_hops_{size}_bytes" / "new"
    path.mkdir(parents=True, exist_ok=True)
    (path / "benchmark.json").write_text("{}\n", encoding="utf-8")

for hop in expected_hops:
    payload_size = payload_baseline
    transition_profile_size = transition_profile_baseline
    if mode == "--negative-control-payload-baseline":
        payload_size = 2049
    if mode == "--negative-control-payload-growth" and hop == expected_hops[-1]:
        payload_size += 1
    if mode == "--negative-control-transition-profile-growth" and hop == expected_hops[-1]:
        transition_profile_size += 1
    if mode == "--negative-control-transition-profile-baseline":
        transition_profile_size = 3073
    if mode == "--negative-control-reserved-lineage-payload-baseline":
        reserved_lineage_payload_baseline = 8193
    if mode == "--negative-control-reserved-lineage-payload-growth" and hop == expected_hops[-1]:
        reserved_lineage_payload_baseline += 1
    if mode == "--negative-control-reserved-lineage-transition-profile-baseline":
        reserved_lineage_transition_profile_baseline = 4097
    if mode == "--negative-control-reserved-lineage-transition-profile-growth" and hop == expected_hops[-1]:
        reserved_lineage_transition_profile_baseline += 1
    if not (mode == "--negative-control-missing-payload" and hop == expected_hops[-1]):
        write_benchmark("kagemusha_recursive_spend_payload_bytes", hop, payload_size)
    if not (mode == "--negative-control-missing-reserved-lineage-payload" and hop == expected_hops[-1]):
        write_benchmark(
            "kagemusha_recursive_spend_reserved_lineage_payload_bytes",
            hop,
            reserved_lineage_payload_baseline,
        )
    if not (mode == "--negative-control-missing-transition-profile" and hop == expected_hops[-1]):
        write_benchmark(
            "kagemusha_recursive_spend_transition_profile_bytes",
            hop,
            transition_profile_size,
        )
    if not (mode == "--negative-control-missing-reserved-lineage-transition-profile" and hop == expected_hops[-1]):
        write_benchmark(
            "kagemusha_reserved_lineage_transition_profile_bytes",
            hop,
            reserved_lineage_transition_profile_baseline,
        )

unexpected_hop = expected_hops[-1] + 1
if mode == "--negative-control-unexpected-payload-hop":
    write_benchmark("kagemusha_recursive_spend_payload_bytes", unexpected_hop, payload_baseline)
if mode == "--negative-control-unexpected-transition-profile-hop":
    write_benchmark(
        "kagemusha_recursive_spend_transition_profile_bytes",
        unexpected_hop,
        transition_profile_baseline,
    )
if mode == "--negative-control-unexpected-reserved-lineage-payload-hop":
    write_benchmark(
        "kagemusha_recursive_spend_reserved_lineage_payload_bytes",
        unexpected_hop,
        reserved_lineage_payload_baseline,
    )
if mode == "--negative-control-unexpected-reserved-lineage-transition-profile-hop":
    write_benchmark(
        "kagemusha_reserved_lineage_transition_profile_bytes",
        unexpected_hop,
        reserved_lineage_transition_profile_baseline,
    )
if mode == "--negative-control-conflicting-payload-size":
    write_benchmark(
        "kagemusha_recursive_spend_payload_bytes",
        expected_hops[0],
        payload_baseline + 1,
    )
if mode == "--negative-control-conflicting-transition-profile-size":
    write_benchmark(
        "kagemusha_recursive_spend_transition_profile_bytes",
        expected_hops[-1],
        transition_profile_baseline + 1,
    )
if mode == "--negative-control-conflicting-reserved-lineage-payload-size":
    write_benchmark(
        "kagemusha_recursive_spend_reserved_lineage_payload_bytes",
        expected_hops[0],
        reserved_lineage_payload_baseline + 1,
    )
if mode == "--negative-control-conflicting-reserved-lineage-transition-profile-size":
    write_benchmark(
        "kagemusha_reserved_lineage_transition_profile_bytes",
        expected_hops[-1],
        reserved_lineage_transition_profile_baseline + 1,
    )

if mode not in {
    "--self-test",
    "--negative-control-payload-baseline",
    "--negative-control-payload-growth",
    "--negative-control-missing-payload",
    "--negative-control-transition-profile-growth",
    "--negative-control-transition-profile-baseline",
    "--negative-control-missing-transition-profile",
    "--negative-control-reserved-lineage-payload-baseline",
    "--negative-control-reserved-lineage-payload-growth",
    "--negative-control-missing-reserved-lineage-payload",
    "--negative-control-reserved-lineage-transition-profile-growth",
    "--negative-control-reserved-lineage-transition-profile-baseline",
    "--negative-control-missing-reserved-lineage-transition-profile",
    "--negative-control-unexpected-payload-hop",
    "--negative-control-unexpected-transition-profile-hop",
    "--negative-control-unexpected-reserved-lineage-payload-hop",
    "--negative-control-unexpected-reserved-lineage-transition-profile-hop",
    "--negative-control-conflicting-payload-size",
    "--negative-control-conflicting-transition-profile-size",
    "--negative-control-conflicting-reserved-lineage-payload-size",
    "--negative-control-conflicting-reserved-lineage-transition-profile-size",
}:
    raise SystemExit(f"unknown synthetic benchmark mode: {mode}")
PY

  if [[ "${MODE}" == "--self-test" ]]; then
    KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_SKIP_BENCH=1 \
      KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_CRITERION_OUT="${NEGATIVE_CRITERION_OUT}" \
      KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ARTIFACTS_DIR="${NEGATIVE_ARTIFACTS_DIR}" \
      KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_HOPS="${EXPECTED_HOPS}" \
      KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_MAX_BYTES="${MAX_BYTES}" \
      KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_MAX_BYTES="${MAX_TRANSITION_PROFILE_BYTES}" \
      "$0"
    test -s "${NEGATIVE_ARTIFACTS_DIR}/payload_bytes.tsv"
    test -s "${NEGATIVE_ARTIFACTS_DIR}/transition_profile_bytes.tsv"
    test -s "${NEGATIVE_ARTIFACTS_DIR}/reserved_lineage_payload_bytes.tsv"
    test -s "${NEGATIVE_ARTIFACTS_DIR}/reserved_lineage_transition_profile_bytes.tsv"
    test -s "${NEGATIVE_ARTIFACTS_DIR}/summary.txt"
    echo "synthetic Kagemusha payload benchmark reduction passed"
    exit 0
  fi

  if KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_SKIP_BENCH=1 \
    KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_CRITERION_OUT="${NEGATIVE_CRITERION_OUT}" \
    KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ARTIFACTS_DIR="${NEGATIVE_ARTIFACTS_DIR}" \
    KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_HOPS="${EXPECTED_HOPS}" \
    KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_MAX_BYTES="${MAX_BYTES}" \
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_MAX_BYTES="${MAX_TRANSITION_PROFILE_BYTES}" \
    "$0" >"${TMP_DIR}/stdout" 2>"${TMP_DIR}/stderr"; then
    cat "${TMP_DIR}/stdout"
    cat "${TMP_DIR}/stderr" >&2
    echo "negative control failed: payload benchmark drift was not detected" >&2
    exit 1
  fi

  echo "negative control rejected Kagemusha payload benchmark drift"
  grep -v "skipping benchmark run" "${TMP_DIR}/stderr" | head -n 1 || true
  exit 0
fi

if [[ -n "${MODE}" ]]; then
  echo "unknown mode: ${MODE}" >&2
  exit 2
fi

BENCH_SOURCE="${ROOT_DIR}/crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs"
for needle in \
  "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight" \
  "kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract" \
  "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract" \
  "kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile" \
  "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1" \
  "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1" \
  "append_boundary_digest" \
  "previous_recursive_proof_open_envelopes_archive_digest" \
  "append_opening_preflight_digest" \
  "previous_proof_open_envelope_archive"; do
  if ! grep -q "${needle}" "${BENCH_SOURCE}"; then
    echo "Kagemusha payload benchmark must measure append-opening preflight-aware transition profiles: missing ${needle}" >&2
    exit 1
  fi
done

mkdir -p "${ARTIFACTS_DIR}"

SUMMARY_FILE="${ARTIFACTS_DIR}/summary.txt"
PAYLOAD_BYTES_FILE="${ARTIFACTS_DIR}/payload_bytes.tsv"
TRANSITION_PROFILE_BYTES_FILE="${ARTIFACTS_DIR}/transition_profile_bytes.tsv"
RESERVED_LINEAGE_PAYLOAD_BYTES_FILE="${ARTIFACTS_DIR}/reserved_lineage_payload_bytes.tsv"
RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES_FILE="${ARTIFACTS_DIR}/reserved_lineage_transition_profile_bytes.tsv"
BENCH_DIR="${CRITERION_OUT}/kagemusha_recursive_spend_payload_bytes"
TRANSITION_PROFILE_BENCH_DIR="${CRITERION_OUT}/kagemusha_recursive_spend_transition_profile_bytes"
RESERVED_LINEAGE_BENCH_DIR="${CRITERION_OUT}/kagemusha_recursive_spend_reserved_lineage_payload_bytes"
RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR="${CRITERION_OUT}/kagemusha_reserved_lineage_transition_profile_bytes"

if [[ "${SKIP_BENCH}" == "1" ]]; then
  echo "[kagemusha-payload-bench] skipping benchmark run; reducing existing Criterion output in ${CRITERION_OUT}" >&2
else
  echo "[kagemusha-payload-bench] running recursive spend payload benchmark…" >&2
  rm -rf \
    "${BENCH_DIR}" \
    "${TRANSITION_PROFILE_BENCH_DIR}" \
    "${RESERVED_LINEAGE_BENCH_DIR}" \
    "${RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR}"
  cargo bench -p iroha_data_model --bench kagemusha_recursive_spend_payload -- \
    --sample-size "${SAMPLE_SIZE}" \
    --warm-up-time "${WARM_UP_TIME}" \
    --measurement-time "${MEASUREMENT_TIME}"
fi

python3 - "$BENCH_DIR" "$TRANSITION_PROFILE_BENCH_DIR" "$RESERVED_LINEAGE_BENCH_DIR" "$RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR" "$PAYLOAD_BYTES_FILE" "$TRANSITION_PROFILE_BYTES_FILE" "$RESERVED_LINEAGE_PAYLOAD_BYTES_FILE" "$RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES_FILE" "$EXPECTED_HOPS" "$MAX_BYTES" "$MAX_TRANSITION_PROFILE_BYTES" "$MAX_RESERVED_LINEAGE_BYTES" "$MAX_RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES" <<'PY'
import re
import sys
from pathlib import Path

bench_dir = Path(sys.argv[1])
transition_profile_bench_dir = Path(sys.argv[2])
reserved_lineage_bench_dir = Path(sys.argv[3])
reserved_lineage_transition_profile_bench_dir = Path(sys.argv[4])
payload_bytes_file = Path(sys.argv[5])
transition_profile_bytes_file = Path(sys.argv[6])
reserved_lineage_payload_bytes_file = Path(sys.argv[7])
reserved_lineage_transition_profile_bytes_file = Path(sys.argv[8])
expected_hops = [int(part) for part in sys.argv[9].split(",") if part]
max_bytes = int(sys.argv[10])
max_transition_profile_bytes = int(sys.argv[11])
max_reserved_lineage_bytes = int(sys.argv[12])
max_reserved_lineage_transition_profile_bytes = int(sys.argv[13])
pattern = re.compile(r"^(\d+)_hops_(\d+)_bytes$")

def parse_benchmark_dir(label, path):
    if not path.is_dir():
        raise SystemExit(f"criterion {label} benchmark directory is missing: {path}")

    observed = {}
    for benchmark_json in sorted(path.glob("*/new/benchmark.json")):
        benchmark_name = benchmark_json.parent.parent.name
        match = pattern.fullmatch(benchmark_name)
        if match is None:
            continue
        hop_count = int(match.group(1))
        payload_bytes = int(match.group(2))
        previous = observed.setdefault(hop_count, payload_bytes)
        if previous != payload_bytes:
            raise SystemExit(
                f"conflicting recursive Kagemusha {label} sizes for hop {hop_count}: "
                f"{previous} and {payload_bytes}"
            )
    return observed

observed = parse_benchmark_dir("D2D payload", bench_dir)
observed_transition_profiles = parse_benchmark_dir(
    "transition-profile",
    transition_profile_bench_dir,
)
observed_reserved_lineage = parse_benchmark_dir(
    "Reserved-lineage D2D payload",
    reserved_lineage_bench_dir,
)
observed_reserved_lineage_transition_profiles = parse_benchmark_dir(
    "Reserved-lineage transition-profile",
    reserved_lineage_transition_profile_bench_dir,
)

missing = [hop for hop in expected_hops if hop not in observed]
unexpected = sorted(set(observed) - set(expected_hops))
if missing:
    raise SystemExit(f"missing recursive Kagemusha payload benchmarks for hops: {missing}")
if unexpected:
    raise SystemExit(f"unexpected recursive Kagemusha payload benchmark hops: {unexpected}")

missing_profiles = [hop for hop in expected_hops if hop not in observed_transition_profiles]
unexpected_profiles = sorted(set(observed_transition_profiles) - set(expected_hops))
if missing_profiles:
    raise SystemExit(
        f"missing recursive Kagemusha transition-profile benchmarks for hops: {missing_profiles}"
    )
if unexpected_profiles:
    raise SystemExit(
        f"unexpected recursive Kagemusha transition-profile benchmark hops: {unexpected_profiles}"
    )

missing_reserved_lineage = [hop for hop in expected_hops if hop not in observed_reserved_lineage]
unexpected_reserved_lineage = sorted(set(observed_reserved_lineage) - set(expected_hops))
if missing_reserved_lineage:
    raise SystemExit(
        f"missing Reserved-lineage recursive Kagemusha payload benchmarks for hops: "
        f"{missing_reserved_lineage}"
    )
if unexpected_reserved_lineage:
    raise SystemExit(
        "unexpected Reserved-lineage recursive Kagemusha payload benchmark hops: "
        f"{unexpected_reserved_lineage}"
    )

missing_reserved_profiles = [
    hop for hop in expected_hops if hop not in observed_reserved_lineage_transition_profiles
]
unexpected_reserved_profiles = sorted(
    set(observed_reserved_lineage_transition_profiles) - set(expected_hops)
)
if missing_reserved_profiles:
    raise SystemExit(
        "missing Reserved-lineage recursive Kagemusha transition-profile benchmarks for hops: "
        f"{missing_reserved_profiles}"
    )
if unexpected_reserved_profiles:
    raise SystemExit(
        "unexpected Reserved-lineage recursive Kagemusha transition-profile benchmark hops: "
        f"{unexpected_reserved_profiles}"
    )

baseline = observed[expected_hops[0]]
if baseline > max_bytes:
    raise SystemExit(
        f"recursive Kagemusha payload baseline exceeds max bytes: {baseline} > {max_bytes}"
    )
for hop in expected_hops:
    payload_bytes = observed[hop]
    if payload_bytes != baseline:
        raise SystemExit(
            f"recursive Kagemusha D2D payload grew at hop {hop}: "
            f"{payload_bytes} != {baseline}"
        )

append_hops = [hop for hop in expected_hops if hop > 1]
if append_hops:
    transition_baseline = observed_transition_profiles[append_hops[0]]
    if transition_baseline > max_transition_profile_bytes:
        raise SystemExit(
            "recursive Kagemusha append transition profile baseline exceeds max bytes: "
            f"{transition_baseline} > {max_transition_profile_bytes}"
        )
    for hop in append_hops:
        profile_bytes = observed_transition_profiles[hop]
        if profile_bytes != transition_baseline:
            raise SystemExit(
                f"recursive Kagemusha append transition profile grew at hop {hop}: "
                f"{profile_bytes} != {transition_baseline}"
            )

reserved_lineage_baseline = observed_reserved_lineage[expected_hops[0]]
if reserved_lineage_baseline > max_reserved_lineage_bytes:
    raise SystemExit(
        "Reserved-lineage recursive Kagemusha payload baseline exceeds max bytes: "
        f"{reserved_lineage_baseline} > {max_reserved_lineage_bytes}"
    )
for hop in expected_hops:
    payload_bytes = observed_reserved_lineage[hop]
    if payload_bytes != reserved_lineage_baseline:
        raise SystemExit(
            f"Reserved-lineage recursive Kagemusha D2D payload grew at hop {hop}: "
            f"{payload_bytes} != {reserved_lineage_baseline}"
        )

if append_hops:
    reserved_transition_baseline = observed_reserved_lineage_transition_profiles[append_hops[0]]
    if reserved_transition_baseline > max_reserved_lineage_transition_profile_bytes:
        raise SystemExit(
            "Reserved-lineage recursive Kagemusha append transition profile baseline exceeds "
            "max bytes: "
            f"{reserved_transition_baseline} > {max_reserved_lineage_transition_profile_bytes}"
        )
    for hop in append_hops:
        profile_bytes = observed_reserved_lineage_transition_profiles[hop]
        if profile_bytes != reserved_transition_baseline:
            raise SystemExit(
                f"Reserved-lineage recursive Kagemusha append transition profile grew at hop {hop}: "
                f"{profile_bytes} != {reserved_transition_baseline}"
            )

payload_bytes_file.write_text(
    "hop_count\tpayload_bytes\n"
    + "".join(f"{hop}\t{observed[hop]}\n" for hop in expected_hops),
    encoding="utf-8",
)
transition_profile_bytes_file.write_text(
    "hop_count\ttransition_profile_bytes\n"
    + "".join(f"{hop}\t{observed_transition_profiles[hop]}\n" for hop in expected_hops),
    encoding="utf-8",
)
reserved_lineage_payload_bytes_file.write_text(
    "hop_count\treserved_lineage_payload_bytes\n"
    + "".join(f"{hop}\t{observed_reserved_lineage[hop]}\n" for hop in expected_hops),
    encoding="utf-8",
)
reserved_lineage_transition_profile_bytes_file.write_text(
    "hop_count\treserved_lineage_transition_profile_bytes\n"
    + "".join(
        f"{hop}\t{observed_reserved_lineage_transition_profiles[hop]}\n"
        for hop in expected_hops
    ),
    encoding="utf-8",
)
PY

{
  echo "kagemusha_recursive_spend_payload_bytes summary"
  echo "generated_at=$(TZ=UTC date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "sample_size=${SAMPLE_SIZE}"
  echo "warm_up_time=${WARM_UP_TIME}"
  echo "measurement_time=${MEASUREMENT_TIME}"
  echo "skip_bench=${SKIP_BENCH}"
  echo "expected_hops=${EXPECTED_HOPS}"
  echo "max_bytes=${MAX_BYTES}"
  echo "max_transition_profile_bytes=${MAX_TRANSITION_PROFILE_BYTES}"
  echo "max_reserved_lineage_bytes=${MAX_RESERVED_LINEAGE_BYTES}"
  echo "max_reserved_lineage_transition_profile_bytes=${MAX_RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES}"
  echo "payload_bytes_file=${PAYLOAD_BYTES_FILE}"
  echo "transition_profile_bytes_file=${TRANSITION_PROFILE_BYTES_FILE}"
  echo "reserved_lineage_payload_bytes_file=${RESERVED_LINEAGE_PAYLOAD_BYTES_FILE}"
  echo "reserved_lineage_transition_profile_bytes_file=${RESERVED_LINEAGE_TRANSITION_PROFILE_BYTES_FILE}"
  echo "bench=crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs"
  if [[ -d "${BENCH_DIR}" ]]; then
    echo "criterion_dir=${BENCH_DIR}"
    while IFS= read -r -d '' benchmark_json; do
      echo "benchmark_json=${benchmark_json}"
    done < <(find "${BENCH_DIR}" -path '*/new/benchmark.json' -print0 | sort -z)
  else
    echo "criterion_dir_missing=${BENCH_DIR}"
  fi
  if [[ -d "${TRANSITION_PROFILE_BENCH_DIR}" ]]; then
    echo "transition_profile_criterion_dir=${TRANSITION_PROFILE_BENCH_DIR}"
    while IFS= read -r -d '' benchmark_json; do
      echo "transition_profile_benchmark_json=${benchmark_json}"
    done < <(find "${TRANSITION_PROFILE_BENCH_DIR}" -path '*/new/benchmark.json' -print0 | sort -z)
  else
    echo "transition_profile_criterion_dir_missing=${TRANSITION_PROFILE_BENCH_DIR}"
  fi
  if [[ -d "${RESERVED_LINEAGE_BENCH_DIR}" ]]; then
    echo "reserved_lineage_criterion_dir=${RESERVED_LINEAGE_BENCH_DIR}"
    while IFS= read -r -d '' benchmark_json; do
      echo "reserved_lineage_benchmark_json=${benchmark_json}"
    done < <(find "${RESERVED_LINEAGE_BENCH_DIR}" -path '*/new/benchmark.json' -print0 | sort -z)
  else
    echo "reserved_lineage_criterion_dir_missing=${RESERVED_LINEAGE_BENCH_DIR}"
  fi
  if [[ -d "${RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR}" ]]; then
    echo "reserved_lineage_transition_profile_criterion_dir=${RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR}"
    while IFS= read -r -d '' benchmark_json; do
      echo "reserved_lineage_transition_profile_benchmark_json=${benchmark_json}"
    done < <(find "${RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR}" -path '*/new/benchmark.json' -print0 | sort -z)
  else
    echo "reserved_lineage_transition_profile_criterion_dir_missing=${RESERVED_LINEAGE_TRANSITION_PROFILE_BENCH_DIR}"
  fi
} > "${SUMMARY_FILE}"

echo "[kagemusha-payload-bench] summary written to ${SUMMARY_FILE}" >&2
