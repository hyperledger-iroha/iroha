"""Hostile tests for the authenticated Linux native-evidence authority."""

from __future__ import annotations

import argparse
import hashlib
import inspect
import sys
import tarfile
from pathlib import Path
from types import SimpleNamespace

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import build_taira_rollout_candidate as candidate
import capture_taira_privacy_protocol_four_peer_receipt as capture
import close_taira_qualification_handoff as closer
import extract_authenticated_taira_privacy_release as extractor
import finalize_taira_rollout_authority as finalizer
import release_artifact_contract as contract
import taira_release_authority as native_authority
import taira_rollout_admission as admission


COMMIT = "11" * 20
DPN_COMMIT = "22" * 20
SOURCE_SHA256 = "33" * 32
FINGERPRINT = "44" * 32
VERIFIER_SHA256 = "55" * 32
SOURCE = admission.SourceIdentity(
    COMMIT,
    DPN_COMMIT,
    hashlib.sha256(b"attacker Cargo.lock\n").hexdigest(),
    SOURCE_SHA256,
)

HOSTILE_NATIVE_CASES = (
    "fabricated-runner",
    "fabricated-receipt",
    "fabricated-stage-artifacts",
    "fabricated-expectations",
    "fabricated-x509-resource",
    "recomputed-archive-hashes",
    "release-and-candidate-signer-reuse",
    "source-splice",
    "legacy-native-archive",
)


def _forged_native_release(
    tmp_path: Path,
    hostile_case: str,
) -> tuple[Path, Path, argparse.Namespace]:
    """Build evidence whose hashes close but whose native claims are attacker-made."""

    commit = "66" * 20 if hostile_case == "source-splice" else COMMIT
    source_sha256 = "77" * 32 if hostile_case == "source-splice" else SOURCE_SHA256
    root = tmp_path / f"evidence-{hostile_case}"
    cargo_payload = b"attacker Cargo.lock\n"
    json_payloads = {
        "command_manifest_json": {
            "commands": ["attacker-owned synthetic success"],
            "schema": "attacker.command-manifest",
        },
        "expectations_json": {
            "all_protocols_ready": True,
            "schema": "attacker.expectations",
        },
        "receipt_json": {
            "passed": True,
            "schema": (
                "iroha.taira.legacy-native-receipt"
                if hostile_case == "legacy-native-archive"
                else "attacker.native-receipt"
            ),
            "schema_version": 0,
        },
        "stage_artifacts_json": {
            "all_stages_passed": True,
            "schema": "attacker.stage-artifacts",
        },
        "x509_resource_json": {
            "resource_ready": True,
            "schema": "attacker.zk-x509-resource",
        },
    }
    for index, (name, relative) in enumerate(
        native_authority.EVIDENCE_PATHS.items(), start=1
    ):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if name == "exact12_matrix":
            payload = (ROOT / "fixtures/privacy/exact12_v1.tsv").read_bytes()
        elif name == "workspace_source_manifest":
            payload = f"{source_sha256}\n".encode("ascii")
        elif name == "cargo_lock":
            payload = cargo_payload
        elif name in json_payloads:
            payload = contract.canonical_json_bytes(json_payloads[name])
        elif name == "runner_binary":
            payload = b"#!/bin/sh\nprintf 'synthetic native success\\n'\n"
        elif name == "validator_binary":
            payload = b"attacker validator binary\n"
        elif name.endswith("_norito"):
            payload = f"attacker-norito-{name}-{hostile_case}\n".encode("ascii")
        else:
            payload = f"attacker-evidence-{index}-{hostile_case}\n".encode("ascii")
        path.write_bytes(payload)

    provenance = {
        "dpn_validator_release_commit": DPN_COMMIT,
        "iroha_git_head": commit,
        "iroha_source_attested": True,
        "iroha_source_bundle_provenance_sha256": "88" * 32,
        "iroha_source_tree_sha256": "99" * 32,
        "iroha_tracked_patch_sha256": "aa" * 32,
        "iroha_worktree_clean": False,
        "schema_version": 1,
        "validator_lock_sha256": hashlib.sha256(cargo_payload).hexdigest(),
        "workspace_source_manifest_sha256": source_sha256,
    }
    (root / native_authority.EVIDENCE_PATHS["dpn_validator_build_provenance"]).write_bytes(
        contract.canonical_json_bytes(provenance)
    )

    archive = tmp_path / f"taira-rollout-{hostile_case}-release.tar.gz"
    prefix = archive.name.removesuffix(".tar.gz")
    with tarfile.open(archive, mode="w:gz") as stream:
        for relative in native_authority.EVIDENCE_PATHS.values():
            stream.add(root / relative, arcname=f"{prefix}/{relative}", recursive=False)

    args = argparse.Namespace(
        archive=str(archive),
        commit=commit,
        dpn_validator_release_commit=DPN_COMMIT,
        evidence_root=str(root),
        image_id=None,
        image_manifest_digest=None,
        image_tag=[],
        native_verifier_sha256=VERIFIER_SHA256,
        signing_fingerprint=FINGERPRINT,
    )
    return root, archive, args


@pytest.mark.parametrize("hostile_case", HOSTILE_NATIVE_CASES)
def test_fabricated_native_evidence_can_close_self_hashes_but_not_gain_authority(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    hostile_case: str,
) -> None:
    root, archive, args = _forged_native_release(tmp_path, hostile_case)

    # This is the bug boundary: the retained helper proves only that attacker
    # bytes and recomputed archive hashes agree.  It does not prove semantics.
    structural = native_authority._build_untrusted_authority_structure(args)
    assert structural["subject"]["sha256"] == hashlib.sha256(
        archive.read_bytes()
    ).hexdigest()
    evidence = {row["name"]: row for row in structural["native_release_evidence"]}
    for name in (
        "runner_binary",
        "receipt_json",
        "receipt_norito",
        "stage_artifacts_json",
        "stage_artifacts_norito",
        "expectations_json",
        "expectations_norito",
        "x509_resource_json",
        "x509_resource_norito",
    ):
        payload = (root / native_authority.EVIDENCE_PATHS[name]).read_bytes()
        assert evidence[name]["sha256"] == hashlib.sha256(payload).hexdigest()

    monkeypatch.setattr(
        native_authority.taira_authority_client,
        "preflight",
        lambda role: {"role": role, "status": "ready"},
    )

    def reject_forgery(*_args, **_kwargs):
        raise native_authority.taira_authority_client.TairaAuthorityClientError(
            "native semantic validation rejected fabricated evidence"
        )

    monkeypatch.setattr(
        native_authority.taira_authority_client, "authorize", reject_forgery
    )
    with pytest.raises(
        native_authority.TairaReleaseAuthorityError,
        match="semantic validation rejected",
    ):
        native_authority.build_authority(args)


def test_provisioning_contract_uses_only_the_fixed_native_client(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    contract_text = (
        native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_PROVISIONING_ERROR
    )
    for required in (
        native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA,
        native_authority.INDEPENDENT_NATIVE_EVIDENCE_REPLAY_NAMESPACE,
        "separately pinned trust root",
        "source-built runner",
        "candidate signer",
        "release signer",
        "runner and validator bytes",
        "command manifest",
        "receipt",
        "stage-artifact",
        "expectation",
        "ZK-X509 resource",
        "Exact12 operation and outcome table",
        "Cargo.lock",
        "workspace-source",
        "native Linux host and installation identity",
        "installed controller digest",
        "run nonce, issued time, expiry, and replay",
        "JSON/Norito correspondence",
        "recomputed archive hashes",
        "legacy unsigned evidence",
    ):
        assert required in contract_text

    monkeypatch.setenv("TAIRA_INDEPENDENT_NATIVE_EVIDENCE_READY", "1")
    (tmp_path / "independent-native-evidence-ready").write_bytes(b"passed\n")
    assert not inspect.signature(
        native_authority.require_independent_native_evidence_authority_provisioned
    ).parameters
    barrier_source = inspect.getsource(
        native_authority.require_independent_native_evidence_authority_provisioned
    )
    assert "os.environ" not in barrier_source
    assert "getenv" not in barrier_source
    calls: list[str] = []
    monkeypatch.setattr(
        native_authority.taira_authority_client,
        "preflight",
        lambda role: calls.append(role) or {"role": role, "status": "ready"},
    )
    native_authority.require_independent_native_evidence_authority_provisioned()
    assert calls == ["native-evidence"]

    def unavailable(_role: str):
        raise native_authority.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(
        native_authority.taira_authority_client, "preflight", unavailable
    )
    with pytest.raises(
        native_authority.TairaReleaseAuthorityError,
        match=native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA,
    ):
        native_authority.require_independent_native_evidence_authority_provisioned()


def test_authority_cli_and_finalizer_stop_before_output_path_or_signer(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _root, archive, authority_args = _forged_native_release(
        tmp_path, "release-and-candidate-signer-reuse"
    )
    output = tmp_path / "authority-output"
    shared_signer = tmp_path / "reused-release-and-candidate-signer"
    shared_signer.write_bytes(b"must remain untouched\n")
    signer_before = shared_signer.stat()
    forbidden_calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            forbidden_calls.append(name)
            raise AssertionError(f"native authority barrier reached {name}")

        return call

    def unavailable(_role: str):
        raise native_authority.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(
        native_authority.taira_authority_client, "preflight", unavailable
    )

    monkeypatch.setattr(
        native_authority,
        "_build_untrusted_authority_structure",
        forbidden("structural authority builder"),
    )
    with pytest.raises(native_authority.TairaReleaseAuthorityError):
        native_authority.build_authority(authority_args)

    cli_output = tmp_path / "legacy-authority.json"
    assert native_authority.main(
        [
            "create",
            "--evidence-root",
            authority_args.evidence_root,
            "--commit",
            authority_args.commit,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--signing-fingerprint",
            FINGERPRINT,
            "--native-verifier-sha256",
            VERIFIER_SHA256,
            "--archive",
            str(archive),
            "--output",
            str(cli_output),
        ]
    ) == 1

    for name in (
        "platform",
        "path inspection",
        "controller inspection",
        "public-input read",
        "output creation",
        "signer invocation",
    ):
        target = {
            "platform": (finalizer.platform, "system"),
            "path inspection": (finalizer, "_canonical_absolute"),
            "controller inspection": (finalizer, "verify_controller_closure"),
            "public-input read": (finalizer, "_verify_public_privacy_inputs"),
            "output creation": (finalizer, "create_fresh_directory"),
            "signer invocation": (finalizer, "sign_release_manifest"),
        }[name]
        monkeypatch.setattr(target[0], target[1], forbidden(name))
    with pytest.raises(
        finalizer.FinalizationError,
        match=native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA,
    ):
        finalizer.finalize(
            SimpleNamespace(
                archive=str(archive),
                external_signer=str(shared_signer),
                output_dir=str(output),
            )
        )

    assert forbidden_calls == []
    assert not cli_output.exists()
    assert not output.exists()
    assert shared_signer.read_bytes() == b"must remain untouched\n"
    signer_after = shared_signer.stat()
    assert (signer_after.st_ino, signer_after.st_size, signer_after.st_mtime_ns) == (
        signer_before.st_ino,
        signer_before.st_size,
        signer_before.st_mtime_ns,
    )


@pytest.mark.parametrize("hostile_case", HOSTILE_NATIVE_CASES)
def test_candidate_and_admission_stop_before_signer_output_archive_or_replay(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    hostile_case: str,
) -> None:
    _root, archive, _args = _forged_native_release(tmp_path, hostile_case)
    output = tmp_path / f"candidate-output-{hostile_case}"
    replay = tmp_path / "replay-ledger.json"
    replay.write_bytes(b"unchanged replay state\n")
    replay_before = replay.stat()
    shared_signer = tmp_path / "shared-signer"
    shared_signer.write_bytes(b"not invoked\n")
    signer_before = shared_signer.stat()
    forbidden_calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            forbidden_calls.append(name)
            raise AssertionError(f"native authority barrier reached {name}")

        return call

    def unavailable(_role: str):
        raise native_authority.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(
        native_authority.taira_authority_client, "preflight", unavailable
    )

    # Reach the independent-native barrier behind the separately provisioned
    # macOS controller-origin barrier.
    monkeypatch.setattr(
        candidate.privacy_evidence,
        "require_controller_origin_authority_provisioned",
        lambda: None,
    )
    for owner, attribute, name in (
        (candidate, "_canonical_path", "candidate path inspection"),
        (candidate, "create_fresh_directory", "candidate output creation"),
        (candidate, "sign_release_manifest", "candidate signer"),
        (admission, "stable_hash_path", "admission path read"),
        (admission, "load_replay_ledger", "admission replay read"),
        (admission, "_extract_final_archive", "admission archive extraction"),
        (admission, "_verify_final_authority", "admission signature verification"),
    ):
        monkeypatch.setattr(owner, attribute, forbidden(name))

    with pytest.raises(
        candidate.TairaCandidateBuildError,
        match=native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA,
    ):
        candidate.assemble_candidate(
            SimpleNamespace(
                external_signer=shared_signer,
                linux_archive=archive,
                output_directory=output,
            )
        )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match=native_authority.INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA,
    ):
        admission.verify_admission(
            archive_path=archive,
            authority_dir=tmp_path / "candidate-authority",
            expected_source=SOURCE,
            expected_receipt_id="aa" * 32,
            replay_ledger_path=replay,
            trusted_signing_fingerprint=FINGERPRINT,
            release_manifest_verifier_path=tmp_path / "verifier",
            trusted_release_manifest_verifier_sha256=VERIFIER_SHA256,
            now_unix=1_900_000_000,
        )

    assert forbidden_calls == []
    assert not output.exists()
    assert replay.read_bytes() == b"unchanged replay state\n"
    replay_after = replay.stat()
    assert (replay_after.st_ino, replay_after.st_size, replay_after.st_mtime_ns) == (
        replay_before.st_ino,
        replay_before.st_size,
        replay_before.st_mtime_ns,
    )
    assert shared_signer.read_bytes() == b"not invoked\n"
    signer_after = shared_signer.stat()
    assert (signer_after.st_ino, signer_after.st_size, signer_after.st_mtime_ns) == (
        signer_before.st_ino,
        signer_before.st_size,
        signer_before.st_mtime_ns,
    )


def test_all_direct_linux_trust_routes_stop_before_lower_level_io(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    forbidden_calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            forbidden_calls.append(name)
            raise AssertionError(f"native authority barrier reached {name}")

        return call

    def unavailable(_role: str):
        raise native_authority.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(
        native_authority.taira_authority_client, "preflight", unavailable
    )

    monkeypatch.setattr(admission, "scan_inventory_paths", forbidden("authority scan"))
    monkeypatch.setattr(extractor, "_canonical_path", forbidden("extractor path"))
    monkeypatch.setattr(capture, "_canonical_file", forbidden("capture path"))
    monkeypatch.setattr(closer, "_canonical_payload", forbidden("handoff path"))

    with pytest.raises(admission.TairaRolloutAdmissionError):
        admission._verify_closed_linux_authority(
            tmp_path,
            expected_source=SOURCE,
            expected_manifest_sha256="aa" * 32,
            expected_native_verifier_sha256=VERIFIER_SHA256,
            trusted_signing_fingerprint=FINGERPRINT,
            release_manifest_verifier_path=tmp_path / "verifier",
            trusted_release_manifest_verifier_sha256=VERIFIER_SHA256,
            linux_archive_path=tmp_path / "forged.tar.gz",
        )
    with pytest.raises(admission.TairaRolloutAdmissionError):
        admission._verify_existing_linux_authority(
            tmp_path,
            linux_archive_path=tmp_path / "forged.tar.gz",
            expected_source=SOURCE,
            trusted_signing_fingerprint=FINGERPRINT,
            native_verifier_sha256=VERIFIER_SHA256,
        )
    with pytest.raises(extractor.PrivacyReleaseExtractionError):
        extractor.authenticate_linux_release(
            tmp_path / "forged.tar.gz",
            tmp_path / "authority",
            source=SOURCE,
            trusted_signing_fingerprint=FINGERPRINT,
            verifier=tmp_path / "verifier",
            verifier_sha256=VERIFIER_SHA256,
            staging_parent=tmp_path,
        )
    extraction_output = tmp_path / "extracted"
    with pytest.raises(extractor.PrivacyReleaseExtractionError):
        extractor.run(SimpleNamespace(output_dir=extraction_output))
    capture_output = tmp_path / "protocol-evidence"
    with pytest.raises(capture.PrivacyProtocolReceiptError):
        capture.capture(SimpleNamespace(output_directory=capture_output))
    handoff_output = tmp_path / "qualification-handoff"
    with pytest.raises(closer.QualificationHandoffError):
        closer.close_handoff(
            tmp_path / "receipt.json",
            tmp_path / "protocol-evidence",
            tmp_path / "source.json",
            handoff_output,
        )

    assert forbidden_calls == []
    assert not (tmp_path / "linux-evidence").exists()
    assert not extraction_output.exists()
    assert not capture_output.exists()
    assert not handoff_output.exists()


def test_untrusted_structural_builder_has_no_unbarriered_production_caller() -> None:
    production_uses = []
    for path in sorted(SCRIPTS.glob("*.py")):
        source = path.read_text(encoding="utf-8")
        if "_build_untrusted_authority_structure" in source:
            production_uses.append(path.relative_to(ROOT).as_posix())
    assert production_uses == ["scripts/taira_release_authority.py"]
    wrapper = inspect.getsource(native_authority.build_authority)
    preflight = wrapper.index(
        "require_independent_native_evidence_authority_provisioned()"
    )
    subject = wrapper.index("subject = _build_untrusted_authority_structure(args)")
    authorization = wrapper.index("taira_authority_client.authorize(")
    assert preflight < subject < authorization
