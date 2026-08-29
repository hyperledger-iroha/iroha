#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly MODEL="${REPO_ROOT}/formal/private_settlement/AtomicPrivateSettlementV1.tla"
readonly INDEXED_MODEL="${REPO_ROOT}/formal/private_settlement/AtomicPrivateSettlementV1CommitteeFaults.tla"
readonly TLC_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the pinned tla2tools.jar}"
readonly JAVA_BIN="${JAVA_BIN:-java}"

if [[ ! -f "$TLC_JAR" ]]; then
  echo "TLA2TOOLS_JAR does not name a regular file: $TLC_JAR" >&2
  exit 2
fi
for model in "$MODEL" "$INDEXED_MODEL"; do
  if [[ ! -f "$model" ]]; then
    echo "atomic private-settlement TLA+ model is missing: $model" >&2
    exit 2
  fi
done

readonly TLC=("$JAVA_BIN" -XX:+UseParallelGC -cp "$TLC_JAR" tlc2.TLC)
readonly SANY=("$JAVA_BIN" -cp "$TLC_JAR" tla2sany.SANY)
readonly POSITIVE_CONFIGS=(
  AtomicPrivateSettlementV1_3.cfg
  AtomicPrivateSettlementV1_255.cfg
  AtomicPrivateSettlementV1_expiry.cfg
)
readonly INDEXED_POSITIVE_CONFIGS=(
  AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg
  AtomicPrivateSettlementV1CommitteeFaults_2.cfg
  AtomicPrivateSettlementV1CommitteeFaults_3.cfg
  AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg
  AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg
)
readonly NEGATIVE_CONFIGS=(
  AtomicPrivateSettlementV1_partial_apply_bug.cfg
  AtomicPrivateSettlementV1_commit_before_prepare_bug.cfg
  AtomicPrivateSettlementV1_drop_stage_on_crash_bug.cfg
)

scratch="$(mktemp -d "${TMPDIR:-/tmp}/atomic-private-settlement-tlc.XXXXXX")"
trap 'rm -rf -- "$scratch"' EXIT

for model in "$MODEL" "$INDEXED_MODEL"; do
  echo "[atomic-private-settlement-tlc] SANY $(basename -- "$model")"
  "${SANY[@]}" "$model"
done

for config in "${POSITIVE_CONFIGS[@]}"; do
  echo "[atomic-private-settlement-tlc] positive $config"
  "${TLC[@]}" -cleanup -workers auto \
    -metadir "${scratch}/${config%.cfg}" \
    -config "${REPO_ROOT}/formal/private_settlement/${config}" \
    "$MODEL"
done

for config in "${INDEXED_POSITIVE_CONFIGS[@]}"; do
  echo "[atomic-private-settlement-tlc] indexed positive $config"
  "${TLC[@]}" -cleanup -workers auto \
    -metadir "${scratch}/${config%.cfg}" \
    -config "${REPO_ROOT}/formal/private_settlement/${config}" \
    "$INDEXED_MODEL"
done

for config in "${NEGATIVE_CONFIGS[@]}"; do
  echo "[atomic-private-settlement-tlc] negative $config"
  output="${scratch}/${config}.log"
  if "${TLC[@]}" -cleanup -workers auto \
      -metadir "${scratch}/${config%.cfg}" \
      -config "${REPO_ROOT}/formal/private_settlement/${config}" \
      "$MODEL" >"$output" 2>&1; then
    echo "negative control unexpectedly passed: $config" >&2
    exit 1
  fi
  if ! grep -Fq "Error: Invariant Safety is violated." "$output"; then
    echo "negative control failed for an unexpected reason: $config" >&2
    tail -n 40 "$output" >&2
    exit 1
  fi
done

echo "[atomic-private-settlement-tlc] all positive and negative controls passed"
