//! Tuple struct derives for JsonSerialize/JsonDeserialize.

use norito::json::{self, JsonDeserialize, JsonSerialize};

#[derive(Debug, PartialEq, JsonSerialize, JsonDeserialize)]
struct Pair(u32, Option<String>);

#[derive(Debug, PartialEq, JsonSerialize, JsonDeserialize)]
struct Trio(
    u32,
    #[norito(default)] bool,
    #[norito(default)] Option<String>,
);

#[derive(Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "payload", no_fast_from_json)]
enum TupleEnum {
    Pair(Pair),
    Trio(Trio),
}

#[test]
fn tuple_struct_roundtrip() {
    let value = Pair(7, Some("iroha".into()));
    let json = json::to_json(&value).expect("serialize");
    assert_eq!(json, "[7,\"iroha\"]");

    let decoded: Pair = json::from_json("[7,null]").expect("deserialize");
    assert_eq!(decoded, Pair(7, None));
}

#[test]
fn tuple_struct_defaults() {
    let json = "[1]";
    let decoded: Trio = json::from_json(json).expect("deserialize defaults");
    assert_eq!(decoded, Trio(1, false, None));

    let encoded = json::to_json(&decoded).expect("serialize defaults");
    assert_eq!(encoded, "[1,false,null]");
}

#[test]
fn tuple_enum_roundtrip() {
    let value = TupleEnum::Pair(Pair(42, Some("iroha".into())));
    let json = json::to_json(&value).expect("serialize enum tuple variant");
    assert_eq!(json, r#"{"kind":"Pair","payload":[42,"iroha"]}"#);

    let decoded: TupleEnum = json::from_json(r#"{"kind":"Trio","payload":[7,true,"note"]}"#)
        .expect("deserialize enum tuple variant");
    assert_eq!(decoded, TupleEnum::Trio(Trio(7, true, Some("note".into()))));
}
