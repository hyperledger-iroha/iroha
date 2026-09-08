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
    assert_eq!(
        <T as norito::NoritoSerialize>::schema_hash(),
        serialize_hash
    );
    assert_eq!(
        <T as norito::NoritoDeserialize>::schema_hash(),
        deserialize_hash
    );
}

#[test]
fn captured_ordinary_codec_schema_identities() {
    check::<super::SettlementPlan>(
        "iroha_data_model::isi::settlement::SettlementPlan",
        "6acc2d9981883db057bb4aed18f48d50",
        "6acc2d9981883db057bb4aed18f48d50",
    );
    check::<super::SettlementLeg>(
        "iroha_data_model::isi::settlement::SettlementLeg",
        "d34579868012ccc793daa2dc01f28b60",
        "d34579868012ccc793daa2dc01f28b60",
    );
    check::<super::FxCorridorId>(
        "iroha_data_model::isi::settlement::FxCorridorId",
        "258e066012c7a10adafbcdbac20e009c",
        "258e066012c7a10adafbcdbac20e009c",
    );
    check::<super::FxCorridorOracleEvidence>(
        "iroha_data_model::isi::settlement::FxCorridorOracleEvidence",
        "adaf16e994ad76f205da0e2ae346f9ad",
        "adaf16e994ad76f205da0e2ae346f9ad",
    );
    check::<super::FxCorridorUsage>(
        "iroha_data_model::isi::settlement::FxCorridorUsage",
        "c588208e58d4eb1f7181270655618b06",
        "c588208e58d4eb1f7181270655618b06",
    );
    check::<super::FxCorridorPolicy>(
        "iroha_data_model::isi::settlement::FxCorridorPolicy",
        "e3678da019e434bf440512f2fb59d606",
        "e3678da019e434bf440512f2fb59d606",
    );
    check::<super::FxCorridorPolicyRegistry>(
        "iroha_data_model::isi::settlement::FxCorridorPolicyRegistry",
        "7d6aba1c78b3a991807b272ba49a302d",
        "7d6aba1c78b3a991807b272ba49a302d",
    );
}
