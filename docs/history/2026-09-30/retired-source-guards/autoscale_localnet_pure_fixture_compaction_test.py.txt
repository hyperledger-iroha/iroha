#!/usr/bin/env python3
"""Guard the autoscale localnet pure-fixture consolidation."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TARGET = REPO_ROOT / "integration_tests/tests/nexus/autoscale_localnet.rs"
SOURCE = TARGET.read_text(encoding="utf-8")

ORIGINAL_RUST_LINES = 13_063
MINIMUM_RUST_LINE_SAVING = 750
MAXIMUM_RUST_LINES = ORIGINAL_RUST_LINES - MINIMUM_RUST_LINE_SAVING
EXPECTED_TEST_COUNT = 108
EXPECTED_TEST_RECORD_DIGEST = (
    "daa7d43c196f168f430760b7376d7b99f26c2b746cfee84283133124a164c5da"
)

REAL_LOCALNET_TEST_FINGERPRINTS = {
    "nexus_autoscale_expands_and_contracts_lanes_in_localnet": (
        "b1e10997d19425d21574d9332d2a54c3d11a5ee516e7a1cbf7033c90291e6a47"
    ),
    "nexus_autoscale_repeats_expand_contract_cycles_in_localnet": (
        "ed73d278689d43c841e8d1c66b5d36bda57a360e1a07cfee82c82638bb636749"
    ),
    "nexus_autoscale_strict_expand_contract_transitions_in_localnet": (
        "876bd2f406aa3027660de6c0d44f1c96864251c32e449f11a5ed3e26606350b6"
    ),
    "nexus_autoscale_public_profile_strict_expand_contract_transitions_in_localnet": (
        "4d1233abf50c994e2a20af3cd1434dda3d15b5970b5f3a57325f7ebcc1b9442b"
    ),
    "nexus_autoscale_soak_expand_contract_cycles_in_localnet": (
        "9a7ad8f1d476e40b6993f3fe4809b0809ed5f8603937ad376a5f85a1f5549920"
    ),
}

REGION_FINGERPRINTS = {
    "transition constructor": (
        "    const fn transition_stats(",
        "    fn relay_snapshot(",
        "4dc4cb72107cdd25d077223af540fe5e10e60da85fac30469c35f7687678ce35",
    ),
    "commit quorum and committed-lane status": (
        "    fn status_with_commit_quorum(",
        "    fn utilization_permille_for_probe_tx(",
        "2aa20b5f8e9bc5cdcc9aa5037636df7516e856dfc966d55ca667a085cb3a7f66",
    ),
    "transition parser and delta fixtures": (
        "    #[test]\n    fn autoscale_transition_stats_parse_log_markers()",
        "    #[test]\n    fn soak_summary_serialization_contains_required_fields()",
        "e2fca03b022fa2491d50fb69f75737a96ac8ded8331e4c3c7fde06c9e0bb195f",
    ),
    "public-profile evidence fixtures": (
        "    #[test]\n    fn public_profile_expansion_ignores_wrong_elastic_lane_signal()",
        "    #[derive(Clone, Copy)]\n    enum ExpansionGovernanceSpec",
        "6feed84a4a1e73af423612a2b2396df26bf2e923a59799d11e62d98e9c660b34",
    ),
    "typed expansion acceptance matrix": (
        "    #[derive(Clone, Copy)]\n    enum ExpansionGovernanceSpec",
        "    #[test]\n    fn expansion_rejects_ambiguous_lane_validator_rows()",
        "f411d1529d72dcdc0fe56c3b4eed91af8e9cdf8b0774740e9f60cb9e0cfc1391",
    ),
}

EXPECTED_WRAPPERS = {
    "expansion_accepts_scale_out_transition_quorum_without_status_signal": (
        "run_scale_out_transition_evidence_case()"
    ),
    "expansion_requires_active_lane_signal_on_quorum_peers": (
        "run_active_lane_evidence_case()"
    ),
    "expansion_accepts_sumeragi_lane_commitment_activity_on_quorum_peers": (
        "run_commitment_evidence_case()"
    ),
    "expansion_accepts_lane_declaration_transition_on_quorum_peers": (
        "run_baseline_expansion_evidence_case(BaselineExpansionEvidenceCase::Declaration)"
    ),
    "expansion_accepts_lane_progress_transition_on_quorum_peers": (
        "run_baseline_expansion_evidence_case("
        "BaselineExpansionEvidenceCase::CommitmentProgress)"
    ),
    "expansion_accepts_lane_validator_transition_on_quorum_peers": (
        "run_baseline_expansion_evidence_case(BaselineExpansionEvidenceCase::Validator)"
    ),
}


def _raw_literal_end(text: str, index: int) -> int | None:
    if text.startswith("br", index):
        delimiter_start = index + 2
    elif text.startswith("r", index):
        delimiter_start = index + 1
    else:
        return None
    quote = delimiter_start
    while quote < len(text) and text[quote] == "#":
        quote += 1
    if quote >= len(text) or text[quote] != '"':
        return None
    hashes = text[delimiter_start:quote]
    closing = '"' + hashes
    end = text.find(closing, quote + 1)
    if end < 0:
        raise AssertionError("unterminated Rust raw string")
    return end + len(closing)


def _string_literal_end(text: str, index: int) -> int:
    cursor = index + 1
    while cursor < len(text):
        if text[cursor] == "\\":
            cursor += 2
            continue
        if text[cursor] == '"':
            return cursor + 1
        cursor += 1
    raise AssertionError("unterminated Rust string")


def _character_literal_end(text: str, index: int) -> int | None:
    if (
        index + 2 < len(text)
        and text[index + 1] != "\\"
        and text[index + 2] == "'"
    ):
        return index + 3
    if index + 1 < len(text) and text[index + 1] == "\\":
        cursor = index + 2
        while cursor < min(len(text), index + 20):
            if text[cursor] == "'":
                return cursor + 1
            cursor += 1
    return None


def _block_comment_end(text: str, index: int) -> int:
    depth = 1
    cursor = index + 2
    while cursor < len(text) and depth:
        if text.startswith("/*", cursor):
            depth += 1
            cursor += 2
        elif text.startswith("*/", cursor):
            depth -= 1
            cursor += 2
        else:
            cursor += 1
    if depth:
        raise AssertionError("unterminated Rust block comment")
    return cursor


def _canonical_rust(text: str) -> str:
    """Discard formatting/comments while retaining every literal byte and token."""

    output: list[str] = []
    cursor = 0
    while cursor < len(text):
        raw_end = _raw_literal_end(text, cursor)
        if raw_end is not None:
            output.append(text[cursor:raw_end])
            cursor = raw_end
            continue
        if text.startswith("//", cursor):
            newline = text.find("\n", cursor + 2)
            cursor = len(text) if newline < 0 else newline + 1
            continue
        if text.startswith("/*", cursor):
            cursor = _block_comment_end(text, cursor)
            continue
        if text[cursor] == '"':
            end = _string_literal_end(text, cursor)
            output.append(text[cursor:end])
            cursor = end
            continue
        if text[cursor] == "'":
            end = _character_literal_end(text, cursor)
            if end is not None:
                output.append(text[cursor:end])
                cursor = end
                continue
        if text[cursor].isspace():
            cursor += 1
            continue
        output.append(text[cursor])
        cursor += 1
    return "".join(output)


def _fingerprint(text: str) -> str:
    return hashlib.sha256(_canonical_rust(text).encode("utf-8")).hexdigest()


def _matching_brace(text: str, opening: int) -> int:
    depth = 1
    cursor = opening + 1
    while cursor < len(text) and depth:
        raw_end = _raw_literal_end(text, cursor)
        if raw_end is not None:
            cursor = raw_end
            continue
        if text.startswith("//", cursor):
            newline = text.find("\n", cursor + 2)
            cursor = len(text) if newline < 0 else newline + 1
            continue
        if text.startswith("/*", cursor):
            cursor = _block_comment_end(text, cursor)
            continue
        if text[cursor] == '"':
            cursor = _string_literal_end(text, cursor)
            continue
        if text[cursor] == "'":
            end = _character_literal_end(text, cursor)
            if end is not None:
                cursor = end
                continue
        if text[cursor] == "{":
            depth += 1
        elif text[cursor] == "}":
            depth -= 1
        cursor += 1
    if depth:
        raise AssertionError("unterminated Rust function body")
    return cursor


def _function_source(source: str, name: str) -> str:
    pattern = re.compile(
        rf"(?m)^[ \t]*(?:pub(?:\([^\n)]*\))?[ \t]+)?"
        rf"(?:async[ \t]+)?fn[ \t]+{re.escape(name)}[ \t]*\("
    )
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise AssertionError(f"expected one function named {name}, found {len(matches)}")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise AssertionError(f"function {name} has no body")
    return source[matches[0].start() : _matching_brace(source, opening)]


def _test_records(source: str) -> list[tuple[tuple[str, ...], str]]:
    pattern = re.compile(
        r"(?m)^(?P<attrs>(?:[ \t]*#\[[^\n]+\][ \t]*\n)+)"
        r"[ \t]*(?:async[ \t]+)?fn[ \t]+(?P<name>[A-Za-z_]\w*)[ \t]*\("
    )
    records = []
    for match in pattern.finditer(source):
        attributes = tuple(re.findall(r"#\[[^\n]+\]", match.group("attrs")))
        if "#[test]" in attributes:
            records.append((attributes, match.group("name")))
    return records


def _region(source: str, start: str, end: str) -> str:
    if source.count(start) != 1 or source.count(end) != 1:
        raise AssertionError(f"region anchors are not unique: {start!r}, {end!r}")
    start_index = source.index(start)
    end_index = source.index(end, start_index + len(start))
    return source[start_index:end_index]


def _validate_source(source: str) -> None:
    line_count = len(source.splitlines())
    assert line_count <= MAXIMUM_RUST_LINES, (
        f"autoscale fixture source grew to {line_count} lines; the required "
        f"{MINIMUM_RUST_LINE_SAVING}-line saving permits at most {MAXIMUM_RUST_LINES}"
    )

    records = _test_records(source)
    assert len(records) == EXPECTED_TEST_COUNT
    digest = hashlib.sha256(repr(records).encode("utf-8")).hexdigest()
    assert digest == EXPECTED_TEST_RECORD_DIGEST, (
        "autoscale test names or ordered attributes changed"
    )

    for name, expected in REAL_LOCALNET_TEST_FINGERPRINTS.items():
        assert _fingerprint(_function_source(source, name)) == expected, (
            f"real localnet/soak test changed: {name}"
        )

    regions = {}
    for name, (start, end, expected) in REGION_FINGERPRINTS.items():
        regions[name] = _region(source, start, end)
        assert _fingerprint(regions[name]) == expected, f"{name} changed"

    callback_free_regions = (
        regions["commit quorum and committed-lane status"],
        regions["typed expansion acceptance matrix"],
    )
    for compacted in callback_free_regions:
        assert "|" not in compacted
        assert not any(token in compacted for token in ("Fn(", "FnMut", "FnOnce", "dyn Fn"))

    assert len(re.findall(r"(?<![A-Za-z0-9_])transition_stats\(", source)) == 41
    assert len(re.findall(r"(?<![A-Za-z0-9_])uniform_commit_quorum\(", source)) == 8
    assert (
        len(
            re.findall(
                r"(?<![A-Za-z0-9_])committed_lane_block_quorum_snapshot\(",
                source,
            )
        )
        == 8
    )
    assert (
        len(re.findall(r"(?<![A-Za-z0-9_])RejectedCommittedLaneExecution\(", source))
        == 10
    )

    for name, call in EXPECTED_WRAPPERS.items():
        expected = f"fn{name}(){{{call};}}"
        assert _canonical_rust(_function_source(source, name)) == expected


class AutoscaleLocalnetPureFixtureCompactionTest(unittest.TestCase):
    def test_source_contract(self) -> None:
        _validate_source(SOURCE)

    def test_semantic_mutations_fail_closed(self) -> None:
        mutations = (
            ("future_unknown_state", "future_unknown_state_accepted"),
            ("lane=3shadow active_lanes=3", "lane=03shadow active_lanes=3"),
            (
                "autoscale-localnet-contraction-relay-drift",
                "autoscale-localnet-contraction-relay-accepted",
            ),
            (".with_commitment(10, 4, 128)", ".with_commitment(10, 4, 127)"),
            (
                "[autoscale-localnet][multi-cycle] network startup",
                "[autoscale-localnet][multi-cycle] altered startup",
            ),
            (
                "expansion_accepts_lane_validator_transition_on_quorum_peers",
                "expansion_accepts_lane_validator_transition_without_quorum",
            ),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, SOURCE)
                mutated = SOURCE.replace(original, replacement, 1)
                self.assertNotEqual(mutated, SOURCE)
                with self.assertRaises(AssertionError):
                    _validate_source(mutated)

    def test_line_budget_mutation_fails_closed(self) -> None:
        padding = "\n".join(
            "// guard mutation padding"
            for _ in range(MAXIMUM_RUST_LINES - len(SOURCE.splitlines()) + 1)
        )
        with self.assertRaises(AssertionError):
            _validate_source(f"{SOURCE}\n{padding}\n")


if __name__ == "__main__":
    unittest.main()
