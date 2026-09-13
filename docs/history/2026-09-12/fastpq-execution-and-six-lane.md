# FASTPQ execution accounting and six-lane proof checks

These results are scoped development evidence, not production qualification.
Work used `/Users/takemiyamakoto/dev/iroha` on `optimizations`. The checkout
base was `db3a2e17770443f3cb5ebf9e22b8c5cf66e1f623` with concurrent uncommitted
changes. No clean whole-workspace source seal or release build is claimed.

## Applied execution preparation and accounting

Independent transfer batches now propagate fallible transcript preparation out
of the whole body before the next movement. Storage starts empty and grows only
for accepted legs. An ignored preparation failure prevents occurrence staging;
the enclosing State transaction remains responsible for rolling back movements.

The complete-entry reservation adapter uses one canonical statement contribution
per entry, remeasuring the complete borrowed bundle across physical/fee fragments.
It preserves E=1, replaces T/D/I and M=S, and rolls back both new identity bindings
and usage through the existing journal. The adapter remains test-only: State
runtime quota policy, rejected/mandatory entry ownership and publication are not
activated. The shared test oracle now counts measured transcript multiplicity.

`cargo iroha-fast --target-slot retail-fees -- test -p iroha_core --lib --no-run
--message-format=json` passed in 810.1 seconds, including build-lock waiting.
This reused the existing warm development directory with Cargo's native
jobserver and Apple's linker. All ten captured FASTPQ source/document hashes
remained unchanged through compilation. The resulting Core test executable was
`iroha_core-88c76e069c109549`, SHA256
`535db7f3e3db17b532c4c360afc03b2c6998c13da56874645dd0f5389c58340a`.

| Executed test selection | Passed | Failed/ignored |
| --- | ---: | ---: |
| `state::prepared_transfer_transcript::tests::` | 13 | 0/0 |
| `fastpq::source_reservation::` | 30 | 0/0 |
| `fastpq::source_capture::budget::tests::` | 17 | 0/0 |

All 60 selected tests passed; 15 were added in this change. The selection includes
real balance rollback preserving prior committed transfers, fee-fragment identity,
stale handles/savepoints, complete-frame byte equality and exact capacity errors.
Formatting, `git diff --check` and `scripts/check_no_legacy_codec.sh` passed.
Unrelated existing Core warnings remain; strict workspace Clippy is unclaimed.
Local receipts: `target/fastpq-production-next/applied-source-20260912.json` and
`target/fastpq-production-next/current-core-20260912/{artifact,tests,result}.json`.

## Retained six-lane public producer execution

The retained executable SHA256
`9926ccd144d6afe870026893e8ca678ec6b116602a46b7639b5dd31ca696081e`
is bound by the retained preparation105 compile record to
`7d831c9543f5a89d65c076a4d2f728d4a944ecf3`. It uses the six-lane profile, not the
retired SHAKE experiment. Its FASTPQ source matched the inspected checkout before
the verifier changes below, but root Cargo files and SoraNet model dependencies
differ. These are therefore
results for that recorded executable, not a fresh current-checkout proof build.

Twenty-nine focused producer, decode, artifact and public-API tests passed.
One mistaken filter selected zero tests and is excluded from that count.
The complete public producer test subsequently passed for both two-segment
ordinary and AXT artifacts, with independent public verification after each
producer's self-verification.

| Route | Complete artifact bytes | Proving including self-verification | Independent verification |
| --- | ---: | ---: | ---: |
| Ordinary | 8,004,591 | 1,266.42 s | 29.83 s |
| AXT | 8,021,972 | 617.43 s | 29.43 s |

Ordinary artifact SHA256:
`09ed72c14fa7efc29c757eab1d1839529ed8faaaf29e5e8c21f7952667a9ae37`.
AXT artifact SHA256:
`f00f9a9ee7f3c361b855e5631fcddc9466fb00c8837f313edd0eb6bea9988579`.
Both verifications evaluated 750 AIR queries and two terminal degree checks.
The combined run took 1,944.51 seconds, with maximum RSS 2,414,379,008 bytes
and reported peak memory footprint 2,359,576,760 bytes. It used the unoptimized
CPU test profile, debug assertions and overflow checks. Concurrent machine work
and differing phases make these timings unsuitable for release comparisons.

Receipts and logs are under
`target/fastpq-production-validation/current-six-lane-20260912/`.
The producer wrote generated artifacts under its retained source's own ignored
`target/fastpq-production-validation/` directory.

The artifacts still exceed the 512 KiB compact target and 1 MiB AXT ceiling.
These successful offline proofs neither enable Core succinct admission nor
establish source finality, spend authority, witness privacy, cryptographic
qualification, hardware parity or four-validator production readiness.

## Parallel Merkle verification awaiting compilation

The compact verifier now dispatches independent parents within a Merkle level
through at most 32 indexed Rayon jobs. Small frontiers and single-worker pools
use the existing serial verifier. Leaf and sibling cardinalities are checked
before entering Rayon; canonical parent order selects the first returned error,
and no later level starts after a failed level. Reconstruction retains the same
two digest frontiers plus a fixed result array; this bound excludes Rayon runtime
and hash-callback allocations. Hash framing, roots, proof bytes and successful
work counters are unchanged by the algorithm.

Four new tests cover serial/parallel equivalence, malformed cardinalities,
deterministic error order and the live-job bound. The real compact binding test
also covers a wider tree. A separate ignored regression verifies the retained
ordinary and AXT artifacts against an independently constructed public fixture.
Formatting and patch-integrity checks pass. These additions are not yet compiled
or executed: the shared build resources remain reserved for release work.
No speedup or current-artifact verification result is claimed.
