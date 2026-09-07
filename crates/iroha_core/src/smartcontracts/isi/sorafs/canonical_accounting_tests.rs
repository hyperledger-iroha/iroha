// Canonical framed identity and persistence regression tests.

#[test]
fn replication_order_rejects_compression_before_allocation() {
    let state = make_state_with_completion_anchor();
    let mut block = state.block(block_header_at_epoch(9));
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
    insert_manifest_with_status(
        &mut stx,
        default_digest(),
        default_chunk_digest(),
        None,
        PinStatus::Approved(5),
    );
    let order_id = ReplicationOrderId::new([0x2B; 32]);
    let providers = [
        ProviderId::new([0x3A; 32]),
        ProviderId::new([0x3B; 32]),
        ProviderId::new([0x3C; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let canonical = encode_replication_order_for_epoch_window(
        replication_order_struct(order_id, default_digest(), &providers, 3),
        5,
        15,
    );
    IssueReplicationOrder {
        order_id,
        order_payload: canonical.clone(),
        issued_epoch: 5,
        deadline_epoch: 15,
        musubi_archive: None,
    }
    .execute(&alice(), &mut stx)
    .expect("admit canonical replication order");
    let record = stx.world.replication_orders.get(&order_id).unwrap().clone();
    validate_stored_replication_order(&record, "canonical order")
        .expect("recover canonical replication order");
    let header = norito::core::Header::read(canonical.as_slice()).expect("frame header");
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    let length_offset = compression_offset + 1;
    let mut forbidden = canonical;
    forbidden[compression_offset] = norito::Compression::Zstd as u8;
    forbidden[length_offset..length_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    let advertised = norito::core::Header::read(forbidden.as_slice()).expect("advertised header");
    assert_eq!(advertised.compression, norito::Compression::Zstd);
    assert_eq!(advertised.length, u64::MAX);
    let zero_allocation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let attempted_id = ReplicationOrderId::new([0x2C; 32]);
    for bytes in [
        forbidden.as_slice(),
        &forbidden[..norito::core::Header::SIZE],
    ] {
        let issue = IssueReplicationOrder {
            order_id: attempted_id,
            order_payload: bytes.to_vec(),
            issued_epoch: 5,
            deadline_epoch: 15,
            musubi_archive: None,
        };
        let error =
            norito::with_decode_limits_scope(zero_allocation, || issue.execute(&alice(), &mut stx))
                .expect_err("issuance rejects compression before allocating");
        assert!(matches!(error, InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("canonical first-release Norito")));
        assert!(stx.world.replication_orders.get(&attempted_id).is_none());
        let mut substituted = record.clone();
        substituted.canonical_order = bytes.to_vec();
        let error = norito::with_decode_limits_scope(zero_allocation, || {
            validate_stored_replication_order(&substituted, "substituted order")
        })
        .expect_err("stored order rejects compression before allocating");
        assert!(
            matches!(error, InstructionExecutionError::InvariantViolation(message)
            if message.contains("not canonical or bound to its record"))
        );
    }
    assert_eq!(
        stx.world.replication_orders.get(&order_id).unwrap(),
        &record
    );
}

#[test]
fn pin_accounting_state_is_canonical_across_caller_layouts() {
    let usage = PinResourceUsage {
        manifest_count: 7,
        content_bytes: 9_876_543,
    };
    let lineage = PinLineageSummaryV1 {
        depth: 3,
        direct_successor_count: 2,
    };
    let canonical_usage = norito::encode_canonical(&usage).expect("canonical usage");
    let canonical_lineage = norito::encode_canonical(&lineage).expect("canonical lineage");
    let mut alternate_layouts = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            encode_pin_accounting_state(&usage, "usage").expect("persist usage"),
            canonical_usage
        );
        assert_eq!(
            encode_pin_accounting_state(&lineage, "lineage").expect("persist lineage"),
            canonical_lineage
        );
        assert_eq!(
            decode_pin_accounting_state::<PinResourceUsage>(&canonical_usage, "usage")
                .expect("recover usage"),
            usage
        );
        assert_eq!(
            decode_pin_accounting_state::<PinLineageSummaryV1>(&canonical_lineage, "lineage")
                .expect("recover lineage"),
            lineage
        );
        let alternate = norito::to_bytes(&usage).expect("advertised alternate usage");
        if alternate != canonical_usage {
            alternate_layouts += 1;
            let error = decode_pin_accounting_state::<PinResourceUsage>(&alternate, "usage")
                .expect_err("alternate state format must fail under its matching caller layout");
            assert!(
                matches!(error, InstructionExecutionError::InvariantViolation(message)
                if message.contains("not exact canonical Norito"))
            );
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(alternate_layouts > 0);
}

#[test]
fn pin_accounting_state_rejects_compression_before_allocation() {
    let usage = PinResourceUsage {
        manifest_count: 1,
        content_bytes: 1_024,
    };
    let canonical = norito::encode_canonical(&usage).expect("canonical usage");
    let header = norito::core::Header::read(canonical.as_slice()).expect("frame header");
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    let length_offset = compression_offset + 1;
    let mut forbidden = canonical.clone();
    forbidden[compression_offset] = norito::Compression::Zstd as u8;
    forbidden[length_offset..length_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    let advertised = norito::core::Header::read(forbidden.as_slice()).expect("advertised header");
    assert_eq!(advertised.compression, norito::Compression::Zstd);
    assert_eq!(advertised.length, u64::MAX);
    let zero_allocation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    for bytes in [
        forbidden.as_slice(),
        &forbidden[..norito::core::Header::SIZE],
    ] {
        let error = norito::with_decode_limits_scope(zero_allocation, || {
            decode_pin_accounting_state::<PinResourceUsage>(bytes, "usage")
        })
        .expect_err("compression is rejected before allocating its advertised length");
        assert!(
            matches!(error, InstructionExecutionError::InvariantViolation(message)
            if message.contains("not exact canonical Norito"))
        );
    }
    assert_eq!(
        decode_pin_accounting_state::<PinResourceUsage>(&canonical, "usage")
            .expect("canonical recovery still succeeds"),
        usage
    );
    assert!(
        decode_pin_accounting_state::<PinResourceUsage>(
            &vec![0; PIN_ACCOUNTING_STATE_MAX_BYTES + 1],
            "usage"
        )
        .is_err()
    );
}

#[test]
fn pin_accounting_key_ignores_ambient_norito_layout() {
    let authority = alice();
    let canonical = norito::encode_canonical(&authority).expect("canonical account identity");
    let expected = StatePath::from_str(&format!(
        "{PIN_AUTHORITY_USAGE_STATE_KEY_PREFIX_V1}{}",
        hex::encode(blake3_hash(&canonical).as_bytes()),
    ))
    .expect("expected authority accounting key");
    let mut distinct_layout = false;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::to_bytes(&authority).expect("ambient account identity");
        distinct_layout |= before != canonical;
        assert_eq!(
            pin_authority_usage_key(&authority).expect("accounting key"),
            expected
        );
        assert_ne!(
            pin_authority_usage_key(&bob()).expect("different authority key"),
            expected
        );
        assert_eq!(
            norito::to_bytes(&authority).expect("restored layout"),
            before
        );
    }
    assert!(distinct_layout);
}

#[test]
fn v1_norito_decoders_reject_advertised_alternate_layouts() {
    let provider = ProviderId::new([0x49; 32]);
    let report = repair_report(
        "REP-ALTERNATE-LAYOUT",
        provider,
        [0x4A; 32],
        &alice(),
        4_000,
    );
    let canonical = norito::encode_canonical(&report).expect("encode canonical repair report");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&report).expect("encode alternate-layout repair report")
    };
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes::<RepairReportV1>(&alternate)
            .expect("ordinary Norito accepts the advertised alternate layout"),
        report
    );
    let payload_error = decode_repair_payload::<RepairReportV1>(&alternate, "repair report")
        .expect_err("admitted repair payload must reject alternate layout");
    assert!(
        smart_contract_error_message(&payload_error)
            .contains("repair report is not exact canonical Norito")
    );
    for error in [
        decode_repair_state::<RepairReportV1>(&alternate, "repair report")
            .expect_err("persisted repair state must reject alternate layout"),
        decode_stored_repair_payload::<RepairReportV1>(&alternate, "repair report")
            .expect_err("stored repair payload must reject alternate layout"),
    ] {
        assert!(matches!(
            error,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("repair report is not exact canonical Norito")
        ));
    }
    let mut alias = default_alias_binding();
    let bundle = decode_alias_proof_untrusted_signers(&alias.proof)
        .expect("decode canonical alias fixture integrity");
    alias.proof = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&bundle).expect("encode alternate-layout alias proof")
    };
    let alias_error = validate_manifest_alias_binding(
        &alias,
        &default_digest(),
        &default_root_cid(),
        Some((5, default_policy().retention_epoch)),
    )
    .expect_err("alias proof must reject alternate layout");
    assert!(
        smart_contract_error_message(&alias_error).contains("not canonical Norito"),
        "unexpected alias rejection: {alias_error:?}"
    );
}

#[test]
fn completion_revalidates_policy_assignment_and_finalized_anchor_at_commit() {
    let state = make_state_with_completion_anchor();
    let mut block = state.block(block_header_at_epoch(9));
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
    insert_manifest_with_status(
        &mut stx,
        default_digest(),
        default_chunk_digest(),
        None,
        PinStatus::Approved(5),
    );
    let order_id = ReplicationOrderId::new([0x7B; 32]);
    let original_provider = ProviderId::new([0x3A; 32]);
    let replacement_provider = ProviderId::new([0x3B; 32]);
    let unchanged_providers = [ProviderId::new([0x3C; 32]), ProviderId::new([0x3D; 32])];
    let initial_providers = [
        original_provider,
        unchanged_providers[0],
        unchanged_providers[1],
    ];
    let revised_providers = [
        replacement_provider,
        unchanged_providers[0],
        unchanged_providers[1],
    ];
    seed_provider_owners(
        &mut stx,
        &[
            original_provider,
            replacement_provider,
            unchanged_providers[0],
            unchanged_providers[1],
        ],
        &alice(),
    );
    let payload = encode_replication_order_for_epoch_window(
        replication_order_struct(order_id, default_digest(), &initial_providers, 3),
        5,
        15,
    );
    IssueReplicationOrder {
        order_id,
        order_payload: payload,
        issued_epoch: 5,
        deadline_epoch: 15,
        musubi_archive: None,
    }
    .execute(&alice(), &mut stx)
    .expect("issue replication order");
    let revision_one = completion_authority(&alice(), 1);
    let revision_two = completion_authority(&alice(), 2);
    let prepared_under_revision_one =
        completion_instruction(order_id, original_provider, 6, &alice());
    SetProviderIngestCompletionAuthority::new(original_provider, Some(revision_one), revision_two)
        .execute(&alice(), &mut stx)
        .expect("rotate original provider completion policy");
    let stale_policy = prepared_under_revision_one
        .execute(&alice(), &mut stx)
        .expect_err("completion prepared under the old policy must fail after rotation");
    assert!(matches!(
        stale_policy,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("completion authority")
    ));
    let mut prepared_before_reassignment =
        completion_instruction(order_id, original_provider, 6, &alice());
    prepared_before_reassignment.expected_authority = completion_authority(&alice(), 2);
    {
        let _alternate = norito::core::DecodeFlagsGuard::enter(0);
        ReviseReplicationOrderAssignments::new(
            order_id,
            1,
            2,
            revised_providers
                .iter()
                .map(|provider| ReplicationAssignmentV1 {
                    provider_id: *provider.as_bytes(),
                    slice_gib: 512,
                    lane: None,
                })
                .collect(),
        )
        .execute(&alice(), &mut stx)
        .expect("atomically reassign pending order under an alternate caller layout");
    }
    let reassigned = stx.world.replication_orders.get(&order_id).unwrap();
    let decoded = validate_stored_replication_order(reassigned, "reassigned order")
        .expect("canonical caller recovers reassignment");
    assert_eq!(
        reassigned.canonical_order,
        norito::encode_canonical(&decoded).expect("canonical reassignment")
    );
    let stale_assignment = prepared_before_reassignment
        .execute(&alice(), &mut stx)
        .expect_err("completion prepared before reassignment must fail");
    assert!(matches!(
        stale_assignment,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("assignment revision")
    ));
    let mut stale_anchor = completion_instruction(order_id, replacement_provider, 7, &alice());
    stale_anchor.expected_assignment_revision = 2;
    stale_anchor.finalized_anchor.block_hash = [0xEE; 32];
    let stale_anchor = stale_anchor
        .execute(&alice(), &mut stx)
        .expect_err("completion anchored to another committed prefix must fail");
    assert!(matches!(
        stale_anchor,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("finalized anchor")
    ));
    let mut valid = completion_instruction(order_id, replacement_provider, 7, &alice());
    valid.expected_assignment_revision = 2;
    valid
        .execute(&alice(), &mut stx)
        .expect("current authority, assignment revision, and anchor must complete");
    assert_eq!(
        stx.world.replication_orders.get(&order_id).unwrap().status,
        ReplicationOrderStatus::Pending,
        "one completed provider does not satisfy the governed three-replica target"
    );
    for (provider, epoch) in unchanged_providers.into_iter().zip([8, 9]) {
        let mut completion = completion_instruction(order_id, provider, epoch, &alice());
        completion.expected_assignment_revision = 2;
        completion
            .execute(&alice(), &mut stx)
            .expect("unchanged provider completes under the revised assignment");
    }
    let record = stx
        .world
        .replication_orders
        .get(&order_id)
        .expect("completed order retained");
    assert_eq!(record.status, ReplicationOrderStatus::Completed(9));
    assert_eq!(record.provider_completions.len(), 3);
    let completion = record
        .provider_completion(replacement_provider)
        .expect("completion audit context retained");
    assert_eq!(completion.assignment_revision, 2);
    assert_eq!(
        completion.completion_authority,
        completion_authority(&alice(), 1)
    );
    assert_eq!(completion.finalized_anchor, completion_anchor());
}
