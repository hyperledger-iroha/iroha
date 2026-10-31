//! Shared public bootstrap preparation and checked nonlinear accounting.
//!
//! This module owns no alternate ciphertext algorithm. It composes existing
//! bootstrap plans, reuses only exactly equal public source shapes, and accounts
//! every fixed physical call for both originally admitted nonlinear classes.
//! All inputs carry closed scalar-subalgebra provenance; each canonical plan
//! explicitly selects `ScalarSlots` while retaining the full profile geometry.

use super::bgv_nonlinear::{
    KEYS, LayerPlan, PlanData, PreparedLayer, RamLfeBgvNonlinearBudgetV1,
    RamLfeBgvNonlinearErrorV1 as Error, RamLfeBgvNonlinearPlanV1, RamLfeBgvNonlinearProgramV1,
    RamLfeBgvNonlinearScheduleV1, private_count, stage, stage_count,
};
use super::{RamLfeBgvNonlinearCompilerV1, RamLfeBgvNonlinearShapeV1};
use iroha_fhe::{
    bgv::{
        BgvError, BgvProfile, BootstrapBudget, BootstrapMessageSpace, BootstrapPlan,
        CiphertextShape, PlaintextPrecision, SlotLinearPlan, cost::CostModel,
    },
    prime_power::{UpperDigitsPlan, UpperDigitsProgram},
};

/// One public physical-refresh choice, applied to each selected class-bank boundary.
///
/// It does not select parameters or implement logical Refresh instructions.
/// Program/level/stage budgets are admitted before private preparation. No
/// private tape value may select a different schedule or omit a target update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamLfeBgvNonlinearRefreshChoiceV1 {
    /// Common source level reached by ordinary switching before every bootstrap.
    pub source_level: usize,
    /// Already admitted immutable public upper-digit program geometry.
    pub program: UpperDigitsPlan,
    /// One public level per actual upper-DAG multiplicative depth.
    pub levels: Vec<usize>,
    /// All canonical bootstrap stage ceilings, unchanged in scope.
    pub budget: BootstrapBudget,
}

/// Separate complete preparation payload and remaining public metadata ceilings.
///
/// Both are requested typed/element storage bounds, not allocator overhead or
/// RSS. Keys/profile/backend and caller-owned normal form remain separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "Explicit byte units distinguish these storage ceilings from arithmetic and word-operation budgets at call sites."
)]
pub struct RamLfeBgvNonlinearPreparationV1 {
    /// Class maps/layouts plus one actual shared bootstrap preparation payload.
    pub retained_payload_bytes: usize,
    /// Retained payload plus the largest sequential private/public preparation.
    pub peak_payload_bytes: usize,
    /// Remaining owning headers/public vectors/shape Natural capacities.
    pub retained_metadata_bytes: usize,
    /// Retained metadata plus simultaneously live public planning temporaries.
    pub peak_metadata_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Work {
    pub(super) arithmetic: u64,
    pub(super) words: u64,
}

fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Geometry)
}
fn mul(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b).ok_or(Error::Geometry)
}
fn word(a: usize) -> Result<u64, Error> {
    u64::try_from(a).map_err(|_| Error::Geometry)
}
fn sum(values: &[usize]) -> Result<usize, Error> {
    values.iter().try_fold(0, |a, &b| add(a, b))
}
fn limit(required: usize, budget: usize, resource: &'static str) -> Result<(), Error> {
    if required > budget {
        Err(Error::PreparationBudget {
            resource,
            required,
            budget,
        })
    } else {
        Ok(())
    }
}

impl Work {
    pub(super) fn add(&mut self, arithmetic: u64, words: u64) -> Result<(), Error> {
        self.arithmetic = self
            .arithmetic
            .checked_add(arithmetic)
            .ok_or(Error::Geometry)?;
        self.words = self.words.checked_add(words).ok_or(Error::Geometry)?;
        Ok(())
    }

    pub(super) fn ciphertext_adds(
        &mut self,
        profile: &BgvProfile,
        level: usize,
        count: usize,
    ) -> Result<(), Error> {
        let words = word(mul(mul(mul(2, profile.degree())?, add(level, 1)?)?, count)?)?;
        self.add(0, words)
    }

    pub(super) fn lower(
        &mut self,
        profile: &BgvProfile,
        shape: &CiphertextShape,
        target: usize,
    ) -> Result<CiphertextShape, Error> {
        if target > shape.level() {
            return Err(BgvError::LevelMismatch {
                lhs: shape.level(),
                rhs: target,
            }
            .into());
        }
        let mut value = shape.clone();
        while value.level() > target {
            self.add(CostModel::new(profile).switch_down_units(value.level())?, 0)?;
            value = profile.plan_switch_down(&value)?;
        }
        Ok(value)
    }

    pub(super) fn route<const I: usize, const O: usize>(
        &mut self,
        profile: &BgvProfile,
        route: &SlotLinearPlan<I, O>,
    ) -> Result<(), Error> {
        self.add(route.arithmetic_units(profile)?, 0)?;
        self.ciphertext_adds(
            profile,
            route.output_shapes()[0].level(),
            route.plaintext_products(),
        )
    }

    pub(super) fn admit(self, budget: RamLfeBgvNonlinearBudgetV1) -> Result<(), Error> {
        for (resource, required, ceiling) in [
            (
                "canonical arithmetic units",
                self.arithmetic,
                budget.canonical_arithmetic_units,
            ),
            (
                "scalar word operations",
                self.words,
                budget.scalar_word_operations,
            ),
        ] {
            if required > ceiling {
                return Err(Error::WorkBudget {
                    resource,
                    required,
                    budget: ceiling,
                });
            }
        }
        Ok(())
    }
}

pub(super) fn validate_refresh(
    profile: &BgvProfile,
    precision: PlaintextPrecision,
    choice: Option<&RamLfeBgvNonlinearRefreshChoiceV1>,
    maximum: usize,
    shape: RamLfeBgvNonlinearShapeV1,
) -> Result<(), Error> {
    let Some(choice) = choice else {
        return if maximum == 0 && shape == RamLfeBgvNonlinearShapeV1::Bounded {
            Ok(())
        } else {
            Err(Error::RefreshSchedule)
        };
    };
    let context = choice.program.context();
    if maximum == 0
        || maximum
            > match shape {
                RamLfeBgvNonlinearShapeV1::Bounded => 357,
                RamLfeBgvNonlinearShapeV1::Refresh => 2872,
            }
        || precision.modulus() != 257
        || precision.profile_identity() != profile.identity()
        || context.prime() != profile.parameters().plaintext_prime
        || context.precision() != profile.master_plaintext().exponent()
        || choice.program.retained_digits() != precision.exponent()
        || precision.exponent() >= context.precision()
        || choice.source_level > profile.top_level()
    {
        return Err(Error::RefreshSchedule);
    }
    // Check context first: a foreign huge prime must not authorize a huge
    // caller vector scan. Here p=257 and master precision<=3 by canonical BGV.
    let maximum_depth = choice
        .program
        .geometry()
        .polynomial_evaluations
        .checked_mul(usize::try_from(context.prime()).map_err(|_| Error::Geometry)?)
        .ok_or(Error::Geometry)?;
    if choice.levels.len() > maximum_depth
        || choice.levels.is_empty()
        || choice
            .levels
            .iter()
            .any(|&level| level > profile.top_level())
        || choice.levels.windows(2).any(|levels| levels[0] < levels[1])
    {
        return Err(Error::RefreshSchedule);
    }
    Ok(())
}

pub(super) fn preparation_requirement<
    const I: usize,
    const O: usize,
    const S: usize,
    const U: usize,
>(
    profile: &BgvProfile,
    precision: PlaintextPrecision,
    schedule: &RamLfeBgvNonlinearScheduleV1,
    budget: RamLfeBgvNonlinearBudgetV1,
) -> Result<RamLfeBgvNonlinearPreparationV1, Error> {
    if precision.profile_identity() != profile.identity() {
        return Err(BgvError::ProfileMismatch.into());
    }
    if precision.modulus() != 257 || profile.degree() < 128 {
        return Err(Error::UnsupportedProfile);
    }
    let shape = schedule.layers.shape();
    if (I, O, S, U)
        != match shape {
            RamLfeBgvNonlinearShapeV1::Bounded => (21, 60, 20, 20),
            RamLfeBgvNonlinearShapeV1::Refresh => (4, 3, 3, 1),
        }
    {
        return Err(Error::Geometry);
    }
    // Source/target levels refuse before any class graph or core allocation.
    if schedule.input_level > profile.top_level() {
        return Err(BgvError::LevelMismatch {
            lhs: profile.top_level(),
            rhs: schedule.input_level,
        }
        .into());
    }
    if schedule.output_level > schedule.projection_level {
        return Err(BgvError::LevelMismatch {
            lhs: schedule.projection_level,
            rhs: schedule.output_level,
        }
        .into());
    }
    validate_refresh(
        profile,
        precision,
        schedule.refresh.as_ref(),
        budget.maximum_refresh_plans,
        shape,
    )?;
    let layer = SlotLinearPlan::<I, O>::metadata_requirement(profile, precision)?;
    let projection = SlotLinearPlan::<I, 1>::metadata_requirement(profile, precision)?;
    let shape_metadata = CiphertextShape::metadata_requirement(profile)?;
    let graph = RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(shape)?;
    let mut retained_payload = add(RamLfeBgvNonlinearPlanV1::matrix_payload_bytes(shape), graph)?;
    let class_scratch = add(
        shape.private_payload_bytes(),
        shape.preparation_scratch_bytes(),
    )?;
    // Layout construction can hold public offsets/segments plus the activity
    // graph while an immutable graph is installed. Charge an additional full
    // largest layout payload conservatively, including its Arc control words.
    let mut largest_graph = shape.projection_layout_metadata_bytes::<I>()?;
    for index in 0..stage_count(shape) {
        largest_graph =
            largest_graph.max(shape.layer_layout_metadata_bytes::<I, O>(stage(shape, index))?);
    }
    let mut payload_scratch = class_scratch.max(largest_graph);
    let mut retained_metadata = sum(&[
        size_of::<RamLfeBgvNonlinearPlanV1>(),
        size_of::<RamLfeBgvNonlinearProgramV1>(),
        2 * size_of::<usize>(),
        mul(stage_count(shape), size_of::<LayerPlan<I, O, S, U>>())?,
        mul(private_count(shape), size_of::<PreparedLayer<I, O>>())?,
        mul(stage_count(shape), layer.retained_heap_bytes)?,
        projection.retained_heap_bytes,
        mul(
            add(3, mul(stage_count(shape), I)?)?,
            shape_metadata.retained_heap_bytes,
        )?,
        mul(
            KEYS + (profile.degree() / 128).ilog2() as usize,
            size_of::<u32>(),
        )?,
    ])?;
    // prior source array, lower/source/operand/result transfers and route search
    // may overlap; bound all typed objects and their actual Natural payload cap.
    let outer_shapes = add(mul(4, I)?, add(O, 4)?)?;
    let outer_scratch = sum(&[
        mul(outer_shapes, shape_metadata.retained_bytes()?)?,
        size_of::<RamLfeBgvNonlinearCompilerV1>(),
        size_of::<Vec<u32>>(),
        size_of::<PlanData<I, O, S, U>>(),
        size_of::<RefreshSelection<'_, I>>(),
    ])?;
    let mut metadata_scratch = add(
        outer_scratch,
        layer
            .preparation_scratch_bytes
            .max(projection.preparation_scratch_bytes),
    )?;
    if let Some(choice) = &schedule.refresh {
        let split = BootstrapPlan::split_metadata_requirement(
            profile,
            precision,
            BootstrapMessageSpace::ScalarSlots,
            &choice.program,
        )?;
        let descriptors = split
            .source_descriptor
            .repeated(budget.maximum_refresh_plans)?;
        retained_metadata = add(
            retained_metadata,
            add(
                split.shared_preparation.retained_bytes()?,
                descriptors.retained_bytes()?,
            )?,
        )?;
        metadata_scratch = metadata_scratch.max(add(
            outer_scratch,
            split
                .shared_preparation
                .preparation_scratch_bytes
                .max(descriptors.preparation_scratch_bytes),
        )?);
        let stage = choice.budget;
        let public_program = add(
            choice.program.geometry().coefficient_bytes,
            choice.program.geometry().program_bytes,
        )?;
        limit(
            public_program,
            stage.upper_digits.metadata_bytes,
            "bootstrap public program payload",
        )?;
        let supplied = sum(&[
            stage.decomposition.routing.coefficient_bytes,
            stage.decomposition.routing.layout_metadata_bytes,
            stage.recomposition.routing.coefficient_bytes,
            stage.recomposition.routing.layout_metadata_bytes,
            stage.upper_digits.metadata_bytes,
        ])?;
        // All exact descriptors share the same actual immutable preparation.
        retained_payload = add(retained_payload, supplied)?;
        payload_scratch = payload_scratch
            .max(choice.program.geometry().preparation_scratch_bytes)
            .max(stage.decomposition.routing.preparation_scratch_bytes)
            .max(stage.recomposition.routing.preparation_scratch_bytes);
    }
    let requirement = RamLfeBgvNonlinearPreparationV1 {
        retained_payload_bytes: retained_payload,
        peak_payload_bytes: add(retained_payload, payload_scratch)?,
        retained_metadata_bytes: retained_metadata,
        peak_metadata_bytes: add(retained_metadata, metadata_scratch)?,
    };
    limit(
        requirement.peak_payload_bytes,
        budget.preparation_payload_bytes,
        "complete preparation payload",
    )?;
    limit(
        requirement.peak_metadata_bytes,
        budget.preparation_metadata_bytes,
        "complete preparation metadata",
    )?;
    Ok(requirement)
}

#[derive(Debug)]
pub(super) struct RefreshPlans {
    pub(super) source_level: usize,
    pub(super) plans: Vec<BootstrapPlan>,
    maximum: usize,
}

/// Borrowed public source frontier and its fixed class-selected refresh subset.
#[derive(Debug)]
pub(super) struct RefreshSelection<'a, const I: usize> {
    pub(super) sources: &'a mut [CiphertextShape; I],
    pub(super) selected: &'a [bool; I],
}

impl RefreshPlans {
    pub(super) fn new(source_level: usize, maximum: usize) -> Result<Self, Error> {
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(maximum)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            source_level,
            plans,
            maximum,
        })
    }

    pub(super) fn group<const I: usize>(
        &mut self,
        profile: &BgvProfile,
        choice: &RamLfeBgvNonlinearRefreshChoiceV1,
        selection: RefreshSelection<'_, I>,
        work: &mut Work,
        switches: &mut usize,
        peak: &mut usize,
    ) -> Result<[Option<usize>; I], Error> {
        let RefreshSelection { sources, selected } = selection;
        let model = CostModel::new(profile);
        for (bank, source) in sources.iter_mut().enumerate() {
            if !selected[bank] {
                continue;
            }
            *switches = add(
                *switches,
                source
                    .level()
                    .checked_sub(self.source_level)
                    .ok_or(Error::RefreshSchedule)?,
            )?;
            *source = work.lower(profile, source, self.source_level)?;
        }
        let mut indices = [None; I];
        for bank in 0..I {
            if !selected[bank] {
                continue;
            }
            let index = if let Some(index) = self
                .plans
                .iter()
                .position(|plan| plan.input_shape() == &sources[bank])
            {
                index
            } else {
                if self.plans.len() == self.maximum {
                    return Err(Error::RefreshPlanBudget {
                        required: add(self.plans.len(), 1)?,
                        budget: self.maximum,
                    });
                }
                let plan = if let Some(first) = self.plans.first() {
                    first.admit_source(profile, &sources[bank], choice.budget)?
                } else {
                    #[cfg(test)]
                    super::bgv_nonlinear::tests::observe_public_preparation();
                    let program = UpperDigitsProgram::prepare(&choice.program)?;
                    BootstrapPlan::new(
                        profile,
                        &sources[bank],
                        BootstrapMessageSpace::ScalarSlots,
                        program,
                        &choice.levels,
                        choice.budget,
                    )?
                };
                self.plans.push(plan);
                self.plans.len() - 1
            };
            let plan = &self.plans[index];
            indices[bank] = Some(index);
            work.add(
                plan.payload().canonical_arithmetic_units,
                plan.payload().scalar_word_operations,
            )?;
            // Other sources/completed outputs remain live through the
            // current bootstrap. H construction peaks at its product plus sum
            // (two top-level two-component ciphertexts). Input plaintexts and
            // backend/key scratch retain the existing separate stage scopes.
            let siblings = sources
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != bank)
                .try_fold(0_usize, |bytes, (_, source)| {
                    add(
                        bytes,
                        usize::try_from(model.ciphertext_residue_bytes(source.level(), 2)?)
                            .map_err(|_| Error::Geometry)?,
                    )
                })?;
            let input_residues =
                usize::try_from(model.ciphertext_residue_bytes(profile.top_level(), 2)?)
                    .map_err(|_| Error::Geometry)?
                    .checked_mul(2)
                    .ok_or(Error::Geometry)?;
            *peak = (*peak).max(add(
                siblings,
                input_residues.max(plan.payload().stream_owned_residue_bytes),
            )?);
            sources[bank] = plan.output_shape().clone();
        }
        Ok(indices)
    }
}
