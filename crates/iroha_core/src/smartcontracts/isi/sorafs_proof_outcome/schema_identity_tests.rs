// Explicit Core SoraFS state roots and actual persistence recovery.

#[test]
fn declared_finance_roots_have_explicit_nominal_and_frame_identity() {
    assert_eq!(
        <ProofOutcomePersistedEventV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomePersistedEventV1"
    );
    assert_eq!(
        <ProofOutcomePersistedEventV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomePersistedEventV1"
    );
    assert_eq!(
        <ProofOutcomeEventJournalHeadV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomeEventJournalHeadV1"
    );
    assert_eq!(
        <ProofOutcomeEventJournalHeadV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomeEventJournalHeadV1"
    );
}

fn declared_state_frame<T>(value: &T, expected: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let bytes = norito::encode_canonical(value).expect("canonical complete state");
    let header = norito::core::Header::read(bytes.as_slice()).expect("valid header");
    assert_eq!(header.schema, norito::core::schema_hash_for_name(expected));
    assert_eq!(norito::canonical_frame_len(value).unwrap(), bytes.len());
    let recovered = norito::decode_canonical::<T>(&bytes).expect("typed recovery");
    assert_eq!(norito::encode_canonical(&recovered).unwrap(), bytes);
    let mut substituted = bytes.clone();
    let offset = header.magic.len() + 2;
    substituted[offset..offset + header.schema.len()].copy_from_slice(
        &norito::core::schema_hash_for_name("iroha_core::different.state.root"),
    );
    assert!(matches!(
        norito::decode_canonical::<T>(&substituted),
        Err(norito::Error::SchemaMismatch)
    ));
    let mut suffixed = bytes.clone();
    suffixed.push(0);
    assert!(norito::decode_canonical::<T>(&suffixed).is_err());
    assert!(norito::decode_canonical::<T>(&bytes[..bytes.len() - 1]).is_err());
    bytes
}

#[test]
fn declared_proof_journal_roots_recover_complete_validated_outcome() {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload = pdp_archive_without_proof(1, 0x23, PdpRejectionReasonV1::InvalidProof);
    let prepared = prepare_pdp_outcome(&payload).expect("validated PDP projection");
    let outcome = prepared.into_record(account(&ed25519_keypair(0x02)), (NOW + 1) * 1_000);
    validate_outcome_record(&outcome).expect("valid stored outcome");
    let record = ProofOutcomePersistedEventV1 {
        sequence: 1,
        target_block_height: 2,
        event_index: 0,
        outcome,
    };
    let head = ProofOutcomeEventJournalHeadV1 {
        last_sequence: 1,
        last_target_block_height: 2,
        last_event_index: 0,
    };
    let record_bytes = declared_state_frame(
        &record,
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomePersistedEventV1",
    );
    let head_bytes = declared_state_frame(
        &head,
        "iroha_core::smartcontracts::isi::sorafs_proof_outcome::ProofOutcomeEventJournalHeadV1",
    );
    assert_eq!(encode_state(&record, "schema event").unwrap(), record_bytes);
    assert_eq!(
        decode_state::<ProofOutcomePersistedEventV1>(&record_bytes, "schema event").unwrap(),
        record
    );
    assert_eq!(
        decode_state::<ProofOutcomeEventJournalHeadV1>(&head_bytes, "schema head").unwrap(),
        head
    );
    assert!(decode_state::<ProofOutcomePersistedEventV1>(&head_bytes, "wrong root").is_err());
}
