"""Qualification rejects missing evidence, cherry-picking and cap overruns."""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "kagemusha_qualify.py"
SPEC = importlib.util.spec_from_file_location("kagemusha_qualify", SCRIPT)
assert SPEC and SPEC.loader
qualify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualify)


def report(cpu=25, wall=8, rss=700_000_000, workers=1):
    return {
        "schema": "kagemusha.m3.process.v1", "workers": workers,
        "binary_sha256": "ab" * 32, "witness_api": "owned",
        "coset_cache": "on_demand", "commitment_tables": False,
        "msm_process_budget_bytes": 64 << 20,
        "msm_process_peak_bytes": 1 << 20, "msm_process_retained_bytes": 0,
        "transcript_profile": "pipa-r", "gate": "G3.6/q_exact_shape", "k": 16, "shape": "test-shape", "seed": 7,
        "descriptor_digest": "cd" * 32, "descriptor_hash": "blake2b256-pipa-v2-circdesc",
        "peak_rss_source": "kernel_lifetime_high_water", "peak_rss_bytes": rss,
        "thermal_before": "nominal", "thermal_after": "nominal",
        "samples": [
            {"index": i, "cpu_ns": int(cpu * 1e9), "total_ns": int(wall * 1e9),
             "verified": True, "thermal_before": "nominal", "thermal_after": "nominal"}
            for i in range(2)
        ],
    }


def attempts(value=None):
    value = value or report()
    return [{"block": block, "report": deepcopy(value), "reasons": []}
            for block in range(3) for _ in range(3)]


def test_all_nine_processes_and_each_block_median_are_required():
    rows = attempts()
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)["status"] == "pass"
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows[:8])["status"] == "inconclusive"
    rows[0]["block"] = 1
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)["status"] == "inconclusive"


def test_single_hard_failure_cannot_be_hidden_by_median_or_faster_second_proof():
    rows = attempts()
    rows[4]["report"]["samples"][0]["cpu_ns"] = 30_000_000_001
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)["status"] == "fail"
    rows = attempts()
    rows[4]["report"]["peak_rss_bytes"] = int(0.75 * qualify.GIB) + 1
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)["status"] == "fail"


def test_headroom_misses_are_borderline_and_every_block_matters():
    rows = attempts()
    for index in (3, 4):
        rows[index]["report"] = report(cpu=28)
    result = qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)
    assert result["block_medians_seconds"] == [25, 28, 25]
    assert result["status"] == "borderline"
    rows = attempts(report(rss=int(0.73 * qualify.GIB)))
    assert qualify.verdict(qualify.CONFIGS["q_chips-1"], rows)["status"] == "borderline"


def test_four_worker_observed_wall_is_never_cpu_divided_by_four():
    rows = attempts(report(cpu=20, wall=10.1, workers=4))
    assert qualify.verdict(qualify.CONFIGS["q_chips-4"], rows)["status"] == "fail"
    # A4 has a memory gate and diagnostic elapsed time, no invented time cap.
    rows = attempts(report(cpu=80, wall=40, workers=4))
    assert qualify.verdict(qualify.CONFIGS["a_exact-4"], rows)["status"] == "pass"


def test_invalid_samples_are_retained_but_cannot_supply_a_passing_count():
    rows = attempts()
    rows[2]["reasons"] = ["thermal state unavailable"]
    assert qualify.verdict(qualify.CONFIGS["q_exact-1"], rows)["status"] == "inconclusive"
    rows.append({"block": 0, "report": report(), "reasons": []})
    result = qualify.verdict(qualify.CONFIGS["q_exact-1"], rows)
    assert result["status"] == "pass"
    assert result["attempts"] == 10
    assert result["valid_processes"] == 9


def test_probe_errors_wrong_api_wrong_pool_and_unverified_proofs_invalidate():
    candidate = {"binary_sha256": "ab" * 32, "layouts": {
        "G3.6/q_exact_shape": {key: report()[key] for key in
            ("gate", "k", "shape", "descriptor_digest", "descriptor_hash")}
    }}
    config = qualify.CONFIGS["q_exact-1"]
    assert not qualify.report_reasons(report(), config, candidate, 7)
    for field, bad in [
        ("workers", 4), ("binary_sha256", "different"), ("witness_api", "borrowed"),
        ("peak_rss_bytes", 0), ("peak_rss_source", "sampled"),
        ("msm_process_budget_bytes", None), ("thermal_after", "unavailable"),
        ("msm_process_peak_bytes", (64 << 20) + 1), ("msm_process_retained_bytes", 1),
        ("gate", "G3.6/q_leaf_chips"), ("k", 15), ("shape", "other"), ("seed", 8),
        ("descriptor_digest", "different"), ("descriptor_hash", "sha256"),
        ("samples", None), ("samples", [None]),
    ]:
        value = report()
        value[field] = bad
        assert qualify.report_reasons(value, config, candidate, 7), field
    for field, bad in [("cpu_ns", 0), ("cpu_ns", -1), ("verified", False), ("index", 2)]:
        value = report()
        value["samples"][0][field] = bad
        assert qualify.report_reasons(value, config, candidate, 7), field


def test_calibration_uses_both_direct_metrics_with_no_normalization():
    assert not qualify.calibration_reasons(report(cpu=20, wall=10), report(cpu=21, wall=10.5))
    assert qualify.calibration_reasons(report(cpu=20), report(cpu=21.01))
    assert qualify.calibration_reasons(report(wall=10), report(wall=10.51))


def test_report_parser_requires_one_complete_known_record():
    text = "test ignored ... " + qualify.PREFIX + json.dumps(report()) + "\nok\n"
    assert qualify.parse_report(text) == report()
    for bad in ("", text + text, qualify.PREFIX + "[]", qualify.PREFIX + "{}"):
        with pytest.raises(ValueError):
            qualify.parse_report(bad)


def test_vm_counters_require_complete_valid_evidence():
    text = "Compressions: 123.\nPageouts: 0.\nSwapouts: 2.\nSwapins: 3.\n"
    assert qualify.parse_vm_stat(text) == {"Compressions": 123, "Pageouts": 0, "Swapouts": 2, "Swapins": 3}
    with pytest.raises(ValueError):
        qualify.parse_vm_stat("Compressions: 0.\n")
    before = {"valid": True, "counters": qualify.parse_vm_stat(text),
              "raw": {"power": {"stdout": "AC Power: normal"}}}
    assert not qualify.environment_reasons(before, deepcopy(before))
    after = deepcopy(before)
    after["counters"]["Compressions"] += 1
    assert qualify.environment_reasons(before, after)
    # Old compressed pages/counters are not a rejection; only new activity is.
    after = deepcopy(before)
    after["valid"] = False
    after["reason"] = "pressure probe failed"
    assert qualify.environment_reasons(before, after)


def ledger_fixture():
    candidate = {"binary_sha256": "ab" * 32, "layouts": {}}
    configs = {}
    for name, config in qualify.CONFIGS.items():
        layout = {key: report()[key] for key in
                  ("k", "shape", "descriptor_digest", "descriptor_hash", "transcript_profile")}
        layout["gate"] = config["gate"]
        candidate["layouts"][config["gate"]] = layout
        rows = attempts(report(workers=config["workers"]))
        for seed, row in enumerate(rows):
            row["seed"] = seed
            row["report"].update(layout)
            row["report"]["seed"] = seed
        configs[name] = rows
    return {"candidate": candidate, "attempts": configs}


def test_summary_does_not_replace_failing_real_chips_with_passing_synthetic(tmp_path):
    ledger = ledger_fixture()
    for row in ledger["attempts"]["q_chips-1"]:
        row["report"]["samples"][0]["cpu_ns"] = 31_000_000_000
    path = tmp_path / "runs.json"
    qualify.write_json(path, ledger)
    result = qualify.summarize(path)
    assert result["status"] == "fail"
    assert result["configurations"]["q_exact-1"]["status"] == "pass"
    assert result["configurations"]["q_chips-1"]["status"] == "fail"


def test_summary_revalidates_retained_reports_and_process_identity(tmp_path):
    path = tmp_path / "runs.json"
    qualify.write_json(path, ledger_fixture())
    assert qualify.summarize(path)["status"] == "pass"
    for field, bad in (("workers", 4), ("seed", 100), ("samples", None),
                       ("peak_rss_bytes", 0), ("descriptor_digest", "other")):
        ledger = ledger_fixture()
        ledger["attempts"]["q_chips-1"][0]["report"][field] = bad
        qualify.write_json(path, ledger)
        assert qualify.summarize(path)["status"] == "inconclusive", field
    ledger = ledger_fixture()
    ledger["attempts"]["q_chips-1"][1] = deepcopy(ledger["attempts"]["q_chips-1"][0])
    qualify.write_json(path, ledger)
    assert qualify.summarize(path)["status"] == "inconclusive"
    ledger = ledger_fixture()
    ledger["attempts"]["q_chips-1"][0]["block"] = 3
    qualify.write_json(path, ledger)
    assert qualify.summarize(path)["status"] == "inconclusive"


def test_hash_stream_and_external_output(tmp_path):
    path = tmp_path / "bytes"
    path.write_bytes(b"abc")
    assert qualify.file_hash(path) == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    assert qualify.ignored_output(tmp_path / "outputs").is_dir()
    with pytest.raises(ValueError):
        qualify.ignored_output(qualify.ROOT / "specs" / "qualification-output")


def test_incomplete_configuration_sets_cannot_pass(tmp_path):
    path = tmp_path / "runs.json"
    for rows in ({}, {"q_exact-1": attempts()}):
        qualify.write_json(path, {"candidate": {}, "attempts": rows})
        result = qualify.summarize(path)
        assert result["status"] == "inconclusive"
        assert set(result["configurations"]) == set(qualify.CONFIGS)
    qualify.write_json(path, {"candidate": {}, "attempts": {"unknown": []}})
    with pytest.raises(ValueError):
        qualify.summarize(path)


def test_layout_inventory_binds_all_actual_descriptors():
    records = []
    for gate in sorted({config["gate"] for config in qualify.CONFIGS.values()}):
        item = {key: report()[key] for key in
                ("k", "shape", "descriptor_digest", "descriptor_hash", "transcript_profile")}
        records.append("M3_LAYOUT_JSON " + json.dumps({**item, "gate": gate}))
    text = "\n".join(records)
    assert len(qualify.parse_layouts(text)) == 4
    for invalid in ("", "\n".join(records[1:]), text + "\n" + records[0],
                    text.replace('"k": 16', '"k": 15'),
                    text.replace("blake2b256-pipa-v2-circdesc", "sha256")):
        with pytest.raises(ValueError):
            qualify.parse_layouts(invalid)
