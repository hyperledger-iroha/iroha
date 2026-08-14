//! Exact conformance guard for the generated Musubi V1 instruction fixture.
use super::musubi_fixture_values;
use norito::json::{self, Value};
const FIXTURE: &str = include_str!("../../../fixtures/musubi/instructions_v1.json");
#[test]
fn shared_musubi_instruction_fixture_matches_its_typed_owner() {
    let actual: Value = json::from_str(FIXTURE).expect("parse Musubi instruction fixture");
    let expected = musubi_fixture_values::instruction_document();
    assert_eq!(
        actual, expected,
        "regenerate instructions_v1.json with the registered typed owner"
    );
    let canonical = format!(
        "{}\n",
        json::to_string_pretty(&actual).expect("render canonical instruction fixture")
    );
    let decoded: Value =
        json::from_str(&canonical).expect("decode canonical instruction fixture rendering");
    assert_eq!(decoded, actual);
    assert_eq!(
        canonical, FIXTURE,
        "instruction fixture bytes are canonical"
    );
}
