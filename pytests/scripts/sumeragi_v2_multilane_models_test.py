"""Negative controls for the multilane model/source-binding contract."""

from __future__ import annotations

import copy
import importlib.util
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER = (
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_multilane_models.py"
)
BINDINGS = (
    ROOT_DIR
    / "formal"
    / "sumeragi_v2"
    / "multilane_source_bindings.json"
)


def load_checker():
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_multilane_models", CHECKER
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_reviewed_rust_source_fixture(
    tmp_path: Path, module, relative: str
) -> Path:
    """Copy and track one parent's exact recursive reviewed include closure."""
    parent_relative = Path(relative)
    errors: list[str] = []
    expanded = module._expanded_source_manifest_paths(
        {parent_relative}, ROOT_DIR, errors
    )
    assert errors == []
    for component_relative in expanded:
        source = ROOT_DIR / component_relative
        if component_relative.suffix != ".rs" or not source.is_file():
            continue
        destination = tmp_path / component_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    initialize_git_fixture(tmp_path)
    return tmp_path / parent_relative


def initialize_git_fixture(root: Path, tracked: tuple[str, ...] | None = None) -> None:
    environment = os.environ.copy()
    environment.pop("GIT_INDEX_FILE", None)
    subprocess.run(
        ["git", "init", "-q"],
        cwd=root, check=True, env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    paths = tracked if tracked is not None else (".",)
    subprocess.run(
        ["git", "add", "--", *paths],
        cwd=root, check=True, env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def test_reviewed_rust_include_manifest_is_pinned_and_current() -> None:
    module = load_checker()
    errors: list[str] = []
    module._validate_reviewed_rust_include_manifest(ROOT_DIR, errors)
    assert errors == []


def test_reviewed_rust_source_expands_exact_lane_work_closure(
    tmp_path: Path,
) -> None:
    module = load_checker()
    relative = "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    copy_reviewed_rust_source_fixture(tmp_path, module, relative)
    errors: list[str] = []
    _path, source = module._read_reviewed_rust_source(
        tmp_path, relative, "reviewed lane-work fixture", errors
    )
    assert errors == []
    assert source is not None
    assert source.count("fn typed_finality_handoff_fences_changed_roster") == 1
    assert (
        source.count("fn historical_certificate_payload_corruption_is_fail_stop")
        == 1
    )
    expanded_paths = module._expanded_source_manifest_paths({Path(relative)})
    assert module.REVIEWED_RUST_SOURCE_HELPER_RELATIVE in expanded_paths
    assert module.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE in expanded_paths
    assert (
        Path(relative).parent
        / "v2_lane_work/typed_finality_handoff_tests.rs"
        in expanded_paths
    )
    assert (
        Path(relative).parent
        / "v2_lane_work/historical_recovery_and_carrier_tests.rs"
        in expanded_paths
    )


def test_reviewed_rust_source_rejects_substituted_lane_work_include(
    tmp_path: Path,
) -> None:
    module = load_checker()
    relative = "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    parent = copy_reviewed_rust_source_fixture(tmp_path, module, relative)
    canonical = 'include!("v2_lane_work/typed_finality_handoff_tests.rs");'
    substitute = 'include!("v2_lane_work/substituted_handoff_tests.rs");'
    replace_once(parent, canonical, substitute)
    shutil.copy2(
        parent.parent / "v2_lane_work/typed_finality_handoff_tests.rs",
        parent.parent / "v2_lane_work/substituted_handoff_tests.rs",
    )
    errors: list[str] = []
    _path, source = module._read_reviewed_rust_source(
        tmp_path, relative, "substituted lane-work fixture", errors
    )
    assert source is None
    assert any("reviewed Rust include inventory must equal" in error for error in errors)


def test_reviewed_rust_source_rejects_symlinked_lane_work_component(
    tmp_path: Path,
) -> None:
    module = load_checker()
    relative = "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    parent = copy_reviewed_rust_source_fixture(tmp_path, module, relative)
    component = parent.parent / "v2_lane_work/typed_finality_handoff_tests.rs"
    component.unlink()
    component.symlink_to("frozen_context_pop_tests.rs")
    errors: list[str] = []
    _path, source = module._read_reviewed_rust_source(
        tmp_path, relative, "symlinked lane-work fixture", errors
    )
    assert source is None
    assert any(
        str(component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors


def canonical_contract() -> dict:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["inflight_first_release_layout_contract"])


def canonical_models() -> list[dict]:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["models"])


def copy_stable_generation_diagnostics_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    relatives = (
        Path("crates/iroha_core/src/state.rs"),
        Path("crates/iroha_core/src/state/diagnostic_state_generation.rs"),
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, set(relatives))
    return tmp_path / relatives[0], tmp_path / relatives[1]


def validate_stable_generation_diagnostics_fixture(
    tmp_path: Path, module
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_stable_generation_diagnostics_contract(
        tmp_path,
        canonical_models(),
        errors,
    )
    return tuple(errors)


def canonical_kura_retention_contract() -> dict:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["kura_replica_retention_contract"])


def copy_kura_retention_fixture(tmp_path: Path, module) -> dict:
    """Copy every file consumed by the isolated Kura retention validator."""
    contract = canonical_kura_retention_contract()
    relatives = {
        module.FORMAL_RELATIVE / f"{contract['module']}.tla",
        module.FORMAL_RELATIVE / contract["positive_config"],
    }
    relatives.update(
        module.FORMAL_RELATIVE / mutation["config"]
        for mutation in contract["mutations"]
    )
    relatives.update(
        Path(binding["path"]) for binding in contract["production_symbols"]
    )
    relatives.update(
        Path(check["path"]) for check in contract["ordered_source_checks"]
    )
    relatives.update(
        Path(relative)
        for relative, _kind, _symbol, _tokens in (
            module.KURA_RETENTION_REQUIRED_BINDINGS
        )
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return contract


def validate_kura_retention_fixture(
    tmp_path: Path, module, contract: dict
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_kura_replica_retention_contract(
        tmp_path,
        tmp_path / module.FORMAL_RELATIVE,
        contract,
        errors,
    )
    return tuple(errors)


def test_kura_replica_retention_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    assert validate_kura_retention_fixture(tmp_path, module, contract) == ()


def test_kura_replica_retention_contract_rejects_unsigned_identity_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/message.rs"
    replace_once(
        path,
        "finality_artifact_hash: self.finality_artifact_hash,",
        "finality_artifact_hash: self.block_hash.cast(),",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "KuraReplicaAdvertV1::signature_preimage" in error
        and "finality_artifact_hash: self.finality_artifact_hash" in error
        for error in errors
    ), errors
def test_kura_replica_retention_contract_rejects_final_prestage_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    swap_ordered_once(
        path,
        "current_authority.key == *expected_authority",
        "&& self.has_all_selected_remote_keepers(",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any("Kura final pre-stage recheck token" in error for error in errors), errors


def test_kura_replica_retention_contract_rejects_relayed_ingress_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs"
    )
    replace_once(
        path,
        "authenticated_via.as_ref() != Some(&advertised_keeper)",
        "authenticated_via.is_none()",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "admit_kura_replica_advert_ingress" in error
        and "authenticated_via.as_ref() != Some(&advertised_keeper)" in error
        for error in errors
    ), errors


def test_kura_replica_retention_contract_rejects_transport_model_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / module.FORMAL_RELATIVE
        / "SumeragiV2KuraReplicaRetention.tla"
    )
    replace_once(path, "THEN NonSignerKeeper", "THEN keeper")
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "transport/capacity token" in error and "THEN NonSignerKeeper" in error
        for error in errors
    ), errors


def test_kura_replica_retention_contract_rejects_unchecked_capacity_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_config/src/parameters/actual.rs"
    replace_once(
        path,
        ".checked_add(evictable_window.get())",
        ".saturating_add(evictable_window.get())",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "kura_replica_advert_registry_key_capacity" in error
        and ".checked_add(evictable_window.get())" in error
        for error in errors
    ), errors


def test_kura_replica_retention_contract_rejects_ttl_floor_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_config/src/parameters/actual.rs"
    replace_once(
        path,
        "pub const KURA_REPLICA_ADVERT_TTL_MIN: Duration = Duration::from_millis(2);",
        "pub const KURA_REPLICA_ADVERT_TTL_MIN: Duration = Duration::from_millis(1);",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any("exact reviewed two-millisecond TTL floor" in error for error in errors), errors


def test_kura_replica_retention_contract_rejects_unbounded_refresh_turn(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_worker/"
        / "kura_replica_advert_refresh.rs"
    )
    replace_once(
        path,
        "KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN: usize = 8;",
        "KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN: usize = usize::MAX;",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any("exact reviewed eight-probe contract" in error for error in errors), errors


def test_kura_replica_retention_contract_rejects_refresh_owner_minimum_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_worker/"
        / "kura_replica_advert_refresh.rs"
    )
    replace_once(
        path,
        "refresh_interval < KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN",
        "refresh_interval.is_zero()",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "KuraReplicaAdvertRefreshOwner::new" in error
        and "KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN" in error
        for error in errors
    ), errors


def test_kura_replica_retention_contract_rejects_scan_deadline_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_worker/"
        / "kura_replica_advert_refresh.rs"
    )
    swap_ordered_once(
        path,
        "let next_cycle_at = now.checked_add(self.refresh_interval).ok_or_else(|| {",
        "state.cursor = Some(KuraReplicaAdvertRefreshCursor::new(",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any("Kura refresh scan-start deadline token" in error for error in errors), errors


def test_kura_replica_retention_contract_rejects_rollover_wakeup_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    swap_ordered_once(
        path,
        "let retired = pending.handoff_applied_height_to_durable_reconstruction(",
        "let scheduled_kura_replica_adverts = self",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any("Kura durable-handoff scheduling token" in error for error in errors), errors


def test_kura_replica_retention_contract_rejects_tip_hash_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_worker/"
        / "kura_replica_advert_refresh.rs"
    )
    replace_once(
        path,
        "previous.height == current.height && previous.block_hash == current.block_hash",
        "previous.height == current.height",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "KuraReplicaAdvertRefreshState::note_durable_tip" in error
        and "previous.block_hash" in error
        for error in errors
    ), errors


def test_kura_replica_retention_contract_rejects_unbound_start_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(
        path,
        "kura.local_peer_id.get().is_none()",
        "false",
    )
    errors = validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "Kura::start" in error and "kura.local_peer_id.get().is_none()" in error
        for error in errors
    ), errors


def copy_layout_fixture(tmp_path: Path, module, contract: dict) -> None:
    """Copy every file consumed by the isolated layout-contract validator."""
    relatives = {
        module.FORMAL_RELATIVE / f"{contract['module']}.tla",
        module.FORMAL_RELATIVE / contract["positive_config"],
        Path(contract["runner"]),
        Path(contract["evidence"]),
        module.CLOSURE_LEDGER_RELATIVE,
        module.INFLIGHT_LAYOUT_TEST,
    }
    relatives.update(
        module.FORMAL_RELATIVE / mutation["config"]
        for mutation in contract["mutations"]
    )
    relatives.update(
        Path(binding["path"]) for binding in contract["production_symbols"]
    )
    relatives.update(
        Path(check["path"]) for check in contract["ordered_source_checks"]
    )
    relatives.update(
        Path(check["path"]) for check in contract["forbidden_source_checks"]
    )
    relatives.update(Path(check["path"]) for check in contract["source_checks"])
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)


def validate_fixture(tmp_path: Path, module, contract: dict) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_inflight_layout_contract(
        tmp_path,
        tmp_path / module.FORMAL_RELATIVE,
        contract,
        errors,
    )
    return tuple(errors)


def replace_once(path: Path, old: str, new: str) -> None:
    source = path.read_text(encoding="utf-8")
    assert source.count(old) >= 1, f"fixture cannot find {old!r} in {path}"
    path.write_text(source.replace(old, new, 1), encoding="utf-8")


def replace_once_after(path: Path, anchor: str, old: str, new: str) -> None:
    """Replace one token after an exact enclosing-item anchor."""
    source = path.read_text(encoding="utf-8")
    anchor_offset = source.find(anchor)
    assert anchor_offset >= 0, f"fixture cannot find {anchor!r} in {path}"
    old_offset = source.find(old, anchor_offset + len(anchor))
    assert old_offset >= 0, f"fixture cannot find {old!r} after {anchor!r} in {path}"
    path.write_text(
        source[:old_offset] + new + source[old_offset + len(old) :],
        encoding="utf-8",
    )


def swap_ordered_once(path: Path, earlier: str, later: str) -> None:
    """Swap one ordered token pair while retaining both source anchors."""
    source = path.read_text(encoding="utf-8")
    earlier_offset = source.find(earlier)
    assert earlier_offset >= 0, f"fixture cannot find {earlier!r} in {path}"
    later_offset = source.find(later, earlier_offset + len(earlier))
    assert later_offset >= 0, (
        f"fixture cannot find {later!r} after {earlier!r} in {path}"
    )
    middle = source[earlier_offset + len(earlier) : later_offset]
    path.write_text(
        source[:earlier_offset]
        + later
        + middle
        + earlier
        + source[later_offset + len(later) :],
        encoding="utf-8",
    )


def swap_ordered_once_after(
    path: Path, anchor: str, earlier: str, later: str
) -> None:
    """Swap one ordered token pair after an exact enclosing-item anchor."""

    source = path.read_text(encoding="utf-8")
    anchor_offset = source.find(anchor)
    assert anchor_offset >= 0, f"fixture cannot find {anchor!r} in {path}"
    earlier_offset = source.find(earlier, anchor_offset + len(anchor))
    assert earlier_offset >= 0, (
        f"fixture cannot find {earlier!r} after {anchor!r} in {path}"
    )
    later_offset = source.find(later, earlier_offset + len(earlier))
    assert later_offset >= 0, (
        f"fixture cannot find {later!r} after {earlier!r} in {path}"
    )
    middle = source[earlier_offset + len(earlier) : later_offset]
    path.write_text(
        source[:earlier_offset]
        + later
        + middle
        + earlier
        + source[later_offset + len(later) :],
        encoding="utf-8",
    )


def copy_reviewed_source_fixture_with_includes(
    tmp_path: Path, module, relatives: set[Path]
) -> None:
    """Copy a live closure, then create its isolated stage-zero fixture."""

    helper_path = ROOT_DIR / module.REVIEWED_RUST_SOURCE_HELPER_RELATIVE
    helper_spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_multilane_reviewed_rust_source_fixture", helper_path
    )
    assert helper_spec is not None
    assert helper_spec.loader is not None
    reviewed_source = importlib.util.module_from_spec(helper_spec)
    sys.modules[helper_spec.name] = reviewed_source
    helper_spec.loader.exec_module(reviewed_source)
    pending = list(relatives)
    copied: set[Path] = set()
    while pending:
        relative = pending.pop()
        if relative in copied:
            continue
        source = ROOT_DIR / relative
        assert source.is_file() and not source.is_symlink()
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        copied.add(relative)
        if relative.suffix != ".rs":
            continue
        errors: list[str] = []
        invocations = reviewed_source._rust_include_invocations(
            source.read_text(encoding="utf-8"), source, errors
        )
        assert errors == []
        for invocation in invocations:
            child = reviewed_source._canonical_provider_relative(
                invocation.relative
            )
            assert child is not None
            pending.append(relative.parent.joinpath(*child.parts))
    initialize_git_fixture(tmp_path)


def copy_native_prepublication_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy the production files consumed by the ML-NAT-06 order contract."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.NATIVE_PREPUBLICATION_BINDINGS
    }
    relatives.update(
        relative
        for relative, _, _ in module.native_merge_manifest.NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_native_prepublication_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    with module._reviewed_rust_source_cache():
        module._validate_native_prepublication_contract(tmp_path, models, errors)
    return tuple(errors)


def copy_native_exact_object_prune_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy the production source consumed by the exact-object prune seal."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.NATIVE_EXACT_OBJECT_PRUNE_BINDINGS
    }
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_native_exact_object_prune_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_native_exact_object_prune_contract(
        tmp_path, models, errors
    )
    return tuple(errors)


def copy_native_participant_classifier_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy the two consumers bound to the shared participant classifier."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in (
            module.NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_BINDINGS
        )
    }
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_native_participant_classifier_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_native_participant_application_classifier_contract(
        tmp_path, models, errors
    )
    return tuple(errors)


def copy_queue_plan_pending_membership_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy sources consumed by the exact QueuePlan route-member contract."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS
    }
    relatives.update(
        Path(relative)
        for relative, _, _ in module.QUEUE_PLAN_PENDING_MEMBERSHIP_TEST_BINDINGS
    )
    relatives.update(
        Path(row[0])
        for row in module.QUEUE_PLAN_PENDING_MEMBERSHIP_ORDERED_SOURCE_CHECKS
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_queue_plan_pending_membership_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_queue_plan_pending_membership_contract(
        tmp_path, models, errors
    )
    return tuple(errors)


def test_queue_plan_pending_membership_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    assert validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    ) == ()


def test_queue_plan_pending_membership_contract_rejects_bound_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "const MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES: usize = 1024;",
        "const MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES: usize = 2048;",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any("one exact reviewed 1024-byte declaration" in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_roster_bound_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "const MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS: usize =\n"
        "    iroha_data_model::merge::MAX_MERGE_QUEUE_PLAN_ADMISSIONS;",
        "const MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS: usize = usize::MAX;",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any("exact merge-admission consensus bound" in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_unbounded_roster_scan(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "            route,\n"
        "            MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS,\n",
        "            route,\n"
        "            usize::MAX,\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_members_from_storage" in error
        and "MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_phantom_member(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "if storage.get(&obligation_key).is_none() {",
        "if false {",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_members_from_storage" in error
        and "storage.get(&obligation_key).is_none()" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_full_roster_obligation_decode(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_members_from_storage_with_limit(",
        "            let obligation_key = Self::queue_plan_pending_obligation_marker_key(\n",
        "            let _ = Self::decode_exact_queue_plan_pending_obligation_marker(\n"
        "                key, payload,\n"
        "            )?;\n"
        "            let obligation_key = Self::queue_plan_pending_obligation_marker_key(\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "without decoding the full obligation payload" in error for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_untyped_member_claim(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_member_identity_from_claim(",
        "        entrypoint_hash: HashOf<TransactionEntrypoint>,\n",
        "        entrypoint_hash: Hash,\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_member_identity_from_claim" in error
        and "HashOf<TransactionEntrypoint>" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_visible_native_prefix(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_HOST_RELATIVE
    replace_once(
        path,
        '    "queue_plan_pending_route_member_v1_",\n',
        "",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_member_v1_" in error
        and "opaque system contract-state namespace" in error
        for error in errors
    ), errors


def assert_inflight_order_drift_rejected(
    tmp_path: Path, earlier: str, later: str,
    rejected_token: str, required_scope: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        required_scope in error
        and f"missing or reorders token {rejected_token!r}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "queue_plan_registry_staging_is_an_exact_idempotent_compare_and_set",
            "failed whole-list staging must restore the exact prior overlay",
        ),
        (
            "queue_plan_pending_resolution_corrupt_route_counts_fail_without_partial_mutation",
            "failed whole-list resolution must restore the exact prior overlay",
        ),
    ),
)
def test_queue_plan_pending_membership_contract_rejects_atomic_test_weakening(
    tmp_path: Path, symbol: str, token: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs"
    )
    replace_once(path, token, "weakened atomic rollback assertion")
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(symbol in error and token in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_inner_stage_prefix_write(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "        let obligation_payload = Self::queue_plan_pending_obligation_marker_payload(&obligation)?;\n",
        "        storage.insert_queue_plan_marker(obligation_key.clone(), Vec::new());\n"
        "        let obligation_payload = Self::queue_plan_pending_obligation_marker_payload(&obligation)?;\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "stage_queue_plan_pending_obligation_marker_in_storage mutates WSV "
        "before completing all-route preflight" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stage_apply_before_list(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        "fn stage_queue_plan_admissions(",
        "State::stage_queue_plan_pending_obligation_in_storage(&mut markers, &admission)?;",
        "markers.apply();",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "ordered QueuePlan pending route-membership item "
        "stage_queue_plan_admissions" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "symbol",
    (
        "resolve_queue_plan_pending_obligations_for_entrypoints",
        "resolve_required_queue_plan_pending_obligations",
    ),
)
def test_queue_plan_pending_membership_contract_rejects_bulk_apply_before_list(
    tmp_path: Path, symbol: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        f"fn {symbol}(",
        "State::resolve_queue_plan_pending_obligation_in_storage(",
        "markers.apply();",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        f"ordered QueuePlan pending route-membership item {symbol}" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_decode_before_bound(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        "fn decode_exact_queue_plan_pending_route_member_marker(",
        "payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES",
        "norito::decode_from_bytes::<QueuePlanPendingRouteMemberV1>(payload)",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "ordered QueuePlan pending route-membership item "
        "decode_exact_queue_plan_pending_route_member_marker" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_lifecycle_height_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "state.lane_incarnation_at_height(route.lane_id, proposal_height)",
        "state.lane_incarnation_at_height("
        "route.lane_id, proposal_height.saturating_add(1))",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_obligation_matches_active_lifecycle" in error
        and "lane_incarnation_at_height" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stale_queue_ownership(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "application_state != Some(QueuePlanAdmissionApplicationState::Pending)",
        "application_state != Some(QueuePlanAdmissionApplicationState::PendingStale)",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_admission_registry_match" in error
        and "QueuePlanAdmissionApplicationState::Pending" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stale_cleanup_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "                    QueuePlanAdmissionApplicationState::PendingStale => {\n"
        "                        PendingQueuePlanAdmissionDisposition::Stale\n"
        "                    }",
        "                    QueuePlanAdmissionApplicationState::PendingStale => {\n"
        "                        PendingQueuePlanAdmissionDisposition::Exact\n"
        "                    }",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "classify_pending_queue_plan_admission" in error
        and "PendingQueuePlanAdmissionDisposition::Stale" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_preserves_historical_applied(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "None if committed => Ok(QueuePlanAdmissionApplicationState::Applied)",
        "None if committed => Ok(QueuePlanAdmissionApplicationState::PendingStale)",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_registry_owner_application_state_in_view" in error
        and "QueuePlanAdmissionApplicationState::Applied" in error
        for error in errors
    ), errors


def test_native_participant_classifier_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_participant_classifier_fixture(tmp_path, module)
    assert validate_native_participant_classifier_fixture(
        tmp_path, module, models
    ) == ()


@pytest.mark.parametrize(
    ("relative", "symbol"),
    (
        (
            "crates/iroha_core/src/block.rs",
            "validate_native_amx_participant_groups",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "native_amx_participant_application_diagnostic_rows_from_native_receipt",
        ),
    ),
    ids=("block-groups", "diagnostic-rows"),
)
def test_native_participant_classifier_contract_rejects_consumer_symbol_drift(
    tmp_path: Path, relative: str, symbol: str
) -> None:
    module = load_checker()
    models = copy_native_participant_classifier_fixture(tmp_path, module)
    path = tmp_path / relative
    replace_once(path, f"fn {symbol}(", f"fn {symbol}_drifted(")
    errors = validate_native_participant_classifier_fixture(
        tmp_path, module, models
    )
    assert any(
        f"source-bound symbol {symbol} must have one fn declaration" in error
        for error in errors
    ), errors


def test_native_participant_classifier_contract_rejects_block_group_role_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_participant_classifier_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/block.rs"
    replace_once(
        path,
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) => {\n"
        "                            continue;\n"
        "                        }\n"
        "                        Ok(\n"
        "                            crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant,\n"
        "                        ) => {}",
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) => {}\n"
        "                        Ok(\n"
        "                            crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant,\n"
        "                        ) => {\n"
        "                            continue;\n"
        "                        }",
    )
    errors = validate_native_participant_classifier_fixture(
        tmp_path, module, models
    )
    assert any(
        "shared participant classifier match relation drifted" in error
        and "validate_native_amx_participant_groups" in error
        for error in errors
    ), errors


def test_native_participant_classifier_contract_rejects_diagnostic_role_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_participant_classifier_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/state.rs"
    replace_once(
        path,
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) => {\n"
        "                    continue;\n"
        "                }\n"
        "                Ok(crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant) => {\n"
        "                }",
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) => {\n"
        "                }\n"
        "                Ok(crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant) => {\n"
        "                    continue;\n"
        "                }",
    )
    errors = validate_native_participant_classifier_fixture(
        tmp_path, module, models
    )
    assert any(
        "shared participant classifier match relation drifted" in error
        and "native_amx_participant_application_diagnostic_rows_from_native_receipt"
        in error
        for error in errors
    ), errors


def test_native_participant_classifier_contract_rejects_diagnostic_publish_order(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_participant_classifier_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/state.rs"
    swap_ordered_once(path, "row.validate().map_err", "rows.push(row)")
    errors = validate_native_participant_classifier_fixture(
        tmp_path, module, models
    )
    assert any(
        "shared participant classifier consumer "
        "native_amx_participant_application_diagnostic_rows_from_native_receipt"
        in error
        and "ordered downstream token" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    assert validate_native_prepublication_fixture(tmp_path, module, models) == ()


def test_native_exact_object_prune_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_exact_object_prune_fixture(tmp_path, module)
    assert (
        validate_native_exact_object_prune_fixture(tmp_path, module, models)
        == ()
    )


@pytest.mark.parametrize(
    ("anchor", "old", "new"),
    (
        (
            "fn verify_bound_open_regular_file_exact_bytes_locked(",
            "|| !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened)",
            "|| false",
        ),
        (
            "fn verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(",
            "|| !Self::progress_mutation_namespace_unchanged(namespace)",
            "|| false",
        ),
        (
            "fn remove_bound_progress_file_if_matches(",
            "|| entry.st_ino as u64 != expected_snapshot.file.ino()",
            "|| false",
        ),
        (
            "fn complete_native_amx_evidence_prune_intent_locked(",
            "Self::remove_bound_progress_file_if_matches(",
            "std::fs::remove_file(",
        ),
    ),
)
def test_native_exact_object_prune_contract_rejects_security_relation_drift(
    tmp_path: Path, anchor: str, old: str, new: str
) -> None:
    module = load_checker()
    models = copy_native_exact_object_prune_fixture(tmp_path, module)
    replace_once_after(
        tmp_path / "crates/iroha_core/src/kura.rs", anchor, old, new
    )
    errors = validate_native_exact_object_prune_fixture(
        tmp_path, module, models
    )
    assert any("exact-object" in error for error in errors), errors


@pytest.mark.parametrize(
    ("earlier", "later"),
    (
        (
            ".store_v2_finality_artifact(artifact)",
            ".prepublish_native_amx_participant_application_evidence(",
        ),
        (
            ".prepublish_native_amx_participant_application_evidence(",
            "State::native_amx_participant_frontier_markers_and_merge_entry(",
        ),
        (
            "State::native_amx_participant_frontier_markers_and_merge_entry(",
            "token.authenticates_state_frontiers(",
        ),
        (
            "token.authenticates_state_frontiers(",
            ".apply_without_execution_with_verified_v2_finality("
            "&committed_block, commit_topology)",
        ),
        (
            ".apply_without_execution_with_verified_v2_finality("
            "&committed_block, commit_topology)",
            ".pending_autoscale_retirement_binding()",
        ),
        (
            ".pending_autoscale_retirement_binding()",
            "Box::new(checked_carrier_applications)",
        ),
        (
            "Box::new(checked_carrier_applications)",
            "if carries_scale_in {",
        ),
        (
            "if carries_scale_in {",
            "self.queue.lock_lane_retirement_observer()",
        ),
        (
            "self.queue.lock_lane_retirement_observer()",
            ".commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto(",
        ),
        (
            ".commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto(",
            "state_block.commit_with_state_commit_authorization(state_commit_authorization)",
        ),
    ),
    ids=(
        "finality-before-prepublication",
        "prepublication-before-state-projection",
        "state-projection-before-readback-token",
        "readback-token-before-wsv-stage",
        "wsv-stage-before-scale-in-projection",
        "scale-in-projection-before-carrier-authorization",
        "carrier-authorization-before-scale-in-branch",
        "scale-in-branch-before-queue-observer",
        "queue-observer-before-scale-in-commit",
        "scale-in-before-ordinary-commit",
    ),
)
def test_native_prepublication_contract_rejects_apply_order_drift(
    tmp_path: Path, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "ordered Native prepublication item "
        "V2ApplyService::validate_and_apply" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later"),
    (
        (
            "let _state_commit_lock = state_ref.state_commit_lock.lock();",
            "let autoscale_lifecycle_guard",
        ),
        (
            "let autoscale_lifecycle_guard",
            "autoscale_retirement_queue_veto.as_mut()",
        ),
        (
            "autoscale_retirement_queue_veto.as_mut()",
            "state_commit_authorization.take()",
        ),
        (
            "state_commit_authorization.take()",
            ".consume_for_state_commit(",
        ),
        (
            ".consume_for_state_commit(",
            "state_ref.apply_committed_autoscale_lane_geometry(",
        ),
        (
            "state_ref.apply_committed_autoscale_lane_geometry(",
            "transactions.commit()",
        ),
    ),
)
def test_native_prepublication_contract_rejects_state_commit_order_drift(
    tmp_path: Path, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/state.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "ordered Native prepublication item commit_inner" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later"),
    (
        (
            "self.write_native_amx_participant_application_manifest_artifact_"
            "with_retention_policy_under_publication_guard(",
            "self.write_native_amx_participant_application_receipt_artifact_"
            "only_with_retention_policy_under_publication_guard(",
        ),
            (
                "self.write_native_amx_participant_application_receipt_artifact_"
                "only_with_retention_policy_under_publication_guard(",
                "self.write_native_amx_participant_receipt_latest_index_for_prepublication_"
                "under_publication_guard(",
            ),
            (
                "self.write_native_amx_participant_receipt_latest_index_for_prepublication_"
                "under_publication_guard(",
                "self.authenticate_native_amx_participant_application_"
                "prepublication_under_publication_guard(",
        ),
    ),
    ids=(
        "all-manifests-before-all-receipts",
        "all-receipts-before-all-latest-indexes",
        "all-latest-indexes-before-readback-auth",
    ),
)
def test_native_prepublication_contract_rejects_kura_phase_order_drift(
    tmp_path: Path, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    swap_ordered_once_after(
        path,
        "fn persist_native_amx_participant_application_evidence_"
        "under_publication_guard(",
        earlier,
        later,
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "ordered Native prepublication item "
        "persist_native_amx_participant_application_evidence_"
        "under_publication_guard" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_prewsv_retention_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/kura/native_amx_participant_application_artifacts.rs"
    )
    replace_once(
        path,
        "const fn permits_retention_cleanup(self) -> bool {\n"
        "        matches!(self, Self::PostWsvRepair)\n"
        "    }",
        "const fn permits_retention_cleanup(self) -> bool {\n"
        "        matches!(self, Self::PreWsv)\n"
        "    }",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "permits_retention_cleanup must authorize only PostWsvRepair" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_unguarded_retention_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(path, "if permit_cleanup {", "if true {")
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "cleanup-only-after-WSV" in error
        or (
            "ordered Native prepublication item "
            "persist_native_amx_participant_application_evidence_"
            "under_publication_guard" in error
        )
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_writer_retention_guard_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(
        path,
        "if !permit_retention_cleanup {\n"
        "            self.require_native_amx_evidence_prune_intent_absent_locked"
        "(&namespace)?;\n"
        "        }",
        "if false {\n"
        "            self.require_native_amx_evidence_prune_intent_absent_locked"
        "(&namespace)?;\n"
        "        }",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "must fail closed on retention state before PostWsvRepair" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_repair_mode_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    source = path.read_text(encoding="utf-8")
    repair_offset = source.index(
        "pub(crate) fn repair_native_amx_participant_application_evidence"
    )
    mode_offset = source.index(
        "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair",
        repair_offset,
    )
    path.write_text(
        source[:mode_offset]
        + "NativeAmxParticipantApplicationPublicationMode::PreWsv"
        + source[
            mode_offset
            + len(
                "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair"
            ) :
        ],
        encoding="utf-8",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "post-WSV Native repair must not use PreWsv publication mode" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_accepts_current_production(tmp_path: Path) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    assert validate_fixture(tmp_path, module, contract) == ()
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    constructor = "Some(CheckedProductionTransition::unwitnessed(projection))"
    symbol = "check_production_in_flight_reservation_transition"
    replace_once_after(path, f"pub(crate) fn {symbol}(", constructor,
                       "Some(CheckedProductionTransition { projection })")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(symbol in error and constructor in error for error in errors), errors


def test_inflight_composed_contract_rejects_rehydrate_without_kura_ownership(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    ownership_guard = "(before.carrier.kura_active & projection.actor) != 0u128"
    replace_once_after(
        path,
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY\n                )",
        ownership_guard,
        "projection.actor != 0u128",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and ownership_guard in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_rehydrate_action_tag_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    symbol = (
        "pub(crate) fn "
        "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition("
    )
    action = "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY"
    replace_once_after(
        path,
        symbol,
        action,
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition"
        in error
        and action in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_rehydrate_ready_tampering(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    ready_guard = "after.session.ready_authorized == before.session.ready_authorized"
    weakened = (
        "after.session.ready_authorized "
        "== (before.session.ready_authorized | projection.actor)"
    )
    replace_once_after(
        path,
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY\n                )",
        ready_guard,
        weakened,
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and ready_guard in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_terminal_rehydrate_resurrection(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    retirement_guard = "!before.release.kura_retired"
    replace_once_after(
        path,
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY",
        retirement_guard,
        "before.release.kura_retired == before.release.kura_retired",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and retirement_guard in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "anchor", "old", "new", "symbol", "required_token"),
    (
        (
            "crates/iroha_core/src/queue/reservation_journal.rs",
            "pub(super) fn consume_snapshot_replay_seal(",
            "if current_content_identity != file_content_identity {",
            "if false {",
            "LaneQueueReservationJournal::consume_snapshot_replay_seal",
            "current_content_identity != file_content_identity",
        ),
        (
            "crates/iroha_core/src/queue/reservation_journal.rs",
            "pub(crate) fn binds_reconciliation_snapshot(",
            "canonical_reconciliation_identity(&owners)?",
            'Hash::new(b"unchecked-reconciliation")',
            "LaneReservationSnapshotReplayReceipt::binds_reconciliation_snapshot",
            "canonical_reconciliation_identity(&owners)?",
        ),
        (
            "crates/iroha_core/src/queue.rs",
            "pub fn install_lane_reservation_journal(",
            "let replay_receipt = journal.consume_snapshot_replay_seal(replay_seal)?;",
            "let replay_receipt = self.lane_reservation_snapshot_replay_receipt()?;",
            "Queue::install_lane_reservation_journal",
            "journal.consume_snapshot_replay_seal(replay_seal)?",
        ),
        (
            "crates/iroha_core/src/queue.rs",
            "pub(crate) fn bind_lane_reservation_startup_reconciliation_receipt(",
            "if !replay_receipt.binds_reconciliation_snapshot(expected_snapshot)? {",
            "if false {",
            "Queue::bind_lane_reservation_startup_reconciliation_receipt",
            "replay_receipt.binds_reconciliation_snapshot(expected_snapshot)?",
        ),
        (
            "crates/iroha_core/src/queue.rs",
            "pub(crate) fn complete_lane_reservation_startup_reconciliation(",
            "Some(&receipt.replay_receipt)",
            "None",
            "Queue::complete_lane_reservation_startup_reconciliation",
            "Some(&receipt.replay_receipt)",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "pub(crate) fn apply_lane_reservation_reconciliation_plan(",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "revalidate_unchecked_startup_reconciliation_receipt(",
            "apply_lane_reservation_reconciliation_plan",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
        ),
        (
            "crates/iroha_sumeragi_core/src/verus_proofs/in_flight_first_release_proofs.rs",
            "pub proof fn production_in_flight_reservation_snapshot_replay_refines_composed_stutter(",
            "production_in_flight_reservation_transition_kernel(primitive),",
            "true,",
            "production_in_flight_reservation_snapshot_replay_refines_composed_stutter",
            "production_in_flight_reservation_transition_kernel(primitive)",
        ),
    ),
)
def test_inflight_layout_contract_rejects_snapshot_replay_bridge_weakening(
    tmp_path: Path,
    relative: str,
    anchor: str,
    old: str,
    new: str,
    symbol: str,
    required_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    replace_once_after(tmp_path / relative, anchor, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(symbol in error and required_token in error for error in errors), errors


def test_inflight_layout_contract_rejects_partial_lane_transport_whitelist(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    replace_once(
        path,
        "        if !message.is_lane_local() {\n",
        "        if !matches!(message, BlockMessage::LaneBlockProposal(_)) {\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "ProductionV2Services::post_lane_block" in error
        and "message.is_lane_local()" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "variant",
    (
        "LaneExecutablePayload",
        "LaneBlockNewViewVote",
        "LaneBlockNewViewCertificate",
    ),
)
def test_inflight_layout_contract_rejects_non_retireable_lane_transport_omission(
    tmp_path: Path, variant: str
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/message.rs"
    replace_once(path, f"                | Self::{variant}(_)\n", "")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "BlockMessage::is_lane_local" in error
        and f"Self::{variant}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "new", "required_token"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
            "ExactOutputRolloverClaim::scope",
            "Self::Exact | Self::NonRetireableLaneTransport { .. } => None",
            "Self::Exact => None",
            "Self::Exact | Self::NonRetireableLaneTransport { .. } => None",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
            "ExactOutputRolloverClaim::validate_non_retireable_lane_transport_fanout",
            "HashOf::new(message) != message_hash",
            "false",
            "HashOf::new(message) != message_hash",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "applied_height_reconstruction_covers",
            "non-retireable lane transport must drain before applied-height handoff",
            "lane transport handoff accepted",
            "non-retireable lane transport must drain before applied-height handoff",
        ),
    ),
)
def test_inflight_layout_contract_rejects_weakened_non_retireable_lane_claim(
    tmp_path: Path,
    relative: str,
    symbol: str,
    old: str,
    new: str,
    required_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    replace_once(tmp_path / relative, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(symbol in error and required_token in error for error in errors), errors


def test_inflight_layout_contract_rejects_early_autonomous_queue_plan_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    replace_once(
        path,
        "                .filter(|transaction_hash| {\n"
        "                    !staged_merge_queue_reservation_hashes.contains(transaction_hash)\n"
        "                }),\n",
        "                ,\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "V2ApplyService::validate_and_apply" in error
        and ".filter(|transaction_hash|" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_queue_cleanup_before_evidence_repair(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    source = path.read_text(encoding="utf-8")
    start = source.index("    fn execute_exact_apply(")
    end = source.index("\n    fn finish_durable_apply_completion_against(", start)
    method = source[start:end]
    promote, finalize = "promote_kagemusha_topup_finality_sidecar", "finalize_committed_block_merge_reservations"
    assert method.count(promote) == 1 and method.count(finalize) == 1
    marker = "__SWAP_POST_CARRIER_REPAIR_ORDER__"
    method = method.replace(promote, marker, 1).replace(finalize, promote, 1).replace(marker, finalize, 1)
    source = source[:start] + method + source[end:]
    delegate = source.index("self.execute_exact_apply(", source.index("    pub(crate) fn execute("), start)
    path.write_text(source[:delegate] + source[delegate:].replace(
        "self.execute_exact_apply(", "self.execute_exact_application(", 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "ordered in-flight item V2ApplyService::execute_exact_apply" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors
    assert any(
        "in-flight production item V2ApplyService::execute" in error
        and "self.execute_exact_apply(" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_da_policy_as_carrier_effect(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/block.rs"
    replace_once(
        path,
        "            block.da_commitments().is_some() || block.da_pin_intents().is_some()\n",
        "            block.da_commitments().is_some()\n"
        "                || block.da_proof_policies().is_some()\n"
        "                || block.da_pin_intents().is_some()\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "ValidBlock::autonomous_merge_carrier_has_da_effect" in error
        and "da_proof_policies" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "effect_accessor",
    ("block.da_commitments().is_some()", "block.da_pin_intents().is_some()"),
)
def test_inflight_layout_contract_rejects_unbound_da_carrier_effect(
    tmp_path: Path, effect_accessor: str
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/block.rs"
    replace_once(path, effect_accessor, "false")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "ValidBlock::autonomous_merge_carrier_has_da_effect" in error
        and effect_accessor in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_partial_ordinary_carrier_filter(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_candidate.rs"
    replace_once(
        path,
        "if record_ordinary_execution_carrier_exclusion(certified_execution_selected, report) {",
        "if record_ordinary_execution_carrier_exclusion(\n"
        "    certified_execution_selected\n"
        "        && transaction.creation_time() >= Duration::from_millis(1_000),\n"
        "    report,\n"
        ") {",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "V2CandidateAssembler::snapshot_routable_candidates" in error
        and "transaction.creation_time()" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_carrier_exclusion_as_unavailable_work(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_candidate.rs"
    replace_once(
        path,
        "    report.carrier_excluded = report.carrier_excluded.saturating_add(1);\n",
        "    report.work_deferred = report.work_deferred.saturating_add(1);\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "record_ordinary_execution_carrier_exclusion" in error
        and "report.work_deferred" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_execution_provider_releasing_pending_anchors(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    source = path.read_text(encoding="utf-8")
    start = source.index("    pub(crate) fn prepare_certified_execution_carrier(")
    end = source.index("\n    fn frozen_roster_contains(", start)
    method = source[start:end]
    assert method.count("        self.planned_lane_proposals.clear();\n") == 1
    method = method.replace(
        "        self.planned_lane_proposals.clear();\n",
        "        self.pending_autonomous_anchor_payloads.clear();\n"
        "        self.planned_lane_proposals.clear();\n",
        1,
    )
    path.write_text(source[:start] + method + source[end:], encoding="utf-8")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "V2LaneWorkAdapter::prepare_certified_execution_carrier" in error
        and "pending_autonomous_anchor_payloads.clear" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "symbol", "required"),
    (
        (
            "super::v2_npos::validate_candidate_records(",
            "super::v2_npos::validate_candidate_records_unchecked(",
            "candidate_attachments",
            "validate_candidate_records(",
        ),
        (
            "!selection.allows_execution() && entry.execution_batch.is_some()",
            "false && entry.execution_batch.is_some()",
            "State::select_pending_certified_merge_entry_for_round",
            "!selection.allows_execution() && entry.execution_batch.is_some()",
        ),
        (
            "matches!(self, Self::Any)",
            "true",
            "PendingCertifiedMergeSelection::allows_execution",
            "matches!(self, Self::Any)",
        ),
        (
            "work_provider: &mut *lane_work",
            "work_provider: &mut *unchecked_lane_work",
            "schedule_local_proposal",
            "work_provider: &mut *lane_work",
        ),
    ),
)
def test_inflight_layout_contract_rejects_weakened_execution_carrier_priority(
    tmp_path: Path,
    old: str,
    new: str,
    symbol: str,
    required: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    relative = (
        "crates/iroha_core/src/state.rs"
        if symbol.startswith(("State::", "PendingCertifiedMergeSelection::"))
        else "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    replace_once(tmp_path / relative, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(symbol in error and required in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            Path(
                "formal/sumeragi_v2/"
                "SumeragiV2InFlightFirstRelease.tla"
            ),
            "LaneExecutablePayloadV1",
            "LaneExecutablePayloadV3",
        ),
        (
            Path(
                "formal/sumeragi_v2/"
                "SumeragiV2InFlightFirstRelease.tla"
            ),
            "SelectQueuePlanV4Conjunction ==\n",
            "SelectQueuePlanV4Snapshot ==\n",
        ),
        (
            Path("crates/iroha_core/src/lane_consensus.rs"),
            "LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 2",
            "LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 3",
        ),
        (
            Path("crates/iroha_core/src/queue/journal.rs"),
            "QUEUE_PLAN_JOURNAL_VERSION: u16 = 4",
            "QUEUE_PLAN_JOURNAL_VERSION: u16 = 5",
        ),
        (
            Path("crates/iroha_core/src/queue/reservation_journal.rs"),
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5",
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 9",
        ),
        (
            Path("crates/iroha_data_model/src/merge.rs"),
            "MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096",
            "MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_097",
        ),
        (
            Path(
                "crates/iroha_core/src/kura/"
                "pipeline_and_lane_artifacts.rs"
            ),
            "pub entrypoint_hashes: Vec<Hash>,\n"
            "    /// Accepted entrypoints in lane descriptor order.\n"
            "    pub entrypoints: Vec<TransactionEntrypoint>,\n"
            "    /// Exact durable queue reservation identities in entrypoint order.\n"
            "    pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub entrypoint_hashes: Vec<Hash>,\n"
            "    /// Accepted entrypoints in lane descriptor order.\n"
            "    pub entrypoints: Vec<TransactionEntrypoint>,\n"
            "    /// Exact durable queue reservation identities in entrypoint order.\n"
            "    pub reservation_tokens: Vec<LaneQueueReservationKeyV2>",
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
            "MLPayloadV3CarriesExactAdmissionPreimage",
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'local invariant_marker="Invariant ${invariant} is violated."',
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            'sumeragi_v2_tlc_assert_exact_line \\\n'
            '    "$config" "$log" "$invariant_marker"',
            'grep -Fq "$invariant_marker" "$log"',
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '"inflight_first_release_fixed.cfg",\n        "18",',
            '"inflight_first_release_fixed.cfg",\n        "17",',
        ),
        (
            Path("formal/sumeragi_v2/inflight_first_release_fixed.cfg"),
            "INVARIANT MLQueuePlanV4SelectedConjunctionBound4096\n",
            "",
        ),
    ),
)
def test_inflight_layout_contract_rejects_semantic_drift(
    tmp_path: Path, relative: Path, old: str, new: str
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    replace_once(tmp_path / relative, old, new)
    assert validate_fixture(tmp_path, module, contract)


def test_inflight_layout_contract_rejects_membership_only_lane_authorship(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/lane_consensus.rs"
    replace_once(
        path,
        ") != Some(&self.producer)\n        {",
        ").is_none()\n        {",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "LaneExecutablePayloadV1::validate" in error
        and ") != Some(&self.producer)" in error
        for error in errors
    ), errors


def copy_mutation_runner_fixture(tmp_path: Path, module) -> Path:
    relative = module.TLC_MUTATION_RUNNER_RELATIVE
    destination = tmp_path / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT_DIR / relative, destination)
    return destination


def validate_mutation_runner_fixture(tmp_path: Path, module) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_mutation_runner(
        tmp_path,
        canonical_models(),
        canonical_kura_retention_contract(),
        errors,
    )
    return tuple(errors)


def test_multilane_mutation_runner_accepts_shared_exact_line_contract(
    tmp_path: Path,
) -> None:
    module = load_checker()
    copy_mutation_runner_fixture(tmp_path, module)
    assert validate_mutation_runner_fixture(tmp_path, module) == ()


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'local invariant_marker="Invariant ${invariant} is violated."',
        ),
        (
            'sumeragi_v2_tlc_assert_exact_line "$name" "$log" "$invariant_marker"',
            'grep -Fq "$invariant_marker" "$log"',
        ),
        (
            'sumeragi_v2_tlc_assert_terminal "$name" "$log"',
            'grep -Fq "Finished in" "$log"',
        ),
    ),
)
def test_multilane_mutation_runner_rejects_weakened_result_contract(
    tmp_path: Path, old: str, new: str
) -> None:
    module = load_checker()
    runner = copy_mutation_runner_fixture(tmp_path, module)
    replace_once(runner, old, new)
    assert validate_mutation_runner_fixture(tmp_path, module)


def test_inflight_layout_contract_rejects_durability_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count("durable_claim.global_admission_binding()") >= 1
    assert source.count("journal.put_batch(") >= 1
    source = source.replace("journal.put_batch(", "journal.put_all(", 1)
    source = source.replace(
        "durable_claim.global_admission_binding()",
        "journal.put_batch(Vec::new()); "
        "durable_claim.global_admission_binding()",
        1,
    )
    path.write_text(source, encoding="utf-8")
    errors = validate_fixture(tmp_path, module, contract)
    assert any("missing or reorders token" in error for error in errors)


def test_inflight_layout_contract_rejects_reservation_pre_state_identity_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "expected_state_identity: self.checked_state_identity,",
        "expected_state_identity: resulting_state_identity,",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::prepare_checked_transition"
        in error
        and "expected_state_identity: self.checked_state_identity" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "authorization_domain: self.authorization_domain.authorization(),",
            "authorization_domain: Arc::new(()),",
            "authorization_domain: self.authorization_domain.authorization()",
        ),
        (
            "expected_shape: self.checked_shape(),",
            "expected_shape: CheckedReplayStateShape { "
            "live: 0, committed: 0, release_barriers: 0, "
            "completed_releases: 0, ownership: 0, fifo_ordinals: 0, "
            "live_lane_incarnations: 0, next_order: 0 },",
            "expected_shape: self.checked_shape()",
        ),
        (
            ".authorizes(&prepared.authorization_domain)",
            ".authorizes(&self.authorization_domain.authorization())",
            ".authorizes(&prepared.authorization_domain)",
        ),
        (
            "self.checked_shape() != prepared.expected_shape",
            "self.checked_shape() == prepared.expected_shape",
            "self.checked_shape() != prepared.expected_shape",
        ),
    ),
)
def test_inflight_layout_contract_rejects_state_instance_and_shape_binding_drift(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        expected_token in error
        and (
            "PreparedReservationJournalTransition" in error
            or "prepare_checked_transition" in error
            or "apply_checked_transition" in error
        )
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "symbol", "expected_token"),
    (
        (
            "fn clone(&self) -> Self {\n        Self::default()\n    }",
            "fn clone(&self) -> Self {\n"
            "        Self(Arc::clone(&self.0))\n"
            "    }",
            "CheckedReplayAuthorizationDomain::clone",
            "Self::default()",
        ),
        (
            "Arc::ptr_eq(&self.0, authorization)",
            "true",
            "CheckedReplayAuthorizationDomain::authorizes",
            "Arc::ptr_eq(&self.0, authorization)",
        ),
    ),
)
def test_inflight_layout_contract_rejects_authorization_domain_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    symbol: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        symbol in error and expected_token in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("field", "projection"),
    (
        ("live", "self.live.len()"),
        ("committed", "self.committed.len()"),
        ("release_barriers", "self.release_barriers.len()"),
        ("completed_releases", "self.completed_releases.len()"),
        ("ownership", "self.ownership.len()"),
        ("fifo_ordinals", "self.fifo_ordinals.len()"),
        (
            "live_lane_incarnations",
            "self.live_by_lane_incarnation.len()",
        ),
        ("next_order", "self.next_order"),
    ),
)
def test_inflight_layout_contract_rejects_checked_shape_projection_weakening(
    tmp_path: Path,
    field: str,
    projection: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, f"{field}: {projection},", f"{field}: Default::default(),")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::checked_shape" in error
        and f"{field}: {projection}" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_checked_shape_field_extension(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "struct CheckedReplayStateShape {\n    live: usize,",
        "struct CheckedReplayStateShape {\n"
        "    unrelated_cache_entries: usize,\n"
        "    live: usize,",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "CheckedReplayStateShape" in error
        and "missing current-layout token" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,\n"
            "                        key,\n"
            "                        release_digest,\n"
            "                        self.ownership.get(&hash).copied(),\n"
            "                        candidate.ownership.get(&hash).copied(),",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,\n"
            "                        key,\n"
            "                        release_digest,\n"
            "                        self.ownership.get(&hash).copied(),\n"
            "                        self.ownership.get(&hash).copied(),",
            "candidate.ownership.get(&hash).copied()",
        ),
        (
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
            "let after = Some(DurableReservationOwnership::Live(key));",
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
        ),
        (
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "let after = before;",
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {",
        ),
        (
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    Some(DurableReservationOwnership::Committed(*key)),",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    before,",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,",
        ),
        (
            "let after = if before == "
            "Some(DurableReservationOwnership::Committed(*key)) {\n"
            "                    None\n"
            "                } else {\n"
            "                    before\n"
            "                };",
            "let after = before;",
            "let after = if before == "
            "Some(DurableReservationOwnership::Committed(*key)) {",
        ),
        (
            "Some(DurableReservationOwnership::Live(existing)) "
            "if existing == *key => {\n"
            "                            "
            "Some(DurableReservationOwnership::Prepared {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "Some(DurableReservationOwnership::Live(_existing)) => {\n"
            "                            "
            "Some(DurableReservationOwnership::Prepared {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "Some(DurableReservationOwnership::Live(existing)) "
            "if existing == *key => {",
        ),
        (
            "}) if existing == key && barrier_digest == release_digest => {",
            "}) if existing == key => {",
            "barrier_digest == release_digest",
        ),
        (
            "let after = if has_completion\n"
            "                        && before\n"
            "                            == "
            "Some(DurableReservationOwnership::Completed {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            }) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "let after = before;",
            "let after = if has_completion",
        ),
    ),
    ids=(
        "snapshot",
        "reserve",
        "release-direct",
        "commit",
        "forget-commit",
        "prepare-release",
        "complete-release",
        "forget-release",
    ),
)
def test_inflight_layout_contract_rejects_action_owner_projection_drift(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::check_in_flight_transition" in error
        and expected_token in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_reordered_owner_token_coverage(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(
        path,
        "prepared.owner_transition_count != prepared.owner_transitions.len()",
        "checked_transition_coverage_identity(&prepared.owner_transitions)?",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::apply_checked_transition" in error
        and "missing or reorders token "
        "'checked_transition_coverage_identity(&prepared.owner_transitions)?'"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            "self.transition_semantics(frame, maximum, false)?;",
            "let current_owner_transitions = "
            "self.check_in_flight_transition(frame, maximum)?;",
            "let current_owner_transitions = "
            "self.check_in_flight_transition(frame, maximum)?;",
        ),
        (
            "for checked in prepared.owner_transitions",
            "self.transition_semantics(frame, maximum, true)?;",
            "checked.into_projection()",
        ),
        (
            "self.transition_semantics(frame, maximum, true)?;",
            "self.transition_generation = prepared.next_generation;",
            "self.transition_generation = prepared.next_generation;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_revalidate_consume_apply_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    assert_inflight_order_drift_rejected(
        tmp_path, earlier, later, rejected_token,
        "IndexedReservationReplayState::apply_checked_transition",
    )


@pytest.mark.parametrize(
    ("injected", "forbidden"),
    (
        ("let _candidate = self.clone();", "self.clone()"),
        ("let _candidate = Clone::clone(self);", "Clone::clone(self)"),
        ("let _candidate = (*self).clone();", "(*self).clone()"),
        ("let _candidate = self.to_owned();", "self.to_owned()"),
        (
            "let _candidate = ToOwned::to_owned(self);",
            "ToOwned::to_owned(self)",
        ),
        (
            "let _ = candidate.transition_semantics("
            "frame, maximum, true)?;",
            "candidate.transition_semantics(",
        ),
        ("*self = candidate;", "*self = candidate"),
    ),
    ids=(
        "clone-method",
        "clone-trait",
        "clone-deref",
        "to-owned-method",
        "to-owned-trait",
        "candidate-transition",
        "candidate-swap",
    ),
)
def test_inflight_layout_contract_rejects_unbounded_full_state_application(
    tmp_path: Path,
    injected: str,
    forbidden: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.transition_semantics(frame, maximum, true)?;",
        f"{injected}\n        self.transition_semantics(frame, maximum, true)?;",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::apply_checked_transition" in error
        and "forbidden source-bound token" in error
        and forbidden in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "earlier", "later"),
    (
        (
            "IndexedReservationReplayState::transition_release_batch",
            ".push(self.validate_live_secondary_indexes("
            "key.signed_transaction_hash, *key)?)",
            "if apply {",
        ),
        (
            "IndexedReservationReplayState::transition_commit",
            "Some(self.validate_live_secondary_indexes("
            "key.signed_transaction_hash, existing)?)",
            "if !apply {",
        ),
        (
            "IndexedReservationReplayState::transition_complete_release",
            "let live_record = "
            "self.validate_live_secondary_indexes(hash, record.key)?;",
            "if apply {",
        ),
    ),
)
def test_inflight_layout_contract_rejects_removal_before_full_preflight(
    tmp_path: Path,
    symbol: str,
    earlier: str,
    later: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        symbol in error and "missing or reorders token" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "expected_key.signed_transaction_hash != hash",
            "false",
            "expected_key.signed_transaction_hash != hash",
        ),
        (
            "let record = self\n"
            "            .live\n"
            "            .get(&hash)\n"
            "            .ok_or_else(|| "
            'invalid_data("live reservation index has no exact record"))?;',
            "let record = self\n"
            "            .live\n"
            "            .values()\n"
            "            .next()\n"
            "            .ok_or_else(|| "
            'invalid_data("live reservation index has no exact record"))?;',
            ".get(&hash)",
        ),
        (
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) "
            "!= Some(&hash)",
            "self.fifo_ordinals\n"
            "            .get(&record.value.fifo_order.ordinal)\n"
            "            .is_some_and(|existing| existing != &hash)",
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) "
            "!= Some(&hash)",
        ),
        (
            ".is_some_and(|hashes| hashes.contains(&hash))",
            ".is_some_and(|_hashes| true)",
            ".is_some_and(|hashes| hashes.contains(&hash))",
        ),
    ),
)
def test_inflight_layout_contract_rejects_secondary_index_preflight_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::validate_live_secondary_indexes"
        in error
        and expected_token in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_panicking_preflighted_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.live.remove(&hash);",
        'self.live.remove(&hash).expect("unchecked live removal");',
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::remove_preflighted_live" in error
        and "forbidden source-bound token 'expect('" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_legacy_unchecked_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.remove_preflighted_live(record);",
        "self.remove_live_unchecked(record.key.signed_transaction_hash);",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::transition_release_batch" in error
        and "remove_live_unchecked" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            ".prepare_checked_transition("
            "frame, self.limits.max_owned_transactions)?",
            "self.append_staged(&encoded, expected_end, prepared)",
            "encode_frame_with_limit(frame, "
            "self.limits.max_frame_payload_bytes)?",
        ),
        (
            "self.append_staged(&encoded, expected_end, prepared)",
            "if let Err(error) = "
            "self.replay_state.apply_checked_transition(",
            "if let Err(error) = "
            "self.replay_state.apply_checked_transition(",
        ),
        (
            "// replay instead of panicking or attempting an in-process retry.",
            "self.poisoned = true;",
            "self.poisoned = true;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_append_publication_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    assert_inflight_order_drift_rejected(
        tmp_path, earlier, later, rejected_token,
        "LaneQueueReservationJournal::append_durable",
    )


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            "self.parent.sync_all()",
            "compacted_replay_state.apply_checked_transition(\n"
            "                    frame,\n"
            "                    self.limits.max_owned_transactions,\n"
            "                    prepared,\n"
            "                )",
            "compacted_replay_state.apply_checked_transition(",
        ),
        (
            "// The replacement is already durable. Keep the previous",
            "self.replay_state = compacted_replay_state;",
            "self.poisoned = true;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_compaction_publication_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    assert_inflight_order_drift_rejected(
        tmp_path, earlier, later, rejected_token,
        "LaneQueueReservationJournal::compact_if_needed",
    )


def test_inflight_layout_contract_rejects_capability_restart_test_name_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/reservation_journal_recovery_tests.rs"
    )
    test_name = (
        "fn runtime_commit_requires_live_owner_but_snapshot_recovery_may_"
        "restore_commit_barrier()"
    )
    replace_once(
        path,
        test_name,
        test_name.replace("restore_commit_barrier", "restore_any_commit_barrier"),
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        f"current-layout token {test_name!r} must occur exactly once, found 0"
        in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_ledger_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["source_checks"][0]["required_tokens"].pop()
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any("whole-file source checks differ" in error for error in errors)


def test_inflight_layout_contract_rejects_closure_ledger_mutation_count_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "specs/sumeragi_v2_multilane_closure_ledger.md"
    replace_once(
        path,
        "twenty-two exact TLC mutation witnesses",
        "twenty exact TLC mutation witnesses",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "missing current-layout closure token "
        "'twenty-two exact TLC mutation witnesses'" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_action_inventory_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["required_actions"].remove("PersistPlanTombstone")
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any("actions differ" in error for error in errors)


def test_inflight_composed_contract_rejects_legacy_layout_only_claim(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["claim"] = "layout_only_no_transition_refinement"
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "in-flight layout contract claim must equal" in error
        and "composed_state_action_relation_with_source_bound_trace_extraction"
        in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_state_order_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "(session.ready_authorized & !carrier.execution_input_durable) == 0u128",
        "(session.ready_authorized & !7u128) == 0u128",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_state_body" in error
        and "ready_authorized" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "prefix_field",
    (
        "reservation_committed_prefix",
        "queue_plan_tombstoned_prefix",
        "reservation_commit_forgotten_prefix",
    ),
)
def test_inflight_composed_contract_rejects_per_key_prefix_skip_weakening(
    tmp_path: Path,
    prefix_field: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        f"== (before.history.{prefix_field} + 1u64) as u128",
        f"== (before.history.{prefix_field} + 2u64) as u128",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and prefix_field in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "consumer_anchor",
    (
        "pub fn native_amx_participant_applications_diagnostics(",
        "fn autonomous_lane_execution_diagnostics_inner(",
    ),
)
def test_stable_generation_diagnostics_rejects_unwrapped_projection(
    tmp_path: Path,
    consumer_anchor: str,
) -> None:
    module = load_checker()
    state, _helper = copy_stable_generation_diagnostics_fixture(tmp_path, module)
    replace_once_after(
        state,
        consumer_anchor,
        "self.derive_diagnostics_at_stable_state_generation(",
        "self.derive_diagnostics_without_stable_generation(",
    )
    errors = validate_stable_generation_diagnostics_fixture(tmp_path, module)
    assert any(
        "diagnostic consumer" in error
        and "derive_diagnostics_at_stable_state_generation" in error
        for error in errors
    ), errors
