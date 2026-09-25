"""Synthetic test fixtures for the separate signed structural testnet evidence mode.

These fixtures exercise validation only; their bytes are not release artifacts.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


FIXTURE_PATH = Path(__file__).with_name("kagemusha_v1_release_evidence_test.py")
SPEC = importlib.util.spec_from_file_location("kagemusha_release_fixture_for_testnet", FIXTURE_PATH)
assert SPEC is not None and SPEC.loader is not None
fixture_module = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = fixture_module
SPEC.loader.exec_module(fixture_module)
VERIFIER = fixture_module.VERIFIER


def _testnet_fixture(root: Path):
    fixture = fixture_module._fixture(root)
    manifest = fixture.manifest
    profile = manifest["profiles"][0]
    hardware = profile["hardware_profile"]
    artifacts = [
        {
            "role": row["role"],
            "sha256": fixture_module._sha256(fixture.path(row["path"]).read_bytes()),
            "byte_len": fixture.path(row["path"]).stat().st_size,
        }
        for row in manifest["artifacts"]
    ]
    artifact_set_digest = VERIFIER.rust_artifact_set_digest(artifacts)
    vk_digest = VERIFIER.rust_vk_set_digest(artifacts, manifest["protocols"])
    qualification_path = profile["qualification_report"]
    old_qualification = json.loads(fixture.path(qualification_path).read_text())
    qualification = {
        "schema": VERIFIER.TESTNET_PROFILE_REPORT_SCHEMA,
        "schema_version": 1,
        "verification_id": old_qualification["verification_id"],
        "provider_id": hardware["provider_id"],
        "policy_epoch": hardware["policy_epoch"],
        "vk_digest": vk_digest,
        "artifact_set_digest": artifact_set_digest,
        "testnet_only": True,
    }
    fixture.write(qualification_path, VERIFIER.canonical_json_bytes(qualification), "report")
    hardware["qualification_report_digest"] = fixture_module._sha256(
        fixture.path(qualification_path).read_bytes()
    )
    hardware["hardware_profile_id"] = VERIFIER.rust_hardware_profile_id(hardware)
    profile_id = hardware["hardware_profile_id"]
    policy = profile["provider_policy"]
    policy["hardware_profile_id"] = profile_id
    policy["issuer_signature"] = fixture_module._provider_policy_signature(
        profile_id, policy["provider_profile_index"], policy["provider_authority_commitment"]
    )

    retained_reports = {manifest["global_reports"]["circuit_shape"], qualification_path}
    retained_reports.update(row["report"] for row in profile["relations"])
    retained_reports.update(row["report"] for row in profile["helpers"])
    for path in retained_reports - {qualification_path, manifest["global_reports"]["circuit_shape"]}:
        report = json.loads(fixture.path(path).read_text())
        report["hardware_profile_id"] = profile_id
        fixture.write(path, VERIFIER.canonical_json_bytes(report), "report")
    retained_ids = {
        json.loads(fixture.path(path).read_text())["verification_id"]
        for path in retained_reports
    }
    fixture.commands[:] = [
        command for command in fixture.commands if command["id"] in retained_ids
    ]
    qualification_command = next(
        command for command in fixture.commands
        if command["id"] == qualification["verification_id"]
    )
    qualification_command["report_schema"] = VERIFIER.TESTNET_PROFILE_REPORT_SCHEMA
    qualification_command["arguments"] = [{"file": qualification_path}]
    observation_path = qualification_command["observation"]
    observation = json.loads(fixture.path(observation_path).read_text())
    observation["subject"]["report_schema"] = VERIFIER.TESTNET_PROFILE_REPORT_SCHEMA
    fixture.write(observation_path, VERIFIER.canonical_json_bytes(observation), "observation")

    retained_files = set(manifest["source"].values())
    retained_files.update(row["path"] for row in manifest["artifacts"])
    for command in fixture.commands:
        retained_files.update((command["stdout"], command["stderr"], command["observation"]))
        retained_files.update(argument["file"] for argument in command["arguments"] if "file" in argument)
    for relative in set(fixture.kinds) - retained_files:
        fixture.path(relative).unlink()
        del fixture.kinds[relative]
    for directory in sorted(
        (path for path in fixture.root.rglob("*") if path.is_dir()),
        key=lambda path: len(path.parts), reverse=True,
    ):
        if not any(directory.iterdir()):
            directory.rmdir()

    manifest["schema"] = VERIFIER.TESTNET_MANIFEST_SCHEMA
    manifest["global_reports"] = {"circuit_shape": manifest["global_reports"]["circuit_shape"]}
    manifest["reproducible_builds"] = []
    manifest["profiles"] = [{key: profile[key] for key in (
        "hardware_profile", "provider_policy", "suite_id", "qualification_report",
        "relations", "helpers",
    )}]
    fixture.refresh_files()
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    return fixture


def _verify(fixture, *, testnet_experiment: bool):
    return VERIFIER.verify_evidence(
        manifest_path=fixture.manifest_path,
        expected_manifest_sha256=fixture_module._sha256(fixture.manifest_path.read_bytes()),
        evidence_root=fixture.root,
        observer_policy_path=fixture.observer_policy_path,
        expected_observer_policy_sha256=fixture.observer_policy_sha256,
        testnet_experiment=testnet_experiment,
    )


def test_testnet_structural_projection_is_distinct_and_closes_signed_reports(tmp_path: Path) -> None:
    fixture = _testnet_fixture(tmp_path)
    projection = _verify(fixture, testnet_experiment=True)
    assert projection["schema"] == VERIFIER.TESTNET_PROJECTION_SCHEMA
    receipt = projection["receipt_projection"]
    assert receipt["fuzz_cases"] == 0
    assert receipt["security_review_report"] == {"sha256": "0" * 64, "byte_len": 0}
    assert receipt["profile_qualifications"][0]["recursive_depths"] == []
    assert len(receipt["profile_qualifications"][0]["relations"]) == len(VERIFIER.RELATIONS)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="schema is unsupported"):
        _verify(fixture, testnet_experiment=False)


def test_production_manifest_and_changed_testnet_report_cannot_cross_modes(tmp_path: Path) -> None:
    production = fixture_module._fixture(tmp_path / "production")
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="schema is unsupported"):
        _verify(production, testnet_experiment=True)

    fixture = _testnet_fixture(tmp_path / "testnet")
    report_path = fixture.manifest["profiles"][0]["relations"][0]["report"]
    report = json.loads(fixture.path(report_path).read_text())
    report["eq_protocol_digest"] = "ab" * 32
    fixture.write(report_path, VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_commands_for_file(report_path)
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="protocol or verifying-key binding"):
        _verify(fixture, testnet_experiment=True)


def test_testnet_source_change_requires_fresh_threshold_observations(tmp_path: Path) -> None:
    fixture = _testnet_fixture(tmp_path)
    source = fixture.manifest["source"]["source_archive"]
    fixture.write(source, b"different source tree\n", "source_archive")
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="substitutes its trusted subject"):
        _verify(fixture, testnet_experiment=True)
