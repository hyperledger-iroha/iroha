# Zero-knowledge

This chapter identifies where \\(\mathrm{Vega}\_{\mathrm{MC}}\\) obtains zero-knowledge in the online proof and why the deterministic algebraic checks do not reveal the witness. It complements the structural proving pipeline in [Proving](./prove.md) and the verifier behavior in [Verification](./verify.md).

## System zero-knowledge comes from masking

The verifier-circuit proof is protected by a fresh relaxed-R1CS mask. For each proof, the prover samples `random_U` and its witness as a full-length random satisfying relaxed instance for the verifier-circuit shape. The sampled error vector need not be zero; it is chosen so the relaxed relation is satisfied for the sampled witness, public values, and scalar \\(u\\).

The prover then invokes the Nova NIFS fold described in [Nova ZK folding](../building-blocks/nova-zk.md). The first input `U1` is the mask instance `random_U` with coefficient \\(1\\). The second input `U2` is the real verifier-circuit instance with coefficient \\(r\\), where \\(r\\) is squeezed after both inputs and the cross-term commitment are absorbed. The folded scalar fields have the form mask plus \\(r\\) times real data, so the folded relaxed instance is masked by fresh full-length randomness.

The folded instance is then proved with a [relaxed-Spartan direct opening](../building-blocks/relaxed-spartan.md). This relaxed-Spartan proof is deliberately non-hiding on its own: it opens linear combinations of the folded witness and folded error. It reveals nothing about the real verifier-circuit witness because those opened values belong to the masked fold. The proof stores `random_U`, and the verifier replays the same Nova NIFS fold with `random_U`, the real `U_verifier`, and the NIFS commitment.

## Witness-evaluation hiding comes from the PCS opening

The main Spartan checks need one committed-witness evaluation for the folded step branch and one for the core branch. The prover folds those two evaluation claims and opens the folded witness commitment at \\(r\_y[1..]\\). The opening argument is the [Hyrax PCS](../building-blocks/pcs.md) evaluation argument backed by the zero-knowledge linear IPA. The IPA samples fresh masking vector and scalar randomness before responding to the transcript challenge, so the opened evaluation is checked without exposing the committed witness vector.

## Folded sum-check transcript hiding

The NeutronNova and Spartan sum-check stages feed the in-circuit verifier through committed multi-round witnesses. Round polynomials and evaluation values are assigned inside the verifier-circuit witness; the transcript absorbs the corresponding commitments and then squeezes the next challenges. The public transcript therefore sees commitments to those values rather than the raw witness values. The generic algebra is described in [sum-check](../building-blocks/sumcheck.md), and the step-folding use of these rounds is described in [NeutronNova](../building-blocks/neutronnova.md).

## Deterministic stages are safe

After setup inputs, circuits, public values, and randomness are fixed, the online prover is deterministic: it builds the same split instances, derives the same Fiat--Shamir challenges, runs the same folding and sum-check reductions, and assembles the same proof fields. Fresh randomness is the only honest-prover divergence between two proofs of the same statement. It enters wherever the prover commits or opens with fresh hiding randomness: the rerandomized prepared state in [Preparation](./prep.md), the per-round commitments of the in-circuit verifier, the cross-term commitment of the Nova fold, the fresh relaxed mask `random_U`, and the zero-knowledge IPA opening.

This supports the byte-equivalence goal developed in [Specification scope](../spec/scope.md): an independent prover driven with identical inputs and identical randomness produces identical proof bytes. Different honest proofs of the same statement differ only because the rerandomization and zero-knowledge openings consume fresh randomness.
