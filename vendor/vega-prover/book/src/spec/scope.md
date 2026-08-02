# Scope and the conformance contract

This chapter is the entry point to the implementable specification for \\(\mathrm{Vega}\_{\mathrm{MC}}\\). It defines the conformance contract for an independently built prover and maps the chapters that pin the bytes, transcript, verifier key, proof object, reference prover, and test vectors.

## Canonical engine

The specification fixes one concrete instantiation: the T256 elliptic curve, a Keccak-based Fiat--Shamir transcript, and the Hyrax polynomial commitment scheme. This instantiation is the **canonical engine**. Other curve or engine choices may exist in an implementation, but they are outside this specification.

All proof-system arithmetic is over the scalar field \\(\mathbb{F}\\) of the canonical curve, a roughly 256-bit prime field defined in [Fields, groups, and the engine](../building-blocks/fields-and-groups.md). The group is \\(\mathbb{G}\\), and this part specifies the multi-circuit prover \\(\mathrm{Vega}\_{\mathrm{MC}}\\), the book's focus.

## Verifier acceptance is the ground truth

The verifier's decision procedure is the acceptance predicate. A proof is valid exactly when `verify` accepts it under the verifier key. `verify` takes the verifier key and the number of instances; it does not take the public values as an argument. Instead it recomputes them from the proof and returns them, and the application compares the returned values against the statement it intended to prove. A prover conforms at the acceptance level when every proof it emits for a satisfied statement is accepted by that verifier.

This chapter does not restate verification. [Verification](../mc/verify.md) gives the verifier procedure, and the following specification chapters pin every input the verifier consumes.

## Proof bytes and determinism

A proof is a structured value serialized to a byte string by the fixed deterministic serialization in [Serialization](serialization.md). Every serialized field is a deterministic function of four inputs:

- the verifier key;
- the public values;
- the witness;
- the prover's random tape.

These inputs are mediated by the fixed Fiat--Shamir transcript. Two provers that implement the same transcript schedule, use the same encodings, and consume identical randomness produce byte-identical proofs.

The only nondeterminism in an honest proof is zero-knowledge randomness: the commitment blinds and one freshly sampled random masking instance drawn once per proof. Because the honest prover samples this randomness fresh, two runs on the same statement normally produce different proof bytes, and both proofs verify. Byte equality is therefore relative to a fixed random tape; validity holds for any well-formed zero-knowledge randomness.

This gives two useful conformance notions:

- An **accept-conforming** prover emits proofs accepted by `verify`.
- A **byte-conforming** prover, when driven with the same inputs and identical randomness, reproduces the reference prover's exact serialized proof bytes.

## What must be reproduced

A conforming implementation must reproduce exactly:

1. the Fiat--Shamir transcript schedule: every absorbed label, absorbed value, absorbed encoding, and squeezed challenge, in order;
2. the proof object's contents and their deterministic serialization.

Nothing else is part of the conformance surface. Prover-internal choices such as delayed modular reduction, small-integer matrix--vector arithmetic, caches, memory layout, batching, and parallelism affect performance but not the emitted proof bytes or the transcript. A direct unoptimized prover can therefore be byte-equivalent to an optimized library while being easier to implement and audit.

Given the same verifier key, public values, witness, and random tape, exact agreement on the transcript schedule and serialized proof object yields the same verifier challenges and proof bytes. Given any well-formed zero-knowledge randomness, exact agreement on the transcript schedule and proof object yields proofs accepted by the canonical verifier.

## Specification map

The remaining chapters in this part fix the byte-exact contract:

- [Serialization](serialization.md) specifies the wire byte format, including integer, sequence, enum, field, and point encodings.
- [The transcript schedule](transcript-schedule.md) specifies the exact ordered sequence of transcript absorbs and squeezes.
- [The verifier key](verifier-key.md) specifies the verifier key contents and its digest.
- [The proof object](proof-object.md) specifies the proof's field-by-field byte layout.
- [The reference prover](reference-prover.md) specifies a direct unoptimized prover that realizes this specification.
- [Test vectors](test-vectors.md) specifies conformance test vectors and how to check them.

Two building-block chapters supply primitives used by this part: [Transcript primitive encodings](../building-blocks/encodings.md) specifies the values absorbed into transcripts, and [The Fiat--Shamir transcript](../building-blocks/transcript.md) specifies the transcript mechanism.

## Out of scope

This specification part does not give the verifier's soundness or zero-knowledge security arguments, which are treated conceptually in [Design goals and threat model](../overview/design-goals.md). It also excludes non-canonical engines and performance characteristics of any particular prover implementation.
