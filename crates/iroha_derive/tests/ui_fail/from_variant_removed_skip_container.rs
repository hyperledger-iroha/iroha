//! Ensures the retired implicit-container conversion control is rejected.

#[derive(iroha_derive::FromVariant)]
enum Example {
    Value(#[skip_container] Box<u8>),
}

fn main() {}
