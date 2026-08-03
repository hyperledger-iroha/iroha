# Protocol overview

Part II defined each primitive in isolation. This part assembles them into the end-to-end multi-circuit prover \\(\mathrm{Vega}\_{\mathrm{MC}}\\): setup fixes the keys, proving turns a batch of circuit instances into a single zero-knowledge proof, and verification defines exactly when that proof is accepted. The runtime phases are summarized in [The proving lifecycle](../overview/lifecycle.md); the chapters here give the structural detail, one stage at a time. Byte-level formats are deferred to the [implementable specification](../spec/scope.md).

## What the protocol proves

A statement consists of \\(n\\) instances of a step circuit \\(C\_1\\) together with one instance of a core circuit \\(C\_2\\). The core circuit joins the batch: every step instance and the core instance share one committed witness. The prover convinces the verifier that it knows satisfying assignments for all \\(n+1\\) instances, revealing nothing beyond the public inputs and outputs that verification returns. The batch size \\(n\\) is fixed at setup and must be at least two; a single instance is the domain of the single-circuit prover \\(\mathrm{Vega}\_{\mathrm{SC}}\\).

## The four stages

The protocol is exposed as four operations, each with its own chapter:

- [Setup](./setup.md) synthesizes the circuit shapes and produces the prover and verifier keys.
- [Rerandomizable precomputation](./prep.md) builds the reusable committed prover state that proving rerandomizes into a fresh proof.
- [Proving](./prove.md) folds the batch and runs the sum-checks, emitting the proof object below.
- [Verification](./verify.md) is the canonical acceptance predicate.

Zero-knowledge cuts across proving and verification and is treated on its own in [Zero-knowledge](./zero-knowledge.md).

## How the building blocks compose

Proving threads the batch through the building blocks of Part II in a fixed order.

The prover first forms a [split R1CS](../building-blocks/r1cs.md) instance and witness for each step circuit and for the core circuit. All of them reuse a single shared witness commitment, so the batch commits its common data once. [NeutronNova folding](../building-blocks/neutronnova.md) then accumulates the \\(n\\) step instances into one folded step instance, using transcript-derived challenges \\(\tau\\) and \\(\rho\\).

The folded-step branch and the core branch are reduced by the [Spartan](../building-blocks/spartan.md) outer and inner [sum-checks](../building-blocks/sumcheck.md), which bind the challenge vectors \\(r\_x\\) and \\(r\_y\\) and produce six algebraic values: the power-polynomial evaluation \\(\mathrm{tau\\_at\\_rx}\\), the public-input evaluations \\(\mathrm{eval\\_X\\_step}\\) and \\(\mathrm{eval\\_X\\_core}\\), the equality evaluation \\(\mathrm{eq\\_rho\\_at\\_rb}\\), and the matrix-quotient values \\(\mathrm{quotient\\_step}\\) and \\(\mathrm{quotient\\_core}\\). These six values are exactly what the [in-circuit verifier](../building-blocks/in-circuit-verifier.md) exposes as public output.

The in-circuit verifier instance is then made zero-knowledge by [Nova folding](../building-blocks/nova-zk.md): the prover samples a fresh random satisfying instance and folds it with the real verifier instance, masking it. The masked instance is proved with [relaxed Spartan](../building-blocks/relaxed-spartan.md), and the committed-witness evaluation the checks need — folded across the step and core branches — is opened by the zero-knowledge [linear IPA](../building-blocks/pcs.md) at the point \\(r\_y[1..]\\).

Two distinct folding schemes appear, for two distinct purposes:

- NeutronNova folding accumulates the many step instances into one, shrinking the batch to a single instance to prove.
- Nova folding masks the in-circuit verifier instance for zero-knowledge, folding it with a fresh random instance so nothing about the real witness leaks.

## The proof object

Proving emits one object whose fields carry everything verification needs:

| Field | Meaning |
| --- | --- |
| `comm_W_shared` | the one shared witness commitment reused by every step and the core instance |
| `step_instances` | the public parts of the \\(n\\) step split R1CS instances |
| `core_instance` | the public part of the core split R1CS instance |
| `eval_arg` | the zero-knowledge IPA opening of the folded step-and-core witness evaluation |
| `U_verifier` | the in-circuit verifier instance exposing the six public values |
| `nifs` | the Nova folding proof that folds the random mask with `U_verifier` |
| `random_U` | the fresh random satisfying instance used to mask `U_verifier` |
| `relaxed_snark` | the relaxed-Spartan proof over the folded verifier instance |

The exact byte layout of these fields is specified in [The proof object](../spec/proof-object.md).

## The acceptance predicate at a glance

Verification mirrors proving. It rebuilds the Fiat--Shamir transcript from the same absorbs, revalidates the split instances, refolds the batch, and checks the Nova fold and the relaxed-Spartan proof. It then recomputes the six public values in the clear from the keys and instances it holds and rejects unless each one matches the value the circuit exposed, and finally verifies the witness opening. That native recomputation is what ties the in-circuit argument to the real statement; [Verification](./verify.md) gives the full ordered checklist.

## Reading guide

The remaining chapters follow the stages in order: [Setup](./setup.md), [Rerandomizable precomputation](./prep.md), [Proving](./prove.md), [Verification](./verify.md), and [Zero-knowledge](./zero-knowledge.md).
