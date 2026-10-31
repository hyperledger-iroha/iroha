//! Fixed-public-shape linear maps between packed scalar ciphertexts.
//!
//! Every publicly supported matrix diagonal is evaluated, including private zeros. Public baby-step/giant-
//! step geometry caches baby images and folds giant partials by a binary
//! Horner tree. The split minimizes Galois work within public noise and buffer
//! budgets, with deterministic ties by smaller owned scratch. Dimensions, input
//! shapes, key accesses and arithmetic counts belong to the immutable public plan.
//! Baby residue buffers are transformed once per request; diagonal products are
//! accumulated in evaluation form and only completed giant leaves are inverted.
//! No request-dependent transformed storage survives this call. The owner prepares
//! private matrices outside an observable request. This is arithmetic only:
//! input/key authentication, scalar-subalgebra membership, circuit privacy and
//! any application-specific relation or complete resource budget are external.

use std::{fmt, mem::size_of};

mod layout;
pub use layout::{SlotLinearLayout, SlotLinearSegment};
use thiserror::Error;
use zeroize::{Zeroize as _, Zeroizing};

use super::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, EvaluationKeys, Plaintext, SlotEncoding,
    SwitchingKeyKind, cost::CostModel, poly::RnsPoly,
};

/// Public byte ceilings for the owned buffers of a packed linear map.
///
/// These are exact payload limits, excluding allocator metadata. The caller
/// separately budgets keys, the profile, borrowed inputs, plaintext encoding
/// scratch, transient shared-arithmetic scratch and public plan/shape metadata
/// other than the explicitly charged shared layout. These are not a claim
/// about the complete working set or an application admission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotLinearBudget {
    /// Maximum bytes of owned matrix and bias coefficients (`u32` scalars).
    pub coefficient_bytes: usize,
    /// Maximum resident payload of the immutable shared public layout.
    pub layout_metadata_bytes: usize,
    /// Maximum private row-scratch payload used only during preparation.
    pub preparation_scratch_bytes: usize,
    /// Maximum residue bytes of the retained output ciphertext accumulators.
    pub output_residue_bytes: usize,
    /// Maximum cached/partial/temporary ciphertext residue bytes, excluding outputs.
    pub scratch_residue_bytes: usize,
}

/// A public geometry, private-coefficient or BGV refusal of a packed linear map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SlotLinearError {
    /// At least one input and one output bank are required.
    #[error("packed linear maps require nonempty public bank dimensions")]
    EmptyDimensions,
    /// The public dimensions cannot be represented without overflow.
    #[error("packed linear map geometry overflows")]
    GeometryOverflow,
    /// The exact coefficient owner exceeds its public byte ceiling.
    #[error("packed coefficient buffer requires {required} bytes, budget {budget}")]
    CoefficientBudget {
        /// Required payload bytes.
        required: usize,
        /// Caller-supplied public ceiling.
        budget: usize,
    },
    /// The public row descriptor has invalid ordering, overlap or geometry.
    #[error("invalid packed public row support")]
    InvalidLayout,
    /// The supplied map and plan do not have the same public row support.
    #[error("packed public row support differs between map and plan")]
    LayoutMismatch,
    /// Public layout payload exceeds the explicit metadata ceiling.
    #[error("packed layout requires {required} metadata bytes, budget {budget}")]
    LayoutBudget {
        /// Required resident public payload bytes.
        required: usize,
        /// Caller-supplied ceiling.
        budget: usize,
    },
    /// The clearing preparation row exceeds its separate private byte ceiling.
    #[error("packed preparation row requires {required} bytes, budget {budget}")]
    PreparationBudget {
        /// Required private scratch bytes.
        required: usize,
        /// Caller-supplied ceiling.
        budget: usize,
    },
    /// Private preparation attempted a nonzero structural-zero coefficient.
    #[error("nonzero coefficient outside public support at row {row}, column {column}")]
    OutsideSupport {
        /// Public logical row.
        row: usize,
        /// Public logical column.
        column: usize,
    },
    /// Retained output residues exceed their public byte ceiling.
    #[error("packed output buffers require {required} bytes, budget {budget}")]
    OutputBudget {
        /// Required payload bytes.
        required: usize,
        /// Caller-supplied public ceiling.
        budget: usize,
    },
    /// No group split fits the public owned-ciphertext scratch ceiling.
    #[error("packed routing scratch requires at least {required} bytes, budget {budget}")]
    ScratchBudget {
        /// Smallest scratch payload of any public split.
        required: usize,
        /// Caller-supplied public ceiling.
        budget: usize,
    },
    /// Publicly sized storage could not be allocated.
    #[error("packed linear map allocation failed")]
    Allocation,
    /// An input does not have its exact publicly planned shape.
    #[error("packed linear input shape differs at bank {bank}")]
    InputShape {
        /// Public input-bank position.
        bank: usize,
    },
    /// An existing BGV validation, encoding or arithmetic check failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Immutable public plan for `INPUTS` input banks and `OUTPUTS` output banks.
///
/// Const dimensions alone are not memory limits: construction also requires
/// explicit public byte budgets, checked before private allocation. Inputs
/// must have two components at one common public level. No modulus switching
/// occurs inside this operation. Each output has its own exact public bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotLinearPlan<const INPUTS: usize, const OUTPUTS: usize> {
    identity: [u8; 32],
    inputs: [CiphertextShape; INPUTS],
    outputs: [CiphertextShape; OUTPUTS],
    nonempty_leaves: usize,
    encoding: SlotEncoding,
    layout: SlotLinearLayout<INPUTS, OUTPUTS>,
    preparation_scratch_bytes: usize,
    generators: Vec<u32>,
    baby_bits: usize,
    baby_elements: Vec<u32>,
    scratch_residue_bytes: usize,
    coefficient_count: usize,
    coefficient_bytes: usize,
    output_residue_bytes: usize,
    galois_applications: usize,
    plaintext_products: usize,
}

impl<const INPUTS: usize, const OUTPUTS: usize> SlotLinearPlan<INPUTS, OUTPUTS> {
    /// Allocation-free typed metadata bound for every admissible public split.
    ///
    /// Shared layout graphs and coefficient/row payloads retain their own budgets.
    /// Scratch includes a retained best split, current candidate, maximum baby
    /// shape cache, Horner owners and failed arithmetic transfers. This is a
    /// requested-payload bound, not allocator/RSS or consensus admission policy.
    ///
    /// # Errors
    /// Refuses incompatible profile/encoding or checked dimension/byte overflow.
    pub fn metadata_requirement(
        profile: &BgvProfile,
        precision: super::PlaintextPrecision,
    ) -> Result<super::PlanMetadataRequirement, super::PlanMetadataError> {
        use super::metadata::{add, mul, shape_heap_bound, shape_value_bound, sum};
        let (slots, _, bits, _) = super::metadata::geometry(profile, precision)?;
        // This validates the complete logical rectangle without instantiating
        // arrays, allocating layouts, or assuming packed sparsity bounds it.
        if INPUTS == 0 || OUTPUTS == 0 {
            return Err(super::PlanMetadataError::Context);
        }
        let rows = mul(OUTPUTS, slots)?;
        let columns = add(mul(INPUTS, slots)?, 1)?;
        mul(rows, columns)?;
        let heap = sum(&[
            mul(add(INPUTS, OUTPUTS)?, shape_heap_bound(profile)?)?,
            mul(add(bits, slots)?, size_of::<u32>())?,
        ])?;
        let candidate = add(
            size_of::<SplitTransfer<OUTPUTS>>(),
            mul(OUTPUTS, shape_heap_bound(profile)?)?,
        )?;
        let best = add(
            size_of::<Option<(usize, usize, usize, SplitTransfer<OUTPUTS>)>>(),
            mul(OUTPUTS, shape_heap_bound(profile)?)?,
        )?;
        let cache = add(
            size_of::<Vec<CiphertextShape>>(),
            mul(mul(INPUTS, slots)?, shape_value_bound(profile)?)?,
        )?;
        // At most `bits` retained left siblings; leaf partial/product/result,
        // outer folded/bias results add five more owning shape values.
        let fold = mul(
            add(bits, 5)?,
            add(
                size_of::<Option<CiphertextShape>>(),
                shape_heap_bound(profile)?,
            )?,
        )?;
        let scratch = sum(&[
            candidate,
            best,
            cache,
            fold,
            size_of::<Vec<u32>>(),
            mul(bits, size_of::<u32>())?,
            super::metadata::transfer_scratch(profile)?,
            super::metadata::encoding_scratch(slots)?,
        ])?;
        Ok(super::PlanMetadataRequirement {
            inline_bytes: size_of::<Self>(),
            retained_heap_bytes: heap,
            preparation_scratch_bytes: scratch,
        })
    }

    /// Inspect retained public vector/bound capacities without counting shared layouts.
    ///
    /// An enclosing admission must compare this with its prebound and refuse any
    /// excess before private preparation/execution, rather than change arithmetic.
    ///
    /// # Errors
    /// Refuses byte overflow. Allocator over-reservation is visible here.
    pub fn retained_metadata(
        &self,
    ) -> Result<super::PlanMetadataRequirement, super::PlanMetadataError> {
        use super::metadata::{add, mul};
        let mut heap = mul(
            add(self.generators.capacity(), self.baby_elements.capacity())?,
            size_of::<u32>(),
        )?;
        for shape in self.inputs.iter().chain(&self.outputs) {
            heap = add(heap, super::metadata::shape_heap(shape)?)?;
        }
        Ok(super::PlanMetadataRequirement {
            inline_bytes: size_of::<Self>(),
            retained_heap_bytes: heap,
            preparation_scratch_bytes: 0,
        })
    }

    /// Plan the complete dense transform from public shape and byte budgets.
    ///
    /// # Errors
    /// Rejects empty/overflowing dimensions, exceeded byte budgets, incompatible
    /// input shapes and any intermediate noise refusal. No private data is read.
    pub fn new(
        profile: &BgvProfile,
        inputs: &[CiphertextShape; INPUTS],
        budget: SlotLinearBudget,
    ) -> Result<Self, SlotLinearError> {
        let first = inputs.first().ok_or(SlotLinearError::EmptyDimensions)?;
        first.check_profile(profile)?;
        let slots = SlotEncoding::new(profile, first.plaintext_precision())?.slots();
        let layout = SlotLinearLayout::dense(slots, budget.layout_metadata_bytes)?;
        Self::with_layout(profile, inputs, layout, budget)
    }

    /// Plan the same complete arithmetic schedule with explicitly public support.
    ///
    /// A diagonal absent from this immutable public support performs no product.
    /// Private zero values never remove work. Every output bank is planned from
    /// its exact remaining products and Horner operations. This plan retains a shared
    /// immutable layout handle, which must match every evaluated map exactly.
    ///
    /// # Errors
    /// Refuses layout/profile mismatches, exceeded public budgets, incompatible
    /// shapes and any intermediate noise refusal before private allocation.
    pub fn with_layout(
        profile: &BgvProfile,
        inputs: &[CiphertextShape; INPUTS],
        layout: SlotLinearLayout<INPUTS, OUTPUTS>,
        budget: SlotLinearBudget,
    ) -> Result<Self, SlotLinearError> {
        let first = inputs.first().ok_or(SlotLinearError::EmptyDimensions)?;
        first.check_profile(profile)?;
        let encoding = SlotEncoding::new(profile, first.plaintext_precision())?;
        let slots = encoding.slots();
        if layout.slots() != slots {
            return Err(SlotLinearError::LayoutMismatch);
        }
        let coefficient_count = layout.coefficient_count();
        let coefficient_bytes = coefficient_count
            .checked_mul(size_of::<u32>())
            .ok_or(SlotLinearError::GeometryOverflow)?;
        if coefficient_bytes > budget.coefficient_bytes {
            return Err(SlotLinearError::CoefficientBudget {
                required: coefficient_bytes,
                budget: budget.coefficient_bytes,
            });
        }
        if layout.metadata_bytes() > budget.layout_metadata_bytes {
            return Err(SlotLinearError::LayoutBudget {
                required: layout.metadata_bytes(),
                budget: budget.layout_metadata_bytes,
            });
        }
        let preparation_scratch_bytes = layout
            .columns()
            .checked_mul(size_of::<u32>())
            .ok_or(SlotLinearError::GeometryOverflow)?;
        if preparation_scratch_bytes > budget.preparation_scratch_bytes {
            return Err(SlotLinearError::PreparationBudget {
                required: preparation_scratch_bytes,
                budget: budget.preparation_scratch_bytes,
            });
        }
        let level = inputs[0].level();
        for input in inputs {
            input.check_profile(profile)?;
            first
                .plaintext_precision()
                .check_same(input.plaintext_precision())?;
            if input.level() != level {
                return Err(BgvError::LevelMismatch {
                    lhs: level,
                    rhs: input.level(),
                }
                .into());
            }
            if input.components() != 2 {
                return Err(BgvError::ComponentCount {
                    found: input.components(),
                }
                .into());
            }
            profile.plan_claimed(input.plaintext_precision(), level, 2, input.bound().clone())?;
        }
        let one_output =
            usize::try_from(CostModel::new(profile).ciphertext_residue_bytes(level, 2)?)
                .map_err(|_| SlotLinearError::GeometryOverflow)?;
        let output_residue_bytes = OUTPUTS
            .checked_mul(one_output)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        if output_residue_bytes > budget.output_residue_bytes {
            return Err(SlotLinearError::OutputBudget {
                required: output_residue_bytes,
                budget: budget.output_residue_bytes,
            });
        }
        let generators = encoding.broadcast_elements();
        let bits = generators.len();
        let minimum_scratch = INPUTS
            .checked_add(bits)
            .and_then(|x| x.checked_add(3))
            .and_then(|x| x.checked_mul(one_output))
            .ok_or(SlotLinearError::GeometryOverflow)?;
        if minimum_scratch > budget.scratch_residue_bytes {
            return Err(SlotLinearError::ScratchBudget {
                required: minimum_scratch,
                budget: budget.scratch_residue_bytes,
            });
        }
        let mut best: Option<(usize, usize, usize, SplitTransfer<OUTPUTS>)> = None;
        let mut noise_error = None;
        for baby_bits in 0..=bits {
            let babies = 1_usize << baby_bits;
            // A transformed leaf retains its partial and current product; its
            // one-polynomial plaintext multiplier fits the third ciphertext
            // allowance. Horner addition still needs three local ciphertexts.
            // Every retained giant-left ancestor adds one further buffer.
            let scratch = INPUTS
                .checked_mul(babies)
                .and_then(|x| x.checked_add(bits - baby_bits + 3))
                .and_then(|x| x.checked_mul(one_output))
                .ok_or(SlotLinearError::GeometryOverflow)?;
            if scratch > budget.scratch_residue_bytes {
                continue;
            }
            let transfer = match plan_split(profile, &encoding, inputs, &layout, baby_bits) {
                Ok(transfer) => transfer,
                Err(error @ BgvError::NoiseBudgetExceeded { .. }) => {
                    noise_error = Some(error);
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let galois = transfer.galois;
            let rank = (galois, scratch, baby_bits);
            if best.as_ref().is_none_or(|(g, s, b, _)| rank < (*g, *s, *b)) {
                best = Some((galois, scratch, baby_bits, transfer));
            }
        }
        let (galois_applications, scratch_residue_bytes, baby_bits, transfer) = best.ok_or(
            SlotLinearError::Bgv(noise_error.unwrap_or(BgvError::Kernel)),
        )?;
        let baby_elements = (0..1_usize << baby_bits)
            .map(|mask| group_element(profile, &generators[..baby_bits], mask))
            .collect();
        Ok(Self {
            identity: *profile.identity(),
            inputs: inputs.clone(),
            outputs: transfer.outputs,
            nonempty_leaves: transfer.leaves,
            encoding,
            layout,
            preparation_scratch_bytes,
            generators,
            baby_bits,
            baby_elements,
            scratch_residue_bytes,
            coefficient_count,
            coefficient_bytes,
            output_residue_bytes,
            galois_applications,
            plaintext_products: transfer.products,
        })
    }

    /// Number of packed scalar slots per input and output bank.
    #[must_use]
    pub const fn slots(&self) -> usize {
        self.encoding.slots()
    }
    /// Shared immutable public support, independent of all coefficient values.
    #[must_use]
    pub const fn layout(&self) -> &SlotLinearLayout<INPUTS, OUTPUTS> {
        &self.layout
    }
    /// Exact resident public layout payload, shared by clones of this layout.
    #[must_use]
    pub fn layout_metadata_bytes(&self) -> usize {
        self.layout.metadata_bytes()
    }
    /// Exact private preparation row payload, excluding the retained matrix.
    #[must_use]
    pub const fn preparation_scratch_bytes(&self) -> usize {
        self.preparation_scratch_bytes
    }
    /// Effective prime-power modulus of every canonical scalar coefficient.
    #[must_use]
    pub fn plaintext_modulus(&self) -> u64 {
        2 * self.encoding.vector_bound().linf + 1
    }
    /// Number of canonical `u32` coefficients, including the final bias vectors.
    #[must_use]
    pub const fn coefficient_count(&self) -> usize {
        self.coefficient_count
    }
    /// Exact retained coefficient payload bytes.
    #[must_use]
    pub const fn coefficient_bytes(&self) -> usize {
        self.coefficient_bytes
    }
    /// Exact retained output residue bytes, excluding frames and allocator metadata.
    #[must_use]
    pub const fn output_residue_bytes(&self) -> usize {
        self.output_residue_bytes
    }
    /// Bound of cached, partial and temporary ciphertext residue payloads.
    #[must_use]
    pub const fn scratch_residue_bytes(&self) -> usize {
        self.scratch_residue_bytes
    }
    /// Public number of cached baby images per input bank.
    #[must_use]
    pub const fn baby_count(&self) -> usize {
        1_usize << self.baby_bits
    }
    /// Public number of giant partials per output bank.
    #[must_use]
    pub fn giant_count(&self) -> usize {
        self.encoding.slots() / self.baby_count()
    }
    /// Exact output shapes in bank order, independent of all private values.
    #[must_use]
    pub const fn output_shapes(&self) -> &[CiphertextShape; OUTPUTS] {
        &self.outputs
    }
    /// Fixed Galois keys used by baby images and the giant Horner tree.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        &self.generators
    }
    /// Total Galois applications; baby images are shared across output banks.
    #[must_use]
    pub const fn galois_applications(&self) -> usize {
        self.galois_applications
    }
    /// Active plaintext products and ciphertext additions; both have this public
    /// count. Each nonempty output adds its bias once; an empty output returns
    /// its bias embedding without addition.
    #[must_use]
    pub const fn plaintext_products(&self) -> usize {
        self.plaintext_products
    }

    /// Deterministic word-multiplication units of this routing schedule.
    ///
    /// Includes one forward transform per cached component, one per diagonal
    /// multiplier, two inverses per giant leaf, all pointwise products and
    /// Galois key switches. Uses the existing cost model's units; additions,
    /// encoding plaintext slot vectors, allocation, clearing and copies are
    /// excluded. This is an arithmetic count, not a measured runtime or a
    /// complete application work budget.
    ///
    /// # Errors
    /// Refuses a different profile or checked arithmetic overflow.
    pub fn arithmetic_units(&self, profile: &BgvProfile) -> Result<u64, SlotLinearError> {
        if profile.identity() != &self.identity {
            return Err(BgvError::ProfileMismatch.into());
        }
        let word = |value| u64::try_from(value).map_err(|_| SlotLinearError::GeometryOverflow);
        let products = word(self.plaintext_products)?;
        let babies = word(INPUTS)?
            .checked_mul(word(self.baby_count())?)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let leaves = word(self.nonempty_leaves)?;
        let forwards = babies
            .checked_mul(2)
            .and_then(|x| x.checked_add(products))
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let inverses = leaves
            .checked_mul(2)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let level = self.inputs[0].level();
        let model = CostModel::new(profile);
        let forward = forwards.checked_mul(model.forward_transform_units());
        let inverse = inverses.checked_mul(model.inverse_transform_units());
        let pointwise = products
            .checked_mul(2)
            .and_then(|x| x.checked_mul(profile.degree() as u64));
        let linear = forward
            .zip(inverse)
            .zip(pointwise)
            .and_then(|((f, i), p)| f.checked_add(i)?.checked_add(p))
            .and_then(|x| x.checked_mul(level as u64 + 1))
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let switching = word(self.galois_applications)?
            .checked_mul(model.automorphism_units(level)?)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        linear
            .checked_add(switching)
            .ok_or(SlotLinearError::GeometryOverflow)
    }

    fn check_inputs(
        &self,
        profile: &BgvProfile,
        inputs: &[Ciphertext; INPUTS],
        keys: &EvaluationKeys,
    ) -> Result<(), SlotLinearError> {
        if profile.identity() != &self.identity {
            return Err(BgvError::ProfileMismatch.into());
        }
        for (bank, (input, shape)) in inputs.iter().zip(&self.inputs).enumerate() {
            if input.profile_identity() != &self.identity {
                return Err(BgvError::ProfileMismatch.into());
            }
            if &input.shape() != shape {
                return Err(SlotLinearError::InputShape { bank });
            }
        }
        for &element in &self.generators {
            let key = keys.galois(element).ok_or(BgvError::WrongSwitchingKey)?;
            if key.profile_identity() != &self.identity {
                return Err(BgvError::ProfileMismatch.into());
            }
            if key.kind() != SwitchingKeyKind::Galois(element) {
                return Err(BgvError::WrongSwitchingKey.into());
            }
        }
        Ok(())
    }
}

/// Compose a public subset of commuting odd-unit generators modulo2N.
fn group_element(profile: &BgvProfile, generators: &[u32], mask: usize) -> u32 {
    let order = 2 * profile.degree() as u64;
    let mut element = 1_u64;
    for (bit, &generator) in generators.iter().enumerate() {
        if (mask >> bit) & 1 != 0 {
            element = element * u64::from(generator) % order;
        }
    }
    element as u32
}

struct SplitTransfer<const OUTPUTS: usize> {
    outputs: [CiphertextShape; OUTPUTS],
    products: usize,
    leaves: usize,
    galois: usize,
}

fn plan_split<const INPUTS: usize, const OUTPUTS: usize>(
    profile: &BgvProfile,
    encoding: &SlotEncoding,
    inputs: &[CiphertextShape; INPUTS],
    layout: &SlotLinearLayout<INPUTS, OUTPUTS>,
    baby_bits: usize,
) -> Result<SplitTransfer<OUTPUTS>, BgvError> {
    let generators = encoding.broadcast_elements();
    let babies = 1_usize << baby_bits;
    // The entire public baby cache is retained, including ancestor images that
    // only structural-zero diagonals might reference. Every cache shape is
    // admitted even when an output has no products.
    let mut images = Vec::with_capacity(INPUTS * babies);
    for input in inputs {
        let start = images.len();
        images.push(input.clone());
        for bit in 0..baby_bits {
            for at in 0..1_usize << bit {
                images.push(profile.plan_galois(&images[start + at])?);
            }
        }
    }
    let mut transfer = SplitTransfer {
        outputs: std::array::from_fn(|_| inputs[0].clone()),
        products: 0,
        leaves: 0,
        galois: INPUTS * (babies - 1),
    };
    for output in 0..OUTPUTS {
        let folded = plan_fold(
            profile,
            encoding,
            layout,
            &generators,
            baby_bits,
            &images,
            output,
            0,
            1,
            &mut transfer,
        )?;
        let bias = profile.plan_trivial(encoding.plaintext_precision(), inputs[0].level())?;
        transfer.outputs[output] = match folded {
            Some(value) => profile.plan_add(&bias, &value)?,
            None => bias,
        };
    }
    Ok(transfer)
}

#[allow(clippy::too_many_arguments)]
fn plan_fold<const INPUTS: usize, const OUTPUTS: usize>(
    profile: &BgvProfile,
    encoding: &SlotEncoding,
    layout: &SlotLinearLayout<INPUTS, OUTPUTS>,
    generators: &[u32],
    baby_bits: usize,
    images: &[CiphertextShape],
    output: usize,
    step: usize,
    giant: u32,
    transfer: &mut SplitTransfer<OUTPUTS>,
) -> Result<Option<CiphertextShape>, BgvError> {
    let giant_keys = &generators[baby_bits..];
    if step == giant_keys.len() {
        let babies = 1_usize << baby_bits;
        let mut partial = None;
        for input in 0..INPUTS {
            for baby in 0..babies {
                let element = group_element(profile, &generators[..baby_bits], baby);
                let diagonal =
                    (u64::from(giant) * u64::from(element) % (2 * profile.degree()) as u64) as u32;
                if !layout.diagonal_active(output, input, diagonal) {
                    continue;
                }
                let product = profile
                    .plan_mul_plain(&images[input * babies + baby], encoding.vector_bound())?;
                transfer.products += 1;
                partial = Some(match partial {
                    None => product,
                    Some(sum) => profile.plan_add(&sum, &product)?,
                });
            }
        }
        if partial.is_some() {
            transfer.leaves += 1;
        }
        return Ok(partial);
    }
    let left = plan_fold(
        profile,
        encoding,
        layout,
        generators,
        baby_bits,
        images,
        output,
        step + 1,
        giant,
        transfer,
    )?;
    let generator = giant_keys[step];
    let composed = (u64::from(giant) * u64::from(generator) % (2 * profile.degree()) as u64) as u32;
    let right = plan_fold(
        profile,
        encoding,
        layout,
        generators,
        baby_bits,
        images,
        output,
        step + 1,
        composed,
        transfer,
    )?;
    let Some(right) = right else {
        return Ok(left);
    };
    let image = profile.plan_galois(&right)?;
    transfer.galois += 1;
    Ok(Some(match left {
        None => image,
        Some(left) => profile.plan_add(&left, &image)?,
    }))
}

/// A private two-component evaluation-domain owner. It is never serialized or
/// exposed as a `Ciphertext`. Each polynomial retains the original clearing
/// allocation. Only a completed leaf is transformed back to coefficient form.
struct NttCiphertext {
    identity: [u8; 32],
    shape: CiphertextShape,
    components: [RnsPoly; 2],
}

impl NttCiphertext {
    fn from_ciphertext(profile: &BgvProfile, ciphertext: Ciphertext) -> Result<Self, BgvError> {
        ciphertext.check_profile(profile)?;
        if ciphertext.component_count() != 2 {
            return Err(BgvError::ComponentCount {
                found: ciphertext.component_count(),
            });
        }
        let shape = ciphertext.shape();
        let mut components: [RnsPoly; 2] =
            ciphertext
                .into_components()
                .try_into()
                .map_err(|components: Vec<RnsPoly>| BgvError::ComponentCount {
                    found: components.len(),
                })?;
        for component in &mut components {
            component.forward(|limb| profile.main_plan(limb))?;
        }
        #[cfg(test)]
        tests::observe_transforms(2 * (shape.level() + 1), 0);
        Ok(Self {
            identity: *profile.identity(),
            shape,
            components,
        })
    }

    fn multiply_plain(
        &self,
        profile: &BgvProfile,
        plaintext: &Plaintext,
    ) -> Result<Self, BgvError> {
        if &self.identity != profile.identity() {
            return Err(BgvError::ProfileMismatch);
        }
        plaintext.check_profile(profile)?;
        self.shape
            .plaintext_precision()
            .check_same(plaintext.plaintext_precision())?;
        let shape = profile.plan_mul_plain(&self.shape, plaintext.bound())?;
        let plan_of = |limb| profile.main_plan(limb);
        let mut multiplier = plaintext.centered_residues(profile, shape.level() + 1);
        multiplier.forward(plan_of)?;
        #[cfg(test)]
        tests::observe_transforms(shape.level() + 1, 0);
        let mut components = self.components.clone();
        for component in &mut components {
            component.pointwise_mul_assign(&multiplier, plan_of)?;
        }
        Ok(Self {
            identity: self.identity,
            shape,
            components,
        })
    }

    fn add_assign(&mut self, profile: &BgvProfile, other: &Self) -> Result<(), BgvError> {
        if &self.identity != profile.identity() || other.identity != self.identity {
            return Err(BgvError::ProfileMismatch);
        }
        let shape = profile.plan_add(&self.shape, &other.shape)?;
        for (left, right) in self.components.iter_mut().zip(&other.components) {
            left.add_assign(right, |limb| profile.main_plan(limb))?;
        }
        self.shape = shape;
        #[cfg(test)]
        tests::observe_addition();
        Ok(())
    }

    fn into_ciphertext(self, profile: &BgvProfile) -> Result<Ciphertext, BgvError> {
        if &self.identity != profile.identity() {
            return Err(BgvError::ProfileMismatch);
        }
        let Self {
            identity: _,
            shape,
            mut components,
        } = self;
        for component in &mut components {
            component.inverse(|limb| profile.main_plan(limb))?;
        }
        #[cfg(test)]
        tests::observe_transforms(0, 2 * (shape.level() + 1));
        Ok(Ciphertext::from_shape(
            profile,
            shape,
            Vec::from(components),
        ))
    }
}

/// One temporary clearing row; constructed before private callbacks.
struct PreparationRow(Vec<u32>);
impl Drop for PreparationRow {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_row_cleared(&self.0);
    }
}

/// Private, fixed-size, clearing matrix and bias owner for a public plan.
///
/// Private scalars follow the immutable public support in row/column order.
/// Missing coordinates are structural zeros, while every supported zero is
/// retained. All stored scalars are below the plan's effective modulus `p^k`, and the
/// owner binds that exact precision and master identity. Preparation may validate secret values and
/// is not constant time. The owner has no wire encoding or application identity.
pub struct SlotLinearMap<const INPUTS: usize, const OUTPUTS: usize> {
    precision: super::PlaintextPrecision,
    slots: usize,
    layout: SlotLinearLayout<INPUTS, OUTPUTS>,
    coefficients: Vec<u32>,
}

impl<const INPUTS: usize, const OUTPUTS: usize> fmt::Debug for SlotLinearMap<INPUTS, OUTPUTS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED packed linear map]")
    }
}
impl<const INPUTS: usize, const OUTPUTS: usize> Drop for SlotLinearMap<INPUTS, OUTPUTS> {
    fn drop(&mut self) {
        self.coefficients.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_cleared(&self.coefficients);
    }
}

impl<const INPUTS: usize, const OUTPUTS: usize> SlotLinearMap<INPUTS, OUTPUTS> {
    /// Copy a dense matrix/bias description into the same clearing row owner.
    ///
    /// Input layout is matrix[output bank][input bank][output slot][input slot],
    /// followed by bias[output bank][output slot]. It may describe a compact
    /// public plan only when all unsupported positions are zero. The caller
    /// remains responsible for clearing this borrowed description.
    ///
    /// # Errors
    /// Refuses the wrong dense length, noncanonical/unsupported coefficients,
    /// callback errors and allocation failure.
    pub fn new(
        plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
        coefficients: &[u32],
    ) -> Result<Self, SlotLinearError> {
        let slots = plan.slots();
        let expected = plan
            .layout
            .rows()
            .checked_mul(plan.layout.columns())
            .ok_or(SlotLinearError::GeometryOverflow)?;
        if coefficients.len() != expected {
            return Err(BgvError::LengthMismatch {
                expected,
                found: coefficients.len(),
            }
            .into());
        }
        Self::prepare_rows(plan, |row, values| {
            let bank = row / slots;
            let slot = row % slots;
            for input in 0..INPUTS {
                let start = ((bank * INPUTS + input) * slots + slot) * slots;
                values[input * slots..(input + 1) * slots]
                    .copy_from_slice(&coefficients[start..start + slots]);
            }
            values[INPUTS * slots] = coefficients[INPUTS * OUTPUTS * slots * slots + row];
            Ok(())
        })
    }

    /// Fill every logical row through one clearing scratch allocation.
    ///
    /// The public plan preflights the retained private payload, shared layout
    /// and the separate (INPUTS*slots+1)-word scratch ceiling. Both clearing
    /// owners exist before the first callback. Rows arrive in increasing public
    /// order, zeroed, with the bias in the last column. Every logical value is
    /// checked for canonicality; every unsupported value must be zero. Only
    /// supported coordinates are copied, in public row/column order. No secret
    /// support selects persistent layout or runtime work. Preparation is outside
    /// observable requests and is not constant time. Owners clear on any error
    /// or panic; caller-created extra copies are the caller's responsibility.
    ///
    /// # Errors
    /// Returns reservation, callback, canonicality and structural-support errors.
    pub fn prepare_rows(
        plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
        mut initialize: impl FnMut(usize, &mut [u32]) -> Result<(), SlotLinearError>,
    ) -> Result<Self, SlotLinearError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(plan.coefficient_count)
            .map_err(|_| SlotLinearError::Allocation)?;
        values.resize(plan.coefficient_count, 0);
        let mut owner = Self {
            precision: plan.encoding.plaintext_precision(),
            slots: plan.slots(),
            layout: plan.layout.clone(),
            coefficients: values,
        };
        let mut scratch = Vec::new();
        scratch
            .try_reserve_exact(plan.layout.columns())
            .map_err(|_| SlotLinearError::Allocation)?;
        scratch.resize(plan.layout.columns(), 0);
        let mut scratch = PreparationRow(scratch);
        for row in 0..plan.layout.rows() {
            scratch.0.fill(0);
            initialize(row, &mut scratch.0)?;
            for (column, &value) in scratch.0.iter().enumerate() {
                if u64::from(value) >= plan.plaintext_modulus() {
                    return Err(BgvError::NonCanonical {
                        index: row * plan.layout.columns() + column,
                    }
                    .into());
                }
                match plan.layout.coefficient_index(row, column) {
                    Some(at) => owner.coefficients[at] = value,
                    None if value != 0 => {
                        return Err(SlotLinearError::OutsideSupport { row, column });
                    }
                    None => {}
                }
            }
        }
        Ok(owner)
    }

    fn coefficient(&self, row: usize, column: usize) -> u32 {
        match self.layout.coefficient_index(row, column) {
            Some(index) => {
                #[cfg(test)]
                tests::observe_private_read(index);
                self.coefficients[index]
            }
            None => 0,
        }
    }

    /// Apply every publicly supported diagonal through existing exact BGV kernels.
    ///
    /// All key/input checks occur before reading coefficients. Results are in
    /// public output-bank order. No coefficient or private sparsity selects an
    /// operation, lookup address or allocation shape. The immutable public
    /// layout alone omits structural-zero products and empty Horner branches.
    /// Every output retains its own exact planned bound. Fixed shapes do not prove
    /// machine-level constant time or hide the noise of evaluated ciphertexts.
    ///
    /// # Errors
    /// Refuses mismatched profile/input/key shapes or underlying BGV errors.
    /// The caller must authenticate honest ciphertext/key provenance separately.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
        inputs: &[Ciphertext; INPUTS],
        keys: &EvaluationKeys,
    ) -> Result<Vec<Ciphertext>, SlotLinearError> {
        self.precision
            .check_same(plan.encoding.plaintext_precision())?;
        if self.slots != plan.encoding.slots() {
            return Err(BgvError::ProfileMismatch.into());
        }
        if self.layout != plan.layout {
            return Err(SlotLinearError::LayoutMismatch);
        }
        plan.check_inputs(profile, inputs, keys)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(OUTPUTS)
            .map_err(|_| SlotLinearError::Allocation)?;
        let mut weights = Zeroizing::new(vec![0_u64; self.slots]);
        #[cfg(test)]
        let matrix_count = INPUTS * OUTPUTS * self.slots * self.slots;
        for bank in 0..OUTPUTS {
            for (slot, to) in weights.iter_mut().enumerate() {
                #[cfg(test)]
                let index = matrix_count + bank * self.slots + slot;
                #[cfg(test)]
                tests::observe_read(index);
                *to = u64::from(self.coefficient(bank * self.slots + slot, INPUTS * self.slots));
            }
            // Start with the bias embedding, not zero followed by an add.
            let bias = plan.encoding.encode(profile, &weights)?;
            output.push(Ciphertext::trivial(
                profile,
                &bias,
                plan.outputs[bank].level(),
            )?);
        }
        let babies = plan.baby_count();
        let mut images = Vec::new();
        images
            .try_reserve_exact(INPUTS * babies)
            .map_err(|_| SlotLinearError::Allocation)?;
        for input in inputs {
            let start = images.len();
            images.push(input.clone());
            for step in 0..plan.baby_bits {
                let key = keys
                    .galois(plan.generators[step])
                    .ok_or(BgvError::WrongSwitchingKey)?;
                for at in 0..1_usize << step {
                    let image = profile.apply_galois(&images[start + at], key)?;
                    #[cfg(test)]
                    tests::observe_galois();
                    images.push(image);
                }
            }
        }
        // Consume each coefficient-domain owner into the transformed cache.
        // Moving the two RNS allocations creates no second residue cache.
        let mut transformed = Vec::new();
        transformed
            .try_reserve_exact(images.len())
            .map_err(|_| SlotLinearError::Allocation)?;
        for image in images {
            transformed.push(NttCiphertext::from_ciphertext(profile, image)?);
        }
        for (bank, accumulator) in output.iter_mut().enumerate() {
            let folded =
                self.fold_giants(profile, plan, keys, &transformed, bank, 0, 1, &mut weights)?;
            if let Some(folded) = folded {
                *accumulator = profile.add(accumulator, &folded)?;
                #[cfg(test)]
                tests::observe_addition();
            }
        }
        debug_assert!(
            output
                .iter()
                .zip(&plan.outputs)
                .all(|(x, shape)| &x.shape() == shape)
        );
        Ok(output)
    }

    #[allow(clippy::too_many_arguments)]
    fn fold_giants(
        &self,
        profile: &BgvProfile,
        plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
        keys: &EvaluationKeys,
        images: &[NttCiphertext],
        out_bank: usize,
        step: usize,
        giant: u32,
        weights: &mut [u64],
    ) -> Result<Option<Ciphertext>, SlotLinearError> {
        let giant_keys = &plan.generators[plan.baby_bits..];
        if step == giant_keys.len() {
            // Euler's theorem on the public power-of-two modulus: phi(2N)=N.
            // Only its induced slot permutation is applied to private masks.
            let inverse = crate::modular::mod_pow_u64(
                u64::from(giant),
                profile.degree() as u64 - 1,
                2 * profile.degree() as u64,
            ) as u32;
            let mut partial: Option<NttCiphertext> = None;
            for input_bank in 0..INPUTS {
                for (baby, &element) in plan.baby_elements.iter().enumerate() {
                    let diagonal = (u64::from(giant) * u64::from(element)
                        % (2 * profile.degree()) as u64) as u32;
                    if !self.layout.diagonal_active(out_bank, input_bank, diagonal) {
                        continue;
                    }
                    for (slot, to) in weights.iter_mut().enumerate() {
                        let row = plan.encoding.slot_source(inverse, slot)?;
                        let column = plan.encoding.slot_source(element, slot)?;
                        #[cfg(test)]
                        let index = ((out_bank * INPUTS + input_bank) * self.slots + row)
                            * self.slots
                            + column;
                        #[cfg(test)]
                        tests::observe_read(index);
                        *to = u64::from(self.coefficient(
                            out_bank * self.slots + row,
                            input_bank * self.slots + column,
                        ));
                    }
                    let diagonal = plan.encoding.encode(profile, weights)?;
                    let product = images[input_bank * plan.baby_count() + baby]
                        .multiply_plain(profile, &diagonal)?;
                    #[cfg(test)]
                    tests::observe_product();
                    partial = Some(match partial {
                        None => product,
                        Some(mut sum) => {
                            sum.add_assign(profile, &product)?;
                            sum
                        }
                    });
                }
            }
            return partial
                .map(|value| value.into_ciphertext(profile))
                .transpose()
                .map_err(Into::into);
        }
        let left = self.fold_giants(
            profile,
            plan,
            keys,
            images,
            out_bank,
            step + 1,
            giant,
            weights,
        )?;
        let generator = giant_keys[step];
        let composed =
            (u64::from(giant) * u64::from(generator) % (2 * profile.degree()) as u64) as u32;
        let right = self.fold_giants(
            profile,
            plan,
            keys,
            images,
            out_bank,
            step + 1,
            composed,
            weights,
        )?;
        #[cfg(test)]
        tests::fail_retained_siblings(step, left.is_some(), right.is_some())?;
        let Some(right) = right else {
            return Ok(left);
        };
        let key = keys.galois(generator).ok_or(BgvError::WrongSwitchingKey)?;
        let image = profile.apply_galois(&right, key)?;
        #[cfg(test)]
        tests::observe_galois();
        // Release the child before allocating the sum: the public scratch bound
        // counts only three local ciphertext buffers plus ancestor partials.
        drop(right);
        Ok(Some(match left {
            None => image,
            Some(left) => {
                let sum = profile.add(&left, &image)?;
                #[cfg(test)]
                tests::observe_addition();
                sum
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    //! Encrypted dense routing, fixed schedules, byte limits and clearing controls.

    use super::*;
    use crate::bgv::{
        BgvParameters, KeySet,
        fixtures::{self, FixtureEntropy},
    };
    use std::cell::{Cell, RefCell};

    impl<const INPUTS: usize, const OUTPUTS: usize> SlotLinearMap<INPUTS, OUTPUTS> {
        fn evaluate_coefficient_oracle(
            &self,
            profile: &BgvProfile,
            plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
            inputs: &[Ciphertext; INPUTS],
            keys: &EvaluationKeys,
        ) -> Result<Vec<Ciphertext>, SlotLinearError> {
            self.precision
                .check_same(plan.encoding.plaintext_precision())?;
            if self.slots != plan.encoding.slots() {
                return Err(BgvError::ProfileMismatch.into());
            }
            if self.layout != plan.layout {
                return Err(SlotLinearError::LayoutMismatch);
            }
            plan.check_inputs(profile, inputs, keys)?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(OUTPUTS)
                .map_err(|_| SlotLinearError::Allocation)?;
            let mut weights = Zeroizing::new(vec![0_u64; self.slots]);
            #[cfg(test)]
            let matrix_count = INPUTS * OUTPUTS * self.slots * self.slots;
            for bank in 0..OUTPUTS {
                for (slot, to) in weights.iter_mut().enumerate() {
                    #[cfg(test)]
                    let index = matrix_count + bank * self.slots + slot;
                    #[cfg(test)]
                    tests::observe_read(index);
                    *to =
                        u64::from(self.coefficient(bank * self.slots + slot, INPUTS * self.slots));
                }
                // Start with the bias embedding, not zero followed by an add.
                let bias = plan.encoding.encode(profile, &weights)?;
                output.push(Ciphertext::trivial(
                    profile,
                    &bias,
                    plan.outputs[bank].level(),
                )?);
            }
            let babies = plan.baby_count();
            let mut images = Vec::new();
            images
                .try_reserve_exact(INPUTS * babies)
                .map_err(|_| SlotLinearError::Allocation)?;
            for input in inputs {
                let start = images.len();
                images.push(input.clone());
                for step in 0..plan.baby_bits {
                    let key = keys
                        .galois(plan.generators[step])
                        .ok_or(BgvError::WrongSwitchingKey)?;
                    for at in 0..1_usize << step {
                        let image = profile.apply_galois(&images[start + at], key)?;
                        #[cfg(test)]
                        tests::observe_galois();
                        images.push(image);
                    }
                }
            }
            for (bank, accumulator) in output.iter_mut().enumerate() {
                let folded = self.fold_coefficient_oracle(
                    profile,
                    plan,
                    keys,
                    &images,
                    bank,
                    0,
                    1,
                    &mut weights,
                )?;
                *accumulator = profile.add(accumulator, &folded)?;
            }
            debug_assert!(
                output
                    .iter()
                    .zip(&plan.outputs)
                    .all(|(x, shape)| &x.shape() == shape)
            );
            Ok(output)
        }

        #[allow(clippy::too_many_arguments)]
        fn fold_coefficient_oracle(
            &self,
            profile: &BgvProfile,
            plan: &SlotLinearPlan<INPUTS, OUTPUTS>,
            keys: &EvaluationKeys,
            images: &[Ciphertext],
            out_bank: usize,
            step: usize,
            giant: u32,
            weights: &mut [u64],
        ) -> Result<Ciphertext, SlotLinearError> {
            let giant_keys = &plan.generators[plan.baby_bits..];
            if step == giant_keys.len() {
                // Euler's theorem on the public power-of-two modulus: phi(2N)=N.
                // Only its induced slot permutation is applied to private masks.
                let inverse = crate::modular::mod_pow_u64(
                    u64::from(giant),
                    profile.degree() as u64 - 1,
                    2 * profile.degree() as u64,
                ) as u32;
                let mut partial: Option<Ciphertext> = None;
                for input_bank in 0..INPUTS {
                    for (baby, &element) in plan.baby_elements.iter().enumerate() {
                        for (slot, to) in weights.iter_mut().enumerate() {
                            let row = plan.encoding.slot_source(inverse, slot)?;
                            let column = plan.encoding.slot_source(element, slot)?;
                            #[cfg(test)]
                            let index = ((out_bank * INPUTS + input_bank) * self.slots + row)
                                * self.slots
                                + column;
                            #[cfg(test)]
                            tests::observe_read(index);
                            *to = u64::from(self.coefficient(
                                out_bank * self.slots + row,
                                input_bank * self.slots + column,
                            ));
                        }
                        let diagonal = plan.encoding.encode(profile, weights)?;
                        let product = profile
                            .mul_plain(&images[input_bank * plan.baby_count() + baby], &diagonal)?;
                        #[cfg(test)]
                        tests::observe_product();
                        partial = Some(match partial {
                            None => product,
                            Some(sum) => profile.add(&sum, &product)?,
                        });
                    }
                }
                return partial.ok_or(BgvError::Kernel.into());
            }
            let left = self.fold_coefficient_oracle(
                profile,
                plan,
                keys,
                images,
                out_bank,
                step + 1,
                giant,
                weights,
            )?;
            let generator = giant_keys[step];
            let composed =
                (u64::from(giant) * u64::from(generator) % (2 * profile.degree()) as u64) as u32;
            let right = self.fold_coefficient_oracle(
                profile,
                plan,
                keys,
                images,
                out_bank,
                step + 1,
                composed,
                weights,
            )?;
            let key = keys.galois(generator).ok_or(BgvError::WrongSwitchingKey)?;
            let image = profile.apply_galois(&right, key)?;
            #[cfg(test)]
            tests::observe_galois();
            // Release the child before allocating the sum: the public scratch bound
            // counts only three local ciphertext buffers plus ancestor partials.
            drop(right);
            Ok(profile.add(&left, &image)?)
        }
    }

    thread_local! {
        static FAIL_SIBLINGS: Cell<u8> = const { Cell::new(0) };
        static CLEAR_CHECKPOINT: Cell<usize> = const { Cell::new(0) };
        static ADDITIONS: Cell<usize> = const { Cell::new(0) };
        static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
        static TRANSFORMS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
        static ACCESSES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
        static PRIVATE_ACCESSES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
        static ROW_CLEARED: Cell<usize> = const { Cell::new(0) };
        static CLEARED: Cell<usize> = const { Cell::new(0) };
    }
    pub(super) fn fail_retained_siblings(
        step: usize,
        left: bool,
        right: bool,
    ) -> Result<(), SlotLinearError> {
        if step != 0 || !left || !right {
            return Ok(());
        }
        let mode = FAIL_SIBLINGS.with(|x| x.replace(0));
        if mode == 0 {
            return Ok(());
        }
        CLEAR_CHECKPOINT.with(|x| x.set(super::super::poly::tests::cleared_words()));
        if mode == 2 {
            panic!("retained public subtree unwind control");
        }
        Err(BgvError::Kernel.into())
    }
    pub(super) fn observe_addition() {
        ADDITIONS.with(|x| x.set(x.get() + 1));
    }
    pub(super) fn observe_transforms(forward: usize, inverse: usize) {
        TRANSFORMS.with(|x| {
            let (f, i) = x.get();
            x.set((f + forward, i + inverse));
        });
    }
    pub(super) fn observe_private_read(index: usize) {
        PRIVATE_ACCESSES.with(|x| x.borrow_mut().push(index));
    }
    pub(super) fn observe_row_cleared(values: &[u32]) {
        assert!(values.iter().all(|&x| x == 0));
        ROW_CLEARED.with(|x| x.set(x.get() + values.len()));
    }
    pub(super) fn observe_read(index: usize) {
        ACCESSES.with(|x| x.borrow_mut().push(index));
    }
    pub(super) fn observe_product() {
        COUNTS.with(|x| {
            let (g, p) = x.get();
            x.set((g, p + 1));
        });
    }
    pub(super) fn observe_galois() {
        COUNTS.with(|x| {
            let (g, p) = x.get();
            x.set((g + 1, p));
        });
    }
    pub(super) fn observe_cleared(values: &[u32]) {
        assert!(values.iter().all(|&x| x == 0));
        CLEARED.with(|x| x.set(x.get() + values.len()));
    }
    fn profile(primes: usize) -> BgvProfile {
        BgvProfile::new(
            BgvParameters::derive(8, 257, 1, (40, primes), (41, 2), primes.min(2)).unwrap(),
        )
        .unwrap()
    }
    fn budget<const I: usize, const O: usize>(profile: &BgvProfile) -> SlotLinearBudget {
        let s = SlotEncoding::new(profile, profile.master_plaintext())
            .unwrap()
            .slots();
        SlotLinearBudget {
            coefficient_bytes: (I * O * s * s + O * s) * size_of::<u32>(),
            layout_metadata_bytes: SlotLinearLayout::<I, O>::metadata_bytes_for(s, O * s).unwrap(),
            preparation_scratch_bytes: (I * s + 1) * size_of::<u32>(),
            output_residue_bytes: O * usize::try_from(
                CostModel::new(profile)
                    .ciphertext_residue_bytes(profile.top_level(), 2)
                    .unwrap(),
            )
            .unwrap(),
            scratch_residue_bytes: (I * s + 3)
                * usize::try_from(
                    CostModel::new(profile)
                        .ciphertext_residue_bytes(profile.top_level(), 2)
                        .unwrap(),
                )
                .unwrap(),
        }
    }
    fn plan<const I: usize, const O: usize>(profile: &BgvProfile) -> SlotLinearPlan<I, O> {
        SlotLinearPlan::new(
            profile,
            &std::array::from_fn(|_| profile.plan_fresh(profile.master_plaintext()).unwrap()),
            budget::<I, O>(profile),
        )
        .unwrap()
    }
    fn key_set<const I: usize, const O: usize>(
        profile: &BgvProfile,
        plan: &SlotLinearPlan<I, O>,
    ) -> KeySet {
        fixtures::key_set(
            profile,
            plan.galois_elements(),
            &mut FixtureEntropy::new([89; 64]),
        )
        .unwrap()
    }
    fn encrypt(profile: &BgvProfile, keys: &KeySet, slots: &[u64], seed: u8) -> Ciphertext {
        fixtures::encrypt(
            profile,
            &keys.public,
            &SlotEncoding::new(profile, profile.master_plaintext())
                .unwrap()
                .encode(profile, slots)
                .unwrap(),
            &mut FixtureEntropy::new([seed; 64]),
        )
        .unwrap()
    }

    #[test]
    fn dense_sparse_zero_and_adversarial_maps_match_scalar_matrix_at_every_slot() {
        let profile = profile(3);
        let plan = plan::<2, 3>(&profile);
        let keys = key_set(&profile, &plan);
        let values = std::array::from_fn::<_, 2, _>(|bank| {
            std::array::from_fn::<_, 128, _>(|slot| ((bank * 91 + slot * 37 + 256) % 257) as u64)
        });
        let inputs =
            std::array::from_fn(|bank| encrypt(&profile, &keys, &values[bank], bank as u8 + 11));
        let matrix_count = 2 * 3 * 128 * 128;
        let mut expected_accesses = None;
        for kind in 0..4 {
            let mut coefficients = Zeroizing::new(vec![0_u32; plan.coefficient_count()]);
            for out in 0..3 {
                for bank in 0..2 {
                    for row in 0..128 {
                        for col in 0..128 {
                            let entry = match kind {
                                0 => (out * 37 + bank * 23 + row * 17 + col * 11 + 1) % 257,
                                1 => usize::from(col == (row + bank + out) % 128),
                                2 => 0,
                                _ => {
                                    if col == 127 - row {
                                        256
                                    } else {
                                        0
                                    }
                                }
                            };
                            coefficients[((out * 2 + bank) * 128 + row) * 128 + col] = entry as u32;
                        }
                    }
                }
                for row in 0..128 {
                    coefficients[matrix_count + out * 128 + row] = if kind == 2 {
                        0
                    } else {
                        ((row * 13 + out * 31 + 256) % 257) as u32
                    };
                }
            }
            let owner = SlotLinearMap::new(&plan, &coefficients).unwrap();
            COUNTS.with(|x| x.set((0, 0)));
            ACCESSES.with(|x| x.borrow_mut().clear());
            let result = owner
                .evaluate(&profile, &plan, &inputs, &keys.evaluation)
                .unwrap();
            assert_eq!(result.len(), 3);
            let accesses = ACCESSES.with(|x| x.borrow().clone());
            assert_eq!(accesses.len(), plan.coefficient_count());
            if let Some(expected) = &expected_accesses {
                assert_eq!(&accesses, expected);
            } else {
                expected_accesses = Some(accesses);
            }
            assert_eq!(
                COUNTS.with(Cell::get),
                (plan.galois_applications(), plan.plaintext_products())
            );
            assert_eq!(plan.galois_applications(), 51);
            assert_eq!((plan.baby_count(), plan.giant_count()), (16, 8));
            assert_eq!(plan.plaintext_products(), 2 * 3 * 128);
            let coefficient_result = owner
                .evaluate_coefficient_oracle(&profile, &plan, &inputs, &keys.evaluation)
                .unwrap();
            assert_eq!(
                result, coefficient_result,
                "dense/sparse/zero/adversarial exact residues"
            );
            for (out, cipher) in result.iter().enumerate() {
                assert_eq!(&cipher.shape(), &plan.output_shapes()[out]);
                let opened = keys.secret.decrypt(&profile, cipher).unwrap();
                let actual = SlotEncoding::new(&profile, profile.master_plaintext())
                    .unwrap()
                    .decode(&opened)
                    .unwrap();
                for row in 0..128 {
                    let mut expected = u64::from(coefficients[matrix_count + out * 128 + row]);
                    for bank in 0..2 {
                        for col in 0..128 {
                            expected +=
                                u64::from(coefficients[((out * 2 + bank) * 128 + row) * 128 + col])
                                    * values[bank][col];
                        }
                    }
                    assert_eq!(
                        actual[row],
                        expected % 257,
                        "kind {kind}, output {out}, slot {row}"
                    );
                }
            }
        }
    }

    #[test]
    fn public_plan_matches_closed_noise_and_exact_buffer_budgets() {
        let profile = profile(3);
        let plan = plan::<2, 3>(&profile);
        let encoding = SlotEncoding::new(&profile, profile.master_plaintext()).unwrap();
        let babies = plan.baby_count() as u64;
        let giants = plan.giant_count() as u64;
        let baby_bits = babies.ilog2() as u64;
        let expected = profile
            .fresh_noise()
            .mul_u64(2 * 128)
            .add(
                &profile
                    .key_switch_noise(profile.top_level())
                    .unwrap()
                    .mul_u64(2 * giants * (babies / 2) * baby_bits),
            )
            .mul_u64(encoding.vector_bound().l1)
            .add(
                &profile
                    .key_switch_noise(profile.top_level())
                    .unwrap()
                    .mul_u64(giants - 1),
            )
            .add_u64(128);
        assert_eq!(plan.output_shapes()[0].bound(), &expected);
        assert_eq!(plan.coefficient_count(), 2 * 3 * 128 * 128 + 3 * 128);
        let exact = budget::<2, 3>(&profile);
        assert_eq!(plan.coefficient_bytes(), exact.coefficient_bytes);
        assert_eq!(plan.output_residue_bytes(), exact.output_residue_bytes);
        let inputs =
            std::array::from_fn(|_| profile.plan_fresh(profile.master_plaintext()).unwrap());
        let mut small = exact;
        small.coefficient_bytes -= 1;
        assert_eq!(
            SlotLinearPlan::<2, 3>::new(&profile, &inputs, small).unwrap_err(),
            SlotLinearError::CoefficientBudget {
                required: exact.coefficient_bytes,
                budget: small.coefficient_bytes
            }
        );
        small = exact;
        small.output_residue_bytes -= 1;
        assert_eq!(
            SlotLinearPlan::<2, 3>::new(&profile, &inputs, small).unwrap_err(),
            SlotLinearError::OutputBudget {
                required: exact.output_residue_bytes,
                budget: small.output_residue_bytes
            }
        );
    }

    #[test]
    fn public_layout_and_preparation_budgets_preflight_before_private_allocation() {
        let profile = profile(3);
        let public = plan::<1, 1>(&profile);
        let before = CLEARED.with(Cell::get);
        for which in 0..2 {
            let mut small = budget::<1, 1>(&profile);
            if which == 0 {
                small.layout_metadata_bytes -= 1;
            } else {
                small.preparation_scratch_bytes -= 1;
            }
            let error =
                SlotLinearPlan::with_layout(&profile, &public.inputs, public.layout.clone(), small)
                    .unwrap_err();
            assert!(matches!(
                error,
                SlotLinearError::LayoutBudget { .. } | SlotLinearError::PreparationBudget { .. }
            ));
        }
        let wrong_slots = SlotLinearLayout::<1, 1>::dense(64, 1 << 20).unwrap();
        assert_eq!(
            SlotLinearPlan::with_layout(
                &profile,
                &public.inputs,
                wrong_slots,
                budget::<1, 1>(&profile)
            )
            .unwrap_err(),
            SlotLinearError::LayoutMismatch
        );
        assert_eq!(CLEARED.with(Cell::get), before);
    }

    #[test]
    fn invalid_geometry_profile_and_shapes_refuse_before_coefficient_allocation() {
        let profile = profile(3);
        let fresh = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let large = SlotLinearBudget {
            coefficient_bytes: usize::MAX,
            layout_metadata_bytes: usize::MAX,
            preparation_scratch_bytes: usize::MAX,
            output_residue_bytes: usize::MAX,
            scratch_residue_bytes: usize::MAX,
        };
        assert_eq!(
            SlotLinearPlan::<0, 1>::new(&profile, &[], large).unwrap_err(),
            SlotLinearError::EmptyDimensions
        );
        assert_eq!(
            SlotLinearPlan::<1, 0>::new(&profile, &[fresh.clone()], large).unwrap_err(),
            SlotLinearError::EmptyDimensions
        );
        assert_eq!(
            // Validate the canonical public geometry before instantiating a
            // plan type whose per-output shape array cannot exist in Rust.
            SlotLinearLayout::<1, { usize::MAX }>::dense(128, usize::MAX).unwrap_err(),
            SlotLinearError::GeometryOverflow
        );
        let three = profile.plan_tensor(&fresh, &fresh).unwrap();
        assert!(matches!(
            SlotLinearPlan::<1, 1>::new(&profile, &[three], large),
            Err(SlotLinearError::Bgv(BgvError::ComponentCount { found: 3 }))
        ));
        let lower = profile.plan_switch_down(&fresh).unwrap();
        assert!(matches!(
            SlotLinearPlan::<2, 1>::new(&profile, &[fresh, lower], large),
            Err(SlotLinearError::Bgv(BgvError::LevelMismatch { .. }))
        ));
        let small = super::tests::profile(1);
        assert!(matches!(
            SlotLinearPlan::<1, 1>::new(
                &small,
                &[small.plan_fresh(small.master_plaintext()).unwrap()],
                large
            ),
            Err(SlotLinearError::Bgv(BgvError::NoiseBudgetExceeded { .. }))
        ));
        let unavailable =
            BgvProfile::new(BgvParameters::derive(4, 3, 1, (30, 3), (31, 2), 2).unwrap()).unwrap();
        assert!(matches!(
            SlotLinearPlan::<1, 1>::new(
                &unavailable,
                &[unavailable
                    .plan_fresh(unavailable.master_plaintext())
                    .unwrap()],
                large
            ),
            Err(SlotLinearError::Bgv(BgvError::EncodingUnavailable))
        ));
    }

    #[test]
    fn coefficient_lengths_values_and_private_owner_clearing_are_checked() {
        let profile = profile(3);
        let plan = plan::<1, 1>(&profile);
        let before = CLEARED.with(Cell::get);
        assert!(matches!(
            SlotLinearMap::new(&plan, &[]),
            Err(SlotLinearError::Bgv(BgvError::LengthMismatch { .. }))
        ));
        let mut coefficients = Zeroizing::new(vec![256; plan.coefficient_count()]);
        coefficients[17] = 257;
        assert_eq!(
            SlotLinearMap::new(&plan, &coefficients).unwrap_err(),
            SlotLinearError::Bgv(BgvError::NonCanonical { index: 17 })
        );
        assert_eq!(CLEARED.with(Cell::get) - before, plan.coefficient_count());
        coefficients[17] = 256;
        let owner = SlotLinearMap::new(&plan, &coefficients).unwrap();
        assert_eq!(format!("{owner:?}"), "[REDACTED packed linear map]");
        drop(owner);
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            2 * plan.coefficient_count()
        );
        let error = || -> Result<(), SlotLinearError> {
            let _owner = SlotLinearMap::new(&plan, &coefficients)?;
            Err(SlotLinearError::Allocation)
        };
        assert!(error().is_err());
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            3 * plan.coefficient_count()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _owner = SlotLinearMap::new(&plan, &coefficients).unwrap();
                panic!("clearing test");
            })
            .is_err()
        );
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            4 * plan.coefficient_count()
        );
    }

    #[test]
    fn input_and_key_mismatch_are_rejected_without_evaluating_private_rows() {
        let profile = profile(3);
        let plan = plan::<1, 1>(&profile);
        let keys = key_set(&profile, &plan);
        let input = encrypt(&profile, &keys, &[17; 128], 37);
        let owner = SlotLinearMap::new(&plan, &vec![256; plan.coefficient_count()]).unwrap();
        let missing = fixtures::evaluation_keys(
            &profile,
            &keys.secret,
            &[],
            &mut FixtureEntropy::new([39; 64]),
        )
        .unwrap();
        COUNTS.with(|x| x.set((0, 0)));
        ACCESSES.with(|x| x.borrow_mut().clear());
        PRIVATE_ACCESSES.with(|x| x.borrow_mut().clear());
        assert_eq!(
            owner
                .evaluate(&profile, &plan, &[input.clone()], &missing)
                .unwrap_err(),
            SlotLinearError::Bgv(BgvError::WrongSwitchingKey)
        );
        let doubled = profile.add(&input, &input).unwrap();
        assert_eq!(
            owner
                .evaluate(&profile, &plan, &[doubled], &keys.evaluation)
                .unwrap_err(),
            SlotLinearError::InputShape { bank: 0 }
        );
        let other = super::tests::profile(4);
        assert_eq!(
            owner
                .evaluate(&other, &plan, &[input.clone()], &keys.evaluation)
                .unwrap_err(),
            SlotLinearError::Bgv(BgvError::ProfileMismatch)
        );
        let other_plan = super::tests::plan::<1, 1>(&other);
        let other_keys = key_set(&other, &other_plan);
        assert_eq!(
            owner
                .evaluate(&profile, &plan, &[input], &other_keys.evaluation)
                .unwrap_err(),
            SlotLinearError::Bgv(BgvError::ProfileMismatch)
        );
        assert_eq!(COUNTS.with(Cell::get), (0, 0));
        assert!(ACCESSES.with(|x| x.borrow().is_empty()));
        assert!(PRIVATE_ACCESSES.with(|x| x.borrow().is_empty()));
    }

    #[test]
    fn encrypted_routing_covers_every_source_destination_pair() {
        // The other encrypted cases use the sparse subalgebra at N=256; this
        // exhaustive placement case also covers fully split batching at N=128.
        let profile =
            BgvProfile::new(BgvParameters::derive(7, 257, 1, (40, 3), (41, 2), 2).unwrap())
                .unwrap();
        let plan = plan::<1, 128>(&profile);
        let keys = key_set(&profile, &plan);
        let values = std::array::from_fn::<_, 128, _>(|slot| ((slot * 37 + 17) % 257) as u64);
        let inputs = [encrypt(&profile, &keys, &values, 71)];
        let mut coefficients = Zeroizing::new(vec![0_u32; plan.coefficient_count()]);
        for source in 0..128 {
            for destination in 0..128 {
                coefficients[(source * 128 + destination) * 128 + source] = 1;
            }
        }
        let owner = SlotLinearMap::new(&plan, &coefficients).unwrap();
        COUNTS.with(|x| x.set((0, 0)));
        ACCESSES.with(|x| x.borrow_mut().clear());
        let result = owner
            .evaluate(&profile, &plan, &inputs, &keys.evaluation)
            .unwrap();
        assert_eq!(COUNTS.with(Cell::get), (127, 128 * 128));
        assert_eq!(result.len(), 128);
        for (source, cipher) in result.iter().enumerate() {
            let opened = keys.secret.decrypt(&profile, cipher).unwrap();
            let actual = SlotEncoding::new(&profile, profile.master_plaintext())
                .unwrap()
                .decode(&opened)
                .unwrap();
            assert_eq!(actual.as_slice(), &[values[source]; 128], "source {source}");
            assert_eq!(&cipher.shape(), &plan.output_shapes()[0]);
        }
        let mut accesses = ACCESSES.with(|x| x.borrow().clone());
        accesses.sort_unstable();
        assert_eq!(accesses, (0..plan.coefficient_count()).collect::<Vec<_>>());
    }

    #[test]
    fn public_bsgs_split_and_scratch_budget_tradeoff_preserve_encrypted_values() {
        let profile = profile(3);
        let shapes = [profile.plan_fresh(profile.master_plaintext()).unwrap()];
        let normal = plan::<1, 1>(&profile);
        assert_eq!((normal.baby_count(), normal.giant_count()), (8, 16));
        assert_eq!(normal.galois_applications(), 22);
        let one = usize::try_from(
            CostModel::new(&profile)
                .ciphertext_residue_bytes(profile.top_level(), 2)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(normal.scratch_residue_bytes(), (8 + 4 + 3) * one);
        let mut restricted = budget::<1, 1>(&profile);
        restricted.scratch_residue_bytes = (1 + 7 + 3) * one;
        let compact = SlotLinearPlan::<1, 1>::new(&profile, &shapes, restricted).unwrap();
        assert_eq!((compact.baby_count(), compact.giant_count()), (2, 64));
        assert_eq!(compact.galois_applications(), 64);
        assert_eq!(
            compact.scratch_residue_bytes(),
            restricted.scratch_residue_bytes
        );
        let mut too_small = restricted;
        too_small.scratch_residue_bytes -= 1;
        assert_eq!(
            SlotLinearPlan::<1, 1>::new(&profile, &shapes, too_small).unwrap_err(),
            SlotLinearError::ScratchBudget {
                required: restricted.scratch_residue_bytes,
                budget: too_small.scratch_residue_bytes
            }
        );
        let keys = key_set(&profile, &normal);
        let values = std::array::from_fn::<_, 128, _>(|slot| ((slot * 53 + 256) % 257) as u64);
        let inputs = [encrypt(&profile, &keys, &values, 97)];
        let mut coefficients = Zeroizing::new(vec![0_u32; normal.coefficient_count()]);
        let mut expected = [0_u64; 128];
        for row in 0..128 {
            coefficients[128 * 128 + row] = ((row * 19 + 256) % 257) as u32;
            expected[row] = u64::from(coefficients[128 * 128 + row]);
            for column in 0..128 {
                let coefficient = ((row * 31 + column * 17 + 256) % 257) as u32;
                coefficients[row * 128 + column] = coefficient;
                expected[row] += u64::from(coefficient) * values[column];
            }
            expected[row] %= 257;
        }
        let owner = SlotLinearMap::new(&normal, &coefficients).unwrap();
        for public_plan in [&normal, &compact] {
            COUNTS.with(|x| x.set((0, 0)));
            ACCESSES.with(|x| x.borrow_mut().clear());
            let output = owner
                .evaluate(&profile, public_plan, &inputs, &keys.evaluation)
                .unwrap();
            let opened = keys.secret.decrypt(&profile, &output[0]).unwrap();
            let actual = SlotEncoding::new(&profile, profile.master_plaintext())
                .unwrap()
                .decode(&opened)
                .unwrap();
            assert_eq!(actual.as_slice(), &expected);
            assert_eq!(&output[0].shape(), &public_plan.output_shapes()[0]);
            assert_eq!(
                COUNTS.with(Cell::get),
                (public_plan.galois_applications(), 128)
            );
            let mut accesses = ACCESSES.with(|x| x.borrow().clone());
            accesses.sort_unstable();
            assert_eq!(
                accesses,
                (0..normal.coefficient_count()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn public_noise_capacity_can_select_a_more_expensive_split() {
        let profile = profile(1);
        let encoding = SlotEncoding::new(&profile, profile.master_plaintext()).unwrap();
        let capacity = profile.capacity(0).unwrap().to_u64().unwrap();
        let key_noise = profile.key_switch_noise(0).unwrap().to_u64().unwrap();
        let slots = encoding.slots() as u64;
        let bound =
            (capacity - 128 - (slots - 1) * key_noise) / (slots * encoding.vector_bound().l1);
        let shape = profile
            .plan_claimed(
                profile.master_plaintext(),
                0,
                2,
                crate::natural::Natural::from_u64(bound),
            )
            .unwrap();
        let public_plan =
            SlotLinearPlan::<1, 1>::new(&profile, &[shape], budget::<1, 1>(&profile)).unwrap();
        assert_eq!(
            (public_plan.baby_count(), public_plan.giant_count()),
            (1, 128)
        );
        assert_eq!(public_plan.galois_applications(), 127);
        let expected = 128 + slots * encoding.vector_bound().l1 * bound + (slots - 1) * key_noise;
        assert_eq!(
            public_plan.output_shapes()[0].bound().to_u64(),
            Some(expected)
        );
        assert!(expected <= capacity);
    }
    #[test]
    fn row_initializer_is_clearing_fallible_and_profile_bound() {
        let profile = profile(3);
        let plan = plan::<1, 1>(&profile);
        assert_eq!(plan.slots(), 128);
        assert_eq!(plan.plaintext_modulus(), 257);
        let before = CLEARED.with(Cell::get);
        let rows_before = ROW_CLEARED.with(Cell::get);
        let mut calls = 0;
        let owner = SlotLinearMap::prepare_rows(&plan, |row, values| {
            assert_eq!(row, calls);
            calls += 1;
            assert_eq!(values.len(), 129);
            assert!(values.iter().all(|&x| x == 0));
            values.fill(256);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 128);
        assert_eq!(owner.precision, plan.encoding.plaintext_precision());
        assert_eq!(owner.precision.profile_identity(), profile.identity());
        assert!(owner.coefficients.iter().all(|&x| x == 256));
        drop(owner);
        assert_eq!(CLEARED.with(Cell::get) - before, plan.coefficient_count());
        assert_eq!(
            SlotLinearMap::prepare_rows(&plan, |row, values| {
                values.fill(256);
                if row == 11 {
                    Err(SlotLinearError::InputShape { bank: 7 })
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
            SlotLinearError::InputShape { bank: 7 }
        );
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            2 * plan.coefficient_count()
        );
        assert_eq!(
            SlotLinearMap::prepare_rows(&plan, |row, values| {
                values.fill(256);
                if row == 127 {
                    *values.last_mut().unwrap() = 257;
                }
                Ok(())
            })
            .unwrap_err(),
            SlotLinearError::Bgv(BgvError::NonCanonical {
                index: plan.coefficient_count() - 1
            })
        );
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            3 * plan.coefficient_count()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = SlotLinearMap::prepare_rows(&plan, |row, values| {
                    values.fill(256);
                    if row == 13 {
                        panic!("row initializer unwind control");
                    }
                    Ok(())
                });
            })
            .is_err()
        );
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            4 * plan.coefficient_count()
        );
        assert_eq!(ROW_CLEARED.with(Cell::get) - rows_before, 4 * 129);
    }

    #[test]
    fn public_support_preserves_dense_residues_and_exact_public_schedules() {
        let profile = profile(3);
        let dense = plan::<2, 3>(&profile);
        let mut offsets = vec![0];
        let mut segments = Vec::new();
        for row in 0..384 {
            // An empty row, a strided row, or separated singletons is public.
            match row % 3 {
                0 => {}
                1 => segments.push(SlotLinearSegment::new(row % 17, 8, 31)),
                _ => {
                    segments.push(SlotLinearSegment::new(row % 128, 1, 1));
                    segments.push(SlotLinearSegment::new(128 + (row * 13) % 128, 1, 1));
                    segments.push(SlotLinearSegment::new(256, 1, 1));
                }
            }
            offsets.push(segments.len());
        }
        let required = SlotLinearLayout::<2, 3>::metadata_bytes_for(128, segments.len()).unwrap();
        let layout = SlotLinearLayout::new(128, offsets, segments, required).unwrap();
        let mut public_budget = budget::<2, 3>(&profile);
        public_budget.layout_metadata_bytes = required;
        public_budget.coefficient_bytes = layout.coefficient_count() * 4;
        let compact =
            SlotLinearPlan::with_layout(&profile, &dense.inputs, layout.clone(), public_budget)
                .unwrap();
        assert!(
            compact
                .output_shapes()
                .iter()
                .zip(dense.output_shapes())
                .all(|(a, b)| a.bound() <= b.bound())
        );
        assert!(compact.galois_applications() <= dense.galois_applications());
        assert!(compact.plaintext_products() < dense.plaintext_products());
        assert!(
            compact.arithmetic_units(&profile).unwrap() < dense.arithmetic_units(&profile).unwrap()
        );
        let keys = key_set(&profile, &dense);
        let values = std::array::from_fn::<_, 2, _>(|bank| {
            std::array::from_fn::<_, 128, _>(|slot| ((slot * 41 + bank * 97 + 256) % 257) as u64)
        });
        let inputs =
            std::array::from_fn(|bank| encrypt(&profile, &keys, &values[bank], 31 + bank as u8));
        let mut access_trace = None;
        for zero in [false, true] {
            let mut description = Zeroizing::new(vec![0_u32; dense.coefficient_count()]);
            for row in 0..384 {
                for column in 0..257 {
                    if layout.coefficient_index(row, column).is_some() {
                        let value = if zero {
                            0
                        } else {
                            ((row * 37 + column * 73 + 256) % 257) as u32
                        };
                        let index = if column == 256 {
                            2 * 3 * 128 * 128 + row
                        } else {
                            ((row / 128 * 2 + column / 128) * 128 + row % 128) * 128 + column % 128
                        };
                        description[index] = value;
                    }
                }
            }
            let map = SlotLinearMap::new(&compact, &description).unwrap();
            let dense_same_split = force_split(&profile, &dense, compact.baby_bits);
            let full = SlotLinearMap::new(&dense_same_split, &description).unwrap();
            PRIVATE_ACCESSES.with(|x| x.borrow_mut().clear());
            ACCESSES.with(|x| x.borrow_mut().clear());
            COUNTS.with(|x| x.set((0, 0)));
            let actual = map
                .evaluate(&profile, &compact, &inputs, &keys.evaluation)
                .unwrap();
            let private = PRIVATE_ACCESSES.with(|x| x.borrow().clone());
            let logical = ACCESSES.with(|x| x.borrow().clone());
            assert_eq!(
                COUNTS.with(Cell::get),
                (compact.galois_applications(), compact.plaintext_products())
            );
            let mut sorted = private.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, (0..compact.coefficient_count()).collect::<Vec<_>>());
            if let Some(expected) = &access_trace {
                assert_eq!(&private, expected);
            } else {
                access_trace = Some(private);
            }
            ACCESSES.with(|x| x.borrow_mut().clear());
            let expected = full
                .evaluate(&profile, &dense_same_split, &inputs, &keys.evaluation)
                .unwrap();
            assert_eq!(
                actual
                    .iter()
                    .map(Ciphertext::canonical_words)
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(Ciphertext::canonical_words)
                    .collect::<Vec<_>>()
            );
            assert!(logical.len() < ACCESSES.with(|x| x.borrow().len()));
            for (value, shape) in actual.iter().zip(compact.output_shapes()) {
                assert_eq!(&value.shape(), shape);
            }
            for (bank, ciphertext) in actual.iter().enumerate() {
                let decoded = SlotEncoding::new(&profile, profile.master_plaintext())
                    .unwrap()
                    .decode(&keys.secret.decrypt(&profile, ciphertext).unwrap())
                    .unwrap();
                for row in 0..128 {
                    let mut scalar = u64::from(description[2 * 3 * 128 * 128 + bank * 128 + row]);
                    for input in 0..2 {
                        for col in 0..128 {
                            scalar += u64::from(
                                description[((bank * 2 + input) * 128 + row) * 128 + col],
                            ) * values[input][col];
                        }
                    }
                    assert_eq!(decoded[row], scalar % 257);
                }
            }
            PRIVATE_ACCESSES.with(|x| x.borrow_mut().clear());
            assert_eq!(
                map.evaluate(&profile, &dense, &inputs, &keys.evaluation)
                    .unwrap_err(),
                SlotLinearError::LayoutMismatch
            );
            assert!(PRIVATE_ACCESSES.with(|x| x.borrow().is_empty()));
        }
        let before = CLEARED.with(Cell::get);
        let rows_before = ROW_CLEARED.with(Cell::get);
        assert_eq!(
            SlotLinearMap::prepare_rows(&compact, |row, values| {
                if row == 0 {
                    values[0] = 1;
                }
                Ok(())
            })
            .unwrap_err(),
            SlotLinearError::OutsideSupport { row: 0, column: 0 }
        );
        assert_eq!(
            SlotLinearMap::prepare_rows(&compact, |row, values| {
                if row == 0 {
                    values[0] = 257;
                }
                Ok(())
            })
            .unwrap_err(),
            SlotLinearError::Bgv(BgvError::NonCanonical { index: 0 })
        );
        assert_eq!(
            CLEARED.with(Cell::get) - before,
            2 * compact.coefficient_count()
        );
        assert_eq!(ROW_CLEARED.with(Cell::get) - rows_before, 2 * 257);
    }

    fn force_split<const I: usize, const O: usize>(
        profile: &BgvProfile,
        plan: &SlotLinearPlan<I, O>,
        bits: usize,
    ) -> SlotLinearPlan<I, O> {
        let mut forced = plan.clone();
        forced.baby_bits = bits;
        forced.baby_elements = (0..1_usize << bits)
            .map(|mask| group_element(profile, &forced.generators[..bits], mask))
            .collect();
        let transfer =
            plan_split(profile, &plan.encoding, &plan.inputs, &plan.layout, bits).unwrap();
        forced.outputs = transfer.outputs;
        forced.nonempty_leaves = transfer.leaves;
        forced.galois_applications = transfer.galois;
        forced.plaintext_products = transfer.products;
        forced
    }

    #[test]
    fn retained_public_siblings_cache_and_outputs_clear_on_error_and_unwind() {
        let profile = profile(3);
        let dense = plan::<1, 1>(&profile);
        let mut offsets = vec![0];
        let mut segments = Vec::new();
        for row in 0..128 {
            let other = ((5 * (2 * row + 1) % 256) - 1) / 2;
            let mut columns = [row, other];
            columns.sort_unstable();
            for column in columns {
                segments.push(SlotLinearSegment::new(column, 1, 1));
            }
            offsets.push(segments.len());
        }
        let layout = SlotLinearLayout::<1, 1>::new(128, offsets, segments, 1 << 20).unwrap();
        let mut limits = budget::<1, 1>(&profile);
        limits.layout_metadata_bytes = layout.metadata_bytes();
        let plan = SlotLinearPlan::with_layout(&profile, &dense.inputs, layout, limits).unwrap();
        let plan = force_split(&profile, &plan, 0);
        let keys = key_set(&profile, &plan);
        let input = [encrypt(&profile, &keys, &[31; 128], 209)];
        let map = SlotLinearMap::prepare_rows(&plan, |row, values| {
            values[row] = 7;
            values[((5 * (2 * row + 1) % 256) - 1) / 2] = 256;
            Ok(())
        })
        .unwrap();
        for mode in [1, 2] {
            FAIL_SIBLINGS.with(|x| x.set(mode));
            let outcome = std::panic::catch_unwind(|| {
                map.evaluate(&profile, &plan, &input, &keys.evaluation)
            });
            if mode == 1 {
                assert_eq!(
                    outcome.unwrap().unwrap_err(),
                    SlotLinearError::Bgv(BgvError::Kernel)
                );
            } else {
                assert!(outcome.is_err());
            }
            assert_eq!(FAIL_SIBLINGS.with(Cell::get), 0);
            // Root left+right, one full baby cache and one bias accumulator:
            // exactly four two-component owners remain when the fault fires.
            assert_eq!(
                super::super::poly::tests::cleared_words() - CLEAR_CHECKPOINT.with(Cell::get),
                4 * 2 * profile.degree() * (profile.top_level() + 1)
            );
        }
    }

    #[test]
    fn public_empty_branches_bias_only_and_private_zeros_preserve_every_split() {
        // Four publicly distinct outputs: empty, bias-only, identity diagonal,
        // and one all-generator diagonal. No private value selects those shapes.
        let profile = profile(3);
        let dense = plan::<1, 4>(&profile);
        let mut offsets = vec![0];
        let mut segments = Vec::new();
        for row in 0..512 {
            match row / 128 {
                0 => {}
                1 => segments.push(SlotLinearSegment::new(128, 1, 1)),
                2 => segments.push(SlotLinearSegment::new(row % 128, 1, 1)),
                _ => segments.push(SlotLinearSegment::new(
                    ((51 * (2 * (row % 128) + 1) % 256) - 1) / 2,
                    1,
                    1,
                )),
            }
            offsets.push(segments.len());
        }
        let layout = SlotLinearLayout::<1, 4>::new(128, offsets, segments, 1 << 20).unwrap();
        let mut limits = budget::<1, 4>(&profile);
        limits.layout_metadata_bytes = layout.metadata_bytes();
        limits.coefficient_bytes = layout.coefficient_count() * 4;
        let normal = SlotLinearPlan::with_layout(&profile, &dense.inputs, layout, limits).unwrap();
        let keys = key_set(&profile, &normal);
        let values = std::array::from_fn::<_, 128, _>(|i| ((i * 73 + 256) % 257) as u64);
        let input = [encrypt(&profile, &keys, &values, 203)];
        for bits in 0..=7 {
            let public = force_split(&profile, &normal, bits);
            let full_plan = force_split(&profile, &dense, bits);
            let mut trace = None;
            for zero in [false, true] {
                let mut description = Zeroizing::new(vec![0_u32; dense.coefficient_count()]);
                if !zero {
                    for row in 0..128 {
                        description[4 * 128 * 128 + 128 + row] = 256;
                        description[(2 * 128 + row) * 128 + row] = 256;
                        description[(3 * 128 + row) * 128 + ((51 * (2 * row + 1) % 256) - 1) / 2] =
                            1;
                    }
                }
                let map = SlotLinearMap::new(&public, &description).unwrap();
                let full = SlotLinearMap::new(&full_plan, &description).unwrap();
                COUNTS.with(|x| x.set((0, 0)));
                ADDITIONS.with(|x| x.set(0));
                TRANSFORMS.with(|x| x.set((0, 0)));
                PRIVATE_ACCESSES.with(|x| x.borrow_mut().clear());
                let result = map
                    .evaluate(&profile, &public, &input, &keys.evaluation)
                    .unwrap();
                assert_eq!(COUNTS.with(Cell::get), (public.galois_applications(), 2));
                assert_eq!(public.plaintext_products(), 2);
                assert_eq!(
                    public.galois_applications(),
                    public.baby_count() - 1 + 7 - bits
                );
                assert_eq!(ADDITIONS.with(Cell::get), 2);
                assert_eq!(public.nonempty_leaves, 2);
                assert_eq!(
                    TRANSFORMS.with(Cell::get),
                    ((2 * public.baby_count() + 2) * 3, 4 * 3)
                );
                let reads = PRIVATE_ACCESSES.with(|x| x.borrow().clone());
                if let Some(previous) = &trace {
                    assert_eq!(&reads, previous);
                } else {
                    trace = Some(reads);
                }
                let expected = full
                    .evaluate(&profile, &full_plan, &input, &keys.evaluation)
                    .unwrap();
                assert_eq!(
                    result
                        .iter()
                        .map(Ciphertext::canonical_words)
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(Ciphertext::canonical_words)
                        .collect::<Vec<_>>()
                );
                for (bank, value) in result.iter().enumerate() {
                    assert_eq!(&value.shape(), &public.output_shapes()[bank]);
                    let decoded = SlotEncoding::new(&profile, profile.master_plaintext())
                        .unwrap()
                        .decode(&keys.secret.decrypt(&profile, value).unwrap())
                        .unwrap();
                    for slot in 0..128 {
                        let scalar = if zero {
                            0
                        } else {
                            match bank {
                                0 => 0,
                                1 => 256,
                                2 => 256 * values[slot] % 257,
                                _ => values[((51 * (2 * slot + 1) % 256) - 1) / 2],
                            }
                        };
                        assert_eq!(decoded[slot], scalar);
                    }
                }
                assert_eq!(
                    public.output_shapes()[0],
                    profile
                        .plan_trivial(profile.master_plaintext(), profile.top_level())
                        .unwrap()
                );
                assert_eq!(public.output_shapes()[1], public.output_shapes()[0]);
                assert!(public.output_shapes()[2].bound() > public.output_shapes()[0].bound());
            }
        }
    }

    #[test]
    fn ntt_reuse_preserves_exact_ciphertexts_addresses_counts_and_every_split() {
        let profile = profile(3);
        let normal = plan::<1, 1>(&profile);
        let keys = key_set(&profile, &normal);
        let input = [encrypt(
            &profile,
            &keys,
            &std::array::from_fn::<_, 128, _>(|s| ((s * 91 + 256) % 257) as u64),
            37,
        )];
        let coefficients = Zeroizing::new(
            (0..normal.coefficient_count())
                .map(|i| ((i * 17 + i / 128 * 31 + 256) % 257) as u32)
                .collect::<Vec<_>>(),
        );
        for bits in 0..=7 {
            let plan = force_split(&profile, &normal, bits);
            let owner = SlotLinearMap::new(&plan, &coefficients).unwrap();
            ACCESSES.with(|x| x.borrow_mut().clear());
            COUNTS.with(|x| x.set((0, 0)));
            TRANSFORMS.with(|x| x.set((0, 0)));
            let actual = owner
                .evaluate(&profile, &plan, &input, &keys.evaluation)
                .unwrap();
            let addresses = ACCESSES.with(|x| x.borrow().clone());
            let counts = COUNTS.with(Cell::get);
            let limbs = profile.top_level() + 1;
            let transforms = (
                (2 * plan.baby_count() + plan.plaintext_products()) * limbs,
                2 * plan.giant_count() * limbs,
            );
            assert_eq!(TRANSFORMS.with(Cell::get), transforms);
            let model = CostModel::new(&profile);
            let expected_units = transforms.0 as u64 * model.forward_transform_units()
                + transforms.1 as u64 * model.inverse_transform_units()
                + 2 * plan.plaintext_products() as u64 * limbs as u64 * profile.degree() as u64
                + plan.galois_applications() as u64
                    * model.automorphism_units(profile.top_level()).unwrap();
            assert_eq!(plan.arithmetic_units(&profile).unwrap(), expected_units);
            ACCESSES.with(|x| x.borrow_mut().clear());
            COUNTS.with(|x| x.set((0, 0)));
            let expected = owner
                .evaluate_coefficient_oracle(&profile, &plan, &input, &keys.evaluation)
                .unwrap();
            assert_eq!(
                actual, expected,
                "exact residues/noise for baby bits {bits}"
            );
            assert_eq!(COUNTS.with(Cell::get), counts);
            assert_eq!(ACCESSES.with(|x| x.borrow().clone()), addresses);
        }
        assert_eq!(
            normal.arithmetic_units(&super::tests::profile(4)),
            Err(SlotLinearError::Bgv(BgvError::ProfileMismatch))
        );
    }

    #[test]
    fn transformed_owner_moves_original_allocations_and_clears_on_error_and_unwind() {
        let profile = profile(3);
        let plan = plan::<1, 1>(&profile);
        let keys = key_set(&profile, &plan);
        let original = encrypt(&profile, &keys, &[256; 128], 42);
        let expected = original.clone();
        let addresses = original
            .components()
            .iter()
            .map(|p| p.words().as_ptr())
            .collect::<Vec<_>>();
        let image = NttCiphertext::from_ciphertext(&profile, original).unwrap();
        assert_eq!(
            image
                .components
                .iter()
                .map(|p| p.words().as_ptr())
                .collect::<Vec<_>>(),
            addresses
        );
        let restored = image.into_ciphertext(&profile).unwrap();
        assert_eq!(restored, expected);
        let words = 2 * (profile.top_level() + 1) * profile.degree();
        let before = super::super::poly::tests::cleared_words();
        let image = NttCiphertext::from_ciphertext(&profile, restored).unwrap();
        drop(image);
        assert_eq!(super::super::poly::tests::cleared_words() - before, words);
        let before = super::super::poly::tests::cleared_words();
        let failing = || -> Result<(), BgvError> {
            let _image = NttCiphertext::from_ciphertext(&profile, expected.clone())?;
            Err(BgvError::Kernel)
        };
        assert!(failing().is_err());
        assert_eq!(super::super::poly::tests::cleared_words() - before, words);
        let before = super::super::poly::tests::cleared_words();
        assert!(
            std::panic::catch_unwind(|| {
                let _image = NttCiphertext::from_ciphertext(&profile, expected.clone()).unwrap();
                panic!("transformed cache unwind control");
            })
            .is_err()
        );
        assert_eq!(super::super::poly::tests::cleared_words() - before, words);
        let wrong = super::tests::profile(4);
        let image = NttCiphertext::from_ciphertext(&profile, expected.clone()).unwrap();
        let wrong_plain = Plaintext::constant(&wrong, wrong.master_plaintext(), 1).unwrap();
        assert!(matches!(
            image.multiply_plain(&wrong, &wrong_plain),
            Err(BgvError::ProfileMismatch)
        ));
        let mut another = NttCiphertext::from_ciphertext(&profile, expected.clone()).unwrap();
        assert_eq!(
            another.add_assign(&wrong, &image),
            Err(BgvError::ProfileMismatch)
        );
        assert!(matches!(
            image.into_ciphertext(&wrong),
            Err(BgvError::ProfileMismatch)
        ));
        drop(another);
        let triple = profile.tensor(&expected, &expected).unwrap();
        let before = super::super::poly::tests::cleared_words();
        assert!(matches!(
            NttCiphertext::from_ciphertext(&profile, triple),
            Err(BgvError::ComponentCount { found: 3 })
        ));
        assert_eq!(
            super::super::poly::tests::cleared_words() - before,
            words / 2 * 3
        );
    }
    #[test]
    fn prepared_map_and_ntt_owner_refuse_cross_precision_before_private_reads() {
        let profile =
            BgvProfile::new(BgvParameters::derive(3, 5, 3, (50, 4), (60, 2), 2).unwrap()).unwrap();
        let low = profile.plaintext_precision(1).unwrap();
        let high = profile.plaintext_precision(2).unwrap();
        let low_plan = SlotLinearPlan::<1, 1>::new(
            &profile,
            &[profile.plan_fresh(low).unwrap()],
            budget::<1, 1>(&profile),
        )
        .unwrap();
        let high_plan = SlotLinearPlan::<1, 1>::new(
            &profile,
            &[profile.plan_fresh(high).unwrap()],
            budget::<1, 1>(&profile),
        )
        .unwrap();
        let owner = SlotLinearMap::new(&low_plan, &[1, 0, 0, 1, 0, 0]).unwrap();
        let mut entropy = FixtureEntropy::new([109; 64]);
        let keys = fixtures::key_set(&profile, high_plan.galois_elements(), &mut entropy).unwrap();
        let plaintext = Plaintext::constant(&profile, high, 1).unwrap();
        let high_input =
            fixtures::encrypt(&profile, &keys.public, &plaintext, &mut entropy).unwrap();
        ACCESSES.with(|x| x.borrow_mut().clear());
        PRIVATE_ACCESSES.with(|x| x.borrow_mut().clear());
        assert_eq!(
            owner
                .evaluate(&profile, &high_plan, &[high_input], &keys.evaluation)
                .err(),
            Some(SlotLinearError::Bgv(BgvError::PlaintextPrecisionMismatch))
        );
        assert!(ACCESSES.with(|x| x.borrow().is_empty()));
        assert!(PRIVATE_ACCESSES.with(|x| x.borrow().is_empty()));
        let low_plain = Plaintext::constant(&profile, low, 1).unwrap();
        let low_input =
            fixtures::encrypt(&profile, &keys.public, &low_plain, &mut entropy).unwrap();
        let ntt = NttCiphertext::from_ciphertext(&profile, low_input).unwrap();
        assert_eq!(
            ntt.multiply_plain(&profile, &plaintext).err(),
            Some(BgvError::PlaintextPrecisionMismatch)
        );
    }
    #[test]
    fn public_metadata_covers_retained_capacities_and_all_split_search_owners() {
        let profile = profile(4);
        let precision = profile.master_plaintext();
        let requirement =
            SlotLinearPlan::<2, 3>::metadata_requirement(&profile, precision).unwrap();
        let plan = plan::<2, 3>(&profile);
        let observed = plan.retained_metadata().unwrap();
        assert_eq!(observed.inline_bytes, size_of::<SlotLinearPlan<2, 3>>());
        assert!(observed.retained_heap_bytes <= requirement.retained_heap_bytes);
        assert_eq!(observed.preparation_scratch_bytes, 0);
        let shared = plan.layout().clone();
        assert_eq!(plan.retained_metadata().unwrap(), observed);
        assert_eq!(shared.metadata_bytes(), plan.layout_metadata_bytes());
        // Hold two real candidates and a maximum-size public shape cache at
        // once, like the planner's best/current search boundary. Their actual
        // vector and Natural capacities must fit the separately admitted peak.
        let inputs = std::array::from_fn(|_| {
            profile
                .plan_trivial(precision, profile.top_level())
                .unwrap()
        });
        let encoding = SlotEncoding::new(&profile, precision).unwrap();
        let mut best = None;
        for bits in 0..=7 {
            let current = plan_split(&profile, &encoding, &inputs, plan.layout(), bits).unwrap();
            let cache = vec![inputs[0].clone(); 2 * (1 << bits)];
            let heap = |shapes: &[CiphertextShape]| {
                shapes
                    .iter()
                    .map(|x| x.bound().allocated_words() * size_of::<u64>())
                    .sum::<usize>()
            };
            let current_bytes = size_of::<SplitTransfer<3>>() + heap(&current.outputs);
            let best_bytes = best.as_ref().map_or(0, |value: &SplitTransfer<3>| {
                size_of::<SplitTransfer<3>>() + heap(&value.outputs)
            });
            let cache_bytes = size_of::<Vec<CiphertextShape>>()
                + cache.capacity() * size_of::<CiphertextShape>()
                + heap(&cache);
            assert!(
                current_bytes + best_bytes + cache_bytes <= requirement.preparation_scratch_bytes
            );
            best = Some(current);
        }
        let near = profile
            .plan_claimed(
                precision,
                profile.top_level(),
                2,
                profile.capacity(profile.top_level()).unwrap().clone(),
            )
            .unwrap();
        let near_inputs = [near.clone(), near];
        assert!(plan_split(&profile, &encoding, &near_inputs, plan.layout(), 0).is_err());
        let other = self::profile(3);
        assert!(matches!(
            SlotLinearPlan::<2, 3>::metadata_requirement(&profile, other.master_plaintext()),
            Err(super::super::PlanMetadataError::Bgv(
                BgvError::ProfileMismatch
            ))
        ));
        assert!(matches!(
            SlotLinearPlan::<0, 1>::metadata_requirement(&profile, precision),
            Err(super::super::PlanMetadataError::Context)
        ));
    }
}
