# ISO 20022 schema table v1

`schema_v1.tsv` is the canonical package-local projection of the private ISO
20022 message schemas. Its ordered records retain the message dispatch order,
field order, required/optional and occurrence bounds, field kinds and enum
members, and alias order byte for byte. The build script validates the complete
inventory before emitting the same private static `MessageSchema` values into
`OUT_DIR`; validation remains allocation-free after compilation.

The repository compile-time asset checker pins the asset and the exact
historical Rust span from which it was projected. A focused standard-library
Python test independently reconstructs every record from that preimage.
