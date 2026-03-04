//! pass: derive FastJson/FastJsonWrite on a named struct

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Item {
    id: u64,
    name: String,
    flag: bool,
    opt: Option<u32>,
}

fn main() {}
