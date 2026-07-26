#!/usr/bin/env bash
# Verify checked-in SoraFS fixtures, signatures, SDK inventory, and heavy
# cross-language vectors. Requires Cargo, Python 3, and the repository's pinned
# dependencies. Node.js and Go are mandatory when
# SORAFS_FIXTURE_REQUIRE_TOOLCHAIN=1 (the release/nightly workflow sets it);
# otherwise their heavyweight local-only checks are reported and skipped.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_snapshot_root=""

usage() {
  cat <<'EOF'
Usage: ci/check_sorafs_fixtures.sh

Verify the complete checked-in SoraFS fixture set. The command is read-only
when the fixtures are current; generators run in place and any byte drift,
missing input, or second-run difference fails the command.

Environment:
  CARGO_NET_OFFLINE                    Cargo offline mode (default: true).
  CARGO_TERM_COLOR                     Cargo colour setting (default: never).
  SORAFS_FIXTURE_REQUIRE_TOOLCHAIN     Set to 1 to require Node.js and Go.
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

require_fixture_toolchain="${SORAFS_FIXTURE_REQUIRE_TOOLCHAIN:-0}"
case "${require_fixture_toolchain}" in
  0|1) ;;
  *)
    echo "[sorafs-fixtures] error: SORAFS_FIXTURE_REQUIRE_TOOLCHAIN must be 0 or 1" >&2
    exit 2
    ;;
esac

cleanup_fixture_snapshots() {
  if [[ -n "${fixture_snapshot_root}" && -d "${fixture_snapshot_root}" ]]; then
    rm -rf -- "${fixture_snapshot_root}"
  fi
}

fixture_tool_available() {
  local tool_name="$1"
  local check_label="$2"
  if command -v "${tool_name}" >/dev/null 2>&1; then
    return 0
  fi
  if [[ "${require_fixture_toolchain}" == "1" ]]; then
    echo "[sorafs-fixtures] error: ${check_label} requires ${tool_name}" >&2
    exit 1
  fi
  echo "[sorafs-fixtures] skipping ${check_label} (${tool_name} not available)" >&2
  return 1
}

snapshot_manifest_tree() {
  local output_path="$1"
  python3 - "fixtures/sorafs_manifest" "${output_path}" <<'PY'
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

snapshot: dict[str, dict[str, object]] = {}
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
    for file_name in file_names:
        path = current / file_name
        relative = path.relative_to(root).as_posix()
        path_stat = path.lstat()
        if stat.S_ISLNK(path_stat.st_mode):
            raise SystemExit(f"{path} must not be a symlink")
        if not stat.S_ISREG(path_stat.st_mode):
            raise SystemExit(f"{path} must be a regular file")
        digest = hashlib.sha256()
        with path.open("rb") as fixture:
            while chunk := fixture.read(1024 * 1024):
                digest.update(chunk)
        snapshot[relative] = {
            "byte_length": path_stat.st_size,
            "sha256": digest.hexdigest(),
        }

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

cd "${repo_root}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

echo "[sorafs-fixtures] verifying chunker fixtures + signatures"
cargo run --locked -p sorafs_chunker --bin export_vectors

if ! git diff --quiet -- fixtures/sorafs_chunker; then
  echo "[sorafs-fixtures] error: chunker fixtures changed; regenerate with a council key before committing" >&2
  git diff -- fixtures/sorafs_chunker >&2 || true
  exit 1
fi

echo "[sorafs-fixtures] regenerating provider admission fixtures"
NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked -p sorafs_car --features cli --bin provider_admission_fixtures

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
for fixture_regeneration_pass in 1 2; do
  echo "[sorafs-fixtures] reference-SDK regeneration pass ${fixture_regeneration_pass}/2"
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked \
    -p iroha_data_model \
    --features test-fixtures \
    --bin cancel_asset_lock_fixtures
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run --locked \
    -p sorafs_manifest \
    --bin generate_por_fixtures
  python3 scripts/check_sorafs_reference_sdk_fixtures.py
  snapshot_manifest_tree \
    "${fixture_snapshot_root}/manifest-pass-${fixture_regeneration_pass}.json"

  if [[ "${fixture_regeneration_pass}" == "1" ]]; then
    fixture_changes="$(git status --short --untracked-files=all -- fixtures/sorafs_manifest)"
    if [[ -n "${fixture_changes}" ]]; then
      echo "[sorafs-fixtures] error: reference-SDK fixtures or signed inventory changed" >&2
      printf '%s\n' "${fixture_changes}" >&2
      git diff -- fixtures/sorafs_manifest >&2 || true
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
cargo test -p sorafs_chunker --test vectors --quiet

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

if fixture_tool_available node "SF1 vector parity"; then
  echo "[sorafs-fixtures] running SF1 vector parity (Node)"
  node scripts/check_sf1_vectors.mjs
fi

echo "[sorafs-fixtures] running 1 GiB chunker regression (Rust)"
cargo test --locked -p sorafs_chunker --test one_gib -- --ignored

if fixture_tool_available go "1 GiB Go regression"; then
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
fi

if fixture_tool_available node "1 GiB Node regression"; then
  echo "[sorafs-fixtures] running 1 GiB chunker regression (Node)"
  (
    cd javascript/iroha_js
    SORAFS_HEAVY=1 node --test test/sorafsChunker.oneGib.test.js
  )
fi

echo "[sorafs-fixtures] fixtures stable and signatures verified"
