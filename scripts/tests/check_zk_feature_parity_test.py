"""Tests of scripts/check_zk_feature_parity.py: record parsing and comparison.

The comparison is exercised on captured outputs; no cargo build runs here.

The record test prints no `gated` record now (STARK verification is compiled
into every build). The gated mechanism is exercised with a sample record of
the `sm` feature, which the census still classifies as a gate and which the
`node-without-sm-gost` selection leaves out.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tomllib

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_zk_feature_parity.py"
RECORD_TEST = ROOT / "crates" / "iroha_core" / "src" / "zk_validity_policy_feature_parity_tests.rs"

_spec = importlib.util.spec_from_file_location("check_zk_feature_parity", SCRIPT)
assert _spec and _spec.loader
parity = importlib.util.module_from_spec(_spec)
sys.modules["check_zk_feature_parity"] = parity
_spec.loader.exec_module(parity)

DIGEST_A = "a" * 64
DIGEST_B = "b" * 64
GATE_ENTRY = "feature.sm_gost_bls_pqc"
GATED_RECORD = "sm.sm2_signature"
RETIRED_STARK_FEATURES = ("zk-stark", "proofs-stark", "proofs-full")


def output(features: str, must_equal: dict, gated: dict, noise: bool = True) -> str:
    lines = ["running 1 test"] if noise else []
    # The harness prints the test name before the first output line.
    prefix = "test zk_validity_policy::parity_tests::feature_parity::feature_parity_record ... " if noise else ""
    lines.append(f"{prefix}ZK_FEATURE_PARITY features={features}")
    lines += [f"ZK_FEATURE_PARITY must_equal.{k}={v}" for k, v in must_equal.items()]
    lines += [f"ZK_FEATURE_PARITY gated.{k}={v}" for k, v in gated.items()]
    if noise:
        lines.append("ok")
        lines.append("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11839 filtered out")
    return "\n".join(lines) + "\n"


def census(status: str = "open", feature_class: str = "validity_gate") -> dict:
    return {
        "entries": [{"id": GATE_ENTRY, "status": status}],
        "features": {
            "iroha_core": {
                "sm": {"class": feature_class, "entry": GATE_ENTRY},
                "simd": {"class": "acceleration", "reason": "scalar fallback"},
            }
        },
    }


def records(with_sm: str, without_sm: str, **overrides) -> dict:
    full = parity.parse_record(
        "node",
        output(
            "sm,gost,simd",
            overrides.get("full_must", {"certified_history": DIGEST_A, "verification_gas": DIGEST_B}),
            {GATED_RECORD: with_sm},
        ),
    )
    reduced = parity.parse_record(
        "node-without-sm-gost",
        output(
            "simd",
            overrides.get("reduced_must", {"certified_history": DIGEST_A, "verification_gas": DIGEST_B}),
            {GATED_RECORD: without_sm},
        ),
    )
    return {"node": full, "node-without-sm-gost": reduced}


def ungated_records(**overrides) -> dict:
    """Records as the test prints them now: `must_equal` lines and no `gated` line."""
    must = {"certified_history": DIGEST_A, "stark_real_proof": DIGEST_B}
    full = parity.parse_record("node", output("sm,gost,simd", overrides.get("full_must", must), {}))
    reduced = parity.parse_record(
        "node-without-sm-gost", output("simd", overrides.get("reduced_must", must), {})
    )
    return {"node": full, "node-without-sm-gost": reduced}


def test_identical_must_equal_and_an_open_gate_pass() -> None:
    gates = parity.load_gates(census())
    assert gates == {"sm": {"entry": GATE_ENTRY, "status": "open"}}
    assert parity.compare(records("verified", "not-compiled"), gates) == []


def test_records_without_a_gated_line_compare_must_equal_only() -> None:
    # No operation is feature-gated: an open census gate of a feature that
    # differs between the selections is neither a difference nor stale.
    for status in ("open", "done"):
        gates = parity.load_gates(census(status=status))
        compared = ungated_records()
        assert all(record["gated"] == {} for record in compared.values())
        assert parity.compare(compared, gates) == []
    assert parity.compare(ungated_records(), {}) == []
    # A census that lists no gate feature of `iroha_core`, or no feature map at all.
    assert parity.load_gates({"entries": [], "features": {}}) == {}
    assert parity.load_gates({}) == {}
    # The real STARK proof is a must-equal record: a build that changed it fails.
    problems = parity.compare(
        ungated_records(reduced_must={"certified_history": DIGEST_A, "stark_real_proof": DIGEST_A}),
        parity.load_gates(census()),
    )
    assert [problem.split(":")[0] for problem in problems] == ["must_equal.stark_real_proof"]
    # A gated record only one selection prints is reported, not skipped.
    lopsided = ungated_records()
    lopsided["node"]["gated"][GATED_RECORD] = "verified"
    assert parity.compare(lopsided, parity.load_gates(census())) == [
        f"gated.{GATED_RECORD}: not printed by ['node-without-sm-gost']"
    ]


def test_a_feature_that_changes_a_root_or_gas_fails() -> None:
    gates = parity.load_gates(census())
    problems = parity.compare(
        records(
            "accepted",
            "not-compiled",
            reduced_must={"certified_history": DIGEST_B, "verification_gas": DIGEST_B},
        ),
        gates,
    )
    assert len(problems) == 1
    assert problems[0].startswith("must_equal.certified_history: a Cargo feature changes this record")
    problems = parity.compare(
        records(
            "accepted",
            "not-compiled",
            reduced_must={"certified_history": DIGEST_A, "verification_gas": DIGEST_A},
        ),
        gates,
    )
    assert [p.split(":")[0] for p in problems] == ["must_equal.verification_gas"]


def test_a_missing_must_equal_record_fails() -> None:
    problems = parity.compare(
        records("accepted", "not-compiled", reduced_must={"certified_history": DIGEST_A}),
        parity.load_gates(census()),
    )
    assert problems == ["must_equal.verification_gas: not printed by ['node-without-sm-gost']"]


def test_a_gated_difference_needs_an_open_census_gate() -> None:
    differing = records("accepted", "not-compiled")
    # The census says the gate is removed, yet the builds still differ.
    problems = parity.compare(differing, parity.load_gates(census(status="done")))
    assert len(problems) == 1 and "has status `done`" in problems[0]
    # The census does not classify the feature as a gate at all.
    problems = parity.compare(differing, parity.load_gates(census(feature_class="acceleration")))
    assert len(problems) == 1 and "does not classify `iroha_core/sm`" in problems[0]


def test_an_open_gate_that_no_longer_differs_is_stale() -> None:
    agreeing = records("accepted", "accepted")
    problems = parity.compare(agreeing, parity.load_gates(census()))
    assert len(problems) == 1 and "update the census" in problems[0]
    # With the gate closed in the census the same records pass.
    assert parity.compare(agreeing, parity.load_gates(census(status="done"))) == []
    # Two selections that both compile the feature say nothing about the gate.
    both = {
        "node": agreeing["node"],
        "node-again": parity.parse_record(
            "node-again",
            output("sm,gost,simd", {"certified_history": DIGEST_A, "verification_gas": DIGEST_B}, {GATED_RECORD: "accepted"}),
        ),
    }
    assert parity.compare(both, parity.load_gates(census())) == []


def test_the_harness_prefix_on_the_first_line_is_tolerated() -> None:
    record = parity.parse_record(
        "node", output("sm,gost,simd", {"certified_history": DIGEST_A}, {GATED_RECORD: "accepted"})
    )
    assert record["features"] == {"compiled": "sm,gost,simd"}
    assert record["must_equal"] == {"certified_history": DIGEST_A}
    assert record["gated"] == {GATED_RECORD: "accepted"}
    plain = parity.parse_record(
        "node",
        output("sm,gost,simd", {"certified_history": DIGEST_A}, {GATED_RECORD: "accepted"}, noise=False),
    )
    assert plain == record
    # A record without a `gated` line parses to an empty gated group.
    ungated = parity.parse_record("node", output("sm,gost,simd", {"certified_history": DIGEST_A}, {}))
    assert ungated["gated"] == {} and ungated["must_equal"] == record["must_equal"]


def test_one_selection_is_not_a_comparison() -> None:
    single = {"node": records("accepted", "not-compiled")["node"]}
    assert parity.compare(single, {}) == ["need at least two feature selections to compare, got ['node']"]


@pytest.mark.parametrize(
    "text, message",
    [
        ("running 1 test\n", "printed no `ZK_FEATURE_PARITY` line"),
        ("ZK_FEATURE_PARITY features=sm\n", "no `must_equal` record"),
        ("ZK_FEATURE_PARITY must_equal.history\n", "malformed record line"),
        ("ZK_FEATURE_PARITY other.history=aa\n", "unknown record `other.history`"),
        ("ZK_FEATURE_PARITY must_equal.history=\n", "record `must_equal.history` is empty"),
        (
            "ZK_FEATURE_PARITY must_equal.history=aa\nZK_FEATURE_PARITY must_equal.history=aa\n",
            "printed twice",
        ),
    ],
)
def test_malformed_output_is_rejected(text: str, message: str) -> None:
    with pytest.raises(parity.ParityError, match=message.replace("`", "`")):
        parity.parse_record("node", text)


def test_cli_compares_captured_outputs(tmp_path: Path, capsys: pytest.CaptureFixture) -> None:
    census_path = tmp_path / "census.json"
    census_path.write_text(json.dumps(census()), encoding="utf-8")
    full = tmp_path / "node.txt"
    reduced = tmp_path / "reduced.txt"
    full.write_text(output("sm,gost", {"certified_history": DIGEST_A}, {GATED_RECORD: "accepted"}))
    reduced.write_text(output("", {"certified_history": DIGEST_A}, {GATED_RECORD: "not-compiled"}))
    arguments = [
        "--census",
        str(census_path),
        "--from-output",
        f"node={full}",
        "--from-output",
        f"node-without-sm-gost={reduced}",
    ]
    assert parity.main(arguments) == 0
    printed = capsys.readouterr().out
    assert "OK (1 must-equal records identical" in printed
    assert "gated differences are open census gates" in printed
    assert f"node: gated.{GATED_RECORD}=accepted" in printed
    reduced.write_text(output("", {"certified_history": DIGEST_B}, {GATED_RECORD: "not-compiled"}))
    assert parity.main(arguments) == 1
    assert "a Cargo feature changes this record" in capsys.readouterr().err
    # The outputs of today's test: no gated line, and the census decides nothing.
    full.write_text(output("sm,gost", {"certified_history": DIGEST_A, "stark_real_proof": DIGEST_B}, {}))
    reduced.write_text(output("", {"certified_history": DIGEST_A, "stark_real_proof": DIGEST_B}, {}))
    assert parity.main(arguments) == 0
    printed = capsys.readouterr().out
    assert "OK (2 must-equal records identical" in printed and "no gated record" in printed
    assert "gated." not in printed
    reduced.write_text(output("", {"certified_history": DIGEST_A, "stark_real_proof": DIGEST_A}, {}))
    assert parity.main(arguments) == 1
    assert "must_equal.stark_real_proof: a Cargo feature changes this record" in capsys.readouterr().err
    assert parity.main(["--census", str(census_path), "--from-output", "node"]) == 2
    assert parity.main(["--census", str(tmp_path / "absent.json"), "--from-output", f"node={full}"]) == 2


def test_selections_and_the_record_test_agree_with_the_source() -> None:
    # The default selection is the crate's default feature set; the other is
    # an explicit list that drops exactly one gate family.
    assert parity.SELECTIONS["node"] is None
    assert set(parity.SELECTIONS) == {"node", "node-without-sm-gost"}
    assert parity.DEFAULT_SELECTIONS == ("node", "node-without-sm-gost")
    assert set(parity.NODE_FEATURES) - set(parity.SELECTIONS["node-without-sm-gost"]) == {"sm", "gost"}
    command = parity.cargo_command("node-without-sm-gost", "/tmp/target")
    assert command[:5] == ["cargo", "test", "-p", "iroha_core", "--lib"]
    assert "--no-default-features" in command and "--target-dir" in command
    assert parity.TEST_FILTER in command
    assert "--no-default-features" not in parity.cargo_command("node", None)
    # `node` of crates/iroha_core/Cargo.toml plus `simd` is the list compared here.
    manifest = tomllib.loads((ROOT / "crates" / "iroha_core" / "Cargo.toml").read_text(encoding="utf-8"))
    declared = manifest["features"]
    assert set(declared["node"]) | {"simd"} == set(parity.NODE_FEATURES)
    assert declared["default"] == ["node", "simd"]
    # The Rust test prints the prefix and the record names this script reads.
    source = RECORD_TEST.read_text(encoding="utf-8")
    assert 'const LINE_PREFIX: &str = "ZK_FEATURE_PARITY";' in source
    assert "fn feature_parity_record()" in source
    assert 'println!("{LINE_PREFIX} gated.{name}={value}");' in source


def test_stark_verification_is_not_a_build_option() -> None:
    # There is no build without STARK: no selection, feature list or manifest
    # names a STARK feature, and the real proof is a must-equal record.
    manifest = tomllib.loads((ROOT / "crates" / "iroha_core" / "Cargo.toml").read_text(encoding="utf-8"))
    for retired in RETIRED_STARK_FEATURES:
        assert retired not in manifest["features"]
        assert retired not in parity.NODE_FEATURES
        for name, features in parity.SELECTIONS.items():
            assert retired not in (features or ()), name
    assert not any("stark" in name for name in parity.SELECTIONS)
    source = RECORD_TEST.read_text(encoding="utf-8")
    assert '"stark_real_proof",' in source
    assert "assert!(record.gated.is_empty()" in source
    assert "record.gated.insert" not in source
    for retired in RETIRED_STARK_FEATURES:
        assert retired not in source
    # Every gate feature the real census lists for `iroha_core` is a feature
    # the manifest declares, so the census names no gate of a retired feature.
    gates = parity.load_gates(json.loads((ROOT / parity.CENSUS).read_text(encoding="utf-8")))
    assert set(gates) <= set(manifest["features"]), sorted(set(gates) - set(manifest["features"]))
