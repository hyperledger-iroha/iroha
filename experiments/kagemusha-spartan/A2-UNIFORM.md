# Stage A2: uniform blocks — status 2026-11-10 (fixed point reached: the uniform pair verifies its own proofs)

This is an experimental Receive harness. The [README](README.md) lists the
unfinished Send, binding, base-case admission and production integration work.

Owner: "Start stage A2".

## What A2 has to deliver

The next proof's verifier must compute the A, B and C matrix MLEs exactly. That has to be cheap enough to synthesize
in-circuit: the census budget is about 8,000 constraints per verified proof. REPORT4 and b04a57's soundness review
settle how:

- uniform blocks, about ten types;
- wiring through copy lookups into a value table, with preprocessed index columns;
- no challenge-dependent "answers" from preprocessing;
- any sparse exception evaluated or proved.

Stage A's P-recv is an arbitrary sparse matrix with 5.74 M nonzeros, so its MLE costs O(nnz).

## Block census

Run it with `ZKS_SCHEDULE=lean ZKS_BLOCKS=1 zk-spartan real-receive`. It assembles the pair, prints the census and
stops before proving.

**Method** (`src/blocks.rs`):

- Gadgets open scopes. Every row and lookup is tagged with its innermost scope. A block is one scope instance.
- Block types come from canonical forms: variables are renumbered by first use, and the shape and the coefficients
  are hashed.
- A **wire** is a variable that two or more blocks reference. Each referencing block reads it with one copy lookup.
- A **chain wire** is shared only by consecutive blocks of one type. A shift can wire it, with no lookup.
- **Template nonzeros** are the sum, over types, of one instance's nonzeros: the verifier's matrix-MLE work for
  uniform types.
- `ZKS_FLATTEN=a,b,...` makes the listed scopes transparent.

**Results**, P-recv / S-recv:

| Layout | Blocks | Types | Template nonzeros | Wires | Copy lookups | Of which chain-wirable |
|---|---:|---:|---:|---:|---:|---:|
| Every gadget its own block (v2) | 14,317 / 11,409 | 202 / 65 | 466k / 570k | 107,805 / 65,296 | 309,892 / 163,203 | 36,650 / 41,102 |
| Outermost gadget per block (v2) | 946 / 628 | 60 / 53 | 2.06 M / 1.01 M | 28,549 / 21,499 | 85,738 / 66,418 | 7,134 / 444 |
| Loop steps as blocks, helpers merged in, whole permutations (v4) | 10,530 / 10,904 | 124 / 62 | 710k / 568k | 58,033 / 45,518 | 172,074 / 140,021 | 42,846 / 41,106 |

For scale: P-recv has 368,601 constraints. The census assumed about ten types, an 8,000-constraint matrix MLE, and 3
copy lookups per non-native check plus 6 per permutation.

**Conclusion.** No grouping of today's gadgets meets the budget:

- small blocks need about as many copy lookups as the circuit has constraints;
- large blocks give templates of millions of nonzeros.

The gadgets themselves have to be written as uniform steps. Their regular dataflow must be wired by structure, and
copy lookups kept for irregular flows.

Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-block-census-v{1,2,3,4}*.log`.

## What blocks uniformity today

1. **Permutations** (perm, permute). They are about 49% of P-recv's rows and 33% of S-recv's.
   - A whole-permutation template has about 6,765 nonzeros, and there are 13-16 types (they differ in ports and
     constants). One template alone nearly fills the 8,000 budget.
   - Splitting into rounds does not help as written, because the state is not materialized. In partial rounds, the
     unboxed lanes stay linear combinations across rounds, so a round references cells many rounds back. That cost
     1.1 M copy lookups (v3).
   - Fix: materialize the three lanes after each round. That gives one full-round and one partial-round template of
     a few rows each, chained by shift, with the round constants as a 57-entry public column.
2. **Elliptic-curve ladders** (endoscale, var_mul, fixed_mul).
   - The steps are already good templates. endoscale_step has 7 rows, 33 nonzeros and 8 ports, with 7,749
     instances in P-recv.
   - But every step reads the base point (fan-out 64), and the bits come from canonical_bits blocks.
   - Fix: carry (accumulator, base) along the chain, and produce the bits inside each step, with the running
     recomposition sum chained across steps.
3. **Non-native checks.** Limbs are allocated outside the check that uses them: mul_check + nn_alloc share 36,912
   wires (v2). Fix: allocate the limbs inside the block that checks them.
4. **ECDSA comb.**
   - The 32 comb steps are each their own type, because the selected fixed-table entries are baked into the rows as
     constants. Each step has 9,050 nonzeros and 1,344 range lookups.
   - Fix: select fixed-table points by lookup (cells, not constants), so there is one comb-step type.
5. **IPA-verifier vector algebra** (s_vector, eq_table, closed_form, grid_weights, product).
   - These blocks share thousands of wires, and some rows carry 203k nonzeros (one dense linear combination).
   - Fix: compute the vectors level by level (entry i reads entry ⌊i/2⌋), and dot products as accumulation chains.
6. **SHA-256 rounds.** Each round has 43 ports and 47 coefficient variants (the round constants). Fix: chain the
   shift-register state by shift, and put K_t in a 64-entry public column.
7. **Binding steps.**
   - Each step has 72 ports. Every step is its own coefficient variant, because the spike's shared values are
     constants.
   - Fix: chain the accumulator, carry ρ along the chain, and read each shared value with one copy lookup.

## Target design

- **A circuit is a set of step types.** Each type is a small template (rows over local cells), repeated in a
  contiguous array.
- **Links between instances:**
  - **shift**: instance i's output cells are instance i+1's input cells. Its MLE term uses the "next" relation, in
    O(log n).
  - **carry**: an input passed along a chain unchanged; a special case of shift.
  - **tree**: instance i reads instance ⌊i/2⌋ (vector expansion). This is also structured.
  - **copy lookup**: LogUp over (address, value) into the value table, for irregular flows. The addresses are a
    preprocessed index column that is committed and opened together with the witness, never trusted as an answer.
- **Per-instance constants** (round constants, K_t, table ids) go in short public columns that the verifier
  evaluates directly.
- **Matrix MLE** = Σ over types (template MLE × structured link terms) + small sparse exceptions. Its cost is about
  the template nonzeros + O(types × log n) + the public-column lengths.
- **The prover never builds the matrices.** It computes Az, Bz, Cz and M(r_x, ·) from the templates over the
  arrays. That also removes the per-proof matrix build (0.72 / 0.33 s on the Pixel) and the ~550 MB matrix set.

## Step 1 done: uniform relations and the RP57 permutation (2026-11-09)

Owner: "Start permutation". Code: `src/uniform.rs`. Run: `ZKS_REPEAT=3 zk-spartan uniform-perm`, with
`ZKS_PERMS_LOG` (default 9, i.e. 512 permutations). Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-perm-v1.log`.

**The uniform relation.**

- A circuit is a set of arrays. Each array is a template repeated over instances i = p · 2^ρ + r; the arrays of
  one family share the p range.
- A template term reads one of three sources:
  - the constant 1;
  - a cell of its own instance;
  - a cell of instance (p, map[r]) of another array in the family (a link).
- A link's map can switch it off for some r. A coefficient can depend on r; the round constants use that.
- Every (array, cell) is an aligned power-of-two block of the witness, in stripes of 2^10 active columns. Every
  template row is an aligned block of constraints.
- So each instance bit lands on one index bit, and each matrix-MLE term factors into three parts:
  - the fixed bits;
  - a product over p's bits;
  - a sum over r.
- The verifier evaluates A~, B~ and C~ in O(terms · 2^ρ + bits). The prover computes Az, Bz, Cz and M(r_x, ·) from
  the templates. M(r_x, ·) is a gather: each cell sums the rows that read it. No matrix is ever built.
- Tests:
  - `uniform_relation_matches_its_expansion`: the structured values equal the fully expanded matrices', at random
    points.
  - `uniform_permutations_prove_and_refuse`: honest proofs verify. A changed round cell is refused, and so is a
    changed state cell that the next block links to.

**RP57 as round arrays.** RP57 is 4 full rounds, then 57 partial, then 4 full. The arrays:

| Array | Instances | Cells | Template rows | Template terms |
|---|---|---:|---:|---:|
| Input lanes | p | 3 | 0 | 0 |
| Full round, f < 8 (rounds 0-3, 61-64) | (p, f) | 9 | 9 | 72 |
| Partial round 4 (materializes its output state) | p | 6 | 6 | 39 |
| 7 partial rounds, b < 8 (rounds 5 + 7b to 11 + 7b) | (p, b) | 24 | 24 | 246 |
| Mask gates | 2 | 3 | 1 | 3 |

- Inside a 7-round block, lanes 1-2 stay linear combinations. The block's output state is materialized as 3 cells
  at its end.
- Rounds link to each other by maps on r. For example, full round f = 4 reads block b = 7, and full round f reads
  f − 1. Every round's constants are per-r coefficients.

**Results** for 512 permutations (about P-recv's count). Mac, load 103, so timings are noisy:

| | Uniform | Same rows as a sparse assembly |
|---|---:|---:|
| Constraint rows | 138,242 (270 per permutation) | 138,242 |
| Witness | 16 rows × 16,384 | 16 rows × 16,384 |
| Proof | 1,952 B | 1,952 B |
| Verifier matrix-MLE work | 2,046 products in total, whatever the count | O(nnz) |
| Prove (core) | 0.205-0.238 s | 0.219-0.241 s, plus 0.021-0.037 s of matrices per proof |
| Verify | 0.012-0.013 s | 0.025-0.034 s, plus 0.018-0.028 s of matrices |
| Resident memory after proving | 135-139 MB | 377-543 MB |

- The stage-A gadget used 243 rows and about 6,765 template nonzeros per permutation, with 13-16 permutation types
  in the census. The uniform form costs 27 more rows per permutation (+11%), for the 9 materialized states. In
  exchange, the whole permutation family's matrix MLE costs 2,046 products, a quarter of the 8,000 budget.
- A changed cell is refused at full size ("ipa: final opening").
- Options, not taken yet:
  - Materialize the pre-MDS lanes 1-2 instead of the output state: 2 rows per block instead of 3, so 9 fewer rows
    per permutation. It costs about 1,500 more matrix-MLE products.
  - Larger partial blocks: fewer materialized rows, more template terms.

## Step 2 done: the elliptic-curve ladders (2026-11-09)

Owner: "Start ladders". Code: `src/ladders.rs`, plus `src/uniform.rs` changes. Run: `zk-spartan uniform-ladders`.
Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-ladders-v{1,2,3,4}.log`, `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-perm-v2.log`.

**Uniform relation additions.**

- A family's p range may be smaller than its power of two (`counts`): p sums use a prefix product. An array may use
  only part of its r slots (`r_count`).
- r sums use closed forms in O(ρ) when an array fills its r range and a term's coefficients are constant or
  geometric. That covers: the same step; a broadcast of one target step; a shift to step r − 1; one point. Anything
  else is enumerated over r-bit tables.
- The verifier's evaluation computes p products, fixed bits and step factors once per target block and step
  pattern. It counts its own multiplications, so the reported work is exactly what the next proof's verifier would
  synthesize.
- Tests: the structured values equal the expansion at random points, including a partial family and partial r
  ranges. All three ladders reproduce independent group arithmetic, and endoscale equals P · endo(bits). Proofs
  verify, and a changed cell is refused.

**The ladders.** Steps own their bits: booleanity in the step, plus a running recomposition sum, so a ladder hands
one scalar to the outside. Bases are read by broadcast links, accumulators by shift links.

- **Endoscale:** base (p); init, which is φ(P) + P followed by a doubling (7 rows); step (p, r): select (2 rows),
  2·acc + s (5 rows), 2 booleanity rows, and a running scalar t = 4t′ + s + 2e (1 row). 63 steps are padded to 64
  with a dummy step whose bits are 0.
- **var_mul:** base; init 2P; step: bit, select, double-add, running scalar (8 rows), 252 steps padded to 256;
  tail: the complete last three steps (13 rows), padded to 4.
- **fixed_mul:** first window; step: bits, product, a materialized table selection (2 rows), chord addition,
  running scalar with weights 4^j (9 rows), 124 windows padded to 128; tail: complete additions (10 rows); end:
  adds −offset. The window tables are per-r constants, so each base (U, H) is its own family.

**P-recv's ladders**: 123 endoscalings of 126 bits, 2 var_muls of 255 bits, and 4 + 2 fixed_muls of 254 bits:

| | Uniform | Gadgets (block census) |
|---|---:|---:|
| Constraint rows | 90,857 | about 78,900, counting the bits' booleanity and recomposition |
| Verifier matrix-MLE multiplications | 16,014 | O(nnz) |
| Mac verify | 0.04-0.07 s | 0.04-0.05 s, plus 0.012 s of matrices |
| Resident memory | 84-92 MB | 199-231 MB |

- The rows are 15% higher. The causes are the chained recomposition (1 row per step), fixed_mul's materialized
  selection (2 rows per window) and the dummy steps.
- Matrix-MLE multiplications by array family:
  - endoscale: about 2,100;
  - var_mul: about 2,500;
  - fixed_mul: about 11,200 over its two base families.
  - Most of fixed_mul's cost is its per-window point tables. They are arbitrary constants, enumerated at 2
    multiplications per window per term.
- The permutation family (512 permutations), counted the same honest way, is 6,396 multiplications.

**The census budget needs revising.** It assumed 8,000 constraints for the matrix MLE per verified proof. With
honest counting, permutations plus ladders already take about 22.4k. Remedies, in order of effect:

1. **Preprocessed constant columns committed in the verification key.** These hold the window tables, round
   constants, SHA K_t and comb tables. The verifier would open them with the witness instead of enumerating them.
   This removes most of fixed_mul's 11k and the permutations' round-constant enumerations.
2. Share the fixed-bit factors across blocks that sit in the same Hyrax rows and stripes.
3. Run fixed bases as var_mul ladders over a constrained constant base. That costs about 880 more rows per fixed_mul
   but removes the tables.

## Constant columns done (2026-11-09)

Owner: "Constant columns first". Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-ladders-v{5,6}*.log`, `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-perm-v3.log`.

**Mechanism.**

- A circuit may end with fixed Hyrax rows holding public constant columns (`Circuit::fixed_rows`, `fixed_values`).
- Their commitments are the verification key's part for constants: unblinded, computed once by
  `spartan::fixed_commitments`.
- The prover gives fixed rows blind 0, does not commit them per proof, and leaves them out of the proof bytes:
  32 B saved per fixed row.
- The verifier appends the key's commitments before absorbing the rows and forming the combined commitment. So the
  IPA opening proves the constants together with the witness, and the matrices only see constant coefficients.

**In the uniform relation.**

- An array can be `fixed`: data only, placed in the trailing rows, with no template rows of its own.
- Links may read a constant array of the same family (per-ladder constants), or a global one (a family with no p
  range) from any family.
- Tests:
  - A prover that changes a constant cell is refused, because the key's row commitment no longer matches the
    opening.
  - Proof bytes exclude the fixed rows.
  - The selector row described below pins each fixed base.

**Fixed-base multiplication without window tables.**

- `[k] B` runs as a variable-base ladder over the constant `B' = [1/2] B`: var_mul gives
  `[2k + 2^m + 1] B' = [k] B + [(2^m + 1)/2] B`. A complete end step then adds the constant
  `Q = −[(2^m + 1)/2] B`.
- Variable and fixed bases share one var_mul family (`mixed_mul`):
  - a constant selector column and the constant `B'` give the check `(base − B') · selector = 0`;
  - each instance has a constant correction point. Consumers of a variable-base instance ignore its end step.

**P-recv's ladders, verifier matrix-MLE multiplications:**

| Fixed bases | Multiplications | Rows |
|---|---:|---:|
| Windowed tables (stage A style), one family per base | 16,014 | 90,857 |
| Constant columns, their own family | 8,278 | 96,451 |
| Constant columns, shared with variable bases (default) | 5,935 | 96,479 |

The extra 5,600 rows are var_mul's 8 rows per bit against the windowed 9 rows per 2 bits.

**Round constants stay coefficients.** The permutation's round constants span only 8 steps per array. Enumerating
them costs about 850 multiplications. As constant columns, each of the 24 constant cells would be its own target
block, at about 54 multiplications of shared factors each (about 1,300 in total), so the verifier would do more
work. Constant columns pay off only for long tables.

**Budget now:** permutations 6,396 + ladders 5,935 = **12,331 multiplications** per verified P-recv proof (22,410
before), against the census's assumed 8,000 for everything. SHA-256, the comb, binding and vector algebra will add
to it. Next levers:

- share the fixed-bit factors of blocks in the same Hyrax rows (each target block costs about sw = 18
  multiplications now);
- share p products between arrays of one family.

## Step 3, part 1 done: LogUp in the uniform relation (2026-11-09)

Owner: "Start step 3". Step 3 needs real lookups. The comb's limb range checks are about 43k of P-recv's 71k
lookups. In stage A, the comb's table selections are only counted, never enforced (`pt_select` allocates fresh
limbs and bumps a counter).

**Mechanism** (`src/uniform.rs`):

- An array declares lookups per instance: (table id, tuple of linear combinations). The builder adds one
  second-phase helper cell and one row per lookup.
- A table is a values array: a constant column for fixed tables, or witness cells. The builder adds a companion
  array per table: multiplicities in phase 1, counted at build time; helpers in phase 2.
- One global sum row checks Σh − Σg = 0. It uses a new source, "every instance of a cell", whose matrix-MLE term
  is a y-only prefix sum.
- The layout has three regions: phase 1, phase 2, then constant columns in the trailing rows.
- The relation owns its rows per proof. Instantiation fills `h · (α − f) = 1` and `g · (α − t) = m`, with `f` and
  `t` compressing (table id + 1, tuple) by powers of β. The prover fills the helpers by batch inversion.
- The sparse expansion maps lookups and tables onto the stage-A assembly's LogUp.
- Test `logup_relation_matches_its_expansion_and_refuses`, using a partial family (6 of 8 slots), range lookups
  into a 2^11-entry constant column, and pair lookups into a 16-entry table:
  - every row holds;
  - the structured values match the expansion under random challenges;
  - honest proofs verify;
  - an out-of-range value (2048) is refused;
  - a pair that satisfies its own row but lies outside the table is refused.
- Cheating rows that exceed their width class now commit at full width instead of panicking.
- All 21 spike tests pass. The permutation and ladder numbers are unchanged.

**What this means for the non-native checks** (block census v1):

- A one-product `mul_check` is 9 rows with 370-465 nonzeros, 30 range lookups and about 130 cells. The
  four-product slope check is 24 rows with 690 nonzeros.
- At the current per-term and per-target costs, a template per call site would cost the verifier about 4k
  multiplications each. The comb has about ten such call sites per step, so per-call-site templates cannot fit.
- **Design for part 2:**
  - one generic check template with canonical bounds. Negative products always get offset encoding with fixed
    offsets, and table selections declare full 88-bit limbs, so no shape depends on data. Instances are every check
    of the comb: 2 signatures × 16 steps × the checks of a step;
  - within a comb step, each check's inputs are masked links to earlier checks' outputs (per-r maps), and the
    previous step's accumulator is read by shift;
  - range checks are lookups inside the block. Table selections are lookups into the fixed G tables (constant
    columns) and the wallet tables.
  - Expected verifier work for the whole comb: about 5-6k multiplications, against about 4k per call-site
    template.

## Step 3, part 2 done: the comb as uniform arrays (2026-11-09)

Owner: "Build it". New module `src/nonnative.rs`, mode `uniform-comb`, test `comb_rows_hold_prove_and_refuse`.

**Arrays.** One family with one instance per signature. Instances are indexed slot-major, `r = slot · 16 + step`.

- **Value**: each value a step allocates. That is 14 per step: the double-add's l1, xr, l2, xo, yo, then l, x3, y3
  for each of the three additions. Per instance: three 88-bit limbs, 24 chunks, 3 recomposition rows and 25 range
  lookups.
- **Selection**: 4 per step. Per instance: the index and the six limbs of the selected point, plus one lookup
  into a single merged constant table. That table holds the wallet key's and G's comb points (4 × 256 entries:
  table id, index, limbs).
- **Check 1** (14 per step) and **check 4** (4 per step): canonical check templates.
  - Bounds come from the template's worst case (each port's source count and coefficient), so no shape depends on
    data.
  - As in stage A's `mul_check`: five offset-encoded evaluation products per product; the offset quotient and three
    offset carries as 11-bit chunks (limbs are chunk combinations, with no limb cells); three carry rows and one
    native row.
  - One-product check: 56 cells, 9 rows, 55 lookups. Four-product check: 71 cells, 24 rows, 55 lookups.
  - The CRT bound holds with about 1.4 bits to spare (2^515.6 against 2^517).
- **Ports** are the check inputs. For each source position and kind there is one link, with per-slot coefficients.
  The kinds are: a value; a selected point's x; its y; or the previous step's final point, read by a shift. At
  step 0 the offset point's limbs are constants.
- **Steps**: every step is a double-add and three additions. The accumulator starts at a fixed offset point O, so
  the comb ends at 2^16 O + u1 G + u2 Q. The test checks this natively for both signatures.

**Checked:**

- every row holds;
- the structured matrix evaluation equals the expansion;
- the honest proof verifies;
- a changed slope is refused (both in the test and in the mode run).

**Not yet in the arrays.** These are per signature, not per step, and stage A has all of them. Each is a
handful of rho-0 instances.

- binding the selection indices to the scalars' digit bits;
- the scalar checks: ranges of r and s, r's inverse, u1 s = z and u2 s = r mod n;
- the final check: subtract 2^16 O, then x = r + flag · n.

**Verifier matrix evaluation.** These are generic changes in `evals`, so every A2 array benefits:

- **Two-field factorization.** It applies when a term's active r are a product of slots × steps, its map factors
  into a slot table and a step relation (the same step, the previous step, or one point), and its coefficient
  depends on the slot only. Such a term costs a slot sum times a step sum, and the step sum uses the existing
  closed forms.
- **Prefix sums** for constant coefficients on partial r ranges (r_count < 2^rho).
- **Closed forms only where they exist.** Links with constant coefficients and general maps used to fall into
  full enumeration inside the closed-form path.
- **Split terms.** An unstructured term becomes its most common coefficient plus a sparse remainder. The case
  here: constants with offset-point corrections at step 0.
- **Shared caches:**
  - factors are cached by the target's bit positions, not its cell, so every cell of an array shares them;
  - fixed-bit products of rows and blocks are memoized by prefix from the top bit;
  - single-point terms are memoized by bit prefix.
- **Per-block accumulation.** Term contributions are summed per target block, and the block's fixed bits and p
  sum are multiplied once.
- **Reporting.** `template_size` now reports one full evaluation with shared caches, instead of a sum of
  per-array evaluations.
- **Breakdown.** `ZKS_EVALS_BREAKDOWN=1` prints the cost by category.

| verifier matrix-MLE work (multiplications) | before | after |
|---|---:|---:|
| RP57 permutations (any count) | 6,396 | 1,809 |
| ladders | 5,935 | 1,598 |
| comb (any signature count) | 26,416* | 5,960 |

\* The first comb build: the two-field factorization only, none of the other changes.

All three together come to 9,367. The census allowed 8,000 for the whole matrix evaluation, and SHA rounds, binding
and vector algebra are still to come. Comb breakdown:

| item | multiplications |
|---|---:|
| one multiplication per template term (1,396 terms) | 1,262 |
| block finals | 851 |
| slot × step factors | 861 |
| prefix sums | 660 |
| fixed-bit tries (rows 361, blocks 648) | 1,009 |
| closed forms | 513 |
| LogUp sum row | 404 |
| rest | 400 |

**Size against stage A, per comb step:**

- **Uniform:** 1,608 rows: 222 check rows, 42 value recomposition rows and 1,344 lookup rows.
- **Cells:** 2,818, including one helper per lookup.
- **Lookups:** 1,344 range lookups, the same as stage A.
- **Sparse expansion** of the same rows for 2 signatures: 18,688 rows (10,240 of them pin the constant tables),
  43,008 lookups and 3,072 table entries.

**Mac proving, standalone** (`runs/zks-uniform-comb-v3*.log`):

| | uniform | sparse expansion |
|---|---|---|
| 2 signatures | 2^18 constraints, witness 16 rows × 16,384, 0.36-0.44 s | 2^17, 8 rows, 0.17-0.19 s |
| 4 signatures | 2^19, 32 rows, 0.39-0.44 s | 2^18, 16 rows, 0.21-0.22 s |

- The uniform layout pads both sizes to the next power of two. Cells match (106,560 against 106,566). Four things
  push it 1-2 rows over the boundary:
  - aligned 2^m blocks (only 224 of 256 slots per signature are used in the check-1 and value arrays);
  - 1,024-column stripes;
  - the two-row range table;
  - separate first-phase, second-phase and constant regions.
- The outer sumcheck costs about 1.7× the sparse form's per constraint, from template evaluation of 50-60-term
  rows.
- **Prover follow-ups:**
  - placement, so that constant tables and helper rows share the regions' slack;
  - per-template `abc` kernels.

  Both matter only for the full pair's layout, where the padding is paid once.

## Step 3 complete: the comb's per-signature checks (2026-11-09)

Owner: "Yes, finish the comb". The comb now verifies complete ECDSA signatures. The inputs are real P-256
signatures `(Q, z, r, s)` from `p256::sign`, and the comb checks everything stage A's `ecdsa_comb` checks.

**Digits array** (2 × 16 instances per signature: scalar × step):

- 16 boolean digit bits per instance: bit `t = 16 K + i` of u at step 15 − i, cell K.
- A Horner accumulator in three limbs: `acc = 2 acc(previous step) + Σ_K b_K 2^(16 K − 88 limb(K))`.
  - Limb 0 stays unnormalized below 2^96; the limbs recompose B exactly.
  - So `u = 2 B − (2^256 − 1) = 2 acc − (2^256 − 1)` is linear in the final step's limbs, which the check
    templates read as a port with |limb| < 2^97, 2^89, 2^81.
- The selection lookups now take their table index from 16 links into these bits, so the index cell is gone.
  The selections are bound to the scalars.

**Per-signature checks:**

- **Seven extra instances of the one-product check**, after the 14 × 16 step checks, in the same template:
  - `r rinv − 1`, `u1 s − z` and `u2 s − r`, all mod n;
  - the final addition `P = acc + C` with `C = −2^16 O`, mod p: slope 1, x3, `t2 = acc.x (acc.x + C.x)`, and
    `λ (acc.y + C.y) − t2 + 3 − C.x²`.
  - Because both slope equations are present, equal points double. So `P = u1 G + u2 Q` holds even when
    `acc = C`, and opposite points have no witness.
  - The modulus is a per-instance coefficient of the quotient and constant-column terms. The canonical bounds take
    the smaller modulus (n) for the quotient and check the CRT bound for both moduli.
- **Integer additions** (4 instances, each with 3 cells and 6 rows):
  - the checks: `r + (n−1−r) = n−1`, `s + (n−1−s) = n−1`, `x + (p−1−x) = p−1`, `r + flag·n = x`;
  - each holds over the integers through two boolean carries, and the flag is boolean;
  - stage A spends a full non-native check on each of these.
- **Values**: r, s, z, rinv, n−1−r, n−1−s, p−1−x, λ, x and t2 are value instances in slot 14 (range-checked like
  every value).

**Checks** (`comb_rows_hold_prove_and_refuse`):

- the native result `P = u1 G + u2 Q` with x(P) ≡ r;
- every row holds;
- the structured matrix evaluation equals the expansion;
- the honest proof verifies;
- refused: a changed slope, a flipped digit bit, a changed r limb, a changed x(P), a flipped flag.

All 23 spike tests pass.

**Size per signature** (`runs/zks-uniform-comb-v5.log`, 2 signatures):

| | uniform | stage A census (`comb m=16 ECDSA own key`) |
|---|---:|---:|
| rows other than lookups | 4,944 | 4,039 |
| range and table lookups | 22,608 | 23,975 |
| constraints with LogUp | 27,552 + 1,536 table share | 30,063 |

- The uniform form has more rows (+22%): value recomposition (702), the digit Horner rows, and the offset point's
  double-add at step 0 plus its removal.
- It has fewer lookups (−6%): the four integer range checks need no chunks.

**Verifier matrix work: 8,247 multiplications** for the whole comb (5,960 for the steps alone).

- Breakdown: 1,568 per-term, 1,071 point sums, 1,065 slot × step, 996 block finals, 698 + 411 fixed-bit tries,
  672 prefix sums, 598 closed forms, 439 LogUp sum row, 287 enumeration.
- New in `evals`: terms with at most 8 active r are summed point by point through the shared bit-prefix memos,
  unless a slot × step split has a closed-form step sum.
- Together with the permutations (1,812) and the ladders (1,598): 11.7k, against the census's 8,000.

**Mac proving at an equal shape** (2^18 for both forms at 2 signatures):

- Uniform 0.38-0.42 s against sparse 0.19-0.26 s. Footprint 156-168 MB against 260-314 MB.
- **The whole gap is two native kernels** (`native kernels` line):
  - the prover's matrix evaluation in "open" takes 0.14-0.16 s. `evals` re-derives the structure (term splitting,
    the split search, string-keyed caches) on every call, for what is about 8k multiplications;
  - the `abc` gather takes 0.06 s, against about 0.015 s for the sparse SpMV.

  `m_table` takes 0.003 s.
- **Follow-up:** compile the evaluation plan once per circuit (structure analysed once, values per call), and gather
  `abc` by target block. Both are needed before step 7's timings mean anything.
- **Still outside the comb:**
  - z's binding to the SHA-256 digest is a copy flow for step 6;
  - the wallet's tables are constant columns here, whereas in the protocol they are per-wallet committed tables.

## Kernels compiled: the matrix evaluation as a program, and the `abc` gather (2026-11-09)

Owner: "Yes, compiled plan". The uniform prover now matches the sparse prover at an equal shape, and the uniform
verifier is 2-3× faster than the sparse one.

**The matrix evaluation is compiled once per circuit into a straight-line program** over
`(r_x, r_y, alpha, beta)`:

- The structure analysis (term splitting, patterns, the split search, the bit-prefix caches) runs once,
  symbolically, and emits instructions. Each call then evaluates the program.
- The builder:
  - folds constants;
  - shares equal instructions, which also shares subexpressions across arrays;
  - simplifies `(p − q) + q` to `p` and folds products of products by constants;
  - drops instructions the outputs never read.
- The LogUp rows are written over their challenge factors (`alpha − c`, `−beta^e`). Their coefficients never
  depend on a challenge, so one program serves every proof.
  - The per-proof rows are now derived from these symbolic rows (one source of truth), so their terms line up with
    the compiled structure in every proof.
- Fewer products:
  - eq tables: one product per entry (`v·b`, `v − v·b`);
  - `(1−x)(1−y) = 1 − x − y + xy`: one product per bit;
  - x-only factors of constant coefficients cost nothing;
  - an r sum is kept as a slot times a constant, so terms of a row that share the slot share its product with
    the row factor (−10% on the comb).
- The program is identical on every build: ties for the most common coefficient go to the first seen.

| circuit | instructions | products of two slots | by a constant | compile | one evaluation |
|---|---:|---:|---:|---:|---:|
| comb, 2 signatures | 6,421 | 3,457 | 729 | 0.023 s | 66 µs (was 0.156 s) |
| comb, 4 signatures | 6,467 | 3,483 | 729 | 0.023 s | 73 µs |
| 512 permutations | 2,035 | 706 | 456 | 0.003 s | 24 µs |
| P-recv ladders | 1,269 | 768 | 43 | 0.002 s | 14 µs |

- **Products of two slots are exact**: one constraint each in an in-circuit verifier. Products by constants and
  sums are free in its linear combinations.
- The earlier figures (comb 8,247, permutations 1,812, ladders 1,598, total 11.7k) were estimates. They counted
  every multiplication, constants included, and two per table entry.
- **The three blocks now need 4,931 products** (706 + 768 + 3,457), against the census's 8,000. The comb's
  program grows by 26 products from 2 to 4 signatures (one more p bit).

**`abc` gathers one row block per job.**

- Each template row and matrix is a job that owns its block of the output: the blocks are split in row order and
  the jobs run in parallel.
- Per present instance, a job computes `Σ coef · z[p_base(p) + offset(r')]`. The offsets per r come from the
  symbolic rows, because a block position is additive over the p and r bits.
- So the loop does no source matching and no presence checks.
- `abc` went from 0.061 s to 0.009 s on the comb.
- The verifier no longer builds the prover's rows and readers: both are built on first use by the prover's kernels.

**Mac proving at an equal shape** (warm runs 2-3; the first proof of a process also builds the fixed-base table):

| circuit | uniform before | uniform now | sparse | verify, uniform | verify, sparse (with matrices) |
|---|---:|---:|---:|---:|---:|
| comb, 2 signatures (v7) | 0.38-0.42 s | 0.133-0.142 s | 0.141-0.142 s | 0.016-0.017 s | 0.034 s |
| comb, 4 signatures (v8) | 0.45-0.53 s | 0.170-0.172 s | 0.173-0.181 s | 0.014-0.015 s | 0.042 s |
| 512 permutations (v6) | | 0.140-0.157 s | 0.151-0.159 s | 0.012-0.013 s | 0.042 s |
| P-recv ladders (v9) | | 0.127-0.131 s | 0.123-0.127 s | 0.012-0.013 s | 0.027 s |

- Uniform "open" is now 0.061-0.069 s against sparse 0.073-0.087 s, which includes its matrix evaluation.
- Footprint 200-220 MB against 320-380 MB on the comb.
- The Mac was loaded (load average 28-40 from other work), but both forms ran back to back in each process.
- Logs:
  - `runs/zks-uniform-comb-v6-plan.log` (program only);
  - `v7-gather` (both kernels);
  - `v8-shared` and `v8-shared-4sigs` (shared slots);
  - `runs/zks-uniform-perm-v6-plan.log` and `runs/zks-uniform-ladders-v9-plan.log`.
- All 7 uniform, ladder and comb tests pass:
  - the program equals the expanded matrices for the permutations, LogUp with random challenges, the ladders and
    the comb;
  - proofs through it verify, and tampered witnesses are refused.

**What this means for the phone (expected, not yet measured):**

- On the Pixel today, the real Receive pair spends 0.27 s per pair in the sparse matrix evaluation (P 0.18 s,
  S 0.10 s).
- In the uniform layout with a compiled program, that is a sub-millisecond pass over a few thousand
  instructions.
- Step 7 has to show it on the full pair. Compiling once per circuit can happen with the key setup, off the
  payment path.

## Step 4 done: SHA-256 rounds and the binding chain (2026-11-09)

Owner: "Start step 4".

- Code: `src/sha.rs` (SHA-256) and `binding_chain` in `src/nonnative.rs`.
- Runs:
  - `ZKS_REPEAT=3 zk-spartan uniform-sha`, with `ZKS_SHA_BLOCKS` (default 2, P-recv's two own-key signatures);
  - `zk-spartan uniform-binding`, with `ZKS_BINDING_VALUES` (default 258, P-recv's shared values).
- Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-sha-v2.log` and `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-binding-v2.log`.

**SHA-256 as uniform arrays.** There is one block per signed message (the 32-byte encoding of a field element,
then the padding), and the family's p is the block.

- **Message** (8 words): each word is decomposed at [3, 7, 8, 16, 18, 24], so its bytes and its σ0 pieces are
  linear in its cells. A running sum gives the encoded element `m = Σ_j 2^(32 j) bswap(w_j)`.
- **Schedule** (48 steps):
  - each σ split reads the pieces of w[t−15] or w[t−2]: message words, padding constants or earlier
    schedule outputs, through links with partial maps;
  - the result is decomposed at [3, 7, 10, 17, 18, 19] for the later σs.
- **Rounds** (64):
  - round t decomposes its inputs a = A_{t−1} and e = E_{t−1} (the IV words at t = 0);
  - Σ0, Σ1, Ch (two odd halves) and Maj are spread splits;
  - the b, c, f, g spreads and the dense d and h are links to rounds t−1..t−4 (IV words before round 0);
  - K_t is a per-round constant;
  - only A and E are materialized per round.
- **Output** (8 words): state + IV as 32 boolean bits and a carry.
- One spread table of (d, spread(d)), d < 2^11, as in stage A's lever 5. Narrower pieces are range-checked by
  the shifted lookup.

| per block | uniform | stage A (lever 5) |
|---|---:|---:|
| rows other than lookups | 2,384 | 2,709 |
| lookups | 3,568 | 3,392 |
| constraints with LogUp | 5,952 | 6,101 |

- There are fewer rows (−12%), because a round materializes only A and E.
- There are more lookups (+5%), because uniformity decomposes round 0's IV words and the padding words' σ inputs.
- The message bytes and m's running sum replace stage A's 255 canonical bits. Three pieces are left outside:
  - m < p stays a counted comparison, as in stage A;
  - binding m to the Poseidon digest is a copy flow for step 6;
  - so is binding the digest bits to the comb's z.

**The binding chain** (Horner over Fq in the Fp circuit, from the mask μ):

- **Binding value:** acc_0 = μ, …, acc_n, then ρ. Each is 3 limbs (88, 88, 79 bits) with range-checked 11-bit
  chunks.
- **Binding check:** the comb's canonical one-product check, `acc_r ρ + v_r − acc_{r+1} = 0 (mod q)`.
  - v_r < 2^128 sits in limb 0 of the linear part. It is a cell, which step 6 binds to its source with a copy
    lookup.
  - ρ is read by broadcast, and acc_{r+1} by the next-instance link.
- Per value: 12.0 rows plus 80.2 lookups = 92.2 constraints with LogUp, against stage A's 92.4 at chunk 11 (83 at
  chunk 13). R equals the native Horner chain.

**Closed forms over intervals** (`r_closed`):

- Every pattern now sums over any interval [lo, hi) of r: one, own, identity, broadcast, and shifts to r − 1 and
  r + 1, with constant or geometric coefficients.
- An interval sum is a difference of prefix sums over r's bits. A shift decomposes by the lowest set (or zero)
  bit, with a prefix sum over the high bits plus the one exact point at the boundary.
- Why: the chain's 258 instances fill 258 of 512 slots, and the full-range closed forms left its three links on
  512-entry tables (2,995 products).
- Now: binding 1,401, comb 3,457 → 3,390, SHA 2,586 → 2,492.
- The separate prefix-sum path is subsumed and deleted.

**A layout bug, found and fixed:**

- The second-phase region started right after the first-phase rows. A 2,048-entry helper block spans two witness
  rows, so after an odd number of first-phase rows it was misaligned.
- The prover's rows held, but the verifier's matrix evaluation, which factors on aligned blocks, disagreed.
- The earlier circuits had even first-phase row counts by luck, and their measured proofs verified. The SHA test
  caught it.
- Each region now starts on a multiple of its tallest block, and `build` asserts that every block is aligned.

**Tests** (27 pass):

- `sha_rows_hold_prove_and_refuse`:
  - the digest equals native SHA-256 of the encodings;
  - every row holds, the evaluation equals the expansion, and the honest proof verifies;
  - refused: a changed round output, schedule word, message piece, running sum, digest bit, or Maj chunk.
- `binding_chain_holds_proves_and_refuses`:
  - R equals native Horner;
  - refused: a changed accumulator limb, shared value, or ρ chunk.
- `closed_forms_match_enumeration`: over 150 cases (ρ = 1-5, random intervals, constant and geometric coefficients,
  all six link shapes) against brute force.

**Mac at the standalone sizes** (warm runs 2-3; load average 94-108 from other work; both forms back to back):

| circuit | uniform | sparse |
|---|---|---|
| SHA, 2 blocks | 2^17; core 0.102-0.113 s; verify 0.014-0.015 s | 2^16; 0.089-0.091 s; 0.018 s |
| binding, 258 values | 2^18; core 0.114-0.148 s; verify 0.014-0.015 s | 2^16; 0.104-0.119 s; 0.020-0.021 s |

Standalone, the uniform layout pads:

- 258 instances take 512 slots;
- regions round up to their tallest blocks;
- so these circuits land one or two powers of two above the sparse form.

In the full pair they share rows and tables with everything else. Per-array padding of partial instance counts
stays a cost to manage in step 7.

**The verifier budget: over, and lookups are half of it.**

| block | products of two slots | of which lookups (measured: compiled with and without) |
|---|---:|---:|
| 512 permutations | 706 | 0 |
| P-recv ladders | 776 | 0 |
| comb, 2 signatures | 3,390 | 1,718 |
| SHA-256 | 2,492 | 1,005 |
| binding chain | 1,401 | 939 |
| **total** | **8,765** (budget 8,000) | **3,662** |

- Steps 5 and 6 still add the IPA vector algebra and the copy lookups.
- **Why lookups cost this much:** each lookup is its own template row with its own helper block. Each costs about
  12-15 products: the row's fixed bits, the helper block's fixed bits and final product, its terms, and its
  sum-row term.
- **Lever 1, lookup arrays:**
  - put the looked-up chunks of an array in a chunk array indexed by (instance, chunk), so a table costs one lookup
    template row per array instead of one per chunk;
  - the recompositions then read the chunks through split maps.
  - This would remove most of the 3,662.
- **Lever 2, shared check types:**
  - the binding's 258 checks are one-product non-native checks like the comb's;
  - as instances of the comb's check type (modulus q beside p and n, v as a value instance), they would add
    instances, not a template;
  - that saves most of the binding's remaining 462.
- Both levers are estimates. The first is a layout change in `build` and in the gadgets' chunk cells.

## Lookup arrays done: the verifier back under budget (2026-11-09)

Owner: "Yes, lookup arrays". The verifier's matrix evaluation for the five blocks went from 8,765 to 5,446
products, against the 8,000 budget. The uniform prover stays at parity with the sparse one or faster (except the
padded binding chain), and it verifies 1.5-3.5× faster. All 27 spike tests pass.

| block | before | after | of which the lookup groups before |
|---|---:|---:|---:|
| 512 permutations | 706 | 642 | 0 |
| P-recv ladders | 776 | 706 | 0 |
| comb, 2 signatures | 3,390 | 2,170 | 1,718 |
| SHA-256, 2 blocks | 2,492 | 1,451 | 1,005 |
| binding chain, 258 values | 1,401 | 477 | 939 |
| **total** | **8,765** | **5,446** | 3,662 |

That leaves about 2,550 products for steps 5 and 6.

**Chunk arrays, built generically in `build`:**

- **Grouping:** an array's lookups into one table, whose tuples are single cells with unit coefficients, form a
  group (range chunks; SHA's (dense, spread) pairs).
- **The chunk array:** the group's cells become the cells of one chunk array. Its instance `j 2^ρ + r` is lookup
  j at instance r, so the group costs one lookup row, and each cell one block, instead of one per chunk.
- **References:** references to moved cells become links shifted to chunk j. A narrow chunk's shifted range check,
  which reads its cell through another coefficient, stays with its array as a lookup through that link.
- **Stable numbering:** cell numbers do not change, and `cell_index` redirects a moved cell to its chunk array. The
  gadgets and tests are untouched.
- **No padding instances:** an instance r of a chunk array is present when `r mod 2^σ < inner` and
  `r >> σ < outer`. This two-field presence runs through every kernel: the matrix evaluation (coefficients masked
  by presence), `abc`, `m_table`, the LogUp multiplicities, the sum row and the sparse export.
- Per circuit: comb 3 chunk arrays (value 24 chunks, check 1 53, check 4 51), SHA 3, binding 2.

**What it took in the compiler:**

- **Constant target bits factored out:**
  - when all active targets of a link share their high bits (a chunk index), those bits are one point product,
    memoized by prefix;
  - the rest of the term sees the low bits only, so every chunk of an array shares one closed form.
- **Grouped products:** in a row, the terms with the same block and r sum add their chunk points with constants
  (free in a circuit) and are multiplied by the row factor once, instead of once per chunk.
- **Shared bit products:** zero bits are `p − p·x` instead of `p·(1 − x)`, so both values of a bit share one
  product. This applies to the fixed-bit tries and point products everywhere, and also helps the permutations
  (−64) and the ladders (−70).
- **Closed forms on the active set:** a closed form sums the active r, which must be an interval. Zero
  coefficients outside it no longer block it. Cache keys include the interval.

**Layout:**

- **Wide blocks:** a block larger than a stripe now spans 2^g aligned stripes by 2^(m−c−g) rows (the earliest-ending
  choice), instead of one stripe by 2^(m−c) rows.
- **Why:** the first chunk version put a 32k-slot chunk block 32 rows tall into one stripe, and the regions
  rounded to it, so the comb's witness grew from 16 to 128 rows. With wide blocks it is back to 16 rows.
- **Bonus:** SHA's 2,048-entry table blocks no longer force tall regions, so the 2-block SHA circuit drops from
  2^17 to 2^16, the sparse form's size.

**Prover fixes found along the way:**

- **`m_table` cell jobs:** `m_table` ran a job for moved cells, whose unused block entries write zeros at the start
  of the witness. It was harmless only because the blocks placed there belonged to arrays written later. Moved
  cells are now skipped.
- **`m_table` parallelism:** large blocks are split into instance ranges, so one 2^15-instance chunk block is no
  longer a single sequential job.
- **Readers:** they are built once per circuit from the symbolic rows (0.017 s for the comb), not per proof. They
  are indexed by target instance in one offsets-plus-pairs table, because a chunk block has one reader per chunk
  and only one maps to each instance.
- `m_table` per proof: comb 0.004 → 0.0024 s, binding 0.0037 → 0.0015 s, SHA 0.0017 → 0.0008 s.
- The comb, SHA and binding tests now also check `M(r_x, ·)` against the expansion.

**Mac** (warm runs 2-3; load average 69-91; both forms back to back; logs `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-*-chunks-v3.log`):

| circuit | size | uniform prove | sparse prove | uniform verify | sparse verify (with matrices) |
|---|---|---:|---:|---:|---:|
| 512 permutations | 2^18 / 2^18 | 0.152-0.155 s | 0.170-0.185 s | 0.013-0.014 s | 0.049 s |
| P-recv ladders | 2^17 / 2^17 | 0.115-0.158 s | 0.125-0.130 s | 0.014 s | 0.026-0.031 s |
| comb, 2 signatures | 2^18 / 2^18 | 0.140 s | 0.155-0.161 s | 0.016 s | 0.033-0.035 s |
| SHA, 2 blocks | 2^16 / 2^16 | 0.091-0.092 s | 0.095-0.098 s | 0.014 s | 0.021 s |
| binding, 258 values | 2^18 / 2^16 | 0.130-0.133 s | 0.111-0.118 s | 0.014-0.015 s | 0.022-0.024 s |

**The binding chain is still padded.**

- Its 258 instances take 512 slots, and its chunk arrays round 25 and 55 chunks up to 32 and 64 slots. So it lands
  at 2^18 against the sparse 2^16, and proves 10-20% slower.
- In the full pair it shares rows with everything else, but half its check slots stay empty.
- **Options:**
  - make the binding checks instances of the comb's check type (lever 2 above), which shares the comb's slots;
  - or lay the chain out so its count fills a power of two.

## Step 5 done: k = 3 slots and the scalar side as uniform arrays (2026-11-10)

Owner: "Yes, k = 3 base-4". Code: `src/grid.rs` and `src/spartan.rs` (slots and the closed form), `src/verifier.rs`
(scalar side), `src/census.rs`, `src/scalar.rs` (the uniform arrays). Runs: `ZKS_SCHEDULE=lean zk-spartan verifier`,
`... real-receive` and `... uniform-scalar`. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-verifier-lean-k3-v3.log`,
`../../output/kagemusha-100x-20261108/spike/runs/zks-real-receive-k3-v2.log`, `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-scalar-v2.log`. All 28 spike tests pass.

**The protocol change.**

- Every committed round has 3 variables (the last round of a sumcheck may have fewer) and owns an aligned
  64-column slot in the IPA vector.
- Grid point (d0, d1, d2) sits at slot offset 16 d0 + 4 d1 + d2. Each variable owns two column bits; digit value 3
  stays empty. A shorter round uses the low digits, with its leading digits 0.
- The IPA's ⟨s(u), b⟩ over a slot then factors per digit:
  - s(u) on the slot is s(u) over the slot's high bits times, per digit, 1, u_lo, u_hi for the values 0, 1, 2;
  - the Boolean factor is (1 − τ) + u_lo τ for an outer round and 1 + u_lo for an inner one;
  - the Lagrange factor is L0(r) + u_lo L1(r) + u_hi L2(r);
  - a missing digit's factor is 1.
- The head (−coefficient · eq(r_col, ·) on the active columns) is the whole eq-weighted s(u) sum minus its tail
  part: one pass over the column bits, a suffix recursion against the bits of the active-column count.
- So the verifier builds no vector: no s(u) over the tail block, no tail weights, no grid-weight tables. Native form:
  `spartan::b_final_slots`, asserted equal to the IPA's own fold on every verification.

**A decision taken while building: short rounds get a full slot.**

- The plan was 788 tail columns: P-recv's two short rounds (1 and 2 variables) in a 4-column and a 16-column slot,
  packed into one 64-column block.
- Packed that way, their leading digits are fixed at 2 and 3. Their s(u) factors are then u_hi and u_hi u_lo, and the
  uniform template needs per-slot exceptions for them.
- With a full 64-column slot each, the leading digits are 0 (factor 1). Every slot has the same template, and the
  fixed-digit code disappears from both verifiers.
- Cost: 896 tail columns instead of 788. P-recv has 15,488 active columns instead of 15,596 (0.7% of a row), and its
  witness still fits 23 + 9 rows. Proof sizes are unchanged.

**Measured** (lean schedule, the receive pair):

| | k = 4 (Nov 9) | k = 3, 64-column slots |
|---|---:|---:|
| P-recv scalar side (verifier synthesis) | 23,263 rows | 13,308 |
| of which closed form / relation | 8,705 / 2,068 | 738 / 84 |
| P-recv points side | 69,402 | 72,845 (4 more round commitments) |
| P-recv used constraints (real receive) | 368,601 | 355,637 |
| P-recv witness, first phase | 366,242 | 353,240 |
| S-recv used constraints | 195,283 | 185,751 |
| proofs | 2,464 + 2,400 B | 2,592 + 2,496 B |
| Pixel model, pair | 1.56 + 0.88 s | 1.52 + 0.85 s |

- The census formulas (`relation_rows`, `closed_form_rows`) follow the synthesized structure. S-proofs match within 3
  rows. P-proofs differ by the C_P range-check allowance that the synthesis leaves out (+520).
- The tamper suite passes for k = 1, 2 and 3. With uniform slots, k = 4 and 5 no longer fit its 1,024-column test
  shape.

**The scalar side as uniform arrays** (`src/scalar.rs`; one P-recv proof with two inherited claims):

| array | instances | template rows | terms | contents |
|---|---:|---:|---:|---|
| slot data | 16 | 21 | 76 | challenge powers and squares, τ's squaring chain, per-digit Boolean weights, weight factors (an α/γ chain), io prefix |
| bits | 14 × 4 | 7 | 32 | r_col, running eq product, tail-sum recursion, s(u)'s shared high bits |
| slot terms | 16 × 4 | 21 | 90 | per digit the Boolean and Lagrange factors against s(u)'s two bits, s(u)'s varying high bits, the term, a running sum |
| glue | 1 | 25 | 86 | γ⁻¹, kappa, coefficient, kappa', Schnorr scalars |
| openings | 4 | 5 | 19 | each opening's value and its weight ξ^q |
| alpha powers | 16 | 1 | 4 | the α^e chain |
| io fold | 128 | 1 | 10 | the io MLE as a heap-ordered fold |

- **Indexing.** Slots are indexed by the low bits of their 64-column block, so s(u)'s varying high bits are bits of
  the instance index. Openings sit in an instance's low bits (r = 4 slot + q; the fourth is padding).
- **Rows.** The non-uniform sections this covers (challenge powers, relation, closed form, io eval, Schnorr scalars)
  are 1,090 rows. The arrays use 2,261 template-row instances: a padded fourth opening, idle slot instances, and rows
  computed everywhere.
- **Verifier matrix evaluation: 968 products.**
- **Checked:**
  - every row holds;
  - b_final, both claim values, kappa, kappa', the coefficient and the three Schnorr scalars equal the native
    verifier's;
  - each array's compiled evaluation equals its expansion, and M(r_x, ·) does too;
  - the uniform proof verifies, and five tampered cells are refused (a weight factor, a tail-sum step, a slot term, an
    io fold value, b_final);
  - the sparse assembly of the same rows proves and verifies.
- **Outside the arrays:** the challenge values (endoscalars from bits, 4,370 rows), the io digest (permutation
  family) and the matrix MLE itself.
- **Inputs** (challenges, proof scalars, io, the claims' u, the matrix MLE's values) are cells; step 6 wires them.
  The bits array reads r_col from the slot data through a stride-3 map (enumerated, about 60 products); as step-6
  copies it would cost lookup rows instead.

**Compiler fixes found on the way.**

- **Closed-form cache key.**
  - The compiled evaluation cached interval closed forms by the first and last active instance. Two terms with the
    same ends but different gaps shared an entry, so the scalar side's B evaluation came out wrong. The per-array
    check caught it.
  - The cache is keyed by the whole active set now.
  - The old key had also cost the comb a closed form: 2,170 → 1,957 products (A/B on the same layout); 1,963 after
    the layout change.
- **Sparse export of sum terms.** `to_assembly` dropped every `Src::All` term, which is right only for the LogUp sum
  row. An ordinary row's sum term is expanded now.
- **New debugging helpers:** `evals_of` (one array's compiled evaluation) and `entries_of`.
- **Tried and dropped:**
  - splitting a term into runs (comb +156 products);
  - a closed form for one-bit selectors (no gain; the split path already handled those terms);
  - the openings in the glue (+29) and α powers by squaring in the glue (+16).

**Budget now:** permutations 642 + ladders 706 + comb 1,963 + SHA 1,451 + binding 477 + scalar side 968 = **6,207**
of 8,000. That leaves about 1,790 for step 6's copy lookups, the challenge values, and whatever step 7's full pair
finds outside these six blocks.

## Step 6 done: the copy argument and the wiring (2026-11-10)

Owner: "Start step 6". Code: `src/uniform.rs` (`Copies`, `Export`, `Import`, `build_with_copies`, `relocate`),
`src/wired.rs` (the wired circuits), `src/nonnative.rs` (the binding chain's shared values), mode `uniform-wired`. Run:
`ZKS_SCHEDULE=lean ZKS_REPEAT=3 zk-spartan uniform-wired`. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-wired-v3.log`,
`../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-binding-shared-v1.log`. All 31 spike tests pass.

**The copy argument.**

- An export site is an array and a tuple of linear combinations. Every present instance is an entry of the copy table,
  with its own address. An import site is an array, a tuple, and per instance the entry it reads, or none (decided per
  r).
- The addresses are preprocessed index columns: one constant array per site array, one column per site, committed in
  the verification key. A read names its entry, so the prover never chooses an address.
- It runs on the existing LogUp:
  - a read is a lookup of (address, values) under table id `COPY_TABLE + 1 + k`;
  - an export gets an entry array: a multiplicity (phase 1, counted at build time) and a helper (phase 2), with the
    row `g (alpha - t) = m`;
  - one sum row covers lookups, tables and entries;
  - a read that is off has right-hand side 0, which forces its helper to 0.
- Addresses are unique, so a read's values must equal its entry's. The builder asserts this on the honest witness.
- The prover kernels, the compiled evaluation, M(r_x, ·) and the expansion see copies as ordinary template rows. The
  sparse export wires them directly, with one equality row per copied value.
- **Copies or links.**
  - Links stay inside a family or read a global array. They cost the prover nothing. The verifier pays per term: about
    a product per point on short maps, an enumeration on long irregular ones.
  - A copy site costs the verifier a fixed amount whatever its length. The prover pays one LogUp row and helper per
    read and per entry.
  - So: links inside a family and from global arrays, copies between families with p ranges (cross-p flows).
- Test `copies_match_expansion_prove_and_refuse`: two families, a 2-tuple and a 1-tuple, one entry read twice, one
  never, one read off.
  - Rows hold; the evaluation and M(r_x, ·) equal the expansion; the honest proof verifies; the sparse form proves.
  - Refused: a read changed with its own row kept, an entry changed with its own row kept, a multiplicity, an index
    column cell, two reads swapped.
  - Accepted: the off read's cells changed, with the entry that is read elsewhere following.

**The own relation's signed messages** (`wired::own_chain`, test `own_chain_wires_digest_sha_and_comb`).

- Each message is a Poseidon digest. The RP57 family exports each permutation's output lane 0 from its input array,
  through links into the last full round, so the export adds no cells. SHA-256's message array reads it as m, by one
  copy at the last word.
- SHA-256 and the comb share the signature family, so z goes by links.
  - SHA-256's output words get lo16, lo24, the word, and three running sums over the words: z's 88-bit limbs. Words 5
    and 2 straddle limb boundaries.
  - Each sum's last step is the comb's z limb.
- The signatures are real P-256 ECDSA over the SHA-256 digests of the digests' encodings.
- Refused:
  - a signed digest's permutation recomputed from another input (every round row holds, but SHA-256 hashed the old
    digest);
  - a running limb sum;
  - the read's index column.
- Accepted: an unread permutation recomputed.

**One proof's scalar side bound to the binding chain** (`wired::scalar_wired`, test
`scalar_side_wired_to_the_binding_chain`, run under both schedules).

- The binding chain holds every shared value of the proof. A P-recv proof has 66: its challenges, nine full-width
  scalars as two limbs each, and the inherited claims' points.
- Raw challenges are read as they are.
- Endo challenges go through the **endo value array**.
  - The raw value is thirteen 10-bit chunks. Each is looked up with its share of the endoscalar in one fixed table of
    1,024 entries, so value = 2^64 (ζ + 1) + Σ 2^(5j) share_j, plus a constant for the last chunk's missing pairs.
  - The chunks recompose the shared value itself.
  - Stage A's endo_value is 190 rows per challenge (126 booleans, 63 products). This is 2 rows and 13 lookups.
- Full-width values go through **limb splits**: value = lo + 2^128 hi, both read from shared values.
- Every array is global, so the 45 bound inputs are links:
  - the round challenges and τ into the slot data;
  - the IPA's u into the bits array;
  - α, γ, ξ, ζ, r_ABC, v_A, v_B and v_C into the glue;
  - κ', κ'ζ, c(z1) and z1 b_final ζ, which the glue computes, bound the same way.
- Every output still equals the native verifier's.
- Refused: an endo value re-derived from other bits (its rows and lookups hold), a limb split off its shared limbs, a
  changed challenge input.
- Still unwired:
  - the io values and the inherited claims' u (the IVC statement);
  - the matrix MLE's values (the in-circuit evaluation, step 7);
  - the io digest (a sponge in the permutation family);
  - z2 and the claims' points, which only the points side uses.

**A soundness gap closed in the binding chain (step 4).**

- The non-native check `acc rho + v - acc' = 0 (mod q)` takes its CRT bound from v < 2^128. But v was a cell with a
  128-bit width class, which is a commitment hint, not a constraint, so a prover could use a larger v.
- v now sits in its own array with 128-bit range chunks (12 lookups), and the check reads it by a link. A value
  re-chunked as v + 1 is refused.
- Cost: 477 → 543 products, and 92.2 → 106.2 constraints per shared value.

**Measured** (lean schedule; Mac, load 14-23, warm runs 2-3):

| circuit | verifier products | same blocks unwired | constraint rows | prove, uniform | prove, sparse | verify, uniform / sparse |
|---|---:|---:|---:|---:|---:|---:|
| own chain: 512 permutations, 2 digests signed, SHA-256, comb | 3,942 | 3,802 (+140) | 210,991 | 0.219-0.223 s (2^19) | 0.225-0.226 s (2^19) | 0.017 s / 0.04 s + 0.03 s matrices |
| one P-recv proof's scalar side bound to the chain | 1,744 | 1,386 (+358) | 12,856 | 0.079-0.080 s (2^17) | 0.099-0.108 s (2^16) | 0.010 s / 0.017 s |
| both, with P-recv's ladders, in one circuit | **5,936** | | 318,277 | 0.282-0.295 s (2^20) | 0.271-0.282 s (2^19) | 0.020 s / 0.04 s + 0.04 s matrices |

- The own chain's +140 is one export site (512 entries), one import site, and z's limb sums. The same blocks standalone
  are 642 + 1,451 + 1,963 = 4,056; in one circuit, 3,802, because the program shares factors across blocks.
- The scalar side's +358 is the endo value array (its chunk lookups and table), the limb splits, and 21 wiring rows.
  Folding the raw cell and the limb cells into their link rows took it from +378.
- **Budget: 5,936 of 8,000** for every block built so far, wired, in one circuit. Step 5 reported 6,207, but that was
  a sum of standalone blocks. That leaves about 2,060 for step 7:
  - the points side: sponge absorption and squeezes, challenge decompositions, point additions, on-curve checks, and
    the ladders' scalars and bases. These cross families, so they are copies.
  - the own relation's small blocks: Merkle path swaps, comparisons, u128 checks, byte encodings, logic;
  - the second proof's scalar side (the same templates, as a p range of 2);
  - the io digest;
  - the matrix MLE program, as a program array whose operands are copies.
- **Layout is now the prover's problem.** The one-circuit composite is 2^20 in the uniform layout against 2^19 sparse
  (64 witness rows against 22 + 10). Causes:
  - every block is a power of two per array (the binding check's 55 chunks pad to 64 per value, and 66 values pad to
    128);
  - the binding's range table duplicates the comb's (only one serves as the table);
  - each region rounds up to its tallest block.

  The proof grows with the shape (3,552 against 2,592 B). Step 7 has to pack the full pair into the sparse form's
  size.

## Step 7 done: the full Receive pair in the uniform layout (2026-11-10)

Owner: "Start step 7". Code: `src/full.rs` (P-recv, S-recv, the own relation, the permutation pool, io digests, the
filler), `src/points.rs` (the points side), `src/uniform.rs` (constraint-row packing, the witness placer), mode
`uniform-full`. Run: `ZKS_SCHEDULE=lean ZKS_REPEAT=3 zk-spartan uniform-full` (`ZKS_PLACE=1` dumps the placement).
Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-full-v8.log`. All 34 spike tests pass.

**What the pair holds.**

- **P-recv** (over Fp; the incoming P proofs are over Vesta):
  - the own relation;
  - per incoming pair, the P proof's scalar side with its io digest, and the S proof's points side;
  - the non-native binding chain over all 263 shared values;
  - the matrix-MLE filler.
- **S-recv** (over Fq):
  - per pair, the P proof's points side, and the S proof's scalar side with its io digest;
  - the native binding chain (acc' = acc ρ + v);
  - the filler.
- **One permutation pool per circuit.** Every RP57 permutation is an instance of one family.
  - P-recv has 582: the statement sponge's 102, the two insertions' 268, both S transcripts and both io digests.
    S-recv has 230.
  - The input array carries seven extra cells (a, b, bit, sib, s, j1, j2), which serve the reads:
    - chain 3-tuples, for absorbed words;
    - single lanes, for transcript words;
    - Merkle nodes: the swap s = bit (sib − in0 + s), with the node domain held by a fixed selector;
    - one output export.
- **The points side.**
  - Squeezes: `c1 + 2^126 c2 + 2^252 c3` for values, and `lo + 2^128 hi` for full words, which are canonical by
    `s0 + d = p − 1` over limbs with boolean carries.
  - On-curve checks, complete additions, endoscale ladders and variable-base ladders.
  - Transcript words are read per lane. Every flow across families is a copy.
- **The own relation.**
  - The statement sponge, and two indexed-tree insertions (four depth-32 paths and six leaf hashes each), with the root
    equality.
  - Two own-key signatures (SHA-256 and the comb).
  - 4 comparisons, 10 u128 checks, 5 byte encodings and 1,000 logic triples.

**Two soundness gaps closed.** Both are also in stage A's recorded circuit.

- **Comparisons.**
  - Stage A checks a, b, b − a − 1 and the canonicity witnesses p − 1 − a, p − 1 − b only mod p, each as a 255-bit
    chunk set. A 255-bit set holds a representative of every residue, so the gadget accepts every pair, a = p − 1 and
    b = 0 included.
  - Now a, b, d = b − a − 1 and c = p − 1 − b are each three limbs (88, 88 and 79 bits) of 11-bit chunks, with
    a + d + 1 = b and b + c = p − 1 enforced limb by limb over boolean carries.
  - Every limb sum stays far below p, so the identities hold over the integers. That gives b ≤ p − 1 and a < b.
  - Cost: 96 chunks instead of 120, and 12 rows instead of 5.
  - Refused: a changed carry, and a changed operand with its limbs kept.
- **Byte encodings.**
  - x = Σ 256^k byte_k holding only mod p admits a second encoding, x + p, for every x < 2^256 − p. Stage A's cost
    model counts a canonicity check that its recorded circuit leaves out.
  - Now X ≤ p − 1, by the same limb identity with c = p − 1 − X (24 chunks and 2 carries per element).
  - Refused: a changed carry, and a changed chunk of c.

**The layout.**

- **Constraint rows.** Blocks go largest first, each at the lowest aligned base where its present rows meet no placed
  present row, so a block's absent rows host other blocks. The constraint space no longer follows the witness:
  P-recv's 402,075 rows fit in 2^19.
- **Absent rows in the witness.** A block marks only the rows that hold present instances, and its absent rows hold
  other blocks. That is safe because:
  - no row reads an absent row with a nonzero coefficient;
  - every link reads a present instance;
  - the LogUp sum runs over present instances only.

  The witness fill and M(r_x, ·) skip absent instances.
- **Width bands.** A row commits at its widest cell, so first-phase cells go on bands of rows by width class (1, 11, 20,
  88, 180 and 255 bits). A class smaller than one row joins the next wider class. Constants take the trailing rows. The
  row budget is the smallest that fits, found by bisection.
- **Placement key.** The placement that ends earliest wins.
  - Taking the fewest wasted cells first fragments the rows: P-recv grew to 128 rows.
  - Tagging rows by class instead of using bands lets one tall, one-stripe block claim a band of rows.
  - 128-column stripes give the same rows as 1,024-column ones, with more verifier products for S-recv (5,702 against
    5,551).
- **Result.**
  - P-recv uses 40 + 10 + 3 of its 64 rows. It had 11 second-phase rows before; the earlier report's 21 counted the
    empty rows after them. S-recv uses 37 + 3 + 2.
  - The commit model fell from 1.00 to 0.81 s for P-recv and from 0.49 to 0.45 s for S-recv. All of it comes from the
    width bands.

**Why the pair does not pack to the sparse form's size.**

- **P-recv.** It has 519,741 cells. 32 rows of 16,384 columns leave 491,520 positions, because each row keeps 896
  columns for the sumcheck tail and 2 for masking, and the stripes take whole 1,024s.
- **S-recv.** Its 227,472 cells would need 97% of its 32-row capacity.
- Both witnesses stay at 64 rows. In the model, the inner sumcheck pays 0.21 s (P-recv) and 0.11 s (S-recv) for that.
- **Packing alone cannot reach 2^19; fewer cells can.** Where P-recv's cells are:
  - the permutation pool, about 175k: about 300 cells per permutation against stage A's 243 rows (block boundary
    states, read cells, LogUp helpers and entries, index cells);
  - range chunks with their helpers, about 160k;
  - the endoscale steps, 82k;
  - the filler, 20.7k.
- **The growth over stage A is in the circuit, not the layout.** The sparse expansion of the same rows is also 2^20 with
  64 rows (416,174 + 88,386 cells in 27 + 37 rows). The growth comes from:
  - the copies;
  - the sound gadgets;
  - the real points side;
  - the filler: 20,694 rows instead of 8,000.

**Measured** (lean schedule; Mac warm runs 2-3; Pixel model on the lane-commit table, with the width bands):

| | P-recv | S-recv |
|---|---:|---:|
| constraint rows | 402,075 (2^19) | 208,101 (2^18) |
| witness cells | 519,741 | 227,472 |
| witness rows (of 16,384 / 8,192 columns) | 40 + 10 + 3 of 64 | 37 + 3 + 2 of 64 |
| verifier matrix-MLE products | 10,345 | 5,568 |
| Pixel model | **1.75 s** | **0.97 s** |
| outer / inner / commit / density / IPA | 0.14 / 0.42 / 0.81 / 0.04 / 0.34 | 0.07 / 0.21 / 0.45 / 0.01 / 0.23 |
| Mac prove (core) | 0.199-0.208 s | 0.116-0.138 s |
| Mac verify | 0.021 s | 0.013-0.014 s |
| proof | 3,520 B | 3,456 B |
| stage A, same role | 355,637 rows (2^19), 23 + 9 rows; model 1.52 s + matrices 0.72 s + relation 0.18 s | 185,751 (2^18), 26 + 6 rows; 0.85 + 0.33 + 0.10 s |

- **Per Receive pair** the model gives **2.72 s**, against 3.70 s for stage A including its matrix and relation builds.
  - The uniform form has neither build. Its kernels, warm, minimum and median of five calls on the Mac
    (`../../output/kagemusha-100x-20261108/spike/runs/zks-uniform-full-v9.log`): A z, B z, C z take 2.9-3.5 ms, and M(r_x, ·) 9.1-10.3 ms.
  - The 0.016 s for A z, B z, C z quoted earlier was a single first call, which also builds the gather plan.
  - The model has no term for those kernels.
- **The sparse expansion of P-recv's rows** proves in 0.224-0.244 s on the Mac. It verifies in 0.026-0.033 s plus
  0.046-0.048 s of matrix build. The uniform proof verifies in 0.021 s with no build.
- **Caveats.**
  - The model understated stage A's real Pixel run: 3.28-3.38 s against 2.44 s.
  - It does not include the peer's kernels (below).
  - No Pixel run of this pair has been made. b04a57 holds a package with the peer's kernels, pending the owner's go
    in its own chat.

**The filler.**

- Each verified proof's matrix-MLE evaluation is filler rows, at the compiled products of the circuits themselves:
  2 × 10,347 rows in P-recv and 2 × 5,551 in S-recv (stage A had 8,000).
- The filler's own size moves the products by a few dozen, through the placement.
- Still open: the evaluation as a program array whose operands are copies. That is 24,054 instructions (10,345
  products of two slots, 3,152 by a constant), and the copies and lookups cost on top of the products.

**Peer kernels merged.** These are from session ebb983, reviewed from source. The 34 tests pass, and the peer reports
identical proof bytes.

- **Faster sumcheck and commit kernels:** rounds on {0,1,∞}^k, first-round shortcuts, parallel eq tables, round-slot
  fixed-base tables, L and R as single MSMs, and cached symbolic rows. The peer measured stage A's Receive pair at
  427 → 312 ms of prover core on the Mac.
- **Inner rounds by corner support.**
  - Per run of 256 rows, a variable whose upper (or lower) half is empty in both tables contributes (1 − Y)² (or Y²).
    The round and the fold then run on the sub-cube.
  - It gains where whole halves are empty. That is always true of the public half. It is true of the witness's top row
    bits only below 32 used rows, which neither circuit reaches.
- **Helper rows committed by value.** The generators are summed per distinct helper value, then one MSM runs over the
  sums.
- **M(r_x, ·) by a flat plan** (third increment). Per placed block and target r', the present rows that read it are
  listed once per circuit, and each reader's rows are summed before one product by its weight.
  - Proofs are byte-identical (proof tags compared over two interleaved rounds).
  - The prover's m_table timer in my runs (medians, loaded Mac): P-recv about 9.9 → 8.9 ms, S-recv about 5.3 → 4.1
    ms.
  - The one-call kernel line in `uniform-full` showed the opposite (9.2-9.7 → 18-19 ms). That was one sample of a
    bimodal timing on the loaded Mac: in either build about a quarter of the calls take twice as long, because workers
    are descheduled (the peer timed six warm calls per process).
  - The line now prints the minimum and the median of five warm calls. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-mplan-*.log`.
- **Timing.** Both are variable-time in which cells are zero or equal, as the MSM already is in its digits.

**Not done, and why.**

- A byte table for the byte encodings: 160 bytes cost 480 cells with shifted lookups, and 832 with a 256-entry table.
- 14-round partial blocks: 12 fewer state cells per permutation (7k in P-recv), worth about 0.015 s of commit.
- Wider transcript permutations (rate 8): they change the protocol and its security analysis, so they are the owner's
  decision.

## Step 8, part 1: packing, the real kernels, and what 2^19 takes (2026-11-10)

Owner: "Start step 8". Code: `src/uniform.rs` (the placer's claims, a position check, `ZKS_CLASSES`), `src/main.rs`
(`pixel_kern`, printed as "Pixel model, kernels of 11-10"). Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-s8-*.log`. All 34 spike tests pass.

**The placer claims only the stripes that hold present instances.**

- Before, a block marked whole rows of its window, up to its array's span. Two kinds of absent instance stayed claimed:
  - absent instances below the span (sparse r fields, and chunk slots past an array's last chunk);
  - the tail of the block's last row.
- Now each block claims exactly the stripes its present instances reach (`claims`), and other blocks take the rest of
  its window. The witness fill asserts that no two present cells share a position.
- Result, with the default 1,024-column stripes and width classes:

  | | rows (first + second phase + constant) | claimed positions | present cells |
  |---|---:|---:|---:|
  | P-recv, before | 40 + 10 + 3 | 762,454 | 519,741 |
  | P-recv, now | 36 + 10 + 3 | 635,478 | 519,741 |
  | S-recv, before | 37 + 3 + 2 | 270,272 | 227,472 |
  | S-recv, now | 35 + 3 + 2 | 256,960 | 227,472 |

- 128-column stripes with two width classes, at most 20 bits and wider (`ZKS_STRIPE_BITS=7 ZKS_CLASSES=20,255`):
  - P-recv uses 33 + 11 + 2 = 46 rows, S-recv 32 + 3 + 2 = 37;
  - the commit model rises 0.01 s, and the verifier products about 300 per circuit;
  - the defaults stay as they are: below the 32-row threshold, fewer rows buy nothing.
- **Best column bits.** The best choice of column bits per block (the low bits of each prefix field: p, r mod 2^sigma, and
  r >> sigma) would cut the empty positions at 128-column stripes:
  - P-recv: 52k → 24k;
  - S-recv: 18k → 13k.

  The dump prints this as `best`. Not implemented: it needs a general instance-to-witness bit map per block, in `Block`
  and the gather's offset; the verifier already reads `wbit` bit by bit.
- **Where the empty positions are.** Counts just past a power of two:
  - the binding chain's 263 = 256 + 7 values: about 22k empty positions in P-recv;
  - 65 = 64 + 1 squeezes: 6k in P-recv and 6k in S-recv;
  - chunk slots: 51 of 64, 24 of 32.

**The kernels on the Pixel.** Source: b04a57, 2026-11-10 08:51Z, stage A's real Receive pair with ebb983's kernels
(`../../ebb983-review/runs/pixel6-real-receive-kern-summary.txt`).

- **Pair core: 2.43 → 1.98 s.**
  - P-recv: 1.25 s = commit 0.55 + outer 0.14 + inner 0.16 + open 0.37 (of which relation 0.11).
  - S-recv: 0.73 s.
- **Inner rounds and folds** cost 50 ms (P) and 28 ms (S). The old model's inner term, 0.2 µs × 2^sz, is 3-4× too high
  once the corner support skips the public half.
- **`pixel_kern`** is calibrated on that run:
  - outer: 0.155 µs per constraint slot;
  - inner: 0.14 µs per witness entry of the non-public half, halved again when at most half the rows are used;
  - the uniform A z, B z, C z and M(r_x, ·): 0.058 and 0.19 µs per row (the Mac kernels times stage A's Pixel/Mac
    ratios);
  - commits: the lane table × 0.78 (fixed-base tables);
  - IPA: the table × 0.76.
- **The uniform pair, modelled: P-recv 1.22 s + S-recv 0.69 s = 1.91 s.** Stage A measures 1.98 s with the same
  kernels.
  - Scaling the Mac kernel timers by stage A's per-kernel Pixel/Mac ratios gives about 1.85 s.
  - At most 32 rows would give 1.15 + 0.65 s, 0.11 s less per pair (the old model said 0.32 s).

**Why 32 rows still matters: the recursion's fixed point.**

- `uniform-full`'s incoming pairs are stage A shapes: 32 rows for P-recv, S-recv and P-send, 16 for S-send.
- **Cost of a 64-row proof to its verifier.** A verified proof's points side folds its row commitments with rows − 1
  endoscalings. A 64-row proof instead of a 32-row one adds about 31k cells to the circuit that verifies it:
  - 32 endoscalings of 64 ten-cell steps;
  - 32 more absorbed points (32 permutations);
  - 64 additions.
- **Fixed point at 64-row proofs.**
  - P-recv: about 551k cells. S-recv: 258-289k, since it also verifies P-send.
  - That is about 0.1 s more per pair in commits, on top of the inner rounds' 0.11 s.
  - So a 32-row fixed point is worth about 0.2 s per pair.
- **What 32 rows needs** (97% packing):
  - P-recv: at most about 477k claimed positions, against 544k with the best column bits; about 65k fewer (12%).
  - S-recv: at most about 226k, against 240k; about 14k fewer.

**The in-circuit matrix evaluation: a program array is not viable.**

- **The compiled evaluation.** P-recv's has 10,338 products of two slots, and S-recv's 5,580 (`ZKS_EVALS_BREAKDOWN=1`).
  P-recv's split, before dead-code removal:

  | per-term | block finals | eq factors (x + y) | split | pattern | points | enumerate | p-sums | sums | single-r |
  |---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
  | 3,073 | 2,035 | 1,347 + 915 | 878 | 516 | 494 | 469 | 277 | 234 | 15 |

- **Per product, with copied operands: about 10 positions.**
  - The two operand reads cost an index column and a LogUp helper each.
  - The result costs its cell, and its export a multiplicity, a helper and an index.
  - Links can carry some operands (the eq tables), so 6-10 per product.
- **Totals.** That is 60-100k positions per verified proof, and 125-200k for P-recv's two: 6-10× the 20.7k-cell filler.
- **Proposal, the owner's decision: defer the matrix claims, as the IPA's generator claims already are.**
  - Each proof carries an accumulated claim M̃(r*) = v* per field.
  - A circuit folds the incoming accumulators and the fresh claims (r_x, r_y, v) two to one by line restriction, as in
    GKR:
    - q(t) = M̃(ℓ(t)), of degree at most 40;
    - check q(0) and q(1) against the two claims;
    - the new claim is (ℓ(τ), q(τ)).
  - Every native verifier evaluates M̃ at the accumulator. The compiled program runs in 0.2 ms on the Mac.
  - **Cost per circuit: 13-31k cells for P-recv, about the filler.**
    - Three folds (two incoming accumulators, two fresh claims) of about 120 cells each.
    - Absorbing q's coefficients: 21 permutations per fold, or none if τ is drawn as a second-phase challenge.
    - The accumulators in the io: about 41 words each, 21 permutations.
  - The proof's io grows by about 41 field elements per field (~2.6 KB).
  - Each fold errs with probability at most 40/|F|, and the final native evaluation is exact.

**What could supply the cells (estimates).**

- **Engineering, no protocol change:**
  - general bit maps: −28k positions in P-recv, −4.6k in S-recv;
  - 14-round partial blocks: −7k and −2.8k;
  - one range table instead of two: −6k in P-recv. The comb and the binding chain each carry the 11-bit table (id 0)
    with its helpers.
  - The constant rows must sit in the first half of the rows, because the corner support needs every used row there.
- **Engineering alone reaches 32 rows for neither circuit:** P-recv stays about 54k short, S-recv about 11k.
- **Protocol, the owner's decision:**
  - rate-8 transcript permutations (width 9): the statement sponge and both S transcripts with io digests go from 314
    permutations to about 86 wider ones, about −50k in P-recv and −36k in S-recv;
  - a 4-ary indexed tree: −30k;
  - at most 254 shared values: the binding chain's 263 = 256 + 7 costs about 22k empty positions;
  - one fixed-base multiplication of U per verified proof instead of two: −2k each.
- **Rate 8 with the engineering items** fits S-recv in 32 rows and brings P-recv to about 481k, just over the limit.
  Adding the 4-ary tree fits both.

**Pixel package, waiting for the owner's go in b04a57's chat.**

- `../../output/kagemusha-100x-20261108/spike/pixel-uniform-pair/`: binary 9556e4ec…, `run-ab.sh` 09556ecc….
  - Built from frozen sources linking the canonical iroha_pasta at b4525a5f6c.
  - The run interleaves the uniform pair with stage A's real Receive pair.
- b04a57 checked it from source.
- The stage A side reproduces b04a57's 08:51Z proof hashes on the Mac (`../../output/kagemusha-100x-20261108/spike/runs/zks-s8-stagea-prof-mac.log`). The
  uniform proofs' hashes are in `../../output/kagemusha-100x-20261108/spike/runs/zks-s8-prof-2.log`.

## Step 8, part 2: the uniform pair on the Pixel (2026-11-10)

b04a57 ran the package after the owner's "Yes, run both" in its chat: 10:35-10:39Z, 25.9 → 26.9 °C, all four runs exit 0.
Logs: `../../output/kagemusha-100x-20261108/spike/runs/pixel6-uniform-pair-{1..4}-<side>.log`; summary `../../output/kagemusha-100x-20261108/spike/runs/pixel6-uniform-pair-summary.txt`. I checked the
summary and the per-proof lines against the logs.

- **Correctness.** Every proof hashes as expected (stage A equal to b04a57's 08:51Z run, the uniform pair equal to the
  Mac's), and the changed cell is refused in both uniform runs.
- **Memory.** Peak RSS 671-672 MB for the uniform pair against 994 MB for stage A.
- **Pair core** (median P + median S per run):

  | | runs | steady state (proofs 2-3) |
  |---|---:|---:|
  | stage A | 1.979 / 2.005 s | about 1.99 s |
  | uniform | 2.113 / 2.101 s | **about 2.08 s** |

  - The first uniform proof in each run was slow: P 1.74-1.82 s, S 0.99-1.05 s. All of the excess was c.phase1 (P
    902 against 502-541 ms). The uniform path built its commit tables inside the first proof, while stage A builds them
    before. Fixed: `uniform-full` now builds them before the timed proofs.
- **Steady state, uniform against stage A:**

  | | P-recv | S-recv |
  |---|---|---|
  | core | 1.35 vs 1.26 s | 0.735 vs 0.732 s |
  | commit | 0.69-0.70 vs 0.53 (c.phase1 525 vs 435 ms, c.phase2 160 vs 101 ms) | c.phase1 325 vs 319 ms, c.phase2 34-58 vs 4 ms |
  | outer | 0.12 vs 0.14 (A z, B z, C z: 36 vs 65 ms) | 0.07 vs 0.09 |
  | inner | about 0.20 vs 0.16 (rounds and folds 82 vs 51 ms) | +0.018 |
  | open | 0.27-0.30 vs 0.39 (stage A's includes the 0.10 s relation) | 0.18 vs 0.24 |
  | verify | 0.11-0.22 vs 0.21 s | 0.06-0.08 vs 0.135 s |

- **Against the kernel-calibrated model** (P 1.22 s, S 0.69 s): measured about 1.35 and 0.735 s. The model is low on
  commits (0.63 against 0.70 s) and outer rounds (0.10 against 0.12 s).
- **Why the uniform prover is 5% slower.** Commits scale with committed cells: P-recv commits 519,741 cells against
  stage A's 353,240 + 76,720 (1.21×), and c.phase1 takes 1.2× as long. The extra cells are the copies, the sound gadgets,
  the real points side and the filler. The inner rounds also run over a 64-row witness whose upper half is not empty.
  These cost about +0.27 s per pair, and the savings in open and outer rounds give back about 0.18 s.
- **What the uniform form buys** is the verifier: native verify is 0.17-0.30 s against 0.34 s per pair, and above all
  recursion. Stage A's in-circuit matrix evaluation is O(5.74 M nonzeros).
- **Outside the core:** compiling the matrix evaluation takes 0.82 s per process on the phone. That happens once per
  circuit, and a verifier would ship it precompiled.

**Fix: helper rows committed by value.** A second-phase row is summed by value when it has at least three cells per
distinct value (`spartan::grouped_row_sum`, ebb983's kernel). In the uniform layout the helpers of fixed-table lookups
(at most one distinct value per table entry) shared rows with copy reads' helpers (all distinct), so most of those rows
fell back to full-width commits.

- The placer now puts fixed-table lookup helpers on second-phase rows of their own, before the other second-phase cells.
  It does so only when they fill at least two rows (`ZKS_P2_SPLIT=off` restores the old placement).
- **P-recv.** Per proof, the cells committed at full width in the second phase fall from 36,224 to 16,178, and the
  cells summed by value rise from 53,785 to 73,831. On the Pixel's c.phase2 rate (about 2.9 µs per full-width cell),
  that is about −45 ms per proof. Not measured on the phone.
- **S-recv.** Its 5k lookup helpers fill under one row and qualify for neither layout, so its layout is unchanged and its
  proofs are byte-identical to the package's.
- All 34 spike tests pass. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-s8-p2-{on,off}.log` (with `ZKS_GROUPED=stats` row counts) and
  `../../output/kagemusha-100x-20261108/spike/runs/zks-s8-p2-final.log`.

## Step 8, part 3: deferred matrix claims, prototype (2026-11-10)

Owner: "Deferral prototype". Code: `src/deferral.rs` (new), `src/uniform.rs` (challenge rows), `src/full.rs`
(`defer_into`, accumulators in the incoming proofs' io; the filler is gone), `src/main.rs` (`deferral_note`). Logs:
`../../output/kagemusha-100x-20261108/spike/runs/zks-s8-defer-{1,2}.log`. All 36 spike tests pass (2 new).

**What the claim is.**

- The verifier's final check uses one combination of the three matrices at the sumcheck point,
  P(r, r_x, r_y) = r A + r² B + r³ C, with the verified proof's own r (the lean schedule's r_A, r_B, r_C = r, r², r³).
  That value is the claim, and nothing else about A, B and C is.
- A selector variable for A, B, C does not work. With a challenge drawn after the values are fixed, the curve's endpoint
  would depend on that challenge, which is circular. With one drawn before, a prover could move error between A, B
  and C while keeping the collapsed claim true.
- The claims of the two circuit kinds a circuit verifies sit on one polynomial:

  P(c, ρ, x, y) = (1 − c) P_0 + c P_1, with P_k = pad_k(x, y) (ρ A_k + ρ² B_k + ρ³ C_k)(x, y).

  Here pad_k zeroes the variables that kind k lacks (S-send has one x and one y variable fewer than S-recv).

**The fold.** Four claims per circuit: proof 0's accumulator and fresh claim, then proof 1's.

- They sit at t = 0..3 on one cubic curve γ. The prover commits q(t) = P(γ(t)) by its deg q + 1 coefficients in the
  first phase:
  - P-recv: 41 coordinates, q of degree 3 (1 + 19 + 20) + 9 = 129;
  - S-recv: 39 coordinates, degree 123.
- The circuit checks q(0..3) against the four values: q(0) is the first coefficient, q(1) a sum row over the
  coefficients, and q(2), q(3) Horner chains.
- In the second phase it computes the new claim (γ(β), q(β)), with β the LogUp challenge drawn after the first-phase
  commitments.
- Only β, β², β³ appear as factors: the folded point is a Lagrange combination, and q(β) is a Horner chain with factor β.
- One curve and one challenge suffice, where three sequential line folds would need three phases. Any fold of four
  claims costs about deg q + 1 evaluations of P.
- Soundness: a false input claim makes q ≠ P∘γ, so q(β) is false except with probability at most deg q / |F|.

**Wiring (all links, no copies).** The scalar sides share the global family, so the fold reads:

- the fresh points from the verified proof's scalar side: kind and padding as constants, ρ from glue R1, and r_x, r_y
  from the slot data's challenge digits;
- the fresh value from glue M1 + M2 + M3, which is the value the proof's check consumes;
- the incoming accumulator from the last n + 1 leaves of the io fold.

**The incoming accumulators** are stand-ins:

- Each incoming proof's io ends with an accumulator: a random point and its true value.
- The existing io digest then absorbs it (21 more permutations per verified proof in P-recv, 20 in S-recv), and the io
  fold evaluates it, which is how the binding is paid for.
- Production must absorb it as late io instead: in the producing proof's transcript after its second-phase challenge
  (the accumulator depends on it) and before the second-phase commitments. The in-circuit cost is the same.

**Framework: challenge rows.**

- A builder declares second-phase rows whose terms carry LogUp challenge factors, together with the cells those rows
  define and a fill computed once the challenges are known (`ChallengeRows`).
- The symbolic rows, the compiled evaluation, `abc` and M(r_x, ·) already handled factor terms for the LogUp rows.
  The new part is the placeholder rows, the second-phase flags and the fill.

**Checks.**

- `deferred_fold_holds_proves_and_refuses`:
  - the honest fold satisfies the relation, its output equals the native fold, the folded claim holds, and the proof
    verifies;
  - a false fresh claim with q bent through it (q + L_1) satisfies the relation, but the folded claim is false;
  - a changed coefficient, folded coordinate, folded value, fresh kind or padding coordinate, or a changed claim value,
    is refused (the last one also by the verifier).
- `uniform-full`, both circuits:
  - the circuit's fold equals the native fold, and the folded claim at challenges (5, 7) holds natively;
  - a changed fresh value, r_x digit or accumulator coordinate breaks a row;
  - all proofs verify, and the changed cell is refused.

**Measured**, against step 7's filler:

| | P-recv, filler | P-recv, deferred | S-recv, filler | S-recv, deferred |
|---|---:|---:|---:|---:|
| witness cells | 519,741 | **513,232** | 227,472 | **229,689** |
| constraint rows | 402,075 (2^19) | 394,598 (2^19) | 208,101 (2^18) | 209,462 (**2^19**) |
| permutations | 582 | 624 | 230 | 270 |
| fold cells | — | 729 | — | 695 |
| Pixel model (kernels of 11-10) | 1.22 s | 1.21 s | 0.69 s | 0.73 s |

- **The filler was a lower bound for something not buildable in this layout.** A copy-operand program array would have
  added 125-200k positions to P-recv. Deferral costs about what the filler did: −6.5k cells in P-recv, +2.2k in S-recv.
- **S-recv's constraint rows now end at 264,304, 2,160 past 2^18.**
  - Its 270 permutations (> 256) give pool blocks of 4,096 rows with 2,160 present. Ten fit in the endoscaling blocks'
    tails, and the last one does not fit.
  - That costs about 0.04 s of outer rounds.
  - Fixes: split the pool family into 256 + 14 (more template terms are cheap now, see below), or rate-8 transcript
    permutations.
- **The verifier's matrix-MLE products no longer cost circuit cells.** P-recv's are 10,548 and S-recv's 5,939, now
  evaluated natively only. The 8,000-product budget is moot. Narrow stripes, split families and finer pieces now cost
  only native time.
- **The prover's new native work** is q: deg q + 1 evaluations of P, each one compiled matrix evaluation per kind.
  - Measured here as 130 × 2 × 0.22-0.80 ms = 57-207 ms on one Mac core for P-recv (machine load 60-110), and
    124 × 2 × 0.15 ms = 38 ms for S-recv.
  - The evaluations are independent, so they parallelize; roughly 6× slower per core on the Pixel.
- **The native verifier's new work** is one evaluation of P per accumulator, under 2 ms.

**Before production.**

1. **Late io.** The proof format gains a public segment absorbed after the second-phase challenges, carrying the
   circuit's output accumulator. The spike leaves the output in witness cells and binds the incoming accumulators as
   ordinary io.
2. **Challenge variables at the fixed point.**
   - A uniform circuit's matrices contain its LogUp rows, so M depends on the verified proof's α and β.
   - The accumulated polynomial needs α (degree 1) and β (degree up to the widest tuple, about 8) as two more
     variables, with the fresh claim at the proof's own α, β. That puts deg q at about 156 for P-recv.
   - The toy incoming proofs have no second phase, so this is not exercised yet.
3. **Proof size.** The io grows by 42 field elements per proof for P and 40 for S (about 2.6 KB per pair), which counts
   against the 12 KB QR.
4. **The fixed point itself:** incoming proofs of the uniform circuits, with their compiled evaluations as the kinds.

## Fixed point, part 1: public io and late io (2026-11-10)

Owner: "Build fixed point". Code: `src/spartan.rs` (late io in the proof format), `src/r1cs.rs` (`late_slots`,
`late_values`), `src/uniform.rs` (io arrays, the compile's selector bit), `src/deferral.rs` (`expose`, `claim_of`),
`src/full.rs`, `src/main.rs`. Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-s9-late-1.log`. All 36 spike tests pass.

**Late io in the proof format.**

- A circuit names io slots that the prover fills after the second-phase challenges (`Circuit::late_slots`,
  `late_values`). The proof carries those values after its rows.
- Prover and verifier absorb their digest (`io_digest`, the same hash as the early io's) right after the second-phase
  challenges and before the second-phase rows. The verifier places the values at their slots for the io evaluation.
- Toy and sparse circuits have no late io, so their proofs are unchanged: stage A's six hashes are equal with
  `ZKS_REPEAT=3`. With `ZKS_REPEAT=1` the S hashes differ only because P and S draw from one generator.

**Public io in the uniform framework.**

- A one-cell array of the global family can sit in z's public half (`Copies::io_arrays`): its block's base is
  2^sw + offset, with the offset aligned to its size.
- Rows read public inputs through ordinary links, so the gather, M(r_x, ·) and the compiled closed forms serve them
  unchanged.
- One fix was needed: the compile multiplied every block by the witness half's (1 − r_y[0]). A public-half block now
  uses r_y[0], with its own memo.
- Placement, the witness fill and the cell counts skip io arrays.

**The accumulator is now public.**

- `deferral::expose` puts the folded claim (point, then value) in an io array at io index 64, after the early io, ties
  it to the fold's second-phase cells, and declares it late io.
- Checks:
  - in the unit test, the proof's late io is its folded claim and holds natively, and a late value changed in the proof
    bytes is refused;
  - in `uniform-full`, P-recv's and S-recv's real proofs carry their accumulators (42 and 40 values), and each holds
    natively;
  - all proofs verify.
- **Proof sizes:** P-recv 3,520 → 4,864 B, S-recv 3,488 → 4,768 B, 9.6 KB per pair. The receiver's prover needs the
  values (the next hop's in-circuit verifier reads them under the digest), so they travel with the proof. They count
  against the 12 KB QR.

**Still to build for the fixed point.**

1. A native trace of circuit proofs:
   - the second-phase squeezes;
   - the late io digest;
   - the fixed rows, absorbed from the verification key.
2. The points side for uniform proofs, in the other circuit:
   - the phase-2 challenges;
   - the late digest, absorbed after them (computed in the scalar-side circuit and shared through the binding chain, like
     the early io digest);
   - fixed rows as constant points in the row fold;
   - 64 rows.
3. The scalar side for uniform proofs:
   - α and β as shared challenges;
   - io with the late values;
   - the fresh claim with α, β coordinates.
4. Uniform pairs proved with the pair link (`Link::First`/`Second`).
5. Deferral kinds that are uniform circuits: evaluation through the compiled program at (α, β), and β's degree from the
   widest tuple.
6. The iteration:
   - C1 (today) proves π1;
   - C2 verifies π1 (plus the Send stand-in) and proves π2;
   - when shape(C2) = shape(C1), C2 verifies its own proofs.

   The expected shapes stay at P 2^19 × 64 rows of 16,384 and S 2^19 × 64 rows of 8,192. Verifying a 64-row
   predecessor adds about 31k cells on each side.

## Fixed point, part 2: the uniform pair verifies its own proofs (2026-11-10)

Owner: "Continue part 2". Mode: `ZKS_HOPS=6 ZKS_GENESIS_HOP=5 ZKS_SCHEDULE=lean zk-spartan fixed-point`. Logs:
`../../output/kagemusha-100x-20261108/spike/runs/zks-s9-fp-{6,7}.log`. All 36 spike tests pass, and `uniform-full` is unchanged in behaviour.

**What was built.**

- **The native trace of circuit proofs** (`verifier::trace_circuit`) reads the transcript as `verify_inner` does:
  - the first phase's rows, then the second-phase challenges and the late io's digest;
  - then the second phase's rows and the fixed rows (the key's commitments).

  On a real uniform proof it reaches the same acceptance as the verifier.
- **The points side for circuit proofs** (`points.rs`) squeezes the second-phase challenges after the first phase's rows
  and absorbs the late digest's word (bound to its shared value).
  - The fixed rows enter the row fold as inputs pinned to the verification key's coordinates.
  - A shared value list puts the late digest before the pair link's R, so R stays last.
- **The scalar side for circuit proofs:**
  - the verified circuit's matrix evaluation is a parameter (`scalar_side_with`);
  - the io fold reads the io with its late values;
  - two glue cells carry α and β, read from the shared values ahead of the pair link.
- **The late digest in-circuit** (`IoDigest::start`): the scalar-side circuit hashes the verified proof's late io
  from its io fold leaves and binds the digest to the shared value that the points side absorbs.
- **Linked uniform pairs** (`spartan::prove_linked_circuit`, `verify_linked_circuit`):
  - P's phase-1 digest d comes from a first pass;
  - S runs with d, giving ρ and R;
  - P runs again with ρ and R, with the same randomness, so it has the same rows and the same d.
- **Uniform deferral kinds** (`full::uniform_kind`):
  - the kind is the verified circuit's compiled evaluation at (α, β);
  - its degrees in α and β come from its symbolic rows' factors (`challenge_degrees`): 1 and 9.
- **The iteration** (`full::next_pairs`): the hop's linked uniform pair becomes the next hop's predecessor Receive, traced
  as circuit proofs, with its late io as its accumulator. A fresh toy Send pair stands in for the payer, because no
  uniform Send circuit exists yet.
- **A base case.**
  - The circuit's io carries a genesis flag g (io 62) and a value v0 (io 63).
  - The predecessor's accumulator node satisfies (P0 − io)(1 − g) = 0 and (P0 − g0)·g = 0, with g0 a fixed point of
    the template. The value is handled the same way, and g is boolean.
  - With g = 1 the genesis claim (g0, v0) replaces the predecessor's accumulator. g and v0 are data, so the matrices do
    not change.
  - Who may set g = 1 is the protocol's to decide (a first offline state, for example). Setting it at any other hop
    would drop the predecessor's deferred claims.

**What it shows** (`../../output/kagemusha-100x-20261108/spike/runs/zks-s9-fp-6.log`):

| hop | verifies | P-recv | S-recv | against the previous hop | accumulators |
|---|---|---|---|---|---|
| 1 | toy pairs | 2^19, 514,464 cells | 2^19, 230,897 | — | true |
| 2 | hop 1's uniform pair | 2^20, 544,968 | 2^19, 261,306 | shape and sizes differ | false (bootstrap) |
| 3 | hop 2's | 2^20, 544,968 | 2^19, 261,306 | sizes differ slightly (P sx 19 → 20) | false |
| 4 | hop 3's | same | same | sizes same, matrices differ | false |
| 5 | hop 4's, **base case** | same | same | **matrices EQUAL** | **true** |
| 6 | hop 5's | same | same | **matrices EQUAL** | **true** |

- **The matrices converge.** They agree at a random point (A, B, C at random r_x, r_y, α, β) from hop 4 → 5 on: the
  circuit that verifies hop k's proofs is the circuit that proved them. The matrices stop changing one hop after the
  sizes do, because each hop's matrices carry the previous circuit's verification key as constants (its fixed rows).
- **Hop 6 is a real fixed-point hop.** It verifies hop 5's real uniform pair in-circuit and folds hop 5's real
  accumulator with fresh claims on its own matrices. Both resulting accumulators hold natively.
- **Without the base case, accumulators are false from hop 2 on.** Hop k's incoming accumulator is a claim on hop k − 1's
  polynomial, whose kind is a different circuit until the matrices settle. The base case at a converged hop fixes that;
  in production the chain starts at a base case.
- **Every hop's linked pair proves and verifies.** Proofs are 4,992 + 4,896 B (9.9 KB per pair).

**At the fixed point.**

- **Size.** Verifying its own 64-row predecessor instead of a 32-row toy adds about 30.5k cells per side, as estimated:
  - P-recv: 544,968 cells and 425,131 constraint rows. Those pack past 2^19, so it runs at 2^20 constraints, with 39 + 11
    + 3 of 64 witness rows.
  - S-recv: 261,306 cells and 239,789 rows, at 2^19, with 42 + 3 + 2 rows.
- **Pixel model (kernels of 11-10):** P-recv 1.34 s (outer 0.19, inner 0.23, commit 0.67, IPA 0.26) and S-recv 0.79 s,
  so **2.13 s per pair**. Part 3 brings this to 2.01 s.
  - P-recv packed into 2^19 constraints would save about 0.08 s, and S-recv into 2^18 about 0.04 s.
  - The model ran about 10% low on the Pixel for step 7's pair.
- **The prover's native q:**
  - the first hop over a uniform kind compiles that kind's matrix program (0.4 s, once per circuit);
  - after that, 112 ms for P (163 evaluations) and 53 ms for S on one core of a heavily loaded Mac;
  - about 1 s single-threaded per pair on the Pixel, or about 0.2 s over its cores. It is unoptimized: no batching across
    the evaluation points.

**Still open.**

- The constraint-row packer: P-recv into 2^19, S-recv into 2^18 (done, see part 3).
- The base case's authorization, which is a protocol matter.
- A uniform Send circuit, to replace the toy Send stand-in.
- The binding chain's ρ and μ stand-ins, a gap carried over from stage A.
- Batched native q (done, see part 4).
- A Pixel run of the fixed-point pair.

## Fixed point, part 3: family splitting (2026-11-10)

Owner: "Fix packing first". Code: `uniform::split_families` (`ZKS_SPLIT=off` disables it). Logs:
`../../output/kagemusha-100x-20261108/spike/runs/zks-s10-fp-1.log`, `../../output/kagemusha-100x-20261108/spike/runs/zks-s10-split-1.log`, and with the split families' counts printed,
`../../output/kagemusha-100x-20261108/spike/runs/zks-s10-fp-2.log` (after) and `../../output/kagemusha-100x-20261108/spike/runs/zks-s10-fp-nosplit.log` (before; `ZKS_SPLIT=off`). All 36 spike tests
pass.

**The waste.** At the fixed point, P-recv's constraint rows ended at 563k with 425k present, and S-recv's at 285k with 240k
present (`ZKS_CPLACE`). Nearly all of it came from four families whose instance counts sit just past a power of two:

- the permutations: P-recv 661 = 512 + 149, S-recv 307 = 256 + 51;
- the endoscalings: P-recv 159 = 128 + 31, S-recv 181 = 128 + 53.

Each template row of such a family is a block of 2^(k+1) instances aligned to its size, about 60% present. Only smaller
blocks could use the empty tails.

**The fix.**

- `build_with_copies` splits every family with 2^k < n < 1.5 · 2^k (2^k ≥ 8) at 2^k. The arrays keep p < 2^k, and clones
  in a new family take the rest, shifted down.
- Links stay within a family (or read a global array), so the clones' links point at the clones.
- Copy sites on the arrays split with them, reads of a split export are renumbered, and a sum over a split array also sums
  its clone. `cell_index` redirects into the clone.
- The builders are untouched.
- The cost is more template terms. Since deferral, those cost only native time.

**Measured at the fixed point:**

| | before | after |
|---|---:|---:|
| P-recv constraints | 2^20 (425,131 rows) | **2^19** (423,870) |
| P-recv witness rows used | 39 + 11 + 3 | 36 + 11 + 2 |
| P-recv cells | 544,968 | 542,778 |
| S-recv constraints | 2^19 (239,789) | **2^18** (239,769) |
| S-recv witness rows used | 42 + 3 + 2 | 39 + 3 + 2 |
| template terms / matrix-MLE products, P | 4,565 / 10,710 | 5,073 / 11,276 |
| template terms / matrix-MLE products, S | 2,074 / 5,835 | 2,576 / 6,457 |
| Pixel model (kernels of 11-10) | 1.34 + 0.79 = 2.13 s | **1.26 + 0.75 = 2.01 s** |

- The fixed point holds: the matrices are identical from hop 4 → 5 on, and the accumulators hold at hops 5 (base case)
  and 6. Every hop's linked pair verifies.
- The smaller P-recv verifies itself with one permutation and one endoscaling fewer: its split families settle at
  660 = 512 + 148 and 158 = 128 + 30. S-recv keeps 307 = 256 + 51 and 181 = 128 + 53.
- Proofs are 5,024 + 4,864 B: P-recv now has 2 constant rows instead of 3.
- The toy-pair circuits (hop 1) improve too: S-recv goes from 2^19 to 2^18 and from 44 to 38 witness rows.
- **Native q** grows with the products: 86-433 ms for P and 40-175 ms for S on one core, with the Mac's load at 80-110.

## Plan

1. **Done:** permutation with materialized rounds, and the uniform relation (see above).
2. **Done:** elliptic-curve ladders, and constant columns: fixed bases share the var_mul family.
3. **Done:** LogUp, the canonical non-native checks and the complete comb, including digit binding, scalar
   checks and the final check. The matrix evaluation is compiled, and `abc` gathers by row block (see above).
4. **Done:** SHA-256 rounds and the binding chain, closed forms over intervals, the region alignment fix, and
   lookup arrays: the five blocks' verifier evaluation is 5,446 products against the 8,000 budget (see above).
5. **Done:** k = 3 with 64-column round slots, the closed form per slot, and the scalar side as uniform arrays
   (968 products; see above).
6. **Done:** the copy argument with preprocessed index columns, the own relation's signed messages and one proof's
   scalar side wired, and wiring-tamper refusals: everything so far is 5,936 products in one circuit (see above).
7. **Done:** the full Receive pair proved and verified in the uniform layout, with every flow wired, sound comparisons
   and canonical byte encodings, width-banded placement, and the filler at the measured matrix-MLE products. The model
   gives 1.75 + 0.97 = 2.72 s per pair (see above). The sparse form's 2^19 witness is not reachable by packing.
8. **In progress** (see "Step 8, part 1"):
   - done: exact stripe claims (P 53 → 49 rows, S 42 → 40), the kernel-calibrated Pixel model (pair 1.91 s; 2^19
     saves 0.11 s), the fixed-point and program-array analyses, the Pixel package;
   - done: the Pixel run (part 2): uniform pair about 2.08 s against stage A's 1.99 s in steady state, verify faster;
     commit tables warmed before timing; fixed-table helpers on rows of their own (P: −20k full-width commits a proof);
   - done: deferred matrix claims prototyped (part 3), replacing the filler at about its cost; both circuits' folded
     claims hold natively, and a false claim is caught at the native check;
   - the owner's decisions: late io in the proof format (for the accumulator); for a 32-row fixed point, rate-8
     transcript permutations and possibly a 4-ary indexed tree;
   - done: public io arrays and late io in the proof format; both circuits' proofs carry their accumulators, which hold
     natively (fixed point, part 1);
   - done: the fixed point (part 2). The uniform pair verifies its own proofs, with identical matrices from hop 4 → 5,
     and a base case gives true accumulators at the fixed point. Model 1.34 + 0.79 = 2.13 s per pair; proofs 9.9 KB.
   - done: family splitting (part 3): P-recv 2^20 → 2^19 and S-recv 2^19 → 2^18 constraints at the fixed point; model
     2.01 s per pair;
   - done: batched native q (part 4), with pointwise parity and degree-bound tests on both Pasta fields;
   - next: a Pixel run of the fixed-point pair including batched native q; a uniform Send circuit;
     the witness at 32 rows (about 65k fewer positions in P-recv);
   - then: general bit maps, 14-round partial blocks, one range table, constants in the first half of the rows.


## Fixed point, part 4: batched native q (2026-11-10)

The native prover now evaluates the compiled matrix program across groups of four
curve points. Register liveness reuses slots after their last reader; an output
register stays live. Independent lane groups run through Rayon, preserving input
order. The scalar evaluator remains a research reference. Every benchmark run
asserts exact equality of the complete coefficient vector with that reference.

Each circuit kind evaluates its matrices only at `kind_degree + 1` points, where
`kind_degree = 3 * (sx + sz + degree_alpha + degree_beta)`. Equispaced Lagrange
extension supplies the remaining values before applying the selector, `rho` and
padding factors. This changes native work, not the committed polynomial or
proof protocol. Both Pasta fields have direct-polynomial, off-grid and boundary
tests; register tests cover repeated operands, retained/aliased outputs, dead
inputs, zero inputs, empty batches and partial lane groups.

Claude's completed local Mac run (16 threads, seven interleaved rounds) measured:

| Native q | Pointwise median | Four lanes, one thread | Four lanes, pool |
|---|---:|---:|---:|
| P-recv | 73.8 ms | 46.7 ms | 12.9 ms |
| S-recv | 37.7 ms | 24.3 ms | 9.3 ms |

All seven pool runs were faster than their pointwise counterpart. These are local
Mac measurements under changing load, not a Pixel prediction. The source log is
`../../output/kagemusha-100x-20261108/spike/runs/zks-s11-q-bench-2.log`. It reports
linked verification accepted, unchanged 5,024 + 4,864-byte proofs, and both native
accumulators true at hops 5 and 6.

The source now lives at `experiments/kagemusha-spartan` in the canonical checkout;
the previous ignored source directory was moved, not copied. See [README.md](README.md)
for runnable commands and the outstanding Send, binding, base-case and integration
work. The fixed-point run is still a research harness and cannot authorize a
monetary transfer.

Codex takeover validation from the moved source passed all 40 release tests,
including the existing forged-proof/bad-claim refusals and three added regression
tests. The six-hop `fixed-point` run exited successfully: every linked pair
verified, both matrix evaluations agreed at hops 5 and 6, and both accumulators
were true at those hops. Proof sizes stayed 5,024 + 4,864 bytes. Scoped package
formatting also passed.

In that later, concurrently loaded Mac run, pointwise native q medians were
95.9 / 36.3 ms (P / S), four-lane pool medians 45.6 / 29.3 ms, and eight-lane pool
medians 27.1 / 25.0 ms. Every variant retained coefficient equality. Lane-width
timing is load-sensitive; the earlier four-lane default is retained without a
claim that it wins on every machine or load. Logs remain ignored at
`../../output/kagemusha-100x-20261108/spike/runs/codex-q-{full-tests,fixed-point}.log`.
