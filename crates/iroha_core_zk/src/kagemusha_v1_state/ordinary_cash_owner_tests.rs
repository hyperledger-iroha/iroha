//! Pure transition/subject regressions; no fixture constructs a Native cash owner.
use super::*;
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

fn outgoing() -> (
    KagemushaStateV1,
    KagemushaStateV1,
    TransitionProofStatementV1,
) {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(300).unwrap();
    let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
        &enrollment,
        Arc::clone(&fixture.release),
    )
    .unwrap();
    let (_, bootstrap) = derive_preview(
        &floor,
        [43; 32],
        KagemushaDurableCapacityV1 {
            inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        },
    )
    .unwrap();
    let base = bootstrap.state;
    let before = KagemushaStateV1::build(
        base.context(),
        base.liability_pool_id,
        base.lane.clone(),
        100,
        (1_u128 << 80) + 5,
        (1_u128 << 90) + 7,
        base.hardware_epoch,
        base.device_policy_binding,
        [44; 32],
        base.consumed_credit_root,
    )
    .unwrap();
    let after = KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        75,
        before.logical_sequence + 1,
        before.secure_index + 1,
        before.hardware_epoch,
        before.device_policy_binding,
        [45; 32],
        before.consumed_credit_root,
    )
    .unwrap();
    let statement = TransitionProofStatementV1 {
        version: 1,
        protocol_version: 1,
        predecessor_suite_id: before.suite_id,
        predecessor_vk_digest: before.vk_digest,
        successor_suite_id: after.suite_id,
        successor_vk_digest: after.vk_digest,
        kind: KagemushaTransitionKindV1::SendSplit,
        amount: 25,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [46; 32],
        recipient_encryption_key_binding: [47; 32],
        lifecycle_binding_digest: [48; 32],
        prepared_transition_binding_digest: [49; 32],
        receive_credit_binding_digest: [0; 32],
        predecessor_release_id: before.release_id,
        release_id: after.release_id,
        asset_incarnation: before.asset_incarnation,
        liability_pool_id: before.liability_pool_id,
        hardware_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        lane: before.lane.clone(),
        predecessor_commitment: before.state_commitment,
        successor_commitment: after.state_commitment,
        predecessor_sequence: before.logical_sequence,
        successor_sequence: after.logical_sequence,
        predecessor_epoch: before.hardware_epoch,
        successor_epoch: after.hardware_epoch,
        predecessor_device_policy_binding: before.device_policy_binding,
        successor_device_policy_binding: after.device_policy_binding,
        predecessor_state_nonce_commitment: before.state_nonce_commitment,
        successor_state_nonce_commitment: after.state_nonce_commitment,
        journal_revision_before: 19,
        journal_revision_after: 20,
        effect_digest: [50; 32],
    };
    (before, after, statement)
}

#[test]
fn native_cash_rederives_subtraction_and_independent_full_indexes() {
    let (before, after, statement) = outgoing();
    require_outgoing(&before, &after, &statement, 19).unwrap();
    assert!(before.secure_index > u128::from(u64::MAX));
    assert!(before.logical_sequence > u128::from(u64::MAX));
    assert_ne!(before.logical_sequence, before.secure_index);
    assert!(require_outgoing(&before, &after, &statement, 18).is_err());
    let mut changed = after.clone();
    changed.secure_index = changed.logical_sequence;
    assert!(require_outgoing(&before, &changed, &statement, 19).is_err());
    let mut changed = after.clone();
    changed.balance += 1;
    assert!(require_outgoing(&before, &changed, &statement, 19).is_err());
    let mut changed = statement.clone();
    changed.amount = before.balance + 1;
    assert!(matches!(
        require_outgoing(&before, &after, &changed, 19),
        Err(KagemushaStateErrorV1::InsufficientBalance)
    ));
    let mut changed = statement.clone();
    changed.successor_sequence = changed.journal_revision_after;
    assert!(require_outgoing(&before, &after, &changed, 19).is_err());
}

#[test]
fn native_cash_rejects_substituted_complete_financial_statement_scope() {
    let (before, after, statement) = outgoing();
    for index in 0..12 {
        let mut changed = statement.clone();
        match index {
            0 => changed.predecessor_commitment[0] ^= 1,
            1 => changed.successor_commitment[0] ^= 1,
            2 => changed.predecessor_suite_id[0] ^= 1,
            3 => changed.successor_vk_digest[0] ^= 1,
            4 => changed.release_id[0] ^= 1,
            5 => changed.liability_pool_id[0] ^= 1,
            6 => changed.hardware_profile_id[0] ^= 1,
            7 => changed.policy_epoch += 1,
            8 => changed.lane.device_lane_id[0] ^= 1,
            9 => changed.successor_epoch.epoch_id[0] ^= 1,
            10 => changed.predecessor_state_nonce_commitment[0] ^= 1,
            _ => changed.journal_revision_after += 1,
        }
        assert!(
            require_outgoing(&before, &after, &changed, 19).is_err(),
            "field {index}"
        );
    }
}

#[test]
fn native_cash_preparation_subject_keeps_real_secure_index_and_full_state_sha() {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(300).unwrap();
    let (before, after, statement) = outgoing();
    let subject =
        preparation_subject(&before, &after, &statement, enrollment.app_credential()).unwrap();
    assert_eq!(subject.secure_index_before, before.secure_index);
    assert_eq!(subject.secure_index_after, after.secure_index);
    assert_eq!(
        subject.transition_statement_digest,
        statement.digest().unwrap()
    );
    assert_eq!(subject.candidate_envelope_digest, [0; 32]);
    assert_eq!(subject.terminal_body_commitment, [0; 32]);
    subject.canonical_prepare_signing_bytes().unwrap();
    assert!(subject.canonical_signing_bytes().is_err());
    let mut changed = statement;
    changed.prepared_transition_binding_digest[0] ^= 1;
    assert_ne!(
        subject.transition_statement_digest,
        changed.digest().unwrap()
    );
}

#[test]
fn native_cash_intent_frame_binds_complete_captured_financial_control_identity() {
    let control = CapturedFinancialControlIdentity {
        original_sha256: [57; 32],
        lower_ms: 100,
        upper_ms: 200,
    };
    let control_frame = norito::encode_canonical(&control).unwrap();
    let header = norito::core::Header::read(control_frame.as_slice()).unwrap();
    assert_eq!(
        header.schema,
        norito::core::schema_hash_for_name(
            "iroha_core::zk::kagemusha_v1_state::CapturedOrdinaryFinancialControlIdentityV1"
        )
    );
    let restored: CapturedFinancialControlIdentity = norito::decode_canonical_with_limits(
        &control_frame,
        norito::canonical_decode_limits(control_frame.len()),
    )
    .unwrap();
    assert_eq!(restored, control);
    let intent = |financial_control| Record::Intent {
        operation: [58; 32],
        nonce: [59; 32],
        predecessor: [60; 32],
        financial_control,
        preparation_clock: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [61; 32],
            signed_observations_original_digest: [62; 32],
            lower_at_ms: 100,
            upper_at_ms: 200,
        },
        reservation: KagemushaOutboxReservationV1 {
            reservation_id: [63; 32],
            operation_kind: KagemushaOperationKindV1::SendSplit,
            // A codec field value only; no Native reservation is constructed or admitted.
            reserved_outbox_bytes: 4096,
            issued_at_ms: 100,
            expires_at_ms: 300,
        },
    };
    let original_record = intent(control);
    let original = encode(&original_record, 16 * 1024).unwrap();
    assert_eq!(decode(&original, 16 * 1024).unwrap(), original_record);
    for field in 0..3 {
        let mut changed = control;
        match field {
            0 => changed.original_sha256[0] ^= 1,
            1 => changed.lower_ms += 1,
            _ => changed.upper_ms -= 1,
        }
        let changed_record = intent(changed);
        let changed_original = encode(&changed_record, 16 * 1024).unwrap();
        assert_ne!(
            changed_original, original,
            "financial control field {field}"
        );
        assert_eq!(
            decode(&changed_original, 16 * 1024).unwrap(),
            changed_record
        );
    }
    for length in 0..original.len() {
        assert!(decode(&original[..length], 16 * 1024).is_err());
    }
    let mut suffix = original.clone();
    suffix.push(0);
    assert!(decode(&suffix, 16 * 1024).is_err());
    let mut foreign_schema = original;
    foreign_schema[6] ^= 1;
    assert!(decode(&foreign_schema, 16 * 1024).is_err());
}

#[test]
fn cash_proving_history_incoming_selection_requires_its_own_retained_identity() {
    // Actual private operation selector with data-only identities. No Native cash owner,
    // captured FI decision, incoming approval or monetary/proof capability is constructed.
    let incoming = CapturedFinancialControlIdentity {
        original_sha256: [71; 32],
        lower_ms: 200,
        upper_ms: 201,
    };
    let outgoing = None;
    let terminal = None;
    // The old outgoing-only selection rejects this exact legitimate slot arrangement.
    assert_eq!(
        ProvingHistoryOperation::OutgoingApproval.select_financial_control_identity(
            outgoing,
            terminal,
            Some(incoming),
            None,
        ),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
    assert_eq!(
        ProvingHistoryOperation::IncomingApproval.select_financial_control_identity(
            outgoing,
            terminal,
            Some(incoming),
            None,
        ),
        Ok(incoming)
    );
    let old_source = CapturedFinancialControlIdentity {
        original_sha256: [72; 32],
        lower_ms: 100,
        upper_ms: 101,
    };
    let terminal = CapturedFinancialControlIdentity {
        original_sha256: [73; 32],
        lower_ms: 300,
        upper_ms: 301,
    };
    // Even when other retained operations exist, the complete incoming FI/bounds are
    // selected rather than an earlier source capture or another operation's identity.
    assert_eq!(
        ProvingHistoryOperation::IncomingApproval.select_financial_control_identity(
            Some(old_source),
            Some(terminal),
            Some(incoming),
            None,
        ),
        Ok(incoming)
    );
    assert_ne!(incoming, old_source);
    assert_ne!(incoming, terminal);
    assert_eq!(
        ProvingHistoryOperation::OutgoingApproval.select_financial_control_identity(
            Some(old_source),
            Some(terminal),
            Some(incoming),
            None,
        ),
        Ok(old_source)
    );
    assert_eq!(
        ProvingHistoryOperation::TerminalApproval.select_financial_control_identity(
            Some(old_source),
            Some(terminal),
            Some(incoming),
            None,
        ),
        Ok(terminal)
    );
}

#[test]
fn cash_proving_history_never_falls_back_to_another_operations_identity() {
    // A missing required operation slot remains a refusal even with three foreign captures.
    let identities = [
        CapturedFinancialControlIdentity {
            original_sha256: [74; 32],
            lower_ms: 100,
            upper_ms: 101,
        },
        CapturedFinancialControlIdentity {
            original_sha256: [75; 32],
            lower_ms: 200,
            upper_ms: 201,
        },
        CapturedFinancialControlIdentity {
            original_sha256: [76; 32],
            lower_ms: 300,
            upper_ms: 301,
        },
        CapturedFinancialControlIdentity {
            original_sha256: [77; 32],
            lower_ms: 400,
            upper_ms: 401,
        },
    ];
    for (missing, operation) in [
        ProvingHistoryOperation::OutgoingApproval,
        ProvingHistoryOperation::TerminalApproval,
        ProvingHistoryOperation::IncomingApproval,
        ProvingHistoryOperation::IncomingTerminal,
    ]
    .into_iter()
    .enumerate()
    {
        let mut slots = identities.map(Some);
        slots[missing] = None;
        assert_eq!(
            operation.select_financial_control_identity(slots[0], slots[1], slots[2], slots[3]),
            Err(KagemushaStateErrorV1::InvalidCandidateStage),
            "missing operation slot {missing}"
        );
        assert_eq!(
            operation.select_financial_control_identity(None, None, None, None),
            Err(KagemushaStateErrorV1::InvalidCandidateStage)
        );
    }
}

#[test]
fn cash_proving_history_incoming_terminal_uses_distinct_w1_identity() {
    // Actual private selector over synthetic DATA; no FI/Native/approval authority is constructed.
    let w2 = CapturedFinancialControlIdentity {
        original_sha256: [81; 32],
        lower_ms: 100,
        upper_ms: 101,
    };
    let w1 = CapturedFinancialControlIdentity {
        original_sha256: [82; 32],
        lower_ms: 200,
        upper_ms: 201,
    };
    assert_eq!(
        ProvingHistoryOperation::IncomingTerminal.select_financial_control_identity(
            Some(w2),
            Some(w2),
            Some(w2),
            Some(w1),
        ),
        Ok(w1)
    );
    assert_eq!(
        ProvingHistoryOperation::IncomingApproval.select_financial_control_identity(
            None,
            None,
            Some(w2),
            Some(w1),
        ),
        Ok(w2)
    );
    assert_eq!(
        ProvingHistoryOperation::IncomingTerminal.select_financial_control_identity(
            Some(w2),
            Some(w2),
            Some(w2),
            None,
        ),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
}

#[test]
fn native_cash_record_budget_counts_full_carriers_and_private_checkpoint() {
    let mib = 1024 * 1024u64;
    let selected = cash_record_payload_limit(9 * mib, 10 * mib, 118 * mib, 7 * mib).unwrap();
    assert_eq!(selected, 144 * mib + 128 * 1024);
    assert!(selected > 128 * mib);
    assert!(cash_record_payload_limit(1, 1, 1, 0).is_err());
    assert!(cash_record_payload_limit(u64::MAX, 1, 1, 1).is_err());
    assert!(cash_record_payload_limit(1, u64::MAX, 1, 1).is_err());
    assert!(cash_record_payload_limit(1, 1, u64::MAX, 1).is_err());
    assert!(cash_record_payload_limit(1, 1, 1, u64::MAX).is_err());
}
#[test]
fn native_cash_record_limit_rejects_complete_frame_before_encoding() {
    // Plain codec DATA only, no owner or signer. Count the actual complete canonical schema frame.
    let record = Record::Mint(MintRecord::ProvenRequest {
        operation: [0; 32],
        original: vec![0; 97],
    });
    let whole = u64::try_from(norito::canonical_frame_len(&record).unwrap()).unwrap();
    assert!(whole > 97);
    assert!(encode(&record, 97).is_err());
    assert!(encode(&record, whole - 1).is_err());
    let raw = encode(&record, whole).unwrap();
    assert_eq!(raw.len() as u64, whole);
    assert!(decode(&raw, whole - 1).is_err());
    assert_eq!(decode(&raw, whole).unwrap(), record);
}

#[test]
fn native_mint_funding_records_roundtrip_with_complete_canonical_frame_limits() {
    // Codec DATA only: these inert originals confer no funding, finality or Native custody.
    use super::mint_funding::MintFundingRecord;
    let rows = [
        MintFundingRecord::ConsentFence,
        MintFundingRecord::Consent([3; 64]),
        MintFundingRecord::PreDebit(vec![vec![1], vec![2, 3]]),
        MintFundingRecord::PreDebitInvoked,
        MintFundingRecord::Decision {
            signed: vec![1],
            clock: vec![2],
            control: vec![3],
            data: vec![4],
        },
        MintFundingRecord::NodeSubmission(vec![5]),
        MintFundingRecord::TransactionFence {
            clock: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [6; 32],
                signed_observations_original_digest: [7; 32],
                lower_at_ms: 10,
                upper_at_ms: 20,
            },
        },
        MintFundingRecord::TransactionOriginal {
            canonical: vec![8],
            wire: vec![9],
        },
        MintFundingRecord::TransactionDispatched,
        MintFundingRecord::FinalizedOriginal(vec![10]),
        MintFundingRecord::FinalizedAcknowledged,
    ];
    for row in rows {
        let record = Record::Mint(MintRecord::Funding(Box::new(row)));
        let whole = u64::try_from(norito::canonical_frame_len(&record).unwrap()).unwrap();
        assert!(encode(&record, whole - 1).is_err());
        let raw = encode(&record, whole).unwrap();
        assert_eq!(u64::try_from(raw.len()).unwrap(), whole);
        assert!(decode(&raw, whole - 1).is_err());
        assert_eq!(decode(&raw, whole).unwrap(), record);
        let mut trailing = raw;
        trailing.push(0);
        assert!(decode(&trailing, whole + 1).is_err());
    }
}
