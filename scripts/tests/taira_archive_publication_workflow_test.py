from __future__ import annotations

import json
import subprocess
import sys
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
    assert "download_pids=()" in source
    assert 'download_pids+=("$!")' in source
    assert 'for download_pid in "${download_pids[@]}"' in source
    assert "--retry-max-time 120" in source
    assert "--max-time 120" in source


def test_source_reconstruction_remains_exact_and_fail_closed() -> None:
    _assert_source_reconstruction_contract(SOURCE_HELPER.read_text(encoding="utf-8"))


def _assert_split_trust_architecture(source: str) -> None:
    jobs = _workflow(source)
    assert list(jobs) == [
        "rollout-budget",
        "release-readiness",
        "public-privacy-input",
        "linux-native-build",
        "linux-native-authority",
        "macos-native-build",
        "macos-secret-free-qualification",
        "macos-candidate-authority",
        "linux-boi-qualification",
        "macos-deploy",
        "macos-publish",
    ]

    build_jobs = {"linux-native-build", "macos-native-build"}
    authority_jobs = {
        "public-privacy-input",
        "linux-native-authority",
        "macos-candidate-authority",
        "linux-boi-qualification",
        "macos-publish",
    }
    product_execution_jobs = {
        "macos-secret-free-qualification",
        "linux-boi-qualification",
        "macos-deploy",
    }

    for name, raw_job in jobs.items():
        assert isinstance(raw_job, dict)
        steps = _steps(raw_job)
        checkout = [step for step in steps if "actions/checkout@" in str(step.get("uses", ""))]
        if name in build_jobs or name == "release-readiness":
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
                "bin/iroha3d --",
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
    assert jobs["public-privacy-input"]["needs"] == "release-readiness"
    assert jobs["macos-native-build"]["needs"] == "release-readiness"
    assert jobs["macos-secret-free-qualification"]["needs"] == [
        "linux-native-authority",
        "macos-native-build",
    ]
    assert jobs["macos-candidate-authority"]["needs"] == "macos-secret-free-qualification"
    assert jobs["linux-boi-qualification"]["needs"] == "macos-candidate-authority"
    assert jobs["macos-deploy"]["needs"] == [
        "macos-candidate-authority",
        "linux-boi-qualification",
        "macos-native-build",
        "linux-native-authority",
    ]
    assert jobs["macos-publish"]["needs"] == "macos-deploy"

    assert jobs["macos-secret-free-qualification"]["environment"] == "taira-validator-qualification"
    assert jobs["linux-boi-qualification"]["environment"] == (
        "taira-validator-boi-qualification"
    )
    assert jobs["macos-deploy"]["environment"] == "taira-validator-deploy"
    assert jobs["macos-publish"]["environment"] == "taira-validator-publish"
    assert jobs["linux-native-authority"]["runs-on"][-1] == "taira-linux-authority"
    assert jobs["macos-secret-free-qualification"]["runs-on"][-1] == "taira-secret-free-qualification"
    assert jobs["macos-candidate-authority"]["runs-on"][-1] == "taira-candidate-authority"
    assert jobs["linux-boi-qualification"]["runs-on"][-1] == (
        "taira-boi-qualification"
    )
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


def test_rollout_budget_fails_runner_queue_stalls_with_action_only_write_scope() -> None:
    jobs = _workflow()
    budget = jobs["rollout-budget"]
    assert budget["runs-on"] == "ubuntu-latest"
    assert budget["timeout-minutes"] == 31
    assert budget["permissions"] == {"actions": "write", "contents": "none"}
    text = _job_text(budget)
    assert "30 * 60" in text
    assert "2 * 60" in text
    assert "No Taira job ran for two minutes" in text
    assert "runner_name" in text
    assert "labels" in text
    assert "/cancel" in text
    assert "secrets." not in text


def test_release_readiness_fails_before_any_native_builder() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    jobs = _workflow()
    readiness_job = jobs["release-readiness"]
    assert readiness_job["runs-on"] == "ubuntu-latest"
    assert readiness_job["timeout-minutes"] == 2
    assert readiness_job["permissions"] == {"contents": "read"}
    readiness_text = _job_text(readiness_job)
    assert "check_taira_release_prerequisites.py" in readiness_text
    assert (
        "Verify all authenticated release-authority source paths" in readiness_text
    )
    assert "source-disabled" not in readiness_text
    assert (
        "# Audit the complete fixed native client and all eight authenticated authority"
        in source
    )
    assert "source barriers on a hosted runner" not in source
    assert jobs["public-privacy-input"]["needs"] == "release-readiness"
    assert jobs["macos-native-build"]["needs"] == "release-readiness"


def test_native_builds_are_parallel_cached_and_not_single_threaded() -> None:
    jobs = _workflow()
    linux = _job_text(jobs["linux-native-build"])
    macos = _job_text(jobs["macos-native-build"])
    assert jobs["macos-native-build"]["needs"] == "release-readiness"
    assert "taira-linux-authority-" not in macos
    for text, cache_variable in (
        (linux, "TAIRA_LINUX_BUILD_CACHE_ROOT"),
        (macos, "TAIRA_MACOS_BUILD_CACHE_ROOT"),
    ):
        assert cache_variable in text
        assert "RUNNER_TOOL_CACHE" in text
        assert "TAIRA_NATIVE_BUILD_JOBS" in text
        assert "CARGO_BUILD_JOBS=1" not in text
        assert "cargo-target" in text
        assert "cache_key=" in text
        assert (
            "$source_manifest-$TAIRA_INPUT_VALIDATOR_RELEASE_REF-$rustc_identity"
            in text
        )
        assert "workspace_source_manifest_sha256" in text
        assert "rustc -Vv" in text
    assert "privacy-release-evidence,iroha-core-tests" in macos
    assert linux.count("CARGO_TARGET_DIR") >= 1
    assert macos.count("CARGO_TARGET_DIR") >= 1


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
    authenticated_tool_sudo = (
        'sudo -n /bin/test ! -d "$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"',
        'sudo -n /bin/test ! -L "$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"',
        'sudo -n /bin/test ! -e "$controller_candidate"',
        'sudo -n /bin/test ! -L "$controller_candidate"',
        "sudo -n /usr/bin/install -o root -g wheel -m 0555 "
        '"$built_controller" "$controller_candidate"',
        'sudo -n /usr/bin/shasum -a 256 "$controller_candidate"',
        'sudo -n /usr/bin/cmp "$built_controller" "$controller_candidate"',
        'sudo -n /bin/mv -f -- "$controller_candidate" '
        '"$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"',
        "sudo -n /usr/bin/shasum -a 256 "
        '"$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"',
        'sudo -n /usr/bin/cmp "$built_controller" '
        '"$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"',
        "sudo -n /usr/bin/env -i LANG=C LC_ALL=C PATH=/usr/bin:/bin "
        'TMPDIR=/private/var/tmp "$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH" '
        "qualify-host-v1",
    )
    for command in authenticated_tool_sudo:
        assert source.count(command) == 2, command
    for line in source.splitlines():
        if "sudo -n " in line:
            assert 'sudo -n "$TAIRA_CONTROLLER_COMMAND"' in line or any(
                command in line for command in authenticated_tool_sudo
            )
        if any(
            token in line
            for token in (
                '"$TAIRA_CONTROLLER_COMMAND" attest ',
                '"$TAIRA_CONTROLLER_COMMAND" inspect-handoff ',
                '"$TAIRA_CONTROLLER_COMMAND" run ',
            )
        ):
            assert 'sudo -n "$TAIRA_CONTROLLER_COMMAND"' in line
    assert source.count('EXPECTED_UID: "0"') == 10

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
        assert source.count(trust_arg) == 11, trust_arg
    assert source.count("--source-commit") >= 10
    assert source.count("inspect-handoff") >= 9
    assert source.count("--stage-name") == source.count("inspect-handoff")
    assert "artifact-handoff-sha256" in source
    assert "--artifact-handoff-sha256" in source
    assert "--expected-artifact-handoff-sha256" in source


def test_workflow_uses_preprovisioned_release_controller_and_exact_native_tool_controller() -> None:
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
        "macos-candidate-authority": (
            "linux-authority",
            "qualification-receipt",
            "privacy-v1-boi-artifacts",
        ),
        "linux-boi-qualification": ("candidate", "privacy-v1-boi-artifacts"),
        "macos-deploy": (
            "linux-authority",
            "candidate",
            "privacy-v1-boi-qualified",
            "macos-build",
        ),
        "macos-publish": ("candidate",),
    }
    for job_name, kinds in consumers.items():
        text = _job_text(jobs[job_name])
        assert text.count("actions/download-artifact@") == len(kinds)
        for kind in kinds:
            assert f"--expected-kind {kind}" in text
        assert text.count("--stage-name") == len(kinds)
        assert "staged_root" in text


def test_boi_cross_run_input_is_required_content_validated_and_deploy_gating() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    jobs = _workflow()
    candidate = _job_text(jobs["macos-candidate-authority"])
    qualification = _job_text(jobs["linux-boi-qualification"])

    assert "privacy_v1_boi_artifact_run_id:" in source
    assert source.count("run-id: ${{ inputs.privacy_v1_boi_artifact_run_id }}") == 2
    assert source.count(
        "privacy-v1-boi-artifacts-${{ github.sha }}-${{ inputs.validator_release_ref }}"
    ) == 2
    assert source.count(
        '[[ "$TAIRA_INPUT_BOI_ARTIFACT_RUN_ID" =~ ^[1-9][0-9]{0,19}$ ]]'
    ) == 2
    for text in (candidate, qualification):
        assert "--expected-kind privacy-v1-boi-artifacts" in text
        assert "staged_root" in text
    assert '--boi-artifact-handoff-dir "$boi_stage"' in source
    assert "assemble-boi --" in qualification
    assert "admit -- init-replay-ledger" not in qualification
    assert '--artifact-handoff-root "$boi_stage"' in source
    assert "--candidate-replay-ledger" not in qualification
    assert "TAIRA_BOI_QUALIFICATION_STAGING_ROOT" in qualification
    assert (
        'boi_output="$TAIRA_BOI_QUALIFICATION_STAGING_ROOT/'
        'boi-qualified-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"'
    ) in source
    assert "TAIRA_RELEASE_EXTERNAL_SIGNER_PATH" not in qualification
    assert "TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH" not in qualification
    assert "TAIRA_BOI_QUALIFICATION_EXTERNAL_SIGNER_PATH" in qualification
    assert "TAIRA_BOI_QUALIFICATION_EXTERNAL_SIGNER_SHA256" in qualification
    assert "TAIRA_BOI_QUALIFICATION_SIGNING_PUBLIC_KEY_PATH" in qualification
    assert "--trusted-qualification-signing-fingerprint" in qualification
    assert '--workflow-run-id "$GITHUB_RUN_ID"' in source
    assert '--workflow-run-attempt "$GITHUB_RUN_ATTEMPT"' in source
    assert "linux-boi-qualification" in jobs["macos-deploy"]["needs"]
    deploy = _job_text(jobs["macos-deploy"])
    assert "taira-privacy-v1-boi-${{ github.run_id }}-${{ github.run_attempt }}" in deploy
    assert "--expected-kind privacy-v1-boi-qualified" in deploy
    assert '--boi-qualified-handoff-root "$boi_stage"' in source
    assert "boi_stage" in deploy
    assert "--trusted-boi-qualification-public-key" in deploy
    assert "--trusted-boi-qualification-signing-fingerprint" in deploy
    assert "--expected-boi-qualification-host-id" in deploy
    assert "--expected-boi-qualification-installation-id" in deploy
    assert "--expected-boi-qualification-controller-digest" in deploy
    assert "--expected-workflow-run-id" in deploy
    assert "--expected-workflow-run-attempt" in deploy
    assert "iroha.taira.boi-native-isolation-broker.v1" in qualification
    assert "iroha.taira.boi-authenticated-run-nonce.v1" in qualification
    assert "iroha.taira.deploy-authenticated-run-nonce.v1" in deploy
    assert "iroha.taira.complete-source-identity-attestation.v1" in qualification
    assert "iroha.taira.complete-source-identity-attestation.v1" in deploy
    assert "TAIRA_PRIVACY_V1_BOI_ARTIFACT_PATH" not in source


def test_authority_host_attestations_are_distinct_and_cannot_be_reused() -> None:
    source = WORKFLOW.read_text(encoding="utf-8")
    for variable in (
        "TAIRA_PUBLIC_INPUT_HOST_ID",
        "TAIRA_LINUX_AUTHORITY_HOST_ID",
        "TAIRA_QUALIFICATION_HOST_ID",
        "TAIRA_CANDIDATE_AUTHORITY_HOST_ID",
        "TAIRA_BOI_QUALIFICATION_HOST_ID",
        "TAIRA_DEPLOY_HOST_ID",
        "TAIRA_PUBLISH_HOST_ID",
        "TAIRA_PUBLIC_INPUT_INSTALLATION_ID",
        "TAIRA_LINUX_AUTHORITY_INSTALLATION_ID",
        "TAIRA_QUALIFICATION_INSTALLATION_ID",
        "TAIRA_CANDIDATE_AUTHORITY_INSTALLATION_ID",
        "TAIRA_BOI_QUALIFICATION_INSTALLATION_ID",
        "TAIRA_DEPLOY_INSTALLATION_ID",
        "TAIRA_PUBLISH_INSTALLATION_ID",
    ):
        assert f"vars.{variable}" in source
    assert "--role macos-qualification" in source
    assert "--role macos-candidate-authority" in source
    assert "--role linux-boi-qualification" in source
    assert "--role macos-deploy" in source
    assert "--role macos-publish" in source


def test_deploy_workflow_rejects_weak_or_incomplete_apply_reports() -> None:
    deploy = "\n".join(
        str(step.get("run", "")) for step in _steps(_workflow()["macos-deploy"])
    )
    for token in (
        '"deployment_completed_at_unix_ms"',
        '"config_set_sha256"',
        '"genesis_block_hash"',
        '"signed_genesis_sha256"',
        '"topology_sha256"',
        '"receipt_signers"',
        'range(1,5)',
        '"protocol_version":4',
        '"peer_count":4',
        'manifest.get("genesis_expected_hash")',
        'canonical_network_id(genesis)',
    ):
        assert token in deploy
    assert "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94" not in deploy
    assert 'int(after["end_block_hash"][-2:],16)&1' not in deploy
    assert "int(genesis[-2:],16)&1" not in deploy


def test_deploy_apply_report_gate_accepts_only_exact_bound_identity(
    tmp_path: Path,
) -> None:
    deploy = "\n".join(
        str(step.get("run", "")) for step in _steps(_workflow()["macos-deploy"])
    )
    marker = 'before,after,manifest=(json.load(open(path,encoding="ascii"))'
    marker_index = deploy.index(marker)
    start = deploy.rfind("import json,sys\n", 0, marker_index)
    script = deploy[start : deploy.index("\nPY\n", marker_index)]
    before = {"admission_receipt_id": "receipt", "applied": False}
    signer = {
        "binary_stat_seal": [1, 2, 3, 4, 5],
        "config_sha256": "1" * 64,
        "lifecycle_binding_sha256": "2" * 64,
        "node_id": "taira-node:receipt-signer:secp256k1:sha256:" + "3" * 64,
        "public_key": {"algorithm": "secp256k1", "payload_hex": "02" + "4" * 64},
        "runtime_binding_sha256": "5" * 64,
    }
    genesis_hash = "9" * 62 + "11"
    crc = 0xFFFF
    for byte in b"hash:" + genesis_hash.upper().encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    network_id = f"hash:{genesis_hash.upper()}#{crc:04X}"
    after = {
        "admission_receipt_id": "receipt",
        "applied": True,
        "binary_sha256": "6" * 64,
        "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
        "config_set_sha256": "7" * 64,
        "deployment_completed_at_unix_ms": 1,
        "end_block_hash": "8" * 62 + "11",
        "end_height": 2,
        "genesis_block_hash": genesis_hash,
        "network_id": network_id,
        "network_name": "taira",
        "peer_count": 4,
        "protocol_version": 4,
        "receipt_signers": {
            f"taira-validator-{index}": {
                **signer,
                "node_id": signer["node_id"][:-1] + str(index),
                "public_key": {
                    **signer["public_key"],
                    "payload_hex": "02" + f"{index:x}" * 64,
                },
            }
            for index in range(1, 5)
        },
        "restart_proof": "passed",
        "signed_genesis_sha256": "a" * 64,
        "start_height": 1,
        "supervisor_sha256": "b" * 64,
        "topology_sha256": "c" * 64,
    }
    before_path = tmp_path / "before.json"
    after_path = tmp_path / "after.json"
    manifest_path = tmp_path / "reset-manifest.json"
    before_path.write_text(json.dumps(before), encoding="ascii")
    manifest_path.write_text(
        json.dumps(
            {
                "schema": "taira-exact2f-reset-bundle",
                "chain_id": after["chain_id"],
                "genesis_expected_hash": genesis_hash,
                "signed_genesis_sha256": after["signed_genesis_sha256"],
            }
        ),
        encoding="ascii",
    )

    def run(value: dict[str, object]) -> subprocess.CompletedProcess[str]:
        after_path.write_text(json.dumps(value), encoding="ascii")
        return subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                "-",
                str(before_path),
                str(after_path),
                str(manifest_path),
            ],
            input=script,
            check=False,
            capture_output=True,
            text=True,
        )

    assert run(after).returncode == 0
    for field, invalid in (
        ("deployment_completed_at_unix_ms", 0),
        ("end_height", 1),
        ("genesis_block_hash", "8" * 64),
        ("network_id", "hash:" + "8" * 64 + "#0000"),
        ("protocol_version", True),
        ("topology_sha256", "C" * 64),
    ):
        mutated = json.loads(json.dumps(after))
        mutated[field] = invalid
        assert run(mutated).returncode != 0, field
    mutated = json.loads(json.dumps(after))
    mutated["receipt_signers"]["taira-validator-4"]["public_key"] = dict(
        mutated["receipt_signers"]["taira-validator-1"]["public_key"]
    )
    assert run(mutated).returncode != 0


def _publisher_step(source: str | None = None) -> dict[str, object]:
    jobs = _workflow(source)
    steps = _steps(jobs["macos-publish"])
    assert [step.get("name") for step in steps] == [
        "Download candidate bytes into publication quarantine",
        "Publish through the sealed installed authority controller",
        "Upload the exact root-closed publication and public-soak handoffs",
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
        "build-public-soak-candidate --",
        '--output "$candidate_prerequisite"',
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
        "build-public-soak-publication --",
        '--candidate-handoff "$candidate_prerequisite/public-soak-prerequisite-v1.json"',
        '--publication-root "$final"',
        '--output "$publication_prerequisite"',
        "public-soak prerequisite handoff identity differs",
        "public-soak prerequisite file identity differs",
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
    assert run.index("build-public-soak-candidate --") < run.index(
        "publish-rollout --"
    ) < run.index("build-public-soak-publication --")


def test_root_closed_publication_upload_is_exactly_receipt_derived() -> None:
    jobs = _workflow()
    upload = _steps(jobs["macos-publish"])[2]
    assert upload.get("uses") == (
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
    )
    with_values = upload.get("with")
    assert isinstance(with_values, dict)
    assert with_values["path"].splitlines() == [
        "${{ vars.TAIRA_PUBLISH_HANDOFF_ROOT }}/publication-receipt-"
        "${{ steps.publish.outputs.receipt_id }}/",
        "${{ vars.TAIRA_PUBLISH_HANDOFF_ROOT }}/public-soak-candidate-"
        "${{ github.run_id }}-${{ github.run_attempt }}/",
        "${{ vars.TAIRA_PUBLISH_HANDOFF_ROOT }}/public-soak-publication-"
        "${{ github.run_id }}-${{ github.run_attempt }}/",
    ]
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
