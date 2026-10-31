//! Fixed public encrypted execution of the complete upper-digit tableau.
//!
//! The sole input owner is [`CoefficientSlotBanks`], produced by real relative
//! trace and inverse Vandermonde evaluation. The immutable public program is
//! lowered to an alias-aware, move-only schedule. Its private division edges
//! exist only after the matching scalar-tableau subtraction; no generic
//! ciphertext, frame, claimed shape or caller-authored instruction can request
//! division or mint scalar provenance.
//!
//! This stage alone is conditional arithmetic, not authentication or a bootstrap.
//! [`super::LocalBootstrapKey`] composes it with a locally generated matching
//! encrypted secret and the complete coefficient transforms. TODO(C.8, R.9):
//! complete external source/key proofs, application resource and parameter
//! qualification, and circuit privacy.

use super::relative_trace::BankRetention;
use super::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, CoefficientSlotBanks, Plaintext,
    PlaintextBound, SwitchingKey, SwitchingKeyKind, cost::CostModel, poly::RnsPoly,
};
use crate::prime_power::UpperDigitsProgram;
use core::mem::size_of;
use thiserror::Error;

mod compiler;
mod planning;
use compiler::{Node, Operation};

/// Explicit local ceilings for compilation and one complete bank-set execution.
///
/// These numeric limits are not an original proof-resource ledger or a security
/// qualification. Requested element payloads include newly retained output-bank
/// headers and conservative public-bound capacities. They exclude incoming
/// owner metadata, allocator headers/over-allocation, profile/key storage and
/// fixed arithmetic stack frames. Backend scratch counts RNS residue buffers;
/// fixed conversion digits/scalars and public contexts are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperDigitsBudget {
    /// Visits of public coefficients and emitted nodes in the counting pass.
    pub compiler_steps: u64,
    /// Retained program and public compiler/runtime metadata element payload.
    pub metadata_bytes: usize,
    /// Complete bank-set, live arena, tensor and switch-output residue payload.
    pub owned_residue_bytes: usize,
    /// One public scalar plaintext coefficient buffer.
    pub plaintext_bytes: usize,
    /// Additional canonical tensor/key-switch/plaintext kernel residue scratch.
    pub backend_scratch_bytes: usize,
    /// Conservative preallocation ceiling in existing shared CostModel units.
    pub canonical_arithmetic_units: u64,
    /// Conservative preallocation ceiling for add/negate/private-division words.
    pub scalar_word_operations: u64,
}

/// Public operation counts for one scalar bank; all banks use the same schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpperDigitsOperations {
    /// Tensor plus relinearization calls.
    pub ciphertext_multiplications: u64,
    /// Public non-unit scalar products using direct residue scaling, without NTTs.
    pub plaintext_multiplications: u64,
    /// Public nonzero scalar additions.
    pub plaintext_additions: u64,
    /// Ciphertext additions and subtractions, including tableau differences.
    pub ciphertext_additions: u64,
    /// Unit-sign negations.
    pub negations: u64,
    /// Private, matched tableau precision reductions.
    pub exact_divisions: u64,
    /// Individual main-prime drops, counting every live owner.
    pub modulus_switches: u64,
}

/// Admitted scoped payloads and work of the complete public schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperDigitsPayload {
    /// Public compiler/program/runtime metadata element upper bound.
    pub metadata_bytes: usize,
    /// Admitted retained-input/output and arithmetic-result residue upper bound.
    /// Public complete-bank plans include all banks; private streaming plans
    /// include the single active bank, with external frontier charged by bootstrap.
    pub owned_residue_bytes: usize,
    /// One bank's exact public replay, including tensor/result and switch overlap.
    pub single_bank_residue_bytes: usize,
    /// One scalar-addition plaintext's coefficient elements; products allocate none.
    pub plaintext_bytes: usize,
    /// Separate shared kernel RNS residue scratch; fixed stack scratch is excluded.
    pub backend_scratch_bytes: usize,
    /// Work bound checked before node/shape array allocation.
    pub preallocation_arithmetic_units: u64,
    /// Word-operation bound checked before node/shape array allocation.
    pub preallocation_scalar_word_operations: u64,
    /// Exact CostModel arithmetic units of all banks at their selected levels.
    pub canonical_arithmetic_units: u64,
    /// Exact add/negate/private-division word operations across all banks.
    pub scalar_word_operations: u64,
}

/// A public preparation or consumed execution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum UpperDigitsError {
    /// A public count or byte/work expression overflowed.
    #[error("upper-digit geometry overflow")]
    SizeOverflow,
    /// The immutable program differs from its canonical context or tableau.
    #[error("upper-digit program invariant failed")]
    Program,
    /// The source owner does not match the prepared shape, precision or geometry.
    #[error("upper-digit source mismatch")]
    Source,
    /// Public levels are missing, extra, increasing or outside the profile.
    #[error("upper-digit level schedule is invalid")]
    Levels,
    /// The allocation-free counting traversal exceeded its explicit ceiling.
    #[error("upper-digit compiler work budget exceeded")]
    CompilerBudget,
    /// One explicit local resource ceiling is insufficient.
    #[error("upper-digit budget exceeded for {resource}")]
    Budget {
        /// Public resource category.
        resource: &'static str,
    },
    /// A fallible metadata or output allocation was refused.
    #[error("upper-digit allocation refused")]
    Allocation,
    /// An existing canonical BGV check or operation refused.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Complete immutable public schedule, bound to genuine coefficient-slot geometry.
///
/// Preparation owns the validated public program. The type exposes neither
/// arbitrary instructions nor an input/precision cast. All ciphertext execution
/// remains in existing BGV methods except the private in-place, proven division.
pub struct EncryptedUpperDigitsPlan {
    program: UpperDigitsProgram,
    input: CiphertextShape,
    output: CiphertextShape,
    banks: usize,
    slots: usize,
    retention: BankRetention,
    nodes: Vec<Node>,
    order: Vec<usize>,
    uses: Vec<usize>,
    levels: Vec<usize>,
    inverses: Vec<u64>,
    output_id: usize,
    compiler_steps: u64,
    operations: UpperDigitsOperations,
    payload: UpperDigitsPayload,
}

impl core::fmt::Debug for EncryptedUpperDigitsPlan {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EncryptedUpperDigitsPlan")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("banks", &self.banks)
            .field("slots", &self.slots)
            .field("nodes", &self.nodes.len())
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}

fn same_shape(value: &Ciphertext, shape: &CiphertextShape) -> bool {
    value.level() == shape.level()
        && value.component_count() == shape.components()
        && value.plaintext_precision() == shape.plaintext_precision()
        && value.noise_bound().as_natural() == shape.bound()
}

fn product(a: usize, b: usize) -> Result<usize, UpperDigitsError> {
    a.checked_mul(b).ok_or(UpperDigitsError::SizeOverflow)
}
fn sum(a: usize, b: usize) -> Result<usize, UpperDigitsError> {
    a.checked_add(b).ok_or(UpperDigitsError::SizeOverflow)
}
fn word(value: usize) -> Result<u64, UpperDigitsError> {
    u64::try_from(value).map_err(|_| UpperDigitsError::SizeOverflow)
}
pub(super) fn reserve<T>(capacity: usize) -> Result<Vec<T>, UpperDigitsError> {
    #[cfg(test)]
    tests::allocation_checkpoint()?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| UpperDigitsError::Allocation)?;
    Ok(values)
}

impl EncryptedUpperDigitsPlan {
    // Exact requirements of this already selected immutable realization. Reuse
    // cannot select a different depth schedule, split, or preallocation bound.
    pub(super) fn required_budget(&self) -> UpperDigitsBudget {
        UpperDigitsBudget {
            compiler_steps: self.compiler_steps,
            metadata_bytes: self.payload.metadata_bytes,
            owned_residue_bytes: self.payload.owned_residue_bytes,
            plaintext_bytes: self.payload.plaintext_bytes,
            backend_scratch_bytes: self.payload.backend_scratch_bytes,
            canonical_arithmetic_units: self.payload.preallocation_arithmetic_units,
            scalar_word_operations: self.payload.preallocation_scalar_word_operations,
        }
    }
    // Node/program/arena arrays retain their already admitted metadata scope.
    // These two outer shape bounds are charged by the enclosing composition.
    pub(super) fn outer_metadata_heap(&self) -> Result<usize, super::PlanMetadataError> {
        super::metadata::add(
            super::metadata::shape_heap(&self.input)?,
            super::metadata::shape_heap(&self.output)?,
        )
    }

    /// Count/admit metadata and work, then replay every canonical shape transfer.
    ///
    /// `levels` gives one nonincreasing ciphertext level per multiplicative
    /// depth. It is public and cannot depend on bank contents. Every source
    /// precision must equal the program's initial precision under this master.
    ///
    /// # Errors
    /// Context/program/level mismatch, budget/allocation or canonical noise refusal.
    pub fn new(
        profile: &BgvProfile,
        program: UpperDigitsProgram,
        source: &CoefficientSlotBanks,
        levels: &[usize],
        budget: UpperDigitsBudget,
    ) -> Result<Self, UpperDigitsError> {
        source.shape().check_profile(profile)?;
        for bank in source.banks() {
            bank.check_profile(profile)?;
            if !same_shape(bank, source.shape()) {
                return Err(UpperDigitsError::Source);
            }
        }
        Self::from_shape(
            profile,
            program,
            source.shape(),
            source.slots(),
            source.banks().len(),
            levels,
            budget,
            BankRetention::Collected,
        )
    }

    // Planning a complete composed operation must not require executing its
    // preceding transform first. This sole shared planner mints no scalar bank
    // owner: execution still consumes actual complete or per-coordinate coefficient origin.
    pub(super) fn from_shape(
        profile: &BgvProfile,
        program: UpperDigitsProgram,
        input: &CiphertextShape,
        slots: usize,
        banks: usize,
        levels: &[usize],
        budget: UpperDigitsBudget,
        retention: BankRetention,
    ) -> Result<Self, UpperDigitsError> {
        input.check_profile(profile)?;
        let context = program.plan().context();
        let precision = input.plaintext_precision();
        if input.components() != 2
            || banks == 0
            || slots.checked_mul(banks) != Some(profile.degree())
            || super::SlotEncoding::slot_count(profile, precision)? != slots
            || context.prime() != precision.prime()
            || context.precision() != precision.exponent()
            || context.modulus() != precision.modulus()
        {
            return Err(UpperDigitsError::Source);
        }
        let (count, output_id, depth) =
            compiler::compile(&program, compiler::Count::new(budget.compiler_steps))?;
        if levels.len() != depth
            || levels.windows(2).any(|pair| pair[0] < pair[1])
            || levels.first().is_some_and(|&level| level > input.level())
        {
            return Err(UpperDigitsError::Levels);
        }
        let preflight = planning::preallocation(
            profile,
            &program,
            count.nodes,
            count.operations,
            banks,
            levels.len(),
            retention,
            budget,
        )?;
        let nodes = reserve(count.nodes)?;
        let (store, built_output, built_depth) = compiler::compile(
            &program,
            compiler::Store {
                nodes,
                limit: count.nodes,
            },
        )?;
        if store.nodes.len() != count.nodes || built_output != output_id || built_depth != depth {
            return Err(UpperDigitsError::Program);
        }
        let mut order = reserve(count.nodes)?;
        order.extend(0..count.nodes);
        order.sort_unstable_by_key(|&i| (store.nodes[i].depth, i));
        let mut uses = reserve(count.nodes)?;
        uses.resize(count.nodes, 0_usize);
        for node in &store.nodes {
            let (args, len) = node.operation.arguments();
            for &arg in &args[..len] {
                let usage = uses.get_mut(arg).ok_or(UpperDigitsError::Program)?;
                *usage = usage.checked_add(1).ok_or(UpperDigitsError::SizeOverflow)?;
            }
        }
        uses[output_id] = uses[output_id]
            .checked_add(1)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        let (output, operations, payload) = planning::replay(
            profile,
            input,
            &store.nodes,
            &order,
            &uses,
            levels,
            output_id,
            count.operations,
            banks,
            retention,
            budget,
            preflight,
        )?;
        let mut retained_levels = reserve(levels.len())?;
        retained_levels.extend_from_slice(levels);
        let mut inverses = reserve(input.level() + 1)?;
        for limb in 0..=input.level() {
            let fixed = profile.main_plan(limb).fixed();
            let inverse = fixed.power(context.prime(), fixed.modulus - 2);
            if fixed.multiply(context.prime(), inverse) != 1 {
                return Err(UpperDigitsError::Program);
            }
            inverses.push(inverse);
        }
        Ok(Self {
            program,
            input: input.clone(),
            output,
            banks,
            slots,
            retention,
            nodes: store.nodes,
            order,
            uses,
            levels: retained_levels,
            inverses,
            output_id,
            compiler_steps: count.visits,
            operations,
            payload,
        })
    }

    /// Exact public output shape of every successfully transformed bank.
    #[must_use]
    pub const fn output_shape(&self) -> &CiphertextShape {
        &self.output
    }
    /// Exact public input shape, including effective precision and master identity.
    #[must_use]
    pub const fn input_shape(&self) -> &CiphertextShape {
        &self.input
    }
    /// Scoped payloads and complete-bank-set work accounting.
    #[must_use]
    pub const fn payload(&self) -> UpperDigitsPayload {
        self.payload
    }
    /// Counts for one bank; multiply by the public bank count for the request.
    #[must_use]
    pub const fn operations(&self) -> UpperDigitsOperations {
        self.operations
    }
    /// Number of consumed/returned banks.
    #[must_use]
    pub const fn banks(&self) -> usize {
        self.banks
    }
    /// Number of coefficient-valued scalar slots in every bank.
    #[must_use]
    pub const fn slots(&self) -> usize {
        self.slots
    }

    /// Consume the genuine source owner and execute its complete fixed tableau.
    ///
    /// # Errors
    /// All context/key/source checks precede arithmetic; any refusal or unwind
    /// clears the consumed input and all completed/partial bank owners.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        source: CoefficientSlotBanks,
        key: &SwitchingKey,
    ) -> Result<CoefficientSlotBanks, UpperDigitsError> {
        source.evaluate_upper_digits(profile, self, key)
    }

    pub(super) fn preflight(
        &self,
        profile: &BgvProfile,
        source: &CoefficientSlotBanks,
        key: &SwitchingKey,
    ) -> Result<(), UpperDigitsError> {
        if self.retention != BankRetention::Collected {
            return Err(UpperDigitsError::Source);
        }
        self.preflight_bank(
            profile,
            source.shape(),
            source.slots(),
            source.banks().len(),
            key,
        )?;
        for input in source.banks() {
            input.check_profile(profile)?;
            if !same_shape(input, &self.input) {
                return Err(UpperDigitsError::Source);
            }
        }
        Ok(())
    }

    // Only validation, never a constructor or scalar capability. The concrete
    // coefficient owner remains the sole caller of execute_bank.
    pub(super) fn preflight_bank(
        &self,
        profile: &BgvProfile,
        shape: &CiphertextShape,
        slots: usize,
        banks: usize,
        key: &SwitchingKey,
    ) -> Result<(), UpperDigitsError> {
        self.input.check_profile(profile)?;
        key.check_profile(profile)?;
        if key.kind() != SwitchingKeyKind::Relinearization {
            return Err(BgvError::WrongSwitchingKey.into());
        }
        if shape != &self.input || slots != self.slots || banks != self.banks {
            return Err(UpperDigitsError::Source);
        }
        Ok(())
    }
}

impl EncryptedUpperDigitsPlan {
    // This private crate edge is called only from the concrete consuming method
    // on closed coefficient banks, after their complete source/key preflight. It is not
    // exported from bgv and cannot serve as a public scalar/precision cast.
    pub(super) fn execute_bank(
        &self,
        profile: &BgvProfile,
        input: Ciphertext,
        key: &SwitchingKey,
    ) -> Result<Ciphertext, UpperDigitsError> {
        if !same_shape(&input, &self.input) {
            return Err(UpperDigitsError::Source);
        }
        let mut arena = reserve(self.nodes.len())?;
        arena.resize_with(self.nodes.len(), || None);
        arena[0] = Some(input);
        let mut uses = reserve(self.uses.len())?;
        uses.extend_from_slice(&self.uses);
        let mut depth = 0;
        #[cfg(test)]
        tests::after_node(
            profile,
            self,
            0,
            arena[0].as_ref().ok_or(UpperDigitsError::Program)?,
        )?;
        for &index in &self.order {
            let node = self.nodes[index];
            if matches!(node.operation, Operation::Input) {
                continue;
            }
            if node.depth != depth {
                let target = *self
                    .levels
                    .get(node.depth - 1)
                    .ok_or(UpperDigitsError::Levels)?;
                for value in arena.iter_mut().flatten() {
                    while value.level() > target {
                        // Keep the old owner until the complete new allocation
                        // exists; the public payload replay counts this overlap.
                        let next = profile.switch_down(value)?;
                        *value = next;
                    }
                    if value.level() != target {
                        return Err(UpperDigitsError::Levels);
                    }
                }
                depth = node.depth;
            }
            let (args, len) = node.operation.arguments();
            let get = |id: usize| {
                arena
                    .get(id)
                    .and_then(Option::as_ref)
                    .ok_or(UpperDigitsError::Program)
            };
            let result = match node.operation {
                Operation::Input => return Err(UpperDigitsError::Program),
                Operation::Multiply(a, b) => profile.multiply(get(a)?, get(b)?, key)?,
                Operation::Add(a, b) => profile.add(get(a)?, get(b)?)?,
                Operation::Subtract(a, b) => profile.sub(get(a)?, get(b)?)?,
                Operation::MultiplyPlain(a, c) => {
                    // The compiler's coefficient is public and centered at this
                    // exact precision; the shared scalar owner derives its norm.
                    let scalar = get(a)?.plaintext_precision().fixed().canonicalize_i64(c);
                    profile.mul_public_scalar(get(a)?, scalar)?
                }
                Operation::AddPlain(a, c) => {
                    let plaintext =
                        self.scalar_plaintext(profile, get(a)?.plaintext_precision(), c)?;
                    profile.add_plain(get(a)?, &plaintext)?
                }
                Operation::Negate(a) => profile.negate(get(a)?)?,
                Operation::DivideProven(parent) => {
                    if uses[parent] != 1
                        || !matches!(self.nodes[parent].operation, Operation::Subtract(..))
                        || self.nodes[parent].precision != node.precision + 1
                    {
                        return Err(UpperDigitsError::Program);
                    }
                    let difference = arena[parent].take().ok_or(UpperDigitsError::Program)?;
                    self.divide_matched_difference(profile, difference, node.precision)?
                }
            };
            #[cfg(test)]
            tests::after_node(profile, self, index, &result)?;
            arena[index] = Some(result);
            for &arg in &args[..len] {
                uses[arg] = uses[arg].checked_sub(1).ok_or(UpperDigitsError::Program)?;
                if uses[arg] == 0 {
                    arena[arg] = None;
                }
            }
            if uses[index] == 0 {
                arena[index] = None;
            }
        }
        let output = arena
            .get_mut(self.output_id)
            .and_then(Option::take)
            .ok_or(UpperDigitsError::Program)?;
        if arena.iter().any(Option::is_some) || output.shape() != self.output {
            return Err(UpperDigitsError::Program);
        }
        Ok(output)
    }

    fn scalar_plaintext(
        &self,
        profile: &BgvProfile,
        precision: super::PlaintextPrecision,
        value: i64,
    ) -> Result<Plaintext, UpperDigitsError> {
        let mut coefficients = reserve(profile.degree())?;
        coefficients.resize(profile.degree(), 0_u64);
        coefficients[0] = precision.fixed().canonicalize_i64(value);
        let magnitude = value.unsigned_abs();
        Ok(Plaintext::with_public_bound(
            profile,
            precision,
            coefficients,
            PlaintextBound {
                linf: magnitude,
                l1: magnitude,
            },
        )?)
    }

    // No public capability, shape import or arbitrary caller can invoke this.
    // Scalar-membership and exact register induction come from the consumed
    // coefficient owner and immutable matching Subtract/Divide compilation.
    fn divide_matched_difference(
        &self,
        profile: &BgvProfile,
        difference: Ciphertext,
        exponent: u32,
    ) -> Result<Ciphertext, UpperDigitsError> {
        difference.check_profile(profile)?;
        if difference.plaintext_precision().exponent() != exponent + 1
            || difference.component_count() != 2
        {
            return Err(UpperDigitsError::Program);
        }
        let level = difference.level();
        let precision = profile.plaintext_precision(exponent)?;
        let bound = difference
            .noise_bound()
            .as_natural()
            .div_floor_u64(self.program.plan().context().prime())
            .ok_or(UpperDigitsError::Program)?;
        let shape = profile.plan_claimed(precision, level, 2, bound)?;
        let mut components = difference.into_components();
        for component in &mut components {
            for limb in 0..=level {
                let fixed = profile.main_plan(limb).fixed();
                let inverse = *self.inverses.get(limb).ok_or(UpperDigitsError::Program)?;
                // The existing SIMD scalar-product adapter is a32-bit kernel
                // with a variable-time fallback. These up-to62-bit secret
                // residues reuse the canonical fixed-time word multiply.
                for value in component.limb_mut(limb) {
                    *value = fixed.multiply(*value, inverse);
                }
                #[cfg(test)]
                tests::during_division()?;
            }
        }
        Ok(Ciphertext::from_shape(profile, shape, components))
    }
}

#[cfg(test)]
mod tests {
    //! Genuine constructor-chain fixtures and independent integer phase/tableau checks.

    use super::*;
    use crate::bgv::{
        BgvParameters, CoefficientSlotBudget, CoefficientSlotDecomposition,
        CoefficientSlotRecomposition, KeySet, SlotEncoding, SlotLinearBudget,
        fixtures::{self, FixtureEntropy},
        poly,
    };
    use crate::{
        natural::Natural,
        prime_power::{PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan},
    };
    use num_bigint::{BigInt, Sign};
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
    };
    use zeroize::Zeroizing;

    thread_local! {
        static DIVISION_FAULT: Cell<u8> = const { Cell::new(0) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
        static FAIL_ALLOCATION: Cell<usize> = const { Cell::new(usize::MAX) };
        static CALLS: Cell<usize> = const { Cell::new(0) };
        static FAULT: Cell<(usize,u8)> = const { Cell::new((usize::MAX,0)) };
        static CLEARED_AT_FAULT: Cell<usize> = const { Cell::new(0) };
        static ORACLE: RefCell<Option<Oracle>> = const { RefCell::new(None) };
    }

    pub(super) fn allocation_checkpoint() -> Result<(), UpperDigitsError> {
        let call = ALLOCATIONS.with(|v| {
            let n = v.get() + 1;
            v.set(n);
            n
        });
        if FAIL_ALLOCATION.with(Cell::get) == call {
            return Err(UpperDigitsError::Allocation);
        }
        Ok(())
    }

    pub(super) fn after_node(
        profile: &BgvProfile,
        plan: &EncryptedUpperDigitsPlan,
        id: usize,
        value: &Ciphertext,
    ) -> Result<(), UpperDigitsError> {
        ORACLE.with(|cell| {
            if let Some(oracle) = cell.borrow_mut().as_mut() {
                oracle.check(profile, plan, id, value);
            }
        });
        let call = CALLS.with(|v| {
            let n = v.get() + 1;
            v.set(n);
            n
        });
        let (at, mode) = FAULT.with(Cell::get);
        if call == at && mode != 0 {
            FAULT.with(|v| v.set((usize::MAX, 0)));
            CLEARED_AT_FAULT.with(|v| v.set(poly::tests::cleared_words()));
            if mode == 2 {
                panic!("upper-digit owned-node unwind control");
            }
            return Err(UpperDigitsError::Program);
        }
        Ok(())
    }
    pub(super) fn during_division() -> Result<(), UpperDigitsError> {
        let mode = DIVISION_FAULT.with(|v| v.replace(0));
        if mode == 0 {
            return Ok(());
        }
        CLEARED_AT_FAULT.with(|v| v.set(poly::tests::cleared_words()));
        if mode == 2 {
            panic!("upper-digit partial in-place division unwind control");
        }
        Err(UpperDigitsError::Program)
    }

    fn reset() {
        DIVISION_FAULT.with(|v| v.set(0));
        ALLOCATIONS.with(|v| v.set(0));
        FAIL_ALLOCATION.with(|v| v.set(usize::MAX));
        CALLS.with(|v| v.set(0));
        FAULT.with(|v| v.set((usize::MAX, 0)));
        ORACLE.with(|v| *v.borrow_mut() = None);
    }
    fn rem(x: BigInt, m: &BigInt) -> BigInt {
        let x = x % m;
        if x.sign() == Sign::Minus { x + m } else { x }
    }
    fn integer(x: &Natural) -> BigInt {
        x.words_le()
            .iter()
            .rev()
            .fold(BigInt::from(0), |a, &w| (a << 64) + w)
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
    fn pow(mut x: u64, mut e: u64, m: u64) -> u64 {
        let mut y = 1;
        while e != 0 {
            if e & 1 != 0 {
                y = (u128::from(y) * u128::from(x) % u128::from(m)) as u64;
            }
            x = (u128::from(x) * u128::from(x) % u128::from(m)) as u64;
            e >>= 1;
        }
        y
    }
    fn encoded(
        profile: &BgvProfile,
        precision: super::super::PlaintextPrecision,
        slots: &[u64],
    ) -> Vec<u64> {
        let encoding = SlotEncoding::new(profile, precision).unwrap();
        let s = slots.len();
        assert_eq!(s, encoding.slots());
        let t = precision.modulus();
        let inv = pow(s as u64, t - t / precision.prime() - 1, t);
        let mut out = vec![0; profile.degree()];
        for k in 0..s {
            let mut value = 0u128;
            for (j, &x) in slots.iter().enumerate() {
                let exponent = (2 * s - ((2 * j + 1) * k) % (2 * s)) % (2 * s);
                value += (u128::from(x) * u128::from(pow(encoding.root(), exponent as u64, t)))
                    % u128::from(t);
            }
            out[k * encoding.stride()] = (value * u128::from(inv) % u128::from(t)) as u64;
        }
        out
    }
    // This CRT and scalar schoolbook convolution shares no production decryption,
    // NTT, basis-extension or ciphertext multiply routine.
    fn phase(profile: &BgvProfile, secret: &[i64], cipher: &Ciphertext) -> Vec<BigInt> {
        let n = profile.degree();
        let primes = &profile.parameters().main_primes[..=cipher.level()];
        let q = primes.iter().fold(BigInt::from(1), |a, &p| a * p);
        let half = (&q - 1) / 2;
        let basis: Vec<_> = primes
            .iter()
            .map(|&p| {
                let part = &q / p;
                let inv = inverse(&rem(part.clone(), &BigInt::from(p)), &BigInt::from(p));
                part * inv
            })
            .collect();
        let words = Zeroizing::new(cipher.canonical_words());
        let count = n * primes.len();
        let lift = |at: usize| {
            (0..n)
                .map(|j| {
                    rem(
                        basis
                            .iter()
                            .enumerate()
                            .fold(BigInt::from(0), |a, (i, b)| a + b * words[at + i * n + j]),
                        &q,
                    )
                })
                .collect::<Vec<_>>()
        };
        let mut values = lift(0);
        let c1 = lift(count);
        for (i, x) in c1.iter().enumerate() {
            for (j, &s) in secret.iter().enumerate() {
                let term = x * s;
                if i + j < n {
                    values[i + j] += &term;
                } else {
                    values[i + j - n] -= &term;
                }
            }
        }
        let bound = integer(cipher.noise_bound().as_natural());
        assert!(bound <= half);
        values
            .into_iter()
            .map(|x| {
                let x = rem(x, &q);
                let x = if x > half { x - &q } else { x };
                assert!(
                    x >= -&bound && x <= bound,
                    "physical phase exceeds public bound"
                );
                x
            })
            .collect()
    }
    struct Oracle {
        secret: Zeroizing<Vec<i64>>,
        coefficients: Vec<u64>,
        bank: usize,
        values: Vec<Vec<u64>>,
        phases: Vec<Vec<BigInt>>,
        divisions: usize,
        checks: usize,
    }
    impl Oracle {
        fn check(
            &mut self,
            profile: &BgvProfile,
            plan: &EncryptedUpperDigitsPlan,
            id: usize,
            cipher: &Ciphertext,
        ) {
            let node = plan.nodes[id];
            let t = cipher.plaintext_precision().modulus();
            let prime = plan.program.plan().context().prime();
            if id == 0 {
                self.values = vec![Vec::new(); plan.nodes.len()];
                self.phases = vec![Vec::new(); plan.nodes.len()];
            }
            let reduce = |x: i128| x.rem_euclid(i128::from(t)) as u64;
            let expected: Vec<u64> = match node.operation {
                Operation::Input => (0..plan.slots)
                    .map(|j| self.coefficients[self.bank + plan.banks * j])
                    .collect(),
                Operation::Multiply(a, b) => self.values[a]
                    .iter()
                    .zip(&self.values[b])
                    .map(|(&x, &y)| reduce(i128::from(x) * i128::from(y)))
                    .collect(),
                Operation::Add(a, b) => self.values[a]
                    .iter()
                    .zip(&self.values[b])
                    .map(|(&x, &y)| reduce(i128::from(x) + i128::from(y)))
                    .collect(),
                Operation::Subtract(a, b) => self.values[a]
                    .iter()
                    .zip(&self.values[b])
                    .map(|(&x, &y)| reduce(i128::from(x) - i128::from(y)))
                    .collect(),
                Operation::MultiplyPlain(a, c) => self.values[a]
                    .iter()
                    .map(|&x| reduce(i128::from(x) * i128::from(c)))
                    .collect(),
                Operation::AddPlain(a, c) => self.values[a]
                    .iter()
                    .map(|&x| reduce(i128::from(x) + i128::from(c)))
                    .collect(),
                Operation::Negate(a) => self.values[a]
                    .iter()
                    .map(|&x| reduce(-i128::from(x)))
                    .collect(),
                Operation::DivideProven(a) => self.values[a]
                    .iter()
                    .map(|&x| {
                        assert_eq!(x % prime, 0);
                        x / prime
                    })
                    .collect(),
            };
            let actual = phase(profile, &self.secret, cipher);
            let polynomial = encoded(profile, cipher.plaintext_precision(), &expected);
            for (x, &wanted) in actual.iter().zip(&polynomial) {
                assert_eq!(
                    rem(x.clone(), &BigInt::from(t)),
                    BigInt::from(wanted),
                    "node {id} residue"
                );
            }
            if let Operation::DivideProven(a) = node.operation {
                assert_eq!(
                    plan.nodes[a].depth, node.depth,
                    "no level change at matched division"
                );
                for (before, after) in self.phases[a].iter().zip(&actual) {
                    assert_eq!(
                        before % prime,
                        BigInt::from(0),
                        "integer phase is divisible"
                    );
                    assert_eq!(
                        before / prime,
                        *after,
                        "centered phase is the exact quotient"
                    );
                }
                self.divisions += 1;
            }
            self.values[id] = expected;
            self.phases[id] = actual;
            self.checks += 1;
            if id == plan.output_id {
                self.bank += 1;
            }
        }
    }
    fn budget() -> UpperDigitsBudget {
        UpperDigitsBudget {
            compiler_steps: 1 << 24,
            metadata_bytes: 1 << 26,
            owned_residue_bytes: 1 << 26,
            plaintext_bytes: 1 << 20,
            backend_scratch_bytes: 1 << 26,
            canonical_arithmetic_units: u64::MAX,
            scalar_word_operations: u64::MAX,
        }
    }
    fn transform_budget() -> CoefficientSlotBudget {
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
    fn program(prime: u64, e: u32, retained: u32) -> UpperDigitsProgram {
        let context = PrimePowerContext::new(prime, e).unwrap();
        let g = UpperDigitsPlan::required(context, retained).unwrap();
        UpperDigitsProgram::prepare(
            &UpperDigitsPlan::new(
                context,
                retained,
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
    }
    struct Fixture {
        profile: BgvProfile,
        transform: CoefficientSlotDecomposition,
        keys: KeySet,
    }
    impl Fixture {
        fn new(e: u32) -> Self {
            let profile =
                BgvProfile::new(BgvParameters::derive(4, 5, e, (50, 4), (60, 2), 2).unwrap())
                    .unwrap();
            let transform = CoefficientSlotDecomposition::new(
                &profile,
                &profile.plan_fresh(profile.master_plaintext()).unwrap(),
                transform_budget(),
            )
            .unwrap();
            let keys = fixtures::key_set(
                &profile,
                transform.galois_elements(),
                &mut FixtureEntropy::new([159; 64]),
            )
            .unwrap();
            Self {
                profile,
                transform,
                keys,
            }
        }
        fn source(&self, values: Vec<u64>) -> CoefficientSlotBanks {
            let plain =
                Plaintext::new(&self.profile, self.profile.master_plaintext(), values).unwrap();
            let cipher = fixtures::encrypt(
                &self.profile,
                &self.keys.public,
                &plain,
                &mut FixtureEntropy::new([163; 64]),
            )
            .unwrap();
            self.transform
                .evaluate(&self.profile, cipher, &self.keys.evaluation)
                .unwrap()
        }
    }
    fn levels(e: u32) -> Vec<usize> {
        if e == 2 {
            vec![3, 2, 2]
        } else {
            vec![3, 2, 2, 1, 1, 1]
        }
    }
    fn upper(mut x: i128, p: i128, drops: u32, modulus: i128) -> u64 {
        for _ in 0..drops {
            let mut digit = x.rem_euclid(p);
            if digit > p / 2 {
                digit -= p;
            }
            x = (x - digit) / p;
        }
        x.rem_euclid(modulus) as u64
    }

    #[test]
    fn genuine_p5_e2_e3_all_residues_have_exact_integer_division_and_outputs() {
        reset();
        for (e, retained) in [(2, 1), (3, 1), (3, 2)] {
            let f = Fixture::new(e);
            let t = 5u64.pow(e);
            let mut total_divisions = 0;
            for start in (0..t).step_by(16) {
                let coefficients: Vec<_> = (0..16).map(|j| (start + j) % t).collect();
                let source = f.source(coefficients.clone());
                assert_eq!(
                    source.shape().bound(),
                    &Natural::from_u64(if e == 2 { 6_728_864 } else { 173_831_764 })
                );
                let plan = EncryptedUpperDigitsPlan::new(
                    &f.profile,
                    program(5, e, retained),
                    &source,
                    &levels(e),
                    budget(),
                )
                .unwrap();
                let expected_bound = if e == 2 {
                    492_745_070
                } else if retained == 1 {
                    21_250_290_901_030_317_478u128
                } else {
                    17_708_575_750_858_598_076u128
                };
                assert_eq!(
                    plan.output_shape().plaintext_precision().exponent(),
                    retained
                );
                assert_eq!(
                    plan.output_shape().bound(),
                    &Natural::from_u128(expected_bound)
                );
                ORACLE.with(|v| {
                    *v.borrow_mut() = Some(Oracle {
                        secret: Zeroizing::new(fixtures::secret_coefficients(&f.keys.secret)),
                        coefficients: coefficients.clone(),
                        bank: 0,
                        values: Vec::new(),
                        phases: Vec::new(),
                        divisions: 0,
                        checks: 0,
                    })
                });
                let result = plan
                    .evaluate(&f.profile, source, f.keys.evaluation.relinearization())
                    .unwrap();
                ORACLE.with(|v| {
                    let oracle = v.borrow_mut().take().unwrap();
                    assert_eq!(oracle.bank, 8);
                    assert_eq!(oracle.checks, 8 * plan.nodes.len());
                    assert_eq!(
                        oracle.divisions,
                        8 * plan.operations.exact_divisions as usize
                    );
                    total_divisions += oracle.divisions;
                });
                let wanted: Vec<_> = coefficients
                    .iter()
                    .map(|&x| {
                        upper(
                            i128::from(x),
                            5,
                            e - retained,
                            i128::from(5u64.pow(retained)),
                        )
                    })
                    .collect();
                for (r, bank) in result.banks().iter().enumerate() {
                    let opened = f.keys.secret.decrypt(&f.profile, bank).unwrap();
                    let decoded = SlotEncoding::new(&f.profile, bank.plaintext_precision())
                        .unwrap()
                        .decode(&opened)
                        .unwrap();
                    assert_eq!(decoded.as_slice(), &[wanted[r], wanted[r + 8]]);
                }
                let inverse = CoefficientSlotRecomposition::new(
                    &f.profile,
                    result.shape(),
                    transform_budget(),
                )
                .unwrap();
                let output = inverse
                    .evaluate(&f.profile, result, &f.keys.evaluation)
                    .unwrap();
                let actual = phase(
                    &f.profile,
                    &fixtures::secret_coefficients(&f.keys.secret),
                    &output,
                );
                for (x, &y) in actual.iter().zip(&wanted) {
                    assert_eq!(
                        rem(x.clone(), &BigInt::from(5u64.pow(retained))),
                        BigInt::from(y)
                    );
                }
                assert_eq!(
                    f.keys
                        .secret
                        .decrypt(&f.profile, &output)
                        .unwrap()
                        .coefficients(),
                    wanted
                );
            }
            assert!(total_divisions > 0);
        }
    }

    #[test]
    fn public_budget_level_context_and_key_refusals_precede_execution() {
        reset();
        let f = Fixture::new(3);
        let source = f.source((0..16).collect());
        let plan = EncryptedUpperDigitsPlan::new(
            &f.profile,
            program(5, 3, 1),
            &source,
            &levels(3),
            budget(),
        )
        .unwrap();
        let (count, _, _) =
            compiler::compile(&program(5, 3, 1), compiler::Count::new(u64::MAX)).unwrap();
        let exact = UpperDigitsBudget {
            compiler_steps: count.visits,
            metadata_bytes: plan.payload.metadata_bytes,
            owned_residue_bytes: plan.payload.owned_residue_bytes,
            plaintext_bytes: plan.payload.plaintext_bytes,
            backend_scratch_bytes: plan.payload.backend_scratch_bytes,
            canonical_arithmetic_units: plan.payload.preallocation_arithmetic_units,
            scalar_word_operations: plan.payload.preallocation_scalar_word_operations,
        };
        EncryptedUpperDigitsPlan::new(&f.profile, program(5, 3, 1), &source, &levels(3), exact)
            .unwrap();
        for field in 0..7 {
            let mut low = exact;
            match field {
                0 => low.compiler_steps -= 1,
                1 => low.metadata_bytes -= 1,
                2 => low.owned_residue_bytes -= 1,
                3 => low.plaintext_bytes -= 1,
                4 => low.backend_scratch_bytes -= 1,
                5 => low.canonical_arithmetic_units -= 1,
                _ => low.scalar_word_operations -= 1,
            }
            CALLS.with(|v| v.set(0));
            let error = EncryptedUpperDigitsPlan::new(
                &f.profile,
                program(5, 3, 1),
                &source,
                &levels(3),
                low,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                UpperDigitsError::CompilerBudget | UpperDigitsError::Budget { .. }
            ));
            assert_eq!(CALLS.with(Cell::get), 0);
        }
        for wrong in [
            vec![],
            vec![3, 2, 2, 1, 1],
            vec![3, 2, 3, 1, 1, 1],
            vec![4, 2, 2, 1, 1, 1],
        ] {
            assert_eq!(
                EncryptedUpperDigitsPlan::new(
                    &f.profile,
                    program(5, 3, 1),
                    &source,
                    &wrong,
                    budget()
                )
                .unwrap_err(),
                UpperDigitsError::Levels
            );
        }
        assert_eq!(
            EncryptedUpperDigitsPlan::new(
                &f.profile,
                program(5, 2, 1),
                &source,
                &levels(2),
                budget()
            )
            .unwrap_err(),
            UpperDigitsError::Source
        );
        let element = f.transform.galois_elements()[0];
        let wrong_key = f.keys.evaluation.galois(element).unwrap();
        assert!(matches!(
            plan.preflight(&f.profile, &source, wrong_key),
            Err(UpperDigitsError::Bgv(BgvError::WrongSwitchingKey))
        ));
        let other = Fixture::new(2);
        assert!(
            plan.preflight(
                &other.profile,
                &source,
                other.keys.evaluation.relinearization()
            )
            .is_err()
        );
        let original = source.banks()[0].to_bytes(&f.profile).unwrap();
        reset();
        EncryptedUpperDigitsPlan::new(&f.profile, program(5, 3, 1), &source, &levels(3), budget())
            .unwrap();
        let allocations = ALLOCATIONS.with(Cell::get);
        for fail in 1..=allocations {
            reset();
            FAIL_ALLOCATION.with(|v| v.set(fail));
            assert_eq!(
                EncryptedUpperDigitsPlan::new(
                    &f.profile,
                    program(5, 3, 1),
                    &source,
                    &levels(3),
                    budget()
                )
                .unwrap_err(),
                UpperDigitsError::Allocation
            );
        }
        reset();
        assert_eq!(source.banks()[0].to_bytes(&f.profile).unwrap(), original);
    }

    #[test]
    fn consumed_banks_clear_on_first_last_node_error_unwind_and_allocation_failure() {
        reset();
        let f = Fixture::new(3);
        let source = f.source((0..16).collect());
        let plan = EncryptedUpperDigitsPlan::new(
            &f.profile,
            program(5, 3, 1),
            &source,
            &levels(3),
            budget(),
        )
        .unwrap();
        drop(source);
        for at in [1, 8 * plan.nodes.len()] {
            for mode in [1, 2] {
                let source = f.source((0..16).collect());
                reset();
                FAULT.with(|v| v.set((at, mode)));
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    plan.evaluate(&f.profile, source, f.keys.evaluation.relinearization())
                }));
                if mode == 1 {
                    assert!(outcome.unwrap().is_err());
                } else {
                    assert!(outcome.is_err());
                }
                let retired = poly::tests::cleared_words() - CLEARED_AT_FAULT.with(Cell::get);
                let level = if at == 1 {
                    plan.input.level()
                } else {
                    plan.output.level()
                };
                assert_eq!(
                    retired,
                    8 * 2 * 16 * (level + 1),
                    "every still-owned input/completed/output bank clears"
                );
            }
        }
        for mode in [1, 2] {
            let source = f.source((0..16).collect());
            reset();
            DIVISION_FAULT.with(|v| v.set(mode));
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                plan.evaluate(&f.profile, source, f.keys.evaluation.relinearization())
            }));
            if mode == 1 {
                assert!(outcome.unwrap().is_err());
            } else {
                assert!(outcome.is_err());
            }
            // The original component Vec survives the partial in-place transform.
            // Its two complete payloads and every remaining input are still owners;
            // the shared destructor observer checks zeroization before counting.
            assert!(
                poly::tests::cleared_words() - CLEARED_AT_FAULT.with(Cell::get)
                    >= 7 * 2 * 16 * 4 + 2 * 16
            );
        }
        for fail in 1..=3 {
            let source = f.source((0..16).collect());
            reset();
            FAIL_ALLOCATION.with(|v| v.set(fail));
            let before = poly::tests::cleared_words();
            assert!(matches!(
                plan.evaluate(&f.profile, source, f.keys.evaluation.relinearization()),
                Err(UpperDigitsError::Allocation)
            ));
            assert_eq!(poly::tests::cleared_words() - before, 8 * 2 * 16 * 4);
            reset();
        }
    }

    #[test]
    fn compiler_count_replay_aliases_and_public_f257_geometry_are_exact() {
        reset();
        for (p, e, r, multiplies, depth) in [
            (257, 2, 1, 27, 9),
            (257, 3, 1, 81, 18),
            (5, 2, 1, 3, 3),
            (5, 3, 1, 9, 6),
            (5, 3, 2, 6, 6),
        ] {
            let program = program(p, e, r);
            let (count, output, d) =
                compiler::compile(&program, compiler::Count::new(u64::MAX)).unwrap();
            let (store, other, d2) = compiler::compile(
                &program,
                compiler::Store {
                    nodes: Vec::with_capacity(count.nodes),
                    limit: count.nodes,
                },
            )
            .unwrap();
            assert_eq!((output, d), (other, d2));
            assert_eq!(d, depth);
            assert_eq!(count.operations.ciphertext_multiplications, multiplies);
            assert_eq!(store.nodes.len(), count.nodes);
            for (i, node) in store.nodes.iter().enumerate() {
                let (args, n) = node.operation.arguments();
                for &a in &args[..n] {
                    assert!(a < i);
                    assert!(store.nodes[a].depth <= node.depth);
                }
                if let Operation::DivideProven(parent) = node.operation {
                    assert!(matches!(
                        store.nodes[parent].operation,
                        Operation::Subtract(..)
                    ));
                    assert_eq!(store.nodes[parent].precision, node.precision + 1);
                }
            }
            assert!(matches!(
                compiler::compile(&program, compiler::Count::new(count.visits - 1)),
                Err(UpperDigitsError::CompilerBudget)
            ));
        }
    }
    #[test]
    fn single_bank_failure_and_partial_division_clear_the_suspended_trace_frontier() {
        reset();
        let f = Fixture::new(3);
        let plan = EncryptedUpperDigitsPlan::from_shape(
            &f.profile,
            program(5, 3, 1),
            f.transform.output_shape(),
            f.transform.slots(),
            f.transform.banks(),
            &levels(3),
            budget(),
            BankRetention::Streaming,
        )
        .unwrap();
        let incoming_words = 2 * f.profile.degree() * (plan.input.level() + 1);
        let pending = f.transform.banks().ilog2() as usize - 1;
        let output_words = 2 * f.profile.degree() * (plan.output.level() + 1);
        let restoration = super::super::CoefficientSlotRecomposition::prepare(
            &f.profile,
            plan.output_shape(),
            transform_budget(),
            BankRetention::Streaming,
        )
        .unwrap();
        for (at, division) in [
            (1, false),
            (plan.nodes.len() / 2, false),
            (plan.nodes.len(), false),
            (0, true),
        ] {
            for mode in [1, 2] {
                reset();
                let plain =
                    Plaintext::new(&f.profile, f.profile.master_plaintext(), (0..16).collect())
                        .unwrap();
                let input = fixtures::encrypt(
                    &f.profile,
                    &f.keys.public,
                    &plain,
                    &mut FixtureEntropy::new([225; 64]),
                )
                .unwrap();
                let (bank, rest) = f
                    .transform
                    .start(&f.profile, input, &f.keys.evaluation)
                    .unwrap()
                    .advance(&f.profile, &f.keys.evaluation)
                    .unwrap()
                    .unwrap();
                // Complete the first bank so the next failure owns both a
                // suspended DFS frontier and a real lower-level accumulator.
                let bank = bank
                    .upper(&f.profile, &plan, f.keys.evaluation.relinearization())
                    .unwrap();
                let accumulator = restoration
                    .start_accumulator(&f.profile, &f.keys.evaluation)
                    .unwrap()
                    .push(&f.profile, bank, &f.keys.evaluation)
                    .unwrap();
                let (bank, rest) = rest
                    .advance(&f.profile, &f.keys.evaluation)
                    .unwrap()
                    .unwrap();
                reset();
                if division {
                    DIVISION_FAULT.set(mode);
                } else {
                    FAULT.set((at, mode));
                }
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let result = bank.upper(&f.profile, &plan, f.keys.evaluation.relinearization());
                    drop(rest);
                    drop(accumulator);
                    result.map(|_| ())
                }));
                assert!(if mode == 2 {
                    outcome.is_err()
                } else {
                    matches!(outcome, Ok(Err(UpperDigitsError::Program)))
                });
                let retired = poly::tests::cleared_words() - CLEARED_AT_FAULT.get();
                if !division && at == 1 {
                    assert_eq!(retired, (pending + 1) * incoming_words + output_words);
                } else if !division && at == plan.nodes.len() {
                    assert_eq!(retired, pending * incoming_words + 2 * output_words);
                } else {
                    assert!(
                        retired >= pending * incoming_words + output_words + 2 * f.profile.degree()
                    );
                }
            }
        }
        reset();
    }
}
