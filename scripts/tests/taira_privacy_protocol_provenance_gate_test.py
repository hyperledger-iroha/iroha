"""Fail-closed tests for the authenticated controller-origin authority."""

from __future__ import annotations

import argparse
import inspect
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import build_taira_rollout_candidate as candidate_builder
import taira_privacy_protocol_receipt as evidence
import taira_rollout_admission as admission


ROOT = SCRIPTS.parent
SOURCE = admission.SourceIdentity(
    commit="11" * 20,
    dpn_validator_release_commit="22" * 20,
    cargo_lock_sha256="33" * 32,
    workspace_source_manifest_sha256="44" * 32,
)

HOSTILE_PROVENANCE_CASES = (
    "synthetic-libtest",
    "recomputed-self-hashes",
    "candidate-signer-reuse",
    "stale-run-nonce",
    "replayed-run",
    "source-splice",
    "controller-splice",
    "host-supervisor-splice",
    "case-operation-table-splice",
    "result-transcript-splice",
    "legacy-unsigned-v2",
)


def test_installed_controller_routes_expose_only_barriered_validation() -> None:
    production_uses = []
    for path in sorted(SCRIPTS.glob("*.py")):
        source = path.read_text(encoding="utf-8")
        if "validate_unsigned_v2_structure" in source:
            production_uses.append(path.relative_to(ROOT).as_posix())
    assert production_uses == ["scripts/taira_privacy_protocol_receipt.py"]

    receipt_source = (SCRIPTS / "taira_privacy_protocol_receipt.py").read_text(
        encoding="utf-8"
    )
    assert receipt_source.count("validate_unsigned_v2_structure(") == 2
    wrapper = inspect.getsource(evidence.validate_evidence_directory)
    preflight = wrapper.index("require_controller_origin_authority_provisioned()")
    structural = wrapper.index("result, subject, artifacts = _validated_authority_request(")
    authorization = wrapper.index("taira_authority_client.authorize(")
    assert preflight < structural < authorization

    sealer = (SCRIPTS / "seal_taira_release_controllers.py").read_text(
        encoding="utf-8"
    )
    assert (
        '"assemble-candidate": "scripts/build_taira_rollout_candidate.py"'
        in sealer
    )
    assert '"admit": "scripts/taira_rollout_admission.py"' in sealer
    assert (
        'PRIVACY_CAPTURE_HELPER = '
        '"scripts/capture_taira_privacy_protocol_four_peer_receipt.py"'
        in sealer
    )
    assert (
        'QUALIFICATION_CLOSE_HELPER = '
        '"scripts/close_taira_qualification_handoff.py"'
        in sealer
    )
    assert "validate_unsigned_v2_structure" not in sealer


def test_provisioning_contract_uses_the_fixed_role_and_has_no_caller_bypass(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    barrier = evidence.CONTROLLER_ORIGIN_AUTHORITY_PROVISIONING_BARRIER
    for required in (
        evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
        evidence.CONTROLLER_ORIGIN_AUTHENTICATED_RUN_CONTRACT,
        evidence.CONTROLLER_ORIGIN_REPLAY_NAMESPACE,
        "exact libtest output bytes and digests",
        "case/operation/outcome table",
        "canonical Cargo.lock digest",
        "workspace source-manifest digest",
        "four-peer validator/supervisor",
        "authority host/installation identities",
        "installed controller closure digest",
        "issue time, expiry",
        "separately pinned trust root",
        "candidate signer",
        "unsigned legacy v2 receipts",
    ):
        assert required in barrier

    # Neither an environment claim nor a marker in the checkout can satisfy a
    # root-provisioned signer/broker contract.  The production barrier accepts
    # no caller-controlled parameters at all.
    monkeypatch.setenv("TAIRA_CONTROLLER_ORIGIN_AUTHORITY_READY", "1")
    (tmp_path / "controller-origin-authority-ready").write_bytes(b"passed\n")
    assert not inspect.signature(
        evidence.require_controller_origin_authority_provisioned
    ).parameters
    calls: list[str] = []
    monkeypatch.setattr(
        evidence.taira_authority_client,
        "preflight",
        lambda role: calls.append(role) or {"role": role, "status": "ready"},
    )
    evidence.require_controller_origin_authority_provisioned()
    assert calls == ["privacy-protocol-origin"]

    def unavailable(_role: str):
        raise evidence.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(evidence.taira_authority_client, "preflight", unavailable)
    with pytest.raises(
        evidence.PrivacyProtocolEvidenceError,
        match=evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
    ):
        evidence.require_controller_origin_authority_provisioned()


def test_validation_preflights_then_authorizes_the_normalized_subject(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The structural subject is signed only after fixed-service authentication."""

    calls: list[tuple[str, str]] = []
    digest = "55" * 32
    structural = {
        "case_count": 7,
        "outcomes": [{"protocol": "test"}] * 12,
    }
    monkeypatch.setattr(
        evidence.taira_authority_client,
        "preflight",
        lambda role: calls.append(("preflight", role)),
    )
    monkeypatch.setattr(
        evidence,
        "validate_unsigned_v2_structure",
        lambda *_args, **_kwargs: calls.append(
            ("structural", "privacy-protocol-origin")
        )
        or structural,
    )
    monkeypatch.setattr(
        evidence.taira_authority_client,
        "authorize",
        lambda role, _subject, **_kwargs: calls.append(("authorize", role))
        or evidence.taira_authority_client.AuthorityResult(
            role=role,
            operation_id="66" * 32,
            run_id="77" * 32,
            status="authorized",
            authority_envelope={"schema": "test-envelope"},
            durable_receipt={"schema": "test-receipt"},
        ),
    )

    result = evidence.validate_evidence_directory(
        tmp_path,
        expected_source=SOURCE.as_dict(),
        expected_validator_binary_sha256=digest,
        expected_linux_release_archive_sha256=digest,
        expected_exact12_matrix_sha256=digest,
        expected_artifact_handoff_sha256=digest,
        expected_receipt_id=digest,
        now_unix=1_900_000_000,
    )
    assert isinstance(result, evidence.AuthenticatedPrivacyProtocolEvidence)
    assert result == structural
    assert result.operation_id == "66" * 32
    assert result.run_id == "77" * 32
    assert result.authority_envelope == evidence.taira_authority_client.canonical_json_bytes(
        {"schema": "test-envelope"}
    )
    assert result.durable_receipt == evidence.taira_authority_client.canonical_json_bytes(
        {"schema": "test-receipt"}
    )
    assert calls == [
        ("preflight", "privacy-protocol-origin"),
        ("structural", "privacy-protocol-origin"),
        ("authorize", "privacy-protocol-origin"),
    ]


@pytest.mark.parametrize("hostile_case", HOSTILE_PROVENANCE_CASES)
def test_receipt_candidate_and_admission_reject_untrusted_provenance_before_io(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    hostile_case: str,
) -> None:
    """Every attacker-controlled v2 variant stops at the same trust boundary."""

    forbidden_calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            forbidden_calls.append(name)
            raise AssertionError(
                f"controller-origin barrier reached forbidden operation: {name}"
            )

        return call

    def unavailable(_role: str):
        raise evidence.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(evidence.taira_authority_client, "preflight", unavailable)

    monkeypatch.setattr(
        evidence,
        "validate_unsigned_v2_structure",
        forbidden("unsigned-v2-structure"),
    )
    monkeypatch.setattr(
        candidate_builder,
        "create_fresh_directory",
        forbidden("candidate-output"),
    )
    monkeypatch.setattr(
        candidate_builder,
        "_canonical_path",
        forbidden("candidate-path-inspection"),
    )
    monkeypatch.setattr(
        candidate_builder,
        "stable_read_path",
        forbidden("candidate-path-read"),
    )
    monkeypatch.setattr(
        candidate_builder,
        "sign_release_manifest",
        forbidden("candidate-signer"),
    )
    monkeypatch.setattr(
        admission,
        "stable_hash_path",
        forbidden("admission-path-read"),
    )
    monkeypatch.setattr(
        admission,
        "load_replay_ledger",
        forbidden("admission-replay-read"),
    )
    monkeypatch.setattr(
        admission,
        "_verify_final_authority",
        forbidden("admission-signer-verification"),
    )
    monkeypatch.setattr(
        admission,
        "_extract_final_archive",
        forbidden("admission-archive-extraction"),
    )

    digest = "55" * 32
    with pytest.raises(
        evidence.PrivacyProtocolEvidenceError,
        match=evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
    ):
        evidence.validate_evidence_directory(
            tmp_path / f"hostile-{hostile_case}",
            expected_source=SOURCE.as_dict(),
            expected_validator_binary_sha256=digest,
            expected_linux_release_archive_sha256=digest,
            expected_exact12_matrix_sha256=digest,
            expected_artifact_handoff_sha256=digest,
            expected_receipt_id=digest,
            now_unix=1_900_000_000,
        )

    output = tmp_path / f"candidate-output-{hostile_case}"
    with pytest.raises(
        candidate_builder.TairaCandidateBuildError,
        match=evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
    ):
        candidate_builder.assemble_candidate(
            argparse.Namespace(
                hostile_provenance_claim=hostile_case,
                output_directory=output,
            )
        )

    replay = tmp_path / "replay-ledger.json"
    replay.write_bytes(b"unchanged attacker-visible state\n")
    replay_before = replay.stat()
    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match=evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
    ):
        admission.verify_admission(
            archive_path=tmp_path / f"hostile-{hostile_case}.tar.gz",
            authority_dir=tmp_path / "candidate-authority",
            expected_source=SOURCE,
            expected_receipt_id=digest,
            replay_ledger_path=replay,
            trusted_signing_fingerprint=digest,
            release_manifest_verifier_path=tmp_path / "verifier",
            trusted_release_manifest_verifier_sha256=digest,
            now_unix=1_900_000_000,
        )

    assert not output.exists()
    assert replay.read_bytes() == b"unchanged attacker-visible state\n"
    replay_after = replay.stat()
    assert (replay_after.st_ino, replay_after.st_size, replay_after.st_mtime_ns) == (
        replay_before.st_ino,
        replay_before.st_size,
        replay_before.st_mtime_ns,
    )
    assert forbidden_calls == []
