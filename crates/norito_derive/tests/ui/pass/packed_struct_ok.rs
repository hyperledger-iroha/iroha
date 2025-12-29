#![allow(unexpected_cfgs)]
//! pass: derive NoritoSerialize/NoritoDeserialize on a mixed struct

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Mixed {
    a: u32,
    b: String,
    c: Option<u64>,
    d: [u8; 16],
}

fn main() {}
