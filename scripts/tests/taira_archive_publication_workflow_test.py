from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import capture_taira_macos_four_peer_receipt as capture
from scripts import deploy_taira_v21_reset as deploy
from scripts import release_artifact_contract as contract
from scripts import taira_rollout_admission as admission


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"
SOURCE_HELPER = ROOT / "configs/soranexus/taira/prepare_taira_release_source.sh"


def _assert_source_reconstruction_contract(source: str) -> None:
    assert "set -euo pipefail" in source
    assert '[[ ! "$VALIDATOR_RELEASE_REF" =~ ^[0-9a-f]{40}$ ]]' in source
    assert '[[ ! "$VALIDATOR_LOCK_SHA256" =~ ^[0-9a-f]{64}$ ]]' in source
    assert (
        "https://raw.githubusercontent.com/soramitsu/dpn-api-rust/${VALIDATOR_RELEASE_REF}"
        in source
    )
    assert "--proto '=https'" in source
    assert "--proto-redir '=https'" in source
    assert "--tlsv1.2" in source
    for name in (
        "provenance.json",
        "tracked.patch",
        "untracked.tar",
        "untracked.manifest.json",
        "source.manifest.json",
        "taira-validator.Cargo.lock",
    ):
        assert name in source
    assert "hashlib.sha256(lock.read_bytes()).hexdigest() != expected" in source
    assert '"$release_dir/iroha_source_bundle.py" reconstruct' in source
    assert '"$release_dir/iroha_source_bundle.py" verify' in source
    assert 'git -C "$WORKSPACE" verify-commit "$git_head"' in source
    assert source.count("compute_workspace_source_manifest.py") == 2
    assert (
        '"iroha_worktree_clean": os.environ["IROHA_WORKTREE_CLEAN"] == "True"' in source
    )
    assert (
        'json.dumps(payload, ensure_ascii=True, sort_keys=True, separators=(",", ":"))'
        in source
    )


def test_dual_target_source_reconstruction_is_exact_and_fail_closed() -> None:
    _assert_source_reconstruction_contract(SOURCE_HELPER.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace("--proto-redir '=https'", "--proto-redir '=all'"),
        lambda source: source.replace(
            "dpn-api-rust/${VALIDATOR_RELEASE_REF}", "dpn-api-rust/main"
        ),
        lambda source: source.replace(
            "hashlib.sha256(lock.read_bytes()).hexdigest() != expected", "False"
        ),
        lambda source: source.replace(
            'git -C "$WORKSPACE" verify-commit "$git_head"', "true"
        ),
        lambda source: source.replace(
            '"$release_dir/iroha_source_bundle.py" verify', "true"
        ),
    ),
)
def test_source_reconstruction_contract_rejects_adversarial_mutations(
    mutation,
) -> None:
    source = SOURCE_HELPER.read_text(encoding="utf-8")
    _assert_source_reconstruction_contract(source)
    with pytest.raises(AssertionError):
        _assert_source_reconstruction_contract(mutation(source))


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
    assert "sudo -n /usr/bin/python3 -S \\" in source
    assert source.count('CARGO_TARGET_DIR="$target_root"') == 2
    assert 'mkdir -m 0700 "$target_root"' in source
    assert "$GITHUB_WORKSPACE/target/release/irohad" not in source
    assert '--validator-binary "$TAIRA_MACOS_IROHAD"' in source
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
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.oci.empty.v1+json",
    ):
        assert media_type in source
    assert (
        "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        in source
    )
    assert source.count("object_pairs_hook=object_from_pairs") >= 3

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
    assert "--image-spec v1.1" in pushed
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
    attachment = source[attach:]
    assert "raw_receipt_manifest" in attachment
    assert "oras manifest fetch" in attachment
    assert 'subject.get("digest")' in attachment
    assert (
        "publication receipt is not attached to the exact primary manifest"
        in attachment
    )
    assert "publication receipt OCI layer contract differs" in attachment
    cleanup = source[source.index("Remove the ephemeral ORAS login") :]
    assert "if: always()" in cleanup
    assert (
        'auth_dir="$RUNNER_TEMP/taira-oras-auth-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"'
        in cleanup
    )
    assert 'rm -f -- "$registry_config"' in cleanup


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
        lambda source: source.replace("--image-spec v1.1", "--image-spec v1.0"),
        lambda source: source.replace(
            'subject.get("digest")', "primary_digest # subject digest ignored"
        ),
        lambda source: source.replace("if: always()", "if: success()", 1),
        lambda source: source + "\n      - run: docker push forbidden\n",
    ),
    ids=(
        "mutable-oras-action",
        "oras-checksum-placeholder",
        "digest-resolution-removed",
        "digest-pull-removed",
        "admission-reverification-removed",
        "publication-signature-removed",
        "oci-image-spec-downgraded",
        "receipt-subject-binding-removed",
        "credential-cleanup-not-always",
        "oci-image-fallback-added",
    ),
)
def test_archive_publication_contract_rejects_adversarial_mutations(mutation) -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_archive_publication_contract(source)
    with pytest.raises(AssertionError):
        _assert_archive_publication_contract(mutation(source))


def _assert_macos_capture_contract(source: str) -> None:
    assert 'platform.system() != "Darwin" or platform.machine() != "arm64"' in source
    assert "for item in running:" in source
    assert "for item in running:\n            _terminal_check(running)" in source
    assert "os.kill(old_child, signal.SIGTERM)" in source
    assert "_wait_new_child(" in source
    assert "deploy.wait_for_advancement(" in source
    assert '"restart_proof": "passed"' in source
    assert "VALIDATION_SUPERVISOR_ROOT" in source
    assert 'installed_name="taira_peer_supervisor.py"' in source
    assert "supervisor=installed_supervisor" in source
    assert 'cleanup_errors.append("validation-supervisor")' in source
    assert "deploy.restore_release_to_bundle(bundle)" in source
    assert "deploy.require_bundle_runtime_unchanged(bundle)" in source


def test_macos_capture_source_requires_every_peer_restart_and_cleanup() -> None:
    source = (ROOT / "scripts/capture_taira_macos_four_peer_receipt.py").read_text(
        encoding="utf-8"
    )
    _assert_macos_capture_contract(source)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            'platform.system() != "Darwin" or platform.machine() != "arm64"',
            "False",
        ),
        lambda source: source.replace(
            "for item in running:\n            _terminal_check(running)",
            "for item in running[:1]:\n            _terminal_check(running)",
        ),
        lambda source: source.replace(
            "os.kill(old_child, signal.SIGTERM)", "# child restart removed"
        ),
        lambda source: source.replace(
            "deploy.wait_for_advancement(", "removed_wait_for_advancement("
        ),
        lambda source: source.replace(
            "supervisor=installed_supervisor", "supervisor=supervisor"
        ),
        lambda source: source.replace(
            "deploy.restore_release_to_bundle(bundle)", "# release restoration removed"
        ),
    ),
)
def test_macos_capture_contract_rejects_adversarial_mutations(mutation) -> None:
    source = (ROOT / "scripts/capture_taira_macos_four_peer_receipt.py").read_text(
        encoding="utf-8"
    )
    _assert_macos_capture_contract(source)
    with pytest.raises(AssertionError):
        _assert_macos_capture_contract(mutation(source))


def test_macos_capture_emits_the_exact_canonical_admission_receipt() -> None:
    source = admission.SourceIdentity("1" * 40, "2" * 64, "3" * 64)
    peers = tuple(
        SimpleNamespace(
            config_sha256=f"{number:x}" * 64,
            label=f"taira-validator-{number}",
            number=number,
            slug=f"taira-validator-{number}",
        )
        for number in range(1, 5)
    )
    bundle = SimpleNamespace(manifest_sha256="8" * 64, peers=peers)
    start = deploy.FleetSample(
        height=100,
        block_hash="4" * 64,
        context="context",
        build="build",
        config="config",
        offline_release="offline",
        nodes=("one", "two", "three", "four"),
    )
    end = deploy.FleetSample(
        height=105,
        block_hash="5" * 64,
        context="context",
        build="build",
        config="config",
        offline_release="offline",
        nodes=("one", "two", "three", "four"),
    )
    issued_at = 1_800_000_000

    receipt = capture._receipt(
        source=source,
        bundle=bundle,
        binary_sha256="6" * 64,
        supervisor_sha256="7" * 64,
        restart_generation="9" * 64,
        start=start,
        end=end,
        issued_at=issued_at,
    )
    payload = contract.canonical_json_bytes(receipt)
    verified = admission._validate_macos_receipt(
        payload,
        expected_source=source,
        expected_receipt_id=str(receipt["receipt_id"]),
        consumed_receipt_ids=set(),
        now_unix=issued_at,
    )

    assert verified["receipt_id"] == receipt["receipt_id"]
    assert verified["end_height"] == 105
    assert receipt["expires_at_unix"] == (
        issued_at + capture.MAX_RECEIPT_LIFETIME_SECONDS
    )
    assert [row["number"] for row in receipt["peers"]] == [1, 2, 3, 4]


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
