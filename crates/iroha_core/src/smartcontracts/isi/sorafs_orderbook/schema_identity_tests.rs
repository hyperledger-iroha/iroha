// Explicit Core SoraFS state roots and actual persistence recovery.

#[test]
fn declared_finance_roots_have_explicit_nominal_and_frame_identity() {
    assert_eq!(
        <OrderbookPersistedEventV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookPersistedEventV1"
    );
    assert_eq!(
        <OrderbookPersistedEventV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookPersistedEventV1"
    );
    assert_eq!(
        <OrderbookEventJournalHeadV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookEventJournalHeadV1"
    );
    assert_eq!(
        <OrderbookEventJournalHeadV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookEventJournalHeadV1"
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
fn declared_orderbook_roots_recover_actual_committed_journal_state() {
    let buyer = keypair(0x2A);
    let authority = account(&buyer);
    let mut state = state_with_accounts(&[&buyer]);
    transact(&mut state, 1, NOW, |transaction| {
        activate_policy(transaction, &authority);
        Ok(())
    })
    .expect("commit policy event");
    let view = state.view();
    let record = read_persisted_event(view.world(), 1)
        .unwrap()
        .expect("persisted event");
    let head = read_event_journal_head(view.world())
        .unwrap()
        .expect("persisted head");
    let record_bytes = declared_state_frame(
        &record,
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookPersistedEventV1",
    );
    let head_bytes = declared_state_frame(
        &head,
        "iroha_core::smartcontracts::isi::sorafs_orderbook::OrderbookEventJournalHeadV1",
    );
    assert_eq!(encode_state(&record, "schema event").unwrap(), record_bytes);
    assert_eq!(
        decode_state::<OrderbookPersistedEventV1>(&record_bytes, "schema event").unwrap(),
        record
    );
    assert_eq!(
        decode_state::<OrderbookEventJournalHeadV1>(&head_bytes, "schema head").unwrap(),
        head
    );
    assert!(decode_state::<OrderbookPersistedEventV1>(&head_bytes, "wrong root").is_err());
}
