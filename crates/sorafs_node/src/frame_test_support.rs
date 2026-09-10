//! Shared frame-boundary assertions for non-secret node test fixtures.

/// Check the exact current owner, all payload fields, and strict malformed-frame rejection.
///
/// Use only with non-secret fixtures: returned and temporary encoded buffers are not scrubbed.
/// Confidential plaintext tests must keep their own protected-buffer guards.
pub(crate) fn assert_current_frame<T>(value: &T, name: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + PartialEq,
{
    assert!(T::nominal_name() == name, "unexpected nominal frame owner");
    assert!(T::frame_name() == name, "unexpected projected frame owner");
    let bytes = norito::encode_canonical(value).expect("current owner frame");
    assert!(
        bytes[6..22] == norito::schema::identity::frame_hash::<T>(),
        "unexpected frame header"
    );
    let decoded: T = norito::decode_canonical(&bytes).expect("current frame roundtrip");
    assert!(decoded == *value, "roundtrip changed payload fields");
    assert!(
        norito::encode_canonical(&decoded).expect("roundtrip bytes") == bytes,
        "roundtrip changed canonical bytes"
    );
    let mut wrong_owner = bytes.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    bytes
}

/// Count a nested payload with a conservative envelope allowance, without creating a frame owner.
pub(crate) fn nested_record_reserve_len<T: norito::SerializePayload>(value: &T) -> usize {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload = norito::core::encoded_payload_len(value).expect("count nested fixture payload");
    let alignment = norito::core::archived_payload_align::<T>();
    let padding = (alignment - norito::core::Header::SIZE % alignment) % alignment;
    norito::core::Header::SIZE
        .checked_add(padding)
        .and_then(|n| n.checked_add(payload))
        .expect("nested fixture reserve fits usize")
}

#[test]
fn nested_record_reserve_matches_a_real_frame_and_counts_field_only_payloads() {
    #[derive(norito::SerializePayload)]
    struct FieldOnly {
        bytes: Vec<u8>,
        count: u64,
    }
    assert_eq!(
        nested_record_reserve_len(&17_u64),
        norito::encode_canonical(&17_u64).unwrap().len()
    );
    let field = FieldOnly {
        bytes: vec![1, 2, 3],
        count: 4,
    };
    let payload = norito::codec::encode_adaptive(&field);
    let reserve = nested_record_reserve_len(&field);
    assert!(reserve >= payload.len() + norito::core::Header::SIZE);
    assert!(
        reserve
            < payload.len()
                + norito::core::Header::SIZE
                + norito::core::archived_payload_align::<FieldOnly>()
    );
}
