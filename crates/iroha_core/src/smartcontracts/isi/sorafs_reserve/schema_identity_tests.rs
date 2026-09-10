// Explicit Core SoraFS state roots and actual persistence recovery.

#[test]
fn declared_finance_roots_have_explicit_nominal_and_frame_identity() {
    assert_eq!(
        <ReservePersistedEventV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReservePersistedEventV1"
    );
    assert_eq!(
        <ReservePersistedEventV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReservePersistedEventV1"
    );
    assert_eq!(
        <ReserveStateV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReserveStateV1"
    );
    assert_eq!(
        <ReserveStateV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReserveStateV1"
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
fn declared_reserve_roots_recover_actual_committed_policy_and_event() {
    let governance = account(&keypair(0x75));
    let provider = account(&keypair(0x76));
    let custody = account(&keypair(0x77));
    let treasury = account(&keypair(0x78));
    let mut state = state_fixture(&governance, &provider, &custody, &treasury);
    let first = policy(1, None, custody, treasury, &governance);
    transact(&mut state, 1, NOW, |transaction| {
        SetSorafsReservePolicy::new(first).execute(&governance, transaction)
    })
    .expect("commit reserve policy");
    let view = state.view();
    let retained = read_reserve_state(view.world())
        .unwrap()
        .expect("retained reserve state");
    let event = read_persisted_event(view.world(), 1)
        .unwrap()
        .expect("retained reserve event");
    let state_bytes = declared_state_frame(
        &retained,
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReserveStateV1",
    );
    let event_bytes = declared_state_frame(
        &event,
        "iroha_core::smartcontracts::isi::sorafs_reserve::ReservePersistedEventV1",
    );
    assert_eq!(
        encode_state(&retained, "schema reserve state").unwrap(),
        state_bytes
    );
    assert_eq!(decode_reserve_state(&state_bytes).unwrap(), retained);
    assert_eq!(decode_persisted_event(&event_bytes, 1).unwrap(), event);
    assert!(decode_reserve_state(&event_bytes).is_err());
}
