# A simple reference prover

The preceding specification chapters pin every byte the verifier consumes: the
[encodings](serialization.md), the [transcript schedule](transcript-schedule.md),
the [verifier key](verifier-key.md), and the [proof object](proof-object.md). This
chapter supplies the last piece: a complete, runnable prover. Rather than restate
the procedure in prose, it points at a working implementation.

> **Status — preliminary.** This reference is an executable specification and
> teaching aid, not a production prover. It currently proves only the fixed cubic
> circuit below, favors clarity over speed, and is validated by mutual acceptance
> with the shipped Rust prover and verifier — but it has not had an independent
> security audit, and its internals may still change.

## The implementation is the specification

The repository ships a small **pure-Python reference implementation** under
[`reference/pyvega`](https://github.com/Microsoft/vega-prover/tree/main/reference).
It omits the performance optimizations in the shipped Rust library — delayed modular
reduction, small-value integer arithmetic, multi-scalar-multiplication caches, and
parallelism — and computes each value in the most direct way. It is slower than
the shipped Rust prover, but its control flow follows the protocol step by step.

A written algorithm can drift from the code it describes; a reference prover
cannot, because it is executed and checked against the shipped Rust verifier. The prose in
this book explains the design and the wire format; the reference prover is the
procedure of record.

## Conformance is mutual acceptance, not byte identity

An honest \\(\mathrm{Vega}\_{\mathrm{MC}}\\) proof is randomized: it carries
commitment blinds and one freshly sampled random masking instance drawn once per
proof (see [Zero-knowledge](../mc/zero-knowledge.md)). Two honest runs on the same
statement therefore emit *different* bytes, and both verify. Conformance between
two implementations is consequently defined at the level of **verifier
acceptance**, in both directions:

- a proof from the shipped Rust prover is accepted by the reference verifier, and
- a proof from the reference prover — under a verifier key that the reference
  setup itself produced — is accepted by the shipped Rust verifier.

The reference implementation establishes both directions on the canonical cubic
example; the mechanics are described in [Conformance and test
vectors](test-vectors.md).

## What the reference covers

The reference implements the entire acceptance path end to end, so an implementer
can read a working counterpart to each specification chapter:

| Concern | Book chapter | Reference module |
| --- | --- | --- |
| Field and group arithmetic | [Fields, groups, and the engine](../building-blocks/fields-and-groups.md) | `field.py`, `curve.py`, `params.py` |
| Byte encodings | [Serialization](serialization.md) | `codec.py`, `field.py`, `curve.py` |
| Fiat--Shamir transcript primitive | [The Fiat--Shamir transcript](../building-blocks/transcript.md) | `transcript.py` |
| Multilinear / eq / sparse polynomials | [Multilinear polynomials](../building-blocks/multilinear.md) | `polys.py` |
| Sum-check | [Sum-check](../building-blocks/sumcheck.md) | `sumcheck.py` |
| R1CS and its instances | [R1CS](../building-blocks/r1cs.md) | `instance.py` |
| Commitments and the linear IPA | [Polynomial commitments](../building-blocks/pcs.md) | `commitment.py`, `hyrax.py` |
| NeutronNova / Nova folding | [NeutronNova folding](../building-blocks/neutronnova.md), [Nova folding](../building-blocks/nova-zk.md) | `nifs.py` |
| Relaxed Spartan | [Relaxed Spartan](../building-blocks/relaxed-spartan.md) | `spartan.py` |
| In-circuit verifier | [The in-circuit verifier](../building-blocks/in-circuit-verifier.md) | `verifier_circuit.py` |
| Verifier key | [Verifier key](verifier-key.md) | `vk.py`, `setup.py` |
| Proof object | [Proof object](proof-object.md) | `proof.py`, `prover_serialize.py` |
| Verification | [Verification](../mc/verify.md) | `verify.py` |
| Setup, prove | [Setup](../mc/setup.md), [Prove](../mc/prove.md) | `setup.py`, `prover.py`, `prover_finish.py` |

## The worked circuit

To stay self-contained and hand-checkable, the reference proves the tiny cubic
relation \\(y = x^3 + x + 5\\) with \\(x = 2\\), \\(y = 15\\) — the same
`CubicCircuit` the Rust test suite uses. Its R1CS has four constraints, yet
proving it drives every phase of the protocol: the in-circuit verifier, both folding
schemes, both sum-checks, relaxed Spartan, and the zero-knowledge opening. An
implementer who reproduces acceptance on this example has therefore exercised every
protocol phase, though only at this fixture's dimensions — two step instances and
single-row witness commitments — not for arbitrary circuits.

Because setup is also implemented in Python (`setup.py` generates the Hyrax
generators by hash-to-curve and serializes the verifier key), the reference runs
with no dependency on the shipped Rust library at all: it performs setup, proving,
and verification itself, and the shipped Rust verifier independently accepts the
result.
