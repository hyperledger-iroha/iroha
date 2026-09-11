//! Typed frame identity assertions shared by the Node owner fixtures.

/// Check the declared root identity and its nominal identity inside a generic parent.
pub(crate) fn assert_identity<T: norito::NoritoSchema>(expected: &str) {
    assert_eq!(T::nominal_name(), expected);
    assert_eq!(T::frame_name(), expected);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        norito::core::schema_hash_for_name(expected),
    );
    assert_eq!(
        <Vec<T> as norito::NoritoSchema>::nominal_name(),
        format!("alloc::vec::Vec<{expected}>"),
    );
}

/// Replay an actual canonical frame under every supported layout and reject a different root.
pub(crate) fn assert_canonical_frame<T>(value: &T, expected: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    assert_identity::<T>(expected);
    let canonical = norito::encode_canonical(value).expect("fixture canonical frame");
    let header = norito::core::Header::read(canonical.as_slice()).expect("actual typed header");
    assert_eq!(header.schema, norito::core::schema_hash_for_name(expected));
    let mut layouts = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        layouts += 1;
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(value).unwrap(), canonical);
        let decoded: T = norito::decode_canonical(&canonical).expect("canonical fixture replay");
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), canonical);
        assert!(matches!(
            norito::decode_canonical::<Vec<T>>(&canonical),
            Err(norito::Error::SchemaMismatch)
        ));
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(layouts, 10);
    canonical
}
