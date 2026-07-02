#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="${BASH_SOURCE[0]%/*}"
if [[ "$SCRIPT_DIR" == "${BASH_SOURCE[0]}" ]]; then
  SCRIPT_DIR="."
fi
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
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

path_list_has_empty_segment() {
  local value="$1"
  [[ -z "$value" \
    || "$value" == :* \
    || "$value" == \;* \
    || "$value" == *: \
    || "$value" == *\; \
    || "$value" == *::* \
    || "$value" == *\;\;* \
    || "$value" == *:\;* \
    || "$value" == *\;:* ]]
}

validate_nonempty_path_list() {
  local label="$1"
  local value="$2"
  if path_list_has_empty_segment "$value"; then
    echo "SCCP .NET SDK validation requires $label to contain no empty path-list segments before native bridge loader setup." >&2
    return 1
  fi
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
import unicodedata

ASSEMBLY = "Hyperledger.Iroha.Sdk.Tests.dll (net8.0)"
NUMBER = r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?"
DURATION = (
    rf"(?:{NUMBER}[ ]+h(?:[ ]+{NUMBER}[ ]+m)?(?:[ ]+{NUMBER}[ ]+s)?(?:[ ]+{NUMBER}[ ]+ms)?"
    rf"|{NUMBER}[ ]+m(?:[ ]+{NUMBER}[ ]+s)?(?:[ ]+{NUMBER}[ ]+ms)?"
    rf"|{NUMBER}[ ]+s(?:[ ]+{NUMBER}[ ]+ms)?"
    rf"|{NUMBER}[ ]+ms)"
)
SUMMARY_RE = re.compile(
    r"^[ ]*Passed![ ]+-[ ]+Failed:[ ]+0,[ ]+Passed:[ ]+(?P<passed>[1-9][0-9]*),"
    r"[ ]+Skipped:[ ]+(?P<skipped>0),"
    rf"[ ]+Total:[ ]+(?P<total>[1-9][0-9]*),[ ]+Duration:[ ]+"
    rf"(?P<duration>{DURATION})[ ]+-[ ]+"
    rf"{re.escape(ASSEMBLY)}$"
)
SUMMARY_LIKE_RE = re.compile(r"^[ ]*Passed!\s*-")
ANSI_ESCAPE_PATTERN = re.compile(
    r"\x1b(?:"
    r"\[[0-?]*[ -/]*[@-~]"
    r"|\][^\x07]*(?:\x07|\x1b\\)"
    r"|[PX^_][^\x1b]*(?:\x1b\\)"
    r"|[@-Z\\-_]"
    r")"
)
ASCII_CONTROL_CHARACTER_PATTERN = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")


def normalized_summary_line(line):
    stripped_line = ASCII_CONTROL_CHARACTER_PATTERN.sub(
        "",
        ANSI_ESCAPE_PATTERN.sub("", line),
    )
    return "".join(
        character
        for character in stripped_line
        if unicodedata.category(character) != "Cf"
    )

matches = []
malformed_summary_lines = 0
for line in sys.stdin.read().splitlines():
    normalized = line.rstrip("\r")
    scan_line = normalized_summary_line(normalized)
    raw_match = SUMMARY_RE.fullmatch(normalized)
    normalized_match = (
        SUMMARY_RE.fullmatch(scan_line) if scan_line != normalized else None
    )
    if raw_match is not None:
        matches.append(raw_match)
    elif normalized_match is not None:
        malformed_summary_lines += 1
    elif SUMMARY_LIKE_RE.match(normalized) or (
        scan_line != normalized and SUMMARY_LIKE_RE.match(scan_line)
    ):
        malformed_summary_lines += 1

if malformed_summary_lines or len(matches) != 1:
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

reject_dotnet_trx_symlink_path() {
  local trx_dir="$1"
  local trx_path="$2"
  reject_symlinked_existing_path_components "$trx_dir" \
    "SCCP .NET SDK validation requires direct TestResults TRX output path to be non-symlinked: $trx_path" || return 1
  if [[ -L "$trx_dir" || -L "$trx_path" ]]; then
    echo "SCCP .NET SDK validation requires direct TestResults TRX output path to be non-symlinked: $trx_path" >&2
    return 1
  fi
}

reject_symlinked_existing_path_components() {
  local path="$1"
  local error_message="$2"
  local current
  local drive_prefix
  local path_parts
  local path_without_trailing_slash
  local part
  local remaining
  path_without_trailing_slash="${path//\\//}"
  path_without_trailing_slash="${path_without_trailing_slash%/}"
  case "$path_without_trailing_slash" in
    [A-Za-z]:/*)
      drive_prefix="${path_without_trailing_slash:0:2}"
      current="$drive_prefix"
      remaining="${path_without_trailing_slash:3}"
      ;;
    /*)
      current="/"
      remaining="${path_without_trailing_slash#/}"
      ;;
    *)
      current="."
      remaining="$path_without_trailing_slash"
      ;;
  esac
  local IFS="/"
  read -r -a path_parts <<< "$remaining"
  for part in "${path_parts[@]}"; do
    if [[ -z "$part" || "$part" == "." ]]; then
      continue
    fi
    if [[ "$current" == "/" ]]; then
      current="/$part"
    else
      current="$current/$part"
    fi
    if [[ -L "$current" ]]; then
      echo "$error_message" >&2
      return 1
    fi
    if [[ ! -e "$current" ]]; then
      break
    fi
  done
  return 0
}

reject_dotnet_bridge_path_text() {
  local label="$1"
  local path="$2"
  if [[ -z "$path" || "$path" =~ [[:space:]] || "$path" =~ [[:cntrl:]] ]]; then
    echo "SCCP .NET SDK validation requires $label path text to be non-empty and free of whitespace or control characters" >&2
    return 1
  fi
}

reject_dotnet_bridge_path_components() {
  local label="$1"
  local path="$2"
  local index
  local normalized
  local part
  local parts
  local remaining
  normalized="${path//\\//}"
  if [[ "$normalized" == *//* || "$normalized" == */./* || "$normalized" == */../* || "$normalized" == */. || "$normalized" == */.. ]]; then
    echo "SCCP .NET SDK validation requires $label path components to be direct and free of empty, dot, or parent segments" >&2
    return 1
  fi
  remaining="${normalized#/}"
  local IFS="/"
  read -r -a parts <<< "$remaining"
  for index in "${!parts[@]}"; do
    part="${parts[$index]}"
    if [[ -z "$part" ]]; then
      continue
    fi
    if [[ "$index" -eq 0 && "$part" =~ ^[A-Za-z]:$ ]]; then
      continue
    fi
    if [[ ! "$part" =~ ^[A-Za-z0-9_.-]+$ ]]; then
      echo "SCCP .NET SDK validation requires $label path components to be canonical and free of non-portable characters" >&2
      return 1
    fi
  done
}

reject_dotnet_runtime_path_text() {
  local label="$1"
  local path="$2"
  if [[ -z "$path" || "$path" =~ [[:cntrl:]] || "$path" =~ ^[[:space:]] || "$path" =~ [[:space:]]$ ]]; then
    echo "SCCP .NET SDK validation requires $label path text to be non-empty and free of surrounding whitespace or control characters" >&2
    return 1
  fi
}

reject_dotnet_path_list_text() {
  local label="$1"
  local path_list="$2"
  if [[ -z "$path_list" || "$path_list" =~ [[:cntrl:]] || "$path_list" == :* || "$path_list" == \;* || "$path_list" == *: || "$path_list" == *\; || "$path_list" == *::* || "$path_list" == *";;"* || "$path_list" == *":;"* || "$path_list" == *";:"* ]]; then
    echo "SCCP .NET SDK validation requires $label to be non-empty, contain no control characters, and contain no empty path-list segments before native bridge loader setup" >&2
    return 1
  fi
}

reject_dotnet_bridge_symlink_path() {
  local bridge_target_dir="$1"
  local bridge_dir="$2"
  local bridge_path="$3"
  reject_symlinked_existing_path_components "$bridge_target_dir" \
    "SCCP .NET SDK validation requires native bridge output path to be non-symlinked: $bridge_path" || return 1
  if [[ -L "$bridge_target_dir" || -L "$bridge_dir" || -L "$bridge_path" ]]; then
    echo "SCCP .NET SDK validation requires native bridge output path to be non-symlinked: $bridge_path" >&2
    return 1
  fi
}

compute_dotnet_bridge_sha256() {
  local bridge_path="$1"
  local digest_output
  if ! digest_output="$("$SCCP_CORRIDOR_PYTHON_BIN" - "$bridge_path" 2>/dev/null <<'PY'
import hashlib
import sys

path = sys.argv[1]
hasher = hashlib.sha256()
with open(path, "rb") as bridge_file:
    for chunk in iter(lambda: bridge_file.read(1024 * 1024), b""):
        hasher.update(chunk)
print(hasher.hexdigest())
PY
)"; then
    echo "SCCP .NET SDK validation requires Python hashlib to record the native bridge digest" >&2
    return 1
  fi
  digest_output="${digest_output//$'\r'/}"
  if [[ "$digest_output" == *$'\n'* || ! "$digest_output" =~ ^[0-9a-f]{64}$ ]]; then
    echo "SCCP .NET SDK validation requires canonical native bridge SHA-256 digest output" >&2
    return 1
  fi
  printf '%s\n' "$digest_output"
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
import urllib.parse
import xml.etree.ElementTree as ET

ASSEMBLY = "Hyperledger.Iroha.Sdk.Tests.dll"
EXPECTED_TEST_NAMESPACE = "Hyperledger.Iroha.Sdk.Tests."
VSTEST_XML_NAMESPACE = "http://microsoft.com/schemas/VisualStudio/TeamTest/2010"
TRUSTED_VSTEST_ELEMENT_NAMES = frozenset(
    (
        "TestRun",
        "Results",
        "TestDefinitions",
        "UnitTestResult",
        "UnitTest",
        "Execution",
        "TestMethod",
    )
)
TRUSTED_VSTEST_ATTRIBUTES_BY_ELEMENT = {
    "TestRun": frozenset(("id", "name", "runUser")),
    "Results": frozenset(),
    "TestDefinitions": frozenset(),
    "UnitTestResult": frozenset(
        (
            "computerName",
            "duration",
            "endTime",
            "executionId",
            "isExecuted",
            "outcome",
            "relativeResultsDirectory",
            "startTime",
            "testId",
            "testListId",
            "testName",
            "testType",
        )
    ),
    "UnitTest": frozenset(("adapterTypeName", "id", "name", "storage")),
    "Execution": frozenset(("id",)),
    "TestMethod": frozenset(("adapterTypeName", "className", "codeBase", "name")),
}
TRUSTED_VSTEST_METADATA_ATTRIBUTES_BY_ELEMENT = {
    "TestRun": frozenset(("id", "name", "runUser")),
    "UnitTestResult": frozenset(
        (
            "computerName",
            "duration",
            "endTime",
            "relativeResultsDirectory",
            "startTime",
            "testListId",
            "testType",
        )
    ),
    "UnitTest": frozenset(("adapterTypeName",)),
    "TestMethod": frozenset(("adapterTypeName",)),
}
SCCP_TEST_NAME_RE = re.compile(r"(^|[.])Sccp[A-Za-z0-9_]+(?:$|[.])")
TRX_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9_.-]*[A-Za-z0-9])?$")
ASCII_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")
ASCII_WHITESPACE_RE = re.compile(r"\s")
TRX_ASSEMBLY_PATH_FORBIDDEN_RE = re.compile(r"[<>\"']")
TRX_ASSEMBLY_PATH_URI_DELIMITER_RE = re.compile(r"[?#&;%]")
TRX_TEST_NAME_FORBIDDEN_RE = re.compile(r"[<>\"'`|\\/:#?&;%]")
TRX_RELATIVE_RESULTS_DIRECTORY_FORBIDDEN_RE = re.compile(r"[<>\"'`|\\/:#?&;%]")
TRX_DURATION_METADATA_RE = re.compile(
    r"^(?:[0-9]+\.)?[0-9]{1,2}:[0-5][0-9]:[0-5][0-9](?:\.[0-9]{1,7})?$"
)
TRX_TIMESTAMP_METADATA_RE = re.compile(
    r"^[1-9][0-9]{3}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12][0-9]|3[01])T"
    r"(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](?:\.[0-9]{1,7})?"
    r"(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])$"
)
TRX_GUID_METADATA_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
)
TRX_ZERO_GUID_METADATA = "00000000-0000-0000-0000-000000000000"
TRX_ATTRIBUTE_PERCENT_DECODE_MAX_ROUNDS = 8
SENSITIVE_TRX_ATTRIBUTE_VALUE_MARKERS = (
    "secret-token",
    "private key",
    "private-key",
    "private_key",
    "seed phrase",
    "seed-phrase",
    "seed_phrase",
    "mnemonic",
    "bearer ",
    "client secret",
    "client-secret",
    "client_secret",
    "password=",
    "api key",
    "api-key",
    "api_key",
    "operator/private",
    "operator\\private",
)
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
dtd_probe = lowered_trx.replace(b"\x00", b"")
if b"<!doctype" in dtd_probe or b"<!entity" in dtd_probe:
    fail("requires TRX result to contain no DTD or entity declarations")
if b"<!--" in dtd_probe:
    fail("requires TRX result to contain no XML comments")
processing_instruction_probe = dtd_probe
stripped_processing_instruction_probe = processing_instruction_probe.lstrip()
if (
    stripped_processing_instruction_probe.startswith(b"<?xml")
    and len(stripped_processing_instruction_probe) > len(b"<?xml")
    and stripped_processing_instruction_probe[len(b"<?xml") : len(b"<?xml") + 1]
    in b" \t\r\n"
):
    declaration_end = stripped_processing_instruction_probe.find(b"?>")
    if declaration_end >= 0:
        processing_instruction_probe = stripped_processing_instruction_probe[
            declaration_end + len(b"?>") :
        ]
if b"<?" in processing_instruction_probe:
    fail("requires TRX result to contain no XML processing instructions")

try:
    root = ET.fromstring(trx_bytes)
except ET.ParseError:
    fail("requires TRX result to be well-formed XML")


def split_tag(tag):
    if tag.startswith("{"):
        namespace, name = tag[1:].split("}", 1)
        return namespace, name
    return "", tag


def local_name(tag):
    return split_tag(tag)[1]


def is_allowed_vstest_namespace(namespace):
    return namespace in ("", VSTEST_XML_NAMESPACE)


def is_vstest_element(element, name):
    namespace, element_name = split_tag(element.tag)
    return element_name == name and is_allowed_vstest_namespace(namespace)


def iter_percent_decoded_trx_attribute_values(value):
    values_to_check = [value]
    decoded_value = value
    for _round in range(TRX_ATTRIBUTE_PERCENT_DECODE_MAX_ROUNDS):
        next_decoded_value = urllib.parse.unquote(decoded_value)
        if next_decoded_value == decoded_value:
            return values_to_check
        values_to_check.append(next_decoded_value)
        decoded_value = next_decoded_value
    if urllib.parse.unquote(decoded_value) != decoded_value:
        fail("requires TRX XML metadata to avoid deeply nested percent encoding")
    return values_to_check


def has_sensitive_trx_attribute_value(value):
    values_to_check = iter_percent_decoded_trx_attribute_values(value)
    for candidate in values_to_check:
        normalized_value = candidate.replace("\\", "/").lower()
        if any(
            marker.replace("\\", "/") in normalized_value
            for marker in SENSITIVE_TRX_ATTRIBUTE_VALUE_MARKERS
        ):
            return True
    return False


def is_printable_ascii_trx_attribute_value(value):
    return all(
        candidate.isascii() and ASCII_CONTROL_RE.search(candidate) is None
        for candidate in iter_percent_decoded_trx_attribute_values(value)
    )


def is_canonical_trx_guid_metadata_value(value):
    return all(
        candidate != TRX_ZERO_GUID_METADATA
        and TRX_GUID_METADATA_RE.fullmatch(candidate) is not None
        for candidate in iter_percent_decoded_trx_attribute_values(value)
    )


def has_empty_dotted_name_component(value):
    return value.startswith(".") or value.endswith(".") or ".." in value


def is_canonical_trx_relative_results_directory(value):
    return all(
        candidate
        and candidate == candidate.strip()
        and candidate.isascii()
        and ASCII_CONTROL_RE.search(candidate) is None
        and TRX_RELATIVE_RESULTS_DIRECTORY_FORBIDDEN_RE.search(candidate) is None
        and "://" not in candidate
        and not has_empty_dotted_name_component(candidate)
        for candidate in iter_percent_decoded_trx_attribute_values(value)
    )


def is_canonical_trx_metadata_attribute(element_name, attribute_name, value):
    if element_name == "TestRun" and attribute_name == "id":
        return is_canonical_trx_guid_metadata_value(value)
    if element_name != "UnitTestResult":
        return True
    if attribute_name in ("testListId", "testType"):
        return is_canonical_trx_guid_metadata_value(value)
    if attribute_name == "relativeResultsDirectory":
        return is_canonical_trx_relative_results_directory(value)
    validator_by_attribute = {
        "duration": TRX_DURATION_METADATA_RE.fullmatch,
        "startTime": TRX_TIMESTAMP_METADATA_RE.fullmatch,
        "endTime": TRX_TIMESTAMP_METADATA_RE.fullmatch,
    }
    validator = validator_by_attribute.get(attribute_name)
    if validator is None:
        return True
    return all(
        validator(candidate) is not None
        for candidate in iter_percent_decoded_trx_attribute_values(value)
    )


def reject_untrusted_trx_attribute_metadata(attribute_name, attribute_value):
    attribute_namespace, local_attribute_name = split_tag(attribute_name)
    if (
        not is_printable_ascii_trx_attribute_value(attribute_namespace)
        or not is_printable_ascii_trx_attribute_value(local_attribute_name)
        or not is_printable_ascii_trx_attribute_value(attribute_value)
    ):
        fail("requires TRX XML attributes to contain only printable ASCII metadata")
    if (
        has_sensitive_trx_attribute_value(attribute_namespace)
        or has_sensitive_trx_attribute_value(local_attribute_name)
        or has_sensitive_trx_attribute_value(attribute_value)
    ):
        fail("requires TRX XML attributes to contain no sensitive metadata")


def reject_untrusted_trx_element_metadata(namespace, name):
    if not is_printable_ascii_trx_attribute_value(
        namespace
    ) or not is_printable_ascii_trx_attribute_value(name):
        fail("requires TRX XML element names to contain only printable ASCII metadata")
    if has_sensitive_trx_attribute_value(namespace) or has_sensitive_trx_attribute_value(
        name
    ):
        fail("requires TRX XML element names to contain no sensitive metadata")


root_namespace, _root_name = split_tag(root.tag)
for element in root.iter():
    namespace, name = split_tag(element.tag)
    if name in TRUSTED_VSTEST_ELEMENT_NAMES and not is_allowed_vstest_namespace(namespace):
        fail(
            "requires TRX VSTest elements to use no namespace or the VSTest 2010 XML namespace"
        )
    if name in TRUSTED_VSTEST_ELEMENT_NAMES and namespace != root_namespace:
        fail("requires TRX VSTest elements to use one consistent namespace")
    if name in TRUSTED_VSTEST_ELEMENT_NAMES:
        for attribute_name in element.attrib:
            attribute_namespace, _attribute_name = split_tag(attribute_name)
            if attribute_namespace:
                fail("requires TRX VSTest attributes to be unqualified")
            if (
                _attribute_name
                not in TRUSTED_VSTEST_ATTRIBUTES_BY_ELEMENT.get(name, frozenset())
            ):
                fail("requires TRX VSTest attributes to use the expected schema")
            if (
                _attribute_name
                in TRUSTED_VSTEST_METADATA_ATTRIBUTES_BY_ELEMENT.get(
                    name, frozenset()
                )
                and not is_printable_ascii_trx_attribute_value(
                    element.attrib[attribute_name]
                )
            ):
                fail(
                    "requires TRX VSTest attributes to contain only printable ASCII metadata"
                )
            if not is_canonical_trx_metadata_attribute(
                name, _attribute_name, element.attrib[attribute_name]
            ):
                fail("requires canonical TRX VSTest metadata attribute values")
            if has_sensitive_trx_attribute_value(element.attrib[attribute_name]):
                fail("requires TRX VSTest attributes to contain no sensitive metadata")
    else:
        reject_untrusted_trx_element_metadata(namespace, name)
        for attribute_name, attribute_value in element.attrib.items():
            reject_untrusted_trx_attribute_metadata(attribute_name, attribute_value)
    for text_value in (element.text, element.tail):
        if text_value is not None and text_value.strip():
            fail("requires TRX XML elements to contain no non-whitespace text")

if not is_vstest_element(root, "TestRun"):
    fail("requires TRX root to be a VSTest TestRun")


def path_basename(value):
    return value.replace("\\", "/").rsplit("/", 1)[-1]


def trx_assembly_reference_problem(value):
    if (
        not value
        or value != value.strip()
        or not is_printable_ascii_trx_attribute_value(value)
        or TRX_ASSEMBLY_PATH_FORBIDDEN_RE.search(value) is not None
        or TRX_ASSEMBLY_PATH_URI_DELIMITER_RE.search(value) is not None
        or has_sensitive_trx_attribute_value(value)
    ):
        return "requires canonical TRX assembly path values"
    normalized = value.replace("\\", "/")
    if "://" in normalized:
        return "requires canonical TRX assembly path values"
    segments = normalized.split("/")
    if any(segment in ("", ".", "..") for segment in segments):
        return "requires canonical TRX assembly path values"
    colon_segments = [
        index for index, segment in enumerate(segments) if ":" in segment
    ]
    if colon_segments and not (
        colon_segments == [0] and re.fullmatch(r"[A-Za-z]:", segments[0])
    ):
        return "requires canonical TRX assembly path values"
    if not segments[-1].endswith(".dll"):
        return "requires canonical TRX assembly path values"
    if any(segment.endswith(".dll") for segment in segments[:-1]):
        return "requires canonical TRX assembly path values"
    if path_basename(value) != ASSEMBLY:
        if ASSEMBLY in value:
            return "requires canonical TRX assembly path values"
        return None
    assembly_segments = [
        index for index, segment in enumerate(segments) if segment == ASSEMBLY
    ]
    if assembly_segments != [len(segments) - 1]:
        return "requires canonical TRX assembly path values"
    return None


def is_trusted_assembly_reference(value):
    return (
        path_basename(value) == ASSEMBLY
        and trx_assembly_reference_problem(value) is None
    )


def is_canonical_trx_test_name(value):
    return bool(
        value
        and value == value.strip()
        and value.isascii()
        and ASCII_CONTROL_RE.search(value) is None
        and ASCII_WHITESPACE_RE.search(value) is None
        and not has_empty_dotted_name_component(value)
        and TRX_TEST_NAME_FORBIDDEN_RE.search(value) is None
    )


def is_canonical_trx_identifier(value):
    return bool(
        value
        and value == value.strip()
        and value.isascii()
        and ASCII_CONTROL_RE.search(value) is None
        and ASCII_WHITESPACE_RE.search(value) is None
        and not has_empty_dotted_name_component(value)
        and TRX_IDENTIFIER_RE.fullmatch(value)
    )


def has_sccp_test_name_token(value):
    return bool(is_canonical_trx_test_name(value) and SCCP_TEST_NAME_RE.search(value))


def trx_unit_test_name_matches_method(value, method_result_name):
    return value == method_result_name or method_result_name.endswith(f".{value}")


assembly_sccp_test_ids = set()
assembly_sccp_execution_ids = set()
execution_id_to_test_id = {}
sccp_test_id_to_names = {}
sccp_execution_id_to_names = {}
seen_unit_test_ids = set()
seen_execution_ids = set()
seen_unit_result_bindings = set()
seen_unit_result_test_ids = set()
seen_unit_result_execution_ids = set()
seen_unit_result_test_names = set()
has_expected_assembly = False

results_sections = [child for child in root if is_vstest_element(child, "Results")]
test_definition_sections = [
    child for child in root if is_vstest_element(child, "TestDefinitions")
]
all_results_sections = [
    element for element in root.iter() if is_vstest_element(element, "Results")
]
all_test_definition_sections = [
    element for element in root.iter() if is_vstest_element(element, "TestDefinitions")
]
if len(results_sections) != len(all_results_sections):
    fail("requires every TRX Results section to appear directly under the VSTest TestRun root")
if len(test_definition_sections) != len(all_test_definition_sections):
    fail(
        "requires every TRX TestDefinitions section to appear directly under the VSTest TestRun root"
    )
all_unit_results = [
    element for element in root.iter() if is_vstest_element(element, "UnitTestResult")
]
if len(results_sections) == 1 and any(
    not is_vstest_element(child, "UnitTestResult") for child in results_sections[0]
):
    fail("requires VSTest Results section to contain only direct UnitTestResult rows")
unit_results = (
    [child for child in results_sections[0] if is_vstest_element(child, "UnitTestResult")]
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
    element for element in root.iter() if is_vstest_element(element, "UnitTest")
]
all_test_methods = [
    element for element in root.iter() if is_vstest_element(element, "TestMethod")
]
all_executions = [
    element for element in root.iter() if is_vstest_element(element, "Execution")
]
if any(list(element) for element in all_unit_results):
    fail("requires every TRX UnitTestResult row to be a leaf element")
if any(list(element) for element in all_test_methods):
    fail("requires every TRX TestMethod definition to be a leaf element")
if any(list(element) for element in all_executions):
    fail("requires every TRX Execution definition to be a leaf element")
unit_tests = (
    [
        child
        for child in test_definition_sections[0]
        if is_vstest_element(child, "UnitTest")
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
unit_test_test_methods = [
    child
    for unit_test in unit_tests
    for child in unit_test
    if is_vstest_element(child, "TestMethod")
]
unit_test_executions = [
    child
    for unit_test in unit_tests
    for child in unit_test
    if is_vstest_element(child, "Execution")
]
if all_test_methods and (
    len(test_definition_sections) != 1
    or len(unit_test_test_methods) != len(all_test_methods)
):
    fail(
        "requires every TRX TestMethod definition to appear directly under a VSTest UnitTest definition"
    )
if all_executions and (
    len(test_definition_sections) != 1
    or len(unit_test_executions) != len(all_executions)
):
    fail(
        "requires every TRX Execution definition to appear directly under a VSTest UnitTest definition"
    )
if len(test_definition_sections) == 1 and any(
    not is_vstest_element(child, "UnitTest") for child in test_definition_sections[0]
):
    fail(
        "requires VSTest TestDefinitions section to contain only direct UnitTest definitions"
    )
if len(results_sections) != 1:
    fail("requires exactly one VSTest Results section")
if len(test_definition_sections) != 1:
    fail("requires exactly one VSTest TestDefinitions section")

for element in unit_tests:
    element_has_expected_assembly = False
    for attribute_name in ("codeBase", "storage"):
        attribute_value = element.attrib.get(attribute_name)
        if attribute_value:
            assembly_problem = trx_assembly_reference_problem(attribute_value)
            if assembly_problem:
                fail(assembly_problem)
        if attribute_value and is_trusted_assembly_reference(attribute_value):
            element_has_expected_assembly = True
            has_expected_assembly = True
    test_id = element.attrib.get("id")
    if test_id is None:
        fail("requires every TRX UnitTest definition to carry id")
    if not is_canonical_trx_identifier(test_id):
        fail("requires canonical TRX UnitTest id values")
    if test_id in seen_unit_test_ids:
        fail("requires unique TRX UnitTest id values")
    seen_unit_test_ids.add(test_id)
    unit_test_name = element.attrib.get("name")
    if unit_test_name is not None and not is_canonical_trx_test_name(unit_test_name):
        fail("requires canonical TRX UnitTest definition name values")
    if any(
        not is_vstest_element(child, "Execution")
        and not is_vstest_element(child, "TestMethod")
        for child in element
    ):
        fail(
            "requires every TRX UnitTest definition to contain only direct Execution and TestMethod children"
        )
    direct_test_methods = [
        child for child in element if is_vstest_element(child, "TestMethod")
    ]
    if len(direct_test_methods) != 1:
        fail(
            "requires every TRX UnitTest definition to contain exactly one direct TestMethod"
        )
    direct_executions = [
        child for child in element if is_vstest_element(child, "Execution")
    ]
    if len(direct_executions) > 1:
        fail(
            "requires every TRX UnitTest definition to contain at most one direct Execution"
        )
    definition_sccp_method_names = set()
    for child in element:
        child_name = local_name(child.tag)
        if child_name == "Execution":
            execution_id = child.attrib.get("id")
            if execution_id is None:
                fail("requires every TRX Execution definition to carry id")
            if not is_canonical_trx_identifier(execution_id):
                fail("requires canonical TRX Execution id values")
            if execution_id in seen_execution_ids:
                fail("requires unique TRX Execution id values")
            seen_execution_ids.add(execution_id)
            execution_id_to_test_id[execution_id] = test_id
        if child_name == "TestMethod":
            method_has_expected_assembly = element_has_expected_assembly
            for attribute_name in ("codeBase", "storage"):
                attribute_value = child.attrib.get(attribute_name)
                if attribute_value:
                    assembly_problem = trx_assembly_reference_problem(attribute_value)
                    if assembly_problem:
                        fail(assembly_problem)
                if attribute_value and is_trusted_assembly_reference(attribute_value):
                    method_has_expected_assembly = True
                    has_expected_assembly = True
            child_name_value = child.attrib.get("name")
            child_class_name = child.attrib.get("className")
            if child_name_value is None:
                fail("requires every TRX TestMethod definition to carry name")
            if child_class_name is None:
                fail("requires every TRX TestMethod definition to carry className")
            if not is_canonical_trx_test_name(child_class_name):
                fail("requires canonical TRX TestMethod className values")
            if not is_canonical_trx_test_name(child_name_value):
                fail("requires canonical TRX TestMethod name values")
            if not child_class_name.startswith(EXPECTED_TEST_NAMESPACE):
                fail(
                    "requires TRX TestMethod className values to use the Hyperledger.Iroha.Sdk.Tests namespace"
                )
            method_result_name = f"{child_class_name}.{child_name_value}"
            if (
                unit_test_name is not None
                and not trx_unit_test_name_matches_method(
                    unit_test_name, method_result_name
                )
            ):
                fail("requires TRX UnitTest definition name to match its TestMethod")
            if method_has_expected_assembly and has_sccp_test_name_token(
                method_result_name
            ):
                definition_sccp_method_names.add(method_result_name)
    if definition_sccp_method_names:
        if test_id:
            assembly_sccp_test_ids.add(test_id)
            sccp_test_id_to_names[test_id] = set(definition_sccp_method_names)
        for child in element:
            if local_name(child.tag) == "Execution":
                execution_id = child.attrib.get("id")
                if execution_id:
                    assembly_sccp_execution_ids.add(execution_id)
                    sccp_execution_id_to_names[execution_id] = set(
                        definition_sccp_method_names
                    )

if not has_expected_assembly:
    fail("requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll")

for result in unit_results:
    if result.attrib.get("outcome") != "Passed":
        fail(
            "requires TRX result to contain no failed, skipped, timed-out, or aborted SCCP test results"
        )
    if result.attrib.get("isExecuted") not in (None, "true"):
        fail("requires every present TRX UnitTestResult isExecuted flag to be true")
    result_test_id = result.attrib.get("testId")
    result_execution_id = result.attrib.get("executionId")
    if result_test_id is not None:
        if not is_canonical_trx_identifier(result_test_id):
            fail("requires canonical TRX UnitTestResult testId values")
        if result_test_id in seen_unit_result_test_ids:
            fail("requires unique TRX UnitTestResult testId values")
        seen_unit_result_test_ids.add(result_test_id)
    if result_execution_id is not None:
        if not is_canonical_trx_identifier(result_execution_id):
            fail("requires canonical TRX UnitTestResult executionId values")
        if result_execution_id in seen_unit_result_execution_ids:
            fail("requires unique TRX UnitTestResult executionId values")
        seen_unit_result_execution_ids.add(result_execution_id)
    if result_execution_id:
        result_binding = ("executionId", result_execution_id)
    elif result_test_id:
        result_binding = ("testId", result_test_id)
    else:
        result_binding = None
    if result_binding is not None:
        if result_binding in seen_unit_result_bindings:
            fail("requires unique TRX UnitTestResult testId/executionId bindings")
        seen_unit_result_bindings.add(result_binding)
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
    if result_test_name is None:
        fail("requires every TRX UnitTestResult to carry testName")
    if not is_canonical_trx_test_name(result_test_name):
        fail("requires canonical TRX UnitTestResult testName values")
    if result_test_name in seen_unit_result_test_names:
        fail("requires unique TRX UnitTestResult testName values")
    seen_unit_result_test_names.add(result_test_name)
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
      reject_dotnet_runtime_path_text ".NET SDK root" "$DOTNET_ROOT" || return 1
      printf '%s\n' "$DOTNET_ROOT/dotnet"
    else
      reject_dotnet_runtime_path_text ".NET SDK executable" "$ROOT/target/dotnet/dotnet" || return 1
      printf '%s\n' "$ROOT/target/dotnet/dotnet"
    fi
    return 0
  fi

  if [[ -n "${DOTNET_ROOT:-}" ]]; then
    reject_dotnet_runtime_path_text ".NET SDK root" "$DOTNET_ROOT" || return 1
  fi
  if [[ -n "${DOTNET_ROOT:-}" && -x "$DOTNET_ROOT/dotnet" ]]; then
    reject_dotnet_runtime_path_text ".NET SDK executable" "$DOTNET_ROOT/dotnet" || return 1
    printf '%s\n' "$DOTNET_ROOT/dotnet"
    return 0
  fi

  local local_dotnet="/tmp/iroha-dotnet/sdk/dotnet"
  if [[ -x "$local_dotnet" ]]; then
    reject_dotnet_runtime_path_text ".NET SDK executable" "$local_dotnet" || return 1
    printf '%s\n' "$local_dotnet"
    return 0
  fi

  if command -v dotnet >/dev/null 2>&1; then
    local path_dotnet
    path_dotnet="$(command -v dotnet)"
    reject_dotnet_runtime_path_text ".NET SDK executable" "$path_dotnet" || return 1
    printf '%s\n' "$path_dotnet"
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
    pytests/scripts/sccp_source_template_hashes_test.py
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
  local dotnet_artifacts_path
  local dotnet_cli
  local dotnet_root
  local dotnet_trx_bytes
  local dotnet_trx_display
  local dotnet_trx_path
  local dotnet_trx_paths
  local dotnet_version
  local dotnet_loader_path
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
  local dotnet_expected_trx_dir
  local dotnet_expected_trx_path
  local dotnet_host_arch
  local dotnet_host_arch_count
  local dotnet_os_arch
  local dotnet_os_arch_count
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
  reject_dotnet_bridge_path_text "native bridge target" "$bridge_target_dir"
  reject_dotnet_bridge_path_text "native bridge output directory" "$bridge_library_dir"
  reject_dotnet_bridge_path_text "native bridge output" "$bridge_library_path"
  reject_dotnet_bridge_path_components "native bridge target" "$bridge_target_dir"
  reject_dotnet_bridge_path_components "native bridge output directory" "$bridge_library_dir"
  reject_dotnet_bridge_path_components "native bridge output" "$bridge_library_path"
  reject_dotnet_path_list_text ".NET phase PATH" "$PATH"
  dotnet_expected_trx_dir="$ROOT/csharp/tests/Hyperledger.Iroha.Sdk.Tests/TestResults"
  dotnet_expected_trx_path="$dotnet_expected_trx_dir/sccp-dotnet-sdk.trx"
  dotnet_artifacts_path="$bridge_target_dir/dotnet-artifacts"
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
    "DOTNET_NOLOGO=1"
    "DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1"
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
    dotnet_os_arch=""
    dotnet_host_arch=""
    if [[ "$dotnet_arch_count" == 1 ]]; then
      dotnet_os_arch="$(dotnet_info_field_value "OS Architecture" <<<"$dotnet_info")"
    else
      if [[ "$dotnet_arch_count" != 0 ]]; then
        echo "SCCP .NET SDK validation requires exactly one OS Architecture from dotnet --info; found: $dotnet_arch_count" >&2
        return 1
      fi
    fi
    dotnet_host_arch_count="$(dotnet_info_section_field_count "Host" "Architecture" <<<"$dotnet_info")"
    if [[ "$dotnet_host_arch_count" == 1 ]]; then
      dotnet_host_arch="$(dotnet_info_section_field_value "Host" "Architecture" <<<"$dotnet_info")"
    elif [[ -z "$dotnet_os_arch" ]]; then
      echo "SCCP .NET SDK validation requires exactly one Host Architecture from dotnet --info when OS Architecture is absent; found: $dotnet_host_arch_count" >&2
      return 1
    elif [[ "$dotnet_host_arch_count" != 0 ]]; then
      echo "SCCP .NET SDK validation requires at most one Host Architecture from dotnet --info; found: $dotnet_host_arch_count" >&2
      return 1
    fi
    if [[ -z "$dotnet_os_arch" && -z "$dotnet_host_arch" ]]; then
      echo "SCCP .NET SDK validation requires exactly one Host Architecture from dotnet --info when OS Architecture is absent; found: $dotnet_host_arch_count" >&2
      return 1
    fi
    if [[ -n "$dotnet_os_arch" && ! "$dotnet_os_arch" =~ ^(x64|x86|arm64|arm)$ ]]; then
      echo "SCCP .NET SDK validation requires a canonical architecture; found: $dotnet_os_arch" >&2
      return 1
    fi
    if [[ -n "$dotnet_host_arch" && ! "$dotnet_host_arch" =~ ^(x64|x86|arm64|arm)$ ]]; then
      echo "SCCP .NET SDK validation requires a canonical architecture; found: $dotnet_host_arch" >&2
      return 1
    fi
    if [[ -n "$dotnet_os_arch" && -n "$dotnet_host_arch" && "$dotnet_os_arch" != "$dotnet_host_arch" ]]; then
      echo "SCCP .NET SDK validation requires OS Architecture and Host Architecture to agree; found OS Architecture: $dotnet_os_arch, Architecture: $dotnet_host_arch" >&2
      return 1
    fi
    dotnet_arch="${dotnet_os_arch:-$dotnet_host_arch}"
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
    validate_nonempty_path_list "PATH" "${PATH:-}"
  fi
  if [[ "$DRY_RUN" -eq 0 ]]; then
    reject_dotnet_bridge_symlink_path "$bridge_target_dir" "$bridge_library_dir" "$bridge_library_path"
  fi
  run_in_dir "$ROOT" \
    env "CARGO_TARGET_DIR=$bridge_target_dir" \
    "CARGO_INCREMENTAL=0" \
    "CARGO_PROFILE_DEV_DEBUG=0" \
    cargo build -p connect_norito_bridge
  if [[ "$DRY_RUN" -eq 0 ]]; then
    reject_dotnet_bridge_symlink_path "$bridge_target_dir" "$bridge_library_dir" "$bridge_library_path"
    if [[ ! -f "$bridge_library_path" ]]; then
      echo "SCCP .NET SDK validation requires freshly built connect_norito_bridge.dll at $bridge_library_path" >&2
      return 1
    fi
    bridge_library_sha256="$(compute_dotnet_bridge_sha256 "$bridge_library_path")" || return 1
    if [[ ! "$bridge_library_sha256" =~ ^[0-9a-f]{64}$ ]]; then
      echo "SCCP .NET SDK validation produced a non-canonical native bridge SHA-256 digest" >&2
      return 1
    fi
    printf 'connect_norito_bridge native bridge: %s\n' "$bridge_library_path"
    printf 'connect_norito_bridge native bridge sha256: %s\n' "$bridge_library_sha256"
  fi
  dotnet_loader_path="$bridge_library_dir"
  if [[ -n "${PATH:-}" ]]; then
    dotnet_loader_path="$dotnet_loader_path:$PATH"
  fi
  run_in_dir "$ROOT/csharp" \
    "${dotnet_env[@]}" \
    "PATH=$dotnet_loader_path" \
    "$dotnet_cli" restore Hyperledger.Iroha.Sdk.sln
  if [[ "$DRY_RUN" -eq 0 ]]; then
    rm -rf "$dotnet_artifacts_path"
  fi
  if [[ "$DRY_RUN" -eq 0 ]]; then
    while IFS= read -r -d '' dotnet_trx_path; do
      rm -f "$dotnet_trx_path"
    done < <(
      find "$ROOT/csharp/tests/Hyperledger.Iroha.Sdk.Tests" \
        -path '*/TestResults/sccp-dotnet-sdk.trx' \
        -type f \
        -print0 2>/dev/null
    )
    reject_dotnet_trx_symlink_path "$dotnet_expected_trx_dir" "$dotnet_expected_trx_path"
  fi
  run_capture_in_dir dotnet_test_output "$ROOT/csharp" \
    "${dotnet_env[@]}" \
    "PATH=$dotnet_loader_path" \
    "$dotnet_cli" test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    --artifacts-path "$dotnet_artifacts_path" \
    --filter "FullyQualifiedName~Sccp" \
    -p:ProduceReferenceAssembly=false \
    --nologo \
    --logger "trx;LogFileName=sccp-dotnet-sdk.trx"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    reject_dotnet_trx_symlink_path "$dotnet_expected_trx_dir" "$dotnet_expected_trx_path"
    if ! dotnet_passed_count="$(dotnet_test_passed_count "$dotnet_test_output")"; then
      return 1
    fi
    dotnet_trx_paths=()
    while IFS= read -r -d '' dotnet_trx_path; do
      dotnet_trx_paths+=("$dotnet_trx_path")
    done < <(
      find "$ROOT/csharp/tests/Hyperledger.Iroha.Sdk.Tests" \
        -path '*/TestResults/sccp-dotnet-sdk.trx' \
        \( -type f -o -type l \) \
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
