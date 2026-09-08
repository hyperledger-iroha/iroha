# Generated privacy carrier identity capture

`privacy_generated_identity_frames.json` was captured on 2026-09-07 before
the four privacy/spentness generators acquired required identity literals.
It records 62 types, 248 JSON values and complete root, `Vec`, `Option` and
`BTreeMap<u8, T>` frames: 992 frames in total. Every JSON value and every
frame is roundtripped during capture and checking.

The 56 digest and two Ristretto byte carriers use zero, all-one bits,
repeated `0x42` and increasing-byte inputs. These exercise byte preservation,
including sentinels; they are not claims of valid curve points or admission.
The two ZK-ACE and two spentness wrappers use canonical six-lane Goldilocks
values: zero, one, modulus-minus-one and distinct increasing lanes. Native
point checks, field canonicality and protocol admission retain their owners.

Fixture SHA-256:
`27f07e37bcb2c573f36e735009d27d3b18808f0a5946fbcf46908ba64047f4b4`.
The unchanged production sources during capture were:

| Source | SHA-256 |
| --- | --- |
| `src/privacy.rs` | `9ba40c8607d957537ca9dfdb25075af600535b37d7b82b1704947cbdfc3b2201` |
| `src/confidential/spentness.rs` | `3c0ffd2bc35b5d230c2c8ad7ab6cd936260ccc9dcd60d6573985a861193a6369` |

All 62 actual compiler names and both directional hashes match the earlier
controlled model capture's generated-family review, SHA-256
`2e00a0c82cc36b5b14d97586bd392f4563fde0de303b7414724a2822a29f2778`.
The explicit literals come from that capture. The one-time writer was removed
before migrating the declarations. The permanent
`generated_privacy_carriers_preserve_captured_frames` test compares every
identity, hash, JSON value and complete frame with the immutable fixture.
Constructor, codec and validation bodies are unchanged. This is declaration
preparation; active codec cutover, other features and release qualification
remain separate obligations.

The four generated-model fixture suites (query, privacy, Musubi and governance)
pass together after the declaration changes. Their success is local fixture
evidence, not complete model or native qualification.
