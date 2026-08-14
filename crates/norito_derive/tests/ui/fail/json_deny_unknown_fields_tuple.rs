//! fail: strict JSON object fields are not meaningful for tuple structs
use norito::derive::JsonDeserialize;
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Tuple(u32);
fn main() {}
