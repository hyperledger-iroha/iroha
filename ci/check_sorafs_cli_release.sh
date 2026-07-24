#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${repo_root}/.target}"

echo "[sorafs-release] fmt check (workspace)"
cargo fmt --all -- --check

echo "[sorafs-release] shell syntax checks"
bash -n scripts/release_sorafs_cli.sh scripts/package_sorafs_validate_release.sh \
  scripts/build_release_bundle.sh scripts/build_release_image.sh \
  scripts/tests/release_manifest_signing_test.sh \
  ci/check_sorafs_reference_ffi_header.sh

echo "[sorafs-release] reference FFI header contract"
ci/check_sorafs_reference_ffi_header.sh

echo "[sorafs-release] release helper adversarial tests"
python3 scripts/check_workflow_action_pins.py
python3 -m pytest -q \
  scripts/tests/check_workflow_action_pins_test.py \
  scripts/tests/check_sorafs_release_automation_test.py \
  scripts/tests/check_sorafs_release_version_map_test.py \
  scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py \
  scripts/tests/build_sorafs_reference_sdk_release_canary_test.py \
  scripts/tests/run_sorafs_reference_sdk_release_evidence_test.py \
  scripts/tests/release_profile_validation_test.py \
  scripts/tests/release_manifest_signing_test.py \
  scripts/tests/generate_release_manifest_test.py \
  scripts/tests/generate_sorafs_cli_release_manifest_test.py \
  scripts/tests/publish_plan_test.py \
  scripts/tests/release_sorafs_cli_test.py \
  scripts/tests/package_sorafs_cli_candidate_test.py \
  scripts/tests/package_sorafs_validate_release_test.py \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_shell_helpers_use_hardened_release_and_no_follow_io \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_validate_release_packager_rejects_symlink_stage_entries \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_cli_release_gate_runs_helper_adversarial_tests
scripts/tests/release_manifest_signing_test.sh

echo "[sorafs-release] clippy sorafs_orchestrator (sorafs_cli)"
cargo clippy --locked -p sorafs_orchestrator --all-targets -- -D warnings

echo "[sorafs-release] clippy sorafs_car helpers (cli feature)"
cargo clippy --locked -p sorafs_car --features cli --all-targets -- -D warnings

echo "[sorafs-release] clippy sorafs_manifest"
cargo clippy --locked -p sorafs_manifest --all-targets -- -D warnings

echo "[sorafs-release] clippy sorafs_chunker"
cargo clippy --locked -p sorafs_chunker --all-targets -- -D warnings

echo "[sorafs-release] tests sorafs_orchestrator (sorafs_cli)"
cargo test --locked -p sorafs_orchestrator --test sorafs_cli

echo "[sorafs-release] tests sorafs_car helpers (cli feature)"
cargo test --locked -p sorafs_car --features cli --all-targets

echo "[sorafs-release] tests sorafs_manifest"
cargo test --locked -p sorafs_manifest --all-targets

echo "[sorafs-release] tests sorafs_chunker"
cargo test --locked -p sorafs_chunker --all-targets

echo "[sorafs-release] release verification complete"
