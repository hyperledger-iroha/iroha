//! fail: conflicting skip+default on named field

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Bad {
    #[norito(skip)]
    #[norito(default)]
    a: u32,
}

fn main() {}
