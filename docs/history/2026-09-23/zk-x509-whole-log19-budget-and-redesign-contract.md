# zk-X509 whole-log19 budget and redesign contract, 2026-09-23

This is a source-derived sizing screen for the sole first-release X5S1 profile
on `optimizations`, not a generated maximum proof, a soundness certificate, or
production activation. The complete DER/RFC 5280, signed CRL, five P-256
signature, 29 SHA-call, projection, byte-memory, compact-CA, and holder-ownership
relation remains mandatory. The prior
[proof-geometry audit](zk-x509-proof-geometry-audit.md) records its current
registration and the canonical 19,156,074-byte maximum.

## A second necessary budget bound

The shared proof has 136 sampled positions. Each direct MAIN trace column
contributes one eight-byte base-field value at the current and next row per
position: `136 × 2 × 8 = 2,176` bytes per column. The 5,623 present columns
therefore contribute 12,235,648 bytes. Subtracting them from the canonical
19,156,074-byte maximum leaves 6,920,426 bytes of current other costs:
X5S1/X5M1/X5C1 frames, the complete compact-CA proof, MAIN roots, FRI,
frontiers, DEEP openings, and the remaining MAIN wire. The 9,437,184-byte
ceiling then leaves `9,437,184 - 6,920,426 = 2,516,758` bytes for any trace
openings **if those other costs stay unchanged**. At the same sampling schedule
this permits at most `floor(2,516,758 / 2,176) = 1,156` columns, with 1,302
bytes spare. At least 4,467 of the present direct-opening columns must be
replaced by a sound different argument, or an equivalent reduction must also
come from other proof costs.

The log-19 group is 1,940 base plus 1,772 auxiliary columns. It contributes
`3,712 × 2,176 = 8,077,312` sampled bytes. The five P-256 signatures own
2,395 of those columns and 5,211,520 bytes; removing their direct openings
alone would still leave 13,944,554 bytes. Even the deliberately impossible
thought experiment of deleting **every** log-19 opening leaves 1,911 other
columns and `19,156,074 - 8,077,312 = 11,078,762` bytes overall: 1,641,578
over the full cap. With other costs unchanged, at least 755 more direct-opening
columns beyond the whole log-19 group would still need replacement.

These are necessary screening inequalities for the present q=136/current-next/
eight-byte opening shape, not a lower bound on every possible STARK or recursive
construction. A redesigned AIR may change the native domain, degree, FRI,
DEEP, multiproof, masking, and CA cost; it must recalculate **all** terms.
`python3 scripts/check_zk_x509_proof_geometry.py` reads the Rust profile and
checks these figures without relaxing the 9 MiB ceiling.

## Construction work required before code admission

A candidate must specify one canonical witness-independent verifier profile
and prove the same complete relation. A serial or microcoded AIR could reuse
columns across signature and SHA instances, but would need explicit role and
instance tags, range/transition constraints, start/end and padding rules, and
an authenticated memory or permutation argument that joins every emitted
value to the existing DER/RFC, CRL, projection, and holder checks. Moving five
signature computations to consecutive row ranges does not by itself prove
their input/output equality or lower the whole wire bound; the 1,911
non-log-19 columns alone exceed the unchanged opening budget.

A recursive construction could replace wide child openings with an outer
proof, but the outer relation must verify *every* child verifier key and
profile digest, complete child acceptance, public-statement binding, all
cross-trace product terminals, the compact-CA root/SPKI link, and exact X5S1
claims. It must enforce the existing Fiat--Shamir chronology: all six MAIN
base roots and the compact-CA base root are fixed before deriving the 272
shared X5B1 challenges; all auxiliary roots and terminal claims precede
constraint alphas. Independent child proofs that select their own challenge
families do not establish the present joined relation.

For either route, admission requires: (1) an executable, verifier-derived
layout and exact worst-case whole-X5S1 encoded bound no greater than
9,437,184; (2) a compositional soundness and zero-knowledge argument covering
the shared 136-query theorem and every new lookup/recursion error event at the
128-bit target; (3) full maximum-shape certificates/CRL/path/disclosures and
adversarial mutations through the production prover and verifier; (4) measured
proving under 300 seconds and 12 GiB peak resident memory on the release
benchmark, plus verification/resource evidence; and (5) regenerated compiled
profile, fixtures, and independent review. The current source has no such
construction or evidence. Its preflight and activation checks must continue
to reject production X509 proofs.

The 136-query shared security profile cannot be lowered to improve the byte
count, and masked field openings cannot be assumed compressible as small
integers. Certificate/CRL coverage, the 9 MiB cap, and the current complete
relation are fixed first-release requirements.
