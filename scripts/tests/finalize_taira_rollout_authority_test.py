from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from scripts import finalize_taira_rollout_authority as finalizer
from scripts import taira_authority_client

DPN_COMMIT = "f" * 40


def _authorized(
    subject: dict[str, object], *, sidecar_generation: int = 1
) -> finalizer.AuthorizedAuthority:
    result = taira_authority_client.AuthorityResult(
        role="native-evidence",
        operation_id="a" * 64,
        run_id="b" * 64,
        status="authorized",
        authority_envelope={
            "generation": sidecar_generation,
            "schema": "test-native-evidence-envelope",
        },
        durable_receipt={
            "generation": sidecar_generation,
            "schema": "test-native-evidence-receipt",
        },
    )
    return finalizer.AuthorizedAuthority(
        subject=dict(subject),
        authority_envelope=result.authority_envelope_bytes,
        durable_receipt=result.durable_receipt_bytes,
    )


@pytest.fixture(autouse=True)
def _exercise_finalizer_checks_behind_native_authority_barrier(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        finalizer,
        "require_independent_native_evidence_authority_provisioned",
        lambda: None,
    )


def _fixture(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    checkout = tmp_path / "checkout"
    checkout.mkdir()
    controller_root = tmp_path / "sealed-controller"
    controller_scripts = controller_root / "scripts"
    controller_scripts.mkdir(parents=True)
    original_script_dir = finalizer.SCRIPT_DIR
    for name in (
        "finalize_taira_rollout_authority.py",
        "generate_release_manifest.py",
        "release_artifact_contract.py",
        "release_manifest_signing.py",
        "seal_taira_release_controllers.py",
        "taira_release_authority.py",
    ):
        (controller_scripts / name).write_bytes(
            (original_script_dir / name).read_bytes()
        )
    controller_manifest = controller_root / finalizer.CONTROLLER_MANIFEST_NAME
    controller_manifest.write_bytes(b'{"sealed":true}\n')
    controller_digest = "d" * 64
    monkeypatch.setattr(finalizer, "SCRIPT_DIR", controller_scripts)
    monkeypatch.setattr(
        finalizer,
        "verify_controller_closure",
        lambda root, digest, platform_name, commit: {
            "controller_root": str(root),
            "controller_digest": digest,
            "platform": platform_name,
            "source_commit": commit,
        },
    )
    evidence = tmp_path / "taira-rollout-test-linux-aarch64"
    scripts = evidence / "scripts"
    scripts.mkdir(parents=True)
    for name in ("release_artifact_contract.py", "taira_release_authority.py"):
        (scripts / name).write_bytes((finalizer.SCRIPT_DIR / name).read_bytes())
    public_inputs = tmp_path / "trusted-public-privacy-inputs"
    public_inputs.mkdir()
    for index, (name, (bundled_relative, _maximum)) in enumerate(
        sorted(finalizer.PUBLIC_PRIVACY_INPUTS.items()),
        start=1,
    ):
        payload = f"reviewed-public-input-{index}\n".encode("ascii")
        (public_inputs / name).write_bytes(payload)
        bundled = evidence / bundled_relative
        bundled.parent.mkdir(parents=True, exist_ok=True)
        bundled.write_bytes(payload)
    archive = tmp_path / f"{evidence.name}.tar.gz"
    archive.write_bytes(b"unsigned-rollout-archive")
    signer = tmp_path / "external-signer"
    signer.write_bytes(b"reviewed-external-signer")
    signer.chmod(0o700)
    public_key = tmp_path / "release.pub"
    public_key.write_bytes(b"p" * 32)
    verifier = tmp_path / "sorafs-validate"
    verifier.write_bytes(b"reviewed-native-verifier")
    verifier.chmod(0o700)
    verifier_sha = hashlib.sha256(verifier.read_bytes()).hexdigest()
    output = tmp_path / f"{evidence.name}.authority"
    args = finalizer.parse_args(
        [
            "--evidence-root",
            str(evidence),
            "--archive",
            str(archive),
            "--output-dir",
            str(output),
            "--commit",
            "a" * 40,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--source-date-epoch",
            "1",
            "--checkout-root",
            str(checkout),
            "--public-privacy-input-dir",
            str(public_inputs),
            "--controller-manifest",
            str(controller_manifest),
            "--controller-digest",
            controller_digest,
            "--external-signer",
            str(signer),
            "--signing-public-key",
            str(public_key),
            "--trusted-signing-fingerprint",
            "b" * 64,
            "--release-manifest-verifier",
            str(verifier),
            "--trusted-release-manifest-verifier-sha256",
            verifier_sha,
        ]
    )
    monkeypatch.setattr(finalizer.platform, "system", lambda: "Linux")
    monkeypatch.setattr(finalizer.platform, "machine", lambda: "aarch64")
    return args, evidence, archive, output, verifier


def _install_pure_finalization_stubs(monkeypatch: pytest.MonkeyPatch) -> None:
    authority = {
        "dpn_validator_release_commit": DPN_COMMIT,
        "schema": "test-authority",
        "workspace_source_manifest_sha256": "c" * 64,
    }
    monkeypatch.setattr(
        finalizer,
        "build_authority",
        lambda _args: _authorized(authority),
    )
    monkeypatch.setattr(
        finalizer,
        "build_release_manifest",
        lambda _args: {"schema": "test-manifest", "version": 1},
    )

    def sign(_manifest, _signer, _key, _fingerprint, signature, public, *_rest):
        finalizer.exclusive_write_bytes(signature, b"s" * 64)
        finalizer.exclusive_write_bytes(public, b"p" * 32)
        return {"signature_verified": True}

    monkeypatch.setattr(finalizer, "sign_release_manifest", sign)
    monkeypatch.setattr(
        finalizer,
        "verify_release_manifest",
        lambda *_args: {"signature_verified": True},
    )


def test_finalizer_completes_one_closed_two_phase_authority(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, _evidence, archive, output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)

    result = finalizer.finalize(args)

    assert result["archive"] == str(archive)
    assert result["authority_dir"] == str(output)
    assert result["dpn_validator_release_commit"] == DPN_COMMIT
    assert finalizer.scan_inventory_paths(output) == sorted(
        [
            "release_manifest.json",
            "release_manifest.json.pub",
            "release_manifest.json.sig",
            "artifacts/SHA256SUMS",
            *[f"artifacts/{name}" for name in finalizer.ARTIFACT_FILES],
        ]
    )
    checksums = (output / "artifacts/SHA256SUMS").read_text(encoding="ascii")
    assert checksums.count("\n") == len(finalizer.ARTIFACT_FILES)
    assert (
        output / "artifacts" / finalizer.CONTROLLER_MANIFEST_NAME
    ).read_bytes() == b'{"sealed":true}\n'
    assert (output / "artifacts" / finalizer.AUTHORITY_ENVELOPE).read_bytes() == (
        _authorized({}).authority_envelope
    )
    assert (output / "artifacts" / finalizer.DURABLE_RECEIPT).read_bytes() == (
        _authorized({}).durable_receipt
    )


def test_finalizer_rejects_authority_subject_replay_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, _evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    calls = 0

    def drifting_authority(_args):
        nonlocal calls
        calls += 1
        return _authorized(
            {
                "dpn_validator_release_commit": DPN_COMMIT,
                "schema": "test-authority",
                "workspace_source_manifest_sha256": "c" * 64,
                "generation": calls,
            }
        )

    monkeypatch.setattr(finalizer, "build_authority", drifting_authority)
    with pytest.raises(
        finalizer.FinalizationError, match="subject changed after signing"
    ):
        finalizer.finalize(args)


def test_finalizer_rejects_authenticated_sidecar_replay_drift(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, _evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    calls = 0
    subject = {
        "dpn_validator_release_commit": DPN_COMMIT,
        "schema": "test-authority",
        "workspace_source_manifest_sha256": "c" * 64,
    }

    def drifting_sidecars(_args):
        nonlocal calls
        calls += 1
        return _authorized(subject, sidecar_generation=calls)

    monkeypatch.setattr(finalizer, "build_authority", drifting_sidecars)
    with pytest.raises(
        finalizer.FinalizationError, match="authenticated sidecars changed"
    ):
        finalizer.finalize(args)


def test_finalizer_rejects_substituted_bundled_authority_helper(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    (evidence / "scripts/taira_release_authority.py").write_bytes(b"substituted")

    with pytest.raises(finalizer.FinalizationError, match="exact finalizer helper"):
        finalizer.finalize(args)


def test_finalizer_rejects_wrong_native_verifier_digest(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, _evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    args.trusted_release_manifest_verifier_sha256 = "0" * 64

    with pytest.raises(finalizer.FinalizationError, match="trusted digest"):
        finalizer.finalize(args)


def test_finalizer_rejects_archive_name_not_bound_to_evidence_root(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, _evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    wrong = tmp_path / "different.tar.gz"
    wrong.write_bytes(b"wrong")
    args.archive = str(wrong)

    with pytest.raises(finalizer.FinalizationError, match="exactly match"):
        finalizer.finalize(args)


def test_checksum_writer_rejects_extra_authority_artifact(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    for name in finalizer.ARTIFACT_FILES:
        (artifacts / name).write_bytes(name.encode("ascii"))
    (artifacts / "unexpected").write_bytes(b"extra")

    with pytest.raises(finalizer.FinalizationError, match="not exactly closed"):
        finalizer._write_checksums(artifacts)


def test_finalizer_rejects_public_privacy_input_substitution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    (evidence / finalizer.PUBLIC_PRIVACY_INPUTS["config.toml"][0]).write_bytes(
        b"hostile substituted config\n"
    )

    with pytest.raises(finalizer.FinalizationError, match="substituted"):
        finalizer.finalize(args)


@pytest.mark.parametrize("mutation", ("missing", "extra"))
def test_finalizer_rejects_non_exact_public_privacy_input_inventory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
) -> None:
    args, _evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    _install_pure_finalization_stubs(monkeypatch)
    public_inputs = Path(args.public_privacy_input_dir)
    if mutation == "missing":
        (public_inputs / "config.toml").unlink()
    else:
        (public_inputs / "unexpected").write_bytes(b"unexpected\n")

    with pytest.raises(finalizer.FinalizationError, match="exactly four"):
        finalizer.finalize(args)


def test_exact_public_privacy_comparison_replays_earlier_paths(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, evidence, _archive, _output, _verifier = _fixture(tmp_path, monkeypatch)
    public_inputs = Path(args.public_privacy_input_dir)
    real_read = finalizer.stable_read_relative
    bundled_config = finalizer.PUBLIC_PRIVACY_INPUTS["config.toml"][0]
    replaced = False

    def replacing_read(root: Path, relative: str, **kwargs):
        nonlocal replaced
        result = real_read(root, relative, **kwargs)
        if root == evidence and relative == bundled_config and not replaced:
            replacement = public_inputs / "replacement-config"
            replacement.write_bytes(b"same-name post-comparison replacement\n")
            replacement.chmod(0o600)
            replacement.replace(public_inputs / "config.toml")
            replaced = True
        return result

    monkeypatch.setattr(finalizer, "stable_read_relative", replacing_read)
    with pytest.raises(
        finalizer.FinalizationError,
        match="changed during exact byte comparison",
    ):
        finalizer._verify_public_privacy_inputs(public_inputs, evidence)
