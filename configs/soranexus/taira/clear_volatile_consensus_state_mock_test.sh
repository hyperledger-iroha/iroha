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
  grep -q 'volatile consensus quarantine dry-run completed.' "$output"
  ! grep -q '^volatile consensus quarantine completed\.$' "$output"
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

run_invalid_torii_ports_fail_before_mutation_case() {
  local root dist output port_value expected_message
  port_value="$1"
  expected_message="$2"
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/invalid-torii-ports.log"

  if "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --torii-ports "$port_value" \
    >"$output" 2>&1; then
    echo "invalid Torii ports case unexpectedly succeeded: $port_value" >&2
    sed -n '1,120p' "$output" >&2 || true
    return 1
  fi

  grep -q -- "$expected_message" "$output"
  test -f "${dist}/peer0.pid"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
  test -f "${dist}/peer0.log"
}

run_apply_ignores_reused_pidfile_for_unrelated_live_pid_case() {
  local root dist output
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/reused-pidfile.log"
  printf '%s\n' "$$" >"${dist}/peer0.pid"

  "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --torii-ports "" \
    >"$output" 2>&1

  grep -q 'ignoring stale or reused pidfile' "$output"
  grep -q 'volatile consensus quarantine completed.' "$output"
  test ! -e "${dist}/peer0.pid"
  test ! -e "${dist}/storage/peer0/queue_plan_journal"
  test ! -d "${dist}/storage/peer0/rbc_sessions"
  test -n "$(find "${dist}/logs" -type f -name 'peer0.log.pre-volatile-clear-*' -print -quit)"
}

run_apply_ignores_config_suffix_collision_pidfile_case() {
  local root dist output bash_env
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/suffix-collision-pidfile.log"
  bash_env="${root}/bash-env.sh"
  printf '%s\n' "$$" >"${dist}/peer0.pid"
  cat >"$bash_env" <<'SH'
ps() {
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_PID:-}" && "${3:-}" == "-o" && "${4:-}" == "command=" ]]; then
    printf '/tmp/irohad --config %s/peer0.toml.bak\n' "${TAIRA_TEST_DIST:?}"
    return 0
  fi
  command ps "$@"
}
SH

  TAIRA_TEST_PID="$$" TAIRA_TEST_DIST="$dist" BASH_ENV="$bash_env" \
    "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --torii-ports "" \
    >"$output" 2>&1

  grep -q 'ignoring stale or reused pidfile' "$output"
  grep -q 'volatile consensus quarantine completed.' "$output"
  test ! -e "${dist}/peer0.pid"
  test ! -e "${dist}/storage/peer0/queue_plan_journal"
  test ! -d "${dist}/storage/peer0/rbc_sessions"
}

run_apply_ignores_config_suffix_collision_ps_scan_case() {
  local root dist output bash_env
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/suffix-collision-ps-scan.log"
  bash_env="${root}/bash-env.sh"
  rm -f "${dist}/peer0.pid"
  cat >"$bash_env" <<'SH'
kill() {
  if [[ "${1:-}" == "${TAIRA_TEST_FAKE_PID:-}" || "${2:-}" == "${TAIRA_TEST_FAKE_PID:-}" ]]; then
    echo "unexpected kill of suffix-collision ps row" >&2
    return 42
  fi
  command kill "$@"
}
ps() {
  if [[ "${1:-}" == "-axo" ]]; then
    printf '%s /tmp/irohad --config %s/peer0.toml.bak\n' "${TAIRA_TEST_FAKE_PID:?}" "${TAIRA_TEST_DIST:?}"
    return 0
  fi
  command ps "$@"
}
SH

  TAIRA_TEST_FAKE_PID="424242" TAIRA_TEST_DIST="$dist" BASH_ENV="$bash_env" \
    "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    --torii-ports "" \
    >"$output" 2>&1

  ! grep -q 'unexpected kill of suffix-collision ps row' "$output"
  grep -q 'no running peer processes matched' "$output"
  grep -q 'volatile consensus quarantine completed.' "$output"
  test ! -e "${dist}/storage/peer0/queue_plan_journal"
  test ! -d "${dist}/storage/peer0/rbc_sessions"
}

run_dry_run_detects_exact_ps_config_peer_case() {
  local root dist output bash_env status
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/exact-ps-scan.log"
  bash_env="${root}/bash-env.sh"
  rm -f "${dist}/peer0.pid"
  cat >"$bash_env" <<'SH'
kill() {
  if [[ "${1:-}" == "-0" && "${2:-}" == "${TAIRA_TEST_FAKE_PID:-}" ]]; then
    return 1
  fi
  command kill "$@"
}
ps() {
  if [[ "${1:-}" == "-axo" ]]; then
    printf '%s /tmp/irohad --config=%s/peer2.toml\n' "${TAIRA_TEST_FAKE_PID:?}" "${TAIRA_TEST_DIST:?}"
    return 0
  fi
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_FAKE_PID:-}" ]]; then
    return 0
  fi
  command ps "$@"
}
SH

  status=0
  TAIRA_TEST_FAKE_PID="424242" TAIRA_TEST_DIST="$dist" BASH_ENV="$bash_env" \
    "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --runtime-bin "${root}/bin/irohad" \
    >"$output" 2>&1 || status="$?"

  [[ "$status" == "2" ]]
  grep -q 'stopping 1 peer process' "$output"
  grep -q 'cannot signal.*quarantining volatile consensus state' "$output"
  grep -q 'volatile consensus quarantine dry-run completed with warnings' "$output"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
}

run_dry_run_warns_for_unsignalable_live_peer_case() {
  local root dist output bash_env status
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/unsignalable-peer-dry-run.log"
  bash_env="${root}/bash-env.sh"
  printf '%s\n' "$$" >"${dist}/peer0.pid"
  cat >"$bash_env" <<'SH'
kill() {
  if [[ "${1:-}" == "-0" ]]; then
    return 1
  fi
  command kill "$@"
}
ps() {
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_PID:-}" && "${3:-}" == "-o" && "${4:-}" == "command=" ]]; then
    printf '/tmp/irohad --config %s/peer0.toml\n' "${TAIRA_TEST_DIST:?}"
    return 0
  fi
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_PID:-}" ]]; then
    return 0
  fi
  command ps "$@"
}
SH

  status=0
  TAIRA_TEST_PID="$$" TAIRA_TEST_DIST="$dist" BASH_ENV="$bash_env" \
    "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --runtime-bin "${root}/bin/irohad" \
    >"$output" 2>&1 || status="$?"

  [[ "$status" == "2" ]]
  grep -q 'cannot signal.*quarantining volatile consensus state' "$output"
  grep -q 'volatile consensus quarantine dry-run completed with warnings' "$output"
  ! grep -q '^volatile consensus quarantine completed\.$' "$output"
  test -f "${dist}/peer0.pid"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
  test -f "${dist}/peer0.log"
}

run_apply_refuses_unsignalable_live_peer_case() {
  local root dist output bash_env
  root="$(make_case_root)"
  cleanup_paths+=("$root")
  dist="${root}/dist"
  output="${root}/unsignalable-peer.log"
  bash_env="${root}/bash-env.sh"
  printf '%s\n' "$$" >"${dist}/peer0.pid"
  cat >"$bash_env" <<'SH'
kill() {
  if [[ "${1:-}" == "-0" ]]; then
    return 1
  fi
  command kill "$@"
}
ps() {
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_PID:-}" && "${3:-}" == "-o" && "${4:-}" == "command=" ]]; then
    printf '/tmp/irohad --config %s/peer0.toml\n' "${TAIRA_TEST_DIST:?}"
    return 0
  fi
  if [[ "${1:-}" == "-p" && "${2:-}" == "${TAIRA_TEST_PID:-}" ]]; then
    return 0
  fi
  command ps "$@"
}
SH

  if TAIRA_TEST_PID="$$" TAIRA_TEST_DIST="$dist" BASH_ENV="$bash_env" \
    "${root}/clear_volatile_consensus_state.sh" \
    --dist "$dist" \
    --apply \
    --runtime-bin "${root}/bin/irohad" \
    >"$output" 2>&1; then
    echo "unsignalable peer case unexpectedly succeeded" >&2
    sed -n '1,120p' "$output" >&2 || true
    return 1
  fi

  grep -q 'cannot signal.*quarantining volatile consensus state' "$output"
  ! grep -q '^volatile consensus quarantine completed\.$' "$output"
  test -f "${dist}/peer0.pid"
  test -f "${dist}/storage/peer0/queue_plan_journal"
  test -d "${dist}/storage/peer0/rbc_sessions"
  test -f "${dist}/peer0.log"
}

run_dry_run_preserves_state_case
run_apply_quarantines_only_volatile_state_case
run_sha_mismatch_fails_before_mutation_case
run_invalid_torii_ports_fail_before_mutation_case "29080,/tmp/29081" "--torii-ports must be a comma-separated list of numeric ports"
run_invalid_torii_ports_fail_before_mutation_case "29080," "--torii-ports contains an empty port entry"
run_invalid_torii_ports_fail_before_mutation_case "65536" "--torii-ports contains out-of-range port 65536"
run_invalid_torii_ports_fail_before_mutation_case "29080, 29080" "--torii-ports contains duplicate port 29080"
run_invalid_torii_ports_fail_before_mutation_case "29080, 029080" "--torii-ports contains duplicate port 29080"
run_apply_ignores_reused_pidfile_for_unrelated_live_pid_case
run_apply_ignores_config_suffix_collision_pidfile_case
run_apply_ignores_config_suffix_collision_ps_scan_case
run_dry_run_detects_exact_ps_config_peer_case
run_dry_run_warns_for_unsignalable_live_peer_case
run_apply_refuses_unsignalable_live_peer_case

echo "clear_volatile_consensus_state mock tests passed."
