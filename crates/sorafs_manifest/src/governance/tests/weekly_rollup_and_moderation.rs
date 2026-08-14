// Appeal-finance weekly-rollup and moderation-governance regressions.
#[test]
fn appeal_finance_weekly_rollup_enforces_exact_size_and_source_boundaries() {
    let rollup = max_source_appeal_finance_weekly_rollup();
    rollup
        .validate()
        .expect("exact source-report boundary validates");
    let exact = rollup
        .encoded_len_exact()
        .expect("appeal finance weekly rollup exact length");
    assert_eq!(
        preflight_appeal_finance_weekly_rollup_len(&rollup, exact)
            .expect("exact weekly rollup boundary"),
        exact
    );
    assert!(matches!(
        preflight_appeal_finance_weekly_rollup_len(&rollup, exact - 1),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::PayloadTooLarge {
                found,
                maximum,
            }
        ) if found == exact && maximum == exact - 1
    ));
    let mut too_many_declared = rollup.clone();
    too_many_declared.report_count += 1;
    assert!(matches!(
        too_many_declared.validate(),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::TooManySourceReports {
                found,
                maximum: SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1,
            }
        ) if found
            == u64::try_from(SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1)
                .expect("source-report ceiling fits u64")
                + 1
    ));
    let mut too_many_ids = rollup;
    too_many_ids.source_report_ids.push(
        (u128::try_from(SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1)
            .expect("source-report ceiling fits u128")
            + 1)
        .to_be_bytes(),
    );
    assert!(matches!(
        too_many_ids.validate(),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::TooManySourceReportIds {
                found,
                maximum: SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1,
            }
        ) if found == SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1 + 1
    ));
}
#[test]
fn appeal_finance_weekly_rollup_enforces_config_and_outcome_boundaries() {
    let first = sample_appeal_finance_report();
    let second = second_appeal_finance_report();
    let cycle = PorReportIsoWeek {
        year: 2026,
        week: 26,
    };
    let mut config_rollup =
        SoraFsAppealFinanceWeeklyRollupV1::from_reports(cycle, 1_800_000_100_000, &[first, second])
            .expect("baseline weekly rollup");
    config_rollup.appeal_finance_config_versions = (0
        ..SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1)
        .map(|index| format!("config-{index:03}"))
        .collect();
    config_rollup
        .validate()
        .expect("exact config-version boundary validates");
    let mut too_many_configs = config_rollup;
    too_many_configs
        .appeal_finance_config_versions
        .push(format!(
            "config-{:03}",
            SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1
        ));
    assert!(matches!(
        too_many_configs.validate(),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::TooManyConfigVersions {
                found,
                maximum: SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1,
            }
        ) if found == SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1 + 1
    ));
    let outcomes = [
        SoraFsAppealFinanceOutcomeV1::Uphold,
        SoraFsAppealFinanceOutcomeV1::Overturn,
        SoraFsAppealFinanceOutcomeV1::Modify,
        SoraFsAppealFinanceOutcomeV1::WithdrawnBeforePanel,
        SoraFsAppealFinanceOutcomeV1::WithdrawnAfterPanel,
        SoraFsAppealFinanceOutcomeV1::Frivolous,
        SoraFsAppealFinanceOutcomeV1::Escalated,
    ];
    let reports: Vec<_> = outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| {
            let mut report = sample_appeal_finance_report();
            report.report_id = u128::try_from(index + 1)
                .expect("fixture index fits u128")
                .to_be_bytes();
            report.case_id = format!("case-{index}");
            report.outcome = outcome;
            report
        })
        .collect();
    let mut outcome_rollup =
        SoraFsAppealFinanceWeeklyRollupV1::from_reports(cycle, 1_800_000_100_000, &reports)
            .expect("seven-outcome rollup");
    assert_eq!(
        outcome_rollup.outcomes.len(),
        SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1
    );
    outcome_rollup
        .validate()
        .expect("exact outcome boundary validates");
    outcome_rollup
        .outcomes
        .push(outcome_rollup.outcomes.last().expect("outcome").clone());
    assert!(matches!(
        outcome_rollup.validate(),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::TooManyOutcomes {
                found,
                maximum: SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1,
            }
        ) if found == SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1 + 1
    ));
}
#[test]
fn appeal_finance_weekly_rollup_rejects_duplicate_report_ids() {
    let report = sample_appeal_finance_report();
    let cycle = PorReportIsoWeek {
        year: 2026,
        week: 26,
    };
    let err = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        cycle,
        1_800_000_100_000,
        &[report.clone(), report.clone()],
    )
    .expect_err("duplicate report rejected");
    assert_eq!(
        err,
        SoraFsAppealFinanceWeeklyRollupBuildError::DuplicateReportId {
            report_id: report.report_id,
        }
    );
}
#[test]
fn appeal_finance_weekly_rollup_build_rejects_quantity_overflow() {
    let mut first = sample_appeal_finance_report();
    first.deposit_xor =
        "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("maximum positive quantity");
    let mut second = second_appeal_finance_report();
    second.deposit_xor = "1".parse().expect("canonical quantity");
    let err = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        PorReportIsoWeek {
            year: 2026,
            week: 26,
        },
        1_800_000_100_000,
        &[first, second.clone()],
    )
    .expect_err("overflowing report totals must fail closed");
    assert!(matches!(
        err,
        SoraFsAppealFinanceWeeklyRollupBuildError::AmountOverflow {
            report_id,
            field: "deposit_xor",
            ..
        } if report_id == second.report_id
    ));
}
#[test]
fn appeal_finance_weekly_rollup_validation_rejects_outcome_overflow() {
    let first = sample_appeal_finance_report();
    let second = second_appeal_finance_report();
    let mut rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        PorReportIsoWeek {
            year: 2026,
            week: 26,
        },
        1_800_000_100_000,
        &[first, second],
    )
    .expect("baseline rollup");
    rollup.outcomes[0].total_deposit_xor =
        "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("maximum positive quantity");
    rollup.outcomes[1].total_deposit_xor = "1".parse().expect("canonical quantity");
    assert!(matches!(
        rollup.validate(),
        Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::AmountOverflow {
                field: "outcomes.total_deposit_xor",
                ..
            }
        )
    ));
}
#[test]
fn moderation_ballot_event_rejects_inconsistent_tally() {
    let event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 6,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
        generated_at_unix_ms: 1_800_000_030_000,
        case_id: "case-42".to_string(),
        round_id: "round-1".to_string(),
        juror_id: None,
        committed_count: 2,
        revealed_count: 2,
        challenge_count: 0,
        tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
            case_id: "case-42".to_string(),
            round_id: "round-2".to_string(),
            counts: SoraFsModerationVoteCountsV1 {
                uphold: 1,
                overturn: 1,
                modify: 0,
                escalate: 0,
            },
            votes_total: 2,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
            contested: false,
            tallied_at_unix_ms: 1_800_000_030_000,
        }),
        challenge: None,
    };
    assert!(matches!(
        event.validate(),
        Err(SoraFsModerationBallotGovernanceEventValidationError::TallyRoundMismatch { .. })
    ));
}
#[test]
fn moderation_ballot_event_accepts_challenge_lifecycle_events() {
    let submitted_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
        challenge_id: "challenge-1".to_owned(),
        case_id: "case-42".to_owned(),
        round_id: "round-1".to_owned(),
        challenger_id: "moderation-provider".to_owned(),
        kind: SoraFsModerationBallotGovernanceChallengeKindV1::EvidenceMismatch,
        target_juror_id: None,
        evidence_digest: [0x42; 32],
        reason: "payload-free-evidence-digest".to_owned(),
        raised_at_unix_ms: 1_800_000_011_000,
        decision: None,
        resolved_by: None,
        resolved_at_unix_ms: None,
        resolution_note: None,
    };
    let submitted = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 3,
        kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted,
        generated_at_unix_ms: 1_800_000_011_000,
        case_id: "case-42".to_owned(),
        round_id: "round-1".to_owned(),
        juror_id: None,
        committed_count: 2,
        revealed_count: 0,
        challenge_count: 1,
        tally: None,
        challenge: Some(submitted_challenge.clone()),
    };
    submitted
        .validate()
        .expect("submitted challenge event validates");
    let resolved_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
        decision: Some(SoraFsModerationBallotGovernanceChallengeDecisionV1::Rejected),
        resolved_by: Some("moderation-operator".to_owned()),
        resolved_at_unix_ms: Some(1_800_000_012_000),
        resolution_note: Some("packet matches ballot".to_owned()),
        ..submitted_challenge
    };
    let resolved = SoraFsModerationBallotGovernanceEventV1 {
        kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved,
        generated_at_unix_ms: 1_800_000_012_000,
        challenge: Some(resolved_challenge),
        ..submitted
    };
    resolved
        .validate()
        .expect("resolved challenge event validates");
}
#[test]
fn moderation_challenge_enforces_account_and_public_text_boundaries() {
    let case_id = "c".repeat(SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1);
    let round_id = "r".repeat(SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1);
    let challenge = SoraFsModerationBallotGovernanceChallengeV1 {
        challenge_id: "h".repeat(SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1),
        case_id: case_id.clone(),
        round_id: round_id.clone(),
        challenger_id: "a".repeat(SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1),
        kind: SoraFsModerationBallotGovernanceChallengeKindV1::EvidenceMismatch,
        target_juror_id: Some("t".repeat(SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1)),
        evidence_digest: [0x42; 32],
        reason: "x".repeat(SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1),
        raised_at_unix_ms: 1_800_000_011_000,
        decision: None,
        resolved_by: None,
        resolved_at_unix_ms: None,
        resolution_note: None,
    };
    let event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 3,
        kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted,
        generated_at_unix_ms: 1_800_000_011_000,
        case_id,
        round_id,
        juror_id: None,
        committed_count: 2,
        revealed_count: 0,
        challenge_count: 1,
        tally: None,
        challenge: Some(challenge),
    };
    event.validate().expect("bounded challenge validates");
    let mut long_reason = event.clone();
    long_reason
        .challenge
        .as_mut()
        .expect("challenge")
        .reason
        .push('x');
    assert!(matches!(
        long_reason.validate(),
        Err(
            SoraFsModerationBallotGovernanceEventValidationError::InvalidBoundedText {
                field: "challenge.reason",
                found,
                maximum: SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1,
            }
        ) if found == SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1 + 1
    ));
    let mut long_account = event;
    long_account
        .challenge
        .as_mut()
        .expect("challenge")
        .challenger_id
        .push('a');
    assert!(matches!(
        long_account.validate(),
        Err(
            SoraFsModerationBallotGovernanceEventValidationError::InvalidBoundedText {
                field: "challenge.challenger_id",
                found,
                maximum: SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1,
            }
        ) if found == SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1 + 1
    ));
}
#[test]
fn moderation_ballot_event_rejects_invalid_challenge_payloads() {
    let base_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
        challenge_id: "challenge-1".to_owned(),
        case_id: "case-42".to_owned(),
        round_id: "round-1".to_owned(),
        challenger_id: "moderation-provider".to_owned(),
        kind: SoraFsModerationBallotGovernanceChallengeKindV1::DuplicateCommit,
        target_juror_id: None,
        evidence_digest: [0x42; 32],
        reason: "payload-free-evidence-digest".to_owned(),
        raised_at_unix_ms: 1_800_000_011_000,
        decision: None,
        resolved_by: None,
        resolved_at_unix_ms: None,
        resolution_note: None,
    };
    let event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 3,
        kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted,
        generated_at_unix_ms: 1_800_000_011_000,
        case_id: "case-42".to_owned(),
        round_id: "round-1".to_owned(),
        juror_id: None,
        committed_count: 2,
        revealed_count: 0,
        challenge_count: 1,
        tally: None,
        challenge: Some(base_challenge),
    };
    assert!(matches!(
        event.validate(),
        Err(SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeTarget)
    ));
    let mut resolved = event;
    let challenge = resolved.challenge.as_mut().expect("challenge");
    challenge.kind = SoraFsModerationBallotGovernanceChallengeKindV1::EvidenceMismatch;
    challenge.decision = Some(SoraFsModerationBallotGovernanceChallengeDecisionV1::Accepted);
    resolved.kind = SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved;
    assert!(matches!(
        resolved.validate(),
        Err(SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeResolver)
    ));
}
