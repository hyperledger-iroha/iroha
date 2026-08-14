//! pass: required JSON keys may carry an explicit null Option value.
use norito::derive::{JsonDeserialize, JsonSerialize};
#[derive(JsonDeserialize, JsonSerialize)]
struct RequiredStruct {
    #[norito(required)]
    value: Option<u32>,
}
#[derive(JsonDeserialize, JsonSerialize)]
#[norito(tag = "kind", content = "payload")]
enum RequiredEnum {
    Value {
        #[norito(required)]
        value: Option<u32>,
    },
}
fn main() {}
