#!/usr/bin/env bash
set -euo pipefail

# Local runner for the Norito feature matrix (subset of CI).
#
# Usage: scripts/run_norito_feature_matrix.sh [--fast] [--downstream [crate]]
#  --fast             Run a small subset: layout=both_on only; compact on/off
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

BASE_FEATURES=(derive compression json columnar)

layouts=(both_on no_packed_seq no_packed_struct none)
compacts=(true false)

if [[ $FAST -eq 1 ]]; then
  layouts=(both_on)
fi

run_case() {
  local layout="$1" compact="$2"
  local features=("${BASE_FEATURES[@]}")
  if [[ "$compact" == "true" ]]; then
    features+=(compact-len)
  fi
  case "$layout" in
    both_on)
      features+=(packed-seq packed-struct)
      ;;
    no_packed_seq)
      features+=(packed-struct)
      ;;
    no_packed_struct)
      features+=(packed-seq)
      ;;
    none)
      ;;
    *)
      echo "Unknown layout: $layout" >&2; return 1;;
  esac
  local feature_str
  feature_str=$(IFS=,; echo "${features[*]}")
  echo "==> norito: layout=$layout compact=$compact features=[$feature_str]"
  cargo test -p norito --no-default-features --features "$feature_str" -- --nocapture
  if [[ $DOWNSTREAM -eq 1 ]]; then
    echo "==> downstream smoke: $DOWNSTREAM_CRATE"
    cargo test -p "$DOWNSTREAM_CRATE" -- --nocapture
  fi
}

for layout in "${layouts[@]}"; do
  for compact in "${compacts[@]}"; do
    run_case "$layout" "$compact"
  done
done

echo "norito feature matrix: OK"
