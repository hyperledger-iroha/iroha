"""Reject physical provenance substitution despite signed generic release observations.

All reports, keys and transcripts here are synthetic test data, never actual OEM
attestation or evidence that a production hardware profile has been qualified.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

from kagemusha_v1_release_evidence_test import (
    PHYSICAL_TEST,
    VERIFIER,
    _fixture,
    _verify_direct,
)


def _rewrite(fixture, path: str, value: object) -> None:
    fixture.write(path, VERIFIER.canonical_json_bytes(value), fixture.kinds[path])
    fixture.resign_commands_for_file(path)
    fixture.refresh_files()


@pytest.mark.parametrize(
    "field",
    [
        "hardware_profile_id", "provider_id", "hardware_policy_id", "device_id",
        "product_id", "firmware_digest", "os_build_digest", "product_class_digest",
        "firmware_policy_digest", "candidate_context_digest", "artifact_set_digest",
        "attestation_verifier_sha256", "run_id", "challenge_sha256",
    ],
)
def test_generic_signed_oem_report_cannot_substitute_subject(tmp_path: Path, field: str) -> None:
    fixture = _fixture(tmp_path)
    paths = fixture.manifest["profiles"][0]["physical_evidence"]
    report = json.loads(fixture.path(paths["oem_report"]).read_text())
    report[field] = "fe" * 32
    _rewrite(fixture, paths["oem_report"], report)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="OEM attestation report substitutes"):
        _verify_direct(fixture)


@pytest.mark.parametrize("field", ["attestation", "trust_roots", "transcript", "observer_policy"])
def test_generic_signed_oem_report_cannot_substitute_raw_bytes(tmp_path: Path, field: str) -> None:
    fixture = _fixture(tmp_path)
    paths = fixture.manifest["profiles"][0]["physical_evidence"]
    report = json.loads(fixture.path(paths["oem_report"]).read_text())
    report[field]["sha256"] = "fe" * 32
    _rewrite(fixture, paths["oem_report"], report)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="OEM attestation report substitutes"):
        _verify_direct(fixture)


@pytest.mark.parametrize(
    ("field", "value"),
    [("passed", 1), ("hardware_backed", 1), ("software_fallback", 0),
     ("production_build", 1), ("capability_mask", 1), ("policy_epoch", True),
     ("started_at_ms", 1), ("ended_at_ms", 2)],
)
def test_generic_signed_oem_report_requires_exact_types_and_measurements(
    tmp_path: Path, field: str, value: object
) -> None:
    fixture = _fixture(tmp_path)
    paths = fixture.manifest["profiles"][0]["physical_evidence"]
    report = json.loads(fixture.path(paths["oem_report"]).read_text())
    report[field] = value
    _rewrite(fixture, paths["oem_report"], report)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="OEM attestation report substitutes"):
        _verify_direct(fixture)


@pytest.mark.parametrize(
    ("field", "error"),
    [("attestation", "retained raw OEM attestation"),
     ("trust_roots", "governed hardware profile"),
     ("observer_policy", "independently pinned observer policy")],
)
def test_raw_physical_evidence_cannot_be_replaced_even_with_updated_observations(
    tmp_path: Path, field: str, error: str
) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.manifest["profiles"][0]["physical_evidence"][field]
    fixture.write(path, b"different raw evidence", fixture.kinds[path])
    fixture.resign_commands_for_file(path)
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match=error):
        _verify_direct(fixture)


def test_signed_release_observation_cannot_replace_transcript_approvals(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.manifest["profiles"][0]["physical_evidence"]["transcript"]
    document = json.loads(fixture.path(path).read_text())
    document["approvals"] = []
    _rewrite(fixture, path, document)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="observer approvals do not meet"):
        _verify_direct(fixture)


def test_release_rechecks_signed_transcript_semantics(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.manifest["profiles"][0]["physical_evidence"]["transcript"]
    document = json.loads(fixture.path(path).read_text())
    document["events"][0]["data"]["counter"] += 1
    # The observer approves the malicious body; the release projector must still
    # reject its broken event chain instead of trusting the positive report.
    policy = VERIFIER._load_observer_policy(fixture.observer_policy_path, fixture.observer_policy_sha256)
    builder = PHYSICAL_TEST._TranscriptBuilder(policy, {fixture.observer_authority_id: fixture.observer_seed})
    builder.approve(document)
    _rewrite(fixture, path, document)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="event_hash"):
        _verify_direct(fixture)


def test_signed_release_observations_cannot_replay_physical_candidate(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.write("source/candidate.tar", b"another immutable candidate source", "source_archive")
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    # Exercise the physical verifier directly to isolate the physical candidate
    # binding from the global source/security report's independent binding.
    captured = {}
    original = VERIFIER.EvidenceVerifier._verify_global_reports
    def remember(self, *args, **kwargs):
        captured["verifier"] = self
        return original(self, *args, **kwargs)
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(VERIFIER.EvidenceVerifier, "_verify_global_reports", remember)
        with pytest.raises(VERIFIER.KagemushaEvidenceError):
            _verify_direct(fixture)
    verifier = captured["verifier"]
    profile = fixture.manifest["profiles"][0]
    report = json.loads(fixture.path(profile["qualification_report"]).read_text())
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="release candidate or artifact set"):
        verifier._verify_physical_evidence(profile["physical_evidence"], profile["hardware_profile"],
                                           profile["qualification_report"], report)


def test_manifest_must_retain_the_physical_provenance_sidecar(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    del fixture.manifest["profiles"][0]["physical_evidence"]
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="physical_evidence"):
        _verify_direct(fixture)


def test_oem_challenge_changes_for_each_freshness_or_device_binding() -> None:
    endpoint = {key: key for key in ("device_id", "firmware_digest", "os_build_digest")}
    run = {key: key for key in ("candidate_context_digest", "artifact_set_digest", "run_id")}
    challenge = VERIFIER.physical_oem_challenge("profile", endpoint, run)
    assert challenge == VERIFIER.physical_oem_challenge("profile", dict(endpoint), dict(run))
    assert challenge != VERIFIER.physical_oem_challenge("other-profile", endpoint, run)
    for field in endpoint:
        assert challenge != VERIFIER.physical_oem_challenge("profile", {**endpoint, field: "other"}, run)
    for field in run:
        assert challenge != VERIFIER.physical_oem_challenge("profile", endpoint, {**run, field: "other"})


def test_candidate_selected_python_is_rejected_before_loading(tmp_path: Path, monkeypatch) -> None:
    fixture = _fixture(tmp_path / "fixture")
    marker = tmp_path / "executed"
    candidate = tmp_path / "candidate-verifier.py"
    candidate.write_text(f"from pathlib import Path\nPath({str(marker)!r}).write_text('executed')\n")
    candidate.chmod(0o600)
    monkeypatch.setattr(VERIFIER, "PHYSICAL_VERIFIER_PATH", candidate)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="exact bundled physical-device verifier"):
        _verify_direct(fixture)
    assert not marker.exists()


def test_hardened_projector_reverifies_full_physical_closure(tmp_path: Path) -> None:
    from run_kagemusha_v1_release_evidence import PROJECTOR_BOOTSTRAP
    from kagemusha_v1_release_evidence_test import SCRIPTS, VERIFIER_PATH

    fixture = _fixture(tmp_path)
    python_path = Path(sys.executable).resolve()
    contract_path = SCRIPTS / "release_artifact_contract.py"
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    result = subprocess.run(
        [
            str(python_path), "-I", "-B", "-S", "-c", PROJECTOR_BOOTSTRAP,
            str(SCRIPTS), str(python_path), digest(python_path),
            str(VERIFIER_PATH), digest(VERIFIER_PATH), str(contract_path), digest(contract_path),
            "--manifest", str(fixture.manifest_path), "--manifest-sha256", digest(fixture.manifest_path),
            "--evidence-root", str(fixture.root), "--observer-policy", str(fixture.observer_policy_path),
            "--observer-policy-sha256", fixture.observer_policy_sha256,
        ], capture_output=True, text=True, timeout=60, check=False,
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)["schema"] == VERIFIER.PROJECTION_SCHEMA


@pytest.mark.parametrize("same_bytes", [False, True])
def test_physical_source_replacement_before_final_projection_is_rejected(
    tmp_path: Path, monkeypatch, same_bytes: bool
) -> None:
    fixture = _fixture(tmp_path / "fixture")
    source = VERIFIER.PHYSICAL_VERIFIER_PATH.read_bytes()
    trusted_path = tmp_path / "trusted-physical.py"
    trusted_path.write_bytes(source)
    trusted_path.chmod(0o600)
    monkeypatch.setattr(VERIFIER, "PHYSICAL_VERIFIER_PATH", trusted_path)
    original = VERIFIER.EvidenceVerifier._revalidate_closure
    reached_final_boundary = []

    def replace_before_publication(self):
        assert self.physical_verifier_info is not None
        reached_final_boundary.append(True)
        replacement = tmp_path / "replacement.py"
        replacement.write_bytes(source if same_bytes else b"raise RuntimeError('substituted code')\n")
        replacement.chmod(0o600)
        replacement.replace(trusted_path)
        original(self)

    monkeypatch.setattr(VERIFIER.EvidenceVerifier, "_revalidate_closure", replace_before_publication)
    error = "changed during release verification" if same_bytes else "exact bundled physical-device verifier"
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match=error):
        _verify_direct(fixture)
    assert reached_final_boundary == [True]
