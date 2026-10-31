//! Complete full-ring coefficient extraction into scalar slots and its inverse.
//!
//! The sole parity executor first produces closed raw banks `d*g_r(X^d)`,
//! where `d=N/S`. Its unit initial product avoids modular inverse amplification.
//! A shared inverse Vandermonde with coefficient `N^-1*beta_j^-k` places
//! original coefficient `a[r+d*k]` in slot `k` of bank `r`. The forward map
//! and existing normalized recomposition reverse this. No noise bound is divided.
//! Every bank is processed and every public matrix coordinate is evaluated.
//!
//! The private bank owner records this actual transform origin, conditional on
//! ordinary ciphertext/key correctness. It authenticates no input, does not
//! certify an imported noise claim. Its concrete immutable upper-digit executor
//! alone may consume it for matched scalar-tableau precision reductions.
//! The separate local bootstrap supplies the honest encrypted-secret relation
//! and complete refresh composition. TODO(C.8, R.9): external proofs, circuit
//! privacy and complete parameter/resource qualification remain open.

use core::{fmt, mem::size_of};

use thiserror::Error;

use super::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, EvaluationKeys, RelativeTraceBudget,
    RelativeTraceError, RelativeTraceRecompositionPlan, SlotEncoding, SlotLinearBudget,
    SlotLinearError, SlotLinearMap, SlotLinearPlan, SwitchingKeyKind,
    cost::CostModel,
    relative_trace::{self, BankRetention, CoefficientTraceCursor, CoefficientTracePlan},
};

/// Explicit payload ceilings for one prepared transform direction.
///
/// The routing budget covers its matrix, layout, preparation row and local
/// residues. The wrapper residue ceiling additionally counts all owned source
/// banks and the consumed original input. Backend RNS/NTT/key-switch and routing
/// plaintext scratch, keys, public plan/container/shape metadata (including the
/// h+1 traversal frames and moved-owner collection), allocator overhead and
/// RSS are separate. The local bootstrap separately caps its new stream metadata. Two simultaneously retained directions count both owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoefficientSlotBudget {
    /// Existing packed-map preparation and local execution ceilings.
    pub routing: SlotLinearBudget,
    /// Peak ciphertext residue payload owned by the complete wrapper.
    pub owned_residue_bytes: usize,
    /// One relative-trace monomial coefficient buffer, excluding backend scratch.
    pub trace_plaintext_bytes: usize,
}

/// Scoped admitted payloads; these are not a whole-process working-set limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoefficientSlotPayload {
    /// Exact prepared matrix/layout/row and bounded local routing residues.
    pub routing: SlotLinearBudget,
    /// Conservative peak of ciphertext residues owned by this wrapper.
    pub owned_residue_bytes: usize,
    /// Relative-trace monomial coefficient payload.
    pub trace_plaintext_bytes: usize,
}

/// Public operation counts, independent of all coefficient values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoefficientSlotOperations {
    /// Plaintext products, including every dense routing diagonal.
    pub plaintext_products: usize,
    /// Ciphertext additions and subtractions.
    pub ciphertext_additions: usize,
    /// Key-switched Galois applications.
    pub galois_applications: usize,
    /// Shared CostModel units; excludes additions, clearing and allocation.
    pub arithmetic_units: u64,
}

/// A coefficient-transform preparation or execution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CoefficientSlotError {
    /// Public geometry or work arithmetic overflowed.
    #[error("coefficient-slot public geometry overflow")]
    SizeOverflow,
    /// The complete wrapper residue peak exceeds its explicit ceiling.
    #[error("coefficient-slot residues require {required} bytes, budget {budget}")]
    ResidueBudget {
        /// Required bounded payload.
        required: usize,
        /// Caller ceiling.
        budget: usize,
    },
    /// The monomial coefficient buffer exceeds its separate ceiling.
    #[error("coefficient-slot trace plaintext requires {required} bytes, budget {budget}")]
    PlaintextBudget {
        /// Required coefficient payload.
        required: usize,
        /// Caller ceiling.
        budget: usize,
    },
    /// Input geometry or exact public shape differs from the prepared transform.
    #[error("coefficient-slot input differs from its prepared shape")]
    InputShape,
    /// A public-sized output container could not be allocated.
    #[error("coefficient-slot allocation failed")]
    Allocation,
    /// Existing BGV admission or kernel failure.
    #[error(transparent)]
    Bgv(#[from] BgvError),
    /// Existing shared routing admission or execution failure.
    #[error(transparent)]
    Routing(#[from] SlotLinearError),
    /// Existing relative-trace admission or execution failure.
    #[error(transparent)]
    Trace(#[from] RelativeTraceError),
}

/// Ordered scalar banks with actual coefficient-transform origin.
///
/// Decomposition initializes bank `r`, slot `j` with original coefficient
/// `a[r+(N/S)*j]`. The concrete upper-digit executor preserves that coordinate
/// order while replacing each value and its effective precision. Only the actual
/// complete decomposition and its concrete upper-digit executor construct this
/// non-Clone owner; no ciphertext-vector,
/// byte-frame or claimed-shape constructor exists. Borrowed ciphertext access
/// does not provide a way to construct another owner. Residues clear on drop.
/// This structural origin is conditional on ordinary ciphertext/key correctness,
/// and is not authentication, an imported-noise proof or generic division authority.
pub struct CoefficientSlotBanks {
    values: Vec<Ciphertext>,
    shape: CiphertextShape,
    slots: usize,
}

impl fmt::Debug for CoefficientSlotBanks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED coefficient-slot banks]")
    }
}

impl CoefficientSlotBanks {
    // The sole successor operation is a concrete immutable compiler/executor,
    // not a callback or a public from-parts/scalar-provenance constructor.
    pub(super) fn evaluate_upper_digits(
        self,
        profile: &BgvProfile,
        plan: &super::upper_digits::EncryptedUpperDigitsPlan,
        key: &super::SwitchingKey,
    ) -> Result<Self, super::upper_digits::UpperDigitsError> {
        plan.preflight(profile, &self, key)?;
        let banks = self.values.len();
        let mut values = super::upper_digits::reserve(banks)?;
        for (ordinal, input) in self.values.into_iter().enumerate() {
            let bank = CoefficientSlotBank {
                value: input,
                ordinal,
                slots: self.slots,
                banks,
            };
            values.push(bank.upper(profile, plan, key)?.value);
        }
        Ok(Self {
            values,
            shape: plan.output_shape().clone(),
            slots: self.slots,
        })
    }

    /// Borrow every bank in fixed original residue-class order.
    #[must_use]
    pub fn banks(&self) -> &[Ciphertext] {
        &self.values
    }

    /// Exact common public ciphertext shape, including effective precision.
    #[must_use]
    pub const fn shape(&self) -> &CiphertextShape {
        &self.shape
    }

    /// Number of scalar coefficients in each bank.
    #[must_use]
    pub const fn slots(&self) -> usize {
        self.slots
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatrixDirection {
    ExtractCoefficients,
    RestoreEvaluations,
}

#[derive(Debug)]
struct Matrix {
    plan: SlotLinearPlan<1, 1>,
    map: SlotLinearMap<1, 1>,
}

impl Matrix {
    fn prepare(
        profile: &BgvProfile,
        plan: SlotLinearPlan<1, 1>,
        precision: super::PlaintextPrecision,
        direction: MatrixDirection,
    ) -> Result<Self, CoefficientSlotError> {
        let encoding = SlotEncoding::new(profile, precision)?;
        let fixed = precision.fixed();
        let slots = encoding.slots();
        let order = (slots as u64)
            .checked_mul(2)
            .ok_or(CoefficientSlotError::SizeOverflow)?;
        let inverse_root = fixed.power(encoding.root(), order - 1);
        let phi = precision.modulus() - precision.modulus() / precision.prime();
        let degree_residue = fixed.reduce_u64(profile.degree() as u64);
        let inverse_degree = fixed.power(degree_residue, phi - 1);
        if fixed.multiply(degree_residue, inverse_degree) != 1 {
            return Err(BgvError::Kernel.into());
        }
        let map = SlotLinearMap::prepare_rows(&plan, |row, values| {
            // Both branches are selected only by this prepared public direction.
            // Raw trace is d*g; A[row,j]=N^-1*zeta^(-(2j+1)*row) cancels
            // d*S=N. The reverse V[row,j]=zeta^((2row+1)*j) is unchanged.
            let (mut coefficient, factor) =
                if matches!(direction, MatrixDirection::ExtractCoefficients) {
                    let root_power = fixed.power(inverse_root, row as u64);
                    (
                        fixed.multiply(inverse_degree, root_power),
                        fixed.multiply(root_power, root_power),
                    )
                } else {
                    (1, fixed.power(encoding.root(), 2 * row as u64 + 1))
                };
            for to in &mut values[..slots] {
                *to = coefficient as u32; // Validated p^k < 2^31.
                coefficient = fixed.multiply(coefficient, factor);
            }
            // The final bias remains the initializer's canonical zero.
            Ok(())
        })?;
        Ok(Self { plan, map })
    }

    fn move_one(
        &self,
        profile: &BgvProfile,
        input: Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<Ciphertext, CoefficientSlotError> {
        let one = [input];
        let mut routed = self.map.evaluate(profile, &self.plan, &one, keys)?;
        drop(one);
        let value = routed.pop().ok_or(BgvError::Kernel)?;
        #[cfg(test)]
        tests::after_bank()?;
        Ok(value)
    }

    fn move_banks(
        &self,
        profile: &BgvProfile,
        inputs: Vec<Ciphertext>,
        keys: &EvaluationKeys,
    ) -> Result<Vec<Ciphertext>, CoefficientSlotError> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(inputs.len())
            .map_err(|_| CoefficientSlotError::Allocation)?;
        for input in inputs {
            output.push(self.move_one(profile, input, keys)?);
        }
        Ok(output)
    }
}

fn checked_mul(a: usize, b: usize) -> Result<usize, CoefficientSlotError> {
    a.checked_mul(b).ok_or(CoefficientSlotError::SizeOverflow)
}

fn trace_budget(
    profile: &BgvProfile,
    shape: &CiphertextShape,
    banks: usize,
    budget: CoefficientSlotBudget,
    retention: BankRetention,
) -> Result<RelativeTraceBudget, CoefficientSlotError> {
    let one = usize::try_from(CostModel::new(profile).ciphertext_residue_bytes(shape.level(), 2)?)
        .map_err(|_| CoefficientSlotError::SizeOverflow)?;
    let plaintext = checked_mul(profile.degree(), size_of::<u64>())?;
    if plaintext > budget.trace_plaintext_bytes {
        return Err(CoefficientSlotError::PlaintextBudget {
            required: plaintext,
            budget: budget.trace_plaintext_bytes,
        });
    }
    // Streaming admits the bounded DFS frontier rather than an unallocated d bank set.
    let retained = if retention == BankRetention::Collected {
        banks
    } else {
        banks.ilog2() as usize
    };
    let trace_peak = checked_mul(
        retained
            .checked_add(3)
            .ok_or(CoefficientSlotError::SizeOverflow)?,
        one,
    )?;
    if trace_peak > budget.owned_residue_bytes {
        return Err(CoefficientSlotError::ResidueBudget {
            required: trace_peak,
            budget: budget.owned_residue_bytes,
        });
    }
    Ok(RelativeTraceBudget {
        output_residue_bytes: checked_mul(
            if retention == BankRetention::Collected {
                banks
            } else {
                1
            },
            one,
        )?,
        scratch_residue_bytes: checked_mul(
            if retention == BankRetention::Collected {
                2
            } else {
                banks.ilog2() as usize + 2
            },
            one,
        )?,
        plaintext_bytes: plaintext,
    })
}

fn payload(
    profile: &BgvProfile,
    route: &SlotLinearPlan<1, 1>,
    banks: usize,
    budget: CoefficientSlotBudget,
    retention: BankRetention,
) -> Result<CoefficientSlotPayload, CoefficientSlotError> {
    let one = usize::try_from(
        CostModel::new(profile).ciphertext_residue_bytes(route.output_shapes()[0].level(), 2)?,
    )
    .map_err(|_| CoefficientSlotError::SizeOverflow)?;
    let retained = if retention == BankRetention::Collected {
        banks
    } else {
        banks.ilog2() as usize
    };
    let trace = checked_mul(
        retained
            .checked_add(3)
            .ok_or(CoefficientSlotError::SizeOverflow)?,
        one,
    )?;
    let routing = checked_mul(
        retained
            .checked_add(if retention == BankRetention::Collected {
                1
            } else {
                2
            })
            .ok_or(CoefficientSlotError::SizeOverflow)?,
        one,
    )?
    .checked_add(route.scratch_residue_bytes())
    .ok_or(CoefficientSlotError::SizeOverflow)?;
    let owned_residue_bytes = trace.max(routing);
    if owned_residue_bytes > budget.owned_residue_bytes {
        return Err(CoefficientSlotError::ResidueBudget {
            required: owned_residue_bytes,
            budget: budget.owned_residue_bytes,
        });
    }
    Ok(CoefficientSlotPayload {
        routing: SlotLinearBudget {
            coefficient_bytes: route.coefficient_bytes(),
            layout_metadata_bytes: route.layout_metadata_bytes(),
            preparation_scratch_bytes: route.preparation_scratch_bytes(),
            output_residue_bytes: route.output_residue_bytes(),
            scratch_residue_bytes: route.scratch_residue_bytes(),
        },
        owned_residue_bytes,
        trace_plaintext_bytes: checked_mul(profile.degree(), size_of::<u64>())?,
    })
}

fn operations(
    profile: &BgvProfile,
    route: &SlotLinearPlan<1, 1>,
    banks: usize,
    trace: super::RelativeTraceOperations,
) -> Result<CoefficientSlotOperations, CoefficientSlotError> {
    let add = |base: usize, per_bank: usize| {
        checked_mul(banks, per_bank)?
            .checked_add(base)
            .ok_or(CoefficientSlotError::SizeOverflow)
    };
    let arithmetic_units = route
        .arithmetic_units(profile)?
        .checked_mul(u64::try_from(banks).map_err(|_| CoefficientSlotError::SizeOverflow)?)
        .and_then(|x| x.checked_add(trace.arithmetic_units))
        .ok_or(CoefficientSlotError::SizeOverflow)?;
    Ok(CoefficientSlotOperations {
        plaintext_products: add(trace.plaintext_products, route.plaintext_products())?,
        ciphertext_additions: add(trace.ciphertext_additions, route.plaintext_products())?,
        galois_applications: add(trace.galois_applications, route.galois_applications())?,
        arithmetic_units,
    })
}

fn key_union(first: &[u32], second: &[u32]) -> Result<Vec<u32>, CoefficientSlotError> {
    let size = first
        .len()
        .checked_add(second.len())
        .ok_or(CoefficientSlotError::SizeOverflow)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| CoefficientSlotError::Allocation)?;
    result.extend_from_slice(first);
    result.extend_from_slice(second);
    result.sort_unstable();
    result.dedup();
    Ok(result)
}

fn check_keys(
    profile: &BgvProfile,
    keys: &EvaluationKeys,
    elements: &[u32],
) -> Result<(), CoefficientSlotError> {
    for &element in elements {
        let key = keys.galois(element).ok_or(BgvError::WrongSwitchingKey)?;
        if key.profile_identity() != profile.identity() {
            return Err(BgvError::ProfileMismatch.into());
        }
        if key.kind() != SwitchingKeyKind::Galois(element) {
            return Err(BgvError::WrongSwitchingKey.into());
        }
    }
    Ok(())
}

/// Prepared complete full-ring decomposition into coefficient-valued scalar slots.
#[derive(Debug)]
pub struct CoefficientSlotDecomposition {
    input: CiphertextShape,
    trace: CoefficientTracePlan,
    matrix: Matrix,
    keys: Vec<u32>,
    payload: CoefficientSlotPayload,
    operations: CoefficientSlotOperations,
    retention: BankRetention,
}

impl CoefficientSlotDecomposition {
    pub(super) fn metadata_heap(&self) -> Result<usize, super::PlanMetadataError> {
        super::metadata::sum(&[
            super::metadata::shape_heap(&self.input)?,
            self.trace.metadata_heap()?,
            self.matrix.plan.retained_metadata()?.retained_heap_bytes,
            super::metadata::mul(self.keys.capacity(), size_of::<u32>())?,
        ])
    }

    /// Preflight exact noise and explicit payload scopes, then prepare one public matrix.
    ///
    /// # Errors
    /// Unsupported encoding, input shape, noise or payload refusal, or allocation failure.
    pub fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: CoefficientSlotBudget,
    ) -> Result<Self, CoefficientSlotError> {
        Self::prepare(profile, input, budget, BankRetention::Collected)
    }

    pub(super) fn prepare(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: CoefficientSlotBudget,
        retention: BankRetention,
    ) -> Result<Self, CoefficientSlotError> {
        input.check_profile(profile)?;
        let encoding = SlotEncoding::new(profile, input.plaintext_precision())?;
        let banks = profile.degree() / encoding.slots();
        let trace = CoefficientTracePlan::new(
            profile,
            input,
            trace_budget(profile, input, banks, budget, retention)?,
            retention,
        )?;
        let route = SlotLinearPlan::new(profile, &[trace.output_shape().clone()], budget.routing)?;
        let payload = payload(profile, &route, banks, budget, retention)?;
        let operations = operations(profile, &route, banks, trace.operations())?;
        let keys = key_union(trace.galois_elements(), route.galois_elements())?;
        let matrix = Matrix::prepare(
            profile,
            route,
            input.plaintext_precision(),
            MatrixDirection::ExtractCoefficients,
        )?;
        Ok(Self {
            input: input.clone(),
            trace,
            matrix,
            keys,
            payload,
            operations,
            retention,
        })
    }

    /// Exact shape of every returned coefficient bank.
    #[must_use]
    pub fn output_shape(&self) -> &CiphertextShape {
        &self.matrix.plan.output_shapes()[0]
    }
    /// Complete required Galois-key union, sorted and unique.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        &self.keys
    }
    /// Number of returned banks; every original coefficient is represented.
    #[must_use]
    pub fn banks(&self) -> usize {
        self.trace.banks()
    }
    /// Scalar slots per returned bank.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.trace.slots()
    }
    /// Scoped admitted payloads.
    #[must_use]
    pub const fn payload(&self) -> CoefficientSlotPayload {
        self.payload
    }
    /// Exact high-level work and shared arithmetic units.
    #[must_use]
    pub const fn operations(&self) -> CoefficientSlotOperations {
        self.operations
    }

    /// Consume the full-ring input and execute all trace branches and matrix rows.
    ///
    /// All public context/shape/key checks precede private arithmetic. Failure or
    /// unwind clears the consumed input and all partial banks. No input is copied
    /// as a second retained full bank set.
    ///
    /// # Errors
    /// Profile/shape/key mismatch, allocation failure or an existing kernel refusal.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        input: Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<CoefficientSlotBanks, CoefficientSlotError> {
        if self.retention != BankRetention::Collected {
            return Err(CoefficientSlotError::InputShape);
        }
        self.input.check_profile(profile)?;
        input.check_profile(profile)?;
        if input.shape() != self.input {
            return Err(CoefficientSlotError::InputShape);
        }
        check_keys(profile, keys, &self.keys)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.banks())
            .map_err(|_| CoefficientSlotError::Allocation)?;
        let mut cursor = self.start(profile, input, keys)?;
        while let Some((bank, rest)) = cursor.advance(profile, keys)? {
            values.push(bank.value);
            cursor = rest;
        }
        relative_trace::natural_order(&mut values);
        Ok(CoefficientSlotBanks {
            values,
            shape: self.output_shape().clone(),
            slots: self.slots(),
        })
    }
}

/// Prepared inverse map and full-ring recomposition of a complete bank owner.
#[derive(Debug)]
pub struct CoefficientSlotRecomposition {
    input: CiphertextShape,
    slots: usize,
    banks: usize,
    matrix: Matrix,
    trace: RelativeTraceRecompositionPlan,
    payload: CoefficientSlotPayload,
    operations: CoefficientSlotOperations,
    retention: BankRetention,
}

impl CoefficientSlotRecomposition {
    pub(super) fn metadata_heap(&self) -> Result<usize, super::PlanMetadataError> {
        super::metadata::sum(&[
            super::metadata::shape_heap(&self.input)?,
            self.trace.metadata_heap()?,
            self.matrix.plan.retained_metadata()?.retained_heap_bytes,
        ])
    }

    /// Prepare the inverse for banks with one exact common public shape.
    ///
    /// This shape creates no bank owner or scalar/division authority. Only actual
    /// decomposition can supply the consuming evaluation input.
    ///
    /// # Errors
    /// Unsupported encoding, input shape/noise/payload refusal, or allocation failure.
    pub fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: CoefficientSlotBudget,
    ) -> Result<Self, CoefficientSlotError> {
        Self::prepare(profile, input, budget, BankRetention::Collected)
    }

    pub(super) fn prepare(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: CoefficientSlotBudget,
        retention: BankRetention,
    ) -> Result<Self, CoefficientSlotError> {
        input.check_profile(profile)?;
        let encoding = SlotEncoding::new(profile, input.plaintext_precision())?;
        let slots = encoding.slots();
        let banks = profile.degree() / slots;
        let trace_budget = trace_budget(profile, input, banks, budget, retention)?;
        let route = SlotLinearPlan::new(profile, &[input.clone()], budget.routing)?;
        let payload = payload(profile, &route, banks, budget, retention)?;
        let mut shapes = Vec::new();
        shapes
            .try_reserve_exact(banks)
            .map_err(|_| CoefficientSlotError::Allocation)?;
        shapes.resize(banks, route.output_shapes()[0].clone());
        let trace = RelativeTraceRecompositionPlan::new(profile, &shapes, trace_budget)?;
        let operations = operations(profile, &route, banks, trace.operations())?;
        let matrix = Matrix::prepare(
            profile,
            route,
            input.plaintext_precision(),
            MatrixDirection::RestoreEvaluations,
        )?;
        Ok(Self {
            input: input.clone(),
            slots,
            banks,
            matrix,
            trace,
            payload,
            operations,
            retention,
        })
    }

    /// Exact public shape of the recomposed full-ring ciphertext.
    #[must_use]
    pub fn output_shape(&self) -> &CiphertextShape {
        self.trace.output_shape()
    }
    /// Complete required Galois-key set for this direction.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        self.matrix.plan.galois_elements()
    }
    /// Scoped admitted payloads.
    #[must_use]
    pub const fn payload(&self) -> CoefficientSlotPayload {
        self.payload
    }
    /// Exact high-level work and shared arithmetic units.
    #[must_use]
    pub const fn operations(&self) -> CoefficientSlotOperations {
        self.operations
    }

    /// Consume every ordered coefficient bank and reconstruct the full-ring value.
    ///
    /// Failure or unwind clears both remaining inputs and completed intermediates.
    /// Recomposition returns an ordinary ciphertext, with no scalar membership claim.
    ///
    /// # Errors
    /// Profile/precision/shape/key mismatch, allocation or existing kernel failure.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        input: CoefficientSlotBanks,
        keys: &EvaluationKeys,
    ) -> Result<Ciphertext, CoefficientSlotError> {
        if self.retention != BankRetention::Collected {
            return Err(CoefficientSlotError::InputShape);
        }
        self.input.check_profile(profile)?;
        if input.slots != self.slots
            || input.values.len() != self.banks
            || input.shape != self.input
        {
            return Err(CoefficientSlotError::InputShape);
        }
        for bank in &input.values {
            bank.check_profile(profile)?;
            if bank.shape() != self.input {
                return Err(CoefficientSlotError::InputShape);
            }
        }
        check_keys(profile, keys, self.galois_elements())?;
        let restored = self.matrix.move_banks(profile, input.values, keys)?;
        let result = self.trace.evaluate(profile, &restored)?;
        drop(restored);
        Ok(result)
    }
}

/// One actual coefficient map result. Geometry/ordinal survives the nonlinear step.
pub(super) struct CoefficientSlotBank {
    value: Ciphertext,
    ordinal: usize,
    slots: usize,
    banks: usize,
}
impl CoefficientSlotBank {
    #[cfg(test)]
    pub(super) fn value(&self) -> &Ciphertext {
        &self.value
    }
    #[cfg(test)]
    pub(super) fn ordinal(&self) -> usize {
        self.ordinal
    }
    #[cfg(test)]
    pub(super) fn banks(&self) -> usize {
        self.banks
    }
    pub(super) fn upper(
        self,
        profile: &BgvProfile,
        plan: &super::upper_digits::EncryptedUpperDigitsPlan,
        key: &super::SwitchingKey,
    ) -> Result<Self, super::upper_digits::UpperDigitsError> {
        plan.preflight_bank(profile, &self.value.shape(), self.slots, self.banks, key)?;
        let value = plan.execute_bank(profile, self.value, key)?;
        Ok(Self {
            value,
            ordinal: self.ordinal,
            slots: self.slots,
            banks: self.banks,
        })
    }
}

pub(super) struct CoefficientSlotCursor<'plan> {
    plan: &'plan CoefficientSlotDecomposition,
    trace: CoefficientTraceCursor<'plan>,
}
impl CoefficientSlotDecomposition {
    pub(super) fn start<'a>(
        &'a self,
        profile: &BgvProfile,
        input: Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<CoefficientSlotCursor<'a>, CoefficientSlotError> {
        self.input.check_profile(profile)?;
        input.check_profile(profile)?;
        if input.shape() != self.input {
            return Err(CoefficientSlotError::InputShape);
        }
        check_keys(profile, keys, self.galois_elements())?;
        let trace = self.trace.start(profile, input, keys)?;
        Ok(CoefficientSlotCursor { plan: self, trace })
    }
}
impl CoefficientSlotCursor<'_> {
    pub(super) fn advance(
        self,
        profile: &BgvProfile,
        keys: &EvaluationKeys,
    ) -> Result<Option<(CoefficientSlotBank, Self)>, CoefficientSlotError> {
        let plan = self.plan;
        match self.trace.advance(profile, keys)? {
            None => Ok(None),
            Some((raw, trace)) => {
                let (input, ordinal) = raw.into_value(
                    profile,
                    plan.trace.output_shape(),
                    plan.slots(),
                    plan.banks(),
                )?;
                let value = plan.matrix.move_one(profile, input, keys)?;
                if value.shape() != *plan.output_shape() {
                    return Err(CoefficientSlotError::InputShape);
                }
                Ok(Some((
                    CoefficientSlotBank {
                        value,
                        ordinal,
                        slots: plan.slots(),
                        banks: plan.banks(),
                    },
                    Self { plan, trace },
                )))
            }
        }
    }
}

pub(super) struct CoefficientSlotAccumulator<'plan> {
    plan: &'plan CoefficientSlotRecomposition,
    value: Option<Ciphertext>,
    completed: usize,
}
impl CoefficientSlotRecomposition {
    pub(super) fn start_accumulator<'a>(
        &'a self,
        profile: &BgvProfile,
        keys: &EvaluationKeys,
    ) -> Result<CoefficientSlotAccumulator<'a>, CoefficientSlotError> {
        self.input.check_profile(profile)?;
        check_keys(profile, keys, self.galois_elements())?;
        Ok(CoefficientSlotAccumulator {
            plan: self,
            value: None,
            completed: 0,
        })
    }
}
impl CoefficientSlotAccumulator<'_> {
    pub(super) fn push(
        mut self,
        profile: &BgvProfile,
        bank: CoefficientSlotBank,
        keys: &EvaluationKeys,
    ) -> Result<Self, CoefficientSlotError> {
        let bits = self.plan.banks.ilog2();
        let ordinal = if bits == 0 {
            0
        } else {
            self.completed.reverse_bits() >> (usize::BITS - bits)
        };
        bank.value.check_profile(profile)?;
        if self.completed >= self.plan.banks
            || bank.ordinal != ordinal
            || bank.slots != self.plan.slots
            || bank.banks != self.plan.banks
            || bank.value.shape() != self.plan.input
        {
            return Err(CoefficientSlotError::InputShape);
        }
        let restored = self.plan.matrix.move_one(profile, bank.value, keys)?;
        let product = relative_trace::monomial_product(profile, &restored, ordinal)?;
        drop(restored);
        self.value = Some(relative_trace::accumulate(
            profile,
            self.value.take(),
            product,
        )?);
        self.completed += 1;
        Ok(self)
    }
    pub(super) fn finish(self) -> Result<Ciphertext, CoefficientSlotError> {
        if self.completed != self.plan.banks {
            return Err(CoefficientSlotError::InputShape);
        }
        let value = self.value.ok_or(CoefficientSlotError::InputShape)?;
        if value.shape() != *self.plan.output_shape() {
            return Err(CoefficientSlotError::InputShape);
        }
        Ok(value)
    }
}

/// Newly retained stream metadata only. Existing map/program metadata, backend
/// scratch and existing prepared public plans retain their separate scopes.
pub(super) fn stream_metadata_bytes(
    profile: &BgvProfile,
    banks: usize,
) -> Result<usize, CoefficientSlotError> {
    if banks == 0 || !banks.is_power_of_two() {
        return Err(CoefficientSlotError::InputShape);
    }
    let add = |a: usize, b: usize| a.checked_add(b).ok_or(CoefficientSlotError::SizeOverflow);
    let bound = checked_mul(
        add(checked_mul(2, profile.noise_bound_words())?, 1)?,
        size_of::<u64>(),
    )?;
    let header = add(
        size_of::<Ciphertext>(),
        add(2 * size_of::<super::poly::RnsPoly>(), bound)?,
    )?;
    // Twelve further owners conservatively cover live split parents/children,
    // current raw/scalar/restored/product/sum wrappers and temporary shapes.
    // Routing/upper internal arena metadata remains in its canonical scope.
    let temporaries = add(checked_mul(12, header)?, checked_mul(24, bound)?)?;
    let wrappers = add(
        size_of::<CoefficientSlotCursor<'_>>(),
        add(
            size_of::<CoefficientSlotBank>(),
            size_of::<CoefficientSlotAccumulator<'_>>(),
        )?,
    )?;
    add(
        relative_trace::cursor_metadata_bytes(profile, banks)?,
        add(temporaries, wrappers)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bgv::{
            BgvParameters, KeySet, Plaintext, RelativeTraceDecompositionPlan, SecretKey,
            fixtures::{self, FixtureEntropy},
            poly,
        },
        natural::Natural,
    };
    use num_bigint::{BigInt, Sign};
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    std::thread_local! {
        static BANK_CALLS: Cell<usize> = const { Cell::new(0) };
        static FAULT: Cell<(usize,u8)> = const { Cell::new((usize::MAX,0)) };
        static CLEAR_AT_FAULT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn after_bank() -> Result<(), CoefficientSlotError> {
        let calls = BANK_CALLS.with(|x| {
            let n = x.get() + 1;
            x.set(n);
            n
        });
        let (at, mode) = FAULT.with(Cell::get);
        if at != calls || mode == 0 {
            return Ok(());
        }
        FAULT.with(|x| x.set((usize::MAX, 0)));
        CLEAR_AT_FAULT.with(|x| x.set(poly::tests::cleared_words()));
        if mode == 2 {
            panic!("coefficient bank unwind control");
        }
        Err(BgvError::Kernel.into())
    }

    fn reset() {
        BANK_CALLS.with(|x| x.set(0));
        FAULT.with(|x| x.set((usize::MAX, 0)));
    }
    fn make_profile(log_n: u32, prime: u64, exponent: u32) -> BgvProfile {
        BgvProfile::new(BgvParameters::derive(log_n, prime, exponent, (50, 4), (60, 2), 2).unwrap())
            .unwrap()
    }
    fn budget() -> CoefficientSlotBudget {
        CoefficientSlotBudget {
            routing: SlotLinearBudget {
                coefficient_bytes: 1 << 20,
                layout_metadata_bytes: 1 << 20,
                preparation_scratch_bytes: 1 << 20,
                output_residue_bytes: 1 << 24,
                scratch_residue_bytes: 1 << 26,
            },
            owned_residue_bytes: 1 << 28,
            trace_plaintext_bytes: 1 << 20,
        }
    }
    fn exact_budget(payload: CoefficientSlotPayload) -> CoefficientSlotBudget {
        CoefficientSlotBudget {
            routing: payload.routing,
            owned_residue_bytes: payload.owned_residue_bytes,
            trace_plaintext_bytes: payload.trace_plaintext_bytes,
        }
    }
    fn keys(profile: &BgvProfile, elements: &[u32]) -> KeySet {
        fixtures::key_set(profile, elements, &mut FixtureEntropy::new([143; 64])).unwrap()
    }
    fn encrypt(
        profile: &BgvProfile,
        precision: super::super::PlaintextPrecision,
        keys: &KeySet,
        values: Vec<u64>,
    ) -> Ciphertext {
        fixtures::encrypt(
            profile,
            &keys.public,
            &Plaintext::new(profile, precision, values).unwrap(),
            &mut FixtureEntropy::new([147; 64]),
        )
        .unwrap()
    }
    fn rem(x: BigInt, m: &BigInt) -> BigInt {
        let x = x % m;
        if x.sign() == Sign::Minus { x + m } else { x }
    }
    fn inverse(x: &BigInt, m: &BigInt) -> BigInt {
        let (mut a, mut b) = (x.clone(), m.clone());
        let (mut u, mut v) = (BigInt::from(1), BigInt::from(0));
        while b != BigInt::from(0) {
            let q = &a / &b;
            let r = &a - &q * &b;
            a = b;
            b = r;
            let w = &u - &q * &v;
            u = v;
            v = w;
        }
        assert_eq!(a, BigInt::from(1));
        rem(u, m)
    }
    fn integer(v: &Natural) -> BigInt {
        v.words_le()
            .iter()
            .rev()
            .fold(BigInt::from(0), |a, &w| (a << 64) + w)
    }
    fn pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
        let mut out = 1;
        while exponent > 0 {
            if exponent & 1 != 0 {
                out = (u128::from(out) * u128::from(base) % u128::from(modulus)) as u64;
            }
            base = (u128::from(base) * u128::from(base) % u128::from(modulus)) as u64;
            exponent >>= 1;
        }
        out
    }
    fn expected_encoded(encoding: &SlotEncoding, values: &[u64], degree: usize) -> Vec<u64> {
        let modulus = encoding.plaintext_precision().modulus();
        let slots = encoding.slots();
        let inverse_slots = pow(
            slots as u64,
            modulus - modulus / encoding.plaintext_precision().prime() - 1,
            modulus,
        );
        let mut out = vec![0; degree];
        for k in 0..slots {
            let mut coefficient = 0_u128;
            for (j, &value) in values.iter().enumerate() {
                let power = ((2 * j + 1) * k) % (2 * slots);
                coefficient += (u128::from(value)
                    * u128::from(pow(
                        encoding.root(),
                        ((2 * slots - power) % (2 * slots)) as u64,
                        modulus,
                    )))
                    % u128::from(modulus);
            }
            out[k * encoding.stride()] =
                (coefficient * u128::from(inverse_slots) % u128::from(modulus)) as u64;
        }
        out
    }
    fn check_phase(
        profile: &BgvProfile,
        secret: &SecretKey,
        cipher: &Ciphertext,
        expected: &[u64],
    ) {
        assert_eq!(cipher.component_count(), 2);
        let n = profile.degree();
        let primes = &profile.parameters().main_primes[..=cipher.level()];
        let q = primes.iter().fold(BigInt::from(1), |a, &p| a * p);
        let half = (&q - 1) / 2;
        let t = BigInt::from(cipher.plaintext_precision().modulus());
        let basis: Vec<_> = primes
            .iter()
            .map(|&p| {
                let part = &q / p;
                let inv = inverse(&rem(part.clone(), &BigInt::from(p)), &BigInt::from(p));
                part * inv
            })
            .collect();
        let words = cipher.canonical_words();
        let count = n * primes.len();
        let lift = |at: usize| {
            (0..n)
                .map(|j| {
                    rem(
                        primes
                            .iter()
                            .enumerate()
                            .fold(BigInt::from(0), |a, (i, _)| {
                                a + &basis[i] * words[at + i * n + j]
                            }),
                        &q,
                    )
                })
                .collect::<Vec<_>>()
        };
        let c0 = lift(0);
        let c1 = lift(count);
        let s = fixtures::secret_coefficients(secret);
        let mut phase = c0;
        for (i, ci) in c1.iter().enumerate() {
            for (j, &sj) in s.iter().enumerate() {
                let term = ci * sj;
                if i + j < n {
                    phase[i + j] += &term;
                } else {
                    phase[i + j - n] -= &term;
                }
            }
        }
        let bound = integer(cipher.noise_bound().as_natural());
        assert!(bound <= half);
        for (value, &wanted) in phase.iter().zip(expected) {
            let value = rem(value.clone(), &q);
            let centered = if value > half { value - &q } else { value };
            assert!(-&bound <= centered && centered <= bound);
            assert_eq!(rem(centered, &t), BigInt::from(wanted));
        }
        let plain = secret.decrypt(profile, cipher).unwrap();
        assert_eq!(plain.plaintext_precision(), cipher.plaintext_precision());
        assert_eq!(plain.coefficients(), expected);
    }

    #[test]
    fn every_coefficient_survives_real_prime_power_transforms_and_independent_crt() {
        for (log_n, p, e) in [
            (4, 5, 2),
            (4, 5, 3),
            (5, 17, 3),
            (4, 257, 3),
            (8, 257, 2),
            (8, 257, 3),
        ] {
            let profile = make_profile(log_n, p, e);
            let master = CoefficientSlotDecomposition::new(
                &profile,
                &profile.plan_fresh(profile.master_plaintext()).unwrap(),
                budget(),
            )
            .unwrap();
            let keys = keys(&profile, master.galois_elements());
            for k in 1..=e {
                let precision = profile.plaintext_precision(k).unwrap();
                let decomposition = CoefficientSlotDecomposition::new(
                    &profile,
                    &profile.plan_fresh(precision).unwrap(),
                    budget(),
                )
                .unwrap();
                let recomposition = CoefficientSlotRecomposition::new(
                    &profile,
                    decomposition.output_shape(),
                    budget(),
                )
                .unwrap();
                let encoding = SlotEncoding::new(&profile, precision).unwrap();
                let n = profile.degree();
                let d = decomposition.banks();
                let t = precision.modulus();
                for case in 0..5 {
                    let values: Vec<_> = (0..n)
                        .map(|j| match case {
                            0 => 0,
                            1 => u64::from(j == 1),
                            2 => u64::from(j + 1 == n),
                            3 => t / 2 + (j % 2) as u64,
                            _ => (j as u64 * 137 + t - 1) % t,
                        })
                        .collect();
                    let input = encrypt(&profile, precision, &keys, values.clone());
                    // Inspect the genuine raw boundary before the matrix. This is
                    // a second actual evaluation for the test, never a raw remint.
                    let raw = decomposition
                        .trace
                        .evaluate(&profile, &input, &keys.evaluation)
                        .unwrap();
                    let raw = raw
                        .into_values(
                            &profile,
                            decomposition.trace.output_shape(),
                            encoding.slots(),
                            d,
                        )
                        .unwrap();
                    let normalized = RelativeTraceDecompositionPlan::new(
                        &profile,
                        &input.shape(),
                        trace_budget(
                            &profile,
                            &input.shape(),
                            d,
                            budget(),
                            BankRetention::Collected,
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(decomposition.trace.operations(), normalized.operations());
                    let normal = normalized
                        .evaluate(&profile, &input, &keys.evaluation)
                        .unwrap();
                    for (r, (bank, normalized_bank)) in raw.iter().zip(&normal).enumerate() {
                        let mut raw_expected = vec![0; n];
                        let mut normal_expected = vec![0; n];
                        for j in 0..encoding.slots() {
                            normal_expected[d * j] = values[r + d * j];
                            raw_expected[d * j] = (d as u64 * values[r + d * j]) % t;
                        }
                        check_phase(&profile, &keys.secret, bank, &raw_expected);
                        check_phase(&profile, &keys.secret, normalized_bank, &normal_expected);
                        if d == 1 {
                            assert_eq!(bank.shape(), normalized_bank.shape());
                            assert_eq!(bank.canonical_words(), normalized_bank.canonical_words());
                        }
                    }
                    drop(raw);
                    drop(normal);
                    reset();
                    let banks = decomposition
                        .evaluate(&profile, input, &keys.evaluation)
                        .unwrap();
                    assert_eq!(BANK_CALLS.with(Cell::get), d);
                    assert_eq!(banks.slots(), encoding.slots());
                    assert_eq!(banks.shape(), decomposition.output_shape());
                    assert_eq!(banks.banks().len(), d);
                    for (r, bank) in banks.banks().iter().enumerate() {
                        let slots: Vec<_> =
                            (0..encoding.slots()).map(|j| values[r + d * j]).collect();
                        let coefficients = expected_encoded(&encoding, &slots, n);
                        check_phase(&profile, &keys.secret, bank, &coefficients);
                        assert_eq!(
                            encoding
                                .decode(&keys.secret.decrypt(&profile, bank).unwrap())
                                .unwrap()
                                .as_slice(),
                            slots
                        );
                    }
                    reset();
                    let restored = recomposition
                        .evaluate(&profile, banks, &keys.evaluation)
                        .unwrap();
                    assert_eq!(BANK_CALLS.with(Cell::get), d);
                    assert_eq!(&restored.shape(), recomposition.output_shape());
                    check_phase(&profile, &keys.secret, &restored, &values);
                }
            }
        }
    }

    #[test]
    fn public_shapes_work_and_payloads_match_independent_closed_forms() {
        let profile = make_profile(5, 17, 3);
        let precision = profile.plaintext_precision(2).unwrap();
        let input = profile.plan_fresh(precision).unwrap();
        let decomposition = CoefficientSlotDecomposition::new(&profile, &input, budget()).unwrap();
        let recomposition =
            CoefficientSlotRecomposition::new(&profile, decomposition.output_shape(), budget())
                .unwrap();
        let n = profile.degree();
        let s = decomposition.slots();
        let d = decomposition.banks();
        let t = precision.modulus();
        let key = profile.key_switch_noise(input.level()).unwrap();
        let trace = input
            .bound()
            .mul_u64(d as u64)
            .add(&key.mul_u64((d - 1) as u64));
        assert_eq!(decomposition.trace.output_shape().bound(), &trace);
        let expected_route = |v: &Natural, route: &SlotLinearPlan<1, 1>| {
            let l = (s as u64) * (t - 1) / 2;
            Natural::from_u64((t - 1) / 2)
                .add(&v.mul_u64((s as u64) * l))
                .add(&key.mul_u64(
                    (s as u64) * u64::from(route.baby_count().ilog2()) / 2 * l
                        + (route.giant_count() - 1) as u64,
                ))
        };
        let coefficient = expected_route(&trace, &decomposition.matrix.plan);
        assert_eq!(decomposition.output_shape().bound(), &coefficient);
        let restored = expected_route(&coefficient, &recomposition.matrix.plan);
        assert_eq!(
            recomposition.output_shape().bound(),
            &restored.mul_u64(d as u64)
        );
        for (extract, route, payload, ops) in [
            (
                true,
                &decomposition.matrix.plan,
                decomposition.payload(),
                decomposition.operations(),
            ),
            (
                false,
                &recomposition.matrix.plan,
                recomposition.payload(),
                recomposition.operations(),
            ),
        ] {
            let b = route.baby_count();
            let g = route.giant_count();
            let c = 16 * n * (input.level() + 1);
            let r = b + g.ilog2() as usize + 3;
            assert_eq!(payload.owned_residue_bytes, (d + 3).max(d + 1 + r) * c);
            assert_eq!(payload.routing.coefficient_bytes, 4 * (s * s + s));
            assert_eq!(payload.routing.preparation_scratch_bytes, 4 * (s + 1));
            assert_eq!(payload.trace_plaintext_bytes, 8 * n);
            assert_eq!(ops.plaintext_products, d * (s + 1));
            assert_eq!(
                ops.ciphertext_additions,
                d * s + (d - 1) * if extract { 2 } else { 1 }
            );
            assert_eq!(
                ops.galois_applications,
                d * (b + g - 2) + if extract { d - 1 } else { 0 }
            );
            assert!(ops.arithmetic_units > 0);
        }
        assert!(
            decomposition
                .galois_elements()
                .windows(2)
                .all(|w| w[0] < w[1])
        );
        assert_eq!(
            checked_mul(usize::MAX, 2),
            Err(CoefficientSlotError::SizeOverflow)
        );
        assert_eq!(checked_mul(0, usize::MAX), Ok(0));
    }

    #[test]
    fn every_explicit_payload_ceiling_refuses_one_byte_below_required() {
        let profile = make_profile(5, 17, 3);
        let input = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let first = CoefficientSlotDecomposition::new(&profile, &input, budget()).unwrap();
        let exact = exact_budget(first.payload());
        assert_eq!(
            CoefficientSlotDecomposition::new(&profile, &input, exact)
                .unwrap()
                .payload(),
            first.payload()
        );
        for field in 0..7 {
            let mut small = exact;
            match field {
                0 => small.owned_residue_bytes -= 1,
                1 => small.trace_plaintext_bytes -= 1,
                2 => small.routing.coefficient_bytes -= 1,
                3 => small.routing.layout_metadata_bytes -= 1,
                4 => small.routing.preparation_scratch_bytes -= 1,
                5 => small.routing.output_residue_bytes -= 1,
                _ => small.routing.scratch_residue_bytes -= 1,
            }
            let error = CoefficientSlotDecomposition::new(&profile, &input, small).unwrap_err();
            assert!(matches!(
                (field, error),
                (0, CoefficientSlotError::ResidueBudget { .. })
                    | (1, CoefficientSlotError::PlaintextBudget { .. })
                    | (
                        2,
                        CoefficientSlotError::Routing(SlotLinearError::CoefficientBudget { .. })
                    )
                    | (
                        3,
                        CoefficientSlotError::Routing(SlotLinearError::LayoutBudget { .. })
                    )
                    | (
                        4,
                        CoefficientSlotError::Routing(SlotLinearError::PreparationBudget { .. })
                    )
                    | (
                        5,
                        CoefficientSlotError::Routing(SlotLinearError::OutputBudget { .. })
                    )
                    | (
                        6,
                        CoefficientSlotError::Routing(SlotLinearError::ScratchBudget { .. })
                    )
            ));
        }
        let reverse =
            CoefficientSlotRecomposition::new(&profile, first.output_shape(), budget()).unwrap();
        let mut small = exact_budget(reverse.payload());
        small.owned_residue_bytes -= 1;
        assert!(matches!(
            CoefficientSlotRecomposition::new(&profile, first.output_shape(), small),
            Err(CoefficientSlotError::ResidueBudget { .. })
        ));
        let three = profile
            .plan_claimed(
                profile.master_plaintext(),
                input.level(),
                3,
                input.bound().clone(),
            )
            .unwrap();
        assert!(CoefficientSlotDecomposition::new(&profile, &three, budget()).is_err());
        let unavailable = make_profile(3, 3, 3);
        assert!(matches!(
            CoefficientSlotDecomposition::new(
                &unavailable,
                &unavailable
                    .plan_fresh(unavailable.master_plaintext())
                    .unwrap(),
                budget()
            ),
            Err(CoefficientSlotError::Bgv(BgvError::EncodingUnavailable))
        ));
    }

    #[test]
    fn complete_key_union_and_contexts_refuse_before_request_arithmetic() {
        let profile = make_profile(5, 17, 3);
        let low = profile.plaintext_precision(1).unwrap();
        let plan = CoefficientSlotDecomposition::new(
            &profile,
            &profile.plan_fresh(low).unwrap(),
            budget(),
        )
        .unwrap();
        let all = keys(&profile, plan.galois_elements());
        let n = profile.degree();
        let words = 2 * n * (profile.top_level() + 1);
        let trace_only = *plan
            .trace
            .galois_elements()
            .iter()
            .find(|g| !plan.matrix.plan.galois_elements().contains(g))
            .unwrap();
        let route_only = *plan
            .matrix
            .plan
            .galois_elements()
            .iter()
            .find(|g| !plan.trace.galois_elements().contains(g))
            .unwrap();
        for excluded in [trace_only, route_only] {
            let elements: Vec<_> = plan
                .galois_elements()
                .iter()
                .copied()
                .filter(|g| *g != excluded)
                .collect();
            let missing = keys(&profile, &elements);
            let input = encrypt(&profile, low, &all, vec![1; n]);
            reset();
            let before = poly::tests::cleared_words();
            assert_eq!(
                plan.evaluate(&profile, input, &missing.evaluation)
                    .unwrap_err(),
                CoefficientSlotError::Bgv(BgvError::WrongSwitchingKey)
            );
            assert_eq!(BANK_CALLS.with(Cell::get), 0);
            assert_eq!(poly::tests::cleared_words() - before, words);
        }
        let high = profile.plaintext_precision(2).unwrap();
        let wrong = encrypt(&profile, high, &all, vec![1; n]);
        reset();
        assert_eq!(
            plan.evaluate(&profile, wrong, &all.evaluation).unwrap_err(),
            CoefficientSlotError::InputShape
        );
        assert_eq!(BANK_CALLS.with(Cell::get), 0);
        let foreign = make_profile(5, 17, 2);
        let other_keys = keys(&foreign, plan.galois_elements());
        let wrong = encrypt(
            &foreign,
            foreign.plaintext_precision(1).unwrap(),
            &other_keys,
            vec![1; n],
        );
        reset();
        assert_eq!(
            plan.evaluate(&profile, wrong, &all.evaluation).unwrap_err(),
            CoefficientSlotError::Bgv(BgvError::ProfileMismatch)
        );
        assert_eq!(BANK_CALLS.with(Cell::get), 0);
        let reverse =
            CoefficientSlotRecomposition::new(&profile, plan.output_shape(), budget()).unwrap();
        for defect in 0..3 {
            let mut banks = plan
                .evaluate(
                    &profile,
                    encrypt(&profile, low, &all, vec![2; n]),
                    &all.evaluation,
                )
                .unwrap();
            match defect {
                0 => {
                    banks.values.pop();
                }
                1 => banks.slots += 1,
                _ => banks.shape = profile.plan_fresh(high).unwrap(),
            }
            reset();
            assert_eq!(
                reverse
                    .evaluate(&profile, banks, &all.evaluation)
                    .unwrap_err(),
                CoefficientSlotError::InputShape
            );
            assert_eq!(BANK_CALLS.with(Cell::get), 0);
        }
    }

    #[test]
    fn partial_and_complete_bank_sets_clear_on_error_and_unwind_in_both_directions() {
        let profile = make_profile(5, 17, 3);
        let precision = profile.plaintext_precision(2).unwrap();
        let plan = CoefficientSlotDecomposition::new(
            &profile,
            &profile.plan_fresh(precision).unwrap(),
            budget(),
        )
        .unwrap();
        let reverse =
            CoefficientSlotRecomposition::new(&profile, plan.output_shape(), budget()).unwrap();
        let keys = keys(&profile, plan.galois_elements());
        let d = plan.banks();
        let one_words = 2 * profile.degree() * (profile.top_level() + 1);
        for recomposing in [false, true] {
            for at in [1, d] {
                for mode in [1, 2] {
                    reset();
                    let input = encrypt(&profile, precision, &keys, vec![3; profile.degree()]);
                    let banks = if recomposing {
                        Some(
                            plan.evaluate(&profile, input.clone(), &keys.evaluation)
                                .unwrap(),
                        )
                    } else {
                        None
                    };
                    // The fixture's spare clone is dropped before the failure checkpoint.
                    reset();
                    FAULT.with(|x| x.set((at, mode)));
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if let Some(banks) = banks {
                            drop(input);
                            reverse
                                .evaluate(&profile, banks, &keys.evaluation)
                                .map(|_| ())
                        } else {
                            plan.evaluate(&profile, input, &keys.evaluation).map(|_| ())
                        }
                    }));
                    if mode == 1 {
                        assert_eq!(
                            result.unwrap().unwrap_err(),
                            CoefficientSlotError::Bgv(BgvError::Kernel)
                        );
                    } else {
                        assert!(result.is_err());
                    }
                    assert_eq!(BANK_CALLS.with(Cell::get), at);
                    assert_eq!(
                        poly::tests::cleared_words() - CLEAR_AT_FAULT.with(Cell::get),
                        (if recomposing {
                            d
                        } else {
                            at + d.ilog2() as usize - (at - 1).count_ones() as usize
                        }) * one_words
                    );
                }
            }
        }
        reset();
    }
    #[test]
    fn streaming_scalar_tokens_accumulator_and_all_pending_owners_are_closed() {
        reset();
        let profile = make_profile(4, 5, 3);
        let precision = profile.plaintext_precision(2).unwrap();
        let shape = profile.plan_fresh(precision).unwrap();
        let plan = CoefficientSlotDecomposition::prepare(
            &profile,
            &shape,
            budget(),
            BankRetention::Streaming,
        )
        .unwrap();
        let reverse = CoefficientSlotRecomposition::prepare(
            &profile,
            plan.output_shape(),
            budget(),
            BankRetention::Streaming,
        )
        .unwrap();
        let keys = keys(&profile, plan.galois_elements());
        assert!(
            reverse
                .start_accumulator(&profile, &keys.evaluation)
                .unwrap()
                .finish()
                .is_err()
        );
        for defect in 0..4 {
            let input = encrypt(&profile, precision, &keys, vec![24; profile.degree()]);
            let (mut bank, rest) = plan
                .start(&profile, input, &keys.evaluation)
                .unwrap()
                .advance(&profile, &keys.evaluation)
                .unwrap()
                .unwrap();
            match defect {
                0 => bank.ordinal = 1,
                1 => bank.slots += 1,
                2 => bank.banks += 1,
                _ => bank.value = profile.switch_down(&bank.value).unwrap(),
            }
            let accumulator = reverse
                .start_accumulator(&profile, &keys.evaluation)
                .unwrap();
            reset();
            assert!(matches!(
                accumulator.push(&profile, bank, &keys.evaluation),
                Err(CoefficientSlotError::InputShape)
            ));
            assert_eq!(BANK_CALLS.get(), 0);
            drop(rest);
        }
        let d = plan.banks();
        let h = d.ilog2() as usize;
        for at in 1..=2 * d {
            for mode in [1, 2] {
                reset();
                let input = encrypt(&profile, precision, &keys, vec![24; profile.degree()]);
                FAULT.set((at, mode));
                let outcome = catch_unwind(AssertUnwindSafe(
                    || -> Result<Ciphertext, CoefficientSlotError> {
                        let mut cursor = plan.start(&profile, input, &keys.evaluation)?;
                        let mut accumulator =
                            reverse.start_accumulator(&profile, &keys.evaluation)?;
                        while let Some((bank, rest)) = cursor.advance(&profile, &keys.evaluation)? {
                            accumulator = accumulator.push(&profile, bank, &keys.evaluation)?;
                            cursor = rest;
                        }
                        accumulator.finish()
                    },
                ));
                assert!(if mode == 2 {
                    outcome.is_err()
                } else {
                    matches!(
                        outcome,
                        Ok(Err(CoefficientSlotError::Bgv(BgvError::Kernel)))
                    )
                });
                let done = (at - 1) / 2;
                let owners = h - done.count_ones() as usize + usize::from(done > 0) + 1;
                assert_eq!(
                    poly::tests::cleared_words() - CLEAR_AT_FAULT.get(),
                    owners * 2 * profile.degree() * (profile.top_level() + 1)
                );
            }
        }
        reset();
    }
}
