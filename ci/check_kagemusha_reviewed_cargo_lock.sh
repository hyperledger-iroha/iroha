#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:---verify}"
EXPECTED_BYTES=315548
EXPECTED_SHA256="ff773ee12a07de45d0e9df9ed29620142d884f365adb5e83d372e15dbedcd409"
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


def identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def validate_regular(metadata: os.stat_result) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_mode & 0o111
        or metadata.st_size != expected_bytes
    ):
        raise SystemExit(f"{label} became an unsafe or size-drifted file")

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

flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
fd = os.open(path, flags)
try:
    opened = os.fstat(fd)
    validate_regular(opened)
    descriptor_identity = identity(opened)
    path_identity = identity(before)
    # Windows obtains Path.stat and descriptor metadata through different APIs;
    # their inode/timestamp representations are not guaranteed to be identical.
    # POSIX keeps the stronger direct path-to-descriptor identity comparison.
    if os.name != "nt" and descriptor_identity != path_identity:
        raise SystemExit(f"{label} changed before verification")
    digest = hashlib.file_digest(os.fdopen(fd, "rb", closefd=False), "sha256").hexdigest()
    after = os.fstat(fd)
    if descriptor_identity != identity(after):
        raise SystemExit(f"{label} changed during verification")
finally:
    os.close(fd)

path_after = path.lstat()
if path_identity != identity(path_after):
    raise SystemExit(f"{label} changed during verification")

# On Windows, close the Path.stat/fstat representation gap with a second
# no-follow open and exact digest check while the path metadata stays fixed.
if os.name == "nt":
    second_fd = os.open(path, flags)
    try:
        second_opened = os.fstat(second_fd)
        validate_regular(second_opened)
        second_identity = identity(second_opened)
        second_digest = hashlib.file_digest(
            os.fdopen(second_fd, "rb", closefd=False), "sha256"
        ).hexdigest()
        if second_identity != identity(os.fstat(second_fd)):
            raise SystemExit(f"{label} changed during verification")
    finally:
        os.close(second_fd)
    if second_digest != digest or identity(path.lstat()) != path_identity:
        raise SystemExit(f"{label} changed during verification")

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

locks_are_byte_identical() {
  python3 - "${REVIEWED_LOCK}" "${WORKSPACE_LOCK}" "${EXPECTED_BYTES}" <<'PY'
from pathlib import Path
import sys

limit = int(sys.argv[3]) + 1
try:
    with Path(sys.argv[1]).open("rb") as reviewed_file:
        reviewed = reviewed_file.read(limit)
    with Path(sys.argv[2]).open("rb") as workspace_file:
        workspace = workspace_file.read(limit)
except OSError:
    raise SystemExit(1)
raise SystemExit(0 if reviewed == workspace else 1)
PY
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

flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
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
    output_flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_BINARY", 0)
    )
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

canonicalize_platform_lock() {
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


def descriptor_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def read_exact(fd: int, maximum: int) -> bytes:
    chunks: list[bytes] = []
    remaining = maximum + 1
    while remaining:
        chunk = os.read(fd, remaining)
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def open_no_follow(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return os.open(path, flags)


def require_safe_file(metadata: os.stat_result, size: int, links: int) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != links
        or metadata.st_mode & 0o111
        or metadata.st_size != size
    ):
        raise SystemExit(
            "platform-normalized workspace Cargo.lock must be a safe, singly "
            "linked regular file"
        )


try:
    source_path = source.lstat()
except FileNotFoundError:
    raise SystemExit("reviewed Kagemusha Cargo.lock artifact disappeared")
require_safe_file(source_path, expected_bytes, 1)
source_path_identity = descriptor_identity(source_path)

source_fd = open_no_follow(source)
try:
    source_before = os.fstat(source_fd)
    require_safe_file(source_before, expected_bytes, 1)
    if os.name != "nt" and descriptor_identity(source_before) != source_path_identity:
        raise SystemExit(
            "reviewed Kagemusha Cargo.lock artifact changed before platform "
            "canonicalization"
        )
    canonical = read_exact(source_fd, expected_bytes)
    if descriptor_identity(source_before) != descriptor_identity(os.fstat(source_fd)):
        raise SystemExit(
            "reviewed Kagemusha Cargo.lock artifact changed during "
            "platform canonicalization"
        )
finally:
    os.close(source_fd)
if descriptor_identity(source.lstat()) != source_path_identity:
    raise SystemExit(
        "reviewed Kagemusha Cargo.lock artifact changed during platform "
        "canonicalization"
    )
if len(canonical) != expected_bytes or hashlib.sha256(canonical).hexdigest() != expected_sha256:
    raise SystemExit(
        "reviewed Kagemusha Cargo.lock artifact changed during platform "
        "canonicalization"
    )

platform_payload = canonical.replace(b"\n", b"\r\n")
platform_bytes = len(platform_payload)

try:
    destination_path = destination.lstat()
except FileNotFoundError:
    raise SystemExit("platform-normalized workspace Cargo.lock disappeared")
require_safe_file(destination_path, platform_bytes, 1)
destination_path_identity = descriptor_identity(destination_path)

destination_fd = open_no_follow(destination)
try:
    destination_before = os.fstat(destination_fd)
    require_safe_file(destination_before, platform_bytes, 1)
    if (
        os.name != "nt"
        and descriptor_identity(destination_before) != destination_path_identity
    ):
        raise SystemExit(
            "platform-normalized workspace Cargo.lock changed before "
            "authentication"
        )
    payload = read_exact(destination_fd, platform_bytes)
    destination_after = os.fstat(destination_fd)
    if descriptor_identity(destination_before) != descriptor_identity(destination_after):
        raise SystemExit(
            "platform-normalized workspace Cargo.lock changed during "
            "authentication"
        )
finally:
    os.close(destination_fd)

if descriptor_identity(destination.lstat()) != destination_path_identity:
    raise SystemExit(
        "platform-normalized workspace Cargo.lock changed during authentication"
    )

if payload != platform_payload:
    raise SystemExit(
        "preexisting workspace Cargo.lock is not the exact authenticated CRLF "
        "derivative"
    )

# Quarantine through an exclusive hard link before unlinking the public path.
# Every subsequent operation reauthenticates the quarantined bytes; a path race
# therefore fails without publishing a canonical lock over an attacker file.
quarantine = destination.parent / (
    f".Cargo.lock.platform.{os.getpid()}.{secrets.token_hex(8)}"
)
quarantine_created = False
try:
    os.link(destination, quarantine, follow_symlinks=False)
    quarantine_created = True
    destination_linked = destination.lstat()
    quarantine_linked = quarantine.lstat()
    require_safe_file(destination_linked, platform_bytes, 2)
    require_safe_file(quarantine_linked, platform_bytes, 2)
    if not os.path.samefile(destination, quarantine):
        raise SystemExit(
            "platform-normalized workspace Cargo.lock changed before quarantine"
        )
    quarantine_fd = open_no_follow(quarantine)
    try:
        quarantine_before = os.fstat(quarantine_fd)
        require_safe_file(quarantine_before, platform_bytes, 2)
        quarantined_payload = read_exact(quarantine_fd, platform_bytes)
        if descriptor_identity(quarantine_before) != descriptor_identity(
            os.fstat(quarantine_fd)
        ):
            raise SystemExit(
                "platform-normalized workspace Cargo.lock changed during "
                "quarantine"
            )
    finally:
        os.close(quarantine_fd)
    if quarantined_payload != platform_payload or not os.path.samefile(
        destination, quarantine
    ):
        raise SystemExit(
            "platform-normalized workspace Cargo.lock failed quarantine "
            "authentication"
        )
    destination.unlink()
    require_safe_file(quarantine.lstat(), platform_bytes, 1)
    if destination.exists() or destination.is_symlink():
        raise SystemExit(
            "workspace Cargo.lock reappeared during platform canonicalization"
        )
finally:
    if quarantine_created:
        try:
            quarantine.unlink()
        except FileNotFoundError:
            pass
PY
}

case "${MODE}" in
  --materialize)
    verify_lock "${REVIEWED_LOCK}" "reviewed Kagemusha Cargo.lock artifact"
    if [[ -e "${WORKSPACE_LOCK}" || -L "${WORKSPACE_LOCK}" ]]; then
      # MSYS cmp can compare in text mode and treat LF and CRLF files as equal.
      # Native Python byte reads keep this security-sensitive branch binary.
      if locks_are_byte_identical; then
        verify_pair
      else
        canonicalize_platform_lock
        publish_reviewed_lock
        verify_pair
      fi
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

    rm -- "${temporary}/Cargo.lock"
    python3 - "${REVIEWED_LOCK}" "${temporary}/Cargo.lock" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_bytes()
Path(sys.argv[2]).write_bytes(source.replace(b"\n", b"\r\n"))
PY
    mkdir -p -- "${temporary}/text-mode-bin"
    printf '#!/usr/bin/env bash\nexit 0\n' >"${temporary}/text-mode-bin/cmp"
    chmod 0755 "${temporary}/text-mode-bin/cmp"
    PATH="${temporary}/text-mode-bin:${PATH}" \
      KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" \
      "$0" --materialize \
      >"${temporary}/platform-normalized.stdout"
    cmp -s -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"

    rm -- "${temporary}/Cargo.lock"
    python3 - "${REVIEWED_LOCK}" "${temporary}/Cargo.lock" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_bytes()
Path(sys.argv[2]).write_bytes(source.replace(b"\n", b"\r\n"))
PY
    ln -- "${temporary}/Cargo.lock" "${temporary}/Cargo.lock.alias"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/platform-hardlink.stdout" \
      2>"${temporary}/platform-hardlink.stderr"; then
      echo "hard-linked platform-normalized Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "platform-normalized workspace Cargo.lock must be a safe" \
      "${temporary}/platform-hardlink.stderr"

    rm -- "${temporary}/Cargo.lock" "${temporary}/Cargo.lock.alias"
    python3 - "${REVIEWED_LOCK}" "${temporary}/Cargo.lock.platform-target" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_bytes()
Path(sys.argv[2]).write_bytes(source.replace(b"\n", b"\r\n"))
PY
    ln -s -- "${temporary}/Cargo.lock.platform-target" "${temporary}/Cargo.lock"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/platform-symlink.stdout" \
      2>"${temporary}/platform-symlink.stderr"; then
      echo "symlinked platform-normalized Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    grep -Fq "platform-normalized workspace Cargo.lock must be a safe" \
      "${temporary}/platform-symlink.stderr"

    rm -- "${temporary}/Cargo.lock" "${temporary}/Cargo.lock.platform-target"

    python3 - "${REVIEWED_LOCK}" "${temporary}/Cargo.lock" <<'PY'
from pathlib import Path
import sys

payload = bytearray(Path(sys.argv[1]).read_bytes().replace(b"\n", b"\r\n"))
payload[len(payload) // 2] ^= 1
Path(sys.argv[2]).write_bytes(payload)
PY
    before_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if KAGEMUSHA_REVIEWED_CARGO_LOCK_ROOT="${temporary}" "$0" --materialize \
      >"${temporary}/platform-mutated.stdout" \
      2>"${temporary}/platform-mutated.stderr"; then
      echo "mutated platform-normalized Cargo.lock negative control unexpectedly passed" >&2
      exit 1
    fi
    after_unsafe="$(shasum -a 256 "${temporary}/Cargo.lock")"
    if [[ "${before_unsafe}" != "${after_unsafe}" ]]; then
      echo "mutated platform-normalized Cargo.lock was modified" >&2
      exit 1
    fi
    grep -Fq "is not the exact authenticated CRLF derivative" \
      "${temporary}/platform-mutated.stderr"

    rm -- "${temporary}/Cargo.lock"
    cp -- "${REVIEWED_LOCK}" "${temporary}/Cargo.lock"

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
    grep -Fq "platform-normalized workspace Cargo.lock must be a safe" \
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

    echo "reviewed Kagemusha Cargo.lock negative controls passed: artifact/root missing, exact CRLF canonicalization under a text-normalizing cmp, CRLF byte/link drift, copy-step removal, root/artifact byte drift, root/artifact symlink, hard link, no-clobber destination, atomic publish collision"
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
