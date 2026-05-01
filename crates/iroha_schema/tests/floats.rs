//! Schema metadata tests for floating-point primitives.

use std::any::TypeId;

use iroha_schema::prelude::*;

#[derive(IntoSchema)]
#[allow(dead_code)]
struct FloatFields {
    sample32: f32,
    sample64: f64,
}

#[test]
fn float_primitives_have_explicit_schema_metadata() {
    let schema = FloatFields::schema();

    assert_eq!(
        schema.get::<f32>(),
        Some(&Metadata::Float(FloatMode::Binary32))
    );
    assert_eq!(
        schema.get::<f64>(),
        Some(&Metadata::Float(FloatMode::Binary64))
    );
    assert_eq!(
        schema.get::<FloatFields>(),
        Some(&Metadata::Struct(NamedFieldsMeta {
            declarations: vec![
                Declaration {
                    name: "sample32".to_owned(),
                    ty: TypeId::of::<f32>(),
                },
                Declaration {
                    name: "sample64".to_owned(),
                    ty: TypeId::of::<f64>(),
                },
            ],
        }))
    );
}
