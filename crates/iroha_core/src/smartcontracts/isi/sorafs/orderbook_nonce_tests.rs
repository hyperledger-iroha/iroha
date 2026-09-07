// Canonical owner identity, persistence and shared nonce replay regressions.

#[test]
fn owner_nonce_identity_and_persistence_ignore_display_and_codec_context() {
    use iroha_data_model::account::address::ChainDiscriminantGuard;

    let owner_key = keypair(0xD1);
    let other_key = keypair(0xD2);
    let owner = account(&owner_key);
    let other = account(&other_key);
    let record = OrderbookOwnerNonceRecord {
        owner: owner.clone(),
        highest_nonce: 41,
    };
    let (identity_frame, expected_bytes) = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        (
            norito::to_bytes(&owner).expect("fixed canonical account frame"),
            norito::to_bytes(&record).expect("fixed canonical nonce frame"),
        )
    };
    let mut expected_hash = blake3::Hasher::new();
    expected_hash.update(NONCE_KEY_DOMAIN_V1);
    expected_hash.update(&identity_frame);
    let expected_key = digest_key(NONCE_STATE_KEY_PREFIX, *expected_hash.finalize().as_bytes());
    let state = state_with_accounts(&[&owner_key, &other_key]);
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    let first_display = {
        let _chain = ChainDiscriminantGuard::enter(73);
        write_nonce(&mut stx, &owner, record.highest_nonce).expect("persist first nonce");
        owner.to_string()
    };
    let (_, expected_allocated) = decode_state_with_limits::<OrderbookOwnerNonceRecord>(
        &expected_bytes,
        "orderbook owner nonce",
        STATE_LIMITS,
    )
    .expect("decode canonical nonce with measured allocation");
    let mut distinct_layout = false;
    for discriminant in [73, 74] {
        let _chain = ChainDiscriminantGuard::enter(discriminant);
        assert_eq!(owner.to_string() == first_display, discriminant == 73);
        for flags in 0..=u8::MAX {
            if norito::core::validate_header_flags(flags).is_err() {
                continue;
            }
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let ambient_frame = norito::to_bytes(&record).expect("ambient record frame");
            distinct_layout |= ambient_frame != expected_bytes;
            assert_eq!(
                nonce_key(&owner).expect("canonical owner key"),
                expected_key
            );
            assert_ne!(nonce_key(&other).expect("distinct owner key"), expected_key);
            assert_eq!(
                read_nonce(stx.world(), &owner).expect("read same partition"),
                Some(record.clone())
            );
            assert_eq!(
                read_nonce(stx.world(), &other).expect("different partition"),
                None
            );
            for stale_nonce in [40, 41] {
                let error = ensure_nonce_advances(stx.world(), &owner, stale_nonce)
                    .expect_err("a display or codec context cannot reset replay protection");
                assert!(matches!(
                    error,
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(message)
                    ) if message == format!(
                        "orderbook nonce {stale_nonce} is stale or replayed; highest committed nonce is 41"
                    )
                ));
            }
            ensure_nonce_advances(stx.world(), &owner, 42).expect("strictly newer nonce");
            assert_eq!(
                stx.world().smart_contract_state().get(&expected_key),
                Some(&expected_bytes),
                "reads and rejected replay leave the exact persisted state unchanged"
            );
            // Exercise the persistence writer separately from the nonce admission guard.
            write_nonce(&mut stx, &owner, record.highest_nonce).expect("canonical persistence");
            assert_eq!(
                stx.world().smart_contract_state().get(&expected_key),
                Some(&expected_bytes)
            );
            let (decoded, allocated) = decode_state_with_limits::<OrderbookOwnerNonceRecord>(
                &expected_bytes,
                "orderbook owner nonce",
                STATE_LIMITS,
            )
            .expect("canonical state under enclosing layout");
            assert_eq!(decoded, record);
            assert_eq!(allocated, expected_allocated);
            assert_eq!(
                norito::to_bytes(&record).expect("restored caller layout"),
                ambient_frame
            );
        }
    }
    assert!(
        distinct_layout,
        "the regression must exercise a different wire layout"
    );
}

#[test]
fn owner_nonce_state_rejects_alternate_frames_and_compression_before_decode() {
    let owner_key = keypair(0xD3);
    let owner = account(&owner_key);
    let record = OrderbookOwnerNonceRecord {
        owner: owner.clone(),
        highest_nonce: 7,
    };
    let canonical = norito::encode_canonical(&record).expect("canonical nonce frame");
    assert_eq!(
        decode_state::<OrderbookOwnerNonceRecord>(&canonical, "orderbook owner nonce")
            .expect("canonical state is admitted"),
        record
    );
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&record).expect("advertised alternate nonce frame")
    };
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes::<OrderbookOwnerNonceRecord>(&alternate)
            .expect("ordinary Norito accepts the advertised layout"),
        record
    );
    let state = state_with_accounts(&[&owner_key]);
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    let key = nonce_key(&owner).expect("canonical partition");
    stx.world
        .smart_contract_state
        .insert(key.clone(), alternate.clone());
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    for error in [
        read_nonce(stx.world(), &owner).expect_err("stored alternate frame is forbidden"),
        ensure_nonce_advances(stx.world(), &owner, 8)
            .expect_err("a malformed high-water cannot admit a new operation"),
    ] {
        assert!(matches!(
            error,
            InstructionExecutionError::InvariantViolation(message)
                if message.as_ref() == "orderbook owner nonce state is not exact canonical Norito"
        ));
    }
    assert_eq!(
        stx.world().smart_contract_state().get(&key),
        Some(&alternate)
    );
    stx.world
        .smart_contract_state
        .insert(key.clone(), canonical.clone());
    assert_eq!(
        read_nonce(stx.world(), &owner).expect("canonical replacement"),
        Some(record)
    );

    // Only change the actual schema's compression tag. An uncompressed or missing body
    // must never reach decompression or consume an allocation budget at this boundary.
    let mut tagged = canonical.clone();
    let header = norito::core::Header::read(tagged.as_slice()).expect("canonical frame header");
    assert_eq!(header.compression, norito::Compression::None);
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    tagged[compression_offset] = norito::Compression::Zstd as u8;
    assert_eq!(
        norito::core::Header::read(tagged.as_slice())
            .expect("forbidden header")
            .compression,
        norito::Compression::Zstd
    );
    let no_resources = DecodeLimits::new(0, 0, 0, 0, 0);
    assert!(matches!(
        norito::decode_canonical_with_limits::<OrderbookOwnerNonceRecord>(&canonical, no_resources),
        Err(error) if error.is_decode_resource_limit()
    ));
    for bytes in [&tagged[..], &tagged[..norito::core::Header::SIZE]] {
        let (result, usage) = norito::core::with_decode_limits_measured(no_resources, || {
            decode_state_with_limits::<OrderbookOwnerNonceRecord>(
                bytes,
                "orderbook owner nonce",
                no_resources,
            )
        });
        assert_eq!(usage.total_allocated_bytes(), 0);
        assert!(matches!(
            result,
            Err(InstructionExecutionError::InvariantViolation(message))
                if message.as_ref() == "orderbook owner nonce state is not exact canonical Norito"
        ));
    }
    assert_eq!(
        stx.world().smart_contract_state().get(&key),
        Some(&canonical)
    );
}

#[test]
fn signed_cancellation_updates_order_and_shared_nonce() {
    let buyer = keypair(0x31);
    let authority = account(&buyer);
    let state = state_with_accounts(&[&buyer]);
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    let policy_digest = activate_policy(&mut stx, &authority);
    let order = order(&buyer, 1);
    let initial_balance = asset_balance(&stx, &authority);
    let escrow_id = orderbook_order_escrow_id(order.order_id);
    SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
        .execute(&authority, &mut stx)
        .expect("order");
    assert!(asset_balance(&stx, &authority) < initial_balance);
    let cancel = cancel(&buyer, order.order_id, 2);
    CancelSorafsOrderbookOrder::new(encode(&cancel), policy_digest)
        .execute(&authority, &mut stx)
        .expect("cancel");
    let stored = read_order(stx.world(), order.order_id)
        .expect("read order")
        .expect("order");
    assert_eq!(stored.status, OrderbookOrderStatusV1::Cancelled);
    assert!(stored.canonical_cancel.is_some());
    assert_eq!(stored.cancelled_at_unix, Some(NOW));
    assert_eq!(stored.cancelled_policy_digest, Some(policy_digest));
    assert_eq!(asset_balance(&stx, &authority), initial_balance);
    let escrow = stx
        .world
        .asset_escrows
        .get(&escrow_id)
        .expect("closed bid custody");
    assert_eq!(
        escrow.status,
        iroha_data_model::escrow::AssetEscrowStatus::Cancelled
    );
    assert_eq!(escrow.remaining_amount, Quantity::zero());
    assert!(
        !crate::smartcontracts::isi::escrow::is_orderbook_order_lock(stx.world(), &escrow_id,)
            .expect("read removed bid marker")
    );
    assert_eq!(
        read_nonce(stx.world(), &authority)
            .expect("read nonce")
            .expect("nonce")
            .highest_nonce,
        2
    );
    assert!(
        CancelSorafsOrderbookOrder::new(encode(&cancel), policy_digest)
            .execute(&authority, &mut stx)
            .is_err()
    );
}
