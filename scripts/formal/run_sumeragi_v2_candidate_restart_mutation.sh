#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

[[ -f "$TLA2TOOLS_JAR" ]] || {
  echo "pinned TLA2Tools v${TLA2TOOLS_VERSION} is required at ${TLA2TOOLS_JAR}" >&2
  exit 1
}
actual_sha256="$(hash_file "$TLA2TOOLS_JAR")"
[[ "$actual_sha256" == "$TLA2TOOLS_SHA256" ]] || {
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-candidate-restart.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

run_green() {
  local name="$1"
  local module="$2"
  local config="$3"
  local log="$run_dir/${name}.log"
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$run_dir/${name}" \
      -config "$config" "$module"
  ) >"$log" 2>&1
  for marker in \
    "TLC2 Version 2.19" \
    "Model checking completed. No error has been found."; do
    grep -Fq "$marker" "$log" || {
      echo "repaired ${name} run missed expected marker: $marker" >&2
      cat "$log" >&2
      exit 1
    }
  done
}

run_mutant() {
  local name="$1"
  local module="$2"
  local config="$3"
  local invariant="$4"
  local log="$run_dir/${name}.log"
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$run_dir/${name}" \
      -config "$config" "$module"
  ) >"$log" 2>&1
  local rc=$?
  set -e
  [[ $rc -eq 12 ]] || {
    echo "${name} mutation did not fail with TLC status 12" >&2
    cat "$log" >&2
    exit 1
  }
  for marker in \
    "TLC2 Version 2.19" \
    "Invariant ${invariant} is violated."; do
    grep -Fq "$marker" "$log" || {
      echo "${name} mutation missed expected marker: $marker" >&2
      cat "$log" >&2
      exit 1
    }
  done
}

readonly IDENTITY_MODULE="SumeragiV2CandidateIdentityMutation.tla"
run_green identity-exact "$IDENTITY_MODULE" candidate_identity_exact.cfg
run_green identity-identical-coalesced "$IDENTITY_MODULE" \
  candidate_identity_exact_identical_coalesced.cfg
run_mutant identity-consumer-context "$IDENTITY_MODULE" \
  candidate_identity_changed_consumer_context_bug.cfg \
  ChangedConsumerContextNotCoalesced
run_mutant identity-consumer-view "$IDENTITY_MODULE" \
  candidate_identity_changed_consumer_view_bug.cfg \
  ChangedConsumerViewNotCoalesced
run_mutant identity-stale-generation "$IDENTITY_MODULE" \
  candidate_identity_stale_generation_bug.cfg \
  StaleGenerationNotCoalesced
run_mutant identity-payload "$IDENTITY_MODULE" \
  candidate_identity_changed_payload_bug.cfg ChangedPayloadNotCoalesced
run_mutant identity-evidence "$IDENTITY_MODULE" \
  candidate_identity_changed_evidence_bug.cfg ChangedEvidenceNotCoalesced
run_mutant identity-causal-origin "$IDENTITY_MODULE" \
  candidate_identity_changed_causal_origin_bug.cfg \
  ChangedCausalOriginNotCoalesced
run_mutant identity-work "$IDENTITY_MODULE" \
  candidate_identity_changed_work_bug.cfg ChangedWorkNotCoalesced
run_mutant identity-body "$IDENTITY_MODULE" \
  candidate_identity_changed_body_bug.cfg ChangedBodyNotCoalesced
run_mutant identity-manifest "$IDENTITY_MODULE" \
  candidate_identity_changed_manifest_bug.cfg ChangedManifestNotCoalesced
run_mutant identity-commitment "$IDENTITY_MODULE" \
  candidate_identity_changed_commitment_bug.cfg \
  ChangedCommitmentNotCoalesced
run_mutant identity-broad-projection "$IDENTITY_MODULE" \
  candidate_identity_broad_projection_bug.cfg ExactCandidateAdmitted

readonly RESTART_MODULE="SumeragiV2CrashReplayMutation.tla"
run_green restart-signature "$RESTART_MODULE" crash_replay_signature_fixed.cfg
run_green restart-body "$RESTART_MODULE" crash_replay_body_fixed.cfg
run_green restart-application "$RESTART_MODULE" \
  crash_replay_application_fixed.cfg
run_mutant restart-signature-volatile "$RESTART_MODULE" \
  crash_replay_signature_volatile_bug.cfg VolatileSignatureProgressWitness
run_mutant restart-signature-drop "$RESTART_MODULE" \
  crash_replay_signature_drop_bug.cfg DurableWorkHasReplayOrRecovery
run_mutant restart-body-drop "$RESTART_MODULE" \
  crash_replay_body_drop_bug.cfg DurableWorkHasReplayOrRecovery
run_mutant restart-application-drop "$RESTART_MODULE" \
  crash_replay_application_drop_bug.cfg DurableWorkHasReplayOrRecovery
run_mutant restart-stale-completion "$RESTART_MODULE" \
  crash_replay_stale_completion_bug.cfg NoStaleCompletion

readonly INGRESS_CLASS_MODULE="SumeragiV2IngressClassMutation.tla"
run_green ingress-class-repaired "$INGRESS_CLASS_MODULE" ingress_class_repaired.cfg
run_mutant ingress-class-outer-commit-vote "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_commit_vote_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-prepare-qc "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_prepare_qc_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-commit-qc "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_commit_qc_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-timeout "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_timeout_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-timeout-certificate "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_timeout_certificate_drop_bug.cfg \
  OuterProgressClassAligned
run_mutant ingress-class-outer-chunk "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_chunk_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-certified "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_certified_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-certified-response "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_certified_response_drop_bug.cfg \
  OuterProgressClassAligned
run_mutant ingress-class-outer-commit "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_commit_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-outer-commit-response "$INGRESS_CLASS_MODULE" \
  ingress_class_outer_commit_response_drop_bug.cfg OuterProgressClassAligned
run_mutant ingress-class-runtime-locked-commit "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_locked_commit_drop_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-prepare-qc "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_prepare_qc_drop_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-commit-qc "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_commit_qc_drop_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-timeout "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_timeout_drop_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-timeout-certificate "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_timeout_certificate_drop_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-proposal "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_proposal_promotion_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-prepare-vote "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_prepare_vote_promotion_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-commit-vote "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_commit_vote_promotion_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-manifest "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_manifest_promotion_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-chunk "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_chunk_promotion_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-certified "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_certified_promotion_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-certified-response "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_certified_response_promotion_bug.cfg \
  RuntimeProgressClassAligned
run_mutant ingress-class-runtime-commit "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_commit_promotion_bug.cfg RuntimeProgressClassAligned
run_mutant ingress-class-runtime-commit-response "$INGRESS_CLASS_MODULE" \
  ingress_class_runtime_commit_response_promotion_bug.cfg \
  RuntimeProgressClassAligned

echo "[tlc] exact candidate identity distinguishes context, view, generation, payload, evidence, work, body, manifest, and commitment and coalesces an identical occurrence"
echo "[tlc] compact crash/replay reconstruction replaces the volatile signature witness with exact crash authority and retains body and application negative controls without preserving the obsolete exclusive RestartReplay order"
echo "[tlc] stale-generation completion and dropped reconstruction mutants fail their named invariants"
echo "[tlc] exact outer Progress and runtime Progress/Normal/bypass partitions reject every single-family mutation"
echo "[tlc] 6 repaired cases passed; 39 mutants failed their named invariants"
