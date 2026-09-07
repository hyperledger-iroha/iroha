# Source audit: degrees of the current 923 compact AIR numerators

Reviewed 2026-09-06. The reviewed source hashes and exact arithmetic certificate
are retained under `target/fastpq-production-validation/compact-air-degree-ledger-*`.
This is an algebraic source audit; it does not report a new Rust test run.
Those optional local evidence files are not build dependencies.

**Finding: the degree premise holds for every current ordinary and AXT slot.**
For arbitrary column polynomials of degree `<N`, with `N=65,536`, the actual
verifier evaluates fixed polynomial numerators of degree at most

```
hash slots: 3N-N/512-2 = 196478,
SMT slots:  2N-2       = 131070.
```

Both are below `3N=196608`. The same bounds hold for arbitrary malformed
polynomial witnesses; they do not depend on the witness satisfying the AIR.
No selector-product or coset-reduction counterexample was found. This settles
the degree premise for the reviewed source, not the separate assertion that
vanishing numerators imply the intended transfer/SMT/AXT semantics.

## Composition and polynomial model

The exact [combined evaluator](../crates/fastpq_prover/src/backend/compact_transfer_air.rs#L247)
decodes current and next rows, evaluates the hash ledger and SMT ledger, and
[concatenates](../crates/fastpq_prover/src/backend/compact_transfer_air.rs#L328)
597 local, 83 edge, and 243 SMT slots. The
[column decoder](../crates/fastpq_prover/src/gadgets/compact_trace_columns.rs#L100)
preserves complete canonical field values; it does not narrow LDE values to
bits or u32 limbs. Thus each current cell is `P_j(X)` and each next cell is
`P_j(omega*X)`, both degree at most `N-1`. Here `omega` is the trace root,
equal to the eighth power of the LDE root by
[validated geometry](../crates/fastpq_prover/src/backend/fixed_domain.rs#L19).
Replacing `X` by `omega*X` does not increase degree.

The [ordinary wrapper](../crates/fastpq_prover/src/backend/compact_public_transfer.rs#L179)
and [AXT wrapper](../crates/fastpq_prover/src/backend/compact_axt_air.rs#L74)
delegate their numerical evaluations and prover preparation to this same
relation. They change statement binding and identity, not the numerator list.
Their constructor checks and public context bytes are not additional polynomial
factors. The physical schema is exactly 342 columns and 65,536 rows, with no
free outer active-selector column in this relation.

## Fixed polynomials are evaluated as polynomials

Let `H=<omega>`, `B=512`, `M=N/B=128`, and `h_r=omega^(M*r)`. The
[periodic evaluator](../crates/fastpq_prover/src/backend/fixed_schedule.rs#L85)
computes

```
E_r(X) = (h_r/B)*(X^N-1)/(X^M-h_r).
```

Because `h_r^B=1`, the denominator divides the numerator in `F_p[X]`.
Every `E_r` has degree `N-M=65408`. The one-hot branch when `X^M=h_r`
evaluates the removable singularity exactly: that condition implies `X in H`,
and the polynomial's value is one for phase r and zero for the other phases.
The batch inversions off the subgroup evaluate the same polynomial. They do
not turn it into an arbitrary rational constraint. A sum of phase selectors
has degree at most 65408.

Each [sparse public column](../crates/fastpq_prover/src/backend/public_table.rs#L151)
is a sum of subgroup Lagrange polynomials

```
F_c(X) = sum_(r in support) value[r,c] *
         omega^r*(X^N-1)/(N*(X-omega^r)).
```

Its degree is at most `N-1=65535`, even though only 704 subgroup rows are
explicit. Sparse support does **not** imply degree at most 703. Omitted rows
are zero and the evaluator handles both included and omitted subgroup points
by exact values. Each apparent denominator divides `X^N-1`; no residual pole
or point-dependent degree branch exists.

The [49-column construction](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs#L89)
stores already combined port/phase/path/update masks as discrete values and
interpolates each whole column once. It never evaluates a path-mask polynomial
times a separate phase-selector polynomial. Public leaves are included as
fixed column values. Fixed sums such as `1-FINAL_ROW-UPDATE_RESET` remain
degree `<N`.

## Every hash-local and hash-edge slot

The [reference hash local equations](../crates/fastpq_prover/src/gadgets/compact_blake2b_air.rs#L277)
contain these operation families:

| Family | Trace-variable degree before the phase mask |
| --- | ---: |
| Bit/present/carry booleanity and disallowed carry pairs | 2 |
| Prefix presence and absent-byte bit products | 2 |
| XOR equations `z-a-b+2ab` | 2 |
| Packing bits, additions with constant radix and carry, initialization, register equality, byte length and digest marker | 1 |
| Constant active equation when `active=1` | 0 (zero) |
| Padding requirements for all 310 hash cells | 1 |

The [reference transition equations](../crates/fastpq_prover/src/gadgets/compact_blake2b_air.rs#L415)
are linear copies/updates, except the quadratic imported-presence prefix edge
`next.present[0]*(1-current.present[23])`. Bits are packed by additions and
constant scaling; `fixed_limb`, radix factors and marker constants do not
introduce further trace variables. All branches select a fixed native phase
while compiling the public relation, not an opcode supplied at an LDE query.

The [compiled ledger](../crates/fastpq_prover/src/backend/compact_hash_quotient.rs#L538)
calls every phase with `Expression::ONE` for active. It interns arithmetic,
tracks constant degree 0, input degree 1, max for sums, and sum for products,
and asserts its complete graph has maximum degree exactly 2. Constant
simplification cannot increase that degree. Hash phases 0–407 use the reference
equations; padding phases 408–511 use direct zero-cell equations. Export,
padding and cyclic-wrap hash transitions are omitted in the fixed graph.

[Each output term](../crates/fastpq_prover/src/backend/compact_hash_quotient.rs#L683)
is exactly one degree-at-most-two expression times **one** phase-mask sum.
Grouping equal expressions and using prefix sums changes neither this form
nor its degree. There is no multiplication of two masks, and no witness active
gate added after compilation. Therefore every local slot 0–596 and every
edge slot 597–679 has degree at most

```
2*(N-1)+(N-N/512) = 3N-N/512-2.
```

Stable slot numbers reuse different families in different phases. The bound
applies term by term before summing those phases, so it covers all 680 slots
without assuming a slot has one unchanging informal meaning. Linear terms
have the sharper bound `2N-N/512-1=130943`.

## Every SMT slot

The actual [SMT numerator function](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs#L293)
contains only sums of one fixed polynomial times one linear trace expression,
and standalone fixed polynomials. Its complete global numbering is:

| Global slots (zero-based) | SMT-local slots | Meaning | Upper degree |
| --- | --- | --- | ---: |
| 680 | 0 | Executing byte length 83 | `2N-129` |
| 681–832 | 1–152 | 19 fixed node-domain bytes, eight bits each | `2N-129` |
| 833–834 | 153–154 | Two marked digest input bits | `2N-129` |
| 835–850 | 155–170 | Two complete eight-limb input ports | `2N-2` |
| 851–866 | 171–186 | Old/new public leaves at each update start | `2N-2` |
| 867–874 | 187–194 | Initial public root | `2N-2` |
| 875–882 | 195–202 | Old path root equals starting root | `2N-2` |
| 883–890 | 203–210 | Final export/carried public root | `2N-2` |
| 891–898 | 211–218 | Starting-root carry and update reset | `2N-2` |
| 899–906 | 219–226 | Old-child carry and old export advance | `2N-2` |
| 907–914 | 227–234 | New-child carry and new export advance | `2N-2` |
| 915–922 | 235–242 | Sibling carry outside its declared free edge | `2N-2` |

The first three groups use only period-512 selectors of degree 65408. The
remaining groups may use sparse polynomials of degree 65535, giving
`(N-1)+(N-1)=2N-2`. In the port group,
[input_limb](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs#L281)
packs 32 bit cells with constant powers of two; a limb crossing an import-row
boundary uses current and next cells linearly. The selector `phases[phase]`
multiplies that linear pack once; the old/new/sibling alternatives use their
already combined sparse masks separately. These are sums, not products of
the phase and sparse masks.

The `edge`, `ordinary` and `1-SIBLING_FREE` coefficients are linear combinations
of fixed polynomials. Export corrections similarly add one fixed-times-linear
term; they do not multiply it by `ordinary` again. Public root/leaf constants
have degree zero in X. This accounts for all 243 slots with no unused gap.

The [physical SMT reference](../crates/fastpq_prover/src/gadgets/compact_smt_air.rs#L596)
supplies semantic equations at execution/padding and noncyclic boundaries;
the polynomial verifier uses the ledger above, not a sampled native row label.
Its generic degree-two assertion includes hash equations. The SMT-only suffix
is trace-linear and is differentially mapped into the fixed 243-slot layout.

## Coset/preparation paths do not truncate the products

[Prover preparation](../crates/fastpq_prover/src/backend/compact_transfer_air.rs#L188)
IFFTs only the 49 fixed degree-`<N` columns and evaluates their LDEs. It then
multiplies those evaluations by current/next trace evaluations when computing
the numerators. It does not interpolate a full numerator back into a
degree-`<N` representative. The hash mask cache repeats every 4096 LDE
indices because the phase polynomial depends on `X^128` and the LDE root
has order 524288: `(g^4096)^128=1`. The prepared lookup additionally checks
that its point equals the exact indexed coset point.

The verifier evaluates fixed polynomials directly and never relies on that
prover cache. [Alpha mixing](../crates/fastpq_prover/src/backend/compact_protocol.rs#L666)
is linear over `K`, so it cannot increase numerator degree. The compact path
uses the [all-row denominator](../crates/fastpq_prover/src/backend/air_quotient.rs#L101)
`1/(X^N-1)` for the already masked full numerator, including edge slots.
The generic alternate transition weight is not used for these compact slots.
Divisibility is a satisfaction condition; it is not assumed merely because
the prover divides pointwise on D.

Thus every residual `sum alpha_k*C_k - (X^N-1)*q` with `deg(q)<2N` has
degree at most `3N-1`, exactly the conservative premise used in the AIR
reduction. A satisfied numerator has quotient degree below `2N`; an
unsatisfied numerator's rational samples need not have that low degree.

## Tight examples and existing regression coverage

The bounds are not based on reducing high powers modulo `X^N-1`:

- Set the hash bit column `bits[0][0]=X^(N-1)` and all other columns to zero.
  Local slot 1 is the executing mask times `X^(2N-2)-X^(N-1)`. The executing
  mask's leading coefficient is `(1/512)*sum_(r=0..407) h_r`, which is
  nonzero because a 512th primitive root raised to 408 is not one. This
  numerator has degree exactly **196478**. It refutes an incorrect claim
  that interpolation of row residues could cap every numerator below N.
- In SMT source-root slot 867, choose `starting_root[0]=X^(N-1)`. The
  `FIRST_ROW` polynomial has leading coefficient `1/N`; multiplying produces
  degree exactly **131070**. Subtracting the public-root constant times that
  mask cannot cancel the leading term. Sparse support does not lower this
  worst-case degree.

These are arbitrary polynomial inputs for degree analysis, not valid transfer
witnesses. The retained arithmetic script verifies the actual field/root
constants, both nonzero leading coefficients, the degree calculations and
coverage of slots 0–922. It does not execute or parse the Rust expression DAG.

Existing source tests reviewed include:

- `compilation_is_deterministic_bounded_and_reports_selector_aware_degree`,
  `every_physical_phase_matches_reference_slot_order_and_zero_filling`, and
  `composed_column_degree_is_not_truncated_to_subgroup_interpolation` in the
  hash quotient module. The last explicitly constructs the high-degree
  numerator and its quotient/remainder at N=512.
- `complete_fixed_and_trace_degree_and_resource_bounds_are_explicit`, using
  a generic max/add degree semiring over every SMT slot; subgroup/independent
  IFFT/Horner checks; and physical-boundary differential checks in the SMT
  quotient module.
- `all_coset_selectors_match_independent_ifft_horner_and_degree`, sparse-table
  independent interpolation tests, combined slot-order/schema tests, and the
  AXT test `evaluations_delegate_every_fixed_slot_and_row_validation`.
- The explicitly ignored fixed-cache diagnostic compares prepared and direct
  evaluation at cache/domain boundaries. Its presence is not a fresh test run.

This audit relies on the inspected algebra and exact source snapshot. Existing
tests substantiate the invariants they exercise; their mere presence is not
reported as execution evidence. Independent review of this ledger and a
separate semantic soundness audit remain appropriate before adopting the
complete interactive reduction or any production profile.
