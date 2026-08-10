//! Exact conformance guard for the generated Musubi SDK V1 fixture.

use norito::json::{self, Value};

use super::musubi_sdk_fixture_values;

const FIXTURE: &str = include_str!("../../../fixtures/musubi/sdk_v1.json");

#[test]
fn shared_musubi_sdk_fixture_matches_its_typed_owner() {
    let actual: Value = json::from_str(FIXTURE).expect("parse Musubi SDK fixture");
    let expected = musubi_sdk_fixture_values::sdk_document();
    assert_eq!(
        actual, expected,
        "regenerate sdk_v1.json with the registered typed owner"
    );

    let canonical = format!(
        "{}\n",
        json::to_string_pretty(&actual).expect("render canonical SDK fixture")
    );
    let decoded: Value =
        json::from_str(&canonical).expect("decode canonical SDK fixture rendering");
    assert_eq!(decoded, actual);
    assert_eq!(canonical, FIXTURE, "SDK fixture bytes are canonical");
}
