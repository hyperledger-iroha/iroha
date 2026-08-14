//! fail: required is restricted to present named Option fields.
use norito::derive::JsonDeserialize;
#[derive(JsonDeserialize)]
struct NonOption {
    #[norito(required)]
    value: u32,
}
#[derive(JsonDeserialize)]
struct Defaulted {
    #[norito(required, default)]
    value: Option<u32>,
}
#[derive(JsonDeserialize)]
struct Skipped {
    #[norito(required, skip)]
    value: Option<u32>,
}
#[derive(JsonDeserialize)]
struct Flattened {
    #[norito(required, flatten)]
    value: Option<u32>,
}
#[derive(JsonDeserialize)]
struct ConditionallySerialized {
    #[norito(required, skip_serializing_if = "Option::is_none")]
    value: Option<u32>,
}
#[derive(JsonDeserialize)]
struct Tuple(#[norito(required)] Option<u32>);
#[derive(JsonDeserialize)]
struct Duplicate {
    #[norito(required, required)]
    value: Option<u32>,
}
fn main() {}
