#!/usr/bin/env bash
# Regenerate Android Norito documentation twice from exact sealed source mirrors.
# shellcheck source-path=SCRIPTDIR
set -euo pipefail

# Git provenance must not inherit caller-selected routing or configuration.
while IFS= read -r openapi_git_variable; do
  [[ "${openapi_git_variable}" == GIT_* ]] && unset "${openapi_git_variable}"
done < <(compgen -e)
export GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1 GIT_NO_REPLACE_OBJECTS=1
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_COUNT=2
export GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/dev/null
export GIT_CONFIG_KEY_1=core.fsmonitor GIT_CONFIG_VALUE_1=false

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROCESS_POLICY="${ROOT_DIR}/scripts/sumeragi_v2_release_process_policy.sh"
if [[ ! -f "${PROCESS_POLICY}" || -L "${PROCESS_POLICY}" ]]; then
  echo "[android-codegen] error: shared release process policy is unavailable or symbolic" >&2
  exit 2
fi
# shellcheck source=../scripts/sumeragi_v2_release_process_policy.sh
source "${PROCESS_POLICY}"

DOCS_REL="specs/sdk/android/generated"
SUMMARY_REL="artifacts/android/codegen_parity_summary.json"
RUN_ROOT=""

usage() {
  cat <<'EOF'
Usage: ci/check_android_codegen.sh

Regenerate Android Norito documentation in two hard-link-free, sealed mirrors
of the exact clean HEAD commit. Each replay receives its own fresh
/private/tmp Cargo target and output stage. Checked-in docs and parity summary
must match both byte-for-byte. Evidence remains outside the repository.
EOF
}

if [[ "$#" -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf '[android-codegen] error: unrecognized argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

umask 077
for tool in cargo git node python3 tee; do
  if ! command -v "${tool}" >/dev/null 2>&1; then
    printf '[android-codegen] error: required tool is unavailable: %s\n' "${tool}" >&2
    exit 1
  fi
done

RUN_ROOT="$(mktemp -d /private/tmp/iroha-android-codegen.XXXXXX)"
chmod 700 "${RUN_ROOT}"
FIRST_TARGET="${RUN_ROOT}/target-first"
SECOND_TARGET="${RUN_ROOT}/target-second"
mkdir -m 700 "${FIRST_TARGET}" "${SECOND_TARGET}"
if [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  IROHA_RELEASE_ARTIFACT_ROOT="${RUN_ROOT}/artifacts"
  mkdir -m 700 "${IROHA_RELEASE_ARTIFACT_ROOT}"
  IROHA_RELEASE_CANCEL_REQUEST_PATH="${RUN_ROOT}/cancel-request.json"
  export IROHA_RELEASE_ARTIFACT_ROOT IROHA_RELEASE_CANCEL_REQUEST_PATH
elif [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  echo "[android-codegen] error: IROHA_RELEASE_ARTIFACT_ROOT and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together" >&2
  exit 2
fi
require_external_release_artifact_root "${ROOT_DIR}"
CANCEL_REQUEST_PARENT="${IROHA_RELEASE_CANCEL_REQUEST_PATH%/*}"
if [[ -z "${CANCEL_REQUEST_PARENT}" ]]; then
  echo "[android-codegen] error: IROHA_RELEASE_CANCEL_REQUEST_PATH must name a file below a private external directory" >&2
  exit 2
fi
require_external_private_directory \
  "${ROOT_DIR}" "${CANCEL_REQUEST_PARENT}" "release cancellation marker parent"
CARGO_TARGET_DIR="${FIRST_TARGET}" \
  require_disjoint_release_roots "${ROOT_DIR}"
CARGO_TARGET_DIR="${SECOND_TARGET}" \
  require_disjoint_release_roots "${ROOT_DIR}"
release_gate_boundary "android-codegen:channels-ready"
EVIDENCE_DIR="$(mktemp -d "${IROHA_RELEASE_ARTIFACT_ROOT}/android-codegen.XXXXXX")"
chmod 700 "${EVIDENCE_DIR}"
require_release_artifact_directory "${EVIDENCE_DIR}"
LOG_PATH="${EVIDENCE_DIR}/android-codegen.log"
exec > >(tee "${LOG_PATH}") 2>&1

report_retained_evidence() {
  local status=$?
  trap - EXIT
  printf '[android-codegen] retained immutable mirrors, targets, and stages: %s\n' \
    "${RUN_ROOT}" >&2
  printf '[android-codegen] authenticated evidence root: %s\n' \
    "${EVIDENCE_DIR}" >&2
  exit "${status}"
}
trap report_retained_evidence EXIT
printf '[android-codegen] authenticated artifact root: %s\n' \
  "${IROHA_RELEASE_ARTIFACT_ROOT}" >&2
printf '[android-codegen] cooperative cancellation marker: %s\n' \
  "${IROHA_RELEASE_CANCEL_REQUEST_PATH}" >&2

if [[ -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "[android-codegen] error: deterministic binding generation requires a clean checkout" >&2
  exit 1
fi
if [[ ! -f "${ROOT_DIR}/Cargo.lock" || -L "${ROOT_DIR}/Cargo.lock" ]]; then
  echo "[android-codegen] error: the pinned tracked root Cargo.lock must be a regular file" >&2
  exit 1
fi
HEAD_COMMIT="$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" rev-parse --verify 'HEAD^{commit}')"
HEAD_TREE="$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" rev-parse --verify "${HEAD_COMMIT}^{tree}")"

CARGO_LOCK_SNAPSHOT="${RUN_ROOT}/Cargo.lock"
python3 -I -S - "${ROOT_DIR}/Cargo.lock" "${CARGO_LOCK_SNAPSHOT}" <<'PY'
import os
import stat
import sys

source, output = sys.argv[1:]
before = os.lstat(source)
if (
    not stat.S_ISREG(before.st_mode)
    or stat.S_ISLNK(before.st_mode)
    or before.st_nlink != 1
    or before.st_uid != os.getuid()
):
    raise SystemExit("pinned Cargo.lock input is not one owner-bound regular file")
read_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
source_fd = os.open(source, read_flags)
try:
    opened = os.fstat(source_fd)
    chunks = []
    total = 0
    while True:
        chunk = os.read(source_fd, 65536)
        if not chunk:
            break
        total += len(chunk)
        if total > 4 * 1024 * 1024:
            raise SystemExit("pinned Cargo.lock exceeds the release input bound")
        chunks.append(chunk)
    after = os.fstat(source_fd)
finally:
    os.close(source_fd)
identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
if identity(before) != identity(opened) or identity(opened) != identity(after):
    raise SystemExit("pinned Cargo.lock changed while it was read")
payload = b"".join(chunks)
if not payload:
    raise SystemExit("pinned Cargo.lock is empty")
write_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
output_fd = os.open(output, write_flags, 0o400)
try:
    view = memoryview(payload)
    while view:
        written = os.write(output_fd, view)
        if written <= 0:
            raise OSError("Cargo.lock snapshot write made no progress")
        view = view[written:]
    os.fsync(output_fd)
finally:
    os.close(output_fd)
directory_fd = os.open(os.path.dirname(output), os.O_RDONLY)
try:
    os.fsync(directory_fd)
finally:
    os.close(directory_fd)
PY

validate_regular_tree() {
  local root="$1"
  local purpose="$2"
  python3 -I -S - "${root}" "${purpose}" <<'PY'
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
purpose = sys.argv[2]
try:
    observed = root.lstat()
except OSError as error:
    raise SystemExit(f"{purpose} root is unavailable: {error}") from error
if stat.S_ISLNK(observed.st_mode) or not stat.S_ISDIR(observed.st_mode):
    raise SystemExit(f"{purpose} root must be one real directory")
for path in root.rglob("*"):
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode):
        raise SystemExit(f"{purpose} must not contain symlinks")
    if not (stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)):
        raise SystemExit(f"{purpose} contains an unsupported file type")
PY
}

create_replay_clone() {
  local source_root="$1"
  local target_root="$2"
  local stage_root="$3"

  release_gate_boundary "android-codegen:before-source-mirror"
  git clone --quiet --local --no-hardlinks --no-checkout \
    "${ROOT_DIR}" "${source_root}"
  GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" checkout --quiet --detach "${HEAD_COMMIT}"
  if [[ -e "${source_root}/.git/objects/info/alternates" || \
        -L "${source_root}/.git/objects/info/alternates" ]]; then
    echo "[android-codegen] error: replay source borrowed an object database" >&2
    exit 1
  fi

  (
    cd "${source_root}"
    node tools/openapi/scripts/provision-openapi-cargo-lock.mjs \
      provision --source="${CARGO_LOCK_SNAPSHOT}"
  )
  if ! cmp -s "${CARGO_LOCK_SNAPSHOT}" "${source_root}/Cargo.lock"; then
    echo "[android-codegen] error: replay Cargo.lock differs from the pinned input" >&2
    exit 1
  fi
  if [[ -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "[android-codegen] error: isolated replay source is not clean" >&2
    exit 1
  fi

  mkdir -m 700 "${stage_root}"
  require_external_private_directory "${source_root}" "${target_root}" "Cargo target"
  require_external_private_directory "${source_root}" "${stage_root}" "Android codegen stage"
  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --seal --root "${source_root}" --no-writable-paths
  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --verify --root "${source_root}" --no-writable-paths
  release_gate_boundary "android-codegen:after-source-mirror"
}

verify_source_identity() {
  local source_root="$1"
  local actual_commit
  local actual_tree

  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --verify --root "${source_root}" --no-writable-paths
  actual_commit="$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" rev-parse --verify 'HEAD^{commit}')"
  actual_tree="$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" rev-parse --verify 'HEAD^{tree}')"
  if [[ "${actual_commit}" != "${HEAD_COMMIT}" || "${actual_tree}" != "${HEAD_TREE}" || \
        -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "[android-codegen] error: immutable replay source identity changed" >&2
    exit 1
  fi
  if ! cmp -s "${CARGO_LOCK_SNAPSHOT}" "${source_root}/Cargo.lock"; then
    echo "[android-codegen] error: replay Cargo.lock identity changed" >&2
    exit 1
  fi
}

write_hash_tree() {
  local generated_root="$1"
  local output="$2"
  python3 -I -S - "${generated_root}" "${output}" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
output = Path(sys.argv[2])
logical_root = Path("specs/sdk/android/generated")
if root.is_symlink() or not root.is_dir():
    raise SystemExit("Android generated-doc stage must be a real directory")
entries = []
for entry in sorted(root.rglob("*"), key=lambda path: path.relative_to(root).as_posix()):
    metadata = entry.lstat()
    if stat.S_ISLNK(metadata.st_mode):
        raise SystemExit("Android generated-doc stage must not contain symlinks")
    if stat.S_ISDIR(metadata.st_mode):
        continue
    if not stat.S_ISREG(metadata.st_mode):
        raise SystemExit("Android generated-doc stage contains an unsupported file type")
    if entry == output or entry.name == ".DS_Store":
        continue
    relative = entry.relative_to(root)
    payload = entry.read_bytes()
    entries.append(
        {
            "path": (logical_root / relative).as_posix(),
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size": len(payload),
        }
    )
concatenated = "\n".join(
    f"{item['sha256']}  {item['path']}" for item in entries
).encode("utf-8")
document = {
    "root": logical_root.as_posix(),
    "tree_sha256": hashlib.sha256(concatenated).hexdigest(),
    "files": entries,
}
flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
descriptor = os.open(output, flags, 0o600)
with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
    json.dump(document, stream, indent=2)
    stream.write("\n")
    stream.flush()
    os.fsync(stream.fileno())
PY
}

run_replay() {
  local source_root="$1"
  local target_root="$2"
  local stage_root="$3"
  local codegen_root="${stage_root}/codegen"
  local generated_root="${stage_root}/generated"
  local summary_path="${stage_root}/codegen_parity_summary.json"
  local temporary_root="${stage_root}/tmp"

  export CARGO_TARGET_DIR="${target_root}"
  export TMPDIR="${temporary_root}"
  mkdir -m 700 "${temporary_root}"
  require_external_cargo_target_dir "${source_root}"
  release_gate_boundary "android-codegen:before-export"
  (
    cd "${source_root}"
    run_cargo run \
      --locked \
      --offline \
      -p norito_codegen_exporter \
      --features dev-tools \
      -- \
      --out "${codegen_root}"
  )
  release_gate_boundary "android-codegen:after-export"
  validate_regular_tree "${codegen_root}" "Android codegen intermediate"

  python3 "${source_root}/scripts/android_codegen_docs.py" \
    --manifest "${codegen_root}/instruction_manifest.json" \
    --builders "${codegen_root}/builder_index.json" \
    --out "${generated_root}"
  mkdir -p "${generated_root}/fixtures"
  cp "${source_root}/${DOCS_REL}/fixtures/smart_contract_code_executor_hashes.json" \
    "${generated_root}/fixtures/smart_contract_code_executor_hashes.json"

  CARGO_TARGET_DIR="${target_root}" \
    python3 "${source_root}/scripts/android_codegen_replay_sorafs_fixture.py" \
      --fixture-dir "${source_root}/fixtures/sorafs_orchestrator/multi_peer_parity_v1" \
      --chunker-fixture "${source_root}/fixtures/sorafs_chunker/sf1_profile_v1.json" \
      --register-pin-example \
        "${codegen_root}/instruction_examples/iroha_data_model::isi::sorafs::RegisterPinManifest.json" \
      --report-dir "${codegen_root}/sorafs_manifest" \
      --tracked-fixture-out \
        "${generated_root}/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json" \
      --cargo-bin "${source_root}/scripts/sumeragi_v2_release_cargo_proxy.sh"

  write_hash_tree "${generated_root}" "${generated_root}/codegen_hash_tree.json"
  python3 "${source_root}/scripts/check_android_codegen_parity.py" \
    --manifest "${codegen_root}/instruction_manifest.json" \
    --builder-index "${codegen_root}/builder_index.json" \
    --metadata "${generated_root}/codegen_manifest_metadata.json" \
    --json-out "${summary_path}" \
    --js-source "${source_root}/javascript/iroha_js/src/norito.js"

  for required in \
    "${generated_root}/codegen_hash_tree.json" \
    "${generated_root}/codegen_manifest_metadata.json" \
    "${generated_root}/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json" \
    "${summary_path}"; do
    if [[ ! -f "${required}" || -L "${required}" ]]; then
      printf '[android-codegen] error: replay omitted required regular output: %s\n' \
        "${required}" >&2
      exit 1
    fi
  done
  validate_regular_tree "${generated_root}" "Android generated-doc stage"
  verify_source_identity "${source_root}"
  release_gate_boundary "android-codegen:after-replay"
}

build_deterministic_archive() {
  local source="$1"
  local output="$2"
  python3 -I -S - "${source}" "${output}" <<'PY'
from __future__ import annotations

import gzip
from pathlib import Path
import stat
import sys
import tarfile

source = Path(sys.argv[1])
output = Path(sys.argv[2])
if source.is_symlink() or not source.is_dir():
    raise SystemExit("Android generated-doc source must be a real directory")
paths = [source, *sorted(source.rglob("*"), key=lambda path: path.relative_to(source).as_posix())]
with output.open("xb") as raw:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
            for path in paths:
                metadata = path.lstat()
                if stat.S_ISLNK(metadata.st_mode):
                    raise SystemExit("Android generated-doc archive must not contain symlinks")
                if not (stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)):
                    raise SystemExit("Android generated-doc archive contains an unsupported file type")
                relative = Path("generated") / path.relative_to(source)
                info = archive.gettarinfo(str(path), arcname=relative.as_posix())
                info.uid = 0
                info.gid = 0
                info.uname = ""
                info.gname = ""
                info.mtime = 0
                info.mode = 0o755 if info.isdir() else 0o644
                if info.isfile():
                    with path.open("rb") as payload:
                        archive.addfile(info, payload)
                else:
                    archive.addfile(info)
PY
}

FIRST_SOURCE="${RUN_ROOT}/source-first"
SECOND_SOURCE="${RUN_ROOT}/source-second"
FIRST_STAGE="${RUN_ROOT}/stage-first"
SECOND_STAGE="${RUN_ROOT}/stage-second"
create_replay_clone "${FIRST_SOURCE}" "${FIRST_TARGET}" "${FIRST_STAGE}"
create_replay_clone "${SECOND_SOURCE}" "${SECOND_TARGET}" "${SECOND_STAGE}"
run_replay "${FIRST_SOURCE}" "${FIRST_TARGET}" "${FIRST_STAGE}"
run_replay "${SECOND_SOURCE}" "${SECOND_TARGET}" "${SECOND_STAGE}"

validate_regular_tree "${ROOT_DIR}/${DOCS_REL}" "checked-in Android generated documentation"
if [[ ! -f "${ROOT_DIR}/${SUMMARY_REL}" || -L "${ROOT_DIR}/${SUMMARY_REL}" ]]; then
  echo "[android-codegen] error: checked-in Android parity summary is not regular" >&2
  exit 1
fi

if ! diff -ru "${FIRST_STAGE}/generated" "${SECOND_STAGE}/generated" >/dev/null; then
  diff -ru "${FIRST_STAGE}/generated" "${SECOND_STAGE}/generated" >&2 || true
  echo "[android-codegen] error: two clean Android binding generations produced different bytes" >&2
  exit 1
fi
if ! cmp -s "${FIRST_STAGE}/codegen_parity_summary.json" \
              "${SECOND_STAGE}/codegen_parity_summary.json"; then
  diff -u "${FIRST_STAGE}/codegen_parity_summary.json" \
          "${SECOND_STAGE}/codegen_parity_summary.json" >&2 || true
  echo "[android-codegen] error: two clean Android parity summaries disagreed" >&2
  exit 1
fi
if ! diff -ru "${ROOT_DIR}/${DOCS_REL}" "${FIRST_STAGE}/generated" >/dev/null; then
  diff -ru "${ROOT_DIR}/${DOCS_REL}" "${FIRST_STAGE}/generated" >&2 || true
  echo "[android-codegen] error: checked-in Android generated documentation is stale" >&2
  exit 1
fi
if ! cmp -s "${ROOT_DIR}/${SUMMARY_REL}" "${FIRST_STAGE}/codegen_parity_summary.json"; then
  diff -u "${ROOT_DIR}/${SUMMARY_REL}" \
          "${FIRST_STAGE}/codegen_parity_summary.json" >&2 || true
  echo "[android-codegen] error: checked-in Android parity summary is stale" >&2
  exit 1
fi

verify_source_identity "${FIRST_SOURCE}"
verify_source_identity "${SECOND_SOURCE}"
CALLER_COMMIT="$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" rev-parse --verify 'HEAD^{commit}')"
CALLER_TREE="$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" rev-parse --verify 'HEAD^{tree}')"
if [[ "${CALLER_COMMIT}" != "${HEAD_COMMIT}" || "${CALLER_TREE}" != "${HEAD_TREE}" || \
      -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${ROOT_DIR}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "[android-codegen] error: caller checkout changed during replay" >&2
  exit 1
fi
if ! cmp -s "${CARGO_LOCK_SNAPSHOT}" "${ROOT_DIR}/Cargo.lock"; then
  echo "[android-codegen] error: caller Cargo.lock changed during replay" >&2
  exit 1
fi

FIRST_ARCHIVE="${RUN_ROOT}/generated-docs-first.tar.gz"
SECOND_ARCHIVE="${RUN_ROOT}/generated-docs-second.tar.gz"
build_deterministic_archive "${FIRST_STAGE}/generated" "${FIRST_ARCHIVE}"
build_deterministic_archive "${SECOND_STAGE}/generated" "${SECOND_ARCHIVE}"
if ! cmp -s "${FIRST_ARCHIVE}" "${SECOND_ARCHIVE}"; then
  echo "[android-codegen] error: two clean Android documentation archives produced different bytes" >&2
  exit 1
fi

release_gate_boundary "android-codegen:before-completion-publication"
cp "${FIRST_ARCHIVE}" "${EVIDENCE_DIR}/generated-docs-first.tar.gz"
cp "${SECOND_ARCHIVE}" "${EVIDENCE_DIR}/generated-docs-second.tar.gz"
cp "${FIRST_STAGE}/generated/codegen_hash_tree.json" "${EVIDENCE_DIR}/codegen_hash_tree.json"
cp "${FIRST_STAGE}/codegen_parity_summary.json" "${EVIDENCE_DIR}/codegen_parity_summary.json"
cp "${FIRST_ARCHIVE}" "${EVIDENCE_DIR}/generated-md.tar.gz"
python3 -I -S - \
  "${EVIDENCE_DIR}/COMPLETED.json" \
  "${HEAD_COMMIT}" \
  "${HEAD_TREE}" \
  "${FIRST_SOURCE}" \
  "${SECOND_SOURCE}" <<'PY'
import json
import os
import sys

receipt, commit, tree, source_first, source_second = sys.argv[1:]
descriptor = os.open(receipt, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
    json.dump(
        {
            "candidate_commit": commit,
            "candidate_tree": tree,
            "schema_version": 1,
            "source_mirrors": [source_first, source_second],
        },
        stream,
        sort_keys=True,
        separators=(",", ":"),
    )
    stream.write("\n")
PY
release_gate_boundary "android-codegen:after-completion-publication"
echo "[android-codegen] two sealed generations matched checked-in bytes; evidence: ${EVIDENCE_DIR}"
