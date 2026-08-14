#!/usr/bin/env bash
# Authenticated function closure for the Sumeragi V2 release-gates runner.
# RELEASE_GATES_SUPPORT_V1

sha256_file() {
  python3 -I -S -c \
    'from pathlib import Path; import hashlib, sys; print(hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest())' \
    "$1"
}
canonical_executable() {
  local executable
  executable="$(command -v "$1")" || return 1
  python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$executable"
}

canonical_git_executable() {
  local executable
  executable="$(canonical_executable git)" || return 1
  # `/usr/bin/git` on macOS is an xcode-select launcher. Archiving that shim
  # would leave the selected developer-tool Git binary outside the protected
  # digest and evidence directory. Resolve the implementation that the shim
  # would execute; the caller-supplied SHA-256 policy below then authenticates
  # those exact bytes before they are used.
  if [[ "$(uname -s)" == Darwin && "$executable" == /usr/bin/git ]]; then
    executable="$(/usr/bin/xcrun --find git)" || return 1
    executable="$(canonical_path "$executable")" || return 1
  fi
  printf '%s\n' "$executable"
}

canonical_path() {
  python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

authenticated_runner_tool_sources() {
  local python_bin="$1"
  "$python_bin" -I -S - "$2" "$3" "$4" "$5" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import re
import stat
import sys

marker_path, expected_marker_digest, exec_path, manifest_path = (
    Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4])
)
marker_bytes = marker_path.read_bytes()
if hashlib.sha256(marker_bytes).hexdigest() != expected_marker_digest:
    raise SystemExit("bootstrap marker changed before Git helper validation")
marker = json.loads(marker_bytes)
tools = marker.get("runner", {}).get("tools", {})
trusted_manifest = marker.get("trusted_inputs", {}).get("runner_tool_manifest", {})
manifest_bytes = manifest_path.read_bytes()
manifest_metadata = manifest_path.lstat()
if (
    not isinstance(trusted_manifest, dict)
    or trusted_manifest.get("archive_name") != "runner-tool-manifest.json"
    or trusted_manifest.get("mode") != "0400"
    or trusted_manifest.get("size_bytes") != len(manifest_bytes)
    or trusted_manifest.get("sha256") != hashlib.sha256(manifest_bytes).hexdigest()
    or stat.S_ISLNK(manifest_metadata.st_mode)
    or not stat.S_ISREG(manifest_metadata.st_mode)
    or stat.S_IMODE(manifest_metadata.st_mode) != 0o400
):
    raise SystemExit("bootstrap runner-tool source manifest changed")
source_tools = json.loads(manifest_bytes).get("tools", {})
expected_tools = {
    "awk", "basename", "cargo", "cargo-verus", "cat", "chmod", "cmp", "cp",
    "cut", "diff", "dirname", "env", "find", "git-index-pack",
    "git-upload-pack", "grep", "java", "ln", "ls", "mkdir", "mkfifo",
    "mktemp", "mv", "node", "openssl", "rm", "rmdir", "rustc", "sed", "sh",
    "sleep", "swift", "tail", "tee", "tlapm", "tr", "uname", "verus", "wc",
    "xargs", "shasum" if sys.platform == "darwin" else "sha256sum",
}
if not isinstance(tools, dict) or set(tools) != expected_tools or set(source_tools) != expected_tools:
    raise SystemExit("production release runner-tool command closure is not exact")
for name in sorted(expected_tools):
    record = tools.get(name)
    source_record = source_tools.get(name)
    alias = exec_path / name
    if not isinstance(record, dict) or set(record) != {
        "archive_id", "alias_name", "archive_name", "mode", "sha256", "size_bytes"
    }:
        raise SystemExit(f"production release lacks authenticated runner tool {name}")
    archive_name = f"runner-tools/{name}"
    archive = exec_path.parent / archive_name
    relative_target = os.path.relpath(archive, alias.parent)
    alias_metadata = alias.lstat()
    archive_metadata = archive.lstat()
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(archive, flags)
    try:
        opened = os.fstat(descriptor)
        digest = hashlib.sha256()
        while block := os.read(descriptor, 1024 * 1024):
            digest.update(block)
        source_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    path_after = archive.lstat()
    alias_after = alias.lstat()
    fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    if (
        not isinstance(source_record, dict)
        or set(source_record) != {"path", "sha256"}
        or not isinstance(source_record.get("path"), str)
        or not isinstance(source_record.get("sha256"), str)
        or record["alias_name"] != name
        or record["archive_id"] != f"release-runner-tool.{name}.v1"
        or record["archive_name"] != archive_name
        or not stat.S_ISLNK(alias_metadata.st_mode)
        or os.readlink(alias) != relative_target
        or archive.resolve(strict=True) != archive
        or stat.S_ISLNK(archive_metadata.st_mode)
        or not stat.S_ISREG(archive_metadata.st_mode)
        or any(getattr(archive_metadata, field) != getattr(opened, field) or getattr(opened, field) != getattr(source_after, field) or getattr(source_after, field) != getattr(path_after, field) for field in fields)
        or (alias_metadata.st_dev, alias_metadata.st_ino, alias_metadata.st_mode) != (alias_after.st_dev, alias_after.st_ino, alias_after.st_mode)
        or os.readlink(alias) != relative_target
        or archive_metadata.st_uid != os.geteuid()
        or archive_metadata.st_nlink != 1
        or stat.S_IMODE(archive_metadata.st_mode) != 0o500
        or archive_metadata.st_size != record["size_bytes"]
        or record["mode"] != "0500"
        or digest.hexdigest() != record["sha256"]
    ):
        raise SystemExit(f"authenticated runner tool {name} changed")
    source = Path(source_record["path"])
    source_text = str(source)
    source_metadata = source.lstat()
    if (
        re.fullmatch(r"/[A-Za-z0-9_./+:@-]+", source_text) is None
        or source.resolve(strict=True) != source
        or stat.S_ISLNK(source_metadata.st_mode)
        or not stat.S_ISREG(source_metadata.st_mode)
        or not source_metadata.st_mode & 0o111
        or hashlib.sha256(source.read_bytes()).hexdigest() != source_record["sha256"]
        or source_record["sha256"] != record["sha256"]
    ):
        raise SystemExit(f"authenticated runner tool source {name} changed")
    print(f"{name}\t{source_text}")
PY
}

resolve_pinned_rust_tool_executable() {
  local rustup_bin="$1"
  local tool="$2"
  local resolved_tool

  resolved_tool="$(
    RUSTUP_AUTO_INSTALL=0 "$rustup_bin" which --toolchain 1.93.1 "$tool"
  )" || return 1
  canonical_path "$resolved_tool"
}
release_identity_json() {
  python3 -I -S scripts/compute_workspace_source_manifest.py \
    --root "$repo_root" \
    --release-identity-json
}
identity_field() {
  local identity_path="$1"
  local field="$2"
  python3 -I -S -c \
    'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8"))[sys.argv[2]])' \
    "$identity_path" "$field"
}
localnet_binary_attestation_valid() {
  sumeragi_v2_localnet_binary_attestation_valid \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

ensure_source_bound_localnet_binaries() {
  sumeragi_v2_ensure_source_bound_localnet_binaries \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

export_source_bound_localnet_binaries() {
  sumeragi_v2_export_source_bound_localnet_binaries \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

verify_release_identity() {
  local checkpoint="$1"
  [[ "$profile" == "--release" ]] || return 0
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" != 1 \
    || "${IROHA_RELEASE_SEALED_ROOT:-}" != "$repo_root" \
    || -z "${IROHA_RELEASE_HOST_ROOT:-}" \
    || -z "${IROHA_RELEASE_WORKSPACE_TARGET:-}" \
    || -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
    || -z "${IROHA_RELEASE_INVOCATION_ROOT:-}" \
    || -z "${CARGO_TARGET_DIR:-}" \
    || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" \
    || -z "${IROHA_RELEASE_CANDIDATE_SOURCE_MANIFEST_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_CONFIGURATION_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST:-}" \
    || -z "${IROHA_RELEASE_SCALING_IROHAD_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_IROHA_CLI_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256:-}" \
    || -z "${IROHA_RELEASE_PYTHON_BIN:-}" \
    || -z "${IROHA_RELEASE_SDK_INPUT_ROOT:-}" \
    || -z "${IROHA_RELEASE_SDK_INPUT_INVENTORY:-}" \
    || -z "${IROHA_RELEASE_SDK_INPUT_INVENTORY_SHA256:-}" \
    || -z "${IROHA_RELEASE_SDK_WORK_PARENT:-}" \
    || -z "${IROHA_RELEASE_SDK_WORK_HELPER:-}" \
    || -z "${IROHA_RELEASE_SDK_WORK_HELPER_SHA256:-}" \
    || "$IROHA_RELEASE_SEALED_ROOT" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/source" \
    || "$IROHA_RELEASE_WORKSPACE_TARGET" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/target" \
    || "$IROHA_RELEASE_ARTIFACT_ROOT" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/output" \
    || "$IROHA_RELEASE_HOST_ROOT" != "$IROHA_RELEASE_ARTIFACT_ROOT" \
    || "$IROHA_RELEASE_SDK_INPUT_ROOT" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/sdk-inputs" \
    || "$IROHA_RELEASE_SDK_INPUT_INVENTORY" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/sdk-dependency-input.json" \
    || "$IROHA_RELEASE_SDK_WORK_PARENT" != "$IROHA_RELEASE_INVOCATION_ROOT" \
    || "$IROHA_RELEASE_SDK_WORK_HELPER" \
      != "$IROHA_RELEASE_INVOCATION_ROOT/runtime/copy-release-runtime.py" \
    || "${PWD:-}" != "$repo_root" \
    || ( -n "${OLDPWD:-}" && "$OLDPWD" != "$repo_root" ) \
    || "${HOME:-}" != "$IROHA_RELEASE_HOST_ROOT/home" \
    || "${TMPDIR:-}" != "$IROHA_RELEASE_HOST_ROOT/tmp" \
    || "${TMP:-}" != "$IROHA_RELEASE_HOST_ROOT/tmp" \
    || "${TEMP:-}" != "$IROHA_RELEASE_HOST_ROOT/tmp" \
    || "${XDG_CACHE_HOME:-}" != "$IROHA_RELEASE_HOST_ROOT/cache" \
    || ! -f "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" \
    || ! -f "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
    || -L "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
    || ! -f "$IROHA_RELEASE_SDK_INPUT_INVENTORY" \
    || -L "$IROHA_RELEASE_SDK_INPUT_INVENTORY" \
    || ! -d "$IROHA_RELEASE_SDK_INPUT_ROOT" \
    || -L "$IROHA_RELEASE_SDK_INPUT_ROOT" \
    || ! -f "$IROHA_RELEASE_SDK_WORK_HELPER" \
    || -L "$IROHA_RELEASE_SDK_WORK_HELPER" \
    || "$(sha256_file "$IROHA_RELEASE_SDK_WORK_HELPER")" \
      != "$IROHA_RELEASE_SDK_WORK_HELPER_SHA256" \
    || "$(sha256_file "$IROHA_RELEASE_SDK_INPUT_INVENTORY")" \
      != "$IROHA_RELEASE_SDK_INPUT_INVENTORY_SHA256" ]]; then
    echo "production release corridor must run from its signed, sealed independent mirror" >&2
    return 1
  fi
  local withheld_name
  for withheld_name in \
    SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION \
    SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION \
    SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT \
    SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY \
    SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR \
    SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256 \
    IROHA_RELEASE_BOOTSTRAP_COMPLETION \
    IROHA_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION \
    IROHA_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT \
    IROHA_RELEASE_BOOTSTRAP_IDENTITY \
    IROHA_RELEASE_BOOTSTRAP_EVIDENCE_DIR \
    IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256 \
    IROHA_RELEASE_CANDIDATE_IDENTITY_PATH \
    IROHA_RELEASE_AGGREGATE_RECEIPT_PATH \
    IROHA_RELEASE_BOOTSTRAP_SOURCE_BINDING \
    IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT \
    IROHA_RELEASE_BOOTSTRAP_RUNNER \
    IROHA_RELEASE_SIGNATURE_ATTESTATION \
    IROHA_RELEASE_SIGNATURE_TRANSCRIPT \
    IROHA_RELEASE_SIGNATURE_RAW_COMMIT \
    IROHA_RELEASE_SIGNATURE_CARGO_LOCK \
    IROHA_RELEASE_SIGNATURE_ALLOWED_SIGNERS \
    IROHA_RELEASE_SIGNATURE_REVOCATION \
    IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST \
    IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256; do
    if [[ -n "${!withheld_name:-}" ]]; then
      echo "protected outer-only input ${withheld_name} reached the build child" >&2
      return 1
    fi
  done
  local scaling_digest_name
  for scaling_digest_name in \
    IROHA_RELEASE_SCALING_CONFIGURATION_SHA256 \
    IROHA_RELEASE_SCALING_IROHAD_SHA256 \
    IROHA_RELEASE_SCALING_IROHA_CLI_SHA256 \
    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256; do
    if [[ ! "${!scaling_digest_name}" =~ ^[0-9a-f]{64}$ ]]; then
      echo "scaling trust anchor ${scaling_digest_name} changed at ${checkpoint}" >&2
      return 1
    fi
  done
  local observed_scaling_manifest
  if ! observed_scaling_manifest="$(
    canonical_path "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
  )" \
    || [[ "$observed_scaling_manifest" \
        != "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
      || "${observed_scaling_manifest##*/}" != scaling_evidence.json ]]; then
    echo "G-SCALE evidence path changed or became noncanonical at ${checkpoint}" >&2
    return 1
  fi
  local observed_target expected_target
  if [[ -e "$repo_root/target" || -L "$repo_root/target" ]]; then
    echo "sealed source unexpectedly contains a target path at ${checkpoint}" >&2
    return 1
  fi
  if ! require_external_cargo_target_dir "$repo_root" \
    || ! require_external_release_artifact_root "$repo_root" \
    || ! require_disjoint_release_roots "$repo_root"; then
    echo "sealed release output roots changed at ${checkpoint}" >&2
    return 1
  fi
  observed_target="$(
    python3 -I -S -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
      "$CARGO_TARGET_DIR"
  )"
  expected_target="$(
    python3 -I -S -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
      "$IROHA_RELEASE_WORKSPACE_TARGET"
  )"
  if [[ "$observed_target" != "$expected_target" ]]; then
    echo "sealed release target changed at ${checkpoint}" >&2
    return 1
  fi
  local expected observed
  expected="$(<"$IROHA_RELEASE_EXPECTED_IDENTITY_PATH")"
  if ! observed="$(release_identity_json)"; then
    echo "failed to compute release identity at ${checkpoint}" >&2
    return 1
  fi
  if [[ "$observed" != "$expected" ]]; then
    echo "release source identity changed at ${checkpoint}" >&2
    return 1
  fi
  if ! python3 -I -S scripts/seal_workspace_source.py \
    --verify --root "$repo_root" --no-writable-paths; then
    echo "release source seal changed at ${checkpoint}" >&2
    return 1
  fi
}
record_corridor_log() {
  local leg_id="$1"
  local kind="$2"
  local required_test_count="$3"
  local command_text="$4"
  local log_path="$5"
  local command_status="$6"
  local tee_status="$7"
  corridor_last_log="$log_path"
  ((corridor_enabled)) || return 0
  if ((command_status != 0 || tee_status != 0)); then
    echo "release corridor leg ${leg_id} failed (command=${command_status}, tee=${tee_status})" >&2
    return 1
  fi
  local observed_test_count
  observed_test_count="$(python3 -I -S - "$kind" "$log_path" <<'PY'
from pathlib import Path
import re
import sys

kind, raw_path = sys.argv[1:]
lines = Path(raw_path).read_text(encoding="utf-8").splitlines()
if kind == "cargo-focus":
    running = [line for line in lines if line == "running 1 test"]
    results = [
        line
        for line in lines
        if re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; "
            r"0 measured; [0-9]+ filtered out; finished in .+",
            line,
        )
    ]
    if not running or len(running) != len(results):
        raise SystemExit("ambiguous focused Cargo test transcript")
    print(len(results))
elif kind.startswith("cargo-"):
    running = [re.fullmatch(r"running ([0-9]+) tests?", line) for line in lines]
    running = [match for match in running if match]
    results = [
        re.fullmatch(
            r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
            r"0 measured; [0-9]+ filtered out; finished in .+",
            line,
        )
        for line in lines
    ]
    results = [match for match in results if match]
    if len(running) != 1 or len(results) != 1 or running[0].group(1) != results[0].group(1):
        raise SystemExit("ambiguous Cargo test transcript")
    print(results[0].group(1))
elif kind == "pytest":
    matches = [
        re.fullmatch(
            r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"
            r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?",
            line,
        )
        for line in lines
    ]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise SystemExit("ambiguous pytest transcript")
    print(matches[0].group(1))
elif kind == "node":
    passed = [re.fullmatch(r"# pass ([0-9]+)", line) for line in lines]
    passed = [match for match in passed if match]
    if (
        len(passed) != 1
        or lines.count(f"# tests {passed[0].group(1)}") != 1
        or lines.count("# fail 0") != 1
        or lines.count("# cancelled 0") != 1
        or lines.count("# skipped 0") != 1
        or lines.count("# todo 0") != 1
    ):
        raise SystemExit("ambiguous Node test transcript")
    print(passed[0].group(1))
elif kind == "native-amx-sdk":
    matches = [
        re.fullmatch(
            r"native-amx-v2-grouped-parity surface=[a-z]+ tests=([0-9]+) "
            r"fixture_sha256=[0-9a-f]{64} "
            r"suite_source_manifest_sha256=[0-9a-f]{64}",
            line,
        )
        for line in lines
    ]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise SystemExit("ambiguous grouped Native AMX V2 SDK transcript")
    print(matches[0].group(1))
elif kind == "sdk-diagnostics":
    matches = [
        re.fullmatch(
            r"sumeragi-v2-sdk-diagnostics surface=[a-z]+ tests=([0-9]+) "
            r"suite_source_manifest_sha256=[0-9a-f]{64}",
            line,
        )
        for line in lines
    ]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise SystemExit("ambiguous Sumeragi v2 SDK diagnostics transcript")
    print(matches[0].group(1))
elif kind == "command":
    print(0)
else:
    raise SystemExit(f"unknown corridor leg kind: {kind}")
PY
)" || {
    echo "release corridor leg ${leg_id} has invalid test output" >&2
    return 1
  }
  if [[ "$kind" == "native-amx-sdk" ]]; then
    local expected_surface="${leg_id#native-amx-grouped-}"
    local expected_marker
    expected_marker="native-amx-v2-grouped-parity surface=${expected_surface} tests=${observed_test_count} fixture_sha256=${native_amx_grouped_fixture_sha256:-} suite_source_manifest_sha256=${native_amx_grouped_suite_source_manifest_sha256:-}"
    if [[ "$(grep -Fxc -- "$expected_marker" "$log_path" || true)" != 1 ]]; then
      echo "release corridor leg ${leg_id} is not bound to the exact grouped Native AMX V2 corpus and suite sources" >&2
      return 1
    fi
  elif [[ "$kind" == "sdk-diagnostics" ]]; then
    local expected_surface="${leg_id#sumeragi-diagnostics-}"
    local expected_marker
    expected_marker="sumeragi-v2-sdk-diagnostics surface=${expected_surface} tests=${observed_test_count} suite_source_manifest_sha256=${sumeragi_v2_sdk_diagnostics_suite_source_manifest_sha256:-}"
    if [[ "$(grep -Fxc -- "$expected_marker" "$log_path" || true)" != 1 ]]; then
      echo "release corridor leg ${leg_id} is not bound to the exact Sumeragi v2 SDK diagnostics suite sources" >&2
      return 1
    fi
  fi
  if [[ "$kind" == "cargo-module" ]]; then
    if ((observed_test_count == 0 || observed_test_count < required_test_count)); then
      echo "release corridor module ${leg_id} ran fewer tests than its required inventory" >&2
      return 1
    fi
  elif [[ "$observed_test_count" != "$required_test_count" ]]; then
    echo "release corridor leg ${leg_id} ran ${observed_test_count} tests, expected ${required_test_count}" >&2
    return 1
  fi
  local relative_log log_sha256
  relative_log="logs/$(basename "$log_path")"
  log_sha256="$(sha256_file "$log_path")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$corridor_leg_index" "$leg_id" "$kind" "$required_test_count" \
    "$observed_test_count" "$command_status" "$tee_status" "$log_sha256" \
    "$relative_log" "$command_text" >>"$corridor_summary"
  ((corridor_leg_index += 1))
}

run_corridor_leg() {
  local leg_id="$1"
  local kind="$2"
  local required_test_count="$3"
  local command_text="$4"
  shift 4
  release_gate_boundary "${leg_id}:before" || return $?
  if ((!corridor_enabled)); then
    "$@"
    release_gate_boundary "${leg_id}:after-natural-completion" || return $?
    return
  fi
  local log_path
  printf -v log_path '%s/%02d-%s.log' \
    "$corridor_logs_dir" "$corridor_leg_index" "$leg_id"
  set +e
  "$@" 2>&1 | tee "$log_path"
  local pipeline_status=("${PIPESTATUS[@]}")
  set -e
  record_corridor_log \
    "$leg_id" "$kind" "$required_test_count" "$command_text" "$log_path" \
    "${pipeline_status[0]}" "${pipeline_status[1]}"
  release_gate_boundary "${leg_id}:after-natural-completion" || return $?
}

run_cooperative_gate() {
  local gate_id="$1"
  local status
  shift
  release_gate_boundary "${gate_id}:before" || return $?
  if "$@"; then
    status=0
  else
    status=$?
  fi
  release_gate_boundary "${gate_id}:after-natural-completion" || return $?
  return "$status"
}

corridor_contract_log_path() {
  local leg_id="$1"
  if ((corridor_enabled)); then
    printf '%s/%02d-%s.log\n' \
      "$corridor_logs_dir" "$corridor_leg_index" "$leg_id"
  else
    mktemp "${TMPDIR:-/tmp}/${leg_id}.XXXXXX"
  fi
}
is_production_data_model_module() {
  local candidate="$1"
  local data_model_module
  for data_model_module in "${production_data_model_modules[@]}"; do
    [[ "$candidate" == "$data_model_module" ]] && return 0
  done
  return 1
}
run_multilane_focus_crate_tests() {
  local package="$1"
  shift
  local focus_test
  for focus_test in "$@"; do
    run_cargo test --locked --offline -p "$package" --lib \
      "$focus_test" -- --exact --test-threads=1 || return
  done
}

run_multilane_focus_test_target() {
  local package="$1"
  local test_target="$2"
  shift 2
  local focus_test
  for focus_test in "$@"; do
    run_cargo test --locked --offline -p "$package" --test "$test_target" \
      "$focus_test" -- --exact --test-threads=1 || return
  done
}

append_g_unit_inventory() {
  local leg_id="$1"
  local package="$2"
  shift 2
  local focus_test
  for focus_test in "$@"; do
    printf '%s\t%s\t%s\n' "$leg_id" "$package" "$focus_test" \
      >>"$corridor_g_unit_inventory"
  done
}

require_g_unit_log_results() {
  local focus_test
  for focus_test in "$@"; do
    if [[ "$(grep -Fxc -- "test ${focus_test} ... ok" "$corridor_last_log" || true)" != 1 ]]; then
      echo "G-UNIT corridor log does not contain one passing result for ${focus_test}" >&2
      return 1
    fi
  done
}
