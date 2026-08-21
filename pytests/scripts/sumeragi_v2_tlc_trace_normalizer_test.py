"""Adversarial tests for the exact TLC 1.7.4 replay transcript contract."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
NORMALIZER_PATH = ROOT / "scripts/normalize_sumeragi_v2_tlc_trace.py"
HELPER_PATH = ROOT / "scripts/formal/sumeragi_v2_replay_receipt_test.py"
FIXTURE = ROOT / "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


NORMALIZER = load("sumeragi_v2_tlc_normalizer", NORMALIZER_PATH)
HELPER = load("sumeragi_v2_replay_test_helper", HELPER_PATH)
VALID = HELPER.canonical_tlc_log()
STATE_BLOCK = re.compile(
    r"@!@!@STARTMSG 2217:4 @!@!@\n.*?\n@!@!@ENDMSG 2217 @!@!@\n?",
    re.DOTALL,
)


def test_exact_transcript_normalizes_to_tracked_bytes() -> None:
    actions = NORMALIZER.normalize(VALID, 19349663, 0)

    assert len(actions) == 100
    assert actions[-1].action == "PersistDecision"
    assert NORMALIZER.render(actions, 19349663).encode("utf-8") == FIXTURE.read_bytes()


def test_requires_exact_101_state_and_100_action_census() -> None:
    states = list(STATE_BLOCK.finditer(VALID))
    assert len(states) == 101
    source = VALID[: states[50].start()] + VALID[states[50].end() :]

    with pytest.raises(ValueError):
        NORMALIZER.normalize(source, 19349663, 0)


@pytest.mark.parametrize(
    "source",
    (
        VALID.replace("Parsing file /sealed/Naturals.tla\nParsing file /sealed/Integers.tla", "Parsing file /sealed/Integers.tla\nParsing file /sealed/Naturals.tla", 1),
        VALID.replace("Semantic processing of module FiniteSets\n", "", 1),
        VALID.replace(
            HELPER.message(2219, 0, "SANY finished."),
            HELPER.message(2219, 0, "SANY finished.")
            + "\nSemantic processing of module ExtraModule",
            1,
        ),
        VALID.replace(
            HELPER.message(2110, 1, "Invariant NoDecision is violated.")
            + "\n"
            + HELPER.message(2121, 1, "The behavior up to this point is:"),
            HELPER.message(2121, 1, "The behavior up to this point is:")
            + "\n"
            + HELPER.message(2110, 1, "Invariant NoDecision is violated."),
            1,
        ),
        VALID.replace(
            HELPER.message(2186, 0, "Finished in 01s at (2026-08-21 12:00:01)"),
            HELPER.message(2000, 0, "unexpected")
            + "\n"
            + HELPER.message(2186, 0, "Finished in 01s at (2026-08-21 12:00:01)"),
            1,
        ),
        VALID.replace(
            HELPER.message(
                2209,
                0,
                "Progress(-1) at 2026-08-21 12:00:01: 1,002 states generated, "
                "-1 distinct states found, -1 states left on queue.",
            ),
            "",
            1,
        ),
        VALID + HELPER.message(2000, 0, "after termination") + "\n",
    ),
)
def test_rejects_incomplete_reordered_or_extra_transcript_items(source: str) -> None:
    with pytest.raises(ValueError):
        NORMALIZER.normalize(source, 19349663, 0)


@pytest.mark.parametrize(
    "source",
    (
        VALID.replace("Starting SANY...", "Starting SANY...\nError: hidden raw diagnostic", 1),
        VALID.replace("  peer |-> -1,", "  peer |-> -1,\nError: hidden state diagnostic", 1),
        VALID.replace("@!@!@STARTMSG 2262:0 @!@!@", "@!@!@BROKEN 2262:0 @!@!@", 1),
        VALID.replace("TLC2 Version", "TLC2\x00 Version", 1),
        VALID.replace("TLC2 Version", "TLC2\x85 Version", 1),
        VALID.replace("TLC2 Version", "TLC2\u00a0Version", 1),
    ),
)
def test_rejects_hidden_diagnostics_bad_framing_and_controls(source: str) -> None:
    with pytest.raises(ValueError):
        NORMALIZER.normalize(source, 19349663, 0)


@pytest.mark.parametrize(
    "source",
    (
        VALID.replace(
            "1: <Initial predicate>\n/\\ witnessAction",
            "1: <Initial predicate>\n2: <WitnessNext line 1, col 1 to line 1, "
            "col 1 of module SumeragiV2TraceWitness>\n/\\ witnessAction",
            1,
        ),
        VALID.replace(
            "2: <WitnessNext line 1, col 1 to line 1, col 1 of module "
            "SumeragiV2TraceWitness>",
            "2: <Initial predicate>",
            1,
        ),
        VALID.replace("  peer |-> -1,", "peer |-> -1,", 1),
        VALID.replace(
            "/\\ witnessAction = [ node |-> -1,",
            "/\\ witnessAction = [ node |-> \"-\",",
            1,
        ),
        VALID.replace('action |-> "SetGST"', 'action |-> "RetiredDecision"', 1),
        VALID.replace(
            '/\\ witnessAction = [ node |-> -1,',
            '/\\ witnessAction = [ action |-> "Initial",\n/\\ witnessAction = [ node |-> -1,',
            1,
        ),
    ),
)
def test_rejects_nested_headings_malformed_assignments_and_records(source: str) -> None:
    with pytest.raises(ValueError):
        NORMALIZER.normalize(source, 19349663, 0)


@pytest.mark.parametrize(
    ("source", "seed", "aril"),
    (
        (VALID.replace("seed 19349663", "seed 99", 1), 19349663, 0),
        (VALID.replace("Simulation using seed 19349663", "Simulation using seed 99", 1), 19349663, 0),
        (VALID.replace("and aril 0", "and aril 9", 1), 19349663, 0),
        (VALID.replace("The number of states generated: 1,001", "The number of states generated: 999", 1), 19349663, 0),
        (VALID.replace("1,002 states generated", "999 states generated", 1), 19349663, 0),
        (VALID, 19349663, 1),
    ),
)
def test_rejects_seed_aril_and_generated_count_drift(
    source: str, seed: int, aril: int
) -> None:
    with pytest.raises(ValueError):
        NORMALIZER.normalize(source, seed, aril)
