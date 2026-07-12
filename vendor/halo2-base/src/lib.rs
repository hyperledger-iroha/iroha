//! Base library to build Halo2 circuits.
#![allow(incomplete_features)]
#![deny(clippy::perf)]
#![allow(clippy::too_many_arguments)]
#![warn(clippy::default_numeric_fallback)]
#![warn(missing_docs)]

use getset::CopyGetters;
use itertools::Itertools;
// Different memory allocator options:
#[cfg(feature = "jemallocator")]
use jemallocator::Jemalloc;
#[cfg(feature = "jemallocator")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// mimalloc is fastest on Mac M2
#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(all(feature = "halo2-pse", feature = "halo2-axiom"))]
compile_error!("Cannot enable both \"halo2-pse\" and \"halo2-axiom\" features.");
#[cfg(not(any(feature = "halo2-pse", feature = "halo2-axiom")))]
compile_error!("Must enable exactly one of \"halo2-pse\" or \"halo2-axiom\".");
#[cfg(all(feature = "halo2-pse", feature = "cuda"))]
compile_error!("The \"cuda\" feature is only supported with \"halo2-axiom\".");

// use gates::flex_gate::MAX_PHASE;
#[cfg(feature = "halo2-pse")]
pub use halo2_proofs;
#[cfg(all(feature = "halo2-axiom", not(feature = "cuda")))]
pub use halo2_proofs_axiom as halo2_proofs;
#[cfg(all(feature = "halo2-axiom", feature = "cuda"))]
pub use halo2_proofs_axiom_gpu as halo2_proofs;

use halo2_proofs::halo2curves::ff;
use halo2_proofs::plonk::Assigned;
use utils::ScalarField;
use virtual_region::copy_constraints::SharedCopyConstraintManager;

/// Module that contains the main API for creating and working with circuits.
/// `gates` is misleading because we currently only use one custom gate throughout.
pub mod gates;
/// Module for the Poseidon hash function.
pub mod poseidon;
/// Module for SafeType which enforce value range and realted functions.
pub mod safe_types;
/// Utility functions for converting between different types of field elements.
pub mod utils;
pub mod virtual_region;

/// Constant representing whether the Layouter calls `synthesize` once just to get region shape.
#[cfg(feature = "halo2-axiom")]
pub const SKIP_FIRST_PASS: bool = false;
/// Constant representing whether the Layouter calls `synthesize` once just to get region shape.
#[cfg(feature = "halo2-pse")]
pub const SKIP_FIRST_PASS: bool = true;

/// Convenience Enum which abstracts the scenarios under a value is added to an advice column.
#[derive(Clone, Copy, Debug)]
pub enum QuantumCell<F: ScalarField> {
    /// An [AssignedValue] already existing in the advice column (e.g., a witness value that was already assigned in a previous cell in the column).
    /// * Assigns a new cell into the advice column with value equal to the value of a.
    /// * Imposes an equality constraint between the new cell and the cell of a so the Verifier guarantees that these two cells are always equal.
    Existing(AssignedValue<F>),
    // This is a guard for witness values assigned after pkey generation. We do not use `Value` api anymore.
    /// A non-existing witness [ScalarField] value (e.g. private input) to add to an advice column.
    Witness(F),
    /// A non-existing witness [ScalarField] marked as a fraction for optimization in batch inversion later.
    WitnessFraction(Assigned<F>),
    /// A known constant value added as a witness value to the advice column and added to the "Fixed" column during circuit creation time.
    /// * Visible to both the Prover and the Verifier.
    /// * Imposes an equality constraint between the two corresponding cells in the advice and fixed columns.
    Constant(F),
}

impl<F: ScalarField> From<AssignedValue<F>> for QuantumCell<F> {
    /// Converts an [`AssignedValue<F>`] into a [`QuantumCell<F>`] of enum variant `Existing`.
    fn from(a: AssignedValue<F>) -> Self {
        Self::Existing(a)
    }
}

impl<F: ScalarField> QuantumCell<F> {
    /// Returns an immutable reference to the underlying [ScalarField] value of a [`QuantumCell<F>`].
    ///
    /// Panics if the [`QuantumCell<F>`] is of type `WitnessFraction`.
    pub fn value(&self) -> &F {
        match self {
            Self::Existing(a) => a.value(),
            Self::Witness(a) => a,
            Self::WitnessFraction(_) => {
                panic!("Trying to get value of a fraction before batch inversion")
            }
            Self::Constant(a) => a,
        }
    }
}

/// Unique tag for a context across all virtual regions.
/// In the form `(type_id, context_id)` where `type_id` should be a unique identifier
/// for the virtual region this context belongs to, and `context_id` is a counter local to that virtual region.
pub type ContextTag = (&'static str, usize);

/// Pointer to the position of a cell at `offset` in an advice column within a [Context] of `context_id`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContextCell {
    /// The unique string identifier of the virtual region that this cell belongs to.
    pub type_id: &'static str,
    /// Identifier of the [Context] that this cell belongs to.
    pub context_id: usize,
    /// Relative offset of the cell within this [Context] advice column.
    pub offset: usize,
}

impl ContextCell {
    /// Creates a new [ContextCell] with the given `type_id`, `context_id`, and `offset`.
    ///
    /// **Warning:** If you create your own `Context` in a new virtual region not provided by our libraries, you must ensure that the `type_id: &str` of the context is a globally unique identifier for the virtual region, distinct from the other `type_id` strings used to identify other virtual regions. We suggest that you either include your crate name as a prefix in the `type_id` or use [`module_path!`](https://doc.rust-lang.org/std/macro.module_path.html) to generate a prefix.
    /// In the future we will introduce a macro to check this uniqueness at compile time.
    pub fn new(type_id: &'static str, context_id: usize, offset: usize) -> Self {
        Self { type_id, context_id, offset }
    }
}

/// Pointer containing cell value and location within [Context].
///
/// Note: Performs a copy of the value, should only be used when you are about to assign the value again elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct AssignedValue<F: crate::ff::Field> {
    /// Value of the cell.
    pub value: Assigned<F>, // we don't use reference to avoid issues with lifetimes (you can't safely borrow from vector and push to it at the same time).
    // only needed during vkey, pkey gen to fetch the actual cell from the relevant context
    /// [ContextCell] pointer to the cell the value is assigned to within an advice column of a [Context].
    pub cell: Option<ContextCell>,
}

impl<F: ScalarField> AssignedValue<F> {
    /// Returns an immutable reference to the underlying value of an [`AssignedValue<F>`].
    ///
    /// Panics if the witness value is of type [Assigned::Rational] or [Assigned::Zero].
    pub fn value(&self) -> &F {
        match &self.value {
            Assigned::Trivial(a) => a,
            _ => unreachable!(), // if trying to fetch an un-evaluated fraction, you will have to do something manual
        }
    }

    /// Debug helper function for writing negative tests. This will change the **witness** value in `ctx` corresponding to `self.offset`.
    /// This assumes that `ctx` is the context that `self` lies in.
    pub fn debug_prank(&self, ctx: &mut Context<F>, prank_value: F) {
        ctx.advice[self.cell.unwrap().offset] = Assigned::Trivial(prank_value);
    }
}

impl<F: ScalarField> AsRef<AssignedValue<F>> for AssignedValue<F> {
    fn as_ref(&self) -> &AssignedValue<F> {
        self
    }
}

/// Represents a single thread of an execution trace.
/// * We keep the naming [Context] for historical reasons.
///
/// [Context] is CPU thread-local.
#[derive(Clone, Debug, CopyGetters)]
pub struct Context<F: ScalarField> {
    /// Flag to determine whether only witness generation or proving and verification key generation is being performed.
    /// * If witness gen is performed many operations can be skipped for optimization.
    #[getset(get_copy = "pub")]
    witness_gen_only: bool,
    /// The challenge phase that this [Context] will map to.
    #[getset(get_copy = "pub")]
    phase: usize,
    /// Identifier for what virtual region this context is in.
    /// Warning: the circuit writer must ensure that distinct virtual regions have distinct names as strings to prevent possible errors.
    /// We do not use [std::any::TypeId] because it is not stable across rust builds or dependencies.
    #[getset(get_copy = "pub")]
    type_id: &'static str,
    /// Identifier to reference cells from this [Context].
    context_id: usize,

    /// Single column of advice cells.
    pub advice: Vec<Assigned<F>>,

    /// Slight optimization: since zero is so commonly used, keep a reference to the zero cell.
    zero_cell: Option<AssignedValue<F>>,

    // ========================================
    // General principle: we don't need to optimize anything specific to `witness_gen_only == false` because it is only done during keygen
    // If `witness_gen_only == false`:
    /// [Vec] representing the selector column of this [Context] accompanying each `advice` column
    /// * Assumed to have the same length as `advice`
    pub selector: Vec<bool>,

    /// Global shared thread-safe manager for all copy (equality) constraints between virtual advice, constants, and raw external Halo2 cells.
    pub copy_manager: SharedCopyConstraintManager<F>,
}

impl<F: ScalarField> Context<F> {
    /// Creates a new [Context] with the given `context_id` and witness generation enabled/disabled by the `witness_gen_only` flag.
    /// * `witness_gen_only`: flag to determine whether public key generation or only witness generation is being performed.
    /// * `context_id`: identifier to reference advice cells from this [Context] later.
    ///
    /// **Warning:** If you create your own `Context` in a new virtual region not provided by our libraries, you must ensure that the `type_id: &str` of the context is a globally unique identifier for the virtual region, distinct from the other `type_id` strings used to identify other virtual regions. We suggest that you either include your crate name as a prefix in the `type_id` or use [`module_path!`](https://doc.rust-lang.org/std/macro.module_path.html) to generate a prefix.
    /// In the future we will introduce a macro to check this uniqueness at compile time.
    pub fn new(
        witness_gen_only: bool,
        phase: usize,
        type_id: &'static str,
        context_id: usize,
        copy_manager: SharedCopyConstraintManager<F>,
    ) -> Self {
        Self {
            witness_gen_only,
            phase,
            type_id,
            context_id,
            advice: Vec::new(),
            selector: Vec::new(),
            zero_cell: None,
            copy_manager,
        }
    }

    /// The context id, this can be used as a tag when CPU multi-threading
    pub fn id(&self) -> usize {
        self.context_id
    }

    /// A unique tag that should identify this context across all virtual regions and phases.
    pub fn tag(&self) -> ContextTag {
        (self.type_id, self.context_id)
    }

    fn latest_cell(&self) -> ContextCell {
        ContextCell::new(self.type_id, self.context_id, self.advice.len() - 1)
    }

    /// Virtually assigns the `input` within the current [Context], with different handling depending on the [QuantumCell] variant.
    pub fn assign_cell(&mut self, input: impl Into<QuantumCell<F>>) {
        // Determine the type of the cell and push it to the relevant vector
        match input.into() {
            QuantumCell::Existing(acell) => {
                self.advice.push(acell.value);
                // If witness generation is not performed, enforce equality constraints between the existing cell and the new cell
                if !self.witness_gen_only {
                    let new_cell = self.latest_cell();
                    self.copy_manager
                        .lock()
                        .unwrap()
                        .advice_equalities
                        .push((new_cell, acell.cell.unwrap()));
                }
            }
            QuantumCell::Witness(val) => {
                self.advice.push(Assigned::Trivial(val));
            }
            QuantumCell::WitnessFraction(val) => {
                self.advice.push(val);
            }
            QuantumCell::Constant(c) => {
                self.advice.push(Assigned::Trivial(c));
                // If witness generation is not performed, enforce equality constraints between the existing cell and the new cell
                if !self.witness_gen_only {
                    let new_cell = self.latest_cell();
                    self.copy_manager.lock().unwrap().constant_equalities.push((c, new_cell));
                }
            }
        }
    }

    /// Returns the [AssignedValue] of the last cell in the `advice` column of [Context] or [None] if `advice` is empty
    pub fn last(&self) -> Option<AssignedValue<F>> {
        self.advice.last().map(|v| {
            let cell = (!self.witness_gen_only).then_some(self.latest_cell());
            AssignedValue { value: *v, cell }
        })
    }

    /// Returns the [AssignedValue] of the cell at the given `offset` in the `advice` column of [Context]
    /// * `offset`: the offset of the cell to be fetched
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    /// * Assumes `offset` is a valid index in `advice`;
    ///     * `0` <= `offset` < `advice.len()` (or `advice.len() + offset >= 0` if `offset` is negative)
    pub fn get(&self, offset: isize) -> AssignedValue<F> {
        let offset = if offset < 0 {
            self.advice.len().wrapping_add_signed(offset)
        } else {
            offset as usize
        };
        assert!(offset < self.advice.len());
        let cell = (!self.witness_gen_only).then_some(ContextCell::new(
            self.type_id,
            self.context_id,
            offset,
        ));
        AssignedValue { value: self.advice[offset], cell }
    }

    /// Creates an equality constraint between two `advice` cells.
    /// * `a`: the first `advice` cell to be constrained equal
    /// * `b`: the second `advice` cell to be constrained equal
    /// * Assumes both cells are `advice` cells
    pub fn constrain_equal(&mut self, a: &AssignedValue<F>, b: &AssignedValue<F>) {
        if !self.witness_gen_only {
            self.copy_manager
                .lock()
                .unwrap()
                .advice_equalities
                .push((a.cell.unwrap(), b.cell.unwrap()));
        }
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset`.
    ///
    /// * `inputs`: Iterator that specifies the cells to be assigned
    /// * `gate_offsets`: specifies relative offset from current position to enable selector for the gate (e.g., `0` is `inputs[0]`).
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last previously assigned cell)
    pub fn assign_region<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
    ) where
        Q: Into<QuantumCell<F>>,
    {
        if self.witness_gen_only {
            for input in inputs {
                self.assign_cell(input);
            }
        } else {
            let row_offset = self.advice.len();
            // note: row_offset may not equal self.selector.len() at this point if we previously used `load_constant` or `load_witness`
            for input in inputs {
                self.assign_cell(input);
            }
            self.selector.resize(self.advice.len(), false);
            for offset in gate_offsets {
                *self
                    .selector
                    .get_mut(row_offset.checked_add_signed(offset).expect("Invalid gate offset"))
                    .expect("Invalid selector offset") = true;
            }
        }
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset` and returns the last assigned cell.
    ///
    /// Assumes `gate_offsets` is the same length as `inputs`
    ///
    /// Returns the last assigned cell
    /// * `inputs`: Iterator that specifies the cells to be assigned
    /// * `gate_offsets`: specifies indices to enable selector for the gate; assume `gate_offsets` is sorted in increasing order
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    pub fn assign_region_last<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
    ) -> AssignedValue<F>
    where
        Q: Into<QuantumCell<F>>,
    {
        self.assign_region(inputs, gate_offsets);
        self.last().unwrap()
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset`.
    ///
    /// Allows for the specification of equality constraints between cells at `equality_offsets` within the `advice` column and external advice cells specified in `external_equality` (e.g, Fixed column).
    /// * `gate_offsets`: specifies indices to enable selector for the gate;
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    /// * `equality_offsets`: specifies pairs of indices to constrain equality
    /// * `external_equality`: specifies an existing cell to constrain equality with the cell at a certain index
    pub fn assign_region_smart<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
        equality_offsets: impl IntoIterator<Item = (isize, isize)>,
        external_equality: impl IntoIterator<Item = (Option<ContextCell>, isize)>,
    ) where
        Q: Into<QuantumCell<F>>,
    {
        let row_offset = self.advice.len();
        self.assign_region(inputs, gate_offsets);

        // note: row_offset may not equal self.selector.len() at this point if we previously used `load_constant` or `load_witness`
        // If not in witness generation mode, add equality constraints.
        if !self.witness_gen_only {
            // Add equality constraints between cells in the advice column.
            for (offset1, offset2) in equality_offsets {
                self.copy_manager.lock().unwrap().advice_equalities.push((
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset1),
                    ),
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset2),
                    ),
                ));
            }
            // Add equality constraints between cells in the advice column and external cells (Fixed column).
            for (cell, offset) in external_equality {
                self.copy_manager.lock().unwrap().advice_equalities.push((
                    cell.unwrap(),
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset),
                    ),
                ));
            }
        }
    }

    /// Assigns a region of witness cells in an iterator and returns a [Vec] of assigned cells.
    /// * `witnesses`: Iterator that specifies the cells to be assigned
    pub fn assign_witnesses(
        &mut self,
        witnesses: impl IntoIterator<Item = F>,
    ) -> Vec<AssignedValue<F>> {
        let row_offset = self.advice.len();
        self.assign_region(witnesses.into_iter().map(QuantumCell::Witness), []);
        self.advice[row_offset..]
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let cell = (!self.witness_gen_only).then_some(ContextCell::new(
                    self.type_id,
                    self.context_id,
                    row_offset + i,
                ));
                AssignedValue { value: *v, cell }
            })
            .collect()
    }

    /// Assigns a witness value and returns the corresponding assigned cell.
    /// * `witness`: the witness value to be assigned
    pub fn load_witness(&mut self, witness: F) -> AssignedValue<F> {
        self.assign_cell(QuantumCell::Witness(witness));
        if !self.witness_gen_only {
            self.selector.resize(self.advice.len(), false);
        }
        self.last().unwrap()
    }

    /// Assigns a constant value and returns the corresponding assigned cell.
    /// * `c`: the constant value to be assigned
    pub fn load_constant(&mut self, c: F) -> AssignedValue<F> {
        self.assign_cell(QuantumCell::Constant(c));
        if !self.witness_gen_only {
            self.selector.resize(self.advice.len(), false);
        }
        self.last().unwrap()
    }

    /// Assigns a list of constant values and returns the corresponding assigned cells.
    /// * `c`: the list of constant values to be assigned
    pub fn load_constants(&mut self, c: &[F]) -> Vec<AssignedValue<F>> {
        c.iter().map(|v| self.load_constant(*v)).collect_vec()
    }

    /// Assigns the 0 value to a new cell or returns a previously assigned zero cell from `zero_cell`.
    pub fn load_zero(&mut self) -> AssignedValue<F> {
        if let Some(zcell) = &self.zero_cell {
            return *zcell;
        }
        let zero_cell = self.load_constant(F::ZERO);
        self.zero_cell = Some(zero_cell);
        zero_cell
    }

    /// Helper function for debugging using `MockProver`. This adds a constraint that always fails.
    /// The `MockProver` will print out the row, column where it fails, so it serves as a debugging "break point"
    /// so you can add to your code to search for where the actual constraint failure occurs.
    pub fn debug_assert_false(&mut self) {
        use rand_chacha::rand_core::OsRng;
        let rand1 = self.load_witness(F::random(OsRng));
        let rand2 = self.load_witness(F::random(OsRng));
        self.constrain_equal(&rand1, &rand2);
    }
}
