//! Aggregated integration tests for `iroha_schema`.

mod common;

#[path = "architecture-dependent.rs"]
mod architecture_dependent;
#[path = "enum_with_default_discriminants.rs"]
mod enum_with_default_discriminants;
#[path = "enum_with_various_discriminants.rs"]
mod enum_with_various_discriminants;
#[path = "fieldless_enum.rs"]
mod fieldless_enum;
#[path = "floats.rs"]
mod floats;
#[path = "non_zero.rs"]
mod non_zero;
#[path = "numbers_compact_and_fixed.rs"]
mod numbers_compact_and_fixed;
#[path = "schema_json.rs"]
mod schema_json;
#[path = "struct_with_generic_bounds.rs"]
mod struct_with_generic_bounds;
#[path = "struct_with_named_fields.rs"]
mod struct_with_named_fields;
#[path = "struct_with_unnamed_fields.rs"]
mod struct_with_unnamed_fields;
#[path = "transparent_types.rs"]
mod transparent_types;
