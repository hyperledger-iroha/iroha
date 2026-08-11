//! pass: FastJsonWrite emits a checked path and accepts explicit custom seams

use norito::json::{BoundedJsonError, JsonSerialize, JsonWriteSink};

mod quoted {
    use super::*;

    pub fn serialize(value: &u64, output: &mut String) {
        output.push('"');
        output.push_str(&value.to_string());
        output.push('"');
    }

    pub fn serialize_bounded(
        value: &u64,
        output: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        output.push('"')?;
        value.json_serialize_to(output)?;
        output.push('"')
    }
}

#[derive(norito::derive::FastJsonWrite)]
struct Item {
    id: u64,
    #[norito(with = "quoted", bounded_with = "quoted::serialize_bounded")]
    displayed: u64,
}

fn main() {
    let item = Item {
        id: 7,
        displayed: 9,
    };
    let ordinary = norito::json::to_json(&item).expect("ordinary JSON");
    let bounded = norito::json::to_json_bounded(&item, ordinary.len()).expect("bounded JSON");
    assert_eq!(bounded, ordinary);
}
