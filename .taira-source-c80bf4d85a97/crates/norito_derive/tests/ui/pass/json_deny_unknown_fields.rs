//! pass: strict JSON derives support named structs, flattening, and tagged enums

use norito::derive::{JsonDeserialize, JsonSerialize};

#[derive(JsonDeserialize, JsonSerialize)]
struct Extra {
    label: String,
}

#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StrictStruct {
    id: u32,
    #[norito(flatten)]
    extra: Extra,
}

#[derive(JsonDeserialize, JsonSerialize)]
#[norito(tag = "kind", content = "payload", deny_unknown_fields)]
enum StrictEnum {
    Unit,
    Record { id: u32 },
}

#[derive(norito::derive::FastJson)]
#[norito(deny_unknown_fields)]
struct StrictFastStruct {
    id: u32,
}

#[derive(norito::derive::FastJson)]
#[norito(tag = "kind", content = "payload", deny_unknown_fields)]
enum StrictFastEnum {
    Unit,
    Record { id: u32 },
}

fn main() {}
