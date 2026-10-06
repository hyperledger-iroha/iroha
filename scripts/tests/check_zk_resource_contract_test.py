"""Drift tests for the unified resource contract checker."""

from __future__ import annotations

import copy
import json
from pathlib import Path
from runpy import run_path

import pytest


CHECKER = run_path(
    str(Path(__file__).resolve().parents[1] / "check_zk_resource_contract.py")
)
ROOT = CHECKER["ROOT"]
CONTRACT = json.loads(CHECKER["CONTRACT"].read_text(encoding="utf-8"))
TASKS = CHECKER["load_tasks"](CHECKER["GRAPH"])
MIB = 1024 * 1024
GIB = 1024 * MIB


def problems(contract: dict, tasks: dict | None = None) -> list:
    return CHECKER["check"](ROOT, contract, TASKS if tasks is None else tasks)


def edited(edit) -> dict:
    contract = copy.deepcopy(CONTRACT)
    edit(contract)
    return contract


def bound(contract: dict, identifier: str) -> dict:
    return next(item for item in contract["bounds"] if item["id"] == identifier)


def relation(contract: dict, identifier: str) -> dict:
    return next(item for item in contract["relations"] if item["id"] == identifier)


def assert_fails(contract: dict, fragment: str, tasks: dict | None = None) -> None:
    found = problems(contract, tasks)
    assert any(fragment in problem for problem in found), found


def test_tracked_contract_matches_the_source() -> None:
    assert problems(CONTRACT) == []
    assert CHECKER["main"]([]) == 0


def test_contract_covers_every_group_stage_and_class() -> None:
    groups = {item["group"] for item in CONTRACT["bounds"]} | {
        item["group"] for item in CONTRACT["declared"]
    }
    assert groups == set(CHECKER["GROUPS"])
    stages = {
        site["stage"] for item in CONTRACT["bounds"] for site in item.get("enforcement", [])
    }
    assert stages == set(CHECKER["STAGES"])
    assert {item["class"] for item in CONTRACT["bounds"]} == set(CHECKER["CLASSES"])
    assert {item["source"] for item in CONTRACT["bounds"]} == set(CHECKER["SOURCES"])
    # Every bound names one owner symbol in one file.
    for item in CONTRACT["bounds"]:
        assert (ROOT / item["owner"]["path"]).is_file(), item["id"]


def test_fixed_x509_bounds_are_preserved_and_classed() -> None:
    expected = {
        "x509.max_proof_bytes": (9_437_184, "consensus"),
        "transaction.max_tx_bytes": (10_485_760, "consensus"),
        "x509.max_crl_age_seconds": (300, "consensus"),
        "x509.max_presentation_window_seconds": (300, "consensus"),
        "x509.prover_target_seconds": (300, "engineering_target"),
        "x509.prover_peak_rss_bytes": (12 * GIB, "local_defer"),
        "x509.prover_address_space_bytes": (32 * GIB, "local_defer"),
    }
    assert CHECKER["FIXED_BOUNDS"] == {key: value for key, (value, _) in expected.items()}
    for identifier, (value, klass) in expected.items():
        item = bound(CONTRACT, identifier)
        assert (item["value"], item["class"], item["fixed"]) == (value, klass, True)
    for identifier in CHECKER["FIXED_NON_VALIDITY"]:
        assert bound(CONTRACT, identifier)["validity"] is False
    # The validity bounds and the targets partition the fixed bounds.
    assert set(CHECKER["FIXED_VALIDITY"]) | set(CHECKER["FIXED_NON_VALIDITY"]) == set(expected)
    assert not set(CHECKER["FIXED_VALIDITY"]) & set(CHECKER["FIXED_NON_VALIDITY"])


def test_the_crl_age_is_a_validity_bound_and_the_prover_time_is_a_target() -> None:
    """Two fixed bounds have the value 300: one decides validity, the other never does."""
    age = bound(CONTRACT, "x509.max_crl_age_seconds")
    window = bound(CONTRACT, "x509.max_presentation_window_seconds")
    target = bound(CONTRACT, "x509.prover_target_seconds")
    assert age["value"] == window["value"] == target["value"] == 300
    assert age["owner"]["symbol"] == "ZK_X509_MAX_CRL_AGE_SECONDS_V1"
    assert target["owner"]["symbol"] == "ZK_X509_PROVER_TARGET_SECONDS_V1"
    assert window["refs"] == {"ZK_X509_MAX_CRL_AGE_SECONDS_V1": "x509.max_crl_age_seconds"}
    for item in (age, window):
        assert item["class"] == "consensus" and item["enforcement"] and item["tests"]
        assert {site["stage"] for site in item["enforcement"]} >= {"sdk", "follower"}
    assert target["class"] == "engineering_target" and "enforcement" not in target
    assert relation(CONTRACT, "x509_presentation_window_within_crl_age")["expect"]["today"] == "holds"

    for identifier in ("x509.max_crl_age_seconds", "x509.max_presentation_window_seconds"):

        def relaxed(contract: dict, identifier: str = identifier) -> None:
            bound(contract, identifier)["value"] = 600

        assert_fails(edited(relaxed), f"{identifier}: the delivery plan fixes 300")

        def demoted(contract: dict, identifier: str = identifier) -> None:
            item = bound(contract, identifier)
            item["class"] = "engineering_target"
            item["validity"] = False
            del item["enforcement"]

        assert_fails(edited(demoted), f"{identifier}: a fixed validity bound is classed consensus")

        def untested(contract: dict, identifier: str = identifier) -> None:
            del bound(contract, identifier)["tests"]

        assert_fails(edited(untested), f"{identifier}: a fixed validity bound is classed consensus")

        def removed(contract: dict, identifier: str = identifier) -> None:
            contract["bounds"] = [item for item in contract["bounds"] if item["id"] != identifier]
            contract["relations"] = [
                item for item in contract["relations"] if identifier not in item["expr"]
            ]

        assert_fails(edited(removed), f"{identifier}: the fixed bound is missing")


@pytest.mark.parametrize("identifier", sorted(CHECKER["FIXED_BOUNDS"]))
def test_relaxing_a_fixed_bound_fails(identifier: str) -> None:
    def relax(contract: dict) -> None:
        bound(contract, identifier)["value"] += 1

    assert_fails(edited(relax), f"{identifier}: the delivery plan fixes")

    def unfix(contract: dict) -> None:
        del bound(contract, identifier)["fixed"]

    assert_fails(edited(unfix), f"{identifier}: the bound is recorded `fixed: true`")


@pytest.mark.parametrize("identifier", CHECKER["FRAMED_BOUNDS"])
def test_measured_lengths_state_their_framing(identifier: str) -> None:
    assert bound(CONTRACT, identifier)["framing"]

    def silent(contract: dict) -> None:
        del bound(contract, identifier)["framing"]

    assert_fails(edited(silent), f"{identifier}: states in `framing`")


def test_time_and_memory_targets_can_never_be_consensus_validity() -> None:
    def promote(contract: dict) -> None:
        item = bound(contract, "x509.prover_target_seconds")
        item["class"] = "consensus"
        item["enforcement"] = [
            {"stage": "follower", "path": item["owner"]["path"], "contains": "300"}
        ]

    assert_fails(edited(promote), "is never consensus validity")

    def promote_memory(contract: dict) -> None:
        bound(contract, "x509.prover_peak_rss_bytes")["class"] = "consensus"

    assert_fails(edited(promote_memory), "is never consensus validity")

    def enforce(contract: dict) -> None:
        bound(contract, "x509.prover_target_seconds")["enforcement"] = [
            {"stage": "follower", "path": "specs/zk_resource_contract.json", "contains": "x"}
        ]

    assert_fails(edited(enforce), "an engineering target has no enforcement site")

    def validity(contract: dict) -> None:
        bound(contract, "x509.prover_target_seconds")["validity"] = True

    assert_fails(edited(validity), "an engineering target records `validity: false`")


def test_prover_limits_name_where_the_prover_applies_them() -> None:
    rss = bound(CONTRACT, "x509.prover_peak_rss_bytes")
    assert rss["on_exhaustion"] == "prover_failure" and rss["enforcement"] and rss["tests"]
    assert rss["observations"], "the RSS observation stays recorded as the engineering target"

    def unsited(contract: dict) -> None:
        del bound(contract, "x509.prover_peak_rss_bytes")["enforcement"]

    assert_fails(edited(unsited), "a prover-side limit names where the prover applies it")

    def drifted(contract: dict) -> None:
        bound(contract, "x509.prover_peak_rss_bytes")["observations"][0]["contains"] = "gone"

    assert_fails(edited(drifted), "no longer contains `gone`")


def test_committed_time_budgets_are_local_scheduling_limits() -> None:
    """REQ-117: consensus limits, local scheduling limits and targets are three things."""
    for identifier, on_exceed in (
        ("work_budget.exec_budget_ms", "retry"),
        ("work_budget.apply_budget_ms", "replan"),
    ):
        item = bound(CONTRACT, identifier)
        assert (item["class"], item["source"]) == ("local_schedule", "committed")
        assert item["on_exceed"] == on_exceed and item["validity"] is False
        assert item["enforcement"] and item["tests"]
    sites = {
        site["path"] for site in bound(CONTRACT, "work_budget.exec_budget_ms")["enforcement"]
    }
    assert "crates/iroha_core/src/sumeragi/executor.rs" in sites
    assert "crates/iroha_sumeragi/src/machine/propose.rs" in sites

    def target(contract: dict) -> None:
        bound(contract, "work_budget.exec_budget_ms")["class"] = "engineering_target"

    assert_fails(edited(target), "an engineering target has no enforcement site")

    def unsited(contract: dict) -> None:
        del bound(contract, "work_budget.exec_budget_ms")["enforcement"]

    assert_fails(edited(unsited), "a local scheduling limit names where it is applied")

    def rejecting(contract: dict) -> None:
        bound(contract, "work_budget.exec_budget_ms")["on_exceed"] = "reject"

    assert_fails(edited(rejecting), "records `on_exceed` as `retry` or `replan`")

    def validity(contract: dict) -> None:
        bound(contract, "work_budget.apply_budget_ms")["validity"] = True

    assert_fails(edited(validity), "a local scheduling limit records `validity: false`")

    def untested(contract: dict) -> None:
        del bound(contract, "work_budget.apply_budget_ms")["tests"]

    assert_fails(edited(untested), "a local scheduling limit names its tests")


def test_value_and_expression_drift_fail() -> None:
    def value(contract: dict) -> None:
        bound(contract, "block.max_block_bytes")["value"] = 16 * MIB

    assert_fails(edited(value), "block.max_block_bytes: `max_block_bytes` evaluates to 4194304")

    def expression(contract: dict) -> None:
        bound(contract, "transport.frame_overhead_bytes")["owner"]["expr"] = "64 * 1024"

    assert_fails(edited(expression), "transport.frame_overhead_bytes: `FRAME_OVERHEAD` in")

    def symbol(contract: dict) -> None:
        bound(contract, "transaction.max_tx_bytes")["owner"]["symbol"] = "max_tx_bytes_gone"

    assert_fails(edited(symbol), "`max_tx_bytes_gone` is not defined")

    def path(contract: dict) -> None:
        bound(contract, "transaction.max_tx_bytes")["owner"]["path"] = "crates/missing.rs"

    assert_fails(edited(path), "crates/missing.rs does not exist")


def test_a_failed_reference_fails_every_bound_derived_from_it() -> None:
    def wrong(contract: dict) -> None:
        bound(contract, "transport.max_committee_size")["value"] = 1

    found = problems(edited(wrong))
    assert any(problem.startswith("transport.max_committee_size:") for problem in found)
    assert any(
        "transport.frame_overhead_bytes: `refs` names transport.max_committee_size" in problem
        for problem in found
    )
    # The chain frame limit is derived from the frame overhead in turn.
    assert any("transport.chain_frame_limit_bytes: `refs` names" in problem for problem in found)

    def unknown(contract: dict) -> None:
        bound(contract, "transport.frame_overhead_bytes")["refs"]["MAX_COMMITTEE_SIZE"] = "no.such"

    assert_fails(edited(unknown), "which is not a bound")

    def missing(contract: dict) -> None:
        del bound(contract, "transport.frame_overhead_bytes")["refs"]["MAX_COMMITTEE_SIZE"]

    assert_fails(edited(missing), "has no entry in `refs`")


def test_enforcement_site_and_test_drift_fail() -> None:
    def site(contract: dict) -> None:
        bound(contract, "transaction.max_tx_bytes")["enforcement"][3]["contains"] = (
            "if tx_encoded_len >= max_tx_bytes {"
        )

    assert_fails(edited(site), "no longer contains `if tx_encoded_len >= max_tx_bytes {`")

    def stage(contract: dict) -> None:
        bound(contract, "transaction.max_tx_bytes")["enforcement"][0]["stage"] = "wallet"

    assert_fails(edited(stage), "unknown stage 'wallet'")

    def named(contract: dict) -> None:
        bound(contract, "block.max_block_bytes")["tests"][0]["name"] = "no_such_test"

    assert_fails(edited(named), "test `no_such_test` is not in")

    def stripped(contract: dict) -> None:
        del bound(contract, "block.max_block_bytes")["enforcement"]

    assert_fails(edited(stripped), "a consensus bound lists its enforcement sites")


def test_relation_expectations_are_evaluated_under_every_profile() -> None:
    today = relation(CONTRACT, "transaction_cap_fits_block_payload")
    assert today["expect"] == {"today": "violated", "x2_target": "holds"}
    feasible = relation(CONTRACT, "committed_payload_has_a_feasible_sync_setting")
    assert feasible["expect"] == {"today": "holds", "x2_target": "violated"}

    def flip_today(contract: dict) -> None:
        relation(contract, "transaction_cap_fits_block_payload")["expect"]["today"] = "holds"

    assert_fails(edited(flip_today), "violated under `today`; the contract records holds")

    def flip_target(contract: dict) -> None:
        relation(contract, "sync_response_serves_committed_payload")["expect"][
            "x2_target"
        ] = "holds"

    assert_fails(edited(flip_target), "violated under `x2_target`; the contract records holds")

    def ownerless(contract: dict) -> None:
        del relation(contract, "gossip_frame_carries_max_transaction")["open"]

    assert_fails(edited(ownerless), "is not a task of the delivery graph")

    def unknown(contract: dict) -> None:
        relation(contract, "block_payload_within_rs16_payload")["expr"] = (
            "block.max_block_bytes <= rs16.no_such_bound"
        )

    assert_fails(edited(unknown), "unknown or open bound `rs16.no_such_bound`")

    def not_boolean(contract: dict) -> None:
        relation(contract, "block_payload_within_rs16_payload")["expr"] = (
            "block.max_block_bytes + 1"
        )

    assert_fails(edited(not_boolean), "the expression is not a comparison")

    def profile(contract: dict) -> None:
        contract["profiles"]["x2_target"]["overrides"]["block.no_such"] = 1

    assert_fails(edited(profile), "profile x2_target: unknown bound `block.no_such`")


def test_relation_values_are_the_recorded_arithmetic() -> None:
    values = {item["id"]: item["value"] for item in CONTRACT["bounds"]}
    evaluate = CHECKER["evaluate_relation"]
    # Today: the 10 MiB cap plus the reserve does not fit the 4 MiB payload.
    assert values["transaction.max_tx_bytes"] + 65_536 == 10_551_296
    assert values["block.max_block_bytes"] == 4_194_304
    # The includable bound queue admission and the proposer share today.
    assert (
        evaluate(
            "min(transaction.max_tx_bytes, block.max_block_bytes - "
            "block.payload_transaction_reserve_bytes)",
            values,
            "test",
        )
        == 4_128_768
    )
    assert values["transport.frame_overhead_bytes"] == 264_224
    assert values["transport.chain_frame_limit_bytes"] == 17_041_440
    # A 16 MiB payload needs 17,305,664 bytes of frame for a sync response.
    target = {**values, "block.max_block_bytes": 16 * MIB}
    assert (
        evaluate(
            "block.max_block_bytes + 2 * transport.frame_overhead_bytes", target, "test"
        )
        == 17_305_664
    )
    assert values["transport.max_plaintext_frame_bytes"] == 17_825_792
    # The largest payload for which a feasible sync setting exists today.
    assert (
        evaluate(
            "transport.chain_frame_limit_bytes - 2 * transport.frame_overhead_bytes",
            values,
            "test",
        )
        == 16_512_992
    )
    assert evaluate("ceil_div(7, 2) == 4 and max(1, 2) == 2", values, "test") is True


def test_node_local_validity_bounds_are_defects_owned_by_a_task() -> None:
    defects = {
        item["id"]: item["defect"]["owner"]
        for item in CONTRACT["bounds"]
        if item["class"] == "consensus" and item["source"] == "node_local"
    }
    assert defects, "today's node-local validity bounds are recorded"
    assert set(defects.values()) == {"F.4"}
    assert "work_budget.config_ivm_max_cycles_upper_bound" in defects
    assert "proof.config_stark_max_proof_bytes" in defects

    def undeclared(contract: dict) -> None:
        del bound(contract, "proof.config_pipa_r_max_proof_bytes")["defect"]

    assert_fails(edited(undeclared), "must be recorded as a defect with an owner task")

    def ownerless(contract: dict) -> None:
        bound(contract, "proof.config_pipa_r_max_proof_bytes")["defect"]["owner"] = "Z.9"

    assert_fails(edited(ownerless), "owner `Z.9` is not a task of the delivery graph")


def test_local_resources_defer_or_are_guarded_by_a_relation() -> None:
    for item in CONTRACT["bounds"]:
        if item["class"] != "local_defer":
            continue
        assert item["validity"] is False, item["id"]
        assert item["on_exhaustion"] in ("defer", "local_refusal", "prover_failure")
        if item["on_exhaustion"] == "defer":
            assert item["tests"], item["id"]
        if item["on_exhaustion"] == "local_refusal":
            assert relation(CONTRACT, item["guarded_by"]), item["id"]

    def untested(contract: dict) -> None:
        del bound(contract, "decoded_scratch.ivm_execution_pool_bytes")["tests"]

    assert_fails(edited(untested), "names the test of its deferral")

    def unguarded(contract: dict) -> None:
        bound(contract, "sync.local_sync_max_bytes")["guarded_by"] = "no_such_relation"

    assert_fails(edited(unguarded), "names its relation in `guarded_by`")

    def rejecting(contract: dict) -> None:
        bound(contract, "queue.capacity")["on_exhaustion"] = "reject"

    assert_fails(edited(rejecting), "records `on_exhaustion` as")

    def validity(contract: dict) -> None:
        del bound(contract, "queue.capacity")["validity"]

    assert_fails(edited(validity), "a local resource records `validity: false`")


def test_ram_lfe_budgets_are_derived_and_open_ones_carry_no_number() -> None:
    section = CONTRACT["ram_lfe"]
    by_id = {budget["id"]: budget for budget in section["budgets"]}
    open_ids = {item["id"] for item in section["open_inputs"]}
    assert by_id["ram_lfe.tape_bytes"]["value"] == 256 * 48
    assert by_id["ram_lfe.initial_state_stream_bytes"]["value"] == 32 * 32
    for budget in section["budgets"]:
        is_open = bool(set(budget["inputs"]) & open_ids)
        assert (budget["value"] is None) == is_open, budget["id"]
    assert all(item["value"] is None for item in section["open_inputs"])

    def invented(contract: dict) -> None:
        budget = next(
            item
            for item in contract["ram_lfe"]["budgets"]
            if item["id"] == "ram_lfe.execution_verification_gas"
        )
        budget["value"] = 1_000_000

    assert_fails(edited(invented), "so the value must be null; no number is invented")

    def stale(contract: dict) -> None:
        contract["ram_lfe"]["budgets"][0]["value"] = 1

    assert_fails(edited(stale), "every input is known and the formula gives 12288")

    def undeclared(contract: dict) -> None:
        contract["ram_lfe"]["budgets"][2]["inputs"].pop()

    assert_fails(edited(undeclared), "`inputs` must be exactly the ids the formula uses")

    def closed(contract: dict) -> None:
        contract["ram_lfe"]["open_inputs"][0]["value"] = 4_096

    assert_fails(edited(closed), "a known input is a bound, not an open input")

    def removed(contract: dict) -> None:
        del contract["ram_lfe"]

    assert_fails(edited(removed), "the contract derives the RAM-LFE budgets")


def test_open_items_are_reconciled_when_their_task_is_implemented() -> None:
    assert_fails(CONTRACT, "task X.2 is recorded implemented", {**TASKS, "X.2": "implemented"})
    assert_fails(CONTRACT, "task F.4 is recorded implemented", {**TASKS, "F.4": "implemented"})
    assert_fails(CONTRACT, "task I.1 is recorded implemented", {**TASKS, "I.1": "implemented"})
    assert_fails(CONTRACT, "task C.3 is recorded implemented", {**TASKS, "C.3": "implemented"})
    # A status that is not `implemented` keeps the open items valid.
    assert problems(CONTRACT, {**TASKS, "X.2": "in_progress"}) == []


def test_declared_items_are_not_yet_implemented_and_state_their_requirements() -> None:
    owners = {item["id"]: item["open"]["owner"] for item in CONTRACT["declared"]}
    assert owners == {
        "attachment.signed_inline_occurrences": "I.1",
        "transaction.structured_size_refusal": "S.1",
        "vm_handles.invocation_scoped_attachment_handles": "I.2",
        "work_budget.unified_deterministic_charge": "I.4",
        "block.maximum_proof_transport_profile": "X.2",
    }
    relation_ids = {item["id"] for item in CONTRACT["relations"]}
    for item in CONTRACT["declared"]:
        assert item["status"] == "not_implemented"
        assert item["requires"] and item["today"]
        assert set(item["relations"]) <= relation_ids, item["id"]

    def implemented(contract: dict) -> None:
        contract["declared"][0]["status"] = "implemented"

    assert_fails(edited(implemented), "is a bound with an owner, not `declared`")

    def empty(contract: dict) -> None:
        contract["declared"][0]["requires"] = []

    assert_fails(edited(empty), "a declared item lists what it must satisfy")


def test_every_stage_names_its_tests_or_the_task_that_owns_the_gap() -> None:
    by_stage = {entry["stage"]: entry for entry in CONTRACT["stage_tests"]}
    assert set(by_stage) == set(CHECKER["STAGES"])
    for stage, entry in by_stage.items():
        assert entry["tests"] or entry["open"]["owner"] in TASKS, stage
    # The stages that cannot be exercised below a network say so and name their owner.
    assert by_stage["torii"]["open"]["owner"] == "V.2"
    assert by_stage["storage"]["open"]["owner"] == "V.2"
    assert {entry["id"] for entry in CONTRACT["behaviors"]} == set(CHECKER["BEHAVIORS"])

    def dropped(contract: dict) -> None:
        contract["stage_tests"] = [
            entry for entry in contract["stage_tests"] if entry["stage"] != "follower"
        ]

    assert_fails(edited(dropped), "stage_tests: stage `follower` is missing")

    def untested(contract: dict) -> None:
        entry = next(item for item in contract["stage_tests"] if item["stage"] == "proposer")
        entry["tests"] = []

    assert_fails(edited(untested), "names a test or the task that owns the gap")

    def renamed(contract: dict) -> None:
        entry = next(item for item in contract["stage_tests"] if item["stage"] == "vm")
        entry["tests"][0]["name"] = "no_such_vm_test"

    assert_fails(edited(renamed), "test `no_such_vm_test` is not in")

    def silent(contract: dict) -> None:
        contract["behaviors"] = [
            entry
            for entry in contract["behaviors"]
            if entry["id"] != "missing_local_resource_defers"
        ]

    assert_fails(edited(silent), "`missing_local_resource_defers` names the tests")


def test_schema_group_and_duplicate_checks() -> None:
    assert problems({**CONTRACT, "schema": "other"}) == [
        "schema is 'other', expected 'iroha.zk_resource_contract.v1'"
    ]

    def duplicate(contract: dict) -> None:
        contract["bounds"].append(copy.deepcopy(contract["bounds"][0]))

    assert_fails(edited(duplicate), "transaction.max_tx_bytes: duplicate bound id")

    def no_storage(contract: dict) -> None:
        contract["bounds"] = [item for item in contract["bounds"] if item["group"] != "storage"]

    assert_fails(edited(no_storage), "group `storage` has no bound or declared item")

    def no_torii(contract: dict) -> None:
        for item in contract["bounds"]:
            item["enforcement"] = [
                site for site in item.get("enforcement", []) if site["stage"] != "torii"
            ]

    assert_fails(edited(no_torii), "stage `torii` has no enforcement site")

    def unknown_class(contract: dict) -> None:
        bound(contract, "queue.capacity")["class"] = "soft"

    assert_fails(edited(unknown_class), "unknown class 'soft'")


def test_rust_expression_reader_and_evaluator(tmp_path: Path) -> None:
    source = """
    /// Owner.
    pub const BASE: usize = 4 * 1024; // four KiB
    pub const DERIVED: u64 =
        (BASE as u64 + crate::other::EXTRA as u64)
            * 2;
    pub mod first { pub const LIMIT: u32 = 1; }
    pub mod second {
        pub const LIMIT: NonZeroU64 = nonzero!(8_u64 * 2_u64.pow(10));
        pub const fn cap() -> NonZeroU32 {
            // a comment inside the body
            nonzero_ext::nonzero!(0x0010_0000_u32)
        }
        pub fn same() -> NonZeroU32 { cap() }
    }
    impl Default for Limits {
        fn default() -> Self {
            Self { small: 1 << 4, large: NonZeroUsize::new(4096).expect("non-zero") }
        }
    }
    fn policy(self) -> u64 { match self { Self::V1 => 64 * 1024, } }
    """
    extract = CHECKER["extract_expression"]
    evaluate = CHECKER["evaluate_rust"]

    def read(symbol: str, kind: str = "const", anchor: str | None = None) -> str:
        owner = {"path": "x.rs", "symbol": symbol, "kind": kind}
        if anchor:
            owner["anchor"] = anchor
        return extract(source, owner, symbol)

    assert read("BASE") == "4 * 1024"
    assert evaluate(read("BASE"), {}, {}, "t") == 4_096
    derived = read("DERIVED")
    assert derived == "(BASE as u64 + crate::other::EXTRA as u64) * 2"
    values = {"a.base": 4_096, "a.extra": 4}
    refs = {"BASE": "a.base", "crate::other::EXTRA": "a.extra"}
    assert evaluate(derived, refs, values, "t") == 8_200
    # An anchor selects the second of two equal names.
    assert evaluate(read("LIMIT"), {}, {}, "t") == 1
    assert evaluate(read("LIMIT", anchor="pub mod second {"), {}, {}, "t") == 8_192
    assert read("cap", "fn") == "nonzero_ext::nonzero!(0x0010_0000_u32)"
    assert evaluate(read("cap", "fn"), {}, {}, "t") == 1_048_576
    assert evaluate(read("same", "fn"), {"cap": "a.cap"}, {"a.cap": 7}, "t") == 7
    assert evaluate(read("small", "field", "impl Default for Limits"), {}, {}, "t") == 16
    assert evaluate(read("large", "field", "impl Default for Limits"), {}, {}, "t") == 4_096
    assert evaluate(read("Self::V1", "arm", "fn policy"), {}, {}, "t") == 65_536
    error = CHECKER["ContractError"]
    with pytest.raises(error, match="has no entry in `refs`"):
        evaluate(derived, {}, {}, "t")
    with pytest.raises(error, match="refers to unverified bound"):
        evaluate(derived, refs, {"a.base": 1}, "t")
    with pytest.raises(error, match="is not defined"):
        read("ABSENT")
    with pytest.raises(error, match="anchor"):
        read("LIMIT", anchor="pub mod third {")
    with pytest.raises(error, match="is not integer arithmetic"):
        evaluate("1 ^ 2", {}, {}, "t")
    # Only arithmetic and comparisons evaluate; nothing else is executed.
    with pytest.raises(error):
        CHECKER["safe_eval"]("__import__('os').getcwd()", "t")
    with pytest.raises(error):
        CHECKER["evaluate_relation"]("a.base.__class__ == 1", {"a.base": 1}, "t")


def test_checker_reads_a_source_tree_and_reports_drift_there(tmp_path: Path) -> None:
    (tmp_path / "a.rs").write_text(
        "pub const LIMIT: u64 = 4 * 1024;\nfn admit(len: u64) -> bool { len > LIMIT }\n"
        "#[test]\nfn limit_is_inclusive() {}\n",
        encoding="utf-8",
    )
    contract = {
        "bounds": [
            {
                "id": "queue.limit",
                "group": "queue",
                "what": "A limit.",
                "unit": "bytes",
                "value": 4_096,
                "source": "constant",
                "class": "consensus",
                "owner": {"path": "a.rs", "symbol": "LIMIT", "kind": "const", "expr": "4 * 1024"},
                "enforcement": [{"stage": "queue", "path": "a.rs", "contains": "len > LIMIT"}],
                "tests": [{"path": "a.rs", "name": "limit_is_inclusive"}],
            }
        ]
    }
    found: list = []
    assert CHECKER["verify_bounds"](tmp_path, contract, found) == {"queue.limit": 4_096}
    CHECKER["_check_sites"](tmp_path, contract["bounds"][0], "queue.limit", found)
    assert found == []
    # The owner changes its value: the same contract now fails.
    (tmp_path / "a.rs").write_text(
        "pub const LIMIT: u64 = 8 * 1024;\nfn admit(len: u64) -> bool { len >= LIMIT }\n",
        encoding="utf-8",
    )
    assert CHECKER["verify_bounds"](tmp_path, contract, found) == {}
    CHECKER["_check_sites"](tmp_path, contract["bounds"][0], "queue.limit", found)
    assert len(found) == 3
    assert "is now `8 * 1024`; the contract records `4 * 1024`" in found[0]
    assert "no longer contains `len > LIMIT`" in found[1]
    assert "test `limit_is_inclusive` is not in a.rs" in found[2]


def test_a_named_test_must_be_an_active_test(tmp_path: Path) -> None:
    """A named test that lost `#[test]` or gained `#[ignore]` no longer backs a bound."""
    problem = CHECKER["test_problem"]
    active = (
        "fn helper() {}\n\n/// Documented; with a semicolon.\n#[test]\n"
        "#[allow(clippy::too_many_lines)]\nfn limit_is_inclusive() {\n    helper();\n}\n"
    )
    assert problem(active, "a.rs", "limit_is_inclusive") is None
    assert problem(active.replace("#[test]", "#[tokio::test(flavor = \"current_thread\")]"),
                   "a.rs", "limit_is_inclusive") is None
    assert problem(active.replace("fn limit", "async fn limit"), "a.rs", "limit_is_inclusive") is None
    assert problem(active, "a.rs", "no_such") == "is not in"
    ignored = active.replace("#[test]", "#[test]\n#[ignore = \"slow\"]")
    assert problem(ignored, "a.rs", "limit_is_inclusive") == "is `#[ignore]`d in"
    assert problem(active.replace("#[test]", "#[ignore]\n#[test]"), "a.rs",
                   "limit_is_inclusive") == "is `#[ignore]`d in"
    conditional = active.replace("#[test]", "#[test]\n#[cfg_attr(not(feature = \"slow\"), ignore)]")
    assert problem(conditional, "a.rs", "limit_is_inclusive") == "is `#[ignore]`d in"
    plain = active.replace("#[test]\n", "")
    assert problem(plain, "a.rs", "limit_is_inclusive") == "has no `#[test]` attribute in"
    # The attribute of the previous test does not leak into the next function.
    neighbours = "#[test]\nfn first() {}\nfn second() {}\n"
    assert problem(neighbours, "a.rs", "first") is None
    assert problem(neighbours, "a.rs", "second") == "has no `#[test]` attribute in"
    # A helper named like the test in a comment or a call is not a definition.
    assert problem("// fn ghost() {}\nfn other() { ghost(); }\n", "a.rs", "ghost") is not None
    # A test generated by a `macro_rules!` of the same file whose arms carry `#[test]`.
    generated = (
        "macro_rules! state_test {\n    (sync $name:ident $($body:tt)*) => {\n        #[test]\n"
        "        fn $name() { $($body)* }\n    };\n}\n"
        "state_test! { sync fanout_rejects_one_over\n    let limit = 256;\n}\n"
    )
    assert problem(generated, "a.rs", "fanout_rejects_one_over", "state_test") is None
    assert problem(generated, "a.rs", "fanout_rejects_one_over") == "is not in"
    assert problem(generated, "a.rs", "absent", "state_test") == "is not in"
    assert "is not defined" in problem(generated, "a.rs", "fanout_rejects_one_over", "other_test")
    assert "generates no `#[test]`" in problem(
        generated.replace("#[test]\n", ""), "a.rs", "fanout_rejects_one_over", "state_test"
    )
    assert "`#[ignore]`d test" in problem(
        generated.replace("#[test]", "#[test]\n        #[ignore]"),
        "a.rs",
        "fanout_rejects_one_over",
        "state_test",
    )
    python = "import pytest\n\n\ndef test_limit() -> None:\n    pass\n"
    assert problem(python, "a.py", "test_limit") is None
    skipped = python.replace("def test_limit", "@pytest.mark.skip(reason='x')\ndef test_limit")
    assert problem(skipped, "a.py", "test_limit") == "is skipped in"
    assert problem(python, "a.py", "test_other") == "is not in"

    # Through the checker: the same record fails once the test is ignored or loses its attribute.
    record = {"tests": [{"path": "a.rs", "name": "limit_is_inclusive"}]}
    for source, expected in (
        (active, []),
        (ignored, ["x: test `limit_is_inclusive` is `#[ignore]`d in a.rs"]),
        (plain, ["x: test `limit_is_inclusive` has no `#[test]` attribute in a.rs"]),
    ):
        (tmp_path / "a.rs").write_text(source + f"// {len(source)}\n" * 2, encoding="utf-8")
        found: list = []
        CHECKER["_check_sites"](tmp_path, record, "x", found)
        assert found == expected

    # `#[cfg(any())]` is never true: the test is not compiled.
    compiled_out = active.replace("#[test]", "#[cfg(any())]\n#[test]")
    assert problem(compiled_out, "a.rs", "limit_is_inclusive") == (
        "is compiled out by `#[cfg(any())]` in"
    )
    # A generator defined in the file that includes the test file, and the `name { .. }` form.
    included = "world_test!(ballot_cap_rejects_one_over {\n    let cap = 1;\n});\n"
    definition = (
        "macro_rules! world_test {\n    ($name:ident $body:block) => {\n        #[test]\n"
        "        fn $name() $body\n    };\n}\ninclude!(\"world_tests.rs\");\n"
    )
    assert problem(included, "a.rs", "ballot_cap_rejects_one_over", "world_test", definition) is None
    assert "is not defined" in problem(included, "a.rs", "ballot_cap_rejects_one_over", "world_test")
    assert problem(included, "a.rs", "absent", "world_test", definition) == "is not in"

    # Every test the tracked contract names is an active test.
    def named_tests(node):
        if isinstance(node, dict):
            if set(node) >= {"path", "name"} and "contains" not in node:
                yield node
            for value in node.values():
                yield from named_tests(value)
        elif isinstance(node, list):
            for value in node:
                yield from named_tests(value)

    names = {(node["path"], node["name"]): node for node in named_tests(CONTRACT)}
    assert len(names) > 170
    for (path, name), node in sorted(names.items()):
        assert CHECKER["_named_test_problem"](ROOT, node, "x") is None, (path, name)
        assert not CHECKER["unreachable_test_file"](ROOT, path), path


def test_a_named_test_file_must_be_part_of_its_crate(tmp_path: Path) -> None:
    """A test in a file no `mod`, `include!`, `#[path]` or test target reaches never runs."""
    unreachable = CHECKER["unreachable_test_file"]
    crate = tmp_path / "crates" / "demo"
    (crate / "src" / "queue").mkdir(parents=True)
    (crate / "tests" / "grouped").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        "[package]\nname = \"demo\"\nautotests = false\n\n"
        "[[test]]\nname = \"group\"\npath = \"tests/grouped/group.rs\"\n",
        encoding="utf-8",
    )
    files = {
        "src/lib.rs": "mod queue;\n#[cfg(test)]\n#[path = \"../tests/in_lib.rs\"]\nmod in_lib;\n",
        "src/queue.rs": "mod limits;\n#[cfg(test)]\ninclude!(\"queue/included_tests.rs\");\n",
        "src/queue/limits.rs": "#[test]\nfn in_module() {}\n",
        "src/queue/included_tests.rs": "#[test]\nfn included() {}\n",
        "src/queue/orphan_tests.rs": "#[test]\nfn orphan() {}\n",
        "tests/in_lib.rs": "#[test]\nfn in_lib() {}\n",
        "tests/grouped/group.rs": "#[path = \"../grouped_case.rs\"]\nmod grouped_case;\n",
        "tests/grouped_case.rs": "#[test]\nfn grouped() {}\n",
        "tests/unlisted.rs": "#[test]\nfn unlisted() {}\n",
    }
    for name, text in files.items():
        (crate / name).write_text(text, encoding="utf-8")

    def judged(name: str) -> bool:
        CHECKER["_REACHABLE"].clear()
        CHECKER["_REFERENCES"].clear()
        return unreachable(tmp_path, f"crates/demo/{name}")

    for reached in (
        "src/lib.rs",
        "src/queue/limits.rs",
        "src/queue/included_tests.rs",
        "tests/in_lib.rs",
        "tests/grouped/group.rs",
        "tests/grouped_case.rs",
    ):
        assert not judged(reached), reached
    # No module names the file; `autotests = false` and no `[[test]]` entry.
    assert judged("src/queue/orphan_tests.rs")
    assert judged("tests/unlisted.rs")
    # A file outside every crate (a scratch source) and a Python test are not judged.
    (tmp_path / "loose.rs").write_text("#[test]\nfn loose() {}\n", encoding="utf-8")
    assert not unreachable(tmp_path, "loose.rs")
    assert not unreachable(tmp_path, "scripts/tests/x.py")

    # Through the checker.
    record = {"tests": [{"path": "crates/demo/src/queue/included_tests.rs", "name": "included"}]}
    found: list = []
    CHECKER["_REACHABLE"].clear()
    CHECKER["_check_sites"](tmp_path, record, "x", found)
    assert found == []
    # Removing the `include!` leaves the file on disk and the test out of the build.
    (crate / "src" / "queue.rs").write_text("mod limits;\n// tests removed\n", encoding="utf-8")
    CHECKER["_REACHABLE"].clear()
    CHECKER["_REFERENCES"].clear()
    CHECKER["_check_sites"](tmp_path, record, "x", found)
    assert len(found) == 1 and "which no `mod`, `include!`, `#[path]`" in found[0]
    # A reference that is itself compiled out does not count.
    (crate / "src" / "queue.rs").write_text(
        "mod limits;\n#[cfg(any())]\ninclude!(\"queue/included_tests.rs\");\n", encoding="utf-8"
    )
    CHECKER["_REACHABLE"].clear()
    CHECKER["_REFERENCES"].clear()
    assert unreachable(tmp_path, "crates/demo/src/queue/included_tests.rs")


def test_a_site_that_only_repeats_the_definition_is_rejected() -> None:
    definition_only = CHECKER["_definition_only"]
    owner = {"path": "a.rs", "symbol": "MAX_FRAME_BYTES", "kind": "const"}
    in_owner = lambda text: {"path": "a.rs", "contains": text}  # noqa: E731
    assert definition_only(in_owner("pub const MAX_FRAME_BYTES: usize = 4 * 1024;"), owner)
    assert definition_only(in_owner("MAX_FRAME_BYTES"), owner)
    assert definition_only({"path": "b.rs", "contains": "crate::a::MAX_FRAME_BYTES,"}, owner)
    assert not definition_only(in_owner("if len > MAX_FRAME_BYTES {"), owner)
    assert not definition_only({"path": "b.rs", "contains": "if len > MAX_FRAME_BYTES {"}, owner)
    default = {"path": "a.rs", "symbol": "max_tx_bytes", "kind": "fn"}
    assert definition_only({"path": "a.rs", "contains": "pub const fn max_tx_bytes() -> u64 {"}, default)
    field = {"path": "a.rs", "symbol": "max_bytes", "kind": "field"}
    assert definition_only({"path": "a.rs", "contains": "max_bytes: 1 << 30,"}, field)
    # The definition of any other constant is no comparison either: an alias, a derived
    # constant, with or without its semicolon, in any file and without an owner.
    for text in (
        "const MAX: usize = MAX_FRAME_BYTES;",
        "pub(crate) const STRICT_INIT_MAX_BLOCK_BYTES: u64 = MAX_FRAME_BYTES;",
        "const DEPTH: usize = MAX_FRAME_BYTES.ilog2() as usize",
        "pub const OUTPUT_START: u64 = Self::INPUT_START + Self::INPUT_SIZE;",
        "static LIMITS: DecodeLimits = DecodeLimits::new( MAX_FRAME_BYTES, MAX_FRAME_BYTES * 8, );",
    ):
        assert definition_only({"path": "b.rs", "contains": text}, owner), text
        assert definition_only({"path": "b.rs", "contains": text}), text
    # A function signature names where a check lives, not the check.
    for text in (
        "pub const fn check_signed_transaction_bytes(",
        "pub const fn from_parameters(parameters: SmartContractParameters) -> Self {",
        "pub fn ivm_execution_budget(&self) -> iroha_allocation::AllocationBudget {",
        "fn effective<T: Limits>(limits: &T) -> usize",
    ):
        assert definition_only({"path": "b.rs", "contains": text}, owner), text
    # A compile-time assertion compares; so do a statement after a constant, a function with
    # its body and the constant a trait implementation binds.
    for text in (
        "const _: () = assert!(MAX_FRAME_BYTES.is_power_of_two());",
        "const LIMIT: usize = MAX_FRAME_BYTES; if len > LIMIT {",
        "fn effective(limits: &Limits) -> usize { limits.max_len.min(MAX_FRAME_BYTES) }",
        "impl SharedDomain for FrameDomain { const MAX: usize = MAX_FRAME_BYTES; }",
    ):
        assert not definition_only({"path": "b.rs", "contains": text}, owner), text

    def defined(contract: dict) -> None:
        item = bound(contract, "block.max_evidence_frame_bytes")
        item["enforcement"] = [
            {
                "stage": "follower",
                "path": item["owner"]["path"],
                "contains": "pub const MAX_EVIDENCE_FRAME_BYTES: usize = 4 * 1024 * 1024;",
            }
        ]

    rejected = "is only a symbol, a definition or a function signature"
    assert_fails(edited(defined), rejected)

    def bare(contract: dict) -> None:
        item = bound(contract, "guest_memory.max_call_frame_bytes")
        item["enforcement"] = [
            {"stage": "vm", "path": item["owner"]["path"], "contains": "MAX_CALL_FRAME_BYTES_V1"}
        ]

    assert_fails(edited(bare), rejected)

    def alias(contract: dict) -> None:
        # The verifier's case: the zk-X509 proof ceiling backed by an alias definition.
        bound(contract, "x509.max_proof_bytes")["enforcement"] = [
            {
                "stage": "follower",
                "path": "crates/iroha_core_privacy/src/privacy_engines/mod.rs",
                "contains": "self::zk_x509::profile::ZK_X509_MAX_PROOF_BYTES_V1 as usize;",
            }
        ]

    assert problems(edited(alias)) == []  # an expression fragment is still text the file holds

    def alias_item(contract: dict) -> None:
        bound(contract, "rs16.max_availability_frame_bytes")["enforcement"] = [
            {
                "stage": "rs16",
                "path": "crates/iroha_sumeragi/src/availability/artifact.rs",
                "contains": "const MAX: usize = MAX_AVAILABILITY_FRAME_BYTES;",
            }
        ]

    assert_fails(edited(alias_item), rejected)

    def signature(contract: dict) -> None:
        # The rule covers every record with sites, not only bounds.
        declared = next(
            item for item in contract["declared"] if item["id"] == "attachment.signed_inline_occurrences"
        )
        declared["enforcement"][0]["contains"] = (
            "pub fn structural_error(&self) -> Option<(&'static str, &'static str)> {"
        )

    assert_fails(edited(signature), rejected)
    # No record of the tracked contract is backed by such a site.
    for item in CONTRACT["bounds"]:
        for site in item.get("enforcement", []):
            assert not definition_only(site, item["owner"]), (item["id"], site["contains"])
    # The zk-X509 proof ceiling names the encoder and the decoder comparison.
    sites = {site["contains"] for site in bound(CONTRACT, "x509.max_proof_bytes")["enforcement"]}
    assert "if encoded_length > ZK_X509_MAX_PROOF_BYTES_V1 as usize {" in sites
    assert "if encoded.len() > ZK_X509_MAX_PROOF_BYTES_V1 as usize {" in sites


def test_a_site_records_how_often_its_text_occurs(tmp_path: Path) -> None:
    """Removing one of two identical comparisons is drift."""
    twice = "fn a(len: usize) { if len > LIMIT {} }\nfn b(len: usize) { if len > LIMIT {} }\n"
    (tmp_path / "a.rs").write_text(twice, encoding="utf-8")
    site = {"stage": "queue", "path": "a.rs", "contains": "if len > LIMIT {", "count": 2}
    record = {"enforcement": [site]}
    found: list = []
    CHECKER["_check_sites"](tmp_path, record, "x", found)
    assert found == []
    once = twice.replace("fn b(len: usize) { if len > LIMIT {} }", "fn b(len: usize) {}")
    (tmp_path / "a.rs").write_text(once, encoding="utf-8")
    CHECKER["_check_sites"](tmp_path, record, "x", found)
    assert found == ["x: a.rs contains `if len > LIMIT {` 1 time(s); the contract records 2"]
    # A site without a count occurs exactly once; an explicit count of one is not written.
    found = []
    CHECKER["_check_sites"](tmp_path, {"enforcement": [{**site, "count": 1}]}, "x", found)
    assert len(found) == 1 and "a site that occurs once records none" in found[0]
    (tmp_path / "a.rs").write_text(twice, encoding="utf-8")
    plain = {"stage": "queue", "path": "a.rs", "contains": "if len > LIMIT {"}
    found = []
    CHECKER["_check_sites"](tmp_path, {"enforcement": [plain]}, "x", found)
    assert found == ["x: a.rs contains `if len > LIMIT {` 2 time(s); the contract records 1"]
    # `--refresh` rewrites the count of a site that is still present and reports it.
    notes: list = []
    found = []
    CHECKER["_check_sites"](tmp_path, {"enforcement": [plain]}, "x", found, None, True, notes)
    assert found == [] and plain["count"] == 2
    assert len(notes) == 1 and "now occurs 2 time(s), was 1" in notes[0]
    (tmp_path / "a.rs").write_text(once, encoding="utf-8")
    CHECKER["_check_sites"](tmp_path, {"enforcement": [plain]}, "x", found, None, True, notes)
    assert "count" not in plain and len(notes) == 2
    # A site that is gone is never refreshed away.
    (tmp_path / "a.rs").write_text("fn a() {}\n", encoding="utf-8")
    CHECKER["_check_sites"](tmp_path, {"enforcement": [plain]}, "x", found, None, True, notes)
    assert found == ["x: a.rs no longer contains `if len > LIMIT {`"]

    # The tracked contract: the verifier's case and the lane batch comparison.
    frame = next(
        site
        for site in bound(CONTRACT, "attachment.list_max_canonical_frame_bytes")["enforcement"]
        if site["contains"].startswith("if canonical_frame_len >")
    )
    assert frame["count"] == 2

    def miscounted(contract: dict) -> None:
        next(
            site
            for site in bound(contract, "attachment.list_max_canonical_frame_bytes")["enforcement"]
            if site["contains"].startswith("if canonical_frame_len >")
        )["count"] = 3

    assert_fails(edited(miscounted), "2 time(s); the contract records 3")
    lane = bound(CONTRACT, "block.lane_batch_framing_reserve_bytes")
    assert "if length <= max_bytes as usize {" in {site["contains"] for site in lane["enforcement"]}
    assert "build_trims_the_batch_to_the_lane_payload_limit" in {test["name"] for test in lane["tests"]}
    counted = [
        site
        for item in CONTRACT["bounds"]
        for site in item.get("enforcement", [])
        if "count" in site
    ]
    assert len(counted) >= 18 and all(site["count"] > 1 for site in counted)


def test_every_consensus_bound_names_a_test_or_the_owner_of_the_gap() -> None:
    consensus = [item for item in CONTRACT["bounds"] if item["class"] == "consensus"]
    pins = CHECKER["pin_tests"](CONTRACT)
    assert len(pins) == len(CONTRACT["pin_tests"]) >= 13

    def only_pins(item: dict) -> bool:
        tests = item.get("tests") or []
        return bool(tests) and all((test["path"], test["name"]) in pins for test in tests)

    tested = [item for item in consensus if item.get("tests") and not only_pins(item)]
    pinned = [item for item in consensus if only_pins(item)]
    gaps = {item["id"]: item["test_gap"]["owner"] for item in consensus if "test_gap" in item}
    assert set(gaps.values()) <= set(TASKS)
    # A bound with a test of its boundary records no gap; one without names the gap's owner.
    assert not [item["id"] for item in tested if "test_gap" in item]
    for item in consensus:
        if not item.get("tests"):
            assert item["id"] in gaps, item["id"]
    # A bound whose every test is a pin names the owner of the boundary test, is recorded
    # unenforced, or states why a pin is the right test.
    assert len(pinned) >= 15
    for item in pinned:
        assert "test_gap" in item or "unenforced" in item or item.get("value_pin"), item["id"]
    assert {item["id"] for item in pinned if item.get("value_pin")} == {
        "block.core_default_max_block_bytes",
        "guest_memory.data_model_heap_max_bytes",
    }
    # The bounds the verifier found backed by a pin alone now name a boundary test or a gap.
    for identifier, name in (
        ("work_budget.fuel", "validate_ivm_max_cycles_exceeds_fuel_rejected"),
        ("work_budget.fuel", "validate_ivm_max_cycles_equal_to_fuel_is_admitted"),
        ("guest_memory.committed_heap_limit_bytes", "per_instance_heap_ceiling_cannot_be_bypassed_by_growth"),
        ("proof.config_pipa_r_max_proof_bytes", "guardrails_enforce_pipa_r_max_proof_bytes_for_open_verify_envelopes"),
        ("proof.config_ballot_history_cap", "direct_zk_ballot_rejects_a_full_corpus_without_pruning"),
        (
            "work_budget.config_confidential_registry_max_delta_per_block",
            "confidential_registry_delta_cap_limits_transitions",
        ),
    ):
        item = bound(CONTRACT, identifier)
        assert name in {test["name"] for test in item["tests"]}, identifier
        assert not only_pins(item) and "test_gap" not in item
    for identifier in (
        "proof.config_preverify_budget_bytes",
        "work_budget.config_query_max_fetch_size",
        "work_budget.config_amx_group_budget_ms",
        "work_budget.config_confidential_max_nullifiers_per_tx",
        "work_budget.config_confidential_max_commitments_per_tx",
        "transaction.max_instructions",
    ):
        item = bound(CONTRACT, identifier)
        assert only_pins(item) and item["test_gap"]["owner"] in TASKS, identifier

    def unowned(contract: dict) -> None:
        del bound(contract, "proof.config_preverify_budget_bytes")["test_gap"]

    assert_fails(edited(unowned), "its tests only pin the value or a digest projection")

    def pin_claimed_as_boundary(contract: dict) -> None:
        # A pin test cannot be relabelled by dropping it from the list while a bound names it
        # as its only evidence: the list is checked both ways.
        contract["pin_tests"] = [
            test for test in contract["pin_tests"] if test["name"] != "t_req_defaults_match_spec"
        ] + [{"path": "scripts/tests/check_zk_resource_contract_test.py", "name": "absent", "pins": "value", "what": "x"}]

    assert_fails(edited(pin_claimed_as_boundary), "pin_tests absent: is not in")

    def unused_pin(contract: dict) -> None:
        contract["pin_tests"].append(
            {
                "path": "scripts/tests/check_zk_resource_contract_test.py",
                "name": "test_tracked_contract_matches_the_source",
                "pins": "value",
                "what": "x",
            }
        )

    assert_fails(edited(unused_pin), "no bound names this test")

    def gap_beside_a_boundary_test(contract: dict) -> None:
        bound(contract, "work_budget.fuel")["test_gap"] = {"owner": "I.4", "summary": "x"}

    assert_fails(
        edited(gap_beside_a_boundary_test),
        "work_budget.fuel: a bound with a test of its boundary records no `test_gap`",
    )

    def pin_reason_beside_a_boundary_test(contract: dict) -> None:
        bound(contract, "work_budget.fuel")["value_pin"] = "x"

    assert_fails(edited(pin_reason_beside_a_boundary_test), "`value_pin` is for a bound whose every test is a pin")

    def fixed_bound_backed_by_a_pin(contract: dict) -> None:
        bound(contract, "transaction.max_tx_bytes")["tests"] = [
            {
                "path": "crates/iroha_data_model/tests/resource_contract_v1.rs",
                "name": "committed_defaults_are_the_values_the_contract_records",
            }
        ]

    assert_fails(
        edited(fixed_bound_backed_by_a_pin),
        "transaction.max_tx_bytes: a fixed validity bound names a test of its boundary",
    )
    # The committed output budget is backed by tests of that budget, not of the overlay cap.
    for identifier in ("queued_effects.max_output_items", "queued_effects.max_output_bytes"):
        names = {entry["name"] for entry in bound(CONTRACT, identifier)["tests"]}
        assert "committed_output_budget_accepts_the_exact_limit_and_refuses_one_over" in names
        assert not any(name.startswith("plain_instruction_group_checks") for name in names)
    overlay = {
        entry["name"]
        for entry in bound(CONTRACT, "queued_effects.config_overlay_max_instructions")["tests"]
    }
    assert "plain_instruction_group_checks_exact_count_before_first_effect" in overlay
    # The zk-X509 proof ceiling names tests of the engine constant.
    engine = {entry["path"] for entry in bound(CONTRACT, "x509.max_proof_bytes")["tests"]}
    assert any("privacy_engines/zk_x509/" in path for path in engine)

    def untested(contract: dict) -> None:
        del bound(contract, "transaction.max_signatures")["tests"]

    assert_fails(edited(untested), "transaction.max_signatures: a consensus bound names a test")

    def both(contract: dict) -> None:
        bound(contract, "transaction.max_signatures")["test_gap"] = {
            "owner": "V.3",
            "summary": "x",
        }

    assert_fails(edited(both), "a bound with a test of its boundary records no `test_gap`")

    def ownerless(contract: dict) -> None:
        item = bound(contract, "transaction.max_signatures")
        del item["tests"]
        item["test_gap"] = {"owner": "Z.9", "summary": "x"}

    assert_fails(edited(ownerless), "test_gap: owner `Z.9` is not a task")
    gap = next(iter(gaps))
    assert_fails(
        CONTRACT,
        f"{gap}: test_gap: task {gaps[gap]} is recorded implemented",
        {**TASKS, gaps[gap]: "implemented"},
    )


def test_a_value_bound_into_state_but_compared_nowhere_is_recorded_unenforced() -> None:
    unenforced = {item["id"] for item in CONTRACT["bounds"] if "unenforced" in item}
    assert unenforced == {
        "queued_effects.config_overlay_chunk_instructions",
        "proof.config_confidential_max_public_inputs",
        "proof.config_confidential_registry_max_vk_entries",
        "proof.config_confidential_registry_max_params_entries",
        "proof.config_private_settlement_max_proof_bytes",
        "proof.config_private_settlement_max_capsule_bytes",
        "storage.config_private_settlement_sidecar_max_records",
        "storage.config_private_settlement_sidecar_max_total_bytes",
    }
    for identifier in unenforced:
        item = bound(CONTRACT, identifier)
        assert "enforcement" not in item and item["unenforced"]["owner"] == "F.4"
    # The settlement leg limits are compared by Torii's service only: an observation, never
    # an enforcement site of a state transition.
    proof = bound(CONTRACT, "proof.config_private_settlement_max_proof_bytes")
    assert proof["value"] == 8 * MIB and len(proof["observations"]) == 2
    assert all("iroha_torii" in entry["path"] for entry in proof["observations"])

    def sited(contract: dict) -> None:
        item = bound(contract, "proof.config_confidential_max_public_inputs")
        item["enforcement"] = bound(contract, "proof.config_pipa_r_max_proof_bytes")["enforcement"]

    assert_fails(edited(sited), "a bound with enforcement sites is not `unenforced`")

    def observation_gone(contract: dict) -> None:
        bound(contract, "proof.config_private_settlement_max_proof_bytes")["observations"][0][
            "contains"
        ] = "|| request.payload.proof.len() as u64 > config.no_such_limit.get()"

    assert_fails(edited(observation_gone), "private_settlement.rs no longer contains")


def test_every_limit_in_an_owner_file_is_listed_or_excluded() -> None:
    """The inventory cannot silently omit a limit an owner file or directory defines."""
    section = CONTRACT["completeness"]
    scopes = CHECKER["expand_scopes"](ROOT, section, [])
    scanned = {scope["path"] for scope in scopes}
    for item in CONTRACT["bounds"]:
        if item["owner"]["kind"] == "const":
            assert item["owner"]["path"] in scanned, item["id"]
    for group in section["excluded"]:
        holders = group["files"] if "files" in group else [group]
        assert group["reason"] and all(holder["path"] and holder["symbols"] for holder in holders)
    # The bounds the first inventory missed are listed.
    for identifier, symbol in (
        ("attachment.list_max_canonical_frame_bytes", "PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1"),
        ("attachment.list_max_attachments", "PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1"),
        ("decoded_scratch.config_ivm_max_decoded_bytes", "IVM_MAX_DECODED_BYTES"),
        ("work_budget.block_gas_limit_default", "DEFAULT_GAS_LIMIT_PER_BLOCK"),
        ("work_budget.fastpq_bootstrap_network_inputs", "BOOTSTRAP_NETWORK_INPUTS"),
        # Found by the directory scan of the second repair.
        ("block.max_evidence_admissions_per_block", "MAX_EVIDENCE_ADMISSIONS_PER_BLOCK"),
        ("block.max_evidence_admission_bytes", "MAX_EVIDENCE_ADMISSION_BYTES"),
        ("block.authenticated_proofs_max_block_wire_bytes", "AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1"),
        ("sync.max_proposal_bytes", "MAX_PROPOSAL_BYTES"),
        ("decoded_scratch.max_private_input_record_bytes", "MAX_PRIVATE_INPUT_RECORD_BYTES_V1"),
        ("decoded_scratch.max_private_inputs", "MAX_PRIVATE_INPUTS_V1"),
        ("decoded_scratch.max_private_input_transport_bytes", "MAX_PRIVATE_INPUT_TRANSPORT_BYTES_V1"),
        ("work_budget.max_contract_call_depth", "MAX_CONTRACT_CALL_DEPTH"),
        ("transport.p2p_max_encrypted_frame_bytes", "MAX_ENCRYPTED_FRAME_BYTES"),
    ):
        assert bound(CONTRACT, identifier)["owner"]["symbol"] == symbol
    for field in ("max_outputs", "max_total_output_bytes", "max_time_invocations"):
        assert bound(CONTRACT, f"block.execution_output_{field}")["source"] == "committed"
    assert bound(CONTRACT, "block.max_time_trigger_invocations")["value"] == 512
    assert bound(CONTRACT, "block.max_evidence_admissions_per_block")["value"] == 8
    assert bound(CONTRACT, "sync.max_proposal_bytes")["value"] == 64 * MIB
    assert bound(CONTRACT, "block.authenticated_proofs_max_block_wire_bytes")["value"] == 32 * MIB
    # The executed block wire may exceed what the block-proof carrier serves: recorded open.
    carrier = relation(CONTRACT, "authenticated_block_proofs_carry_an_executed_block")
    assert carrier["expect"] == {"today": "violated", "x2_target": "violated"}
    assert carrier["open"]["owner"] in TASKS

    # The directories on the path are scanned whole, so a new file there is scanned too.
    trees = {scope["tree"] for scope in section["scopes"] if "tree" in scope}
    assert trees == {
        "crates/iroha_data_model/src/transaction",
        "crates/iroha_data_model/src/block",
        "crates/iroha_data_model/src/proof",
        "crates/iroha_data_model/src/parameter",
        "crates/iroha_data_model/src/isi",
        "crates/iroha_core/src/sumeragi",
        "crates/iroha_core/src/pipeline",
        "crates/ivm_abi/src",
        "crates/ivm/src",
        "crates/iroha_sumeragi/src",
        "crates/iroha_p2p/src",
    }
    assert len(scopes) > 500
    for path in (
        "crates/iroha_core/src/sumeragi/evidence.rs",
        "crates/iroha_core/src/sumeragi/certified_chain.rs",
        "crates/iroha_data_model/src/block/proofs.rs",
        "crates/ivm_abi/src/private_input.rs",
        "crates/ivm/src/limits.rs",
        "crates/iroha_p2p/src/peer.rs",
        "crates/iroha_core/src/tx.rs",
    ):
        assert path in scanned, path
    # Test, fixture and simulator sources are not production limits.
    assert not [path for path in scanned if "/tests/" in path or path.endswith("_tests.rs")]
    assert not [path for path in scanned if path.startswith("crates/iroha_sumeragi/src/sim/")]
    nexus = [
        scope
        for scope in section["scopes"]
        if scope.get("module") == ["pub mod nexus {"]
    ]
    assert len(nexus) == 1 and nexus[0]["path"].endswith("parameters/defaults.rs")

    def unlisted(contract: dict) -> None:
        contract["bounds"] = [
            item for item in contract["bounds"] if item["id"] != "attachment.list_max_attachments"
        ]
        contract["relations"] = [
            item for item in contract["relations"] if "list_max_attachments" not in item["expr"]
        ]

    assert_fails(
        edited(unlisted),
        "`PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1` is neither a listed bound nor excluded",
    )

    def unlisted_default(contract: dict) -> None:
        contract["bounds"] = [
            item for item in contract["bounds"] if item["id"] != "transaction.max_signatures"
        ]

    assert_fails(edited(unlisted_default), "`max_signatures` is neither a listed bound nor excluded")

    def symbols_of(group: dict) -> list:
        holders = group["files"] if "files" in group else [group]
        return [symbol for holder in holders for symbol in holder["symbols"]]

    def unexcluded(contract: dict) -> None:
        group = next(
            item for item in contract["completeness"]["excluded"] if "MAX_DA_STRIPE_WIDTH" in symbols_of(item)
        )
        group["symbols"].remove("MAX_DA_STRIPE_WIDTH")

    assert_fails(edited(unexcluded), "`MAX_DA_STRIPE_WIDTH` is neither a listed bound nor excluded")

    def unexcluded_in_a_tree(contract: dict) -> None:
        # A limit in a file that owns no listed bound: found through the directory scope.
        for group in contract["completeness"]["excluded"]:
            for holder in group.get("files", []):
                if "MAX_QUEUE_SCAN" in holder["symbols"]:
                    holder["symbols"].remove("MAX_QUEUE_SCAN")
            if "MAX_QUEUE_SCAN" in group.get("symbols", []):
                group["symbols"].remove("MAX_QUEUE_SCAN")

    assert_fails(
        edited(unexcluded_in_a_tree),
        "completeness crates/iroha_core/src/sumeragi/payload.rs: `MAX_QUEUE_SCAN` is neither",
    )

    def nexus_default_unlisted(contract: dict) -> None:
        # The verifier's case: the settlement carrier default was in no scope.
        identifier = "transaction.config_private_settlement_max_carrier_bytes"
        contract["bounds"] = [item for item in contract["bounds"] if item["id"] != identifier]
        contract["relations"] = [item for item in contract["relations"] if identifier not in item["expr"]]
        catalog = next(item for item in contract["catalogs"] if item["id"] == "nexus_consensus_policy_v1")
        del catalog["fields"]["atomic_private_settlement.max_carrier_bytes"]
        catalog["excluded"][0]["fields"].append("atomic_private_settlement.max_carrier_bytes")

    assert_fails(
        edited(nexus_default_unlisted),
        "`pub mod nexus {`: `MAX_CARRIER_BYTES` is neither a listed bound nor excluded",
    )

    def stale(contract: dict) -> None:
        contract["completeness"]["excluded"][0]["symbols"].append("NO_SUCH_LIMIT")

    assert_fails(edited(stale), "the exclusion of `NO_SUCH_LIMIT`")

    def unreasoned(contract: dict) -> None:
        contract["completeness"]["excluded"][0]["reason"] = ""

    assert_fails(edited(unreasoned), "an exclusion states its reason")

    def both(contract: dict) -> None:
        contract["completeness"]["excluded"].append(
            {
                "path": "crates/iroha_data_model/src/proof.rs",
                "reason": "x",
                "symbols": ["PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1"],
            }
        )

    assert_fails(edited(both), "is both listed and excluded")

    def unscanned(contract: dict) -> None:
        contract["completeness"]["scopes"] = [
            scope
            for scope in contract["completeness"]["scopes"]
            if scope.get("path") != "crates/iroha_data_model/src/proof.rs"
        ]
        contract["completeness"]["excluded"] = [
            group
            for group in contract["completeness"]["excluded"]
            if group.get("path") != "crates/iroha_data_model/src/proof.rs"
        ]

    assert_fails(edited(unscanned), "is outside every completeness scope")

    def tree_removed(contract: dict) -> None:
        contract["completeness"]["scopes"] = [
            scope for scope in contract["completeness"]["scopes"] if scope.get("tree") != "crates/ivm_abi/src"
        ]

    assert_fails(edited(tree_removed), "matches no scanned limit")

    def unexplained_skip(contract: dict) -> None:
        tree = next(
            scope for scope in contract["completeness"]["scopes"] if scope.get("tree") == "crates/iroha_sumeragi/src"
        )
        tree["skip"][0]["why"] = ""

    assert_fails(edited(unexplained_skip), "a skipped path states `why`")

    def cites_nothing(contract: dict) -> None:
        contract["completeness"]["excluded"][0]["reason"] += " See guest_memory.abi_heap_bytes."

    assert_fails(edited(cites_nothing), "the text cites `guest_memory.abi_heap_bytes`, which is not a bound")

    def removed(contract: dict) -> None:
        del contract["completeness"]

    assert_fails(edited(removed), "the contract lists the owner files it scans")


def test_completeness_scan_reads_constants_and_defaults(tmp_path: Path) -> None:
    source = """
    pub const MAX_FRAME_BYTES: usize = 4;
    const RESERVE: u32 = 2;
    pub const NAME: &str = "x";
    pub mod queue {
        pub const CAPACITY: usize = 8;
        pub mod nested { pub const MAX_DEPTH: u8 = 3; }
    }
    pub mod torii {
        pub const MAX_CONNECTIONS: usize = 1;
        pub const PROOF_MAX_BODY_BYTES: u64 = 9;
    }
    pub mod transaction {
        pub const fn max_tx_bytes() -> u64 { 10 }
        pub fn max_signatures() -> u64 { 16 }
        fn private() -> u64 { 1 }
        pub fn with_argument(x: u64) -> u64 { x }
    }
    #[cfg(test)]
    mod tests {
        const MAX_IN_TEST: usize = 1;
    }
    #[cfg(test)]
    /// Documented test module with a further attribute.
    #[allow(clippy::too_many_lines)]
    pub mod more_tests {
        const TEST_GAS_LIMIT: u64 = 1;
    }
    #[cfg(all(test, feature = "sim"))]
    mod sim_tests {
        const MAX_IN_SIM: usize = 1;
    }
    pub const OVERHEAD_AFTER_TESTS: usize = 5;
    """
    symbols = CHECKER["scope_symbols"]
    assert symbols(source, {"kind": "const"}, "t") == [
        "MAX_FRAME_BYTES",
        "RESERVE",
        "CAPACITY",
        "MAX_DEPTH",
        "MAX_CONNECTIONS",
        "PROOF_MAX_BODY_BYTES",
        "OVERHEAD_AFTER_TESTS",
    ]
    assert symbols(source, {"kind": "const", "module": ["pub mod queue {"]}, "t") == [
        "CAPACITY",
        "MAX_DEPTH",
    ]
    assert symbols(
        source, {"kind": "const", "module": ["pub mod queue {", "pub mod nested {"]}, "t"
    ) == ["MAX_DEPTH"]
    narrowed = {"kind": "const", "module": ["pub mod torii {"], "matching": "BODY_BYTES", "why": "x"}
    assert symbols(source, narrowed, "t") == ["PROOF_MAX_BODY_BYTES"]
    assert symbols(
        source, {"kind": "default_fn", "module": ["pub mod transaction {"]}, "t"
    ) == ["max_tx_bytes", "max_signatures"]
    error = CHECKER["ContractError"]
    with pytest.raises(error, match="states `why`"):
        symbols(source, {"kind": "const", "matching": "BODY"}, "t")
    with pytest.raises(error, match="is not in the file"):
        symbols(source, {"kind": "const", "module": ["pub mod absent {"]}, "t")
    with pytest.raises(error, match="unknown scope kind"):
        symbols(source, {"kind": "static"}, "t")

    # A new limit in a scanned file fails until it is listed or excluded.
    (tmp_path / "a.rs").write_text(
        "pub const MAX_A: u64 = 4;\nfn admit(len: u64) -> bool { len > MAX_A }\n", encoding="utf-8"
    )
    contract = {
        "bounds": [
            {"id": "queue.a", "owner": {"path": "a.rs", "symbol": "MAX_A", "kind": "const"}}
        ],
        "completeness": {"scopes": [{"path": "a.rs", "kind": "const"}], "excluded": []},
    }
    found: list = []
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert found == []
    (tmp_path / "a.rs").write_text(
        "pub const MAX_A: u64 = 4;\npub const MAX_B: u64 = 8;\n", encoding="utf-8"
    )
    unlisted: list = []
    CHECKER["check_completeness"](tmp_path, contract, found, False, None, unlisted)
    assert found == ["completeness a.rs: `MAX_B` is neither a listed bound nor excluded with a reason"]
    assert [(scope["path"], symbol) for scope, symbol in unlisted] == [("a.rs", "MAX_B")]
    contract["completeness"]["excluded"] = [{"path": "a.rs", "reason": "derived", "symbols": ["MAX_B"]}]
    found = []
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert found == []


def test_a_directory_scope_scans_every_production_source_below_it(tmp_path: Path) -> None:
    """A new file in a scanned directory is scanned; test and skipped sources are not."""
    tree = tmp_path / "crates" / "demo" / "src"
    for name, text in {
        "lib.rs": "pub const MAX_A: u64 = 4;\n",
        "wire/frame.rs": "pub const MAX_FRAME: u64 = 8;\npub const WIDTH: u64 = 2;\n",
        "wire/frame_tests.rs": "const MAX_IN_TEST_FILE: u64 = 1;\n",
        "wire/tests/cases.rs": "const MAX_IN_TEST_DIR: u64 = 1;\n",
        "wire/release_fixture.rs": "const MAX_FIXTURE: u64 = 1;\n",
        "sim/driver.rs": "const INGRESS_CAP: u64 = 1;\n",
        "tests.rs": "const MAX_ROOT_TESTS: u64 = 1;\n",
    }.items():
        file = tree / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text, encoding="utf-8")
    scope = {
        "tree": "crates/demo/src",
        "kind": "const",
        "skip": [{"path": "crates/demo/src/sim", "why": "a simulator"}],
    }
    contract = {
        "bounds": [
            {"id": "queue.a", "owner": {"path": "crates/demo/src/lib.rs", "symbol": "MAX_A", "kind": "const"}}
        ],
        "completeness": {"scopes": [scope], "excluded": []},
    }
    expanded = CHECKER["expand_scopes"](tmp_path, contract["completeness"], [])
    assert [item["path"] for item in expanded] == [
        "crates/demo/src/lib.rs",
        "crates/demo/src/wire/frame.rs",
    ]
    found: list = []
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert found == [
        "completeness crates/demo/src/wire/frame.rs: `MAX_FRAME` is neither a listed bound "
        "nor excluded with a reason"
    ]
    # One reason may cover several files; every symbol is still named.
    contract["completeness"]["excluded"] = [
        {
            "reason": "wire geometry",
            "files": [{"path": "crates/demo/src/wire/frame.rs", "symbols": ["MAX_FRAME"]}],
        }
    ]
    found = []
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert found == []
    # A file added later is scanned without touching the contract.
    (tree / "wire" / "sync.rs").write_text("pub const SYNC_LIMIT: u64 = 3;\n", encoding="utf-8")
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert found == [
        "completeness crates/demo/src/wire/sync.rs: `SYNC_LIMIT` is neither a listed bound "
        "nor excluded with a reason"
    ]
    (tree / "wire" / "sync.rs").unlink()
    # A stale exclusion fails; `--refresh` removes it and reports it.
    contract["completeness"]["excluded"][0]["files"].append(
        {"path": "crates/demo/src/wire/frame.rs", "symbols": ["GONE_LIMIT"]}
    )
    found = []
    CHECKER["check_completeness"](tmp_path, contract, found)
    assert len(found) == 1 and "the exclusion of `GONE_LIMIT`" in found[0]
    found, notes = [], []
    CHECKER["check_completeness"](tmp_path, contract, found, True, notes)
    assert found == [] and len(notes) == 1 and notes[0].endswith("matches no scanned limit; removed")
    assert contract["completeness"]["excluded"] == [
        {
            "reason": "wire geometry",
            "files": [{"path": "crates/demo/src/wire/frame.rs", "symbols": ["MAX_FRAME"]}],
        }
    ]
    # Malformed tree scopes are reported.
    for broken, fragment in (
        ({"tree": "crates/absent", "kind": "const"}, "the directory does not exist"),
        ({"tree": "crates/demo/src", "kind": "default_fn"}, "a tree scope scans constants"),
        (
            {"tree": "crates/demo/src", "kind": "const", "skip": [{"path": "crates/other", "why": "x"}]},
            "is not below the tree",
        ),
    ):
        found = []
        CHECKER["expand_scopes"](tmp_path, {"scopes": [broken]}, found)
        assert len(found) == 1 and fragment in found[0], found


def test_node_local_validity_limits_are_checked_against_the_typed_catalogs() -> None:
    """The contract's node-local list is a view of the three catalogs the code owns."""
    catalogs = {catalog["id"]: catalog for catalog in CONTRACT["catalogs"]}
    assert set(catalogs) == set(CHECKER["REQUIRED_CATALOGS"])
    local = ("execution_policy_digest_v1", "zk_consensus_policy_hash", "nexus_consensus_policy_v1")
    assert {name for name, catalog in catalogs.items() if catalog["source"] == "node_local"} == set(local)
    listed = {
        identifier for name in local for identifier in catalogs[name]["fields"].values()
    }
    node_local = {
        item["id"]
        for item in CONTRACT["bounds"]
        if item["class"] == "consensus" and item["source"] == "node_local"
    }
    assert listed == node_local, "every node-local validity bound is a catalog field"
    assert len(node_local) == 72
    for identifier in node_local:
        assert bound(CONTRACT, identifier)["defect"]["owner"] == "F.4"
    pipeline = catalogs["execution_policy_digest_v1"]["fields"]
    for field in (
        "pipeline.ivm_max_decoded_instructions",
        "pipeline.ivm_max_decoded_bytes",
        "pipeline.overlay_max_instructions",
        "pipeline.overlay_max_bytes",
        "pipeline.quarantine_tx_max_cycles",
        "pipeline.quarantine_max_txs_per_block",
    ):
        assert field in pipeline
    zk = catalogs["zk_consensus_policy_hash"]["fields"]
    for field in (
        "pipa_r.max_envelope_bytes",
        "max_verify_batch",
        "sccp.max_proofs_per_transaction",
        "sccp.max_proofs_per_block",
        "sccp.max_proof_bytes_per_transaction",
        "max_nullifiers_per_tx",
        "gas.per_proof_byte",
    ):
        assert field in zk
    # The view covers each catalog exactly: listed, excluded and digest fields are the
    # source's field list.
    for catalog in catalogs.values():
        source = (ROOT / catalog["path"]).read_text(encoding="utf-8")
        observed = CHECKER["catalog_fields"](source, catalog, "t")
        excluded = [field for group in catalog["excluded"] for field in group["fields"]]
        recorded = list(catalog["fields"]) + excluded + list(catalog.get("digests", {}))
        assert sorted(observed) == sorted(recorded)
        assert all(group["reason"] for group in catalog["excluded"])
    assert len(catalogs["execution_policy_digest_v1"]["fields"]) == 15
    assert len(catalogs["zk_consensus_policy_hash"]["fields"]) == 47
    assert len(catalogs["nexus_consensus_policy_v1"]["fields"]) == 10
    # The execution-policy digest binds 164 fields: ten committee sizes come from a loop.
    execution_source = (ROOT / catalogs["execution_policy_digest_v1"]["path"]).read_text(encoding="utf-8")
    execution_fields = CHECKER["catalog_fields"](execution_source, catalogs["execution_policy_digest_v1"], "t")
    assert len(execution_fields) == 164
    assert "governance.fma_committee_size" in execution_fields
    # The Nexus digest is not a catalog of identities only: it is the third catalog.
    assert catalogs["execution_policy_digest_v1"]["digests"] == {
        "nexus.policy_digest": "nexus_consensus_policy_v1",
        "zk.policy_digest": "zk_consensus_policy_hash",
    }
    nexus = catalogs["nexus_consensus_policy_v1"]
    assert nexus["struct"] == "NexusConsensusPolicyPreimageV1"
    assert nexus["function"] == "nexus_consensus_policy_preimage_with_runtime_policies"
    for field, identifier in (
        ("atomic_private_settlement.max_participants", "transaction.config_private_settlement_max_participants"),
        ("atomic_private_settlement.max_carrier_bytes", "transaction.config_private_settlement_max_carrier_bytes"),
        ("atomic_private_settlement.max_proof_bytes", "proof.config_private_settlement_max_proof_bytes"),
        ("atomic_private_settlement.max_capsule_bytes", "proof.config_private_settlement_max_capsule_bytes"),
        ("atomic_private_settlement.sidecar_max_records", "storage.config_private_settlement_sidecar_max_records"),
        ("uploaded_models.max_plaintext_bytes_per_model", "storage.config_uploaded_model_max_plaintext_bytes"),
        ("da.ingest_quota_max_bytes_per_account", "storage.config_da_ingest_quota_max_bytes_per_account"),
    ):
        assert nexus["fields"][field] == identifier
    carrier = bound(CONTRACT, "transaction.config_private_settlement_max_carrier_bytes")
    assert carrier["value"] == 4 * MIB and carrier["defect"]["owner"] == "F.4"
    assert {site["stage"] for site in carrier["enforcement"]} >= {"follower"}
    # The carrier limit is above today's includable transaction bound: recorded, with an owner.
    includable = relation(CONTRACT, "private_settlement_carrier_within_includable_transaction")
    assert includable["expect"] == {"today": "violated", "x2_target": "holds"}
    assert includable["open"]["owner"] == "X.2"

    def unlisted(contract: dict) -> None:
        del contract["catalogs"][0]["fields"]["pipeline.ivm_max_decoded_bytes"]

    assert_fails(
        edited(unlisted),
        "field `pipeline.ivm_max_decoded_bytes` of `execution_policy_digest_v1` is neither "
        "listed as a bound nor excluded",
    )

    def unexcluded(contract: dict) -> None:
        contract["catalogs"][1]["excluded"][0]["fields"].pop()

    assert_fails(edited(unexcluded), "of `compute_zk_consensus_policy_hash` is neither listed")

    def loop_field_unexcluded(contract: dict) -> None:
        for group in contract["catalogs"][0]["excluded"]:
            if "governance.mpc_committee_size" in group["fields"]:
                group["fields"].remove("governance.mpc_committee_size")

    assert_fails(
        edited(loop_field_unexcluded),
        "field `governance.mpc_committee_size` of `execution_policy_digest_v1` is neither",
    )

    def nexus_field_unlisted(contract: dict) -> None:
        catalog = next(item for item in contract["catalogs"] if item["id"] == "nexus_consensus_policy_v1")
        for group in catalog["excluded"]:
            if "da.sample_size_max" in group["fields"]:
                group["fields"].remove("da.sample_size_max")

    assert_fails(
        edited(nexus_field_unlisted),
        "field `da.sample_size_max` of `nexus_consensus_policy_preimage_with_runtime_policies` is neither",
    )

    def invented(contract: dict) -> None:
        contract["catalogs"][1]["excluded"][0]["fields"].append("pipa_r.no_such_limit")

    assert_fails(edited(invented), "`pipa_r.no_such_limit` is not a field of")

    def twice(contract: dict) -> None:
        contract["catalogs"][0]["excluded"][0]["fields"].append("pipeline.overlay_max_bytes")

    assert_fails(edited(twice), "`pipeline.overlay_max_bytes` is recorded twice")

    def unlinked(contract: dict) -> None:
        del bound(contract, "decoded_scratch.config_ivm_max_decoded_bytes")["catalog"]

    assert_fails(edited(unlinked), "records `catalog` as execution_policy_digest_v1")

    def committed(contract: dict) -> None:
        item = bound(contract, "proof.config_pipa_r_max_envelope_bytes")
        item["source"] = "constant"
        del item["defect"]

    assert_fails(edited(committed), "a field of zk_consensus_policy_hash has source `node_local`")

    def local_class(contract: dict) -> None:
        item = bound(contract, "work_budget.config_quarantine_tx_max_cycles")
        item["class"] = "local_defer"

    assert_fails(edited(local_class), "the bound is classed consensus with its defect")

    def nexus_without_defect(contract: dict) -> None:
        del bound(contract, "storage.config_da_ingest_quota_max_count_per_account")["defect"]

    assert_fails(
        edited(nexus_without_defect),
        "storage.config_da_ingest_quota_max_count_per_account: a validity-affecting bound read "
        "from node-local configuration must be recorded as a defect",
    )

    def unreasoned(contract: dict) -> None:
        contract["catalogs"][0]["excluded"][0]["reason"] = ""

    assert_fails(edited(unreasoned), "an exclusion states its reason")

    def dropped(contract: dict) -> None:
        contract["catalogs"] = [
            item for item in contract["catalogs"] if item["id"] != "zk_consensus_policy_hash"
        ]

    assert_fails(edited(dropped), "catalogs: `zk_consensus_policy_hash` is checked against the source")
    assert_fails(
        edited(dropped),
        "`zk.policy_digest` is recorded as the digest of `zk_consensus_policy_hash`, which is "
        "not another catalog",
    )

    def nexus_dropped(contract: dict) -> None:
        # The first repair's state: the Nexus digest dismissed with a reason instead of checked.
        contract["catalogs"] = [
            item for item in contract["catalogs"] if item["id"] != "nexus_consensus_policy_v1"
        ]
        execution = contract["catalogs"][0]
        del execution["digests"]["nexus.policy_digest"]
        execution["excluded"].append(
            {"reason": "holds identities, not resource bounds", "fields": ["nexus.policy_digest"]}
        )
        for item in contract["bounds"]:
            if item.get("catalog", {}).get("id") == "nexus_consensus_policy_v1":
                del item["catalog"]

    assert_fails(edited(nexus_dropped), "catalogs: `nexus_consensus_policy_v1` is checked against the source")

    def stray(contract: dict) -> None:
        bound(contract, "queue.capacity")["catalog"] = {
            "id": "zk_consensus_policy_hash",
            "field": "max_verify_batch",
        }

    assert_fails(edited(stray), "queue.capacity: its `catalog` entry is not a field of that catalog")


def test_committed_policy_structs_are_catalogs_of_their_fields() -> None:
    """A field added to a committed policy struct is a bound the contract must list."""
    catalogs = {catalog["id"]: catalog for catalog in CONTRACT["catalogs"]}
    output = catalogs["execution_output_policy_v1"]
    fastpq = catalogs["fastpq_source_policy_v1"]
    assert output["source"] == fastpq["source"] == "committed"
    assert len(output["fields"]) == 7 and len(fastpq["fields"]) == 20
    assert output["excluded"] == fastpq["excluded"] == []
    assert fastpq["fields"]["mandatory.per_obligation.max_deltas"] == "work_budget.fastpq_mandatory_max_deltas"
    assert fastpq["fields"]["max_native_maintenance_invocations"] == (
        "work_budget.fastpq_native_maintenance_invocations"
    )
    for catalog in (output, fastpq):
        for identifier in catalog["fields"].values():
            assert bound(CONTRACT, identifier)["source"] == "committed"

    def field_unlisted(contract: dict) -> None:
        catalog = next(item for item in contract["catalogs"] if item["id"] == "execution_output_policy_v1")
        del catalog["fields"]["max_time_triggers"]
        del bound(contract, "block.execution_output_max_time_triggers")["catalog"]

    assert_fails(
        edited(field_unlisted),
        "field `max_time_triggers` of `ExecutionOutputPolicyV1` is neither listed",
    )

    def node_local(contract: dict) -> None:
        bound(contract, "work_budget.fastpq_block_max_deltas")["source"] = "constant"

    assert_fails(edited(node_local), "a field of fastpq_source_policy_v1 has source `committed`")


def test_catalog_field_reader(tmp_path: Path) -> None:
    source = """
    pub fn execution_policy_digest_v1(pipeline: &Pipeline) -> [u8; 32] {
        let mut policy = Fields::default();
        policy.push("pipeline.dynamic_prepass", &pipeline.dynamic_prepass);
        policy.push(
            "pipeline.overlay_max_bytes",
            &pipeline.overlay_max_bytes,
        );
        // policy.push("pipeline.commented_out", &0);
        for (name, size) in [
            (
                "governance.rules_committee_size",
                governance.rules_committee_size,
            ),
            ("governance.review_panel_size", governance.review_panel_size),
        ] {
            policy.push(name, &usize_field(size));
        }
        policy.finish()
    }

    pub fn compute_zk_consensus_policy_hash(zk: &Zk) -> [u8; 32] {
        let mut h = Sha256::new();
        zk_policy_put_bytes(&mut h, b"iroha:zk:consensus-policy:v1");
        zk_policy_put_u32(&mut h, "max_verify_batch", zk.max_verify_batch);
        zk_policy_put_usize(
            &mut h,
            "pipa_r.max_proof_bytes",
            zk.pipa_r.max_proof_bytes,
        );
        h.finalize().into()
    }
    """
    fields = CHECKER["catalog_fields"]
    push = {"function": "execution_policy_digest_v1", "field_form": "policy_push", "path": "a.rs"}
    assert fields(source, push, "t") == [
        "pipeline.dynamic_prepass",
        "pipeline.overlay_max_bytes",
        "governance.rules_committee_size",
        "governance.review_panel_size",
    ]
    put = {"function": "compute_zk_consensus_policy_hash", "field_form": "zk_policy_put", "path": "a.rs"}
    assert fields(source, put, "t") == ["max_verify_batch", "pipa_r.max_proof_bytes"]
    error = CHECKER["ContractError"]
    with pytest.raises(error, match="is not in a.rs"):
        fields(source, {**push, "function": "absent"}, "t")
    with pytest.raises(error, match="unknown field form"):
        fields(source, {**push, "field_form": "macro"}, "t")
    # A field added to the loop's table is read like any other.
    grown = source.replace(
        '("governance.review_panel_size", governance.review_panel_size),',
        '("governance.review_panel_size", governance.review_panel_size),\n'
        '            ("governance.new_committee_size", governance.new_committee_size),',
    )
    assert fields(grown, push, "t")[-1] == "governance.new_committee_size"
    # A field whose name the reader cannot read fails instead of being skipped.
    for unreadable in (
        source.replace('policy.push("pipeline.dynamic_prepass",', "policy.push(PREPASS_FIELD,"),
        source.replace("policy.push(name, &usize_field(size));", "policy.push(other, &usize_field(size));"),
        source.replace('"governance.review_panel_size", governance.review_panel_size', "REVIEW, 3"),
        source.replace("] {\n            policy.push(name", "] { }\n        {\n            policy.push(name"),
    ):
        with pytest.raises(error, match="binds a field whose name the checker cannot read"):
            fields(unreadable, push, "t")
    with pytest.raises(error, match="`zk_policy_put_u32\\(` call binds a field whose name"):
        fields(source.replace('"max_verify_batch"', "MAX_BATCH_FIELD"), put, "t")

    # A typed preimage struct: nested structs contribute their fields below the field's name.
    structs = """
    #[derive(Encode)]
    struct PreimageV1 {
        version: u8,
        /// Routing identities.
        routing: RoutingV1,
        rules: Vec<RuleV1>,
        settlement: SettlementV1,
        sponsors: Vec<(u64, ProgramId)>,
        digest: Option<[u8; 32]>,
        delay: DurationV1,
    }
    #[derive(Encode)]
    struct RoutingV1 { default_lane: u32 }
    struct RuleV1 { lane: u32, account: Option<String> }
    struct SettlementV1 {
        pub max_participants: u16,
        pub(crate) max_carrier_bytes: u64,
    }
    struct DurationV1 { seconds: u64, nanoseconds: u32 }
    pub fn preimage(config: &Config) -> Vec<u8> {
        let value = PreimageV1 { version: 1, routing: RoutingV1 { default_lane: 0 } };
        value.encode()
    }
    pub fn unrelated() -> u8 { 0 }
    """
    tree = {"struct": "PreimageV1", "function": "preimage", "field_form": "struct_tree", "path": "a.rs"}
    assert fields(structs, tree, "t") == [
        "version",
        "routing.default_lane",
        "rules.lane",
        "rules.account",
        "settlement.max_participants",
        "settlement.max_carrier_bytes",
        "sponsors",
        "digest",
        "delay.seconds",
        "delay.nanoseconds",
    ]
    assert CHECKER["struct_field_tree"](structs, "SettlementV1", "t") == [
        "max_participants",
        "max_carrier_bytes",
    ]
    with pytest.raises(error, match="struct `AbsentV1` is not defined"):
        fields(structs, {"struct": "AbsentV1", "field_form": "struct_tree", "path": "a.rs"}, "t")
    with pytest.raises(error, match="`unrelated` no longer builds `PreimageV1`"):
        fields(structs, {**tree, "function": "unrelated"}, "t")
    with pytest.raises(error, match="missing struct"):
        fields(structs, {"field_form": "struct_tree", "path": "a.rs"}, "t")

    # A field added to the function fails the view until it is listed or excluded.
    (tmp_path / "a.rs").write_text(
        "pub const MAX_K: u32 = 16;\nfn admit(k: u32) -> bool { k > MAX_K }\n" + source + structs,
        encoding="utf-8",
    )
    contract = {
        "bounds": [
            {
                "id": "proof.k",
                "source": "node_local",
                "class": "consensus",
                "catalog": {"id": "zk_consensus_policy_hash", "field": "max_verify_batch"},
            },
            {
                "id": "transaction.carrier",
                "source": "node_local",
                "class": "consensus",
                "catalog": {"id": "nexus_consensus_policy_v1", "field": "settlement.max_carrier_bytes"},
            },
        ],
        "catalogs": [
            {
                **put,
                "id": "zk_consensus_policy_hash",
                "source": "node_local",
                "what": "x",
                "fields": {"max_verify_batch": "proof.k"},
                "excluded": [],
            },
            {
                **tree,
                "id": "nexus_consensus_policy_v1",
                "source": "node_local",
                "what": "x",
                "fields": {"settlement.max_carrier_bytes": "transaction.carrier"},
                "excluded": [
                    {
                        "reason": "identities and windows",
                        "fields": [
                            "version", "routing.default_lane", "rules.lane", "rules.account",
                            "sponsors", "digest", "delay.seconds", "delay.nanoseconds",
                        ],
                    }
                ],
            },
        ],
    }
    found: list = []
    unlisted: list = []
    CHECKER["check_catalogs"](tmp_path, contract, TASKS, found, unlisted)
    assert found == [
        "catalog zk_consensus_policy_hash: field `pipa_r.max_proof_bytes` of "
        "`compute_zk_consensus_policy_hash` is neither listed as a bound nor excluded with a reason",
        "catalog nexus_consensus_policy_v1: field `settlement.max_participants` of `preimage` "
        "is neither listed as a bound nor excluded with a reason",
        "catalogs: `execution_policy_digest_v1` is checked against the source",
        "catalogs: `execution_output_policy_v1` is checked against the source",
        "catalogs: `fastpq_source_policy_v1` is checked against the source",
    ]
    assert [(catalog["id"], field) for catalog, field in unlisted] == [
        ("zk_consensus_policy_hash", "pipa_r.max_proof_bytes"),
        ("nexus_consensus_policy_v1", "settlement.max_participants"),
    ]


def test_derived_bounds_are_recomputed_and_join_the_relations() -> None:
    derived = {item["id"]: item for item in CONTRACT["derived"]}
    effective = derived["block.effective_max_own_transactions"]
    assert effective["value"] == 11
    assert bound(CONTRACT, "block.max_transactions")["value"] == 512
    assert "not the largest number" in bound(CONTRACT, "block.max_transactions")["what"]
    assert any(
        site["contains"] == ".min(fastpq_inputs)" for site in effective["enforcement"]
    )
    assert relation(CONTRACT, "fastpq_network_inputs_within_transaction_ceiling")["expect"] == {
        "today": "holds",
        "x2_target": "holds",
    }

    def stale(contract: dict) -> None:
        contract["derived"][0]["value"] = 512

    assert_fails(edited(stale), "the formula gives 11; the contract records 512")

    def unsited(contract: dict) -> None:
        contract["derived"][0]["enforcement"] = []

    assert_fails(edited(unsited), "names its enforcement sites and tests")

    def site(contract: dict) -> None:
        contract["derived"][0]["enforcement"][0]["contains"] = ".min(no_such_cap)"

    assert_fails(edited(site), "no longer contains `.min(no_such_cap)`")

    def clash(contract: dict) -> None:
        contract["derived"][0]["id"] = "block.max_transactions"

    assert_fails(edited(clash), "the id is already a bound")


def test_attachment_list_bounds_and_their_open_relation() -> None:
    frame = bound(CONTRACT, "attachment.list_max_canonical_frame_bytes")
    assert (frame["value"], frame["class"]) == (8 * MIB, "consensus")
    assert bound(CONTRACT, "attachment.list_max_attachments")["value"] == 16
    assert frame["value"] < bound(CONTRACT, "x509.max_proof_bytes")["value"]
    carried = relation(CONTRACT, "attachment_list_frame_holds_a_ceiling_proof")
    assert carried["expect"] == {"today": "violated", "x2_target": "violated"}
    assert carried["open"]["owner"] == "I.1"
    declared = next(
        item for item in CONTRACT["declared"] if item["id"] == "attachment.signed_inline_occurrences"
    )
    assert "8 MiB" in declared["today"] and "attachment.list_max_attachments" in declared["today"]
    assert "attachment_list_frame_holds_a_ceiling_proof" in declared["relations"]

    def claimed(contract: dict) -> None:
        relation(contract, "attachment_list_frame_holds_a_ceiling_proof")["expect"]["today"] = "holds"

    assert_fails(edited(claimed), "violated under `today`; the contract records holds")


def test_ram_lfe_budgets_cover_the_full_workload() -> None:
    section = CONTRACT["ram_lfe"]
    components = {budget["component"] for budget in section["budgets"]}
    assert components == set(CHECKER["WORKLOAD_COMPONENTS"])
    used = set()
    for budget in section["budgets"]:
        used.update(budget["inputs"])
    assert {item["id"] for item in section["open_inputs"]} <= used
    shapes = {item["id"] for item in CONTRACT["bounds"] if item["id"].startswith("ram_lfe.shape_")}
    assert len(shapes) == 10 and shapes <= used
    by_id = {budget["id"]: budget for budget in section["budgets"]}
    # The parts the first derivation missed have a formula, and no invented number.
    for identifier, component in (
        ("ram_lfe.key_registration_transaction_bytes", "key_registration"),
        ("ram_lfe.wellformedness_verification_gas", "verification"),
        ("ram_lfe.opening_verification_gas", "opening"),
        ("ram_lfe.threshold_opening_transaction_bytes", "threshold_opening"),
        ("ram_lfe.threshold_opening_verification_gas", "threshold_opening"),
        ("ram_lfe.validator_request_scratch_bytes", "validator_scratch"),
        ("ram_lfe.validator_receipt_scratch_bytes", "validator_scratch"),
        ("ram_lfe.affine_evaluation_work_units", "class_affine"),
        ("ram_lfe.bounded_evaluation_work_units", "class_bounded"),
        ("ram_lfe.refresh_evaluation_work_units", "class_refresh"),
    ):
        assert by_id[identifier]["component"] == component
        assert by_id[identifier]["value"] is None
    assert "ram_lfe.evaluation_key_bytes" in by_id["ram_lfe.key_registration_transaction_bytes"]["inputs"]
    assert by_id["ram_lfe.bounded_multiplicative_depth"]["value"] == 16
    assert by_id["ram_lfe.refresh_multiplicative_depth"]["value"] == 16 * 65
    assert by_id["ram_lfe.cleartext_trace_scalars"]["value"] == 257 * 36
    assert relation(CONTRACT, "ram_lfe_leakage_bits_are_the_output_space")["expect"]["today"] == "holds"
    assert 2**512 < 257**64 < 2**513

    def unused_input(contract: dict) -> None:
        contract["ram_lfe"]["budgets"] = [
            budget
            for budget in contract["ram_lfe"]["budgets"]
            if "ram_lfe.evaluation_key_bytes" not in budget["inputs"]
        ]

    assert_fails(
        edited(unused_input),
        "ram_lfe input ram_lfe.evaluation_key_bytes: an open input is used by a budget formula",
    )

    def unused_shape(contract: dict) -> None:
        contract["ram_lfe"]["budgets"] = [
            budget
            for budget in contract["ram_lfe"]["budgets"]
            if "ram_lfe.shape_bounded_max_rank" not in budget["inputs"]
        ]

    assert_fails(
        edited(unused_shape),
        "ram_lfe.shape_bounded_max_rank: a RAM-LFE shape bound is used by a budget formula",
    )

    def no_threshold(contract: dict) -> None:
        contract["ram_lfe"]["budgets"] = [
            budget
            for budget in contract["ram_lfe"]["budgets"]
            if budget["component"] != "threshold_opening"
        ]

    assert_fails(edited(no_threshold), "the workload component `threshold_opening` has a budget")

    def unlabelled(contract: dict) -> None:
        del contract["ram_lfe"]["budgets"][0]["component"]

    assert_fails(edited(unlabelled), "names its workload `component`")


def test_behaviors_name_tests_at_every_required_stage() -> None:
    behaviors = {entry["id"]: entry for entry in CONTRACT["behaviors"]}
    for identifier, stages in CHECKER["BEHAVIOR_STAGES"].items():
        reached = {entry["stage"] for entry in behaviors[identifier]["tests"]}
        assert set(stages) <= reached, identifier
    malformed = behaviors["malformed_carried_evidence_rejects_deterministically"]
    names = {entry["name"] for entry in malformed["tests"]}
    # Evidence carried by a transaction, at admission and in block validation.
    assert "malformed_proof_attachments_rejected_at_transaction_admission" in names
    assert "attachment_list_bounds_accept_the_limit_and_reject_one_over" in names
    assert "privacy_proof_ceiling_accepts_the_limit_and_rejects_one_over" in names
    deferral = {entry["name"] for entry in behaviors["missing_local_resource_defers"]["tests"]}
    assert "deferred_includable_assessment_defers_queue_admission_without_custody" in deferral

    def consensus_only(contract: dict) -> None:
        entry = next(
            item
            for item in contract["behaviors"]
            if item["id"] == "malformed_carried_evidence_rejects_deterministically"
        )
        entry["tests"] = [item for item in entry["tests"] if item["stage"] == "follower"]

    assert_fails(edited(consensus_only), "names a test at the `queue` stage")

    def no_queue_deferral(contract: dict) -> None:
        entry = next(
            item for item in contract["behaviors"] if item["id"] == "missing_local_resource_defers"
        )
        entry["tests"] = [item for item in entry["tests"] if item["stage"] != "queue"]

    assert_fails(edited(no_queue_deferral), "`missing_local_resource_defers` names a test at the `queue` stage")


def test_refresh_rewrites_values_and_expressions_and_keeps_classifications(tmp_path: Path) -> None:
    (tmp_path / "a.rs").write_text(
        "pub const LIMIT: u64 = 8 * 1024;\npub const DOUBLE: u64 = LIMIT * 2;\n"
        "fn admit(len: u64) -> bool { len > LIMIT || len > DOUBLE }\n"
        "#[test]\nfn limit_is_inclusive() {}\n",
        encoding="utf-8",
    )
    owner = lambda symbol: {"path": "a.rs", "symbol": symbol, "kind": "const"}  # noqa: E731
    contract = {
        "bounds": [
            {
                "id": "queue.limit",
                "group": "queue",
                "what": "A limit.",
                "unit": "bytes",
                "value": 4_096,
                "source": "constant",
                "class": "consensus",
                "owner": {**owner("LIMIT"), "expr": "4 * 1024"},
                "enforcement": [{"stage": "queue", "path": "a.rs", "contains": "len > LIMIT"}],
                "tests": [{"path": "a.rs", "name": "limit_is_inclusive"}],
            },
            {
                "id": "queue.double",
                "group": "queue",
                "what": "Twice the limit.",
                "unit": "bytes",
                "source": "constant",
                "class": "consensus",
                "owner": owner("DOUBLE"),
                "refs": {"LIMIT": "queue.limit"},
            },
        ]
    }
    before = copy.deepcopy(contract)
    found: list = []
    values = CHECKER["verify_bounds"](tmp_path, contract, found, True)
    assert found == [] and values == {"queue.limit": 8_192, "queue.double": 16_384}
    assert contract["bounds"][0]["owner"]["expr"] == "8 * 1024"
    assert contract["bounds"][0]["value"] == 8_192
    assert contract["bounds"][1]["owner"]["expr"] == "LIMIT * 2"
    assert contract["bounds"][1]["value"] == 16_384
    # Nothing but the expression and the value changed.
    for refreshed, original in zip(contract["bounds"], before["bounds"]):
        for key in original:
            if key not in ("value", "owner"):
                assert refreshed[key] == original[key]
        assert refreshed["owner"]["symbol"] == original["owner"]["symbol"]
    # The refreshed contract verifies; the stale one did not.
    assert CHECKER["verify_bounds"](tmp_path, contract, found) == values and found == []
    stale: list = []
    CHECKER["verify_bounds"](tmp_path, before, stale)
    assert stale and "is now `8 * 1024`" in stale[0]
    # Derived bounds and closed budgets are recomputed by the same refresh.
    stale_tracked = copy.deepcopy(CONTRACT)
    stale_tracked["derived"][0]["value"] = 512
    stale_tracked["ram_lfe"]["budgets"][0]["value"] = 1
    stale_problems = CHECKER["check"](ROOT, copy.deepcopy(stale_tracked), TASKS)
    assert any("the formula gives 11; the contract records 512" in item for item in stale_problems)
    assert any("the formula gives 12288; the contract records 1" in item for item in stale_problems)
    assert CHECKER["check"](ROOT, stale_tracked, TASKS, True) == []
    assert CHECKER["render"](stale_tracked) == CHECKER["render"](CONTRACT)
    # Refreshing the tracked contract changes nothing and leaves it valid.
    tracked = copy.deepcopy(CONTRACT)
    assert CHECKER["check"](ROOT, tracked, TASKS, True) == []
    assert CHECKER["render"](tracked) == CHECKER["render"](CONTRACT)
    assert CHECKER["render"](CONTRACT) == CHECKER["CONTRACT"].read_text(encoding="utf-8")


def test_refresh_reports_what_it_changes_and_never_adds_a_classification() -> None:
    """`--refresh` rewrites counts and drops stale exclusions, and says so."""
    stale = copy.deepcopy(CONTRACT)
    frame = next(
        site
        for site in bound(stale, "attachment.list_max_canonical_frame_bytes")["enforcement"]
        if site["contains"].startswith("if canonical_frame_len >")
    )
    del frame["count"]
    stale["completeness"]["excluded"][0]["symbols"].append("NO_SUCH_LIMIT")
    assert len(CHECKER["check"](ROOT, copy.deepcopy(stale), TASKS)) == 2
    notes: list = []
    assert CHECKER["check"](ROOT, stale, TASKS, True, notes) == []
    assert len(notes) == 2
    assert any("now occurs 2 time(s), was 1" in note for note in notes)
    assert any("`NO_SUCH_LIMIT`" in note and note.endswith("removed") for note in notes)
    assert CHECKER["render"](stale) == CHECKER["render"](CONTRACT)
    # Refresh classifies nothing: an unlisted limit and a gone site still fail after it.
    unlisted = copy.deepcopy(CONTRACT)
    unlisted["bounds"] = [
        item for item in unlisted["bounds"] if item["id"] != "work_budget.max_contract_call_depth"
    ]
    bound(unlisted, "sync.max_proposal_bytes")["enforcement"][0]["contains"] = "|| no_such_comparison"
    remaining = CHECKER["check"](ROOT, unlisted, TASKS, True, [])
    assert any("`MAX_CONTRACT_CALL_DEPTH` is neither a listed bound nor excluded" in item for item in remaining)
    assert any("no longer contains `|| no_such_comparison`" in item for item in remaining)


def test_propose_suggests_entries_and_writes_nothing(tmp_path: Path, capsys) -> None:
    """`--propose` prints a skeleton with candidate sites and tests for an unlisted limit."""
    crate = tmp_path / "crates" / "demo"
    (crate / "src").mkdir(parents=True)
    (crate / "Cargo.toml").write_text("[package]\nname = \"demo\"\n", encoding="utf-8")
    (crate / "src" / "lib.rs").write_text(
        "pub mod limits {\n    /// Largest frame.\n    pub const MAX_FRAME_BYTES: usize = 4 * 1024;\n}\n"
        "pub const DOUBLE_LIMIT: usize = limits::MAX_FRAME_BYTES * 2;\n"
        "pub fn admit(len: usize) -> bool {\n    if len > limits::MAX_FRAME_BYTES {\n        return false;\n    }\n"
        "    true\n}\n"
        "pub fn again(len: usize) -> bool {\n    if len > limits::MAX_FRAME_BYTES {\n        return false;\n    }\n"
        "    true\n}\n"
        "#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n    fn frame_limit_is_inclusive() {\n"
        "        assert!(admit(limits::MAX_FRAME_BYTES));\n    }\n    #[test]\n    #[ignore]\n"
        "    fn slow_frame_case() {\n        assert!(!admit(limits::MAX_FRAME_BYTES + 1));\n    }\n"
        "    #[test]\n    fn unrelated() {}\n}\n",
        encoding="utf-8",
    )
    scope = {"path": "crates/demo/src/lib.rs", "kind": "const", "module": ["pub mod limits {"]}
    entry = CHECKER["propose_entry"](tmp_path, scope, "MAX_FRAME_BYTES", {}, [])
    assert entry["owner"] == {
        "path": "crates/demo/src/lib.rs",
        "symbol": "MAX_FRAME_BYTES",
        "kind": "const",
        "anchor": ["pub mod limits {"],
        "expr": "4 * 1024",
    }
    assert entry["value"] == 4096 and entry["source"] == "constant"
    # The comparison is proposed with its occurrence count; the definition that only
    # multiplies the constant, the ignored test and the unrelated test are not.
    assert entry["candidate_sites"] == [
        {
            "stage": "<stage>",
            "path": "crates/demo/src/lib.rs",
            "contains": "if len > limits::MAX_FRAME_BYTES {",
            "count": 2,
        }
    ]
    assert entry["candidate_tests"] == [
        {"path": "crates/demo/src/lib.rs", "name": "frame_limit_is_inclusive"}
    ]
    # An expression that names another constant is not guessed.
    derived = CHECKER["propose_entry"](
        tmp_path, {"path": "crates/demo/src/lib.rs", "kind": "const"}, "DOUBLE_LIMIT", {}, []
    )
    assert derived["value"] is None and "add `refs`" in derived["value_note"]
    # A further word finds the sites of a configuration field read through another name.
    worded = CHECKER["propose_entry"](
        tmp_path, {"path": "crates/demo/src/lib.rs", "kind": "const"}, "DOUBLE_LIMIT", {}, [], ("admit",)
    )
    assert {test["name"] for test in worded["candidate_tests"]} == {"frame_limit_is_inclusive"}

    # On the tracked contract there is nothing to propose, and nothing is written.
    before = CHECKER["CONTRACT"].read_text(encoding="utf-8")
    assert CHECKER["propose"](ROOT, copy.deepcopy(CONTRACT), TASKS, []) == []
    assert CHECKER["main"](["--propose"]) == 0
    assert "nothing to propose" in capsys.readouterr().out
    assert CHECKER["CONTRACT"].read_text(encoding="utf-8") == before
    # An unlisted limit, an unrecorded catalog field, a stale exclusion and a gone site are
    # each reported with what to do.
    drifted = copy.deepcopy(CONTRACT)
    drifted["bounds"] = [
        item for item in drifted["bounds"] if item["id"] != "work_budget.max_contract_call_depth"
    ]
    nexus = next(item for item in drifted["catalogs"] if item["id"] == "nexus_consensus_policy_v1")
    nexus["excluded"][0]["fields"].remove("version")
    drifted["completeness"]["excluded"][0]["symbols"].append("NO_SUCH_LIMIT")
    bound(drifted, "sync.max_proposal_bytes")["enforcement"][0]["contains"] = (
        "|| header.payload_len as usize >= MAX_PROPOSAL_BYTES"
    )
    lines = CHECKER["propose"](ROOT, drifted, TASKS, [])
    text = "\n".join(lines)
    assert "# unlisted limit `MAX_CONTRACT_CALL_DEPTH` in crates/ivm/src/limits.rs" in text
    assert '"contains": "if self.len() >= MAX_CONTRACT_CALL_DEPTH {"' in text
    assert "protected_depth_limit_retains_full_lifo_stack_on_refusal_and_reuses_popped_slot" in text
    assert "# catalog nexus_consensus_policy_v1: field `version` is not recorded" in text
    assert "# --refresh would change: completeness: the exclusion of `NO_SUCH_LIMIT`" in text
    assert "# sync.max_proposal_bytes: site `|| header.payload_len as usize >= MAX_PROPOSAL_BYTES` is gone" in text
    assert "#   closest: || header.payload_len as usize > MAX_PROPOSAL_BYTES" in text
    # The proposal did not edit the contract it was given.
    assert "NO_SUCH_LIMIT" in drifted["completeness"]["excluded"][0]["symbols"]
    one = CHECKER["propose"](ROOT, CONTRACT, TASKS, [], "crates/ivm/src/limits.rs:MAX_CONTRACT_CALL_DEPTH")
    assert len(one) == 1 and json.loads(one[0])["value"] == 1024


def test_text_that_cites_a_bound_cites_one_that_exists() -> None:
    cited = CHECKER["_CITED_ID"]
    assert cited.findall("bounded by guest_memory.heap_max_bytes and rs16.* (see queue.rs)") == [
        "guest_memory.heap_max_bytes",
        "queue.rs",
    ]
    assert cited.findall("crates/iroha_core/src/queue.rs and transaction.max_tx_bytes()") == []

    def renamed(contract: dict) -> None:
        # A bound renamed without its reasons: every text that cited it is reported once.
        bound(contract, "block.payload_transaction_reserve_bytes")["id"] = "block.payload_reserve_bytes"
        for item in contract["relations"]:
            item["expr"] = item["expr"].replace(
                "block.payload_transaction_reserve_bytes", "block.payload_reserve_bytes"
            )

    found = problems(edited(renamed))
    assert any(
        "the text cites `block.payload_transaction_reserve_bytes`, which is not a bound" in item
        for item in found
    ), found

    def miscited(contract: dict) -> None:
        bound(contract, "work_budget.fuel")["test_note"] += " See work_budget.gas_limit_total."

    assert_fails(edited(miscited), "bounds/test_note: the text cites `work_budget.gas_limit_total`")


def test_lane_route_statements_state_that_there_is_no_rescue_above_the_global_budget() -> None:
    """The includable bound holds for the route at admission; it is not a promise of inclusion."""
    agreement = CONTRACT["stage_decisions"]["agreement"]
    assert "nothing admitted is unincludable" not in agreement
    assert "only its lane's proposer can carry it" in agreement
    assert "stays queued until it expires" in agreement
    system = (ROOT / "crates/iroha_data_model/src/parameter/system.rs").read_text(encoding="utf-8")
    payload = (ROOT / "crates/iroha_core/src/sumeragi/payload.rs").read_text(encoding="utf-8")
    assert "rescues it when the lane stalls" not in system
    assert "never left waiting" not in system
    assert "The bound is not a promise of inclusion." in system
    assert "with the\n/// global chain as its rescue" not in payload
    lanes = (ROOT / "specs/sumeragi_lanes.md").read_text(encoding="utf-8")
    assert "such a\ntransaction is never rescued" in lanes
    proposer = next(entry for entry in CONTRACT["stage_tests"] if entry["stage"] == "proposer")
    assert "global_rescue_carries_a_stalled_lane_transaction_only_within_the_global_budget" in {
        test["name"] for test in proposer["tests"]
    }
    assert "if next > max_bytes {" in {site["contains"] for site in proposer["enforcement"]}
