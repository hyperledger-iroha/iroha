//! Complete fixed-shape bounded arithmetic under an explicit public schedule.
//!
//! Every one of sixteen layers routes 21 banks to all 60 A/B/C banks and
//! computes all 20 packed A*B+C products, including zero and carry rows. The
//! original bank is masked input plus per-request lanes and one. The public
//! plan derives every transfer before any private coefficient is prepared.
//! Caller-supplied levels and scratch ceilings do not select cryptographic
//! parameters or override noise refusal. There is no global schedule optimizer.
//!
//! This is an unqualified arithmetic algorithm, not production admission.
//! TODO(R.7, C.3, R.11): qualify parameters and complete input/key authentication,
//! committed execution proofs, circuit-privacy sanitization and full resource
//! qualification. The existing bounded candidate can fail this full schedule;
//! no class limit, resource cap or refusal is weakened to make it fit.

use super::bgv_bounded_refresh::{
    self, RamLfeBgvBoundedPreparationV1, RamLfeBgvBoundedRefreshChoiceV1, RefreshPlans, Work,
};
use super::{
    RamLfeBgvNonlinearCompileErrorV1, RamLfeBgvNonlinearCompilerV1, RamLfeBgvNonlinearShapeV1,
    RamLfeBgvNonlinearStageV1, RamLfeBgvPackedInputV1, RamLfeClassV1, RamLfeInitialMemoryV1,
    RamLfeNormalFormV1, clearing::ClearingArray,
};
use iroha_fhe::bgv::{
    BgvError, BgvProfile, BootstrapError, CiphertextShape, LocalBootstrapCiphertext,
    LocalBootstrapKey, Plaintext, PlaintextPrecision, PlanMetadataError, SlotEncoding,
    SlotLinearBudget, SlotLinearError, SlotLinearMap, SlotLinearPlan, cost::CostModel,
};
use std::fmt;
use thiserror::Error;

pub(super) const LAYERS: usize = 16;
pub(super) const INPUTS: usize = 21;
pub(super) const STATE: usize = 20;
pub(super) const OPERANDS: usize = 60;
const SLOTS: usize = 128;
pub(super) const KEYS: usize = 7;

/// Public routing and multiplication choices for one complete physical layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RamLfeBgvBoundedLayerChoiceV1 {
    /// Common level of all 21 banks before dense routing.
    pub routing_level: usize,
    /// Level of all 60 routed operands and the retained base bank before products.
    pub multiplication_level: usize,
    /// Public ceiling for shared routing-owned cached/temporary residue bytes.
    pub routing_scratch_bytes: usize,
}

/// Public schedule; every choice is fixed independently of a private program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamLfeBgvBoundedScheduleV1 {
    /// None uses the same arithmetic without physical refresh. Some executes
    /// all21 initial and post-layer refreshes, independent of the hidden tape.
    pub refresh: Option<RamLfeBgvBoundedRefreshChoiceV1>,
    /// All sixteen layers in order; no private count can shorten them.
    pub layers: [RamLfeBgvBoundedLayerChoiceV1; LAYERS],
    /// Common bank level before the final ordered-output projection.
    pub projection_level: usize,
    /// Output level before applying the final 64-slot prefix mask.
    pub mask_level: usize,
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
pub struct RamLfeBgvBoundedBudgetV1 {
    /// Maximum distinct exactly matched bootstrap source plans;0 without
    /// refresh,1..=357 with it. All357 physical calls still execute.
    pub maximum_refresh_plans: usize,
    /// Complete retained payload plus maximum sequential preparation scratch.
    pub preparation_payload_bytes: usize,
    /// Complementary owning headers/public vectors/Natural-capacity peak.
    pub preparation_metadata_bytes: usize,
    /// Complete CostModel arithmetic including every physical bootstrap call.
    /// Excludes encoding, preparation, copies/clears, keys and proof work.
    pub canonical_arithmetic_units: u64,
    /// Complete separately counted explicit scalar-word operations. Internal
    /// kernel additions retain CostModel's exclusions; this is not CPU time.
    pub scalar_word_operations: u64,
    /// Maximum bytes of all sixteen private matrices and the projection.
    pub matrix_payload_bytes: usize,
    /// Maximum public layout payload, counted once for the shared layer layout
    /// and once for the distinct projection layout; excludes handles/allocators.
    pub layout_metadata_bytes: usize,
    /// Maximum clearing row payload used only during owner-side preparation.
    pub preparation_scratch_bytes: usize,
    /// Maximum application/shared-routing-owned ciphertext residue payload.
    pub owned_residue_bytes: usize,
}

/// Exact counts of the public high-level arithmetic schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RamLfeBgvBoundedOperationsV1 {
    /// Complete physical bootstraps; their inner operations are accounted by
    /// the aggregate work totals, separately from the outer-layer counts below.
    pub bootstrap_calls: usize,
    /// Shared complete linear transformations: sixteen layers and one projection.
    pub routing_calls: usize,
    /// Key-switched Galois applications inside shared routing.
    pub galois_applications: usize,
    /// Plaintext products including original input and final output masks.
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
pub enum RamLfeBgvBoundedErrorV1 {
    /// Checked public dimensions, counts or byte/work arithmetic overflowed.
    #[error("bounded evaluation geometry overflow")]
    Geometry,
    /// The public refresh mode/count/context/level vector is inconsistent.
    #[error("invalid bounded physical refresh schedule")]
    RefreshSchedule,
    /// Exact shape deduplication needs more admitted prepared plans.
    #[error("bounded refresh needs {required} plans, budget {budget}")]
    RefreshPlanBudget {
        /// Minimum distinct count discovered before another plan is allocated.
        required: usize,
        /// Explicit preadmitted count ceiling.
        budget: usize,
    },
    /// A complete scoped preparation ceiling was exceeded.
    #[error("bounded {resource} requires {required} bytes, budget {budget}")]
    PreparationBudget {
        /// Named complementary storage scope.
        resource: &'static str,
        /// Checked required storage.
        required: usize,
        /// Explicit byte ceiling.
        budget: usize,
    },
    /// A complete declared evaluation work ceiling was exceeded.
    #[error("bounded {resource} requires {required}, budget {budget}")]
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
    #[error("bounded BGV evaluation requires 128 F257 slots")]
    UnsupportedProfile,
    /// The exact matrix payload exceeds its explicit ceiling.
    #[error("bounded matrices require {required} bytes, budget {budget}")]
    MatrixBudget {
        /// Exact required matrix coefficient bytes.
        required: usize,
        /// Caller-supplied byte ceiling.
        budget: usize,
    },
    /// A conservative application/routing residue bound exceeds its ceiling.
    #[error("bounded arithmetic owns up to {required} residue bytes, budget {budget}")]
    ResidueBudget {
        /// Conservative payload bound.
        required: usize,
        /// Caller-supplied byte ceiling.
        budget: usize,
    },
    /// The input is not the planned fresh, two-component, top-level shape.
    #[error("bounded BGV input does not have the required fresh shape")]
    InputShape,
    /// A public-size container reservation failed.
    #[error("bounded BGV container allocation failed")]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LayerPlan {
    choice: RamLfeBgvBoundedLayerChoiceV1,
    routing: SlotLinearPlan<21, 60>,
    products: ProductPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProductPlan {
    level: usize,
    state_outputs: [CiphertextShape; STATE],
    base_output: CiphertextShape,
}

/// Immutable complete public arithmetic plan with exact intermediate transfers.
#[derive(Debug)]
pub struct RamLfeBgvBoundedPlanV1 {
    identity: [u8; 32],
    fresh: CiphertextShape,
    augmented: CiphertextShape,
    layers: Vec<LayerPlan>,
    projection: SlotLinearPlan<21, 1>,
    projection_level: usize,
    mask_level: usize,
    output: CiphertextShape,
    galois: Vec<u32>,
    refresh: Option<RefreshPlans>,
    preparation: RamLfeBgvBoundedPreparationV1,
    work: Work,
    operations: RamLfeBgvBoundedOperationsV1,
    owned_residue_bytes: usize,
}

impl RamLfeBgvBoundedPlanV1 {
    /// Derive every transfer at explicit effective F257 precision in one master profile.
    ///
    /// The master p^E error scale remains unchanged; only the message context
    /// is F257. A master exponent of one follows this same ordinary path.
    ///
    /// Routing splits are selected by the shared owner under exactly the
    /// supplied scratch ceilings and then retained in this plan. A later
    /// product or mask can refuse even if a different split would have fit;
    /// this constructor does not search for a globally optimal schedule.
    ///
    /// # Errors
    /// Refuses incompatible profiles, exceeded payload ceilings, upward level
    /// moves and every shared intermediate noise failure before private work.
    pub fn new(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
        schedule: &RamLfeBgvBoundedScheduleV1,
        budget: RamLfeBgvBoundedBudgetV1,
    ) -> Result<Self, RamLfeBgvBoundedErrorV1> {
        let preparation =
            bgv_bounded_refresh::preparation_requirement(profile, precision, schedule, budget)?;
        let encoding = SlotEncoding::new(profile, precision)?;
        if precision.modulus() != 257 || encoding.slots() != SLOTS {
            return Err(RamLfeBgvBoundedErrorV1::UnsupportedProfile);
        }
        let required = Self::matrix_payload_bytes();
        if required > budget.matrix_payload_bytes {
            return Err(RamLfeBgvBoundedErrorV1::MatrixBudget {
                required,
                budget: budget.matrix_payload_bytes,
            });
        }
        let metadata = Self::layout_metadata_bytes()?;
        if metadata > budget.layout_metadata_bytes {
            return Err(SlotLinearError::LayoutBudget {
                required: metadata,
                budget: budget.layout_metadata_bytes,
            }
            .into());
        }
        let preparation_scratch = RamLfeBgvNonlinearShapeV1::Bounded.preparation_scratch_bytes();
        if preparation_scratch > budget.preparation_scratch_bytes {
            return Err(SlotLinearError::PreparationBudget {
                required: preparation_scratch,
                budget: budget.preparation_scratch_bytes,
            }
            .into());
        }
        // All sixteen plans and maps clone this one immutable public allocation.
        let layer_layout = RamLfeBgvNonlinearShapeV1::Bounded
            .layer_layout::<21, 60>(RamLfeBgvNonlinearStageV1::BoundedLayer(1), metadata)?;
        let projection_layout =
            RamLfeBgvNonlinearShapeV1::Bounded.projection_layout::<21>(metadata)?;
        let mut galois = encoding.broadcast_elements();
        if galois.len() != KEYS {
            return Err(RamLfeBgvBoundedErrorV1::UnsupportedProfile);
        }
        let maximum_keys = KEYS
            .checked_add((profile.degree() / SLOTS).ilog2() as usize)
            .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?;
        galois
            .try_reserve_exact(maximum_keys - galois.len())
            .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
        let fresh = profile.plan_fresh(precision)?;
        let masked = profile.plan_mul_plain(&fresh, encoding.public_prefix_mask_bound(64)?)?;
        let augmented = profile.plan_add_plain(&masked, encoding.vector_bound())?;
        drop(masked);
        // Trivial zero ciphertexts have the public bound128, never a zero bound.
        let initial_state = profile.plan_trivial(precision, profile.top_level())?;
        let mut state: [CiphertextShape; STATE] = std::array::from_fn(|_| initial_state.clone());
        drop(initial_state);
        let mut base = augmented.clone();
        let model = CostModel::new(profile);
        let mut work = Work::default();
        work.add(
            model.plaintext_multiplication_units(profile.top_level())?,
            u64::try_from(profile.degree())
                .map_err(|_| RamLfeBgvBoundedErrorV1::Geometry)?
                .checked_mul(
                    u64::try_from(profile.top_level() + 1)
                        .map_err(|_| RamLfeBgvBoundedErrorV1::Geometry)?,
                )
                .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?,
        )?;
        let mut refresh = schedule
            .refresh
            .as_ref()
            .map(|choice| RefreshPlans::new(choice.source_level, budget.maximum_refresh_plans))
            .transpose()?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(LAYERS)
            .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
        let mut operations = RamLfeBgvBoundedOperationsV1 {
            plaintext_products: 2,
            plaintext_additions: 1,
            ..Default::default()
        };
        let mut peak = scaled_residues(profile, profile.top_level(), INPUTS + 1)?;
        if let (Some(refresh), Some(choice)) = (&mut refresh, &schedule.refresh) {
            let mut sources = std::array::from_fn(|i| {
                if i == STATE {
                    base.clone()
                } else {
                    state[i].clone()
                }
            });
            refresh.initial = refresh.group(
                profile,
                choice,
                &mut sources,
                &mut work,
                &mut operations.modulus_switches,
                &mut peak,
            )?;
            for i in 0..STATE {
                state[i] = sources[i].clone();
            }
            base = sources[STATE].clone();
            operations.bootstrap_calls += INPUTS;
        }
        for (layer_index, choice) in schedule.layers.iter().copied().enumerate() {
            let entry = state[0].level();
            let mut at_route = state.clone();
            for shape in &mut at_route {
                *shape = work.lower(profile, shape, choice.routing_level)?;
            }
            let base_route = work.lower(profile, &base, choice.routing_level)?;
            let inputs = std::array::from_fn(|i| {
                if i == STATE {
                    base_route.clone()
                } else {
                    at_route[i].clone()
                }
            });
            drop(at_route);
            let routing = SlotLinearPlan::with_layout(
                profile,
                &inputs,
                layer_layout.clone(),
                SlotLinearBudget {
                    coefficient_bytes: RamLfeBgvNonlinearShapeV1::Bounded
                        .maximum_layer_payload_bytes(),
                    layout_metadata_bytes: metadata,
                    preparation_scratch_bytes: preparation_scratch,
                    output_residue_bytes: scaled_residues(profile, choice.routing_level, OPERANDS)?,
                    scratch_residue_bytes: choice.routing_scratch_bytes,
                },
            )?;
            drop(inputs);
            work.route(profile, &routing)?;
            let mut operands = routing.output_shapes().clone();
            for shape in &mut operands {
                *shape = work.lower(profile, shape, choice.multiplication_level)?;
            }
            for bank in 0..STATE {
                let product = profile.plan_multiply(&operands[bank], &operands[STATE + bank])?;
                state[bank] = profile.plan_add(&product, &operands[2 * STATE + bank])?;
                work.add(model.multiplication_units(choice.multiplication_level)?, 0)?;
                work.ciphertext_adds(profile, choice.multiplication_level, 1)?;
            }
            base = work.lower(profile, &base_route, choice.multiplication_level)?;
            drop(base_route);
            drop(operands);
            // At routing, old21 + retained60 + the shared owner's scratch coexist.
            peak = peak.max(checked_add(
                scaled_residues(profile, choice.routing_level, INPUTS + OPERANDS)?,
                routing.scratch_residue_bytes(),
            )?);
            // One switch output coexists with all current source banks.
            peak = peak.max(scaled_residues(profile, entry, INPUTS + 1)?);
            peak = peak.max(scaled_residues(
                profile,
                choice.routing_level,
                OPERANDS + 2,
            )?);
            // Conservative wrapper product/add bound, with tensor/relinearized
            // ciphertext payloads; backend polynomial/NTT scratch is separate.
            peak = peak.max(scaled_residues(
                profile,
                choice.multiplication_level,
                OPERANDS + 5,
            )?);
            operations.routing_calls += 1;
            operations.galois_applications += routing.galois_applications();
            operations.plaintext_products += routing.plaintext_products();
            operations.ciphertext_additions += routing.plaintext_products() + STATE;
            operations.ciphertext_products += STATE;
            operations.modulus_switches += INPUTS * (entry - choice.routing_level)
                + (OPERANDS + 1) * (choice.routing_level - choice.multiplication_level);
            layers.push(LayerPlan {
                choice,
                routing,
                products: ProductPlan {
                    level: choice.multiplication_level,
                    state_outputs: state.clone(),
                    base_output: base.clone(),
                },
            });
            if let (Some(refresh), Some(choice)) = (&mut refresh, &schedule.refresh) {
                let mut sources = std::array::from_fn(|i| {
                    if i == STATE {
                        base.clone()
                    } else {
                        state[i].clone()
                    }
                });
                refresh.layers[layer_index] = refresh.group(
                    profile,
                    choice,
                    &mut sources,
                    &mut work,
                    &mut operations.modulus_switches,
                    &mut peak,
                )?;
                for i in 0..STATE {
                    state[i] = sources[i].clone();
                }
                base = sources[STATE].clone();
                operations.bootstrap_calls += INPUTS;
            }
        }
        let entry = state[0].level();
        for shape in &mut state {
            *shape = work.lower(profile, shape, schedule.projection_level)?;
        }
        base = work.lower(profile, &base, schedule.projection_level)?;
        let inputs = std::array::from_fn(|i| {
            if i == STATE {
                base.clone()
            } else {
                state[i].clone()
            }
        });
        let projection = SlotLinearPlan::with_layout(
            profile,
            &inputs,
            projection_layout,
            SlotLinearBudget {
                coefficient_bytes: RamLfeBgvNonlinearShapeV1::Bounded.projection_payload_bytes(),
                layout_metadata_bytes: metadata,
                preparation_scratch_bytes: preparation_scratch,
                output_residue_bytes: scaled_residues(profile, schedule.projection_level, 1)?,
                scratch_residue_bytes: schedule.projection_scratch_bytes,
            },
        )?;
        work.route(profile, &projection)?;
        let before_mask =
            work.lower(profile, &projection.output_shapes()[0], schedule.mask_level)?;
        let output =
            profile.plan_mul_plain(&before_mask, encoding.public_prefix_mask_bound(64)?)?;
        work.add(
            model.plaintext_multiplication_units(schedule.mask_level)?,
            0,
        )?;
        peak = peak.max(scaled_residues(profile, entry, INPUTS + 1)?);
        peak = peak.max(checked_add(
            scaled_residues(profile, schedule.projection_level, INPUTS + 1)?,
            projection.scratch_residue_bytes(),
        )?);
        peak = peak.max(scaled_residues(profile, schedule.projection_level, 2)?);
        operations.routing_calls += 1;
        operations.galois_applications += projection.galois_applications();
        operations.plaintext_products += projection.plaintext_products();
        operations.ciphertext_additions += projection.plaintext_products();
        operations.modulus_switches += INPUTS * (entry - schedule.projection_level)
            + schedule.projection_level
            - schedule.mask_level;
        if peak > budget.owned_residue_bytes {
            return Err(RamLfeBgvBoundedErrorV1::ResidueBudget {
                required: peak,
                budget: budget.owned_residue_bytes,
            });
        }
        work.admit(budget)?;
        if let Some(refresh) = &refresh {
            for plan in &refresh.plans {
                for &element in plan.galois_elements() {
                    if !galois.contains(&element) {
                        if galois.len() == maximum_keys {
                            return Err(RamLfeBgvBoundedErrorV1::Geometry);
                        }
                        galois.push(element);
                    }
                }
            }
            galois.sort_unstable();
        }
        let plan = Self {
            identity: *profile.identity(),
            fresh,
            augmented,
            layers,
            projection,
            projection_level: schedule.projection_level,
            mask_level: schedule.mask_level,
            output,
            galois,
            refresh,
            preparation,
            work,
            operations,
            owned_residue_bytes: peak,
        };
        plan.check_retained_metadata(LAYERS)?;
        Ok(plan)
    }

    /// Exact payload of the sixteen prepared layer matrices and final projection.
    #[must_use]
    pub const fn matrix_payload_bytes() -> usize {
        LAYERS * RamLfeBgvNonlinearShapeV1::Bounded.maximum_layer_payload_bytes()
            + RamLfeBgvNonlinearShapeV1::Bounded.projection_payload_bytes()
    }
    /// Resident public metadata for one shared layer layout and one projection.
    ///
    /// Counts layout allocation payloads and Arc control words, excluding per-plan
    /// handles and allocator headers. All sixteen layers share the same layout.
    ///
    /// # Errors
    /// Reports checked public byte arithmetic overflow.
    pub fn layout_metadata_bytes() -> Result<usize, RamLfeBgvNonlinearCompileErrorV1> {
        RamLfeBgvNonlinearShapeV1::Bounded
            .layer_layout_metadata_bytes::<21, 60>(RamLfeBgvNonlinearStageV1::BoundedLayer(1))?
            .checked_add(
                RamLfeBgvNonlinearShapeV1::Bounded.projection_layout_metadata_bytes::<21>()?,
            )
            .ok_or(SlotLinearError::GeometryOverflow.into())
    }
    /// Exact admitted output shape of every evaluation under this plan.
    #[must_use]
    pub const fn output_shape(&self) -> &CiphertextShape {
        &self.output
    }
    /// Exact routing/trace Galois union; relinearization is additionally mandatory.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        &self.galois
    }
    /// Exact high-level operation counts, independent of the private program.
    #[must_use]
    pub const fn operations(&self) -> &RamLfeBgvBoundedOperationsV1 {
        &self.operations
    }
    /// Conservative application/shared-routing-owned ciphertext residue payload.
    ///
    /// Includes the consumed source owners; excludes keys, profile and plaintext/NTT/conversion scratch
    /// and allocation metadata; it is not a complete process memory bound.
    #[must_use]
    pub const fn owned_residue_bytes(&self) -> usize {
        self.owned_residue_bytes
    }

    /// Complete declared canonical arithmetic units, including all bootstrap calls.
    #[must_use]
    pub const fn canonical_arithmetic_units(&self) -> u64 {
        self.work.arithmetic
    }
    /// Complete separately declared scalar-word operations, including all calls.
    #[must_use]
    pub const fn scalar_word_operations(&self) -> u64 {
        self.work.words
    }
    /// Preadmitted complementary preparation scopes at the explicit count ceiling.
    #[must_use]
    pub const fn preparation_requirement(&self) -> &RamLfeBgvBoundedPreparationV1 {
        &self.preparation
    }
    /// Distinct fully prepared source shapes; physical calls are counted separately.
    #[must_use]
    pub fn prepared_refresh_plans(&self) -> usize {
        self.refresh
            .as_ref()
            .map_or(0, |refresh| refresh.plans.len())
    }

    fn check_retained_metadata(&self, map_capacity: usize) -> Result<(), RamLfeBgvBoundedErrorV1> {
        let mut bytes = checked_add(size_of::<Self>(), size_of::<RamLfeBgvBoundedProgramV1>())?;
        bytes = checked_add(
            bytes,
            self.layers
                .capacity()
                .checked_mul(size_of::<LayerPlan>())
                .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?,
        )?;
        let maps = map_capacity;
        bytes = checked_add(
            bytes,
            maps.checked_mul(size_of::<SlotLinearMap<21, 60>>())
                .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?,
        )?;
        bytes = checked_add(
            bytes,
            self.galois
                .capacity()
                .checked_mul(size_of::<u32>())
                .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?,
        )?;
        for shape in [&self.fresh, &self.augmented, &self.output] {
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
        if let Some(refresh) = &self.refresh {
            bytes = checked_add(
                bytes,
                refresh
                    .plans
                    .capacity()
                    .checked_mul(size_of::<iroha_fhe::bgv::BootstrapPlan>())
                    .ok_or(RamLfeBgvBoundedErrorV1::Geometry)?,
            )?;
            for plan in &refresh.plans {
                bytes = checked_add(bytes, plan.retained_metadata()?.retained_heap_bytes)?;
            }
        }
        if bytes > self.preparation.retained_metadata_bytes {
            return Err(RamLfeBgvBoundedErrorV1::PreparationBudget {
                resource: "retained metadata coverage",
                required: bytes,
                budget: self.preparation.retained_metadata_bytes,
            });
        }
        Ok(())
    }

    fn check_inputs(
        &self,
        profile: &BgvProfile,
        input: &LocalBootstrapCiphertext<'_>,
        key: &LocalBootstrapKey,
    ) -> Result<(), RamLfeBgvBoundedErrorV1> {
        if profile.identity() != &self.identity
            || input.ciphertext().profile_identity() != &self.identity
        {
            return Err(BgvError::ProfileMismatch.into());
        }
        if input.shape() != self.fresh {
            return Err(RamLfeBgvBoundedErrorV1::InputShape);
        }
        key.preflight_evaluation(profile, input, &self.fresh, &self.galois)?;
        Ok(())
    }
}

fn checked_add(a: usize, b: usize) -> Result<usize, RamLfeBgvBoundedErrorV1> {
    a.checked_add(b)
        .ok_or(SlotLinearError::GeometryOverflow.into())
}
fn scaled_residues(
    profile: &BgvProfile,
    level: usize,
    count: usize,
) -> Result<usize, RamLfeBgvBoundedErrorV1> {
    let one = usize::try_from(CostModel::new(profile).ciphertext_residue_bytes(level, 2)?)
        .map_err(|_| SlotLinearError::GeometryOverflow)?;
    one.checked_mul(count)
        .ok_or(SlotLinearError::GeometryOverflow.into())
}

/// Private complete prepared bounded matrices for one profile.
///
/// Construction happens outside observable requests and retains all fixed
/// matrices, including zero rows. No private preparation occurs during eval.
/// The tape remains the committed function; this owner adds no wire encoding,
/// authentication, execution proof or cryptographic qualification.
pub struct RamLfeBgvBoundedProgramV1 {
    identity: [u8; 32],
    layers: Vec<SlotLinearMap<21, 60>>,
    projection: SlotLinearMap<21, 1>,
}
impl fmt::Debug for RamLfeBgvBoundedProgramV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED RAM-LFE bounded BGV program]")
    }
}
impl RamLfeBgvBoundedProgramV1 {
    /// Compile and prepare the fixed private matrices after complete public admission.
    ///
    /// Canonical dimensions and actual map-container capacity are checked before
    /// private compiler allocation. The compiler and one clearing row coexist
    /// with retained maps under the admitted peak, then the compiler drops.
    ///
    /// # Errors
    /// Refuses another class/dimension, metadata coverage, container, compiler
    /// or shared-map allocation/validation errors. Partial private owners clear.
    pub fn from_normal_form(
        normal: &RamLfeNormalFormV1,
        plan: &RamLfeBgvBoundedPlanV1,
    ) -> Result<Self, RamLfeBgvBoundedErrorV1> {
        if normal.class() != RamLfeClassV1::Bounded {
            return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch.into());
        }
        if normal.gate_count() > 255 || normal.depth() > 16 {
            return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm.into());
        }
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(LAYERS)
            .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
        plan.check_retained_metadata(layers.capacity())?;
        let compiler = RamLfeBgvNonlinearCompilerV1::from_normal_form(
            normal,
            RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
        )?;
        for (layer, public) in plan.layers.iter().enumerate() {
            layers.push(compiler.prepare_layer(
                RamLfeBgvNonlinearStageV1::BoundedLayer(layer as u16 + 1),
                &public.routing,
            )?);
            #[cfg(test)]
            tests::checkpoint("private layer prepared")?;
        }
        let projection = compiler.prepare_projection(&plan.projection)?;
        #[cfg(test)]
        tests::checkpoint("private projection prepared")?;
        drop(compiler);
        let prepared = Self {
            identity: plan.identity,
            layers,
            projection,
        };
        plan.check_retained_metadata(prepared.layers.capacity())?;
        Ok(prepared)
    }

    /// Evaluate every fixed physical layer and all 64 padded ordered outputs.
    ///
    /// The sole input constructor supplies actual admitted scalar-slot packing.
    /// Same-owner/profile/fresh-input/all-key checks precede request-memory and
    /// private matrix reads. Every operation preserves closed local origin;
    /// outputs do not regain canonical byte-input provenance. External input/key
    /// authentication, committed execution and circuit privacy remain separate.
    ///
    /// # Errors
    /// Refuses mismatched input/key/program shapes and canonical arithmetic or
    /// refresh errors. All consumed and partial ciphertext owners clear on failure.
    pub fn evaluate<'key>(
        &self,
        profile: &BgvProfile,
        plan: &RamLfeBgvBoundedPlanV1,
        memory: &RamLfeInitialMemoryV1,
        input: RamLfeBgvPackedInputV1<'key>,
        key: &'key LocalBootstrapKey,
    ) -> Result<LocalBootstrapCiphertext<'key>, RamLfeBgvBoundedErrorV1> {
        if self.identity != plan.identity {
            return Err(BgvError::ProfileMismatch.into());
        }
        let input = input.into_local();
        plan.check_inputs(profile, &input, key)?;
        let (original, mut state) = initialize_state(profile, plan, memory, input, key)?;
        drop(original);
        #[cfg(test)]
        tests::checkpoint("initial banks")?;
        if let Some(refresh) = &plan.refresh {
            state = refresh_banks(profile, key, refresh, &refresh.initial, state)?;
        }
        for (index, (public, private)) in plan.layers.iter().zip(&self.layers).enumerate() {
            state = execute_layer(profile, public, private, state, key)?;
            if let Some(refresh) = &plan.refresh {
                state = refresh_banks(profile, key, refresh, &refresh.layers[index], state)?;
            }
        }
        state = lower_banks(profile, key, state, plan.projection_level)?;
        let inputs: [LocalBootstrapCiphertext<'key>; INPUTS] =
            state.try_into().map_err(|_| BgvError::Kernel)?;
        let (originals, [projected]) = key.route(
            profile,
            &plan.projection,
            &self.projection,
            inputs,
            plan.owned_residue_bytes,
        )?;
        #[cfg(test)]
        tests::observe_route(&plan.projection);
        drop(originals);
        let value = lower(profile, key, projected, plan.mask_level)?;
        let encoding = SlotEncoding::new(profile, plan.fresh.plaintext_precision())?;
        let (original, output) =
            key.mul_plain(profile, value, &encoding.public_prefix_mask(profile, 64)?)?;
        drop(original);
        #[cfg(test)]
        tests::observe_plain_product();
        debug_assert_eq!(output.shape(), plan.output);
        Ok(output)
    }
}

fn initialize_state<'key>(
    profile: &BgvProfile,
    plan: &RamLfeBgvBoundedPlanV1,
    memory: &RamLfeInitialMemoryV1,
    input: LocalBootstrapCiphertext<'key>,
    key: &'key LocalBootstrapKey,
) -> Result<
    (
        LocalBootstrapCiphertext<'key>,
        Vec<LocalBootstrapCiphertext<'key>>,
    ),
    RamLfeBgvBoundedErrorV1,
> {
    let encoding = SlotEncoding::new(profile, plan.fresh.plaintext_precision())?;
    let (input, masked) =
        key.mul_plain(profile, input, &encoding.public_prefix_mask(profile, 64)?)?;
    #[cfg(test)]
    tests::observe_plain_product();
    let mut values = ClearingArray::<u64, SLOTS>::zeroed();
    for (out, &lane) in values[64..96].iter_mut().zip(memory.lanes()) {
        *out = u64::from(lane);
    }
    values[96] = 1;
    let packed = encoding.encode(profile, &values[..])?;
    let base = key.add_plain(profile, masked, &packed)?;
    #[cfg(test)]
    tests::observe_plain_add();
    drop(packed);
    drop(values);
    debug_assert_eq!(base.shape(), plan.augmented);
    let mut banks = Vec::new();
    banks
        .try_reserve_exact(INPUTS)
        .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
    let zero = Plaintext::constant(profile, plan.fresh.plaintext_precision(), 0)?;
    for _ in 0..STATE {
        banks.push(key.trivial(profile, &zero, profile.top_level())?);
    }
    banks.push(base);
    Ok((input, banks))
}

fn lower<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    mut value: LocalBootstrapCiphertext<'key>,
    target: usize,
) -> Result<LocalBootstrapCiphertext<'key>, RamLfeBgvBoundedErrorV1> {
    if target > value.ciphertext().level() {
        return Err(BgvError::LevelMismatch {
            lhs: value.ciphertext().level(),
            rhs: target,
        }
        .into());
    }
    while value.ciphertext().level() > target {
        let next_level = value.ciphertext().level() - 1;
        value = key.switch_to(profile, value, next_level)?;
        #[cfg(test)]
        tests::observe_switch();
    }
    Ok(value)
}

fn lower_banks<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    values: Vec<LocalBootstrapCiphertext<'key>>,
    target: usize,
) -> Result<Vec<LocalBootstrapCiphertext<'key>>, RamLfeBgvBoundedErrorV1> {
    let mut lowered = Vec::new();
    lowered
        .try_reserve_exact(values.len())
        .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
    for value in values {
        lowered.push(lower(profile, key, value, target)?);
    }
    Ok(lowered)
}

fn refresh_banks<'key>(
    profile: &BgvProfile,
    key: &'key LocalBootstrapKey,
    plans: &RefreshPlans,
    indices: &[usize; INPUTS],
    values: Vec<LocalBootstrapCiphertext<'key>>,
) -> Result<Vec<LocalBootstrapCiphertext<'key>>, RamLfeBgvBoundedErrorV1> {
    if values.len() != INPUTS {
        return Err(BgvError::Kernel.into());
    }
    let values = lower_banks(profile, key, values, plans.source_level)?;
    let mut refreshed = Vec::new();
    refreshed
        .try_reserve_exact(INPUTS)
        .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
    for (value, &index) in values.into_iter().zip(indices) {
        let plan = plans.plans.get(index).ok_or(BgvError::Kernel)?;
        refreshed.push(key.refresh(profile, plan, value)?);
        #[cfg(test)]
        {
            tests::observe_refresh();
            tests::checkpoint("refreshed bank")?;
        }
    }
    Ok(refreshed)
}

fn execute_layer<'key>(
    profile: &BgvProfile,
    plan: &LayerPlan,
    map: &SlotLinearMap<21, 60>,
    state: Vec<LocalBootstrapCiphertext<'key>>,
    key: &'key LocalBootstrapKey,
) -> Result<Vec<LocalBootstrapCiphertext<'key>>, RamLfeBgvBoundedErrorV1> {
    let state = lower_banks(profile, key, state, plan.choice.routing_level)?;
    let inputs: [LocalBootstrapCiphertext<'key>; INPUTS] =
        state.try_into().map_err(|_| BgvError::Kernel)?;
    let (originals, operands) = key.route(
        profile,
        &plan.routing,
        map,
        inputs,
        checked_add(
            scaled_residues(profile, plan.choice.routing_level, INPUTS + OPERANDS)?,
            plan.routing.scratch_residue_bytes(),
        )?,
    )?;
    #[cfg(test)]
    tests::observe_route(&plan.routing);
    #[cfg(test)]
    tests::checkpoint("layer routed")?;
    // The final original owner is the base. Iteration moves it and drops all
    // twenty old state owners without another ciphertext/header allocation.
    let base = originals.into_iter().last().ok_or(BgvError::Kernel)?;
    let mut moved_operands = Vec::new();
    moved_operands
        .try_reserve_exact(OPERANDS)
        .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
    moved_operands.extend(operands);
    finish_layer(profile, &plan.products, moved_operands, base, key)
}

fn finish_layer<'key>(
    profile: &BgvProfile,
    plan: &ProductPlan,
    operands: Vec<LocalBootstrapCiphertext<'key>>,
    base: LocalBootstrapCiphertext<'key>,
    key: &'key LocalBootstrapKey,
) -> Result<Vec<LocalBootstrapCiphertext<'key>>, RamLfeBgvBoundedErrorV1> {
    if operands.len() != OPERANDS {
        return Err(BgvError::Kernel.into());
    }
    let mut operands = lower_banks(profile, key, operands, plan.level)?;
    let base = lower(profile, key, base, plan.level)?;
    let c = operands.split_off(2 * STATE);
    let b = operands.split_off(STATE);
    let a = operands;
    let mut next = Vec::new();
    next.try_reserve_exact(INPUTS)
        .map_err(|_| RamLfeBgvBoundedErrorV1::Allocation)?;
    for (bank, ((left, right), addend)) in a.into_iter().zip(b).zip(c).enumerate() {
        let product = key.multiply(profile, left, right)?;
        #[cfg(test)]
        {
            tests::observe_product();
            tests::checkpoint("layer product")?;
        }
        let value = key.add(profile, product, addend)?;
        #[cfg(test)]
        tests::observe_add();
        debug_assert_eq!(value.shape(), plan.state_outputs[bank]);
        next.push(value);
    }
    debug_assert_eq!(base.shape(), plan.base_output);
    next.push(base);
    Ok(next)
}

#[cfg(test)]
#[path = "bgv_bounded_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "bgv_bounded_refresh_tests.rs"]
mod independent_refresh_tests;
