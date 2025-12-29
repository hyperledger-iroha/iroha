//! fail: FastJson derives are only supported on structs

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite)]
enum E {
    A { id: u64 },
}

fn main() {}
