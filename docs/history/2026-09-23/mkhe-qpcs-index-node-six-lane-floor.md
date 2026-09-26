# MKHE qPCS indexed-leaf and node work floor, 2026-09-23

The current `optimizations` qPCS tree remains fail-closed under the fixed
128,000,000,000 tracked-work ceiling. This check is a source-derived lower
bound for *the present six-lane leaf/node construction*, not a replacement
commitment protocol, full-source measurement, or production qualification.

The initial oracle has 524,288 leaves and 524,287 binary internal nodes. The
canonical six-lane frame charges 487,008 Goldilocks adds/multiplies for each
index-bound leaf hash and 476,862 for each internal-node hash. Thus the current
index-bound layer alone costs
`524,288 × 487,008 = 255,332,450,304` operations, more than twice the
whole-proof work cap. Index-bound leaves plus nodes cost
`255,332,450,304 + 524,287 × 476,862 = 505,344,997,698`, nearly four
times the cap **before hashing any 6,000-byte codeword payload**. The actual
constructor hashes all payloads and charges 3,032,072,370,498 operations for
this tree. The one-payload diagnostic is 505,349,817,048 operations; neither
diagnostic value is an admission discount for distinct source-derived leaves.

`RnsNativeTreeWorkV1::index_node_floor_field_operations` now derives the
structural floor from the same verifier-owned oracle geometry and exact shared
hash-frame work as the constructor, using checked arithmetic. Its focused
test pins the initial/index/node decomposition and verifies a four-leaf FRI
terminal tree stays below this particular floor screen. The constructor still
charges **all** payload, index, and node operations before allocating buffers
or accessing the source. No budget, six-lane owner, leaf wire, root identity,
or verifier acceptance path changed.

The work calculation uses one representative index and node frame per oracle:
their positions and levels occupy fixed-width frame fields, so changing a
valid index or height changes values but not the six-lane word count.

This rules out a payload-only compression or caching fix under the current
index-bound frame; even making payload hashes free and reducing binary-node
work to zero leaves the index-only cost above the gate. A viable redesign must
change the cryptographic commitment/evaluation argument and its exact
operation geometry while preserving full 40-limb opening linkage, shared
six-lane transcript contract, 128-bit soundness, 512 MiB resident, 16 GiB
spool, 64 GiB authenticated I/O, 128-billion-work, and fixed wire ceilings.
This lower bound is specific to the present tree construction; it does not
prove that every redesigned qPCS exceeds those limits or close any release
gate.
The [earlier geometry review](mkhe-native40-qpcs-geometry-review.md) lists
unresolved source, challenge-chronology, memory, and audit obligations.

Validation: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p
iroha_zkp_halo2 actual_frames_derive_exact_full_and_repeated_payload_initial_bounds
-- --nocapture` passed (1 test; 1,722 other unit tests filtered out).
