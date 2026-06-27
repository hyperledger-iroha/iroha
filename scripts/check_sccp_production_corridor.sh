#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/sccp-production-corridor}"
NORITO_SKIP_BINDINGS_SYNC="${NORITO_SKIP_BINDINGS_SYNC:-1}"
SCCP_CORRIDOR_NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
SCCP_CORRIDOR_PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
SCCP_GRADLE_JVMARGS="${SCCP_GRADLE_JVMARGS:--Xmx6g}"
SCCP_KOTLIN_DAEMON_JVMARGS="${SCCP_KOTLIN_DAEMON_JVMARGS:-$SCCP_GRADLE_JVMARGS}"
SCCP_GRADLE_OPTS_DEFAULT="-Dorg.gradle.jvmargs=$SCCP_GRADLE_JVMARGS -Dkotlin.daemon.jvmargs=$SCCP_KOTLIN_DAEMON_JVMARGS -Dkotlin.daemon.jvm.options=$SCCP_KOTLIN_DAEMON_JVMARGS"
SCCP_GRADLE_OPTS="${GRADLE_OPTS:-$SCCP_GRADLE_OPTS_DEFAULT}"
SCCP_DOTNET_TRX_MAX_BYTES=16777216
DRY_RUN=0
LOG_DIR=""

ALL_PHASES=(
  rust-sccp
  evidence-scripts
  js-sdk
  python-sdk
  swift-sdk
  kotlin-sdk
  java-android
  dotnet-sdk
  contract-smoke
  core-admission
)

SELECTED_PHASES=()

usage() {
  cat <<'EOF'
Usage: scripts/check_sccp_production_corridor.sh [OPTIONS]

Run the focused SCCP production-readiness validation corridor.

Options:
  --phase NAME     Run one phase. Repeatable; comma-separated names are accepted.
  --dry-run        Print selected phase commands without executing them.
  --log-dir DIR    Run each selected phase in its own corridor invocation and
                   tee strict phase transcripts to DIR/<phase>.log.
  --list           Print available phases and exit.
  -h, --help       Show this help.

Environment:
  CARGO_TARGET_DIR             Cargo target directory for Rust phases.
                               Defaults to target/sccp-production-corridor.
  NORITO_SKIP_BINDINGS_SYNC    Defaults to 1 for focused Rust validation.
  JAVA_HOME                    JDK 21 for Gradle phases. Falls back to
                               target/java/jdk-21/Contents/Home, then
                               /usr/libexec/java_home -v 21 on macOS, then
                               Homebrew openjdk@21 installations.
  ANDROID_HOME                 Android SDK for the Java Android phase.
                               Defaults to ~/Library/Android/sdk when present.
  ANDROID_SDK_ROOT             Defaults to ANDROID_HOME when unset.
  DOTNET_ROOT                  .NET SDK root for the native C# SCCP phase.
                               Falls back to /tmp/iroha-dotnet/sdk, then dotnet
                               on PATH.
  SCCP_DOTNET_BRIDGE_TARGET_DIR
                               Cargo target directory used to build the Windows
                               connect_norito_bridge.dll for the .NET phase.
                               Defaults to CARGO_TARGET_DIR.
  SCCP_GRADLE_JVMARGS          Default Gradle heap for Kotlin/Android phases
                               when GRADLE_OPTS is unset. Defaults to -Xmx6g.
  SCCP_KOTLIN_DAEMON_JVMARGS   Default Kotlin daemon heap when GRADLE_OPTS is
                               unset. Defaults to SCCP_GRADLE_JVMARGS.
  GRADLE_OPTS                  Overrides the corridor Gradle/Kotlin heap
                               defaults when set by the operator.
  SCCP_CORRIDOR_NODE_BIN       Node runtime for JavaScript and contract phases.
                               Defaults to node.
  SCCP_CORRIDOR_PYTHON_BIN     Python runtime for evidence and Python SDK phases.
                               Defaults to python3.
EOF
}

list_phases() {
  printf 'Available SCCP production corridor phases:\n'
  local phase
  for phase in "${ALL_PHASES[@]}"; do
    printf '  %s\n' "$phase"
  done
}

is_known_phase() {
  case "$1" in
    rust-sccp|evidence-scripts|js-sdk|python-sdk|swift-sdk|kotlin-sdk|java-android|dotnet-sdk|contract-smoke|core-admission)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

add_phase() {
  local value="$1"
  local parts=()
  local phase
  IFS=',' read -r -a parts <<<"$value"
  for phase in "${parts[@]}"; do
    if [[ -z "$phase" ]]; then
      echo "Empty --phase value is not valid." >&2
      exit 2
    fi
    if ! is_known_phase "$phase"; then
      echo "Unknown SCCP production corridor phase: $phase" >&2
      list_phases >&2
      exit 2
    fi
    SELECTED_PHASES+=("$phase")
  done
}

phase_selected() {
  local requested="$1"
  local phase
  if [[ ${#SELECTED_PHASES[@]} -eq 0 ]]; then
    return 0
  fi
  for phase in "${SELECTED_PHASES[@]}"; do
    if [[ "$phase" == "$requested" ]]; then
      return 0
    fi
  done
  return 1
}

selected_phases() {
  local phase
  for phase in "${ALL_PHASES[@]}"; do
    if phase_selected "$phase"; then
      printf '%s\n' "$phase"
    fi
  done
}

print_cmd() {
  printf '+'
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf '\n'
}

run_cmd() {
  print_cmd "$@"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    return 0
  fi
  "$@"
}

print_in_dir_cmd() {
  local dir="$1"
  shift
  printf '+ (cd %q &&' "$dir"
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf ')\n'
}

run_in_dir() {
  local dir="$1"
  shift
  print_in_dir_cmd "$dir" "$@"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    return 0
  fi
  (cd "$dir" && "$@")
}

run_capture_in_dir() {
  local result_var="$1"
  local dir="$2"
  shift 2
  print_in_dir_cmd "$dir" "$@"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf -v "$result_var" ''
    return 0
  fi
  local output
  if ! output="$(cd "$dir" && "$@" 2>&1)"; then
    printf '%s\n' "$output"
    return 1
  fi
  printf '%s\n' "$output"
  printf -v "$result_var" '%s' "$output"
}

dotnet_info_field_count() {
  local label="$1"
  awk -v label="$label" '
    {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      if (index(line, label ":") == 1) {
        count++
      }
    }
    END {
      print count + 0
    }
  '
}

dotnet_info_field_value() {
  local label="$1"
  awk -v label="$label" '
    {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      if (index(line, label ":") == 1) {
        value = substr(line, length(label) + 2)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        print value
        exit
      }
    }
  '
}

dotnet_info_section_field_count() {
  local section="$1"
  local label="$2"
  awk -v section="$section" -v label="$label" '
    {
      raw = $0
      line = raw
      sub(/^[[:space:]]+/, "", line)
      if (line == section ":") {
        in_section = 1
        next
      }
      if (in_section && raw !~ /^[[:space:]]/ && line ~ /:$/) {
        in_section = 0
      }
      if (in_section && index(line, label ":") == 1) {
        count++
      }
    }
    END {
      print count + 0
    }
  '
}

dotnet_info_section_field_value() {
  local section="$1"
  local label="$2"
  awk -v section="$section" -v label="$label" '
    {
      raw = $0
      line = raw
      sub(/^[[:space:]]+/, "", line)
      if (line == section ":") {
        in_section = 1
        next
      }
      if (in_section && raw !~ /^[[:space:]]/ && line ~ /:$/) {
        in_section = 0
      }
      if (in_section && index(line, label ":") == 1) {
        value = substr(line, length(label) + 2)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        print value
        exit
      }
    }
  '
}

dotnet_test_passed_count() {
  local dotnet_output="$1"
  printf '%s\n' "$dotnet_output" | "$SCCP_CORRIDOR_PYTHON_BIN" -c '
import re
import sys

ASSEMBLY = "Hyperledger.Iroha.Sdk.Tests.dll (net8.0)"
DURATION = (
    r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[ ]+(?:ms|s|m|h)"
    r"(?:[ ]+(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[ ]+(?:ms|s|m|h))*"
)
SUMMARY_RE = re.compile(
    r"^[ ]*Passed![ ]+-[ ]+Failed:[ ]+0,[ ]+Passed:[ ]+(?P<passed>[1-9][0-9]*),"
    r"[ ]+Skipped:[ ]+(?P<skipped>0),"
    rf"[ ]+Total:[ ]+(?P<total>[1-9][0-9]*),[ ]+Duration:[ ]+"
    rf"(?P<duration>{DURATION})[ ]+-[ ]+"
    rf"{re.escape(ASSEMBLY)}$"
)

matches = []
for line in sys.stdin.read().splitlines():
    normalized = line.rstrip("\r")
    match = SUMMARY_RE.fullmatch(normalized)
    if match is not None:
        matches.append(match)

if len(matches) != 1:
    print(
        "SCCP .NET SDK validation requires exactly one canonical VSTest summary "
        f"for {ASSEMBLY}; found: {len(matches)}",
        file=sys.stderr,
    )
    sys.exit(1)

match = matches[0]
passed = int(match.group("passed"))
skipped = int(match.group("skipped"))
total = int(match.group("total"))
if skipped != 0 or total != passed:
    print(
        "SCCP .NET SDK validation requires VSTest summary to report Failed: 0, Skipped: 0, and Total == Passed",
        file=sys.stderr,
    )
    sys.exit(1)

print(passed)
'
}

validate_dotnet_trx_content() {
  local trx_path="$1"
  local expected_passed_count="$2"
  local trx_bytes
  if ! [[ "$expected_passed_count" =~ ^[1-9][0-9]*$ ]]; then
    echo "SCCP .NET SDK validation requires a positive VSTest passed-test count before TRX XML parsing: $trx_path" >&2
    return 1
  fi
  trx_bytes="$(wc -c < "$trx_path" | tr -d '[:space:]')"
  if ! [[ "$trx_bytes" =~ ^[1-9][0-9]*$ ]]; then
    echo "SCCP .NET SDK validation requires TRX result to have a positive byte size: $trx_path" >&2
    return 1
  fi
  if (( trx_bytes > SCCP_DOTNET_TRX_MAX_BYTES )); then
    echo "SCCP .NET SDK validation requires TRX result to be at most $SCCP_DOTNET_TRX_MAX_BYTES bytes before XML parsing: $trx_path" >&2
    return 1
  fi
  "$SCCP_CORRIDOR_PYTHON_BIN" - "$trx_path" "$expected_passed_count" <<'PY'
import sys
import re
import xml.etree.ElementTree as ET

ASSEMBLY = "Hyperledger.Iroha.Sdk.Tests.dll"
SCCP_TEST_NAME_RE = re.compile(r"(^|[.])Sccp[A-Za-z0-9_]*(?:$|[.])")
ASCII_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")
trx_path = sys.argv[1]
expected_passed_count = int(sys.argv[2])


def fail(message):
    print(f"SCCP .NET SDK validation {message}: {trx_path}", file=sys.stderr)
    sys.exit(1)


try:
    trx_bytes = open(trx_path, "rb").read()
except OSError:
    fail("requires TRX result to be readable XML")

lowered_trx = trx_bytes.lower()
if b"<!doctype" in lowered_trx or b"<!entity" in lowered_trx:
    fail("requires TRX result to contain no DTD or entity declarations")

try:
    root = ET.fromstring(trx_bytes)
except ET.ParseError:
    fail("requires TRX result to be well-formed XML")


def local_name(tag):
    return tag.rsplit("}", 1)[-1]


if local_name(root.tag) != "TestRun":
    fail("requires TRX root to be a VSTest TestRun")


def path_basename(value):
    return value.replace("\\", "/").rsplit("/", 1)[-1]


def is_canonical_trx_test_name(value):
    return bool(
        value
        and value == value.strip()
        and ASCII_CONTROL_RE.search(value) is None
    )


def has_sccp_test_name_token(value):
    return bool(is_canonical_trx_test_name(value) and SCCP_TEST_NAME_RE.search(value))


assembly_sccp_test_ids = set()
assembly_sccp_execution_ids = set()
execution_id_to_test_id = {}
sccp_test_id_to_names = {}
sccp_execution_id_to_names = {}
seen_unit_test_ids = set()
seen_execution_ids = set()
has_expected_assembly = False

results_sections = [child for child in root if local_name(child.tag) == "Results"]
test_definition_sections = [
    child for child in root if local_name(child.tag) == "TestDefinitions"
]
all_unit_results = [
    element for element in root.iter() if local_name(element.tag) == "UnitTestResult"
]
unit_results = (
    [child for child in results_sections[0] if local_name(child.tag) == "UnitTestResult"]
    if len(results_sections) == 1
    else []
)
if all_unit_results and (
    len(results_sections) != 1 or len(unit_results) != len(all_unit_results)
):
    fail(
        "requires every TRX UnitTestResult to appear directly under the VSTest Results section"
    )
all_unit_tests = [
    element for element in root.iter() if local_name(element.tag) == "UnitTest"
]
unit_tests = (
    [
        child
        for child in test_definition_sections[0]
        if local_name(child.tag) == "UnitTest"
    ]
    if len(test_definition_sections) == 1
    else []
)
if all_unit_tests and (
    len(test_definition_sections) != 1 or len(unit_tests) != len(all_unit_tests)
):
    fail(
        "requires every TRX UnitTest definition to appear directly under the VSTest TestDefinitions section"
    )

for element in unit_tests:
    element_has_expected_assembly = False
    for attribute_name in ("codeBase", "storage"):
        attribute_value = element.attrib.get(attribute_name)
        if attribute_value and path_basename(attribute_value) == ASSEMBLY:
            element_has_expected_assembly = True
            has_expected_assembly = True
    test_id = element.attrib.get("id")
    if test_id:
        if test_id in seen_unit_test_ids:
            fail("requires unique TRX UnitTest id values")
        seen_unit_test_ids.add(test_id)
    definition_values = [
        element.attrib.get("name"),
        element.attrib.get("className"),
    ]
    definition_result_names = set(
        value for value in (element.attrib.get("name"),) if value
    )
    definition_has_expected_assembly = element_has_expected_assembly
    for child in element:
        child_name = local_name(child.tag)
        if child_name == "Execution":
            execution_id = child.attrib.get("id")
            if execution_id:
                if execution_id in seen_execution_ids:
                    fail("requires unique TRX Execution id values")
                seen_execution_ids.add(execution_id)
                if test_id:
                    execution_id_to_test_id[execution_id] = test_id
        if child_name == "TestMethod":
            for attribute_name in ("codeBase", "storage"):
                attribute_value = child.attrib.get(attribute_name)
                if attribute_value and path_basename(attribute_value) == ASSEMBLY:
                    definition_has_expected_assembly = True
                    has_expected_assembly = True
            definition_values.extend(
                (
                    child.attrib.get("name"),
                    child.attrib.get("className"),
                )
            )
            child_name_value = child.attrib.get("name")
            child_class_name = child.attrib.get("className")
            if child_name_value and child_class_name:
                definition_result_names.add(f"{child_class_name}.{child_name_value}")
    if definition_has_expected_assembly and any(
        has_sccp_test_name_token(value) for value in definition_values
    ):
        if test_id:
            assembly_sccp_test_ids.add(test_id)
            sccp_test_id_to_names[test_id] = set(definition_result_names)
        for child in element:
            if local_name(child.tag) == "Execution":
                execution_id = child.attrib.get("id")
                if execution_id:
                    assembly_sccp_execution_ids.add(execution_id)
                    sccp_execution_id_to_names[execution_id] = set(
                        definition_result_names
                    )

if not has_expected_assembly:
    fail("requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll")

for result in unit_results:
    if result.attrib.get("outcome") != "Passed":
        fail(
            "requires TRX result to contain no failed, skipped, timed-out, or aborted SCCP test results"
        )
    result_test_id = result.attrib.get("testId")
    result_execution_id = result.attrib.get("executionId")
    test_id_matches = result_test_id in assembly_sccp_test_ids
    execution_id_matches = result_execution_id in assembly_sccp_execution_ids
    if not (test_id_matches or execution_id_matches):
        fail(
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition"
        )
    if result_test_id and not test_id_matches:
        fail(
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition"
        )
    if result_execution_id and not execution_id_matches:
        fail(
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition"
        )
    if (
        result_test_id
        and result_execution_id
        and execution_id_to_test_id.get(result_execution_id) != result_test_id
    ):
        fail(
            "requires TRX UnitTestResult testId and executionId to bind the same SCCP test definition"
        )
    result_test_name = result.attrib.get("testName")
    if result_test_name is not None:
        allowed_result_names = set()
        if result_test_id:
            allowed_result_names.update(sccp_test_id_to_names.get(result_test_id, set()))
        if result_execution_id:
            allowed_result_names.update(
                sccp_execution_id_to_names.get(result_execution_id, set())
            )
        if (
            result_test_name not in allowed_result_names
            or not has_sccp_test_name_token(result_test_name)
        ):
            fail(
                "requires TRX UnitTestResult testName to match its SCCP test definition"
            )

has_passed_sccp_result = False
for result in unit_results:
    if result.attrib.get("outcome") != "Passed":
        continue
    if (
        result.attrib.get("testId") in assembly_sccp_test_ids
        or result.attrib.get("executionId") in assembly_sccp_execution_ids
    ):
        has_passed_sccp_result = True
        break

if not has_passed_sccp_result:
    fail("requires TRX result to contain at least one passed SCCP test result")

if len(unit_results) != expected_passed_count:
    fail("requires TRX UnitTestResult count to match VSTest passed-test count")
PY
}

ensure_swift_bridge_artifact() {
  local bridge_dir="$ROOT/dist/NoritoBridge.xcframework"
  local bridge_zip="$ROOT/dist/NoritoBridge.xcframework.zip"
  local rust_targets=(
    aarch64-apple-ios
    aarch64-apple-ios-sim
    x86_64-apple-ios
    aarch64-apple-darwin
  )

  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -f "$bridge_zip" ]]; then
      run_cmd rm -rf "$bridge_dir"
      run_cmd unzip -q -o "$bridge_zip" -d "$ROOT/dist"
    else
      run_cmd rustup target add "${rust_targets[@]}"
      run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh"
    fi
    return 0
  fi

  if [[ -f "$bridge_dir/Info.plist" ]]; then
    return 0
  fi

  if [[ -f "$bridge_zip" ]]; then
    run_cmd rm -rf "$bridge_dir"
    run_cmd unzip -q -o "$bridge_zip" -d "$ROOT/dist"
  else
    run_cmd rustup target add "${rust_targets[@]}"
    run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh"
  fi

  if [[ ! -f "$bridge_dir/Info.plist" ]]; then
    echo "NoritoBridge.xcframework was not materialized at $bridge_dir." >&2
    echo "Provide $bridge_zip or ensure scripts/build_norito_xcframework.sh can build it." >&2
    return 1
  fi
}

resolve_java_home() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${JAVA_HOME:-}" ]]; then
      printf '%s\n' "$JAVA_HOME"
    else
      printf '%s\n' "$ROOT/target/java/jdk-21/Contents/Home"
    fi
    return 0
  fi

  if [[ -n "${JAVA_HOME:-}" ]] && is_java_21_home "$JAVA_HOME"; then
    printf '%s\n' "$JAVA_HOME"
    return 0
  fi

  local bundled="$ROOT/target/java/jdk-21/Contents/Home"
  if is_java_21_home "$bundled"; then
    printf '%s\n' "$bundled"
    return 0
  fi

  if command -v /usr/libexec/java_home >/dev/null 2>&1; then
    local macos_java_home
    if macos_java_home="$(/usr/libexec/java_home -v 21 2>/dev/null)" \
      && is_java_21_home "$macos_java_home"; then
      printf '%s\n' "$macos_java_home"
      return 0
    fi
  fi

  local homebrew_candidates=(
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /opt/homebrew/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
    /usr/local/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
  )
  local candidate
  for candidate in "${homebrew_candidates[@]}"; do
    if is_java_21_home "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  echo "JDK 21 not found. Set JAVA_HOME, install the repo-local target/java/jdk-21 bundle, or install Homebrew openjdk@21." >&2
  return 1
}

is_java_21_home() {
  local java_home="$1"
  local version_line
  [[ -x "$java_home/bin/java" ]] || return 1
  version_line="$("$java_home/bin/java" -version 2>&1 | head -n 1)"
  [[ "$version_line" =~ version[[:space:]]+\"21(\.|\") ]]
}

run_java_version_check() {
  local java_home="$1"
  run_cmd \
    env "JAVA_HOME=$java_home" "PATH=$java_home/bin:$PATH" \
    java -version
}

resolve_android_home() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${ANDROID_HOME:-}" ]]; then
      printf '%s\n' "$ANDROID_HOME"
    else
      printf '%s\n' "$HOME/Library/Android/sdk"
    fi
    return 0
  fi

  if [[ -n "${ANDROID_HOME:-}" && -d "$ANDROID_HOME" ]]; then
    printf '%s\n' "$ANDROID_HOME"
    return 0
  fi

  local default_sdk="$HOME/Library/Android/sdk"
  if [[ -d "$default_sdk" ]]; then
    printf '%s\n' "$default_sdk"
    return 0
  fi

  echo "Android SDK not found. Set ANDROID_HOME for the java-android phase." >&2
  return 1
}

resolve_dotnet() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${DOTNET_ROOT:-}" ]]; then
      printf '%s\n' "$DOTNET_ROOT/dotnet"
    else
      printf '%s\n' "$ROOT/target/dotnet/dotnet"
    fi
    return 0
  fi

  if [[ -n "${DOTNET_ROOT:-}" && -x "$DOTNET_ROOT/dotnet" ]]; then
    printf '%s\n' "$DOTNET_ROOT/dotnet"
    return 0
  fi

  local local_dotnet="/tmp/iroha-dotnet/sdk/dotnet"
  if [[ -x "$local_dotnet" ]]; then
    printf '%s\n' "$local_dotnet"
    return 0
  fi

  if command -v dotnet >/dev/null 2>&1; then
    command -v dotnet
    return 0
  fi

  echo "dotnet not found. Set DOTNET_ROOT, install /tmp/iroha-dotnet/sdk, or install dotnet on PATH." >&2
  return 1
}

phase_rust_sccp() {
  run_cmd \
    env "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" "NORITO_SKIP_BINDINGS_SYNC=$NORITO_SKIP_BINDINGS_SYNC" \
    cargo test -p iroha_sccp -- --nocapture
}

phase_evidence_scripts() {
  local tests=(
    pytests/scripts/check_sccp_production_corridor_test.py
    pytests/scripts/sccp_release_bundle_test.py
    pytests/scripts/sccp_release_readiness_report_test.py
    pytests/scripts/sccp_all_lanes_evidence_test.py
    pytests/scripts/sccp_eth_source_bridge_evidence_test.py
    pytests/scripts/sccp_bsc_source_bridge_evidence_test.py
    pytests/scripts/sccp_evm_destination_evidence_test.py
    pytests/scripts/sccp_evm_live_evidence_test.py
    pytests/scripts/sccp_evm_receipt_proof_evidence_test.py
    pytests/scripts/sccp_evm_source_live_evidence_test.py
    pytests/scripts/sccp_solana_destination_evidence_test.py
    pytests/scripts/sccp_solana_live_evidence_test.py
    pytests/scripts/sccp_solana_source_state_evidence_test.py
    pytests/scripts/sccp_ton_destination_evidence_test.py
    pytests/scripts/sccp_ton_live_evidence_test.py
    pytests/scripts/sccp_ton_source_state_evidence_test.py
    pytests/scripts/sccp_tron_live_evidence_test.py
    pytests/scripts/sccp_tron_source_bridge_evidence_test.py
    pytests/scripts/sccp_retired_network_surface_test.py
  )
  run_cmd "$SCCP_CORRIDOR_PYTHON_BIN" -m pytest -q "${tests[@]}"
}

phase_js_sdk() {
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --test \
    javascript/iroha_js/test/sccpSolanaProver.test.js \
    javascript/iroha_js/test/sccpEthereumMainnet.test.js \
    javascript/iroha_js/test/sccpBscMainnet.test.js \
    javascript/iroha_js/test/package_dist.test.js \
    javascript/iroha_js/test/sccpPackageExports.test.js
}

phase_python_sdk() {
  run_cmd "$SCCP_CORRIDOR_PYTHON_BIN" -m pytest -q python/iroha_torii_client/tests/sccp_test.py
}

phase_swift_sdk() {
  ensure_swift_bridge_artifact
  run_in_dir "$ROOT/IrohaSwift" \
    swift test --filter SccpSolanaProverTests --disable-swift-testing
  run_in_dir "$ROOT/IrohaSwift" \
    swift test --filter \
      ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions \
      --disable-swift-testing
}

phase_kotlin_sdk() {
  local java_home
  java_home="$(resolve_java_home)"
  run_java_version_check "$java_home"
  run_in_dir "$ROOT/kotlin" \
    env "JAVA_HOME=$java_home" "GRADLE_OPTS=$SCCP_GRADLE_OPTS" "PATH=$java_home/bin:$PATH" \
    ./gradlew :core-jvm:test --console=plain \
      --tests 'org.hyperledger.iroha.sdk.sccp.*' \
      --tests 'org.hyperledger.iroha.sdk.sccp.TonSccpProverTest'
}

phase_java_android() {
  local java_home
  local android_home
  local android_sdk_root
  local android_harness_mains
  java_home="$(resolve_java_home)"
  android_home="$(resolve_android_home)"
  android_sdk_root="${ANDROID_SDK_ROOT:-$android_home}"
  android_harness_mains="org.hyperledger.iroha.android.sccp.EvmSccpProverTests,org.hyperledger.iroha.android.sccp.SourceSccpProofsTests,org.hyperledger.iroha.android.sccp.TonSccpProverTests,org.hyperledger.iroha.android.sccp.TronSccpProverTests"
  run_java_version_check "$java_home"
  run_in_dir "$ROOT/java/iroha_android" \
    env "JAVA_HOME=$java_home" "ANDROID_HOME=$android_home" "ANDROID_SDK_ROOT=$android_sdk_root" "GRADLE_OPTS=$SCCP_GRADLE_OPTS" "PATH=$java_home/bin:$PATH" \
    "ANDROID_HARNESS_MAINS=$android_harness_mains" \
    ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests
  run_in_dir "$ROOT/java/iroha_android" \
    env "JAVA_HOME=$java_home" "ANDROID_HOME=$android_home" "ANDROID_SDK_ROOT=$android_sdk_root" "GRADLE_OPTS=$SCCP_GRADLE_OPTS" "PATH=$java_home/bin:$PATH" \
    ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests
}

phase_dotnet_sdk() {
  local bridge_library_dir
  local bridge_library_path
  local bridge_library_sha256
  local bridge_target_dir
  local dotnet_cli
  local dotnet_root
  local dotnet_trx_bytes
  local dotnet_trx_display
  local dotnet_trx_path
  local dotnet_trx_paths
  local dotnet_version
  local dotnet_info
  local dotnet_test_output
  local dotnet_passed_count
  local dotnet_os_name
  local dotnet_os_name_count
  local dotnet_os_platform
  local dotnet_os_platform_count
  local dotnet_rid
  local dotnet_rid_count
  local dotnet_arch
  local dotnet_arch_count
  local dotnet_arch_lc
  bridge_target_dir="${SCCP_DOTNET_BRIDGE_TARGET_DIR:-$CARGO_TARGET_DIR}"
  case "$bridge_target_dir" in
    /* | [A-Za-z]:/* | [A-Za-z]:\\*)
      ;;
    *)
      bridge_target_dir="$ROOT/$bridge_target_dir"
      ;;
  esac
  bridge_library_dir="$bridge_target_dir/debug"
  bridge_library_path="$bridge_library_dir/connect_norito_bridge.dll"
  dotnet_cli="$(resolve_dotnet)"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    dotnet_root="$(dirname "$dotnet_cli")"
  else
    dotnet_root="$(cd "$(dirname "$dotnet_cli")" && pwd)"
  fi
  local dotnet_env=(
    env
    "DOTNET_ROOT=$dotnet_root"
    "DOTNET_CLI_TELEMETRY_OPTOUT=1"
    "DOTNET_CLI_UI_LANGUAGE=en"
  )
  run_capture_in_dir dotnet_version "$ROOT/csharp" \
    "${dotnet_env[@]}" "$dotnet_cli" --version
  if [[ "$DRY_RUN" -eq 0 ]]; then
    dotnet_version="${dotnet_version//$'\r'/}"
    if [[ "$dotnet_version" == *$'\n'* ]]; then
      echo "SCCP .NET SDK validation requires dotnet --version to emit exactly one canonical SDK version line." >&2
      return 1
    fi
    if [[ ! "$dotnet_version" =~ ^8\.0\.[1-9][0-9]*$ ]]; then
      echo "SCCP .NET SDK validation requires a stable canonical .NET 8.0.x SDK version with a non-zero patch; found: $dotnet_version" >&2
      return 1
    fi
    printf 'SCCP .NET SDK version: %s\n' "$dotnet_version"
  fi
  run_capture_in_dir dotnet_info "$ROOT/csharp" \
    "${dotnet_env[@]}" "$dotnet_cli" --info
  if [[ "$DRY_RUN" -eq 0 ]]; then
    dotnet_os_name_count="$(dotnet_info_field_count "OS Name" <<<"$dotnet_info")"
    dotnet_os_platform_count="$(dotnet_info_field_count "OS Platform" <<<"$dotnet_info")"
    if [[ "$dotnet_os_name_count" != 1 || "$dotnet_os_platform_count" != 1 ]]; then
      echo "SCCP .NET SDK validation requires exactly one OS Name and one OS Platform from dotnet --info." >&2
      return 1
    fi
    dotnet_os_name="$(dotnet_info_field_value "OS Name" <<<"$dotnet_info")"
    dotnet_os_platform="$(dotnet_info_field_value "OS Platform" <<<"$dotnet_info")"
    if [[ "$dotnet_os_name" != "Windows" || "$dotnet_os_platform" != "Windows" ]]; then
      echo "SCCP .NET SDK validation must be captured on Windows; found OS Name: $dotnet_os_name, OS Platform: $dotnet_os_platform" >&2
      return 1
    fi
    dotnet_rid_count="$(dotnet_info_field_count "RID" <<<"$dotnet_info")"
    if [[ "$dotnet_rid_count" != 1 ]]; then
      echo "SCCP .NET SDK validation requires exactly one canonical Windows RID from dotnet --info; found: $dotnet_rid_count" >&2
      return 1
    fi
    dotnet_rid="$(dotnet_info_field_value "RID" <<<"$dotnet_info")"
    if [[ ! "$dotnet_rid" =~ ^win-(x64|x86|arm64|arm)$ ]]; then
      echo "SCCP .NET SDK validation requires a canonical Windows RID; found: $dotnet_rid" >&2
      return 1
    fi
    dotnet_arch_count="$(dotnet_info_field_count "OS Architecture" <<<"$dotnet_info")"
    if [[ "$dotnet_arch_count" == 1 ]]; then
      dotnet_arch="$(dotnet_info_field_value "OS Architecture" <<<"$dotnet_info")"
    else
      if [[ "$dotnet_arch_count" != 0 ]]; then
        echo "SCCP .NET SDK validation requires exactly one OS Architecture from dotnet --info; found: $dotnet_arch_count" >&2
        return 1
      fi
      dotnet_arch_count="$(dotnet_info_section_field_count "Host" "Architecture" <<<"$dotnet_info")"
      if [[ "$dotnet_arch_count" != 1 ]]; then
        echo "SCCP .NET SDK validation requires exactly one Host Architecture from dotnet --info when OS Architecture is absent; found: $dotnet_arch_count" >&2
        return 1
      fi
      dotnet_arch="$(dotnet_info_section_field_value "Host" "Architecture" <<<"$dotnet_info")"
    fi
    if [[ ! "$dotnet_arch" =~ ^(x64|x86|arm64|arm)$ ]]; then
      echo "SCCP .NET SDK validation requires a canonical architecture; found: $dotnet_arch" >&2
      return 1
    fi
    dotnet_arch_lc="$dotnet_arch"
    if [[ "${dotnet_rid#win-}" != "$dotnet_arch_lc" ]]; then
      echo "SCCP .NET SDK validation requires the Windows RID architecture to match the reported architecture; found RID: $dotnet_rid, architecture: $dotnet_arch_lc" >&2
      return 1
    fi
    printf 'SCCP .NET SDK OS: Windows\n'
    printf 'SCCP .NET SDK RID: %s\n' "$dotnet_rid"
    printf 'SCCP .NET SDK Architecture: %s\n' "$dotnet_arch_lc"
  fi
  run_in_dir "$ROOT" \
    env "CARGO_TARGET_DIR=$bridge_target_dir" \
    cargo build -p connect_norito_bridge
  if [[ "$DRY_RUN" -eq 0 ]]; then
    if [[ ! -f "$bridge_library_path" ]]; then
      echo "SCCP .NET SDK validation requires freshly built connect_norito_bridge.dll at $bridge_library_path" >&2
      return 1
    fi
    if command -v sha256sum >/dev/null 2>&1; then
      bridge_library_sha256="$(sha256sum "$bridge_library_path" | cut -d ' ' -f 1)"
    elif command -v shasum >/dev/null 2>&1; then
      bridge_library_sha256="$(shasum -a 256 "$bridge_library_path" | cut -d ' ' -f 1)"
    else
      echo "SCCP .NET SDK validation requires sha256sum or shasum to record the native bridge digest" >&2
      return 1
    fi
    if [[ ! "$bridge_library_sha256" =~ ^[0-9a-f]{64}$ ]]; then
      echo "SCCP .NET SDK validation produced a non-canonical native bridge SHA-256: $bridge_library_sha256" >&2
      return 1
    fi
    printf 'connect_norito_bridge native bridge: %s\n' "$bridge_library_path"
    printf 'connect_norito_bridge native bridge sha256: %s\n' "$bridge_library_sha256"
  fi
  run_in_dir "$ROOT/csharp" \
    "${dotnet_env[@]}" \
    "PATH=$bridge_library_dir:$PATH" \
    "$dotnet_cli" restore Hyperledger.Iroha.Sdk.sln
  if [[ "$DRY_RUN" -eq 0 ]]; then
    while IFS= read -r -d '' dotnet_trx_path; do
      rm -f "$dotnet_trx_path"
    done < <(
      find "$ROOT/csharp/tests/Hyperledger.Iroha.Sdk.Tests" \
        -path '*/TestResults/sccp-dotnet-sdk.trx' \
        -type f \
        -print0 2>/dev/null
    )
  fi
  run_capture_in_dir dotnet_test_output "$ROOT/csharp" \
    "${dotnet_env[@]}" \
    "PATH=$bridge_library_dir:$PATH" \
    "$dotnet_cli" test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    --filter "FullyQualifiedName~Sccp" \
    --nologo \
    --logger "trx;LogFileName=sccp-dotnet-sdk.trx"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    if ! dotnet_passed_count="$(dotnet_test_passed_count "$dotnet_test_output")"; then
      return 1
    fi
    dotnet_trx_paths=()
    while IFS= read -r -d '' dotnet_trx_path; do
      dotnet_trx_paths+=("$dotnet_trx_path")
    done < <(
      find "$ROOT/csharp/tests/Hyperledger.Iroha.Sdk.Tests" \
        -path '*/TestResults/sccp-dotnet-sdk.trx' \
        -type f \
        -print0 2>/dev/null
    )
    if [[ "${#dotnet_trx_paths[@]}" -ne 1 ]]; then
      echo "SCCP .NET SDK validation requires exactly one .NET TRX result; found: ${#dotnet_trx_paths[@]}" >&2
      return 1
    fi
    dotnet_trx_path="${dotnet_trx_paths[0]}"
    dotnet_trx_display="${dotnet_trx_path#$ROOT/}"
    case "$dotnet_trx_display" in
      csharp/tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx)
        ;;
      *)
        echo "SCCP .NET SDK validation produced an unexpected TRX path: $dotnet_trx_path" >&2
        return 1
        ;;
    esac
    if [[ ! -s "$dotnet_trx_path" ]]; then
      echo "SCCP .NET SDK validation produced an empty TRX result: $dotnet_trx_path" >&2
      return 1
    fi
    validate_dotnet_trx_content "$dotnet_trx_path" "$dotnet_passed_count"
    dotnet_trx_bytes="$(wc -c < "$dotnet_trx_path" | tr -d '[:space:]')"
    if ! [[ "$dotnet_trx_bytes" =~ ^[1-9][0-9]*$ ]]; then
      echo "SCCP .NET SDK validation could not compute a non-zero TRX byte size: $dotnet_trx_path" >&2
      return 1
    fi
    printf 'SCCP .NET SDK TRX: %s\n' "$dotnet_trx_display"
    printf 'SCCP .NET SDK TRX bytes: %s\n' "$dotnet_trx_bytes"
  fi
}

phase_contract_smoke() {
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --test scripts/sccp_bsc_groth16_material.test.mjs scripts/sccp_bsc_taira_xor_deploy.test.mjs scripts/sccp_tron_taira_xor_deploy.test.mjs scripts/sccp_taira_xor_contract.test.mjs
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js
  run_cmd bash scripts/sccp_evm_contract_smoke.sh
}

phase_core_admission() {
  run_cmd \
    env "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" "NORITO_SKIP_BINDINGS_SYNC=$NORITO_SKIP_BINDINGS_SYNC" \
    cargo test -p iroha_core --test iroha_core_group_01 bridge_proofs:: -- --nocapture
}

run_with_log_dir() {
  local phase
  mkdir -p "$LOG_DIR"
  while IFS= read -r phase; do
    [[ -n "$phase" ]] || continue
    bash "$ROOT/scripts/check_sccp_production_corridor.sh" \
      --phase "$phase" 2>&1 | tee "$LOG_DIR/$phase.log"
  done < <(selected_phases)
  printf '\nSCCP production corridor logs written to %s\n' "$LOG_DIR"
}

main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --phase)
        if [[ $# -lt 2 ]]; then
          echo "--phase requires a value." >&2
          exit 2
        fi
        add_phase "$2"
        shift 2
        ;;
      --phase=*)
        add_phase "${1#--phase=}"
        shift
        ;;
      --list)
        list_phases
        exit 0
        ;;
      --dry-run)
        DRY_RUN=1
        shift
        ;;
      --log-dir)
        if [[ $# -lt 2 ]]; then
          echo "--log-dir requires a directory." >&2
          exit 2
        fi
        LOG_DIR="$2"
        if [[ -z "$LOG_DIR" ]]; then
          echo "--log-dir requires a directory." >&2
          exit 2
        fi
        shift 2
        ;;
      --log-dir=*)
        LOG_DIR="${1#--log-dir=}"
        if [[ -z "$LOG_DIR" ]]; then
          echo "--log-dir requires a directory." >&2
          exit 2
        fi
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown option: $1" >&2
        usage >&2
        exit 2
        ;;
    esac
  done

  if [[ -n "$LOG_DIR" && "$DRY_RUN" -eq 0 ]]; then
    run_with_log_dir
    return 0
  fi

  if [[ -n "$LOG_DIR" ]]; then
    printf 'SCCP production corridor logs would be written to %s\n' "$LOG_DIR"
  fi

  local phase
  for phase in "${ALL_PHASES[@]}"; do
    if ! phase_selected "$phase"; then
      continue
    fi

    printf '\n==> SCCP production corridor: %s\n' "$phase"
    case "$phase" in
      rust-sccp)
        phase_rust_sccp
        ;;
      evidence-scripts)
        phase_evidence_scripts
        ;;
      js-sdk)
        phase_js_sdk
        ;;
      python-sdk)
        phase_python_sdk
        ;;
      swift-sdk)
        phase_swift_sdk
        ;;
      kotlin-sdk)
        phase_kotlin_sdk
        ;;
      java-android)
        phase_java_android
        ;;
      dotnet-sdk)
        phase_dotnet_sdk
        ;;
      contract-smoke)
        phase_contract_smoke
        ;;
      core-admission)
        phase_core_admission
        ;;
    esac
  done

  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '\nSCCP production corridor dry run completed.\n'
  else
    printf '\nSCCP production corridor completed.\n'
  fi
}

main "$@"
