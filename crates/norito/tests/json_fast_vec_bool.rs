#![cfg(feature = "json")]
//! Validate fast-path parsing for Vec<bool> matches the generic parser.

use norito::json::{JsonDeserialize, from_json, from_json_fast};

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Flags {
    flags: Vec<bool>,
}

impl JsonDeserialize for Flags {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut flags: Option<Vec<bool>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "flags" => {
                    flags = Some(p.parse_array::<bool>()?);
                }
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Flags {
            flags: flags.unwrap(),
        })
    }
}

#[test]
fn fast_vec_bool_matches_generic() {
    let s = r#"{"flags":[true,false]}"#;
    let a: Flags = from_json(s).expect("generic parse");
    let b: Flags = from_json_fast(s).expect("fast parse");
    assert_eq!(a, b);
    assert_eq!(b.flags, vec![true, false]);
}
