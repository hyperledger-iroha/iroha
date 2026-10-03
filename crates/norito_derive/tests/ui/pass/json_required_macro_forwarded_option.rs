//! pass: macro-forwarded Option types retain required JSON and binary derive support.
use norito::derive::{Decode, Encode, JsonDeserialize, JsonSerialize};

macro_rules! forwarded_options {
    ($name:ident, $ty:ty) => {
        #[derive(
            Debug,
            PartialEq,
            Eq,
            Decode,
            Encode,
            JsonDeserialize,
            JsonSerialize,
            norito::NoritoSchema,
        )]
        #[norito_schema(name = "norito_derive::ui::ForwardedOptions")]
        struct $name {
            #[norito(required)]
            required: $ty,
            optional: $ty,
        }
    };
}
forwarded_options!(ForwardedOptions, Option<u32>);

fn main() {
    let value = ForwardedOptions {
        required: None,
        optional: Some(7),
    };
    let original = norito::json::to_json(&value).unwrap();
    assert_eq!(
        norito::json::from_json::<ForwardedOptions>(&original).unwrap(),
        value
    );
    let bytes = norito::to_bytes(&value).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<ForwardedOptions>(&bytes).unwrap(),
        value
    );
}
