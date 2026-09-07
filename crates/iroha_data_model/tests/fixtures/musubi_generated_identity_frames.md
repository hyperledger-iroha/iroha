# Generated Musubi identity capture

`musubi_generated_identity_frames.json` was recorded on 2026-09-07 before the
three Musubi model generators acquired explicit identity parameters. It covers
12 digest types, three bounded text types and two page types. The 49 values
include digest byte patterns, minimum/Unicode/maximum-length text and empty or
populated pages validated against the canonical SDK fixture. Each value has
JSON and complete root, `Vec`, `Option` and `BTreeMap<u8, T>` frames: 196 frames.
The SORA address-format context is scoped to the test thread.

Fixture SHA-256:
`38646b081ce079897b73f62d8f4d91f6176e1aa0d378c6b56715fdce1f4c693d`.
Every nominal name and both directional hashes also matched the controlled
model capture documented in [the identity contract](../../../../specs/norito_schema_identity.md),
report SHA-256
`be82d3661d9e2a79fd1a60d6922f1387d3821a0ad5a65d80d294aca1251caedd`.

The unchanged-generator source hashes through capture were:

| Source | SHA-256 |
| --- | --- |
| `src/musubi.rs` | `4a243d6c8c7ae1e65771af6e0a54d2c6eca0896f07fe7224e66e1ef199d95c22` |
| `src/musubi/query_models.rs` | `2d5d4b2285a048fecdb4dc1b220b35e7456df182f50404986c3666190dadaa3b` |

The temporary writer was removed. The permanent
`generated_musubi_identities_preserve_captured_frames` test only reads this
fixture, compares all values and frames, and roundtrips each frame. This is
local preparation evidence, not the atomic codec cutover or native/release
qualification.
