# Proving

This chapter describes the online proving pipeline for \\(\mathrm{Vega}\_{\mathrm{MC}}\\). It follows the structural order of the prover: what each stage constructs, what the transcript binds, and which building block carries the algebra. The exact byte order for transcript absorbs, squeezes, and serialization belongs to [Transcript schedule](../spec/transcript-schedule.md) and [Serialization](../spec/serialization.md).

## Ordered pipeline

### 1. Rerandomize and validate the prepared state

The prover consumes the prepared state from [Preparation](./prep.md) and rerandomizes it in place. The core prepared state receives fresh commitment randomness, and each step prepared state is rerandomized with the same shared witness commitment and blind. The prover then checks that the number of step circuits matches the prepared step state. If the matrix-vector cache is active, it also recomputes each step circuit's public values and rejects stale cached public data.

### 2. Build split R1CS instances and witnesses

For every step circuit, the prover opens a per-instance [Fiat--Shamir transcript](../building-blocks/transcript.md), absorbs the verifier-key digest, the number of step circuits, the step circuit index, and that step circuit's public values. The core branch opens the same transcript domain, absorbs the verifier-key digest and the core public values, and omits the step count and index. Each branch then invokes the split [R1CS](../building-blocks/r1cs.md) assignment machinery to commit the round witnesses and produce a `SplitR1CSInstance` with its witness. The shared witness commitment is stored once and reused by all step and core instances.

### 3. Open the main protocol transcript

The main protocol transcript starts in the same domain and absorbs the verifier-key digest and the regularized core instance. The step instances are absorbed by the NeutronNova folding procedure, followed by the initial cross-term target \\(T = 0\\). The folding procedure then squeezes \\(\tau\\) and the sequence of \\(\rho\\) challenges that parameterize step-instance folding. Byte-level labels and ordering are specified in [Transcript schedule](../spec/transcript-schedule.md).

### 4. Fold all step instances with NeutronNova

[Vega-MC NeutronNova folding](../building-blocks/neutronnova.md) folds the many regular step instances into one folded step instance and one folded step witness. During the fold, the prover records the folding sum-check polynomials in the verifier-circuit witness, computes the accumulated equality value \\(\mathrm{eq\\_rho\\_at\\_rb}\\), and carries the folded step error through the target term \\(\mathrm{t\\_out\\_step}\\).

### 5. Run the outer sum-check

The prover prepares multilinear tables for the folded step branch and for the core branch. For the step branch it uses the folded \\(Az\\), \\(Bz\\), and \\(Cz\\) layers returned by NeutronNova; for the core branch it multiplies the core shape by the core witness, unit coordinate, public values, and challenges. It then invokes the batched cubic [Spartan](../building-blocks/spartan.md) outer reduction, with an additive step target, through the generic [sum-check](../building-blocks/sumcheck.md). This binds \\(r\_x\\), records the claimed \\(Az\\), \\(Bz\\), and \\(Cz\\) evaluations for step and core, and records \\(\mathrm{tau\\_at\\_rx}\\).

### 6. Process the verifier-circuit batching round

The prover advances the [in-circuit verifier](../building-blocks/in-circuit-verifier.md) by one round after the outer sum-check. That round absorbs the committed verifier-circuit witness data and squeezes the batching challenge \\(r\\). The prover forms the two inner joint claims
\\[ \mathrm{claim\\_inner\\_joint}\_\star = \mathrm{eval\\_A}\_\star + r \\, \mathrm{eval\\_B}\_\star + r^2 \\, \mathrm{eval\\_C}\_\star \\]
for \\(\star \in \{\mathrm{step}, \mathrm{core}\}\\).

### 7. Run the inner sum-check

The prover binds the row variables at \\(r\_x\\), constructs the joint matrix-evaluation tables, and builds the folded-step and core \\(z\\) vectors. It invokes the batched quadratic Spartan inner reduction to bind \\(r\_y\\). The result yields \\(\mathrm{eval\\_X}\\) and \\(\mathrm{eval\\_W}\\) for the folded step branch and for the core branch. The first coordinate \\(r\_y[0]\\) separates the witness half from the public-value half, and the remaining point \\(r\_y[1..]\\) is the committed-witness evaluation point.

### 8. Finalize the verifier-circuit instance

The prover feeds the inner final-equality round and the two witness-evaluation commitment rounds into `VegaMcVerifierCircuit`. The finalized `U_verifier` is a `SplitMultiRoundR1CSInstance` whose public values are, in order, \\(\mathrm{tau\\_at\\_rx}\\), \\(\mathrm{eval\\_X\\_step}\\), \\(\mathrm{eval\\_X\\_core}\\), \\(\mathrm{eq\\_rho\\_at\\_rb}\\), \\(\mathrm{quotient\\_step}\\), and \\(\mathrm{quotient\\_core}\\), where \\(\mathrm{quotient}\_\star = \mathrm{eval\\_A}\_\star + r \\, \mathrm{eval\\_B}\_\star + r^2 \\, \mathrm{eval\\_C}\_\star\\). The prover also converts this split multi-round instance to a regular R1CS instance for the zero-knowledge fold.

### 9. Apply the zero-knowledge mask fold

For the verifier-circuit regular shape, the prover samples a fresh random satisfying relaxed instance and witness with `sample_random_instance_witness`. It then invokes the Nova NIFS zero-knowledge fold from [Nova ZK folding](../building-blocks/nova-zk.md): `U1` is the random mask `random_U` with coefficient \\(1\\), and `U2` is the real verifier-circuit instance with coefficient \\(r\\). This stage produces the NIFS proof and the folded relaxed verifier-circuit witness described in [Zero-knowledge](./zero-knowledge.md).

### 10. Prove the folded verifier instance with relaxed Spartan

The prover invokes the [relaxed-Spartan direct opening](../building-blocks/relaxed-spartan.md) proof on the folded verifier-circuit relaxed instance. This proof establishes satisfiability of the folded instance. It is non-hiding by itself, and its zero-knowledge role depends on the fresh random mask folded in the previous stage.

### 11. Open the witness evaluation with ZK PCS

The verifier circuit committed separately to \\(\mathrm{eval\\_W\\_step}\\) and \\(\mathrm{eval\\_W\\_core}\\). After those commitments are already bound to the transcript, the prover squeezes \\(c\_\mathrm{eval}\\), folds the step and core witness commitments, folds their blinds, folds the witness vectors, and folds the two evaluation commitments and blinds with coefficients \\(1\\) and \\(c\_\mathrm{eval}\\). It then opens the folded commitment at \\(r\_y[1..]\\) using the [Hyrax PCS with ZK linear IPA](../building-blocks/pcs.md), producing `eval_arg`.

### 12. Assemble the proof and return reusable state

The prover extracts the shared witness commitment, removes that shared commitment from each stored step and core split instance, and assembles the eight proof fields: `comm_W_shared`, `step_instances`, `core_instance`, `eval_arg`, `U_verifier`, `nifs`, `random_U`, and `relaxed_snark`. The method returns the proof together with the consumed prepared state, which can be reused after another rerandomization pass.

## Neighboring chapters

For the inputs to this pipeline, see [Setup](./setup.md) and [Preparation](./prep.md). For the verifier's reconstruction of the same transcript and checks, see [Verification](./verify.md). For the hiding argument behind the deterministic stages, see [Zero-knowledge](./zero-knowledge.md).
