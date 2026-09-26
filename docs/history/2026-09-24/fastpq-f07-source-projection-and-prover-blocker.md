# FASTPQ F07 source projection and DEEP producer blocker — 2026-09-24

This is a bounded implementation record for the `optimizations` checkout. It
does not contain a produced DEEP proof, an accepted proof, a witness-privacy
argument, or production qualification. The 512 KiB proof target and 1 MiB AXT
inner-payload ceiling remain unchanged.

The private `SourceTraceColumns` bridge can borrow the existing quantity
producer's 342 complete physical columns; it is not wired into a producer.
Before a DEEP transform could consume those columns, it checks every column's
65,536-row extent, every base-field coordinate,
and all 41 verifier-known public cells at every physical row. Only then does it
expose the 301 retained columns in the exact committed order or read a retained
row without allocating another matrix. Its test covers the exact borrowed
mapping, first/last rows, width and extent refusal, a noncanonical final cell,
and a changed late public cell. A later audit found that the original mapping
test aliased every retained column to one zero slice, so its pointer and row
assertions could not detect a retained-column permutation. The test now places
distinct nonzero first/last-row sentinels on both sides of each omitted-column
range and at the final column; its result awaits the coordinated Cargo selector.
This validates source projection, not the 923 AIR constraints or source
authority.

A sound same-profile producer cannot be made from the currently wired DEEP
owners by simply connecting their functions. The candidate fixes trace degree
`< N` with `N = 65,536` and FRI degree progression
`[65,536, 4,096, 256, 32, 4, 1]`. Its `DeepPolynomialSource` accepts only
degree-`< N` retained trace polynomials and a degree-`< 2N` quotient. The
existing private masking owner preserves all N physical evaluations with
`C'(X) = C(X) + (X^N - 1) M(X)`. For any nonzero `M`, `C'` has degree at least N;
the leading term cannot cancel against `C`, whose degree is below N. Thus a
nonzero mask from this construction cannot satisfy the candidate's present
trace/FRI degree claim. More generally, every nonzero additive polynomial
which leaves all N subgroup values unchanged is divisible by `X^N - 1`, hence
also has degree at least N. An unmasked proof would expose private trace values
at the query positions and cannot qualify as the requested private proof. A
different hiding or opening construction is required; none is specified or
implemented at this source boundary.

The current DTO also opens each retained row as 301 base-field `u64` values,
while `PreparedMaskedTrace` produces Fp4-valued masked coefficients. Widening
the same 64 complete row openings to Fp4 would cost
`64 * 301 * 32 = 616,448` raw bytes, **92,160 bytes above** the entire 512 KiB
proof target before quotient, FRI, authentication or framing. The hiding design
must therefore change the opening geometry too; a field-type substitution in
the existing DTO cannot work.

The concrete prover resource owner is also missing. A direct complete
301-column base-field LDE at blowup 128 contains
`301 * 8,388,608 * 8 = 20,199,768,064` bytes before quotient arrays, Merkle
nodes, FRI, witness or allocator overhead. The test-only verifier's 506,351-byte
DTO bound is a wire calculation; it does not reserve or measure this private
workspace. The existing `compact_prover_resources` charge models the old
eightfold/375-query geometry, not the DEEP candidate. Current Core still
requires witness replay.

The next testable interface is a fixed, reviewable `DeepProverPlan` derived from
the complete source relation and `SourceTraceColumns`, not proof-supplied
geometry. A bounded next source cut can preflight the currently proposed mask
shape against `deep_geometry::FRI_DEGREES`, the complete
`CompactTransferAir::numerator_degree_bounds`, and fixed proof/AXT ceilings,
returning a typed refusal before interpolation or LDE allocation. Focused tests
should refuse every nonzero subgroup-preserving additive mask and insufficient
prover workspace, while preserving the checked source-order sentinels. This
refusal is not a substitute for the required reviewed hiding/opening
construction and fresh entropy source. Once such a construction exists, the
plan must compute its trace/AIR numerator/quotient/DEEP/FRI degree and resource
bounds before any allocation. A producer must then form the full 923-slot
quotient with checked zero remainder, commit retained rows and quotient halves
before drawing OOD challenges, produce all FRI layers and exact minimal
frontiers under a bounded external-memory owner, and pass a full
source-derived one-delta `deep_engine::verify` roundtrip. Negative tests must
change the source row, mask, OOD answer, quotient half, public statement,
frontier, and terminal value independently. AXT additionally needs a
bundle-wide measurement: two maximum-size 506,351-byte children total
1,012,702 bytes and leave 35,874 bytes below the 1 MiB inner ceiling before
carrier and public-context bytes. That arithmetic does not establish a fit or
an impossibility for two children; three already exceed the ceiling before
their carrier. The exact encoded AXT bundle and inner payload must be checked.

No production registry, default, Core admission path, or compact verifier is
changed by this slice. F07 remains open.

`rustfmt --edition 2024 --check` passed for the two modified Rust files.
The focused
`compact_public_columns::tests::complete_source_columns_are_borrowed_in_exact_retained_order`
test passed 1/1 with the distinct source sentinels. This verifies only the
borrowed projection and source order, not compact proving, masking or a
production-sized proof.
