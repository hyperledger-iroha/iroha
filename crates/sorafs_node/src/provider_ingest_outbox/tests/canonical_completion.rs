// The generic Box codec owns completion framing and retained-allocation accounting in V1.

fn populate_canonical_completion(outbox: &ProviderIngestOutbox) -> ([u8; 32], [u8; 32]) {
    let authorization = authorization(0x55, 7);
    let job_id = authorization.job_id();
    enqueue_and_store_local(outbox, &authorization, 100);
    let transaction = signed_completion(&authorization, 8, 8);
    let claim = claim_for_transaction(outbox, job_id, &transaction, 8, 102, cursor(8));
    let transaction_hash = outbox
        .store_completion_transaction(&claim, transaction)
        .expect("retain a real signed transaction and its exact signing context");
    begin_submission(outbox, job_id, transaction_hash, 103).expect("authorize exposure");
    outbox
        .mark_completion_submitted(job_id, transaction_hash)
        .expect("record durable submission");
    let completion = stored_completion(outbox, job_id);
    assert!(completion.signed_transaction.is_some());
    assert!(completion.signing_context.is_some());
    assert!(completion.signer_policy_owner.is_some());
    assert!(completion.signer_policy_floor.is_some());
    assert!(completion.finalized_authority_observation.is_some());
    assert!(completion.ever_exposed);
    assert_eq!(completion.transaction_hash, Some(transaction_hash));
    (job_id, transaction_hash)
}

#[test]
fn canonical_completion_checkpoint_preserves_populated_state_in_every_layout() {
    let mut baseline = None;
    let mut layouts = 0;
    let mut alternate_frames = 0;
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        layouts += 1;
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let directory = tempdir().expect("private checkpoint directory");
        let path = checkpoint_path(&directory);
        let outbox = ProviderIngestOutbox::open(&path, policy()).expect("new private outbox");
        let (job_id, transaction_hash) = populate_canonical_completion(&outbox);
        let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
        let completion = stored_completion(&outbox, job_id);
        let independent = norito::encode_canonical(&checkpoint).expect("canonical frame oracle");
        assert_eq!(
            encode_provider_ingest_checkpoint(&checkpoint, policy()).unwrap(),
            independent,
        );
        assert_eq!(fs::read(&path).unwrap(), independent);
        assert_eq!(
            decode_provider_ingest_checkpoint(&independent, policy()).unwrap(),
            checkpoint,
        );
        if let Some(baseline) = &baseline {
            assert_eq!(&independent, baseline, "caller layout {flags:#04x}");
        } else {
            baseline = Some(independent.clone());
        }

        outbox
            .mark_completion_submitted(job_id, transaction_hash)
            .expect("duplicate acknowledgement is idempotent");
        assert_eq!(
            outbox.mark_completion_not_submitted(job_id, transaction_hash, 0, cursor(8)),
            Err(ProviderIngestOutboxError::InvalidRuntimeTimestamp),
        );
        assert_eq!(outbox.state.lock().unwrap().checkpoint, checkpoint);
        assert_eq!(fs::read(&path).unwrap(), independent);
        drop(outbox);

        let restored = ProviderIngestOutbox::open(&path, policy()).expect("populated restart");
        assert_eq!(restored.state.lock().unwrap().checkpoint, checkpoint);
        assert_eq!(stored_completion(&restored, job_id), completion);
        restored
            .mark_completion_submitted(job_id, transaction_hash)
            .expect("restart retains exact acknowledgement replay");
        assert_eq!(fs::read(&path).unwrap(), independent);
        drop(restored);

        let alternate = norito::to_bytes(&checkpoint).unwrap();
        if alternate != independent {
            alternate_frames += 1;
            assert_eq!(
                norito::decode_from_bytes::<ProviderIngestOutboxCheckpointV1>(&alternate)
                    .expect("a genuine equivalent advertised layout"),
                checkpoint,
            );
            assert_eq!(
                decode_provider_ingest_checkpoint(&alternate, policy()),
                Err(ProviderIngestOutboxError::InvalidCheckpoint),
            );
            write_local_checkpoint_atomic_bounded(&path, &alternate, policy().checkpoint_max_bytes)
                .unwrap();
            assert!(matches!(
                ProviderIngestOutbox::open(&path, policy()),
                Err(ProviderIngestOutboxError::InvalidCheckpoint),
            ));
            assert_eq!(fs::read(&path).unwrap(), alternate);
            write_local_checkpoint_atomic_bounded(
                &path,
                &independent,
                policy().checkpoint_max_bytes,
            )
            .unwrap();
            assert_eq!(
                ProviderIngestOutbox::open(&path, policy())
                    .unwrap()
                    .state
                    .lock()
                    .unwrap()
                    .checkpoint,
                checkpoint,
            );
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(layouts, 10);
    assert!(alternate_frames > 0);
}

#[test]
fn canonical_completion_box_charges_exact_owned_allocation() {
    use norito::core::DecodeFromSlice as _;

    let outbox = ProviderIngestOutbox::in_memory(policy()).unwrap();
    let (job_id, _) = populate_canonical_completion(&outbox);
    let completion = stored_completion(&outbox, job_id);
    let boxed = Box::new(completion.clone());
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (bare, flags) = norito::codec::encode_with_header_flags(&boxed);
    let _encoded_layout = norito::core::DecodeFlagsGuard::enter(flags);
    let box_bytes = norito::core::owned_box_allocation_bytes::<StoredCompletionDeliveryV1>();
    assert_eq!(box_bytes, size_of::<StoredCompletionDeliveryV1>());
    assert!(box_bytes > 0);
    let broad = norito::canonical_decode_limits(bare.len());
    let limits_with_allocation = |allocation| {
        norito::DecodeLimits::new(
            broad.max_sequence_elements(),
            broad.max_field_bytes(),
            broad.max_total_elements(),
            allocation,
            broad.max_nesting_depth(),
        )
    };

    for offset in 0..align_of::<StoredCompletionDeliveryV1>() {
        let mut storage = vec![0; offset];
        storage.extend_from_slice(&bare);
        let encoded = &storage[offset..];
        let (length, prefix) = norito::core::inspect_len_from_slice(encoded).unwrap();
        assert_eq!(prefix.checked_add(length), Some(encoded.len()));
        let inner = &encoded[prefix..];

        // Decode the very same borrowed child address as Box<T> will use. This accounts for all
        // child buffers/alignment copies, leaving only the final owned Box allocation isolated.
        let (decoded, inner_usage) = norito::core::with_decode_limits_measured(broad, || {
            norito::core::decode_field_canonical::<StoredCompletionDeliveryV1>(inner)
        });
        let (decoded, consumed) = decoded.expect("populated inner record");
        assert_eq!(decoded, completion);
        assert_eq!(consumed, inner.len());
        let expected = inner_usage
            .total_allocated_bytes()
            .checked_add(box_bytes)
            .unwrap();
        let (decoded, boxed_usage) = norito::core::with_decode_limits_measured(broad, || {
            Box::<StoredCompletionDeliveryV1>::decode_from_slice(encoded)
        });
        let (decoded, consumed) = decoded.expect("generic owned completion");
        assert_eq!(decoded.as_ref(), &completion);
        assert_eq!(consumed, encoded.len());
        assert_eq!(boxed_usage.total_allocated_bytes(), expected);

        let decoded = norito::with_decode_limits_scope(limits_with_allocation(expected), || {
            Box::<StoredCompletionDeliveryV1>::decode_from_slice(encoded)
        })
        .expect("exact child-plus-Box allowance");
        assert_eq!(decoded.0.as_ref(), &completion);
        for insufficient in [inner_usage.total_allocated_bytes(), expected - 1] {
            let error =
                norito::with_decode_limits_scope(limits_with_allocation(insufficient), || {
                    Box::<StoredCompletionDeliveryV1>::decode_from_slice(encoded)
                })
                .expect_err("the complete Box allocation must be reserved before construction");
            assert!(
                matches!(
                    error,
                    norito::Error::TotalAllocationExceeded { attempted, limit }
                        if attempted == u64::try_from(expected).unwrap()
                            && limit == u64::try_from(insufficient).unwrap()
                ),
                "offset {offset}: {error:?}",
            );
        }
    }
}

#[test]
fn canonical_completion_box_rejects_prior_inner_layout_under_current_schema() {
    let outbox = ProviderIngestOutbox::in_memory(policy()).unwrap();
    let (job_id, _) = populate_canonical_completion(&outbox);
    let completion = stored_completion(&outbox, job_id);
    let boxed = Box::new(completion.clone());
    let canonical = norito::encode_canonical(&boxed).unwrap();
    assert_eq!(
        norito::decode_canonical::<Box<StoredCompletionDeliveryV1>>(&canonical)
            .unwrap()
            .as_ref(),
        &completion,
    );
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (inner, flags) = norito::codec::encode_with_header_flags(&completion);
    let prior_inner =
        norito::core::frame_bare_with_header_flags::<Box<StoredCompletionDeliveryV1>>(
            &inner, flags,
        )
        .expect("advertise the current Box schema around the rejected inner-only layout");
    let header = norito::core::Header::read(prior_inner.as_slice()).unwrap();
    assert_eq!(
        header.schema,
        <Box<StoredCompletionDeliveryV1> as norito::core::NoritoSerialize>::schema_hash(),
    );
    let error = norito::decode_canonical::<Box<StoredCompletionDeliveryV1>>(&prior_inner)
        .expect_err("V1 has exactly one owned completion layout");
    assert!(!matches!(error, norito::Error::SchemaMismatch), "{error:?}");
}
