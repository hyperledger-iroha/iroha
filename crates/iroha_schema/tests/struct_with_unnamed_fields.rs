//! Schema metadata test for tuple structs and unnamed field handling.
use crate::common::{assert_schema, entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};
#[derive(IntoSchema, Encode, Decode)]
struct Command(String, Vec<String>, #[codec(skip)] bool);
#[test]
fn unnamed() {
    assert_schema::<Command>(
        "unnamed.unnamed",
        &[
            entry::<String>("String"),
            entry::<Vec<String>>("Vec<String>"),
            entry::<Command>("Command"),
        ],
    );
}
