from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"
SOURCE_HELPER = ROOT / "configs/soranexus/taira/prepare_taira_release_source.sh"


def _workflow(source: str | None = None) -> dict[str, object]:
    ruby = (
        "document = YAML.safe_load(STDIN.read, aliases: false); "
        'puts JSON.generate(document.fetch("jobs"))'
    )
    result = subprocess.run(
        ["ruby", "-ryaml", "-rjson", "-e", ruby],
        input=WORKFLOW.read_text(encoding="utf-8") if source is None else source,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    value = json.loads(result.stdout)
    assert isinstance(value, dict)
    return value


def _steps(job: dict[str, object]) -> list[dict[str, object]]:
    value = job.get("steps")
    assert isinstance(value, list)
    return value


def _job_text(job: dict[str, object]) -> str:
    return json.dumps(job, ensure_ascii=True, sort_keys=True)


def _assert_source_reconstruction_contract(source: str) -> None:
    assert "set -euo pipefail" in source
    assert '[[ ! "$VALIDATOR_RELEASE_REF" =~ ^[0-9a-f]{40}$ ]]' in source
    assert '[[ ! "$VALIDATOR_LOCK_SHA256" =~ ^[0-9a-f]{64}$ ]]' in source
    assert "https://raw.githubusercontent.com/soramitsu/dpn-api-rust/${VALIDATOR_RELEASE_REF}" in source
    assert "--proto '=https'" in source
    assert "--proto-redir '=https'" in source
    assert "--tlsv1.2" in source
    assert 'git -C "$WORKSPACE" verify-commit "$git_head"' in source
    assert '"$release_dir/iroha_source_bundle.py" reconstruct' in source
    assert '"$release_dir/iroha_source_bundle.py" verify' in source
    assert "hashlib.sha256(lock.read_bytes()).hexdigest() != expected" in source


def test_source_reconstruction_remains_exact_and_fail_closed() -> None:
    _assert_source_reconstruction_contract(SOURCE_HELPER.read_text(encoding="utf-8"))


def _assert_split_trust_architecture(source: str) -> None:
    jobs = _workflow(source)
    assert list(jobs) == [
        "public-privacy-input",
        "linux-native-build",
        "linux-native-authority",
        "macos-native-build",
        "macos-secret-free-qualification",
        "macos-candidate-authority",
        "macos-deploy",
        "macos-publish",
    ]

    build_jobs = {"linux-native-build", "macos-native-build"}
    authority_jobs = {
        "public-privacy-input",
        "linux-native-authority",
        "macos-candidate-authority",
        "macos-publish",
    }
    product_execution_jobs = {"macos-secret-free-qualification", "macos-deploy"}

    for name, raw_job in jobs.items():
        assert isinstance(raw_job, dict)
        steps = _steps(raw_job)
        checkout = [step for step in steps if "actions/checkout@" in str(step.get("uses", ""))]
        if name in build_jobs:
            assert len(checkout) == 1
            assert "environment" not in raw_job
        else:
            assert not checkout, name
        if name in authority_jobs:
            text = _job_text(raw_job)
            for forbidden in (
                "cargo build",
                "prepare_taira_release_source.sh",
                "$GITHUB_WORKSPACE/scripts/",
                "configs/soranexus/taira/build_taira_rollout_bundle.sh",
                "bin/irohad --",
            ):
                assert forbidden not in text, (name, forbidden)
        if name in product_execution_jobs:
            text = _job_text(raw_job)
            assert "TAIRA_OCI_USERNAME" not in text
            assert "TAIRA_OCI_PASSWORD" not in text
            assert "TAIRA_RELEASE_EXTERNAL_SIGNER_PATH" not in text
            assert "TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH" not in text

    assert jobs["linux-native-authority"]["needs"] == [
        "public-privacy-input",
        "linux-native-build",
    ]
    assert jobs["macos-secret-free-qualification"]["needs"] == [
        "linux-native-authority",
        "macos-native-build",
    ]
    assert jobs["macos-candidate-authority"]["needs"] == "macos-secret-free-qualification"
    assert jobs["macos-deploy"]["needs"] == [
        "macos-candidate-authority",
        "macos-native-build",
        "linux-native-authority",
    ]
    assert jobs["macos-publish"]["needs"] == "macos-deploy"

    assert jobs["macos-secret-free-qualification"]["environment"] == "taira-validator-qualification"
    assert jobs["macos-deploy"]["environment"] == "taira-validator-deploy"
    assert jobs["macos-publish"]["environment"] == "taira-validator-publish"
    assert jobs["linux-native-authority"]["runs-on"][-1] == "taira-linux-authority"
    assert jobs["macos-secret-free-qualification"]["runs-on"][-1] == "taira-secret-free-qualification"
    assert jobs["macos-candidate-authority"]["runs-on"][-1] == "taira-candidate-authority"
    assert jobs["macos-deploy"]["runs-on"][-1] == "taira-deploy"
    assert jobs["macos-publish"]["runs-on"][-1] == "taira-publish-authority"


def test_release_workflow_splits_hostile_build_qualification_authority_deploy_and_publish() -> None:
    _assert_split_trust_architecture(WORKFLOW.read_text(encoding="utf-8"))


def test_rollouts_are_globally_serialized_and_unsigned_authority_outputs_stay_root_anchored() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    assert "concurrency:\n  group: taira-validator-rollout\n  cancel-in-progress: false" in source
    assert "vars.TAIRA_PUBLIC_INPUT_STAGING_ROOT" in source
    assert "vars.TAIRA_QUALIFICATION_STAGING_ROOT" in source
    assert (
        'handoff="$AUTHORITY_STAGING_ROOT/public-input-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"'
        in source
    )
    assert (
        'stage="$AUTHORITY_STAGING_ROOT/qualification-receipt-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"'
        in source
    )
    assert "${{ runner.temp }}/taira-public-privacy-input/" not in source
    assert "${{ runner.temp }}/taira-qualification-receipt/" not in source


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            "    environment: taira-validator-qualification\n",
            "    environment: taira-validator-publish\n",
            1,
        ),
        lambda source: source.replace(
            "          TAIRA_WORKFLOW_COMMIT: ${{ github.sha }}\n        run: |",
            "          TAIRA_RELEASE_EXTERNAL_SIGNER_PATH: ${{ vars.TAIRA_RELEASE_EXTERNAL_SIGNER_PATH }}\n          TAIRA_WORKFLOW_COMMIT: ${{ github.sha }}\n        run: |",
            1,
        ).replace("public-privacy-input", "macos-secret-free-qualification", 1),
        lambda source: source.replace(
            "  macos-publish:\n    needs: macos-deploy",
            "  macos-publish:\n    needs: macos-secret-free-qualification",
            1,
        ),
    ),
    ids=("qualification-publish-environment", "qualification-signer", "publish-skips-deploy"),
)
def test_persistent_product_process_isolation_contract_rejects_mutations(mutation) -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_split_trust_architecture(source)
    with pytest.raises((AssertionError, KeyError)):
        _assert_split_trust_architecture(mutation(source))


def _assert_fixed_controller_contract(source: str) -> None:
    assert "/usr/local/libexec/iroha-taira-release-controller-v1" in source
    assert "/usr/local/libexec/iroha-taira-release-controller-v1.d" in source
    for forbidden in (
        "sudo -n /usr/bin/python3",
        "sudo -n python3",
        "exec(compile(",
        "git show <workflow-commit>",
        "controller-linux.*",
        "controller-macos.*",
        " seal --workspace",
        " cleanup --sealed-root",
        "Seal the reviewed",
        "Remove the exact sealed",
    ):
        assert forbidden not in source
    for line in source.splitlines():
        if "sudo -n " in line:
            assert 'sudo -n "$TAIRA_CONTROLLER_COMMAND"' in line
        if any(
            token in line
            for token in (
                '"$TAIRA_CONTROLLER_COMMAND" attest ',
                '"$TAIRA_CONTROLLER_COMMAND" inspect-handoff ',
                '"$TAIRA_CONTROLLER_COMMAND" run ',
            )
        ):
            assert 'sudo -n "$TAIRA_CONTROLLER_COMMAND"' in line
    assert source.count('EXPECTED_UID: "0"') == 9

    for trust_arg in (
        "--expected-launcher-sha256",
        "--expected-controller-digest",
        "--expected-version",
        "--expected-host-id",
        "--expected-installation-id",
        "--expected-uid",
        "--platform",
        "--role",
    ):
        assert source.count(trust_arg) == 10, trust_arg
    assert source.count("--source-commit") >= 10
    assert source.count("inspect-handoff") >= 9
    assert source.count("--stage-name") == source.count("inspect-handoff")
    assert "artifact-handoff-sha256" in source
    assert "--artifact-handoff-sha256" in source
    assert "--expected-artifact-handoff-sha256" in source


def test_workflow_uses_only_preprovisioned_digest_and_identity_attested_controller() -> None:
    _assert_fixed_controller_contract(WORKFLOW.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "needle",
    (
        "--expected-host-id",
        "--expected-installation-id",
        "--expected-uid",
        "--stage-name",
    ),
)
def test_controller_trust_inputs_are_fail_closed_against_removal(needle: str) -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_fixed_controller_contract(source)
    with pytest.raises(AssertionError):
        _assert_fixed_controller_contract(source.replace(needle, f"--removed-{needle[2:]}", 1))


def _assert_no_github_output_channel(source: str) -> None:
    writes = [line.strip() for line in source.splitlines() if "$GITHUB_OUTPUT" in line]
    assert writes == [
        "printf 'receipt_id=%s\\n' \"$receipt_id\" >>\"$GITHUB_OUTPUT\""
    ]
    assert '[[ "$receipt_id" =~ ^[0-9a-f]{64}$ ]]' in source
    assert source.index('[[ "$receipt_id" =~ ^[0-9a-f]{64}$ ]]') < source.index(
        writes[0]
    )


def test_hostile_filenames_cannot_inject_github_outputs() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_no_github_output_channel(source)
    assert "unsigned Linux archive name is not canonical" in source
    assert r"[0-9a-f]{12}-release-linux-aarch64\.tar\.gz" in source


def test_github_output_injection_contract_rejects_mutation() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    with pytest.raises(AssertionError):
        _assert_no_github_output_channel(
            source + '\n# echo "archive=$hostile_name" >>$GITHUB_OUTPUT\n'
        )


def test_downloaded_artifacts_are_quarantined_staged_and_consumed_only_after_inspection() -> None:
    jobs = _workflow()
    consumers = {
        "linux-native-authority": ("linux-unsigned", "public-privacy-input"),
        "macos-secret-free-qualification": ("linux-authority", "macos-build"),
        "macos-candidate-authority": ("linux-authority", "qualification-receipt"),
        "macos-deploy": ("linux-authority", "candidate", "macos-build"),
        "macos-publish": ("candidate",),
    }
    for job_name, kinds in consumers.items():
        text = _job_text(jobs[job_name])
        assert text.count("actions/download-artifact@") == len(kinds)
        for kind in kinds:
            assert f"--expected-kind {kind}" in text
        assert text.count("--stage-name") == len(kinds)
        assert "staged_root" in text


def test_authority_host_attestations_are_distinct_and_cannot_be_reused() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    for variable in (
        "TAIRA_PUBLIC_INPUT_HOST_ID",
        "TAIRA_LINUX_AUTHORITY_HOST_ID",
        "TAIRA_QUALIFICATION_HOST_ID",
        "TAIRA_CANDIDATE_AUTHORITY_HOST_ID",
        "TAIRA_DEPLOY_HOST_ID",
        "TAIRA_PUBLISH_HOST_ID",
        "TAIRA_PUBLIC_INPUT_INSTALLATION_ID",
        "TAIRA_LINUX_AUTHORITY_INSTALLATION_ID",
        "TAIRA_QUALIFICATION_INSTALLATION_ID",
        "TAIRA_CANDIDATE_AUTHORITY_INSTALLATION_ID",
        "TAIRA_DEPLOY_INSTALLATION_ID",
        "TAIRA_PUBLISH_INSTALLATION_ID",
    ):
        assert f"vars.{variable}" in source
    assert "--role macos-qualification" in source
    assert "--role macos-candidate-authority" in source
    assert "--role macos-deploy" in source
    assert "--role macos-publish" in source


def _publisher_step(source: str | None = None) -> dict[str, object]:
    jobs = _workflow(source)
    steps = _steps(jobs["macos-publish"])
    assert [step.get("name") for step in steps] == [
        "Download candidate bytes into publication quarantine",
        "Publish through the sealed installed authority controller",
        "Upload the exact root-closed publication handoff",
    ]
    return steps[1]


def _assert_sealed_publication_workflow(source: str) -> None:
    step = _publisher_step(source)
    env = step.get("env")
    assert isinstance(env, dict)
    assert env["TAIRA_PUBLISH_HANDOFF_ROOT"] == (
        "${{ vars.TAIRA_PUBLISH_HANDOFF_ROOT }}"
    )
    assert env["TAIRA_OCI_REPOSITORY"] == "${{ vars.TAIRA_OCI_REPOSITORY }}"
    assert env["TAIRA_OCI_TAG_SUFFIX"] == "${{ vars.TAIRA_OCI_TAG_SUFFIX }}"
    for forbidden in (
        "TAIRA_OCI_USERNAME",
        "TAIRA_OCI_PASSWORD",
        "TAIRA_RELEASE_EXTERNAL_SIGNER_PATH",
        "TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH",
        "TAIRA_RELEASE_MANIFEST_VERIFIER_PATH",
        "TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT",
        "ORAS_VERSION",
    ):
        assert forbidden not in env
    run = str(step.get("run", ""))
    for required in (
        "inspect-handoff",
        "--expected-kind candidate",
        'candidate="$TAIRA_PUBLISH_HANDOFF_ROOT/publish-candidate-',
        'test "$inspected_candidate" = "$candidate"',
        'test "$source_commit" = "$TAIRA_WORKFLOW_COMMIT"',
        'test "$dpn_commit" = "$TAIRA_INPUT_VALIDATOR_RELEASE_REF"',
        "publish-rollout --",
        '--candidate-root "$candidate"',
        '--expected-source-commit "$source_commit"',
        '--expected-dpn-validator-release-commit "$dpn_commit"',
        '--expected-cargo-lock-sha256 "$cargo_lock"',
        '--expected-workspace-source-manifest-sha256 "$source_manifest"',
        '--expected-qualification-receipt-id "$receipt_id"',
        '--repository "$TAIRA_OCI_REPOSITORY"',
        '--suffix "$TAIRA_OCI_TAG_SUFFIX"',
        "publication handoff inventory differs",
        "publication handoff file identity differs",
    ):
        assert required in run
    for forbidden in (
        "oras-project/setup-oras@",
        "oras push",
        "oras attach",
        "oras login",
        "oras logout",
        "--password-stdin",
        "sign-manifest",
        "admit --",
        "--registry-config",
        "--external-signer",
        "--signing-public-key",
        "--release-manifest-verifier",
        "--terminal-handoff",
        "--authority-uid",
        "--scratch-parent",
    ):
        assert forbidden not in _job_text(_workflow(source)["macos-publish"])
    assert "TAIRA_OCI_PASSWORD" not in source
    assert "TAIRA_OCI_USERNAME" not in source


def test_publication_credentials_and_tool_paths_never_enter_workflow_state() -> None:
    _assert_sealed_publication_workflow(WORKFLOW.read_text(encoding="utf-8"))


def test_publisher_rechecks_run_scoped_root_and_source_before_sealed_dispatch() -> None:
    step = _publisher_step()
    run = str(step.get("run", ""))
    assert run.index('test "$inspected_candidate" = "$candidate"') < run.index(
        "publish-rollout --"
    )
    assert run.index('test "$source_commit" = "$TAIRA_WORKFLOW_COMMIT"') < run.index(
        "publish-rollout --"
    )
    assert run.index(
        'test "$dpn_commit" = "$TAIRA_INPUT_VALIDATOR_RELEASE_REF"'
    ) < run.index("publish-rollout --")


def test_root_closed_publication_upload_is_exactly_receipt_derived() -> None:
    jobs = _workflow()
    upload = _steps(jobs["macos-publish"])[2]
    assert upload.get("uses") == (
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
    )
    with_values = upload.get("with")
    assert isinstance(with_values, dict)
    assert with_values["path"] == (
        "${{ vars.TAIRA_PUBLISH_HANDOFF_ROOT }}/publication-receipt-"
        "${{ steps.publish.outputs.receipt_id }}/"
    )
    assert with_values["if-no-files-found"] == "error"


@pytest.mark.parametrize(
    "needle",
    (
        "publish-rollout --",
        '--expected-qualification-receipt-id "$receipt_id"',
        '--repository "$TAIRA_OCI_REPOSITORY"',
        '--suffix "$TAIRA_OCI_TAG_SUFFIX"',
        "publication handoff inventory differs",
    ),
)
def test_sealed_publication_contract_rejects_removed_security_leg(
    needle: str,
) -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    _assert_sealed_publication_workflow(source)
    with pytest.raises(AssertionError):
        _assert_sealed_publication_workflow(
            source.replace(needle, "REMOVED_SECURITY_LEG", 1)
        )
