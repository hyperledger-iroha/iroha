# Generated instruction enum identity capture

`instruction_enum_generated_identity_frames.json` was captured on 2026-09-07
before `enum_type!` acquired its identity derive and the ten callers covered by
the historical inventory acquired explicit identity literals. It records all
53 variants of those types, their numeric tags and
JSON values, plus 212 complete root, `Vec`, `Option` and `BTreeMap<u8, T>` frames.
Every frame and JSON value is roundtripped during capture and checking. All 256
possible byte tags are checked for each type; tags outside the captured variant
inventory and unknown JSON names must reject.

Fixture SHA-256:
`f4db30a2d6aced419580291fea8625603b09eba435d47fcff32e5e7b4220cc15`.
The sources before adding the test module were:

| Source | SHA-256 |
| --- | --- |
| `src/isi/mod.rs` | `5b63bb000a7503784af7f331bb7eec9ca718f1fbbaa7bd15c92bef35518f016c` |
| `src/isi/register.rs` | `512044382c7bea87b61ebafa400f758b2e21bda330a4faacc9be42616018c626` |
| `src/isi/settlement.rs` | `e1790bac77af024034ada47daa5cf7234f906db72ed10848264b759450d8e1b6` |
| `src/isi/transfer.rs` | `69a063af357962f0a9a0c85d683faca3529f9761b08ae304b9dfef3bb7a7d40f` |

Production declarations and codec bodies remained unchanged during capture;
only the test module registration was appended. All ten actual compiler names
and both directional hashes match the controlled model capture's generated
family review, SHA-256
`2e00a0c82cc36b5b14d97586bd392f4563fde0de303b7414724a2822a29f2778`.
The one-time writer was removed before adding the production declarations.

The compiler exposed two additional current callers, `MintType` and `BurnType`,
absent from that historical review. They were separately captured with the
original generator and unchanged `src/isi/mint_burn.rs` (SHA-256
`c1bfdbc8589e0e457e0743e540d81cff4b4c12d8d8981afd22d0c33951c48417`).
The supplemental `instruction_mint_burn_generated_identity_frames.json`
preserves their four variants and 16 complete frames; its SHA-256 is
`1a98fcde9bfa97a16af0412e139684fe7ee841409760cef4a896eaf4bd2de147`.
Their literals come from this additional compiler capture, not an inferred
module path. Both directional hashes agree. The supplemental writer was also
removed before adding the declarations, and the original ten-type fixture was
not rewritten. Together the fixtures cover all 12 current callers, 57 variants
and 228 frames. Historical inventory counts remain historical observations.

The permanent owner-scoped `generated_instruction_enums_preserve_captured_frames`
test and its mint/burn counterpart check all identities, tags, JSON values and complete frames against the
immutable fixture. Private discriminators remain private. This is declaration
preparation; it does not cut over the active codecs, qualify unobserved feature
selections or establish source-bound release qualification.
