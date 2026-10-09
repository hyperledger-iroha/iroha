//! Hard composition of complete byte parsing and exact native epoch hashing.
//!
//! Both source-qualified child wrappers are verified. Their original spans,
//! roots, lengths, metadata and complete boundaries are linked in circuit, with
//! all two-curve obligations retained. This authenticates a selected context only
//! relative to the proposed result tape. A complete R scan and genesis-rooted
//! predecessor/quorum history remain mandatory before granting any authority.

use ff::Field;
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::GlueChip;
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, verifier::VerifierChip};

use super::{
    context_hash,
    source::{
        self, ScheduleSourceBinding, ScheduleSourceCircuit, ScheduleSourceInput,
        ScheduleSourceStage,
    },
};
use crate::finality::continuity::{
    SourceEndpoints, SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig, SourcePairPlan,
    pair::SourcePairWitness,
};

/// Complete schedule parsing plus exact epoch-context hashing program.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwscmp1");

impl ScheduleSourceInput {
    /// Exact mandatory complete parser and epoch-hash intervals, in child order.
    /// This predicts statements only; original source proofs remain mandatory.
    /// # Errors
    /// Empty/out-of-bounds original payload or oversized result frame.
    pub fn source_endpoints(&self) -> Result<[[Fp; 6]; 2], Error> {
        let input = &self.epoch_hash;
        if input.payload_len == 0
            || input.result_len > 65_536
            || input
                .payload_start
                .checked_add(input.payload_len)
                .is_none_or(|end| end > input.result_len)
        {
            return Err(Error::Synthesis);
        }
        Ok([
            [
                Fp::from(source::PROGRAM_ID),
                self.digest(),
                Fp::ZERO,
                Fp::from(u64::from(source::PROGRAM_LENGTH)),
                source::boundary_digest_native(self, false),
                source::boundary_digest_native(self, true),
            ],
            [
                Fp::from(context_hash::PROGRAM_ID),
                input.digest(),
                Fp::ZERO,
                Fp::from(u64::from(context_hash::PROGRAM_LENGTH)),
                context_hash::boundary_digest_native(input, false),
                context_hash::boundary_digest_native(input, true),
            ],
        ])
    }
}

/// Actual two-child verifier for one complete selected native schedule context.
/// A caller may not replace either complete source with decoded host metadata.
#[derive(Clone, Debug)]
pub struct ScheduleCircuit {
    pair: SourcePairWitness,
    input: ScheduleSourceInput,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl ScheduleCircuit {
    /// Witnessless fixed layout for original-key import; no live frame or proof.
    /// Child order is complete parser first, complete epoch hash second.
    /// # Errors
    /// Invalid fixed filler encoding.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            input: *ScheduleSourceCircuit::for_source(ScheduleSourceStage::Graph).input(),
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }
    /// Verify both original qualified child proofs and prepare their full fold.
    /// Native checks are repeated against actual child cells in the circuit.
    /// # Errors
    /// Partial/substituted endpoints, proof/key/claim errors or resource refusal.
    pub fn prepare(
        plan: SourcePairPlan,
        input: &ScheduleSourceInput,
        children: [SourceNodeEvidence; 2],
        vesta: &PinnedParams<Eq>,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<Self, Error> {
        if children.each_ref().map(|child| child.endpoints) != input.source_endpoints()? {
            return Err(Error::Synthesis);
        }
        let context = input.digest();
        let endpoints = [
            Fp::from(PROGRAM_ID),
            context,
            Fp::ZERO,
            Fp::ONE,
            Fp::ZERO,
            context,
        ];
        let pair = SourcePairWitness::prepare(plan, children, vesta, salt, config)?;
        let public = pair.frame(endpoints, vesta, config.kernel_budget)?;
        Ok(Self {
            pair,
            input: *input,
            endpoints,
            public,
        })
    }
    /// Exact selected result/context proposal, bound to both verified children.
    pub const fn input(&self) -> &ScheduleSourceInput {
        &self.input
    }
    /// Complete public source frame, independently constrained during proving.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact complete schedule-composition interval opening.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Entire folded Pallas claim; the mandatory source wrapper decides/retains it.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}
impl Circuit<Fp> for ScheduleCircuit {
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
            || "complete native schedule and epoch context",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let binding = ScheduleSourceBinding::assign(
                    &mut chip,
                    &mut region,
                    &self.pair.value(self.input),
                )?;
                let parser = binding.complete_parser_endpoints(&mut chip, &mut region)?;
                let epoch = binding.complete_epoch_hash_endpoints(&mut chip, &mut region)?;
                for (actual, required) in assigned.endpoints.iter().zip([parser, epoch]) {
                    for (actual, required) in actual.words().iter().zip(required.words()) {
                        GlueChip::assert_equal(&mut region, actual, &required)?;
                    }
                }
                let zero = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(PROGRAM_ID),
                    binding.digest(),
                    0,
                    1,
                    &zero,
                    binding.digest(),
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

#[cfg(test)]
mod tests;
