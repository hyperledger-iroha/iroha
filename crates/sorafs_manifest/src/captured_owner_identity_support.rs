//! Private checks for actual nominal and directional SoraFS owner identities.

/// Decode an immutable independently captured sixteen-byte identity hash.
fn parsed_hash(value: &str) -> [u8; 16] {
    hex::decode(value)
        .expect("captured hexadecimal identity hash")
        .try_into()
        .expect("captured identity hash has sixteen bytes")
}

/// Check only an owner's existing serialization capability and declared identity.
pub(crate) fn check_serialize_only<T>(nominal: &str, frame: &str, serialize_hash: &str)
where
    T: norito::NoritoSchema + norito::NoritoSerialize,
{
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), frame);
    let serialize_hash = parsed_hash(serialize_hash);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), serialize_hash);
    assert_eq!(
        <T as norito::NoritoSerialize>::schema_hash(),
        serialize_hash
    );
}

/// Check both independently observed directions without broadening other owners.
pub(crate) fn check_both<T>(
    nominal: &str,
    frame: &str,
    serialize_hash: &str,
    deserialize_hash: &str,
) where
    T: norito::NoritoSchema + norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    check_serialize_only::<T>(nominal, frame, serialize_hash);
    let deserialize_hash = parsed_hash(deserialize_hash);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        deserialize_hash
    );
    assert_eq!(
        <T as norito::NoritoDeserialize<'_>>::schema_hash(),
        deserialize_hash
    );
}
