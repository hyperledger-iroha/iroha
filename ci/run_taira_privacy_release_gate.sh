#!/usr/bin/env bash
# Run exactly one authenticated, operator-only privacy release gate and emit bounded evidence.
set -Eeuo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 EVIDENCE_DIR" >&2
  exit 64
fi

evidence_dir="$1"
case_slug="${TAIRA_EVIDENCE_CASE:?TAIRA_EVIDENCE_CASE is required}"
test_filter="${TAIRA_EVIDENCE_FILTER:?TAIRA_EVIDENCE_FILTER is required}"
expected_commit="${TAIRA_INPUT_EXPECTED_COMMIT:?TAIRA_INPUT_EXPECTED_COMMIT is required}"
expected_source_sha="${TAIRA_INPUT_SOURCE_SHA256:?TAIRA_INPUT_SOURCE_SHA256 is required}"
expected_lock_sha="${TAIRA_INPUT_LOCK_SHA256:?TAIRA_INPUT_LOCK_SHA256 is required}"
elapsed_ceiling_ms="${TAIRA_INPUT_ELAPSED_CEILING_MS:?TAIRA_INPUT_ELAPSED_CEILING_MS is required}"
peak_rss_ceiling_bytes="${TAIRA_INPUT_PEAK_RSS_CEILING_BYTES:?TAIRA_INPUT_PEAK_RSS_CEILING_BYTES is required}"
address_space_ceiling_bytes="${TAIRA_INPUT_ADDRESS_SPACE_CEILING_BYTES:?TAIRA_INPUT_ADDRESS_SPACE_CEILING_BYTES is required}"
toolchain_tree_sha="${TAIRA_RUST_TOOLCHAIN_TREE_SHA256:?TAIRA_RUST_TOOLCHAIN_TREE_SHA256 is required}"

case "$case_slug" in
  zk-ams-production-dispatch)
    expected_filter='privacy_verifier::tests::zk_ams_production_dispatch_covers_batch_and_successor_provisioning'
    ;;
  vega-action-api)
    expected_filter='privacy_release_evidence::tests::vega_action_api_binds_signs_and_rejects_transaction_proof_and_statement_drift'
    ;;
  zk-ace-release-stages)
    expected_filter='privacy_release_evidence::tests::zk_ace_release_stages_exercise_the_activatable_profile'
    ;;
  bootle-lantern-release-stage)
    expected_filter='privacy_release_evidence::tests::bootle_lantern_release_stage_exercises_one_shot_issuance_and_wire_rejection'
    ;;
  zk-ams-corruption-stage)
    expected_filter='privacy_release_evidence::tests::zk_ams_corruption_stage_rejects_maximum_and_submaximum_wire_mutations'
    ;;
  zk-ams-complete-batch)
    expected_filter='privacy_engines::zk_ams::tests::complete_batch_admission_proves_verifies_and_fails_closed'
    ;;
  pq-masp-full-domain)
    expected_filter='privacy_engines::pq_masp::stark::tests::full_domain_authorized_facade_roundtrip_and_adversarial_wires_fail_closed'
    ;;
  ivm-private-note-full-domain)
    expected_filter='privacy_engines::ivm_private_note::stark::tests::full_domain_stark_roundtrip_and_adversarial_wires_fail_closed'
    ;;
  *)
    echo "release-gate case is outside the closed inventory" >&2
    exit 64
    ;;
esac

if [[ "$expected_filter" != "$test_filter" ]]; then
  echo "release-gate case and exact test filter do not match the closed inventory" >&2
  exit 64
fi

require_sha256() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9a-f]{64}$ || "$value" = "$(printf '0%.0s' {1..64})" ]]; then
    echo "$label must be one nonzero lowercase SHA-256" >&2
    exit 64
  fi
}

require_decimal() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
    echo "$label must be canonical positive decimal" >&2
    exit 64
  fi
}

if [[ ! "$expected_commit" =~ ^[0-9a-f]{40}$ || "$expected_commit" = "$(printf '0%.0s' {1..40})" ]]; then
  echo "expected commit must be one nonzero lowercase full Git commit" >&2
  exit 64
fi
require_sha256 source_sha256 "$expected_source_sha"
require_sha256 cargo_lock_sha256 "$expected_lock_sha"
require_sha256 rust_toolchain_tree_sha256 "$toolchain_tree_sha"
require_decimal elapsed_ceiling_ms "$elapsed_ceiling_ms"
require_decimal peak_rss_ceiling_bytes "$peak_rss_ceiling_bytes"
require_decimal address_space_ceiling_bytes "$address_space_ceiling_bytes"

if [[ -e "$evidence_dir" || -L "$evidence_dir" ]]; then
  echo "evidence path must be fresh" >&2
  exit 73
fi
umask 077
mkdir -p "$evidence_dir"
evidence_dir="$(cd "$evidence_dir" && pwd -P)"

command -v timeout >/dev/null
test -x /usr/bin/time
test "$(git rev-parse HEAD)" = "$expected_commit"
git diff --quiet
git diff --cached --quiet
test -z "$(git ls-files --others --exclude-standard)"
printf '%s  %s\n' "$expected_lock_sha" Cargo.lock | sha256sum -c -
observed_source_sha="$(python3 -I -S scripts/compute_workspace_source_manifest.py --root "$PWD")"
test "$observed_source_sha" = "$expected_source_sha"

python3 -I -S - <<'PY'
import json
from pathlib import Path

rollout = json.loads(
    Path("configs/soranexus/taira/privacy_rollout_plan_v1.json").read_text(
        encoding="utf-8"
    )
)
entry = next(
    item for item in rollout["protocols"] if item["label"] == "iroha-zk-ams-v1"
)
if entry["assurance"] != "unavailable" or not entry["missing_evidence"]:
    raise SystemExit("ZK-AMS release evidence must not imply activation readiness")
PY

address_space_ceiling_kib="$((address_space_ceiling_bytes / 1024))"
if (( address_space_ceiling_kib == 0 )); then
  echo "address-space ceiling is below one KiB" >&2
  exit 64
fi
ulimit -v "$address_space_ceiling_kib"
timeout_seconds="$(((elapsed_ceiling_ms + 999) / 1000))"

test_log="$evidence_dir/test.log"
resource_log="$evidence_dir/resources.txt"
rustc -Vv >"$evidence_dir/rustc.txt"
cargo -Vv >"$evidence_dir/cargo.txt"

command=(
  cargo test
  --locked
  --release
  -p iroha_core
  --lib
  --features privacy-release-evidence
  "$test_filter"
  --
  --ignored
  --exact
  --nocapture
  --test-threads=1
)

started_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
started_ns="$(date +%s%N)"
set +e
LC_ALL=C /usr/bin/time -v -o "$resource_log" \
  timeout --signal=TERM --kill-after=60s "$timeout_seconds" \
  "${command[@]}" >"$test_log" 2>&1
test_exit_code=$?
set -e
finished_ns="$(date +%s%N)"
finished_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
elapsed_ms="$(((finished_ns - started_ns) / 1000000))"
peak_rss_kib="$(awk -F ': ' '/Maximum resident set size \(kbytes\)/ {print $2}' "$resource_log")"
if [[ ! "$peak_rss_kib" =~ ^(0|[1-9][0-9]*)$ ]]; then
  echo "GNU time did not emit canonical peak RSS" >&2
  exit 70
fi
peak_rss_bytes="$((peak_rss_kib * 1024))"

status=passed
if (( test_exit_code != 0 || elapsed_ms > elapsed_ceiling_ms || peak_rss_bytes > peak_rss_ceiling_bytes )); then
  status=failed
fi

test_log_sha="$(sha256sum "$test_log" | awk '{print $1}')"
resource_log_sha="$(sha256sum "$resource_log" | awk '{print $1}')"
python3 -I -S - \
  "$evidence_dir/evidence.json" \
  "$case_slug" \
  "$test_filter" \
  "$expected_commit" \
  "$expected_source_sha" \
  "$expected_lock_sha" \
  "$toolchain_tree_sha" \
  "$elapsed_ceiling_ms" \
  "$peak_rss_ceiling_bytes" \
  "$address_space_ceiling_bytes" \
  "$elapsed_ms" \
  "$peak_rss_bytes" \
  "$test_exit_code" \
  "$status" \
  "$started_at_utc" \
  "$finished_at_utc" \
  "$test_log_sha" \
  "$resource_log_sha" <<'PY'
import json
from pathlib import Path
import sys

(
    output,
    case_slug,
    test_filter,
    commit,
    source_sha,
    lock_sha,
    toolchain_sha,
    elapsed_ceiling,
    rss_ceiling,
    address_ceiling,
    elapsed,
    rss,
    exit_code,
    status,
    started,
    finished,
    test_log_sha,
    resource_log_sha,
) = sys.argv[1:]
payload = {
    "schema": "iroha.taira.privacy_release_gate_evidence.v1",
    "schema_version": 1,
    "activation_readiness_authority": False,
    "case": case_slug,
    "test_filter": test_filter,
    "source": {
        "commit": commit,
        "workspace_source_sha256": source_sha,
        "cargo_lock_sha256": lock_sha,
        "rust_toolchain_tree_sha256": toolchain_sha,
    },
    "ceilings": {
        "elapsed_millis": int(elapsed_ceiling),
        "peak_rss_bytes": int(rss_ceiling),
        "address_space_bytes": int(address_ceiling),
    },
    "observed": {
        "elapsed_millis": int(elapsed),
        "peak_rss_bytes": int(rss),
        "test_exit_code": int(exit_code),
    },
    "started_at_utc": started,
    "finished_at_utc": finished,
    "status": status,
    "test_log_sha256": test_log_sha,
    "resource_log_sha256": resource_log_sha,
}
Path(output).write_text(
    json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
PY

(
  cd "$evidence_dir"
  sha256sum cargo.txt evidence.json resources.txt rustc.txt test.log >SHA256SUMS
  sha256sum -c SHA256SUMS
)

test "$(git rev-parse HEAD)" = "$expected_commit"
git diff --quiet
git diff --cached --quiet
test -z "$(git ls-files --others --exclude-standard)"
printf '%s  %s\n' "$expected_lock_sha" Cargo.lock | sha256sum -c -
test "$(python3 -I -S scripts/compute_workspace_source_manifest.py --root "$PWD")" = "$expected_source_sha"

if [[ "$status" != passed ]]; then
  echo "privacy release gate failed or exceeded a reviewed resource ceiling" >&2
  exit 1
fi
