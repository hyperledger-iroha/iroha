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
macro_rules! record {
    ($ty:ty) => {
        #[derive(JsonDeserialize, JsonSerialize, norito_derive::FastJson)]
        #[norito(no_fast_from_json)]
        struct ForwardedRequired {
            #[norito(required)]
            value: $ty,
            optional: $ty,
        }
    };
}
record!(Option<u32>);
fn main() {}
