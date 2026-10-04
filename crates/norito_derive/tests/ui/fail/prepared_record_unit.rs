//! A unit record has no positional destination fields.
#[derive(norito::DeserializePayload)]
#[norito(decode_fields)]
struct Unit;
fn main() {}
