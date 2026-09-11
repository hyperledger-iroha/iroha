//! Shared checks for the immutable original SDK and dependency frame observations.

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("write fixture digest");
    }
    value
}

fn fixture() -> &'static Vec<Value> {
    static FIXTURE: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(|| {
        let source = include_str!("frame_identity_observations.v1.json");
        assert_eq!(
            hex(&iroha_crypto::sha256(source)),
            "389cf8f47123437d56bc608f5989936bb7fdb1000da104e404dddc3cc73d6631"
        );
        let rows: Vec<Value> = norito::json::from_str(source).expect("original frame observations");
        assert_eq!(rows.len(), 271);
        rows
    })
}

fn assert_direction<T: NoritoSchema>(nominal: &str, direction: &str) {
    let rows: Vec<_> = fixture()
        .iter()
        .filter(|row| {
            row["nominal"].as_str() == Some(nominal) && row["direction"].as_str() == Some(direction)
        })
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "one observation for each owner and direction"
    );
    let row = rows[0];
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), row["root_hint"].as_str().unwrap());
    assert_eq!(
        hex(&norito::schema::identity::frame_hash::<T>()),
        row["schema_hash"].as_str().unwrap()
    );
}

/// Check the serializer's independently observed original frame identity.
pub fn assert_serialize<T: NoritoSerialize>(nominal: &str) {
    assert_direction::<T>(nominal, "serialize");
}

/// Check both independently observed original framing directions.
pub fn assert_bidirectional<T: NoritoSerialize + for<'de> NoritoDeserialize<'de>>(nominal: &str) {
    assert_serialize::<T>(nominal);
    assert_direction::<T>(nominal, "deserialize");
}

/// Require exact package coverage without duplicate or unverified observations.
pub fn assert_package_complete(package: &str, owners: usize, directions: usize) {
    let mut names = std::collections::BTreeSet::new();
    let mut keys = std::collections::BTreeSet::new();
    for row in fixture()
        .iter()
        .filter(|row| row["package"].as_str() == Some(package))
    {
        assert_eq!(row["kind"].as_str(), Some("capture"));
        assert_eq!(row["root_matches"].as_bool(), Some(true));
        let nominal = row["nominal"].as_str().unwrap();
        assert!(keys.insert((nominal, row["direction"].as_str().unwrap())));
        names.insert(nominal);
    }
    assert_eq!(names.len(), owners);
    assert_eq!(keys.len(), directions);
}
