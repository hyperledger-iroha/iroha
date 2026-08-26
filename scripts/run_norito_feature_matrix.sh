#!/usr/bin/env bash
set -euo pipefail

# Local runner for the Norito feature matrix (subset of CI).
#
# Usage: scripts/run_norito_feature_matrix.sh [--fast] [--downstream [crate]]
#  --fast             Run only the canonical node codec surface
#  --downstream <cr>  Also run `cargo test -p <cr>` after each Norito run (default: iroha_data_model)

FAST=0
DOWNSTREAM=0
DOWNSTREAM_CRATE="iroha_data_model"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --fast)
      FAST=1; shift ;;
    --downstream)
      DOWNSTREAM=1; shift
      if [[ $# -gt 0 && "$1" != --* ]]; then
        DOWNSTREAM_CRATE="$1"; shift
      fi ;;
    *)
      echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done

feature_sets=(base-codec,compression node-codec)

if [[ $FAST -eq 1 ]]; then
  feature_sets=(node-codec)
fi

run_case() {
  local features="$1"
  echo "==> norito: features=[$features]"
  cargo test -p norito --no-default-features --features "$features" -- --nocapture
  if [[ $DOWNSTREAM -eq 1 ]]; then
    echo "==> downstream smoke: $DOWNSTREAM_CRATE"
    cargo test -p "$DOWNSTREAM_CRATE" -- --nocapture
  fi
}

for features in "${feature_sets[@]}"; do
  run_case "$features"
done

echo "norito feature matrix: OK"
