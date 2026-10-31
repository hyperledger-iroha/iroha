//! Exact full-ring decomposition into the existing scalar subalgebra.
//!
//! With `S` scalar slots, `d=N/S` and `Y=X^d`, every ring plaintext has the
//! unique expansion `a(X)=sum_r X^r g_r(Y)`, `0<=r<d`. The subgroup
//! `H={h=1 mod 2S}` of the odd units modulo `2N` has order `d` and generator
//! `5^(S/2)`. Its relative trace kills every exponent not divisible by `d`:
//! `g_r(Y)=d^-1 sum_{h in H} a(X^h) X^(-r*h)`.
//!
//! A fixed parity-split tree evaluates the relative trace jointly for all
//! banks. At depth j, the current polynomial is supported on multiples of
//! 2^j. The automorphism 1+N/2^j negates precisely its odd coordinates. Its
//! sum and difference select the two children; the odd child is shifted by
//! X^(-2^j). The public normalized transform applies one unit inverse d^-1
//! before the tree. Coefficient extraction alone uses a private unit-scaled
//! entry into that same tree and combines normalization into its inverse map.
//! Its private bank-zero selection follows only even children; geometric `d`
//! remains unchanged, and no odd sibling arithmetic is executed.
//!
//! Each output bank encodes the evaluations of `g_r` at the existing slot
//! roots; it is not the vector of coefficients of `g_r`. Recomposition is the
//! exact sum `sum_r X^r g_r(X^d)` and requires no Galois keys.
//!
//! Both schedules use public loops and the existing BGV transfers and kernels.
//! The inverse of `d` is a unit multiplier modulo the effective `p^k` with its actual centered magnitude,
//! never a division of the noise bound. Levels are unchanged and noise grows.
//! The separate local bootstrap owner composes these transforms with its genuine
//! encrypted-secret and digit-extraction arithmetic. TODO(C.8, R.9): external
//! proofs, circuit privacy and parameter/resource qualification remain unfinished.

use super::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, EvaluationKeys, Plaintext, PlaintextBound,
    PlaintextPrecision, SlotEncoding, SwitchingKeyKind, cost::CostModel,
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Caller-supplied payload ceilings, checked before request arithmetic.
///
/// Residues count only ciphertexts owned by this transform, excluding borrowed
/// inputs/keys and BGV/NTT/key-switch scratch. Plaintext bytes cover the one
/// clearing monomial buffer. Public plan/container metadata and allocator
/// overhead, including private plan-wrapper tags and public bank shape metadata,
/// are excluded. These are not a whole-process resource limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "Every resource ceiling names its explicit byte unit; public fields and payload scopes stay unambiguous."
)]
pub struct RelativeTraceBudget {
    /// Retained output ciphertext residue bytes.
    pub output_residue_bytes: usize,
    /// Peak extra ciphertext residues beyond the complete retained output size.
    pub scratch_residue_bytes: usize,
    /// Bytes of the one public monomial coefficient buffer.
    pub plaintext_bytes: usize,
}

/// Which explicitly bounded payload exceeded its ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeTracePayload {
    /// Retained output residues.
    Output,
    /// Additional temporary ciphertext residues.
    Scratch,
    /// Public monomial coefficients.
    Plaintext,
}

/// Refusal by a relative-trace plan or execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RelativeTraceError {
    /// A checked public size or operation count overflowed.
    #[error("relative-trace public geometry overflow")]
    SizeOverflow,
    /// A bounded payload exceeds the supplied ceiling.
    #[error("relative-trace {kind:?} needs {required} bytes, budget {budget}")]
    Budget {
        /// Payload that is too large.
        kind: RelativeTracePayload,
        /// Required bytes.
        required: usize,
        /// Supplied ceiling.
        budget: usize,
    },
    /// The request shape differs from the immutable public plan.
    #[error("relative-trace request differs from its planned shape")]
    InputShape,
    /// A public-sized owner could not be allocated.
    #[error("relative-trace allocation failed")]
    Allocation,
    /// Existing BGV profile, component, key, level or noise validation failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Exact high-level operation counts and existing `CostModel` multiplication units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelativeTraceOperations {
    /// Plaintext products, including public identity monomials.
    pub plaintext_products: usize,
    /// Key-switched automorphisms; one per internal decomposition tree node.
    pub galois_applications: usize,
    /// Ciphertext additions and subtractions.
    pub ciphertext_additions: usize,
    /// Canonical `CostModel` units; excludes additions, allocation, clearing and copies.
    pub arithmetic_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Geometry {
    precision: PlaintextPrecision,
    degree: usize,
    slots: usize,
    banks: usize,
    inverse: u64,
    inverse_bound: PlaintextBound,
    generators: Vec<u32>,
}

impl Geometry {
    fn new(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
    ) -> Result<Self, RelativeTraceError> {
        let encoding = SlotEncoding::new(profile, precision)?;
        let degree = profile.degree();
        let slots = encoding.slots();
        let banks = degree / slots;
        let modulus = precision.modulus();
        let phi = modulus - modulus / precision.prime();
        let fixed = precision.fixed();
        let bank_residue = fixed.reduce_u64(banks as u64);
        let inverse = fixed.power(bank_residue, phi - 1);
        if fixed.multiply(bank_residue, inverse) != 1 {
            return Err(BgvError::Kernel.into());
        }
        let magnitude = inverse.min(modulus - inverse);

        let mut generators = Vec::new();
        generators
            .try_reserve_exact(banks.ilog2() as usize)
            .map_err(|_| RelativeTraceError::Allocation)?;
        for depth in 0..banks.ilog2() {
            generators.push(
                u32::try_from(1 + (degree >> depth))
                    .map_err(|_| RelativeTraceError::SizeOverflow)?,
            );
        }
        Ok(Self {
            precision,
            degree,
            slots,
            banks,
            inverse,
            inverse_bound: PlaintextBound {
                linf: magnitude,
                l1: magnitude,
            },
            generators,
        })
    }

    fn check_profile(&self, profile: &BgvProfile) -> Result<(), RelativeTraceError> {
        self.precision.check_profile(profile)?;
        Ok(())
    }

    fn payload(
        &self,
        profile: &BgvProfile,
        level: usize,
        outputs: usize,
        scratch: usize,
        budget: RelativeTraceBudget,
    ) -> Result<RelativeTraceBudget, RelativeTraceError> {
        let bytes = usize::try_from(CostModel::new(profile).ciphertext_residue_bytes(level, 2)?)
            .map_err(|_| RelativeTraceError::SizeOverflow)?;
        // At an internal split, completed leaves + pending sibling roots +
        // current form a frontier with at most d-1 ciphertexts. The image,
        // even sum and odd difference add three, giving at most d+2.
        // Odd shifting occurs after current/image are dropped and adds one.
        // Recomposition: accumulator/product/sum. For d=1 no add is executed.
        let required = RelativeTraceBudget {
            output_residue_bytes: checked_mul(outputs, bytes)?,
            scratch_residue_bytes: checked_mul(scratch, bytes)?,
            plaintext_bytes: checked_mul(self.degree, size_of::<u64>())?,
        };
        for (kind, need, limit) in [
            (
                RelativeTracePayload::Output,
                required.output_residue_bytes,
                budget.output_residue_bytes,
            ),
            (
                RelativeTracePayload::Scratch,
                required.scratch_residue_bytes,
                budget.scratch_residue_bytes,
            ),
            (
                RelativeTracePayload::Plaintext,
                required.plaintext_bytes,
                budget.plaintext_bytes,
            ),
        ] {
            if need > limit {
                return Err(RelativeTraceError::Budget {
                    kind,
                    required: need,
                    budget: limit,
                });
            }
        }
        Ok(required)
    }
}

fn checked_mul(a: usize, b: usize) -> Result<usize, RelativeTraceError> {
    a.checked_mul(b).ok_or(RelativeTraceError::SizeOverflow)
}

fn operations(
    profile: &BgvProfile,
    level: usize,
    products: usize,
    galois: usize,
    additions: usize,
) -> Result<RelativeTraceOperations, RelativeTraceError> {
    let cost = CostModel::new(profile);
    let automorphism_units = cost.automorphism_units(level)?;
    let units = cost
        .plaintext_multiplication_units(level)?
        .checked_mul(u64::try_from(products).map_err(|_| RelativeTraceError::SizeOverflow)?)
        .and_then(|plain| {
            automorphism_units
                .checked_mul(galois as u64)?
                .checked_add(plain)
        })
        .ok_or(RelativeTraceError::SizeOverflow)?;
    Ok(RelativeTraceOperations {
        plaintext_products: products,
        galois_applications: galois,
        ciphertext_additions: additions,
        arithmetic_units: units,
    })
}

fn monomial(
    profile: &BgvProfile,
    precision: PlaintextPrecision,
    exponent: usize,
    coefficient: u64,
    bound: PlaintextBound,
) -> Result<Plaintext, RelativeTraceError> {
    let mut words = Zeroizing::new(Vec::new());
    words
        .try_reserve_exact(profile.degree())
        .map_err(|_| RelativeTraceError::Allocation)?;
    words.resize(profile.degree(), 0);
    words[exponent] = coefficient;
    #[cfg(test)]
    tests::observe_plaintext();
    Ok(Plaintext::with_public_bound(
        profile,
        precision,
        std::mem::take(&mut *words),
        bound,
    )?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Normalization {
    InverseBanks,
    Unit,
}

impl Normalization {
    fn scale(self, geometry: &Geometry) -> (u64, PlaintextBound) {
        match self {
            Self::InverseBanks => (geometry.inverse, geometry.inverse_bound),
            Self::Unit => (1, PlaintextBound { linf: 1, l1: 1 }),
        }
    }
}

/// Private resource retention only; never changes logical bank geometry or work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BankRetention {
    Collected,
    Streaming,
}

/// Private coefficient selection, independent of geometric stride and retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CoefficientBankSelection {
    All,
    Zero,
}

impl CoefficientBankSelection {
    pub(super) const fn count(self, banks: usize) -> usize {
        match self {
            Self::All => banks,
            Self::Zero => 1,
        }
    }

    /// Exact next ordinal in the canonical even-first traversal.
    pub(super) fn expected_ordinal(self, completed: usize, banks: usize) -> Option<usize> {
        if banks == 0 || !banks.is_power_of_two() || completed >= self.count(banks) {
            return None;
        }
        let bits = banks.ilog2();
        Some(if self == Self::Zero || bits == 0 {
            0
        } else {
            completed.reverse_bits() >> (usize::BITS - bits)
        })
    }

    const fn frontier_capacity(self, banks: usize) -> usize {
        match self {
            Self::All => banks.ilog2() as usize + 1,
            Self::Zero => 1,
        }
    }
}

struct ParityFrame {
    value: Ciphertext,
    depth: usize,
    ordinal: usize,
}

/// Sole move-owned parity traversal. No public callback or resumable failed cursor.
struct ParityCursor<'plan> {
    plan: &'plan ParityDecompositionPlan,
    frontier: Vec<ParityFrame>,
    visited: usize,
}

impl ParityCursor<'_> {
    fn advance(
        mut self,
        profile: &BgvProfile,
        keys: &EvaluationKeys,
    ) -> Result<Option<(Ciphertext, usize, Self)>, RelativeTraceError> {
        let geometry = &self.plan.geometry;
        while let Some(ParityFrame {
            value: current,
            depth,
            ordinal,
        }) = self.frontier.pop()
        {
            if depth == geometry.generators.len() {
                if current.shape() != self.plan.output
                    || self
                        .plan
                        .selection
                        .expected_ordinal(self.visited, geometry.banks)
                        != Some(ordinal)
                {
                    return Err(RelativeTraceError::InputShape);
                }
                self.visited += 1;
                #[cfg(test)]
                tests::after_bank()?;
                return Ok(Some((current, ordinal, self)));
            }
            let key = keys
                .galois(geometry.generators[depth])
                .ok_or(BgvError::WrongSwitchingKey)?;
            let image = profile.apply_galois(&current, key)?;
            #[cfg(test)]
            tests::observe_galois();
            #[cfg(test)]
            tests::split_checkpoint()?;
            let even = profile.add(&current, &image)?;
            #[cfg(test)]
            tests::observe_add();
            #[cfg(test)]
            tests::split_checkpoint()?;
            if self.plan.selection == CoefficientBankSelection::Zero {
                // Only the even branch exists. Do not compute an odd sibling,
                // subtraction or monomial and discard it afterward.
                drop(current);
                drop(image);
                self.frontier.push(ParityFrame {
                    value: even,
                    depth: depth + 1,
                    ordinal,
                });
                continue;
            }
            let odd = profile.sub(&current, &image)?;
            #[cfg(test)]
            tests::observe_add();
            #[cfg(test)]
            tests::split_checkpoint()?;
            drop(current);
            drop(image);
            let multiplier = monomial(
                profile,
                geometry.precision,
                geometry.degree - (1_usize << depth),
                geometry.precision.modulus() - 1,
                PlaintextBound { linf: 1, l1: 1 },
            )?;
            let shifted = profile.mul_plain(&odd, &multiplier)?;
            drop(odd);
            drop(multiplier);
            #[cfg(test)]
            tests::observe_product();
            #[cfg(test)]
            tests::split_checkpoint()?;
            // Capacity h+1 was admitted/reserved before the first private operation.
            // Push odd first so the next pop follows fixed even-first DFS.
            self.frontier.push(ParityFrame {
                value: shifted,
                depth: depth + 1,
                ordinal: ordinal + (1 << depth),
            });
            self.frontier.push(ParityFrame {
                value: even,
                depth: depth + 1,
                ordinal,
            });
        }
        if self.visited != self.plan.selection.count(geometry.banks) {
            return Err(RelativeTraceError::InputShape);
        }
        Ok(None)
    }
}

/// Public bit reversal moves owners without allocating another residue set.
pub(super) fn natural_order(values: &mut [Ciphertext]) {
    if values.len() < 2 {
        return;
    }
    let bits = values.len().ilog2();
    if bits > 0 {
        for i in 0..values.len() {
            let j = i.reverse_bits() >> (usize::BITS - bits);
            if i < j {
                values.swap(i, j);
            }
        }
    }
}

/// Sole private parity plan; normalization and its bound are derived together.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ParityDecompositionPlan {
    geometry: Geometry,
    normalization: Normalization,
    retention: BankRetention,
    selection: CoefficientBankSelection,
    input: CiphertextShape,
    output: CiphertextShape,
    payload: RelativeTraceBudget,
    operations: RelativeTraceOperations,
}

impl ParityDecompositionPlan {
    /// Plan every monomial product, subgroup automorphism and addition.
    ///
    /// # Errors
    /// Refuses unavailable scalar encodings, non-two-component inputs, invalid
    /// levels, any intermediate noise failure and exceeded payload ceilings.
    fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: RelativeTraceBudget,
        normalization: Normalization,
        retention: BankRetention,
        selection: CoefficientBankSelection,
    ) -> Result<Self, RelativeTraceError> {
        input.check_profile(profile)?;
        let geometry = Geometry::new(profile, input.plaintext_precision())?;
        if input.components() != 2 {
            return Err(BgvError::ComponentCount {
                found: input.components(),
            }
            .into());
        }
        let (_, scale_bound) = normalization.scale(&geometry);
        let mut output = profile.plan_mul_plain(input, scale_bound)?;
        for _ in &geometry.generators {
            let image = profile.plan_galois(&output)?;
            let sum = profile.plan_add(&output, &image)?;
            // All-mode odd monomials have norm1. Zero follows only real sums.
            output = if selection == CoefficientBankSelection::All {
                profile.plan_mul_plain(&sum, PlaintextBound { linf: 1, l1: 1 })?
            } else {
                sum
            };
        }
        let (outputs, scratch) = match (selection, retention) {
            (CoefficientBankSelection::Zero, _) => (1, usize::from(geometry.banks > 1) * 2),
            (CoefficientBankSelection::All, BankRetention::Collected) => {
                (geometry.banks, usize::from(geometry.banks > 1) * 2)
            }
            (CoefficientBankSelection::All, BankRetention::Streaming) => (
                1,
                if geometry.banks > 1 {
                    geometry.generators.len() + 2
                } else {
                    0
                },
            ),
        };
        let payload = geometry.payload(profile, input.level(), outputs, scratch, budget)?;
        let (products, applications, additions) = match selection {
            CoefficientBankSelection::All => (
                geometry.banks,
                geometry.banks - 1,
                checked_mul(geometry.banks - 1, 2)?,
            ),
            CoefficientBankSelection::Zero => {
                (1, geometry.generators.len(), geometry.generators.len())
            }
        };
        let operations = operations(profile, input.level(), products, applications, additions)?;
        Ok(Self {
            geometry,
            normalization,
            retention,
            selection,
            input: input.clone(),
            output,
            payload,
            operations,
        })
    }

    /// Number of scalar slots in each resulting bank.
    #[must_use]
    const fn slots(&self) -> usize {
        self.geometry.slots
    }

    /// Number of resulting banks, exactly `N/S`.
    #[must_use]
    const fn banks(&self) -> usize {
        self.geometry.banks
    }

    /// Required parity-character Galois elements, in fixed tree-depth order.
    #[must_use]
    fn galois_elements(&self) -> &[u32] {
        &self.geometry.generators
    }

    /// Exact public shape of every resulting bank.
    #[must_use]
    const fn output_shape(&self) -> &CiphertextShape {
        &self.output
    }

    /// Payload admitted by the plan, with the scope of [`RelativeTraceBudget`].
    #[must_use]
    const fn payload(&self) -> RelativeTraceBudget {
        self.payload
    }

    /// Exact high-level operations and shared multiplication-unit accounting.
    #[must_use]
    const fn operations(&self) -> RelativeTraceOperations {
        self.operations
    }

    fn preflight(
        &self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<(), RelativeTraceError> {
        self.geometry.check_profile(profile)?;
        input.check_profile(profile)?;
        if input.shape() != self.input {
            return Err(RelativeTraceError::InputShape);
        }
        for &element in &self.geometry.generators {
            let key = keys.galois(element).ok_or(BgvError::WrongSwitchingKey)?;
            key.check_profile(profile)?;
            if key.kind() != SwitchingKeyKind::Galois(element) {
                return Err(BgvError::WrongSwitchingKey.into());
            }
        }
        Ok(())
    }

    fn start<'a>(
        &'a self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<ParityCursor<'a>, RelativeTraceError> {
        self.preflight(profile, input, keys)?;
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(self.selection.frontier_capacity(self.geometry.banks))
            .map_err(|_| RelativeTraceError::Allocation)?;
        // Both modes perform the same real initial product. This borrows input;
        // the consuming coefficient caller drops its root immediately on return.
        let (scale, scale_bound) = self.normalization.scale(&self.geometry);
        let multiplier = monomial(profile, self.geometry.precision, 0, scale, scale_bound)?;
        let value = profile.mul_plain(input, &multiplier)?;
        drop(multiplier);
        #[cfg(test)]
        tests::observe_product();
        frontier.push(ParityFrame {
            value,
            depth: 0,
            ordinal: 0,
        });
        Ok(ParityCursor {
            plan: self,
            frontier,
            visited: 0,
        })
    }

    fn evaluate(
        &self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<Vec<Ciphertext>, RelativeTraceError> {
        if self.retention != BankRetention::Collected {
            return Err(RelativeTraceError::InputShape);
        }
        self.preflight(profile, input, keys)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.selection.count(self.banks()))
            .map_err(|_| RelativeTraceError::Allocation)?;
        let mut cursor = self.start(profile, input, keys)?;
        while let Some((value, _ordinal, rest)) = cursor.advance(profile, keys)? {
            output.push(value);
            cursor = rest;
        }
        natural_order(&mut output);
        Ok(output)
    }
}

/// Public plan for all normalized `d=N/S` scalar-subalgebra banks.
///
/// The public transform always applies the modular unit inverse of `d` before
/// the sole parity executor. Its bank plaintexts are exactly `g_r(X^d)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeTraceDecompositionPlan {
    core: ParityDecompositionPlan,
}

impl RelativeTraceDecompositionPlan {
    /// Plan the normalized transform with every ordinary transfer and payload check.
    ///
    /// # Errors
    /// Invalid profile/components/encoding, noise, geometry or payload refusal.
    pub fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: RelativeTraceBudget,
    ) -> Result<Self, RelativeTraceError> {
        Ok(Self {
            core: ParityDecompositionPlan::new(
                profile,
                input,
                budget,
                Normalization::InverseBanks,
                BankRetention::Collected,
                CoefficientBankSelection::All,
            )?,
        })
    }
    /// Scalar slots in each normalized output bank.
    #[must_use]
    pub const fn slots(&self) -> usize {
        self.core.slots()
    }
    /// Number of banks, exactly `N/S` in increasing residue-class order.
    #[must_use]
    pub const fn banks(&self) -> usize {
        self.core.banks()
    }
    /// Complete required parity-character Galois elements in depth order.
    #[must_use]
    pub fn galois_elements(&self) -> &[u32] {
        self.core.galois_elements()
    }
    /// Exact common output shape of normalized banks.
    #[must_use]
    pub const fn output_shape(&self) -> &CiphertextShape {
        self.core.output_shape()
    }
    /// Admitted residue/plaintext payload; public wrapper metadata is separate.
    #[must_use]
    pub const fn payload(&self) -> RelativeTraceBudget {
        self.core.payload()
    }
    /// Exact public operations and shared `CostModel` multiplication units.
    #[must_use]
    pub const fn operations(&self) -> RelativeTraceOperations {
        self.core.operations()
    }
    /// Execute all normalized branches after complete shape/profile/key preflight.
    ///
    /// Returned ciphertexts own clearing residues; this provides no input authentication.
    ///
    /// # Errors
    /// Context/shape/key mismatch, allocation failure or canonical arithmetic refusal.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<Vec<Ciphertext>, RelativeTraceError> {
        self.core.evaluate(profile, input, keys)
    }
}

/// Closed unit-scaled parity leaves, with no vector or claimed-shape constructor.
///
/// These plaintexts are `d*g_r(X^d)`, not normalized trace or coefficient banks.
/// Only the concrete inverse-N coefficient map consumes them into coefficient origin.
#[cfg(test)]
pub(super) struct UnnormalizedCoefficientBanks {
    values: Vec<Ciphertext>,
    shape: CiphertextShape,
    slots: usize,
    banks: usize,
    selection: CoefficientBankSelection,
}

#[cfg(test)]
impl core::fmt::Debug for UnnormalizedCoefficientBanks {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("[REDACTED unnormalized coefficient banks]")
    }
}

#[cfg(test)]
impl UnnormalizedCoefficientBanks {
    pub(super) fn into_values(
        self,
        profile: &BgvProfile,
        shape: &CiphertextShape,
        slots: usize,
        banks: usize,
    ) -> Result<Vec<Ciphertext>, RelativeTraceError> {
        self.shape.check_profile(profile)?;
        if self.shape != *shape
            || self.slots != slots
            || self.banks != banks
            || self.selection != CoefficientBankSelection::All
            || self.values.len() != banks
        {
            return Err(RelativeTraceError::InputShape);
        }
        for value in &self.values {
            value.check_profile(profile)?;
            if value.shape() != self.shape {
                return Err(RelativeTraceError::InputShape);
            }
        }
        Ok(self.values)
    }
}

/// Coefficient extraction's private unit-normalized entry into the sole executor.
#[derive(Debug)]
pub(super) struct CoefficientTracePlan {
    core: ParityDecompositionPlan,
}

impl CoefficientTracePlan {
    pub(super) fn metadata_heap(&self) -> Result<usize, super::PlanMetadataError> {
        super::metadata::sum(&[
            super::metadata::shape_heap(&self.core.input)?,
            super::metadata::shape_heap(&self.core.output)?,
            super::metadata::mul(self.core.geometry.generators.capacity(), size_of::<u32>())?,
        ])
    }

    pub(super) fn new(
        profile: &BgvProfile,
        input: &CiphertextShape,
        budget: RelativeTraceBudget,
        retention: BankRetention,
        selection: CoefficientBankSelection,
    ) -> Result<Self, RelativeTraceError> {
        Ok(Self {
            core: ParityDecompositionPlan::new(
                profile,
                input,
                budget,
                Normalization::Unit,
                retention,
                selection,
            )?,
        })
    }
    pub(super) const fn slots(&self) -> usize {
        self.core.slots()
    }
    pub(super) const fn banks(&self) -> usize {
        self.core.banks()
    }
    pub(super) const fn selection(&self) -> CoefficientBankSelection {
        self.core.selection
    }
    pub(super) const fn selected_banks(&self) -> usize {
        self.core.selection.count(self.core.banks())
    }
    pub(super) fn galois_elements(&self) -> &[u32] {
        self.core.galois_elements()
    }
    pub(super) const fn output_shape(&self) -> &CiphertextShape {
        self.core.output_shape()
    }
    pub(super) const fn operations(&self) -> RelativeTraceOperations {
        self.core.operations()
    }
    pub(super) fn start<'a>(
        &'a self,
        profile: &BgvProfile,
        input: Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<CoefficientTraceCursor<'a>, RelativeTraceError> {
        let core = self.core.start(profile, &input, keys)?;
        drop(input);
        Ok(CoefficientTraceCursor { core })
    }

    #[cfg(test)]
    pub(super) fn evaluate(
        &self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<UnnormalizedCoefficientBanks, RelativeTraceError> {
        let values = self.core.evaluate(profile, input, keys)?;
        Ok(UnnormalizedCoefficientBanks {
            values,
            shape: self.core.output.clone(),
            slots: self.core.slots(),
            banks: self.core.banks(),
            selection: self.core.selection,
        })
    }
}

pub(super) fn cursor_metadata_bytes(
    profile: &BgvProfile,
    banks: usize,
    selection: CoefficientBankSelection,
) -> Result<usize, RelativeTraceError> {
    if banks == 0 || !banks.is_power_of_two() {
        return Err(RelativeTraceError::InputShape);
    }
    let words = profile
        .noise_bound_words()
        .checked_mul(2)
        .and_then(|x| x.checked_add(1))
        .ok_or(RelativeTraceError::SizeOverflow)?;
    let each = size_of::<ParityFrame>()
        .checked_add(2 * size_of::<super::poly::RnsPoly>())
        .and_then(|x| x.checked_add(words.checked_mul(size_of::<u64>())?))
        .ok_or(RelativeTraceError::SizeOverflow)?;
    checked_mul(selection.frontier_capacity(banks), each)?
        .checked_add(size_of::<CoefficientTraceCursor<'_>>())
        .ok_or(RelativeTraceError::SizeOverflow)
}

/// Unit-mode cursor: only the concrete coefficient map consumes its leaves.
pub(super) struct CoefficientTraceCursor<'plan> {
    core: ParityCursor<'plan>,
}
pub(super) struct UnnormalizedCoefficientBank {
    value: Ciphertext,
    ordinal: usize,
    slots: usize,
    banks: usize,
    selection: CoefficientBankSelection,
}
impl UnnormalizedCoefficientBank {
    pub(super) fn into_value(
        self,
        profile: &BgvProfile,
        shape: &CiphertextShape,
        slots: usize,
        banks: usize,
        selection: CoefficientBankSelection,
    ) -> Result<(Ciphertext, usize), RelativeTraceError> {
        self.value.check_profile(profile)?;
        if self.value.shape() != *shape
            || self.slots != slots
            || self.banks != banks
            || self.ordinal >= banks
            || self.selection != selection
            || (selection == CoefficientBankSelection::Zero && self.ordinal != 0)
        {
            return Err(RelativeTraceError::InputShape);
        }
        Ok((self.value, self.ordinal))
    }
}
impl CoefficientTraceCursor<'_> {
    pub(super) fn advance(
        self,
        profile: &BgvProfile,
        keys: &EvaluationKeys,
    ) -> Result<Option<(UnnormalizedCoefficientBank, Self)>, RelativeTraceError> {
        let slots = self.core.plan.slots();
        let banks = self.core.plan.banks();
        let selection = self.core.plan.selection;
        Ok(self
            .core
            .advance(profile, keys)?
            .map(|(value, ordinal, core)| {
                (
                    UnnormalizedCoefficientBank {
                        value,
                        ordinal,
                        slots,
                        banks,
                        selection,
                    },
                    Self { core },
                )
            }))
    }
}

/// Shared monomial product and ordinary accumulation used by both public
/// heterogeneous recomposition and the closed streaming coefficient pipeline.
pub(super) fn monomial_product(
    profile: &BgvProfile,
    input: &Ciphertext,
    ordinal: usize,
) -> Result<Ciphertext, RelativeTraceError> {
    if ordinal >= profile.degree() {
        return Err(RelativeTraceError::InputShape);
    }
    let multiplier = monomial(
        profile,
        input.plaintext_precision(),
        ordinal,
        1,
        PlaintextBound { linf: 1, l1: 1 },
    )?;
    let product = profile.mul_plain(input, &multiplier)?;
    drop(multiplier);
    #[cfg(test)]
    tests::observe_product();
    Ok(product)
}
pub(super) fn accumulate(
    profile: &BgvProfile,
    output: Option<Ciphertext>,
    product: Ciphertext,
) -> Result<Ciphertext, RelativeTraceError> {
    let result = match output {
        None => product,
        Some(sum) => {
            let combined = profile.add(&sum, &product)?;
            #[cfg(test)]
            tests::observe_add();
            combined
        }
    };
    #[cfg(test)]
    tests::after_bank()?;
    Ok(result)
}

/// Public plan for recomposing the exact ordered scalar-subalgebra banks.
///
/// Recomposition is a ring-linear operation on any supplied bank plaintexts;
/// checking that hidden banks lie in the scalar subalgebra requires a separate
/// input proof. In particular this plan grants no refresh or proof authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeTraceRecompositionPlan {
    geometry: Geometry,
    inputs: Vec<CiphertextShape>,
    output: CiphertextShape,
    payload: RelativeTraceBudget,
    operations: RelativeTraceOperations,
}

impl RelativeTraceRecompositionPlan {
    pub(super) fn metadata_heap(&self) -> Result<usize, super::PlanMetadataError> {
        let mut bytes = super::metadata::sum(&[
            super::metadata::shape_heap(&self.output)?,
            super::metadata::mul(self.geometry.generators.capacity(), size_of::<u32>())?,
            super::metadata::mul(self.inputs.capacity(), size_of::<CiphertextShape>())?,
        ])?;
        for input in &self.inputs {
            bytes = super::metadata::add(bytes, super::metadata::shape_heap(input)?)?;
        }
        Ok(bytes)
    }

    /// Plan `sum_r X^r * bank_r`, without level changes or noise reductions.
    ///
    /// # Errors
    /// Wrong bank count/components/levels, any noise failure, allocation failure
    /// or exceeded payload ceilings. Input shapes may have different bounds.
    pub fn new(
        profile: &BgvProfile,
        inputs: &[CiphertextShape],
        budget: RelativeTraceBudget,
    ) -> Result<Self, RelativeTraceError> {
        Self::prepare(profile, inputs, budget, CoefficientBankSelection::All)
    }

    pub(super) fn prepare(
        profile: &BgvProfile,
        inputs: &[CiphertextShape],
        budget: RelativeTraceBudget,
        selection: CoefficientBankSelection,
    ) -> Result<Self, RelativeTraceError> {
        let first = inputs.first().ok_or(BgvError::LengthMismatch {
            expected: 1,
            found: 0,
        })?;
        first.check_profile(profile)?;
        let geometry = Geometry::new(profile, first.plaintext_precision())?;
        let selected = selection.count(geometry.banks);
        if inputs.len() != selected {
            return Err(BgvError::LengthMismatch {
                expected: selected,
                found: inputs.len(),
            }
            .into());
        }
        let unit = PlaintextBound { linf: 1, l1: 1 };
        let mut output: Option<CiphertextShape> = None;
        for input in inputs {
            if input.components() != 2 {
                return Err(BgvError::ComponentCount {
                    found: input.components(),
                }
                .into());
            }
            let product = profile.plan_mul_plain(input, unit)?;
            output = Some(match output {
                None => product,
                Some(sum) => profile.plan_add(&sum, &product)?,
            });
        }
        let output = output.ok_or(BgvError::Kernel)?;
        let payload = geometry.payload(
            profile,
            output.level(),
            1,
            usize::from(selected > 1) * 2,
            budget,
        )?;
        let operations = operations(profile, output.level(), selected, 0, selected - 1)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(inputs.len())
            .map_err(|_| RelativeTraceError::Allocation)?;
        owned.extend_from_slice(inputs);
        Ok(Self {
            geometry,
            inputs: owned,
            output,
            payload,
            operations,
        })
    }

    /// Exact public shape of the recomposed ciphertext.
    #[must_use]
    pub const fn output_shape(&self) -> &CiphertextShape {
        &self.output
    }

    /// Payload admitted by the plan, with the scope of [`RelativeTraceBudget`].
    #[must_use]
    pub const fn payload(&self) -> RelativeTraceBudget {
        self.payload
    }

    /// Exact high-level operations and shared multiplication-unit accounting.
    #[must_use]
    pub const fn operations(&self) -> RelativeTraceOperations {
        self.operations
    }

    /// Recompose every ordered bank using fixed public monomials.
    ///
    /// # Errors
    /// Any profile/count/shape mismatch before private arithmetic, allocation
    /// failure or a shared BGV error. Borrowed input banks remain owned by caller.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        inputs: &[Ciphertext],
    ) -> Result<Ciphertext, RelativeTraceError> {
        self.geometry.check_profile(profile)?;
        if inputs.len() != self.inputs.len() {
            return Err(BgvError::LengthMismatch {
                expected: self.inputs.len(),
                found: inputs.len(),
            }
            .into());
        }
        for (input, shape) in inputs.iter().zip(&self.inputs) {
            input.check_profile(profile)?;
            if input.shape() != *shape {
                return Err(RelativeTraceError::InputShape);
            }
        }
        let mut output: Option<Ciphertext> = None;
        for (r, input) in inputs.iter().enumerate() {
            let product = monomial_product(profile, input, r)?;
            output = Some(accumulate(profile, output, product)?);
        }
        let output = output.ok_or(BgvError::Kernel)?;
        debug_assert_eq!(output.shape(), self.output);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    //! Algebra, genuine encrypted round trips, exact public accounting and clearing.

    use super::*;
    use crate::bgv::{
        BgvParameters, KeySet,
        fixtures::{self, FixtureEntropy},
    };
    use crate::modular;
    use num_bigint::{BigInt, Sign};
    use std::cell::Cell;

    thread_local! {
        static SPLIT_CALLS: Cell<usize> = const { Cell::new(0) };
        static SPLIT_FAULT: Cell<(usize, bool)> = const { Cell::new((usize::MAX, false)) };
        static COUNTS: Cell<(usize, usize, usize, usize, usize)> = const { Cell::new((0,0,0,0,0)) };
        static FAIL_AT: Cell<usize> = const { Cell::new(0) };
        static PANIC_AT: Cell<usize> = const { Cell::new(0) };
        static CLEARED_AT_FAULT: Cell<usize> = const { Cell::new(0) };
    }
    fn reset() {
        SPLIT_CALLS.set(0);
        SPLIT_FAULT.set((usize::MAX, false));
        COUNTS.set((0, 0, 0, 0, 0));
        FAIL_AT.set(0);
        PANIC_AT.set(0);
    }
    pub(super) fn split_checkpoint() -> Result<(), RelativeTraceError> {
        let call = SPLIT_CALLS.get() + 1;
        SPLIT_CALLS.set(call);
        let (at, panic) = SPLIT_FAULT.get();
        if call == at {
            SPLIT_FAULT.set((usize::MAX, false));
            CLEARED_AT_FAULT.set(super::super::poly::tests::cleared_words());
            assert!(!panic, "owned parity split unwind");
            return Err(BgvError::Kernel.into());
        }
        Ok(())
    }
    pub(super) fn observe_plaintext() {
        COUNTS.with(|c| {
            let mut x = c.get();
            x.3 += 1;
            c.set(x);
        });
    }
    pub(super) fn observe_product() {
        COUNTS.with(|c| {
            let mut x = c.get();
            x.0 += 1;
            c.set(x);
        });
    }
    pub(super) fn observe_galois() {
        COUNTS.with(|c| {
            let mut x = c.get();
            x.1 += 1;
            c.set(x);
        });
    }
    pub(super) fn observe_add() {
        COUNTS.with(|c| {
            let mut x = c.get();
            x.2 += 1;
            c.set(x);
        });
    }
    pub(super) fn after_bank() -> Result<(), RelativeTraceError> {
        let bank = COUNTS.with(|c| {
            let mut x = c.get();
            x.4 += 1;
            c.set(x);
            x.4
        });
        if FAIL_AT.get() == bank {
            CLEARED_AT_FAULT.set(super::super::poly::tests::cleared_words());
            FAIL_AT.set(0);
            return Err(BgvError::Kernel.into());
        }
        if PANIC_AT.get() == bank {
            CLEARED_AT_FAULT.set(super::super::poly::tests::cleared_words());
            PANIC_AT.set(0);
            panic!("injected relative-trace unwind");
        }
        Ok(())
    }
    fn make_profile(log_n: u32, p: u64, primes: usize) -> BgvProfile {
        BgvProfile::new(
            BgvParameters::derive(log_n, p, 1, (40, primes), (41, 2), primes.min(2)).unwrap(),
        )
        .unwrap()
    }
    fn unlimited() -> RelativeTraceBudget {
        RelativeTraceBudget {
            output_residue_bytes: usize::MAX,
            scratch_residue_bytes: usize::MAX,
            plaintext_bytes: usize::MAX,
        }
    }
    fn make_keys(profile: &BgvProfile, elements: &[u32]) -> KeySet {
        fixtures::key_set(profile, elements, &mut FixtureEntropy::new([193; 64])).unwrap()
    }
    fn encrypted(profile: &BgvProfile, keys: &KeySet, coefficients: Vec<u64>) -> Ciphertext {
        fixtures::encrypt(
            profile,
            &keys.public,
            &Plaintext::new(profile, profile.master_plaintext(), coefficients).unwrap(),
            &mut FixtureEntropy::new([47; 64]),
        )
        .unwrap()
    }
    fn signed_shift(values: &[u64], exponent: usize, scale: u64, modulus: u64) -> Vec<u64> {
        let degree = values.len();
        let mut out = vec![0; degree];
        for (index, &value) in values.iter().enumerate() {
            let shifted = (index + exponent) % (2 * degree);
            let scaled = value * scale % modulus;
            out[shifted % degree] = if shifted < degree {
                scaled
            } else {
                (modulus - scaled) % modulus
            };
        }
        out
    }
    fn sigma(a: &[u64], power: usize, p: u64) -> Vec<u64> {
        let n = a.len();
        let mut out = vec![0; n];
        for (i, &v) in a.iter().enumerate() {
            let j = i * power % (2 * n);
            out[j % n] = if j < n { v } else { (p - v) % p };
        }
        out
    }
    fn expected_bank(a: &[u64], r: usize, d: usize) -> Vec<u64> {
        let mut out = vec![0; a.len()];
        for j in (r..a.len()).step_by(d) {
            out[j - r] = a[j];
        }
        out
    }
    // Independent BigInt CRT of the real decryption inner product. Test-only
    // synthetic oracle allocations are not a production secret-custody API.
    fn physical_noise(
        profile: &BgvProfile,
        keys: &KeySet,
        ciphertext: &Ciphertext,
        expected: &[u64],
    ) {
        let primes = &profile.parameters().main_primes[..=ciphertext.level()];
        let q = primes.iter().fold(BigInt::from(1), |x, &p| x * p);
        let half = (&q - 1_u32) / 2_u32;
        let basis: Vec<_> = primes
            .iter()
            .map(|&p| {
                let part = &q / p;
                let inv = part.modpow(&BigInt::from(p - 2), &BigInt::from(p));
                part * inv
            })
            .collect();
        let words = Zeroizing::new(
            fixtures::inner_product_words(profile, &keys.secret, ciphertext).unwrap(),
        );
        let declared = ciphertext
            .noise_bound()
            .as_natural()
            .words_le()
            .iter()
            .rev()
            .fold(BigInt::from(0), |x, &w| (x << 64) + w);
        assert!(declared <= half);
        let t = BigInt::from(profile.master_plaintext_modulus());
        for (j, &want) in expected.iter().enumerate() {
            let canonical = basis
                .iter()
                .enumerate()
                .fold(BigInt::from(0), |sum, (l, w)| {
                    sum + w * words[l * profile.degree() + j]
                })
                % &q;
            let centered = if canonical > half {
                canonical - &q
            } else {
                canonical
            };
            let magnitude = if centered.sign() == Sign::Minus {
                -&centered
            } else {
                centered.clone()
            };
            assert!(magnitude <= declared);
            assert!(magnitude <= half);
            assert_eq!(((&centered % &t) + &t) % &t, BigInt::from(want));
        }
    }
    fn assert_counts(plan: RelativeTraceOperations, banks: usize) {
        assert_eq!(
            COUNTS.get(),
            (
                plan.plaintext_products,
                plan.galois_applications,
                plan.ciphertext_additions,
                plan.plaintext_products,
                banks
            )
        );
    }

    fn scalar_tree(a: &[u64], geometry: &Geometry, p: u64) -> Vec<Vec<u64>> {
        fn visit(
            current: Vec<u64>,
            depth: usize,
            residue: usize,
            geometry: &Geometry,
            p: u64,
            output: &mut [Vec<u64>],
        ) {
            if depth == geometry.generators.len() {
                output[residue] = current;
                return;
            }
            let image = sigma(&current, geometry.generators[depth] as usize, p);
            let even: Vec<_> = current
                .iter()
                .zip(&image)
                .map(|(&a, &b)| (a + b) % p)
                .collect();
            let odd: Vec<_> = current
                .iter()
                .zip(&image)
                .map(|(&a, &b)| (a + p - b) % p)
                .collect();
            let shifted = signed_shift(&odd, 2 * geometry.degree - (1_usize << depth), 1, p);
            visit(even, depth + 1, residue, geometry, p, output);
            visit(
                shifted,
                depth + 1,
                residue + (1_usize << depth),
                geometry,
                p,
                output,
            );
        }
        let mut output = vec![Vec::new(); geometry.banks];
        let scaled = a.iter().map(|&v| v * geometry.inverse % p).collect();
        visit(scaled, 0, 0, geometry, p, &mut output);
        output
    }

    #[test]
    fn every_basis_and_small_slot_corner_match_direct_polynomial_decomposition() {
        for (log_n, p) in [
            (2, 5),
            (3, 5),
            (4, 5),
            (4, 17),
            (7, 257),
            (8, 257),
            (9, 257),
        ] {
            let profile = make_profile(log_n, p, 3);
            let geometry = Geometry::new(&profile, profile.master_plaintext()).unwrap();
            let n = profile.degree();
            let d = geometry.banks;
            let mut group = vec![1_usize];
            for &g in &geometry.generators {
                let images: Vec<_> = group.iter().map(|&h| h * g as usize % (2 * n)).collect();
                group.extend(images);
            }
            group.sort_unstable();
            assert_eq!(
                group,
                (1..2 * n).step_by(2 * geometry.slots).collect::<Vec<_>>()
            );
            for j in 0..n {
                let mut a = vec![0; n];
                a[j] = 1;
                let tree = scalar_tree(&a, &geometry, p);
                let mut recomposed = vec![0; n];
                for (r, tree_bank) in tree[..d].iter().enumerate() {
                    let mut bank = signed_shift(&a, (2 * n - r) % (2 * n), geometry.inverse, p);
                    for &g in &geometry.generators {
                        let image = sigma(&bank, g as usize, p);
                        for (x, y) in bank.iter_mut().zip(image) {
                            *x = (*x + y) % p;
                        }
                    }
                    assert_eq!(bank, expected_bank(&a, r, d));
                    assert_eq!(*tree_bank, bank);
                    for (x, y) in recomposed.iter_mut().zip(signed_shift(&bank, r, 1, p)) {
                        *x = (*x + y) % p;
                    }
                }
                assert_eq!(recomposed, a);
            }
        }
    }

    fn encrypted_roundtrip(profile: &BgvProfile, kinds: usize) {
        let encoding = SlotEncoding::new(profile, profile.master_plaintext()).unwrap();
        let plan = RelativeTraceDecompositionPlan::new(
            profile,
            &profile.plan_fresh(profile.master_plaintext()).unwrap(),
            unlimited(),
        )
        .unwrap();
        let keys = make_keys(profile, plan.galois_elements());
        let n = profile.degree();
        assert_eq!(plan.slots(), encoding.slots());
        assert_eq!(plan.banks(), encoding.stride());
        let recompose = RelativeTraceRecompositionPlan::new(
            profile,
            &vec![plan.output_shape().clone(); plan.banks()],
            unlimited(),
        )
        .unwrap();
        let mut rng = 0x1234_5678_9abc_def0_u64;
        for kind in 0..kinds {
            let input: Vec<_> = (0..n)
                .map(|i| match kind {
                    0 => 0,
                    1 => {
                        rng ^= rng << 13;
                        rng ^= rng >> 7;
                        rng ^= rng << 17;
                        rng % 257
                    }
                    2 => {
                        if i % 2 == 0 {
                            128
                        } else {
                            129
                        }
                    }
                    _ => {
                        if i == 0 || i == n - 1 || i == plan.banks() - 1 {
                            256
                        } else {
                            (i * i + 3 * i + 1) as u64 % 257
                        }
                    }
                })
                .collect();
            let ciphertext = encrypted(profile, &keys, input.clone());
            reset();
            let banks = plan
                .evaluate(profile, &ciphertext, &keys.evaluation)
                .unwrap();
            assert_counts(plan.operations(), plan.banks());
            for (r, bank) in banks.iter().enumerate() {
                assert_eq!(bank.shape(), *plan.output_shape());
                let expected = expected_bank(&input, r, plan.banks());
                let opened = keys.secret.decrypt(profile, bank).unwrap();
                assert_eq!(opened.coefficients(), expected.as_slice());
                let slots = encoding.decode(&opened).unwrap();
                for (j, &slot) in slots.iter().enumerate() {
                    let beta = modular::mod_pow_u64(encoding.root(), (2 * j + 1) as u64, 257);
                    let want = (0..encoding.slots())
                        .rev()
                        .fold(0, |v, k| (v * beta + input[k * plan.banks() + r]) % 257);
                    assert_eq!(slot, want);
                }
                physical_noise(profile, &keys, bank, &expected);
            }
            reset();
            let restored = recompose.evaluate(profile, &banks).unwrap();
            assert_counts(recompose.operations(), plan.banks());
            assert_eq!(restored.shape(), *recompose.output_shape());
            assert_eq!(
                keys.secret
                    .decrypt(profile, &restored)
                    .unwrap()
                    .coefficients(),
                input.as_slice()
            );
            physical_noise(profile, &keys, &restored, &input);
            assert_eq!(restored.level(), ciphertext.level());
            if kind == 3 && plan.banks() > 1 {
                // Different public input bounds are admitted without padding
                // or silently reusing the original all-equal recomposition.
                let mut altered = banks;
                altered[1] = profile.add(&altered[1], &altered[1]).unwrap();
                let shapes: Vec<_> = altered.iter().map(Ciphertext::shape).collect();
                assert_ne!(shapes[0], shapes[1]);
                let heterogeneous =
                    RelativeTraceRecompositionPlan::new(profile, &shapes, unlimited()).unwrap();
                let expected: Vec<_> = input
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        if i % plan.banks() == 1 {
                            2 * v % 257
                        } else {
                            v
                        }
                    })
                    .collect();
                reset();
                let result = heterogeneous.evaluate(profile, &altered).unwrap();
                assert_counts(heterogeneous.operations(), plan.banks());
                assert_eq!(result.shape(), *heterogeneous.output_shape());
                assert_eq!(
                    keys.secret
                        .decrypt(profile, &result)
                        .unwrap()
                        .coefficients(),
                    expected.as_slice()
                );
                physical_noise(profile, &keys, &result, &expected);
            }
        }
    }

    #[test]
    fn encrypted_full_ring_zero_dense_and_adversarial_vectors_roundtrip_at_d1_d2_d4() {
        for log_n in [7, 8, 9] {
            encrypted_roundtrip(&make_profile(log_n, 257, 3), 4);
        }
    }

    #[test]
    fn exact_noise_work_and_payload_boundaries_follow_the_runtime_order() {
        let profile = make_profile(9, 257, 3);
        let input = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let plan = RelativeTraceDecompositionPlan::new(&profile, &input, unlimited()).unwrap();
        let d = plan.banks();
        let k = profile.key_switch_noise(input.level()).unwrap();
        let expected = input
            .bound()
            .mul_u64(d as u64 * plan.core.geometry.inverse_bound.l1)
            .add(&k.mul_u64((d - 1) as u64));
        assert_eq!(plan.output_shape().bound(), &expected);
        let b = plan.payload();
        let bytes = usize::try_from(
            CostModel::new(&profile)
                .ciphertext_residue_bytes(input.level(), 2)
                .unwrap(),
        )
        .expect("small fixture ciphertext");
        assert_eq!(
            b,
            RelativeTraceBudget {
                output_residue_bytes: d * bytes,
                scratch_residue_bytes: 2 * bytes,
                plaintext_bytes: 8 * profile.degree()
            }
        );
        assert_eq!(
            plan.operations().arithmetic_units,
            (d as u64)
                * CostModel::new(&profile)
                    .plaintext_multiplication_units(input.level())
                    .unwrap()
                + (d - 1) as u64
                    * CostModel::new(&profile)
                        .automorphism_units(input.level())
                        .unwrap()
        );
        assert_eq!(
            RelativeTraceDecompositionPlan::new(&profile, &input, b).unwrap(),
            plan
        );
        let shapes = vec![plan.output_shape().clone(); d];
        let reverse = RelativeTraceRecompositionPlan::new(&profile, &shapes, unlimited()).unwrap();
        assert_eq!(reverse.output_shape().bound(), &expected.mul_u64(d as u64));
        assert_eq!(reverse.payload().output_residue_bytes, bytes);
        assert_eq!(reverse.payload().scratch_residue_bytes, 2 * bytes);
        assert_eq!(
            reverse.operations().arithmetic_units,
            d as u64
                * CostModel::new(&profile)
                    .plaintext_multiplication_units(input.level())
                    .unwrap()
        );
        for kind in [
            RelativeTracePayload::Output,
            RelativeTracePayload::Scratch,
            RelativeTracePayload::Plaintext,
        ] {
            let short = |mut b: RelativeTraceBudget| {
                match kind {
                    RelativeTracePayload::Output => b.output_residue_bytes -= 1,
                    RelativeTracePayload::Scratch => b.scratch_residue_bytes -= 1,
                    RelativeTracePayload::Plaintext => b.plaintext_bytes -= 1,
                }
                b
            };
            assert!(
                matches!(RelativeTraceDecompositionPlan::new(&profile,&input,short(b)),Err(RelativeTraceError::Budget{kind:k,..}) if k==kind)
            );
            assert!(
                matches!(RelativeTraceRecompositionPlan::new(&profile,&shapes,short(reverse.payload())),Err(RelativeTraceError::Budget{kind:k,..}) if k==kind)
            );
        }
        assert_eq!(
            checked_mul(usize::MAX, 2),
            Err(RelativeTraceError::SizeOverflow)
        );
        let identity = make_profile(7, 257, 3);
        let identity_plan = RelativeTraceDecompositionPlan::new(
            &identity,
            &identity.plan_fresh(identity.master_plaintext()).unwrap(),
            unlimited(),
        )
        .unwrap();
        assert_eq!(identity_plan.banks(), 1);
        assert!(identity_plan.galois_elements().is_empty());
        assert_eq!(identity_plan.payload().scratch_residue_bytes, 0);
    }

    #[test]
    fn profile_shape_key_and_capacity_refusals_precede_request_allocation() {
        let profile = make_profile(8, 257, 3);
        let plan = RelativeTraceDecompositionPlan::new(
            &profile,
            &profile.plan_fresh(profile.master_plaintext()).unwrap(),
            unlimited(),
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![1; profile.degree()]);
        let other = make_profile(8, 257, 4);
        let foreign_keys = make_keys(&other, plan.galois_elements());
        let foreign = encrypted(&other, &foreign_keys, vec![1; other.degree()]);
        reset();
        assert_eq!(
            plan.evaluate(&other, &foreign, &foreign_keys.evaluation),
            Err(BgvError::ProfileMismatch.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        reset();
        assert_eq!(
            plan.evaluate(&profile, &foreign, &keys.evaluation),
            Err(BgvError::ProfileMismatch.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        reset();
        assert_eq!(
            plan.evaluate(&profile, &input, &foreign_keys.evaluation),
            Err(BgvError::ProfileMismatch.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        let missing = make_keys(&profile, &[]);
        reset();
        assert_eq!(
            plan.evaluate(&profile, &input, &missing.evaluation),
            Err(BgvError::WrongSwitchingKey.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        let doubled = profile.add(&input, &input).unwrap();
        reset();
        assert_eq!(
            plan.evaluate(&profile, &doubled, &keys.evaluation),
            Err(RelativeTraceError::InputShape)
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        let tensor = profile.plan_tensor(&input.shape(), &input.shape()).unwrap();
        assert!(matches!(
            RelativeTraceDecompositionPlan::new(&profile, &tensor, unlimited()),
            Err(RelativeTraceError::Bgv(BgvError::ComponentCount {
                found: 3
            }))
        ));
        let mut full = input.shape();
        while let Ok(next) = profile.plan_add(&full, &full) {
            full = next;
        }
        assert!(matches!(
            RelativeTraceDecompositionPlan::new(&profile, &full, unlimited()),
            Err(RelativeTraceError::Bgv(
                BgvError::NoiseBudgetExceeded { .. }
            ))
        ));
        let unavailable = make_profile(3, 3, 3);
        assert_eq!(
            RelativeTraceDecompositionPlan::new(
                &unavailable,
                &unavailable
                    .plan_fresh(unavailable.master_plaintext())
                    .unwrap(),
                unlimited()
            ),
            Err(BgvError::EncodingUnavailable.into())
        );
        let banks = plan.evaluate(&profile, &input, &keys.evaluation).unwrap();
        let shapes: Vec<_> = banks.iter().map(Ciphertext::shape).collect();
        let reverse = RelativeTraceRecompositionPlan::new(&profile, &shapes, unlimited()).unwrap();
        assert!(matches!(
            RelativeTraceRecompositionPlan::new(&profile, &[], unlimited()),
            Err(RelativeTraceError::Bgv(BgvError::LengthMismatch { .. }))
        ));
        let mut wrong = shapes.clone();
        wrong[1] = profile.plan_switch_down(&wrong[1]).unwrap();
        assert!(matches!(
            RelativeTraceRecompositionPlan::new(&profile, &wrong, unlimited()),
            Err(RelativeTraceError::Bgv(BgvError::LevelMismatch { .. }))
        ));
        wrong[1] = tensor;
        assert!(matches!(
            RelativeTraceRecompositionPlan::new(&profile, &wrong, unlimited()),
            Err(RelativeTraceError::Bgv(BgvError::ComponentCount {
                found: 3
            }))
        ));
        reset();
        assert!(matches!(
            reverse.evaluate(&profile, &banks[..1]),
            Err(RelativeTraceError::Bgv(BgvError::LengthMismatch { .. }))
        ));
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        let mut changed = banks.clone();
        changed[1] = profile.add(&banks[1], &banks[1]).unwrap();
        reset();
        assert_eq!(
            reverse.evaluate(&profile, &changed),
            Err(RelativeTraceError::InputShape)
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        changed[1] = foreign;
        reset();
        assert_eq!(
            reverse.evaluate(&profile, &changed),
            Err(BgvError::ProfileMismatch.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
        reset();
        assert_eq!(
            reverse.evaluate(&other, &banks),
            Err(BgvError::ProfileMismatch.into())
        );
        assert_eq!(COUNTS.get(), (0, 0, 0, 0, 0));
    }

    #[test]
    fn returned_owners_and_all_partial_results_clear_on_error_and_unwind() {
        let profile = make_profile(8, 257, 3);
        let plan = RelativeTraceDecompositionPlan::new(
            &profile,
            &profile.plan_fresh(profile.master_plaintext()).unwrap(),
            unlimited(),
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![256; profile.degree()]);
        let cleared = super::super::poly::tests::cleared_words;
        reset();
        let before = cleared();
        let banks = plan.evaluate(&profile, &input, &keys.evaluation).unwrap();
        let before_drop = cleared();
        drop(banks);
        assert_eq!(
            cleared() - before_drop,
            plan.payload().output_residue_bytes / 8
        );
        let total = cleared() - before;
        reset();
        FAIL_AT.set(plan.banks());
        let before = cleared();
        assert_eq!(
            plan.evaluate(&profile, &input, &keys.evaluation),
            Err(BgvError::Kernel.into())
        );
        assert_eq!(cleared() - before, total);
        reset();
        PANIC_AT.set(plan.banks());
        let before = cleared();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plan.evaluate(&profile, &input, &keys.evaluation)
        }));
        assert!(panic.is_err());
        assert_eq!(cleared() - before, total);
        reset();
        let banks = plan.evaluate(&profile, &input, &keys.evaluation).unwrap();
        let reverse = RelativeTraceRecompositionPlan::new(
            &profile,
            &banks.iter().map(Ciphertext::shape).collect::<Vec<_>>(),
            unlimited(),
        )
        .unwrap();
        reset();
        let before = cleared();
        let output = reverse.evaluate(&profile, &banks).unwrap();
        let before_drop = cleared();
        drop(output);
        assert_eq!(
            cleared() - before_drop,
            reverse.payload().output_residue_bytes / 8
        );
        let total = cleared() - before;
        reset();
        FAIL_AT.set(plan.banks());
        let before = cleared();
        assert_eq!(
            reverse.evaluate(&profile, &banks),
            Err(BgvError::Kernel.into())
        );
        assert_eq!(cleared() - before, total);
        reset();
        PANIC_AT.set(plan.banks());
        let before = cleared();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reverse.evaluate(&profile, &banks)
        }));
        assert!(panic.is_err());
        assert_eq!(cleared() - before, total);
        reset();
        assert_eq!(
            keys.secret
                .decrypt(&profile, &input)
                .unwrap()
                .coefficients(),
            vec![256; profile.degree()]
        );
    }

    #[test]
    #[ignore = "full N16384 q6 relative trace: 128 banks and 127 key switches; run explicitly"]
    fn n14_q6_full_geometry_roundtrip_with_physical_noise() {
        let start = std::time::Instant::now();
        encrypted_roundtrip(
            &BgvProfile::candidate(super::super::BgvCandidate::AffineF257).unwrap(),
            2,
        );
        println!(
            "relative_trace n=16384 q=6 banks=128 cases=2 elapsed_seconds={:.6}",
            start.elapsed().as_secs_f64()
        );
    }

    #[test]
    fn raw_trace_owner_checks_complete_geometry_and_clears_every_refused_bank() {
        reset();
        let profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 3, (50, 4), (60, 2), 2).unwrap()).unwrap();
        let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let plan = CoefficientTracePlan::new(
            &profile,
            &shape,
            unlimited(),
            BankRetention::Collected,
            CoefficientBankSelection::All,
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![124; profile.degree()]);
        let expected_words = 2 * profile.degree() * (shape.level() + 1) * plan.banks();
        for refusal in 0..4 {
            let raw = plan.evaluate(&profile, &input, &keys.evaluation).unwrap();
            let wrong_shape = profile
                .plan_fresh(profile.plaintext_precision(1).unwrap())
                .unwrap();
            let target_shape = if refusal == 0 {
                &wrong_shape
            } else {
                plan.output_shape()
            };
            let slots = plan.slots() + usize::from(refusal == 1);
            let banks = plan.banks() + usize::from(refusal == 2);
            let foreign =
                BgvProfile::new(BgvParameters::derive(4, 5, 2, (50, 4), (60, 2), 2).unwrap())
                    .unwrap();
            let target_profile = if refusal == 3 { &foreign } else { &profile };
            let before = super::super::poly::tests::cleared_words();
            let result = raw.into_values(target_profile, target_shape, slots, banks);
            assert!(matches!(
                result,
                Err(RelativeTraceError::InputShape
                    | RelativeTraceError::Bgv(BgvError::ProfileMismatch))
            ));
            assert_eq!(
                super::super::poly::tests::cleared_words() - before,
                expected_words
            );
        }
        let raw = plan.evaluate(&profile, &input, &keys.evaluation).unwrap();
        let pointers: Vec<_> = raw
            .values
            .iter()
            .map(|bank| bank.components()[0].words().as_ptr())
            .collect();
        let values = raw
            .into_values(&profile, plan.output_shape(), plan.slots(), plan.banks())
            .unwrap();
        assert_eq!(
            pointers,
            values
                .iter()
                .map(|bank| bank.components()[0].words().as_ptr())
                .collect::<Vec<_>>()
        );
        let before = super::super::poly::tests::cleared_words();
        drop(values);
        assert_eq!(
            super::super::poly::tests::cleared_words() - before,
            expected_words
        );
        reset();
    }

    #[test]
    fn raw_trace_first_last_error_and_unwind_clear_the_exact_pending_frontier() {
        reset();
        let profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 3, (50, 4), (60, 2), 2).unwrap()).unwrap();
        let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let plan = CoefficientTracePlan::new(
            &profile,
            &shape,
            unlimited(),
            BankRetention::Collected,
            CoefficientBankSelection::All,
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![124; profile.degree()]);
        for at in [1, plan.banks()] {
            for panic in [false, true] {
                reset();
                if panic {
                    PANIC_AT.set(at);
                } else {
                    FAIL_AT.set(at);
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plan.evaluate(&profile, &input, &keys.evaluation)
                }));
                assert!(if panic {
                    result.is_err()
                } else {
                    matches!(result, Ok(Err(RelativeTraceError::Bgv(BgvError::Kernel))))
                });
                let frontier = if at == 1 {
                    1 + plan.banks().ilog2() as usize
                } else {
                    plan.banks()
                };
                assert_eq!(
                    super::super::poly::tests::cleared_words() - CLEARED_AT_FAULT.get(),
                    frontier * 2 * profile.degree() * (shape.level() + 1)
                );
            }
        }
        reset();
    }
    #[test]
    fn consuming_cursor_clears_every_split_temporary_and_pending_sibling() {
        reset();
        let profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 3, (50, 4), (60, 2), 2).unwrap()).unwrap();
        let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let plan = CoefficientTracePlan::new(
            &profile,
            &shape,
            unlimited(),
            BankRetention::Streaming,
            CoefficientBankSelection::All,
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![124; profile.degree()]);
        let h = plan.banks().ilog2() as usize;
        let mut stack = vec![0_usize];
        let mut expected = Vec::new();
        while let Some(depth) = stack.pop() {
            if depth == h {
                continue;
            }
            let pending = stack.len();
            expected.extend([pending + 2, pending + 3, pending + 4, pending + 2]);
            stack.push(depth + 1);
            stack.push(depth + 1);
        }
        for (index, owners) in expected.into_iter().enumerate() {
            for panic in [false, true] {
                reset();
                SPLIT_FAULT.set((index + 1, panic));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || -> Result<(), RelativeTraceError> {
                        let mut cursor = plan.core.start(&profile, &input, &keys.evaluation)?;
                        while let Some((value, _, rest)) =
                            cursor.advance(&profile, &keys.evaluation)?
                        {
                            drop(value);
                            cursor = rest;
                        }
                        Ok(())
                    },
                ));
                assert!(if panic {
                    result.is_err()
                } else {
                    matches!(result, Ok(Err(RelativeTraceError::Bgv(BgvError::Kernel))))
                });
                assert_eq!(
                    super::super::poly::tests::cleared_words() - CLEARED_AT_FAULT.get(),
                    owners * 2 * profile.degree() * (shape.level() + 1)
                );
            }
        }
        assert!(cursor_metadata_bytes(&profile, 0, CoefficientBankSelection::All).is_err());
        assert!(cursor_metadata_bytes(&profile, 3, CoefficientBankSelection::All).is_err());
        reset();
    }

    #[test]
    fn selected_zero_trace_has_one_real_even_path_and_exact_public_work() {
        for (log_n, p) in [(7, 257), (8, 257), (4, 5)] {
            let profile = make_profile(log_n, p, 4);
            let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
            for retention in [BankRetention::Collected, BankRetention::Streaming] {
                let plan = CoefficientTracePlan::new(
                    &profile,
                    &shape,
                    unlimited(),
                    retention,
                    CoefficientBankSelection::Zero,
                )
                .unwrap();
                let d = plan.banks();
                let h = d.ilog2() as usize;
                assert_eq!(plan.selected_banks(), 1);
                assert_eq!(plan.slots() * d, profile.degree());
                assert_eq!(
                    plan.operations(),
                    operations(&profile, shape.level(), 1, h, h).unwrap()
                );
                let exact = plan.core.payload();
                CoefficientTracePlan::new(
                    &profile,
                    &shape,
                    exact,
                    retention,
                    CoefficientBankSelection::Zero,
                )
                .unwrap();
                for kind in [
                    RelativeTracePayload::Output,
                    RelativeTracePayload::Scratch,
                    RelativeTracePayload::Plaintext,
                ] {
                    let mut low = exact;
                    let bound = match kind {
                        RelativeTracePayload::Output => &mut low.output_residue_bytes,
                        RelativeTracePayload::Scratch => &mut low.scratch_residue_bytes,
                        RelativeTracePayload::Plaintext => &mut low.plaintext_bytes,
                    };
                    if *bound == 0 {
                        continue;
                    }
                    *bound -= 1;
                    assert!(
                        matches!(CoefficientTracePlan::new(&profile, &shape, low, retention, CoefficientBankSelection::Zero), Err(RelativeTraceError::Budget { kind: found, .. }) if found == kind)
                    );
                }
                let keys = make_keys(&profile, plan.galois_elements());
                let values: Vec<_> = (0..profile.degree())
                    .map(|i| (i as u64 * 7 + 3) % p)
                    .collect();
                let input = encrypted(&profile, &keys, values.clone());
                reset();
                let cursor = plan.start(&profile, input, &keys.evaluation).unwrap();
                let (bank, rest) = cursor.advance(&profile, &keys.evaluation).unwrap().unwrap();
                assert_eq!(bank.ordinal, 0);
                assert_eq!(bank.banks, d);
                assert_eq!(bank.selection, CoefficientBankSelection::Zero);
                assert!(rest.advance(&profile, &keys.evaluation).unwrap().is_none());
                assert_counts(plan.operations(), 1);
                assert_eq!(SPLIT_CALLS.get(), 2 * h);
                let expected: Vec<_> = values
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| if i % d == 0 { v * d as u64 % p } else { 0 })
                    .collect();
                let (value, ordinal) = bank
                    .into_value(
                        &profile,
                        plan.output_shape(),
                        plan.slots(),
                        d,
                        CoefficientBankSelection::Zero,
                    )
                    .unwrap();
                assert_eq!(ordinal, 0);
                physical_noise(&profile, &keys, &value, &expected);
                reset();
            }
        }
        let selection = CoefficientBankSelection::All;
        assert_eq!(
            (0..8)
                .map(|i| selection.expected_ordinal(i, 8).unwrap())
                .collect::<Vec<_>>(),
            [0, 4, 2, 6, 1, 5, 3, 7]
        );
        assert_eq!(selection.expected_ordinal(8, 8), None);
        assert_eq!(CoefficientBankSelection::Zero.expected_ordinal(1, 8), None);
        assert_eq!(CoefficientBankSelection::Zero.expected_ordinal(0, 0), None);
        assert_eq!(CoefficientBankSelection::Zero.expected_ordinal(0, 3), None);
    }

    #[test]
    fn selected_zero_trace_refuses_wrong_origin_and_clears_even_only_failures() {
        let profile = make_profile(4, 5, 4);
        let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
        let plan = CoefficientTracePlan::new(
            &profile,
            &shape,
            unlimited(),
            BankRetention::Streaming,
            CoefficientBankSelection::Zero,
        )
        .unwrap();
        let keys = make_keys(&profile, plan.galois_elements());
        let input = encrypted(&profile, &keys, vec![4; profile.degree()]);
        let one_words = 2 * profile.degree() * (shape.level() + 1);
        let (bank, rest) = plan
            .start(&profile, input.clone(), &keys.evaluation)
            .unwrap()
            .advance(&profile, &keys.evaluation)
            .unwrap()
            .unwrap();
        let before = super::super::poly::tests::cleared_words();
        assert!(
            bank.into_value(
                &profile,
                plan.output_shape(),
                plan.slots(),
                plan.banks(),
                CoefficientBankSelection::All
            )
            .is_err()
        );
        assert_eq!(
            super::super::poly::tests::cleared_words() - before,
            one_words
        );
        drop(rest);
        for at in 1..=2 * plan.banks().ilog2() as usize {
            for panic in [false, true] {
                reset();
                SPLIT_FAULT.set((at, panic));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plan.core
                        .start(&profile, &input, &keys.evaluation)
                        .unwrap()
                        .advance(&profile, &keys.evaluation)
                }));
                assert!(if panic {
                    result.is_err()
                } else {
                    matches!(result, Ok(Err(RelativeTraceError::Bgv(BgvError::Kernel))))
                });
                // Exactly current+image, then current+image+even; no siblings.
                assert_eq!(
                    super::super::poly::tests::cleared_words() - CLEARED_AT_FAULT.get(),
                    (2 + (at + 1) % 2) * one_words
                );
            }
        }
        assert!(
            cursor_metadata_bytes(&profile, 8, CoefficientBankSelection::Zero).unwrap()
                < cursor_metadata_bytes(&profile, 8, CoefficientBankSelection::All).unwrap()
        );
        reset();
    }
}
