use std::cell::RefCell;

use getset::CopyGetters;

use crate::{
    gates::{
        circuit::CircuitBuilderStage,
        flex_gate::{BasicGateConfig, ThreadBreakPoints},
    },
    utils::halo2::{raw_assign_advice, raw_constrain_equal},
    utils::ScalarField,
    virtual_region::copy_constraints::{CopyConstraintManager, SharedCopyConstraintManager},
    Context, ContextCell,
};
use crate::{
    halo2_proofs::circuit::{Region, Value},
    virtual_region::manager::VirtualRegionManager,
};

/// Virtual region manager for [`Vec<BasicGateConfig>`] in a single challenge phase.
/// This is the core manager for [Context]s.
#[derive(Clone, Debug, Default, CopyGetters)]
pub struct SinglePhaseCoreManager<F: ScalarField> {
    /// Virtual columns. These cannot be shared across CPU threads while keeping the circuit deterministic.
    pub threads: Vec<Context<F>>,
    /// Global shared copy manager
    pub copy_manager: SharedCopyConstraintManager<F>,
    /// Flag for witness generation. If true, the gate thread builder is used for witness generation only.
    #[getset(get_copy = "pub")]
    witness_gen_only: bool,
    /// The `unknown` flag is used during key generation. If true, during key generation witness [Value]s are replaced with Value::unknown() for safety.
    #[getset(get_copy = "pub")]
    pub(crate) use_unknown: bool,
    /// The challenge phase the virtual regions will map to.
    #[getset(get_copy = "pub", set)]
    pub(crate) phase: usize,
    /// A very simple computation graph for the basic vertical gate. Must be provided as a "pinning"
    /// when running the production prover.
    pub break_points: RefCell<Option<ThreadBreakPoints>>,
}

impl<F: ScalarField> SinglePhaseCoreManager<F> {
    /// Creates a new [SinglePhaseCoreManager] and spawns a main thread.
    /// * `witness_gen_only`: If true, the [SinglePhaseCoreManager] is used for witness generation only.
    ///     * If true, the gate thread builder only does witness asignments and does not store constraint information -- this should only be used for the real prover.
    ///     * If false, the gate thread builder is used for keygen and mock prover (it can also be used for real prover) and the builder stores circuit information (e.g. copy constraints, fixed columns, enabled selectors).
    ///         * These values are fixed for the circuit at key generation time, and they do not need to be re-computed by the prover in the actual proving phase.
    pub fn new(witness_gen_only: bool, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        Self {
            threads: vec![],
            witness_gen_only,
            use_unknown: false,
            phase: 0,
            copy_manager,
            ..Default::default()
        }
    }

    /// Sets the phase to `phase`
    pub fn in_phase(self, phase: usize) -> Self {
        Self { phase, ..self }
    }

    /// Creates a new [SinglePhaseCoreManager] depending on the stage of circuit building. If the stage is [CircuitBuilderStage::Prover], the [SinglePhaseCoreManager] is used for witness generation only.
    pub fn from_stage(
        stage: CircuitBuilderStage,
        copy_manager: SharedCopyConstraintManager<F>,
    ) -> Self {
        Self::new(stage.witness_gen_only(), copy_manager)
            .unknown(stage == CircuitBuilderStage::Keygen)
    }

    /// Creates a new [SinglePhaseCoreManager] with `use_unknown` flag set.
    /// * `use_unknown`: If true, during key generation witness [Value]s are replaced with Value::unknown() for safety.
    pub fn unknown(self, use_unknown: bool) -> Self {
        Self { use_unknown, ..self }
    }

    /// Mutates `self` to use the given copy manager everywhere, including in all threads.
    pub fn set_copy_manager(&mut self, copy_manager: SharedCopyConstraintManager<F>) {
        self.copy_manager = copy_manager.clone();
        for ctx in &mut self.threads {
            ctx.copy_manager = copy_manager.clone();
        }
    }

    /// Returns `self` with a given copy manager
    pub fn use_copy_manager(mut self, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        self.set_copy_manager(copy_manager);
        self
    }

    /// Clears all threads and copy manager
    pub fn clear(&mut self) {
        self.threads = vec![];
        self.copy_manager.lock().unwrap().clear();
    }

    /// Returns a mutable reference to the [Context] of a gate thread. Spawns a new thread for the given phase, if none exists.
    pub fn main(&mut self) -> &mut Context<F> {
        if self.threads.is_empty() {
            self.new_thread()
        } else {
            self.threads.last_mut().unwrap()
        }
    }

    /// Returns the number of threads
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// A distinct tag for this particular type of virtual manager, which is different for each phase.
    pub fn type_of(&self) -> &'static str {
        match self.phase {
            0 => "halo2-base:SinglePhaseCoreManager:FirstPhase",
            1 => "halo2-base:SinglePhaseCoreManager:SecondPhase",
            2 => "halo2-base:SinglePhaseCoreManager:ThirdPhase",
            _ => panic!("Unsupported phase"),
        }
    }

    /// Creates new context but does not append to `self.threads`
    pub fn new_context(&self, context_id: usize) -> Context<F> {
        Context::new(
            self.witness_gen_only,
            self.phase,
            self.type_of(),
            context_id,
            self.copy_manager.clone(),
        )
    }

    /// Spawns a new thread for a new given `phase`. Returns a mutable reference to the [Context] of the new thread.
    /// * `phase`: The phase (index) of the gate thread.
    pub fn new_thread(&mut self) -> &mut Context<F> {
        let context_id = self.thread_count();
        self.threads.push(self.new_context(context_id));
        self.threads.last_mut().unwrap()
    }

    /// Returns total advice cells
    pub fn total_advice(&self) -> usize {
        self.threads.iter().map(|ctx| ctx.advice.len()).sum::<usize>()
    }
}

impl<F: ScalarField> VirtualRegionManager<F> for SinglePhaseCoreManager<F> {
    type Config = (Vec<BasicGateConfig<F>>, usize); // usize = usable_rows
    type Assignment = ();

    fn assign_raw(&self, (config, usable_rows): &Self::Config, region: &mut Region<F>) {
        if self.witness_gen_only {
            let binding = self.break_points.borrow();
            let break_points = binding.as_ref().expect("break points not set");
            assign_witnesses(&self.threads, config, region, break_points);
        } else {
            let mut copy_manager = self.copy_manager.lock().unwrap();
            let break_points = assign_with_constraints::<F, 4>(
                &self.threads,
                config,
                region,
                &mut copy_manager,
                *usable_rows,
                self.use_unknown,
            );
            let mut bp = self.break_points.borrow_mut();
            if let Some(bp) = bp.as_ref() {
                assert_eq!(bp, &break_points, "break points don't match");
            } else {
                *bp = Some(break_points);
            }
        }
    }
}

/// Assigns all virtual `threads` to the physical columns in `basic_gates` and returns the break points.
/// Also enables corresponding selectors and adds raw assigned cells to the `copy_manager`.
/// This function should be called either during proving & verifier key generation or when running MockProver.
///
/// For proof generation, see [assign_witnesses].
///
/// This is generic for a "vertical" custom gate that uses a single column and `ROTATIONS` contiguous rows in that column.
///
/// ⚠️ Right now we only support "overlaps" where you can have the gate enabled at `offset` and `offset + ROTATIONS - 1`, but not at `offset + delta` where `delta < ROTATIONS - 1`.
///
/// # Inputs
/// - `max_rows`: The number of rows that can be used for the assignment. This is the number of rows that are not blinded for zero-knowledge.
/// - If `use_unknown` is true, then the advice columns will be assigned as unknowns.
///
/// # Assumptions
/// - All `basic_gates` are in the same phase.
pub fn assign_with_constraints<F: ScalarField, const ROTATIONS: usize>(
    threads: &[Context<F>],
    basic_gates: &[BasicGateConfig<F>],
    region: &mut Region<F>,
    copy_manager: &mut CopyConstraintManager<F>,
    max_rows: usize,
    use_unknown: bool,
) -> ThreadBreakPoints {
    let mut break_points = vec![];
    let mut gate_index = 0;
    let mut row_offset = 0;
    for ctx in threads {
        if ctx.advice.is_empty() {
            continue;
        }
        let mut basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
        assert_eq!(ctx.selector.len(), ctx.advice.len());

        for (i, (advice, &q)) in ctx.advice.iter().zip(ctx.selector.iter()).enumerate() {
            let column = basic_gate.value;
            let value = if use_unknown { Value::unknown() } else { Value::known(advice) };
            #[cfg(feature = "halo2-axiom")]
            let cell = region.assign_advice(column, row_offset, value).cell();
            #[cfg(not(feature = "halo2-axiom"))]
            let cell = region
                .assign_advice(|| "", column, row_offset, || value.map(|v| *v))
                .unwrap()
                .cell();
            if let Some(old_cell) = copy_manager
                .assigned_advices
                .insert(ContextCell::new(ctx.type_id, ctx.context_id, i), cell)
            {
                assert!(
                    old_cell.row_offset == cell.row_offset && old_cell.column == cell.column,
                    "Trying to overwrite virtual cell with a different raw cell"
                );
            }

            // If selector enabled and row_offset is valid add break point, account for break point overlap, and enforce equality constraint for gate outputs.
            // ⚠️ This assumes overlap is of form: gate enabled at `i - delta` and `i`, where `delta = ROTATIONS - 1`. We currently do not support `delta < ROTATIONS - 1`.
            if (q && row_offset + ROTATIONS > max_rows) || row_offset >= max_rows - 1 {
                break_points.push(row_offset);
                row_offset = 0;
                gate_index += 1;

                // safety check: make sure selector is not enabled on `i - delta` for `0 < delta < ROTATIONS - 1`
                if ROTATIONS > 1 && i + 2 >= ROTATIONS {
                    for delta in 1..ROTATIONS - 1 {
                        assert!(
                            !ctx.selector[i - delta],
                            "We do not support overlaps with delta = {delta}"
                        );
                    }
                }
                // when there is a break point, because we may have two gates that overlap at the current cell, we must copy the current cell to the next column for safety
                basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
                let column = basic_gate.value;
                #[cfg(feature = "halo2-axiom")]
                let ncell = region.assign_advice(column, row_offset, value);
                #[cfg(not(feature = "halo2-axiom"))]
                let ncell =
                    region.assign_advice(|| "", column, row_offset, || value.map(|v| *v)).unwrap();
                raw_constrain_equal(region, ncell.cell(), cell);
            }

            if q {
                basic_gate
                    .q_enable
                    .enable(region, row_offset)
                    .expect("enable selector should not fail");
            }

            row_offset += 1;
        }
    }
    break_points
}

/// Assigns all virtual `threads` to the physical columns in `basic_gates` according to a precomputed "computation graph"
/// given by `break_points`. (`break_points` tells the assigner when to move to the next column.)
///
/// This function does not impose **any** constraints. It only assigns witnesses to advice columns, and should be called
/// only during proof generation.
///
/// # Assumptions
/// - All `basic_gates` are in the same phase.
pub fn assign_witnesses<F: ScalarField>(
    threads: &[Context<F>],
    basic_gates: &[BasicGateConfig<F>],
    region: &mut Region<F>,
    break_points: &ThreadBreakPoints,
) {
    if basic_gates.is_empty() {
        assert_eq!(
            threads.iter().map(|ctx| ctx.advice.len()).sum::<usize>(),
            0,
            "Trying to assign threads in a phase with no columns"
        );
        return;
    }

    let mut break_points = break_points.clone().into_iter();
    let mut break_point = break_points.next();

    let mut gate_index = 0;
    let mut column = basic_gates[gate_index].value;
    let mut row_offset = 0;

    for ctx in threads {
        // Assign advice values to the advice columns in each [Context]
        for advice in &ctx.advice {
            raw_assign_advice(region, column, row_offset, Value::known(advice));

            if break_point == Some(row_offset) {
                break_point = break_points.next();
                row_offset = 0;
                gate_index += 1;
                column = basic_gates[gate_index].value;

                raw_assign_advice(region, column, row_offset, Value::known(advice));
            }

            row_offset += 1;
        }
    }
}
