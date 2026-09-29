//! pass: binary derives on structs and enums mixing fixed-size, raw byte-array,
//! self-delimiting and optional fields, including a flattened named field.
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
#[norito(decode_from_slice)]
struct Inner {
    x: u32,
    label: String,
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Mixed {
    a: u32,
    b: [u8; 16],
    c: String,
    d: Option<u64>,
    e: Vec<u8>,
    #[norito(flatten)]
    inner: Inner,
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Tuple(u16, [u8; 8], String);

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
enum Message {
    Unit,
    Tuple(u32, [u8; 16], Option<String>),
    Named { digest: [u8; 32], values: Vec<u32> },
}

fn main() {}
