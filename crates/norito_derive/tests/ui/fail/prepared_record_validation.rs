//! Prepared traversal must not omit an existing whole-value validation hook.
#[derive(norito::DeserializePayload)]
#[norito(decode_fields, validate = "Self::validate")]
struct Checked {
    value: u8,
}
fn main() {}
