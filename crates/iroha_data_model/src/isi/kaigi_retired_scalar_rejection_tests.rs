//! Exact retired scalar payloads retained solely as negative decoder tests.

use super::{hex, unhex};
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    json::{self, Value},
};
use std::{collections::BTreeMap, fmt::Debug, sync::OnceLock};

fn rejected_capture() -> &'static [Value] {
    static CAPTURE: OnceLock<Value> = OnceLock::new();
    CAPTURE
        .get_or_init(|| {
            use sha2::{Digest as _, Sha256};
            let source = include_str!("../../tests/fixtures/kaigi_retired_scalar_rejections.json");
            assert_eq!(
                hex(&Sha256::digest(source.as_bytes())),
                "702f638cd319d36ee645f07ae12221a499d9317055b491e33f61a35df2609e8d"
            );
            let value: Value = json::from_str(source).expect("retired Kaigi rejection capture");
            let rows = value.as_array().expect("Kaigi rejection rows");
            assert_eq!(rows.len(), 5);
            let names: Vec<_> = rows
                .iter()
                .map(|row| row.get("nominal").and_then(Value::as_str).expect("nominal"))
                .collect();
            assert_eq!(
                names,
                [
                    "iroha_data_model::isi::kaigi::CreateKaigi",
                    "iroha_data_model::isi::kaigi::EndKaigi",
                    "iroha_data_model::isi::kaigi::JoinKaigi",
                    "iroha_data_model::isi::kaigi::LeaveKaigi",
                    "iroha_data_model::isi::kaigi::RecordKaigiUsage",
                ]
            );
            value
        })
        .as_array()
        .expect("retired Kaigi rejection rows")
}

fn reject_retired<T>(case: &Value, field: &str)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug,
{
    let bytes = unhex(
        case.get(field)
            .and_then(Value::as_str)
            .expect("retired frame"),
    );
    let error =
        norito::decode_from_bytes::<T>(&bytes).expect_err("retired scalar payload must not decode");
    assert!(
        matches!(error, norito::core::Error::Message(ref reason)
        if reason == "noncanonical Kaigi authorization Pasta Fp scalar"),
        "{field}: {error:?}"
    );
}

pub(super) fn check<T>(nominal: &str, private_case: usize)
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug,
{
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    let row = rejected_capture()
        .iter()
        .find(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal))
        .expect("retired Kaigi record");
    for field in ["serialize_hash", "deserialize_hash"] {
        assert_eq!(
            row.get(field).and_then(Value::as_str),
            Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
        );
    }
    assert_eq!(
        row.get("private_case_index").and_then(Value::as_u64),
        Some(u64::try_from(private_case).expect("bounded private case index"))
    );
    let expected = row.get("case").expect("retired private case");
    reject_retired::<T>(expected, "frame");
    reject_retired::<Vec<T>>(expected, "vector_frame");
    reject_retired::<Option<T>>(expected, "option_frame");
    reject_retired::<BTreeMap<u8, T>>(expected, "map_frame");
}
