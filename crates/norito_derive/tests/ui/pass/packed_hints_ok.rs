#![allow(unexpected_cfgs)]
//! pass: fields that exercise hybrid packed-struct classification hints
//! - fixed-size: u32, [u8; 16]
//! - self-delimiting: String
//! - option/result presence

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Hints {
    a: u32,
    b: [u8; 16],
    c: String,
    d: Option<u64>,
}

fn main() {}
