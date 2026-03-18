#!/usr/bin/env bash
# Swift sample smoke helper for IOS5: builds/tests both iOS demo apps so the CI
# roadmap has a runnable gate. Intended for macOS runners with simulators. Emits
# machine-readable JSON + JUnit summaries so callers can surface skips/failures in CI.
set -u -o pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLES_ROOT="${REPO_ROOT}/examples/ios"
DERIVED_BASE="${SWIFT_SAMPLES_DERIVED_DATA:-${REPO_ROOT}/artifacts/swift_samples}"
SUMMARY_PATH="${SWIFT_SAMPLES_SUMMARY:-${DERIVED_BASE}/summary.json}"
JUNIT_PATH_DEFAULT="${DERIVED_BASE}/swift_samples.junit.xml"
JUNIT_PATH="${SWIFT_SAMPLES_JUNIT:-${JUNIT_PATH_DEFAULT}}"
DESTINATION_DEFAULT="platform=iOS Simulator,name=iPhone 15"
DESTINATION="${SWIFT_SAMPLES_DESTINATION:-${DESTINATION_DEFAULT}}"
declare -a SAMPLE_NAMES=()
declare -a SAMPLE_STATUSES=()
declare -a SAMPLE_REASONS=()
declare -a SAMPLE_DURATIONS=()

now_seconds() {
  if command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY'
import time
print(f"{time.time():.6f}")
PY
  else
    date +%s | awk '{ printf "%.3f", $1 }'
  fi
}

calc_duration() {
  local start="${1:-}"
  if [[ -z "${start}" ]]; then
    printf "0"
    return
  fi
  local end
  end="$(now_seconds)"
  awk -v s="${start}" -v e="${end}" 'BEGIN { d = e - s; if (d < 0) d = 0; printf "%.3f", d }'
}

json_escape() {
  local raw="${1:-}"
  raw=${raw//\\/\\\\}
  raw=${raw//\"/\\\"}
  raw=${raw//$'\n'/\\n}
  echo "${raw}"
}

xml_escape() {
  local raw="${1:-}"
  raw=${raw//\&/&amp;}
  raw=${raw//</&lt;}
  raw=${raw//>/&gt;}
  raw=${raw//\"/&quot;}
  raw=${raw//\'/&apos;}
  raw=${raw//$'\n'/&#10;}
  echo "${raw}"
}

record_result() {
  local sample="$1"
  local status="$2"
  local reason="${3:-}"
  local duration="${4:-0}"
  SAMPLE_NAMES+=("${sample}")
  SAMPLE_STATUSES+=("${status}")
  SAMPLE_REASONS+=("${reason}")
  SAMPLE_DURATIONS+=("${duration}")
}

finish_sample() {
  local sample="$1"
  local status="$2"
  local reason="${3:-}"
  local start="${4:-}"
  local duration="0"
  if [[ -n "${start}" ]]; then
    duration="$(calc_duration "${start}")"
  fi
  record_result "${sample}" "${status}" "${reason}" "${duration}"
}

write_summary() {
  mkdir -p "$(dirname "${SUMMARY_PATH}")"

  local has_fail=0
  local has_pass=0
  local has_skip=0
  local -a entries=()

  if [[ "${#SAMPLE_NAMES[@]}" -gt 0 ]]; then
    for idx in "${!SAMPLE_NAMES[@]}"; do
      local sample="${SAMPLE_NAMES[$idx]}"
      local status="${SAMPLE_STATUSES[$idx]}"
      local reason="${SAMPLE_REASONS[$idx]}"
      local duration="${SAMPLE_DURATIONS[$idx]:-0}"
      case "${status}" in
        failed)
          has_fail=1
          ;;
        passed)
          has_pass=1
          ;;
        skipped)
          has_skip=1
          ;;
      esac
      entries+=("{\"sample\":\"$(json_escape "${sample}")\",\"status\":\"${status}\",\"reason\":\"$(json_escape "${reason}")\",\"duration_seconds\":${duration}}")
    done
  fi

  local overall="skipped"
  if [[ "${has_fail}" -eq 1 ]]; then
    overall="failed"
  elif [[ "${has_pass}" -eq 1 ]]; then
    overall="passed"
  elif [[ "${has_skip}" -eq 1 ]]; then
    overall="skipped"
  elif [[ "${#entries[@]}" -eq 0 ]]; then
    overall="unknown"
  fi

  local samples_joined=""
  if [[ "${#entries[@]}" -gt 0 ]]; then
    samples_joined=$(IFS=,; echo "${entries[*]}")
  fi

  cat >"${SUMMARY_PATH}" <<EOF
{"overall_status":"${overall}","destination":"$(json_escape "${DESTINATION}")","derived_data_base":"$(json_escape "${DERIVED_BASE}")","samples":[${samples_joined}]}
EOF
}

write_junit_report() {
  if [[ -z "${JUNIT_PATH}" ]]; then
    return
  fi

  mkdir -p "$(dirname "${JUNIT_PATH}")"

  local total="${#SAMPLE_NAMES[@]}"
  local failures=0
  local skipped=0
  local -a case_lines=()

  if [[ "${#SAMPLE_NAMES[@]}" -gt 0 ]]; then
    for idx in "${!SAMPLE_NAMES[@]}"; do
      local sample="${SAMPLE_NAMES[$idx]}"
      local status="${SAMPLE_STATUSES[$idx]}"
      local reason="${SAMPLE_REASONS[$idx]}"
      local duration="${SAMPLE_DURATIONS[$idx]:-0}"
      local testcase="  <testcase name=\"$(xml_escape "${sample}")\" classname=\"SwiftSamples\" time=\"${duration}\""
      local xml_reason
      xml_reason="$(xml_escape "${reason}")"
      if [[ "${status}" == "failed" ]]; then
        ((failures += 1))
        testcase+=">"
        testcase+=$'\n    <failure message="'
        testcase+="${xml_reason}"
        testcase+=$'"></failure>'
        testcase+=$'\n  </testcase>'
      elif [[ "${status}" == "skipped" ]]; then
        ((skipped += 1))
        testcase+=">"
        testcase+=$'\n    <skipped message="'
        testcase+="${xml_reason}"
        testcase+=$'"/>'
        testcase+=$'\n  </testcase>'
      else
        testcase+=" />"
      fi
      case_lines+=("${testcase}")
    done
  fi

  {
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    printf '<testsuite name="swift-sample-smokes" tests="%s" failures="%s" errors="0" skipped="%s">\n' "${total}" "${failures}" "${skipped}"
    if [[ "${#case_lines[@]}" -gt 0 ]]; then
      printf '%s\n' "${case_lines[@]}"
    fi
    echo '</testsuite>'
  } >"${JUNIT_PATH}"
}

write_reports() {
  write_summary
  write_junit_report
}

for cmd in swift xcodebuild xcrun; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "warning: ${cmd} not found; skipping Swift sample smoke tests" >&2
    record_result "NoritoDemo" "skipped" "missing required tool: ${cmd}" "0"
    record_result "NoritoDemoXcode" "skipped" "missing required tool: ${cmd}" "0"
    write_reports
    exit 0
  fi
done

mkdir -p "${DERIVED_BASE}"

run_xcodebuild() {
  if command -v xcbeautify >/dev/null 2>&1; then
    xcodebuild "$@" | xcbeautify
  elif command -v xcpretty >/dev/null 2>&1; then
    xcodebuild "$@" | xcpretty
  else
    xcodebuild "$@"
  fi
}

resolve_destination() {
  local project="$1"
  local scheme="$2"
  local destination="$3"

  local dest_name
  dest_name="$(printf '%s\n' "${destination}" | sed -n 's/.*name=\([^,]*\).*/\1/p')"
  if [[ -z "${dest_name}" ]]; then
    echo "${destination}"
    return
  fi

  if xcrun simctl list devices available >/dev/null 2>&1; then
    if xcrun simctl list devices available | grep -Fq "${dest_name}"; then
      echo "${destination}"
      return
    fi
  else
    echo "warning: unable to query simulators; skipping sample build" >&2
    echo ""
    return
  fi

  local show_dest
  show_dest=$(
    xcodebuild \
      -project "${project}" \
      -scheme "${scheme}" \
      -showdestinations 2>/dev/null || true
  )

  local fallback_line
  fallback_line=$(printf '%s\n' "${show_dest}" | sed -n 's/^ \+{[[:space:]]*\(platform:[^}]*name:My Mac\)[[:space:]]*}.*/\1/p' | head -n1)
  if [[ -z "${fallback_line}" ]]; then
    fallback_line=$(printf '%s\n' "${show_dest}" | sed -n 's/^ \+{[[:space:]]*\(platform:[^}]*\)[[:space:]]*}.*/\1/p' | head -n1)
  fi

  if [[ -z "${fallback_line}" ]]; then
    echo ""
  else
    local clean
    clean="$(printf '%s\n' "${fallback_line}" | sed 's/: */=/g' | tr -d '\n')"
    echo "${clean//, /,}"
  fi
}

build_template_demo() {
  local template_dir="${SAMPLES_ROOT}/NoritoDemo"
  local project="${template_dir}/NoritoDemo.xcodeproj"
  local scheme="NoritoDemo"
  local derived="${DERIVED_BASE}/NoritoDemo"
  local sample="NoritoDemo"
  local start
  start="$(now_seconds)"

  if [[ "${SWIFT_SAMPLES_SKIP_TEMPLATE:-0}" == "1" ]]; then
    echo "==> Skipping NoritoDemo (XcodeGen) per SWIFT_SAMPLES_SKIP_TEMPLATE"
    finish_sample "${sample}" "skipped" "environment toggle" "${start}"
    return
  fi

  if [[ ! -d "${template_dir}" ]]; then
    echo "error: missing template sample at ${template_dir}" >&2
    finish_sample "${sample}" "failed" "missing template sample directory" "${start}"
    return 1
  fi

  if ! command -v xcodegen >/dev/null 2>&1; then
    echo "error: xcodegen is required to build NoritoDemo; install it or set SWIFT_SAMPLES_SKIP_TEMPLATE=1" >&2
    finish_sample "${sample}" "failed" "xcodegen not installed" "${start}"
    return 1
  fi

  echo "==> Generating NoritoDemo.xcodeproj via xcodegen"
  (cd "${template_dir}" && xcodegen generate)

  local resolved_dest
  resolved_dest="$(resolve_destination "${project}" "${scheme}" "${DESTINATION}")"
  if [[ -z "${resolved_dest}" ]]; then
    echo "warning: no suitable destination found; skipping NoritoDemo build/test" >&2
    finish_sample "${sample}" "skipped" "no simulator available for destination '${DESTINATION}' (simctl unavailable or device missing)" "${start}"
    return
  fi

  mkdir -p "${derived}"

  echo "==> Building NoritoDemo for destination '${resolved_dest}'"
  if ! run_xcodebuild \
    -project "${project}" \
    -scheme "${scheme}" \
    -configuration Debug \
    -destination "${resolved_dest}" \
    -derivedDataPath "${derived}" \
    build; then
    finish_sample "${sample}" "failed" "build failed (see ${derived})" "${start}"
    return 1
  fi

  echo "==> Running NoritoDemo tests"
  if ! run_xcodebuild \
    -project "${project}" \
    -scheme "${scheme}" \
    -configuration Debug \
    -destination "${resolved_dest}" \
    -derivedDataPath "${derived}" \
    test; then
    finish_sample "${sample}" "failed" "tests failed (see ${derived})" "${start}"
    return 1
  fi

  finish_sample "${sample}" "passed" "build+tests succeeded (${derived})" "${start}"
}

build_swiftui_demo() {
  local sample="NoritoDemoXcode"
  local start
  start="$(now_seconds)"

  if [[ "${SWIFT_SAMPLES_SKIP_NORITO_DEMO_XCODE:-0}" == "1" ]]; then
    echo "==> Skipping NoritoDemoXcode per SWIFT_SAMPLES_SKIP_NORITO_DEMO_XCODE"
    finish_sample "${sample}" "skipped" "environment toggle" "${start}"
    return
  fi

  local derived="${DERIVED_BASE}/NoritoDemoXcode"
  mkdir -p "${derived}"

  echo "==> Building NoritoDemoXcode (SwiftUI sample)"
  if NORITO_DEMO_DESTINATION="${DESTINATION}" \
    NORITO_DEMO_DERIVED_DATA="${derived}" \
    "${REPO_ROOT}/scripts/ci/verify_norito_demo.sh"; then
    finish_sample "${sample}" "passed" "build+tests succeeded (${derived})" "${start}"
  else
    finish_sample "${sample}" "failed" "verify_norito_demo.sh failed (see ${derived})" "${start}"
    return 1
  fi
}

overall_status=0

if ! build_template_demo; then
  overall_status=1
fi
if ! build_swiftui_demo; then
  overall_status=1
fi

write_reports

if [[ "${overall_status}" -ne 0 ]]; then
  exit "${overall_status}"
fi

echo "Swift sample smoke tests completed."
