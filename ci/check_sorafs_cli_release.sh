#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${repo_root}/.target}"

echo "[sorafs-release] fmt check (workspace)"
cargo fmt --all -- --check

echo "[sorafs-release] clippy sorafs_car (cli feature)"
cargo clippy --locked -p sorafs_car --features cli --all-targets -- -D warnings

echo "[sorafs-release] clippy sorafs_manifest"
cargo clippy --locked -p sorafs_manifest --all-targets -- -D warnings

echo "[sorafs-release] clippy sorafs_chunker"
cargo clippy --locked -p sorafs_chunker --all-targets -- -D warnings

echo "[sorafs-release] tests sorafs_car (cli feature)"
cargo test --locked -p sorafs_car --features cli --all-targets

echo "[sorafs-release] tests sorafs_manifest"
cargo test --locked -p sorafs_manifest --all-targets

echo "[sorafs-release] tests sorafs_chunker"
cargo test --locked -p sorafs_chunker --all-targets

echo "[sorafs-release] release verification complete"
