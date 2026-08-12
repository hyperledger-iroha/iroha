//! `IntoSchema` derive tests for named fields.

mod common;

use common::{assert_schema, entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};

#[derive(IntoSchema, Encode, Decode)]
struct Command {
    executable: String,
    args: Vec<String>,
    #[codec(skip)]
    mock: bool,
    num: i32,
}

#[test]
fn named_fields() {
    assert_schema::<Command>(
        "named.named_fields",
        &[
            entry::<String>("String"),
            entry::<Vec<String>>("Vec<String>"),
            entry::<i32>("i32"),
            entry::<Command>("Command"),
        ],
    );
}
