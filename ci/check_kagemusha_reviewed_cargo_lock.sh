#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:---verify}"
EXPECTED_BYTES=315545
EXPECTED_SHA256="88398dc1838777493c314ee26c56ba0abd797f0f66ba30a879181f13306c5a11"
REVIEWED_LOCK="${ROOT_DIR}/fixtures/kagemusha/cargo-lock.reviewed.v1"
WORKSPACE_LOCK="${ROOT_DIR}/Cargo.lock"

if [[ $# -gt 1 ]]; then
  echo "usage: ci/check_kagemusha_reviewed_cargo_lock.sh [--materialize|--verify|--self-test]" >&2
  exit 2
fi

verify_lock() {
  local path="$1"
  local label="$2"
  python3 - "${path}" "${label}" "${EXPECTED_BYTES}" "${EXPECTED_SHA256}" <<'PY'
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
label = sys.argv[2]
expected_bytes = int(sys.argv[3])
expected_sha256 = sys.argv[4]

try:
    before = path.lstat()
except FileNotFoundError:
    raise SystemExit(f"{label} is missing")
if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
    raise SystemExit(f"{label} must be a singly linked regular file")
if before.st_mode & 0o111:
    raise SystemExit(f"{label} must not be executable")
if before.st_size != expected_bytes:
    raise SystemExit(
        f"{label} size drifted: "
        f"expected={expected_bytes} actual={before.st_size}"
    )

flags = os.O_RDONLY
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
fd = os.open(path, flags)
try:
    opened = os.fstat(fd)
    identity = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_nlink,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
    )
    if identity != (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ):
        raise SystemExit(f"{label} changed before verification")
    digest = hashlib.file_digest(os.fdopen(fd, "rb", closefd=False), "sha256").hexdigest()
    after = os.fstat(fd)
    if identity != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        raise SystemExit(f"{label} changed during verification")
finally:
    os.close(fd)

if digest != expected_sha256:
    raise SystemExit(
        f"{label} SHA-256 drifted: "
        f"expected={expected_sha256} actual={digest}"
    )
PY
}

verify_pair() {
  verify_lock "${REVIEWED_LOCK}" "reviewed Kagemusha Cargo.lock artifact"
  verify_lock "${WORKSPACE_LOCK}" "workspace Kagemusha Cargo.lock"
  if ! cmp -s -- "${REVIEWED_LOCK}" "${WORKSPACE_LOCK}"; then
    echo "workspace Kagemusha Cargo.lock is not byte-identical to its reviewed artifact" >&2
    exit 1
  fi
}

publish_reviewed_lock() {
  python3 - "${REVIEWED_LOCK}" "${WORKSPACE_LOCK}" \
    "${EXPECTED_BYTES}" "${EXPECTED_SHA256}" <<'PY'
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import secrets
import stat
import sys

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
expected_bytes = int(sys.argv[3])
expected_sha256 = sys.argv[4]

flags = os.O_RDONLY
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
source_fd = os.open(source, flags)
temporary: Path | None = None
try:
    before = os.fstat(source_fd)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_mode & 0o111
        or before.st_size != expected_bytes
    ):
        raise SystemExit("reviewed Kagemusha Cargo.lock artifact became unsafe")
    chunks: list[bytes] = []
    remaining = expected_bytes + 1
    while remaining:
        chunk = os.read(source_fd, remaining)
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    payload = b"".join(chunks)
    after = os.fstat(source_fd)
    if (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        raise SystemExit(
            "reviewed Kagemusha Cargo.lock artifact changed during materialization"
        )
    if len(payload) != expected_bytes or hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise SystemExit(
            "reviewed Kagemusha Cargo.lock artifact changed during materialization"
        )

    temporary = destination.parent / (
        f".Cargo.lock.kagemusha.{os.getpid()}.{secrets.token_hex(8)}"
    )
    output_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        output_flags |= os.O_NOFOLLOW
    output_fd = os.open(temporary, output_flags, 0o644)
    try:
        view = memoryview(payload)
        while view:
            written = os.write(output_fd, view)
            if written <= 0:
                raise SystemExit("failed to materialize reviewed Kagemusha Cargo.lock")
            view = view[written:]
        os.fsync(output_fd)
    finally:
        os.close(output_fd)
    os.link(temporary, destination, follow_symlinks=False)
except FileExistsError:
    raise SystemExit(
        "workspace Cargo.lock appeared during reviewed-lock materialization; "
        "refusing to replace it"
    )
finally:
    os.close(source_fd)
    if temporary is not None:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
PY
}

case "${MODE}" in
  --materialize)
    verify_lock "${REVIEWED_LOCK}" "reviewed Kagemusha Cargo.lock artifact"
    if [[ -e "${WORKSPACE_LOCK}" || -L "${WORKSPACE_LOCK}" ]]; then
      verify_pair
    else
      publish_reviewed_lock
      verify_pair
    fi
    echo "reviewed Kagemusha Cargo.lock materialized and verified: ${EXPECTED_SHA256}"
    ;;
  --verify)
    verify_pair
    echo "reviewed Kagemusha Cargo.lock verified: ${EXPECTED_SHA256}"
    ;;
  --self-test)
    verify_pair
    temporary="$(mktemp -d "${TMPDIR:-/tmp}/kagemusha-cargo-lock.XXXXXX")"
    trap 'rm -rf -- "${temporary}"' EXIT

    mkdir -p -- "${temporary}/fixtures/kagemusha"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/artifact-missing.stdout" \
      2>"${temporary}/artifact-missing.stderr"; then
      echo "missing reviewed lock artifact negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "reviewed Kagemusha Cargo.lock artifact is missing" \
      "${temporary}/artifact-missing.stderr"

    cp -- "${REVIEWED_LOCK}" \
      "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/missing.stdout" 2>"${temporary}/missing.stderr"; then
      echo "missing Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "workspace Kagemusha Cargo.lock is missing" "${temporary}/missing.stderr"

    KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/materialize.stdout"
    cmp -s -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"

    python3 - "${temporary}/Cargo.lock" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
payload = bytearray(path.read_bytes())
payload[len(payload) // 2] ^= 1
path.write_bytes(payload)
PY
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/mutated.stdout" 2>"${temporary}/mutated.stderr"; then
      echo "mutated Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "workspace Kagemusha Cargo.lock SHA-256 drifted" \
      "${temporary}/mutated.stderr"

    rm -- "${temporary}/Cargo.lock"
    ln -s -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/symlink.stdout" 2>"${temporary}/symlink.stderr"; then
      echo "symlinked Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "must be a singly linked regular file" "${temporary}/symlink.stderr"

    rm -- "${temporary}/Cargo.lock"
    cp -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"
    ln -- "${temporary}/Cargo.lock" "${temporary}/Cargo.lock.alias"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/hardlink.stdout" 2>"${temporary}/hardlink.stderr"; then
      echo "hard-linked Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "must be a singly linked regular file" "${temporary}/hardlink.stderr"

    rm -- "${temporary}/Cargo.lock" "${temporary}/Cargo.lock.alias"
    cp -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"
    python3 - "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
payload = bytearray(path.read_bytes())
payload[len(payload) // 3] ^= 1
path.write_bytes(payload)
PY
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --verify \
      >"${temporary}/artifact-mutated.stdout" \
      2>"${temporary}/artifact-mutated.stderr"; then
      echo "mutated reviewed lock artifact negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "reviewed Kagemusha Cargo.lock artifact SHA-256 drifted" \
      "${temporary}/artifact-mutated.stderr"

    rm -- "${temporary}/Cargo.lock" \
      "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1"
    ln -s -- "${REVIEWED_LOCK}" \
      "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/artifact-symlink.stdout" \
      2>"${temporary}/artifact-symlink.stderr"; then
      echo "symlinked reviewed lock artifact negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "reviewed Kagemusha Cargo.lock artifact must be a singly linked regular file" \
      "${temporary}/artifact-symlink.stderr"

    rm -- "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1"
    cp -- "${REVIEWED_LOCK}" \
      "${temporary}/fixtures/kagemusha/cargo-lock.reviewed.v1"
    printf 'unsafe preexisting destination\n' >"${temporary}/Cargo.lock"
    before_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/clobber.stdout" 2>"${temporary}/clobber.stderr"; then
      echo "unsafe destination clobber negative control unexpectedly passed" >&2
      exit 1
    fi
    after_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if [[ "${before_unsafe}" != "${after_unsafe}" ]]; then
      echo "unsafe destination was modified during failed materialization" >&2
      exit 1
    fi
    grep -Fq "workspace Kagemusha Cargo.lock size drifted" \
      "${temporary}/clobber.stderr"

    before_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" \
      "$0" --self-test-publish-no-replace \
      >"${temporary}/publish-race.stdout" \
      2>"${temporary}/publish-race.stderr"; then
      echo "atomic destination collision negative control unexpectedly passed" >&2
      exit 1
    fi
    after_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if [[ "${before_unsafe}" != "${after_unsafe}" ]]; then
      echo "destination collision modified the preexisting Cargo.lock" >&2
      exit 1
    fi
    grep -Fq "appeared during reviewed-lock materialization" \
      "${temporary}/publish-race.stderr"

    echo "reviewed Kagemusha Cargo.lock negative controls passed: artifact/root missing, copy-step removal, root/artifact byte drift, root/artifact symlink, hard link, no-clobber destination, atomic publish collision"
    ;;
  --self-test-publish-no-replace)
    # Internal adversarial hook: exercise the atomic no-replace publication path
    # directly against a destination that appeared after the caller's absence check.
    verify_lock "${REVIEWED_LOCK}" "reviewed Kagemusha Cargo.lock artifact"
    publish_reviewed_lock
    ;;
  *)
    echo "usage: ci/check_kagemusha_reviewed_cargo_lock.sh [--materialize|--verify|--self-test]" >&2
    exit 2
    ;;
esac
