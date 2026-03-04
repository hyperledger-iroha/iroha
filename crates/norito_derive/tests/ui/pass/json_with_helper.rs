#![allow(unexpected_cfgs)]
//! pass: Json derives invoke `#[norito(with = ...)]` helper for fields lacking
//! built-in trait implementations.

#[cfg(feature = "json")]
mod json_with_helper {
    use norito::derive::{JsonDeserialize, JsonSerialize};
    use norito::json::{self, JsonDeserialize as _, JsonSerialize as _, Parser};
    use std::vec::Vec;

    #[derive(Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
    struct Wrapper {
        #[norito(with = "helpers")]
        payload: [u8; 4],
    }

    mod helpers {
        use super::*;

        pub fn serialize(bytes: &[u8; 4], out: &mut String) {
            let buf: Vec<u8> = bytes.to_vec();
            JsonSerialize::json_serialize(&buf, out);
        }

        pub fn deserialize(parser: &mut Parser<'_>) -> Result<[u8; 4], json::Error> {
            let buf = Vec::<u8>::json_deserialize(parser)?;
            let boxed: Box<[u8]> = buf.into_boxed_slice();
            boxed.try_into().map_err(|slice: Box<[u8]>| {
                json::Error::Message(format!("expected 4 bytes, got {}", slice.len()))
            })
        }
    }

    #[test]
    fn roundtrip() {
        let input = Wrapper {
            payload: [1, 2, 3, 4],
        };
        let json = json::to_json(&input).expect("serialize");
        assert_eq!(json, "{\"payload\":[1,2,3,4]}");
        let decoded: Wrapper = json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, input);
    }
}

fn main() {}
