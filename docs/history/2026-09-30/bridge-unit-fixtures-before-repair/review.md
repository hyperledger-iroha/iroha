# Bridge unit fixture repair

`index.json` authenticates the exact eight preimages taken at HEAD
`4032051e02fa9782cefe21e8f5b9761c9281cc34`, with Cargo.lock SHA-256
`ef670cd07bde7afed2110e967285bdc8e9719ed4a7420362a46a59a6784bcb43`.
The preceding workspace bridge harness reported 590 passes, nine failures and one
explicit maintenance ignore. Concurrent Core privacy ownership extraction is
preserved. This record describes fixture repairs, not release qualification.

| Failing case | Root cause and retained controls |
| --- | --- |
| Coordinator contract | The independent ABI pin remained 23 while the real owner advertises 25. All other exact contract words, fourteen method codes and unknown-code refusals remain. |
| Bootstrap freshness read delay | A half-second positive installation lease raced real signature/finality verification under load. The positive uses the existing full native lifetime; the negative still represents a half-second remaining UTC snapshot returned after one second of elapsed native time, and checks the exact expired-lease error and freshness callback. |
| Bootstrap issuance, expiry and sequence | Installation at the last valid UTC instant also consumes real verification time. Canonical signed-package authentication checks issuance minus one, inclusive issuance, expiry minus one and exclusive expiry. Invalid UTC values also fail the bridge, ample-lifetime installation succeeds, and all three native sequence-floor controls remain. |
| C startup contract and bounds | The former 1 MiB pin predates the current finality checkpoint allowance. The independent contract pin is 72,351,744 bytes (69 MiB) and also equals its model owner. Insufficient/null contract buffers, null/empty activation, a real allocated maximum-plus-one input and unprovisioned activation refusals remain. |
| JNI startup bounds | The same stale 1 MiB expectation is replaced by the exact current contract; minus one, zero, maximum plus one and i32 maximum remain rejected, with one and the exact maximum accepted before allocation. |
| Observation oversized checkpoint | A fabricated huge span starting at a short stack array overlapped output_len and correctly triggered its earlier no-write custody refusal. A real separate oversized allocation exercises the intended bound rejection and zero output length; unchanged output bytes and the separate aliased-length sentinel refusal remain. |
| Startup concurrent dispatch | Fixture authentication/signing happened inside the five-second publication staging window. Genuine context and signed archive preparation now precede it; actual archive verification remains inside activation. Both final success and final failure, staged owner/ledger checks, blocked dispatch, release and joins remain. No production clock or budget changes. |
| Sender release byte replay | Redemption terminal receipts predate mandatory finalized_core_hash and finalized_result. The maintained ignored Rust emitter regenerated all eighteen cases from independently validated source-derived inputs; nine send-split command bytes are unchanged. All full projections, unique identities, canonical roundtrips, credential/policy bindings, receipts, signatures and case inventories remain mandatory. Current-source replay remains required after rebuilding. |
| Signed transaction type-name alias | The mutation helper assumed compact lengths and ten payload fields instead of the fixed canonical V1 default flags and nine fields. It now uses the codec's explicit length primitives, retaining exact field/tail checks, a valid canonical bridge control and rejection of the solely mutated instruction type name as a codec error. |

The sender candidate is 561,989 bytes with SHA-256
`fcaa095a9bf0d2334f9834d37ddcfc1fc0f9dc8607af9b0f52067307d98a2b8f`.
Its first diagnostic native capture came from the existing compiled bridge
harness (SHA-256 `9686badeda9a66114e122f8396316aaf2a9fe9a95a64265f23d2c76ebe3169d8`),
whose emitter source is unchanged. The source input generator independently
validated all three contexts, six cases per context, raw-byte digests and public
projections before installation. A fresh source build and canonical replay must
qualify the repaired candidate; no missing-artifact or obsolete-graph result is
treated as a current pass.
