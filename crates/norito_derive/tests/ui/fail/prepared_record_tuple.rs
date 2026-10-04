//! Prepared field traversal requires a named positional record.
#[derive(norito::DeserializePayload)]
#[norito(decode_fields)]
struct Tuple(u8);
fn main() {}
