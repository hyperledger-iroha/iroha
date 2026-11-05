# SNS recorded-lock drift recheck

The earlier 36 focused SNS tests passed against recorded Cargo.lock
`eee6edbd0c0c7ee334e617cbf3e4e6d20d0ba3d552a508495ef2fec9675b8384`.
The live lock is now
`de2c5b604926e792d35705a4d426bb0547d103779d96fb572103ca96c5bf6e88`.
The earlier successful receipt, independent review, logs and manifest remain
unchanged. All 11 captured SNS source hashes, 173 registry inputs and six
independent-review input hashes still match. The existing scoped verifier now
correctly fails on Cargo.lock. Nothing was repinned.

A read-only `--exact --list` invocation of the retained Torii harness discovers
`finality_interval::tests::maximum_payment::full_payment_blocks_use_actual_native_owner_for_every_receipt_and_last_http_slice`:
one test, zero benchmarks. The test was not executed. This lightweight check
records binary path/stat identity but does not recompute its large binary hash.
Discovery does not qualify that separate route or the new lock candidate.

The old results remain scoped component evidence for their recorded lock and
captured inputs, with no complete transitive Torii build custody, whole-node or
release qualification. This recheck ran no Rust build or test and changed no
production source, lock or previous evidence.
