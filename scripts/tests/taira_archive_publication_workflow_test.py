from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"


def _assert_archive_publication_contract(source: str) -> None:
    lowered = source.lower()
    for forbidden in (
        "docker/build-push-action@",
        "docker/login-action@",
        "docker push",
        "docker manifest",
        "setup-buildx-action@",
        "push_latest",
        "taira-latest",
    ):
        assert forbidden not in lowered

    assert "runs-on: [self-hosted, Linux, ARM64, iroha2]" in source
    assert "runs-on: [self-hosted, macOS, ARM64, taira-release]" in source
    assert source.count("prepare_taira_release_source.sh") == 2
    assert (
        'cmp "$MACOS_SOURCE_IDENTITY" "$linux_root/taira-source-identity-v1.json"'
        in source
    )
    assert "build_taira_rollout_bundle.sh" in source
    assert "capture_taira_macos_four_peer_receipt.py" in source
    assert "build_taira_rollout_candidate.py assemble" in source
    assert "build_taira_rollout_candidate.py pack-deploy-payload" in source
    for invalid_isolated_invocation in (
        "python3 -I -S scripts/build_taira_rollout_candidate.py",
        "/usr/bin/python3 -I -S \\",
        "python3 -I -S scripts/taira_rollout_admission.py",
        "python3 -I -S scripts/write_release_sha256sums.py",
        "python3 -I -S scripts/generate_release_manifest.py",
    ):
        assert invalid_isolated_invocation not in source

    assert (
        "uses: oras-project/setup-oras@"
        "22ce207df3b08e061f537244349aac6ae1d214f6" in source
    )
    assert "ORAS_VERSION: 1.3.2" in source
    assert (
        "https://github.com/oras-project/oras/releases/download/v1.3.2/"
        "oras_1.3.2_darwin_arm64.tar.gz" in source
    )
    assert "7929f792cf272268412375ecad6f0fb3c20f164368d5b57966e67ad6d36eca53" in source
    for media_type in (
        "application/vnd.hyperledger.iroha.taira.rollout-admission.v1",
        "application/vnd.hyperledger.iroha.taira.rollout-admission.archive.v1+tar+gzip",
        "application/vnd.hyperledger.iroha.taira.macos-arm64.deploy.v1+tar+gzip",
        "application/vnd.hyperledger.iroha.release-manifest.v1+json",
        "application/vnd.hyperledger.iroha.release-manifest.signature.v1+ed25519",
        "application/vnd.hyperledger.iroha.ed25519-public-key.v1",
        "application/vnd.hyperledger.iroha.taira.publication-receipt.v1",
    ):
        assert media_type in source

    upload = source.index("Upload the authenticated pre-publication bytes")
    download = source.index(
        "Download the pre-publication bytes into a fresh replay root"
    )
    readmit = source.index(
        "Byte-compare and re-admit the uploaded archive before registry mutation"
    )
    oras_setup = source.index("Install exact checksum-pinned ORAS")
    push = source.index("Publish the exact generic OCI artifact")
    pull = source.index("Pull by immutable digest, compare every byte")
    receipt = source.index("Create and verify the signed publication receipt")
    attach = source.index(
        "Attach, pull, and byte-verify the signed publication receipt"
    )
    assert upload < download < readmit < oras_setup < push < pull < receipt < attach

    pre_mutation = source[readmit:push]
    assert "taira_rollout_admission.py verify" in pre_mutation
    assert "upload replay bytes differ" in pre_mutation
    pushed = source[push:pull]
    assert "oras push" in pushed
    assert '--artifact-type "$TAIRA_PRIMARY_ARTIFACT_TYPE"' in pushed
    assert "manifest_digest" in pushed
    assert "oras resolve" in pushed
    assert "oras manifest fetch" in pushed
    assert "sha256:$(shasum -a 256" in pushed
    pulled = source[pull:receipt]
    assert "oras pull" in pulled
    assert '"$IMMUTABLE_REFERENCE"' in pulled
    assert "OCI pull changed bytes" in pulled
    assert "taira_rollout_admission.py verify" in pulled
    signed = source[receipt:]
    assert "release_manifest_signing.py sign" in signed
    assert "release_manifest_signing.py verify" in signed
    assert "release_manifest.replay.json" in signed
    assert "oras attach" in signed
    assert "receipt_reference" in signed


def test_taira_publish_is_archive_only_digest_replayed_and_receipted() -> None:
    _assert_archive_publication_contract(WORKFLOW.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            "22ce207df3b08e061f537244349aac6ae1d214f6",
            "oras-project/setup-oras@v1",
        ),
        lambda source: source.replace(
            "7929f792cf272268412375ecad6f0fb3c20f164368d5b57966e67ad6d36eca53",
            "0" * 64,
        ),
        lambda source: source.replace("oras resolve", "true # resolve removed", 1),
        lambda source: source.replace("oras pull", "true # pull removed", 1),
        lambda source: source.replace(
            "python3 -S scripts/taira_rollout_admission.py verify",
            "true # admission removed",
        ),
        lambda source: source.replace(
            "python3 -I -S scripts/release_manifest_signing.py sign",
            "true # receipt signing removed",
            1,
        ),
        lambda source: source + "\n      - run: docker push forbidden\n",
    ),
    ids=(
        "mutable-oras-action",
        "oras-checksum-placeholder",
        "digest-resolution-removed",
        "digest-pull-removed",
        "admission-reverification-removed",
        "publication-signature-removed",
        "oci-image-fallback-added",
    ),
)
def test_archive_publication_contract_rejects_adversarial_mutations(mutation) -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_archive_publication_contract(source)
    with pytest.raises(AssertionError):
        _assert_archive_publication_contract(mutation(source))


def test_macos_capture_source_requires_every_peer_restart_and_cleanup() -> None:
    source = (ROOT / "scripts/capture_taira_macos_four_peer_receipt.py").read_text(
        encoding="utf-8"
    )
    assert 'platform.system() != "Darwin" or platform.machine() != "arm64"' in source
    assert "for item in running:" in source
    assert "os.kill(old_child, signal.SIGTERM)" in source
    assert "_wait_new_child(" in source
    assert "deploy.wait_for_advancement(" in source
    assert '"restart_proof": "passed"' in source
    assert "deploy.restore_release_to_bundle(bundle)" in source
    assert "deploy.require_bundle_runtime_unchanged(bundle)" in source


@pytest.mark.parametrize(
    "relative",
    (
        "scripts/build_taira_rollout_candidate.py",
        "scripts/capture_taira_macos_four_peer_receipt.py",
        "scripts/taira_rollout_admission.py",
        "scripts/write_release_sha256sums.py",
        "scripts/generate_release_manifest.py",
    ),
)
def test_workflow_local_import_scripts_start_with_exact_runtime_flags(
    relative: str,
) -> None:
    result = subprocess.run(
        [sys.executable, "-S", relative, "--help"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
