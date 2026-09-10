// Explicit Core SoraFS state roots and actual persistence recovery.

#[test]
fn declared_finance_roots_have_explicit_nominal_and_frame_identity() {
    assert_eq!(
        <NonceBindingStateV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::smartcontracts::isi::sorafs_pop_registry::NonceBindingStateV1"
    );
    assert_eq!(
        <NonceBindingStateV1 as norito::NoritoSchema>::frame_name(),
        "iroha_core::smartcontracts::isi::sorafs_pop_registry::NonceBindingStateV1"
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
fn declared_nonce_binding_root_preserves_both_commitments() {
    let binding = NonceBindingStateV1 {
        credential_commitment: [0x31; 32],
        revocation_nonce_commitment: [0x42; 32],
    };
    let bytes = declared_state_frame(
        &binding,
        "iroha_core::smartcontracts::isi::sorafs_pop_registry::NonceBindingStateV1",
    );
    assert_eq!(
        encode_state(&binding, "schema nonce binding").unwrap(),
        bytes
    );
    let decoded = decode_exact::<NonceBindingStateV1>(
        &bytes,
        STATE_LIMITS,
        STATE_MAX_BYTES,
        "schema nonce binding",
        true,
    )
    .unwrap();
    assert_eq!(decoded.credential_commitment, binding.credential_commitment);
    assert_eq!(
        decoded.revocation_nonce_commitment,
        binding.revocation_nonce_commitment
    );
    let changed = NonceBindingStateV1 {
        credential_commitment: binding.credential_commitment,
        revocation_nonce_commitment: [0x43; 32],
    };
    assert_ne!(encode_state(&changed, "changed binding").unwrap(), bytes);
}
