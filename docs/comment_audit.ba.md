---
lang: ba
direction: ltr
source: docs/comment_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56951fb1e07d90ed062c24f6ef80b8e211b37be008becba9a1fdb16d522d5230
source_last_modified: "2026-01-21T19:17:13.230513+00:00"
translation_last_reviewed: 2026-02-07
---

# Comment Audit Notes

Checked the following IVM modules and confirmed their inline/docs comments match the current behaviour (no code edits required):

- `crates/ivm/src/runtime.rs`

- `crates/ivm/src/memory.rs`
- `crates/ivm/src/register_file.rs`
- `crates/ivm/src/registers.rs`
- `crates/ivm/src/decoder.rs`
- `crates/ivm/src/core_host.rs`
- `crates/ivm/src/ivm.rs`
- `crates/ivm/src/vector.rs`
- `crates/ivm/src/host.rs`
- `crates/ivm_abi/src/syscalls.rs`
- `crates/ivm/src/parallel.rs`
- `crates/ivm/src/mock_wsv.rs`
- `crates/ivm/src/axt.rs`
- `crates/ivm/src/byte_merkle_tree.rs`
- `crates/ivm/src/merkle_utils.rs`
- `crates/ivm/src/gas.rs`
- `crates/ivm/src/error.rs`
- `crates/ivm/src/instruction.rs`
- `crates/ivm/src/encoding.rs`
- `crates/ivm/src/iso20022.rs`
- `crates/ivm/src/halo2.rs`
- `crates/ivm/src/signature.rs`
- `crates/ivm_abi/src/pointer_abi.rs`
- `crates/ivm/src/ivm_cache.rs`

Remaining workspace crates still need a pass if further comment updates become necessary.
