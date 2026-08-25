"""Static contract for the authenticated operator-only privacy release gates."""

from __future__ import annotations

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = (
    ROOT
    / ".github"
    / "workflows"
    / "capture_taira_privacy_release_gate_evidence.yml"
)
RUNNER = ROOT / "ci" / "run_taira_privacy_release_gate.sh"
EXPECTED_TESTS = (
    "privacy_verifier::tests::"
    "zk_ams_production_dispatch_covers_batch_and_successor_provisioning",
    "privacy_release_evidence::tests::"
    "vega_action_api_binds_signs_and_rejects_transaction_proof_and_statement_drift",
    "privacy_release_evidence::tests::"
    "zk_ace_release_stages_exercise_the_activatable_profile",
    "privacy_release_evidence::tests::"
    "bootle_lantern_release_stage_exercises_one_shot_issuance_and_wire_rejection",
    "privacy_release_evidence::tests::"
    "zk_ams_corruption_stage_rejects_maximum_and_submaximum_wire_mutations",
    "privacy_engines::zk_ams::tests::"
    "complete_batch_admission_proves_verifies_and_fails_closed",
    "privacy_engines::pq_masp::stark::tests::"
    "full_domain_authorized_facade_roundtrip_and_adversarial_wires_fail_closed",
    "privacy_engines::ivm_private_note::stark::tests::"
    "full_domain_stark_roundtrip_and_adversarial_wires_fail_closed",
)


def _workflow() -> str:
    return WORKFLOW.read_text(encoding="utf-8")


def _runner() -> str:
    return RUNNER.read_text(encoding="utf-8")


def test_release_gate_corridor_is_manual_protected_and_immutable() -> None:
    workflow = _workflow()
    assert "workflow_dispatch:" in workflow
    for automatic_trigger in ("\n  push:", "\n  pull_request:", "\n  schedule:"):
        assert automatic_trigger not in workflow
    assert "environment: taira-privacy-release-gates" in workflow
    assert "permissions:\n  contents: read" in workflow
    assert "cancel-in-progress: false" in workflow
    assert "fail-fast: false" in workflow
    assert "max-parallel: 1" in workflow
    assert "timeout-minutes: 360" in workflow
    assert "if: always()" in workflow
    assert "if-no-files-found: error" in workflow


def test_dispatch_requires_every_source_identity_and_resource_ceiling() -> None:
    workflow = _workflow()
    for name in (
        "expected_commit",
        "expected_source_sha256",
        "expected_cargo_lock_sha256",
        "elapsed_ceiling_ms",
        "peak_rss_ceiling_bytes",
        "address_space_ceiling_bytes",
    ):
        match = re.search(
            rf"(?m)^      {name}:\n(?P<body>(?:        [^\n]*\n)+)", workflow
        )
        assert match is not None, name
        body = match.group("body")
        assert "required: true" in body
        assert "type: string" in body
    assert 'test "$TAIRA_INPUT_EXPECTED_COMMIT" = "$GITHUB_SHA"' in workflow
    assert 'test "$TAIRA_INPUT_EXPECTED_COMMIT" = "$WORKFLOW_SHA"' in workflow
    assert "scripts/compute_workspace_source_manifest.py" in workflow
    assert '"$TAIRA_INPUT_LOCK_SHA256" Cargo.lock | sha256sum -c -' in workflow


def test_source_and_toolchain_are_authenticated_by_protected_identity() -> None:
    workflow = _workflow()
    for protected in (
        "TAIRA_PRIVACY_IROHA_SIGNER_FINGERPRINT",
        "TAIRA_PRIVACY_IROHA_SIGNER_PRINCIPAL",
        "TAIRA_PRIVACY_IROHA_SIGNER_PUBLIC_KEY",
        "TAIRA_PRIVACY_RUST_SYSROOT_TREE_SHA256",
    ):
        assert f"${{{{ vars.{protected} }}}}" in workflow
    for required in (
        'namespaces="git"',
        "gpg.format=ssh",
        "gpg.minTrustLevel=fully",
        "verify-commit --raw",
        'Good \\"git\\" signature for',
        "hash_taira_rust_toolchain.py",
        "check_taira_privacy_native_host.sh",
        "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
        "host: aarch64-unknown-linux-gnu",
    ):
        assert required in workflow
    assert "actions/checkout@11d5960a326750d5838078e36cf38b85af677262" in workflow
    assert (
        "actions-rust-lang/setup-rust-toolchain@"
        "166cdcfd11aee3cb47222f9ddb555ce30ddb9659"
    ) in workflow
    assert "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02" in workflow


def test_exact_eight_ignored_tests_are_wired_without_skips() -> None:
    workflow = _workflow()
    runner = _runner()
    filters = tuple(re.findall(r"(?m)^            filter: (\S+)$", workflow))
    assert filters == EXPECTED_TESTS
    for test_name in EXPECTED_TESTS:
        assert runner.count(test_name) == 1
    assert runner.count("--ignored") == 1
    assert runner.count("--exact") == 1
    assert runner.count("--test-threads=1") == 1
    assert "--features privacy-release-evidence" in runner
    assert "--locked" in runner
    assert "--release" in runner


def test_runner_enforces_resources_and_emits_non_authoritative_evidence() -> None:
    runner = _runner()
    for required in (
        "ulimit -v",
        "/usr/bin/time -v",
        "timeout --signal=TERM --kill-after=60s",
        "peak_rss_bytes > peak_rss_ceiling_bytes",
        "elapsed_ms > elapsed_ceiling_ms",
        '"activation_readiness_authority": False',
        'entry["assurance"] != "unavailable"',
        "sha256sum -c SHA256SUMS",
        "git diff --quiet",
        "compute_workspace_source_manifest.py",
    ):
        assert required in runner
