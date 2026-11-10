//! The folded lineage's segment rebase ([`AKind::Rebase`], DESIGN.md §8): a
//! segment's first A step. It folds the segment's first Q instance (a Frame
//! step without predecessor), requires the segment base state
//! ([`segment_base`]) and hard-verifies the wallet's last published Omega as
//! today's stage A1 verifies its predecessor (`verify_predecessor`: `D_A` over
//! the lineage fields and the transported `acc_P`, the instance
//! `[D_A, acc_V]`, the Omega key digest bound to the lineage's). The segment
//! then continues at that lineage with no context, as after a completed
//! operation.
//!
//! The Omega's deferred claims enter the claim accumulators. Its transported
//! `acc_P` and its proof opening (both Pallas claims) start A's; its
//! transported `acc_V` leaves in the export column in mode Accept, and the
//! Frame step folding this instance folds it into Q's. The hard verifier
//! leaves no burn branch: an Omega the prover cannot verify has no Rebase.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    circuit::{
        FoldLanes,
        ivc::{fold_frame, frame_outputs},
    },
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Error as SynthesisError, Layouter, Region, SimpleFloorPlanner},
    transcript::generator_coordinates,
};
use iroha_plonk_gadgets::{GlueChip, Word, bytes::tape::BytesChip, statement::foreign_limbs};
use iroha_plonk_recursion::{AccumulatorT, FoldInput, verifier::VerifierChip};

use super::{AStateCells, AStepConfig, Assign, Error, receive::ReceiveFoldPlan};
use crate::{
    a_relation::{LINEAGE_FIELDS, LineagePublicCells, verify_predecessor},
    fold_lineage::{
        A_CLAIM_SLOT, A_EXPORT, A_STATE, AKind, BASE, CLAIM_A, ClaimFolds, REBASE_PC, claim_cells,
        export_words, fold_claims_cells, trivial_vesta,
    },
    q_sigma::native::IncomingMode,
};

/// The originals of a segment rebase: the wallet's last published Omega.
#[derive(Clone, Debug)]
pub struct RebaseSource {
    /// Its lineage public fields (as `D_A` hashes them).
    pub lineage: [Fp; LINEAGE_FIELDS],
    /// Its transported `acc_P`.
    pub pallas: AccumulatorT<Ep>,
    /// Its transported `acc_V`.
    pub vesta: AccumulatorT<Eq>,
    /// Its proof bytes.
    pub proof: Vec<u8>,
    /// The opening claim its verification defers, natively.
    pub opening: FoldInput<Ep>,
}

impl RebaseSource {
    /// A source of the plan's shapes with zero values and trivial claims, for
    /// key generation.
    ///
    /// # Errors
    /// [`Error::Artifact`] for a plan without an Omega program.
    pub fn blank(plan: &ReceiveFoldPlan) -> Result<Self, Error> {
        let omega = plan.context().operation().omega().ok_or(Error::Artifact)?;
        let pallas = plan.trivial_pallas().clone();
        Ok(Self {
            lineage: [Fp::ZERO; LINEAGE_FIELDS],
            pallas: AccumulatorT::new(*pallas.g(), *pallas.challenges())
                .map_err(|_| Error::Artifact)?,
            vesta: plan.trivial_vesta().clone(),
            proof: vec![0; omega.proof_length()],
            opening: pallas,
        })
    }

    /// The A state after the rebase: the lineage, no context, no sub-step done.
    #[must_use]
    pub fn next_state(&self) -> Vec<Fp> {
        let mut state = self.lineage.to_vec();
        state.extend([Fp::ZERO, Fp::ZERO]);
        state
    }

    /// The Pallas claims the rebase folds into A's claim accumulator, in
    /// order: the transported `acc_P`, then the proof opening.
    #[must_use]
    pub fn pallas_claims(&self) -> [FoldInput<Ep>; 2] {
        [self.pallas.as_input(), self.opening.clone()]
    }

    /// The claim the rebase hands to the Q side: the transported `acc_V`.
    #[must_use]
    pub fn vesta_claim(&self) -> FoldInput<Eq> {
        self.vesta.as_input()
    }

    /// Its export column ([`A_EXPORT`]): the transported `acc_V` in mode
    /// Accept (the correction slot holds the trivial generator).
    #[must_use]
    pub fn export(&self) -> Vec<Fp> {
        export_words(
            &self.vesta.as_input(),
            IncomingMode::Accept,
            trivial_vesta().g(),
        )
    }
}

/// The witness of a Rebase step.
#[derive(Clone, Debug)]
pub struct RebaseStepWitness {
    /// The predecessor Omega.
    pub source: Arc<RebaseSource>,
    /// The frame: the segment's first Q instance and the accumulator it starts.
    pub frame: FrameWitness<Ep>,
    /// The folds of the two Pallas claims into A's claim accumulator.
    pub claims: ClaimFolds<Ep>,
}

/// The Rebase step.
#[derive(Clone, Debug)]
pub struct RebaseFoldStep {
    plan: Arc<ReceiveFoldPlan>,
    shape: FoldShape,
    witness: Option<RebaseStepWitness>,
}

impl RebaseFoldStep {
    /// The Rebase folding instances of the Q relation of `shape` (the Frame
    /// step's), under `plan`'s Omega program and key.
    #[must_use]
    pub const fn new(
        plan: Arc<ReceiveFoldPlan>,
        shape: FoldShape,
        witness: Option<RebaseStepWitness>,
    ) -> Self {
        Self {
            plan,
            shape,
            witness,
        }
    }

    fn layout(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<(Vec<Word<Fp>>, Vec<Word<Fp>>), SynthesisError> {
        let assign = Assign {
            known: self.witness.is_some(),
        };
        let blank;
        let source = match &self.witness {
            Some(witness) => witness.source.as_ref(),
            None => {
                blank = RebaseSource::blank(&self.plan).map_err(|_| SynthesisError::Synthesis)?;
                &blank
            }
        };
        let chain = AKind::Rebase.chain();
        let frame = {
            let refs = chip.fold_lanes()?;
            let mut lanes = FoldLanes {
                glue: refs.glue,
                range: refs.range,
                ff: refs.ff,
                ecc: refs.ecc,
                duplex: refs.hash,
            };
            fold_frame(
                &mut lanes,
                region,
                &chain,
                &self.shape,
                Fp::from(BASE),
                Fp::ZERO,
                A_STATE,
                self.witness.as_ref().map(|w| &w.frame),
            )?
        };
        // Only a segment's base state is rebased: no lineage, no context and
        // the rebase program counter, which no operation's sub-step writes.
        let state = AStateCells::from_frame(&frame.state)?;
        for word in &state.lineage {
            GlueChip::assert_constant(region, word, Fp::ZERO)?;
        }
        GlueChip::assert_constant(region, &state.context, Fp::ZERO)?;
        GlueChip::assert_constant(region, &state.pc, Fp::from(REBASE_PC))?;
        // The predecessor Omega, hard, as today's stage A1 verifies it.
        let fields = assign.words(chip, region, &source.lineage)?;
        let public = LineagePublicCells::constrain(&mut chip.uint(), region, &fields)?;
        let pallas = assign.pallas(chip, region, &source.pallas.as_input())?;
        let vesta = assign.vesta(chip, region, &source.vesta)?;
        let key = assign.omega_key(chip, region, self.plan.omega_key())?;
        let proof = assign.proof(chip, region, &source.proof)?;
        let predecessor = verify_predecessor(
            chip,
            region,
            self.plan.context().operation(),
            &key,
            &public,
            &public,
            &pallas,
            &vesta,
            &proof,
        )?;
        let export = export_cells(chip, region, &predecessor.vesta.words())?;
        // Its Pallas claims start A's claim accumulator.
        let values = self.witness.as_ref().map(|w| w.source.pallas_claims());
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes {
            glue: refs.glue,
            range: refs.range,
            ff: refs.ff,
            ecc: refs.ecc,
            duplex: refs.hash,
        };
        let claims = [
            claim_cells(
                &mut lanes,
                region,
                &predecessor.pallas,
                values.as_ref().map(|v| &v[0]),
            )?,
            claim_cells(
                &mut lanes,
                region,
                &predecessor.opening,
                values.as_ref().map(|v| &v[1]),
            )?,
        ];
        let digests = fold_claims_cells(
            &mut lanes,
            region,
            (&CLAIM_A, A_CLAIM_SLOT),
            &frame.digests,
            &claims,
            self.witness.as_ref().map(|w| &w.claims),
        )?;
        let zero = lanes.glue.constant(region, Fp::ZERO)?;
        let mut next = predecessor.public.to_vec();
        next.extend([zero.clone(), zero]);
        let outputs = frame_outputs(&mut lanes, region, &chain, &frame, &next, &digests, &[])?;
        Ok((outputs, export))
    }
}

/// The export column of the Rebase: the hard-verified `acc_V`'s words, mode
/// Accept and the trivial generator's coordinate limbs.
fn export_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    vesta: &[Word<Fp>],
) -> Result<Vec<Word<Fp>>, SynthesisError> {
    let mut words = vesta.to_vec();
    for bit in [1u64, 0, 0] {
        words.push(chip.uint().glue().constant(region, Fp::from(bit))?);
    }
    let (x, y) = generator_coordinates::<Eq>(trivial_vesta().g());
    for v in [x, y] {
        for limb in foreign_limbs(&v) {
            words.push(chip.uint().glue().constant(region, Fp::from_u128(limb))?);
        }
    }
    if words.len() == A_EXPORT {
        Ok(words)
    } else {
        Err(SynthesisError::Synthesis)
    }
}

impl Circuit<Fp> for RebaseFoldStep {
    type Config = AStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            witness: None,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> AStepConfig {
        AStepConfig::configure(meta)
    }

    fn synthesize(
        &self,
        config: AStepConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), SynthesisError> {
        let mut chip = VerifierChip::new(config.verifier);
        let bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let (outputs, export) = layouter.assign_region(
            || "folded lineage rebase",
            |mut region| self.layout(&mut chip, &mut region),
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        for (row, word) in export.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.export, row)?;
        }
        Ok(())
    }
}
