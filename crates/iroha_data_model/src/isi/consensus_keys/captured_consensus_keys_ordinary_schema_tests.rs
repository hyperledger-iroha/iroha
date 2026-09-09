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
    check::<super::RegisterConsensusKey>(
        "iroha_data_model::isi::consensus_keys::RegisterConsensusKey",
        "ed8c3d112e49adabe1144774581ee476",
        "ed8c3d112e49adabe1144774581ee476",
    );
    check::<super::RotateConsensusKey>(
        "iroha_data_model::isi::consensus_keys::RotateConsensusKey",
        "981d95dc2c6118a1f5aa090f9d787752",
        "981d95dc2c6118a1f5aa090f9d787752",
    );
    check::<super::DisableConsensusKey>(
        "iroha_data_model::isi::consensus_keys::DisableConsensusKey",
        "87ea07060f7caba5d8e01f1e08d8275a",
        "87ea07060f7caba5d8e01f1e08d8275a",
    );
    check::<super::ThresholdKeyLifecycleActionV1>(
        "iroha_data_model::isi::consensus_keys::ThresholdKeyLifecycleActionV1",
        "9726fe2e1b7ccc557be3fdcff488bd5d",
        "9726fe2e1b7ccc557be3fdcff488bd5d",
    );
    check::<super::ThresholdKeyLifecycleSignatureV1>(
        "iroha_data_model::isi::consensus_keys::ThresholdKeyLifecycleSignatureV1",
        "28bb446f73a7a00b4ec258275c4f7792",
        "28bb446f73a7a00b4ec258275c4f7792",
    );
    check::<super::ThresholdKeyLifecycleCertificateV1>(
        "iroha_data_model::isi::consensus_keys::ThresholdKeyLifecycleCertificateV1",
        "026af94cfcc6b7d5234c33a3332f2501",
        "026af94cfcc6b7d5234c33a3332f2501",
    );
}
