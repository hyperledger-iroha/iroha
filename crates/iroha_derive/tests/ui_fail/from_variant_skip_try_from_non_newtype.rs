//! Ensures conversion controls are rejected where no conversion can be generated.

#[derive(iroha_derive::FromVariant)]
enum Example {
    Invalid {
        #[skip_try_from]
        value: u32,
    },
}

fn main() {}
