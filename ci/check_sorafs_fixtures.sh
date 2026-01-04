#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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

# Run parity tests to ensure all generated bindings remain aligned.
cargo test -p sorafs_chunker --test vectors --quiet

# Verify canonical handles are published everywhere.
python3 <<'PY'
import json
from pathlib import Path

CANONICAL = "sorafs.sf1@1.0.0"

def expect_aliases(path: Path) -> None:
    data = json.loads(path.read_text())
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

echo "[sorafs-fixtures] running 1 GiB chunker regression (Rust)"
cargo test --locked -p sorafs_chunker --test one_gib -- --ignored

if command -v go >/dev/null 2>&1; then
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
else
  echo "[sorafs-fixtures] skipping Go regression (go not available)"
fi

if command -v node >/dev/null 2>&1; then
  echo "[sorafs-fixtures] running 1 GiB chunker regression (Node)"
  (
    cd javascript/iroha_js
    SORAFS_HEAVY=1 node --test test/sorafsChunker.oneGib.test.js
  )
else
  echo "[sorafs-fixtures] skipping Node regression (node not available)"
fi

echo "[sorafs-fixtures] fixtures stable and signatures verified"
