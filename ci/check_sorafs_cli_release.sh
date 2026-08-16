#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

echo "[sorafs-release] build-efficiency provenance check"
python3 -I -S scripts/check_build_efficiency_provenance.py

echo "[sorafs-release] source-file budget check"
python3 scripts/check_source_file_budget.py --require-objective

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${repo_root}/.target}"

cargo_lock_sha256() {
  python3 -I -S - Cargo.lock <<'PY'
import hashlib
import os
import stat
import sys

path = sys.argv[1]
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
try:
    fd = os.open(path, flags)
except OSError as error:
    raise SystemExit(f"workspace Cargo.lock is unavailable: {error}") from None
try:
    before = os.fstat(fd)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SystemExit("workspace Cargo.lock must be a singly linked regular file")
    if before.st_size <= 0 or before.st_size > 2 * 1024 * 1024:
        raise SystemExit("workspace Cargo.lock has an invalid size")
    digest = hashlib.sha256()
    remaining = before.st_size
    while remaining:
        chunk = os.read(fd, min(64 * 1024, remaining))
        if not chunk:
            raise SystemExit("workspace Cargo.lock was truncated while read")
        digest.update(chunk)
        remaining -= len(chunk)
    if os.read(fd, 1):
        raise SystemExit("workspace Cargo.lock grew while read")
    after = os.fstat(fd)
    identity = lambda value: (
        value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
        value.st_size, value.st_mtime_ns, value.st_ctime_ns,
    )
    if identity(before) != identity(after):
        raise SystemExit("workspace Cargo.lock changed while read")
    print(digest.hexdigest())
finally:
    os.close(fd)
PY
}

expected_cargo_lock_sha256="$(cargo_lock_sha256)"

echo "[sorafs-release] fmt check (workspace)"
cargo fmt --all -- --check

echo "[sorafs-release] shell syntax checks"
bash -n scripts/release_sorafs_cli.sh scripts/package_sorafs_validate_release.sh \
  scripts/build_line.sh scripts/build_release_bundle.sh scripts/build_release_image.sh \
  configs/sorafs/external_software_signer/launchd/sorafs-external-software-signer-launchd-v1 \
  python/iroha_python/scripts/release_smoke.sh \
  scripts/tests/release_manifest_signing_test.sh \
  ci/check_sorafs_reference_ffi_header.sh

echo "[sorafs-release] reference FFI header contract"
ci/check_sorafs_reference_ffi_header.sh

echo "[sorafs-release] release helper adversarial tests"
python3 scripts/check_workflow_action_pins.py
python3 -m pytest -q \
  scripts/tests/check_workflow_action_pins_test.py \
  scripts/tests/check_sorafs_release_automation_test.py \
  scripts/tests/check_build_efficiency_provenance_test.py \
  scripts/tests/check_sorafs_release_version_map_test.py \
  scripts/tests/check_sorafs_provider_ingest_runtime_contract_test.py \
  scripts/tests/build_sorafs_reference_sdk_supply_chain_sources_test.py \
  scripts/tests/sorafs_reference_sdk_supply_chain_test.py \
  scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py \
  scripts/tests/build_sorafs_reference_sdk_release_canary_test.py \
  scripts/tests/build_sorafs_foundational_prerequisite_test.py \
  scripts/tests/check_sorafs_ai_prescreen_rollout_evidence_test.py \
  scripts/tests/check_sorafs_appeal_finance_rollout_evidence_test.py \
  scripts/tests/check_sorafs_gateway_compliance_rollout_evidence_test.py \
  scripts/tests/check_sorafs_gateway_load_rollout_evidence_test.py \
  scripts/tests/check_sorafs_governance_dag_rollout_evidence_test.py \
  scripts/tests/check_sorafs_hedging_rollout_evidence_test.py \
  scripts/tests/check_sorafs_l1_deployment_qualification_test.py \
  scripts/tests/check_sorafs_l1_resilience_qualification_test.py \
  scripts/tests/check_sorafs_moderation_panel_rollout_evidence_test.py \
  scripts/tests/check_sorafs_orderbook_rollout_evidence_test.py \
  scripts/tests/check_sorafs_pdp_rollout_evidence_test.py \
  scripts/tests/check_sorafs_pop_credentials_rollout_evidence_test.py \
  scripts/tests/check_sorafs_por_rollout_evidence_test.py \
  scripts/tests/check_sorafs_potr_rollout_evidence_test.py \
  scripts/tests/check_sorafs_production_readiness_test.py \
  scripts/tests/run_sorafs_production_readiness_test.py \
  scripts/tests/run_sorafs_production_readiness_negative_archive_test.py \
  scripts/tests/check_sorafs_production_promotion_bundle_test.py \
  scripts/tests/check_sorafs_repair_rollout_evidence_test.py \
  scripts/tests/check_sorafs_reputation_rollout_evidence_test.py \
  scripts/tests/check_sorafs_reserve_rent_rollout_evidence_test.py \
  scripts/tests/check_sorafs_transparency_rollout_evidence_test.py \
  scripts/tests/sorafs_archive_path_components_test.py \
  scripts/tests/sorafs_evidence_json_test.py \
  scripts/tests/sorafs_l1_lane_evidence_inventory_test.py \
  scripts/tests/sorafs_required_kinds_test.py \
  scripts/tests/sorafs_response_args_test.py \
  scripts/tests/sorafs_topology_qualification_test.py \
  scripts/tests/run_sorafs_reference_sdk_release_evidence_test.py \
  scripts/tests/release_profile_validation_test.py \
  scripts/tests/release_manifest_signing_test.py \
  scripts/tests/generate_release_manifest_test.py \
  scripts/tests/generate_sorafs_cli_release_manifest_test.py \
  scripts/tests/publish_plan_test.py \
  scripts/tests/release_sorafs_cli_test.py \
  scripts/tests/package_sorafs_cli_candidate_test.py \
  scripts/tests/build_release_bundle_test.py \
  scripts/tests/build_release_image_test.py \
  scripts/tests/package_sorafs_validate_release_test.py \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_production_readiness_aggregate_gate_is_documented \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_pdp_provider_protocol_and_chain_repair_boundary_are_documented \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_repair_chain_authority_is_closed_and_live_evidence_stays_open_in_docs \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_reserve_rent_chain_authoritative_contract_stays_open_until_evidence \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_release_http_clients_do_not_follow_redirects \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_shell_helpers_use_hardened_release_and_no_follow_io \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_validate_release_packager_rejects_symlink_stage_entries \
  scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_sorafs_cli_release_gate_runs_helper_adversarial_tests
scripts/tests/release_manifest_signing_test.sh

echo "[sorafs-release] repair, reserve, redirect, and provider-ingest contracts"
cargo test --locked -p iroha --lib client::repair::tests -- --nocapture
cargo test --locked -p iroha --lib client::reserve::tests -- --nocapture
cargo test --locked -p iroha --lib does_not_follow_signed_body_redirects -- --nocapture
provider_ingest_test="sorafs_provider_ingest_runtime::tests::quarantine_restart::post_admission_quarantine_survives_restart_with_shared_chunks"
provider_ingest_list="$(
  cargo test --locked -p irohad --lib "${provider_ingest_test}" -- --exact --list
)"
if [[ "$(grep -Fxc -- "${provider_ingest_test}: test" <<<"${provider_ingest_list}" || true)" != 1 ]]; then
  echo "provider-ingest crash/restart contract must expose exactly one runnable test" >&2
  exit 1
fi
cargo test --locked -p irohad --lib "${provider_ingest_test}" -- \
  --exact --include-ignored --nocapture

echo "[sorafs-release] external software signer protocol and CLI tests"
cargo test --locked -p irohad --lib external_software_signer
cargo test --locked -p irohad --features external-software-signer-bin \
  --bin sorafs_external_software_signer

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

if [[ "$(cargo_lock_sha256)" != "${expected_cargo_lock_sha256}" ]]; then
  echo "workspace Cargo.lock changed during the release gate" >&2
  exit 1
fi
echo "[sorafs-release] release verification complete"
