//! Source-bound compiler identities for the existing event wire owners.
//!
//! These checks retain nominal identity and both observed codec directions;
//! owning event suites continue to validate payloads, matching and stream behavior.

/// Compare one existing owner against its independently captured codec identities.
pub(super) fn check<T>(nominal: &str, serialize_hash: &str, deserialize_hash: &str)
where
    T: norito::NoritoSchema + norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let parse = |value: &str| -> [u8; 16] {
        hex::decode(value)
            .expect("captured hexadecimal schema hash")
            .try_into()
            .expect("captured schema hash has sixteen bytes")
    };
    let serialize_hash = parse(serialize_hash);
    let deserialize_hash = parse(deserialize_hash);
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), serialize_hash);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        deserialize_hash
    );
    assert_eq!(norito::schema::identity::frame_hash::<T>(), serialize_hash);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        deserialize_hash
    );
}

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::SharedDataEvent>(
        "iroha_data_model::events::SharedDataEvent",
        "3551683d828ed4a1554af5d3957f2ab8",
        "3551683d828ed4a1554af5d3957f2ab8",
    );
    check::<super::EventBox>(
        "iroha_data_model::events::model::EventBox",
        "cb3f92c23f1dccf7e438497ad8a5fcd9",
        "cb3f92c23f1dccf7e438497ad8a5fcd9",
    );
    check::<super::TriggeringEventType>(
        "iroha_data_model::events::model::TriggeringEventType",
        "cd3576d3e46bd292c1f497068e07d2e0",
        "cd3576d3e46bd292c1f497068e07d2e0",
    );
    check::<super::EventFilterBox>(
        "iroha_data_model::events::model::EventFilterBox",
        "97df98410e282598c02d17fa140b68d1",
        "97df98410e282598c02d17fa140b68d1",
    );
}
