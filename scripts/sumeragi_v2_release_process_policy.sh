#!/usr/bin/env bash
# Shared no-interference policy for Sumeragi v2 release and validation runners.
#
# This file must be sourced. It never observes, signals, reprioritizes, or
# otherwise controls an unrelated process. Cargo gates serialize only this
# invocation's Cargo calls with an owner-private directory lock below the
# authenticated external artifact root. Operator cancellation is a canonical
# owner-only marker checked between gates; it never interrupts an in-flight
# command.

if [[ "${SUMERAGI_V2_RELEASE_PROCESS_POLICY_LOADED:-0}" == 1 ]]; then
  return 0
fi
readonly SUMERAGI_V2_RELEASE_PROCESS_POLICY_LOADED=1

readonly SUMERAGI_V2_RELEASE_CANCELLED_STATUS=125
readonly SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS=2
SUMERAGI_V2_CARGO_LOCK_PATH=""
SUMERAGI_V2_CARGO_LOCK_TOKEN=""
SUMERAGI_V2_CARGO_LOCK_DEVICE=""
SUMERAGI_V2_CARGO_LOCK_INODE=""

_cargo_configuration_state() {
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"
  "$policy_python" -I -S - "${SUMERAGI_V2_CARGO_CONFIG_ROOT:-$PWD}" "${CARGO_HOME:-}" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import stat
import sys

root = Path(sys.argv[1])
cargo_home = Path(sys.argv[2]) if sys.argv[2] else None
if not root.is_absolute() or Path(os.path.abspath(root)) != root or root.resolve(strict=True) != root:
    raise SystemExit("Cargo configuration root is not absolute and canonical")
paths = []
for ancestor in (root, *root.parents):
    paths.extend((ancestor / ".cargo/config", ancestor / ".cargo/config.toml"))
if cargo_home is not None:
    if not cargo_home.is_absolute() or Path(os.path.abspath(cargo_home)) != cargo_home:
        raise SystemExit("CARGO_HOME is not absolute and normalized")
    paths.extend((cargo_home / "config", cargo_home / "config.toml"))
records = []
for path in paths:
    try:
        before = path.lstat()
    except FileNotFoundError:
        records.append({"path": str(path), "state": "absent"})
        continue
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid not in {0, os.geteuid()}
        or before.st_nlink != 1
        or before.st_mode & 0o022
        or before.st_size > 1024 * 1024
    ):
        raise SystemExit(f"Cargo configuration is unsafe: {path}")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        digest = hashlib.sha256()
        total = 0
        while block := os.read(descriptor, 1024 * 1024):
            digest.update(block)
            total += len(block)
            if total > 1024 * 1024:
                raise SystemExit(f"Cargo configuration is oversized: {path}")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    path_after = path.lstat()
    fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    if total != opened.st_size or any(getattr(before, field) != getattr(opened, field) or getattr(opened, field) != getattr(after, field) or getattr(after, field) != getattr(path_after, field) for field in fields):
        raise SystemExit(f"Cargo configuration changed while read: {path}")
    records.append({"path": str(path), "state": "file", "mode": format(stat.S_IMODE(opened.st_mode), "04o"), "sha256": digest.hexdigest(), "size": total})
print(hashlib.sha256((json.dumps(records, sort_keys=True, separators=(",", ":")) + "\n").encode()).hexdigest())
PY
}

SUMERAGI_V2_CARGO_CONFIG_ROOT="$PWD"
readonly SUMERAGI_V2_CARGO_CONFIG_ROOT
SUMERAGI_V2_CARGO_CONFIGURATION_STATE="$(_cargo_configuration_state)" || return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
readonly SUMERAGI_V2_CARGO_CONFIGURATION_STATE

_require_cargo_configuration_unchanged() {
  local observed
  observed="$(_cargo_configuration_state)" || return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  if [[ "$observed" != "$SUMERAGI_V2_CARGO_CONFIGURATION_STATE" ]]; then
    echo "Cargo configuration roots changed during the release invocation" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

release_gate_boundary() {
  local label="${1:-unnamed-gate}"
  local marker_path="${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"
  local boundary_exit_code

  if [[ -z "$marker_path" ]]; then
    return 0
  fi

  if "$policy_python" -I -S - "$marker_path" <<'PY'
import errno
import os
import stat
import sys

path = sys.argv[1]
expected = b'{"reason":"operator-request","schema_version":1}\n'

if not os.path.isabs(path):
    print("cancellation marker path is not absolute", file=sys.stderr)
    raise SystemExit(2)

parent = os.path.dirname(path)
try:
    parent_stat = os.stat(parent, follow_symlinks=False)
except OSError as error:
    print(f"cancellation marker parent is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    not stat.S_ISDIR(parent_stat.st_mode)
    or parent_stat.st_uid != os.getuid()
    or parent_stat.st_mode & 0o077
    or os.path.realpath(parent) != parent
):
    print(
        "cancellation marker parent is not one canonical private owner directory",
        file=sys.stderr,
    )
    raise SystemExit(2)

try:
    before = os.lstat(path)
except FileNotFoundError:
    raise SystemExit(0)
except OSError as error:
    print(f"cancellation marker cannot be inspected: {error}", file=sys.stderr)
    raise SystemExit(2) from error

if (
    not stat.S_ISREG(before.st_mode)
    or stat.S_ISLNK(before.st_mode)
    or before.st_nlink != 1
    or before.st_uid != os.getuid()
    or before.st_mode & 0o077
    or os.path.realpath(path) != path
):
    print("cancellation marker is not canonical, private, and owner-bound", file=sys.stderr)
    raise SystemExit(2)

flags = os.O_RDONLY
if hasattr(os, "O_CLOEXEC"):
    flags |= os.O_CLOEXEC
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
try:
    descriptor = os.open(path, flags)
except OSError as error:
    if error.errno == errno.ENOENT:
        raise SystemExit(0)
    print(f"cancellation marker cannot be opened safely: {error}", file=sys.stderr)
    raise SystemExit(2) from error
try:
    opened = os.fstat(descriptor)
    data = os.read(descriptor, len(expected) + 1)
    trailing = os.read(descriptor, 1)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)

identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
opened_identity = (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns)
after_identity = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
if identity != opened_identity or opened_identity != after_identity:
    print("cancellation marker changed while it was inspected", file=sys.stderr)
    raise SystemExit(2)
if data != expected or trailing:
    print("cancellation marker is not the exact canonical request", file=sys.stderr)
    raise SystemExit(2)
raise SystemExit(125)
PY
  then
    return 0
  else
    boundary_exit_code=$?
  fi

  if ((boundary_exit_code == SUMERAGI_V2_RELEASE_CANCELLED_STATUS)); then
    printf 'cooperative cancellation requested at gate boundary %s\n' "$label" >&2
    return "$SUMERAGI_V2_RELEASE_CANCELLED_STATUS"
  fi
  printf 'invalid cooperative cancellation marker at gate boundary %s\n' "$label" >&2
  return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
}

acquire_invocation_cargo_lock() {
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local lock_path
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$artifact_root" ]]; then
    echo "run_cargo requires an authenticated external artifact root" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  lock_path="${artifact_root}/.sumeragi-v2-cargo.lock"
  local lock_identity
  if ! lock_identity="$("$policy_python" -I -S - "$artifact_root" "$lock_path" <<'PY'
import os
from pathlib import Path
import secrets
import stat
import sys

root = Path(sys.argv[1])
lock = Path(sys.argv[2])
if not root.is_absolute() or not lock.is_absolute():
    raise SystemExit("Cargo lock root and path must be absolute")
try:
    root_metadata = root.lstat()
except OSError as error:
    raise SystemExit(f"Cargo lock root is unavailable: {error}") from error
if (
    root.resolve(strict=True) != root
    or stat.S_ISLNK(root_metadata.st_mode)
    or not stat.S_ISDIR(root_metadata.st_mode)
    or root_metadata.st_uid != os.getuid()
    or root_metadata.st_mode & 0o077
    or lock != root / ".sumeragi-v2-cargo.lock"
):
    raise SystemExit("Cargo lock path is not bound to the private artifact root")
try:
    lock.mkdir(mode=0o700)
except FileExistsError as error:
    raise SystemExit("invocation-local Cargo lock is already held") from error
except OSError as error:
    raise SystemExit(f"invocation-local Cargo lock cannot be created: {error}") from error
try:
    lock_metadata = lock.lstat()
    if (
        lock.resolve(strict=True) != lock
        or stat.S_ISLNK(lock_metadata.st_mode)
        or not stat.S_ISDIR(lock_metadata.st_mode)
        or lock_metadata.st_uid != os.getuid()
        or stat.S_IMODE(lock_metadata.st_mode) != 0o700
        or lock_metadata.st_nlink != 2
    ):
        raise RuntimeError("invocation-local Cargo lock has unsafe metadata")
    token = secrets.token_hex(32)
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    directory_flags |= getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    directory_fd = os.open(lock, directory_flags)
    try:
        opened = os.fstat(directory_fd)
        if (opened.st_dev, opened.st_ino) != (
            lock_metadata.st_dev,
            lock_metadata.st_ino,
        ):
            raise RuntimeError("invocation-local Cargo lock changed while opened")
        token_fd = os.open(
            "owner-token",
            os.O_WRONLY | os.O_CREAT | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
            0o400,
            dir_fd=directory_fd,
        )
        try:
            payload = (token + "\n").encode()
            if os.write(token_fd, payload) != len(payload):
                raise RuntimeError("invocation-local Cargo lock token write failed")
            os.fchmod(token_fd, 0o400)
            os.fsync(token_fd)
        finally:
            os.close(token_fd)
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    print(f"{lock_metadata.st_dev}\t{lock_metadata.st_ino}\t{token}")
except BaseException:
    try:
        (lock / "owner-token").unlink()
    except OSError:
        pass
    try:
        lock.rmdir()
    except OSError:
        pass
    raise
PY
  )"; then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  IFS=$'\t' read -r \
    SUMERAGI_V2_CARGO_LOCK_DEVICE SUMERAGI_V2_CARGO_LOCK_INODE \
    SUMERAGI_V2_CARGO_LOCK_TOKEN <<<"$lock_identity"
  SUMERAGI_V2_CARGO_LOCK_PATH="$lock_path"
  if [[ ! "$SUMERAGI_V2_CARGO_LOCK_DEVICE" =~ ^[0-9]+$ \
    || ! "$SUMERAGI_V2_CARGO_LOCK_INODE" =~ ^[0-9]+$ \
    || ! "$SUMERAGI_V2_CARGO_LOCK_TOKEN" =~ ^[0-9a-f]{64}$ ]]; then
    echo "invocation-local Cargo lock identity is malformed" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

release_invocation_cargo_lock() {
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local lock_path="${artifact_root}/.sumeragi-v2-cargo.lock"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ "$SUMERAGI_V2_CARGO_LOCK_PATH" != "$lock_path" \
    || ! "$SUMERAGI_V2_CARGO_LOCK_DEVICE" =~ ^[0-9]+$ \
    || ! "$SUMERAGI_V2_CARGO_LOCK_INODE" =~ ^[0-9]+$ \
    || ! "$SUMERAGI_V2_CARGO_LOCK_TOKEN" =~ ^[0-9a-f]{64}$ ]]; then
    echo "refusing to release a Cargo lock not owned by this shell" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - \
    "$artifact_root" "$lock_path" "$SUMERAGI_V2_CARGO_LOCK_DEVICE" \
    "$SUMERAGI_V2_CARGO_LOCK_INODE" "$SUMERAGI_V2_CARGO_LOCK_TOKEN" <<'PY'
import os
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
lock = Path(sys.argv[2])
expected_device, expected_inode = map(int, sys.argv[3:5])
expected_token = (sys.argv[5] + "\n").encode()
try:
    root_metadata = root.lstat()
    metadata = lock.lstat()
except OSError as error:
    raise SystemExit(f"invocation-local Cargo lock is unavailable: {error}") from error
if (
    root.resolve(strict=True) != root
    or stat.S_ISLNK(root_metadata.st_mode)
    or not stat.S_ISDIR(root_metadata.st_mode)
    or root_metadata.st_uid != os.getuid()
    or root_metadata.st_mode & 0o077
    or lock != root / ".sumeragi-v2-cargo.lock"
    or lock.resolve(strict=True) != lock
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or metadata.st_uid != os.getuid()
    or stat.S_IMODE(metadata.st_mode) != 0o700
    or (metadata.st_dev, metadata.st_ino) != (expected_device, expected_inode)
):
    raise SystemExit("refusing to release an unsafe invocation-local Cargo lock")
root_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
lock_flags = root_flags
if hasattr(os, "O_NOFOLLOW"):
    root_flags |= os.O_NOFOLLOW
    lock_flags |= os.O_NOFOLLOW
root_fd = os.open(root, root_flags)
try:
    lock_fd = os.open(lock.name, lock_flags, dir_fd=root_fd)
    try:
        opened = os.fstat(lock_fd)
        if (opened.st_dev, opened.st_ino) != (expected_device, expected_inode):
            raise RuntimeError("invocation-local Cargo lock inode changed")
        if os.listdir(lock_fd) != ["owner-token"]:
            raise RuntimeError("invocation-local Cargo lock inventory changed")
        token_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            token_flags |= os.O_NOFOLLOW
        token_fd = os.open("owner-token", token_flags, dir_fd=lock_fd)
        try:
            token_metadata = os.fstat(token_fd)
            token = os.read(token_fd, len(expected_token) + 1)
            if (
                not stat.S_ISREG(token_metadata.st_mode)
                or token_metadata.st_uid != os.getuid()
                or token_metadata.st_nlink != 1
                or stat.S_IMODE(token_metadata.st_mode) != 0o400
                or token != expected_token
            ):
                raise RuntimeError("invocation-local Cargo lock token changed")
        finally:
            os.close(token_fd)
        os.unlink("owner-token", dir_fd=lock_fd)
        os.fsync(lock_fd)
    finally:
        os.close(lock_fd)
    os.rmdir(lock.name, dir_fd=root_fd)
    os.fsync(root_fd)
finally:
    os.close(root_fd)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  SUMERAGI_V2_CARGO_LOCK_PATH=""
  SUMERAGI_V2_CARGO_LOCK_TOKEN=""
  SUMERAGI_V2_CARGO_LOCK_DEVICE=""
  SUMERAGI_V2_CARGO_LOCK_INODE=""
}

_release_scoped_invocation_cargo_lock() {
  local cargo_scope_status=$?
  trap - RETURN EXIT
  if [[ -n "$SUMERAGI_V2_CARGO_LOCK_TOKEN" ]] \
    && ! release_invocation_cargo_lock; then
    cargo_scope_status="$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  return "$cargo_scope_status"
}

_run_cargo_with_scoped_lock() {
  local label="$1"
  shift
  local cargo_exit_code
  _require_cargo_configuration_unchanged || return $?
  acquire_invocation_cargo_lock || return $?
  trap _release_scoped_invocation_cargo_lock RETURN EXIT
  release_gate_boundary "${label}:before-exec" || return $?
  _require_cargo_configuration_unchanged || return $?
  if "$IROHA_RELEASE_CARGO_BIN" "$@"; then
    cargo_exit_code=0
  else
    cargo_exit_code=$?
  fi
  _require_cargo_configuration_unchanged || return $?
  release_invocation_cargo_lock || return $?
  trap - RETURN EXIT
  release_gate_boundary "${label}:after-natural-completion" || return $?
  return "$cargo_exit_code"
}

require_external_private_directory() {
  local source_root="${1:-}"
  local external_root="${2:-}"
  local purpose="${3:-release output}"
  local temp_base="${IROHA_RELEASE_TEMP_BASE:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$source_root" || -z "$external_root" ]]; then
    printf 'source root and %s directory are required\n' "$purpose" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$source_root" "$external_root" "$purpose" "$temp_base" <<'PY'
import os
import stat
import sys

source_root, external_root, purpose, configured_base = sys.argv[1:]
if not os.path.isabs(source_root) or not os.path.isabs(external_root):
    print(f"source and {purpose} roots must be absolute", file=sys.stderr)
    raise SystemExit(2)
source = os.path.realpath(source_root)
external = os.path.realpath(external_root)
if not configured_base:
    configured_base = "/private/tmp" if os.path.isdir("/private/tmp") else "/tmp"
if not os.path.isabs(configured_base) or os.path.abspath(configured_base) != configured_base:
    print("release temporary base must be absolute and normalized", file=sys.stderr)
    raise SystemExit(2)
temp_base = os.path.realpath(configured_base)
try:
    external_lstat = os.lstat(external_root)
    base_lstat = os.lstat(configured_base)
except OSError as error:
    print(f"{purpose} root is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    external != external_root
    or stat.S_ISLNK(external_lstat.st_mode)
    or not stat.S_ISDIR(external_lstat.st_mode)
    or external_lstat.st_uid != os.getuid()
    or external_lstat.st_mode & 0o077
):
    print(f"{purpose} root is not one canonical private owner directory", file=sys.stderr)
    raise SystemExit(2)
if (
    temp_base != configured_base
    or stat.S_ISLNK(base_lstat.st_mode)
    or not stat.S_ISDIR(base_lstat.st_mode)
    or not (
        (base_lstat.st_uid == 0 and base_lstat.st_mode & stat.S_ISVTX)
        or (base_lstat.st_uid == os.getuid() and not base_lstat.st_mode & 0o077)
    )
):
    print("release temporary base is not authenticated and private", file=sys.stderr)
    raise SystemExit(2)
try:
    external_under_temp_base = os.path.commonpath((external, temp_base)) == temp_base
    external_under_source = os.path.commonpath((external, source)) == source
except ValueError:
    external_under_temp_base = False
    external_under_source = True
if (
    external == temp_base
    or not external_under_temp_base
    or external_under_source
):
    print(
        f"{purpose} root must be a dedicated authenticated temporary directory outside source",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_external_cargo_target_dir() {
  require_external_private_directory \
    "${1:-}" "${CARGO_TARGET_DIR:-}" "Cargo target"
}

require_external_release_artifact_root() {
  require_external_private_directory \
    "${1:-}" "${IROHA_RELEASE_ARTIFACT_ROOT:-}" "release artifact"
}

require_disjoint_release_roots() {
  local source_root="${1:-}"
  local cargo_root="${CARGO_TARGET_DIR:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local cancel_path="${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}"
  local cancel_parent
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$source_root" || -z "$cargo_root" || -z "$artifact_root" \
    || -z "$cancel_path" ]]; then
    echo "source, Cargo target, release artifact, and cancellation paths are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  cancel_parent="${cancel_path%/*}"
  [[ -n "$cancel_parent" ]] || cancel_parent=/
  require_external_private_directory \
    "$source_root" "$cancel_parent" "release cancellation parent" || return $?
  if ! "$policy_python" -I -S - \
    "$source_root" "$cargo_root" "$artifact_root" "$cancel_path" <<'PY'
import os
import sys

source_root, cargo_root, artifact_root, cancel_path = sys.argv[1:]
if not all(map(os.path.isabs, (source_root, cargo_root, artifact_root, cancel_path))):
    print("release source/output/cancellation paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
if os.path.abspath(cancel_path) != cancel_path or os.path.realpath(cancel_path) != cancel_path:
    print("release cancellation marker path must be normalized and canonical", file=sys.stderr)
    raise SystemExit(2)
source_root, cargo_root, artifact_root = map(
    os.path.realpath, (source_root, cargo_root, artifact_root)
)


def overlap(left, right):
    try:
        common = os.path.commonpath((left, right))
    except ValueError:
        return False
    return common == left or common == right


try:
    target_artifact_overlap = overlap(cargo_root, artifact_root)
    cancel_source_overlap = overlap(cancel_path, source_root)
    cancel_target_overlap = overlap(cancel_path, cargo_root)
    cancel_artifact_overlap = overlap(cancel_path, artifact_root)
except OSError as error:
    print(f"release root disjointness could not be checked: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if target_artifact_overlap:
    print(
        "Cargo target and release artifact roots must be disjoint",
        file=sys.stderr,
    )
    raise SystemExit(2)
if cancel_source_overlap or cancel_target_overlap or cancel_artifact_overlap:
    print(
        "release cancellation marker must be outside source, Cargo target, and "
        "release artifact roots",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_release_artifact_path() {
  local candidate="${1:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$candidate" || -z "$artifact_root" ]]; then
    echo "release artifact path and root are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$artifact_root" "$candidate" <<'PY'
import os
import stat
import sys

artifact_root, candidate = sys.argv[1:]
if not os.path.isabs(artifact_root) or not os.path.isabs(candidate):
    print("release artifact paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
if os.path.abspath(candidate) != candidate:
    print("release artifact path must be normalized", file=sys.stderr)
    raise SystemExit(2)
root = os.path.realpath(artifact_root)
try:
    contained = os.path.commonpath((candidate, root)) == root
except ValueError:
    contained = False
if not contained:
    print("release artifact path escapes its authenticated root", file=sys.stderr)
    raise SystemExit(2)

current = root
relative = os.path.relpath(candidate, root)
for component in () if relative == "." else relative.split(os.sep):
    if component in {"", ".", ".."}:
        print("release artifact path contains an unsafe component", file=sys.stderr)
        raise SystemExit(2)
    current = os.path.join(current, component)
    try:
        observed = os.lstat(current)
    except FileNotFoundError:
        break
    except OSError as error:
        print(f"release artifact path is unavailable: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    if stat.S_ISLNK(observed.st_mode) or not stat.S_ISDIR(observed.st_mode):
        print("release artifact path crosses a non-directory or symlink", file=sys.stderr)
        raise SystemExit(2)
    if observed.st_uid != os.getuid():
        print("release artifact path is not owner-bound", file=sys.stderr)
        raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_release_artifact_directory() {
  local directory="${1:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$directory" || -z "$artifact_root" ]]; then
    echo "release artifact directory and root are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$artifact_root" "$directory" <<'PY'
import os
import stat
import sys

artifact_root, directory = sys.argv[1:]
if not os.path.isabs(artifact_root) or not os.path.isabs(directory):
    print("release artifact paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
root = os.path.realpath(artifact_root)
resolved = os.path.realpath(directory)
try:
    observed = os.lstat(directory)
except OSError as error:
    print(f"release artifact directory is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    resolved != directory
    or stat.S_ISLNK(observed.st_mode)
    or not stat.S_ISDIR(observed.st_mode)
    or observed.st_uid != os.getuid()
):
    print("release artifact directory is not canonical and owner-bound", file=sys.stderr)
    raise SystemExit(2)
try:
    contained = os.path.commonpath((resolved, root)) == root
except ValueError:
    contained = False
if not contained:
    print("release artifact directory escapes its authenticated root", file=sys.stderr)
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

run_cargo() {
  local subcommand="${1:-}"
  local label="cargo-${subcommand:-missing-subcommand}"
  local argument
  local cargo_prefix=1
  local skip_subcommand=1
  local requires_locked_offline=0
  local locked_count=0
  local offline_count=0
  local cargo_exit_code
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"
  local -a pinned_arguments

  if [[ -z "$subcommand" ]]; then
    echo "run_cargo requires one Cargo subcommand" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if [[ "${RUSTUP_AUTO_INSTALL+x}" == x \
    && "${RUSTUP_AUTO_INSTALL:-}" != 0 ]]; then
    echo "run_cargo forbids caller-owned rustup auto-install policy" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi

  case "$subcommand" in
    --version|version)
      if (($# != 1)); then
        echo "the pinned Cargo version probe accepts no additional arguments" >&2
        return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
      fi
      ;;
    fmt)
      ;;
    build|test|run|clippy|verus)
      requires_locked_offline=1
      ;;
    *)
      printf 'run_cargo rejects unsupported Cargo subcommand: %s\n' "$subcommand" >&2
      return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
      ;;
  esac

  # Cargo stops interpreting options at the first literal `--`. Test-harness
  # and program arguments after that separator must neither satisfy Cargo's
  # locked/offline policy nor be mistaken for caller-owned Cargo controls.
  for argument in "$@"; do
    if ((skip_subcommand)); then
      skip_subcommand=0
      continue
    fi
    if ((cargo_prefix)) && [[ "$argument" == "--" ]]; then
      cargo_prefix=0
      continue
    fi
    if ((!cargo_prefix)); then
      continue
    fi
    case "$argument" in
      --target-dir|--target-dir=*|--manifest-path|--manifest-path=*|--config|--config=*)
        echo "run_cargo owns Cargo target, manifest, and configuration selection" >&2
        return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
        ;;
      -j*|--jobs|--jobs=*)
        echo "run_cargo owns the one global -j1 flag; caller job flags are forbidden" >&2
        return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
        ;;
      --locked)
        ((locked_count += 1))
        ;;
      --offline)
        ((offline_count += 1))
        ;;
    esac
  done
  if ((requires_locked_offline \
    && (locked_count != 1 || offline_count != 1))); then
    echo "run_cargo requires exactly one --locked and one --offline flag before --" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi

  if [[ -z "${IROHA_RELEASE_CARGO_BIN:-}" ]]; then
    echo "run_cargo requires IROHA_RELEASE_CARGO_BIN" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$IROHA_RELEASE_CARGO_BIN" <<'PY'
import os
import stat
import sys

path = sys.argv[1]
if not os.path.isabs(path) or os.path.abspath(path) != path:
    print("IROHA_RELEASE_CARGO_BIN must be absolute and normalized", file=sys.stderr)
    raise SystemExit(2)
try:
    metadata = os.lstat(path)
except OSError as error:
    print(f"IROHA_RELEASE_CARGO_BIN is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    os.path.realpath(path) != path
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISREG(metadata.st_mode)
    or not os.access(path, os.X_OK)
):
    print(
        "IROHA_RELEASE_CARGO_BIN must be one canonical executable regular file",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi

  release_gate_boundary "${label}:before-lock" || return $?
  if [[ "$subcommand" == "--version" || "$subcommand" == "version" \
    || "$subcommand" == "fmt" ]]; then
    pinned_arguments=("$@")
  else
    pinned_arguments=("$subcommand" -j1)
    shift
    pinned_arguments+=("$@")
  fi
  local RUSTUP_AUTO_INSTALL=0
  local CARGO="$IROHA_RELEASE_CARGO_BIN"
  export RUSTUP_AUTO_INSTALL
  export CARGO
  ( _run_cargo_with_scoped_lock "$label" "${pinned_arguments[@]}" )
}
