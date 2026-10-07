//! Actual two-child source circuit for a contiguous program interval.

use super::{pair::SourcePairWitness, *};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner},
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, verifier::VerifierConfig};

/// Fixed verifier lanes and public69 frame shared by binary source compositions.
#[derive(Clone, Debug)]
pub struct SourcePairConfig {
    pub(crate) verifier: VerifierConfig<Ep>,
    pub(crate) public: Column<Instance>,
}

/// Actual interval merge under two source-qualified child wrapper keys.
/// Native preparation fully verifies both originals and their claims; the
/// circuit independently hard verifies them and constrains exact continuity.
#[derive(Clone, Debug)]
pub struct SourceMergeCircuit {
    pair: SourcePairWitness,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl SourceMergeCircuit {
    /// Build the witnessless fixed layout for importing an original source key.
    /// No child proof or live operation is needed to mount original artifacts.
    /// # Errors
    /// Invalid pinned filler encodings.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }

    /// Verify both original children and prepare the complete four-claim fold.
    /// Native endpoint validation does not substitute for the circuit relation.
    /// # Errors
    /// Invalid endpoint geometry, discontinuity, proof, claim, fold or budget.
    pub fn prepare(
        plan: SourcePairPlan,
        children: [SourceNodeEvidence; 2],
        vesta_params: &PinnedParams<Eq>,
        salt: Fp,
        fold_config: &FoldConfig,
    ) -> Result<Self, Error> {
        let [a, b] = children.each_ref().map(|child| child.endpoints);
        for e in [a, b] {
            let start = to_u128(&e[2]).ok_or(Error::Synthesis)?;
            let end = to_u128(&e[3]).ok_or(Error::Synthesis)?;
            if e[0] == Fp::ZERO || start >= end || end > u128::from(u32::MAX) {
                return Err(Error::Synthesis);
            }
        }
        if a[0] != b[0] || a[1] != b[1] || a[3] != b[2] || a[5] != b[4] {
            return Err(Error::Synthesis);
        }
        let endpoints = [a[0], a[1], a[2], b[3], a[4], b[5]];
        let pair = SourcePairWitness::prepare(plan, children, vesta_params, salt, fold_config)?;
        let public = pair.frame(endpoints, vesta_params, fold_config.kernel_budget)?;
        Ok(Self {
            pair,
            endpoints,
            public,
        })
    }

    /// Complete predicted public frame, independently constrained when proving.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact interval opening retained by the source wrapper.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Entire folded Pallas claim bound into the source digest.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(super) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}
impl Circuit<Fp> for SourceMergeCircuit {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        let mut blank = self.clone();
        blank.pair.known = false;
        blank
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed source pair profile");
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        SourcePairConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "source interval merge",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                self.pair
                    .plan
                    .merge(&mut chip, &mut region, assigned.children(), &assigned.fold)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
