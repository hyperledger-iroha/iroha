# FASTPQ parallel-verifier execution

Work used `/Users/takemiyamakoto/dev/iroha` on `optimizations`. The focused
development build reused the existing `target/cargo-fast/dpn-devex-linux` cache
in the local Linux guest, preserving its LLVM 18 linker configuration and using
Cargo's native jobserver. No target was created or cleaned.

The new depth-19 regression covers clustered and sparse query sets of 31, 32,
33 and 375 leaves in three- and seven-worker pools. It compares every parent
coordinate, both children, parent output, root and work count with the serial
implementation. It constructs only the requested frontier, not a full tree.

## Executed checks

The accelerated wrapper ran `test --locked --offline -p fastpq_prover --lib
--test offline_compact --no-run --message-format=json`. Compilation passed in
173.02 seconds. The resulting unoptimized Linux test executables have SHA256:

- Library: `c2b6408d315f87f76c5ba51fc745d164760091115c724ce5fef00f0ea86dd5f7`.
- Public integration: `10d56d56b8cb16087d2f01300fee717ad637d707e9a63ccfa55a27342b5bf524`.

| Selection | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `backend::merkle_multiproof::` | 19 | 0 | 0 |
| `backend::compact_protocol::profile::` | 7 | 0 | 0 |
| `backend::compact_quantity_producer::` | 14 | 0 | 2 |
| `offline_compact` integration | 9 | 0 | 0 |

All 49 executed tests passed, including the new depth-19 regression. The two
explicit full-artifact tests are excluded from that count. Both executable
hashes remained unchanged through execution. Formatting and scoped diff checks
passed. The earlier retained executable separately passed four parallel Merkle
tests and one compact binding test; those five are not added to the new count.

All 162 captured FASTPQ source/configuration files remained unchanged through
compilation and focused execution. Four Halo2 files changed during compilation:
`plonk/prover/stored/proof_evaluations.rs`, its tests, and the
`poly/stored_advice/phase/retirement/blind.rs` module and its tests. HEAD also
advanced during this development run. This is executable-bound, scoped test
evidence, not an immutable dependency-closure or release qualification.

A follow-up incremental build passed in 227.73 seconds and produced exactly
the same two executable hashes. The 49-test result therefore applies to both
artifacts without repeating identical tests. That build still observed a change
to `vendor/halo2-axiom/src/poly/ipa/multiopen/verifier.rs`; it does not establish
an immutable dependency closure either. Its receipt is under `composed/` in the
evidence directory. No further rebuild was started for unrelated source churn.

Receipts, source hashes and logs are in
`target/fastpq-production-validation/20260922/`. The complete ordinary/AXT
producer test was then started separately with the retained tested executable;
its ordinary artifact completed generation and independent verification. The
8,004,591-byte artifact has SHA256
`09ed72c14fa7efc29c757eab1d1839529ed8faaaf29e5e8c21f7952667a9ae37`, matching
the ordinary artifact recorded on September 12. This uses the pre-fixed-row
frame and does not qualify the new codec. Proving including self-verification
took 2,547.62 seconds and independent verification took 21.12 seconds, with 750
AIR evaluations and two terminal checks. These uncontrolled development timings
are not a speedup or release-latency result. The process subsequently disappeared
without a terminal libtest or timing summary. Its AXT file was written immediately
before that observation. Both files match their filename hashes; the AXT artifact
is 8,021,972 bytes with SHA256
`f00f9a9ee7f3c361b855e5631fcddc9466fb00c8837f313edd0eb6bea9988579`.
A separate four-worker captured-artifact test passed ordinary positive verification
and earlier identity/context assertions, then failed its expected error variant
for reordered segments. The verifier rejected the malformed proof; the test
expected `TransferInvariant`, whereas the exact-index check constructs
`InvalidTraceShape`. The corrected test preserves the exact error detail and adds
diagnostics. AXT was not reached by that failed test. The combined producer result
and process peak remain unavailable; the correction needs fresh execution.

Production replay replacement, proof-size limits, witness privacy, authenticated
source admission, cryptographic review and release qualification remain open.

## Fixed-width row encoding

The maintained shared proof now stores each complete row as `[u64; 342]` and
encodes exactly 2,736 little-endian bytes. Canonical scalar checks and exact field
consumption remain mandatory. The enclosing row vector charges its complete
inline allocation. The new `FixedRowSharedProofV1` frame rejects previous shared
frames; no alternate decoder is retained. The transcript, commitments, geometry,
query count and production limits are unchanged.

The exact valid-shape bound falls from 4,279,877 to 4,017,376 bytes per segment,
a reduction of 262,501 bytes at 750 opened rows. The loose decode shape measures
6,451,024 bytes, 88,131 dynamic elements and 45,767,831 cumulative allocation
charges. This dummy shape is not a valid proof, and cumulative charges are not
peak RSS. Neither bound meets the production proof-size target.

The first compile exposed a denied trivial-cast lint; using `ptr::from_ref`
corrected it. The successful build took 200.73 seconds. Its retained library
executable SHA256 is
`ccdaf88ecd6e8f99cefe63061f3a3577bb36d89cc1f72f64d3501eac20bed95d`.
All five new row codec tests pass: exact bytes and unaligned decoding, ambient
layout independence, malformed spans and noncanonical scalars, exact allocation
limits, and preservation of authenticated roots and AIR evaluations.

The broader shared-opening run and boundary review exposed two stale tests: dynamic
elements still included the retired scalar vectors, and a byte-limit rejection
control still used the previous cap. Both tests now use the new exact boundaries.
The final test-only rebuild passed in 144.73 seconds with all captured input
hashes unchanged. It retained these executables:

- Library: `c9008a49e0b4446cd5f4d818cd3a933764f7059d6db47e050359c0bd40c2e436`.
- Integration: `2937478ac9193aebd5109cfdc1592900348434e32409cd3ba8716bea0b68e9a4`.

Both exact boundary reruns, five resource tests, fourteen producer tests and
nine public API tests pass. Two full-artifact tests remain excluded from that
30-test count. The original complete shared-opening process disappeared without
a terminal libtest summary after its host supervisor exited; its partial log
does not establish completion. A rerun uses the final retained executable and
writes stdout and exit status directly inside the guest. Its result remains
pending. Only the two test owners changed between the first successful
codec build and this rebuild. These runs reuse the existing Linux target with
a command-line `profile.test.package.fastpq_prover.opt-level=2` override;
debug assertions and overflow checks stay enabled. No workspace profile changed.

The maintained profile checker passes with 22 negative controls, and the codec
guard, scoped Rust formatting and diff checks pass. Receipts and logs are under
`target/fastpq-production-validation/20260922/row-encoding/`; the first compiler
failure and intermediate test failures remain recorded. Complete quantity
artifacts under the new frame and release measurements remain required.


## Borrowed hash-body fields

The current hash-framing owner borrows leaf payloads, child digests, predecessor
bytes and transcript tapes while producing the same canonical BodyV1 frame. This
removes temporary field vectors and digest copies. The canonical frame allocation
remains; the previous implementation already moved the raw transcript tape without
cloning it. The nominal schema, frame identity, field lengths and field order are
preserved. There is no alternate production codec.

Five new test groups compare borrowed fields with an independent owned encoder
across layout flags and length boundaries, check a fixed header/checksum vector,
and cover transcript tape ownership and rejection. Actual leaf and parent hash
calls are compared against independently framed one-shot hashes across all 21
oracle shapes, boundary coordinates and ordered unequal children. Those additions
and the 11 existing framing tests pass in the new executable. The optimized
focused build passed in 310.30 seconds with all captured source hashes unchanged.
Its retained library SHA256 is
`4f559e06f7f27b0c900f6f698dfe62e32c456850b5d02038a38fb0f12cdf4f75`;
the integration executable is
`dc87124b6e68292c8d511ef82f95f50e2db3ec9f01879bc07cbf638be9b42158`.
All 37 focused checks pass: 16 framing, seven typed hash-binding, five fixed-row
codec and nine public API tests. Both executable hashes remain unchanged. The
captured-input claim is scoped, not a complete immutable dependency closure.
Read-only review found no blocker. Scoped formatting, codec guard, diff checks
and the maintained profile checker with 25 negative controls pass.

Test-only phase timers now surround trace interpolation/extension, row, mixed
and quotient commitments, quotient evaluation and FRI folds. They record durations
and phase names without witness values; production library code is unchanged by
these timers. The complete quantity run now uses this tested executable alongside
the retained shared-opening suite, with four workers per process. The guest had
12 CPUs and 45,020,596 KiB available memory before dispatch. A shared kernel lock
prevents the earlier queued continuation from dispatching a duplicate producer.
The independent positive/negative captured-artifact test follows successful
production of both artifacts. Neither full run is counted as complete here.
Their shared-load development timings do not qualify release performance or a
speedup.
