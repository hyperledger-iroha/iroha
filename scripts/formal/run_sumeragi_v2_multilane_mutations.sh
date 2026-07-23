#!/usr/bin/env bash
set -euo pipefail

# Run the bounded multilane lifecycle/evidence/carrier negative-control corpus.
# Prerequisites: the pinned TLA2Tools jar installed by
# install_sumeragi_v2_tla2tools.sh and a working Java runtime. No environment
# variables are required; TLA2TOOLS_JAR and JAVA_BIN may override safe defaults.

if (($#)); then
  if (($# == 1)) && [[ "$1" == "--help" ]]; then
    echo "usage: $0" >&2
    exit 0
  fi
  echo "usage: $0" >&2
  exit 2
fi

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"
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

python3 -I -S "$CONTRACT_CHECKER"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-multilane.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 94 -seed 20260723
)

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
  local status=$?
  set -e
  if [[ "$status" -ne 12 ]] ||
    ! grep -Fq "Invariant ${invariant} is violated." "$log"; then
    echo "${name} did not produce the expected ${invariant} counterexample (status=${status})" >&2
    cat "$log" >&2
    exit 1
  fi
  grep -Fq "TLC2 Version 2.19" "$log" || {
    echo "${name} did not run with the pinned TLC engine" >&2
    cat "$log" >&2
    exit 1
  }
  echo "[tlc] observed ${name}: ${invariant}"
}

readonly AUTOSCALE_MODULE="SumeragiV2AutoscaleLifecycle.tla"
run_mutant autoscale-early-drain "$AUTOSCALE_MODULE" \
  multilane_autoscale_early_drain_bug.cfg DrainEvidenceInvariant
run_mutant autoscale-destroy-before-archive "$AUTOSCALE_MODULE" \
  multilane_autoscale_destroy_before_archive_bug.cfg \
  ArchiveBeforeDestroyInvariant
run_mutant autoscale-incarnation-reuse "$AUTOSCALE_MODULE" \
  multilane_autoscale_incarnation_reuse_bug.cfg NoIncarnationReuseInvariant

readonly NATIVE_MODULE="SumeragiV2NativeApplicationEvidence.tla"
run_mutant native-frontier-before-sidecars "$NATIVE_MODULE" \
  multilane_native_frontier_before_sidecars_bug.cfg \
  FrontierPublicationInvariant
run_mutant native-hash-only-pruning "$NATIVE_MODULE" \
  multilane_native_hash_only_pruning_bug.cfg \
  PrunedEvidenceVerifiableInvariant
run_mutant native-same-route-marker "$NATIVE_MODULE" \
  multilane_native_same_route_marker_bug.cfg SameRouteControlOnlyInvariant

readonly AUTONOMOUS_MODULE="SumeragiV2AutonomousReservationCarrier.tla"
run_mutant autonomous-carrier-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_carrier_drift_bug.cfg ExactCarrierIdentityInvariant
run_mutant autonomous-duplicate-application "$AUTONOMOUS_MODULE" \
  multilane_autonomous_duplicate_application_bug.cfg \
  AtMostOnceApplicationInvariant
run_mutant autonomous-release-after-apply "$AUTONOMOUS_MODULE" \
  multilane_autonomous_release_after_apply_bug.cfg \
  NoReleaseAfterApplicationInvariant
run_mutant autonomous-release-before-barrier "$AUTONOMOUS_MODULE" \
  multilane_autonomous_release_before_barrier_bug.cfg \
  ReleaseOrderingInvariant
run_mutant autonomous-aba-release "$AUTONOMOUS_MODULE" \
  multilane_autonomous_aba_release_bug.cfg \
  NoStaleIncarnationReleaseInvariant
run_mutant autonomous-digest-only-authorization "$AUTONOMOUS_MODULE" \
  multilane_autonomous_digest_only_authorization_bug.cfg \
  CandidateAuthorizationInvariant
run_mutant autonomous-ordinary-anchor-execution "$AUTONOMOUS_MODULE" \
  multilane_autonomous_ordinary_anchor_execution_bug.cfg \
  ControlOnlyAnchorInvariant

echo "[tlc] all 13 multilane mutations produced their exact named counterexamples; no deductive proof status was changed"
