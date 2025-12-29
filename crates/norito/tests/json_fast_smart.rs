#![cfg(feature = "json")]
//! Validate that from_json_fast_smart parses small inputs via the typed parser.

use norito::json::from_json_fast_smart;

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Item {
    id: u64,
    name: String,
    flag: bool,
}

// Implement the generic typed parser contract
impl norito::json::JsonDeserialize for Item {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        // Parse three fields in any order
        let mut id = None;
        let mut name = None;
        let mut flag = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => {
                    id = Some(p.parse_u64()?);
                }
                "name" => {
                    name = Some(p.parse_string()?);
                }
                "flag" => {
                    flag = Some(p.parse_bool()?);
                }
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Item {
            id: id.unwrap(),
            name: name.unwrap(),
            flag: flag.unwrap(),
        })
    }
}

#[test]
fn smart_prefers_small_typed_parser() {
    let s = "{\"id\":1,\"name\":\"x\",\"flag\":true}";
    let it: Item = from_json_fast_smart(s).expect("parse");
    assert_eq!(
        it,
        Item {
            id: 1,
            name: "x".to_string(),
            flag: true
        }
    );
}
