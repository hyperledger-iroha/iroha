// Explicit Core SoraFS state roots and actual persistence recovery.

#[test]
fn declared_finance_roots_have_explicit_nominal_and_frame_identity() {
    assert_eq!(
        <ReputationJournalHeadStateV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_reputation::ReputationJournalHeadStateV1"
    );
    assert_eq!(
        <ReputationJournalHeadStateV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_reputation::ReputationJournalHeadStateV1"
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
fn declared_reputation_head_root_preserves_exact_cursor_components() {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let head = ReputationJournalHeadStateV1 {
        last_sequence: 7,
        last_target_block_height: 3,
        last_event_index: 2,
    };
    let bytes = declared_state_frame(
        &head,
        "iroha_core::smartcontracts::isi::sorafs_reputation::ReputationJournalHeadStateV1",
    );
    assert_eq!(encode_state(&head, "schema head").unwrap(), bytes);
    assert_eq!(
        decode_state::<ReputationJournalHeadStateV1>(&bytes, "schema head").unwrap(),
        head
    );
    let changed = ReputationJournalHeadStateV1 {
        last_event_index: 3,
        ..head
    };
    assert_ne!(encode_state(&changed, "changed head").unwrap(), bytes);
}
