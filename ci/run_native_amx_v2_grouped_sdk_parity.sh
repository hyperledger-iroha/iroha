#!/usr/bin/env bash
# Run one consumer of the Rust-owned grouped Native AMX V2 golden/negative corpus.

set -euo pipefail

repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
readonly repo_root
readonly fixture_path="${repo_root}/fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly gradle_init_path="${repo_root}/ci/native_amx_v2_grouped_gradle_init.gradle"
readonly expected_negative_control_count=55
readonly source_closure_resolver="${repo_root}/ci/resolve_sumeragi_v2_sdk_source_closure.py"
readonly source_closure_manifest="${repo_root}/ci/sumeragi_v2_sdk_source_closure.json"

usage() {
  echo "usage: $0 {openapi|python|javascript|swift|kotlin|java|--fixture-sha256|--suite-source-manifest-sha256}" >&2
  exit 2
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

require_regular_source() {
  local path="$1"
  if [[ ! -f "$path" || -L "$path" ]]; then
    echo "Native AMX V2 grouped parity source must be a regular non-symlink file: ${path}" >&2
    exit 1
  fi
}

require_regular_source "$fixture_path"
require_regular_source "$source_closure_resolver"
require_regular_source "$source_closure_manifest"

readonly python_bin="${PYTHON_BIN:-python3}"
if [[ -z "$python_bin" ]] || ! command -v "$python_bin" >/dev/null 2>&1; then
  echo "A valid Python 3 interpreter is required for grouped Native AMX V2 parity (PYTHON_BIN=${python_bin})" >&2
  exit 1
fi

fixture_sha256="$(sha256_file "$fixture_path")"
if [[ ! "$fixture_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "grouped Native AMX V2 fixture digest is not lowercase SHA-256" >&2
  exit 1
fi
readonly fixture_sha256

suite_source_manifest_sha256="$(
  "$python_bin" "$source_closure_resolver" \
    --root "$repo_root" \
    --manifest "$source_closure_manifest" \
    --suite "native-amx-v2-grouped" \
    --manifest-sha256
)"
if [[ ! "$suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "grouped Native AMX V2 suite-source manifest digest is invalid" >&2
  exit 1
fi
readonly suite_source_manifest_sha256

if [[ $# != 1 ]]; then
  usage
fi
readonly surface="$1"
case "$surface" in
  --fixture-sha256)
    printf '%s\n' "$fixture_sha256"
    exit 0
    ;;
  --suite-source-manifest-sha256)
    printf '%s\n' "$suite_source_manifest_sha256"
    exit 0
    ;;
  openapi|python|javascript|swift|kotlin|java)
    ;;
  *)
    usage
    ;;
esac

"$python_bin" - "$fixture_path" "$expected_negative_control_count" <<'PY'
import json
from pathlib import Path
import sys

path = Path(sys.argv[1])
expected = int(sys.argv[2])
document = json.loads(path.read_text(encoding="utf-8"))
if document.get("format") != "iroha-native-amx-v2-grouped":
    raise SystemExit("unexpected grouped Native AMX V2 fixture format")
if document.get("fixture_version") != 1:
    raise SystemExit("unexpected grouped Native AMX V2 fixture version")
if document.get("rust_owner") != "iroha_data_model::block::consensus":
    raise SystemExit("grouped Native AMX V2 fixture is not Rust-owned")
controls = document.get("negative_controls")
if not isinstance(controls, list) or len(controls) != expected:
    raise SystemExit(
        f"expected exactly {expected} grouped Native AMX V2 negative controls"
    )
if len({control.get("id") for control in controls}) != expected:
    raise SystemExit("grouped Native AMX V2 negative-control IDs are not unique")
if any(control.get("expectation") != "reject" for control in controls):
    raise SystemExit("grouped Native AMX V2 negative controls must fail closed")
PY

readonly temporary_parent="${TMPDIR:-/tmp}"
temporary_root="$(mktemp -d "${temporary_parent%/}/native-amx-v2-grouped-parity.XXXXXX")"
readonly temporary_root
cleanup() {
  local status=$?
  if [[ -n "$temporary_root" && -d "$temporary_root" \
    && "${temporary_root##*/}" == native-amx-v2-grouped-parity.* ]]; then
    rm -rf -- "$temporary_root"
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup EXIT
readonly transcript="${temporary_root}/${surface}.log"

run_and_capture() {
  set +e
  "$@" 2>&1 | tee "$transcript"
  local pipeline_status=("${PIPESTATUS[@]}")
  set -e
  if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
    echo "grouped Native AMX V2 ${surface} parity command failed (command=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
    exit 1
  fi
}

resolve_java_home() {
  local java_bin
  if [[ -n "${JAVA_BIN:-}" ]]; then
    java_bin="$JAVA_BIN"
  else
    java_bin="$("${repo_root}/scripts/formal/resolve_java.sh")"
  fi
  java_bin="$(
    "$python_bin" - "$java_bin" <<'PY'
from pathlib import Path
import sys
print(Path(sys.argv[1]).resolve(strict=True))
PY
  )"
  if [[ "${java_bin##*/}" != java || ! -x "$java_bin" ]]; then
    echo "grouped Native AMX V2 JVM parity requires an executable JDK java binary" >&2
    exit 1
  fi
  local java_home="${java_bin%/bin/java}"
  if [[ ! -x "${java_home}/bin/javac" ]]; then
    echo "grouped Native AMX V2 JVM parity requires a complete JDK" >&2
    exit 1
  fi
  printf '%s\n' "$java_home"
}

assert_pytest_count() {
  local expected="$1"
  "$python_bin" - "$transcript" "$expected" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
expected = int(sys.argv[2])
matches = [
    match
    for line in lines
    if (
        match := re.fullmatch(
            r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"
            r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?",
            line,
        )
    )
]
if len(matches) != 1 or int(matches[0].group(1)) != expected:
    raise SystemExit(f"expected one exact {expected}-test pytest transcript")
PY
}

assert_gradle_report() {
  local report_root="$1"
  local class_name="$2"
  local expected="$3"
  "$python_bin" - "$report_root" "$class_name" "$expected" <<'PY'
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

root = Path(sys.argv[1])
class_name = sys.argv[2]
expected = int(sys.argv[3])
matches = list(root.rglob(f"TEST-{class_name}.xml"))
if len(matches) != 1:
    raise SystemExit(
        f"expected one Gradle report for {class_name}, found {len(matches)}"
    )
suite = ET.parse(matches[0]).getroot()
observed = int(suite.attrib.get("tests", "-1"))
failures = int(suite.attrib.get("failures", "-1"))
errors = int(suite.attrib.get("errors", "-1"))
skipped = int(suite.attrib.get("skipped", "-1"))
if (observed, failures, errors, skipped) != (expected, 0, 0, 0):
    raise SystemExit(
        "unexpected Gradle grouped Native AMX V2 result: "
        f"tests={observed} failures={failures} errors={errors} skipped={skipped}"
    )
PY
}

observed_test_count=0
case "$surface" in
  openapi)
    observed_test_count=7
    run_and_capture \
      env PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
      "$python_bin" -m pytest -q -p no:cacheprovider \
      --basetemp="${temporary_root}/pytest" \
      "${repo_root}/pytests/scripts/native_amx_v2_grouped_fixture_test.py"
    assert_pytest_count "$observed_test_count"
    ;;
  python)
    observed_test_count=62
    "$python_bin" -c \
      'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else "Python Native AMX V2 parity requires Python >=3.10")'
    if [[ "${IROHA_PYTHON_TEST_INSTALLED_PACKAGE:-}" == "1" ]]; then
      readonly python_parity_path="${repo_root}/python/norito_py/src:${repo_root}/python"
    else
      readonly python_parity_path="${repo_root}/python/iroha_python/src:${repo_root}/python/norito_py/src:${repo_root}/python/iroha_torii_client:${repo_root}/python${PYTHONPATH:+:${PYTHONPATH}}"
    fi
    run_and_capture \
      env PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
      PYTHONPATH="$python_parity_path" \
      "$python_bin" -m pytest -q -p no:cacheprovider \
      --basetemp="${temporary_root}/pytest" \
      "${repo_root}/python/iroha_python/tests/native_amx_v2_grouped_fixture_test.py"
    assert_pytest_count "$observed_test_count"
    ;;
  javascript)
    observed_test_count=60
    if ! command -v node >/dev/null 2>&1; then
      echo "Node.js is required for grouped Native AMX V2 JavaScript parity" >&2
      exit 1
    fi
    readonly javascript_sdk_root="${repo_root}/javascript/iroha_js"
    readonly javascript_source_root="${javascript_sdk_root}/src"
    readonly javascript_package_root="${temporary_root}/javascript-package"
    readonly javascript_staged_source_root="${javascript_package_root}/src"
    readonly javascript_staged_scripts_root="${javascript_package_root}/scripts"
    readonly javascript_dist_root="${javascript_package_root}/dist"
    readonly javascript_dist_diff="${temporary_root}/javascript-dist.diff"
    if [[ -n "$(find "$javascript_source_root" -type l -print -quit)" ]]; then
      echo "grouped Native AMX V2 JavaScript source tree must not contain symlinks" >&2
      exit 1
    fi
    if [[ ! -d "${javascript_sdk_root}/node_modules" \
      || -L "${javascript_sdk_root}/node_modules" ]]; then
      echo "grouped Native AMX V2 JavaScript parity requires a regular installed node_modules directory" >&2
      exit 1
    fi
    mkdir -p -- "$javascript_staged_source_root" "$javascript_staged_scripts_root"
    cp "${javascript_sdk_root}/package.json" "${javascript_package_root}/package.json"
    cp "${javascript_sdk_root}/scripts/native-build-provenance.mjs" \
      "${javascript_staged_scripts_root}/native-build-provenance.mjs"
    ln -s "${javascript_sdk_root}/node_modules" "${javascript_package_root}/node_modules"
    cp -R "${javascript_source_root}/." "$javascript_staged_source_root/"
    env \
      IROHA_JS_BUILD_DIST_ROOT="$javascript_package_root" \
      node "${javascript_sdk_root}/scripts/build-dist.mjs"
    if [[ -n "$(find "$javascript_dist_root" -type l -print -quit)" ]]; then
      echo "grouped Native AMX V2 staged JavaScript distribution must not contain symlinks" >&2
      exit 1
    fi
    if ! diff -qr "$javascript_source_root" "$javascript_dist_root" >"$javascript_dist_diff"; then
      echo "grouped Native AMX V2 staged JavaScript distribution differs from source" >&2
      sed -n '1,40p' "$javascript_dist_diff" >&2
      exit 1
    fi
    run_and_capture \
      env \
      IROHA_JS_NATIVE_AMX_V2_PARITY_DIST_TORII_CLIENT="${javascript_dist_root}/toriiClient.js" \
      node --test --test-reporter=tap \
      "${repo_root}/javascript/iroha_js/test/nativeAmxV2GroupedFixture.test.js"
    if [[ "$(grep -Fxc -- "# pass ${observed_test_count}" "$transcript" || true)" != 1 \
      || "$(grep -Fxc -- "# fail 0" "$transcript" || true)" != 1 \
      || "$(grep -Fxc -- "# skipped 0" "$transcript" || true)" != 1 ]]; then
      echo "grouped Native AMX V2 JavaScript parity did not run exactly ${observed_test_count} passing tests" >&2
      exit 1
    fi
    ;;
  swift)
    observed_test_count=4
    if ! command -v swift >/dev/null 2>&1; then
      echo "Swift is required for grouped Native AMX V2 Swift parity" >&2
      exit 1
    fi
    run_and_capture \
      swift test \
      --package-path "${repo_root}/IrohaSwift" \
      --scratch-path "${temporary_root}/swift-build" \
      --disable-automatic-resolution \
      --only-use-versions-from-resolved-file \
      --disable-swift-testing \
      --filter NativeAmxV2GroupedFixtureTests
    "$python_bin" - "$transcript" "$observed_test_count" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
expected = int(sys.argv[2])
matches = [
    match
    for line in lines
    if (
        match := re.search(
            r"Executed ([0-9]+) tests?, with 0 failures(?: \(0 unexpected\))?",
            line,
        )
    )
]
if not matches or any(int(match.group(1)) != expected for match in matches):
    raise SystemExit(f"expected exact {expected}-test Swift XCTest transcript")
PY
    ;;
  kotlin)
    observed_test_count=6
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    mkdir -p -- "$gradle_build_root"
    run_and_capture \
      env JAVA_HOME="$java_home" \
      PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${repo_root}/kotlin/gradlew" \
      --offline --no-daemon --no-build-cache --rerun-tasks --console=plain \
      --project-dir "${repo_root}/kotlin" \
      --project-cache-dir "${temporary_root}/kotlin-project-cache" \
      --init-script "$gradle_init_path" \
      :core-jvm:test \
      --tests org.hyperledger.iroha.sdk.consensus.NativeAmxV2GroupedFixtureTest
    assert_gradle_report \
      "$gradle_build_root" \
      org.hyperledger.iroha.sdk.consensus.NativeAmxV2GroupedFixtureTest \
      "$observed_test_count"
    ;;
  java)
    observed_test_count=5
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    mkdir -p -- "$gradle_build_root"
    run_and_capture \
      env JAVA_HOME="$java_home" \
      PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${repo_root}/java/iroha_android/gradlew" \
      --offline --no-daemon --no-build-cache --rerun-tasks --console=plain \
      --project-dir "${repo_root}/java/iroha_android" \
      --project-cache-dir "${temporary_root}/java-project-cache" \
      --init-script "$gradle_init_path" \
      :core:test \
      --tests org.hyperledger.iroha.android.consensus.NativeAmxV2GroupedFixtureTests
    assert_gradle_report \
      "$gradle_build_root" \
      org.hyperledger.iroha.android.consensus.NativeAmxV2GroupedFixtureTests \
      "$observed_test_count"
    ;;
esac

printf 'native-amx-v2-grouped-parity surface=%s tests=%s fixture_sha256=%s suite_source_manifest_sha256=%s\n' \
  "$surface" \
  "$observed_test_count" \
  "$fixture_sha256" \
  "$suite_source_manifest_sha256"
