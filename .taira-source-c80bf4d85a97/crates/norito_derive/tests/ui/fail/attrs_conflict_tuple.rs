//! fail: conflicting skip+default on tuple field

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Tup(
    #[norito(skip)]
    #[norito(default)]
    u32,
);

fn main() {}
