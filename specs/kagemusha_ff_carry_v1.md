# KAGEMUSHA FF-CRT carry bounds V1

Chip: `iroha_plonk_gadgets::ff` (`crates/iroha_plonk_gadgets/src/ff/mod.rs`).
Moduli: Pasta `q` in an `Fp` circuit (Fq-in-Fp), Pasta `p` in an `Fq` circuit
(Fp-in-Fq), P-256 `p` and P-256 `n` (in either native field).
Executable form: `ff::tests::ff_carry_memo_bounds_hold_for_every_modulus`
(exact `num-bigint` interval arithmetic, and Lemma 1 exhaustively).
Adversarial ("tamper") tests of the memo's premises:
`ff_carry_bound_overflow_is_unsatisfiable`, `ff_noncanonical_limbs_rejected`
(including top sublimbs that pass the plain membership and fail only the
scaled one, rejected on the block's operand row),
`ff_adversarial_blocks_rejected_on_the_shared_table` (the same attacks on the
Q leaf's shared table) and `ff_range_patterns_follow_the_block_layout`.
The boundary tests `ff_final_usable_row_binds_fused_and_comparison_blocks` and
`ff_carry_endpoints_are_range_checked_through_the_final_row` exercise the final
usable row, the first forbidden blinding row, and both signed carry endpoints
with immediately invalid neighbors, on private and shared tables in both fields.

**What changed in M3b (D1).** Only the range-check mechanism and the block
rows: the `(tag, value)` table with 15/12/7-bit sub-tables became one 15-bit
column `V`, a top sublimb is checked by two memberships, and the operand row
moved from block row 6 to row 0. Limb geometry (87/87/82), quotient
capacity (`2^261`), carry offset and width (`2^104`, seven 15-bit
sublimbs), padding and every numeric bound below are unchanged.

## 1. Parameters

| Symbol | Value | Enforced by |
|---|---|---|
| `B` | `2^87` | limb radix |
| proper limbs `c_0, c_1, c_2` | `< 2^87, 2^87, 2^82` (so `c < 2^256`) | running sums, 6 sublimbs, top 12/12/7 bits (two memberships) |
| quotient limbs `q_0, q_1, q_2` | `< 2^87` each (so `q < 2^261`) | running sums, top 12 bits (two memberships) |
| carries `u_0..u_3` | `u_k + 2^104 in [0, 2^105)` | running sums, seven 15-bit sublimbs (top: one membership) |
| operand limbs | nonnegative integers `<= 2^94 - 1` (tracked bounds) | chip bound tracking (Section 6) |
| `N` | native order, `p` or `q` of Pasta; `2^254 < N < 2^254 + 2^126` | field |
| `m` | odd, `2^252 <= m < 2^256` | `ForeignModulus::new` |

The range table is one fixed column `V` with `V = v` on row `v` for every
`v < 2^15` (32,768 rows, `k >= 16`). In the Q leaf it is `V` of the shared
table, every entry of which on a usable row is below `2^15` (Section 11).

## 2. Lemma 1 (running sums, two-membership tops)

Block rows (7 per block, rows `0..6` from the block start):

| group | row 0 | rows 1..5 | row 6 |
|---|---|---|---|
| `C`, `Q` | operand (copy or constant) | `z_0..z_4` | `z_5` (top) |
| `U` | `z_0` | `z_1..z_5` | `z_6` (top) |

Lookups (every input into `V`; the pattern columns are fixed, so the set of
checks is the same for every witness):

- `C`/`Q` column with top width `w` and ternary pattern `h` (1 on rows 1..5,
  2 on row 6, 0 elsewhere): input `s (z - 2^15 z_next) + t z + e 2^(15-w)
  z_(+6)` with `s = h (2 - h)`, `t = h (h - 1) / 2`, `e = t(h at +6)`.
  For `h in {0, 1, 2}`, `(s, t) in {(0,0), (1,0), (0,1)}`, and `e = 1`
  exactly on a row whose row `+6` has `h = 2`, i.e. a block's row 0, whose own
  `h` is 0 (blocks are 7 contiguous rows; `ff_range_patterns_follow_the_block_layout`
  checks `h(r) = 0` wherever `h(r + 6) = 2`). So rows 1..5 check the steps
  `z_j - 2^15 z_{j+1}`, row 6 checks `z_5`, row 0 checks `2^(15-w) z_5`, and
  every other row checks 0.
- `U` column with binary patterns `s` (rows 0..5) and `t` (row 6), never
  both 1: input `(s + t) z - 2^15 s z_next`.

**Lemma 1.** A running sum `z_0..z_{L-1}` with step lookups
`z_j - 2^15 z_{j+1} in V` for `j < L-1` and top lookups `z_{L-1} in V` and,
for a top width `w < 15`, `2^(15-w) z_{L-1} in V`, forces `z_0 = sum_{j<L-1}
s_j 2^{15j} + 2^{15(L-1)} z_{L-1}` with every `s_j < 2^15` and `z_{L-1} <
2^w` integers.

*Proof.* `V` holds exactly the integers `[0, 2^15)`. The plain top
membership makes `z_{L-1}` an integer `v < 2^15`. Then `2^(15-w) v` is an
integer below `2^(15-w) 2^15 <= 2^23 < 2^30 < N`, so its field value is that
integer, and its membership gives `2^(15-w) v < 2^15`, i.e. `v < 2^w`
(conversely every `v < 2^w` passes both). Backward induction as before: if
`z_{j+1}` is a small integer and `z_j - 2^15 z_{j+1} = s_j` is a table
value, `z_j = s_j + 2^15 z_{j+1}` is a small integer; every quantity stays
below `2^105 < N`. So `z_0` is an integer in `[0, 2^{15(L-1)+w})`: `2^87`
(top 12), `2^82` (top 7), `2^105` (seven 15-bit sublimbs). The test checks
the equivalence exhaustively for `w in {7, 12}` and the widths
`15 * 5 + {12, 12, 7} = 87, 87, 82`, `15 * 5 + 12 = 87`, `15 * 7 = 105`.

Degrees: `C`/`Q` inputs have degree 3 (lookup degree `2 + 3 + 1 = 6`), `U`
inputs degree 2 (5). Advice queries per operand column: rotations 0, 1, 6
(three, as before: blinding stays 5; a `Rotation::prev` design would have
needed a fourth query and six blinding rows).

## 3. Lemma 2 (no column equation wraps)

Every gate is enabled on a block's row 0 and reads operands at rotation 0,
the `C`/`Q` running-sum roots `z_0` at rotation 1 and the `U` roots at
rotation 0, so the values below are the Lemma 1 integers of that block.
The multiplication gate (`P = a`, `R = b`, `S = c`, `K = 0`) and the division
gate (`P = b`, `R = c`, `S = a`, `K = k m` with limbs `K_i`) enforce, for `k = 0..3`
(with `u_{-1} = 0`, `S_3 = K_3 = 0`):

`E_k:  t_k + u_{k-1} - u_k B = 0 (mod N)`, `t_k = sum_{i+j=k} (P_i R_j - q_i m_j) - S_k + K_k`.

For every assignment satisfying the range checks (not only honest ones),
`|t_k + u_{k-1} - u_k B| <= spread_k + 2^104 (B + 1)`, where `spread_k` is the sum
of the maximal positive and negative parts of `t_k`. With operand limbs `<= 2^94`
the worst `spread_k + 2^104 (B+1)` is `2^191.46` for every modulus (table), far
below `N > 2^254`. So each `E_k` holds over the integers.

## 4. Lemma 3 (CRT)

Summing `E_0 + B E_1 + B^2 E_2 + B^3 E_3` over the integers gives
`sum_{k<=3} t_k B^k = u_3 B^4`, so `X := P R - S + K - q m = (u_3 + t_4) B^4`
with `t_4 = P_2 R_2 - q_2 m_2`: `X = 0 (mod 2^348)`.

The native-residue constraint is `X = 0 (mod N)` (all limbs recomposed with
`B^i mod N`). Since `N` is odd, `X = 0 (mod N 2^348)`.

For every range-checked assignment, with the operand envelope value
`A = (2^94 - 1)(1 + 2^87 + 2^174) < 2^268 (1 + 2^-86)` and the padding
`K < 2^269`: `X <= max(A^2, A (2^256 - 1) + K) < 2^537` and
`-X <= max(2^256 + (2^261 - 1) m, A + (2^261 - 1) m) < 2^518`, so
`|X| < 2^537 < 2^602 < N 2^348`. Hence `X = 0`:

- multiplication: `a b = c + q m`, `c` proper, so `c = a b (mod m)`;
- division: `b c = a + q m - K = a (mod m)`, so `c = a / b (mod m)` when `b` is
  invertible, and no assignment exists when `b = 0 (mod m)` and `a != 0 (mod m)`.

The 65-bit margin (`2^602` against `2^537`) makes the argument independent of the modulus beyond
`m < 2^256` and `N > 2^254`; it does not need `m` prime or `m` vs `N` ordering
(the 3-carry variant, `T = 261`, would need `q_max m + c_max < N 2^261`, which
for Fq-in-Fp (`m > N`) forces `q < 2^260`; four carries remove that case split).

## 5. Per-modulus table (both native fields; exact values from the script)

| modulus | m limbs (bits) | div `K = k m` | mul carries `u_0..u_3`, envelope operands | div carries | worst wrap bound | `m 2^261` |
|---|---|---|---|---|---|---|
| Pasta q (Fq-in-Fp) | 85, 39, 81 | k = 16385 (2^268.00), limbs 95/95/95 bits | [-2^84.37, 2^101.00], [-2^84.37, 2^102.00], [-2^84.44, 2^102.58], [-2^80.00, 2^102.00] | [-2^84.37, 2^94.00], [-2^84.37, 2^95.00], [-2^84.44, 2^95.02], [-2^80.00, 2^94.04] | 2^191.46 | 2^515.00 |
| Pasta p (Fp-in-Fq) | 87, 39, 81 | k = 16385 (2^268.00), 95/95/95 | [-2^86.27, 2^101.00], [-2^86.27, 2^102.00], [-2^86.28, 2^102.58], [-2^80.00, 2^102.00] | [-2^86.27, 2^94.00], [-2^86.27, 2^95.00], [-2^86.28, 2^95.02], [-2^80.00, 2^94.04] | 2^191.46 | 2^515.00 |
| P-256 p | 87, 9, 82 | k = 4097 (2^268.00), 95/95/95 | [-2^87.00, 2^101.00], [-2^87.00, 2^102.00], [-2^87.04, 2^102.58], [-2^82.00, 2^102.00] | [-2^87.00, 2^94.00], [-2^87.00, 2^95.00], [-2^87.04, 2^95.02], [-2^82.00, 2^94.04] | 2^191.46 | 2^517.00 |
| P-256 n | 85, 87, 82 | k = 4097 (2^268.00), 95/95/95 | [-2^84.56, 2^101.00], [-2^87.24, 2^102.00], [-2^87.28, 2^102.58], [-2^87.04, 2^102.00] | [-2^84.56, 2^94.00], [-2^87.24, 2^95.00], [-2^87.28, 2^95.02], [-2^87.04, 2^94.04] | 2^191.46 | 2^517.00 |

`log2 N = 254.000000` for both `Fp` and `Fq`; `N 2^348 > 2^602` (rounded log2: 602.00); the largest
`|X|` over all range-checked assignments is below `2^537`.

## 6. Completeness (honest provers)

- Carries: for operand limbs `<= 2^94 - 1` the honest carry intervals above lie in
  `[-2^104, 2^104)` with at least 1.4 bits of margin (largest `2^102.58`).
- Quotient: `q = floor(a b / m) < 2^261` iff `a b < m 2^261` (the chip's
  `mul_admissible`, checked at synthesis from structural bounds; inadmissible
  operands are reduced first). Proper x proper `< 2^512 < m 2^261` always (this is
  why `m >= 2^252` is required). Sums of two proper values (`< 2^257`):
  `2^514`, admissible for every modulus. Subtraction results `x - y + K`
  (`K` dominating proper `y`): `2^257.17` (Pasta, `k = 5`) and `2^257.58` (P-256,
  `k = 2`); their products `2^514.34` and `2^515.17` are admissible
  (`< 2^515.00` resp. `2^517.00`).
- Division: `q = (b c + K - a) / m` with `c < m` canonical, `0 <= q <= b + k`; the
  chip requires `max(b) + k < 2^261` (`div_admissible`). Strict `q < b + k`
  requires `b > 0`; at `a = b = 0`, the honest quotient is `q = k`.
- Canonical comparison: `x <= m - 1` gives `d = m - 1 - x` with standard limbs
  (`< 2^87`) and a borrow `beta in {0, 1}` of the low 174 bits.

## 7. Canonical comparison

Constraints (with `x` proper and `d` range-checked to `2^87, 2^87, 2^87`;
`x` is the copy on the `Q` operand row or, for a canonical witness, the `C`
root, `d` the `Q` root, both read from row 0):
`beta := (m-1)_2 - d_2 - x_2`, `beta (1 - beta) = 0`, and
`(d_0 + x_0 - (m-1)_0) + (d_1 + x_1 - (m-1)_1) B - beta B^2 = 0`.
`beta` is 0 or 1, and `|(m-1)_2 - d_2 - x_2 - beta| < 3B < N`,
so the high-limb equality lifts to `d_2 = (m-1)_2 - x_2 - beta` over the integers; the low
equation is bounded by `4 B + 4 B^2 < 2^177 < N`, so it is an integer equation.
Adding: `d + x = m - 1`, and `d >= 0` gives `x <= m - 1`. A value `x >= m` makes
`d` negative, which no running sum represents (tests at `m - 1`, `m`, `m + 1`).

## 8. Bound tracking (operands are proven bounded integers)

Every `FfValue` comes from (i) a range-checked running sum, (ii) constant limbs
pinned through the constants column, or (iii) a limb-wise linear combination of
values with nonnegative integer coefficients plus a padding `K = k m` whose limbs
dominate the subtracted value's tracked bounds (`ForeignModulus::padding`). The
tracked bound of every result limb is the integer maximum of that combination,
stays `<= 2^94 - 1` (otherwise inputs are reduced first), and is far below `N`, so
the field values are exactly those integers. Bounds are structural (circuit
shape), never witness-dependent.

## 9. Gate shape (measured)

- Exact (synthesis inventory, `ff_inventory_per_operation`): 70 advice cells per
  multiplication or division (7 rows x 10 columns, no idle cell); proper witness
  18; canonical witness 36; comparison of an existing value 21. Gates degree 3,
  `C`/`Q` lookups degree 6 and `U` lookups degree 5 (10 width-1 range lookups,
  one per column), 4 fixed pattern columns plus one table column (32,768
  rows, `k >= 16`), 3 queries per operand column (blinding 5, unchanged).
  G3.4 threshold <= 100 cells: met (70).
- M3 release measurement (`ff_gate_shape_measurement_release`, before D1, with
  the tagged table and 6 pattern columns; P-256 `p` in `Fq`, Pallas proof,
  KAGEMUSHA transcript, Direct instances): 9,000 chained products in 63,014 rows
  at k = 16 (630,036 FF cells), 14 advice / 15 fixed columns, proof 6,944 B,
  keygen 2.7 s, prove 6.96 s on 20 threads, verify 62 ms, process peak RSS
  0.92 GiB; with `RAYON_NUM_THREADS=1`: keygen 10.5 s, prove 30.4 s wall,
  verify 0.45 s, peak RSS 0.84 GiB, at load1 ~50 (indicative only; counts
  exact).

## 10. Residual assumptions and flags

- Engineering argument, not a published proof (design C4 flag); the integer
  argument above is elementary and fully quantified, and every inequality is
  re-checked by the named test and the script.
- The chip trusts `iroha_plonk`'s lookup and permutation arguments (C1).
- `m < 2^256` is required for canonical values to be proper; P-256 `m + 1 <
  2^256` holds, so `witness_canonical(m)` exercises the comparison, not the limb
  ranges.

## 11. The shared table of the Q leaf (M3b decision D2)

In the Q-leaf layout (`iroha_plonk_gadgets::q_leaf`) `V` is the last column
of the shared table `[T, x_0, x_1, x_2, y_0, y_1, y_2, V]`. Eight range
arguments read `V` alone; the `c_0` argument also carries the SHA-256 spread
lookup (tuple `(T, x_0, V)`), the `u_0` argument the P-256 window lookup
(all eight components). Lemma 1 needs only "a range input matches only a
value `< 2^15`":

- width-1 arguments: every `V` entry on a usable row is below `2^15` (range
  rows `v`, SHA dense values `< 2^11`, window digits `< 2^8`, dynamic entry
  indices `1..16`, zero elsewhere);
- merged arguments: on foreign-field rows the guest is inactive (fixed,
  witness-independent patterns: SHA and window rows are below the split row,
  the foreign-field patterns start at it), so the tuple is `(0, .., 0, R)`,
  which matches only rows with `T = 0`; those are the range rows (all other
  components zero) and all-zero rows. SHA tags are `2^33 + w`, dynamic window
  tags `2^32 + row`, fixed window tags in `[1, 2^32)`.

`QLeafConfig::audit` checks these conditions on the synthesized fixed
columns (row-disjoint activation of host and guest in both merged arguments,
the tag namespace of every table row, `V < 2^15`, and the window's
dynamic-entry enable `q_dyn` zero on every fixed row and boolean above, so
no advice is added to a fixed entry); the leaf tests run it on
key-generation and proving syntheses and on deliberately malformed layouts,
which it rejects. (Codex's implementation review found the `q_dyn` gap in
a first version of the audit; it found no soundness issue in the
`QLeafConfig::chips` layout itself.)

## Review boundary

The original independent Codex review accepted all four modulus bounds for
source digest `fb786437051f425862895196d64e59610fe2aade700a6c7abfdd9b1f00cae1b4`
of `ff/mod.rs`, before the M3b range layout. Its four wording corrections are
included above. That acceptance covers the interval/CRT argument, not the
whole P-256 gadget, prover, or a changed range-check implementation.

The M3b implementation must additionally establish the running-sum and shared-table
premises of §§2 and 11: every result, quotient, carry and comparison difference
is range-checked, copied to the arithmetic cell, and enabled at the correct
rows through the final usable row. Tests must exercise both native fields,
all moduli, terminal digits, carry endpoints, quotient overflow and malformed
shared-table layouts. Passing only the integer-bound calculation is insufficient.

Qualification records bind this review to the measured source and binary. A
change to widths, radix, carries, padding or operand provenance requires new
numerical bounds; a pure range-layout change requires a new range/binding proof.
Independent engineering review on 2026-10-06 accepted the current M3b arithmetic,
two-membership range binding, canonical comparison and shared-table argument
for all four moduli in both native fields. The original 20 FF cases, the two
new boundary cases and four Q-leaf layout-audit cases passed; the three ignored
FF measurement/exhaustive cases are not included in that result. Exact integer
rederivation matched the bounds above. The reviewed implementation hashes are:

- `ff/mod.rs` SHA-256 `1ec6cf5a17146a07af5d5f1f4a45bd8752fef691f311ef225e6a0d49cca8bfc5`.
- `q_leaf/mod.rs` SHA-256 `ac22585f76976b554d8a5a4ecb765eaf3f3ae89cb09e1cbf17b5a6f7701ab2d5`.

This closes the scoped carry/range implementation review. It is not an external
cryptographic audit, proof-engine qualification, performance/device result or
deployment authorization; those gates remain separate.

## 12. Recursive scalar adapter and canonical S6 certificates

The 2026-10-06 adapter revision keeps the seven-row fused gate, radix, quotient,
carry ranges and its admission checks unchanged. It changes operand provenance
and reuses already proved integer ranges. The earlier carry review does not by
itself establish these adapter properties.

Internal recursive scalar addition, subtraction and negation may retain an
unreduced integer congruent to the scalar. For limb bounds `A_i,B_i`, addition
tracks `A_i+B_i`. Subtraction chooses the existing fixed multiple `K=t m` with
`K_i>=B_i`, and tracks `0<=a_i+K_i-b_i<=A_i+K_i`; negation tracks
`0<=K_i-b_i<=K_i`. Each limb remains at most `2^94-1`, or the existing reduction
path is used before retrying. These native-field equations therefore cannot
wrap. Multiplication still invokes `make_admissible`, including
`max(a) max(b)<m 2^261`, and division retains its padding/quotient admission.
The value's native-field residue is not its foreign scalar: equality, zero
testing, the guarded inverse's zero test, S6 export, scalar-bit transfer and
transcript/public transfer canonicalize the integer before interpreting it.

`CanonicalS6` is an opaque certificate for the same bounded cells
`x=lo+2^128 hi<m<2^255`. It has no public unchecked constructor. Import to FF
splits `lo=a+2^87 b0` and `hi=b1+2^46 c`, with widths `87/41/46/81`, then sets
`b=b0+2^41 b1`. The equalities are integer equalities: their nonnegative sides
are below `2^128`, and `b<2^87`. Export first proves the three-limb FF integer
canonical. Since `x<m<2^255`, its nonnegative high limb satisfies `c<2^81`;
splitting the middle limb into 41 and 46 bits yields the same integer and
proves `lo<2^128, hi<2^127` without duplicate range lookups. Changing a
certificate's modulus checks the narrower bound when needed; widening retains
the original constraints. `ScalarCells` additionally requires the exact curve
scalar modulus, so a foreign certificate cannot silently change meaning.

The root implementation reviewer independently checked this import/export
integer argument and modulus handling. Exact-cell caches live only for one
chip/synthesis and retain the earlier constraints. Structural constant folding
recognizes pinned constant cells, never witness values. General operation
reuse is keyed by both operands' proving-cell identities. Fixed small-scalar
multiplication uses the existing bounded `scale` operation and its reduction
checks, not a wider carry envelope.

Validation at this source snapshot passed:

- both-direction S6 roundtrips, every assigned-cell mutation, modulus/top-bit
  aliases, explicit widening/narrowing and known/unknown layout equality;
- seven recursive verifier tests on both curves, including all malformed proof
  messages, modular aliases at each semantic boundary, a nonconstant long
  arithmetic chain, and distinguishing a witness one from a pinned constant;
- all three uniform Omega source choices and frame/proof mutations;
- strict gadget and recursion all-target clippy.

The snapshot is based on shared checkout HEAD
`e5c89263b05d82efdc11127f5d9418373f4f4382` plus the ongoing changes. Its SHA-256
source bindings are:

| Source | SHA-256 |
| --- | --- |
| `iroha_plonk_gadgets/src/ff/mod.rs` | `5d9056a03e935490f144674b2300e8e256578f787b9855cfa32c08031941aedc` |
| `iroha_plonk_gadgets/src/ff/s6.rs` | `2769230d3c6d6749874613a2970774499ac67c7cd8f8b770d3b35e23c50a7b55` |
| `iroha_plonk_recursion/src/verifier/scalar.rs` | `dd1c4c6f1a7d462f0942e774727a712e96e931d41796392c9794373bf9f21e56` |
| `iroha_plonk_recursion/src/codec.rs` | `d239c30a30c84ef8776578c910ed0db765aaaa5aa1fb328bff6ce273c9516d41` |

The `ff/mod.rs` change from the reviewed M3b hash adds the S6 module/export
and an explicitly selected serialized backend; the original fused gate
polynomials and their admission checks are unchanged. The Q-leaf source remains bound
to the earlier `ac22585f…` hash. This scoped adapter/range review is neither
an external audit nor a recursive release qualification. The measured generic
Omega descriptor still exceeds the transport cap; none of these tests changes
that verdict.


## 13. Serialized CRT lowering and shared range certificates

The explicit serialized backend retains the same result limbs (87/87/82),
quotient limbs (87/87/87), four offset carries (105 each), division padding,
and tracked operand admission from the fused backend. It lowers each of the
four carry equalities and the independent native-residue equality to ordinary
Glue rows. Intermediate field-valued partial sums are not treated as bounded
integers: only the complete original carry residuals use the established
non-wrapping bounds. The native-residue constraint is retained separately, so
satisfying only the four low-radix equalities cannot accept a CRT alias.
Canonical comparison retains its proper limbs, three 87-bit difference limbs,
boolean borrow and both original integer comparison equalities.

Every emitted result, quotient, carry and comparison difference is checked by
the existing running-sum predicate and copied to the exact arithmetic cell.
Independent range buses share only their fixed table. Each has its own
activation pattern and a full scalar membership argument; a tuple is never
interpreted as independent membership. The deterministic scheduler chooses
the least occupied bus from structural widths and prior row counts, with ties
resolved by fixed bus order. No witness value changes this selection.

The optional range certificate map is fresh for each synthesis and keyed by
physical advice cell (column and absolute row). It is populated only after a
range predicate and any necessary equality copy have been emitted. A proved
bound of `b` bits discharges a request for `b' >= b`; a narrower request emits
new constraints. Equal witness values in distinct cells do not share a
certificate. Cloned chips share both reservations and certificates, preserving
the original constrained cells. The ordinary retained layout does not enable
this optimization implicitly.

Validation includes all four foreign moduli in both native fields, multiply,
divide, zero-divisor and modulus mismatch rejection, noncanonical aliases,
carry overflow, a low-radix-only forgery rejected by the native residue,
every-cell mutation and known/unknown layout equality. The current FF unit
slice passed 35 tests (three explicitly ignored exhaustive/measurement cases
excluded). The certificate/banked-range tests passed in both fields; the full
parallel interpreter additionally matched native verdicts with two and three
buses, including malformed encodings and lengths. Scoped strict gadget and
recursion all-target clippy passed. These are implementation checks; this
section does not claim an external cryptographic sign-off. The root
implementation reviewer separately read the serialized equations, per-bus
range arguments, exact-cell certificates and structural scheduler. That scoped
source review found no local range/binding gap and confirmed the original
carry/result/quotient widths and four carry plus native-residue equalities. It
does not establish current M3 qualification, whole-recursion soundness or
release readiness.

Serialized/range source bindings for this snapshot:

| Source | SHA-256 |
| --- | --- |
| `iroha_plonk_gadgets/src/ff/serialized.rs` | `18cdc38ccbb796297f0e980fbc040c1df1b649451d5e1b9b401709ddeb4a040f` |
| `iroha_plonk_gadgets/src/range/running_sum.rs` | `b6b62eea7f55a42c2275f4a57b393facfa481dcad181dcc97d273187de141906` |
| `iroha_plonk_gadgets/src/arith.rs` | `24f870b3d4efb161e0d29355c6ad9d543665acb2f75963edb04fd186222d5c75` |
| `iroha_plonk_gadgets/src/phase.rs` | `a7ab721dc06249afe4bffdc776e780320c2b0a8579dbfab7d13aab365f45da1a` |


## 14. Four-row CRT placement

`RotatedFfConfig` places the exact sixteen existing roots on four equality
ports over four physical rows: three left limbs, three right limbs, three
result limbs, three quotient limbs and four offset carries. The gate anchors
at the second row and queries rotations `-1..2`. It invokes the original five
fused residuals; division retains `(right,result,left,padding)`. Result bounds
remain **87/87/82**, quotient bounds **87/87/87**, and each offset carry remains
**105 bits**. A review message initially misstated the limb widths as 86/86/84;
the reviewer explicitly corrected that summary. No width change was requested
or implemented.

Every result/quotient/carry root comes from its exact range certificate and is
copied into its assigned port/rotation. The chip rejects a fused-only backend,
wrong ports, or a different/multiple configured modulus before attaching the
kernel. All four rows are reserved together. The selector at global row zero
is off, so the negative rotation cannot introduce a wraparound relation.
The last block may end at the final usable row; a block crossing into blinding
rows is rejected by both the bounded cursor and the assembly.

The shared phase layout uses ECC payload5 codes3/4 for multiplication/division;
its guard/split indicator domain includes those codes, so ECC constraints stay
disabled on CRT rows. The independent reviewer checked these exact root/copy,
phase and unchanged arithmetic-bound premises. This is a scoped source review,
not current M3 qualification or acceptance of a different integer envelope.

Validation passed all four moduli in both native fields: arithmetic/reference
parity, every-cell mutation, known/unknown shape, bad modulus/profile/ports,
unchanged admission rejection, first/final physical blocks, every copied root
and rotation, and final usable/blinding boundaries. The complete compact and
parallel interpreters passed native differential tests. The actual phased
Glue/CRT/ECC/Poseidon component passed every-cell mutations on both curves.
At the preceding four-row snapshot, the centered compact descriptor was 4,960 transport bytes and the genuine
pooled-source Bootstrap needs 74,597 shared rows and 152,850 range rows;
these remain explicit failures of the 4,821-byte and k16 gates. Pinning its
single complete source key reduces shared rows to 71,964 without changing the
range or transport failures; the complete catalog is not yet qualified.

Current rotated/pooled source bindings (shared HEAD `e5c89263b05d82efdc11127f5d9418373f4f4382`
plus ongoing changes; prior snapshot hashes above remain historical):

| Source | SHA-256 |
| --- | --- |
| `iroha_plonk_gadgets/src/ff/rotated.rs` | `27a117c097eae7652594d4793ac1cc4683e02024ee268029fd5f891937bbccdc` |
| `iroha_plonk_gadgets/src/ff/serialized.rs` | `679c556a9791dba7a8a63e26e4e846b575e1d26db587734e52afcaf4b19923aa` |
| `iroha_plonk_gadgets/src/range/running_sum.rs` | `b6b62eea7f55a42c2275f4a57b393facfa481dcad181dcc97d273187de141906` |
| `iroha_plonk_gadgets/src/phase.rs` | `a7ab721dc06249afe4bffdc776e780320c2b0a8579dbfab7d13aab365f45da1a` |
| `iroha_plonk_gadgets/src/arith.rs` | `5875082946b7a59ac6ed4590622fe28a0c2aafbf5e38990bd68310f6d52db560` |
| `iroha_plonk_gadgets/src/ecc/gates.rs` | `b52d9ce6317dd7a407d15619a9bb10082f73c96f6f90cd00bd6815a0c14f7f40` |

## 15. Unsigned Proper dot batches

The new standalone `UnsignedDot` predicate admits a fixed batch of one to eight
pairs, each with proven Proper or Canonical 87/87/82-bit limbs and one common
supported modulus. It rejects signed/lazy forms, mixed moduli and counts0/9.
Let `S=sum_t a_t b_t`, `B=2^87`, and `m` be the foreign modulus. Since each input
is below `2^256`, `S<2^515`; every supported `m>2^254` therefore gives the honest
quotient `q=floor(S/m)<2^261`. The result is Proper and congruent to the sum;
canonical interpretation still requires the separate comparison.

For `j=0..3`, define
`D_j=sum_t sum_{r+s=j} a_tr b_ts - sum_{r+s=j}q_r m_s - c_j`, with `c_3=0`.
The equations are `D_0-Bv_0=0` and `D_j+v_(j-1)-Bv_j=0`, plus the independent
native residue of `S-c-qm`. Result, quotient and offset carry ranges are exactly
those above. Each honest `|D_j|<28B^2`; induction gives `|v_j|<29B<2^92`, inside
the existing signed104-bit carry interval. Even malicious105-bit offset roots
make each local residual smaller than `2^194`, below either native prime.
Telescoping therefore proves divisibility by `B^4`. The global residual is below
`2^518`, whereas `B^4*p>2^602`, so the two congruences imply integer equality.
The root reviewer independently re-derived this specific unsigned Proper
argument. It does not cover a signed/lazy or larger batch.

The component tests passed batch1..8 with zero and maximum256-bit inputs in
both native fields for all four moduli; batch1/8 every-cell and known/unknown
shape tests; malformed batch/form/modulus rejection; quotient/carry overflow;
and a low-radix-only forgery rejected by the native residue. The initial
lowering uses `20+11(n-1)` Glue rows and one set of result/quotient/carry checks
(70 range rows at15-bit table width). The shared-profile S11 and multiopen
Horner folds now use fixed unsigned batches; negative terms are materialized
through the separately admitted subtraction/negation path. This does not
extend the unsigned kernel to signed or lazy operands.

## 16. Staged products, streaming dots and fixed-coefficient constants

The explicit compact profile retains the same sixteen roots and the same
four physical rows for multiplication/division. It binds the four low
convolutions and native product on five spare advice columns, then checks
the original carry/native residuals on the following row. Its queries use
only rotations `-1,0,1`. Division first orders its operands as
`(right,result,left,padding)`, including the unchanged fixed padding.
Neither the range envelopes nor the integer argument in the earlier sections
changes: the intermediate products are field elements, and the complete
residuals still have the established non-wrapping bounds.

For a fixed unsigned batch of `n=1..8`, the streaming version starts all five
partial sums at constrained zero, adds each operand product, copies each sum
to the next state, and finishes with the same result/quotient/carry residuals.
It consumes `2n+3` arithmetic rows and one set of range certificates. Its
admission remains exactly the Proper/Canonical, common-modulus predicate in
§15. The explicit reference backend retains the Glue lowering.

The phase coding separates every predicate: multiplication/division use ECC
payload5 codes3/4, final residuals use payload2 code4, zero initialization
uses payload2 code3, product steps use payload1 code3 and state carry uses
payload3 code3. Algebraic15 uses payload0 code3. The corresponding ECC
indicator domains include those exact extra codes, disabling ECC gates on
these arithmetic rows. No witness value selects a phase or row span.

The compact Glue profile constrains a constant with the fixed-coefficient
equation `x-c=0`, removing its standalone equality-enabled fixed column.
ECC coordinates and Poseidon constant inputs copy these exact constrained
cells. Attaching this source requires matching copy ports and a shared,
bounded cursor; unbounded, unrelated-port and ordinary fixed-column profiles
are rejected. The shared cursor reserves constant rows with all other Glue
and CRT rows, so these copies cannot silently collide with another phase.

The root implementation reviewer read the staged operand ordering, padding,
zero/product/carry/final equations, unchanged admissions, coefficient-only
constant constraints and bounded cursor/port checks. That scoped source
review found no local gap. It is not a new M3 qualification, an external
audit or a proof of whole-recursion soundness.

The staged kernels passed both native fields and all four foreign moduli,
including all batch sizes, endpoint/admission and alias attacks, every-cell
mutation, first/final usable and blinding-row boundaries, and known/unknown
shape parity. The final mixed-phase component passed both fields with
Glue, multiplication/division, an eight-pair dot, ECC, Poseidon and
Algebraic15 together, including every-cell mutations and exact public
binding attacks. The complete compact interpreter passed native differential
tests, and scoped strict gadget/recursion all-target lint passed.

The five-bus authenticated Bootstrap source is 8,960 proof bytes. The complete
compact descriptor has 11 advice columns, 11 fixed queries, 26 advice queries,
three instance queries, five permutation columns in one set, exactly one
lookup, degree9 and five blinding factors. Its 3,680-byte proof plus the
1,088-byte accumulator is **4,768 bytes**. This descriptor meets the byte
formula only. At the first staged five-bus snapshot, its pinned one-key
predicate still needed **73,829 shared rows and 148,816 range rows**, exceeding
the 65,530 usable rows at k16; the full predicate passed only a diagnostic
larger-domain synthesis. No complete compact proof or final catalog is
qualified by this inventory. Later arithmetic measurements supersede these
row counts only when recorded against their tested source.

The public-input evaluator factors
`L_i(x)=((x^n-1)/n) * omega^i/(x-omega^i)` before unsigned dot batching. It
retains an independently constrained guard `g=[x^n-1 != 0]` in the global
verdict and caches weights only by exact fixed index within one proof
evaluation. A narrowly scoped guarded division proves
`(x-omega^i)*inverse = g (mod m)` using the unchanged FF division predicate,
and constrains `(1-g)*inverse_limb=0` for all three Proper result limbs.
When `g=1`, `omega^(i*n)=1` implies `x-omega^i != 0`, so this is the exact
inverse. When `g=0`, all inverse limbs equal zero; the relation is total even
at a zero denominator, while the global verdict is already false. This
reuses the single vanishing proof rather than repeating canonical zero tests
for each root. The generic inverse retains its original independent nonzero
predicate and guarded-one behavior.

The direct regression tests `x=1`, another nontrivial root and an ordinary
non-root, first/last and matching/nonmatching root indices, each inverse limb
and guard/verdict mutation, index-cache cell identity, and known/unknown
shape on both curves. Factoring and sharing this root-specific guard change
neither the accepted native predicate nor total malformed-proof handling.
Large structural constants are included in unsigned batches unless their
product has an existing cheap lowering (both constants, or zero, one, minus
one or an unsigned constant at most128). A witness equal to one never
acquires constant metadata. Both-field constant/witness output-limb mutations
and known/unknown shape checks pin this distinction.

The root reviewer independently read the guarded weight and its caller guard,
including the nth-root implication, false-guard limb equalities and global
verdict binding, and the structural constant dispatch with Proper conversion.
That narrow source review found no gap; it is not a blanket sign-off on a new
carry layout or complete recursion. The scoped latest recursion all-target
strict lint passed.

Staged/constant/guard source snapshot at shared HEAD
`c2e5bd8978d4ae931b8beef54392bb10a33ad3a6` plus ongoing changes:

| Source | SHA-256 |
| --- | --- |
| `iroha_plonk_gadgets/src/ff/mod.rs` | `6ba7cac96c404dfc04721e6c359e3c2e999a0fdbe242517e51993fe7d9022ce3` |
| `iroha_plonk_gadgets/src/ff/rotated.rs` | `3ba5992ebd3d2f4da94401e0bc97d37c52350d40fbe30ba4aa33bc12203e4f85` |
| `iroha_plonk_gadgets/src/ff/dot.rs` | `d2b5fc970e91a3fe11ab378c2d04b8f91c367b80d3e716e5181a2c9704592422` |
| `iroha_plonk_gadgets/src/arith.rs` | `8f8f9b4a48c817366236aafeedbdc009abaf911096051ed6bb9928b351cebf3e` |
| `iroha_plonk_gadgets/src/ecc/gates.rs` | `8ed1743c9873b089f321d43245648c2a09ad4f06b413ede5d728cda5e8b9ff4a` |
| `iroha_plonk_gadgets/src/ecc/mod.rs` | `d450f5c53f3e86bcfa4c63dc43cdd6bf757e427be57b0570c728efecfebe5476` |
| `iroha_plonk_gadgets/src/poseidon/pow5.rs` | `2c0fc66ee124ec129bf8461cc06ade94ae644ec2ccb738305786e09d1ad9009d` |
| `iroha_plonk_recursion/src/verifier/scalar.rs` | `02a127c06fad302f1587c0fde172940ac9447aca60bb6ac0dd8c56f0c300a13f` |
| `iroha_plonk_recursion/src/verifier/expressions.rs` | `9c728a189b5f3f83a2ba24674ffa6bda2596ca82d78d9cdf0bdbce2a47c03e65` |
| `iroha_plonk_recursion/src/verifier/multiopen.rs` | `7c57d5608e706ec96ddd4483988b9fb1fa8574285af7eedb6ab73481d356837a` |
| `iroha_plonk_recursion/src/verifier/compact.rs` | `fa161ae19ec1b466e0e0d5c4c105512fbe9b17146c3f81b2c47851c48bad9a14` |
