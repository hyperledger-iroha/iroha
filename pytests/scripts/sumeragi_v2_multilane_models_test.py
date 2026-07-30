"""Negative controls for the multilane model/source-binding contract."""

from __future__ import annotations

import copy
import importlib.util
import json
import shutil
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER = (
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_multilane_models.py"
)
BINDINGS = (
    ROOT_DIR
    / "docs"
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


def canonical_contract() -> dict:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["inflight_first_release_layout_contract"])


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
    relatives.update(Path(check["path"]) for check in contract["source_checks"])
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)


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


def test_inflight_layout_contract_accepts_current_production(tmp_path: Path) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    assert validate_fixture(tmp_path, module, contract) == ()


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
            Path("crates/iroha_core/src/kura.rs"),
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
            Path("formal/sumeragi_v2/inflight_first_release_fixed.cfg"),
            "INVARIANT MLQueuePlanV4PutBatchBound4096\n",
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


def test_inflight_layout_contract_rejects_ledger_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["source_checks"][0]["required_tokens"].pop()
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any("whole-file source checks differ" in error for error in errors)


def test_inflight_layout_contract_rejects_refinement_claim_inflation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    module_path = (
        tmp_path
        / "docs"
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2InFlightFirstRelease.tla"
    )
    replace_once(
        module_path,
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
        "InFlightFirstReleaseProductionRefinementObligation == TRUE\n\n"
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any("must not declare a production refinement" in error for error in errors)
