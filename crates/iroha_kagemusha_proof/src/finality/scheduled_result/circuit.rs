//! Hard verification and retained two-curve obligations for the scheduled result.

use super::*;
use crate::finality::{
    certificate::CertificateStatement,
    consensus::CommitVoteCells,
    continuity::{
        SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig, SourcePairPlan,
        pair::SourcePairWitness,
    },
    schedule::source::{ScheduleSourceCircuit, ScheduleSourceStage},
};
use iroha_pasta::Eq;
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig};

/// Two exact qualified wrappers for the certified result and its current schedule.
/// All four Pallas obligations are folded and both Vesta claims are retained.
/// Authenticating the complete genesis-rooted history remains mandatory.
#[derive(Clone, Debug)]
pub struct ScheduledResultCircuit {
    pair: SourcePairWitness,
    input: ScheduledResultInput,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl ScheduledResultCircuit {
    /// Build the witnessless original layout without live evidence or host facts.
    /// Child order is complete certified result, then complete current schedule.
    /// # Errors
    /// Invalid fixed parameter or filler encodings.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            input: ScheduledResultInput {
                certified: CertifiedResultContext {
                    certificate: CertificateStatement {
                        roster_root: Fp::ZERO,
                        members: 0,
                        faults: 0,
                        message: [0; CommitVoteCells::BYTES],
                    },
                    root: Fp::ZERO,
                    frame_len: 0,
                },
                schedule: *ScheduleSourceCircuit::for_source(ScheduleSourceStage::Graph).input(),
            },
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }
    /// Verify original complete children and prepare their full two-curve fold.
    /// The circuit independently repeats all endpoint and original-cell links.
    /// # Errors
    /// Partial or substituted programs, invalid proof/claim/key, resource refusal.
    pub fn prepare(
        plan: SourcePairPlan,
        input: &ScheduledResultInput,
        children: [SourceNodeEvidence; 2],
        vesta: &PinnedParams<Eq>,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<Self, Error> {
        if children.each_ref().map(|child| child.endpoints) != input.source_endpoints() {
            return Err(Error::Synthesis);
        }
        let endpoints = singleton_native(PROGRAM_ID, input.statement().digest());
        let pair = SourcePairWitness::prepare(plan, children, vesta, salt, config)?;
        let public = pair.frame(endpoints, vesta, config.kernel_budget)?;
        Ok(Self {
            pair,
            input: *input,
            endpoints,
            public,
        })
    }
    /// Native statement prediction, independently derived from the linked cells.
    pub fn statement(&self) -> ScheduledResultStatement {
        self.input.statement()
    }
    /// Exact source public frame, including both retained Vesta claims.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Complete singleton source endpoint opening.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Complete folded Pallas obligation bound by the internal source digest.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}
impl Circuit<Fp> for ScheduledResultCircuit {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        let mut blank = self.clone();
        blank.pair.known = false;
        blank
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        SourceMergeCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "complete scheduled native result",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let certificate = CertificateStatementCells::assign(
                    &mut chip,
                    &mut region,
                    self.pair.value(self.input.certified.certificate),
                )?;
                let [root] =
                    self.pair
                        .words(&mut chip, &mut region, [self.input.certified.root])?;
                let frame_len = chip.uint().assign::<32>(
                    &mut region,
                    self.pair.value(u128::from(self.input.certified.frame_len)),
                )?;
                let schedule = ScheduleSourceBinding::assign(
                    &mut chip,
                    &mut region,
                    &self.pair.value(self.input.schedule),
                )?;
                let linked = ScheduledResultLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    [&assigned.endpoints[0], &assigned.endpoints[1]],
                    &certificate,
                    &root,
                    &frame_len,
                    &schedule,
                )?;
                let zero = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(PROGRAM_ID),
                    linked.digest(),
                    0,
                    1,
                    &zero,
                    linked.digest(),
                )?;
                self.pair
                    .plan
                    .bind(
                        &mut chip,
                        &mut region,
                        assigned.children(),
                        &assigned.fold,
                        endpoints,
                    )?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
