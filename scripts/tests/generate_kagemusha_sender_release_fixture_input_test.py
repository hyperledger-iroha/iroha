"""Source-only bootstrap checks for the ignored native sender fixture exporter."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/generate_kagemusha_sender_release_fixture_input.py"
GOLDEN = ROOT / "fixtures/offline/kagemusha_sender_release_parser_v1.json"


def test_fixture_input_is_derived_without_native_golden(tmp_path: Path) -> None:
    output = tmp_path / "sender-input.json"
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--output", str(output)],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    assert "source-derived Rust exporter input" in result.stdout
    payload = json.loads(output.read_text())
    assert set(payload) == {"schema", "schema_version", "contexts", "positive_attempts"}
    assert payload["schema_version"] == 1
    assert set(payload["contexts"]) == {
        "physical_standalone", "release_default", "release_provider_ab",
    }
    assert set(payload["positive_attempts"]) == set(payload["contexts"])
    for name, context in payload["contexts"].items():
        assert "app_attestation_authority_policy_digest" in context["hardware_profile"]
        assert "app_policy_binding_digest" in context["credentials"][0]
        attempts = payload["positive_attempts"][name]
        assert len(attempts) == 6
        assert all("canonical_sender_command_hex" not in row for row in attempts)
    assert b'"canonical_sender_command_hex"' not in output.read_bytes()


def test_fixture_input_generator_refuses_native_golden_output() -> None:
    before = GOLDEN.read_bytes()
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--output", str(GOLDEN)],
        cwd=ROOT, capture_output=True, text=True,
    )
    assert result.returncode == 2
    assert "cannot replace the reviewed native-byte golden" in result.stderr
    assert GOLDEN.read_bytes() == before


def test_relabelled_native_candidate_cannot_match_current_source(tmp_path: Path) -> None:
    output = tmp_path / "sender-input.json"
    candidate = json.loads(GOLDEN.read_text())
    candidate["contexts"]["physical_standalone"]["context"]["hardware_profile"]["hardware_profile_id"] = "ff" * 32
    candidate_path = tmp_path / "relabelled-candidate.json"
    candidate_path.write_text(json.dumps(candidate))
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--output", str(output), "--validate-candidate", str(candidate_path)],
        cwd=ROOT, capture_output=True, text=True,
    )
    assert result.returncode != 0
    assert "AssertionError" in result.stderr
    assert output.exists()
