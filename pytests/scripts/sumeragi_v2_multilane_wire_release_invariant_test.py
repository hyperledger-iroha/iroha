"""Wire-release invariant controls for the multilane source-binding contract."""

from __future__ import annotations

import copy
import json
import shutil
from pathlib import Path

import pytest

from sumeragi_v2_multilane_models_test import (
    BINDINGS,
    ROOT_DIR,
    load_checker,
    replace_once,
)


def canonical_binding_ledger() -> dict:
    return copy.deepcopy(json.loads(BINDINGS.read_text(encoding="utf-8")))


def wire_release_invariant(ledger: dict) -> dict:
    return next(
        mutation
        for mutation in ledger["closure_mutations"]
        if mutation["id"] == "ML-MUT-WIRE-01"
    )


def validate_closure_mutations(root: Path, module, ledger: dict) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_closure_mutation_ledger(
        root,
        root / "formal" / "sumeragi_v2",
        ledger["closure_mutations"],
        ledger["models"],
        ledger[module.KURA_RETENTION_CONTRACT_KEY],
        errors,
    )
    return tuple(errors)


def copy_wire_release_invariant_fixture(
    tmp_path: Path, ledger: dict
) -> None:
    closure = Path("specs/sumeragi_v2_multilane_closure_ledger.md")
    relatives = {
        closure,
        *(
            Path(check["path"])
            for check in wire_release_invariant(ledger)["source_checks"]
        ),
    }
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)


def test_wire_release_invariant_binds_current_semantic_sources() -> None:
    module = load_checker()
    ledger = canonical_binding_ledger()
    wire = wire_release_invariant(ledger)
    assert tuple(check["path"] for check in wire["source_checks"]) == (
        "scripts/check_no_legacy_codec.sh",
        "fixtures/sumeragi_v2/wire_v2.tsv",
        "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
        "crates/iroha_data_model/tests/sumeragi_v2_cross_sdk_fixtures.rs",
        "scripts/run_sumeragi_v2_release_gates.sh",
        "ci/check_sumeragi_v2_multilane_release_inventory.sh",
    )
    assert validate_closure_mutations(ROOT_DIR, module, ledger) == ()


@pytest.mark.parametrize("weakening", ("path", "token"))
def test_wire_release_invariant_rejects_ledger_weakening(
    weakening: str,
) -> None:
    module = load_checker()
    ledger = canonical_binding_ledger()
    source_checks = wire_release_invariant(ledger)["source_checks"]
    if weakening == "path":
        source_checks.pop()
        expected = "source checks differ from the exact reviewed paths"
    else:
        source_checks[1]["required_tokens"].pop()
        expected = "semantic source checks differ from the exact reviewed contract"
    assert any(
        expected in error
        for error in validate_closure_mutations(ROOT_DIR, module, ledger)
    )


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            "fixtures/sumeragi_v2/wire_v2.tsv",
            "# kind\tname\thex\texpectation",
            "# category\tname\thex\texpectation",
        ),
        (
            "fixtures/sumeragi_v2/wire_v2.tsv",
            "negative_message\texecution_commitment_merge_carrier_wrong_version\t",
            "negative_message\texecution_commitment_merge_carrier_retired_version\t",
        ),
        (
            "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
            "&options.output_dir.join(WIRE_FIXTURE_BASENAME),",
            '&options.output_dir.join("wire_v2.tsv"),',
        ),
        (
            "crates/iroha_data_model/tests/sumeragi_v2_cross_sdk_fixtures.rs",
            'const FIXTURES: &str = include_str!("../../../fixtures/sumeragi_v2/wire_v2.tsv");',
            'const FIXTURES: &str = "";',
        ),
        (
            "crates/iroha_data_model/tests/sumeragi_v2_cross_sdk_fixtures.rs",
            "fn shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation()",
            "fn shared_sdk_negative_fixtures_only_decode_structure()",
        ),
        (
            "scripts/run_sumeragi_v2_release_gates.sh",
            "cross-sdk-rust cargo-exact 2",
            "cross-sdk-rust cargo-exact 1",
        ),
    ),
)
def test_wire_release_invariant_rejects_semantic_source_mutation(
    tmp_path: Path,
    relative: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    ledger = canonical_binding_ledger()
    copy_wire_release_invariant_fixture(tmp_path, ledger)
    replace_once(tmp_path / relative, old, new)
    errors = validate_closure_mutations(tmp_path, module, ledger)
    assert any(
        relative in error and "missing source-binding token" in error
        for error in errors
    ), errors
