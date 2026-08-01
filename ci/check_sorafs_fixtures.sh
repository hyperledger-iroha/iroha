#!/usr/bin/env bash
# Verify checked-in SoraFS fixtures, signatures, SDK inventory, and heavy
# cross-language vectors. Requires Cargo, Python 3, and the repository's pinned
# dependencies. Node.js and Go are mandatory because their heavyweight
# cross-language checks are part of the release fixture contract.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_snapshot_root=""

usage() {
  cat <<'EOF'
Usage: ci/check_sorafs_fixtures.sh

Verify the complete checked-in SoraFS fixture set. The command is read-only
when the fixtures are current. Reference-SDK generators run in two isolated
temporary copies; any byte drift, missing or extra path, or second-run
difference fails.

Environment:
  CARGO_NET_OFFLINE                    Cargo offline mode (default: true).
  CARGO_TERM_COLOR                     Cargo colour setting (default: never).
EOF
}

if [[ "$#" -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[sorafs-fixtures] error: unrecognized argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

cleanup_fixture_snapshots() {
  if [[ -n "${fixture_snapshot_root}" && -d "${fixture_snapshot_root}" ]]; then
    rm -rf -- "${fixture_snapshot_root}"
  fi
}

require_fixture_tool() {
  local tool_name="$1"
  local check_label="$2"
  if command -v "${tool_name}" >/dev/null 2>&1; then
    return 0
  fi
  echo "[sorafs-fixtures] error: ${check_label} requires ${tool_name}" >&2
  exit 1
}

snapshot_manifest_tree() {
  local fixture_root="$1"
  local output_path="$2"
  python3 - "${fixture_root}" "${output_path}" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
output = Path(sys.argv[2])
if root.is_symlink():
    raise SystemExit(f"{root} must not be a symlink")
try:
    root_stat = root.lstat()
except FileNotFoundError as exc:
    raise SystemExit(f"{root} is missing") from exc
if not stat.S_ISDIR(root_stat.st_mode):
    raise SystemExit(f"{root} must be a directory")

max_snapshot_file_bytes = 64 << 20
read_flags = (
    os.O_RDONLY
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
snapshot: dict[str, dict[str, object]] = {}
directory_identities: dict[Path, tuple[int, int]] = {
    root: (root_stat.st_dev, root_stat.st_ino)
}
for current_root, directory_names, file_names in os.walk(root, followlinks=False):
    directory_names.sort()
    file_names.sort()
    current = Path(current_root)
    for directory_name in directory_names:
        directory = current / directory_name
        directory_stat = directory.lstat()
        if stat.S_ISLNK(directory_stat.st_mode):
            raise SystemExit(f"{directory} must not be a symlink")
        if not stat.S_ISDIR(directory_stat.st_mode):
            raise SystemExit(f"{directory} must be a directory")
        directory_identities[directory] = (
            directory_stat.st_dev,
            directory_stat.st_ino,
        )
    for file_name in file_names:
        path = current / file_name
        relative = path.relative_to(root).as_posix()
        before = path.lstat()
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
        ):
            raise SystemExit(f"{path} must be a single-link regular file")
        if before.st_size > max_snapshot_file_bytes:
            raise SystemExit(
                f"{path} exceeds the {max_snapshot_file_bytes}-byte snapshot bound"
            )
        descriptor = os.open(path, read_flags)
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino)
                or before.st_size != opened.st_size
                or before.st_mtime_ns != opened.st_mtime_ns
            ):
                raise SystemExit(f"{path} changed while it was opened")
            digest = hashlib.sha256()
            byte_length = 0
            while True:
                chunk = os.read(
                    descriptor,
                    min(1024 * 1024, max_snapshot_file_bytes - byte_length + 1),
                )
                if not chunk:
                    break
                byte_length += len(chunk)
                if byte_length > max_snapshot_file_bytes:
                    raise SystemExit(
                        f"{path} exceeds the {max_snapshot_file_bytes}-byte snapshot bound"
                    )
                digest.update(chunk)
            after = os.fstat(descriptor)
            path_after = path.lstat()
            if (
                not stat.S_ISREG(after.st_mode)
                or after.st_nlink != 1
                or (opened.st_dev, opened.st_ino) != (after.st_dev, after.st_ino)
                or opened.st_size != after.st_size
                or opened.st_mtime_ns != after.st_mtime_ns
                or byte_length != after.st_size
                or stat.S_ISLNK(path_after.st_mode)
                or not stat.S_ISREG(path_after.st_mode)
                or path_after.st_nlink != 1
                or (before.st_dev, before.st_ino)
                != (path_after.st_dev, path_after.st_ino)
                or path_after.st_size != after.st_size
                or path_after.st_mtime_ns != after.st_mtime_ns
            ):
                raise SystemExit(f"{path} changed while it was hashed")
        finally:
            os.close(descriptor)
        snapshot[relative] = {
            "byte_length": byte_length,
            "sha256": digest.hexdigest(),
        }

for directory, expected_identity in directory_identities.items():
    after = directory.lstat()
    if (
        stat.S_ISLNK(after.st_mode)
        or not stat.S_ISDIR(after.st_mode)
        or (after.st_dev, after.st_ino) != expected_identity
    ):
        raise SystemExit(f"{directory} changed during fixture snapshot")

flags = (
    os.O_WRONLY
    | os.O_CREAT
    | os.O_EXCL
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
fd = os.open(output, flags, 0o600)
try:
    body = (json.dumps(snapshot, sort_keys=True, separators=(",", ":")) + "\n").encode()
    view = memoryview(body)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write fixture snapshot")
        view = view[written:]
    os.fsync(fd)
finally:
    os.close(fd)
PY
}

copy_manifest_tree() {
  local output_root="$1"
  python3 - "fixtures/sorafs_manifest" "${output_root}" <<'PY'
from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
if source.is_symlink():
    raise SystemExit(f"{source} must not be a symlink")
try:
    source_stat = source.lstat()
except FileNotFoundError as exc:
    raise SystemExit(f"{source} is missing") from exc
if not stat.S_ISDIR(source_stat.st_mode):
    raise SystemExit(f"{source} must be a directory")
if os.path.lexists(target):
    raise SystemExit(f"{target} already exists")

target_parent = target.parent
if os.path.lexists(target_parent):
    raise SystemExit(f"{target_parent} already exists")
os.mkdir(target_parent, 0o700)
os.mkdir(target, 0o700)

max_copy_file_bytes = 64 << 20
read_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
write_flags = (
    os.O_WRONLY
    | os.O_CREAT
    | os.O_EXCL
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
directory_identities: dict[Path, tuple[int, int]] = {
    source: (source_stat.st_dev, source_stat.st_ino)
}
for current_root, directory_names, file_names in os.walk(source, followlinks=False):
    directory_names.sort()
    file_names.sort()
    current = Path(current_root)
    relative_root = current.relative_to(source)
    destination_root = target / relative_root
    for directory_name in directory_names:
        source_directory = current / directory_name
        metadata = source_directory.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise SystemExit(f"{source_directory} must be a real directory")
        directory_identities[source_directory] = (
            metadata.st_dev,
            metadata.st_ino,
        )
        os.mkdir(destination_root / directory_name, 0o700)
    for file_name in file_names:
        source_path = current / file_name
        destination_path = destination_root / file_name
        before = source_path.lstat()
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
        ):
            raise SystemExit(f"{source_path} must be a single-link regular file")
        if before.st_size > max_copy_file_bytes:
            raise SystemExit(
                f"{source_path} exceeds the {max_copy_file_bytes}-byte copy bound"
            )
        source_fd = os.open(source_path, read_flags)
        try:
            opened = os.fstat(source_fd)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino)
                or before.st_size != opened.st_size
                or before.st_mtime_ns != opened.st_mtime_ns
            ):
                raise SystemExit(f"{source_path} changed while it was opened")
            destination_fd = os.open(destination_path, write_flags, 0o600)
            try:
                byte_length = 0
                while chunk := os.read(
                    source_fd,
                    min(1024 * 1024, max_copy_file_bytes - byte_length + 1),
                ):
                    byte_length += len(chunk)
                    if byte_length > max_copy_file_bytes:
                        raise SystemExit(
                            f"{source_path} exceeds the "
                            f"{max_copy_file_bytes}-byte copy bound"
                        )
                    view = memoryview(chunk)
                    while view:
                        written = os.write(destination_fd, view)
                        if written <= 0:
                            raise OSError(f"failed to copy bytes into {destination_path}")
                        view = view[written:]
                os.fsync(destination_fd)
            finally:
                os.close(destination_fd)
            after = os.fstat(source_fd)
            path_after = source_path.lstat()
            if (
                not stat.S_ISREG(after.st_mode)
                or after.st_nlink != 1
                or (opened.st_dev, opened.st_ino) != (after.st_dev, after.st_ino)
                or opened.st_size != after.st_size
                or opened.st_mtime_ns != after.st_mtime_ns
                or byte_length != after.st_size
                or stat.S_ISLNK(path_after.st_mode)
                or not stat.S_ISREG(path_after.st_mode)
                or path_after.st_nlink != 1
                or (before.st_dev, before.st_ino)
                != (path_after.st_dev, path_after.st_ino)
                or path_after.st_size != after.st_size
                or path_after.st_mtime_ns != after.st_mtime_ns
            ):
                raise SystemExit(f"{source_path} changed while it was copied")
        finally:
            os.close(source_fd)

for directory, expected_identity in directory_identities.items():
    after = directory.lstat()
    if (
        stat.S_ISLNK(after.st_mode)
        or not stat.S_ISDIR(after.st_mode)
        or (after.st_dev, after.st_ino) != expected_identity
    ):
        raise SystemExit(f"{directory} changed during fixture copy")
PY
}

verify_manifest_tree_paths() {
  local fixture_root="$1"
  python3 - "${fixture_root}" <<'PY'
from __future__ import annotations

import os
import stat
import subprocess
import sys
from pathlib import Path

source_root = Path("fixtures/sorafs_manifest")
fixture_root = Path(sys.argv[1])
tracked_output = subprocess.check_output(
    ["git", "ls-files", "-z", "--", str(source_root)]
)
tracked_paths = {
    Path(raw.decode("utf-8")).relative_to(source_root).as_posix()
    for raw in tracked_output.split(b"\0")
    if raw
}
if not tracked_paths:
    raise SystemExit("fixtures/sorafs_manifest has no tracked fixture files")

actual_paths: set[str] = set()
for current_root, directory_names, file_names in os.walk(
    fixture_root,
    followlinks=False,
):
    directory_names.sort()
    file_names.sort()
    current = Path(current_root)
    for directory_name in directory_names:
        directory = current / directory_name
        metadata = directory.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise SystemExit(f"{directory} must be a real directory")
    for file_name in file_names:
        path = current / file_name
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
            raise SystemExit(f"{path} must be a regular non-symlink file")
        actual_paths.add(path.relative_to(fixture_root).as_posix())

if actual_paths != tracked_paths:
    missing = sorted(tracked_paths - actual_paths)
    extra = sorted(actual_paths - tracked_paths)
    raise SystemExit(
        "SoraFS manifest fixture path set differs from git "
        f"(missing={missing}, extra={extra})"
    )
PY
}

cd "${repo_root}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

echo "[sorafs-fixtures] verifying chunker fixtures + signatures"
cargo run --locked -p sorafs_chunker --features dev-tools --bin export_vectors

if ! git diff --quiet -- fixtures/sorafs_chunker; then
  echo "[sorafs-fixtures] error: chunker fixtures changed; regenerate with a council key before committing" >&2
  git diff -- fixtures/sorafs_chunker >&2 || true
  exit 1
fi

echo "[sorafs-fixtures] regenerating provider admission fixtures"
NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked -p sorafs_car --features cli,dev-tools --bin provider_admission_fixtures

if ! git diff --quiet -- fixtures/sorafs_manifest/provider_admission; then
  echo "[sorafs-fixtures] error: provider admission fixtures changed; rerun generator with the council keys" >&2
  git diff -- fixtures/sorafs_manifest/provider_admission >&2 || true
  exit 1
fi

echo "[sorafs-fixtures] regenerating pin registry snapshot fixture"
cargo run --locked -p iroha_core --example gen_pin_snapshot

if ! git diff --quiet -- crates/iroha_core/tests/fixtures/sorafs_pin_registry; then
  echo "[sorafs-fixtures] error: pin registry snapshot changed; run the generator and commit updated fixtures" >&2
  git diff -- crates/iroha_core/tests/fixtures/sorafs_pin_registry >&2 || true
  exit 1
fi

echo "[sorafs-fixtures] verifying closed reference-SDK inventory before regeneration"
python3 scripts/check_sorafs_reference_sdk_fixtures.py

fixture_snapshot_root="$(mktemp -d "${TMPDIR:-/tmp}/sorafs-fixture-snapshots.XXXXXX")"
trap cleanup_fixture_snapshots EXIT
# macOS exposes both /tmp and /var as symlinks. Resolve the private directory
# once so the strict fixture generators can reject symlinked ancestry without
# making the platform's standard temporary directory unusable.
fixture_snapshot_root="$(
  cd -- "${fixture_snapshot_root}"
  pwd -P
)"
verify_manifest_tree_paths "fixtures/sorafs_manifest"
snapshot_manifest_tree \
  "fixtures/sorafs_manifest" \
  "${fixture_snapshot_root}/manifest-checked-in.json"
for fixture_regeneration_pass in 1 2; do
  echo "[sorafs-fixtures] reference-SDK regeneration pass ${fixture_regeneration_pass}/2"
  pass_root="${fixture_snapshot_root}/pass-${fixture_regeneration_pass}/sorafs_manifest"
  copy_manifest_tree "${pass_root}"
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked \
    -p iroha_data_model \
    --features dev-tools,test-fixtures \
    --bin cancel_asset_lock_fixtures \
    -- \
    --output-dir "${pass_root}/appeal_finance"
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked \
    -p sorafs_manifest \
    --features dev-tools \
    --bin generate_pdp_fixtures \
    -- \
    --output-dir "${pass_root}/pdp"
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked \
    -p sorafs_manifest \
    --features dev-tools \
    --bin generate_por_fixtures \
    -- \
    --output-dir "${pass_root}"
  python3 scripts/check_sorafs_reference_sdk_fixtures.py \
    --inventory "${pass_root}/reference_sdk_validation_inventory_v1.json"
  verify_manifest_tree_paths "${pass_root}"
  snapshot_manifest_tree \
    "${pass_root}" \
    "${fixture_snapshot_root}/manifest-pass-${fixture_regeneration_pass}.json"

  if [[ "${fixture_regeneration_pass}" == "1" ]]; then
    if ! cmp -s \
      "${fixture_snapshot_root}/manifest-checked-in.json" \
      "${fixture_snapshot_root}/manifest-pass-1.json"; then
      echo "[sorafs-fixtures] error: reference-SDK fixtures or signed inventory changed" >&2
      diff -u \
        "${fixture_snapshot_root}/manifest-checked-in.json" \
        "${fixture_snapshot_root}/manifest-pass-1.json" >&2 || true
      exit 1
    fi
  fi
done

if ! cmp -s \
  "${fixture_snapshot_root}/manifest-pass-1.json" \
  "${fixture_snapshot_root}/manifest-pass-2.json"; then
  echo "[sorafs-fixtures] error: reference-SDK fixtures are not byte-identical across two regenerations" >&2
  diff -u \
    "${fixture_snapshot_root}/manifest-pass-1.json" \
    "${fixture_snapshot_root}/manifest-pass-2.json" >&2 || true
  exit 1
fi
echo "[sorafs-fixtures] reference-SDK fixtures and signed inventory are deterministic"

# Run parity tests to ensure all generated bindings remain aligned.
cargo test --locked -p sorafs_chunker --test vectors --quiet

# Verify canonical handles are published everywhere.
python3 <<'PY'
import json
import os
import stat
from pathlib import Path

CANONICAL = "sorafs.sf1@1.0.0"

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def fail(path: Path, message: str) -> None:
    raise SystemExit(f"{path} {message}")

def read_json_no_follow(path: Path) -> dict:
    if path.is_symlink():
        fail(path, "must not be a symlink")
    try:
        path_stat = path.lstat()
    except FileNotFoundError:
        fail(path, "is missing")
    if not stat.S_ISREG(path_stat.st_mode):
        fail(path, "must be a regular file")
    fd = os.open(path, read_open_flags())
    try:
        descriptor_stat = os.fstat(fd)
        if not stat.S_ISREG(descriptor_stat.st_mode):
            fail(path, "must be a regular file")
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            fd = -1
            return json.load(handle)
    finally:
        if fd >= 0:
            os.close(fd)

def expect_aliases(path: Path) -> None:
    data = read_json_no_follow(path)
    aliases = data.get("profile_aliases")
    if not isinstance(aliases, list):
        raise SystemExit(f"{path} missing profile_aliases")
    if CANONICAL not in aliases:
        raise SystemExit(f"{path} missing canonical handle {CANONICAL}")

fixtures_dir = Path("fixtures/sorafs_chunker")
expect_aliases(fixtures_dir / "sf1_profile_v1.json")
expect_aliases(fixtures_dir / "manifest_signatures.json")
expect_aliases(fixtures_dir / "manifest_blake3.json")

backpressure = Path("fuzz/sorafs_chunker/sf1_profile_v1_backpressure.json")
expect_aliases(backpressure)
PY

require_fixture_tool node "SF1 vector parity"
echo "[sorafs-fixtures] running SF1 vector parity (Node)"
node scripts/check_sf1_vectors.mjs

echo "[sorafs-fixtures] running 1 GiB chunker regression (Rust)"
cargo test --locked -p sorafs_chunker --test one_gib -- --ignored

require_fixture_tool go "1 GiB Go regression"
echo "[sorafs-fixtures] running 1 GiB chunker regression (Go)"
go_cache="${repo_root}/target/go-cache"
go_mod_cache="${repo_root}/target/go-mod-cache"
go_tmp="${repo_root}/target/go-tmp"
go_path="${repo_root}/target/go"
mkdir -p "${go_cache}" "${go_mod_cache}" "${go_tmp}" "${go_path}"
(
  cd fixtures/sorafs_chunker
  SORAFS_HEAVY=1 \
  GOCACHE="${go_cache}" \
  GOMODCACHE="${go_mod_cache}" \
  GOPATH="${go_path}" \
  TMPDIR="${go_tmp}" \
  GOTMPDIR="${go_tmp}" \
    go test ./...
)

require_fixture_tool node "1 GiB Node regression"
echo "[sorafs-fixtures] running 1 GiB chunker regression (Node)"
(
  cd javascript/iroha_js
  node scripts/run-test-profile.mjs heavy
)

echo "[sorafs-fixtures] fixtures stable and signatures verified"
