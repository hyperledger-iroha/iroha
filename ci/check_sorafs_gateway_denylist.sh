#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
check_sorafs_gateway_denylist.sh [--evidence-out PATH]

Validates the denylist pack/diff helpers and, when requested, captures the CLI
evidence summary so CI can persist it as build artefacts.

Options:
  --evidence-out PATH   Copy the generated evidence JSON to PATH after
                        validation (directories created automatically).
  -h, --help            Show this message.
USAGE
}

evidence_copy_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --evidence-out)
      [[ $# -lt 2 ]] && { echo "error: --evidence-out requires a path" >&2; usage; exit 1; }
      evidence_copy_path="$2"
      shift 2
      ;;
    --evidence-out=*)
      evidence_copy_path="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLE_JSON="${ROOT_DIR}/docs/source/sorafs_gateway_denylist_sample.json"

run_xtask() {
  cargo run -p xtask --bin xtask --quiet -- "$@"
}

copy_file_no_follow() {
  local source_file="$1"
  local target_file="$2"
  local label="$3"
  SORAFS_DENYLIST_COPY_SOURCE="${source_file}" \
  SORAFS_DENYLIST_COPY_TARGET="${target_file}" \
  SORAFS_DENYLIST_COPY_LABEL="${label}" \
  python3 <<'PY'
import os
import pathlib
import stat
import sys

source = pathlib.Path(os.environ["SORAFS_DENYLIST_COPY_SOURCE"])
target = pathlib.Path(os.environ["SORAFS_DENYLIST_COPY_TARGET"])
label = os.environ["SORAFS_DENYLIST_COPY_LABEL"]

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def write_open_flags() -> int:
    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_TRUNC
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

def validate_path(path: pathlib.Path, path_label: str) -> None:
    if path.is_symlink():
        sys.exit(f"[sorafs-gateway-denylist] {path_label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            sys.exit(
                f"[sorafs-gateway-denylist] {path_label} parent must be a directory: {parent}"
            )

def write_all(fd: int, chunk: bytes) -> None:
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("short write")
        view = view[written:]

def sync_output_parent(path: pathlib.Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def require_regular_file(path: pathlib.Path, path_label: str) -> None:
    validate_path(path, path_label)
    try:
        path_stat = path.lstat()
    except FileNotFoundError as exc:
        raise SystemExit(f"[sorafs-gateway-denylist] {path_label} missing: {path}") from exc
    if not stat.S_ISREG(path_stat.st_mode):
        sys.exit(f"[sorafs-gateway-denylist] {path_label} must be a regular file: {path}")
    if path_stat.st_size <= 0:
        sys.exit(f"[sorafs-gateway-denylist] {path_label} missing or empty: {path}")

require_regular_file(source, f"{label} source")
validate_path(target, f"{label} target")
target.parent.mkdir(parents=True, exist_ok=True)
validate_path(target, f"{label} target")

read_fd = os.open(source, read_open_flags())
write_fd = -1
try:
    write_fd = os.open(target, write_open_flags(), 0o666)
    while True:
        chunk = os.read(read_fd, 1024 * 1024)
        if not chunk:
            break
        write_all(write_fd, chunk)
    os.fsync(write_fd)
finally:
    os.close(read_fd)
    if write_fd >= 0:
        os.close(write_fd)
sync_output_parent(target)
PY
}

require_nonempty_file() {
  local target_file="$1"
  local label="$2"
  SORAFS_DENYLIST_REQUIRED_PATH="${target_file}" \
  SORAFS_DENYLIST_REQUIRED_LABEL="${label}" \
  python3 <<'PY'
import os
import pathlib
import stat
import sys

path = pathlib.Path(os.environ["SORAFS_DENYLIST_REQUIRED_PATH"])
label = os.environ["SORAFS_DENYLIST_REQUIRED_LABEL"]

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def fail(message: str) -> None:
    sys.exit(f"[sorafs-gateway-denylist] {message}: {path}")

try:
    if path.is_symlink():
        fail(f"{label} must not be a symlink")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            fail(f"{label} parent must be a directory")
    path_stat = path.lstat()
    if not stat.S_ISREG(path_stat.st_mode):
        fail(f"{label} must be a regular file")
    if path_stat.st_size <= 0:
        fail(f"{label} missing or empty")
    fd = os.open(path, read_open_flags())
    try:
        if os.fstat(fd).st_size <= 0:
            fail(f"{label} missing or empty")
    finally:
        os.close(fd)
except FileNotFoundError:
    fail(f"{label} missing or empty")
except OSError as exc:
    sys.exit(f"[sorafs-gateway-denylist] failed to inspect {label}: {path}: {exc}")
PY
}

first_bundle_json() {
  local bundle_dir="$1"
  local label="$2"
  SORAFS_DENYLIST_BUNDLE_DIR="${bundle_dir}" \
  SORAFS_DENYLIST_BUNDLE_LABEL="${label}" \
  python3 <<'PY'
import os
import pathlib
import stat
import sys

bundle_dir = pathlib.Path(os.environ["SORAFS_DENYLIST_BUNDLE_DIR"])
label = os.environ["SORAFS_DENYLIST_BUNDLE_LABEL"]

def validate_path(path: pathlib.Path, path_label: str) -> None:
    if path.is_symlink():
        sys.exit(f"[sorafs-gateway-denylist] {path_label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            sys.exit(
                f"[sorafs-gateway-denylist] {path_label} parent must be a directory: {parent}"
            )

validate_path(bundle_dir, f"{label} directory")
if not bundle_dir.is_dir():
    sys.exit(f"[sorafs-gateway-denylist] {label} directory missing: {bundle_dir}")
bundles = []
for path in sorted(bundle_dir.glob("*.json")):
    validate_path(path, f"{label} bundle")
    try:
        path_stat = path.lstat()
    except FileNotFoundError:
        continue
    if stat.S_ISREG(path_stat.st_mode) and path_stat.st_size > 0:
        bundles.append(path)
if not bundles:
    sys.exit(f"[sorafs-gateway-denylist] {label} bundle JSON missing: {bundle_dir}")
print(bundles[0])
PY
}

echo "[sorafs-gateway-denylist] verifying bundle + diff tooling"

workdir="$(mktemp -d)"
cleanup() {
  rm -rf "${workdir}"
}
trap cleanup EXIT

old_json="${workdir}/denylist_old.json"
new_json="${workdir}/denylist_new.json"
copy_file_no_follow "${SAMPLE_JSON}" "${new_json}" "new denylist snapshot"

python3 - <<'PY' "${SAMPLE_JSON}" "${old_json}"
import json, os, pathlib, stat, sys
src, dst = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def write_open_flags() -> int:
    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_TRUNC
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

def validate_path(path: pathlib.Path, label: str) -> None:
    if path.is_symlink():
        raise SystemExit(f"{label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            raise SystemExit(f"{label} parent must be a directory: {parent}")

def write_all(fd: int, chunk: bytes) -> None:
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("short write")
        view = view[written:]

def sync_output_parent(path: pathlib.Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def read_json(path: pathlib.Path):
    validate_path(path, "sample denylist")
    if not stat.S_ISREG(path.lstat().st_mode):
        raise SystemExit(f"sample denylist must be a regular file: {path}")
    fd = os.open(path, read_open_flags())
    try:
        with os.fdopen(fd, "r", encoding="utf-8") as fh:
            fd = -1
            return json.load(fh)
    finally:
        if fd >= 0:
            os.close(fd)

def write_json(path: pathlib.Path, payload) -> None:
    validate_path(path, "old denylist snapshot")
    path.parent.mkdir(parents=True, exist_ok=True)
    validate_path(path, "old denylist snapshot")
    rendered = (json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n").encode(
        "utf-8"
    )
    fd = os.open(path, write_open_flags(), 0o666)
    try:
        write_all(fd, rendered)
        os.fsync(fd)
    finally:
        if fd >= 0:
            os.close(fd)
    sync_output_parent(path)

data = read_json(src)
if len(data) < 2:
    raise SystemExit("sample denylist must contain at least two entries")
old_only_entry = {
    "kind": "url",
    "url": "https://example.invalid/retired/old-only.bin",
    "reason": "Retired CI denylist diff control",
    "issued_at": "2025-01-01T00:00:00Z",
    "expires_at": "2025-04-01T00:00:00Z",
}
write_json(dst, [*data[:-1], old_only_entry])
PY

old_out="${workdir}/bundle_old"
new_out="${workdir}/bundle_new"

echo "[sorafs-gateway-denylist] packing old denylist snapshot"
run_xtask sorafs-gateway denylist pack \
  --input "${old_json}" \
  --out "${old_out}" \
  --label "ci-old" \
  --force >/dev/null

echo "[sorafs-gateway-denylist] packing new denylist snapshot"
run_xtask sorafs-gateway denylist pack \
  --input "${new_json}" \
  --out "${new_out}" \
  --label "ci-new" \
  --force >/dev/null

old_bundle="$(first_bundle_json "${old_out}" "old denylist")"
new_bundle="$(first_bundle_json "${new_out}" "new denylist")"
diff_report="${workdir}/denylist_diff.json"

echo "[sorafs-gateway-denylist] running diff"
run_xtask sorafs-gateway denylist diff \
  --old "${old_bundle}" \
  --new "${new_bundle}" \
  --report-json "${diff_report}" >/dev/null

require_nonempty_file "${diff_report}" "diff report"

python3 - <<'PY' "${diff_report}"
import json, os, pathlib, stat, sys
path = pathlib.Path(sys.argv[1])

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def validate_path(path: pathlib.Path, label: str) -> None:
    if path.is_symlink():
        raise SystemExit(f"{label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            raise SystemExit(f"{label} parent must be a directory: {parent}")

validate_path(path, "diff report")
if not stat.S_ISREG(path.lstat().st_mode):
    raise SystemExit(f"diff report must be a regular file: {path}")
fd = os.open(path, read_open_flags())
try:
    with os.fdopen(fd, "r", encoding="utf-8") as handle:
        fd = -1
        report = json.load(handle)
finally:
    if fd >= 0:
        os.close(fd)
added = len(report.get("added", []))
removed = len(report.get("removed", []))
if added == 0 or removed == 0:
    raise SystemExit("expected both added and removed entries in diff report")
PY

echo "[sorafs-gateway-denylist] diff evidence ok (${diff_report})"

evidence_json="${workdir}/denylist_evidence.json"
echo "[sorafs-gateway-denylist] generating evidence summary via iroha3"
cargo run -p iroha_cli --bin iroha3 --quiet -- \
  -c "${ROOT_DIR}/defaults/client.toml" \
  app sorafs gateway evidence \
  --denylist "${new_json}" \
  --out "${evidence_json}" \
  --label "ci-denylist" >/dev/null

python3 - <<'PY' "${evidence_json}" "${new_json}"
import json, os, pathlib, stat, sys
evidence_path, denylist_path = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def validate_path(path: pathlib.Path, label: str) -> None:
    if path.is_symlink():
        raise SystemExit(f"{label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.exists() and not parent.is_dir():
            raise SystemExit(f"{label} parent must be a directory: {parent}")

def read_json(path: pathlib.Path, label: str):
    validate_path(path, label)
    if not stat.S_ISREG(path.lstat().st_mode):
        raise SystemExit(f"{label} must be a regular file: {path}")
    fd = os.open(path, read_open_flags())
    try:
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            fd = -1
            return json.load(handle)
    finally:
        if fd >= 0:
            os.close(fd)

report = read_json(evidence_path, "evidence summary")
entries = read_json(denylist_path, "denylist bundle")
entry_count = len(entries)
source = report.get("source", {})
if source.get("entry_count") != entry_count:
    raise SystemExit("evidence entry_count mismatch")
totals = report.get("totals", {})
kind_total = sum(item.get("count", 0) for item in totals.get("by_kind", []))
if kind_total != entry_count:
    raise SystemExit("kind totals do not match entry count")
tier_entries = totals.get("by_policy_tier", [])
tier_total = sum(item.get("count", 0) for item in tier_entries)
if tier_total != entry_count:
    raise SystemExit("policy tier totals do not match entry count")
tier_map = {item.get("tier"): item.get("count", 0) for item in tier_entries}
emergency_reviews = report.get("emergency_reviews", [])
if "emergency" in tier_map and tier_map["emergency"] != len(emergency_reviews):
    raise SystemExit("emergency review count mismatch")
PY

echo "[sorafs-gateway-denylist] evidence summary ok (${evidence_json})"

if [[ -n "${evidence_copy_path}" ]]; then
  echo "[sorafs-gateway-denylist] writing evidence copy to ${evidence_copy_path}"
  copy_file_no_follow "${evidence_json}" "${evidence_copy_path}" "evidence copy"
fi
