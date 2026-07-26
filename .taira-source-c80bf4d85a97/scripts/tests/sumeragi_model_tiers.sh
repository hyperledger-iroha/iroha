#!/bin/bash
set -euo pipefail

mode="${1:-fast}"
shift || true

cargo_bin="${CARGO_BIN:-cargo}"

run_fast() {
  "$cargo_bin" test -p iroha_core --lib state_machine_model_tests:: -- --nocapture "$@"
  "$cargo_bin" test -p iroha_core --lib state_machine_fairness_model_tests:: -- --nocapture "$@"
}

run_deep() {
  "$cargo_bin" test -p iroha_core --lib state_machine_model_tests:: -- --nocapture "$@"
  "$cargo_bin" test -p iroha_core --lib state_machine_fairness_model_tests:: -- --nocapture "$@"
  "$cargo_bin" test -p iroha_core --lib state_machine_fairness_model_tests:: -- --ignored --nocapture "$@"
}

case "$mode" in
  fast)
    run_fast "$@"
    ;;
  deep)
    run_deep "$@"
    ;;
  *)
    echo "usage: $0 {fast|deep} [extra test args]" >&2
    exit 2
    ;;
esac
