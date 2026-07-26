//! JSON derive coverage for enums using tag/content encoding.

use norito::json;

#[derive(Debug, PartialEq, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "kind", content = "payload", no_fast_from_json)]
enum Message {
    Ping,
    Data(u32),
    Pair(u32, bool),
    Record { id: u32, label: Option<String> },
}

#[derive(Debug, PartialEq, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "type", content = "body", no_fast_from_json)]
enum Renamed {
    #[norito(rename = "pong")]
    Pong,
    #[norito(rename = "data")]
    Data(u64),
}

#[test]
fn message_roundtrip() {
    let cases = [
        Message::Ping,
        Message::Data(7),
        Message::Pair(3, true),
        Message::Record {
            id: 42,
            label: Some("iroha".into()),
        },
    ];

    for case in cases {
        let json = json::to_json(&case).expect("serialize enum");
        let decoded: Message = json::from_json(&json).expect("deserialize enum");
        assert_eq!(decoded, case);
    }
}

#[test]
fn renamed_variants() {
    let value = Renamed::Data(10);
    let json = json::to_json(&value).expect("serialize renamed");
    assert_eq!(json, "{\"type\":\"data\",\"body\":10}");

    let decoded: Renamed =
        json::from_json("{\"type\":\"pong\",\"body\":null}").expect("deserialize renamed");
    assert_eq!(decoded, Renamed::Pong);
}
