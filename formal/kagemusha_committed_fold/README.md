# Committed-fold stopped-path scalar controls

This bounded standard-library model exercises the selected committed-fold
change of variables in [C12](../../specs/kagemusha_c12_composition_v1.md).
It is separate from the retained salt-only `kagemusha_fold` package and does not
change or refresh that package's source pins.

```sh
python3.12 -B -S -m unittest discover -s formal/kagemusha_committed_fold -p 'test_*.py' -v
```

The experiment fixes a small cyclic additive group, nonzero W/U logs, a public
context and one or two rounds. It first selects all `1+2k` field coins in
rho/L/R order. D opens a deterministic digest that includes the ordered inputs
and **all** masks, including masks not reached after a stop. Alpha/z/zeta follow
D and may be zero. Each round emits L then R and maps its deterministic raw
challenge zero to one. A zero D/L/R stops immediately without a retry. The
unabsorbed final G may be zero, but it must satisfy the full generator decision.
Every admitted source has width1..k, corresponding to native source_k1..16;
zero-width input lists are rejected even alongside an otherwise valid full-width source.

The forward algorithm folds mutable coefficient, power and generator vectors.
The inverse algorithm independently expands source polynomials and projects
their original vectors through all prior challenges using the closed bit-product
formula. Its recurrence does not call the forward algorithm or coefficient
helper. Both algorithms share an admission check that uses the bit-product
helper to verify the deciding source claims before either transform. Both also
share the public toy digest/transcript functions: this tests two algorithms for
one fixed function, not two unrelated transcript laws.
The independent verifier equation uses Horner input commitments, product-form
evaluations and the exact `-gamma*W` correction.

Every tape is enumerated for four parameter shapes: fields5/7, each at one/two
rounds. Two different deciding input lists are checked for each shape, totaling
40,800 forward tapes. Every stopped-prefix/unused-mask completion is enumerated
independently. For stop position s (D=0, then ordered L/R), each particular prefix
has `p^(2k-s)` tape preimages; successful paths have one. Counts are compared as
integers without conditioning on success. The inverse completes the unreached
masks before solving rho from the entire digest. Its recovered tape must equal
the original tape, so the control retains private witness coupling rather than
discarding rho or the masks. Public stopped path laws are compared across each
pair of input lists; private c and tape values are deliberately not claimed
input-independent.

A separate ordered-pair control holds D and the prior path fixed, changes only
the current L mask, and checks that every observable current R is unchanged.
The unique L-identity case stops before exposing that already computed R.

Mutation controls distinguish different claims:

- Solving rho before completing the unseen masks breaks fixed-source round trips.
- Dropping a mask only in the inverse breaks correspondence to the fixed forward
  digest. If both algorithms consistently omit that mask, the abstract bijection
  still holds. The suite requires that positive result; omission is not labeled
  an abstract privacy or bijection attack.
- Resampling until every point is finite deletes the actual stopped mass;
  keeping only one completion per stopped prefix loses its multiplicity.
- Removing or reversing gamma's sign fails the pure-generator equation on
  successful cases, although the same point-path sampler still exists.

## Source map and substitutions

The exact selected bodies are consistency-pinned, not a compiler dependency
closure. Missing/changed bytes refuse the inventory test.

| Selected owner | Mapped relation or cut | Deliberate model limit |
|---|---|---|
| `accumulation.rs::input_digest_bound`, `digest_limbs`, `opening` | Exact binding: ordered source inputs and all masks, then two digest limbs and rho in D | Toy integer framing and radix-p limbs replace Pasta scalar/base encodings and RP57; only `InputBinding::Exact` is represented. Closed TerminalFrame prefix reuse is not modeled. |
| `accumulation.rs::prelude`, `create_fold_bound` | Public context and D precede alpha/z/zeta; shifted source coefficients; ordered masked L/R; inverse-u/u vector updates | k=1/2 additive scalar logs replace the native k16 curve/MSM implementation. The model computes both candidate points before the ordered identity checks, but models no timing or work leakage. |
| `accumulation.rs::verify_fold_bound` | D opening and complete equation including `gamma=sum(mask_L/u+mask_R*u)` | Independent scalar equality, not a native or circuit verifier execution. |
| `accumulation_circuit.rs::committed_fold_prelude`, `verify_fold_bound` | Corresponding mask/digest/opening and gamma equations are recorded as a review boundary | No circuit synthesis, constraints, witness parser or native/circuit differential claim. |
| `fold_tape.rs::try_fill` | Full rho then32-mask native tape exists before D | Toy depth has3/5 coins. Enumeration assumes ideal complete canonical tapes. OS entropy, partial-fill errors, rejection sampling and wiping are not modeled or assigned an error probability. |
| `purpose.rs`, `claim.rs` | Fixed purpose/count, a full-width deciding source, full-group final G | The four toy context words are not production purpose tags, parameter identity or admission. |
| `transcript/mod.rs`, `transcript/pipa_r.rs` | Finite absorbed D/L/R; raw zero-to-one actual-round map; full-group unabsorbed G | Public polynomial toy hashing replaces RP57. There is no random-oracle, uniform-challenge or concrete challenge-law claim. |
| `pcs/ipa/mod.rs::fold_scalars`, `fold_evaluation` | Bit-product source coefficients and product-form evaluation | Exhaustive controls cover only the stated finite shapes. |
| `kagemusha_c12_composition_v1.md:105–136` | Stopped triangular coupling and delayed rho solve | The general lemma, actual source-validity invariant and adaptive private materializer are not proved by this finite enumeration. |

The generator logs are public toy data and may be dependent. They are not logs
for installed Pasta parameters. The digest is an arbitrary deterministic
function into two small limbs; it is not cryptographically hiding or collision
resistant. The point-path projection includes the index of a failed identity
attempt; that identity is not a serialized proof point. Successful G is a
deterministic function of the path challenges. c/rho/masks remain private.

No native proof, RP57 execution, actual33-coin tape, k16 fold, key, catalog,
recursive continuation, physical storage or phone measurement is produced.
Cancellation, work/timing observations and entropy exhaustion are separate cuts
and are not validated by this finite-point-law enumeration.
No claim of native refinement, C12 closure, full protocol security or release
qualification follows. Independent inspection of the source map and current
native/circuit tests remains necessary; fresh source pins alone are not proof
of correspondence.
