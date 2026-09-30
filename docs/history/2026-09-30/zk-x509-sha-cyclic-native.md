# X509 cyclic-padding native qualification

This record continues the September 29 X509 padding investigation retained
under `dist/zk-remediation/2026-09-29/x509-sha-cyclic-padding`.
It distinguishes the abandoned historical preparation from subsequent validation
on the user's current `optimizations` checkout. No release activation or
independent cryptographic qualification is established.

## Candidate

The fixed historical candidate combines the SHA physical-padding recurrence
repair, signature allocation clearing, immutable validated P-256 arithmetic
owners, and bounded auxiliary replay. The recurrence gate is
`1 - segment_last - physical_padding`; initial products, terminal equalities,
and zero padding constraints remain. The resulting compiled profile pin is
`9d2d34512de90d13a0f68d352bbcc887ba9ac2f2a89e5deb845ff5c4c64d45ff`.
Native constructor and both component proof KATs subsequently passed on the current checkout. This candidate remains insufficient: the complete native word relation exposed another cyclic carry, described below.

The 20,882-file candidate manifest is
`160eada1c83279d532493ce9e54a4f9923a400bcfe854dab3b9d281e79148380`.
Ten source files amend the prior frozen candidate. The capture additionally
identifies the explicit `&mut [F]` closure return annotation already present in
main. During preparation, concurrent integration moved the implementation to
`crates/iroha_core_privacy`. The algorithm postimages match that implementation;
the moved engine and SHA error declarations additionally expose types/functions
and add a production-only derive. Those integration changes are outside this
frozen proof candidate.

Capture, exact source review receipts, normal Cargo command, source/binary
guards, and subsequent test output are retained under
`dist/zk-remediation/2026-09-30/x509-sha-cyclic-native`.
The earlier failed maximum-proof binary and receipts are preserved unchanged.

## Validation

The source geometry controls pass: `python3 -m pytest -q
scripts/tests/check_zk_x509_proof_geometry_test.py` reports nine passes on the
frozen candidate. The projected complete encoding remains 9,420,938 bytes
against 9,437,184. This is a geometry check, not an emitted proof.

The first ordinary optimized no-run build used:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_TARGET_DIR=/Users/takemiyamakoto/devstuff/iroha/target/zk-x509-rfc-sha-debug-20260929 \
GIT_WORK_TREE=/Users/takemiyamakoto/.codex/worktrees/zk-network-qualification/iroha \
cargo +1.93.1 test --locked --offline -p iroha_core --lib --release \
  --features iroha-core-tests,fastpq-gpu --no-run \
  --message-format=json-render-diagnostics
```

The user subsequently restricted all edits and validation to the current
`/Users/takemiyamakoto/devstuff/iroha` checkout. The already-running Cargo process
was left to finish, as required by repository policy. A non-executable sentinel
at the driver's output destination prevents subsequent retained-binary creation
or test enumeration. No native tests or proofs are queued against that historical
checkout. `opt3-native/follow-on-disabled.json` records this transition.
The old build subsequently ended naturally; its driver stopped at the sentinel
before any retained binary or test inventory was created. The corresponding
`follow-on-stop-confirmed.json` records the completed stop.

The replacement ordinary build runs directly on `optimizations`:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_TARGET_DIR=/Users/takemiyamakoto/devstuff/iroha/dist/zk-remediation/2026-09-30/x509-current-privacy-opt3/target \
cargo +1.93.1 test --locked --offline -p iroha_core_privacy --lib --release \
  --features fastpq_prover/fastpq-gpu --no-run \
  --message-format=json-render-diagnostics
```

Its initially empty target requires every local artifact to rebuild. The driver
retains a complete source census and checks the transitive local Cargo dependency
closure, global Cargo/toolchain configuration, vendor and fixture inputs before
accepting an executable. Unrelated documentation changes are separately recorded.
Receipts live under `dist/zk-remediation/2026-09-30/x509-current-privacy-opt3`.
The current-checkout geometry suite passes all ten controls, including the
extracted owner path. The initial current dependency closure covers 47 local
package roots and 7,001 inputs; its manifest is
`16673c467ab302a93083ce6264b27402a7d4753379a5be172a4f5b009e6c7a85`.

The parallel Core build found an unsigned fixture position inferred as `i32`
when its test-only witness field is absent. Its owner applied only the explicit
`_u32` suffix to `0x89ab_cdef` in the private-note fixture. The amendment was
captured before privacy compilation began, with a retained Cargo log prefix;
the amended file digest is
`06b684c95d40b6c714a4a5ebd41fc8241ffa622a92d6050e85b19ece3e77f50a`.
A normal same-target Cargo recheck verified the final inputs before retaining
the native binary (`272f93e0aded09201c3849175e241cabc3c785d7631fc70fa6c6445e6805ef0d`). Four existing private-note relation, membership, and nullifier
controls were added to the selection to exercise the high-bit position.

The native selection includes 37 ordinary controls and explicitly executes three
otherwise-ignored every-cell, bound-terminal, and seeded commitment/DEEP/opening
parity controls. Two independent component proof KATs must then be regenerated
and rerun before complete proof qualification.

## Complete native word-boundary finding

The current primary optimized build succeeded with a complete source/dependency
receipt. The four private-note controls passed, as did the current profile pin,
superseded-profile rejection, exhaustive outer SHA selector census, complete
SHA degree-six control, and both Fp4 polynomial controls. Actual projection and
I/O proof producers completed verification, canonical re-encoding, query and
size checks before revealing their old KAT literals. Updating just those two
literals and the Core-owned fixture gates produced native executable
`7590ae20c2d4b3eec7167a521fe6130659588260c9911176a1665fd86144cfd2`;
its two KATs, two profile checks and four private-note controls all passed.

The full native SHA padding test nevertheless failed. Unlike the earlier
isolated harness, it supplies the actual first call's word auxiliary state.
The actual maximum-credential source test independently pinpointed residues
`276, 285, 294, 303` at segment zero's cyclic last-padding-to-first-live edge.
Those are the four carried word-memory terminal products, whose recurrence
still used `1 - call_last`. On physical padding, that gate was one. The
maximum-source diagnostic took 15.031983 seconds and 1,291,534,336 bytes peak
RSS; its failure is retained under `x509-current-privacy-opt3` and is not
reported as a passing proof.

The additional repair uses
`local_compute + digest + memory - call_last` for word product, message-count,
padding-phase and active-block carries. The row-kind sum consists solely of
verifier-owned fixed columns. It is one on every logical call row, including
privately inactive capacity rows, and zero on physical padding. The gate is
linear, adds no columns or constraints, and preserves all first-row, terminal
and zero-padding checks. Expanded regression controls cover every native
fixed row and mutate all seven word carries on a live edge.

This changes the compiled relation descriptor again. Independent exact
29-field framing gives
`19aa35927ebc6e0ff800a0c80b6b315c2615350f35c82afdb9eae609d4c9ca9c`;
only field 17 changes. Both superseded profiles are rejected. The normal native
rebuild completed in 218.970540 seconds with no relevant input drift; the
retained executable is
`3ecdf115e8af4bfebf2e29008a67c2d6036bc51abaca10e38573c62731153b44`
and its dependency-input manifest is
`4b6a095dd87052cadef2d0c9f3bd9fd7aa0670f9b94fcde5a1e7e4667179288e`.

All 40 selected native controls pass, including all three explicitly executed
otherwise-ignored P-256 parity controls. Four additional direct capacity tests
pass for private lengths, exact-length enforcement, mutated padding selectors,
and algebraic opened selectors. The actual maximum SHA boundary case passes
in 14.727051 seconds with 1,272,037,376 bytes peak RSS. The maximum bound-source
handoff passes in 19.874102 seconds with 5,587,550,208 bytes peak RSS. The
seeded commitment/DEEP/opening parity case passes in 45.786670 seconds with
7,160,348,672 bytes peak RSS. These are scoped native tests, not complete-proof
performance measurements.

The newly bound actual component proofs derive projection KAT
`775b6fc6871d99d6211ba840c22df45beb0d7eaf5015c09bffc603906240fb31`
and I/O KAT
`bd4b3c6494b0d6d10cc22515e1014b6beb2c7bfa7cc0d5f9c14d218276234a2c`.
Their verification/re-encoding/query checks complete before the expected old
literal mismatches. Exactly those two public literals have been updated;
both literals passed in the rebuilt candidate described below.
Receipts are under
`dist/zk-remediation/2026-09-30/x509-word-cyclic-repair`.


## All-registration native preflight

A test-only boundary observer replays the actual maximum credential through all
49 MAIN registrations. It uses the existing bound providers and streams,
retains only boundary openings in clearing owners, and visits each selected row
exactly once. P-256 auxiliary sources stream once rather than reconstructing
one full source per column. The test checks first, logical and physical padding,
final, and cyclic edges, validates exact residue counts, and confirms mutated
live openings are rejected.

The normal current-checkout optimized build retained executable
`b71ba937e207347697f80bee14851094ba35f2887735592a5b14dd1030008385`.
Both updated component proof KATs and both profile checks pass. The explicitly
executed `maximum_credential_all_49_native_registration_boundaries_have_zero_residues`
test passes all 2,831 edges across exactly 49 registrations in 85.915036 seconds,
with 5,896,126,464 bytes peak RSS. Evidence is retained under
`dist/zk-remediation/2026-09-30/x509-current-complete-candidate`.
The final test-only guard amendment now clears an unexpected extra source row
before returning a topology error. Its normal optimized rebuild completed in
210.982541 seconds with no relevant input drift and retained executable
`a66a519824602248428e6e972393314e41734ca45ee54ab76624c7c301991da1`.
Both KATs and both profile controls pass again. The final all-registration
preflight passes all 2,831 edges in 87.722459 seconds, with 5,895,077,888 bytes
peak RSS. This final epoch is retained under
`dist/zk-remediation/2026-09-30/x509-current-proof-ready`.

The complete maximum proof began at 08:13:00 UTC on September 30 against that
retained executable and source. It remains running as of this record update.
Its unchanged 300-second limit has already been exceeded; the runner will
still verify and retain any completed public artifact and report size, memory,
time, canonical replay, wrong-context rejection and tamper rejection separately.
Neither native preflight nor an unfinished proof establishes complete-proof
success. The 9,437,184-byte and 12-GiB limits and independent qualification
remain unestablished for this candidate. Production activation stays unavailable.

Two short read-only stack samples at approximately 16 and 24 minutes attribute
153/153 and 162/163 sampled proving-thread stacks respectively to repeated
scalar-column ValueBus auxiliary source construction and replay, first during
mask sampling and then during commitment. These narrow samples are not a
whole-run time profile. An unapplied bounded eight-column ValueBus proposal
and a separate periodic-denominator proposal are retained under
`dist/zk-remediation/2026-09-30/x509-value-aux-batch-proposal` and
`dist/zk-remediation/2026-09-30/x509-periodic-denominator-proposal`.
They require coordinated application and native validation after the sealed
proof/build epoch; no speedup or acceptance claim is made from source review.
