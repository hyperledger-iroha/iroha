#!/usr/bin/env bash
# Shared fail-closed result checks for pinned Sumeragi V2 TLC runs.

readonly SUMERAGI_V2_TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly SUMERAGI_V2_TLC_SUCCESS_MARKER="Model checking completed. No error has been found."
readonly SUMERAGI_V2_TLC_STATE_SUMMARY_PATTERN='^[0-9][0-9,]* states generated, [0-9][0-9,]* distinct states found, [0-9][0-9,]* states left on queue[.]$'
readonly SUMERAGI_V2_TLC_STATE_SUMMARY_PREFIX='^[0-9][0-9,]* states generated, [0-9][0-9,]* distinct states found'
readonly SUMERAGI_V2_TLC_FAILURE_DIAGNOSTIC_PATTERN='^[[:space:]]*(Error:|Deadlock reached([.]|$)|Temporal properties were violated[.]$)'
readonly SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN='^[[:space:]]*(Error: (Invariant |Action property |Temporal properties were violated[.]$|Deadlock reached([.]|$))|Deadlock reached([.]|$)|Temporal properties were violated[.]$)'
readonly SUMERAGI_V2_REPLAY_TOOL_MESSAGE_COUNT=113
readonly SUMERAGI_V2_REPLAY_TOOL_STATE_COUNT=101

sumeragi_v2_tlc_contract_fail() {
  local label="$1"
  local log="$2"
  local message="$3"
  echo "${label}: ${message}" >&2
  if [[ -f "$log" ]]; then
    cat "$log" >&2
  fi
  exit 1
}

sumeragi_v2_tlc_assert_regular_log() {
  local label="$1"
  local log="$2"
  if [[ ! -f "$log" || -L "$log" ]]; then
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC log must be a fresh regular file"
  fi
}

sumeragi_v2_tlc_assert_nonzero_state_space() {
  local label="$1"
  local log="$2"
  local state_line
  local generated
  local distinct
  sumeragi_v2_tlc_assert_regular_log "$label" "$log"
  state_line="$(
    grep -E "$SUMERAGI_V2_TLC_STATE_SUMMARY_PREFIX" "$log" |
      tail -n 1 || true
  )"
  [[ -n "$state_line" ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC emitted no final state-count summary"
  }
  grep -Eq "$SUMERAGI_V2_TLC_STATE_SUMMARY_PATTERN" <<<"$state_line" || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC emitted a malformed final state-count summary"
  }
  generated="$(awk '{print $1}' <<<"$state_line" | tr -d ',')"
  distinct="$(awk '{print $4}' <<<"$state_line" | tr -d ',')"
  if ((generated <= 0 || distinct <= 0)); then
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC explored a zero-state model: ${state_line}"
  fi
}

sumeragi_v2_tlc_assert_terminal() {
  local label="$1"
  local log="$2"
  local terminal_count
  local last_nonblank
  sumeragi_v2_tlc_assert_regular_log "$label" "$log"
  terminal_count="$(
    grep -Ec "$SUMERAGI_V2_TLC_FINISHED_PATTERN" "$log" || true
  )"
  [[ "$terminal_count" == 1 ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" \
      "TLC must emit exactly one terminal marker; found ${terminal_count}"
  }
  last_nonblank="$(awk 'NF { line = $0 } END { print line }' "$log")"
  grep -Eq \
    "$SUMERAGI_V2_TLC_FINISHED_PATTERN" <<<"$last_nonblank" || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC log did not end at its terminal marker"
  }
}

sumeragi_v2_tlc_assert_exact_line() {
  local label="$1"
  local log="$2"
  local marker="$3"
  local marker_count
  sumeragi_v2_tlc_assert_regular_log "$label" "$log"
  marker_count="$(grep -Fxc "$marker" "$log" || true)"
  [[ "$marker_count" == 1 ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" \
      "TLC must emit exactly one full-line marker '${marker}'; found ${marker_count}"
  }
}

sumeragi_v2_tlc_assert_fixed_success() {
  local label="$1"
  local log="$2"
  local actual_status="$3"
  local failure_count
  [[ "$actual_status" -eq 0 ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" "TLC returned status ${actual_status}, expected 0"
  }
  sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
  sumeragi_v2_tlc_assert_exact_line \
    "$label" "$log" "$SUMERAGI_V2_TLC_SUCCESS_MARKER"
  failure_count="$(
    grep -Ec "$SUMERAGI_V2_TLC_FAILURE_DIAGNOSTIC_PATTERN" "$log" ||
      true
  )"
  [[ "$failure_count" == 0 ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$log" \
      "successful TLC run emitted ${failure_count} error/deadlock diagnostics"
  }
  sumeragi_v2_tlc_assert_terminal "$label" "$log"
}

# Assert the process-level result contract before the Python normalizer applies
# its stricter ordered transcript and payload validation. stdout and stderr are
# deliberately separate; merged output is never admissible evidence.
sumeragi_v2_tlc_assert_replay_tool_result() {
  local label="$1"
  local stdout_log="$2"
  local stderr_log="$3"
  local actual_status="$4"
  local start_count
  local end_count
  local state_start_count
  local state_end_count

  sumeragi_v2_tlc_assert_regular_log "$label" "$stdout_log"
  sumeragi_v2_tlc_assert_regular_log "$label-stderr" "$stderr_log"
  [[ ! -s "$stderr_log" ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$stderr_log" "TLC emitted separate stderr"
  }
  [[ "$actual_status" -eq 12 ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$stdout_log" \
      "TLC returned status ${actual_status}, expected invariant status 12"
  }
  # The replay paths are ASCII. Anything outside TAB, LF, and printable ASCII
  # includes forbidden controls, C1 bytes, or framing-obscuring whitespace.
  if [[ -n "$(LC_ALL=C tr -d '\11\12\40-\176' <"$stdout_log")" ]]; then
    sumeragi_v2_tlc_contract_fail \
      "$label" "$stdout_log" "TLC stdout contains forbidden bytes"
  fi
  start_count="$(grep -Ec '^@!@!@STARTMSG [0-9]+:[0-9]+ @!@!@$' "$stdout_log" || true)"
  end_count="$(grep -Ec '^@!@!@ENDMSG [0-9]+ @!@!@$' "$stdout_log" || true)"
  state_start_count="$(grep -Fxc '@!@!@STARTMSG 2217:4 @!@!@' "$stdout_log" || true)"
  state_end_count="$(grep -Fxc '@!@!@ENDMSG 2217 @!@!@' "$stdout_log" || true)"
  [[ "$start_count" == "$SUMERAGI_V2_REPLAY_TOOL_MESSAGE_COUNT" \
    && "$end_count" == "$SUMERAGI_V2_REPLAY_TOOL_MESSAGE_COUNT" \
    && "$state_start_count" == "$SUMERAGI_V2_REPLAY_TOOL_STATE_COUNT" \
    && "$state_end_count" == "$SUMERAGI_V2_REPLAY_TOOL_STATE_COUNT" ]] || {
    sumeragi_v2_tlc_contract_fail \
      "$label" "$stdout_log" \
      "TLC tool-message census differs (start=${start_count}, end=${end_count}, state-start=${state_start_count}, state-end=${state_end_count})"
  }
}
