# ISO 20022 schema table v1

`schema_v1.tsv` is the canonical package-local projection of the private ISO
20022 message schemas. Its ordered records retain the message dispatch order,
field order, required/optional and occurrence bounds, field kinds and enum
members, and alias order byte for byte. The build script validates the complete
inventory before emitting the same private static `MessageSchema` values into
`OUT_DIR`; validation remains allocation-free after compilation.

The repository compile-time asset checker pins the asset and its sole current
Rust `include_str!` consumer. A focused standard-library Python test verifies
the closed record inventory and the reviewed PACS009 canonical alias targets.
