#!/usr/bin/env bash
# Regenerate Android Norito binding documentation twice from the exact clean
# HEAD commit, require byte-identical checked-in outputs, and stage a
# deterministic documentation archive. Requires Cargo, Git, Make, and Python 3.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCS_REL="specs/sdk/android/generated"
SUMMARY_REL="artifacts/android/codegen_parity_summary.json"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/android/codegen_docs"
TMP_ROOT=""
WORKTREES=()

usage() {
  cat <<'EOF'
Usage: ci/check_android_codegen.sh

Run the Android Norito codegen/docs pipeline in two detached worktrees at the
clean checked-out HEAD. The gate fails on missing, extra, stale, symlinked, or
non-deterministic generated files and parity summaries. On success it stages a
deterministic generated-docs archive under artifacts/android/codegen_docs/.
EOF
}

if [[ "$#" -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[android-codegen] error: unrecognized argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

cleanup() {
  local worktree
  for worktree in "${WORKTREES[@]}"; do
    git -C "${ROOT_DIR}" worktree remove --force "${worktree}" >/dev/null 2>&1 || true
  done
  if [[ -n "${TMP_ROOT}" && -d "${TMP_ROOT}" ]]; then
    rm -rf -- "${TMP_ROOT}"
  fi
}
trap cleanup EXIT

for tool in cargo git make python3; do
  if ! command -v "${tool}" >/dev/null 2>&1; then
    echo "[android-codegen] error: required tool is unavailable: ${tool}" >&2
    exit 1
  fi
done

cd "${ROOT_DIR}"
if [[ -n "$(git status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "[android-codegen] error: deterministic binding generation requires a clean checkout" >&2
  exit 1
fi
if [[ ! -f Cargo.lock || -L Cargo.lock ]]; then
  echo "[android-codegen] error: the pinned root Cargo.lock must be a regular file" >&2
  exit 1
fi

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-android-codegen.XXXXXX")"
HEAD_COMMIT="$(git rev-parse --verify 'HEAD^{commit}')"

case "${CARGO_TARGET_DIR:-target/android-codegen-ci}" in
  /*) CODEGEN_CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/android-codegen-ci}" ;;
  *) CODEGEN_CARGO_TARGET_DIR="${ROOT_DIR}/${CARGO_TARGET_DIR:-target/android-codegen-ci}" ;;
esac

create_replay_worktree() {
  local worktree="$1"
  WORKTREES+=("${worktree}")
  git -C "${ROOT_DIR}" worktree add --quiet --detach "${worktree}" "${HEAD_COMMIT}"
  cp "${ROOT_DIR}/Cargo.lock" "${worktree}/Cargo.lock"
  if ! cmp -s "${ROOT_DIR}/Cargo.lock" "${worktree}/Cargo.lock"; then
    echo "[android-codegen] error: isolated replay Cargo.lock copy changed bytes" >&2
    exit 1
  fi
  if [[ -n "$(git -C "${worktree}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "[android-codegen] error: isolated replay worktree is not clean" >&2
    exit 1
  fi
}

run_replay() {
  local worktree="$1"
  (
    cd "${worktree}"
    CARGO_TARGET_DIR="${CODEGEN_CARGO_TARGET_DIR}" make android-codegen-verify
  )
  if [[ -n "$(git -C "${worktree}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "[android-codegen] error: checked-in Android generated bindings are stale or the generator mutated an unexpected path" >&2
    git -C "${worktree}" status --short --untracked-files=all >&2 || true
    exit 1
  fi
  for required in \
    "${worktree}/${DOCS_REL}/codegen_hash_tree.json" \
    "${worktree}/${DOCS_REL}/codegen_manifest_metadata.json" \
    "${worktree}/${DOCS_REL}/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json" \
    "${worktree}/${SUMMARY_REL}"; do
    if [[ ! -f "${required}" || -L "${required}" ]]; then
      echo "[android-codegen] error: replay omitted required regular output: ${required#"${worktree}/"}" >&2
      exit 1
    fi
  done
}

build_deterministic_archive() {
  local source="$1"
  local output="$2"
  python3 - "${source}" "${output}" <<'PY'
from __future__ import annotations

import gzip
import stat
import sys
import tarfile
from pathlib import Path

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

FIRST_WORKTREE="${TMP_ROOT}/source-first"
SECOND_WORKTREE="${TMP_ROOT}/source-second"
create_replay_worktree "${FIRST_WORKTREE}"
create_replay_worktree "${SECOND_WORKTREE}"
run_replay "${FIRST_WORKTREE}"
run_replay "${SECOND_WORKTREE}"

if ! diff -ru "${FIRST_WORKTREE}/${DOCS_REL}" "${SECOND_WORKTREE}/${DOCS_REL}" >/dev/null; then
  diff -ru "${FIRST_WORKTREE}/${DOCS_REL}" "${SECOND_WORKTREE}/${DOCS_REL}" >&2 || true
  echo "[android-codegen] error: two clean Android binding generations produced different bytes" >&2
  exit 1
fi
if ! cmp -s "${FIRST_WORKTREE}/${SUMMARY_REL}" "${SECOND_WORKTREE}/${SUMMARY_REL}"; then
  diff -u "${FIRST_WORKTREE}/${SUMMARY_REL}" "${SECOND_WORKTREE}/${SUMMARY_REL}" >&2 || true
  echo "[android-codegen] error: two clean Android parity summaries disagreed" >&2
  exit 1
fi

FIRST_ARCHIVE="${TMP_ROOT}/generated-docs-first.tar.gz"
SECOND_ARCHIVE="${TMP_ROOT}/generated-docs-second.tar.gz"
build_deterministic_archive "${FIRST_WORKTREE}/${DOCS_REL}" "${FIRST_ARCHIVE}"
build_deterministic_archive "${SECOND_WORKTREE}/${DOCS_REL}" "${SECOND_ARCHIVE}"
if ! cmp -s "${FIRST_ARCHIVE}" "${SECOND_ARCHIVE}"; then
  echo "[android-codegen] error: two clean Android documentation archives produced different bytes" >&2
  exit 1
fi

mkdir -p "${ARTIFACT_DIR}"
cp "${FIRST_WORKTREE}/${DOCS_REL}/codegen_hash_tree.json" \
  "${ARTIFACT_DIR}/hash_tree.json"
cp "${FIRST_WORKTREE}/${DOCS_REL}/codegen_hash_tree.json" \
  "${ARTIFACT_DIR}/codegen_hash_tree.json"
cp "${FIRST_WORKTREE}/${SUMMARY_REL}" \
  "${ARTIFACT_DIR}/codegen_parity_summary.json"
cp "${FIRST_ARCHIVE}" "${ARTIFACT_DIR}/generated-md.tar.gz"

echo "[android-codegen] two clean generations were byte-identical; artifacts ready under ${ARTIFACT_DIR}"
