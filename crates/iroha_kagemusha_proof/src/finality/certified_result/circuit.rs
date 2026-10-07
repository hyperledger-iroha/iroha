//! Hard composition of a complete quorum certificate and its full result scan.

use super::*;
use crate::finality::consensus::CommitVoteCells;
use crate::finality::continuity::{
    SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig, SourcePairPlan,
    pair::SourcePairWitness,
};
use iroha_pasta::Eq;
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig};

/// Two exact source-qualified wrappers binding the entire original result to R.
/// Both child equations and all four Pallas obligations are hard verified;
/// both Vesta claims are retained for the mandatory wrapper fold.
#[derive(Clone, Debug)]
pub struct CertifiedResultCircuit {
    pair: SourcePairWitness,
    context: CertifiedResultContext,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl CertifiedResultCircuit {
    /// Build a witnessless source for importing its original proving artifact.
    /// The first child is the complete certificate and second is the full R scan.
    /// # Errors
    /// Invalid fixed parameter or filler encodings.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            context: CertifiedResultContext {
                certificate: CertificateStatement {
                    roster_root: Fp::ZERO,
                    members: 0,
                    faults: 0,
                    message: [0; CommitVoteCells::BYTES],
                },
                root: Fp::ZERO,
                frame_len: 0,
            },
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }

    /// Fully verify the original children and prepare their complete fold.
    /// Host predictions never replace the exact circuit endpoint/context links.
    /// # Errors
    /// Partial or different source programs, invalid proofs, claims or resources.
    pub fn prepare(
        plan: SourcePairPlan,
        context: CertifiedResultContext,
        children: [SourceNodeEvidence; 2],
        vesta_params: &PinnedParams<Eq>,
        salt: Fp,
        fold_config: &FoldConfig,
    ) -> Result<Self, Error> {
        if children.each_ref().map(|child| child.endpoints) != context.source_endpoints() {
            return Err(Error::Synthesis);
        }
        let digest = context.digest();
        let endpoints = [
            Fp::from(PROGRAM_ID),
            digest,
            Fp::ZERO,
            Fp::ONE,
            Fp::ZERO,
            digest,
        ];
        let pair = SourcePairWitness::prepare(plan, children, vesta_params, salt, fold_config)?;
        let public = pair.frame(endpoints, vesta_params, fold_config.kernel_budget)?;
        Ok(Self {
            pair,
            context,
            endpoints,
            public,
        })
    }

    /// Complete native prediction checked against the actual source circuit.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact full certified-result program endpoint opening.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Complete carried Pallas obligation bound by the internal source digest.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}

impl Circuit<Fp> for CertifiedResultCircuit {
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
            || "complete certified native result",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let statement = CertificateStatementCells::assign(
                    &mut chip,
                    &mut region,
                    self.pair.value(self.context.certificate),
                )?;
                let [root] = self
                    .pair
                    .words(&mut chip, &mut region, [self.context.root])?;
                let frame_len = chip.uint().assign::<32>(
                    &mut region,
                    self.pair.value(u128::from(self.context.frame_len)),
                )?;
                let linked = CertifiedResultLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &assigned.endpoints[0],
                    &assigned.endpoints[1],
                    &statement,
                    &root,
                    &frame_len,
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
        for (index, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, index)?;
        }
        Ok(())
    }
}
