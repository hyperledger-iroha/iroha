// Same-scope moderation regressions extracted to keep the parent source budget bounded.
#[test]
fn private_pop_proof_sortition_and_activation_reject_adversarial_inputs() {
    let mut fixture = PanelFixture::new();
    fixture.submit(1, 0, 1);
    let appeal = fixture.appeal();
    let juror = fixture.juror_id();
    let outsider = fixture.outsider_id();
    let mut wrong_root = proof_for_appeal(&appeal);
    wrong_root.commitment_root[0] ^= 1;
    assert!(
        fixture
            .run(1_002_000, |transaction| {
                RegisterSorafsModerationJurorEligibility::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    encode(&wrong_root),
                )
                .execute(&juror, transaction)
            })
            .is_err()
    );
    assert_eq!(fixture.appeal().eligible_jurors.len(), 0);
    fixture.register_juror();
    let proof = proof_for_appeal(&fixture.appeal());
    assert!(
        fixture
            .run(1_002_001, |transaction| {
                RegisterSorafsModerationJurorEligibility::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    encode(&proof),
                )
                .execute(&outsider, transaction)
            })
            .is_err()
    );
    assert!(
        FindSorafsModerationJurorEligibility::new(
            "panel-case".to_owned(),
            "round-1".to_owned(),
            outsider.clone(),
        )
        .execute(&fixture.state.view())
        .is_err()
    );
    let snapshot_digest = fixture.appeal().pop_snapshot_digest;
    let manager = fixture.manager_id();
    let appellant = fixture.appellant_id();
    assert!(
        fixture
            .run(1_003_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    vec![juror.clone()],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    vec![juror.clone()],
                    Vec::new(),
                )
                .execute(&appellant, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    [0; 32],
                    vec![juror.clone()],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    [0xFF; 32],
                    vec![juror.clone()],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    vec![juror.clone()],
                    Vec::new(),
                )
                .execute(&outsider, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    vec![outsider.clone()],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    vec![juror.clone(), juror.clone()],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert_eq!(
        fixture.appeal().status,
        ModerationAppealStatusV1::RegisteringJurors
    );
    let sortition_digest = fixture.finalize_single_juror_sortition();
    assert!(
        fixture
            .run(1_004_001, |transaction| {
                RegisterSorafsModerationJurorEligibility::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    encode(&proof),
                )
                .execute(&outsider, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_001, |transaction| {
                AcceptSorafsModerationJurorAssignment::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&outsider, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_004_001, |transaction| {
                AcceptSorafsModerationJurorAssignment::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    [0xFF; 32],
                )
                .execute(&juror, transaction)
            })
            .is_err()
    );
    fixture
        .run(1_004_001, |transaction| {
            AcceptSorafsModerationJurorAssignment::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&juror, transaction)
        })
        .unwrap();
    assert!(
        fixture
            .run(1_004_002, |transaction| {
                AcceptSorafsModerationJurorAssignment::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&juror, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_005_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_006_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    [0xFE; 32],
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_006_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&outsider, transaction)
            })
            .is_err()
    );
    assert!(
        fixture
            .run(1_006_000, |transaction| {
                let mut status = status_for_mutation(transaction.world(), 1_006_000)?;
                status.open_cases = u64::MAX;
                transaction
                    .world
                    .smart_contract_state
                    .insert(status_key().clone(), encode_status(&status)?);
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    assert_eq!(
        fixture.appeal().status,
        ModerationAppealStatusV1::AwaitingAcceptance
    );
    assert!(
        FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .is_err()
    );
    fixture
        .run(1_006_000, |transaction| {
            ActivateSorafsModerationCase::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    let appeal = fixture.appeal();
    assert_eq!(appeal.status, ModerationAppealStatusV1::BallotOpen);
    assert!(appeal.replacements.is_empty());
    let case = FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
        .execute(&fixture.state.view())
        .unwrap();
    assert_eq!(case.spec.jurors, vec![juror]);
    assert_eq!(
        case.spec.context.panel_roster_hash,
        sorafs_moderation_panel_roster_hash_v1(&case.spec.jurors, 1)
    );
}
#[test]
fn insufficient_pool_and_no_show_failover_exhaustion_are_terminal() {
    let mut insufficient = PanelFixture::new();
    insufficient.submit(1, 0, 1);
    let snapshot_digest = insufficient.appeal().pop_snapshot_digest;
    let manager = insufficient.manager_id();
    insufficient
        .run(1_004_000, |transaction| {
            FinalizeSorafsModerationSortition::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                snapshot_digest,
                latest_parent_randomness_anchor(transaction)?,
                Vec::new(),
                Vec::new(),
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    assert_eq!(
        insufficient.appeal().status,
        ModerationAppealStatusV1::InsufficientEligiblePool
    );
    assert!(
        FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
            .execute(&insufficient.state.view())
            .is_err()
    );
    assert!(
        insufficient
            .run(1_004_001, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    latest_parent_randomness_anchor(transaction)?,
                    Vec::new(),
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    let mut no_show = PanelFixture::new();
    no_show.submit(1, 0, 1);
    no_show.register_juror();
    let sortition_digest = no_show.finalize_single_juror_sortition();
    let manager = no_show.manager_id();
    no_show
        .run(1_006_000, |transaction| {
            ActivateSorafsModerationCase::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    assert_eq!(
        no_show.appeal().status,
        ModerationAppealStatusV1::FailoverExhausted
    );
    assert!(
        FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
            .execute(&no_show.state.view())
            .is_err()
    );
    assert!(
        no_show
            .run(1_006_001, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .is_err()
    );
    let status = FindSorafsModerationStatus
        .execute(&no_show.state.view())
        .unwrap();
    assert_eq!(status.failed_panel_formations, 1);
    assert_eq!(status.open_cases, 0);
}
#[test]
fn primary_no_show_uses_next_unique_waitlist_juror_atomically() {
    let mut fixture = PanelFixture::new();
    let intake = panel_intake(&fixture.appellant, "panel-case", 1, 1, 1, 0x92);
    let manager = fixture.manager_id();
    let appellant = fixture.appellant_id();
    fixture
        .run(1_001_000, |transaction| {
            SubmitSorafsModerationAppeal::new(intake).execute(&appellant, transaction)
        })
        .unwrap();
    let mut appeal = fixture.appeal();
    let juror = fixture.juror_id();
    let outsider = fixture.outsider_id();
    let records = [
        ModerationJurorEligibilityRecordV1 {
            case_id: "panel-case".to_owned(),
            round_id: "round-1".to_owned(),
            juror: juror.clone(),
            eligibility_class: ModerationJurorEligibilityClassV1::General,
            proof_digest: [0xA1; 32],
            nullifier: [0xB1; 32],
            pop_snapshot_digest: appeal.pop_snapshot_digest,
            credential_expires_at_epoch: 2_000,
            registered_at_unix_ms: 1_002_000,
        },
        ModerationJurorEligibilityRecordV1 {
            case_id: "panel-case".to_owned(),
            round_id: "round-1".to_owned(),
            juror: outsider.clone(),
            eligibility_class: ModerationJurorEligibilityClassV1::General,
            proof_digest: [0xA2; 32],
            nullifier: [0xB2; 32],
            pop_snapshot_digest: appeal.pop_snapshot_digest,
            credential_expires_at_epoch: 2_000,
            registered_at_unix_ms: 1_002_000,
        },
    ];
    appeal.eligible_jurors = vec![juror, outsider];
    appeal.eligible_jurors.sort_by_key(ToString::to_string);
    fixture
        .run(1_002_000, |transaction| {
            let mut status = status_for_mutation(transaction.world(), 1_002_000)?;
            status.eligibility_proofs = 2;
            status.updated_at_unix_ms = 1_002_000;
            transaction.world.smart_contract_state.insert(
                appeal_key("panel-case", "round-1"),
                encode_state(&appeal, "synthetic verified appeal")?,
            );
            for record in &records {
                let encoded = encode_state(record, "synthetic verified eligibility")?;
                transaction.world.smart_contract_state.insert(
                    eligibility_key("panel-case", "round-1", &record.juror),
                    encoded.clone(),
                );
                transaction
                    .world
                    .smart_contract_state
                    .insert(nullifier_key(record.nullifier), encoded);
            }
            transaction
                .world
                .smart_contract_state
                .insert(status_key().clone(), encode_status(&status)?);
            Ok(())
        })
        .unwrap();
    let snapshot_digest = appeal.pop_snapshot_digest;
    let mut expected_selection = None;
    fixture
        .run(1_004_000, |transaction| {
            let randomness_anchor = latest_parent_randomness_anchor(transaction)?;
            let (expected_jurors, expected_waitlist, _, _) = sorafs_moderation_select_panel_v1(
                appeal.intake_digest,
                appeal.pop_snapshot_digest,
                randomness_anchor,
                &records,
                1,
                1,
                1,
            )
            .map_err(|error| corrupt_state(format!("fixture sortition failed: {error}")))?;
            expected_selection = Some((expected_jurors.clone(), expected_waitlist.clone()));
            FinalizeSorafsModerationSortition::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                snapshot_digest,
                randomness_anchor,
                expected_jurors.clone(),
                expected_waitlist.clone(),
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    let (expected_jurors, expected_waitlist) =
        expected_selection.expect("fixture captured deterministic selection");
    let sortition_digest = fixture
        .appeal()
        .selection
        .expect("selection")
        .sortition_digest;
    fixture
        .run(1_006_000, |transaction| {
            ActivateSorafsModerationCase::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    let appeal = fixture.appeal();
    assert_eq!(appeal.status, ModerationAppealStatusV1::BallotOpen);
    assert_eq!(appeal.replacements.len(), 1);
    assert_eq!(appeal.replacements[0].absent_juror, expected_jurors[0]);
    assert_eq!(
        appeal.replacements[0].replacement_juror,
        expected_waitlist[0]
    );
    let case = FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
        .execute(&fixture.state.view())
        .unwrap();
    assert_eq!(case.spec.jurors, expected_waitlist);
    assert_eq!(
        FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .unwrap()
            .failover_replacements,
        1
    );
}
#[test]
fn later_pop_revocation_rotation_does_not_rewrite_or_brick_admitted_snapshot() {
    let mut fixture = PanelFixture::new();
    fixture.submit(1, 0, 1);
    let manager = fixture.manager_id();
    let material = shared_pop_material();
    let entries = vec![PopRevocationEntryV1 {
        nonce: material.credential.revocation_nonce,
        revoked_at_epoch: 1_001,
        reason: PopRevocationReasonV1::GovernanceSuspension,
    }];
    let revocation_root = pop_revocation_root_v1(&entries).expect("rotated revocation root");
    let publication = sign_pop_revocations(
        PopRevocationListV1 {
            version: POP_REVOCATION_LIST_VERSION_V1,
            list_version: 2,
            commitment_root: material.root.root_digest,
            revocation_root,
            revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch: 1_001,
            entries,
            publisher_signature: empty_pop_signature(&fixture.manager),
        },
        &fixture.manager,
    );
    let issuer_policy_digest = pop_policy(&fixture.manager)
        .digest()
        .expect("policy digest");
    fixture
        .run(1_001_500, |transaction| {
            PublishSorafsPopRevocationList::new(encode(&publication), issuer_policy_digest)
                .execute(&manager, transaction)
        })
        .unwrap();
    let proof = proof_for_appeal(&fixture.appeal());
    let juror = fixture.juror_id();
    fixture
        .run(1_002_000, |transaction| {
            RegisterSorafsModerationJurorEligibility::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                encode(&proof),
            )
            .execute(&juror, transaction)
        })
        .unwrap();
    let admitted = fixture.appeal();
    assert_eq!(admitted.eligible_jurors, vec![juror.clone()]);
    assert_eq!(admitted.pop_snapshot.revocation_list_version, 1);
    assert_eq!(admitted.pop_snapshot.registry_audit_sequence, 2);
    let sortition_digest = fixture.finalize_single_juror_sortition();
    fixture
        .run(1_004_001, |transaction| {
            AcceptSorafsModerationJurorAssignment::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&juror, transaction)
        })
        .unwrap();
    fixture
        .run(1_006_000, |transaction| {
            ActivateSorafsModerationCase::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )
            .execute(&manager, transaction)
        })
        .unwrap();
    assert_eq!(
        fixture.appeal().status,
        ModerationAppealStatusV1::BallotOpen
    );
    assert_eq!(
        FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .unwrap()
            .eligibility_proofs,
        1
    );
}
#[test]
fn committed_event_pages_resolve_final_hashes_and_enforce_exact_cursors() {
    let mut fixture = Fixture::new(2);
    let juror0 = fixture.juror_id(0);
    let juror1 = fixture.juror_id(1);
    let reveal0 = reveal(
        &fixture.spec,
        &juror0,
        SoraFsModerationVoteChoice::Uphold,
        0x91,
    );
    let reveal1 = reveal(
        &fixture.spec,
        &juror1,
        SoraFsModerationVoteChoice::Overturn,
        0x92,
    );
    fixture
        .run(1_500, |transaction| {
            SubmitSorafsModerationCommit::new(encode(&commit(&reveal0)))
                .execute(&juror0, transaction)?;
            SubmitSorafsModerationCommit::new(encode(&commit(&reveal1)))
                .execute(&juror1, transaction)
        })
        .expect("commit two moderation events in one block");
    let view = fixture.state.view();
    let anchor = resolve_finalized_cursor(&view).expect("resolve finalized moderation anchor");
    assert_eq!(anchor.height, 2);
    let first = FindSorafsModerationEvents::new(anchor, None, 2)
        .execute(&view)
        .expect("read first committed-event page");
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(first.events[0].block_height, 1);
    assert_eq!(first.events[0].event_index, 0);
    assert_eq!(first.events[1].block_height, 2);
    assert_eq!(first.events[1].event_index, 0);
    assert_eq!(
        first.events[1].block_hash,
        *iroha_crypto::HashOf::new(&header(2, 1_500)).as_ref()
    );
    assert!(first.has_more);
    let continuation = first.next_after.expect("continuation cursor");
    let second = FindSorafsModerationEvents::new(anchor, Some(continuation), 2)
        .execute(&view)
        .expect("read second committed-event page");
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.events[0].sequence, 3);
    assert_eq!(second.events[0].block_height, 2);
    assert_eq!(second.events[0].event_index, 1);
    assert!(!second.has_more);
    assert!(second.next_after.is_none());
    let exhausted = FindSorafsModerationEvents::new(anchor, Some(second.events[0].cursor()), 2)
        .execute(&view)
        .expect("a cursor at the journal head yields an empty page");
    assert!(exhausted.events.is_empty());
    assert!(!exhausted.has_more);
    assert!(exhausted.next_after.is_none());
    let mut tampered_after = continuation;
    tampered_after.block_hash[0] ^= 0xFF;
    assert!(matches!(
        FindSorafsModerationEvents::new(anchor, Some(tampered_after), 2).execute(&view),
        Err(QueryExecutionFail::Expired)
    ));
    let mut tampered_anchor = anchor;
    tampered_anchor.block_hash[0] ^= 0xFF;
    assert!(matches!(
        FindSorafsModerationEvents::new(tampered_anchor, None, 2).execute(&view),
        Err(QueryExecutionFail::Expired)
    ));
    assert!(
        FindSorafsModerationEvents::new(anchor, None, 0)
            .execute(&view)
            .is_err()
    );
    assert!(
        FindSorafsModerationEvents::new(anchor, None, MODERATION_QUERY_MAX_EVENTS_V1 + 1)
            .execute(&view)
            .is_err()
    );
}
#[test]
fn snapshot_rebuilds_complete_chain_projection_in_logical_order() {
    let mut fixture = PanelFixture::new();
    let appellant = fixture.appellant_id();
    let z_intake = panel_intake(&fixture.appellant, "z-case", 1, 0, 1, 0x91);
    fixture
        .run(1_001_000, |transaction| {
            SubmitSorafsModerationAppeal::new(z_intake).execute(&appellant, transaction)
        })
        .expect("submit z appeal");
    let mut a_intake = panel_intake(&fixture.appellant, "a-case", 1, 0, 1, 0x92);
    a_intake.proof_token_digest = [0x35; 32];
    fixture
        .run(1_001_001, |transaction| {
            SubmitSorafsModerationAppeal::new(a_intake).execute(&appellant, transaction)
        })
        .expect("submit a appeal");
    let view = fixture.state.view();
    let snapshot = FindSorafsModerationSnapshot::new(8, 16)
        .execute(&view)
        .expect("rebuild complete finalized moderation snapshot");
    assert_eq!(snapshot.finalized_height, 3);
    assert_eq!(
        snapshot.finalized_at_unix_ms,
        view.latest_block()
            .expect("exact finalized block")
            .header()
            .creation_time_ms
    );
    assert_eq!(
        snapshot
            .appeals
            .iter()
            .map(|appeal| appeal.appeal.intake.case_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a-case", "z-case"]
    );
    assert!(snapshot.cases.is_empty());
    assert_eq!(snapshot.events.len(), 3);
    assert_eq!(
        snapshot
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(
        FindSorafsModerationSnapshot::new(1, 16)
            .execute(&view)
            .is_err(),
        "a complete snapshot must fail instead of truncating cases"
    );
    let appeal = snapshot.appeals[0].appeal.clone();
    drop(view);
    fixture.state.world.smart_contract_state.insert(
        digest_key(APPEAL_STATE_KEY_PREFIX, [0xEE; 32]),
        encode_state(&appeal, "corrupt duplicate appeal").expect("encode corrupt fixture"),
    );
    assert!(
        FindSorafsModerationSnapshot::new(8, 16)
            .execute(&fixture.state.view())
            .is_err(),
        "a mismatched persisted key must fail the complete projection"
    );
}
#[test]
fn snapshot_includes_all_eligibility_and_latest_typed_events() {
    let mut fixture = PanelFixture::new();
    fixture.submit(1, 0, 1);
    fixture.register_juror();
    fixture.finalize_single_juror_sortition();
    let snapshot = FindSorafsModerationSnapshot::new(8, 16)
        .execute(&fixture.state.view())
        .expect("rebuild eligibility-bearing moderation snapshot");
    assert_eq!(snapshot.appeals.len(), 1);
    assert_eq!(snapshot.appeals[0].eligibility.len(), 1);
    assert_eq!(snapshot.appeals[0].eligibility[0].juror, fixture.juror_id());
    assert_eq!(snapshot.events.len(), 4);
    assert_eq!(
        snapshot.events.last().map(|event| event.event.kind),
        Some(SorafsModerationLedgerEventKind::SortitionFinalized)
    );
    assert_eq!(
        snapshot.events.last().map(|event| event.block_height),
        Some(4)
    );
}
#[test]
fn journal_rejects_missing_committed_parent_and_orphan_records() {
    let manager_pair = keypair(0xA1);
    let manager = account(&manager_pair);
    let mut state = state(&[&manager_pair], &manager);
    transact(&mut state, 1, OPENED_AT, |transaction| {
        SetSorafsModerationPolicy::new(policy()).execute(&manager, transaction)
    })
    .expect("write initial policy event");
    let active = FindSorafsModerationPolicy
        .execute(&state.view())
        .expect("active policy");
    let mut next = policy();
    next.revision = 2;
    next.predecessor_policy_digest = Some(active.policy_digest);
    assert!(
        transact(&mut state, 2, OPENED_AT + 1, |transaction| {
            SetSorafsModerationPolicy::new(next).execute(&manager, transaction)
        })
        .is_err(),
        "journal append must derive height from the committed parent, not fabricate a block hash"
    );
    assert_eq!(
        FindSorafsModerationPolicy
            .execute(&state.view())
            .expect("policy rollback")
            .policy
            .revision,
        1
    );
    state.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header(1, OPENED_AT)));
    let (head, terminal) = {
        let view = state.view();
        let head = read_event_journal_head(view.world())
            .expect("read event head")
            .expect("event head");
        let terminal = read_persisted_event(view.world(), head.last_sequence)
            .expect("read terminal event")
            .expect("terminal event");
        (head, terminal)
    };
    let orphan = ModerationPersistedEventV1 {
        sequence: head.last_sequence + 2,
        target_block_height: terminal.target_block_height,
        event_index: terminal.event_index + 2,
        event: terminal.event,
    };
    state.world.smart_contract_state.insert(
        event_key(orphan.sequence),
        encode_state(&orphan, "orphan moderation event").expect("encode orphan event"),
    );
    let anchor =
        resolve_finalized_cursor(&state.view()).expect("resolve finalized moderation anchor");
    assert!(
        FindSorafsModerationEvents::new(anchor, None, 8)
            .execute(&state.view())
            .is_err(),
        "event records beyond the journal head must fail closed"
    );
}
#[test]
fn snapshot_budget_fails_closed_at_record_and_byte_ceilings() {
    let key = StatePath::from_str("moderation_budget_probe").expect("bounded probe key");
    let mut byte_budget = ModerationSnapshotReadBudget {
        records: 0,
        encoded_bytes: MODERATION_QUERY_MAX_SNAPSHOT_BYTES_V1,
    };
    assert!(byte_budget.charge(&key, &[0x01]).is_err());
    let mut record_budget = ModerationSnapshotReadBudget {
        records: MODERATION_QUERY_MAX_SNAPSHOT_RECORDS_V1,
        encoded_bytes: 0,
    };
    assert!(record_budget.charge(&key, &[]).is_err());
}
