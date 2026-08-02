# The transcript schedule

The transcript schedule is the exact ordered sequence of `absorb`, `squeeze`, and `dom_sep` operations that derives every Fiat--Shamir challenge in \\(\mathrm{Vega}\_{\mathrm{MC}}\\). The canonical order is the order performed by `verify`; a byte-conforming prover reproduces the identical operation stream. The transcript mechanism is specified in [The Fiat--Shamir transcript](../building-blocks/transcript.md), and the value bytes fed to `absorb` are specified in [Byte encodings and serialization](../building-blocks/encodings.md). Every transcript in this chapter is created with the domain label `b"neutronnova_prove"`.

## Notation

This chapter writes transcript operations as:

```text
absorb(label, value)
squeeze(label) -> challenge
dom_sep(bytes)
```

The `label` argument is a fixed ASCII byte string. The `value` argument is encoded as specified by [Byte encodings and serialization](../building-blocks/encodings.md). A count written `Scalar(n)` — for example `Scalar(num_steps)` — denotes the integer `n` reduced into \\(\mathbb{F}\\) and absorbed with the scalar transcript encoding, not as a machine word. A `squeeze` consumes the transcript state produced by all preceding operations, including everything absorbed since the previous `squeeze`. Ordering disambiguates reused labels: `b"r"`, `b"challenge"`, and `b"p"`/`b"c"` appear in more than one place.

Symbolic counts are used here. Concrete verifier-key dimensions and proof-object field shapes are pinned by [The verifier key](./verifier-key.md) and [The proof object](./proof-object.md).

```text
num_steps        = vk.num_steps
padded_steps     = next_pow2(num_steps)
num_rounds_b     = log2(padded_steps)
num_vars         = S_step.num_shared + S_step.num_precommitted + S_step.num_rest
num_rounds_x     = log2(S_step.num_cons)
num_rounds_y     = log2(num_vars) + 1
outer_rounds     = log2(vc_shape_regular.num_cons)
inner_rounds     = log2(next_pow2(vc_shape_regular.num_vars)) + 1
```

The verifier-instance challenges are concatenated as `r_b | r_x | r | r_y`, with lengths `num_rounds_b`, `num_rounds_x`, `1`, and `num_rounds_y`.

During setup every R1CS shape has its constraint count padded to a power of two, and the step and core shapes `S_step` and `S_core` also have their variable counts padded to a power of two. So `num_rounds_x`, `num_rounds_y`, and `outer_rounds` are exact base-two logarithms. The one exception is `vc_shape_regular.num_vars`: it is the sum of the verifier circuit's per-round witness widths and is not itself padded to a power of two — it is `640` in the cubic fixture — so `inner_rounds` rounds it up with `next_pow2`, exactly as the shipped relaxed-Spartan verifier does. Every round count is a fixed non-negative integer determined by the shapes rather than by the witness.

## Phases at a glance

1. Instance validation.
2. Main setup.
3. Verifier-instance rounds.
4. NIFS fold.
5. Relaxed Spartan.
6. PCS opening.

## Phase A — Instance validation

Instance validation uses independent throwaway transcripts, one per instance. The prover produces these per-instance challenges during witness commitment, and the verifier replays the same stream to re-derive and check them.

For each step instance `i`, in increasing order from `0` through `num_steps - 1`, the step-instance preamble absorbs `b"vk"`, `b"num_circuits"`, `b"circuit_index"`, and `b"public_values"`. The validation body is then run for `S_step`.

```text
new transcript: TE::new(b"neutronnova_prove")
absorb(b"vk", vk_digest)
absorb(b"num_circuits", Scalar(num_steps))
absorb(b"circuit_index", Scalar(i))
absorb(b"public_values", step_i.public_values)

if S_step.num_shared > 0:
    absorb(b"comm_W_shared", comm_W_shared)
if S_step.num_precommitted > 0:
    absorb(b"comm_W_precommitted", comm_W_precommitted)
repeat S_step.num_challenges times:
    squeeze(b"challenge") -> checked against step_i.challenges
absorb(b"comm_W_rest", comm_W_rest)
```

The core instance uses the same transcript domain and validation body, but its preamble omits `b"num_circuits"` and `b"circuit_index"`. The body is run with `S_core` dimensions.

```text
new transcript: TE::new(b"neutronnova_prove")
absorb(b"vk", vk_digest)
absorb(b"public_values", core.public_values)

if S_core.num_shared > 0:
    absorb(b"comm_W_shared", comm_W_shared)
if S_core.num_precommitted > 0:
    absorb(b"comm_W_precommitted", comm_W_precommitted)
repeat S_core.num_challenges times:
    squeeze(b"challenge") -> checked against core.challenges
absorb(b"comm_W_rest", comm_W_rest)
```

## Phase B — Main setup

Phase B creates the single main transcript that is carried through Phase F. It binds the verifier-key digest, the regular core instance, every padded regular step instance, and the initial zero scalar `b"T"` before deriving \\(\tau\\) and the `rho` vector.

```text
new transcript: TE::new(b"neutronnova_prove")
absorb(b"vk", vk_digest)
absorb(b"core_instance", core_instance_regular)        // R1CSInstance
// step_instances_regular has num_steps entries; pad to padded_steps
// by repeating the first entry, step_instances_regular[0].
for each U in the padded step-instance list, in order:
    absorb(b"U", U)                                     // R1CSInstance
absorb(b"T", Scalar::ZERO)
squeeze(b"tau") -> tau
repeat num_rounds_b times:
    squeeze(b"rho") -> next rho
```

## Phase C — Verifier-instance rounds

Phase C validates the split multi-round verifier instance on the main transcript. Each round absorbs that round's witness commitment and then squeezes exactly that round's challenge count.

```text
for each round in 0..vc_shape.num_rounds, in order:
    absorb(b"comm_w_round", comm_w_per_round[round])
    repeat num_challenges_per_round[round] times:
        squeeze(b"challenge") -> checked against challenges_per_round[round]
```

The concatenated squeezed challenges are the in-circuit challenges `r_b | r_x | r | r_y`.

## Phase D — NIFS fold

Phase D performs the transcript part of `nifs.verify` on the main transcript. The concurrent fold performed alongside it does not touch the transcript.

```text
absorb(b"U1", random_U)                         // RelaxedR1CSInstance
absorb(b"U2", U_verifier_regular)               // R1CSInstance
absorb(b"comm_T", nifs.comm_T)
squeeze(b"r") -> folding challenge
```

## Phase E — Relaxed Spartan

Phase E verifies the relaxed Spartan proof on the main transcript. The first two absorbs bind the folded relaxed verifier instance, then `outer_rounds` squeezes derive the outer sumcheck evaluation point.

```text
absorb(b"u_relaxed", folded_U_verifier.u)
absorb(b"X_relaxed", folded_U_verifier.X)
repeat outer_rounds times:
    squeeze(b"t") -> next outer challenge
```

A sumcheck round absorbs the round's univariate polynomial as `b"p"`, encoded as a `UniPoly`, and then squeezes `b"c"`. The outer sumcheck uses degree bound `3` and runs for `outer_rounds` rounds.

```text
repeat outer_rounds times:
    absorb(b"p", round_poly)                    // UniPoly, degree bound 3
    squeeze(b"c") -> next outer sumcheck challenge
```

The transcript then binds the three outer claims, derives the inner-combination challenge, runs the inner sumcheck with degree bound `2`, and absorbs the witness and error evaluations.

```text
absorb(b"claims_outer", [claim_Az, claim_Bz, claim_uCzE])
squeeze(b"r") -> inner-combination challenge
repeat inner_rounds times:
    absorb(b"p", round_poly)                    // UniPoly, degree bound 2
    squeeze(b"c") -> next inner sumcheck challenge
absorb(b"v_W", v_W)
absorb(b"v_E", v_E)
```

## Phase F — PCS opening

Phase F derives the folded evaluation challenge, verifies the Hyrax opening, and then verifies the linear IPA subargument. The `b_vec` value is intentionally omitted from the IPA instance absorb because the verifier recomputes it from public transcript state and opening inputs.

```text
squeeze(b"c_eval") -> folded evaluation challenge

absorb(b"poly_com", comm)                       // folded HyraxCommitment

dom_sep(b"inner product argument (linear)")
absorb(b"U", ipa_instance)                       // comm_a_vec || comm_c; b_vec omitted
absorb(b"delta", delta)
absorb(b"beta", beta)
squeeze(b"r") -> IPA challenge
```

The encodings for `HyraxCommitment`, group elements, scalar slices, and IPA values are defined in [Byte encodings and serialization](../building-blocks/encodings.md). The polynomial-commitment context is described in [Polynomial commitments](../building-blocks/pcs.md).

## Complete ordered schedule

This block is the end-to-end reference summary. It preserves the ordering from Phase A through Phase F.

```text
Phase A: independent validation transcript for each step instance i = 0..num_steps - 1
    new transcript: TE::new(b"neutronnova_prove")
    absorb(b"vk", vk_digest)
    absorb(b"num_circuits", Scalar(num_steps))
    absorb(b"circuit_index", Scalar(i))
    absorb(b"public_values", step_i.public_values)
    if S_step.num_shared > 0:
        absorb(b"comm_W_shared", comm_W_shared)
    if S_step.num_precommitted > 0:
        absorb(b"comm_W_precommitted", comm_W_precommitted)
    repeat S_step.num_challenges times:
        squeeze(b"challenge")
    absorb(b"comm_W_rest", comm_W_rest)

Phase A: independent validation transcript for the core instance
    new transcript: TE::new(b"neutronnova_prove")
    absorb(b"vk", vk_digest)
    absorb(b"public_values", core.public_values)
    if S_core.num_shared > 0:
        absorb(b"comm_W_shared", comm_W_shared)
    if S_core.num_precommitted > 0:
        absorb(b"comm_W_precommitted", comm_W_precommitted)
    repeat S_core.num_challenges times:
        squeeze(b"challenge")
    absorb(b"comm_W_rest", comm_W_rest)

Phase B: main transcript setup
    new transcript: TE::new(b"neutronnova_prove")
    absorb(b"vk", vk_digest)
    absorb(b"core_instance", core_instance_regular)
    // pad step_instances_regular to padded_steps by repeating its first entry
    for each U in the padded step-instance list, in order:
        absorb(b"U", U)
    absorb(b"T", Scalar::ZERO)
    squeeze(b"tau")
    repeat num_rounds_b times:
        squeeze(b"rho")

Phase C: verifier-instance rounds on the main transcript
    for each round in 0..vc_shape.num_rounds, in order:
        absorb(b"comm_w_round", comm_w_per_round[round])
        repeat num_challenges_per_round[round] times:
            squeeze(b"challenge")

Phase D: NIFS fold on the main transcript
    absorb(b"U1", random_U)
    absorb(b"U2", U_verifier_regular)
    absorb(b"comm_T", nifs.comm_T)
    squeeze(b"r")

Phase E: relaxed Spartan on the main transcript
    absorb(b"u_relaxed", folded_U_verifier.u)
    absorb(b"X_relaxed", folded_U_verifier.X)
    repeat outer_rounds times:
        squeeze(b"t")
    repeat outer_rounds times:
        absorb(b"p", round_poly)       // outer sumcheck, degree bound 3
        squeeze(b"c")
    absorb(b"claims_outer", [claim_Az, claim_Bz, claim_uCzE])
    squeeze(b"r")
    repeat inner_rounds times:
        absorb(b"p", round_poly)       // inner sumcheck, degree bound 2
        squeeze(b"c")
    absorb(b"v_W", v_W)
    absorb(b"v_E", v_E)

Phase F: PCS opening on the main transcript
    squeeze(b"c_eval")
    absorb(b"poly_com", comm)
    dom_sep(b"inner product argument (linear)")
    absorb(b"U", ipa_instance)
    absorb(b"delta", delta)
    absorb(b"beta", beta)
    squeeze(b"r")
```

## What a conforming prover must reproduce

A byte-conforming prover must reproduce the exact operation stream: the same transcript domain label, the same `absorb`, `squeeze`, and `dom_sep` calls, the same labels, the same ordering, and the same value encodings. This is the transcript component of the conformance contract defined in [Scope and the conformance contract](./scope.md).

For adjacent byte-exact rules, see [Byte encodings and serialization](../building-blocks/encodings.md) for transcript input encodings, [The Fiat--Shamir transcript](../building-blocks/transcript.md) for the transcript primitive, [The verifier key](./verifier-key.md) for verifier-key dimensions and digest construction, and [The proof object](./proof-object.md) for proof fields consumed by this schedule.
