#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_SCRIPT="${SCRIPT_DIR}/clear_volatile_consensus_state.sh"

cleanup_paths=()

cleanup() {
  local path
  for path in "${cleanup_paths[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
  return 0
}

trap cleanup EXIT

make_case_root() {
  local root dist
  root="$(mktemp -d)"
  dist="${root}/dist"
  mkdir -p \
    "${dist}/storage/peer0/rbc_sessions" \
    "${dist}/storage/peer1" \
    "${dist}/storage/peer2/rbc_sessions" \
    "${dist}/storage/peer3" \
    "${root}/bin"
  cp "$SOURCE_SCRIPT" "${root}/clear_volatile_consensus_state.sh"
  chmod +x "${root}/clear_volatile_consensus_state.sh"
  printf 'journal-0\n' >"${dist}/storage/peer0/queue_plan_journal"
  printf 'journal-1\n' >"${dist}/storage/peer1/queue_plan_journal.1"
  printf 'durable\n' >"${dist}/storage/peer1/durable_state"
  printf 'rbc\n' >"${dist}/storage/peer0/rbc_sessions/session"
  printf 'rbc2\n' >"${dist}/storage/peer2/rbc_sessions/session"
  printf 'log0\n' >"${dist}/peer0.log"
  printf 'log1\n' >"${dist}/peer1.log"
  printf '999999\n' >"${dist}/peer0.pid"
  cat >"${dist}/start.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf 'IROHAD_BIN=%s LOG_LEVEL=%s\n' "${IROHAD_BIN:-}" "${LOG_LEVEL:-}" >start.invocation
SH
  chmod +x "${dist}/start.sh"
  printf '#!/usr/bin/env bash\nexit 0\n' >"${root}/bin/irohad"
  chmod +x "${root}/bin/irohad"
  printf '%s\n' "$root"
}

run_dry_run_preserves_state_case() {
  local root dist output
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/dry-run.log"

  "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --runtime-bin "${root}/bin/irohad" \
    >"$output" 2>&1

  grep -q 'dry-run: pass --apply' "$output"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
  test -f "${dist}/peer0.log"
  test ! -e "${dist}/start.invocation"
}

run_apply_quarantines_only_volatile_state_case() {
  local root dist output
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/apply.log"

  "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --start \
    --torii-ports "" \
    --log-level warn \
    >"$output" 2>&1

  grep -q 'volatile consensus quarantine completed.' "$output"
  test ! -e "${dist}/storage/peer0/queue_plan_journal"
  test ! -e "${dist}/storage/peer1/queue_plan_journal.1"
  test ! -d "${dist}/storage/peer0/rbc_sessions"
  test ! -d "${dist}/storage/peer2/rbc_sessions"
  test -f "${dist}/storage/peer1/durable_state"
  test -n "$(find "${dist}/storage/peer0/queue_journal_quarantine" -type f -name 'queue_plan_journal.*' -print -quit)"
  test -n "$(find "${dist}/storage/peer1/queue_journal_quarantine" -type f -name 'queue_plan_journal.1.*' -print -quit)"
  test -n "$(find "${dist}/storage/peer0/rbc_sessions_quarantine" -type d -name 'rbc_sessions.*' -print -quit)"
  test -n "$(find "${dist}/logs" -type f -name 'peer0.log.pre-volatile-clear-*' -print -quit)"
  test ! -e "${dist}/peer0.pid"
  grep -q "IROHAD_BIN=${root}/bin/irohad LOG_LEVEL=warn" "${dist}/start.invocation"
}

run_sha_mismatch_fails_before_mutation_case() {
  local root dist output
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/sha-mismatch.log"

  if "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --expected-runtime-sha 0000000000000000000000000000000000000000000000000000000000000000 \
    >"$output" 2>&1; then
    echo "SHA mismatch case unexpectedly succeeded" >&2
    sed -n '1,120p' "$output" >&2 || true
    return 1
  fi

  grep -q 'runtime SHA mismatch' "$output"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
  test -f "${dist}/peer0.log"
}

run_dry_run_preserves_state_case
run_apply_quarantines_only_volatile_state_case
run_sha_mismatch_fails_before_mutation_case

echo "clear_volatile_consensus_state mock tests passed."
