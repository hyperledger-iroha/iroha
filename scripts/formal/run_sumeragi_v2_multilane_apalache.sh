#!/usr/bin/env bash
set -euo pipefail

# Fail-closed bounded Apalache gate for the four Sumeragi v2 multilane
# refinement kernels plus the layout-only in-flight carrier kernel. The fixed
# bounds are part of the reviewed contract and are not configurable.

if (($#)); then
  if (($# == 1)) && [[ "$1" == "--help" ]]; then
    echo "usage: $0" >&2
    exit 0
  fi
  echo "usage: $0" >&2
  exit 2
fi

readonly APALACHE_VERSION="0.52.2"
readonly APALACHE_LAUNCHER_SHA256="bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7"
readonly APALACHE_JAR_SHA256="1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"
readonly RUNNER_CONTRACT_TEST="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"
readonly INSTALL_ROOT="${APALACHE_INSTALL_ROOT:-${REPO_ROOT}/target/apalache/toolchains}"
readonly APALACHE_BIN="${APALACHE_BIN:-${INSTALL_ROOT}/v${APALACHE_VERSION}/bin/apalache-mc}"
readonly EVIDENCE_DIR="${REPO_ROOT}/target/formal/sumeragi_v2"
readonly LOG_DIR="${EVIDENCE_DIR}/multilane_apalache"
readonly EVIDENCE_PATH="${EVIDENCE_DIR}/multilane_apalache_evidence.tsv"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

canonical_path() {
  python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

for forbidden_override in \
  APALACHE_JAR \
  CONFIG_FILE \
  OUT_DIR \
  RUN_DIR \
  SMT_ENCODING \
  TUNING_OPTIONS \
  TUNING_OPTIONS_FILE; do
  if [[ -n "${!forbidden_override:-}" ]]; then
    echo "${forbidden_override} is not accepted by the pinned multilane Apalache gate" >&2
    exit 1
  fi
done

mkdir -p "$EVIDENCE_DIR"
rm -f -- "$EVIDENCE_PATH"
rm -rf -- "$LOG_DIR"
mkdir -p "$LOG_DIR"

python3 -I -S "$CONTRACT_CHECKER"
python3 -I -S "$RUNNER_CONTRACT_TEST"
source_manifest_sha256="$(
  python3 -I -S "$CONTRACT_CHECKER" --print-source-manifest-sha256
)"
readonly source_manifest_sha256

if [[ ! -f "$APALACHE_BIN" || -L "$APALACHE_BIN" || ! -x "$APALACHE_BIN" ]]; then
  echo "pinned Apalache v${APALACHE_VERSION} is required at ${APALACHE_BIN}" >&2
  echo "run scripts/formal/install_apalache.sh ${APALACHE_VERSION} first" >&2
  exit 1
fi
RESOLVED_APALACHE_BIN="$(canonical_path "$APALACHE_BIN")"
readonly RESOLVED_APALACHE_BIN
APALACHE_HOME="$(cd -- "$(dirname -- "$RESOLVED_APALACHE_BIN")/.." && pwd)"
readonly APALACHE_HOME
readonly APALACHE_JAR_PATH="${APALACHE_HOME}/lib/apalache.jar"
if [[ ! -f "$APALACHE_JAR_PATH" || -L "$APALACHE_JAR_PATH" ]]; then
  echo "pinned Apalache jar is missing or unsafe: ${APALACHE_JAR_PATH}" >&2
  exit 1
fi

actual_launcher_sha256="$(hash_file "$RESOLVED_APALACHE_BIN")"
if [[ "$actual_launcher_sha256" != "$APALACHE_LAUNCHER_SHA256" ]]; then
  echo "Apalache launcher checksum mismatch" >&2
  echo "expected: ${APALACHE_LAUNCHER_SHA256}" >&2
  echo "actual:   ${actual_launcher_sha256}" >&2
  exit 1
fi
actual_jar_sha256="$(hash_file "$APALACHE_JAR_PATH")"
if [[ "$actual_jar_sha256" != "$APALACHE_JAR_SHA256" ]]; then
  echo "Apalache jar checksum mismatch" >&2
  echo "expected: ${APALACHE_JAR_SHA256}" >&2
  echo "actual:   ${actual_jar_sha256}" >&2
  exit 1
fi

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"
java_bin_dir="$(dirname -- "$JAVA_BIN")"
readonly java_bin_dir
PATH="${java_bin_dir}:${PATH}"
export PATH
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required for Apalache" >&2
  exit 1
}

tool_version="$("$RESOLVED_APALACHE_BIN" version)"
if [[ "$tool_version" != "$APALACHE_VERSION" ]]; then
  echo "Apalache version mismatch" >&2
  echo "expected: ${APALACHE_VERSION}" >&2
  echo "actual:   ${tool_version}" >&2
  exit 1
fi

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-multilane-apalache.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

run_typecheck() {
  local module="$1"
  local log="${LOG_DIR}/${module}.typecheck.log"
  local out="${run_dir}/${module}.typecheck.out"
  set +e
  (
    cd "$FORMAL_DIR"
    "$RESOLVED_APALACHE_BIN" --out-dir="$out" typecheck "${module}.tla"
  ) >"$log" 2>&1
  local status=$?
  set -e
  cat "$log"
  if [[ "$status" -ne 0 ]] ||
    [[ "$(grep -Fc "# APALACHE version: ${APALACHE_VERSION} |" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fc "Type checker [OK]" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fxc "EXITCODE: OK" "$log" || true)" != 1 ]]; then
    echo "Apalache typecheck did not report exact success for ${module} (status=${status})" >&2
    exit 1
  fi
  echo "[apalache] typechecked ${module} with pinned v${APALACHE_VERSION}"
}

run_positive() {
  local name="$1"
  local module="$2"
  local config="$3"
  local length="$4"
  local invariants="$5"
  local log="${LOG_DIR}/${name}.check.log"
  local out="${run_dir}/${name}.check.out"
  set +e
  (
    cd "$FORMAL_DIR"
    "$RESOLVED_APALACHE_BIN" --out-dir="$out" check \
      --algo=incremental \
      --config="$config" \
      --length="$length" \
      --no-deadlock \
      "${module}.tla"
  ) >"$log" 2>&1
  local status=$?
  set -e
  cat "$log"
  if [[ "$status" -ne 0 ]] ||
    [[ "$(grep -Fc "# APALACHE version: ${APALACHE_VERSION} |" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fc "Using inv predicate(s) ${invariants} from the TLC config" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fc "The outcome is: NoError" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fc "Checker reports no error up to computation length ${length}" "$log" || true)" != 1 ]] ||
    [[ "$(grep -Fxc "EXITCODE: OK" "$log" || true)" != 1 ]]; then
    echo "Apalache bounded check did not report exact NoError for ${name} (status=${status})" >&2
    exit 1
  fi
  echo "[apalache] ${name}: exact NoError through ${length} Next steps"
}

readonly AUTOSCALE_MODULE="SumeragiV2AutoscaleLifecycle"
readonly NATIVE_MODULE="SumeragiV2NativeApplicationEvidence"
readonly AUTONOMOUS_MODULE="SumeragiV2AutonomousReservationCarrier"
readonly QUEUE_PLAN_ADMISSION_MODULE="SumeragiV2QueuePlanAdmissionRegistry"
readonly INFLIGHT_FIRST_RELEASE_MODULE="SumeragiV2InFlightFirstRelease"

run_typecheck "$AUTOSCALE_MODULE"
run_typecheck "$NATIVE_MODULE"
run_typecheck "$AUTONOMOUS_MODULE"
run_typecheck "$QUEUE_PLAN_ADMISSION_MODULE"
run_typecheck "$INFLIGHT_FIRST_RELEASE_MODULE"

run_positive \
  autoscale-lifecycle \
  "$AUTOSCALE_MODULE" \
  multilane_autoscale_lifecycle_fixed.cfg \
  8 \
  "LifecycleTypeInvariant, StorageBeforeActivationInvariant, DrainEvidenceInvariant, ArchiveBeforeDestroyInvariant, NoIncarnationReuseInvariant, MLActivationAfterAtomicCreate, MLDrainImpliesNoOwnedWork, MLDrainCertificateMonotonic, MLRetirementConsumesExactIncarnation"
run_positive \
  native-application-evidence \
  "$NATIVE_MODULE" \
  multilane_native_application_evidence_fixed.cfg \
  5 \
  "NativeEvidenceTypeInvariant, NativeStandaloneEvidenceInvariant, NativeEvidenceRetentionBoundInvariant, NativeNoClobberPublicationInvariant, NativeLegacyDenseRejectedInvariant, NativePruneJournalInvariant, SidecarsRequireManifestInvariant, FrontierPublicationInvariant, PrunedEvidenceVerifiableInvariant, SameRouteControlOnlyInvariant, MLSeparateParticipantApplication, MLNativeSourceClaimInjective, MLNativeContiguousActiveRoute, MLNativeGroupExactCover, MLNativeManifestAuthenticates, MLNativeDurabilityPrecedesFrontier, MLNativeLatestIndexExact"
run_positive \
  autonomous-reservation-carrier \
  "$AUTONOMOUS_MODULE" \
  multilane_autonomous_reservation_carrier_fixed.cfg \
  10 \
  "ReservationCarrierTypeInvariant, SingleOwnershipInvariant, ExactCarrierIdentityInvariant, ControlOnlyAnchorInvariant, CandidateAuthorizationInvariant, ReleaseOrderingInvariant, QueueReleaseCompletionInvariant, AtMostOnceApplicationInvariant, NoReleaseAfterApplicationInvariant, NoStaleIncarnationReleaseInvariant, ForgottenOnlyAfterApplicationInvariant, MLReservationSingleOwner, MLReservationIdentityStable, MLCertifiedBundleDurable, MLMergeCandidateExactPrefix, MLCarrierExactlyOnce, MLRestartOwnershipPartition, MLStageEvidenceMonotonic"
run_positive \
  queue-plan-admission-registry \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_admission_registry_fixed.cfg \
  8 \
  "QueuePlanAdmissionTypeInvariant, MLAdmissionCasUnique, MLCertificateDurable, MLPublic202Exact, MLExecutionRequiresExactBinding, MLQueueEligibilityExact, MLAdmissionAtMostOnceExecution, MLImmutableAdmissionTombstone, MLCancellationStopsExecution"
run_positive \
  inflight-first-release-layout \
  "$INFLIGHT_FIRST_RELEASE_MODULE" \
  inflight_first_release_fixed.cfg \
  10 \
  "FirstReleaseTypeInvariant, MLPayloadSchemaV2CarriesExactAdmissionPreimage, MLValidatorCarrierOwnership, MLPutBatchV4BeforeReservationV5, MLReservationV5BeforeKuraActive, MLKuraActiveBeforeExecutionInput, MLExecutionInputBeforeReady, MLCrashPrefixLossFree, MLCommitAndReleaseRetainExactScope, MLExactlyOnceCarrierApplication, MLQueuePlanV4PutBatchBound4096"

final_source_manifest_sha256="$(
  python3 -I -S "$CONTRACT_CHECKER" --print-source-manifest-sha256
)"
if [[ "$final_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
  echo "multilane formal or production sources changed during the Apalache run" >&2
  exit 1
fi

evidence_tmp="$(mktemp "${EVIDENCE_DIR}/.multilane_apalache_evidence.XXXXXX")"
{
  printf 'schema_version\t1\n'
  printf 'backend\tapalache\n'
  printf 'version\t%s\n' "$APALACHE_VERSION"
  printf 'launcher_sha256\t%s\n' "$APALACHE_LAUNCHER_SHA256"
  printf 'jar_sha256\t%s\n' "$APALACHE_JAR_SHA256"
  printf 'source_manifest_sha256\t%s\n' "$source_manifest_sha256"
  printf 'result_count\t5\n'
  printf 'result\tautoscale-lifecycle\t%s\t%s\t8\tNoError\t%s\t%s\t%s\n' \
    "$AUTOSCALE_MODULE" \
    "multilane_autoscale_lifecycle_fixed.cfg" \
    "$(hash_file "${FORMAL_DIR}/${AUTOSCALE_MODULE}.tla")" \
    "$(hash_file "${FORMAL_DIR}/multilane_autoscale_lifecycle_fixed.cfg")" \
    "$(hash_file "${LOG_DIR}/autoscale-lifecycle.check.log")"
  printf 'result\tnative-application-evidence\t%s\t%s\t5\tNoError\t%s\t%s\t%s\n' \
    "$NATIVE_MODULE" \
    "multilane_native_application_evidence_fixed.cfg" \
    "$(hash_file "${FORMAL_DIR}/${NATIVE_MODULE}.tla")" \
    "$(hash_file "${FORMAL_DIR}/multilane_native_application_evidence_fixed.cfg")" \
    "$(hash_file "${LOG_DIR}/native-application-evidence.check.log")"
  printf 'result\tautonomous-reservation-carrier\t%s\t%s\t10\tNoError\t%s\t%s\t%s\n' \
    "$AUTONOMOUS_MODULE" \
    "multilane_autonomous_reservation_carrier_fixed.cfg" \
    "$(hash_file "${FORMAL_DIR}/${AUTONOMOUS_MODULE}.tla")" \
    "$(hash_file "${FORMAL_DIR}/multilane_autonomous_reservation_carrier_fixed.cfg")" \
    "$(hash_file "${LOG_DIR}/autonomous-reservation-carrier.check.log")"
  printf 'result\tqueue-plan-admission-registry\t%s\t%s\t8\tNoError\t%s\t%s\t%s\n' \
    "$QUEUE_PLAN_ADMISSION_MODULE" \
    "multilane_queue_plan_admission_registry_fixed.cfg" \
    "$(hash_file "${FORMAL_DIR}/${QUEUE_PLAN_ADMISSION_MODULE}.tla")" \
    "$(hash_file "${FORMAL_DIR}/multilane_queue_plan_admission_registry_fixed.cfg")" \
    "$(hash_file "${LOG_DIR}/queue-plan-admission-registry.check.log")"
  printf 'result\tinflight-first-release-layout\t%s\t%s\t10\tNoError\t%s\t%s\t%s\n' \
    "$INFLIGHT_FIRST_RELEASE_MODULE" \
    "inflight_first_release_fixed.cfg" \
    "$(hash_file "${FORMAL_DIR}/${INFLIGHT_FIRST_RELEASE_MODULE}.tla")" \
    "$(hash_file "${FORMAL_DIR}/inflight_first_release_fixed.cfg")" \
    "$(hash_file "${LOG_DIR}/inflight-first-release-layout.check.log")"
} >"$evidence_tmp"
mv -- "$evidence_tmp" "$EVIDENCE_PATH"

echo "[apalache] all 4 source-bound refinement kernels plus the layout-only in-flight carrier passed pinned v${APALACHE_VERSION} bounded checks; no proof status was changed; evidence=${EVIDENCE_PATH}"
