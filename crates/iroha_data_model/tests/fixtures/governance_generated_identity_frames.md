# Generated governance hash identity capture

`governance_generated_identity_frames.json` was captured on 2026-09-07 before
`define_hash32_newtype!` acquired explicit identity parameters. These 16 types
implement their codecs by hand and were outside the derive-probe inventory.
The capture records the compiler's actual nominal names and both active codec
hashes, plus 64 values with JSON and complete root, `Vec`, `Option` and
`BTreeMap<u8, T>` frames: 256 frames in total. Input patterns include zero,
all-one bits, repeated `0x42` and increasing bytes.

Fixture SHA-256:
`1347ecfc4d60160b19d8ec2a5a25b73d41706f54206de4451b4771ad8a2eb53e`.
The generator source `src/governance/types.rs` retained SHA-256
`992b2e6cd0b3fc591ef3518e3fed06f7d02d268169ec78c626e0b435476badcb`
through the capture. Despite the physical file path, the compiler names these
types under `iroha_data_model::parliament_types`; the explicit literals come
from the capture rather than a reconstructed source path.

The one-time writer was removed before declaration migration. The permanent
`generated_governance_hashes_preserve_captured_frames` test compares the explicit
identities and both directional hashes, verifies every JSON value and complete
frame, and roundtrips each frame. The handwritten codec bodies are unchanged.
This is local declaration-preparation evidence; the atomic active-codec cutover,
other features, native platforms and source-bound release qualification remain
separate obligations.
