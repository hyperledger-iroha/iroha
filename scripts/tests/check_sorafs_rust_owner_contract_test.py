"""Rust parent-module and regression ownership contracts for SoraFS release gates."""

from __future__ import annotations

import re
from pathlib import Path

from scripts.tests.sorafs_rollout_gate_source_support import read_source as read
from scripts.tests.state_source_bundle import read_rust_source_bundle


REPO_ROOT = Path(__file__).resolve().parents[2]
IROHA_CLI_SORAFS_RS = REPO_ROOT / "crates" / "iroha_cli" / "src" / "commands" / "sorafs.rs"


def test_sorafs_release_http_clients_do_not_follow_redirects() -> None:
    cli = read(IROHA_CLI_SORAFS_RS)
    run_impls = (
        "TransparencyExplorerCanaryArgs",
        "TransparencyPublicationCanaryArgs",
        "ModerationQuarantineNotificationsDeliverArgs",
        "ModerationQuarantineNotificationsCanaryArgs",
        "ModerationQuarantineOperatorCanaryArgs",
    )
    for name in run_impls:
        implementation = cli.split(f"impl Run for {name}", 1)[1].split("\n}", 1)[0]
        assert ".redirect(reqwest::redirect::Policy::none())" in implementation
    for name in (
        "fn moderation_quarantine_notifications_run_does_not_follow_cross_origin_redirects()",
        "fn sorafs_get_canary_runs_do_not_follow_cross_origin_redirects()",
    ):
        assert name in cli
    xtask_root = REPO_ROOT / "xtask" / "src"
    assert re.search(r"(?m)^mod sorafs;$", read(xtask_root / "main.rs"))
    xtask = read(xtask_root / "sorafs.rs")
    probe = xtask.split("fn probe_headers_via_http(", 1)[1].split("\n}", 1)[0]
    assert "Client::builder().redirect(reqwest::redirect::Policy::none())" in probe
    assert re.search(r"(?m)^#\[cfg\(test\)\]\nmod tests;$", xtask)
    xtask_tests = read(xtask_root / "sorafs" / "tests.rs")
    assert "fn gateway_probe_rejects_cross_origin_head_and_get_redirects()" in xtask_tests


def test_commit_reveal_authoritative_ledger_foundation_is_pinned() -> None:
    model = read(
        REPO_ROOT
        / "crates"
        / "iroha_data_model"
        / "src"
        / "sorafs"
        / "moderation_ledger.rs"
    )
    instructions = read(
        REPO_ROOT / "crates" / "iroha_data_model" / "src" / "isi" / "sorafs.rs"
    )
    query_root = REPO_ROOT / "crates" / "iroha_data_model" / "src" / "query"
    queries = read(query_root / "mod.rs") + read(query_root / "domain_queries.rs")
    core_root = REPO_ROOT / "crates" / "iroha_core" / "src" / "smartcontracts" / "isi"
    core_source = read(core_root / "sorafs_moderation.rs")
    assert re.search(r"(?m)^pub mod sorafs_moderation;$", read(core_root / "mod.rs"))
    test_module = re.search(
        r"(?ms)^#\[cfg\(test\)\]\nmod tests \{(?P<body>.*?)^\}", core_source
    )
    assert test_module is not None
    for component in (
        "sorafs_moderation_setup_and_commit_tests.rs",
        "sorafs_moderation_challenge_and_intake_tests.rs",
    ):
        assert f'include!("{component}");' in test_module.group("body")
    challenge_tests = read(core_root / "sorafs_moderation_challenge_and_intake_tests.rs")
    assert 'include!("sorafs/moderation_tail_tests.rs");' in challenge_tests
    core = read_rust_source_bundle(core_root / "sorafs_moderation.rs", root=REPO_ROOT)
    executor_permission = read(
        REPO_ROOT
        / "crates"
        / "iroha_executor_data_model"
        / "src"
        / "permission.rs"
    )

    for marker in (
        "pub struct ModerationLedgerPolicyV1",
        "pub struct ModerationAppealIntakeV1",
        "pub struct ModerationPoPRegistrySnapshotV1",
        "pub struct ModerationJurorEligibilityRecordV1",
        "pub struct ModerationPanelSelectionV1",
        "pub struct ModerationAppealRecordV1",
        "pub enum ModerationAppealStatusV1",
        "pub struct ModerationCaseRecordV1",
        "pub struct ModerationCommitRecordV1",
        "pub struct ModerationRevealRecordV1",
        "pub struct ModerationChallengeRecordV1",
        "pub struct ModerationOutcomeRecordV1",
        "pub struct ModerationNoShowRecordV1",
        "pub enum ModerationNoShowKindV1",
        "pub enum ModerationOutcomeKindV1",
        "pub fn sorafs_moderation_select_panel_v1",
        "pub fn sorafs_moderation_panel_roster_hash_v1",
    ):
        assert marker in model

    for instruction in (
        "SetSorafsModerationPolicy",
        "SubmitSorafsModerationAppeal",
        "RegisterSorafsModerationJurorEligibility",
        "FinalizeSorafsModerationSortition",
        "AcceptSorafsModerationJurorAssignment",
        "ActivateSorafsModerationCase",
        "SubmitSorafsModerationCommit",
        "RaiseSorafsModerationChallenge",
        "ResolveSorafsModerationChallenge",
        "SubmitSorafsModerationReveal",
        "FinalizeSorafsModerationCase",
    ):
        assert f"pub struct {instruction}" in instructions
        assert f"impl Execute for {instruction}" in core

    assert "pub struct OpenSorafsModerationCase" not in instructions
    assert "impl Execute for OpenSorafsModerationCase" not in core

    for query in (
        "FindSorafsModerationPolicy",
        "FindSorafsModerationAppeal",
        "FindSorafsModerationJurorEligibility",
        "FindSorafsModerationCase",
        "FindSorafsModerationCommit",
        "FindSorafsModerationReveal",
        "FindSorafsModerationChallenge",
        "FindSorafsModerationOutcome",
        "FindSorafsModerationNoShow",
        "FindSorafsModerationStatus",
    ):
        assert f"pub struct {query}" in queries
        assert f"impl ValidSingularQuery for {query}" in core

    assert "pub struct CanManageSorafsModeration" in executor_permission
    for adversarial_test in (
        "duplicate_wrong_authority_phase_and_mismatched_reveal_are_atomic",
        "accepted_challenge_blocks_reveal_and_closes_without_penalties",
        "rejected_challenge_unblocks_reveals_and_tied_quorum_is_contested",
        "missed_quorum_persists_distinct_no_show_penalties",
        "bounds_permissions_and_counter_overflow_reject_without_partial_case",
        "appeal_intake_is_authority_bound_replay_safe_and_transaction_atomic",
        "private_pop_proof_sortition_and_activation_reject_adversarial_inputs",
        "insufficient_pool_and_no_show_failover_exhaustion_are_terminal",
        "primary_no_show_uses_next_unique_waitlist_juror_atomically",
        "later_pop_revocation_rotation_does_not_rewrite_or_brick_admitted_snapshot",
        "unresolved_challenge_expires_permissionlessly_and_fails_open",
        "genesis_moderation_permission_bypass_matches_executor_policy",
    ):
        assert f"fn {adversarial_test}" in core
