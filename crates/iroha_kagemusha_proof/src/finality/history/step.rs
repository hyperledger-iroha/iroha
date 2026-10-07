//! Exact ordinary block transition with both original source proofs hard verified.

use ff::Field;
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{Bit, GlueChip, Word};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, verifier::VerifierChip};

use super::{
    HistoryAnchor, HistoryAnchorCells, HistorySlot, HistoryState, HistoryStateCells, PROGRAM_ID,
};
use crate::finality::{
    continuity::{
        SourceEndpoints, SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig, SourcePairPlan,
        pair::SourcePairWitness,
    },
    schedule::{
        complete,
        source::{
            ScheduleSourceBinding, ScheduleSourceCircuit, ScheduleSourceInput, ScheduleSourceStage,
        },
    },
    scheduled_result::{self, ScheduledResultStatement, ScheduledResultStatementCells},
};

/// Untrusted openings for one exact history transition and its two source proofs.
#[derive(Clone, Copy, Debug)]
pub struct HistoryStepInput {
    /// Previous authenticated history state, opened by the enclosing history join.
    pub before: HistoryState,
    /// Proposed next complete history state, constrained field by field.
    pub after: HistoryState,
    /// Original compact certified-result/current-schedule output.
    pub scheduled: ScheduledResultStatement,
    /// Complete authorized-context parser and native hash for the same result.
    pub authorized: ScheduleSourceInput,
}
impl HistoryStepInput {
    /// Predict the exact next state from the two untrusted source openings.
    /// This does not verify either source, the predecessor or the genesis anchor.
    /// # Errors
    /// Native height overflow.
    pub fn expected_after(&self) -> Result<HistoryState, Error> {
        let slot = |index: usize| {
            let projection = self.authorized.projection.slots[index];
            HistorySlot {
                pending: projection.pending,
                epoch: if projection.pending {
                    0
                } else {
                    self.authorized.projection.epoch
                },
                context: if projection.pending {
                    [0; 32]
                } else {
                    self.authorized.epoch_hash.context_id
                },
                boundary_height: projection.boundary_height,
                predecessor: projection.predecessor,
                parameters: projection.parameters,
            }
        };
        Ok(HistoryState {
            next_height: self
                .before
                .next_height
                .checked_add(1)
                .ok_or(Error::Synthesis)?,
            current: slot(0),
            following: slot(1),
            result: self.scheduled.result,
            tape_root: self.scheduled.tape_root,
            frame_len: self.scheduled.frame_len,
        })
    }
    /// Exact mandatory source intervals, in scheduled-result/authorized-schedule order.
    pub fn source_endpoints(&self) -> [[Fp; 6]; 2] {
        [
            singleton(scheduled_result::PROGRAM_ID, self.scheduled.digest()),
            singleton(complete::PROGRAM_ID, self.authorized.digest()),
        ]
    }
    /// Early native consistency check, independently repeated in circuit.
    /// No native acceptance bit enters the proof.
    /// # Errors
    /// Noncanonical states, changed source/anchor/height, unauthorized context or lag-two promise.
    pub fn validate(&self, anchor: &HistoryAnchor) -> Result<(), Error> {
        let schedule = &self.authorized;
        let result = &self.scheduled;
        let before = &self.before;
        if !before.is_canonical()
            || !self.after.is_canonical()
            || !schedule.authorized
            || result.network != anchor.network
            || result.instance != anchor.instance
            || schedule.projection.network != anchor.network
            || result.height != before.next_height
            || schedule.height != result.height
            || schedule.epoch_hash.tape_root != result.tape_root
            || schedule.epoch_hash.result_len != result.frame_len
            || before.current.epoch != result.epoch
            || before.current.context != result.context
            || self.after != self.expected_after()?
            || before.following.parameters != self.after.current.parameters
        {
            return Err(Error::Synthesis);
        }
        if schedule.projection.boundary_present {
            if !before.following.pending
                || schedule.projection.boundary_height != result.height
                || before.following.boundary_height != schedule.projection.boundary_height
                || before.following.predecessor != schedule.projection.boundary_ids[0]
                || schedule.projection.boundary_ids[0] != result.context
            {
                return Err(Error::Synthesis);
            }
        } else if before.following != self.after.current {
            return Err(Error::Synthesis);
        }
        Ok(())
    }
}
fn singleton(program: u64, digest: Fp) -> [Fp; 6] {
    [
        Fp::from(program),
        digest,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        digest,
    ]
}
fn require_source(
    region: &mut Region<'_, Fp>,
    source: &SourceEndpoints,
    program: u64,
    digest: &Word<Fp>,
) -> Result<(), Error> {
    let words = source.words();
    for (i, value) in [
        (0, Fp::from(program)),
        (2, Fp::ZERO),
        (3, Fp::ONE),
        (4, Fp::ZERO),
    ] {
        GlueChip::assert_constant(region, &words[i], value)?;
    }
    for i in [1, 5] {
        GlueChip::assert_equal(region, &words[i], digest)?;
    }
    Ok(())
}
fn equal_when(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    flag: &Bit<Fp>,
    a: &Word<Fp>,
    b: &Word<Fp>,
) -> Result<(), Error> {
    let difference = chip.uint().glue().sub(region, a, b)?;
    let zero = chip.uint().glue().mul(region, flag.word(), &difference)?;
    GlueChip::assert_constant(region, &zero, Fp::ZERO)
}
fn bytes_equal(
    region: &mut Region<'_, Fp>,
    a: &[Word<Fp>; 32],
    b: &[Word<Fp>; 32],
) -> Result<(), Error> {
    for (a, b) in a.iter().zip(b) {
        GlueChip::assert_equal(region, a, b)?;
    }
    Ok(())
}

/// Exact step linkage using original source endpoint cells. The enclosing owner
/// must hard verify both wrappers and retain all of their IPA obligations.
#[derive(Clone, Debug)]
pub struct HistoryStepCells {
    before: HistoryStateCells,
    after: HistoryStateCells,
}
impl HistoryStepCells {
    /// Constrain the original source openings, current authority and lag-two successor.
    /// # Errors
    /// Layout errors; changed fields, gaps, unresolved authority or overflow fail.
    pub fn constrain(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        sources: [&SourceEndpoints; 2],
        anchor: &HistoryAnchorCells,
        input: &Value<HistoryStepInput>,
    ) -> Result<Self, Error> {
        let before =
            HistoryStateCells::assign(chip, region, &input.map(|v| v.before), anchor.digest())?;
        let after =
            HistoryStateCells::assign(chip, region, &input.map(|v| v.after), anchor.digest())?;
        let scheduled =
            ScheduledResultStatementCells::assign(chip, region, input.map(|v| v.scheduled))?;
        let schedule = ScheduleSourceBinding::assign(chip, region, &input.map(|v| v.authorized))?;
        require_source(
            region,
            sources[0],
            scheduled_result::PROGRAM_ID,
            scheduled.digest(),
        )?;
        require_source(region, sources[1], complete::PROGRAM_ID, schedule.digest())?;
        GlueChip::assert_constant(region, schedule.authorized().word(), Fp::ONE)?;
        bytes_equal(region, scheduled.network(), anchor.network())?;
        bytes_equal(region, scheduled.instance(), anchor.instance())?;
        bytes_equal(region, schedule.network(), anchor.network())?;
        GlueChip::assert_equal(
            region,
            scheduled.height().word(),
            before.next_height().word(),
        )?;
        GlueChip::assert_equal(region, schedule.height().word(), scheduled.height().word())?;
        GlueChip::assert_equal(region, schedule.tape().root(), scheduled.tape_root())?;
        GlueChip::assert_equal(
            region,
            schedule.tape().frame_len().word(),
            scheduled.frame_len().word(),
        )?;
        GlueChip::assert_equal(
            region,
            before.current().epoch().word(),
            scheduled.epoch().word(),
        )?;
        bytes_equal(region, before.current().context(), scheduled.context())?;
        let next = chip
            .uint()
            .checked_add_constant(region, before.next_height(), 1)?;
        GlueChip::assert_equal(region, next.word(), after.next_height().word())?;
        bytes_equal(region, after.result(), scheduled.result())?;
        GlueChip::assert_equal(region, after.tape_root(), scheduled.tape_root())?;
        GlueChip::assert_equal(
            region,
            after.frame_len().word(),
            scheduled.frame_len().word(),
        )?;
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let one = chip.uint().glue().constant(region, Fp::ONE)?;
        for (i, actual) in [after.current(), after.following()].into_iter().enumerate() {
            let declared = schedule.slot(i).ok_or(Error::Synthesis)?;
            GlueChip::assert_equal(region, actual.pending().word(), declared.pending())?;
            let epoch =
                chip.uint()
                    .glue()
                    .select(region, actual.pending(), &zero, schedule.epoch())?;
            GlueChip::assert_equal(region, actual.epoch().word(), &epoch)?;
            for (actual_byte, expected) in actual.context().iter().zip(schedule.context_id()) {
                let expected =
                    chip.uint()
                        .glue()
                        .select(region, actual.pending(), &zero, expected)?;
                GlueChip::assert_equal(region, actual_byte, &expected)?;
            }
            GlueChip::assert_equal(
                region,
                actual.boundary_height().word(),
                declared.boundary_height(),
            )?;
            bytes_equal(region, actual.predecessor(), declared.predecessor())?;
            for (actual, expected) in actual.parameters().iter().zip(declared.parameters()) {
                GlueChip::assert_equal(region, actual.word(), expected)?;
            }
        }
        for (old, new) in before
            .following()
            .parameters()
            .iter()
            .zip(after.current().parameters())
        {
            GlueChip::assert_equal(region, old.word(), new.word())?;
        }
        let boundary = chip
            .uint()
            .glue()
            .assert_bool(region, schedule.boundary_present())?;
        let continuation = chip.uint().glue().not(region, &boundary)?;
        for (old, new) in before
            .following()
            .words()
            .iter()
            .zip(after.current().words())
        {
            equal_when(chip, region, &continuation, old, new)?;
        }
        equal_when(
            chip,
            region,
            &boundary,
            before.following().pending().word(),
            &one,
        )?;
        equal_when(
            chip,
            region,
            &boundary,
            schedule.boundary_height(),
            scheduled.height().word(),
        )?;
        equal_when(
            chip,
            region,
            &boundary,
            before.following().boundary_height().word(),
            schedule.boundary_height(),
        )?;
        for ((old, parsed), incumbent) in before
            .following()
            .predecessor()
            .iter()
            .zip(schedule.boundary_predecessor())
            .zip(scheduled.context())
        {
            equal_when(chip, region, &boundary, old, parsed)?;
            equal_when(chip, region, &boundary, parsed, incumbent)?;
        }
        Ok(Self { before, after })
    }
    /// Complete opened predecessor state.
    pub const fn before(&self) -> &HistoryStateCells {
        &self.before
    }
    /// Complete exact next state.
    pub const fn after(&self) -> &HistoryStateCells {
        &self.after
    }
}

/// Actual two-child history step under one circuit-fixed authenticated genesis policy.
#[derive(Clone, Debug)]
pub struct HistoryStepCircuit {
    pair: SourcePairWitness,
    anchor: HistoryAnchor,
    input: HistoryStepInput,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl HistoryStepCircuit {
    /// Witnessless fixed source layout; the anchor remains fixed original-key policy.
    /// Child order is complete `ScheduledResult` then authorized Schedule.
    /// # Errors
    /// Invalid source-plan filler encoding.
    pub fn for_source(anchor: HistoryAnchor, plan: SourcePairPlan) -> Result<Self, Error> {
        let initial = HistoryState::genesis(&anchor);
        Ok(Self {
            pair: SourcePairWitness::for_source(plan)?,
            anchor,
            input: HistoryStepInput {
                before: initial,
                after: initial,
                scheduled: ScheduledResultStatement::default(),
                authorized: *ScheduleSourceCircuit::for_source(ScheduleSourceStage::Graph).input(),
            },
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }
    /// Verify both original source proofs and prepare the complete four-claim fold.
    /// # Errors
    /// Changed native geometry/anchor, wrong source, proof, claim or resource failure.
    pub fn prepare(
        anchor: HistoryAnchor,
        plan: SourcePairPlan,
        input: &HistoryStepInput,
        children: [SourceNodeEvidence; 2],
        vesta: &PinnedParams<Eq>,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<Self, Error> {
        input.validate(&anchor)?;
        if children.each_ref().map(|child| child.endpoints) != input.source_endpoints() {
            return Err(Error::Synthesis);
        }
        let digest = anchor.digest();
        let endpoints = [
            Fp::from(PROGRAM_ID),
            digest,
            Fp::ZERO,
            Fp::ONE,
            input.before.digest(digest),
            input.after.digest(digest),
        ];
        let pair = SourcePairWitness::prepare(plan, children, vesta, salt, config)?;
        let public = pair.frame(endpoints, vesta, config.kernel_budget)?;
        Ok(Self {
            pair,
            anchor,
            input: *input,
            endpoints,
            public,
        })
    }
    /// Fixed installation policy, preserved during witness erasure.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }
    /// Exact before/after and source openings.
    pub const fn input(&self) -> &HistoryStepInput {
        &self.input
    }
    /// Original six endpoint words; the height is in each constrained state.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Complete original source frame.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Complete folded Pallas claim retained by the mandatory source wrapper.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}
impl Circuit<Fp> for HistoryStepCircuit {
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
            || "genesis-bound exact native history step",
            |mut region| {
                let anchor = HistoryAnchorCells::assign(&mut chip, &mut region, &self.anchor)?;
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let linked = HistoryStepCells::constrain(
                    &mut chip,
                    &mut region,
                    [&assigned.endpoints[0], &assigned.endpoints[1]],
                    &anchor,
                    &self.pair.value(self.input),
                )?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(PROGRAM_ID),
                    anchor.digest(),
                    0,
                    1,
                    linked.before().digest(),
                    linked.after().digest(),
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
