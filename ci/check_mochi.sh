#!/usr/bin/env bash
set -euo pipefail

# Gate MOCHI changes with targeted cargo checks/tests for every crate.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

packages=(
  mochi-core
  mochi-ui-egui
  mochi-integration
)

for package in "${packages[@]}"; do
  echo "[mochi] cargo check -p ${package}"
  cargo check -p "${package}"

  echo "[mochi] cargo test -p ${package}"
  cargo test -p "${package}"
done

echo "[mochi] cargo gating complete"
