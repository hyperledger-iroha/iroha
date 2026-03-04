#![allow(unexpected_cfgs)]
//! pass: allowed attributes on named fields

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct WithAttrs {
    #[norito(rename = "id2")]
    id: u64,
    #[norito(skip)]
    skip_me: u32,
    #[norito(default)]
    opt: Option<u32>,
}

fn main() {}
