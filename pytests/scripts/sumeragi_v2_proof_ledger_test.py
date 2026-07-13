"""Tests for the fail-closed Sumeragi v2 formal proof ledger gate."""

from __future__ import annotations

import copy
import importlib.util
import json
import re
import shutil
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("sumeragi_v2_proof_ledger", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_repository_ledger_has_only_explicit_unproved_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    result = module.validate_ledger(ledger)

    missing_prefix = "missing required TLA+ module: "
    missing_symbol_prefixes = {
        f"obligations[{index}] references missing symbol "
        for index, obligation in enumerate(ledger["obligations"])
        if obligation["status"] == "specified_unproved"
    }
    assert all(
        error.startswith(missing_prefix)
        or any(error.startswith(prefix) for prefix in missing_symbol_prefixes)
        for error in result.errors
    )
    for error in result.errors:
        if error.startswith(missing_prefix):
            assert not Path(error.removeprefix(missing_prefix)).is_file()
    assert result.machine_checked_completion is ledger["machine_checked_completion"]


def test_retired_v1_corridor_is_absent() -> None:
    module = load_checker()

    assert all(not module._retired_path_present(path) for path in module.RETIRED_PATHS)


def test_release_gate_fails_closed_while_completion_is_false() -> None:
    module = load_checker()
    result = module.validate_ledger(module.load_ledger(), release=True)

    assert "release gate requires machine_checked_completion=true" in result.errors
    assert any("release gate rejects unproved obligation" in error for error in result.errors)
    assert "release gate requires fresh TLAPS proof evidence" in result.errors


def complete_ledger(module):
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True
    for obligation in ledger["obligations"]:
        if obligation["status"] == "specified_unproved":
            obligation["status"] = "tlaps_proved"
    return ledger


def build_test_evidence(module, tmp_path: Path):
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    log_dir = tmp_path / "target" / "formal" / "sumeragi_v2" / "tlaps"
    log_dir.mkdir(parents=True)
    for name in module.RELEASE_PROOF_MODULES:
        (log_dir / f"{name}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            f"SUMERAGI_TLAPS_BACKEND_COMPLETE module={name} "
            f"commit={module.TLAPM_COMMIT}\n",
            encoding="utf-8",
        )
    evidence = module.build_release_evidence(
        tlapm_version=f"TLAPM 1.6.0-pre (commit {module.TLAPM_COMMIT[:7]})",
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )
    return formal_dir, log_dir, evidence


def test_release_gate_requires_every_deductive_module_and_positive_counts(
    tmp_path: Path,
) -> None:
    module = load_checker()
    ledger = complete_ledger(module)
    formal_dir, _, evidence = build_test_evidence(module, tmp_path)

    assert module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    ) == []

    evidence["modules"].pop()
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    )
    assert any("cover exactly the release proof modules" in error for error in errors)


def test_release_evidence_is_bound_to_current_sources_and_logs(tmp_path: Path) -> None:
    module = load_checker()
    ledger = complete_ledger(module)
    formal_dir, log_dir, evidence = build_test_evidence(module, tmp_path)

    first_source = next(formal_dir.glob("*.tla"))
    first_source.write_text(first_source.read_text() + "\n\\* drift\n", encoding="utf-8")
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    )
    assert "proof evidence source manifest does not match current TLA+ sources" in errors

    formal_dir, log_dir, evidence = build_test_evidence(module, tmp_path / "fresh")
    first_log = log_dir / f"{module.RELEASE_PROOF_MODULES[0]}.log"
    first_log.write_text(first_log.read_text() + "drift\n", encoding="utf-8")
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path / "fresh"
    )
    assert any("log digest mismatch" in error for error in errors)


def test_tlaps_proved_symbol_must_be_a_theorem_declaration() -> None:
    module = load_checker()
    source = """---- MODULE Example ----
OperatorOnly == TRUE
THEOREM Proved == TRUE
=============================================================================
"""

    assert module._symbol_exists(source, "OperatorOnly")
    assert not module._symbol_exists(source, "OperatorOnly", theorem_only=True)
    assert module._symbol_exists(source, "Proved", theorem_only=True)


def test_release_module_list_covers_every_present_module_with_theorems() -> None:
    module = load_checker()
    theorem_modules = {
        path.stem
        for path in module.FORMAL_DIR.glob("*.tla")
        if re.search(r"(?m)^THEOREM\b", module.strip_tla_comments(path.read_text()))
    }
    present_release_modules = {
        name
        for name in module.RELEASE_PROOF_MODULES
        if (module.FORMAL_DIR / f"{name}.tla").is_file()
    }

    assert theorem_modules == present_release_modules


def test_async_production_model_and_proofs_are_ci_gated() -> None:
    module = load_checker()

    assert "SumeragiV2AsyncNetwork" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2AsyncLivenessProofs" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2AsyncLivenessProofs" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2AsyncNetwork" not in module.RELEASE_PROOF_MODULES


def test_verus_runner_records_output_without_masking_failures() -> None:
    source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text(
        encoding="utf-8"
    )

    assert "set -euo pipefail" in source
    assert "target/formal/sumeragi_v2/verus.log" in source
    assert '2>&1 | tee "$VERUS_LOG"' in source


def test_tla_shortcut_scan_rejects_unchecked_constructs_but_allows_proof_assume(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = """---- MODULE Example ----
ASSUME Unsafe
AXIOM Hidden
THEOREM Broken == TRUE BY OMITTED
THEOREM Structured == TRUE
PROOF
  <1>1. ASSUME TRUE
         PROVE TRUE
    OBVIOUS
  <1> QED BY <1>1
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert any("top-level ASSUME" in error for error in errors)
    assert any("top-level AXIOM" in error for error in errors)
    assert any("OMITTED proof" in error for error in errors)


def test_tla_shortcut_scan_ignores_comments_and_nested_comments(tmp_path: Path) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = """---- MODULE Example ----
(* ASSUME CommentOnly (* AXIOM Nested *) OMITTED *)
\\* AXIOM line comment
Safe == "OMITTED"
=============================================================================
"""

    assert module.tla_shortcut_errors(path, source) == []


def test_verus_shortcut_scan_rejects_assume_admit_and_external_body(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "proof.rs"
    source = """
fn bad() { assume(true); admit(); }
#[verifier::external_body]
fn hidden() {}
"""

    errors = module.verus_shortcut_errors(path, source)
    assert len(errors) == 3


def test_duplicate_json_keys_are_rejected(tmp_path: Path) -> None:
    module = load_checker()
    path = tmp_path / "ledger.json"
    path.write_text('{"schema_version": 1, "schema_version": 2}', encoding="utf-8")

    with pytest.raises(module.DuplicateKeyError):
        module.load_ledger(path)


def test_duplicate_obligation_ids_and_unknown_status_are_rejected() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["obligations"][1]["id"] = ledger["obligations"][0]["id"]
    ledger["obligations"][1]["status"] = "bounded_model_checked"

    errors = module.validate_ledger(ledger).errors
    assert any("duplicate proof obligation id" in error for error in errors)
    assert any("unknown value" in error for error in errors)


def test_tlc_runner_cannot_claim_or_mutate_proof_completion() -> None:
    runner = (ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh").read_text()

    assert "COUNTEREXAMPLE SEARCH ONLY" in runner
    assert "no proof status was changed" in runner
    assert "proof_coverage.json" not in runner
    assert "machine_checked_completion" not in runner


def test_formal_gate_validates_fresh_evidence_before_tlc_and_replay() -> None:
    source = (ROOT_DIR / "ci" / "check_sumeragi_formal.sh").read_text()

    tlaps = source.index("run_sumeragi_v2_tlaps.sh")
    release = source.index("--release")
    tlc = source.index("run_sumeragi_v2_tlc.sh")
    replay = source.index("check_sumeragi_v2_replay_trace.sh")
    verus = source.index("verify_sumeragi_v2.sh")
    assert tlaps < release < tlc < replay < verus
    assert "proof_evidence.json" in source


def test_tla2tools_and_replay_share_the_same_pin() -> None:
    scripts = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all('1.8.0' in source for source in sources)
    assert all(
        "33de7da9ce1b7fffb9d1c184021178dbb051747be48504e65c584c423721a32e"
        in source
        for source in sources
    )


def test_workspace_excluded_harness_names_every_required_fast_simulation() -> None:
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    expected = {
        "lossy_offline_leader_simulations_commit_for_4_7_and_10_validators",
        "two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits",
        "leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum",
        "corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission",
        "crash_after_proposal_wal_before_signature_replays_exact_intent",
        "taira_divergent_views_converge_and_commit_within_one_rotation",
        "accelerated_chain_chaos_smoke_preserves_prefix",
    }

    assert all(name in source for name in expected)
    assert "expected six Sumeragi v2 network simulations" not in source
    assert "--unit" in source
    assert "--model-replay" in source
    assert "--chaos-100k" in source


def test_installers_use_fixed_urls_and_literal_checksums() -> None:
    installers = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_verus.sh",
    ]
    for installer in installers:
        source = installer.read_text(encoding="utf-8")
        assert "latest" not in source.lower()
        assert re.search(r'readonly [A-Z_]*SHA256="[0-9a-f]{64}"', source)
        assert "curl" in source
        assert "checksum mismatch" in source


def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"
