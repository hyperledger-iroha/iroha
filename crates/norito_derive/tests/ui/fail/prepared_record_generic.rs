//! Generic prepared graphs require a separately proven destination contract.
#[derive(norito::DeserializePayload)]
#[norito(decode_fields)]
struct Generic<T> {
    value: T,
}
fn main() {}
