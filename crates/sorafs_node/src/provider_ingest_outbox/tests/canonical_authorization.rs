// Typed canonical CID admission and bounded persisted authorization regressions.

fn authorization_with_root_bytes(
    cid: Vec<u8>,
) -> Result<FinalizedProviderIngestAuthorizationV1, ProviderIngestOutboxError> {
    FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        7,
        cursor(7).block_hash,
        [0x11; 32],
        [0x51; 32],
        [0x71; 32],
        cid,
        "sorafs.sf1@1.0.0".to_owned(),
        [0x81; 32],
        [0x91; 32],
        4_096,
    )
}

#[test]
fn canonical_authorization_factories_reject_every_noncanonical_root_cid() {
    let cid = ManifestRootCid::from_blake3_digest([0xB4; 32]).expect("canonical CID");
    let generic = authorization_with_root_bytes(cid.as_bytes().to_vec())
        .expect("canonical generic authorization");
    let context =
        FinalizedProviderIngestMusubiContextV1::new(network_id(0x43), ArchiveId::new([0x44; 32]))
            .expect("Musubi context");
    let musubi = authorization_with_musubi_context(&generic, context.clone());
    assert_eq!(generic.manifest_cid(), cid.as_bytes());
    assert_eq!(musubi.manifest_cid(), cid.as_bytes());
    assert_ne!(generic.job_id(), musubi.job_id());
    let mut invalid = vec![
        Vec::new(),
        cid.as_bytes()[..35].to_vec(),
        [cid.as_bytes().as_slice(), &[0x01]].concat(),
    ];
    for (index, byte) in [(0, 2), (1, 0x70), (2, 0x12), (3, 31)] {
        let mut altered = cid.as_bytes().to_vec();
        altered[index] = byte;
        invalid.push(altered);
    }
    let mut inert = cid.as_bytes().to_vec();
    inert[4..].fill(0);
    invalid.push(inert);
    for bytes in invalid {
        assert!(ManifestRootCid::try_from_slice(&bytes).is_err());
        assert_eq!(
            authorization_with_root_bytes(bytes.clone()),
            Err(ProviderIngestOutboxError::InvalidAuthorization)
        );
        assert_eq!(
            FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
                7,
                cursor(7).block_hash,
                [0x11; 32],
                [0x51; 32],
                [0x71; 32],
                bytes,
                "sorafs.sf1@1.0.0".to_owned(),
                [0x81; 32],
                [0x91; 32],
                4_096,
                context.clone(),
            ),
            Err(ProviderIngestOutboxError::InvalidAuthorization)
        );
    }
}

#[test]
fn canonical_authorization_checkpoint_recovers_exactly_under_every_valid_caller_layout() {
    let authorization = authorization(0x61, 7);
    let outbox = ProviderIngestOutbox::in_memory(policy()).expect("outbox");
    outbox
        .enqueue(authorization.clone())
        .expect("enqueue canonical authorization");
    let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
    let expected =
        encode_provider_ingest_checkpoint(&checkpoint, policy()).expect("canonical checkpoint");
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let bytes = encode_provider_ingest_checkpoint(&checkpoint, policy())
            .expect("stable canonical checkpoint");
        assert_eq!(bytes, expected, "caller flags {flags:#04x}");
        let recovered =
            decode_provider_ingest_checkpoint(&bytes, policy()).expect("exact checkpoint recovery");
        assert_eq!(recovered, checkpoint);
        assert_eq!(recovered.active[0].authorization, authorization);
    }
    let directory = tempdir().expect("checkpoint directory");
    let path = checkpoint_path(&directory);
    write_local_checkpoint_atomic_bounded(&path, &expected, policy().checkpoint_max_bytes)
        .expect("persist canonical checkpoint with the production private-file contract");
    let recovered =
        ProviderIngestOutbox::open(&path, policy()).expect("reopen canonical checkpoint");
    assert_eq!(
        recovered
            .authorization(authorization.job_id())
            .expect("retained authorization"),
        authorization
    );
    drop(recovered);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            ProviderIngestOutbox::open(&path, policy()),
            Err(ProviderIngestOutboxError::Checkpoint(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), expected);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            ProviderIngestOutbox::open(&path, policy())
                .unwrap()
                .authorization(authorization.job_id())
                .unwrap(),
            authorization
        );
    }
}

#[test]
fn canonical_authorization_rejects_malformed_typed_cid_before_checkpoint_installation() {
    use norito::codec::Encode as _;

    let authorization = authorization(0x61, 7);
    let outbox = ProviderIngestOutbox::in_memory(policy()).expect("outbox");
    outbox
        .enqueue(authorization.clone())
        .expect("enqueue canonical authorization");
    let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (payload, flags) = norito::codec::encode_with_header_flags(&checkpoint);
    let _field_layout = norito::core::DecodeFlagsGuard::enter(flags);
    let cid = *authorization.manifest_cid.as_bytes();
    let encoded_cid = cid.encode();
    assert_eq!(authorization.manifest_cid.encode(), encoded_cid);
    let positive = norito::core::frame_bare_with_header_flags::<ProviderIngestOutboxCheckpointV1>(
        &payload, flags,
    )
    .expect("positive current-schema checkpoint");
    assert_eq!(
        decode_provider_ingest_checkpoint(&positive, policy()).unwrap(),
        checkpoint
    );
    let offsets = payload
        .windows(encoded_cid.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == encoded_cid).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        offsets.len(),
        1,
        "one unambiguous canonical root in fixture"
    );
    for (index, byte) in [(0, 2), (1, 0x70), (2, 0x12), (3, 31)] {
        let mut altered_cid = cid;
        altered_cid[index] = byte;
        assert!(ManifestRootCid::new(altered_cid).is_err());
        let encoded_altered_cid = altered_cid.encode();
        assert_eq!(encoded_altered_cid.len(), encoded_cid.len());
        let mut malformed = payload.clone();
        malformed[offsets[0]..offsets[0] + encoded_cid.len()].copy_from_slice(&encoded_altered_cid);
        let bytes = norito::core::frame_bare_with_header_flags::<ProviderIngestOutboxCheckpointV1>(
            &malformed, flags,
        )
        .expect("current schema with exact checksum");
        let error = norito::decode_canonical::<ProviderIngestOutboxCheckpointV1>(&bytes)
            .expect_err("typed CID rejects invalid header");
        assert!(
            matches!(error, norito::Error::Message(ref message) if message.contains("CID")),
            "{error:?}"
        );
        assert_eq!(
            decode_provider_ingest_checkpoint(&bytes, policy()),
            Err(ProviderIngestOutboxError::InvalidCheckpoint)
        );
        let directory = tempdir().expect("checkpoint directory");
        let path = checkpoint_path(&directory);
        write_local_checkpoint_atomic_bounded(&path, &bytes, policy().checkpoint_max_bytes)
            .expect("persist malformed checkpoint with valid private-file metadata");
        assert!(matches!(
            ProviderIngestOutbox::open(&path, policy()),
            Err(ProviderIngestOutboxError::InvalidCheckpoint)
        ));
    }
}

#[derive(NoritoSerialize)]
struct RejectedVectorCidAuthorization {
    job_id: [u8; 32],
    admission_finalized_cursor: ProviderIngestFinalizedCursorV1,
    provider_id: [u8; 32],
    order_id: [u8; 32],
    manifest_digest: [u8; 32],
    manifest_cid: Vec<u8>,
    chunker_handle: String,
    chunk_digest_sha3_256: [u8; 32],
    por_root: [u8; 32],
    content_length: u64,
    musubi_context: Option<FinalizedProviderIngestMusubiContextV1>,
}

#[test]
fn canonical_authorization_has_no_vector_cid_wire_fallback() {
    let current = authorization(0x61, 7);
    let rejected = RejectedVectorCidAuthorization {
        job_id: current.job_id,
        admission_finalized_cursor: current.admission_finalized_cursor,
        provider_id: current.provider_id,
        order_id: current.order_id,
        manifest_digest: current.manifest_digest,
        manifest_cid: current.manifest_cid().to_vec(),
        chunker_handle: current.chunker_handle.clone(),
        chunk_digest_sha3_256: current.chunk_digest_sha3_256,
        por_root: current.por_root,
        content_length: current.content_length,
        musubi_context: current.musubi_context.clone(),
    };
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let wire = norito::codec::encode_adaptive(&rejected);
    let error =
        norito::core::decode_field_canonical::<FinalizedProviderIngestAuthorizationV1>(&wire)
            .expect_err("vector CID field layout is retired");
    assert!(
        !matches!(error, norito::Error::SchemaMismatch),
        "exercise layout rather than unrelated schema"
    );
    assert!(
        norito::codec::decode_adaptive::<FinalizedProviderIngestAuthorizationV1>(&wire).is_err()
    );
    assert_eq!(
        norito::codec::decode_adaptive::<FinalizedProviderIngestAuthorizationV1>(
            &norito::codec::encode_adaptive(&current)
        )
        .unwrap(),
        current
    );
}

#[test]
fn canonical_authorization_seal_and_signing_frames_keep_exact_budgets_in_every_layout() {
    let authorization = authorization(0x61, 7);
    let outbox = ProviderIngestOutbox::in_memory(policy()).expect("outbox");
    outbox
        .enqueue(authorization.clone())
        .expect("canonical authorization");
    let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
    let checkpoint_bytes =
        encode_provider_ingest_checkpoint(&checkpoint, policy()).expect("canonical checkpoint");
    let sealed = ProviderIngestSealedCheckpointRecordV1::new(1, None, None, checkpoint_bytes);
    let sealed_bytes = sealed
        .to_canonical_bytes(policy().checkpoint_max_bytes)
        .expect("canonical sealed record");
    let transaction = signed_completion_at(&authorization, 8, cursor(7), 0x41);
    let context = completion_context(&transaction, 8, cursor(7));
    let expected_token =
        derive_signing_token(authorization.job_id(), 1, &context).expect("canonical signing token");
    assert_ne!(
        derive_signing_token(authorization.job_id(), 2, &context).unwrap(),
        expected_token
    );
    let payload_len = norito::encode_canonical(&context.expected_payload)
        .unwrap()
        .len() as u64;
    let transaction_len = norito::encode_canonical(&transaction).unwrap().len() as u64;
    assert!(payload_len < transaction_len);
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            sealed
                .to_canonical_bytes(policy().checkpoint_max_bytes)
                .expect("canonical sealed record"),
            sealed_bytes
        );
        assert_eq!(
            ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(
                &sealed_bytes,
                policy().checkpoint_max_bytes
            )
            .expect("recover canonical sealed record"),
            sealed
        );
        assert_eq!(
            derive_signing_token(authorization.job_id(), 1, &context)
                .expect("stable signing token"),
            expected_token
        );
        assert!(completion_account_id_fits_canonical_bound(
            &context.provider_owner
        ));
        let mut exact = policy();
        exact.max_signed_transaction_bytes = payload_len;
        validate_completion_signing_context(&authorization, &context, exact)
            .expect("exact canonical payload budget");
        exact.max_signed_transaction_bytes -= 1;
        assert_eq!(
            validate_completion_signing_context(&authorization, &context, exact),
            Err(ProviderIngestOutboxError::InvalidSigningContext)
        );
        exact.max_signed_transaction_bytes = transaction_len;
        assert_eq!(
            validate_completion_transaction(&authorization, &context, &transaction, exact)
                .expect("exact canonical transaction budget"),
            *transaction.hash().as_ref()
        );
        exact.max_signed_transaction_bytes -= 1;
        assert_eq!(
            validate_completion_transaction(&authorization, &context, &transaction, exact),
            Err(ProviderIngestOutboxError::InvalidSignedTransaction)
        );
    }
}

#[test]
fn musubi_context_is_bounded_and_separates_job_identity() {
    use iroha_crypto::HashOf;
    use iroha_data_model::block::BlockHeader;

    let generic = authorization(0x5A, 7);
    let commitment = musubi_commitment(&generic, 0x31);
    let first_context =
        FinalizedProviderIngestMusubiContextV1::new(network_id(0x41), commitment.archive_id())
            .expect("first context");
    assert_eq!(first_context.network_id(), &network_id(0x41));
    assert_eq!(first_context.archive_id(), commitment.archive_id());
    first_context.validate().expect("valid bounded context");
    let encoded = norito::codec::encode_adaptive(&first_context);
    let decoded: FinalizedProviderIngestMusubiContextV1 =
        norito::codec::decode_adaptive(&encoded).expect("decode nested context");
    assert_eq!(decoded, first_context);
    let first = authorization_with_musubi_context(&generic, first_context.clone());
    let second_context =
        FinalizedProviderIngestMusubiContextV1::new(network_id(0x42), commitment.archive_id())
            .expect("second context");
    let second = authorization_with_musubi_context(&generic, second_context);
    assert_ne!(generic.job_id(), first.job_id());
    assert_ne!(first.job_id(), second.job_id());
    assert!(!generic.same_binding(&first));
    assert!(!first.same_binding(&second));
    assert_musubi_context_rejects_unmarked_network(&first_context);
    let mut unmarked_network = first_context.clone();
    let mut unmarked_hash = Hash::prehashed([0; 32]);
    iroha_crypto::zeroize_value_for_confidential_discard(&mut unmarked_hash);
    assert_eq!(unmarked_hash.as_ref()[31] & 1, 0);
    unmarked_network.network_id =
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(unmarked_hash));
    assert_eq!(
        unmarked_network.validate(),
        Err(ProviderIngestOutboxError::InvalidAuthorization)
    );
    let mut zero_archive = first_context;
    zero_archive.archive_id = ArchiveId::new([0; 32]);
    assert_eq!(
        zero_archive.validate(),
        Err(ProviderIngestOutboxError::InvalidAuthorization)
    );
}
