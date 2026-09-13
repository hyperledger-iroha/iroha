"""Shared fixture-byte and Python ingress checks; Rust execution is independent."""
import hashlib
import json
from pathlib import Path

import pytest

from scripts.fastpq import wrap_benchmark
from scripts.fastpq.report_projection import project_bundle
from scripts.fastpq.tests.generate_contract_fixtures import generate

FIXTURES = Path(__file__).resolve().parents[3] / "fixtures/fastpq/benchmark_v1"
MANIFEST = json.loads((FIXTURES / "manifest.json").read_text())


@pytest.mark.parametrize("case", MANIFEST["files"], ids=lambda case: case["path"])
def test_shared_fixture_expected_ingress(case):
    data = (FIXTURES / case["path"]).read_bytes()
    assert hashlib.sha256(data).hexdigest() == case["sha256"]
    payload = json.loads(data)
    if case["expected"] == "accept":
        report = wrap_benchmark.normalize_report(payload)
        schema = payload["producer_schema"]
        wrap_benchmark.validate_report_header(report, schema)
        wrap_benchmark.summarize_operations(report, schema)
        project_bundle(payload)
    else:
        assert case["expected"] == "reject"
        with pytest.raises((ValueError, SystemExit)):
            report = wrap_benchmark.normalize_report(payload)
            wrap_benchmark.validate_report_header(report, payload["producer_schema"])
            wrap_benchmark.summarize_operations(report, payload["producer_schema"])
            project_bundle(payload)


def test_shared_fixtures_are_reproducible_and_explicitly_synthetic(tmp_path):
    generate(tmp_path)
    assert MANIFEST["hardware_evidence"] is False
    assert len(MANIFEST["files"]) == 16
    expected = {entry.name for entry in FIXTURES.iterdir() if entry.is_file()}
    actual = {entry.name for entry in tmp_path.iterdir() if entry.is_file()}
    assert actual == expected
    for name in expected:
        assert (tmp_path / name).read_bytes() == (FIXTURES / name).read_bytes(), name
