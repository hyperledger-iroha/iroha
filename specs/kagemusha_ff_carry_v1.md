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
