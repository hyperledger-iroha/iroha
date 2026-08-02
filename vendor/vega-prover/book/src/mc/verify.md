# Verification

This chapter states the canonical acceptance predicate for \\(\mathrm{Vega}\_{\mathrm{MC}}\\). It describes the verifier's ordered structural checks; byte-exact absorb and squeeze order is fixed separately in [the transcript schedule](../spec/transcript-schedule.md).

## Verifier interface

The verifier entry point has the shape

```rust
verify(&self, vk: &VegaMcVerifierKey<E>, num_instances: usize)
  -> Result<(Vec<Vec<E::Scalar>>, Vec<E::Scalar>), VegaError>
```

The proof is accepted exactly when every check below succeeds, in this order. Failure at any step rejects the proof. The returned value is the per-step public values and the core public values; these vectors are the public statement exposed by the proof-system layer.

## Canonical acceptance predicate

1. **Check the step count.** The supplied `num_instances` must be nonzero. It must equal the number of step instances carried by the proof, and it must equal the `num_steps` value bound into the verifier key. These checks prevent a proof for one step count from being replayed under another count.

2. **Restore the shared witness commitment.** The proof stores one shared witness commitment, `comm_W_shared`. The verifier attaches that same commitment to every step instance and to the core instance before any instance validation. After this point, each split instance is interpreted as if it had carried the shared commitment inline.

3. **Validate the split step and core instances.** Each step instance is validated against the step split shape. For step index \\(i\\), the verifier creates a fresh [Keccak Fiat--Shamir transcript](../building-blocks/transcript.md), absorbs the verifier-key digest, the number of step circuits, the circuit index, and that instance's public values, then calls the split-instance validation routine described in [R1CS and its variants](../building-blocks/r1cs.md). Validation checks public-value length, checks the commitments required by the shape, absorbs the early witness commitments, re-derives the stored Fiat--Shamir challenges, compares them exactly, and absorbs the rest commitment. The core instance is validated similarly against the core split shape, with the verifier-key digest and core public values as its public transcript context. After validation, the verifier checks that every step instance and the core instance carry the same `comm_W_shared`.

4. **Convert split instances to regular R1CS instances.** The step list is padded to a power of two by repeating the first step instance when necessary. Each padded step instance is converted to a regular instance by combining its split witness commitments and setting its public vector to public values followed by split challenges. The core instance is converted the same way, without step-count padding. This is the strict R1CS layer; relaxed R1CS appears only after the later Nova fold.

5. **Reconstruct the main protocol transcript.** The verifier starts the protocol transcript in the same domain as the prover, absorbs the verifier-key digest, the regular core instance, every regular step instance, and the scalar \\(T=0\\). It computes the padded step-folding round count, the step row round count, and the inner round count. It then draws \\(\tau\\) and the \\(\rho\\) challenges used by [NeutronNova folding](../building-blocks/neutronnova.md). This chapter records the structure; [the transcript schedule](../spec/transcript-schedule.md) fixes the byte-level labels and order.

6. **Validate the verifier-circuit instance and read its public input.** The proof's `U_verifier` is a multi-round split R1CS instance for the verifier circuit. The verifier validates it against `vc_shape`, using the live protocol transcript, then converts it to regular form. In regular form, the public vector is the flattened verifier-circuit challenges followed by the verifier-circuit public values, as described in [the in-circuit verifier](../building-blocks/in-circuit-verifier.md). The verifier requires the challenge prefix length to be

   \\[ \ell\_b + \ell\_x + 1 + \ell\_y, \\]

   where \\(\ell\_b\\) is the step-folding round count, \\(\ell\_x\\) is the outer row round count, and \\(\ell\_y\\) is the inner round count. The six public values must follow immediately, in this order:

   | Position | Value |
   | --- | --- |
   | 0 | `tau_at_rx` |
   | 1 | `eval_X_step` |
   | 2 | `eval_X_core` |
   | 3 | `eq_rho_at_rb` |
   | 4 | `quotient_step` |
   | 5 | `quotient_core` |

   The challenge prefix is parsed as \\(r\_b\\), then \\(r\_x\\), then the single bridge challenge \\(r\\), then \\(r\_y\\).

7. **Fold the step instances and verify the Nova fold.** Using \\(r\_b\\), the verifier calls `fold_multiple` on the regular step instances. This derives the NeutronNova folding weights and linearly combines the step public vectors and witness commitments into one folded step instance. In parallel with that computation, the verifier checks the [Nova folding for zero-knowledge](../building-blocks/nova-zk.md) proof: it absorbs the random relaxed instance, the regular verifier-circuit instance, and the cross-term commitment, squeezes the Nova fold challenge, and obtains the folded relaxed verifier-circuit instance. The folding background is in [NeutronNova folding](../building-blocks/neutronnova.md).

8. **Verify the relaxed-Spartan proof.** The verifier runs the [Relaxed Spartan](../building-blocks/relaxed-spartan.md) verifier over the folded relaxed verifier-circuit instance, using the regular verifier-circuit shape and verifier-circuit commitment key from the verifier key. This proves satisfaction of the masked folded verifier-circuit relation produced by the Nova fold.

9. **Pin the six soundness-binding public values natively.** The verifier recomputes the six public values outside the circuit and compares them with the six values exposed by `U_verifier`. This is the soundness-binding step.

   First, it evaluates the step and core matrix tables at the native verifier points. The row point is \\(r\_x\\), and the column point is the full \\(r\_y\\). For \\(\star \in \\{\mathrm{step},\mathrm{core}\\}\\), the verifier computes

   \\[ Q\_\star = \widetilde{A}\_\star(r\_x,r\_y) + r\\,\widetilde{B}\_\star(r\_x,r\_y) + r^2\\,\widetilde{C}\_\star(r\_x,r\_y). \\]

   The implementation names these values `quotient_step` and `quotient_core`. This quotient convention matches [the Spartan argument](../building-blocks/spartan.md): the batched matrix evaluation is `eval_A + r·eval_B + r²·eval_C`.

   Next, it evaluates the folded step public-input polynomial and the core public-input polynomial at \\(r\_y[1..]\\). Each public-input table is formed from the leading constant \\(1\\) followed by the regular instance public vector. It also computes `tau_at_rx` from the powers polynomial determined by \\(\tau\\) at \\(r\_x\\), and computes `eq_rho_at_rb` as the equality-polynomial value \\(\widetilde{\mathrm{eq}}(r\_b,\rho)\\).

   The verifier then applies an exact six-way equality gate:

   \\[ (\mathrm{tau\\_at\\_rx},\mathrm{eval\\_X\\_step},\mathrm{eval\\_X\\_core},\mathrm{eq\\_rho\\_at\\_rb},\mathrm{quotient\\_step},\mathrm{quotient\\_core})
   = (x\_0,x\_1,x\_2,x\_3,x\_4,x\_5). \\]

   Here \\((x\_0,\ldots,x\_5)\\) are the six public values read from `U_verifier`. Any mismatch rejects.

   This native recomputation is mandatory. The verifier circuit proves the folding and sum-check relations, but the six exposed openings must be tied to the actual verifier key, step instances, core instance, and transcript challenges held by the native verifier. The equality gate closes the soundness gap that would remain if the verifier trusted only the circuit-exposed values.

10. **Verify the final PCS opening.** The transcript supplies the folding challenge `c_eval`. The verifier folds the folded-step witness commitment with the core witness commitment using coefficients \\(1\\) and `c_eval`. It also folds the two verifier-circuit evaluation commitments with the same coefficients. The prover-side relation folds the matching blinds. The verifier then checks the zero-knowledge [Hyrax/IPA polynomial-commitment opening](../building-blocks/pcs.md) at the point \\(r\_y[1..]\\), using the folded witness commitment, the folded evaluation commitment, and `eval_arg`.

## Result

If all ten checks pass, verification returns the original per-step public values and the original core public values. Those values are not reinterpreted by the proof system; their application meaning is supplied by the circuits used in setup and proving. The surrounding proving flow is described in [Proving](./prove.md), and the privacy layer is described in [Zero knowledge](./zero-knowledge.md).
