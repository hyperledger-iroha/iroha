#!/usr/bin/env bash
# Check the mutation/repair pairs introduced by the protected service-rank proof.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) readonly TLAPM_PLATFORM="x86_64-linux-gnu" ;;
  Darwin-arm64) readonly TLAPM_PLATFORM="arm64-darwin" ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac
readonly TLAPM_STDLIB="${TLAPM_STDLIB:-${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${TLAPM_PLATFORM}/tlapm/lib/tlapm/stdlib}"

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

for module_and_hash in \
  "FiniteSets:eca905df95c16fb12f0f205a38b429c216a11b5c8429d270af044f05600a6380" \
  "Functions:b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063" \
  "Folds:aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da" \
  "TLAPS:5cc604533e49792c1c3d050a38d845d08d9c209879ca20c86de04975bc4bc563" \
  "WellFoundedInduction:6f2f274c2e987d1edcf004d8e37b053f1f82b912e66d6a51bae0af8012ddcbec" \
  "SequenceTheorems:1fdbed9077bba9db329e499535be29f8d2e6fba3a2b338e364c3b0ec56596bf9" \
  "FunctionTheorems:f89871fb5b35d2fc54f583b12adb489dd6660be7aced67ab295fba0d4999c116" \
  "NaturalsInduction:08f52420cdaaf11292ed366782b5ce5b596bb7cbe789526a1cfd8806dbf98624" \
  "FiniteSetTheorems:484bf0f9ab6a69ef45f7282f7f92dcf1e6ae139e44117b0d5a4427635818e773"; do
  module="${module_and_hash%%:*}"
  expected_sha256="${module_and_hash#*:}"
  module_path="${TLAPM_STDLIB}/${module}.tla"
  [[ -f "$module_path" ]] || {
    echo "pinned TLAPM ${TLAPM_COMMIT} module is required at ${module_path}" >&2
    exit 1
  }
  actual_sha256="$(hash_file "$module_path")"
  [[ "$actual_sha256" == "$expected_sha256" ]] || {
    echo "pinned TLAPM standard-library checksum mismatch for ${module}.tla" >&2
    echo "expected: ${expected_sha256}" >&2
    echo "actual:   ${actual_sha256}" >&2
    exit 1
  }
done

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-progress-mutations.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
tlapm_compat_dir="${run_dir}/tlapm-stdlib"
mkdir -p "$tlapm_compat_dir"
for module in \
  FiniteSets Functions Folds TLAPS WellFoundedInduction \
  SequenceTheorems FunctionTheorems \
  NaturalsInduction FiniteSetTheorems; do
  ln -s "${TLAPM_STDLIB}/${module}.tla" "${tlapm_compat_dir}/${module}.tla"
done

common=(
  "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${tlapm_compat_dir}"
  -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

run_case() {
  local label="$1"
  local model="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local actual_status
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" "$model"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  for marker in "$@"; do
    if ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case causal-debt-completion-bug \
  SumeragiV2CausalDebtMutation.tla causal_debt_completion_bug.cfg 13 \
  "Temporal properties were violated." "12 distinct states" "Back to state 2"
run_case causal-debt-completion-fixed \
  SumeragiV2CausalDebtMutation.tla causal_debt_completion_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "22 distinct states" "depth of the complete state graph search is 10"
run_case causal-debt-duplicate-fixed \
  SumeragiV2CausalDebtMutation.tla causal_debt_duplicate_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "4 distinct states" "depth of the complete state graph search is 3"
for scenario in no-producer producer-refill progress-ingress; do
  config_name="${scenario//-/_}"
  run_case "causal-debt-${scenario}-bug" \
    SumeragiV2CausalDebtMutation.tla "causal_debt_${config_name}_bug.cfg" 13 \
    "Temporal properties were violated." "3 distinct states" "Back to state 1"
  run_case "causal-debt-${scenario}-fixed" \
    SumeragiV2CausalDebtMutation.tla "causal_debt_${config_name}_fixed.cfg" 0 \
    "Model checking completed. No error has been found." \
    "5 distinct states" "depth of the complete state graph search is 5"
done

run_case causal-replacement-bug \
  SumeragiV2CausalReplacementMutation.tla causal_replacement_bug.cfg 13 \
  "Temporal properties were violated." "10 distinct states" "Back to state 2"
run_case causal-replacement-coalesced \
  SumeragiV2CausalReplacementMutation.tla causal_replacement_coalesced.cfg 0 \
  "Model checking completed. No error has been found." \
  "5 distinct states" "depth of the complete state graph search is 4"
run_case causal-fifo-rank-multiplier-one-bug \
  SumeragiV2CausalFifoRankMutation.tla \
  causal_fifo_rank_multiplier_one_bug.cfg 12 \
  "Invariant EarlierHeadRemovalStrictlyDropsTargetRank is violated." \
  "State 2: <RemoveEarlierHead" \
  "earlierHeadRemoved = TRUE" \
  "2 states generated, 2 distinct states found, 0 states left on queue."
run_case causal-fifo-rank-doubled \
  SumeragiV2CausalFifoRankMutation.tla \
  causal_fifo_rank_doubled.cfg 0 \
  "Model checking completed. No error has been found." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"
run_case discovery-debt-bug \
  SumeragiV2DiscoveryDebtMutation.tla discovery_debt_bug.cfg 13 \
  "Temporal properties were violated." "4 distinct states" "Back to state 2"
run_case discovery-debt-fixed \
  SumeragiV2DiscoveryDebtMutation.tla discovery_debt_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "4 distinct states" "depth of the complete state graph search is 3"
run_case io-candidate-all-jobs-bug \
  SumeragiV2IoCandidateIndexMutation.tla io_candidate_index_all_jobs_bug.cfg 12 \
  "Invariant OldIndexSelectsConsensus is violated by the initial state"
run_case io-candidate-consensus-only \
  SumeragiV2IoCandidateIndexMutation.tla io_candidate_index_consensus_only.cfg 0 \
  "Model checking completed. No error has been found." \
  "1 distinct states" "depth of the complete state graph search is 1"

run_case successor-stale-token-bug \
  SumeragiV2SuccessorStaleTokenMutation.tla \
  successor_stale_token_bug.cfg 12 \
  "Invariant SuccessorActivationProtocolInvariantProjection is violated." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "BuggyBeginSuccessorActivation"
run_case successor-stale-token-fixed \
  SumeragiV2SuccessorStaleTokenMutation.tla \
  successor_stale_token_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

run_case effective-lock-rebind-fixed \
  SumeragiV2EffectiveLockAcquisitionMutation.tla \
  effective_lock_rebind_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "10 distinct states" "depth of the complete state graph search is 2"
run_case effective-lock-rebind-bug \
  SumeragiV2EffectiveLockAcquisitionMutation.tla \
  effective_lock_rebind_bug.cfg 12 \
  "Invariant ViewRebindKeepsOnePhysicalLoad is violated." \
  "BuggyRebindSameLock"
run_case effective-lock-no-retry-bug \
  SumeragiV2EffectiveLockAcquisitionMutation.tla \
  effective_lock_no_retry_bug.cfg 13 \
  "Temporal properties were violated." \
  "5 distinct states" "State 4: Stuttering"
run_case effective-lock-future-completion-bug \
  SumeragiV2EffectiveLockAcquisitionMutation.tla \
  effective_lock_future_completion_bug.cfg 12 \
  "Invariant BuggyFutureCompletionFailsClosed is violated by the initial state"

run_case ownership-invariant-n1 \
  SumeragiV2OwnershipInvariantCheck.tla ownership_n1.cfg 0 \
  "Model checking completed. No error has been found." \
  "983041 states generated, 99328 distinct states found" \
  "depth of the complete state graph search is 49"

run_case reply-route-fixed \
  SumeragiV2ReplyRouteOwnershipMutation.tla reply_route_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "16 states generated, 16 distinct states found"
run_case reply-route-cursor-reset-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_cursor_reset_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "messageCursor |-> 0"
run_case reply-route-source-replacement-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_source_replacement_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "attempts = { [ connectionTenure |-> 1"
run_case reply-route-target-substitution-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_target_substitution_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "acceptedInvalidCapability = TRUE" "phase = 16"
run_case reply-route-ticket-payload-reuse-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_ticket_payload_reuse_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "ticketTenure |-> 1" "phase = 3"
run_case reply-route-reconnect-sibling-ticket-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_reconnect_sibling_ticket_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "ticketTenure |-> 1"
run_case reply-route-retired-ordinal-collision-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_retired_ordinal_collision_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "acceptedInvalidCapability = TRUE"
run_case reply-route-intrinsic-tenure-substitution-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_intrinsic_tenure_substitution_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "acceptedInvalidCapability = TRUE"
run_case reply-route-source-capacity-substitution-bug \
  SumeragiV2ReplyRouteOwnershipMutation.tla \
  reply_route_source_capacity_substitution_bug.cfg 12 \
  "Invariant RouteMutationSafety is violated." \
  "acceptedInvalidCapability = TRUE"

run_case reply-route-pipeline-fixed \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "23 states generated, 18 distinct states found" \
  "depth of the complete state graph search is 18"
run_case reply-route-pipeline-replay-isolation-fixed \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_replay_isolation_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "23 states generated, 18 distinct states found" \
  "depth of the complete state graph search is 18"
run_case reply-route-pipeline-replay-step-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_replay_step_bug.cfg 13 \
  "Action property MutationPipeline!ReplyTenureAwareReplay is violated." \
  "15 states generated, 14 distinct states found" \
  "phase = 34"
run_case reply-route-pipeline-source-isolation-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_source_isolation_bug.cfg 13 \
  "Action property MutationPipeline!ReplySourceIsolation is violated." \
  "14 states generated, 13 distinct states found" \
  "phase = 35"
run_case reply-route-pipeline-unfair-attach-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_unfair_attach_bug.cfg 13 \
  "Temporal properties were violated." \
  "2 states generated, 2 distinct states found" \
  "State 3: Stuttering"
run_case reply-route-pipeline-fifo-bypass-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_fifo_bypass_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "phase = 30"
run_case reply-route-pipeline-cursor-regression-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_cursor_regression_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "phase = 34"
run_case reply-route-pipeline-ticket-reuse-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_ticket_reuse_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "ticketTenure |-> 2" "phase = 31"
run_case reply-route-pipeline-premature-reconnect-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_premature_reconnect_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "sourceActive = (0 :> (0 :> FALSE" "phase = 32"
run_case reply-route-pipeline-reconnect-observation-not-ready-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_reconnect_observation_not_ready_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "phase = 36" "kind |-> \"Later\""
run_case reply-route-pipeline-old-flush-double-apply-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_old_flush_double_apply_bug.cfg 12 \
  "Invariant PipelineMutationSafety is violated." \
  "oldFlushAppliedTwice = TRUE" "messageCursor |-> 2"
run_case reply-route-pipeline-source-class-writer-fixed \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_source_class_writer_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "14 states generated, 13 distinct states found" \
  "depth of the complete state graph search is 13"
run_case reply-route-pipeline-cross-semantic-close-cycle-bug \
  SumeragiV2ReplyRoutePipelineMutation.tla \
  reply_route_pipeline_cross_semantic_close_cycle_bug.cfg 13 \
  "Temporal properties were violated." \
  "14 states generated, 13 distinct states found" \
  "Back to state 7"

echo "[tlc] protected-rank, causal-FIFO, successor, effective-lock, ownership, replay/isolation, and per-source reply-route/pipeline mutation matrix passed"
