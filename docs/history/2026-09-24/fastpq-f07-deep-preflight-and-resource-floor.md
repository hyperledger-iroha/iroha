# FASTPQ F07 DEEP preflight and resource floor — 2026-09-24

The inactive private DEEP candidate now has a typed `DeepProverPlan` preflight.
It accepts only the checked `SourceTraceColumns` projection and the complete
`CompactTransferAir`, verifies 301 exact retained mask shapes and all Fp4 mask
coordinates, and runs the full 923-slot `numerator_degree_bounds` calculation
before any interpolation or LDE. It does not construct a proof or open a
production route.

For the current transform `C'(X) = C(X) + (X^N - 1) M(X)`, the highest nonzero
mask term at index `i` makes the trace polynomial's exact exclusive degree
bound `N + i + 1`. The plan records the source and retained column indices and
refuses every such mask against `FRI_DEGREES[0] = N = 65,536`. It also refuses
all-zero masks because the existing 301-base-value row DTO would disclose
private witness cells. It cannot authorize a private proof.

The preflight computes a checked additional payload floor for a directly
materialized, complete base-field row LDE:
`301 * 8,388,608 * 8 = 20,199,768,064` bytes. This excludes the borrowed
source, quotient, FRI, Merkle and allocator overhead; it is a floor for this
in-memory strategy, not a lower bound on every possible external-memory
construction. An explicit smaller workspace cap is refused before a mask is
inspected. The plan also counts `616,448` raw bytes to widen the same 64
complete row openings to Fp4, exceeding the 512 KiB proof target, and
`1,012,702` bytes for two maximum-size child frames, leaving only `35,874`
bytes under the 1 MiB AXT inner-payload ceiling before carrier and public
context. These calculations do not establish a compact private proof or an
AXT bundle fit.

The remaining blockers are a reviewed hiding/opening construction with fresh
entropy, authenticated column and quotient degrees, a complete 923-slot
zero-remainder quotient, bounded external-memory LDE/FRI and Merkle ownership,
source-derived transcript order, and full source-derived one-delta verification.
The production registry, default, Core admission path and compact verifier
remain unchanged. F07 does not qualify private FASTPQ production.

Validation: `rustfmt --edition 2024` completed for the new Rust owner and
tests. `cargo test -p fastpq_prover --lib deep_prover_plan -- --nocapture`
passed all six focused tests after the concurrent SoraFS topology DataModel
derive errors were repaired. Tests cover zero and nonzero masks around omitted
column ranges, a high Fp4 mask term, canonicality and degree padding, the
insufficient workspace cap, exact source-order sentinels, and checked byte and
degree arithmetic. No complete prover or private proof roundtrip was tested.
