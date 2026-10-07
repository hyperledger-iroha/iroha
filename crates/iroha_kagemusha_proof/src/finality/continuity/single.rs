//! One source-qualified hard verifier with inseparable two-curve obligations.

use super::*;
use iroha_pasta::{Fq, msm::MemoryBudget};
use iroha_plonk::verifier::{accumulate_generator, verify_full};
use iroha_plonk_recursion::{AccumulatorT, FoldInput};

/// Original source claims bound by its fixed wrapper's hard verifier.
/// Construction is private; the owning stage must fold both Pallas obligations
/// and export the Vesta claim to its mandatory wrapper.
#[derive(Clone, Debug)]
pub struct VerifiedSourceCells {
    pallas: FoldInputCells<Ep>,
    opening: FoldInputCells<Ep>,
    vesta: VestaClaimCells,
}

impl VerifiedSourceCells {
    pub(crate) const fn pallas(&self) -> &FoldInputCells<Ep> {
        &self.pallas
    }
    pub(crate) const fn opening(&self) -> &FoldInputCells<Ep> {
        &self.opening
    }
    pub(crate) const fn vesta(&self) -> &VestaClaimCells {
        &self.vesta
    }
}

/// Verify a canonical key supplied by the owning relation, without authorizing it.
/// The closed history owner binds the returned complete digest to its unchanged
/// prefix context; every other owner supplies its source-qualified fixed key.
pub fn verify_key_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    plan: &VerifierPlan<Ep>,
    child: SourceChild<'_>,
    assign_key: impl FnOnce(
        &mut VerifierChip<Ep>,
        &mut Region<'_, Fp>,
    ) -> Result<iroha_plonk_recursion::verifier::VerifierKeyCells<Ep>, Error>,
) -> Result<(VerifiedSourceCells, Word<Fp>), Error> {
    if child.vesta.source_k() != 16 {
        return Err(Error::Synthesis);
    }
    let digest = binding_digest(chip, region, child.endpoints, child.pallas)?;
    let digest = ScalarCells::from_native_word(&mut chip.uint(), region, &digest)?;
    let challenges = child
        .vesta
        .challenges()
        .iter()
        .map(|word| ScalarCells::from_native_word(&mut chip.uint(), region, word))
        .collect::<Result<Vec<_>, _>>()?;
    let instances = vec![vec![digest], child.vesta.coordinates().to_vec(), challenges];
    let key = assign_key(chip, region)?;
    let verified = chip.verify(
        region,
        plan,
        &key,
        &instances,
        child.proof.messages(),
        child.proof.length(),
        VerificationMode::Hard,
    )?;
    Ok((
        VerifiedSourceCells {
            pallas: child.pallas.clone(),
            opening: FoldInputCells::from_claim(chip, region, &verified.claim)?,
            vesta: child.vesta.clone(),
        },
        verified.key_digest,
    ))
}

/// Verify the exact original proof and decide both carried generator claims.
pub fn verify_key_native(
    plan: &VerifierPlan<Ep>,
    key: &VerifyingKey<Ep>,
    evidence: &SourceNodeEvidence,
    vesta: &PinnedParams<Eq>,
    budget: MemoryBudget,
) -> Result<FoldInput<Ep>, Error> {
    if vesta.k() != 16 || evidence.proof.len() != plan.proof_length() {
        return Err(Error::Synthesis);
    }
    leaf_frame_native(evidence.endpoints)?;
    evidence
        .pallas
        .decide(plan.params(), budget)
        .map_err(|_| Error::Synthesis)?;
    evidence
        .vesta
        .decide(vesta, budget)
        .map_err(|_| Error::Synthesis)?;
    let instances = evidence.instances()?;
    verify_full(
        plan.params(),
        plan.binding(),
        key,
        &instances,
        &evidence.proof,
        budget,
    )
    .map_err(|_| Error::Synthesis)?;
    let claim = accumulate_generator(
        plan.params(),
        plan.binding(),
        key,
        &instances,
        &evidence.proof,
        budget,
    )
    .map_err(|_| Error::Synthesis)?;
    let opening =
        FoldInput::from_opening(*claim.g(), claim.challenges()).map_err(|_| Error::Synthesis)?;
    opening
        .decide(plan.params(), budget)
        .map_err(|_| Error::Synthesis)?;
    Ok(opening)
}

impl SourceVerifier {
    /// Exact qualified wrapper descriptor; this does not permit key substitution.
    pub fn binding(&self) -> &iroha_plonk::DescriptorBinding {
        self.verifier.binding()
    }
    /// Exact immutable wrapper verifier from this sealed source qualification.
    /// Reading metadata does not permit construction or substitution of a source owner.
    pub const fn verifying_key(&self) -> &VerifyingKey<Ep> {
        &self.key
    }
    /// Complete digest of the original qualified wrapper key.
    /// # Errors
    /// Invalid original descriptor/key binding.
    pub fn key_digest(&self) -> Result<Fp, Error> {
        self.key
            .kagemusha_digest(self.verifier.binding())
            .map_err(|_| Error::Synthesis)
    }
    #[cfg(test)]
    pub(crate) fn layout_metadata(&self) -> (VerifierPlan<Ep>, VerifyingKey<Ep>) {
        (self.verifier.clone(), self.key.clone())
    }
    /// Hard verify the exact source endpoint/P binding and original V columns.
    pub(crate) fn verify_cells(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        child: SourceChild<'_>,
    ) -> Result<VerifiedSourceCells, Error> {
        verify_key_cells(chip, region, &self.verifier, child, |chip, region| {
            chip.constant_key(region, &self.verifier, &self.key)
        })
        .map(|(verified, _)| verified)
    }

    /// Recheck all original evidence and decide every carried native obligation.
    pub(crate) fn verify_native(
        &self,
        evidence: &SourceNodeEvidence,
        vesta: &PinnedParams<Eq>,
        budget: MemoryBudget,
    ) -> Result<FoldInput<Ep>, Error> {
        verify_key_native(&self.verifier, &self.key, evidence, vesta, budget)
    }

    /// Untrusted exact-size filler, used only by witnessless source import.
    pub(crate) fn blank_evidence(&self) -> Result<SourceNodeEvidence, Error> {
        Ok(SourceNodeEvidence {
            endpoints: [Fp::ZERO; 6],
            proof: vec![0; self.verifier.proof_length()],
            pallas: AccumulatorT::new(
                decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?,
                [Fq::ONE; 16],
            )
            .map_err(|_| Error::Synthesis)?,
            vesta: AccumulatorT::new(
                decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?,
                [Fp::ONE; 16],
            )
            .map_err(|_| Error::Synthesis)?,
        })
    }
}
