#!/usr/bin/env bash
# Run one consumer of the Rust-owned grouped Native AMX V2 golden/negative corpus.

set -euo pipefail

repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
readonly repo_root
readonly fixture_path="${repo_root}/fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly gradle_init_path="${repo_root}/ci/native_amx_v2_grouped_gradle_init.gradle"
readonly expected_negative_control_count=56
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
sdk_command_work_root=""
cleanup() {
  local status=$?
  if [[ -n "$sdk_command_work_root" \
    && -d "$sdk_command_work_root" \
    && "${sdk_command_work_root##*/}" == sdk-command-work.* ]]; then
    "$python_bin" -I -S "$sdk_work_helper" \
      --cleanup-sdk-command-work \
      --sdk-input-root "$sdk_input_root" \
      --sdk-work-root "$sdk_command_work_root" || status=1
  fi
  if [[ -n "$temporary_root" && -d "$temporary_root" \
    && "${temporary_root##*/}" == native-amx-v2-grouped-parity.* ]]; then
    rm -rf -- "$temporary_root"
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup EXIT
openapi_require_signed=0
if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]; then
  openapi_require_signed=1
  readonly sdk_input_root="${IROHA_RELEASE_SDK_INPUT_ROOT:-}"
  readonly sdk_input_inventory="${IROHA_RELEASE_SDK_INPUT_INVENTORY:-}"
  readonly sdk_input_inventory_sha256="${IROHA_RELEASE_SDK_INPUT_INVENTORY_SHA256:-}"
  readonly sdk_work_parent="${IROHA_RELEASE_SDK_WORK_PARENT:-}"
  readonly sdk_work_helper="${IROHA_RELEASE_SDK_WORK_HELPER:-}"
  readonly sdk_work_helper_sha256="${IROHA_RELEASE_SDK_WORK_HELPER_SHA256:-}"
  readonly sdk_runtime_inventory="${IROHA_RELEASE_RUNTIME_INVENTORY:-}"
  readonly sdk_runtime_inventory_sha256="${IROHA_RELEASE_RUNTIME_INVENTORY_SHA256:-}"
  readonly sdk_openapi_node_bin="${IROHA_RELEASE_NODE_BIN:-}"
  if [[ "$sdk_input_root" != "${IROHA_RELEASE_INVOCATION_ROOT:-}/sdk-inputs" \
    || "$sdk_input_inventory" != "${IROHA_RELEASE_INVOCATION_ROOT:-}/sdk-dependency-input.json" \
    || "$sdk_work_parent" != "${IROHA_RELEASE_INVOCATION_ROOT:-}" \
    || ! "$sdk_input_inventory_sha256" =~ ^[0-9a-f]{64}$ \
    || ! "$sdk_work_helper_sha256" =~ ^[0-9a-f]{64}$ \
    || "$sdk_openapi_node_bin" != "${IROHA_RELEASE_INVOCATION_ROOT:-}/runtime/bin/node" \
    || ! -f "$sdk_openapi_node_bin" || -L "$sdk_openapi_node_bin" \
    || ! -x "$sdk_openapi_node_bin" \
    || "$sdk_runtime_inventory" != "${IROHA_RELEASE_INVOCATION_ROOT:-}/runtime-input.json" \
    || ! "$sdk_runtime_inventory_sha256" =~ ^[0-9a-f]{64}$ \
    || ! -f "$sdk_runtime_inventory" || -L "$sdk_runtime_inventory" \
    || "$(sha256_file "$sdk_runtime_inventory")" != "$sdk_runtime_inventory_sha256" \
    || ! -d "$sdk_input_root/node/node_modules" \
    || -L "$sdk_input_root/node/node_modules" \
    || ! -f "$sdk_work_helper" || -L "$sdk_work_helper" \
    || "$(sha256_file "$sdk_work_helper")" != "$sdk_work_helper_sha256" \
    || ! -f "$sdk_input_inventory" || -L "$sdk_input_inventory" \
    || "$(sha256_file "$sdk_input_inventory")" != "$sdk_input_inventory_sha256" ]]; then
    echo "sealed grouped SDK parity lacks its authenticated private dependency closure" >&2
    exit 1
  fi
  for _ in {1..32}; do
    sdk_command_work_root="${sdk_work_parent%/}/sdk-command-work.$(
      "$python_bin" -I -S -c 'import secrets; print(secrets.token_hex(16))'
    )"
    [[ ! -e "$sdk_command_work_root" && ! -L "$sdk_command_work_root" ]] && break
    sdk_command_work_root=""
  done
  [[ -n "$sdk_command_work_root" ]] || {
    echo "could not allocate a fresh SDK command work root" >&2
    exit 1
  }
  "$python_bin" -I -S "$sdk_work_helper" \
    --create-sdk-command-work \
    --sdk-input-root "$sdk_input_root" \
    --sdk-work-root "$sdk_command_work_root"
  readonly sdk_command_work_root
  readonly sdk_swiftpm_work_root="$sdk_command_work_root/swiftpm"
  readonly sdk_gradle_user_home="$sdk_command_work_root/gradle-home"
  readonly sdk_node_modules_root="$sdk_input_root/node/node_modules"
  readonly sdk_openapi_node_modules_root="$sdk_input_root/openapi/node_modules"
  if [[ ! -d "$sdk_openapi_node_modules_root" \
    || -L "$sdk_openapi_node_modules_root" \
    || ! -f "$sdk_input_root/openapi/package-lock.json" \
    || -L "$sdk_input_root/openapi/package-lock.json" ]]; then
    echo "sealed grouped OpenAPI parity lacks its authenticated private Node closure" >&2
    exit 1
  fi
  "$python_bin" -I -S - \
    "$sdk_input_inventory" "$sdk_input_root" \
    "$sdk_runtime_inventory" "$sdk_openapi_node_bin" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

inventory_path = Path(sys.argv[1])
sdk_root = Path(sys.argv[2])
runtime_inventory_path = Path(sys.argv[3])
node_bin = Path(sys.argv[4])
data = inventory_path.read_bytes()
document = json.loads(data)
if data != (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode():
    raise SystemExit("SDK dependency inventory is not canonical JSON")
binding = document.get("bindings", {}).get("openapi_node")
expected = {
    "node_modules_archive_name": "openapi/node_modules",
    "package_lock_archive_name": "openapi/package-lock.json",
}
if (
    not isinstance(binding, dict)
    or any(binding.get(key) != value for key, value in expected.items())
    or not isinstance(binding.get("package_lock_sha256"), str)
    or not isinstance(binding.get("installed_lock_sha256"), str)
):
    raise SystemExit("authenticated SDK inventory lacks the exact OpenAPI Node binding")
if any(re.fullmatch(r"[0-9a-f]{64}", binding[key]) is None for key in (
    "package_lock_sha256", "installed_lock_sha256",
)):
    raise SystemExit("authenticated OpenAPI Node binding has an invalid digest")
records = document.get("records")
if not isinstance(records, list):
    raise SystemExit("authenticated SDK inventory lacks member records")
by_path = {
    record.get("path"): record
    for record in records
    if isinstance(record, dict) and isinstance(record.get("path"), str)
}
root_record = by_path.get("openapi/node_modules")
if not isinstance(root_record, dict) or root_record.get("kind") != "directory":
    raise SystemExit("authenticated SDK inventory omits OpenAPI node_modules")
for relative, key in (
    ("openapi/package-lock.json", "package_lock_sha256"),
    ("openapi/node_modules/.package-lock.json", "installed_lock_sha256"),
):
    path = sdk_root / relative
    metadata = path.lstat()
    record = by_path.get(relative)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
        or not isinstance(record, dict)
        or record.get("kind") != "file"
        or record.get("mode") != format(stat.S_IMODE(metadata.st_mode), "04o")
        or record.get("size") != metadata.st_size
    ):
        raise SystemExit(f"authenticated OpenAPI Node control metadata changed: {relative}")
    if digest != binding[key]:
        raise SystemExit(f"authenticated OpenAPI Node control changed: {relative}")
    if record.get("sha256") != binding[key]:
        raise SystemExit(f"authenticated OpenAPI Node record changed: {relative}")
observed = set()
root = sdk_root / "openapi/node_modules"
for path in [root, *sorted(root.rglob("*"))]:
    metadata = path.lstat()
    relative = "openapi/node_modules" if path == root else (
        "openapi/node_modules/" + path.relative_to(root).as_posix()
    )
    record = by_path.get(relative)
    if (
        not isinstance(record, dict)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or record.get("mode") != format(stat.S_IMODE(metadata.st_mode), "04o")
    ):
        raise SystemExit("OpenAPI Node copied-input inventory differs")
    if stat.S_ISDIR(metadata.st_mode):
        if record.get("kind") != "directory":
            raise SystemExit("OpenAPI Node directory inventory differs")
    elif stat.S_ISREG(metadata.st_mode):
        if metadata.st_nlink != 1 or record.get("kind") != "file":
            raise SystemExit("OpenAPI Node file metadata is unsafe")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if record.get("size") != metadata.st_size or record.get("sha256") != digest:
            raise SystemExit("OpenAPI Node file inventory differs")
    else:
        raise SystemExit("OpenAPI Node copied input contains a special file")
    observed.add(relative)
expected_members = {
    relative for relative in by_path
    if relative == "openapi/node_modules"
    or relative.startswith("openapi/node_modules/")
}
if observed != expected_members:
    raise SystemExit("OpenAPI Node copied-input inventory has omissions or extras")
runtime = json.loads(runtime_inventory_path.read_bytes())
runtime_records = runtime.get("records")
node_record = next(
    (
        record for record in runtime_records
        if isinstance(record, dict) and record.get("path") == "bin/node"
    ),
    None,
) if isinstance(runtime_records, list) else None
metadata = node_bin.lstat()
if (
    node_bin.resolve(strict=True) != node_bin
    or not stat.S_ISREG(metadata.st_mode)
    or stat.S_ISLNK(metadata.st_mode)
    or metadata.st_nlink != 1
    or metadata.st_uid != os.geteuid()
    or metadata.st_mode & 0o111 == 0
    or not isinstance(node_record, dict)
    or node_record.get("kind") != "file"
    or node_record.get("mode") != format(stat.S_IMODE(metadata.st_mode), "04o")
    or node_record.get("size") != metadata.st_size
    or node_record.get("sha256") != hashlib.sha256(node_bin.read_bytes()).hexdigest()
):
    raise SystemExit("protected OpenAPI Node executable disagrees with runtime inventory")
PY
  sdk_gradle_launcher="$(
    "$python_bin" -I -S - "$sdk_input_inventory" "$sdk_gradle_user_home" <<'PY'
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys

inventory_path = Path(sys.argv[1])
gradle_home = Path(sys.argv[2])
data = inventory_path.read_bytes()
document = json.loads(data)
if data != (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode():
    raise SystemExit("SDK dependency inventory is not canonical JSON")
binding = document.get("bindings", {}).get("gradle", {})
key = "79n14ral3mx1ozqr3csh2u872"
expected_archive = (
    "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
    f"{key}/gradle-9.3.0/bin/gradle"
)
if (
    document.get("format") != "iroha-sumeragi-v2-sdk-dependency-bundle"
    or document.get("schema_version") != 1
    or binding.get("distribution_url")
    != "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
    or binding.get("wrapper_cache_key") != key
    or binding.get("launcher_archive_name") != expected_archive
):
    raise SystemExit("authenticated SDK inventory lacks the exact Gradle 9.3.0 launcher")
relative = PurePosixPath(expected_archive).relative_to(
    "gradle/gradle-user-home"
)
launcher = gradle_home.joinpath(*relative.parts)
metadata = launcher.lstat()
if (
    not stat.S_ISREG(metadata.st_mode)
    or stat.S_ISLNK(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or metadata.st_nlink != 1
    or metadata.st_mode & 0o111 == 0
    or launcher.resolve(strict=True) != launcher
):
    raise SystemExit("authenticated private Gradle launcher is unsafe")
print(launcher)
PY
  )"
  readonly sdk_gradle_launcher
else
  readonly sdk_work_helper=""
  readonly sdk_node_modules_root="${repo_root}/javascript/iroha_js/node_modules"
  readonly sdk_openapi_node_modules_root="${repo_root}/tools/openapi/node_modules"
  sdk_openapi_node_bin="$(command -v node || true)"
  if [[ -z "$sdk_openapi_node_bin" ]]; then
    echo "Node.js is required for grouped Native AMX V2 OpenAPI parity" >&2
    exit 1
  fi
  sdk_openapi_node_bin="$($python_bin -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$sdk_openapi_node_bin")"
  readonly sdk_openapi_node_bin
  readonly sdk_swiftpm_work_root="${temporary_root}/swift-build"
  readonly sdk_gradle_user_home="${GRADLE_USER_HOME:-${HOME}/.gradle}"
  readonly sdk_gradle_launcher=""
fi
readonly openapi_require_signed
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

assert_openapi_replay_marker() {
  local candidate_oid
  local candidate_tree
  local require_signed
  local expected_marker
  candidate_oid="$(
    GIT_OPTIONAL_LOCKS=0 git -C "$repo_root" rev-parse --verify "HEAD^{commit}"
  )"
  candidate_tree="$(
    GIT_OPTIONAL_LOCKS=0 git -C "$repo_root" rev-parse --verify "${candidate_oid}^{tree}"
  )"
  require_signed="$openapi_require_signed"
  expected_marker="openapi-two-mirror-replay status=success candidate_oid=${candidate_oid} candidate_tree=${candidate_tree} mirrors=2 artifacts=5 require_signed=${require_signed}"
  if [[ "$(grep -Fxc -- "$expected_marker" "$transcript" || true)" != 1 \
    || "$(grep -Ec -- '^openapi-two-mirror-replay ' "$transcript" || true)" != 1 ]]; then
    echo "grouped Native AMX V2 OpenAPI parity lacks one exact path-free two-mirror replay marker" >&2
    exit 1
  fi
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
    run_openapi_grouped_parity() {
      OPENAPI_NODE_BIN="$sdk_openapi_node_bin" \
        OPENAPI_NODE_MODULES_ROOT="$sdk_openapi_node_modules_root" \
        OPENAPI_REQUIRE_SIGNED="$openapi_require_signed" \
        bash "${repo_root}/ci/check_openapi_spec.sh"
      env PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
        "$python_bin" -m pytest -q -p no:cacheprovider \
        --basetemp="${temporary_root}/pytest" \
        "${repo_root}/pytests/scripts/native_amx_v2_grouped_fixture_test.py"
    }
    run_and_capture run_openapi_grouped_parity
    assert_pytest_count "$observed_test_count"
    assert_openapi_replay_marker
    ;;
  python)
    observed_test_count=63
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
    observed_test_count=61
    if ! command -v node >/dev/null 2>&1; then
      echo "Node.js is required for grouped Native AMX V2 JavaScript parity" >&2
      exit 1
    fi
    readonly javascript_sdk_root="${repo_root}/javascript/iroha_js"
    readonly javascript_source_root="${javascript_sdk_root}/src"
    readonly javascript_repository_root_first="${temporary_root}/javascript-repository-first"
    readonly javascript_repository_root_second="${temporary_root}/javascript-repository-second"
    readonly javascript_package_root_first="${javascript_repository_root_first}/javascript/iroha_js"
    readonly javascript_package_root_second="${javascript_repository_root_second}/javascript/iroha_js"
    readonly javascript_dist_root_first="${javascript_package_root_first}/dist"
    readonly javascript_dist_root_second="${javascript_package_root_second}/dist"
    if [[ -n "$(find "$javascript_source_root" -type l -print -quit)" ]]; then
      echo "grouped Native AMX V2 JavaScript source tree must not contain symlinks" >&2
      exit 1
    fi
    if [[ ! -d "$sdk_node_modules_root" \
      || -L "$sdk_node_modules_root" ]]; then
      echo "grouped Native AMX V2 JavaScript parity requires a regular installed node_modules directory" >&2
      exit 1
    fi
    for javascript_package_root in \
      "$javascript_package_root_first" "$javascript_package_root_second"; do
      javascript_staged_source_root="${javascript_package_root}/src"
      javascript_staged_scripts_root="${javascript_package_root}/scripts"
      javascript_staged_test_root="${javascript_package_root}/test"
      javascript_staged_fixture_root="${javascript_package_root%/javascript/iroha_js}/fixtures/sumeragi_v2"
      mkdir -p -- "$javascript_staged_source_root" \
        "$javascript_staged_scripts_root" "$javascript_staged_test_root" \
        "$javascript_staged_fixture_root"
      chmod 700 "$javascript_package_root"
      cp "${javascript_sdk_root}/package.json" "${javascript_package_root}/package.json"
      cp "${javascript_sdk_root}/index.d.ts" "${javascript_package_root}/index.d.ts"
      cp "${javascript_sdk_root}/scripts/build-dist.mjs" \
        "${javascript_staged_scripts_root}/build-dist.mjs"
      cp "${javascript_sdk_root}/scripts/native-build-provenance.mjs" \
        "${javascript_staged_scripts_root}/native-build-provenance.mjs"
      cp "${javascript_sdk_root}/test/nativeAmxV2GroupedFixture.test.js" \
        "${javascript_staged_test_root}/nativeAmxV2GroupedFixture.test.js"
      cp "$fixture_path" \
        "${javascript_staged_fixture_root}/native_amx_v2_grouped.json"
      ln -s "$sdk_node_modules_root" "${javascript_package_root}/node_modules"
      cp -R "${javascript_source_root}/." "$javascript_staged_source_root/"
      env \
        IROHA_JS_BUILD_DIST_ROOT="$javascript_package_root" \
        node "${javascript_staged_scripts_root}/build-dist.mjs"
      chmod 700 "${javascript_package_root}/dist"
    done
    "$python_bin" "$source_closure_resolver" \
      --root "$repo_root" \
      --manifest "$source_closure_manifest" \
      --suite "native-amx-v2-grouped" \
      --check-regeneration javascript \
      --first-output-root "$javascript_dist_root_first" \
      --second-output-root "$javascript_dist_root_second"
    run_and_capture \
      env \
      IROHA_JS_NATIVE_AMX_V2_PARITY_DIST_TORII_CLIENT="${javascript_dist_root_first}/toriiClient.js" \
      node --test --test-reporter=tap \
      "${javascript_package_root_first}/test/nativeAmxV2GroupedFixture.test.js"
    if [[ "$(grep -Fxc -- "# pass ${observed_test_count}" "$transcript" || true)" != 1 \
      || "$(grep -Fxc -- "# fail 0" "$transcript" || true)" != 1 \
      || "$(grep -Fxc -- "# skipped 0" "$transcript" || true)" != 1 ]]; then
      echo "grouped Native AMX V2 JavaScript parity did not run exactly ${observed_test_count} passing tests" >&2
      exit 1
    fi
    ;;
  swift)
    observed_test_count=5
    if ! command -v swift >/dev/null 2>&1; then
      echo "Swift is required for grouped Native AMX V2 Swift parity" >&2
      exit 1
    fi
    run_and_capture \
      env GIT_ALLOW_PROTOCOL= GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
      GIT_TERMINAL_PROMPT=0 HTTP_PROXY=http://127.0.0.1:9 \
      HTTPS_PROXY=http://127.0.0.1:9 ALL_PROXY=http://127.0.0.1:9 NO_PROXY= \
      swift test \
      --package-path "${repo_root}/IrohaSwift" \
      --scratch-path "$sdk_swiftpm_work_root" \
      --disable-automatic-resolution \
      --only-use-versions-from-resolved-file \
      --skip-update \
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
    observed_test_count=$((6 + 1))
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    mkdir -p -- "$gradle_build_root"
    if [[ -n "$sdk_gradle_launcher" ]]; then
      gradle_command=("$sdk_gradle_launcher")
    else
      gradle_command=(sh "${repo_root}/kotlin/gradlew")
    fi
    run_and_capture \
      env GRADLE_USER_HOME="$sdk_gradle_user_home" JAVA_HOME="$java_home" \
      PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${gradle_command[@]}" \
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
    observed_test_count=6
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    mkdir -p -- "$gradle_build_root"
    if [[ -n "$sdk_gradle_launcher" ]]; then
      gradle_command=("$sdk_gradle_launcher")
    else
      gradle_command=(sh "${repo_root}/java/iroha_android/gradlew")
    fi
    run_and_capture \
      env GRADLE_USER_HOME="$sdk_gradle_user_home" JAVA_HOME="$java_home" \
      PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${gradle_command[@]}" \
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
