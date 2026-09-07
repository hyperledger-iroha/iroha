// Canonical framed identity and persistence regression tests.

#[test]
fn billing_identity_signatures_and_checkpoint_ignore_ambient_norito_layout() {
    let root = tempfile::tempdir().expect("state root");
    let (service, feed_policy, reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    settle_first_period(&service, &reference);
    let policy = service_policy();
    let checkpoint = service.state.lock().expect("state").checkpoint.clone();
    let canonical = norito::encode_canonical(&checkpoint).expect("canonical checkpoint");
    let policy_bytes = norito::encode_canonical(&policy).expect("canonical policy");
    let mut policy_hasher = blake3::Hasher::new();
    policy_hasher.update(POLICY_DIGEST_DOMAIN_V1);
    policy_hasher.update(
        &u64::try_from(policy_bytes.len())
            .expect("policy length")
            .to_le_bytes(),
    );
    policy_hasher.update(&policy_bytes);
    let policy_digest = *policy_hasher.finalize().as_bytes();
    let stored = checkpoint.statements.first().expect("settled statement");
    let signed = stored.signed_statement.as_ref().expect("signature");
    let signed_bytes = norito::encode_canonical(signed).expect("canonical signed statement");
    let signed_digest = *blake3::hash(&signed_bytes).as_bytes();
    let receipt = stored
        .publication_receipt
        .as_ref()
        .expect("publication receipt");
    let close = checkpoint.period_closes.first().expect("period close");
    let mut substituted = signed.clone();
    substituted.signed_at_unix += 1;
    let mut forged = signed.clone();
    forged.signature[0] ^= 0x80;
    let mut distinct_layout = false;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::to_bytes(&checkpoint).expect("ambient checkpoint");
        distinct_layout |= before != canonical;
        assert_eq!(
            policy.canonical_bytes().expect("policy bytes"),
            policy_bytes
        );
        assert_eq!(
            policy.canonical_digest().expect("policy digest"),
            policy_digest
        );
        assert_eq!(
            encode_checkpoint(&checkpoint, &policy, &feed_policy).expect("checkpoint"),
            canonical
        );
        assert_eq!(
            decode_checkpoint(&canonical, &policy, &feed_policy).expect("canonical recovery"),
            checkpoint
        );
        if before != canonical {
            assert_eq!(
                decode_checkpoint(&before, &policy, &feed_policy),
                Err(HedgingBillingServiceError::NonCanonicalCheckpoint)
            );
        }
        assert_eq!(
            signed
                .canonical_bytes(&policy, &feed_policy, close)
                .expect("verified statement"),
            signed_bytes
        );
        assert_eq!(
            signed_statement_digest(signed).expect("signed identity"),
            signed_digest
        );
        receipt
            .validate(signed, &policy.statement_publisher)
            .expect("same signed receipt");
        assert!(substituted.verify(&policy, &feed_policy, close).is_err());
        assert!(forged.verify(&policy, &feed_policy, close).is_err());
        assert_eq!(
            norito::to_bytes(&checkpoint).expect("restored layout"),
            before
        );
    }
    assert!(distinct_layout);
}

#[test]
fn billing_checkpoint_exact_byte_ceiling_ignores_ambient_norito_layout() {
    let root = tempfile::tempdir().expect("state root");
    let (service, feed_policy, _reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    let source_limit = BILLING_SOURCE_ID_MAX_BYTES_V1.min(MAX_HEDGING_IDENTIFIER_BYTES - "storage:".len());
    for first_sequence in [1, 65] {
        let events = (first_sequence..first_sequence + 64)
            .map(|sequence| {
                let prefix = format!("bounded:{sequence}:");
                let source = format!("{prefix}{}", "x".repeat(source_limit - prefix.len()));
                event(sequence, &source, "1")
            })
            .collect();
        service
            .ingest_finalized_page(&page(events))
            .expect("admit bounded finalized billing page");
    }
    let checkpoint = service.state.lock().expect("state").checkpoint.clone();
    let expected_length = norito::encode_canonical(&checkpoint)
        .expect("independent full frame")
        .len();
    assert!(u64::try_from(expected_length).unwrap() > HEDGING_BILLING_MIN_CHECKPOINT_BYTES_V1);
    for deficit in [0, 1] {
        let mut policy = service_policy();
        policy.checkpoint_max_bytes = u64::try_from(expected_length - deficit).unwrap();
        policy
            .validate()
            .expect("ceiling remains a legal production policy");
        let mut candidate = checkpoint.clone();
        candidate.policy_digest = policy
            .canonical_digest()
            .expect("bind chosen policy ceiling");
        candidate
            .validate(&policy, &feed_policy)
            .expect("valid checkpoint before size admission");
        let expected = norito::encode_canonical(&candidate).expect("independent canonical frame");
        assert_eq!(
            expected.len(),
            expected_length,
            "fixed-width policy digest preserves the frame size"
        );
        for flags in 0..=u8::MAX {
            if norito::core::validate_header_flags(flags).is_err() {
                continue;
            }
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let encoded = encode_checkpoint(&candidate, &policy, &feed_policy);
            if deficit == 0 {
                assert_eq!(encoded.expect("exact full-frame ceiling"), expected);
            } else {
                assert_eq!(encoded, Err(HedgingBillingServiceError::ResourceExhausted));
            }
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }
}

#[test]
fn epoch_witness_has_one_bounded_canonical_persistence_format() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, reference, verifier, _publisher, _acknowledgement_authority) =
        ready_service(root.path());
    settle_first_period(&service, &reference);
    let (next_policy, next_feed_policy) = rotated_policies();
    service
        .transition_epoch(
            next_policy.clone(),
            next_feed_policy,
            vec![0xD8],
            &TestSigner::transition(),
        )
        .expect("transition");
    let record = verifier
        .witness_records
        .lock()
        .expect("epoch witness state")
        .get(&1)
        .expect("epoch witness")
        .clone();
    let bytes = record
        .to_canonical_bytes(next_policy.checkpoint_max_bytes)
        .expect("canonical witness bytes");
    assert_eq!(
        HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
            &bytes,
            next_policy.checkpoint_max_bytes,
        )
        .expect("decode canonical witness"),
        record
    );
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            record
                .to_canonical_bytes(next_policy.checkpoint_max_bytes)
                .expect("same witness bytes"),
            bytes
        );
        assert_eq!(
            HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
                &bytes,
                next_policy.checkpoint_max_bytes
            )
            .expect("same witness recovery"),
            record
        );
        let alternate = norito::to_bytes(&record).expect("ambient witness");
        if alternate != bytes {
            assert!(
                HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
                    &alternate,
                    next_policy.checkpoint_max_bytes
                )
                .is_err()
            );
        }
    }
    let mut substituted = record;
    substituted.revision[0] ^= 0x80;
    let substituted_bytes = norito::to_bytes(&substituted).expect("substituted bytes");
    assert!(matches!(
        HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
            &substituted_bytes,
            next_policy.checkpoint_max_bytes,
        ),
        Err(HedgingBillingServiceError::InvalidEpochWitness)
    ));
}
