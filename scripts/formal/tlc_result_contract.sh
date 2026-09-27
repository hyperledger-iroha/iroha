#!/usr/bin/env bash
# Shared fail-closed result checks for pinned TLC runs.

readonly TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly TLC_SUCCESS_MARKER="Model checking completed. No error has been found."
readonly TLC_VIOLATION_BEHAVIOR_MARKER="Error: The behavior up to this point is:"
readonly TLC_STATE_SUMMARY_PATTERN='^[0-9][0-9,]* states generated, [0-9][0-9,]* distinct states found, [0-9][0-9,]* states left on queue[.]$'
readonly TLC_STATE_SUMMARY_PREFIX='^[0-9][0-9,]* states generated, [0-9][0-9,]* distinct states found'
readonly TLC_FAILURE_DIAGNOSTIC_PATTERN='^[[:space:]]*(Error:|Deadlock reached([.]|$)|Temporal properties were violated[.]$)'
readonly TLC_PRIMARY_DIAGNOSTIC_PATTERN='^[[:space:]]*(Error: (Invariant |Action property |Temporal properties were violated[.]$|Deadlock reached([.]|$))|Deadlock reached([.]|$)|Temporal properties were violated[.]$)'

tlc_contract_fail() {
  local label="$1"
  local log="$2"
  local message="$3"
  echo "${label}: ${message}" >&2
  if [[ -f "$log" ]]; then
    cat "$log" >&2
  fi
  exit 1
}

tlc_assert_regular_log() {
  local label="$1"
  local log="$2"
  if [[ ! -f "$log" || -L "$log" ]]; then
    tlc_contract_fail \
      "$label" "$log" "TLC log must be a fresh regular file"
  fi
}

tlc_assert_nonzero_state_space() {
  local label="$1"
  local log="$2"
  local state_line
  local generated
  local distinct
  tlc_assert_regular_log "$label" "$log"
  state_line="$(
    grep -E "$TLC_STATE_SUMMARY_PREFIX" "$log" |
      tail -n 1 || true
  )"
  [[ -n "$state_line" ]] || {
    tlc_contract_fail \
      "$label" "$log" "TLC emitted no final state-count summary"
  }
  grep -Eq "$TLC_STATE_SUMMARY_PATTERN" <<<"$state_line" || {
    tlc_contract_fail \
      "$label" "$log" "TLC emitted a malformed final state-count summary"
  }
  generated="$(awk '{print $1}' <<<"$state_line" | tr -d ',')"
  distinct="$(awk '{print $4}' <<<"$state_line" | tr -d ',')"
  if ((generated <= 0 || distinct <= 0)); then
    tlc_contract_fail \
      "$label" "$log" "TLC explored a zero-state model: ${state_line}"
  fi
}

tlc_assert_terminal() {
  local label="$1"
  local log="$2"
  local terminal_count
  local last_nonblank
  tlc_assert_regular_log "$label" "$log"
  terminal_count="$(
    grep -Ec "$TLC_FINISHED_PATTERN" "$log" || true
  )"
  [[ "$terminal_count" == 1 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "TLC must emit exactly one terminal marker; found ${terminal_count}"
  }
  last_nonblank="$(awk 'NF { line = $0 } END { print line }' "$log")"
  grep -Eq \
    "$TLC_FINISHED_PATTERN" <<<"$last_nonblank" || {
    tlc_contract_fail \
      "$label" "$log" "TLC log did not end at its terminal marker"
  }
}

tlc_assert_exact_line() {
  local label="$1"
  local log="$2"
  local marker="$3"
  local marker_count
  tlc_assert_regular_log "$label" "$log"
  marker_count="$(grep -Fxc "$marker" "$log" || true)"
  [[ "$marker_count" == 1 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "TLC must emit exactly one full-line marker '${marker}'; found ${marker_count}"
  }
}

tlc_assert_fixed_success() {
  local label="$1"
  local log="$2"
  local actual_status="$3"
  local failure_count
  [[ "$actual_status" -eq 0 ]] || {
    tlc_contract_fail \
      "$label" "$log" "TLC returned status ${actual_status}, expected 0"
  }
  tlc_assert_nonzero_state_space "$label" "$log"
  tlc_assert_exact_line \
    "$label" "$log" "$TLC_SUCCESS_MARKER"
  failure_count="$(
    grep -Ec "$TLC_FAILURE_DIAGNOSTIC_PATTERN" "$log" ||
      true
  )"
  [[ "$failure_count" == 0 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "successful TLC run emitted ${failure_count} error/deadlock diagnostics"
  }
  tlc_assert_terminal "$label" "$log"
}

tlc_assert_action_property_violation() {
  local label="$1"
  local log="$2"
  local actual_status="$3"
  local expected_marker="$4"
  local failure_count
  local primary_diagnostic_count
  [[ "$expected_marker" =~ ^Error:\ Action\ property\ .+\ is\ violated\.$ ]] || {
    tlc_contract_fail \
      "$label" "$log" "action-property marker is not canonical"
  }
  [[ "$actual_status" -eq 13 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "TLC returned status ${actual_status}, expected action-property status 13"
  }
  tlc_assert_nonzero_state_space "$label" "$log"
  tlc_assert_exact_line \
    "$label" "$log" "$expected_marker"
  tlc_assert_exact_line \
    "$label" "$log" "$TLC_VIOLATION_BEHAVIOR_MARKER"
  primary_diagnostic_count="$(
    grep -Ec "$TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$log" || true
  )"
  [[ "$primary_diagnostic_count" == 1 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "action-property TLC run emitted ${primary_diagnostic_count} primary diagnostics"
  }
  failure_count="$(
    grep -Ec "$TLC_FAILURE_DIAGNOSTIC_PATTERN" "$log" || true
  )"
  [[ "$failure_count" == 2 ]] || {
    tlc_contract_fail \
      "$label" "$log" \
      "action-property TLC run must contain exactly its primary and behavior diagnostics; found ${failure_count}"
  }
  tlc_assert_terminal "$label" "$log"
}
