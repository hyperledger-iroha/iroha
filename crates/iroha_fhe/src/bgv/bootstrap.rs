//! Same-master local bootstrap construction from an honestly generated key owner.
//!
//! For master `T=p^E`, source precision `p^r` and `a=p^(E-r)`, compute
//! `w_i=[a*c_i]_Q` by exact centered RNS conversion and encrypt `w0+w1*s`
//! using a genuine encryption of the same secret. Complete coefficient-slot,
//! upper-digit and inverse transforms recover the original full-ring message.
//! The strict public condition `2*a*V+(N+1)*Q<a*Q` uses dense ternary secrets.
//!
//! Closed local sources carry construction provenance only. There is no imported
//! ciphertext/key constructor or external authentication claim. Runtime keys
//! contain no SecretKey: separate custody is returned once during setup.
//! TODO(C.8, R.9, R.11): qualify production parameters, complete application
//! resource ledgers, external key/input proofs and circuit privacy. Small-ring
//! arithmetic does not settle those obligations or activate current candidates.

use core::{fmt, mem::size_of, ptr};
use std::sync::Arc;
use thiserror::Error;
use zeroize::Zeroizing;

#[cfg(test)]
use super::coefficient_slots::CoefficientSlotBank;
use super::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, CoefficientSlotBudget,
    CoefficientSlotDecomposition, CoefficientSlotError, CoefficientSlotRecomposition,
    EncryptedUpperDigitsPlan, EvaluationKeys, KeySet, Plaintext, PublicKey, SecretKey,
    SlotLinearError, SlotLinearMap, SlotLinearPlan, SwitchingKeyKind, UpperDigitsBudget,
    UpperDigitsError, coefficient_slots,
    coins::{EntropySource, OsEntropy, small_residue},
    cost::CostModel,
    relative_trace::BankRetention,
};
use crate::prime_power::UpperDigitsProgram;

/// Scoped setup ceilings, measured as resident element payload rather than frames.
///
/// Key-generation/encryption backend scratch, coin state, public metadata,
/// allocator overhead and the profile remain separate. The secret ceiling is
/// charged although its owner is returned separately from the runtime key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalBootstrapSetupBudget {
    /// Public/evaluation/encrypted-secret resident residue elements.
    pub retained_residue_bytes: usize,
    /// Returned secret coefficients and full-basis evaluation residues.
    pub returned_secret_bytes: usize,
    /// Temporary master-precision secret-plaintext coefficients.
    pub plaintext_bytes: usize,
}

/// Explicit ceilings for preparing and executing one complete local pipeline.
///
/// The two transform and upper-digit owners retain their own distinct budgets.
/// `retained_preparation_bytes` conservatively admits the sum of their supplied
/// coefficient/layout/metadata ceilings before any of them is prepared. A shared
/// source descriptor retains that original core scope; its stage caps must cover
/// the already selected requirements, without rebuilding under different caps. Backend
/// scratch, profile/keys, caller owners, wrapper/trace/routing public plan and
/// shape metadata (including the Galois list), and allocator overhead remain separate;
/// this is not a whole-process RSS or original proof-resource ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapBudget {
    /// Complete input + both transforms + all logical banks of the upper DAG,
    /// in existing canonical arithmetic units. Excludes public preparation,
    /// slot-vector encoding, copies/clears, key generation and proof work.
    pub canonical_arithmetic_units: u64,
    /// Explicit input/plaintext and ciphertext add/sub/negate/divide word work
    /// across the complete pipeline. Internal shared-kernel additions remain in
    /// their arithmetic scope; this is not a CPU instruction or time estimate.
    pub scalar_word_operations: u64,

    /// Combined post-H residues: DFS siblings, active map or upper DAG including
    /// tensor/switch results, and the monomial/output accumulator. Excludes
    /// borrowed keys, plaintexts and separately scoped backend scratch.
    pub stream_owned_residue_bytes: usize,
    /// New bounded traversal/frame/bank/accumulator headers and their requested
    /// Natural payload capacities. Existing prepared plans, routing internals,
    /// upper arena/program metadata and allocator overhead retain separate scopes.
    pub stream_metadata_bytes: usize,

    /// Forward coefficient-to-slot owner.
    pub decomposition: CoefficientSlotBudget,
    /// Sole encrypted upper-digit compiler/evaluator.
    pub upper_digits: UpperDigitsBudget,
    /// Inverse coefficient-slot owner.
    pub recomposition: CoefficientSlotBudget,
    /// Simultaneously retained map/layout and upper-program metadata ceilings.
    pub retained_preparation_bytes: usize,
    /// Consumed source plus streamed plaintext/product/sum owner payload.
    pub input_owned_bytes: usize,
    /// Both source-derived master plaintext coefficient buffers.
    pub input_plaintext_bytes: usize,
    /// Input conversion and plaintext-product shared CostModel units.
    pub input_arithmetic_units: u64,
    /// Separate input plaintext-add coefficient words.
    pub input_addition_words: u64,
}

/// Scoped admitted per-call payload/work and the complete shared preparation ceiling.
///
/// The preparation field is reported in full for each descriptor. Aggregates
/// charge it once only when actual shared-owner identity has been established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapPayload {
    /// Complete input + both transforms + all logical banks of the upper DAG,
    /// in existing canonical arithmetic units. Excludes public preparation,
    /// slot-vector encoding, copies/clears, key generation and proof work.
    pub canonical_arithmetic_units: u64,
    /// Explicit input/plaintext and ciphertext add/sub/negate/divide word work
    /// across the complete pipeline. Internal shared-kernel additions remain in
    /// their arithmetic scope; this is not a CPU instruction or time estimate.
    pub scalar_word_operations: u64,

    /// Combined post-H residues: DFS siblings, active map or upper DAG including
    /// tensor/switch results, and the monomial/output accumulator. Excludes
    /// borrowed keys, plaintexts and separately scoped backend scratch.
    pub stream_owned_residue_bytes: usize,
    /// New bounded traversal/frame/bank/accumulator headers and their requested
    /// Natural payload capacities. Existing prepared plans, routing internals,
    /// upper arena/program metadata and allocator overhead retain separate scopes.
    pub stream_metadata_bytes: usize,

    /// Conservative simultaneous supplied map/layout/program payload ceilings.
    pub retained_preparation_bytes: usize,
    /// Input stage explicit owners, excluding borrowed keys/backend scratch.
    pub input_owned_bytes: usize,
    /// Two master-precision plaintext buffers.
    pub input_plaintext_bytes: usize,
    /// Exact conversion/scalar/product CostModel units, excluding copies/clears.
    pub input_arithmetic_units: u64,
    /// Exact separate plaintext-add coefficient words.
    pub input_addition_words: u64,
}

/// A local preparation, origin or consumed-execution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum BootstrapError {
    /// Public count or work overflow.
    #[error("bootstrap geometry overflow")]
    SizeOverflow,
    /// An explicit payload or work ceiling is too small.
    #[error("bootstrap budget exceeded for {resource}")]
    Budget {
        /// Public resource category.
        resource: &'static str,
    },
    /// The source is not bound to this immutable local key owner.
    #[error("bootstrap local source origin mismatch")]
    Origin,
    /// Source/master/program precision or exact shape mismatch.
    #[error("bootstrap source or program shape mismatch")]
    Shape,
    /// The strict deterministic dense-secret carry condition does not hold.
    #[error("bootstrap source rounding bound is not admitted")]
    RoundingBound,
    /// A public-sized allocation was refused.
    #[error("bootstrap allocation failed")]
    Allocation,
    /// Existing BGV validation or arithmetic refusal.
    #[error(transparent)]
    Bgv(#[from] BgvError),
    /// Existing canonical packed routing refusal.
    #[error(transparent)]
    Routing(#[from] SlotLinearError),
    /// Existing coefficient transform refusal.
    #[error(transparent)]
    Coefficients(#[from] CoefficientSlotError),
    /// Existing sole upper-digit evaluator refusal.
    #[error(transparent)]
    Digits(#[from] UpperDigitsError),
}

fn mul(a: usize, b: usize) -> Result<usize, BootstrapError> {
    a.checked_mul(b).ok_or(BootstrapError::SizeOverflow)
}
fn add(a: usize, b: usize) -> Result<usize, BootstrapError> {
    a.checked_add(b).ok_or(BootstrapError::SizeOverflow)
}
fn word(a: usize) -> Result<u64, BootstrapError> {
    u64::try_from(a).map_err(|_| BootstrapError::SizeOverflow)
}
fn limit(required: usize, ceiling: usize, resource: &'static str) -> Result<(), BootstrapError> {
    if required > ceiling {
        Err(BootstrapError::Budget { resource })
    } else {
        Ok(())
    }
}
fn reserve<T>(count: usize) -> Result<Vec<T>, BootstrapError> {
    #[cfg(test)]
    tests::allocation_checkpoint()?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| BootstrapError::Allocation)?;
    Ok(values)
}
fn zeros(count: usize) -> Result<Zeroizing<Vec<u64>>, BootstrapError> {
    let mut values = reserve(count)?;
    values.resize(count, 0);
    Ok(Zeroizing::new(values))
}

/// Split complementary metadata for one immutable preparation and exact source.
///
/// Charge `shared_preparation` once per actual shared owner and
/// `source_descriptor` for every retained plan. Matrix/layout/upper-DAG payloads
/// keep their separate existing scopes. Arc control words are included; allocator
/// headers, excess allocation, stack frames and RSS are not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapMetadata {
    /// Shared allocation body, Arc control words and complementary dynamic data.
    pub shared_preparation: super::PlanMetadataRequirement,
    /// One plan handle, exact input shape and per-source payload/work record.
    pub source_descriptor: super::PlanMetadataRequirement,
}
impl BootstrapMetadata {
    /// Charge one complete standalone plan, including the shared owner once.
    ///
    /// # Errors
    /// Refuses byte overflow.
    pub fn standalone(self) -> Result<super::PlanMetadataRequirement, super::PlanMetadataError> {
        Ok(super::PlanMetadataRequirement {
            inline_bytes: self.source_descriptor.inline_bytes,
            retained_heap_bytes: super::metadata::add(
                self.source_descriptor.retained_heap_bytes,
                self.shared_preparation.retained_bytes()?,
            )?,
            preparation_scratch_bytes: self
                .shared_preparation
                .preparation_scratch_bytes
                .max(self.source_descriptor.preparation_scratch_bytes),
        })
    }
}

// Only complete canonical preparation constructs this core. Public shapes and
// shared ownership create neither key relation nor ciphertext origin authority.
struct BootstrapPreparation {
    homomorphic: CiphertextShape,
    scale: u64,
    decomposition: CoefficientSlotDecomposition,
    digits: EncryptedUpperDigitsPlan,
    recomposition: CoefficientSlotRecomposition,
    galois: Vec<u32>,
    retained_preparation_bytes: usize,
    stream_owned_residue_bytes: usize,
    stream_metadata_bytes: usize,
}

/// Complete public plan with an exact source and shared immutable preparation.
///
/// There is no Clone or arbitrary core/source constructor. [`Self::admit_source`]
/// repeats source and resource admission without rebuilding or changing stages.
pub struct BootstrapPlan {
    input: CiphertextShape,
    preparation: Arc<BootstrapPreparation>,
    payload: BootstrapPayload,
}
impl fmt::Debug for BootstrapPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BootstrapPlan")
            .field("input", &self.input)
            .field("output", self.output_shape())
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}
fn stream_residue_bytes(
    profile: &BgvProfile,
    input: &CiphertextShape,
    extraction: &CoefficientSlotDecomposition,
    digits: &EncryptedUpperDigitsPlan,
    restoration: &CoefficientSlotRecomposition,
) -> Result<usize, BootstrapError> {
    let cost = CostModel::new(profile);
    let incoming = usize::try_from(cost.ciphertext_residue_bytes(input.level(), 2)?)
        .map_err(|_| BootstrapError::SizeOverflow)?;
    let outgoing =
        usize::try_from(cost.ciphertext_residue_bytes(restoration.output_shape().level(), 2)?)
            .map_err(|_| BootstrapError::SizeOverflow)?;
    let banks = extraction.banks();
    if banks == 0
        || !banks.is_power_of_two()
        || digits.banks() != banks
        || digits.slots() != extraction.slots()
    {
        return Err(BootstrapError::Shape);
    }
    let depth = banks.ilog2() as usize;
    let mut peak = mul(2, incoming)?;
    for done in 0..banks {
        // Binary DFS visits leaf bit_reverse(done,h). The still-pending odd
        // siblings are exactly h-popcount(done); previous output exists iff done>0.
        let pending = depth - done.count_ones() as usize;
        let accumulator = if done == 0 { 0 } else { outgoing };
        let splits = if done == 0 {
            depth
        } else {
            done.trailing_zeros() as usize
        };
        for offset in 0..splits {
            let siblings = pending - splits + offset;
            // current/image/even/odd; parents drop before the smaller odd shift.
            peak = peak.max(add(
                add(mul(siblings, incoming)?, accumulator)?,
                mul(4, incoming)?,
            )?);
        }
        let outside = add(mul(pending, incoming)?, accumulator)?;
        peak = peak.max(add(
            outside,
            add(
                mul(2, incoming)?,
                extraction.payload().routing.scratch_residue_bytes,
            )?,
        )?);
        peak = peak.max(add(outside, digits.payload().single_bank_residue_bytes)?);
        peak = peak.max(add(
            outside,
            add(
                mul(2, outgoing)?,
                restoration.payload().routing.scratch_residue_bytes,
            )?,
        )?);
        peak = peak.max(add(outside, mul(2, outgoing)?)?);
        peak = peak.max(add(
            mul(pending, incoming)?,
            mul(if done == 0 { 1 } else { 3 }, outgoing)?,
        )?);
    }
    Ok(peak)
}

fn aggregate_work(
    profile: &BgvProfile,
    input_arithmetic: u64,
    input_words: u64,
    extraction: &CoefficientSlotDecomposition,
    digits: &EncryptedUpperDigitsPlan,
    restoration: &CoefficientSlotRecomposition,
) -> Result<(u64, u64), BootstrapError> {
    let checked_sum = |parts: [u64; 4]| {
        parts.into_iter().try_fold(0_u64, |total, value| {
            total.checked_add(value).ok_or(BootstrapError::SizeOverflow)
        })
    };
    let transform_words = |level: usize, additions: usize| -> Result<u64, BootstrapError> {
        word(mul(mul(2, profile.degree())?, add(level, 1)?)?)?
            .checked_mul(word(additions)?)
            .ok_or(BootstrapError::SizeOverflow)
    };
    let arithmetic = checked_sum([
        input_arithmetic,
        extraction.operations().arithmetic_units,
        digits.payload().canonical_arithmetic_units,
        restoration.operations().arithmetic_units,
    ])?;
    // Each transform stays at its output level and already counts all d banks.
    // Upper payload likewise already counts every bank: never multiply by d again.
    let words = checked_sum([
        input_words,
        transform_words(
            extraction.output_shape().level(),
            extraction.operations().ciphertext_additions,
        )?,
        digits.payload().scalar_word_operations,
        transform_words(
            restoration.output_shape().level(),
            restoration.operations().ciphertext_additions,
        )?,
    ])?;
    Ok((arithmetic, words))
}

fn retained_preparation_budget(budget: BootstrapBudget) -> Result<usize, BootstrapError> {
    add(
        add(
            budget.decomposition.routing.coefficient_bytes,
            budget.decomposition.routing.layout_metadata_bytes,
        )?,
        add(
            add(
                budget.recomposition.routing.coefficient_bytes,
                budget.recomposition.routing.layout_metadata_bytes,
            )?,
            budget.upper_digits.metadata_bytes,
        )?,
    )
}
fn work_limit(required: u64, ceiling: u64, resource: &'static str) -> Result<(), BootstrapError> {
    if required > ceiling {
        Err(BootstrapError::Budget { resource })
    } else {
        Ok(())
    }
}
fn transform_budget(
    required: super::CoefficientSlotPayload,
    budget: CoefficientSlotBudget,
    resource: &'static str,
) -> Result<(), BootstrapError> {
    for (needed, ceiling) in [
        (
            required.routing.coefficient_bytes,
            budget.routing.coefficient_bytes,
        ),
        (
            required.routing.layout_metadata_bytes,
            budget.routing.layout_metadata_bytes,
        ),
        (
            required.routing.preparation_scratch_bytes,
            budget.routing.preparation_scratch_bytes,
        ),
        (
            required.routing.output_residue_bytes,
            budget.routing.output_residue_bytes,
        ),
        (
            required.routing.scratch_residue_bytes,
            budget.routing.scratch_residue_bytes,
        ),
        (required.owned_residue_bytes, budget.owned_residue_bytes),
        (required.trace_plaintext_bytes, budget.trace_plaintext_bytes),
    ] {
        limit(needed, ceiling, resource)?;
    }
    Ok(())
}
impl BootstrapPreparation {
    fn admit_budget(
        &self,
        profile: &BgvProfile,
        source: SourceAdmission,
        budget: BootstrapBudget,
    ) -> Result<BootstrapPayload, BootstrapError> {
        // Existing stages stay exactly selected. Reuse never reruns a routing
        // search under new ceilings or substitutes a cheaper compiler schedule.
        transform_budget(
            self.decomposition.payload(),
            budget.decomposition,
            "decomposition stage",
        )?;
        transform_budget(
            self.recomposition.payload(),
            budget.recomposition,
            "recomposition stage",
        )?;
        let upper = self.digits.required_budget();
        work_limit(
            upper.compiler_steps,
            budget.upper_digits.compiler_steps,
            "upper compiler",
        )?;
        for (needed, ceiling) in [
            (upper.metadata_bytes, budget.upper_digits.metadata_bytes),
            (
                upper.owned_residue_bytes,
                budget.upper_digits.owned_residue_bytes,
            ),
            (upper.plaintext_bytes, budget.upper_digits.plaintext_bytes),
            (
                upper.backend_scratch_bytes,
                budget.upper_digits.backend_scratch_bytes,
            ),
        ] {
            limit(needed, ceiling, "upper stage")?;
        }
        work_limit(
            upper.canonical_arithmetic_units,
            budget.upper_digits.canonical_arithmetic_units,
            "upper preallocation arithmetic",
        )?;
        work_limit(
            upper.scalar_word_operations,
            budget.upper_digits.scalar_word_operations,
            "upper preallocation scalar words",
        )?;
        // This is the original core's conservative supplied retained scope,
        // not a claim that the newly supplied caps shrink already owned data.
        limit(
            self.retained_preparation_bytes,
            budget.retained_preparation_bytes,
            "retained preparation",
        )?;
        limit(
            self.stream_owned_residue_bytes,
            budget.stream_owned_residue_bytes,
            "stream residues",
        )?;
        limit(
            self.stream_metadata_bytes,
            budget.stream_metadata_bytes,
            "stream metadata",
        )?;
        let (arithmetic, words) = aggregate_work(
            profile,
            source.arithmetic,
            source.addition,
            &self.decomposition,
            &self.digits,
            &self.recomposition,
        )?;
        work_limit(
            arithmetic,
            budget.canonical_arithmetic_units,
            "complete arithmetic",
        )?;
        work_limit(
            words,
            budget.scalar_word_operations,
            "complete scalar words",
        )?;
        Ok(BootstrapPayload {
            canonical_arithmetic_units: arithmetic,
            scalar_word_operations: words,
            stream_owned_residue_bytes: self.stream_owned_residue_bytes,
            stream_metadata_bytes: self.stream_metadata_bytes,
            retained_preparation_bytes: self.retained_preparation_bytes,
            input_owned_bytes: source.owned,
            input_plaintext_bytes: source.plaintext,
            input_arithmetic_units: source.arithmetic,
            input_addition_words: source.addition,
        })
    }
}

// Source-specific checks happen before any shared-core clone or heavy stage
// preparation. An exact public shape is planning data, never source authority.
struct SourceAdmission {
    owned: usize,
    plaintext: usize,
    arithmetic: u64,
    addition: u64,
}
impl SourceAdmission {
    fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        effective: super::PlaintextPrecision,
        budget: BootstrapBudget,
    ) -> Result<Self, BootstrapError> {
        input.check_profile(profile)?;
        effective.check_profile(profile)?;
        let master = profile.master_plaintext();
        if input.components() != 2
            || input.plaintext_precision() != effective
            || effective.exponent() >= master.exponent()
        {
            return Err(BootstrapError::Shape);
        }
        let scale = master.modulus() / effective.modulus();
        let q = profile.level_modulus(input.level())?;
        let twice_scale = scale.checked_mul(2).ok_or(BootstrapError::SizeOverflow)?;
        let degree_plus_one = word(profile.degree())?
            .checked_add(1)
            .ok_or(BootstrapError::SizeOverflow)?;
        if input
            .bound()
            .mul_u64(twice_scale)
            .add(&q.mul_u64(degree_plus_one))
            >= q.mul_u64(scale)
        {
            return Err(BootstrapError::RoundingBound);
        }
        let model = CostModel::new(profile);
        let nbytes = mul(profile.degree(), size_of::<u64>())?;
        let source_poly = mul(nbytes, input.level() + 1)?;
        let top_poly = mul(nbytes, profile.top_level() + 1)?;
        let conversion = add(mul(2, source_poly)?, nbytes)?.max(add(source_poly, mul(2, nbytes)?)?);
        let evaluation =
            add(mul(2, top_poly)?, mul(2, nbytes)?)?.max(add(mul(4, top_poly)?, nbytes)?);
        let owned = conversion.max(evaluation);
        let plaintext = mul(2, nbytes)?;
        limit(owned, budget.input_owned_bytes, "input owners")?;
        limit(plaintext, budget.input_plaintext_bytes, "input plaintext")?;
        let scalar_words = word(mul(mul(2, profile.degree())?, input.level() + 1)?)?;
        let arithmetic = model
            .conversion_units(word(input.level() + 1)?, 1)
            .checked_mul(2)
            .and_then(|x| x.checked_add(scalar_words))
            .and_then(|x| {
                x.checked_add(
                    model
                        .plaintext_multiplication_units(profile.top_level())
                        .ok()?,
                )
            })
            .ok_or(BootstrapError::SizeOverflow)?;
        let addition = word(mul(profile.degree(), profile.top_level() + 1)?)?;
        if arithmetic > budget.input_arithmetic_units {
            return Err(BootstrapError::Budget {
                resource: "input arithmetic",
            });
        }
        if addition > budget.input_addition_words {
            return Err(BootstrapError::Budget {
                resource: "input addition",
            });
        }
        Ok(Self {
            owned,
            plaintext,
            arithmetic,
            addition,
        })
    }
}

impl BootstrapPlan {
    /// Allocation-free metadata prebound for one complete prepared bootstrap.
    ///
    /// Counts the complete enclosing struct, all otherwise uncharged shape and
    /// vector allocations, and maximum additional sequential preparation scratch.
    /// Existing matrix/layout and upper-DAG metadata ceilings remain mandatory
    /// and separate. Their embedded handles are included here, their allocation
    /// graphs are not duplicated. Public polynomial/encoding scratch is included.
    /// Requirements are architecture-local requested payloads, not RSS or policy.
    ///
    /// # Errors
    /// Refuses foreign/incompatible contexts or checked count/byte overflow.
    pub fn metadata_requirement(
        profile: &BgvProfile,
        precision: super::PlaintextPrecision,
        program: &crate::prime_power::UpperDigitsPlan,
    ) -> Result<super::PlanMetadataRequirement, super::PlanMetadataError> {
        Self::split_metadata_requirement(profile, precision, program)?.standalone()
    }

    /// Allocation-free shared-core and per-source complementary metadata bounds.
    ///
    /// Count the shared allocation once by actual owner identity, then every
    /// descriptor. This never assumes equal shapes imply shared storage.
    ///
    /// # Errors
    /// Refuses foreign/incompatible contexts or checked byte overflow.
    pub fn split_metadata_requirement(
        profile: &BgvProfile,
        precision: super::PlaintextPrecision,
        program: &crate::prime_power::UpperDigitsPlan,
    ) -> Result<BootstrapMetadata, super::PlanMetadataError> {
        use super::metadata::{add, mul, shape_heap_bound, shape_value_bound, sum};
        precision.check_profile(profile)?;
        let master = profile.master_plaintext();
        if precision.exponent() >= master.exponent()
            || program.context().prime() != master.prime()
            || program.context().precision() != master.exponent()
            || program.retained_digits() != precision.exponent()
        {
            return Err(super::PlanMetadataError::Context);
        }
        let (slots, banks, bits, depth) = super::metadata::geometry(profile, master)?;
        // Shared H:1; decomposition input/trace pair/route pair:5;
        // restoration input/route pair/trace output:4 plus d trace inputs;
        // upper outer input/output:2. All stored bounds are admitted.
        let heap = sum(&[
            mul(add(banks, 12)?, shape_heap_bound(profile)?)?,
            mul(banks, size_of::<CiphertextShape>())?,
            mul(
                sum(&[mul(2, slots)?, mul(5, bits)?, mul(4, depth)?])?,
                size_of::<u32>(),
            )?,
        ])?;
        let route = SlotLinearPlan::<1, 1>::metadata_requirement(profile, master)?;
        // Recomposition keeps its temporary d-shape input vector while the
        // trace owns its cloned vector and the matrix is prepared. The retained
        // vector is in `heap`; this charges only the second staging vector.
        let staging = add(
            size_of::<Vec<CiphertextShape>>(),
            mul(banks, shape_value_bound(profile)?)?,
        )?;
        // Trace construction retains output, image, sum and monomial result;
        // the outer constructor also owns fresh/mul-plain/H transfer temporaries.
        let outer_shapes = mul(7, shape_value_bound(profile)?)?;
        let trace = add(
            super::metadata::transfer_scratch(profile)?,
            super::metadata::encoding_scratch(slots)?,
        )?;
        let program_scratch = add(
            program.geometry().preparation_scratch_bytes,
            program.preparation_owner_bytes(),
        )?;
        let scratch = sum(&[
            staging,
            outer_shapes,
            route
                .preparation_scratch_bytes
                .max(trace)
                .max(program_scratch),
        ])?;
        Ok(BootstrapMetadata {
            shared_preparation: super::PlanMetadataRequirement {
                inline_bytes: add(
                    size_of::<BootstrapPreparation>(),
                    mul(2, size_of::<usize>())?,
                )?,
                retained_heap_bytes: heap,
                preparation_scratch_bytes: scratch,
            },
            source_descriptor: super::PlanMetadataRequirement {
                inline_bytes: size_of::<Self>(),
                retained_heap_bytes: shape_heap_bound(profile)?,
                preparation_scratch_bytes: super::metadata::transfer_scratch(profile)?,
            },
        })
    }

    /// Inspect retained capacities in the same complementary metadata scope.
    ///
    /// An aggregate caller admits the static requirement before preparation,
    /// then compares this observation before private work. Any allocation excess
    /// is a local resource refusal, never a bound widening or semantic change.
    ///
    /// # Errors
    /// Refuses byte overflow; no source or secret is inspected.
    pub fn retained_metadata(
        &self,
    ) -> Result<super::PlanMetadataRequirement, super::PlanMetadataError> {
        self.retained_metadata_split()?.standalone()
    }

    /// Observe the same split using actual visible retained capacities.
    ///
    /// The shared allocation is reported in full even when other descriptors
    /// reference it. A caller deduplicates only via [`Self::shares_preparation`].
    ///
    /// # Errors
    /// Refuses byte overflow; no private source is inspected.
    pub fn retained_metadata_split(&self) -> Result<BootstrapMetadata, super::PlanMetadataError> {
        let core = &self.preparation;
        let heap = super::metadata::sum(&[
            super::metadata::shape_heap(&core.homomorphic)?,
            core.decomposition.metadata_heap()?,
            core.recomposition.metadata_heap()?,
            core.digits.outer_metadata_heap()?,
            super::metadata::mul(core.galois.capacity(), size_of::<u32>())?,
        ])?;
        Ok(BootstrapMetadata {
            shared_preparation: super::PlanMetadataRequirement {
                inline_bytes: super::metadata::add(
                    size_of::<BootstrapPreparation>(),
                    super::metadata::mul(2, size_of::<usize>())?,
                )?,
                retained_heap_bytes: heap,
                preparation_scratch_bytes: 0,
            },
            source_descriptor: super::PlanMetadataRequirement {
                inline_bytes: size_of::<Self>(),
                retained_heap_bytes: super::metadata::shape_heap(&self.input)?,
                preparation_scratch_bytes: 0,
            },
        })
    }

    /// Admit the entire constructor/transform/digit/inverse schedule before input access.
    ///
    /// `levels` is the sole upper-digit evaluator's public depth schedule. A plan
    /// may preserve the message without reducing a fresh input's bound; callers
    /// can inspect [`Self::reduces_phase_bound`] rather than assume a gain.
    ///
    /// # Errors
    /// Context/shape/strict carry refusal, budget/allocation failure, or an existing
    /// canonical stage's precision/noise/resource refusal.
    pub fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        program: UpperDigitsProgram,
        levels: &[usize],
        budget: BootstrapBudget,
    ) -> Result<Self, BootstrapError> {
        input.check_profile(profile)?;
        let master = profile.master_plaintext();
        let effective = input.plaintext_precision();
        if program.plan().context().prime() != master.prime()
            || program.plan().context().precision() != master.exponent()
            || program.plan().retained_digits() != effective.exponent()
        {
            return Err(BootstrapError::Shape);
        }
        let source = SourceAdmission::new(profile, input, effective, budget)?;
        let scale = master.modulus() / effective.modulus();
        let slots = super::SlotEncoding::slot_count(profile, master)?;
        let banks = profile.degree() / slots;
        let stream_metadata = coefficient_slots::stream_metadata_bytes(profile, banks)?;
        limit(
            stream_metadata,
            budget.stream_metadata_bytes,
            "stream metadata",
        )?;
        let retained = retained_preparation_budget(budget)?;
        limit(
            retained,
            budget.retained_preparation_bytes,
            "retained preparation",
        )?;
        let fresh = profile.plan_fresh(master)?;
        let plain = Plaintext::worst_case_bound(profile, master);
        let homomorphic = profile.plan_add_plain(&profile.plan_mul_plain(&fresh, plain)?, plain)?;
        let decomposition = CoefficientSlotDecomposition::prepare(
            profile,
            &homomorphic,
            budget.decomposition,
            BankRetention::Streaming,
        )?;
        let digits = EncryptedUpperDigitsPlan::from_shape(
            profile,
            program,
            decomposition.output_shape(),
            decomposition.slots(),
            decomposition.banks(),
            levels,
            budget.upper_digits,
            BankRetention::Streaming,
        )?;
        let recomposition = CoefficientSlotRecomposition::prepare(
            profile,
            digits.output_shape(),
            budget.recomposition,
            BankRetention::Streaming,
        )?;
        if recomposition.output_shape().plaintext_precision() != effective {
            return Err(BootstrapError::Shape);
        }
        let count = add(
            decomposition.galois_elements().len(),
            recomposition.galois_elements().len(),
        )?;
        let stream_owned = stream_residue_bytes(
            profile,
            &homomorphic,
            &decomposition,
            &digits,
            &recomposition,
        )?;
        limit(
            stream_owned,
            budget.stream_owned_residue_bytes,
            "stream residues",
        )?;
        let mut galois = reserve(count)?;
        galois.extend_from_slice(decomposition.galois_elements());
        galois.extend_from_slice(recomposition.galois_elements());
        galois.sort_unstable();
        galois.dedup();
        let preparation = BootstrapPreparation {
            homomorphic,
            scale,
            decomposition,
            digits,
            recomposition,
            galois,
            retained_preparation_bytes: retained,
            stream_owned_residue_bytes: stream_owned,
            stream_metadata_bytes: stream_metadata,
        };
        let payload = preparation.admit_budget(profile, source, budget)?;
        // Arc::new follows the standard allocator's OOM policy. This API does
        // not claim recoverable allocation failure for every owner allocation.
        Ok(Self {
            input: input.clone(),
            preparation: Arc::new(preparation),
            payload,
        })
    }

    /// Admit an exact source against this same immutable stage realization.
    ///
    /// Rechecks profile, precision, components, strict carry, selected stage
    /// requirements and every source/stream/complete-work ceiling. No supplied
    /// program or level schedule can replace the core, and no source origin is
    /// created. A smaller routing budget cannot silently choose another split.
    ///
    /// # Errors
    /// Source/carry mismatch or any insufficient public resource ceiling.
    pub fn admit_source(
        &self,
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: BootstrapBudget,
    ) -> Result<Self, BootstrapError> {
        self.input.check_profile(profile)?;
        let source =
            SourceAdmission::new(profile, input, self.input.plaintext_precision(), budget)?;
        let payload = self.preparation.admit_budget(profile, source, budget)?;
        Ok(Self {
            input: input.clone(),
            preparation: Arc::clone(&self.preparation),
            payload,
        })
    }

    /// Whether two descriptors retain the actual same immutable preparation.
    ///
    /// This public allocation identity supports resource accounting only; it is
    /// neither key identity nor authorization for an arbitrary ciphertext.
    #[must_use]
    pub fn shares_preparation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.preparation, &other.preparation)
    }

    /// Supplied retained matrix/layout/upper scope of the shared core, counted once.
    ///
    /// Complementary typed metadata is reported separately. This conservative
    /// scope is unchanged by reference count or smaller later source ceilings.
    #[must_use]
    pub fn shared_preparation_bytes(&self) -> usize {
        self.preparation.retained_preparation_bytes
    }
    /// Exact accepted source shape.
    #[must_use]
    pub const fn input_shape(&self) -> &CiphertextShape {
        &self.input
    }
    /// Exact actual master-precision homomorphic-decryption output shape.
    #[must_use]
    pub fn homomorphic_input_shape(&self) -> &CiphertextShape {
        &self.preparation.homomorphic
    }
    /// Exact complete whole-ring output shape.
    #[must_use]
    pub fn output_shape(&self) -> &CiphertextShape {
        self.preparation.recomposition.output_shape()
    }
    /// Whether this public source/output pair strictly reduces the phase bound.
    #[must_use]
    pub fn reduces_phase_bound(&self) -> bool {
        self.output_shape().bound() < self.input.bound()
    }
    /// Complete required Galois key set, in ascending order.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        &self.preparation.galois
    }
    /// Scoped admitted local payload and input work.
    #[must_use]
    pub const fn payload(&self) -> BootstrapPayload {
        self.payload
    }
}

/// A generated matching public/evaluation/encrypted-secret owner, with no runtime secret.
///
/// There is no clone, frame import, replacement key or from-parts constructor.
/// Returning secret custody separately does not let a caller alter this relation.
pub struct LocalBootstrapKey {
    public: PublicKey,
    evaluation: EvaluationKeys,
    encrypted_secret: Ciphertext,
}
impl fmt::Debug for LocalBootstrapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED local bootstrap key]")
    }
}

/// One canonical ciphertext with closed local construction provenance.
///
/// The immutable borrowed key cannot move or be replaced during this lifetime.
/// Taking an ordinary ciphertext out of this owner loses this local provenance;
/// no raw-ciphertext or frame constructor exists to recover it.
pub struct LocalBootstrapCiphertext<'key> {
    key: &'key LocalBootstrapKey,
    value: Ciphertext,
}
impl fmt::Debug for LocalBootstrapCiphertext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED local bootstrap source]")
    }
}
impl LocalBootstrapCiphertext<'_> {
    /// Borrow the ordinary ciphertext without manufacturing another local source.
    #[must_use]
    pub const fn ciphertext(&self) -> &Ciphertext {
        &self.value
    }
    /// Current canonical public shape.
    #[must_use]
    pub fn shape(&self) -> CiphertextShape {
        self.value.shape()
    }
    /// Consume this local source and relinquish its closed provenance.
    #[must_use]
    pub fn into_ciphertext(self) -> Ciphertext {
        self.value
    }
}

impl LocalBootstrapKey {
    /// Master parameter identity of this immutable locally generated key owner.
    ///
    /// This is public compatibility metadata, not a key fingerprint, imported
    /// key authentication or proof that an arbitrary ciphertext has this origin.
    #[must_use]
    pub const fn profile_identity(&self) -> &[u8; 32] {
        self.public.profile_identity()
    }

    /// Generate all matching material and return separate secret custody.
    ///
    /// Only this setup path reads the secret polynomial. Runtime bootstrap and
    /// local arithmetic use the generated public/evaluation/encrypted-secret owners.
    ///
    /// # Errors
    /// Geometry/budget/allocation, invalid Galois elements, entropy or canonical
    /// encryption failure. All partial secret owners clear on error/unwind.
    pub fn generate(
        profile: &BgvProfile,
        galois: &[u32],
        budget: LocalBootstrapSetupBudget,
    ) -> Result<(SecretKey, Self), BootstrapError> {
        Self::generate_with(profile, galois, budget, &mut OsEntropy)
    }
    fn generate_with(
        profile: &BgvProfile,
        galois: &[u32],
        budget: LocalBootstrapSetupBudget,
        entropy: &mut dyn EntropySource,
    ) -> Result<(SecretKey, Self), BootstrapError> {
        let nbytes = mul(profile.degree(), size_of::<u64>())?;
        let top = profile.top_level() + 1;
        let full = add(top, profile.special_count())?;
        // Repeated public elements deliberately overcharge setup; no secret-sized allocations.
        let switches = mul(
            mul(mul(add(galois.len(), 1)?, 2)?, profile.digit_count())?,
            full,
        )?;
        let retained = mul(nbytes, add(mul(4, top)?, switches)?)?;
        let secret = mul(nbytes, add(full, 1)?)?;
        limit(
            retained,
            budget.retained_residue_bytes,
            "setup retained residues",
        )?;
        limit(
            secret,
            budget.returned_secret_bytes,
            "setup returned secret",
        )?;
        limit(nbytes, budget.plaintext_bytes, "setup plaintext")?;
        profile.plan_fresh(profile.master_plaintext())?;
        let KeySet {
            secret,
            public,
            evaluation,
        } = KeySet::generate_with(profile, galois, entropy)?;
        let mut coefficients = zeros(profile.degree())?;
        for (out, &value) in coefficients.iter_mut().zip(secret.coefficients()) {
            *out = small_residue(value, profile.master_plaintext_modulus());
        }
        let plaintext = Plaintext::new(
            profile,
            profile.master_plaintext(),
            core::mem::take(&mut *coefficients),
        )?;
        let encrypted_secret = public.encrypt_with(profile, &plaintext, entropy)?;
        drop(plaintext);
        Ok((
            secret,
            Self {
                public,
                evaluation,
                encrypted_secret,
            },
        ))
    }
    fn check_source(
        &self,
        profile: &BgvProfile,
        input: &LocalBootstrapCiphertext<'_>,
    ) -> Result<(), BootstrapError> {
        if !ptr::eq(self, input.key) {
            return Err(BootstrapError::Origin);
        }
        input.value.check_profile(profile)?;
        self.public.check_profile(profile)?;
        Ok(())
    }
    /// Validate a complete public evaluation entry before private work.
    ///
    /// Checks the exact local owner, master/effective context and expected shape,
    /// plus relinearization and every requested Galois key. This returns no new
    /// source, scalar-membership capability, authentication or privacy claim.
    ///
    /// # Errors
    /// Foreign origin/profile, mismatched shape or missing/wrong switching key.
    pub fn preflight_evaluation(
        &self,
        profile: &BgvProfile,
        input: &LocalBootstrapCiphertext<'_>,
        expected: &CiphertextShape,
        galois: &[u32],
    ) -> Result<(), BootstrapError> {
        self.check_source(profile, input)?;
        expected.check_profile(profile)?;
        if input.value.shape() != *expected {
            return Err(BootstrapError::Shape);
        }
        let relin = self.evaluation.relinearization();
        relin.check_profile(profile)?;
        if relin.kind() != SwitchingKeyKind::Relinearization {
            return Err(BgvError::WrongSwitchingKey.into());
        }
        for &element in galois {
            let key = self
                .evaluation
                .galois(element)
                .ok_or(BgvError::WrongSwitchingKey)?;
            key.check_profile(profile)?;
            if key.kind() != SwitchingKeyKind::Galois(element) {
                return Err(BgvError::WrongSwitchingKey.into());
            }
        }
        Ok(())
    }

    /// Construct the canonical noiseless `(plaintext,0)` under this local owner.
    ///
    /// This proves only local constructibility and bounded decryption semantics.
    /// It hides no plaintext and establishes neither scalar-subalgebra membership
    /// nor external key/input authentication. No raw ciphertext can be imported.
    ///
    /// # Errors
    /// Canonical master/precision/level/noise refusal before coefficient access.
    pub fn trivial<'key>(
        &'key self,
        profile: &BgvProfile,
        plaintext: &Plaintext,
        level: usize,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.public.check_profile(profile)?;
        let value = Ciphertext::trivial(profile, plaintext, level)?;
        #[cfg(test)]
        tests::checkpoint("trivial output")?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }

    /// Consume two same-owner sources through the canonical ciphertext sum.
    ///
    /// # Errors
    /// Foreign origin or canonical master/precision/level/component/noise refusal.
    /// Both consumed sources and partial outputs clear on failure or unwind.
    pub fn add<'key>(
        &'key self,
        profile: &BgvProfile,
        lhs: LocalBootstrapCiphertext<'key>,
        rhs: LocalBootstrapCiphertext<'key>,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &lhs)?;
        self.check_source(profile, &rhs)?;
        let value = profile.add(&lhs.value, &rhs.value)?;
        #[cfg(test)]
        tests::checkpoint("add output")?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }

    /// Consume a local source through the canonical plaintext sum.
    ///
    /// # Errors
    /// Foreign origin or canonical master/precision/noise refusal before private
    /// coefficients are read. Consumed and partial ciphertexts clear on failure.
    pub fn add_plain<'key>(
        &'key self,
        profile: &BgvProfile,
        input: LocalBootstrapCiphertext<'key>,
        plaintext: &Plaintext,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &input)?;
        let value = profile.add_plain(&input.value, plaintext)?;
        #[cfg(test)]
        tests::checkpoint("plaintext sum output")?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }

    /// Apply the canonical plaintext product and return both checked owners.
    ///
    /// The first result is the unchanged input, moved with the same shape and
    /// residue allocations; the second is the actual product. Callers may
    /// retain the input for another transformation or drop it immediately.
    /// Both ciphertexts coexist with the canonical product's existing scratch;
    /// neither is cloned or reconstructed from raw residues.
    ///
    /// # Errors
    /// Foreign origin or canonical master/precision/noise refusal before private
    /// coefficients are read. Consumed and partial ciphertexts clear on failure.
    pub fn mul_plain<'key>(
        &'key self,
        profile: &BgvProfile,
        input: LocalBootstrapCiphertext<'key>,
        plaintext: &Plaintext,
    ) -> Result<
        (
            LocalBootstrapCiphertext<'key>,
            LocalBootstrapCiphertext<'key>,
        ),
        BootstrapError,
    > {
        self.check_source(profile, &input)?;
        let value = profile.mul_plain(&input.value, plaintext)?;
        #[cfg(test)]
        tests::checkpoint("plaintext product output")?;
        Ok((input, LocalBootstrapCiphertext { key: self, value }))
    }

    /// Encrypt an ordinary plaintext into this closed local source domain.
    ///
    /// # Errors
    /// Existing context/noise/entropy refusal; no local source is returned on failure.
    pub fn encrypt<'key>(
        &'key self,
        profile: &BgvProfile,
        plaintext: &Plaintext,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.encrypt_with(profile, plaintext, &mut OsEntropy)
    }
    fn encrypt_with<'key>(
        &'key self,
        profile: &BgvProfile,
        plaintext: &Plaintext,
        entropy: &mut dyn EntropySource,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        let value = self.public.encrypt_with(profile, plaintext, entropy)?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }
    /// Consume a local source and lower it using the canonical modulus-switch path.
    ///
    /// # Errors
    /// Foreign local origin or canonical profile/level/noise refusal.
    pub fn switch_to<'key>(
        &'key self,
        profile: &BgvProfile,
        input: LocalBootstrapCiphertext<'key>,
        level: usize,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &input)?;
        if level > input.value.level() {
            return Err(BgvError::LevelMismatch {
                lhs: input.value.level(),
                rhs: level,
            }
            .into());
        }
        let mut value = input.value;
        while value.level() > level {
            value = profile.switch_down(&value)?;
        }
        Ok(LocalBootstrapCiphertext { key: self, value })
    }
    /// Consume two same-owner inputs and use the sole canonical multiplication.
    ///
    /// # Errors
    /// Foreign local origin or canonical context/component/noise refusal.
    pub fn multiply<'key>(
        &'key self,
        profile: &BgvProfile,
        lhs: LocalBootstrapCiphertext<'key>,
        rhs: LocalBootstrapCiphertext<'key>,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &lhs)?;
        self.check_source(profile, &rhs)?;
        let value = profile.multiply(&lhs.value, &rhs.value, self.evaluation.relinearization())?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }
    /// Consume one source and square it without cloning its ciphertext owner.
    ///
    /// # Errors
    /// Foreign local origin or canonical component/noise refusal.
    pub fn square<'key>(
        &'key self,
        profile: &BgvProfile,
        input: LocalBootstrapCiphertext<'key>,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &input)?;
        let value = profile.multiply(
            &input.value,
            &input.value,
            self.evaluation.relinearization(),
        )?;
        Ok(LocalBootstrapCiphertext { key: self, value })
    }
    /// Consume same-owner inputs through the sole canonical packed linear map.
    ///
    /// Return the original checked inputs and computed outputs by moving their
    /// owners. A caller may retain a carried input without cloning ciphertexts
    /// or performing another arithmetic output; drop unneeded inputs explicitly.
    ///
    /// The explicit ceiling admits all consumed input residues, planned outputs
    /// and the existing routing ciphertext scratch simultaneously. Backend
    /// RNS/NTT/key-switch scratch, plaintext buffers, keys/profile, public plan
    /// metadata and allocator overhead remain separate; this is not an RSS or
    /// whole-proof ledger ceiling. No raw ciphertext can enter this origin domain.
    ///
    /// # Errors
    /// Foreign local origin or checked payload overflow/ceiling refusal precedes
    /// private map access. The shared evaluator then checks exact profile,
    /// precision, layout, input shapes and its complete Galois key union. Every
    /// consumed input and partial output clears on refusal or unwind.
    pub fn route<'key, const I: usize, const O: usize>(
        &'key self,
        profile: &BgvProfile,
        plan: &SlotLinearPlan<I, O>,
        map: &SlotLinearMap<I, O>,
        inputs: [LocalBootstrapCiphertext<'key>; I],
        owned_residue_bytes: usize,
    ) -> Result<
        (
            [LocalBootstrapCiphertext<'key>; I],
            [LocalBootstrapCiphertext<'key>; O],
        ),
        BootstrapError,
    > {
        for input in &inputs {
            self.check_source(profile, input)?;
        }
        let mut input_bytes = 0;
        for input in &inputs {
            let words = mul(
                input.value.component_count(),
                mul(profile.degree(), add(input.value.level(), 1)?)?,
            )?;
            input_bytes = add(input_bytes, mul(words, size_of::<u64>())?)?;
        }
        let required = add(
            input_bytes,
            add(plan.output_residue_bytes(), plan.scratch_residue_bytes())?,
        )?;
        limit(required, owned_residue_bytes, "route owners")?;
        let inputs = inputs.map(|input| input.value);
        let outputs = map.evaluate(profile, plan, &inputs, &self.evaluation)?;
        #[cfg(test)]
        tests::checkpoint("route outputs")?;
        let outputs: [Ciphertext; O] = outputs.try_into().map_err(|_| BootstrapError::Shape)?;
        Ok((
            inputs.map(|value| LocalBootstrapCiphertext { key: self, value }),
            outputs.map(|value| LocalBootstrapCiphertext { key: self, value }),
        ))
    }
    /// Consume a local source and execute the complete admitted bootstrap pipeline.
    ///
    /// This reads no SecretKey and performs no runtime decryption/re-encryption.
    /// Every stage retains its canonical exact shape and genuine bank origin.
    ///
    /// # Errors
    /// Foreign origin, exact source/key mismatch or canonical allocation/arithmetic
    /// refusal. All consumed and partial owners clear on failure or unwind.
    pub fn refresh<'key>(
        &'key self,
        profile: &BgvProfile,
        plan: &BootstrapPlan,
        input: LocalBootstrapCiphertext<'key>,
    ) -> Result<LocalBootstrapCiphertext<'key>, BootstrapError> {
        self.check_source(profile, &input)?;
        if input.value.shape() != plan.input {
            return Err(BootstrapError::Shape);
        }
        plan.input.check_profile(profile)?;
        if self.encrypted_secret.shape() != profile.plan_fresh(profile.master_plaintext())? {
            return Err(BootstrapError::Shape);
        }
        self.evaluation.relinearization().check_profile(profile)?;
        for &element in &plan.preparation.galois {
            let key = self
                .evaluation
                .galois(element)
                .ok_or(BgvError::WrongSwitchingKey)?;
            key.check_profile(profile)?;
            if key.kind() != SwitchingKeyKind::Galois(element) {
                return Err(BgvError::WrongSwitchingKey.into());
            }
        }
        plan.preparation.digits.preflight_bank(
            profile,
            plan.preparation.decomposition.output_shape(),
            plan.preparation.decomposition.slots(),
            plan.preparation.decomposition.banks(),
            self.evaluation.relinearization(),
        )?;
        #[cfg(test)]
        tests::observe_source(profile, plan, &input.value)?;
        let master = profile.master_plaintext();
        let context = profile.level(input.value.level())?;
        let mut parts = input.value.into_components().into_iter();
        let mut convert = || -> Result<Plaintext, BootstrapError> {
            let mut component = parts.next().ok_or(BootstrapError::Shape)?;
            for limb in 0..component.limbs() {
                let fixed = profile.main_plan(limb).fixed();
                for value in component.limb_mut(limb) {
                    *value = fixed.multiply(*value, plan.preparation.scale);
                }
            }
            let mut coefficients = zeros(profile.degree())?;
            context
                .basis
                .convert_centered(
                    component.words(),
                    profile.degree(),
                    core::slice::from_ref(&context.plaintext_target),
                    &mut coefficients,
                )
                .map_err(|_| BgvError::Kernel)?;
            #[cfg(test)]
            tests::checkpoint("converted")?;
            drop(component);
            Plaintext::new(profile, master, core::mem::take(&mut *coefficients)).map_err(Into::into)
        };
        let w0 = convert()?;
        let w1 = convert()?;
        drop(convert);
        drop(parts);
        let product = profile.mul_plain(&self.encrypted_secret, &w1)?;
        drop(w1);
        let homomorphic = profile.add_plain(&product, &w0)?;
        drop(product);
        drop(w0);
        if homomorphic.shape() != plan.preparation.homomorphic {
            return Err(BootstrapError::Shape);
        }
        #[cfg(test)]
        tests::observe_homomorphic(profile, &homomorphic)?;
        let mut cursor =
            plan.preparation
                .decomposition
                .start(profile, homomorphic, &self.evaluation)?;
        let mut accumulator = plan
            .preparation
            .recomposition
            .start_accumulator(profile, &self.evaluation)?;
        while let Some((bank, rest)) = cursor.advance(profile, &self.evaluation)? {
            #[cfg(test)]
            tests::observe_bank(profile, &bank, false)?;
            let bank = bank.upper(
                profile,
                &plan.preparation.digits,
                self.evaluation.relinearization(),
            )?;
            #[cfg(test)]
            tests::observe_bank(profile, &bank, true)?;
            accumulator = accumulator.push(profile, bank, &self.evaluation)?;
            cursor = rest;
        }
        let value = accumulator.finish()?;
        #[cfg(test)]
        tests::observe_result(profile, &value)?;
        if value.shape() != *plan.output_shape() {
            return Err(BootstrapError::Shape);
        }
        Ok(LocalBootstrapCiphertext { key: self, value })
    }
}

#[cfg(test)]
mod tests {
    //! Actual local constructor chain, independent integer CRT and complete repeated refresh.
    use super::*;
    use crate::{
        bgv::{
            BgvParameters, SlotEncoding, SlotLinearBudget, SlotLinearLayout, SlotLinearSegment,
            fixtures::FixtureEntropy, poly,
        },
        modular,
        natural::Natural,
        prime_power::{PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan},
    };
    use num_bigint::{BigInt, Sign};
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
    };

    thread_local! {
        static OBSERVER: RefCell<Option<Oracle>> = const {RefCell::new(None)};
        static ACCESSES: Cell<usize> = const {Cell::new(0)};
        static ALLOCS: Cell<usize> = const {Cell::new(0)};
        static FAIL_ALLOC: Cell<usize> = const {Cell::new(usize::MAX)};
        static FAULT: Cell<(usize,u8)> = const {Cell::new((usize::MAX,0))};
        static AT_FAULT: Cell<usize> = const {Cell::new(0)};
    }
    pub(super) fn allocation_checkpoint() -> Result<(), BootstrapError> {
        let n = ALLOCS.with(|v| {
            let n = v.get() + 1;
            v.set(n);
            n
        });
        if FAIL_ALLOC.with(Cell::get) == n {
            return Err(BootstrapError::Allocation);
        }
        Ok(())
    }
    pub(super) fn checkpoint(_stage: &str) -> Result<(), BootstrapError> {
        let call = ACCESSES.with(|v| {
            let n = v.get() + 1;
            v.set(n);
            n
        });
        let (at, mode) = FAULT.with(Cell::get);
        if call == at && mode != 0 {
            FAULT.with(|v| v.set((usize::MAX, 0)));
            AT_FAULT.with(|v| v.set(poly::tests::cleared_words()));
            if mode == 2 {
                panic!("local bootstrap clearing unwind control");
            }
            return Err(BootstrapError::Shape);
        }
        Ok(())
    }
    fn reset() {
        ACCESSES.with(|v| v.set(0));
        ALLOCS.with(|v| v.set(0));
        FAIL_ALLOC.with(|v| v.set(usize::MAX));
        FAULT.with(|v| v.set((usize::MAX, 0)));
        OBSERVER.with(|v| *v.borrow_mut() = None);
    }
    fn bigint(n: &Natural) -> BigInt {
        BigInt::from_bytes_le(
            Sign::Plus,
            &n.words_le()
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )
    }
    fn rem(x: &BigInt, q: &BigInt) -> BigInt {
        let r = x % q;
        if r.sign() == Sign::Minus { r + q } else { r }
    }
    fn center(x: &BigInt, q: &BigInt) -> BigInt {
        let r = rem(x, q);
        if &r > &(q / 2_u32) { r - q } else { r }
    }
    fn small(x: &BigInt, q: u64) -> u64 {
        let (_, words) = rem(x, &BigInt::from(q)).to_u64_digits();
        words.first().copied().unwrap_or(0)
    }
    fn abs(x: &BigInt) -> BigInt {
        if x.sign() == Sign::Minus {
            -x
        } else {
            x.clone()
        }
    }
    fn conv(a: &[BigInt], b: &[BigInt]) -> Vec<BigInt> {
        let n = a.len();
        let mut out = vec![BigInt::from(0); n];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                if i + j < n {
                    out[i + j] += x * y;
                } else {
                    out[i + j - n] -= x * y;
                }
            }
        }
        out
    }
    fn components(profile: &BgvProfile, value: &Ciphertext) -> Vec<Vec<BigInt>> {
        let n = profile.degree();
        let limbs = value.level() + 1;
        let qs = &profile.parameters().main_primes[..limbs];
        let q = qs.iter().fold(BigInt::from(1), |a, &v| a * v);
        let weights: Vec<_> = qs
            .iter()
            .map(|&p| {
                let f = &q / p;
                let inverse = modular::mod_pow_u64(small(&f, p), p - 2, p);
                f * inverse
            })
            .collect();
        let words = Zeroizing::new(value.canonical_words());
        (0..value.component_count())
            .map(|c| {
                (0..n)
                    .map(|i| {
                        let x = qs.iter().enumerate().fold(BigInt::from(0), |a, (l, _)| {
                            a + words[(c * limbs + l) * n + i] * &weights[l]
                        });
                        center(&x, &q)
                    })
                    .collect()
            })
            .collect()
    }
    fn phase(profile: &BgvProfile, value: &Ciphertext, secret: &[i64]) -> Vec<BigInt> {
        let c = components(profile, value);
        assert_eq!(c.len(), 2);
        let s: Vec<_> = secret.iter().map(|&v| BigInt::from(v)).collect();
        let product = conv(&c[1], &s);
        let q = bigint(&profile.level_modulus(value.level()).unwrap());
        c[0].iter()
            .zip(product)
            .map(|(a, b)| center(&(a + b), &q))
            .collect()
    }
    fn assert_bound(profile: &BgvProfile, value: &Ciphertext, secret: &[i64]) -> Vec<BigInt> {
        let out = phase(profile, value, secret);
        let bound = bigint(value.noise_bound().as_natural());
        assert!(out.iter().all(|x| abs(x) <= bound));
        out
    }
    fn max_phase(profile: &BgvProfile, value: &Ciphertext, secret: &[i64]) -> BigInt {
        assert_bound(profile, value, secret)
            .iter()
            .map(abs)
            .max()
            .unwrap()
    }
    struct Oracle {
        secret: Zeroizing<Vec<i64>>,
        homomorphic: Vec<u64>,
        message: Vec<u64>,
        r: u32,
        seen: [usize; 4],
    }
    pub(super) fn observe_source(
        profile: &BgvProfile,
        plan: &BootstrapPlan,
        value: &Ciphertext,
    ) -> Result<(), BootstrapError> {
        OBSERVER.with(|cell| {
            if let Some(o) = cell.borrow_mut().as_mut() {
                let phase = assert_bound(profile, value, &o.secret);
                let t = value.plaintext_precision().modulus();
                o.r = value.plaintext_precision().exponent();
                o.message = phase.iter().map(|x| small(x, t)).collect();
                let components = components(profile, value);
                let q = bigint(&profile.level_modulus(value.level()).unwrap());
                let scaled: Vec<Vec<_>> = components
                    .iter()
                    .map(|c| {
                        c.iter()
                            .map(|x| center(&(x * plan.preparation.scale), &q))
                            .collect()
                    })
                    .collect();
                let secret: Vec<_> = o.secret.iter().map(|&x| BigInt::from(x)).collect();
                let product = conv(&scaled[1], &secret);
                let master = profile.master_plaintext_modulus();
                o.homomorphic = scaled[0]
                    .iter()
                    .zip(product)
                    .map(|(a, b)| small(&(a + b), master))
                    .collect();
                o.seen = [0; 4];
            }
        });
        checkpoint("source")
    }
    pub(super) fn observe_homomorphic(
        profile: &BgvProfile,
        value: &Ciphertext,
    ) -> Result<(), BootstrapError> {
        OBSERVER.with(|cell| {
            if let Some(o) = cell.borrow_mut().as_mut() {
                let actual = assert_bound(profile, value, &o.secret);
                assert!(
                    actual
                        .iter()
                        .zip(&o.homomorphic)
                        .all(|(a, &b)| small(a, profile.master_plaintext_modulus()) == b)
                );
                o.seen[0] += 1;
            }
        });
        checkpoint("homomorphic")
    }
    pub(super) fn observe_bank(
        profile: &BgvProfile,
        bank: &CoefficientSlotBank,
        upper: bool,
    ) -> Result<(), BootstrapError> {
        OBSERVER.with(|cell| {
            if let Some(o) = cell.borrow_mut().as_mut() {
                let precision = bank.value().plaintext_precision();
                let modulus = precision.modulus();
                let encoding = SlotEncoding::new(profile, precision).unwrap();
                let slots = encoding.slots();
                let d = bank.banks();
                let r = bank.ordinal();
                let expected = if upper { &o.message } else { &o.homomorphic };
                let actual = assert_bound(profile, bank.value(), &o.secret);
                assert!(
                    actual
                        .iter()
                        .enumerate()
                        .all(|(i, x)| i % d == 0 || small(x, modulus) == 0)
                );
                for j in 0..slots {
                    let beta = modular::mod_pow_u64(encoding.root(), (2 * j + 1) as u64, modulus);
                    let mut power = 1_u64;
                    let mut sum = 0_u64;
                    for k in 0..slots {
                        sum = (sum + small(&actual[d * k], modulus) * power) % modulus;
                        power = power * beta % modulus;
                    }
                    assert!(sum == expected[r + d * j]);
                }
                o.seen[if upper { 2 } else { 1 }] += 1;
            }
        });
        checkpoint(if upper {
            "upper bank"
        } else {
            "coefficient bank"
        })
    }
    pub(super) fn observe_result(
        profile: &BgvProfile,
        value: &Ciphertext,
    ) -> Result<(), BootstrapError> {
        OBSERVER.with(|cell| {
            if let Some(o) = cell.borrow_mut().as_mut() {
                let actual = assert_bound(profile, value, &o.secret);
                let modulus = value.plaintext_precision().modulus();
                assert!(
                    actual
                        .iter()
                        .zip(&o.message)
                        .all(|(a, &b)| small(a, modulus) == b)
                );
                assert_eq!(value.plaintext_precision().exponent(), o.r);
                o.seen[3] += 1;
                let banks = profile.degree()
                    / SlotEncoding::slot_count(profile, profile.master_plaintext()).unwrap();
                assert_eq!(o.seen, [1, banks, banks, 1]);
            }
        });
        checkpoint("result")
    }
    fn make_profile(q: usize) -> BgvProfile {
        BgvProfile::new(BgvParameters::derive(4, 5, 3, (50, q), (60, 2), 2).unwrap()).unwrap()
    }
    fn program() -> UpperDigitsProgram {
        let context = PrimePowerContext::new(5, 3).unwrap();
        let g = UpperDigitsPlan::required(context, 1).unwrap();
        let plan = UpperDigitsPlan::new(
            context,
            1,
            ProgramBudget {
                coefficient_bytes: g.coefficient_bytes,
                preparation_scratch_bytes: g.preparation_scratch_bytes,
                program_bytes: g.program_bytes,
                preparation_operations: g.preparation_operations,
                live_values: g.live_values,
            },
        )
        .unwrap();
        UpperDigitsProgram::prepare(&plan).unwrap()
    }
    fn budget() -> BootstrapBudget {
        let route = SlotLinearBudget {
            coefficient_bytes: 1024,
            layout_metadata_bytes: 4096,
            preparation_scratch_bytes: 4096,
            output_residue_bytes: 1 << 20,
            scratch_residue_bytes: 1 << 20,
        };
        let transform = CoefficientSlotBudget {
            routing: route,
            owned_residue_bytes: 1 << 20,
            trace_plaintext_bytes: 4096,
        };
        BootstrapBudget {
            decomposition: transform,
            recomposition: transform,
            upper_digits: UpperDigitsBudget {
                compiler_steps: 1_000_000,
                metadata_bytes: 1 << 20,
                owned_residue_bytes: 1 << 20,
                plaintext_bytes: 4096,
                backend_scratch_bytes: 1 << 20,
                canonical_arithmetic_units: 1_000_000_000,
                scalar_word_operations: 1_000_000_000,
            },
            canonical_arithmetic_units: 1_000_000_000_000,
            scalar_word_operations: 1_000_000_000_000,
            stream_owned_residue_bytes: 1 << 20,
            stream_metadata_bytes: 1 << 20,
            retained_preparation_bytes: 1 << 22,
            input_owned_bytes: 1 << 20,
            input_plaintext_bytes: 4096,
            input_arithmetic_units: 1_000_000_000,
            input_addition_words: 1_000_000,
        }
    }
    fn setup_budget() -> LocalBootstrapSetupBudget {
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 1 << 20,
            returned_secret_bytes: 1 << 20,
            plaintext_bytes: 4096,
        }
    }
    fn starting_shape(profile: &BgvProfile, level: usize) -> CiphertextShape {
        let mut shape = profile
            .plan_fresh(profile.plaintext_precision(1).unwrap())
            .unwrap();
        while shape.level() > level {
            shape = profile.plan_switch_down(&shape).unwrap();
        }
        shape = profile.plan_multiply(&shape, &shape).unwrap();
        profile.plan_multiply(&shape, &shape).unwrap()
    }
    fn make_plan(profile: &BgvProfile, shape: &CiphertextShape) -> BootstrapPlan {
        let offset = profile.top_level() - 4;
        let levels = [
            3 + offset,
            3 + offset,
            3 + offset,
            2 + offset,
            1 + offset,
            1 + offset,
        ];
        BootstrapPlan::new(profile, shape, program(), &levels, budget()).unwrap()
    }
    fn setup(
        profile: &BgvProfile,
        plan: &BootstrapPlan,
        seed: u8,
    ) -> (SecretKey, LocalBootstrapKey) {
        LocalBootstrapKey::generate_with(
            profile,
            plan.galois_elements(),
            setup_budget(),
            &mut FixtureEntropy::new([seed; 64]),
        )
        .unwrap()
    }
    fn source<'a>(
        profile: &BgvProfile,
        key: &'a LocalBootstrapKey,
        level: usize,
        seed: u8,
    ) -> LocalBootstrapCiphertext<'a> {
        let plaintext = Plaintext::new(
            profile,
            profile.plaintext_precision(1).unwrap(),
            (0..profile.degree())
                .map(|i| ((i * i + 2 * i + 1) % 5) as u64)
                .collect(),
        )
        .unwrap();
        let source = key
            .encrypt_with(profile, &plaintext, &mut FixtureEntropy::new([seed; 64]))
            .unwrap();
        let source = key.switch_to(profile, source, level).unwrap();
        let source = key.square(profile, source).unwrap();
        key.square(profile, source).unwrap()
    }
    fn install(secret: &SecretKey) {
        OBSERVER.with(|cell| {
            *cell.borrow_mut() = Some(Oracle {
                secret: Zeroizing::new(secret.coefficients().to_vec()),
                homomorphic: vec![],
                message: vec![],
                r: 0,
                seen: [0; 4],
            })
        });
    }

    #[test]
    fn complete_same_key_constructor_and_repeated_square_refresh_preserve_all_coefficients() {
        reset();
        let profile = make_profile(5);
        let plan = make_plan(&profile, &starting_shape(&profile, 1));
        let repeat_input = profile
            .plan_multiply(plan.output_shape(), plan.output_shape())
            .unwrap();
        let repeat = make_plan(&profile, &repeat_input);
        assert!(plan.reduces_phase_bound());
        assert!(repeat.reduces_phase_bound());
        assert_eq!(
            plan.homomorphic_input_shape().bound(),
            &Natural::from_u64(85_993_566)
        );
        assert_eq!(
            plan.output_shape().bound(),
            &Natural::from_u64(4_710_378_958_336)
        );
        assert_eq!(plan.output_shape().level(), 1);
        assert_eq!(plan.galois_elements(), repeat.galois_elements());
        let (secret, key) = setup(&profile, &plan, 37);
        install(&secret);
        let mut expected: Vec<u64> = (0..profile.degree())
            .map(|i| ((i * i + 2 * i + 1) % 5) as u64)
            .collect();
        let clear_square = |values: &[u64]| {
            let integer: Vec<_> = values.iter().map(|&v| BigInt::from(v)).collect();
            conv(&integer, &integer)
                .iter()
                .map(|x| small(x, 5))
                .collect::<Vec<u64>>()
        };
        expected = clear_square(&clear_square(&expected));
        let mut value = source(&profile, &key, 1, 41);
        assert!(
            secret
                .decrypt(&profile, value.ciphertext())
                .unwrap()
                .coefficients()
                == expected
        );
        let before = max_phase(&profile, value.ciphertext(), secret.coefficients());
        value = key.refresh(&profile, &plan, value).unwrap();
        let after = max_phase(&profile, value.ciphertext(), secret.coefficients());
        assert!(after < before);
        for _ in 0..2 {
            value = key.square(&profile, value).unwrap();
            expected = clear_square(&expected);
            let before = max_phase(&profile, value.ciphertext(), secret.coefficients());
            assert_eq!(value.shape(), repeat_input);
            value = key.refresh(&profile, &repeat, value).unwrap();
            let after = max_phase(&profile, value.ciphertext(), secret.coefficients());
            assert!(after < before);
            assert!(
                secret
                    .decrypt(&profile, value.ciphertext())
                    .unwrap()
                    .coefficients()
                    == expected
            );
        }
        let decoded = secret.decrypt(&profile, value.ciphertext()).unwrap();
        OBSERVER.with(|cell| {
            assert!(decoded.coefficients() == cell.borrow().as_ref().unwrap().message)
        });
        let ordinary = value.into_ciphertext();
        assert_eq!(ordinary.shape(), *repeat.output_shape());
        let mut basis = vec![0_u64; profile.degree()];
        basis[profile.degree() - 1] = 1;
        for (index, message) in [
            vec![0_u64; profile.degree()],
            basis,
            vec![3_u64; profile.degree()],
        ]
        .into_iter()
        .enumerate()
        {
            let plaintext = Plaintext::new(
                &profile,
                profile.plaintext_precision(1).unwrap(),
                message.clone(),
            )
            .unwrap();
            let value = key
                .encrypt_with(
                    &profile,
                    &plaintext,
                    &mut FixtureEntropy::new([110 + index as u8; 64]),
                )
                .unwrap();
            let value = key.switch_to(&profile, value, 1).unwrap();
            let value = key.square(&profile, value).unwrap();
            let value = key.square(&profile, value).unwrap();
            let expected = clear_square(&clear_square(&message));
            let value = key.refresh(&profile, &plan, value).unwrap();
            assert!(
                secret
                    .decrypt(&profile, value.ciphertext())
                    .unwrap()
                    .coefficients()
                    == expected
            );
        }
        reset();
    }

    #[test]
    fn q4_whole_ring_completion_fits_but_ciphertext_square_is_refused() {
        reset();
        let profile = make_profile(4);
        let shape = starting_shape(&profile, 1);
        let plan =
            BootstrapPlan::new(&profile, &shape, program(), &[2, 2, 2, 1, 0, 0], budget()).unwrap();
        assert_eq!(plan.output_shape().level(), 0);
        assert_eq!(
            plan.output_shape().bound(),
            &Natural::from_u64(4_710_378_958_336)
        );
        assert!(matches!(
            profile.plan_multiply(plan.output_shape(), plan.output_shape()),
            Err(BgvError::NoiseBudgetExceeded { .. })
        ));
    }

    #[test]
    fn closed_origin_and_complete_key_preflight_precede_source_access() {
        reset();
        let profile = make_profile(5);
        let plan = make_plan(&profile, &starting_shape(&profile, 1));
        let (_, key) = setup(&profile, &plan, 43);
        let (_, foreign) = setup(&profile, &plan, 47);
        let value = source(&profile, &foreign, 1, 49);
        ACCESSES.with(|v| v.set(0));
        assert!(matches!(
            key.refresh(&profile, &plan, value),
            Err(BootstrapError::Origin)
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let (_, missing) = LocalBootstrapKey::generate_with(
            &profile,
            &[],
            setup_budget(),
            &mut FixtureEntropy::new([53; 64]),
        )
        .unwrap();
        let value = source(&profile, &missing, 1, 59);
        ACCESSES.with(|v| v.set(0));
        assert!(matches!(
            missing.refresh(&profile, &plan, value),
            Err(BootstrapError::Bgv(BgvError::WrongSwitchingKey))
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let plaintext =
            Plaintext::constant(&profile, profile.plaintext_precision(1).unwrap(), 1).unwrap();
        let fresh = key
            .encrypt_with(&profile, &plaintext, &mut FixtureEntropy::new([60; 64]))
            .unwrap();
        ACCESSES.with(|v| v.set(0));
        assert!(matches!(
            key.refresh(&profile, &plan, fresh),
            Err(BootstrapError::Shape)
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let foreign_profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 2, (50, 5), (60, 2), 2).unwrap()).unwrap();
        let value = source(&profile, &key, 1, 62);
        ACCESSES.with(|v| v.set(0));
        assert!(matches!(
            key.refresh(&foreign_profile, &plan, value),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let a = source(&profile, &key, 1, 61);
        let b = source(&profile, &foreign, 1, 67);
        assert!(matches!(
            key.multiply(&profile, a, b),
            Err(BootstrapError::Origin)
        ));
        let a = source(&profile, &key, 1, 71);
        let words = a.ciphertext().components()[0].words().as_ptr();
        let a = key.switch_to(&profile, a, 1).unwrap();
        assert_eq!(words, a.ciphertext().components()[0].words().as_ptr());
        assert!(matches!(
            key.switch_to(&profile, a, 2),
            Err(BootstrapError::Bgv(BgvError::LevelMismatch { .. }))
        ));
        reset();
    }

    #[test]
    fn input_stream_work_budgets_and_invalid_geometry_refuse_before_private_execution() {
        reset();
        let profile = make_profile(5);
        let shape = starting_shape(&profile, 1);
        let plan = make_plan(&profile, &shape);
        let payload = plan.payload();
        assert_eq!(payload.canonical_arithmetic_units, 318_128);
        assert_eq!(payload.scalar_word_operations, 13_776);
        assert!(matches!(
            aggregate_work(
                &profile,
                u64::MAX,
                0,
                &plan.preparation.decomposition,
                &plan.preparation.digits,
                &plan.preparation.recomposition
            ),
            Err(BootstrapError::SizeOverflow)
        ));
        assert!(matches!(
            aggregate_work(
                &profile,
                0,
                u64::MAX,
                &plan.preparation.decomposition,
                &plan.preparation.digits,
                &plan.preparation.recomposition
            ),
            Err(BootstrapError::SizeOverflow)
        ));
        let mut exact = budget();
        exact.canonical_arithmetic_units = payload.canonical_arithmetic_units;
        exact.scalar_word_operations = payload.scalar_word_operations;
        exact.stream_owned_residue_bytes = payload.stream_owned_residue_bytes;
        exact.stream_metadata_bytes = payload.stream_metadata_bytes;
        exact.retained_preparation_bytes = payload.retained_preparation_bytes;
        exact.input_owned_bytes = payload.input_owned_bytes;
        exact.input_plaintext_bytes = payload.input_plaintext_bytes;
        exact.input_arithmetic_units = payload.input_arithmetic_units;
        exact.input_addition_words = payload.input_addition_words;
        assert!(
            BootstrapPlan::new(&profile, &shape, program(), &[3, 3, 3, 2, 1, 1], exact).is_ok()
        );
        for field in 0..9 {
            let mut bad = exact;
            let name = match field {
                7 => {
                    bad.canonical_arithmetic_units -= 1;
                    "complete arithmetic"
                }
                8 => {
                    bad.scalar_word_operations -= 1;
                    "complete scalar words"
                }
                5 => {
                    bad.stream_owned_residue_bytes -= 1;
                    "stream residues"
                }
                6 => {
                    bad.stream_metadata_bytes -= 1;
                    "stream metadata"
                }
                0 => {
                    bad.retained_preparation_bytes -= 1;
                    "retained preparation"
                }
                1 => {
                    bad.input_owned_bytes -= 1;
                    "input owners"
                }
                2 => {
                    bad.input_plaintext_bytes -= 1;
                    "input plaintext"
                }
                3 => {
                    bad.input_arithmetic_units -= 1;
                    "input arithmetic"
                }
                _ => {
                    bad.input_addition_words -= 1;
                    "input addition"
                }
            };
            let result = BootstrapPlan::new(&profile, &shape, program(), &[3, 3, 3, 2, 1, 1], bad);
            assert!(matches!(result,Err(BootstrapError::Budget{resource}) if resource==name));
        }
        let q = profile.level_modulus(1).unwrap();
        let a = 25_u64;
        let threshold = q
            .mul_u64(a - profile.degree() as u64 - 1)
            .div_floor_u64(2 * a)
            .unwrap();
        let below = profile
            .plan_claimed(
                profile.plaintext_precision(1).unwrap(),
                1,
                2,
                threshold.clone(),
            )
            .unwrap();
        assert!(
            BootstrapPlan::new(&profile, &below, program(), &[3, 3, 3, 2, 1, 1], budget()).is_ok()
        );
        let above = profile
            .plan_claimed(
                profile.plaintext_precision(1).unwrap(),
                1,
                2,
                threshold.add_u64(1),
            )
            .unwrap();
        assert!(matches!(
            BootstrapPlan::new(&profile, &above, program(), &[3, 3, 3, 2, 1, 1], budget()),
            Err(BootstrapError::RoundingBound)
        ));
        let master = profile.plan_fresh(profile.master_plaintext()).unwrap();
        assert!(matches!(
            BootstrapPlan::new(&profile, &master, program(), &[3, 3, 3, 2, 1, 1], budget()),
            Err(BootstrapError::Shape)
        ));
        let fresh = profile.plan_fresh(profile.master_plaintext()).unwrap();
        assert!(matches!(
            EncryptedUpperDigitsPlan::from_shape(
                &profile,
                program(),
                &fresh,
                4,
                4,
                &[3, 3, 3, 2, 1, 1],
                budget().upper_digits,
                BankRetention::Collected,
            ),
            Err(UpperDigitsError::Source)
        ));
        assert_eq!(
            SlotEncoding::slot_count(&profile, profile.master_plaintext()).unwrap(),
            2
        );
        let unavailable =
            BgvProfile::new(BgvParameters::derive(4, 3, 2, (50, 4), (60, 2), 2).unwrap()).unwrap();
        assert_eq!(
            SlotEncoding::slot_count(&unavailable, unavailable.master_plaintext()),
            Err(BgvError::EncodingUnavailable)
        );
        assert_eq!(
            SlotEncoding::slot_count(&profile, unavailable.master_plaintext()),
            Err(BgvError::ProfileMismatch)
        );
        assert_eq!(mul(usize::MAX, 2), Err(BootstrapError::SizeOverflow));
        assert_eq!(add(usize::MAX, 1), Err(BootstrapError::SizeOverflow));
        reset();
    }

    #[test]
    fn setup_uses_resident_sizes_and_refuses_before_entropy_or_private_allocations() {
        reset();
        let profile = make_profile(5);
        let plan = make_plan(&profile, &starting_shape(&profile, 1));
        let n = profile.degree();
        let top = profile.top_level() + 1;
        let full = top + profile.special_count();
        let g = plan.galois_elements();
        let exact = LocalBootstrapSetupBudget {
            retained_residue_bytes: 8
                * n
                * (4 * top + (g.len() + 1) * 2 * profile.digit_count() * full),
            returned_secret_bytes: 8 * n * (full + 1),
            plaintext_bytes: 8 * n,
        };
        for field in 0..3 {
            let mut bad = exact;
            match field {
                0 => bad.retained_residue_bytes -= 1,
                1 => bad.returned_secret_bytes -= 1,
                _ => bad.plaintext_bytes -= 1,
            };
            let mut entropy = FixtureEntropy::new([73; 64]);
            assert!(matches!(
                LocalBootstrapKey::generate_with(&profile, g, bad, &mut entropy),
                Err(BootstrapError::Budget { .. })
            ));
            assert_eq!(entropy.draws(), 0);
        }
        let mut entropy = FixtureEntropy::new([79; 64]);
        let (secret, key) =
            LocalBootstrapKey::generate_with(&profile, g, exact, &mut entropy).unwrap();
        assert_eq!(
            key.encrypted_secret.shape(),
            profile.plan_fresh(profile.master_plaintext()).unwrap()
        );
        let decoded = secret.decrypt(&profile, &key.encrypted_secret).unwrap();
        assert!(
            decoded
                .coefficients()
                .iter()
                .zip(secret.coefficients())
                .all(|(&a, &b)| a == small_residue(b, 125))
        );
        let mut entropy = FixtureEntropy::failing_after([83; 64], 0);
        assert!(matches!(
            LocalBootstrapKey::generate_with(&profile, g, exact, &mut entropy),
            Err(BootstrapError::Bgv(BgvError::EntropyUnavailable))
        ));
        let mut entropy = FixtureEntropy::failing_after([89; 64], (g.len() + 3) as u32);
        let before = poly::tests::cleared_words();
        assert!(LocalBootstrapKey::generate_with(&profile, g, exact, &mut entropy).is_err());
        assert!(poly::tests::cleared_words() > before);
        reset();
    }

    #[test]
    fn conversion_allocation_late_stage_error_and_unwind_clear_consumed_sources() {
        reset();
        let profile = make_profile(5);
        let plan = make_plan(&profile, &starting_shape(&profile, 1));
        let (_, key) = setup(&profile, &plan, 97);
        for fail in 1..=2 {
            let value = source(&profile, &key, 1, 101);
            let words = value.ciphertext().canonical_words().len();
            let before = poly::tests::cleared_words();
            ALLOCS.with(|v| v.set(0));
            FAIL_ALLOC.with(|v| v.set(fail));
            assert!(matches!(
                key.refresh(&profile, &plan, value),
                Err(BootstrapError::Allocation)
            ));
            FAIL_ALLOC.with(|v| v.set(usize::MAX));
            assert!(poly::tests::cleared_words() - before >= words);
        }
        // Source, two conversions, H, every emitted scalar/upper bank, final result.
        for stage in 1..=5 + 2 * plan.preparation.decomposition.banks() {
            for mode in [1, 2] {
                let value = source(&profile, &key, 1, 103);
                ACCESSES.with(|v| v.set(0));
                FAULT.with(|v| v.set((stage, mode)));
                let result = catch_unwind(AssertUnwindSafe(|| key.refresh(&profile, &plan, value)));
                assert!(if mode == 2 {
                    result.is_err()
                } else {
                    matches!(result, Ok(Err(BootstrapError::Shape)))
                });
                assert!(poly::tests::cleared_words() > AT_FAULT.with(Cell::get));
            }
        }
        reset();
    }

    fn routing_fixture(profile: &BgvProfile) -> (SlotLinearPlan<2, 3>, SlotLinearMap<2, 3>) {
        let fresh = profile
            .plan_fresh(profile.plaintext_precision(1).unwrap())
            .unwrap();
        let squared = profile.plan_multiply(&fresh, &fresh).unwrap();
        let segments = (0..6)
            .map(|row| match row / 2 {
                0 => SlotLinearSegment::new(4, 1, 1),
                1 => SlotLinearSegment::new(row % 2, 1, 1),
                _ => SlotLinearSegment::new(0, 5, 1),
            })
            .collect();
        let layout = SlotLinearLayout::new(2, (0..=6).collect(), segments, 4096).unwrap();
        let plan = SlotLinearPlan::with_layout(
            profile,
            &[fresh, squared],
            layout,
            budget().decomposition.routing,
        )
        .unwrap();
        let map = SlotLinearMap::prepare_rows(&plan, |row, values| {
            match row / 2 {
                0 => values[4] = (row + 1) as u32,
                1 => values[row % 2] = 1,
                _ => values.copy_from_slice(if row % 2 == 0 {
                    &[1, 2, 3, 4, 1]
                } else {
                    &[4, 3, 2, 1, 2]
                }),
            }
            Ok(())
        })
        .unwrap();
        (plan, map)
    }
    fn routing_inputs<'a>(
        profile: &BgvProfile,
        lhs: &'a LocalBootstrapKey,
        rhs: &'a LocalBootstrapKey,
    ) -> [LocalBootstrapCiphertext<'a>; 2] {
        let encoding = SlotEncoding::new(profile, profile.plaintext_precision(1).unwrap()).unwrap();
        let a = encoding.encode(profile, &[1, 2]).unwrap();
        let b = encoding.encode(profile, &[3, 4]).unwrap();
        [
            lhs.encrypt_with(profile, &a, &mut FixtureEntropy::new([151; 64]))
                .unwrap(),
            rhs.square(
                profile,
                rhs.encrypt_with(profile, &b, &mut FixtureEntropy::new([157; 64]))
                    .unwrap(),
            )
            .unwrap(),
        ]
    }
    fn routing_ceiling(profile: &BgvProfile, plan: &SlotLinearPlan<2, 3>) -> usize {
        4 * profile.degree() * (profile.top_level() + 1) * size_of::<u64>()
            + plan.output_residue_bytes()
            + plan.scratch_residue_bytes()
    }
    fn scalar_pair_coefficients(root: u64, values: [u64; 2]) -> Vec<u64> {
        assert_eq!(root * root % 5, 4);
        let inverse = (1..5).find(|x| 2 * root * x % 5 == 1).unwrap();
        let mut coefficients = vec![0; 16];
        coefficients[0] = (values[0] + values[1]) * 3 % 5;
        coefficients[8] = (values[0] + 5 - values[1]) * inverse % 5;
        coefficients
    }
    #[test]
    fn closed_routing_exact_owner_ceiling_keeps_heterogeneous_shapes_and_refresh_origin() {
        reset();
        let profile = make_profile(5);
        let (route, map) = routing_fixture(&profile);
        assert_ne!(route.output_shapes()[0], route.output_shapes()[1]);
        assert_ne!(route.output_shapes()[1], route.output_shapes()[2]);
        let plans: Vec<_> = route
            .output_shapes()
            .iter()
            .map(|shape| {
                let squared = profile.plan_multiply(shape, shape).unwrap();
                make_plan(&profile, &squared)
            })
            .collect();
        let mut galois = route.galois_elements().to_vec();
        for plan in &plans {
            galois.extend_from_slice(plan.galois_elements());
        }
        galois.sort_unstable();
        galois.dedup();
        let (secret, key) = LocalBootstrapKey::generate_with(
            &profile,
            &galois,
            setup_budget(),
            &mut FixtureEntropy::new([163; 64]),
        )
        .unwrap();
        let input = routing_inputs(&profile, &key, &key);
        let input_words = input.each_ref().map(|value| value.value.canonical_words());
        let input_shapes = input.each_ref().map(|value| value.shape());
        let input_pointers = input.each_ref().map(|value| {
            std::array::from_fn::<_, 2, _>(|component| {
                value.value.components()[component].words().as_ptr()
            })
        });
        let (returned, output) = key
            .route(
                &profile,
                &route,
                &map,
                input,
                routing_ceiling(&profile, &route),
            )
            .unwrap();
        for (index, value) in returned.iter().enumerate() {
            assert_eq!(value.value.canonical_words(), input_words[index]);
            assert_eq!(value.shape(), input_shapes[index]);
            for component in 0..2 {
                assert_eq!(
                    value.value.components()[component].words().as_ptr(),
                    input_pointers[index][component]
                );
            }
            key.preflight_evaluation(
                &profile,
                value,
                &input_shapes[index],
                route.galois_elements(),
            )
            .unwrap();
        }
        drop(returned);
        let encoding =
            SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
        for (((value, shape), plan), slots) in output
            .into_iter()
            .zip(route.output_shapes())
            .zip(&plans)
            .zip([[1, 2], [1, 2], [2, 1]])
        {
            assert_eq!(value.shape(), *shape);
            let expected = scalar_pair_coefficients(encoding.root(), slots);
            let actual: Vec<_> = assert_bound(&profile, value.ciphertext(), secret.coefficients())
                .iter()
                .map(|x| small(x, 5))
                .collect();
            assert_eq!(actual, expected);
            let value = key.square(&profile, value).unwrap();
            let integer: Vec<_> = expected.iter().map(|&x| BigInt::from(x)).collect();
            let expected: Vec<_> = conv(&integer, &integer)
                .iter()
                .map(|x| small(x, 5))
                .collect();
            let actual: Vec<_> = assert_bound(&profile, value.ciphertext(), secret.coefficients())
                .iter()
                .map(|x| small(x, 5))
                .collect();
            assert_eq!(actual, expected);
            let value = key.refresh(&profile, plan, value).unwrap();
            let actual: Vec<_> = assert_bound(&profile, value.ciphertext(), secret.coefficients())
                .iter()
                .map(|x| small(x, 5))
                .collect();
            assert_eq!(actual, expected);
            assert_eq!(value.shape(), *plan.output_shape());
        }
        reset();
    }
    #[test]
    fn closed_routing_origin_budget_and_canonical_preflight_clear_only_inputs() {
        reset();
        let profile = make_profile(5);
        let (route, map) = routing_fixture(&profile);
        let (_, key) = LocalBootstrapKey::generate_with(
            &profile,
            route.galois_elements(),
            setup_budget(),
            &mut FixtureEntropy::new([167; 64]),
        )
        .unwrap();
        let (_, foreign) = LocalBootstrapKey::generate_with(
            &profile,
            route.galois_elements(),
            setup_budget(),
            &mut FixtureEntropy::new([173; 64]),
        )
        .unwrap();
        let words = 4 * profile.degree() * (profile.top_level() + 1);
        let ceiling = routing_ceiling(&profile, &route);
        for foreign_at in 0..2 {
            let input = if foreign_at == 0 {
                routing_inputs(&profile, &foreign, &key)
            } else {
                routing_inputs(&profile, &key, &foreign)
            };
            let before = poly::tests::cleared_words();
            assert!(matches!(
                key.route(&profile, &route, &map, input, ceiling),
                Err(BootstrapError::Origin)
            ));
            assert_eq!(poly::tests::cleared_words() - before, words);
            assert_eq!(ACCESSES.with(Cell::get), 0);
        }
        let input = routing_inputs(&profile, &key, &key);
        let before = poly::tests::cleared_words();
        assert!(matches!(
            key.route(&profile, &route, &map, input, ceiling - 1),
            Err(BootstrapError::Budget {
                resource: "route owners"
            })
        ));
        assert_eq!(poly::tests::cleared_words() - before, words);
        assert_eq!(ACCESSES.with(Cell::get), 0);
        assert!(!route.galois_elements().is_empty());
        let (_, missing) = LocalBootstrapKey::generate_with(
            &profile,
            &[],
            setup_budget(),
            &mut FixtureEntropy::new([179; 64]),
        )
        .unwrap();
        let input = routing_inputs(&profile, &missing, &missing);
        let before = poly::tests::cleared_words();
        assert!(matches!(
            missing.route(&profile, &route, &map, input, ceiling),
            Err(BootstrapError::Routing(SlotLinearError::Bgv(
                BgvError::WrongSwitchingKey
            )))
        ));
        assert_eq!(poly::tests::cleared_words() - before, words);
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let input = routing_inputs(&profile, &key, &key);
        let [a, b] = input;
        let before = poly::tests::cleared_words();
        assert!(matches!(
            key.route(&profile, &route, &map, [b, a], ceiling),
            Err(BootstrapError::Routing(SlotLinearError::InputShape {
                bank: 0
            }))
        ));
        assert_eq!(poly::tests::cleared_words() - before, words);
        assert_eq!(ACCESSES.with(Cell::get), 0);
        reset();
    }
    #[test]
    fn closed_routing_late_error_and_unwind_clear_inputs_and_actual_outputs() {
        reset();
        let profile = make_profile(5);
        let (route, map) = routing_fixture(&profile);
        let (_, key) = LocalBootstrapKey::generate_with(
            &profile,
            route.galois_elements(),
            setup_budget(),
            &mut FixtureEntropy::new([181; 64]),
        )
        .unwrap();
        let words = 10 * profile.degree() * (profile.top_level() + 1);
        for mode in [1, 2] {
            let inputs = routing_inputs(&profile, &key, &key);
            ACCESSES.with(|v| v.set(0));
            FAULT.with(|v| v.set((1, mode)));
            let result = catch_unwind(AssertUnwindSafe(|| {
                key.route(
                    &profile,
                    &route,
                    &map,
                    inputs,
                    routing_ceiling(&profile, &route),
                )
            }));
            assert!(if mode == 2 {
                result.is_err()
            } else {
                matches!(result, Ok(Err(BootstrapError::Shape)))
            });
            assert_eq!(
                poly::tests::cleared_words() - AT_FAULT.with(Cell::get),
                words
            );
        }
        reset();
    }
    #[test]
    fn streamed_and_collected_coefficient_upper_inverse_have_identical_residues_and_all_bank_work()
    {
        reset();
        for (log_n, prime, exponent, retained, primes) in [
            (2, 17, 2, 1, 8),
            (2, 5, 2, 1, 5),
            (2, 5, 3, 2, 5),
            (4, 5, 3, 1, 5),
        ] {
            let profile = BgvProfile::new(
                BgvParameters::derive(log_n, prime, exponent, (50, primes), (60, 2), 2).unwrap(),
            )
            .unwrap();
            let precision = profile.master_plaintext();
            let input_shape = profile.plan_fresh(precision).unwrap();
            let make_program = || {
                let context = PrimePowerContext::new(prime, exponent).unwrap();
                let required = UpperDigitsPlan::required(context, retained).unwrap();
                UpperDigitsProgram::prepare(
                    &UpperDigitsPlan::new(
                        context,
                        retained,
                        ProgramBudget {
                            coefficient_bytes: required.coefficient_bytes,
                            preparation_scratch_bytes: required.preparation_scratch_bytes,
                            program_bytes: required.program_bytes,
                            preparation_operations: required.preparation_operations,
                            live_values: required.live_values,
                        },
                    )
                    .unwrap(),
                )
                .unwrap()
            };
            let levels: Vec<usize> = if prime == 17 {
                vec![6, 5, 4, 3, 2]
            } else if exponent == 2 {
                vec![3, 2, 2]
            } else {
                vec![3, 3, 3, 2, 1, 1]
            };
            let collected =
                CoefficientSlotDecomposition::new(&profile, &input_shape, budget().decomposition)
                    .unwrap();
            let streamed = CoefficientSlotDecomposition::prepare(
                &profile,
                &input_shape,
                budget().decomposition,
                BankRetention::Streaming,
            )
            .unwrap();
            assert_eq!(collected.output_shape(), streamed.output_shape());
            assert_eq!(collected.operations(), streamed.operations());
            let prepare_upper = |retention| {
                EncryptedUpperDigitsPlan::from_shape(
                    &profile,
                    make_program(),
                    collected.output_shape(),
                    collected.slots(),
                    collected.banks(),
                    &levels,
                    budget().upper_digits,
                    retention,
                )
                .unwrap()
            };
            let upper_all = prepare_upper(BankRetention::Collected);
            let upper_one = prepare_upper(BankRetention::Streaming);
            assert_eq!(upper_all.output_shape(), upper_one.output_shape());
            assert_eq!(upper_all.operations(), upper_one.operations());
            assert_eq!(
                upper_all.payload().canonical_arithmetic_units,
                upper_one.payload().canonical_arithmetic_units
            );
            assert_eq!(
                upper_all.payload().scalar_word_operations,
                upper_one.payload().scalar_word_operations
            );
            assert_eq!(
                upper_one.payload().owned_residue_bytes,
                upper_one.payload().single_bank_residue_bytes
            );
            let restored_all = CoefficientSlotRecomposition::new(
                &profile,
                upper_all.output_shape(),
                budget().recomposition,
            )
            .unwrap();
            let restored_one = CoefficientSlotRecomposition::prepare(
                &profile,
                upper_one.output_shape(),
                budget().recomposition,
                BankRetention::Streaming,
            )
            .unwrap();
            let mut galois = collected.galois_elements().to_vec();
            galois.extend_from_slice(restored_all.galois_elements());
            galois.sort_unstable();
            galois.dedup();
            let keys = crate::bgv::fixtures::key_set(
                &profile,
                &galois,
                &mut FixtureEntropy::new([209; 64]),
            )
            .unwrap();
            let n = profile.degree();
            let t = precision.modulus();
            let scale = prime.pow(exponent - retained);
            let mut messages: Vec<Vec<u64>> = (0..n)
                .map(|j| (0..n).map(|i| if i == j { scale } else { 0 }).collect())
                .collect();
            messages.extend([
                vec![0; n],
                vec![t / 2; n],
                (0..n).map(|i| (17 * i * i + i + 3) as u64 % t).collect(),
            ]);
            for message in messages {
                let plaintext = Plaintext::new(&profile, precision, message.clone()).unwrap();
                let input = keys
                    .public
                    .encrypt_with(&profile, &plaintext, &mut FixtureEntropy::new([211; 64]))
                    .unwrap();
                let banks = collected
                    .evaluate(&profile, input.clone(), &keys.evaluation)
                    .unwrap();
                let before = Zeroizing::new(
                    banks
                        .banks()
                        .iter()
                        .map(Ciphertext::canonical_words)
                        .collect::<Vec<_>>(),
                );
                let after = upper_all
                    .evaluate(&profile, banks, keys.evaluation.relinearization())
                    .unwrap();
                let mut cursor = streamed.start(&profile, input, &keys.evaluation).unwrap();
                let mut accumulator = restored_one
                    .start_accumulator(&profile, &keys.evaluation)
                    .unwrap();
                let mut ordinals = Vec::new();
                while let Some((bank, rest)) = cursor.advance(&profile, &keys.evaluation).unwrap() {
                    let r = bank.ordinal();
                    ordinals.push(r);
                    assert_eq!(bank.value().canonical_words(), before[r]);
                    let bank = bank
                        .upper(&profile, &upper_one, keys.evaluation.relinearization())
                        .unwrap();
                    assert_eq!(bank.value().shape(), after.banks()[r].shape());
                    assert_eq!(
                        bank.value().canonical_words(),
                        after.banks()[r].canonical_words()
                    );
                    accumulator = accumulator.push(&profile, bank, &keys.evaluation).unwrap();
                    cursor = rest;
                }
                ordinals.sort_unstable();
                assert_eq!(ordinals, (0..collected.banks()).collect::<Vec<_>>());
                let actual = accumulator.finish().unwrap();
                let reference = restored_all
                    .evaluate(&profile, after, &keys.evaluation)
                    .unwrap();
                assert_eq!(actual.shape(), reference.shape());
                assert_eq!(actual.canonical_words(), reference.canonical_words());
                assert_eq!(
                    actual.to_bytes(&profile).unwrap(),
                    reference.to_bytes(&profile).unwrap()
                );
                assert_bound(&profile, &actual, keys.secret.coefficients());
                let expected: Vec<_> = message
                    .into_iter()
                    .map(|x| {
                        let mut x = i128::from(x);
                        for _ in 0..exponent - retained {
                            let mut digit = x.rem_euclid(i128::from(prime));
                            if digit > i128::from(prime / 2) {
                                digit -= i128::from(prime);
                            }
                            x = (x - digit) / i128::from(prime);
                        }
                        x.rem_euclid(i128::from(prime.pow(retained))) as u64
                    })
                    .collect();
                assert_eq!(
                    keys.secret
                        .decrypt(&profile, &actual)
                        .unwrap()
                        .coefficients(),
                    expected
                );
            }
        }
        reset();
    }
    #[test]
    fn supported_single_bank_local_bootstrap_repeats_with_strict_carry_and_exact_phase() {
        reset();
        let profile =
            BgvProfile::new(BgvParameters::derive(2, 17, 2, (50, 8), (60, 2), 2).unwrap()).unwrap();
        let make_program = || {
            let context = PrimePowerContext::new(17, 2).unwrap();
            let g = UpperDigitsPlan::required(context, 1).unwrap();
            UpperDigitsProgram::prepare(
                &UpperDigitsPlan::new(
                    context,
                    1,
                    ProgramBudget {
                        coefficient_bytes: g.coefficient_bytes,
                        preparation_scratch_bytes: g.preparation_scratch_bytes,
                        program_bytes: g.program_bytes,
                        preparation_operations: g.preparation_operations,
                        live_values: g.live_values,
                    },
                )
                .unwrap(),
            )
            .unwrap()
        };
        let levels = [6, 5, 4, 3, 2];
        let mut source = profile
            .plan_fresh(profile.plaintext_precision(1).unwrap())
            .unwrap();
        while source.level() > 2 {
            source = profile.plan_switch_down(&source).unwrap();
        }
        assert_eq!(source.bound(), &crate::natural::Natural::from_u64(722));
        let plan =
            BootstrapPlan::new(&profile, &source, make_program(), &levels, budget()).unwrap();
        assert_eq!(plan.preparation.decomposition.banks(), 1);
        let blueprint = make_program().plan();
        let requirement = BootstrapPlan::metadata_requirement(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &blueprint,
        )
        .unwrap();
        assert!(
            plan.retained_metadata().unwrap().retained_heap_bytes
                <= requirement.retained_heap_bytes
        );

        assert_eq!(
            plan.preparation.homomorphic.bound(),
            &crate::natural::Natural::from_u64(31_544_784)
        );
        assert_eq!(
            plan.preparation.decomposition.output_shape().bound(),
            &crate::natural::Natural::from_u64(72_680_014_946)
        );
        assert_eq!(
            plan.preparation.digits.output_shape().bound(),
            &crate::natural::Natural::from_u64(122_740)
        );
        assert_eq!(
            plan.output_shape().bound(),
            &crate::natural::Natural::from_u64(15_757_658)
        );
        let repeat_shape = profile
            .plan_multiply(plan.output_shape(), plan.output_shape())
            .unwrap();
        assert_eq!(
            repeat_shape.bound(),
            &crate::natural::Natural::from_u64(993_215_142_580_578)
        );
        let repeat =
            BootstrapPlan::new(&profile, &repeat_shape, make_program(), &levels, budget()).unwrap();
        assert_eq!(repeat.output_shape(), plan.output_shape());
        assert!(repeat.output_shape().bound() < repeat_shape.bound());
        let (secret, key) = setup(&profile, &plan, 217);
        install(&secret);
        let mut expected = vec![0, 1, 8, 16];
        let plain = Plaintext::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            expected.clone(),
        )
        .unwrap();
        let value = key
            .encrypt_with(&profile, &plain, &mut FixtureEntropy::new([219; 64]))
            .unwrap();
        let value = key.switch_to(&profile, value, 2).unwrap();
        let mut value = key.refresh(&profile, &plan, value).unwrap();
        for _ in 0..3 {
            assert_eq!(
                secret
                    .decrypt(&profile, value.ciphertext())
                    .unwrap()
                    .coefficients(),
                expected
            );
            assert_bound(&profile, value.ciphertext(), secret.coefficients());
            value = key.square(&profile, value).unwrap();
            assert_eq!(value.shape(), repeat_shape);
            let integers: Vec<_> = expected.iter().map(|&x| BigInt::from(x)).collect();
            expected = conv(&integers, &integers)
                .iter()
                .map(|x| small(x, 17))
                .collect();
            value = key.refresh(&profile, &repeat, value).unwrap();
        }
        assert_eq!(
            secret
                .decrypt(&profile, value.ciphertext())
                .unwrap()
                .coefficients(),
            expected
        );
        assert_bound(&profile, value.ciphertext(), secret.coefficients());
        reset();
    }

    #[test]
    fn local_arithmetic_delegates_preserve_exact_context_words_and_constructibility() {
        reset();
        let profile = make_profile(5);
        let elements = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap())
            .unwrap()
            .broadcast_elements();
        let (secret, key) = LocalBootstrapKey::generate_with(
            &profile,
            &elements,
            setup_budget(),
            &mut FixtureEntropy::new([211; 64]),
        )
        .unwrap();
        assert_eq!(key.profile_identity(), profile.identity());
        for exponent in 1..=3 {
            let precision = profile.plaintext_precision(exponent).unwrap();
            let t = precision.modulus();
            let left_values: Vec<_> = (0..profile.degree())
                .map(|i| (i as u64 * 7 + 1) % t)
                .collect();
            let right_values: Vec<_> = (0..profile.degree())
                .map(|i| (i as u64 * 11 + t - 1) % t)
                .collect();
            let left_plain = Plaintext::new(&profile, precision, left_values.clone()).unwrap();
            let right_plain = Plaintext::new(&profile, precision, right_values.clone()).unwrap();
            let source = || {
                key.encrypt_with(&profile, &left_plain, &mut FixtureEntropy::new([212; 64]))
                    .unwrap()
            };
            let other = || {
                key.encrypt_with(&profile, &right_plain, &mut FixtureEntropy::new([213; 64]))
                    .unwrap()
            };
            let value = key.trivial(&profile, &left_plain, 2).unwrap();
            let expected = Ciphertext::trivial(&profile, &left_plain, 2).unwrap();
            assert_eq!(
                value.ciphertext().canonical_words(),
                expected.canonical_words()
            );
            assert_eq!(value.shape(), expected.shape());
            key.preflight_evaluation(&profile, &value, &expected.shape(), &elements)
                .unwrap();
            for operation in 0..3 {
                let left = source();
                let right = other();
                let expected = match operation {
                    0 => profile.add(left.ciphertext(), right.ciphertext()).unwrap(),
                    1 => profile.add_plain(left.ciphertext(), &right_plain).unwrap(),
                    _ => profile.mul_plain(left.ciphertext(), &right_plain).unwrap(),
                };
                let value = match operation {
                    0 => key.add(&profile, left, right).unwrap(),
                    1 => {
                        drop(right);
                        key.add_plain(&profile, left, &right_plain).unwrap()
                    }
                    _ => {
                        drop(right);
                        let original_shape = left.shape();
                        let original_words = Zeroizing::new(left.ciphertext().canonical_words());
                        let original_pointers: Vec<_> = left
                            .value
                            .components()
                            .iter()
                            .map(|component| component.words().as_ptr())
                            .collect();
                        let (retained, product) =
                            key.mul_plain(&profile, left, &right_plain).unwrap();
                        assert_eq!(retained.shape(), original_shape);
                        assert_eq!(
                            *Zeroizing::new(retained.ciphertext().canonical_words()),
                            *original_words
                        );
                        let retained_pointers: Vec<_> = retained
                            .value
                            .components()
                            .iter()
                            .map(|component| component.words().as_ptr())
                            .collect();
                        assert_eq!(retained_pointers, original_pointers);
                        key.preflight_evaluation(&profile, &retained, &original_shape, &elements)
                            .unwrap();
                        drop(retained);
                        product
                    }
                };
                assert_eq!(
                    value.ciphertext().canonical_words(),
                    expected.canonical_words()
                );
                assert_eq!(value.shape(), expected.shape());
                key.preflight_evaluation(&profile, &value, &expected.shape(), &elements)
                    .unwrap();
                let want: Vec<_> = if operation == 2 {
                    conv(
                        &left_values
                            .iter()
                            .map(|&x| BigInt::from(x))
                            .collect::<Vec<_>>(),
                        &right_values
                            .iter()
                            .map(|&x| BigInt::from(x))
                            .collect::<Vec<_>>(),
                    )
                    .iter()
                    .map(|x| small(x, t))
                    .collect()
                } else {
                    left_values
                        .iter()
                        .zip(&right_values)
                        .map(|(&a, &b)| (a + b) % t)
                        .collect()
                };
                let phases = assert_bound(&profile, value.ciphertext(), secret.coefficients());
                assert_eq!(phases.iter().map(|x| small(x, t)).collect::<Vec<_>>(), want);
                assert_eq!(
                    secret
                        .decrypt(&profile, value.ciphertext())
                        .unwrap()
                        .coefficients(),
                    want
                );
            }
            assert_eq!(
                secret
                    .decrypt(&profile, value.ciphertext())
                    .unwrap()
                    .coefficients(),
                left_values
            );
        }
        reset();
    }

    #[test]
    fn local_delegate_preflight_refuses_foreign_origin_context_shape_and_missing_keys() {
        reset();
        let profile = make_profile(5);
        let precision = profile.plaintext_precision(1).unwrap();
        let elements = SlotEncoding::new(&profile, precision)
            .unwrap()
            .broadcast_elements();
        let (_, key) = LocalBootstrapKey::generate_with(
            &profile,
            &elements,
            setup_budget(),
            &mut FixtureEntropy::new([221; 64]),
        )
        .unwrap();
        let (_, foreign) = LocalBootstrapKey::generate_with(
            &profile,
            &elements,
            setup_budget(),
            &mut FixtureEntropy::new([222; 64]),
        )
        .unwrap();
        let (_, missing) = LocalBootstrapKey::generate_with(
            &profile,
            &[],
            setup_budget(),
            &mut FixtureEntropy::new([223; 64]),
        )
        .unwrap();
        let plain = Plaintext::new(&profile, precision, vec![1; profile.degree()]).unwrap();
        let wrong_precision = Plaintext::new(
            &profile,
            profile.plaintext_precision(2).unwrap(),
            vec![1; profile.degree()],
        )
        .unwrap();
        let source = || {
            key.encrypt_with(&profile, &plain, &mut FixtureEntropy::new([224; 64]))
                .unwrap()
        };
        let alien = || {
            foreign
                .encrypt_with(&profile, &plain, &mut FixtureEntropy::new([225; 64]))
                .unwrap()
        };
        reset();
        assert!(matches!(
            key.add(&profile, source(), alien()),
            Err(BootstrapError::Origin)
        ));
        assert!(matches!(
            key.add(&profile, alien(), source()),
            Err(BootstrapError::Origin)
        ));
        assert!(matches!(
            key.add_plain(&profile, alien(), &plain),
            Err(BootstrapError::Origin)
        ));
        assert!(matches!(
            key.mul_plain(&profile, alien(), &plain),
            Err(BootstrapError::Origin)
        ));
        assert!(matches!(
            key.add_plain(&profile, source(), &wrong_precision),
            Err(BootstrapError::Bgv(BgvError::PlaintextPrecisionMismatch))
        ));
        assert!(matches!(
            key.mul_plain(&profile, source(), &wrong_precision),
            Err(BootstrapError::Bgv(BgvError::PlaintextPrecisionMismatch))
        ));
        let lowered = key.switch_to(&profile, source(), 1).unwrap();
        assert!(matches!(
            key.add(&profile, source(), lowered),
            Err(BootstrapError::Bgv(BgvError::LevelMismatch { .. }))
        ));
        assert!(matches!(
            key.trivial(&profile, &plain, profile.top_level() + 1),
            Err(BootstrapError::Bgv(BgvError::LevelOutOfRange { .. }))
        ));
        let value = source();
        let before = poly::tests::cleared_words();
        key.preflight_evaluation(&profile, &value, &value.shape(), &elements)
            .unwrap();
        assert!(matches!(
            foreign.preflight_evaluation(&profile, &value, &value.shape(), &elements),
            Err(BootstrapError::Origin)
        ));
        let wrong_shape = profile
            .plan_fresh(profile.plaintext_precision(2).unwrap())
            .unwrap();
        assert!(matches!(
            key.preflight_evaluation(&profile, &value, &wrong_shape, &elements),
            Err(BootstrapError::Shape)
        ));
        let lower_shape = profile.plan_switch_down(&value.shape()).unwrap();
        assert!(matches!(
            key.preflight_evaluation(&profile, &value, &lower_shape, &elements),
            Err(BootstrapError::Shape)
        ));
        assert_eq!(poly::tests::cleared_words(), before);
        let missing_input = missing
            .encrypt_with(&profile, &plain, &mut FixtureEntropy::new([226; 64]))
            .unwrap();
        assert!(matches!(
            missing.preflight_evaluation(
                &profile,
                &missing_input,
                &missing_input.shape(),
                &elements
            ),
            Err(BootstrapError::Bgv(BgvError::WrongSwitchingKey))
        ));
        let other_profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 2, (50, 5), (60, 2), 2).unwrap()).unwrap();
        let other_plain = Plaintext::new(
            &other_profile,
            other_profile.plaintext_precision(1).unwrap(),
            vec![1; 16],
        )
        .unwrap();
        assert!(matches!(
            key.trivial(&other_profile, &other_plain, 2),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert!(matches!(
            key.add_plain(&profile, source(), &other_plain),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert!(matches!(
            key.mul_plain(&profile, source(), &other_plain),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert!(matches!(
            key.preflight_evaluation(&other_profile, &value, &value.shape(), &elements),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert!(matches!(
            key.preflight_evaluation(
                &profile,
                &value,
                &other_profile
                    .plan_fresh(other_profile.plaintext_precision(1).unwrap())
                    .unwrap(),
                &elements
            ),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        assert_eq!(
            ACCESSES.with(Cell::get),
            0,
            "no successful arithmetic checkpoint after any refusal"
        );
        reset();
    }

    #[test]
    fn local_delegates_clear_consumed_sources_and_partial_outputs_on_error_and_unwind() {
        reset();
        let profile = make_profile(5);
        let (_, key) = LocalBootstrapKey::generate_with(
            &profile,
            &[],
            setup_budget(),
            &mut FixtureEntropy::new([231; 64]),
        )
        .unwrap();
        let plain = Plaintext::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            vec![2; 16],
        )
        .unwrap();
        let source = || {
            key.encrypt_with(&profile, &plain, &mut FixtureEntropy::new([232; 64]))
                .unwrap()
        };
        let words = 2 * profile.degree() * (profile.top_level() + 1);
        for operation in 0..4 {
            for mode in [1, 2] {
                reset();
                FAULT.with(|x| x.set((1, mode)));
                let result = catch_unwind(AssertUnwindSafe(|| match operation {
                    0 => key.trivial(&profile, &plain, profile.top_level()),
                    1 => key.add(&profile, source(), source()),
                    2 => key.add_plain(&profile, source(), &plain),
                    _ => key
                        .mul_plain(&profile, source(), &plain)
                        .map(|(input, output)| {
                            drop(input);
                            output
                        }),
                }));
                if mode == 2 {
                    assert!(result.is_err());
                } else {
                    assert!(matches!(result.unwrap(), Err(BootstrapError::Shape)));
                }
                let owners = match operation {
                    0 => 1,
                    1 => 3,
                    _ => 2,
                };
                assert_eq!(
                    poly::tests::cleared_words() - AT_FAULT.with(Cell::get),
                    owners * words
                );
            }
        }
        reset();
        let (input, result) = key.mul_plain(&profile, source(), &plain).unwrap();
        let before = poly::tests::cleared_words();
        drop(input);
        assert_eq!(poly::tests::cleared_words() - before, words);
        drop(result);
        assert_eq!(poly::tests::cleared_words() - before, 2 * words);
        reset();
    }

    #[test]
    fn local_plain_product_refuses_a_genuinely_reached_noise_ceiling() {
        reset();
        let profile = make_profile(5);
        let (_, key) = LocalBootstrapKey::generate_with(
            &profile,
            &[],
            setup_budget(),
            &mut FixtureEntropy::new([241; 64]),
        )
        .unwrap();
        let plain = Plaintext::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            vec![2; 16],
        )
        .unwrap();
        let input = key
            .encrypt_with(&profile, &plain, &mut FixtureEntropy::new([242; 64]))
            .unwrap();
        let mut value = key.switch_to(&profile, input, 0).unwrap();
        let mut admitted = 0;
        while profile
            .plan_mul_plain(&value.shape(), plain.bound())
            .is_ok()
        {
            assert!(admitted < 16, "finite public level-zero geometry");
            let (previous, next) = key.mul_plain(&profile, value, &plain).unwrap();
            drop(previous);
            value = next;
            admitted += 1;
        }
        assert!(admitted > 0);
        reset();
        let before = poly::tests::cleared_words();
        assert!(matches!(
            key.mul_plain(&profile, value, &plain),
            Err(BootstrapError::Bgv(BgvError::NoiseBudgetExceeded {
                level: 0
            }))
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        assert_eq!(poly::tests::cleared_words() - before, 2 * profile.degree());
        reset();
    }
    #[test]
    fn typed_preparation_metadata_covers_real_complete_plan_owners() {
        for primes in [5, 6] {
            let profile = make_profile(primes);
            let precision = profile.plaintext_precision(1).unwrap();
            let program = program();
            let program_plan = program.plan();
            let requirement =
                BootstrapPlan::metadata_requirement(&profile, precision, &program_plan).unwrap();
            let source = starting_shape(&profile, 2);
            let plan = make_plan(&profile, &source);
            let observed = plan.retained_metadata().unwrap();
            assert_eq!(observed.inline_bytes, size_of::<BootstrapPlan>());
            assert!(observed.retained_heap_bytes <= requirement.retained_heap_bytes);
            assert_eq!(observed.preparation_scratch_bytes, 0);
            assert!(
                requirement.preparation_scratch_bytes
                    >= program_plan.geometry().preparation_scratch_bytes
                        + program_plan.preparation_owner_bytes()
            );
            assert_eq!(
                requirement.repeated(21).unwrap().retained_bytes().unwrap(),
                21 * requirement.retained_bytes().unwrap()
            );
            assert_eq!(
                requirement.repeated(21).unwrap().preparation_scratch_bytes,
                requirement.preparation_scratch_bytes
            );
            // Each actual trace bank shape and every outer bound allocation
            // contributes. This count is independent of private ciphertexts.
            assert!(
                observed.retained_heap_bytes >= profile.degree() / 2 * size_of::<CiphertextShape>()
            );
            assert!(
                BootstrapPlan::metadata_requirement(
                    &profile,
                    profile.master_plaintext(),
                    &program_plan
                )
                .is_err()
            );
            let other = make_profile(primes + 1);
            assert!(matches!(
                BootstrapPlan::metadata_requirement(
                    &profile,
                    other.plaintext_precision(1).unwrap(),
                    &program_plan
                ),
                Err(super::super::PlanMetadataError::Bgv(
                    BgvError::ProfileMismatch
                ))
            ));
            let shape = source.retained_metadata().unwrap();
            assert!(
                shape.retained_heap_bytes
                    <= CiphertextShape::metadata_requirement(&profile)
                        .unwrap()
                        .retained_heap_bytes
            );
        }
    }

    fn exact_shared_budget(plan: &BootstrapPlan) -> BootstrapBudget {
        let mut exact = budget();
        let stage = |payload: super::super::CoefficientSlotPayload| CoefficientSlotBudget {
            routing: payload.routing,
            owned_residue_bytes: payload.owned_residue_bytes,
            trace_plaintext_bytes: payload.trace_plaintext_bytes,
        };
        exact.decomposition = stage(plan.preparation.decomposition.payload());
        exact.recomposition = stage(plan.preparation.recomposition.payload());
        exact.upper_digits = plan.preparation.digits.required_budget();
        let payload = plan.payload();
        exact.canonical_arithmetic_units = payload.canonical_arithmetic_units;
        exact.scalar_word_operations = payload.scalar_word_operations;
        exact.stream_owned_residue_bytes = payload.stream_owned_residue_bytes;
        exact.stream_metadata_bytes = payload.stream_metadata_bytes;
        exact.retained_preparation_bytes = payload.retained_preparation_bytes;
        exact.input_owned_bytes = payload.input_owned_bytes;
        exact.input_plaintext_bytes = payload.input_plaintext_bytes;
        exact.input_arithmetic_units = payload.input_arithmetic_units;
        exact.input_addition_words = payload.input_addition_words;
        exact
    }

    #[test]
    fn shared_preparation_preserves_exact_source_and_complete_arithmetic() {
        reset();
        let profile = make_profile(5);
        let first = make_plan(&profile, &starting_shape(&profile, 1));
        let (secret, key) = setup(&profile, &first, 181);
        for level in [1, 2] {
            let shape = starting_shape(&profile, level);
            let shared = first.admit_source(&profile, &shape, budget()).unwrap();
            let independent = make_plan(&profile, &shape);
            assert!(first.shares_preparation(&shared));
            assert!(!first.shares_preparation(&independent));
            assert_eq!(shared.input_shape(), &shape);
            assert_eq!(
                shared.homomorphic_input_shape(),
                independent.homomorphic_input_shape()
            );
            assert_eq!(shared.output_shape(), independent.output_shape());
            assert_eq!(shared.payload(), independent.payload());
            assert_eq!(shared.galois_elements(), independent.galois_elements());
            let a = source(&profile, &key, level, 182 + level as u8);
            let b = source(&profile, &key, level, 182 + level as u8);
            assert_eq!(
                a.ciphertext().canonical_words(),
                b.ciphertext().canonical_words()
            );
            install(&secret);
            let a = key.refresh(&profile, &shared, a).unwrap();
            let b = key.refresh(&profile, &independent, b).unwrap();
            assert_eq!(
                a.ciphertext().canonical_words(),
                b.ciphertext().canonical_words()
            );
            assert_eq!(
                a.ciphertext().to_bytes(&profile).unwrap(),
                b.ciphertext().to_bytes(&profile).unwrap()
            );
            assert_eq!(a.shape(), *shared.output_shape());
            assert_eq!(
                secret
                    .decrypt(&profile, a.ciphertext())
                    .unwrap()
                    .coefficients(),
                secret
                    .decrypt(&profile, b.ciphertext())
                    .unwrap()
                    .coefficients()
            );
            OBSERVER.with(|cell| {
                assert_eq!(
                    cell.borrow().as_ref().unwrap().seen,
                    [1, profile.degree() / 2, profile.degree() / 2, 1]
                )
            });
            reset();
        }
        let shared = first
            .admit_source(&profile, &starting_shape(&profile, 2), budget())
            .unwrap();
        ACCESSES.with(|v| v.set(0));
        assert!(matches!(
            key.refresh(&profile, &shared, source(&profile, &key, 1, 185)),
            Err(BootstrapError::Shape)
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        let (_, foreign) = setup(&profile, &first, 186);
        assert!(matches!(
            key.refresh(&profile, &shared, source(&profile, &foreign, 2, 187)),
            Err(BootstrapError::Origin)
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        reset();
    }

    #[test]
    fn reused_source_readmits_every_selected_stage_and_per_call_ceiling() {
        reset();
        let profile = make_profile(5);
        let shape = starting_shape(&profile, 1);
        let plan = make_plan(&profile, &shape);
        let exact = exact_shared_budget(&plan);
        let same = plan.admit_source(&profile, &shape, exact).unwrap();
        assert!(plan.shares_preparation(&same));
        assert_eq!(same.payload(), plan.payload());
        drop(same);
        for field in 0..30 {
            let mut bad = exact;
            let expected = match field {
                0 => {
                    bad.retained_preparation_bytes -= 1;
                    "retained preparation"
                }
                1 => {
                    bad.input_owned_bytes -= 1;
                    "input owners"
                }
                2 => {
                    bad.input_plaintext_bytes -= 1;
                    "input plaintext"
                }
                3 => {
                    bad.input_arithmetic_units -= 1;
                    "input arithmetic"
                }
                4 => {
                    bad.input_addition_words -= 1;
                    "input addition"
                }
                5 => {
                    bad.stream_owned_residue_bytes -= 1;
                    "stream residues"
                }
                6 => {
                    bad.stream_metadata_bytes -= 1;
                    "stream metadata"
                }
                7 => {
                    bad.canonical_arithmetic_units -= 1;
                    "complete arithmetic"
                }
                8 => {
                    bad.scalar_word_operations -= 1;
                    "complete scalar words"
                }
                9..=22 => {
                    let stage = if field < 16 {
                        &mut bad.decomposition
                    } else {
                        &mut bad.recomposition
                    };
                    match (field - 9) % 7 {
                        0 => stage.routing.coefficient_bytes -= 1,
                        1 => stage.routing.layout_metadata_bytes -= 1,
                        2 => stage.routing.preparation_scratch_bytes -= 1,
                        3 => stage.routing.output_residue_bytes -= 1,
                        4 => stage.routing.scratch_residue_bytes -= 1,
                        5 => stage.owned_residue_bytes -= 1,
                        _ => stage.trace_plaintext_bytes -= 1,
                    }
                    if field < 16 {
                        "decomposition stage"
                    } else {
                        "recomposition stage"
                    }
                }
                23 => {
                    bad.upper_digits.compiler_steps -= 1;
                    "upper compiler"
                }
                24 => {
                    bad.upper_digits.metadata_bytes -= 1;
                    "upper stage"
                }
                25 => {
                    bad.upper_digits.owned_residue_bytes -= 1;
                    "upper stage"
                }
                26 => {
                    bad.upper_digits.plaintext_bytes -= 1;
                    "upper stage"
                }
                27 => {
                    bad.upper_digits.backend_scratch_bytes -= 1;
                    "upper stage"
                }
                28 => {
                    bad.upper_digits.canonical_arithmetic_units -= 1;
                    "upper preallocation arithmetic"
                }
                _ => {
                    bad.upper_digits.scalar_word_operations -= 1;
                    "upper preallocation scalar words"
                }
            };
            assert!(
                matches!(plan.admit_source(&profile, &shape, bad),
                Err(BootstrapError::Budget { resource }) if resource == expected),
                "field{field}"
            );
            assert_eq!(Arc::strong_count(&plan.preparation), 1);
        }
        let q = profile.level_modulus(1).unwrap();
        let threshold = q
            .mul_u64(25 - profile.degree() as u64 - 1)
            .div_floor_u64(50)
            .unwrap();
        let admitted = profile
            .plan_claimed(
                profile.plaintext_precision(1).unwrap(),
                1,
                2,
                threshold.clone(),
            )
            .unwrap();
        assert!(plan.admit_source(&profile, &admitted, budget()).is_ok());
        let refused = profile
            .plan_claimed(
                profile.plaintext_precision(1).unwrap(),
                1,
                2,
                threshold.add_u64(1),
            )
            .unwrap();
        assert!(matches!(
            plan.admit_source(&profile, &refused, budget()),
            Err(BootstrapError::RoundingBound)
        ));
        let other = make_profile(6);
        assert!(matches!(
            plan.admit_source(&other, &starting_shape(&other, 1), budget()),
            Err(BootstrapError::Bgv(BgvError::ProfileMismatch))
        ));
        let k2 = profile
            .plan_fresh(profile.plaintext_precision(2).unwrap())
            .unwrap();
        assert!(matches!(
            plan.admit_source(&profile, &k2, budget()),
            Err(BootstrapError::Shape)
        ));
        let fresh = profile
            .plan_fresh(profile.plaintext_precision(1).unwrap())
            .unwrap();
        let tensor = profile.plan_tensor(&fresh, &fresh).unwrap();
        assert!(matches!(
            plan.admit_source(&profile, &tensor, budget()),
            Err(BootstrapError::Shape)
        ));
        assert_eq!(ACCESSES.with(Cell::get), 0);
        assert_eq!(Arc::strong_count(&plan.preparation), 1);
    }

    #[test]
    fn split_metadata_charges_one_actual_core_and_every_exact_source_descriptor() {
        let profile = make_profile(5);
        let source = starting_shape(&profile, 1);
        let plan = make_plan(&profile, &source);
        let requirement = BootstrapPlan::split_metadata_requirement(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &program().plan(),
        )
        .unwrap();
        let observed = plan.retained_metadata_split().unwrap();
        assert_eq!(
            requirement.standalone().unwrap(),
            BootstrapPlan::metadata_requirement(
                &profile,
                profile.plaintext_precision(1).unwrap(),
                &program().plan()
            )
            .unwrap()
        );
        assert_eq!(
            observed.standalone().unwrap(),
            plan.retained_metadata().unwrap()
        );
        assert!(
            observed.shared_preparation.retained_bytes().unwrap()
                <= requirement.shared_preparation.retained_bytes().unwrap()
        );
        assert!(
            observed.source_descriptor.retained_bytes().unwrap()
                <= requirement.source_descriptor.retained_bytes().unwrap()
        );
        assert_eq!(
            observed.source_descriptor.inline_bytes,
            size_of::<BootstrapPlan>()
        );
        assert_eq!(
            observed.shared_preparation.inline_bytes,
            size_of::<BootstrapPreparation>() + 2 * size_of::<usize>()
        );
        assert_eq!(
            plan.shared_preparation_bytes(),
            plan.payload().retained_preparation_bytes
        );
        let plans: Vec<_> = (0..449)
            .map(|i| {
                let shape = profile
                    .plan_claimed(
                        profile.plaintext_precision(1).unwrap(),
                        1,
                        2,
                        source.bound().add_u64(i),
                    )
                    .unwrap();
                plan.admit_source(&profile, &shape, budget()).unwrap()
            })
            .collect();
        assert_eq!(Arc::strong_count(&plan.preparation), 450);
        let actual_sources = plans
            .iter()
            .map(|p| {
                assert!(p.shares_preparation(&plan));
                let observed = p.retained_metadata_split().unwrap();
                assert_eq!(
                    observed.shared_preparation,
                    plan.retained_metadata_split().unwrap().shared_preparation
                );
                observed.source_descriptor.retained_bytes().unwrap()
            })
            .sum::<usize>();
        let source_bound = requirement
            .source_descriptor
            .repeated(449)
            .unwrap()
            .retained_bytes()
            .unwrap();
        assert!(actual_sources <= source_bound);
        assert!(requirement.source_descriptor.repeated(usize::MAX).is_err());
        assert!(
            BootstrapMetadata {
                shared_preparation: super::super::PlanMetadataRequirement {
                    inline_bytes: usize::MAX,
                    retained_heap_bytes: 1,
                    preparation_scratch_bytes: 0
                },
                source_descriptor: requirement.source_descriptor,
            }
            .standalone()
            .is_err()
        );
    }

    #[test]
    fn shared_core_survives_refusal_and_retires_only_after_last_descriptor_on_unwind() {
        reset();
        let profile = make_profile(5);
        let plan = make_plan(&profile, &starting_shape(&profile, 1));
        let weak = Arc::downgrade(&plan.preparation);
        let second = plan
            .admit_source(&profile, &starting_shape(&profile, 2), budget())
            .unwrap();
        let (_, key) = setup(&profile, &plan, 191);
        let third = plan
            .admit_source(&profile, &starting_shape(&profile, 1), budget())
            .unwrap();
        drop(plan);
        assert_eq!(weak.strong_count(), 2);
        for mode in [1, 2] {
            let value = source(&profile, &key, 2, 192);
            ACCESSES.with(|v| v.set(0));
            FAULT.with(|v| v.set((5, mode)));
            let result = catch_unwind(AssertUnwindSafe(|| key.refresh(&profile, &second, value)));
            assert!(if mode == 2 {
                result.is_err()
            } else {
                matches!(result, Ok(Err(BootstrapError::Shape)))
            });
            assert!(poly::tests::cleared_words() > AT_FAULT.with(Cell::get));
            assert_eq!(weak.strong_count(), 2);
        }
        reset();
        let result = catch_unwind(AssertUnwindSafe(move || {
            drop(second);
            assert_eq!(Arc::strong_count(&third.preparation), 1);
            let _last = third;
            panic!("last shared preparation owner unwind");
        }));
        assert!(result.is_err());
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
        reset();
    }

    #[test]
    fn full_refresh_geometry_metadata_prebounds_one_core_and_449_sources() {
        let profile =
            BgvProfile::new(BgvParameters::derive(7, 257, 3, (60, 20), (60, 2), 2).unwrap())
                .unwrap();
        let context = PrimePowerContext::new(257, 3).unwrap();
        let geometry = UpperDigitsPlan::required(context, 1).unwrap();
        let program = UpperDigitsPlan::new(
            context,
            1,
            ProgramBudget {
                coefficient_bytes: geometry.coefficient_bytes,
                preparation_scratch_bytes: geometry.preparation_scratch_bytes,
                program_bytes: geometry.program_bytes,
                preparation_operations: geometry.preparation_operations,
                live_values: geometry.live_values,
            },
        )
        .unwrap();
        let split = BootstrapPlan::split_metadata_requirement(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &program,
        )
        .unwrap();
        let bound_bytes = 8 * (2 * profile.noise_bound_words() + 1);
        assert_eq!(bound_bytes, 312);
        assert_eq!(
            split.shared_preparation.retained_heap_bytes,
            13 * bound_bytes + size_of::<CiphertextShape>() + 4 * (2 * 128 + 5 * 7)
        );
        assert_eq!(split.source_descriptor.retained_heap_bytes, bound_bytes);
        let sources = split.source_descriptor.repeated(449).unwrap();
        let retained = split
            .shared_preparation
            .retained_bytes()
            .unwrap()
            .checked_add(sources.retained_bytes().unwrap())
            .unwrap();
        let peak = retained
            .checked_add(
                split
                    .shared_preparation
                    .preparation_scratch_bytes
                    .max(sources.preparation_scratch_bytes),
            )
            .unwrap();
        assert!(peak > retained);
        assert!(
            retained
                < split
                    .standalone()
                    .unwrap()
                    .repeated(449)
                    .unwrap()
                    .retained_bytes()
                    .unwrap()
        );
        eprintln!(
            "R9 bootstrap complementary metadata: shared={:?}, source={:?}, descriptors=449, retained={}, peak={}",
            split.shared_preparation, split.source_descriptor, retained, peak
        );
        // All counts are requested typed payloads, excluding the separately
        // budgeted shared matrices/layout/upper program and caller containers.
    }
}
