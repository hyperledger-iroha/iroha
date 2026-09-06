//! State-level regression for abandoned authenticated-history mint attempts.

use super::*;

#[test]
fn mint_fold_abandon_preserves_credit_and_requires_a_fresh_bound_attempt() {
    let artifacts = crate::zk::kagemusha_v1_recursion::tests::artifacts();
    let suite_id = snapshot_digest(b"snapshot-suite", 1);
    let vk_digest = snapshot_digest(b"snapshot-verifier-set", 2);
    let governance_key = SigningKey::from_bytes((&[8; 32]).into()).expect("governance key");
    let profile = snapshot_hardware_profile(suite_id, &governance_key);
    let enabled_profile = KagemushaEnabledProfileV1 {
        hardware_profile: profile,
        hardware_profile_id: profile.hardware_profile_id,
        suite_id,
        vk_digest,
        qualification_digest: snapshot_digest(b"snapshot-qualification-matrix", 3),
        policy_epoch: profile.policy_epoch,
        qualification_report: KagemushaEvidenceFileV1 {
            sha256: profile.qualification_report_digest,
            byte_len: 1,
        },
    };
    let proof_release =
        KagemushaStateProofReleaseV1::from_test_artifacts(artifacts, vec![enabled_profile])
            .expect("snapshot-test proof release");
    let payment_context =
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request;
    let lane = KagemushaLaneIdV1 {
        network_id: payment_context.network_id,
        device_lane_id: snapshot_digest(b"snapshot-lane", 4),
        asset: payment_context.asset.clone(),
        scale: payment_context.scale,
    };
    let old_epoch = HardwareEpochV1 {
        generation: 7,
        epoch_id: snapshot_digest(b"snapshot-old-epoch", 5),
    };
    let old_device_key = SigningKey::from_bytes((&[17; 32]).into()).expect("old device key");
    let old_credential = snapshot_hardware_credential(
        lane.network_id,
        lane.device_lane_id,
        old_epoch,
        &profile,
        suite_id,
        &old_device_key,
        &governance_key,
    );
    let old_policy = DevicePolicyBindingV1 {
        device_key_reference: old_credential.device_key_reference,
        hardware_policy_id: snapshot_digest(b"snapshot-old-policy", 7),
    };
    let context = KagemushaStateContextV1 {
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id,
        vk_digest,
        release_id: artifacts.release_id,
        asset_incarnation: payment_context.asset_incarnation,
        hardware_profile_id: profile.hardware_profile_id,
        policy_epoch: profile.policy_epoch,
    };
    let liability_pool_id = derive_liability_pool_id(&lane, payment_context.asset_incarnation)
        .expect("snapshot-test liability pool");
    let state = KagemushaStateV1::build(
        context,
        liability_pool_id,
        lane.clone(),
        0,
        0,
        old_epoch,
        old_policy,
        snapshot_digest(b"snapshot-old-state-nonce", 8),
        ExactConsumedCreditIndex::empty().root(),
    )
    .expect("snapshot-test old-epoch state");
    let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
    )
    .expect("empty authenticated history");
    let mut machine = KagemushaStateMachineV1 {
        state,
        journal_revision: 0,
        inbox_revision: 0,
        pending_credits: BTreeMap::new(),
        accepted_recipient_bindings: BTreeSet::from([old_policy]),
        accepted_payment_receipts: BTreeMap::new(),
        mint_inbox: KagemushaMintInboxV1::default(),
        consumed_credits: ExactConsumedCreditIndex::empty(),
        authenticated_history,
        receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(32 * 1024 * 1024),
        sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(8 * 1024 * 1024),
        outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
        proof_release: proof_release.clone(),
        recursive_verifier: AcceptSnapshotRecursiveVerifierV1,
        guard_verifier: AcceptSnapshotGuardVerifierV1,
    };

    let mint_amount = 4;
    let (mint_authorization, mint_credit, mint_opening) = snapshot_mint_credit(
        machine.state(),
        artifacts,
        &old_credential,
        payment_context.recipient.clone(),
        mint_amount,
    );
    let recipient_key_handle_binding = snapshot_digest(b"snapshot-mint-key-handle", 9);
    let reserved_inbox_bytes = MintInboxReservationV1::required_reservation_bytes(
        &mint_authorization,
        &old_credential,
        &mint_opening,
        recipient_key_handle_binding,
    )
    .expect("snapshot-test mint reservation size");
    let mint_reservation = MintInboxReservationV1::new(
        mint_authorization.clone(),
        old_credential,
        mint_opening,
        recipient_key_handle_binding,
        reserved_inbox_bytes,
    )
    .expect("snapshot-test mint reservation");
    let reservation_statement = machine
        .preview_mint_reservation(&mint_reservation)
        .expect("preview old-epoch mint reservation");
    machine
        .reserve_mint_credit(
            &mint_reservation,
            &MintReservationCertificateV1 {
                statement: reservation_statement,
                guard_bundle: vec![0x91],
            },
        )
        .expect("reserve old-epoch mint inbox capacity");
    let verified_mint = VerifiedMintStageV1::for_tests(mint_reservation, mint_credit.clone())
        .expect("test backend verified exact mint inputs");
    let mint_stage_statement = machine
        .preview_stage_mint_credit(&verified_mint, 200)
        .expect("preview old-epoch mint stage");
    machine
        .stage_mint_credit(
            &mint_authorization,
            &mint_credit,
            Some(&verified_mint),
            Some(&MintStageCertificateV1 {
                statement: mint_stage_statement,
                guard_bundle: vec![0x92],
            }),
        )
        .expect("stage finalized old-epoch mint");
    assert_eq!(machine.inbox_revision(), 2);

    let first_nonce = snapshot_digest(b"history-mint-first-attempt", 21);
    let first = machine
        .preview_mint_fold(&mint_credit, first_nonce, 300)
        .expect("first valid mint attempt");
    let first_id = first.authenticated_history_transaction.transaction_id();
    let first_usage = machine.authenticated_history.overlay_usage().live_bytes();
    let exact_retry = machine
        .preview_mint_fold(&mint_credit, first_nonce, 300)
        .expect("exact retry keeps its prepared attempt");
    assert_eq!(
        exact_retry
            .authenticated_history_transaction
            .transaction_id(),
        first_id
    );
    assert_eq!(
        machine.authenticated_history.overlay_usage().live_bytes(),
        first_usage
    );
    let first_signing_bytes = machine
        .mint_fold_history_root_selection_signing_bytes(&first)
        .unwrap();
    let first_signature = snapshot_device_signature(&old_device_key, &first_signing_bytes);
    let authorization_for = |preview: &CreditFoldPreviewV1| {
        TransitionAuthorizationV1::new(
            HardwareTransitionCertificateV1 {
                statement: preview.transition.hardware_statement.clone(),
                guard_bundle: vec![0x95],
            },
            snapshot_paired_proof(
                preview.transition.transport_semantic_digest,
                artifacts.eq_protocol_digest,
                artifacts.ep_protocol_digest,
                0xA8,
            ),
        )
    };
    machine
        .abandon_mint_fold_preview(&first)
        .expect("abandon only this uncommitted attempt");
    assert_eq!(
        machine.authenticated_history.overlay_usage().live_bytes(),
        0
    );
    assert!(matches!(
        machine.preview_mint_fold(&mint_credit, first_nonce, 300),
        Err(KagemushaStateErrorV1::StateInvariant)
    ));

    assert_eq!(
        machine.mint_fold_history_root_selection_signing_bytes(&first),
        Err(KagemushaStateErrorV1::AuthenticatedHistoryAttemptNotPrepared(first_id))
    );
    assert!(
        matches!(machine.authorize_mint_fold_history(&first, authorization_for(&first), &snapshot_device_public_key(&old_device_key), first_signature), Err(KagemushaStateErrorV1::AuthenticatedHistoryAttemptNotPrepared(id)) if id == first_id)
    );
    machine
        .abandon_mint_fold_preview(&first)
        .expect("abandon retry remains idempotent");

    let fresh = machine
        .preview_mint_fold(
            &mint_credit,
            snapshot_digest(b"history-mint-fresh-attempt", 22),
            301,
        )
        .expect("a fresh Core preview can still fold the same pending credit");
    assert_ne!(
        fresh.authenticated_history_transaction.transaction_id(),
        first_id
    );
    assert_eq!(
        fresh.authenticated_history_transaction.root_selection(),
        first.authenticated_history_transaction.root_selection()
    );
    machine
        .mint_fold_history_root_selection_signing_bytes(&fresh)
        .expect("fresh attempt is eligible for hardware authorization");

    let fresh_signing_bytes = machine
        .mint_fold_history_root_selection_signing_bytes(&fresh)
        .unwrap();
    machine
        .authorize_mint_fold_history(
            &fresh,
            authorization_for(&fresh),
            &snapshot_device_public_key(&old_device_key),
            snapshot_device_signature(&old_device_key, &fresh_signing_bytes),
        )
        .expect("fresh prepared attempt can receive initial hardware authorization");

    let mut swapped = fresh.clone();
    swapped.authenticated_history_transaction = first.authenticated_history_transaction.clone();
    swapped.proof_root_bridge_request = first.proof_root_bridge_request;
    assert!(matches!(
        machine.mint_fold_history_root_selection_signing_bytes(&swapped),
        Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable)
    ));
    assert_eq!(machine.state().balance, 0);
    assert_eq!(machine.pending_credit_count(), 1);
    assert_eq!(
        machine
            .authenticated_history
            .abort_prepared(first_id)
            .unwrap(),
        KagemushaHistoryAbortOutcomeV1::AlreadyAborted
    );
}
