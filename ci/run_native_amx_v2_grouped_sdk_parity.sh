#!/usr/bin/env bash
# Run one consumer of the Rust-owned grouped Native AMX V2 golden/negative corpus.

set -euo pipefail

repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
readonly repo_root
readonly fixture_path="${repo_root}/fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly gradle_init_path="${repo_root}/ci/native_amx_v2_grouped_gradle_init.gradle"
readonly expected_negative_control_count=45
readonly source_paths=(
  ci/run_native_amx_v2_grouped_sdk_parity.sh
  ci/native_amx_v2_grouped_gradle_init.gradle
  crates/iroha_data_model/src/bin/native_amx_grouped.rs
  pytests/scripts/native_amx_v2_grouped_fixture_test.py
  python/iroha_python/tests/native_amx_v2_grouped_fixture_test.py
  python/iroha_python/src/iroha_python/client.py
  python/iroha_python/src/iroha_python/__init__.py
  python/iroha_torii_client/client.py
  javascript/iroha_js/test/nativeAmxV2GroupedFixture.test.js
  javascript/iroha_js/src/toriiClient.js
  javascript/iroha_js/dist/toriiClient.js
  javascript/iroha_js/index.d.ts
  IrohaSwift/Tests/IrohaSwiftTests/NativeAmxV2GroupedFixtureTests.swift
  IrohaSwift/Sources/IrohaSwift/ToriiClient.swift
  IrohaSwift/Package.swift
  IrohaSwift/Package.resolved
  kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/consensus/NativeAmxV2GroupedFixtureTest.kt
  kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/NativeAmxV2.kt
  kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/SumeragiDiagnosticsModels.kt
  kotlin/core-jvm/build.gradle.kts
  kotlin/settings.gradle.kts
  kotlin/gradlew
  kotlin/gradle/wrapper/gradle-wrapper.jar
  kotlin/gradle/wrapper/gradle-wrapper.properties
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/consensus/NativeAmxV2GroupedFixtureTests.java
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/NativeAmxV2Models.java
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/SumeragiDiagnosticsModels.java
  java/iroha_android/core/build.gradle.kts
  java/iroha_android/settings.gradle.kts
  java/iroha_android/gradlew
  java/iroha_android/gradle/wrapper/gradle-wrapper.jar
  java/iroha_android/gradle/wrapper/gradle-wrapper.properties
  docs/portal/static/openapi/torii.json
  docs/portal/static/openapi/versions/current/torii.json
)

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
for relative_path in "${source_paths[@]}"; do
  require_regular_source "${repo_root}/${relative_path}"
done

fixture_sha256="$(sha256_file "$fixture_path")"
if [[ ! "$fixture_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "grouped Native AMX V2 fixture digest is not lowercase SHA-256" >&2
  exit 1
fi
readonly fixture_sha256

suite_source_manifest_sha256="$(
  {
    for relative_path in "${source_paths[@]}"; do
      printf '%s\t%s\n' \
        "$relative_path" \
        "$(sha256_file "${repo_root}/${relative_path}")"
    done
  } | if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
  else
    shasum -a 256 | awk '{print $1}'
  fi
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

if ! command -v python3 >/dev/null 2>&1; then
  echo "Python 3 is required for grouped Native AMX V2 parity" >&2
  exit 1
fi
python3 - "$fixture_path" "$expected_negative_control_count" <<'PY'
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
    python3 - "$java_bin" <<'PY'
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
  python3 - "$transcript" "$expected" <<'PY'
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
  python3 - "$report_root" "$class_name" "$expected" <<'PY'
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
    observed_test_count=4
    run_and_capture \
      env PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
      python3 -m pytest -q -p no:cacheprovider \
      "${repo_root}/pytests/scripts/native_amx_v2_grouped_fixture_test.py"
    assert_pytest_count "$observed_test_count"
    ;;
  python)
    observed_test_count=47
    python3 -c \
      'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else "Python Native AMX V2 parity requires Python >=3.10")'
    run_and_capture \
      env PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
      PYTHONPATH="${repo_root}/python/iroha_python/src:${repo_root}/python/norito_py/src:${repo_root}/python/iroha_torii_client:${repo_root}/python" \
      python3 -m pytest -q -p no:cacheprovider \
      "${repo_root}/python/iroha_python/tests/native_amx_v2_grouped_fixture_test.py"
    assert_pytest_count "$observed_test_count"
    ;;
  javascript)
    observed_test_count=48
    if ! command -v node >/dev/null 2>&1; then
      echo "Node.js is required for grouped Native AMX V2 JavaScript parity" >&2
      exit 1
    fi
    run_and_capture \
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
    observed_test_count=3
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
    python3 - "$transcript" "$observed_test_count" <<'PY'
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
