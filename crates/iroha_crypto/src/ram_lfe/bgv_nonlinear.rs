//! One class-bound nonlinear arithmetic kernel under a fixed public schedule.
//!
//! Bounded has sixteen 21-to-60 layers. Refresh has 2871 four-to-three microsteps
//! over 319 gate positions. One kernel computes A*B+C and scatters each result
//! into its public target bank. Untouched state and base owners move unchanged.
//! Refresh starts three honest zero banks at the routing level and refreshes
//! only the base initially, then the target bank after every physical step.
//! Its canonical rank weight remains ten; the closed zero-future invariant
//! removes the redundant Unit step and leaves nine actual steps per gate. The
//! original bank is proven packed input plus per-request lanes and one. The public
//! plan derives every transfer before any private coefficient is prepared.
//! Checked scalar ciphertext owners preserve membership through every operation;
//! physical refresh extracts only the scalar coefficient bank. The public
//! result consumes this capability into an ordinary local ciphertext.
//! Caller-supplied levels and scratch ceilings do not select cryptographic
//! parameters or override noise refusal. There is no global schedule optimizer.
//!
//! This is an unqualified arithmetic algorithm, not production admission.
//! TODO(R.7, R.9, C.3, R.11): qualify parameters and complete input/key authentication,
//! committed execution proofs, circuit-privacy sanitization and full resource
//! qualification. The existing bounded candidate can fail this full schedule;
//! no class limit, resource cap or refusal is weakened to make it fit.

use super::bgv_nonlinear_refresh::{
    self, RamLfeBgvNonlinearPreparationV1, RamLfeBgvNonlinearRefreshChoiceV1, RefreshPlans,
    RefreshSelection, Work,
};
use super::{
    RamLfeBgvNonlinearCompileErrorV1, RamLfeBgvNonlinearCompilerV1, RamLfeBgvNonlinearShapeV1,
    RamLfeBgvNonlinearStageV1, RamLfeBgvPackedInputV1, RamLfeBgvRefreshMapV1,
    RamLfeInitialMemoryV1, RamLfeNormalFormV1, clearing::ClearingArray,
};
use iroha_fhe::bgv::{
    BgvError, BgvProfile, BootstrapError, CiphertextShape, LocalBootstrapCiphertext,
    LocalBootstrapKey, LocalScalarCiphertext, Plaintext, PlaintextPrecision, PlanMetadataError,
    SlotEncoding, SlotLinearBudget, SlotLinearError, SlotLinearMap, SlotLinearPlan,
    cost::CostModel,
};
use std::{fmt, sync::Arc};
use thiserror::Error;

const SLOTS: usize = 128;
pub(super) const KEYS: usize = 7;

/// Public routing and multiplication choices for one complete physical layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RamLfeBgvNonlinearLayerChoiceV1 {
    /// Common level of every class bank before fixed-support routing.
    pub routing_level: usize,
    /// Level of every routed operand before products; Refresh carries base unchanged.
    pub multiplication_level: usize,
    /// Public ceiling for shared routing-owned cached/temporary residue bytes.
    pub routing_scratch_bytes: usize,
}

/// Public stage choices for exactly one nonlinear class.
///
/// Both variants own one fixed-size boxed array. This allocation belongs to
/// the caller-owned schedule and is separate from retained plan metadata.
/// The array types enforce the exact sixteen- or three-choice cardinality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RamLfeBgvNonlinearLayersV1 {
    /// All sixteen bounded rank layers, in canonical order.
    Bounded(Box<[RamLfeBgvNonlinearLayerChoiceV1; 16]>),
    /// Initial, Square and Final choices at each of 319 gate positions.
    /// Square executes seven times; the other choices execute once.
    Refresh(Box<[RamLfeBgvNonlinearLayerChoiceV1; 3]>),
}
impl RamLfeBgvNonlinearLayersV1 {
    pub(super) const fn shape(&self) -> RamLfeBgvNonlinearShapeV1 {
        match self {
            Self::Bounded(_) => RamLfeBgvNonlinearShapeV1::Bounded,
            Self::Refresh(_) => RamLfeBgvNonlinearShapeV1::Refresh,
        }
    }
    fn choice(&self, index: usize) -> RamLfeBgvNonlinearLayerChoiceV1 {
        match self {
            Self::Bounded(values) => values[index],
            Self::Refresh(values) => values[index % 3],
        }
    }
    #[cfg(test)]
    fn bounded_mut(&mut self) -> &mut [RamLfeBgvNonlinearLayerChoiceV1; 16] {
        match self {
            Self::Bounded(values) => values,
            _ => panic!("bounded test schedule"),
        }
    }
}
/// Public schedule, independent of the private tape and its logical refreshes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamLfeBgvNonlinearScheduleV1 {
    /// Actual source level selected by the canonical packed-input constructor.
    pub input_level: usize,
    /// Physical refresh of each updated bank; mandatory for Refresh.
    pub refresh: Option<RamLfeBgvNonlinearRefreshChoiceV1>,
    /// Class-fixed public choices; no private count can shorten the schedule.
    pub layers: RamLfeBgvNonlinearLayersV1,
    /// Common bank level before the final ordered-output projection.
    pub projection_level: usize,
    /// Canonical output level after projection, without a redundant mask.
    pub output_level: usize,
    /// Public scratch ceiling for the final shared routing operation.
    pub projection_scratch_bytes: usize,
}

/// Explicit payload ceilings for prepared matrices and arithmetic-owned residues.
///
/// The complete preparation payload/metadata fields include private compiler,
/// class maps and all prepared public plans under their complementary scopes.
/// The arithmetic residue field excludes those prepared owners, runtime object
/// metadata, profiles, borrowed keys, plaintext encoding and backend scratch.
/// Allocator overhead/over-reservation and process RSS remain outside this
/// requested-payload model; it is not a protocol resource policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RamLfeBgvNonlinearBudgetV1 {
    /// Maximum distinct exactly matched bootstrap source plans: zero without
    /// refresh, up to 357 for Bounded or 2872 for Refresh. Every call executes.
    pub maximum_refresh_plans: usize,
    /// Complete retained payload plus maximum sequential preparation scratch.
    pub preparation_payload_bytes: usize,
    /// Complementary owning headers/public vectors/Natural-capacity peak.
    pub preparation_metadata_bytes: usize,
    /// Complete `CostModel` arithmetic including every physical bootstrap call.
    /// Excludes encoding, preparation, copies/clears, keys and proof work.
    pub canonical_arithmetic_units: u64,
    /// Complete separately counted explicit scalar-word operations. Internal
    /// kernel additions retain `CostModel`'s exclusions; this is not CPU time.
    pub scalar_word_operations: u64,
    /// Maximum bytes of all class matrices, public and private, plus projection.
    pub matrix_payload_bytes: usize,
    /// Maximum public layout payload: one shared bounded layer graph or every
    /// unique Refresh stage graph, plus projection; excludes allocator headers.
    pub layout_metadata_bytes: usize,
    /// Maximum clearing row payload used only during owner-side preparation.
    pub preparation_scratch_bytes: usize,
    /// Maximum application/shared-routing-owned ciphertext residue payload.
    pub owned_residue_bytes: usize,
}

/// Exact counts of the public high-level arithmetic schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RamLfeBgvNonlinearOperationsV1 {
    /// Complete physical bootstraps; their inner operations are accounted by
    /// the aggregate work totals, separately from the outer-layer counts below.
    pub bootstrap_calls: usize,
    /// Complete class linear transformations, including every repeated step and projection.
    pub routing_calls: usize,
    /// Key-switched Galois applications inside shared routing.
    pub galois_applications: usize,
    /// Plaintext products inside public-support routing; no redundant masks.
    pub plaintext_products: usize,
    /// Ciphertext additions including every routed accumulation and C addition.
    pub ciphertext_additions: usize,
    /// Plaintext additions for the per-request lane/one embedding.
    pub plaintext_additions: usize,
    /// Ciphertext products, each including relinearization.
    pub ciphertext_products: usize,
    /// Individual single-prime ciphertext modulus switches.
    pub modulus_switches: usize,
}

/// A public admission or private preparation/evaluation refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RamLfeBgvNonlinearErrorV1 {
    /// Checked public dimensions, counts or byte/work arithmetic overflowed.
    #[error("nonlinear evaluation geometry overflow")]
    Geometry,
    /// The public refresh mode/count/context/level vector is inconsistent.
    #[error("invalid nonlinear physical refresh schedule")]
    RefreshSchedule,
    /// Exact shape deduplication needs more admitted prepared plans.
    #[error("nonlinear refresh needs {required} plans, budget {budget}")]
    RefreshPlanBudget {
        /// Minimum distinct count discovered before another plan is allocated.
        required: usize,
        /// Explicit preadmitted count ceiling.
        budget: usize,
    },
    /// A complete scoped preparation ceiling was exceeded.
    #[error("nonlinear {resource} requires {required} bytes, budget {budget}")]
    PreparationBudget {
        /// Named complementary storage scope.
        resource: &'static str,
        /// Checked required storage.
        required: usize,
        /// Explicit byte ceiling.
        budget: usize,
    },
    /// A complete declared evaluation work ceiling was exceeded.
    #[error("nonlinear {resource} requires {required}, budget {budget}")]
    WorkBudget {
        /// Named canonical work category.
        resource: &'static str,
        /// Checked complete work count.
        required: u64,
        /// Explicit finite ceiling.
        budget: u64,
    },
    /// A canonical public metadata prebound or observation failed.
    #[error(transparent)]
    Metadata(#[from] PlanMetadataError),
    /// Canonical public lifting-program preparation failed.
    #[error(transparent)]
    PublicProgram(#[from] iroha_fhe::prime_power::PrimePowerError),
    /// The arithmetic requires 128 F257 scalar slots.
    #[error("nonlinear BGV evaluation requires 128 F257 slots")]
    UnsupportedProfile,
    /// The exact matrix payload exceeds its explicit ceiling.
    #[error("nonlinear matrices require {required} bytes, budget {budget}")]
    MatrixBudget {
        /// Exact required matrix coefficient bytes.
        required: usize,
        /// Caller-supplied byte ceiling.
        budget: usize,
    },
    /// A conservative application/routing residue bound exceeds its ceiling.
    #[error("nonlinear arithmetic owns up to {required} residue bytes, budget {budget}")]
    ResidueBudget {
        /// Conservative payload bound.
        required: usize,
        /// Caller-supplied byte ceiling.
        budget: usize,
    },
    /// The input is not the planned exact constructed two-component source shape.
    #[error("nonlinear BGV input does not have the required constructed shape")]
    InputShape,
    /// The prepared program belongs to another exact admitted plan.
    #[error("nonlinear program and admitted plan differ")]
    ProgramPlan,
    /// A public-size container reservation failed.
    #[error("nonlinear BGV container allocation failed")]
    Allocation,
    /// A compiler allocation, class or matrix validation failed.
    #[error(transparent)]
    Compile(#[from] RamLfeBgvNonlinearCompileErrorV1),
    /// A shared routing geometry, allocation or arithmetic check failed.
    #[error(transparent)]
    Routing(#[from] SlotLinearError),
    /// A shared BGV shape, key, level or noise check failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
    /// A closed local-origin or complete physical refresh check failed.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
}

#[derive(Debug)]
pub(super) struct LayerPlan<const I: usize, const O: usize, const S: usize, const U: usize> {
    stage: RamLfeBgvNonlinearStageV1,
    choice: RamLfeBgvNonlinearLayerChoiceV1,
    routing: SlotLinearPlan<I, O>,
    products: ProductPlan<S, U>,
    after_refresh: [Option<usize>; I],
    repetitions: usize,
    coefficients: Coefficients<I, O>,
}
#[derive(Debug)]
enum Coefficients<const I: usize, const O: usize> {
    Public(PreparedLayer<I, O>),
    Private(usize),
}
#[derive(Debug)]
pub(super) struct PreparedLayer<const I: usize, const O: usize> {
    stage: RamLfeBgvNonlinearStageV1,
    map: SlotLinearMap<I, O>,
}
// Only initialization and scatter create these private ordered state owners.
// General scalar provenance alone grants no reachable nonlinear-state admission.
#[derive(Debug)]
struct StateBank<'key> {
    ordinal: usize,
    value: LocalScalarCiphertext<'key>,
}
impl StateBank<'_> {
    #[cfg(test)]
    fn shape(&self) -> CiphertextShape {
        self.value.shape()
    }
    #[cfg(test)]
    fn ciphertext(&self) -> &iroha_fhe::bgv::Ciphertext {
        self.value.ciphertext()
    }
    #[cfg(test)]
    fn as_local(&self) -> &LocalBootstrapCiphertext<'_> {
        self.value.as_local()
    }
}
fn check_bank_order<const I: usize>(
    banks: &[StateBank<'_>],
) -> Result<(), RamLfeBgvNonlinearErrorV1> {
    if banks.len() != I || banks.iter().enumerate().any(|(i, bank)| bank.ordinal != i) {
        return Err(RamLfeBgvNonlinearErrorV1::Geometry);
    }
    Ok(())
}
fn update_banks<const I: usize, const S: usize, const U: usize>(
    stage: RamLfeBgvNonlinearStageV1,
) -> Result<[usize; U], RamLfeBgvNonlinearErrorV1> {
    if I != S + 1 {
        return Err(RamLfeBgvNonlinearErrorV1::Geometry);
    }
    match stage {
        RamLfeBgvNonlinearStageV1::BoundedLayer(layer)
            if S == 20 && U == 20 && (1..=16).contains(&layer) =>
        {
            Ok(std::array::from_fn(|i| i))
        }
        RamLfeBgvNonlinearStageV1::RefreshGate { gate, .. } if S == 3 && U == 1 && gate < 319 => {
            Ok([usize::from(gate) / SLOTS; U])
        }
        _ => Err(RamLfeBgvNonlinearErrorV1::Geometry),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProductPlan<const S: usize, const U: usize> {
    level: usize,
    updates: [usize; U],
    state_outputs: [CiphertextShape; S],
    base_output: CiphertextShape,
}
#[derive(Debug)]
pub(super) struct PlanData<const I: usize, const O: usize, const S: usize, const U: usize> {
    identity: [u8; 32],
    // Process-local preparation binding; this grants no external authentication.
    binding: Arc<()>,
    shape: RamLfeBgvNonlinearShapeV1,
    source: CiphertextShape,
    augmented: CiphertextShape,
    state_level: usize,
    layers: Vec<LayerPlan<I, O, S, U>>,
    projection: SlotLinearPlan<I, 1>,
    projection_level: usize,
    output_level: usize,
    output: CiphertextShape,
    galois: Vec<u32>,
    refresh: Option<RefreshPlans>,
    initial_refresh: [Option<usize>; I],
    preparation: RamLfeBgvNonlinearPreparationV1,
    work: Work,
    operations: RamLfeBgvNonlinearOperationsV1,
    owned_residue_bytes: usize,
}
/// Immutable complete public plan for one originally admitted nonlinear class.
#[derive(Debug)]
pub struct RamLfeBgvNonlinearPlanV1 {
    data: PlanKind,
}
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "The inline plan owner is charged by the preparation metadata model; boxing would add an allocation and change that resource contract."
)]
enum PlanKind {
    Bounded(PlanData<21, 60, 20, 20>),
    Refresh(PlanData<4, 3, 3, 1>),
}
macro_rules! with_plan {
    ($self:expr, $plan:ident, $body:expr) => {
        match &$self.data {
            PlanKind::Bounded($plan) => $body,
            PlanKind::Refresh($plan) => $body,
        }
    };
}
impl RamLfeBgvNonlinearPlanV1 {
    /// Admit every public transfer, storage scope and complete work count.
    ///
    /// No outer schedule search or private coefficient read occurs; canonical routing
    /// still selects its public split within each admitted scratch ceiling. Source level is
    /// the level genuinely produced by the packed-input constructor. Refresh
    /// requires a physical bootstrap for the initial base and every target update.
    /// # Errors
    /// Refuses invalid class geometry, levels, resource caps and canonical noise.
    pub fn new(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
        schedule: &RamLfeBgvNonlinearScheduleV1,
        budget: RamLfeBgvNonlinearBudgetV1,
    ) -> Result<Self, RamLfeBgvNonlinearErrorV1> {
        let data = match schedule.layers.shape() {
            RamLfeBgvNonlinearShapeV1::Bounded => {
                PlanKind::Bounded(PlanData::new(profile, precision, schedule, budget)?)
            }
            RamLfeBgvNonlinearShapeV1::Refresh => {
                PlanKind::Refresh(PlanData::new(profile, precision, schedule, budget)?)
            }
        };
        Ok(Self { data })
    }
    /// Exact class matrix payload, including fixed public maps and projection.
    #[must_use]
    pub const fn matrix_payload_bytes(shape: RamLfeBgvNonlinearShapeV1) -> usize {
        match shape {
            RamLfeBgvNonlinearShapeV1::Bounded => {
                16 * shape.maximum_layer_payload_bytes() + shape.projection_payload_bytes()
            }
            RamLfeBgvNonlinearShapeV1::Refresh => 1_962_112,
        }
    }
    /// Exact retained support graphs, sharing only the identical bounded graph.
    /// # Errors
    /// Refuses checked geometry arithmetic overflow.
    pub fn layout_metadata_bytes(
        shape: RamLfeBgvNonlinearShapeV1,
    ) -> Result<usize, RamLfeBgvNonlinearCompileErrorV1> {
        match shape {
            RamLfeBgvNonlinearShapeV1::Bounded => shape
                .layer_layout_metadata_bytes::<21, 60>(RamLfeBgvNonlinearStageV1::BoundedLayer(1))?
                .checked_add(shape.projection_layout_metadata_bytes::<21>()?)
                .ok_or_else(|| SlotLinearError::GeometryOverflow.into()),
            RamLfeBgvNonlinearShapeV1::Refresh => {
                let mut bytes = shape.projection_layout_metadata_bytes::<4>()?;
                for index in 0..957 {
                    bytes = bytes
                        .checked_add(
                            shape.layer_layout_metadata_bytes::<4, 3>(stage(shape, index))?,
                        )
                        .ok_or(SlotLinearError::GeometryOverflow)?;
                }
                Ok(bytes)
            }
        }
    }
    /// Original nonlinear class geometry represented by this exact plan.
    #[must_use]
    pub fn shape(&self) -> RamLfeBgvNonlinearShapeV1 {
        with_plan!(self, p, p.shape)
    }
    /// Exact output shape, including the actual post-projection level and bound.
    #[must_use]
    pub fn output_shape(&self) -> &CiphertextShape {
        with_plan!(self, p, &p.output)
    }
    /// Exact routing/trace key union; relinearization remains mandatory.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        with_plan!(self, p, &p.galois)
    }
    /// Complete high-level schedule counts, independent of private coefficients.
    #[must_use]
    pub fn operations(&self) -> &RamLfeBgvNonlinearOperationsV1 {
        with_plan!(self, p, &p.operations)
    }
    /// Conservative application/shared-routing residue payload; excludes backend,
    /// key/profile/plaintext scratch, allocation metadata and process RSS.
    #[must_use]
    pub fn owned_residue_bytes(&self) -> usize {
        with_plan!(self, p, p.owned_residue_bytes)
    }
    /// Canonical arithmetic units, including every repeated physical bootstrap.
    #[must_use]
    pub fn canonical_arithmetic_units(&self) -> u64 {
        with_plan!(self, p, p.work.arithmetic)
    }
    /// Separately declared scalar-word operations across the complete schedule.
    #[must_use]
    pub fn scalar_word_operations(&self) -> u64 {
        with_plan!(self, p, p.work.words)
    }
    /// Preadmitted complementary preparation scopes at the explicit count cap.
    #[must_use]
    pub fn preparation_requirement(&self) -> &RamLfeBgvNonlinearPreparationV1 {
        with_plan!(self, p, &p.preparation)
    }
    /// Exact source descriptors sharing one actual immutable bootstrap core.
    #[must_use]
    pub fn prepared_refresh_plans(&self) -> usize {
        with_plan!(self, p, p.refresh.as_ref().map_or(0, |r| r.plans.len()))
    }
    #[cfg(test)]
    fn bounded(&self) -> &PlanData<21, 60, 20, 20> {
        match &self.data {
            PlanKind::Bounded(p) => p,
            _ => panic!("bounded test plan"),
        }
    }
}
pub(super) const fn stage_count(shape: RamLfeBgvNonlinearShapeV1) -> usize {
    match shape {
        RamLfeBgvNonlinearShapeV1::Bounded => 16,
        RamLfeBgvNonlinearShapeV1::Refresh => 957,
    }
}
pub(super) const fn private_count(shape: RamLfeBgvNonlinearShapeV1) -> usize {
    match shape {
        RamLfeBgvNonlinearShapeV1::Bounded => 16,
        RamLfeBgvNonlinearShapeV1::Refresh => 638,
    }
}
pub(super) fn stage(shape: RamLfeBgvNonlinearShapeV1, index: usize) -> RamLfeBgvNonlinearStageV1 {
    match shape {
        RamLfeBgvNonlinearShapeV1::Bounded => RamLfeBgvNonlinearStageV1::BoundedLayer(
            u16::try_from(index).expect("sixteen bounded stage indices fit u16") + 1,
        ),
        RamLfeBgvNonlinearShapeV1::Refresh => RamLfeBgvNonlinearStageV1::RefreshGate {
            gate: u16::try_from(index / 3).expect("319 Refresh gate ordinals fit u16"),
            mode: match index % 3 {
                0 => RamLfeBgvRefreshMapV1::Initial,
                1 => RamLfeBgvRefreshMapV1::Square,
                _ => RamLfeBgvRefreshMapV1::Final,
            },
        },
    }
}
fn public_stage(stage: RamLfeBgvNonlinearStageV1) -> bool {
    matches!(
        stage,
        RamLfeBgvNonlinearStageV1::RefreshGate {
            mode: RamLfeBgvRefreshMapV1::Square,
            ..
        }
    )
}
fn repetitions(stage: RamLfeBgvNonlinearStageV1) -> usize {
    if matches!(
        stage,
        RamLfeBgvNonlinearStageV1::RefreshGate {
            mode: RamLfeBgvRefreshMapV1::Square,
            ..
        }
    ) {
        7
    } else {
        1
    }
}
impl<const I: usize, const O: usize, const S: usize, const U: usize> PlanData<I, O, S, U> {
    fn new(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
        schedule: &RamLfeBgvNonlinearScheduleV1,
        budget: RamLfeBgvNonlinearBudgetV1,
    ) -> Result<Self, RamLfeBgvNonlinearErrorV1> {
        let shape = schedule.layers.shape();
        if (I, O, S, U)
            != match shape {
                RamLfeBgvNonlinearShapeV1::Bounded => (21, 60, 20, 20),
                RamLfeBgvNonlinearShapeV1::Refresh => (4, 3, 3, 1),
            }
        {
            return Err(RamLfeBgvNonlinearErrorV1::Geometry);
        }
        let preparation = bgv_nonlinear_refresh::preparation_requirement::<I, O, S, U>(
            profile, precision, schedule, budget,
        )?;
        let encoding = SlotEncoding::new(profile, precision)?;
        if precision.modulus() != 257 || encoding.slots() != SLOTS {
            return Err(RamLfeBgvNonlinearErrorV1::UnsupportedProfile);
        }
        let required = RamLfeBgvNonlinearPlanV1::matrix_payload_bytes(shape);
        if required > budget.matrix_payload_bytes {
            return Err(RamLfeBgvNonlinearErrorV1::MatrixBudget {
                required,
                budget: budget.matrix_payload_bytes,
            });
        }
        let metadata = RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(shape)?;
        if metadata > budget.layout_metadata_bytes {
            return Err(SlotLinearError::LayoutBudget {
                required: metadata,
                budget: budget.layout_metadata_bytes,
            }
            .into());
        }
        let preparation_scratch = shape.preparation_scratch_bytes();
        if preparation_scratch > budget.preparation_scratch_bytes {
            return Err(SlotLinearError::PreparationBudget {
                required: preparation_scratch,
                budget: budget.preparation_scratch_bytes,
            }
            .into());
        }
        // The bounded stages alone have one identical immutable support graph.
        let shared_layout = if shape == RamLfeBgvNonlinearShapeV1::Bounded {
            Some(shape.layer_layout::<I, O>(stage(shape, 0), metadata)?)
        } else {
            None
        };
        let projection_layout = shape.projection_layout::<I>(metadata)?;
        let mut galois = encoding.broadcast_elements();
        if galois.len() != KEYS {
            return Err(RamLfeBgvNonlinearErrorV1::UnsupportedProfile);
        }
        let maximum_keys = KEYS
            .checked_add((profile.degree() / SLOTS).ilog2() as usize)
            .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?;
        galois
            .try_reserve_exact(maximum_keys - galois.len())
            .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
        let source =
            profile.plan_switch_to_level(&profile.plan_fresh(precision)?, schedule.input_level)?;
        let augmented = profile.plan_add_plain(&source, encoding.vector_bound())?;
        let state_level = if shape == RamLfeBgvNonlinearShapeV1::Refresh {
            schedule.layers.choice(0).routing_level
        } else {
            schedule.input_level
        };
        // A fresh public zero is constructed at this level; no existing source
        // is raised or assigned a smaller bound.
        let initial_state = profile.plan_trivial(precision, state_level)?;
        let mut sources: [CiphertextShape; I] = std::array::from_fn(|i| {
            if i == S {
                augmented.clone()
            } else {
                initial_state.clone()
            }
        });
        drop(initial_state);
        let model = CostModel::new(profile);
        let mut work = Work::default();
        work.add(
            0,
            u64::try_from(profile.degree())
                .map_err(|_| RamLfeBgvNonlinearErrorV1::Geometry)?
                .checked_mul(
                    u64::try_from(schedule.input_level + 1)
                        .map_err(|_| RamLfeBgvNonlinearErrorV1::Geometry)?,
                )
                .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
        )?;
        let mut refresh = schedule
            .refresh
            .as_ref()
            .map(|c| RefreshPlans::new(c.source_level, budget.maximum_refresh_plans))
            .transpose()?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(stage_count(shape))
            .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
        let mut operations = RamLfeBgvNonlinearOperationsV1 {
            plaintext_additions: 1,
            ..Default::default()
        };
        let mut peak = checked_add(
            residue_sum(profile, &sources)?,
            scaled_residues(profile, schedule.input_level, 1)?,
        )?;
        let initial_selection: [bool; I] =
            std::array::from_fn(|bank| shape == RamLfeBgvNonlinearShapeV1::Bounded || bank == S);
        let mut initial_refresh = [None; I];
        if let (Some(pool), Some(choice)) = (&mut refresh, &schedule.refresh) {
            initial_refresh = pool.group(
                profile,
                choice,
                RefreshSelection {
                    sources: &mut sources,
                    selected: &initial_selection,
                },
                &mut work,
                &mut operations.modulus_switches,
                &mut peak,
            )?;
            operations.bootstrap_calls += initial_selection.iter().filter(|&&yes| yes).count();
        }
        let mut private_index = 0;
        for index in 0..stage_count(shape) {
            let stage = stage(shape, index);
            let repeats = repetitions(stage);
            let choice = schedule.layers.choice(index);
            let prior = sources.clone();
            let prior_work = work;
            let prior_counts = operations;
            let updates = update_banks::<I, S, U>(stage)?;
            if O != 3 * U {
                return Err(RamLfeBgvNonlinearErrorV1::Geometry);
            }
            let entering_residues = residue_sum(profile, &sources)?;
            let largest_entering = sources.iter().try_fold(0, |largest, source| {
                Ok::<_, RamLfeBgvNonlinearErrorV1>(largest.max(scaled_residues(
                    profile,
                    source.level(),
                    1,
                )?))
            })?;
            for source in &mut sources {
                // Refresh carries inactive state and base owners exactly. A
                // schedule that would lower those owners is not this circuit.
                if shape == RamLfeBgvNonlinearShapeV1::Refresh
                    && source.level() != choice.routing_level
                {
                    return Err(RamLfeBgvNonlinearErrorV1::RefreshSchedule);
                }
                operations.modulus_switches = checked_add(
                    operations.modulus_switches,
                    source
                        .level()
                        .checked_sub(choice.routing_level)
                        .ok_or(RamLfeBgvNonlinearErrorV1::RefreshSchedule)?,
                )?;
                *source = work.lower(profile, source, choice.routing_level)?;
            }
            let layout = match &shared_layout {
                Some(layout) => layout.clone(),
                None => shape.layer_layout::<I, O>(stage, metadata)?,
            };
            let routing = SlotLinearPlan::with_layout(
                profile,
                &sources,
                layout,
                SlotLinearBudget {
                    coefficient_bytes: shape.maximum_layer_payload_bytes(),
                    layout_metadata_bytes: metadata,
                    preparation_scratch_bytes: preparation_scratch,
                    output_residue_bytes: scaled_residues(profile, choice.routing_level, O)?,
                    scratch_residue_bytes: choice.routing_scratch_bytes,
                },
            )?;
            work.route(profile, &routing)?;
            let mut operands = routing.output_shapes().clone();
            for operand in &mut operands {
                *operand = work.lower(profile, operand, choice.multiplication_level)?;
            }
            operations.modulus_switches = checked_add(
                operations.modulus_switches,
                O.checked_mul(
                    choice
                        .routing_level
                        .checked_sub(choice.multiplication_level)
                        .ok_or(RamLfeBgvNonlinearErrorV1::RefreshSchedule)?,
                )
                .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
            )?;
            for (product_bank, &state_bank) in updates.iter().enumerate() {
                let product =
                    profile.plan_multiply(&operands[product_bank], &operands[U + product_bank])?;
                sources[state_bank] =
                    profile.plan_add(&product, &operands[2 * U + product_bank])?;
                work.add(model.multiplication_units(choice.multiplication_level)?, 0)?;
                work.ciphertext_adds(profile, choice.multiplication_level, 1)?;
            }
            // The bounded circuit retains its existing base transfer. Refresh
            // moves all non-target owners unchanged at the routing level.
            if shape == RamLfeBgvNonlinearShapeV1::Bounded {
                operations.modulus_switches = checked_add(
                    operations.modulus_switches,
                    sources[S]
                        .level()
                        .checked_sub(choice.multiplication_level)
                        .ok_or(RamLfeBgvNonlinearErrorV1::RefreshSchedule)?,
                )?;
                sources[S] = work.lower(profile, &sources[S], choice.multiplication_level)?;
            }
            let carried_residues = sources
                .iter()
                .enumerate()
                .filter(|(bank, _)| !updates.contains(bank))
                .try_fold(0, |bytes, (_, source)| {
                    checked_add(bytes, scaled_residues(profile, source.level(), 1)?)
                })?;
            drop(operands);
            peak = peak.max(checked_add(
                scaled_residues(profile, choice.routing_level, I + O)?,
                routing.scratch_residue_bytes(),
            )?);
            peak = peak
                .max(checked_add(entering_residues, largest_entering)?)
                .max(scaled_residues(
                    profile,
                    choice.routing_level,
                    I - U + O + 1,
                )?)
                .max(checked_add(
                    carried_residues,
                    scaled_residues(profile, choice.multiplication_level, O + 4)?,
                )?);
            operations.routing_calls += 1;
            operations.galois_applications += routing.galois_applications();
            operations.plaintext_products += routing.plaintext_products();
            operations.ciphertext_additions += routing.plaintext_products() + U;
            operations.ciphertext_products += U;
            let products = ProductPlan {
                level: choice.multiplication_level,
                updates,
                state_outputs: std::array::from_fn(|i| sources[i].clone()),
                base_output: sources[S].clone(),
            };
            let selection: [bool; I] = std::array::from_fn(|bank| {
                shape == RamLfeBgvNonlinearShapeV1::Bounded || updates.contains(&bank)
            });
            let mut after_refresh = [None; I];
            if let (Some(pool), Some(choice)) = (&mut refresh, &schedule.refresh) {
                after_refresh = pool.group(
                    profile,
                    choice,
                    RefreshSelection {
                        sources: &mut sources,
                        selected: &selection,
                    },
                    &mut work,
                    &mut operations.modulus_switches,
                    &mut peak,
                )?;
                operations.bootstrap_calls += selection.iter().filter(|&&yes| yes).count();
            }
            // Reusing a routing/map owner is justified only by complete exact
            // source equality. Every physical repetition is still charged/run.
            if repeats > 1 {
                if sources != prior {
                    return Err(RamLfeBgvNonlinearErrorV1::RefreshSchedule);
                }
                let a = work.arithmetic - prior_work.arithmetic;
                let w = work.words - prior_work.words;
                let delta = operations_delta(&operations, &prior_counts);
                for _ in 1..repeats {
                    work.add(a, w)?;
                    operations.add_delta(&delta)?;
                }
            }
            let coefficients = if public_stage(stage) {
                Coefficients::Public(PreparedLayer {
                    stage,
                    map: shape.prepare_public_layer(stage, &routing)?,
                })
            } else {
                let at = private_index;
                private_index += 1;
                Coefficients::Private(at)
            };
            layers.push(LayerPlan {
                stage,
                choice,
                routing,
                products,
                after_refresh,
                repetitions: repeats,
                coefficients,
            });
        }
        debug_assert_eq!(private_index, private_count(shape));
        let entering_residues = residue_sum(profile, &sources)?;
        let largest_entering = sources.iter().try_fold(0, |largest, source| {
            Ok::<_, RamLfeBgvNonlinearErrorV1>(largest.max(scaled_residues(
                profile,
                source.level(),
                1,
            )?))
        })?;
        for source in &mut sources {
            operations.modulus_switches = checked_add(
                operations.modulus_switches,
                source
                    .level()
                    .checked_sub(schedule.projection_level)
                    .ok_or(RamLfeBgvNonlinearErrorV1::RefreshSchedule)?,
            )?;
            *source = work.lower(profile, source, schedule.projection_level)?;
        }
        let projection = SlotLinearPlan::with_layout(
            profile,
            &sources,
            projection_layout,
            SlotLinearBudget {
                coefficient_bytes: shape.projection_payload_bytes(),
                layout_metadata_bytes: metadata,
                preparation_scratch_bytes: preparation_scratch,
                output_residue_bytes: scaled_residues(profile, schedule.projection_level, 1)?,
                scratch_residue_bytes: schedule.projection_scratch_bytes,
            },
        )?;
        work.route(profile, &projection)?;
        let output = work.lower(
            profile,
            &projection.output_shapes()[0],
            schedule.output_level,
        )?;
        peak = peak
            .max(checked_add(entering_residues, largest_entering)?)
            .max(checked_add(
                scaled_residues(profile, schedule.projection_level, I + 1)?,
                projection.scratch_residue_bytes(),
            )?)
            .max(scaled_residues(profile, schedule.projection_level, 2)?);
        operations.routing_calls += 1;
        operations.galois_applications += projection.galois_applications();
        operations.plaintext_products += projection.plaintext_products();
        operations.ciphertext_additions += projection.plaintext_products();
        operations.modulus_switches += schedule.projection_level - schedule.output_level;
        if peak > budget.owned_residue_bytes {
            return Err(RamLfeBgvNonlinearErrorV1::ResidueBudget {
                required: peak,
                budget: budget.owned_residue_bytes,
            });
        }
        work.admit(budget)?;
        if let Some(pool) = &refresh {
            for plan in &pool.plans {
                for &element in plan.galois_elements() {
                    if !galois.contains(&element) {
                        if galois.len() == maximum_keys {
                            return Err(RamLfeBgvNonlinearErrorV1::Geometry);
                        }
                        galois.push(element);
                    }
                }
            }
            galois.sort_unstable();
        }
        let plan = Self {
            identity: *profile.identity(),
            binding: Arc::new(()),
            shape,
            source,
            augmented,
            state_level,
            layers,
            projection,
            projection_level: schedule.projection_level,
            output_level: schedule.output_level,
            output,
            galois,
            refresh,
            initial_refresh,
            preparation,
            work,
            operations,
            owned_residue_bytes: peak,
        };
        plan.check_retained_metadata(private_count(shape))?;
        Ok(plan)
    }
    fn check_retained_metadata(
        &self,
        map_capacity: usize,
    ) -> Result<(), RamLfeBgvNonlinearErrorV1> {
        let mut bytes = checked_add(
            size_of::<RamLfeBgvNonlinearPlanV1>(),
            size_of::<RamLfeBgvNonlinearProgramV1>(),
        )?;
        // Exact-plan binding allocation has a zero-sized body and two Arc words.
        bytes = checked_add(bytes, 2 * size_of::<usize>())?;
        bytes = checked_add(
            bytes,
            self.layers
                .capacity()
                .checked_mul(size_of::<LayerPlan<I, O, S, U>>())
                .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
        )?;
        bytes = checked_add(
            bytes,
            map_capacity
                .checked_mul(size_of::<PreparedLayer<I, O>>())
                .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
        )?;
        bytes = checked_add(
            bytes,
            self.galois
                .capacity()
                .checked_mul(size_of::<u32>())
                .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
        )?;
        for shape in [&self.source, &self.augmented, &self.output] {
            bytes = checked_add(bytes, shape.retained_metadata()?.retained_heap_bytes)?;
        }
        for layer in &self.layers {
            bytes = checked_add(
                bytes,
                layer.routing.retained_metadata()?.retained_heap_bytes,
            )?;
            for shape in layer
                .products
                .state_outputs
                .iter()
                .chain(std::iter::once(&layer.products.base_output))
            {
                bytes = checked_add(bytes, shape.retained_metadata()?.retained_heap_bytes)?;
            }
        }
        bytes = checked_add(
            bytes,
            self.projection.retained_metadata()?.retained_heap_bytes,
        )?;
        if let Some(pool) = &self.refresh {
            bytes = checked_add(
                bytes,
                pool.plans
                    .capacity()
                    .checked_mul(size_of::<iroha_fhe::bgv::BootstrapPlan>())
                    .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)?,
            )?;
            if let Some(first) = pool.plans.first() {
                bytes = checked_add(
                    bytes,
                    first
                        .retained_metadata_split()?
                        .shared_preparation
                        .retained_bytes()?,
                )?;
                for plan in &pool.plans {
                    if !first.shares_preparation(plan) {
                        return Err(RamLfeBgvNonlinearErrorV1::RefreshSchedule);
                    }
                    bytes = checked_add(
                        bytes,
                        plan.retained_metadata_split()?
                            .source_descriptor
                            .retained_heap_bytes,
                    )?;
                }
            }
        }
        if bytes > self.preparation.retained_metadata_bytes {
            return Err(RamLfeBgvNonlinearErrorV1::PreparationBudget {
                resource: "retained metadata coverage",
                required: bytes,
                budget: self.preparation.retained_metadata_bytes,
            });
        }
        Ok(())
    }
    #[cfg(test)]
    fn check_inputs(
        &self,
        profile: &BgvProfile,
        input: &LocalScalarCiphertext<'_>,
        key: &LocalBootstrapKey,
    ) -> Result<(), RamLfeBgvNonlinearErrorV1> {
        if profile.identity() != &self.identity
            || input.ciphertext().profile_identity() != &self.identity
        {
            return Err(BgvError::ProfileMismatch.into());
        }
        if input.shape() != self.source {
            return Err(RamLfeBgvNonlinearErrorV1::InputShape);
        }
        key.preflight_evaluation(profile, input.as_local(), &self.source, &self.galois)?;
        Ok(())
    }
}
fn residue_sum(
    profile: &BgvProfile,
    shapes: &[CiphertextShape],
) -> Result<usize, RamLfeBgvNonlinearErrorV1> {
    shapes.iter().try_fold(0, |bytes, shape| {
        checked_add(bytes, scaled_residues(profile, shape.level(), 1)?)
    })
}

fn operations_delta(
    now: &RamLfeBgvNonlinearOperationsV1,
    prior: &RamLfeBgvNonlinearOperationsV1,
) -> RamLfeBgvNonlinearOperationsV1 {
    RamLfeBgvNonlinearOperationsV1 {
        bootstrap_calls: now.bootstrap_calls - prior.bootstrap_calls,
        routing_calls: now.routing_calls - prior.routing_calls,
        galois_applications: now.galois_applications - prior.galois_applications,
        plaintext_products: now.plaintext_products - prior.plaintext_products,
        ciphertext_additions: now.ciphertext_additions - prior.ciphertext_additions,
        plaintext_additions: now.plaintext_additions - prior.plaintext_additions,
        ciphertext_products: now.ciphertext_products - prior.ciphertext_products,
        modulus_switches: now.modulus_switches - prior.modulus_switches,
    }
}
impl RamLfeBgvNonlinearOperationsV1 {
    fn add_delta(&mut self, delta: &Self) -> Result<(), RamLfeBgvNonlinearErrorV1> {
        self.bootstrap_calls = checked_add(self.bootstrap_calls, delta.bootstrap_calls)?;
        self.routing_calls = checked_add(self.routing_calls, delta.routing_calls)?;
        self.galois_applications =
            checked_add(self.galois_applications, delta.galois_applications)?;
        self.plaintext_products = checked_add(self.plaintext_products, delta.plaintext_products)?;
        self.ciphertext_additions =
            checked_add(self.ciphertext_additions, delta.ciphertext_additions)?;
        self.plaintext_additions =
            checked_add(self.plaintext_additions, delta.plaintext_additions)?;
        self.ciphertext_products =
            checked_add(self.ciphertext_products, delta.ciphertext_products)?;
        self.modulus_switches = checked_add(self.modulus_switches, delta.modulus_switches)?;
        Ok(())
    }
}
fn checked_add(a: usize, b: usize) -> Result<usize, RamLfeBgvNonlinearErrorV1> {
    a.checked_add(b).ok_or(RamLfeBgvNonlinearErrorV1::Geometry)
}
fn scaled_residues(
    profile: &BgvProfile,
    level: usize,
    count: usize,
) -> Result<usize, RamLfeBgvNonlinearErrorV1> {
    usize::try_from(CostModel::new(profile).ciphertext_residue_bytes(level, 2)?)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Geometry)?
        .checked_mul(count)
        .ok_or(RamLfeBgvNonlinearErrorV1::Geometry)
}

struct ProgramData<const I: usize, const O: usize> {
    binding: Arc<()>,
    layers: Vec<PreparedLayer<I, O>>,
    projection: SlotLinearMap<I, 1>,
}
enum ProgramKind {
    Bounded(ProgramData<21, 60>),
    Refresh(ProgramData<4, 3>),
}
/// Clearing private coefficient maps bound to one exact admitted public plan.
///
/// Preparation remains outside requests. This process-local owner adds no wire
/// encoding, input/key authentication, execution proof or qualification.
pub struct RamLfeBgvNonlinearProgramV1 {
    data: ProgramKind,
}
impl fmt::Debug for RamLfeBgvNonlinearProgramV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED RAM-LFE nonlinear BGV program]")
    }
}
impl RamLfeBgvNonlinearProgramV1 {
    /// Prepare private maps only after complete public class/resource admission.
    /// # Errors
    /// Refuses mismatched original class, metadata coverage or private compilation.
    pub fn from_normal_form(
        normal: &RamLfeNormalFormV1,
        plan: &RamLfeBgvNonlinearPlanV1,
    ) -> Result<Self, RamLfeBgvNonlinearErrorV1> {
        let data = match &plan.data {
            PlanKind::Bounded(p) => ProgramKind::Bounded(ProgramData::new(normal, p)?),
            PlanKind::Refresh(p) => ProgramKind::Refresh(ProgramData::new(normal, p)?),
        };
        Ok(Self { data })
    }
    /// Evaluate every padded physical stage and all 64 ordered output slots.
    ///
    /// Exact-plan identity, key origin/profile/source and complete key union are
    /// checked before memory or private reads. Only genuine `PackedInput` enters;
    /// general output never regains input admission provenance.
    /// # Errors
    /// Refuses another plan/class or canonical arithmetic/refresh failure. All
    /// consumed and partial ciphertext owners clear on failure or unwind.
    pub fn evaluate<'key>(
        &self,
        profile: &BgvProfile,
        plan: &RamLfeBgvNonlinearPlanV1,
        memory: &RamLfeInitialMemoryV1,
        input: RamLfeBgvPackedInputV1<'key>,
        key: &'key LocalBootstrapKey,
    ) -> Result<LocalBootstrapCiphertext<'key>, RamLfeBgvNonlinearErrorV1> {
        let result = match (&self.data, &plan.data) {
            (ProgramKind::Bounded(p), PlanKind::Bounded(q)) => {
                p.evaluate(profile, q, memory, input, key)
            }
            (ProgramKind::Refresh(p), PlanKind::Refresh(q)) => {
                p.evaluate(profile, q, memory, input, key)
            }
            _ => Err(RamLfeBgvNonlinearErrorV1::ProgramPlan),
        }?;
        Ok(result.into_local())
    }
}
impl<const I: usize, const O: usize> ProgramData<I, O> {
    fn new<const S: usize, const U: usize>(
        normal: &RamLfeNormalFormV1,
        plan: &PlanData<I, O, S, U>,
    ) -> Result<Self, RamLfeBgvNonlinearErrorV1> {
        if normal.class() != plan.shape.class() {
            return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch.into());
        }
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(private_count(plan.shape))
            .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
        plan.check_retained_metadata(layers.capacity())?;
        let compiler = RamLfeBgvNonlinearCompilerV1::from_normal_form(
            normal,
            plan.shape.private_payload_bytes(),
        )?;
        for public in &plan.layers {
            if let Coefficients::Private(index) = public.coefficients {
                if index != layers.len() {
                    return Err(RamLfeBgvNonlinearErrorV1::Geometry);
                }
                layers.push(PreparedLayer {
                    stage: public.stage,
                    map: compiler.prepare_layer(public.stage, &public.routing)?,
                });
                #[cfg(test)]
                tests::checkpoint("private layer prepared")?;
            }
        }
        let projection = compiler.prepare_projection(&plan.projection)?;
        #[cfg(test)]
        tests::checkpoint("private projection prepared")?;
        drop(compiler);
        plan.check_retained_metadata(layers.capacity())?;
        Ok(Self {
            binding: Arc::clone(&plan.binding),
            layers,
            projection,
        })
    }
    // Check public ordinal/target tags before consuming input or touching memory
    // or private coefficient rows. These local tags are not an external proof.
    fn check_layers<const S: usize, const U: usize>(
        &self,
        plan: &PlanData<I, O, S, U>,
    ) -> Result<(), RamLfeBgvNonlinearErrorV1> {
        if O != 3 * U || plan.layers.len() != stage_count(plan.shape) {
            return Err(RamLfeBgvNonlinearErrorV1::Geometry);
        }
        for (index, public) in plan.layers.iter().enumerate() {
            let expected = stage(plan.shape, index);
            let prepared = match &public.coefficients {
                Coefficients::Public(prepared) => prepared,
                Coefficients::Private(index) => self
                    .layers
                    .get(*index)
                    .ok_or(RamLfeBgvNonlinearErrorV1::ProgramPlan)?,
            };
            if public.stage != expected || prepared.stage != expected {
                return Err(RamLfeBgvNonlinearErrorV1::ProgramPlan);
            }
            if public.products.updates != update_banks::<I, S, U>(expected)? {
                return Err(RamLfeBgvNonlinearErrorV1::Geometry);
            }
        }
        Ok(())
    }
    fn evaluate<'key, const S: usize, const U: usize>(
        &self,
        profile: &BgvProfile,
        plan: &PlanData<I, O, S, U>,
        memory: &RamLfeInitialMemoryV1,
        input: RamLfeBgvPackedInputV1<'key>,
        key: &'key LocalBootstrapKey,
    ) -> Result<LocalScalarCiphertext<'key>, RamLfeBgvNonlinearErrorV1> {
        if !Arc::ptr_eq(&self.binding, &plan.binding) {
            return Err(RamLfeBgvNonlinearErrorV1::ProgramPlan);
        }
        self.check_layers(plan)?;
        let (original, mut state) = initialize_state(profile, plan, memory, input, key)?;
        drop(original);
        #[cfg(test)]
        tests::checkpoint("initial banks")?;
        if let Some(pool) = &plan.refresh {
            state = refresh_banks(profile, key, pool, &plan.initial_refresh, state)?;
        }
        for public in &plan.layers {
            let map = match &public.coefficients {
                Coefficients::Public(map) => map,
                Coefficients::Private(index) => self.layers.get(*index).ok_or(BgvError::Kernel)?,
            };
            for _ in 0..public.repetitions {
                state = execute_layer(profile, public, map, state, key)?;
                if let Some(pool) = &plan.refresh {
                    state = refresh_banks(profile, key, pool, &public.after_refresh, state)?;
                }
            }
            #[cfg(test)]
            r9_tests::observe_boundary(profile, public.stage, &state);
        }
        check_bank_order::<I>(&state)?;
        state = lower_state(profile, key, state, plan.projection_level)?;
        let inputs: [StateBank<'key>; I] = state.try_into().map_err(|_| BgvError::Kernel)?;
        let inputs = inputs.map(|bank| bank.value);
        let (originals, [projected]) = key.route_scalar(
            profile,
            &plan.projection,
            &self.projection,
            inputs,
            plan.owned_residue_bytes,
        )?;
        #[cfg(test)]
        tests::observe_route(&plan.projection);
        drop(originals);
        let output = lower(profile, key, projected, plan.output_level)?;
        debug_assert_eq!(output.shape(), plan.output);
        Ok(output)
    }
}
fn initialize_state<'key, const I: usize, const O: usize, const S: usize, const U: usize>(
    profile: &BgvProfile,
    plan: &PlanData<I, O, S, U>,
    memory: &RamLfeInitialMemoryV1,
    input: RamLfeBgvPackedInputV1<'key>,
    key: &'key LocalBootstrapKey,
) -> Result<(RamLfeBgvPackedInputV1<'key>, Vec<StateBank<'key>>), RamLfeBgvNonlinearErrorV1> {
    // This borrow grants no reverse conversion from ordinary local ciphertexts.
    if profile.identity() != &plan.identity
        || input.ciphertext().profile_identity() != &plan.identity
    {
        return Err(BgvError::ProfileMismatch.into());
    }
    if input.shape() != plan.source {
        return Err(RamLfeBgvNonlinearErrorV1::InputShape);
    }
    input.preflight(profile, key, &plan.source, &plan.galois)?;
    let encoding = SlotEncoding::new(profile, plan.source.plaintext_precision())?;
    let mut values = ClearingArray::<u64, SLOTS>::zeroed();
    for (out, &lane) in values[64..96].iter_mut().zip(memory.lanes()) {
        *out = u64::from(lane);
    }
    values[96] = 1;
    let packed = encoding.encode(profile, &values[..])?;
    let (input, base) = input.add_plain_retaining_input(profile, key, &packed)?;
    #[cfg(test)]
    tests::observe_plain_add();
    drop(packed);
    drop(values);
    debug_assert_eq!(base.shape(), plan.augmented);
    let mut banks = Vec::new();
    banks
        .try_reserve_exact(I)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    let zero = Plaintext::constant(profile, plan.source.plaintext_precision(), 0)?;
    for ordinal in 0..S {
        banks.push(StateBank {
            ordinal,
            value: key.trivial_scalar(profile, &encoding, &zero, plan.state_level)?,
        });
    }
    banks.push(StateBank {
        ordinal: S,
        value: base,
    });
    Ok((input, banks))
}
fn lower<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    mut value: LocalScalarCiphertext<'key>,
    target: usize,
) -> Result<LocalScalarCiphertext<'key>, RamLfeBgvNonlinearErrorV1> {
    if target > value.ciphertext().level() {
        return Err(BgvError::LevelMismatch {
            lhs: value.ciphertext().level(),
            rhs: target,
        }
        .into());
    }
    while value.ciphertext().level() > target {
        let next_level = value.ciphertext().level() - 1;
        value = key.switch_scalar(profile, value, next_level)?;
        #[cfg(test)]
        tests::observe_switch();
    }
    Ok(value)
}

fn lower_banks<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    values: Vec<LocalScalarCiphertext<'key>>,
    target: usize,
) -> Result<Vec<LocalScalarCiphertext<'key>>, RamLfeBgvNonlinearErrorV1> {
    let mut lowered = Vec::new();
    lowered
        .try_reserve_exact(values.len())
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    for value in values {
        lowered.push(lower(profile, key, value, target)?);
    }
    Ok(lowered)
}

fn lower_state<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    values: Vec<StateBank<'key>>,
    target: usize,
) -> Result<Vec<StateBank<'key>>, RamLfeBgvNonlinearErrorV1> {
    let mut lowered = Vec::new();
    lowered
        .try_reserve_exact(values.len())
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    for bank in values {
        lowered.push(StateBank {
            ordinal: bank.ordinal,
            value: lower(profile, key, bank.value, target)?,
        });
    }
    Ok(lowered)
}

fn refresh_banks<'key, const I: usize>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    plans: &RefreshPlans,
    indices: &[Option<usize>; I],
    values: Vec<StateBank<'key>>,
) -> Result<Vec<StateBank<'key>>, RamLfeBgvNonlinearErrorV1> {
    check_bank_order::<I>(&values)?;
    // Lower exactly selected sources before any refresh. Unselected owners
    // retain their actual shape and allocation throughout the boundary.
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(I)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    for bank in values {
        let value = if indices[bank.ordinal].is_some() {
            lower(profile, key, bank.value, plans.source_level)?
        } else {
            bank.value
        };
        pending.push(StateBank {
            ordinal: bank.ordinal,
            value,
        });
    }
    let mut refreshed = Vec::new();
    refreshed
        .try_reserve_exact(I)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    for bank in pending {
        let ordinal = bank.ordinal;
        let value = if let Some(index) = indices[ordinal] {
            let plan = plans.plans.get(index).ok_or(BgvError::Kernel)?;
            let value = key.refresh_scalar(profile, plan, bank.value)?;
            refreshed.push(StateBank { ordinal, value });
            #[cfg(test)]
            {
                tests::observe_refresh();
                tests::checkpoint("refreshed bank")?;
            }
            continue;
        } else {
            bank.value
        };
        refreshed.push(StateBank { ordinal, value });
    }
    Ok(refreshed)
}

fn execute_layer<'key, const I: usize, const O: usize, const S: usize, const U: usize>(
    profile: &BgvProfile,
    plan: &LayerPlan<I, O, S, U>,
    prepared: &PreparedLayer<I, O>,
    state: Vec<StateBank<'key>>,
    key: &'key LocalBootstrapKey,
) -> Result<Vec<StateBank<'key>>, RamLfeBgvNonlinearErrorV1> {
    if prepared.stage != plan.stage {
        return Err(RamLfeBgvNonlinearErrorV1::ProgramPlan);
    }
    if O != 3 * U || plan.products.updates != update_banks::<I, S, U>(plan.stage)? {
        return Err(RamLfeBgvNonlinearErrorV1::Geometry);
    }
    check_bank_order::<I>(&state)?;
    let state = lower_state(profile, key, state, plan.choice.routing_level)?;
    let inputs: [StateBank<'key>; I] = state.try_into().map_err(|_| BgvError::Kernel)?;
    let (originals, operands) = key.route_scalar(
        profile,
        &plan.routing,
        &prepared.map,
        inputs.map(|bank| bank.value),
        checked_add(
            scaled_residues(profile, plan.choice.routing_level, I + O)?,
            plan.routing.scratch_residue_bytes(),
        )?,
    )?;
    #[cfg(test)]
    tests::observe_route(&plan.routing);
    #[cfg(test)]
    tests::checkpoint("layer routed")?;
    let mut moved_operands = Vec::new();
    moved_operands
        .try_reserve_exact(O)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    moved_operands.extend(operands);
    finish_layer(
        profile,
        &plan.products,
        moved_operands,
        originals.map(Some),
        key,
    )
}

fn finish_layer<'key, const I: usize, const S: usize, const U: usize>(
    profile: &BgvProfile,
    plan: &ProductPlan<S, U>,
    operands: Vec<LocalScalarCiphertext<'key>>,
    mut originals: [Option<LocalScalarCiphertext<'key>>; I],
    key: &'key LocalBootstrapKey,
) -> Result<Vec<StateBank<'key>>, RamLfeBgvNonlinearErrorV1> {
    if I != S + 1 || operands.len() != 3 * U || plan.updates.iter().any(|&bank| bank >= S) {
        return Err(RamLfeBgvNonlinearErrorV1::Geometry);
    }
    for (i, &bank) in plan.updates.iter().enumerate() {
        if plan.updates[..i].contains(&bank) {
            return Err(RamLfeBgvNonlinearErrorV1::Geometry);
        }
        // Every source has already been read by the route. Active old owners
        // can retire now; inactive owners remain the actual returned inputs.
        drop(originals[bank].take());
    }
    let mut operands = lower_banks(profile, key, operands, plan.level)?;
    for (bank, original) in originals.iter_mut().enumerate() {
        if let Some(value) = original.take() {
            let expected = if bank == S {
                &plan.base_output
            } else {
                &plan.state_outputs[bank]
            };
            let value = lower(profile, key, value, expected.level())?;
            if value.shape() != *expected {
                return Err(RamLfeBgvNonlinearErrorV1::InputShape);
            }
            *original = Some(value);
        }
    }
    let c = operands.split_off(2 * U);
    let b = operands.split_off(U);
    let a = operands;
    for (&bank, ((left, right), addend)) in plan.updates.iter().zip(a.into_iter().zip(b).zip(c)) {
        let product = key.multiply_scalar(profile, left, right)?;
        #[cfg(test)]
        {
            tests::observe_product();
            tests::checkpoint("layer product")?;
        }
        let value = key.add_scalar(profile, product, addend)?;
        #[cfg(test)]
        tests::observe_add();
        debug_assert_eq!(value.shape(), plan.state_outputs[bank]);
        originals[bank] = Some(value);
    }
    let mut next = Vec::new();
    next.try_reserve_exact(I)
        .map_err(|_| RamLfeBgvNonlinearErrorV1::Allocation)?;
    for (ordinal, value) in originals.into_iter().enumerate() {
        next.push(StateBank {
            ordinal,
            value: value.ok_or(BgvError::Kernel)?,
        });
    }
    Ok(next)
}

#[cfg(test)]
#[path = "bgv_nonlinear_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "bgv_nonlinear_bounded_refresh_tests.rs"]
mod independent_refresh_tests;

#[cfg(test)]
#[path = "bgv_nonlinear_r9_tests.rs"]
mod r9_tests;
