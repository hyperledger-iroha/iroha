//! fail: rename not allowed on tuple fields

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Tup(#[norito(rename = "x")] u32, String);

fn main() {}
