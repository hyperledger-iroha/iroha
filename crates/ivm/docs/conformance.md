# IVM Conformance & Test Vectors

This checklist pins required golden tests/vectors and their locations to keep implementations stable across changes.

- Decoder/Encoder
  - IVM 32‑bit encodings and 16‑bit compressed forms roundtrip.
  - Files: `crates/ivm/tests/decoder_roundtrip.rs`

- Opcode Semantics
  - Arithmetic/bit/branch/memory ops including overflow and alignment traps.
  - Files: `crates/ivm/tests/op_semantics.rs`

- Pointer‑ABI TLVs
  - TLV structure and big‑endian length; hash field = Iroha Hash (blake2b‑32, LSB set). VM validates version=1, known type_id, bounds within INPUT, and hash.
  - Files: `crates/ivm/tests/tlv_examples.rs`, `crates/ivm/tests/pointer_tlv.rs`
  - Docs: `crates/ivm/docs/tlv_examples.md`

- Syscall ABI
  - Number ranges, argument placement, pointer returns in `r10`, and permission enforcement via `CoreHost`.
  - Docs: `crates/ivm/docs/syscalls.md`

- Gas Schedule
  - Golden vectors tied to header version; bump requires changelog and regenerated vectors.
  - Files: `crates/ivm/tests/gas_golden.rs`

- Predecoder Vectors
  - Mixed 16/32-bit streams decode identically across allowed header/metadata variants.
  - Generator: `cargo run -p ivm --bin ivm_predecoder_export`
  - Output: `crates/ivm/tests/fixtures/predecoder/mixed/` (`code.bin`, `decoded.json`, `index.json`, `artifacts/*.to`)

- Cross‑Backend Vectors
  - Vector/crypto helpers produce identical outputs across scalar/SIMD/GPU.
  - Files: `crates/ivm/tests/crypto_vectors.rs`

- ZK Mode
  - Trace commitment and padding semantics; ASSERT behavior in ZK vs non‑ZK.
  - Files: `crates/ivm/tests/zk_trace.rs`

Maintainers: update this file when adding new opcodes, syscalls, or encoding changes. Tie each change to explicit tests.
