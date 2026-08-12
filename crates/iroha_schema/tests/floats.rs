//! Schema metadata tests for floating-point primitives.

mod common;

use common::{assert_schema, entry};
use iroha_schema::prelude::*;

#[derive(IntoSchema)]
#[allow(dead_code)]
struct FloatFields {
    sample32: f32,
    sample64: f64,
}

#[test]
fn float_primitives_have_explicit_schema_metadata() {
    assert_schema::<FloatFields>(
        "floats.float_primitives_have_explicit_schema_metadata",
        &[
            entry::<f32>("f32"),
            entry::<f64>("f64"),
            entry::<FloatFields>("FloatFields"),
        ],
    );
}
