# Rerandomizable precomputation

This chapter describes `prep_prove`, the reusable precomputation phase for \\(\mathrm{Vega}\_{\mathrm{MC}}\\). Preparation builds committed prover state once; each call to `prove` then rerandomizes that state before emitting a fresh zero-knowledge proof.

## Input and output

`prep_prove` consumes the prover key, a slice of concrete step circuits, the concrete core circuit, and the `is_small` speed hint. It returns `VegaMcPrepZkSNARK`, which contains:

| Field | Meaning |
|---|---|
| `ps_step` | One `PrecommittedState` per step circuit. |
| `ps_core` | The `PrecommittedState` for the core circuit. |
| `cached_step_matvec` | Optional cached step matrix-vector products. |
| `cached_step_i64` | Optional small-integer form of cached matrix-vector products. |
| `large_positions` | Positions where the small-integer cache needs field-arithmetic correction. |
| `cached_step_public_values` | Public values used to validate the cache at prove time. |

The output is prover-private state. It is not a proof object and is not part of verifier input.

## Shared witness

Preparation first synthesizes the shared witness from the first step circuit. That shared witness and its commitment are then reused by every step instance and by the core instance. The result is a single shared commitment, `comm_W_shared`, that represents the common shared segment across the whole presentation.

This shared commitment appears once in the emitted proof object. The per-step instances and the core instance carry compatible shared-commitment data internally, and verification checks that all instances agree with the core instance. The proof-level storage avoids repeating the same shared commitment for every step.

## Precommitted states

After the shared witness is available, preparation builds one step precommitted state for each supplied step circuit. Each step state starts from a clone of the shared state, then fills and commits its step-specific precommitted segment.

The core state reuses the original shared state and fills the core-specific precommitted segment. The prepared result therefore has a one-to-one `ps_step` entry for each step circuit plus a single `ps_core` entry for the core circuit.

The online `prove` call later checks that the number of prepared step states still matches the number of step circuits it receives. A mismatch is rejected before proof generation proceeds.

## Prover-internal caches

The cache fields are deterministic speed optimizations. They do not define the public proof format, and they do not add verifier input. A byte-equivalent reference prover may omit these caches and compute the same matrix-vector data during proving instead; see the byte-equivalence boundary in [Specification scope](../spec/scope.md).

`cached_step_matvec` is populated only when the step shape has no challenge variables and no unpadded rest variables. In that case the full step vector is known during preparation, so the step matrix products can be computed early. `cached_step_i64` stores a small-integer representation for faster folding arithmetic; `large_positions` records entries that must be corrected with field arithmetic. `cached_step_public_values` records the public values used when the cache was computed.

Before using a cached matrix-vector product, `prove` recomputes each step circuit's public values and compares them with `cached_step_public_values`. If the public values changed between preparation and proving, the cache is stale and proving is rejected.

## Rerandomization per proof

The prepared state is reusable because `prove` owns it by value, rerandomizes it in place, and returns it together with the proof. A caller passes the returned `VegaMcPrepZkSNARK` into the next `prove` call to reuse the expensive preparation.

At the start of each proof, `prove` rerandomizes the core precommitted state. It then takes the core state's refreshed shared commitment and blind, and rerandomizes every step state with that same shared commitment and blind. This keeps the shared segment consistent across all instances while giving the proof fresh hiding randomness.

This fresh commitment randomness is the per-proof zero-knowledge input for prepared state reuse. The surrounding zero-knowledge construction is described in [Zero knowledge](./zero-knowledge.md).

## The small-value hint

`is_small` is a prover-side speed hint indicating whether witness values fit in machine words. It selects faster commitment and witness paths when the values satisfy that condition. The verifier still checks the resulting commitments and algebraic claims, so the hint is not trusted as a statement of soundness. It does not change the proof format; with valid inputs it is an implementation choice rather than serialized data.

Preparation feeds the rerandomized state into the online prover described in [Proving](./prove.md), and the resulting proof is checked by [Verification](./verify.md).
