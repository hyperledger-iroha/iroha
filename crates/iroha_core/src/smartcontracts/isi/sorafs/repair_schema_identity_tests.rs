// Explicit identity checks use the actual bounded repair persistence owner.

#[test]
fn repair_persisted_roots_use_explicit_schema_identities() {
    use super::schema_test_support::{assert_canonical_frame, assert_identity};
    assert_identity::<RepairSourceBindingV1>(
        "iroha_core::smartcontracts::isi::sorafs::RepairSourceBindingV1",
    );
    assert_identity::<RepairPersistedEventV1>(
        "iroha_core::smartcontracts::isi::sorafs::RepairPersistedEventV1",
    );
    assert_identity::<RepairEventJournalHeadV1>(
        "iroha_core::smartcontracts::isi::sorafs::RepairEventJournalHeadV1",
    );
    let binding = RepairSourceBindingV1 {
        source_identity: [0x31; 32],
        task_id: [0x32; 32],
        ticket_id: "SCHEMA-REPAIR-TICKET".to_owned(),
        report_digest: [0x33; 32],
    };
    assert!(RepairTicketId::is_valid_str(&binding.ticket_id));
    let encoded =
        encode_repair_state(&binding, "schema binding").expect("real persistence encoder");
    assert_eq!(
        encoded,
        assert_canonical_frame(
            &binding,
            "iroha_core::smartcontracts::isi::sorafs::RepairSourceBindingV1"
        )
    );
    let restored: RepairSourceBindingV1 =
        decode_repair_state(&encoded, "schema binding").expect("real bounded persistence decoder");
    assert_eq!(restored.source_identity, binding.source_identity);
    assert_eq!(restored.task_id, binding.task_id);
    assert_eq!(restored.ticket_id, binding.ticket_id);
    assert_eq!(restored.report_digest, binding.report_digest);
    let authority = AccountId::new(checked_ed25519_keypair().public_key().clone());
    let record = RepairPersistedEventV1 {
        sequence: 1,
        target_block_height: 7,
        event_index: 0,
        event: SorafsRepairLedgerEvent {
            kind: SorafsRepairLedgerEventKind::TaskSubmitted,
            ticket_id: binding.ticket_id.clone(),
            task_id: binding.task_id,
            provider_id: ProviderId::new([0x34; 32]),
            manifest_digest: ManifestDigest::new([0x35; 32]),
            revision: 1,
            authority,
            occurred_at_unix_ms: 1_000,
        },
    };
    validate_repair_persisted_event(&record, 1).expect("actual event structural policy");
    let event_bytes = assert_canonical_frame(
        &record,
        "iroha_core::smartcontracts::isi::sorafs::RepairPersistedEventV1",
    );
    assert_eq!(
        decode_repair_state::<RepairPersistedEventV1>(&event_bytes, "schema event").unwrap(),
        record
    );
    let head = RepairEventJournalHeadV1 {
        last_sequence: 1,
        last_target_block_height: 7,
        last_event_index: 0,
    };
    assert_canonical_frame(
        &head,
        "iroha_core::smartcontracts::isi::sorafs::RepairEventJournalHeadV1",
    );
    assert!(matches!(
        decode_repair_state::<RepairEventJournalHeadV1>(&event_bytes, "schema event"),
        Err(InstructionExecutionError::InvariantViolation(_))
    ));
    let mut invalid = record;
    invalid.sequence = 0;
    assert!(matches!(
        validate_repair_persisted_event(&invalid, 1),
        Err(InstructionExecutionError::InvariantViolation(_))
    ));
}

#[test]
fn repair_digest_commits_the_explicit_canonical_root_in_every_layout() {
    let authority = AccountId::new(checked_ed25519_keypair().public_key().clone());
    let binding = RepairSourceBindingV1 {
        source_identity: [0x41; 32],
        task_id: [0x42; 32],
        ticket_id: "SCHEMA-DIGEST-TICKET".to_owned(),
        report_digest: [0x43; 32],
    };
    assert!(RepairTicketId::is_valid_str(&binding.ticket_id));
    let bytes = norito::encode_canonical(&binding).unwrap();
    let account_text = authority.to_string();
    let mut preimage = b"sorafs.repair.action-digest.v1".to_vec();
    preimage.extend_from_slice(&(account_text.len() as u64).to_le_bytes());
    preimage.extend_from_slice(account_text.as_bytes());
    preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    preimage.extend_from_slice(&bytes);
    let expected = *blake3::hash(&preimage).as_bytes();
    let mut layouts = 0;
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        layouts += 1;
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        // This is the private generic digest contract, not an authorized repair instruction.
        assert_eq!(
            repair_action_digest(&authority, &binding).unwrap(),
            expected
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(layouts, 10);
    let mut changed = binding;
    changed.report_digest[0] ^= 1;
    assert_ne!(
        repair_action_digest(&authority, &changed).unwrap(),
        expected
    );
}
