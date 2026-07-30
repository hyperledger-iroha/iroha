//! fail: two variants must not share one canonical Norito index

#[derive(norito::NoritoSerialize)]
enum DuplicateIndex {
    #[codec(index = 1)]
    First,
    Second,
}

fn main() {}
