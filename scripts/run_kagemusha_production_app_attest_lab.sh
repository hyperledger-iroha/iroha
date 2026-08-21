#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage:
  # Phase 1: build, sign, verify, and retain the exact capture app.
  scripts/run_kagemusha_production_app_attest_lab.sh \
    --prepare-only \
    --device-id <paired-CoreDevice-identifier> \
    --development-team <Apple-team-id> \
    --bundle-id org.hyperledger.iroha.kagemusha.appattestlab \
    --prepared-root /absolute/new/private/path

  # Phase 2: install that exact app with an authority-issued request.
  scripts/run_kagemusha_production_app_attest_lab.sh \
    --device-id <paired-CoreDevice-identifier> \
    --development-team <Apple-team-id> \
    --bundle-id org.hyperledger.iroha.kagemusha.appattestlab \
    --prepared-root /absolute/existing/private/path \
    --evidence-root /absolute/new/private/path \
    --request /absolute/canonical/capture-request-v1.json \
    --production-policy /absolute/canonical/production-policy-v1.json

Omitting both --prepare-only and --prepared-root performs a one-shot,
artifact-independent qualification run. It cannot consume a production request.

The checked-in application entitlement is fixed to the production App Attest
environment. The device selector and signing team remain runtime-only. The
retained evidence contains no device identifier, provisioning profile, or
signing credential. The owner-private prepared root contains the exact signed
app only until the release-bound capture completes.
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
LAB_ROOT="$ROOT_DIR/IrohaSwift/KagemushaProductionAppAttestLab"
DEVICE_ID=""
DEVELOPMENT_TEAM=""
BUNDLE_ID=""
PRODUCTION_BUNDLE_ID="org.hyperledger.iroha.kagemusha.appattestlab"
EVIDENCE_ROOT=""
REQUEST=""
PRODUCTION_POLICY=""
PREPARED_ROOT=""
PREPARE_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device-id)
      DEVICE_ID="${2:-}"
      shift 2
      ;;
    --development-team)
      DEVELOPMENT_TEAM="${2:-}"
      shift 2
      ;;
    --bundle-id)
      BUNDLE_ID="${2:-}"
      shift 2
      ;;
    --evidence-root)
      EVIDENCE_ROOT="${2:-}"
      shift 2
      ;;
    --request)
      REQUEST="${2:-}"
      shift 2
      ;;
    --production-policy)
      PRODUCTION_POLICY="${2:-}"
      shift 2
      ;;
    --prepared-root)
      PREPARED_ROOT="${2:-}"
      shift 2
      ;;
    --prepare-only)
      PREPARE_ONLY=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[kagemusha-app-attest-lab] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

if [[ -z "$DEVICE_ID" || -z "$DEVELOPMENT_TEAM" || -z "$BUNDLE_ID" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: device, team, and bundle id are required" >&2
  exit 64
fi
if [[ ! "$DEVELOPMENT_TEAM" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: development team must be an exact 10-character ID" >&2
  exit 64
fi
if [[ ! "$BUNDLE_ID" =~ ^[A-Za-z0-9][A-Za-z0-9.-]{0,254}$ ]] \
  || [[ "$BUNDLE_ID" == *..* || "$BUNDLE_ID" == *. ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: bundle id is not canonical" >&2
  exit 64
fi
if [[ "$BUNDLE_ID" != "$PRODUCTION_BUNDLE_ID" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: bundle id must equal the reviewed production App ID $PRODUCTION_BUNDLE_ID" >&2
  exit 64
fi
if $PREPARE_ONLY; then
  if [[ -z "$PREPARED_ROOT" || -n "$EVIDENCE_ROOT" || -n "$REQUEST" \
    || -n "$PRODUCTION_POLICY" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: prepare-only requires only a new prepared root" >&2
    exit 64
  fi
else
  if [[ -z "$EVIDENCE_ROOT" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: evidence root is required for capture" >&2
    exit 64
  fi
  if [[ -n "$REQUEST" && -z "$PREPARED_ROOT" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: a production request requires an independently prepared signed app" >&2
    exit 64
  fi
  if [[ -n "$REQUEST" && -z "$PRODUCTION_POLICY" ]] \
    || [[ -z "$REQUEST" && -n "$PRODUCTION_POLICY" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: production request and policy must be supplied together" >&2
    exit 64
  fi
fi
for output_root in "$EVIDENCE_ROOT" "$PREPARED_ROOT"; do
  [[ -n "$output_root" ]] || continue
  if [[ "$output_root" != /* ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: retained roots must be absolute" >&2
    exit 64
  fi
  if [[ "$output_root" == "$ROOT_DIR" || "$output_root" == "$ROOT_DIR"/* ]] \
    || [[ "$ROOT_DIR" == "$output_root"/* ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: retained roots must be disjoint from the repository" >&2
    exit 64
  fi
done
if [[ -n "$REQUEST" ]]; then
  if [[ "$REQUEST" != /* || ! -f "$REQUEST" || -L "$REQUEST" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: request must be an absolute non-symlink file" >&2
    exit 66
  fi
  if [[ "$PRODUCTION_POLICY" != /* || ! -f "$PRODUCTION_POLICY" || -L "$PRODUCTION_POLICY" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: production policy must be an absolute non-symlink file" >&2
    exit 66
  fi
fi

PYTHON3_BINARY="${KAGEMUSHA_PRODUCTION_APP_ATTEST_PYTHON:-$(command -v python3 2>/dev/null || true)}"
XCODEGEN_BINARY="$(command -v xcodegen 2>/dev/null || true)"
XCODEBUILD_BINARY="/usr/bin/xcodebuild"
XCRUN_BINARY="/usr/bin/xcrun"
CODESIGN_BINARY="/usr/bin/codesign"
PLUTIL_BINARY="/usr/bin/plutil"
DITTO_BINARY="/usr/bin/ditto"
for tool in \
  "$PYTHON3_BINARY" "$XCODEGEN_BINARY" "$XCODEBUILD_BINARY" \
  "$XCRUN_BINARY" "$CODESIGN_BINARY" "$PLUTIL_BINARY" "$DITTO_BINARY"
do
  if [[ ! -x "$tool" ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: required executable is unavailable: $tool" >&2
    exit 69
  fi
done
if [[ "$($XCODEGEN_BINARY --version)" != "Version: 2.46.0" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: exact XcodeGen 2.46.0 is required" >&2
  exit 69
fi
if ! "$PYTHON3_BINARY" -I -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo "[kagemusha-app-attest-lab] ERROR: Python 3.10 or newer is required" >&2
  exit 69
fi

TRANSIENT_PARENT="$($PYTHON3_BINARY -I - "${TMPDIR:-/tmp}" <<'PY'
from pathlib import Path
import os
import re
import stat
import subprocess
import sys

raw = Path(sys.argv[1])
try:
    parent = Path(os.path.realpath(raw, strict=True))
except (FileNotFoundError, OSError) as error:
    raise SystemExit(f"private transient parent is unavailable: {error}")
effective_uid = os.geteuid()
current = Path(parent.anchor)
ancestry = [current]
for part in parent.parts[1:]:
    current /= part
    ancestry.append(current)
for current in ancestry:
    metadata = current.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(f"private transient ancestry is not a real directory: {current}")
    if metadata.st_uid not in (0, effective_uid):
        raise SystemExit(f"private transient ancestry has a foreign owner: {current}")
    if stat.S_IMODE(metadata.st_mode) & 0o022:
        raise SystemExit(f"private transient ancestry is group/world writable: {current}")
    if sys.platform == "darwin":
        inspected = subprocess.run(
            ["/bin/ls", "-lde", str(current)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            text=True,
            encoding="utf-8",
        )
        lines = inspected.stdout.splitlines()
        permission = lines[0].split(maxsplit=1)[0] if lines else ""
        if inspected.returncode != 0 or permission.endswith("+") or any(
            re.match(r"^\s*\d+:\s", line) for line in lines[1:]
        ):
            raise SystemExit(f"private transient ancestry has an extended ACL: {current}")
metadata = parent.lstat()
if metadata.st_uid != effective_uid or stat.S_IMODE(metadata.st_mode) != 0o700:
    raise SystemExit("canonical transient parent must be euid-owned with exact mode 0700")
print(parent)
PY
)"
TRANSIENT="$(mktemp -d "$TRANSIENT_PARENT/kagemusha-production-app-attest.XXXXXX")"
chmod 0700 "$TRANSIENT"
PREPARED_STAGE=""
cleanup() {
  if [[ -n "${PREPARED_STAGE:-}" && -n "${PREPARED_PARENT:-}" \
    && "$PREPARED_STAGE" == "$PREPARED_PARENT"/.kagemusha-app-attest-prepared.* \
    && -d "$PREPARED_STAGE" \
    && -n "${PREPARED_STAGE_ROOT_SNAPSHOT:-}" ]] \
    && custody revalidate-root "$PREPARED_STAGE" "$PREPARED_STAGE_ROOT_SNAPSHOT" >/dev/null 2>&1; then
    find "$PREPARED_STAGE" -depth -delete
  fi
  if [[ -n "${TRANSIENT:-}" && "$TRANSIENT" == "$TRANSIENT_PARENT"/kagemusha-production-app-attest.* \
    && -d "$TRANSIENT" \
    && -n "${TRANSIENT_PARENT_SNAPSHOT:-}" && -n "${TRANSIENT_ROOT_SNAPSHOT:-}" ]] \
    && custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing >/dev/null 2>&1 \
    && custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT" >/dev/null 2>&1; then
    find "$TRANSIENT" -depth -delete
  fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# This helper is deliberately dependency-free and runs isolated.  The release
# runner uses it to hold filesystem identity across every trust-boundary read,
# install, capture, and publication operation.
custody() {
  "$PYTHON3_BINARY" -I - "$@" <<'PY'
# KAGEMUSHA_CUSTODY_HELPER_BEGIN
from __future__ import annotations

import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
from typing import Any


def fail(message: str) -> "NoReturn":
    raise SystemExit("filesystem custody violation: " + message)


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii") + b"\n"


def identity(metadata: os.stat_result) -> dict[str, int]:
    return {
        "device": metadata.st_dev,
        "inode": metadata.st_ino,
        "mode": stat.S_IMODE(metadata.st_mode),
        "uid": metadata.st_uid,
        "gid": metadata.st_gid,
    }


def exact_metadata(metadata: os.stat_result) -> dict[str, int]:
    value = identity(metadata)
    value.update(
        {
            "nlink": metadata.st_nlink,
            "size": metadata.st_size,
            "mtime_ns": metadata.st_mtime_ns,
            "ctime_ns": metadata.st_ctime_ns,
        }
    )
    return value


def reject_acl(path: Path) -> None:
    if sys.platform != "darwin":
        return
    try:
        result = subprocess.run(
            ["/bin/ls", "-lde", str(path)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            text=True,
            encoding="utf-8",
        )
    except OSError as error:
        fail(f"cannot inspect the extended ACL on {path}: {error}")
    if result.returncode != 0:
        fail(f"cannot inspect the extended ACL on {path}")
    lines = result.stdout.splitlines()
    permission = lines[0].split(maxsplit=1)[0] if lines else ""
    if permission.endswith("+") or any(
        re.match(r"^\s*\d+:\s", line) for line in lines[1:]
    ):
        fail(f"extended ACLs are forbidden on {path}")


def require_absolute_normal(path: Path) -> None:
    raw = str(path)
    if not path.is_absolute() or os.path.normpath(raw) != raw:
        fail(f"path is not canonical and absolute: {path}")


def existing_ancestry(path: Path) -> None:
    require_absolute_normal(path)
    effective_uid = os.geteuid()
    current = Path(path.anchor)
    ancestry = [current]
    for part in path.parts[1:]:
        current /= part
        ancestry.append(current)
    for current in ancestry:
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            fail(f"ancestor does not exist: {current}")
        if stat.S_ISLNK(metadata.st_mode):
            fail(f"symbolic-link ancestor is forbidden: {current}")
        if not stat.S_ISDIR(metadata.st_mode):
            fail(f"ancestor is not a directory: {current}")
        if metadata.st_uid not in (0, effective_uid):
            fail(f"ancestor is not owned by root or the effective uid: {current}")
        if stat.S_IMODE(metadata.st_mode) & 0o022:
            fail(f"ancestor is group/world writable: {current}")
        reject_acl(current)


def require_private_directory(path: Path) -> os.stat_result:
    existing_ancestry(path)
    metadata = path.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        fail(f"private custody path is not a real directory: {path}")
    if metadata.st_uid != os.geteuid():
        fail(f"private custody path is not owned by the effective uid: {path}")
    if stat.S_IMODE(metadata.st_mode) != 0o700:
        fail(f"private custody path must have exact mode 0700: {path}")
    reject_acl(path)
    return metadata


def read_regular(path: Path, *, maximum: int | None = None) -> tuple[bytes, dict[str, Any]]:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open regular file without following links: {path}: {error}")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            fail(f"file is not a singly-linked regular file: {path}")
        if before.st_uid != os.geteuid():
            fail(f"file is not owned by the effective uid: {path}")
        if stat.S_IMODE(before.st_mode) & 0o022:
            fail(f"file is group/world writable: {path}")
        if maximum is not None and before.st_size > maximum:
            fail(f"file exceeds its custody size bound: {path}")
        reject_acl(path)
        chunks: list[bytes] = []
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if maximum is not None and total > maximum:
                fail(f"file exceeds its custody size bound: {path}")
            chunks.append(chunk)
            digest.update(chunk)
        after = os.fstat(descriptor)
        try:
            current = path.lstat()
        except FileNotFoundError:
            fail(f"file disappeared while it was read: {path}")
        if exact_metadata(before) != exact_metadata(after) or identity(after) != identity(current):
            fail(f"file identity changed while it was read: {path}")
        value: dict[str, Any] = exact_metadata(after)
        value["sha256"] = digest.hexdigest()
        return b"".join(chunks), value
    finally:
        os.close(descriptor)


def path_record(path: Path, relative: str) -> dict[str, Any]:
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode):
        fail(f"symbolic links are forbidden in the prepared app tree: {path}")
    if metadata.st_uid != os.geteuid():
        fail(f"prepared app tree entry is not owned by the effective uid: {path}")
    if stat.S_IMODE(metadata.st_mode) & 0o022:
        fail(f"prepared app tree entry is group/world writable: {path}")
    reject_acl(path)
    record: dict[str, Any] = {"path": relative, **exact_metadata(metadata)}
    if stat.S_ISDIR(metadata.st_mode):
        record["kind"] = "directory"
    elif stat.S_ISREG(metadata.st_mode):
        _, file_metadata = read_regular(path)
        record.update(file_metadata)
        record["kind"] = "file"
    else:
        fail(f"special files are forbidden in the prepared app tree: {path}")
    return record


def tree_once(root: Path, *, prepared_layout: bool = False) -> dict[str, Any]:
    if prepared_layout:
        root_metadata = require_private_directory(root)
    else:
        existing_ancestry(root)
        root_metadata = root.lstat()
    if prepared_layout:
        expected = {
            "KagemushaProductionAppAttestLab.app",
            "capture-app-code-sign-measurements-v1.json",
        }
        try:
            observed = {entry.name for entry in os.scandir(root)}
        except OSError as error:
            fail(f"cannot enumerate prepared root: {error}")
        if observed != expected:
            fail("prepared root contains fields outside the exact reviewed layout")
    records = [path_record(root, ".")]
    pending = [root]
    while pending:
        directory = pending.pop()
        try:
            children = sorted(os.scandir(directory), key=lambda item: item.name)
        except OSError as error:
            fail(f"cannot enumerate prepared app tree: {directory}: {error}")
        for child in children:
            path = Path(child.path)
            relative = path.relative_to(root).as_posix()
            record = path_record(path, relative)
            records.append(record)
            if record["kind"] == "directory":
                pending.append(path)
    current_root = root.lstat()
    if identity(root_metadata) != identity(current_root):
        fail("prepared root identity changed while its tree was snapshotted")
    return {"schema": "iroha.kagemusha.filesystem_custody_tree.v1", "entries": records}


def stable_tree(root: Path, *, prepared_layout: bool = False) -> dict[str, Any]:
    first = tree_once(root, prepared_layout=prepared_layout)
    second = tree_once(root, prepared_layout=prepared_layout)
    if first != second:
        fail(f"tree changed while its custody snapshot was taken: {root}")
    return second


def write_new(path: Path, payload: bytes, mode: int = 0o600) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, mode)
    except OSError as error:
        fail(f"refusing to overwrite or follow publication path {path}: {error}")
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                fail(f"short write while publishing {path}")
            view = view[written:]
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            fail(f"published path is not a singly-linked regular file: {path}")
        if metadata.st_uid != os.geteuid():
            fail(f"published path is not owned by the effective uid: {path}")
    finally:
        os.close(descriptor)
    parent = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(parent)
    finally:
        os.close(parent)


def write_snapshot(path: Path, value: Any) -> None:
    write_new(path, canonical_json(value))


def load_snapshot(path: Path) -> Any:
    payload, _ = read_regular(path, maximum=16 * 1024 * 1024)
    try:
        return json.loads(payload.decode("ascii"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"custody snapshot is invalid: {path}: {error}")


def normalize_target(raw: str, expected: str) -> Path:
    path = Path(raw)
    if not path.is_absolute() or path.name in ("", ".", ".."):
        fail("retained root must be an absolute child path")
    if expected == "absent":
        if os.path.lexists(path):
            fail(f"refusing to overwrite retained root: {path}")
        try:
            parent = Path(os.path.realpath(path.parent, strict=True))
        except (FileNotFoundError, OSError) as error:
            fail(f"retained parent does not exist: {error}")
        result = parent / path.name
    elif expected == "existing":
        try:
            leaf = path.lstat()
        except (FileNotFoundError, OSError) as error:
            fail(f"prepared root does not exist: {error}")
        if stat.S_ISLNK(leaf.st_mode):
            fail("prepared root may not be a symbolic link")
        result = Path(os.path.realpath(path, strict=True))
    else:
        fail("internal normalize-target mode is invalid")
    require_absolute_normal(result)
    return result


def parent_snapshot(target: Path, expected: str) -> dict[str, Any]:
    require_absolute_normal(target)
    if expected == "absent" and os.path.lexists(target):
        fail(f"retained root appeared before use: {target}")
    if expected == "existing" and not os.path.lexists(target):
        fail(f"prepared root disappeared before use: {target}")
    metadata = require_private_directory(target.parent)
    return {
        "schema": "iroha.kagemusha.filesystem_custody_parent.v1",
        "target": str(target),
        "identity": identity(metadata),
    }


def verify_parent(target: Path, snapshot_path: Path, expected: str) -> dict[str, Any]:
    expected_snapshot = load_snapshot(snapshot_path)
    observed = parent_snapshot(target, expected)
    if observed != expected_snapshot:
        fail(f"retained parent identity changed: {target.parent}")
    return observed


def root_snapshot(root: Path) -> dict[str, Any]:
    metadata = require_private_directory(root)
    return {
        "schema": "iroha.kagemusha.filesystem_custody_root.v1",
        "root": str(root),
        "identity": identity(metadata),
    }


def verify_root(root: Path, snapshot_path: Path) -> None:
    if root_snapshot(root) != load_snapshot(snapshot_path):
        fail(f"retained root identity changed: {root}")


def file_set(paths: list[Path]) -> dict[str, Any]:
    entries = []
    for path in paths:
        _, metadata = read_regular(path)
        entries.append({"path": str(path), **metadata})
    return {"schema": "iroha.kagemusha.filesystem_custody_files.v1", "entries": entries}


command = sys.argv[1] if len(sys.argv) > 1 else ""
arguments = sys.argv[2:]

if command == "normalize-target" and len(arguments) == 2:
    print(normalize_target(arguments[0], arguments[1]))
elif command == "snapshot-parent" and len(arguments) == 3:
    write_snapshot(Path(arguments[1]), parent_snapshot(Path(arguments[0]), arguments[2]))
elif command == "revalidate-parent" and len(arguments) == 3:
    verify_parent(Path(arguments[0]), Path(arguments[1]), arguments[2])
elif command == "create-root" and len(arguments) == 3:
    target, parent_state, root_state = map(Path, arguments)
    verify_parent(target, parent_state, "absent")
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    parent_descriptor = os.open(target.parent, parent_flags)
    try:
        expected_identity = load_snapshot(parent_state)["identity"]
        if identity(os.fstat(parent_descriptor)) != expected_identity:
            fail("retained parent descriptor does not match its custody snapshot")
        os.mkdir(target.name, 0o700, dir_fd=parent_descriptor)
        os.fsync(parent_descriptor)
    finally:
        os.close(parent_descriptor)
    verify_parent(target, parent_state, "existing")
    write_snapshot(root_state, root_snapshot(target))
elif command == "create-private-child" and len(arguments) == 4:
    parent, parent_state, name, child_state = (
        Path(arguments[0]), Path(arguments[1]), arguments[2], Path(arguments[3])
    )
    if not name or name in (".", "..") or "/" in name:
        fail("private child name is invalid")
    verify_root(parent, parent_state)
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(parent, flags)
    try:
        if identity(os.fstat(descriptor)) != load_snapshot(parent_state)["identity"]:
            fail("private parent descriptor changed before child creation")
        os.mkdir(name, 0o700, dir_fd=descriptor)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    verify_root(parent, parent_state)
    write_snapshot(child_state, root_snapshot(parent / name))
elif command == "snapshot-prepared" and len(arguments) == 2:
    write_snapshot(Path(arguments[1]), stable_tree(Path(arguments[0]), prepared_layout=True))
elif command == "snapshot-tree" and len(arguments) == 2:
    write_snapshot(Path(arguments[1]), stable_tree(Path(arguments[0])))
elif command == "revalidate-tree" and len(arguments) == 3:
    observed = stable_tree(Path(arguments[0]), prepared_layout=arguments[2] == "prepared")
    if observed != load_snapshot(Path(arguments[1])):
        fail(f"prepared app tree identity or contents changed: {arguments[0]}")
elif command == "revalidate-root" and len(arguments) == 2:
    verify_root(Path(arguments[0]), Path(arguments[1]))
elif command == "snapshot-root" and len(arguments) == 2:
    write_snapshot(Path(arguments[1]), root_snapshot(Path(arguments[0])))
elif command == "copy-exclusive" and len(arguments) == 2:
    payload, _ = read_regular(Path(arguments[0]))
    write_new(Path(arguments[1]), payload)
elif command == "snapshot-files" and len(arguments) >= 2:
    write_snapshot(Path(arguments[0]), file_set([Path(item) for item in arguments[1:]]))
elif command == "revalidate-files" and len(arguments) >= 2:
    observed = file_set([Path(item) for item in arguments[1:]])
    if observed != load_snapshot(Path(arguments[0])):
        fail("release evidence input identity or contents changed")
elif command == "publish-directory" and len(arguments) == 5:
    source, destination, parent_state, tree_state, root_state = map(Path, arguments)
    observed = stable_tree(source)
    if observed != load_snapshot(tree_state):
        fail("prepared stage changed before publication")
    verify_parent(destination, parent_state, "absent")
    if sys.platform != "darwin":
        fail("atomic no-replace directory publication requires Darwin renamex_np")
    libc = ctypes.CDLL(None, use_errno=True)
    renamex_np = libc.renamex_np
    renamex_np.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
    renamex_np.restype = ctypes.c_int
    rename_exclusive = 0x00000004
    result = renamex_np(os.fsencode(source), os.fsencode(destination), rename_exclusive)
    if result != 0:
        error = ctypes.get_errno()
        fail(f"atomic no-replace prepared publication failed: {os.strerror(error)}")
    verify_parent(destination, parent_state, "existing")
    published = stable_tree(destination)
    if published != observed:
        fail("prepared tree changed across atomic publication")
    write_snapshot(root_state, root_snapshot(destination))
elif command == "assert-absent" and len(arguments) >= 1:
    for item in arguments:
        if os.path.lexists(item):
            fail(f"refusing to overwrite or follow output path: {item}")
else:
    fail("internal custody command or argument count is invalid")
# KAGEMUSHA_CUSTODY_HELPER_END
PY
}

PREPARED_PARENT_SNAPSHOT="$TRANSIENT/prepared-parent-custody-v1.json"
PREPARED_TREE_SNAPSHOT="$TRANSIENT/prepared-tree-custody-v1.json"
PREPARED_ROOT_SNAPSHOT="$TRANSIENT/prepared-root-custody-v1.json"
EVIDENCE_PARENT_SNAPSHOT="$TRANSIENT/evidence-parent-custody-v1.json"
EVIDENCE_ROOT_SNAPSHOT="$TRANSIENT/evidence-root-custody-v1.json"
RAW_ROOT_SNAPSHOT="$TRANSIENT/evidence-raw-root-custody-v1.json"
TRANSIENT_PARENT_SNAPSHOT="$TRANSIENT/transient-parent-custody-v1.json"
TRANSIENT_ROOT_SNAPSHOT="$TRANSIENT/transient-root-custody-v1.json"

custody snapshot-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
custody snapshot-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"

if [[ -n "$PREPARED_ROOT" ]]; then
  if $PREPARE_ONLY; then
    PREPARED_ROOT="$(custody normalize-target "$PREPARED_ROOT" absent)"
    custody snapshot-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" absent
  else
    PREPARED_ROOT="$(custody normalize-target "$PREPARED_ROOT" existing)"
    custody snapshot-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
    custody snapshot-prepared "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT"
    custody snapshot-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  fi
  PREPARED_PARENT="${PREPARED_ROOT%/*}"
fi
if ! $PREPARE_ONLY; then
  EVIDENCE_ROOT="$(custody normalize-target "$EVIDENCE_ROOT" absent)"
  custody snapshot-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" absent
fi
for retained_root in "$EVIDENCE_ROOT" "$PREPARED_ROOT"; do
  [[ -n "$retained_root" ]] || continue
  if [[ "$retained_root" == "$ROOT_DIR" || "$retained_root" == "$ROOT_DIR"/* ]] \
    || [[ "$ROOT_DIR" == "$retained_root"/* ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: canonical retained roots must be disjoint from the repository" >&2
    exit 64
  fi
done
custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"

DEVICE_JSON="$TRANSIENT/devices.json"
XCODE_DEVICE_FILE="$TRANSIENT/xcode-device-id.txt"
"$XCRUN_BINARY" devicectl device info details \
  --device "$DEVICE_ID" \
  --timeout 20 \
  --quiet \
  --json-output "$TRANSIENT/device-details.json"
"$XCRUN_BINARY" devicectl list devices \
  --timeout 20 \
  --quiet \
  --json-output "$DEVICE_JSON"
"$PYTHON3_BINARY" -I - "$DEVICE_JSON" "$DEVICE_ID" "$XCODE_DEVICE_FILE" <<'PY'
from pathlib import Path
import json
import sys

document = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
selector = sys.argv[2]
matches = []
for device in document.get("result", {}).get("devices", []):
    hardware = device.get("hardwareProperties", {})
    candidates = {
        str(device.get("identifier", "")),
        str(hardware.get("udid", "")),
        str(hardware.get("ecid", "")),
        str(hardware.get("serialNumber", "")),
        str(device.get("deviceProperties", {}).get("name", "")),
    }
    if selector in candidates:
        matches.append(device)
if len(matches) != 1:
    raise SystemExit("requested physical device is not uniquely visible")
device = matches[0]
connection = device.get("connectionProperties", {})
properties = device.get("deviceProperties", {})
hardware = device.get("hardwareProperties", {})
checks = {
    "pairingState": connection.get("pairingState") == "paired",
    "tunnelState": connection.get("tunnelState") == "connected",
    "ddiServicesAvailable": properties.get("ddiServicesAvailable") is True,
    "developerModeStatus": properties.get("developerModeStatus") == "enabled",
    "reality": hardware.get("reality") == "physical",
    "platform": hardware.get("platform") == "iOS",
    "isProductionFused": hardware.get("isProductionFused") is True,
}
failed = [name for name, passed in checks.items() if not passed]
if failed:
    raise SystemExit("physical iPhone is unavailable: " + ",".join(failed))
udid = str(hardware.get("udid", ""))
if not udid:
    raise SystemExit("physical iPhone omitted its Xcode destination identifier")
Path(sys.argv[3]).write_text(udid, encoding="ascii")
PY
XCODE_DEVICE_ID="$(<"$XCODE_DEVICE_FILE")"

RAW_ROOT=""
RETAINED_REQUEST=""
REQUEST_SHA256=""
if ! $PREPARE_ONLY; then
  custody revalidate-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" absent
  custody create-root "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" "$EVIDENCE_ROOT_SNAPSHOT"
  custody create-private-child "$EVIDENCE_ROOT" "$EVIDENCE_ROOT_SNAPSHOT" raw "$RAW_ROOT_SNAPSHOT"
  RAW_ROOT="$EVIDENCE_ROOT/raw"
  RETAINED_REQUEST="$RAW_ROOT/request-v1.json"
  if [[ -n "$REQUEST" ]]; then
    custody copy-exclusive "$REQUEST" "$RETAINED_REQUEST"
  else
  GENERATED_REQUEST="$TRANSIENT/generated-request-v1.json"
  "$PYTHON3_BINARY" -I - "$GENERATED_REQUEST" <<'PY'
from pathlib import Path
import base64
import json
import secrets
import sys
import time

output = Path(sys.argv[1])
issued_at = time.time_ns() // 1_000_000
attestation = {
    "domain": "iroha:kagemusha:ios:production:app-attest:qualification:attestation:v1",
    "issued_at_unix_ms": issued_at,
    "nonce_base64": base64.b64encode(secrets.token_bytes(32)).decode("ascii"),
    "schema": "iroha.kagemusha.ios.app_attest_qualification_attestation_challenge.v1",
    "version": 1,
}
assertion = {
    "domain": "iroha:kagemusha:ios:production:app-attest:qualification:assertion:v1",
    "issued_at_unix_ms": issued_at,
    "nonce_base64": base64.b64encode(secrets.token_bytes(32)).decode("ascii"),
    "schema": "iroha.kagemusha.ios.app_attest_qualification_assertion_challenge.v1",
    "version": 1,
}
attestation_bytes = (
    json.dumps(attestation, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    + b"\n"
)
request = {
    "schema": "iroha.kagemusha.ios.app_attest_capture_request.v1",
    "version": 1,
    "attestation_client_data_base64": base64.b64encode(attestation_bytes).decode("ascii"),
    "assertion_client_data_template": assertion,
}
output.write_bytes(
    json.dumps(request, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    + b"\n"
)
PY
    custody copy-exclusive "$GENERATED_REQUEST" "$RETAINED_REQUEST"
  fi
  REQUEST_CUSTODY_SNAPSHOT="$TRANSIENT/request-custody-v1.json"
  custody snapshot-files "$REQUEST_CUSTODY_SNAPSHOT" "$RETAINED_REQUEST"
  custody revalidate-files "$REQUEST_CUSTODY_SNAPSHOT" "$RETAINED_REQUEST"
  REQUEST_SHA256_FILE="$TRANSIENT/request-sha256.txt"
  "$PYTHON3_BINARY" -I - "$RETAINED_REQUEST" "$REQUEST_SHA256_FILE" <<'PY'
from pathlib import Path
import hashlib
import json
import stat
import sys

path = Path(sys.argv[1])
digest_path = Path(sys.argv[2])
metadata = path.lstat()
if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size > 256 * 1024:
    raise SystemExit("capture request is not a bounded regular file")
payload = path.read_bytes()
value = json.loads(payload.decode("ascii"))
canonical = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii") + b"\n"
if payload != canonical:
    raise SystemExit("capture request is not canonical ASCII JSON")
if set(value) != {"schema", "version", "attestation_client_data_base64", "assertion_client_data_template"}:
    raise SystemExit("capture request fields are not exact")
if value["schema"] != "iroha.kagemusha.ios.app_attest_capture_request.v1" or value["version"] != 1:
    raise SystemExit("capture request schema/version is invalid")
digest_path.write_text(hashlib.sha256(payload).hexdigest(), encoding="ascii")
PY
  custody revalidate-files "$REQUEST_CUSTODY_SNAPSHOT" "$RETAINED_REQUEST"
  REQUEST_SHA256="$(<"$REQUEST_SHA256_FILE")"
  if [[ ! "$REQUEST_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: capture request digest is invalid" >&2
    exit 1
  fi
fi

if [[ -n "$PREPARED_ROOT" ]] && ! $PREPARE_ONLY; then
  PREPARED_APP="$PREPARED_ROOT/KagemushaProductionAppAttestLab.app"
  PREPARED_MEASUREMENTS_SOURCE="$PREPARED_ROOT/capture-app-code-sign-measurements-v1.json"
  PREPARED_MEASUREMENTS="$TRANSIENT/prepared-capture-app-code-sign-measurements-v1.json"
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
  APP="$TRANSIENT/KagemushaProductionAppAttestLab.app"
  "$DITTO_BINARY" --noqtn "$PREPARED_APP" "$APP"
  custody copy-exclusive "$PREPARED_MEASUREMENTS_SOURCE" "$PREPARED_MEASUREMENTS"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
else
  export KAGEMUSHA_APP_ATTEST_LAB_ROOT="$LAB_ROOT"
  "$XCODEGEN_BINARY" generate \
    --quiet \
    --spec "$LAB_ROOT/project.yml" \
    --project-root "$LAB_ROOT" \
    --project "$TRANSIENT"
  PROJECT="$TRANSIENT/KagemushaProductionAppAttestLab.xcodeproj"
  DERIVED_DATA="$TRANSIENT/DerivedData"
  BUILD_LOG="$TRANSIENT/xcodebuild.log"
  set +e
  "$XCODEBUILD_BINARY" \
    -project "$PROJECT" \
    -scheme KagemushaProductionAppAttestLab \
    -configuration Debug \
    -destination "platform=iOS,id=$XCODE_DEVICE_ID" \
    -derivedDataPath "$DERIVED_DATA" \
    -allowProvisioningUpdates \
    -allowProvisioningDeviceRegistration \
    DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM" \
    PRODUCT_BUNDLE_IDENTIFIER="$BUNDLE_ID" \
    CODE_SIGN_STYLE=Automatic \
    build >"$BUILD_LOG" 2>&1
  BUILD_STATUS=$?
  set -e
  if [[ $BUILD_STATUS -ne 0 ]]; then
    echo "[kagemusha-app-attest-lab] ERROR: production-entitled iPhone build/sign failed" >&2
    /usr/bin/grep -E '(^|: )(error:|No Accounts|Provisioning profile|requires a provisioning profile|doesn.t support|not available)' "$BUILD_LOG" \
      | /usr/bin/tail -n 40 >&2 || true
    exit "$BUILD_STATUS"
  fi
  APP="$DERIVED_DATA/Build/Products/Debug-iphoneos/KagemushaProductionAppAttestLab.app"
fi
if [[ ! -d "$APP" || -L "$APP" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: signed application bundle is missing" >&2
  exit 1
fi
"$CODESIGN_BINARY" --verify --deep --strict "$APP"
SIGNED_ENTITLEMENTS="$TRANSIENT/signed-entitlements.plist"
"$CODESIGN_BINARY" -d --entitlements :- "$APP" >"$SIGNED_ENTITLEMENTS" 2>/dev/null
if [[ ! -s "$SIGNED_ENTITLEMENTS" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: signed application entitlements are unavailable" >&2
  exit 1
fi
SIGNED_ENVIRONMENT="$($PLUTIL_BINARY -extract com.apple.developer.devicecheck.appattest-environment raw "$SIGNED_ENTITLEMENTS" 2>/dev/null || true)"
SIGNED_TEAM="$($PLUTIL_BINARY -extract com.apple.developer.team-identifier raw "$SIGNED_ENTITLEMENTS" 2>/dev/null || true)"
SIGNED_APPLICATION_ID="$($PLUTIL_BINARY -extract application-identifier raw "$SIGNED_ENTITLEMENTS" 2>/dev/null || true)"
if [[ "$SIGNED_ENVIRONMENT" != "production" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: signed app lost the production App Attest entitlement" >&2
  exit 1
fi
if [[ "$SIGNED_TEAM" != "$DEVELOPMENT_TEAM" || "$SIGNED_APPLICATION_ID" != "$DEVELOPMENT_TEAM.$BUNDLE_ID" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: signed app identity does not match the requested team and bundle" >&2
  exit 1
fi
if [[ ! -f "$APP/embedded.mobileprovision" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: signed app has no embedded provisioning profile" >&2
  exit 1
fi
PROFILE_PLIST="$TRANSIENT/profile.plist"
/usr/bin/security cms -D -i "$APP/embedded.mobileprovision" >"$PROFILE_PLIST" 2>/dev/null
PROFILE_APPLICATION_ID="$($PLUTIL_BINARY -extract Entitlements.application-identifier raw "$PROFILE_PLIST" 2>/dev/null || true)"
if [[ "$PROFILE_APPLICATION_ID" != "$DEVELOPMENT_TEAM.$BUNDLE_ID" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: provisioning profile does not authorize the exact production App Attest identity" >&2
  exit 1
fi
# PROFILE_APP_ATTEST_AUTHORIZATION_V1
# Apple profiles may authorize App Attest with either a scalar entitlement or
# the exact development/production set. The signed executable above must still
# select the scalar production environment; this check only proves that the
# embedded profile authorizes that selection and rejects unknown environments.
if ! "$PYTHON3_BINARY" -I - "$PROFILE_PLIST" <<'PY'
import plistlib
import sys

try:
    with open(sys.argv[1], "rb") as source:
        profile = plistlib.load(source)
    environment = profile["Entitlements"][
        "com.apple.developer.devicecheck.appattest-environment"
    ]
except (KeyError, OSError, plistlib.InvalidFileException, TypeError, ValueError):
    raise SystemExit(1)

if isinstance(environment, str):
    authorized = environment == "production"
elif isinstance(environment, list):
    allowed = {"development", "production"}
    authorized = (
        1 <= len(environment) <= len(allowed)
        and all(isinstance(item, str) for item in environment)
        and len(set(environment)) == len(environment)
        and set(environment) <= allowed
        and "production" in environment
    )
else:
    authorized = False
raise SystemExit(0 if authorized else 1)
PY
then
  echo "[kagemusha-app-attest-lab] ERROR: profile App Attest entitlement does not authorize production" >&2
  exit 1
fi

CAPTURE_APP_MEASUREMENTS="$TRANSIENT/capture-app-code-sign-measurements-v1.json"
"$PYTHON3_BINARY" -I "$ROOT_DIR/scripts/measure_kagemusha_production_app_attest_bundle.py" \
  --app "$APP" \
  --signed-entitlements "$SIGNED_ENTITLEMENTS" \
  --development-team "$DEVELOPMENT_TEAM" \
  --bundle-id "$BUNDLE_ID" \
  --output "$CAPTURE_APP_MEASUREMENTS"
chmod 0600 "$CAPTURE_APP_MEASUREMENTS"

if [[ -n "$PREPARED_ROOT" ]] && ! $PREPARE_ONLY; then
  if ! /usr/bin/cmp -s "$CAPTURE_APP_MEASUREMENTS" "$PREPARED_MEASUREMENTS"; then
    echo "[kagemusha-app-attest-lab] ERROR: prepared app no longer matches its code-sign measurement" >&2
    exit 1
  fi
fi
CAPTURE_APP_TREE_SNAPSHOT="$TRANSIENT/capture-app-tree-custody-v1.json"
custody snapshot-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT"
CAPTURE_HOST_INPUTS=("$SIGNED_ENTITLEMENTS" "$CAPTURE_APP_MEASUREMENTS")
if [[ -n "$PREPARED_ROOT" ]] && ! $PREPARE_ONLY; then
  CAPTURE_HOST_INPUTS+=("$PREPARED_MEASUREMENTS")
fi
CAPTURE_HOST_INPUTS_SNAPSHOT="$TRANSIENT/capture-host-inputs-custody-v1.json"
custody snapshot-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"

if $PREPARE_ONLY; then
  custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
  custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" absent
  custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
  custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
  PREPARED_STAGE="$(mktemp -d "${PREPARED_ROOT%/*}/.kagemusha-app-attest-prepared.XXXXXX")"
  chmod 0700 "$PREPARED_STAGE"
  PREPARED_STAGE_ROOT_SNAPSHOT="$TRANSIENT/prepared-stage-root-custody-v1.json"
  custody snapshot-root "$PREPARED_STAGE" "$PREPARED_STAGE_ROOT_SNAPSHOT"
  "$DITTO_BINARY" --noqtn "$APP" "$PREPARED_STAGE/KagemushaProductionAppAttestLab.app"
  "$CODESIGN_BINARY" --verify --deep --strict "$PREPARED_STAGE/KagemushaProductionAppAttestLab.app"
  custody copy-exclusive \
    "$CAPTURE_APP_MEASUREMENTS" \
    "$PREPARED_STAGE/capture-app-code-sign-measurements-v1.json"
  "$PYTHON3_BINARY" -I - "$PREPARED_STAGE" <<'PY'
from pathlib import Path
import os
import stat
import sys

root = Path(sys.argv[1])
for path in sorted(root.rglob("*"), key=lambda item: len(item.parts), reverse=True):
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode):
        raise SystemExit("prepared app tree contains a symbolic link")
    if stat.S_ISREG(metadata.st_mode):
        descriptor = os.open(path, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
for path in sorted(
    (item for item in root.rglob("*") if item.is_dir()),
    key=lambda item: len(item.parts),
    reverse=True,
):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
descriptor = os.open(root, os.O_RDONLY)
try:
    os.fsync(descriptor)
finally:
    os.close(descriptor)
PY
  PREPARED_STAGE_TREE_SNAPSHOT="$TRANSIENT/prepared-stage-tree-custody-v1.json"
  custody snapshot-prepared "$PREPARED_STAGE" "$PREPARED_STAGE_TREE_SNAPSHOT"
  custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
  custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" absent
  custody publish-directory \
    "$PREPARED_STAGE" \
    "$PREPARED_ROOT" \
    "$PREPARED_PARENT_SNAPSHOT" \
    "$PREPARED_STAGE_TREE_SNAPSHOT" \
    "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_STAGE_TREE_SNAPSHOT" prepared
  echo "[kagemusha-app-attest-lab] exact signed capture app is prepared: $PREPARED_ROOT"
  exit 0
fi

custody copy-exclusive \
  "$CAPTURE_APP_MEASUREMENTS" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json"

CAPTURE_POLICY="$EVIDENCE_ROOT/production-ios-policy-v1.json"
if [[ -n "$PRODUCTION_POLICY" ]]; then
  custody copy-exclusive "$PRODUCTION_POLICY" "$CAPTURE_POLICY"
else
  ROOT_CERTIFICATE="$ROOT_DIR/certs/apple_app_attestation_root.der"
  GENERATED_POLICY="$TRANSIENT/generated-production-ios-policy-v1.json"
  "$PYTHON3_BINARY" -I - \
    "$ROOT_CERTIFICATE" "$DEVELOPMENT_TEAM" "$BUNDLE_ID" \
    "$GENERATED_POLICY" <<'PY'
from pathlib import Path
import base64
import hashlib
import json
import sys

root, team, bundle, output = map(Path, sys.argv[1:])
root_bytes = root.read_bytes()
value = {
    "schema": "iroha.kagemusha.ios.production_device_policy.v1",
    "version": 1,
    "policy_id": "kagemusha-ios-production-app-attest-qualification-v1",
    "app_id_prefix": str(team),
    "bundle_id": str(bundle),
    "environment": "production",
    "allowed_validation_categories": [1, 2, 3, 4, 5, 6, 10],
    "allowed_bundle_versions": ["1"],
    "trusted_app_attest_roots": [
        {
            "der_base64": base64.b64encode(root_bytes).decode("ascii"),
            "sha256": hashlib.sha256(root_bytes).hexdigest(),
        }
    ],
    "revoked_certificate_sha256": [],
    "x509_validation_profile": "apple-app-attest-x509-chain-and-nonce-v1",
    "secure_enclave_key_profile": "dcappattest-generated-secure-enclave-key-v1",
}
output.write_text(
    json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="ascii",
)
PY
  custody copy-exclusive "$GENERATED_POLICY" "$CAPTURE_POLICY"
fi

EVIDENCE_INPUTS_SNAPSHOT="$TRANSIENT/evidence-inputs-custody-v1.json"
custody snapshot-files \
  "$EVIDENCE_INPUTS_SNAPSHOT" \
  "$RETAINED_REQUEST" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  "$CAPTURE_POLICY"

# Final identity gate immediately before the exact signed executable leaves
# host custody for installation on the physical iPhone.
custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"
custody revalidate-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" existing
custody revalidate-root "$EVIDENCE_ROOT" "$EVIDENCE_ROOT_SNAPSHOT"
custody revalidate-root "$RAW_ROOT" "$RAW_ROOT_SNAPSHOT"
custody revalidate-files \
  "$EVIDENCE_INPUTS_SNAPSHOT" \
  "$RETAINED_REQUEST" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  "$CAPTURE_POLICY"
custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
if [[ -n "$PREPARED_ROOT" ]]; then
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
fi

"$XCRUN_BINARY" devicectl device install app \
  --device "$DEVICE_ID" \
  --timeout 120 \
  --quiet \
  --json-output "$TRANSIENT/install.json" \
  "$APP"
"$XCRUN_BINARY" devicectl device copy to \
  --device "$DEVICE_ID" \
  --source "$RETAINED_REQUEST" \
  --destination "Documents/kagemusha-production-app-attest/request-v1.json" \
  --domain-type appDataContainer \
  --domain-identifier "$BUNDLE_ID" \
  --timeout 120 \
  --quiet \
  --json-output "$TRANSIENT/stage-request.json"
"$XCRUN_BINARY" devicectl device process launch \
  --device "$DEVICE_ID" \
  --terminate-existing \
  --quiet \
  --json-output "$TRANSIENT/launch.json" \
  "$BUNDLE_ID"

CAPTURE="$RAW_ROOT/capture-v1.json"
DEVICE_CAPTURE=""
for _attempt in {1..90}; do
  CAPTURE_CANDIDATE="$TRANSIENT/device-capture-$_attempt.json"
  if "$XCRUN_BINARY" devicectl device copy from \
    --device "$DEVICE_ID" \
    --source "Documents/kagemusha-production-app-attest/capture-$REQUEST_SHA256.json" \
    --destination "$CAPTURE_CANDIDATE" \
    --domain-type appDataContainer \
    --domain-identifier "$BUNDLE_ID" \
    --timeout 20 \
    --quiet \
    --json-output "$TRANSIENT/pull-capture-$_attempt.json" >/dev/null 2>&1; then
    DEVICE_CAPTURE="$CAPTURE_CANDIDATE"
    break
  fi
  /bin/sleep 2
done
if [[ -z "$DEVICE_CAPTURE" || ! -s "$DEVICE_CAPTURE" ]]; then
  echo "[kagemusha-app-attest-lab] ERROR: device did not export an App Attest result within 180 seconds" >&2
  exit 1
fi
custody copy-exclusive "$DEVICE_CAPTURE" "$CAPTURE"

CAPTURE_OUTPUT_SNAPSHOT="$TRANSIENT/capture-output-custody-v1.json"
custody snapshot-files "$CAPTURE_OUTPUT_SNAPSHOT" "$CAPTURE"

# The physical capture is accepted only if every host-side object still has
# the exact identity and bytes that were approved immediately before install.
custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"
custody revalidate-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" existing
custody revalidate-root "$EVIDENCE_ROOT" "$EVIDENCE_ROOT_SNAPSHOT"
custody revalidate-root "$RAW_ROOT" "$RAW_ROOT_SNAPSHOT"
custody revalidate-files \
  "$EVIDENCE_INPUTS_SNAPSHOT" \
  "$RETAINED_REQUEST" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  "$CAPTURE_POLICY"
custody revalidate-files "$CAPTURE_OUTPUT_SNAPSHOT" "$CAPTURE"
custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
if [[ -n "$PREPARED_ROOT" ]]; then
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
fi

"$CODESIGN_BINARY" --verify --deep --strict "$APP"
POST_CAPTURE_MEASUREMENTS="$TRANSIENT/post-capture-code-sign-measurements-v1.json"
"$PYTHON3_BINARY" -I "$ROOT_DIR/scripts/measure_kagemusha_production_app_attest_bundle.py" \
  --app "$APP" \
  --signed-entitlements "$SIGNED_ENTITLEMENTS" \
  --development-team "$DEVELOPMENT_TEAM" \
  --bundle-id "$BUNDLE_ID" \
  --output "$POST_CAPTURE_MEASUREMENTS"
if ! /usr/bin/cmp -s "$CAPTURE_APP_MEASUREMENTS" "$POST_CAPTURE_MEASUREMENTS"; then
  echo "[kagemusha-app-attest-lab] ERROR: signed capture app changed during the physical run" >&2
  exit 1
fi
if [[ -n "$PREPARED_ROOT" ]] \
  && ! /usr/bin/cmp -s "$PREPARED_MEASUREMENTS" "$POST_CAPTURE_MEASUREMENTS"; then
  echo "[kagemusha-app-attest-lab] ERROR: prepared measurement changed during the physical run" >&2
  exit 1
fi
custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
custody revalidate-files \
  "$EVIDENCE_INPUTS_SNAPSHOT" \
  "$RETAINED_REQUEST" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  "$CAPTURE_POLICY"
custody revalidate-files "$CAPTURE_OUTPUT_SNAPSHOT" "$CAPTURE"
if [[ -n "$PREPARED_ROOT" ]]; then
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
fi

if [[ -n "$REQUEST" ]]; then
  PLATFORM_OUTPUT="$EVIDENCE_ROOT/platform-evidence-v1.json"
  SUMMARY_OUTPUT="$EVIDENCE_ROOT/production-capture-summary-v1.json"
  COMPLETION_LABEL="release-bound production App Attest capture"
else
  PLATFORM_OUTPUT="$EVIDENCE_ROOT/qualification-material-v1.json"
  SUMMARY_OUTPUT="$EVIDENCE_ROOT/qualification-summary-v1.json"
  COMPLETION_LABEL="production-environment App Attest qualification"
fi
PLATFORM_OUTPUT_TRANSIENT="$TRANSIENT/${PLATFORM_OUTPUT##*/}"
SUMMARY_OUTPUT_TRANSIENT="$TRANSIENT/${SUMMARY_OUTPUT##*/}"
custody assert-absent \
  "$PLATFORM_OUTPUT" \
  "$SUMMARY_OUTPUT" \
  "$PLATFORM_OUTPUT_TRANSIENT" \
  "$SUMMARY_OUTPUT_TRANSIENT"

"$PYTHON3_BINARY" "$ROOT_DIR/scripts/check_kagemusha_production_app_attest_capture.py" \
  --capture "$CAPTURE" \
  --request "$RETAINED_REQUEST" \
  --production-policy "$CAPTURE_POLICY" \
  --capture-app-code-sign-measurements "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  --platform-evidence-output "$PLATFORM_OUTPUT_TRANSIENT" \
  --summary-output "$SUMMARY_OUTPUT_TRANSIENT"

# The validator receives pinned inputs, and its products cross into retained
# evidence only through exclusive, no-follow, singly-linked publication.
custody revalidate-parent "$TRANSIENT" "$TRANSIENT_PARENT_SNAPSHOT" existing
custody revalidate-root "$TRANSIENT" "$TRANSIENT_ROOT_SNAPSHOT"
custody revalidate-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" existing
custody revalidate-root "$EVIDENCE_ROOT" "$EVIDENCE_ROOT_SNAPSHOT"
custody revalidate-root "$RAW_ROOT" "$RAW_ROOT_SNAPSHOT"
custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT" tree
custody revalidate-files "$CAPTURE_HOST_INPUTS_SNAPSHOT" "${CAPTURE_HOST_INPUTS[@]}"
custody revalidate-files \
  "$EVIDENCE_INPUTS_SNAPSHOT" \
  "$RETAINED_REQUEST" \
  "$EVIDENCE_ROOT/capture-app-code-sign-measurements-v1.json" \
  "$CAPTURE_POLICY"
custody revalidate-files "$CAPTURE_OUTPUT_SNAPSHOT" "$CAPTURE"
if [[ -n "$PREPARED_ROOT" ]]; then
  custody revalidate-parent "$PREPARED_ROOT" "$PREPARED_PARENT_SNAPSHOT" existing
  custody revalidate-root "$PREPARED_ROOT" "$PREPARED_ROOT_SNAPSHOT"
  custody revalidate-tree "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT" prepared
fi
custody copy-exclusive "$PLATFORM_OUTPUT_TRANSIENT" "$PLATFORM_OUTPUT"
custody copy-exclusive "$SUMMARY_OUTPUT_TRANSIENT" "$SUMMARY_OUTPUT"
FINAL_OUTPUTS_SNAPSHOT="$TRANSIENT/final-outputs-custody-v1.json"
custody snapshot-files "$FINAL_OUTPUTS_SNAPSHOT" "$PLATFORM_OUTPUT" "$SUMMARY_OUTPUT"
custody revalidate-parent "$EVIDENCE_ROOT" "$EVIDENCE_PARENT_SNAPSHOT" existing
custody revalidate-root "$EVIDENCE_ROOT" "$EVIDENCE_ROOT_SNAPSHOT"
custody revalidate-root "$RAW_ROOT" "$RAW_ROOT_SNAPSHOT"
custody revalidate-files "$FINAL_OUTPUTS_SNAPSHOT" "$PLATFORM_OUTPUT" "$SUMMARY_OUTPUT"

echo "[kagemusha-app-attest-lab] $COMPLETION_LABEL is ready: $EVIDENCE_ROOT"
