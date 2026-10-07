//! Hard recursive composition of the complete key-sum and native BLS programs.

use super::*;
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

/// Source program for complete ordinary quorum-certificate verification.
/// Schedule and genesis authority must still be composed with this program.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwqccv1");

/// Two exact source-qualified wrappers linked to the same native certificate.
/// Both child equations and all four Pallas obligations are hard verified;
/// both Vesta claims are retained for the mandatory wrapper fold.
#[derive(Clone, Debug)]
pub struct CertificateCircuit {
    pair: SourcePairWitness,
    context: CertificateContext,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl CertificateCircuit {
    /// Build a witnessless source for importing its original proving artifact.
    /// The plan's first child is the complete aggregation and second is BLS.
    /// # Errors
    /// Invalid fixed parameter or filler encodings.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            context: CertificateContext {
                aggregation: AggregateContext {
                    roster_root: Fp::ZERO,
                    members: 0,
                    faults: 0,
                    bitmap: [0; 4],
                    aggregate_key: [0; 48],
                },
                message: [0; CommitVoteCells::BYTES],
                signature: [0; 96],
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
        context: &CertificateContext,
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
            context: *context,
            endpoints,
            public,
        })
    }

    /// Complete native prediction checked against the actual source circuit.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact full certificate-program endpoint opening.
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

impl Circuit<Fp> for CertificateCircuit {
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
            || "complete native quorum certificate",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let [root, members, faults] = self.pair.words(
                    &mut chip,
                    &mut region,
                    [
                        self.context.aggregation.roster_root,
                        Fp::from(u64::from(self.context.aggregation.members)),
                        Fp::from(u64::from(self.context.aggregation.faults)),
                    ],
                )?;
                let bitmap =
                    self.pair
                        .bytes(&mut chip, &mut region, self.context.aggregation.bitmap)?;
                let key = self.pair.bytes(
                    &mut chip,
                    &mut region,
                    self.context.aggregation.aggregate_key,
                )?;
                let message = self
                    .pair
                    .bytes(&mut chip, &mut region, self.context.message)?;
                let signature = self
                    .pair
                    .bytes(&mut chip, &mut region, self.context.signature)?;
                let linked = CertificateLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &assigned.endpoints[0],
                    &assigned.endpoints[1],
                    CertificateInputs {
                        roster_root: &root,
                        members: &members,
                        faults: &faults,
                        bitmap: &bitmap,
                        aggregate_key: &key,
                        message: &message,
                        signature: &signature,
                    },
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

#[cfg(test)]
mod tests;
