#!/usr/bin/env bash
set -euo pipefail

# Gate MOCHI changes with targeted cargo checks/tests for every crate.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

packages=(
  mochi-core
  mochi-integration
)

for package in "${packages[@]}"; do
  echo "[mochi] cargo check -p ${package}"
  cargo check -p "${package}"

  echo "[mochi] cargo test -p ${package}"
  cargo test -p "${package}"
done

echo "[mochi] cargo test -p mochi-integration --features dev-tools --test supervisor"
cargo test -p mochi-integration --features dev-tools --test supervisor

echo "[mochi] resolve Cargo target directory"
mochi_target_dir="$(
  cargo metadata --locked --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
)"
if [[ -z "$mochi_target_dir" || "$mochi_target_dir" != /* ]]; then
  echo "[mochi] Cargo returned an invalid target directory: ${mochi_target_dir}" >&2
  exit 1
fi

echo "[mochi] cargo build -p iroha_kagami --bin kagami"
cargo build --locked -p iroha_kagami --bin kagami
kagami_executable="kagami"
if [[ "${OS:-}" == "Windows_NT" ]]; then
  kagami_executable="kagami.exe"
fi
kagami_binary="${mochi_target_dir}/debug/${kagami_executable}"
if [[ ! -x "$kagami_binary" ]]; then
  echo "[mochi] source-built Kagami is unavailable at ${kagami_binary}" >&2
  exit 1
fi

echo "[mochi] real-Kagami SupervisorBuilder Hijiri regression"
MOCHI_REAL_KAGAMI="$kagami_binary" cargo test --locked \
  -p mochi-integration \
  --features dev-tools \
  --test supervisor \
  supervisor_real_kagami_preserves_first_release_hijiri_bootstrap \
  -- --exact --ignored

echo "[mochi] cargo check -p mochi-ui --features gui --bin mochi"
cargo check -p mochi-ui --features gui --bin mochi

echo "[mochi] cargo test -p mochi-ui --features gui --bin mochi"
cargo test -p mochi-ui --features gui --bin mochi

echo "[mochi] bash -n scripts/mochi_local_sandbox.sh"
bash -n scripts/mochi_local_sandbox.sh

echo "[mochi] cargo gating complete"
