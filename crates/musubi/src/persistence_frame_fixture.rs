//! Assertions against bounded, source-exact original persistence codec observations.

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};
use std::{fmt::Debug, sync::OnceLock};

fn fixture() -> &'static Value {
    static FIXTURE: OnceLock<Value> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let rows: Value = norito::json::from_str(include_str!(
            "../tests/fixtures/persistence_frame_identity_v1.json"
        ))
        .expect("immutable original persistence capture");
        assert_eq!(rows["identities"].as_array().expect("identities").len(), 13);
        assert_eq!(rows["frames"].as_array().expect("frames").len(), 32);
        rows
    })
}

fn identity<T: NoritoSchema>(row: &Value) {
    let nominal = row["nominal"].as_str().expect("observed nominal");
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    let actual = hex::encode(norito::schema::identity::frame_hash::<T>());
    assert_eq!(actual, row["serialize_hash"].as_str().expect("writer hash"));
    assert_eq!(
        actual,
        row["deserialize_hash"].as_str().expect("reader hash")
    );
}

fn frame<T>(owner: &str, shape: &str, value: &T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + PartialEq + Debug,
{
    let matches: Vec<_> = fixture()["frames"]
        .as_array()
        .expect("frame rows")
        .iter()
        .filter(|row| row["owner"].as_str() == Some(owner) && row["shape"].as_str() == Some(shape))
        .collect();
    assert_eq!(matches.len(), 1, "one captured {owner}/{shape} frame");
    let row = matches[0];
    identity::<T>(row);
    let expected = hex::decode(row["frame_hex"].as_str().expect("frame hex"))
        .expect("captured canonical bytes");
    assert_eq!(
        &expected[6..22],
        norito::schema::identity::frame_hash::<T>().as_slice()
    );
    for ambient in [
        0,
        norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
    ] {
        let _flags = norito::core::DecodeFlagsGuard::enter(ambient);
        let actual = norito::encode_canonical(value).expect("canonical writer");
        assert_eq!(actual, expected, "{owner}/{shape}");
        let decoded: T = norito::decode_canonical(&expected).expect("canonical reader");
        assert_eq!(&decoded, value);
    }
    for end in 0..expected.len() {
        assert!(
            norito::decode_canonical::<T>(&expected[..end]).is_err(),
            "truncated {owner}/{shape} at {end}"
        );
    }
    let mut trailing = expected.clone();
    trailing.push(0xa5);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let mut substituted = expected;
    substituted[6] ^= 0x80;
    assert!(matches!(
        norito::decode_canonical::<T>(&substituted),
        Err(norito::Error::SchemaMismatch)
    ));
}

/// Check both root identities and every independently captured container shape.
pub fn assert_captured<T>(nominal: &str, value: Option<&T>)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + PartialEq + Debug,
{
    let matches: Vec<_> = fixture()["identities"]
        .as_array()
        .expect("identity rows")
        .iter()
        .filter(|row| row["nominal"].as_str() == Some(nominal))
        .collect();
    assert_eq!(matches.len(), 1, "one original root identity");
    identity::<T>(matches[0]);
    let owner = nominal.rsplit("::").next().expect("owner name");
    frame(owner, "option_none", &Option::<T>::None);
    frame(owner, "vec_empty", &Vec::<T>::new());
    if let Some(value) = value {
        frame(owner, "root", value);
        frame(owner, "option_some", &Some(value.clone()));
        frame(owner, "vec_two", &vec![value.clone(), value.clone()]);
    }
}
