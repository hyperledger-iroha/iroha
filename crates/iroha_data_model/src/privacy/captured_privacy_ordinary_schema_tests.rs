//! Exact compiler-captured identities for unchanged ordinary codec declarations.
//!
//! These checks preserve nominal/root identity and both codec directions.
//! Full payload and feature qualification remains with the owning runtime suites.

fn check<T>(nominal: &str, serialize_hash: &str, deserialize_hash: &str)
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
}

#[test]
fn captured_ordinary_codec_schema_identities() {
    check::<super::PrivacyProtocolIdV1>(
        "iroha_data_model::privacy::PrivacyProtocolIdV1",
        "e65fd37d6bddc560220d3754c7ad062e",
        "e65fd37d6bddc560220d3754c7ad062e",
    );
    check::<super::PrivacySecurityModelV1>(
        "iroha_data_model::privacy::PrivacySecurityModelV1",
        "ef1295c7ec655e4044f4ef48bfe64683",
        "ef1295c7ec655e4044f4ef48bfe64683",
    );
    check::<super::PrivacyProofSystemIdV1>(
        "iroha_data_model::privacy::PrivacyProofSystemIdV1",
        "d096f678c600b4c958d52dcbdebf44fd",
        "d096f678c600b4c958d52dcbdebf44fd",
    );
    check::<super::PrivacyEngineIdV1>(
        "iroha_data_model::privacy::PrivacyEngineIdV1",
        "2735ad4116f85d8110f54119b158cd60",
        "2735ad4116f85d8110f54119b158cd60",
    );
}
