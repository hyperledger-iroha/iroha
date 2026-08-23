Attribute macro to attach a stable wire identifier to an instruction type.

Usage: `#[instruction(id = "iroha.log")]` applied to a struct/enum.
It generates an inherent associated constant `WIRE_ID: &str` on the type.

This macro does not alter registration automatically; the registry can
choose to use the constant, or you can pass the same ID into
`InstructionRegistry::register_with_id::<T>(id)`.
