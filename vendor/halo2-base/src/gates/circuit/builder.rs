use std::sync::{Arc, Mutex};

use getset::{Getters, MutGetters, Setters};
use itertools::Itertools;

use crate::{
    gates::{
        circuit::CircuitBuilderStage,
        flex_gate::{
            threads::{GateStatistics, MultiPhaseCoreManager, SinglePhaseCoreManager},
            MultiPhaseThreadBreakPoints, MAX_PHASE,
        },
        range::RangeConfig,
        RangeChip,
    },
    halo2_proofs::{
        circuit::{Layouter, Region},
        plonk::{Column, Instance},
    },
    utils::ScalarField,
    virtual_region::{
        copy_constraints::{CopyConstraintManager, SharedCopyConstraintManager},
        lookups::LookupAnyManager,
        manager::VirtualRegionManager,
    },
    AssignedValue, Context,
};

use super::BaseCircuitParams;

/// Keeping the naming `RangeCircuitBuilder` for backwards compatibility.
pub type RangeCircuitBuilder<F> = BaseCircuitBuilder<F>;

/// A circuit builder is a collection of virtual region managers that together assign virtual
/// regions into a single physical circuit.
///
/// [BaseCircuitBuilder] is a circuit builder to create a circuit where the columns correspond to [super::BaseConfig].
/// This builder can hold multiple threads, but the `Circuit` implementation only evaluates the first phase.
/// The user will have to implement a separate `Circuit` with multi-phase witness generation logic.
///
/// This is used to manage the virtual region corresponding to [super::FlexGateConfig] and (optionally) [RangeConfig].
/// This can be used even if only using [`GateChip`](crate::gates::flex_gate::GateChip) without [RangeChip].
///
/// The circuit will have `NI` public instance (aka public inputs+outputs) columns.
#[derive(Clone, Debug, Getters, MutGetters, Setters)]
pub struct BaseCircuitBuilder<F: ScalarField> {
    /// Virtual region for each challenge phase. These cannot be shared across threads while keeping circuit deterministic.
    #[getset(get = "pub", get_mut = "pub", set = "pub")]
    pub(super) core: MultiPhaseCoreManager<F>,
    /// The range lookup manager
    #[getset(get = "pub", get_mut = "pub", set = "pub")]
    pub(super) lookup_manager: [LookupAnyManager<F, 1>; MAX_PHASE],
    /// Configuration parameters for the circuit shape
    pub config_params: BaseCircuitParams,
    /// The assigned instances to expose publicly at the end of circuit synthesis
    pub assigned_instances: Vec<Vec<AssignedValue<F>>>,
}

impl<F: ScalarField> Default for BaseCircuitBuilder<F> {
    /// Quick start default circuit builder which can be used for MockProver, Keygen, and real prover.
    /// For best performance during real proof generation, we recommend using [BaseCircuitBuilder::prover] instead.
    fn default() -> Self {
        Self::new(false)
    }
}

impl<F: ScalarField> BaseCircuitBuilder<F> {
    /// Creates a new [BaseCircuitBuilder] with all default managers.
    /// * `witness_gen_only`:
    ///     * If true, the builder only does witness asignments and does not store constraint information -- this should only be used for the real prover.
    ///     * If false, the builder also imposes constraints (selectors, fixed columns, copy constraints). Primarily used for keygen and mock prover (but can also be used for real prover).
    ///
    /// By default, **no** circuit configuration parameters have been set.
    /// These should be set separately using `use_params`, or `use_k`, `use_lookup_bits`, and `calculate_params`.
    ///
    /// Upon construction, there are no public instances (aka all witnesses are private).
    /// The intended usage is that _before_ calling `synthesize`, witness generation can be done to populate
    /// assigned instances, which are supplied as `assigned_instances` to this struct.
    /// The `Circuit` implementation for this struct will then expose these instances and constrain
    /// them using the Halo2 API.
    pub fn new(witness_gen_only: bool) -> Self {
        let core = MultiPhaseCoreManager::new(witness_gen_only);
        let lookup_manager = [(); MAX_PHASE]
            .map(|_| LookupAnyManager::new(witness_gen_only, core.copy_manager.clone()));
        Self { core, lookup_manager, config_params: Default::default(), assigned_instances: vec![] }
    }

    /// Creates a new [MultiPhaseCoreManager] depending on the stage of circuit building. If the stage is [CircuitBuilderStage::Prover], the [MultiPhaseCoreManager] is used for witness generation only.
    pub fn from_stage(stage: CircuitBuilderStage) -> Self {
        Self::new(stage.witness_gen_only()).unknown(stage == CircuitBuilderStage::Keygen)
    }

    /// Creates a new [BaseCircuitBuilder] with a pinned circuit configuration given by `config_params` and `break_points`.
    pub fn prover(
        config_params: BaseCircuitParams,
        break_points: MultiPhaseThreadBreakPoints,
    ) -> Self {
        Self::new(true).use_params(config_params).use_break_points(break_points)
    }

    /// Sets the copy manager to the given one in all shared references.
    pub fn set_copy_manager(&mut self, copy_manager: SharedCopyConstraintManager<F>) {
        for lm in &mut self.lookup_manager {
            lm.set_copy_manager(copy_manager.clone());
        }
        self.core.set_copy_manager(copy_manager);
    }

    /// Returns `self` with a given copy manager
    pub fn use_copy_manager(mut self, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        self.set_copy_manager(copy_manager);
        self
    }

    /// Deep clone of `self`, where the underlying object of shared references in [SharedCopyConstraintManager] and [LookupAnyManager] are cloned.
    pub fn deep_clone(&self) -> Self {
        let cm: CopyConstraintManager<F> = self.core.copy_manager.lock().unwrap().clone();
        let cm_ref = Arc::new(Mutex::new(cm));
        let mut clone = self.clone().use_copy_manager(cm_ref.clone());
        for lm in &mut clone.lookup_manager {
            *lm = lm.deep_clone(cm_ref.clone());
        }
        clone
    }

    /// The log_2 size of the lookup table, if using.
    pub fn lookup_bits(&self) -> Option<usize> {
        self.config_params.lookup_bits
    }

    /// Set lookup bits
    pub fn set_lookup_bits(&mut self, lookup_bits: usize) {
        self.config_params.lookup_bits = Some(lookup_bits);
    }

    /// Returns new with lookup bits
    pub fn use_lookup_bits(mut self, lookup_bits: usize) -> Self {
        self.set_lookup_bits(lookup_bits);
        self
    }

    /// Sets new `k` = log2 of domain
    pub fn set_k(&mut self, k: usize) {
        self.config_params.k = k;
    }

    /// Returns new with `k` set
    pub fn use_k(mut self, k: usize) -> Self {
        self.set_k(k);
        self
    }

    /// Set the number of instance columns. This resizes `self.assigned_instances`.
    pub fn set_instance_columns(&mut self, num_instance_columns: usize) {
        self.config_params.num_instance_columns = num_instance_columns;
        while self.assigned_instances.len() < num_instance_columns {
            self.assigned_instances.push(vec![]);
        }
        assert_eq!(self.assigned_instances.len(), num_instance_columns);
    }

    /// Returns new with `self.assigned_instances` resized to specified number of instance columns.
    pub fn use_instance_columns(mut self, num_instance_columns: usize) -> Self {
        self.set_instance_columns(num_instance_columns);
        self
    }

    /// Set config params
    pub fn set_params(&mut self, params: BaseCircuitParams) {
        self.set_instance_columns(params.num_instance_columns);
        self.config_params = params;
    }

    /// Returns new with config params
    pub fn use_params(mut self, params: BaseCircuitParams) -> Self {
        self.set_params(params);
        self
    }

    /// The break points of the circuit.
    pub fn break_points(&self) -> MultiPhaseThreadBreakPoints {
        self.core
            .phase_manager
            .iter()
            .map(|pm| pm.break_points.borrow().as_ref().expect("break points not set").clone())
            .collect()
    }

    /// Sets the break points of the circuit.
    pub fn set_break_points(&mut self, break_points: MultiPhaseThreadBreakPoints) {
        if break_points.is_empty() {
            return;
        }
        self.core.touch(break_points.len() - 1);
        for (pm, bp) in self.core.phase_manager.iter().zip_eq(break_points) {
            *pm.break_points.borrow_mut() = Some(bp);
        }
    }

    /// Returns new with break points
    pub fn use_break_points(mut self, break_points: MultiPhaseThreadBreakPoints) -> Self {
        self.set_break_points(break_points);
        self
    }

    /// Returns if the circuit is only used for witness generation.
    pub fn witness_gen_only(&self) -> bool {
        self.core.witness_gen_only()
    }

    /// Creates a new [MultiPhaseCoreManager] with `use_unknown` flag set.
    /// * `use_unknown`: If true, during key generation witness `Value`s are replaced with `Value::unknown()` for safety.
    pub fn unknown(mut self, use_unknown: bool) -> Self {
        self.core = self.core.unknown(use_unknown);
        self
    }

    /// Clears state and copies, effectively resetting the circuit builder.
    pub fn clear(&mut self) {
        self.core.clear();
        for lm in &mut self.lookup_manager {
            lm.clear();
        }
        self.assigned_instances.iter_mut().for_each(|c| c.clear());
    }

    /// Returns a mutable reference to the [Context] of a gate thread. Spawns a new thread for the given phase, if none exists.
    /// * `phase`: The challenge phase (as an index) of the gate thread.
    pub fn main(&mut self, phase: usize) -> &mut Context<F> {
        self.core.main(phase)
    }

    /// Returns [SinglePhaseCoreManager] with the virtual region with all core threads in the given phase.
    pub fn pool(&mut self, phase: usize) -> &mut SinglePhaseCoreManager<F> {
        self.core.phase_manager.get_mut(phase).unwrap()
    }

    /// Spawns a new thread for a new given `phase`. Returns a mutable reference to the [Context] of the new thread.
    /// * `phase`: The phase (index) of the gate thread.
    pub fn new_thread(&mut self, phase: usize) -> &mut Context<F> {
        self.core.new_thread(phase)
    }

    /// Returns some statistics about the virtual region.
    pub fn statistics(&self) -> RangeStatistics {
        let gate = self.core.statistics();
        let total_lookup_advice_per_phase = self.total_lookup_advice_per_phase();
        RangeStatistics { gate, total_lookup_advice_per_phase }
    }

    fn total_lookup_advice_per_phase(&self) -> Vec<usize> {
        self.lookup_manager.iter().map(|lm| lm.total_rows()).collect()
    }

    /// Auto-calculates configuration parameters for the circuit and sets them.
    ///
    /// * `k`: The number of in the circuit (i.e. numeber of rows = 2<sup>k</sup>)
    /// * `minimum_rows`: The minimum number of rows in the circuit that cannot be used for witness assignments and contain random `blinding factors` to ensure zk property, defaults to 0.
    /// * `lookup_bits`: The fixed lookup table will consist of [0, 2<sup>lookup_bits</sup>)
    pub fn calculate_params(&mut self, minimum_rows: Option<usize>) -> BaseCircuitParams {
        let k = self.config_params.k;
        let ni = self.config_params.num_instance_columns;
        assert_ne!(k, 0, "k must be set");
        let max_rows = (1 << k) - minimum_rows.unwrap_or(0);
        let gate_params = self.core.calculate_params(k, minimum_rows);
        let total_lookup_advice_per_phase = self.total_lookup_advice_per_phase();
        let num_lookup_advice_per_phase = total_lookup_advice_per_phase
            .iter()
            .map(|count| count.div_ceil(max_rows))
            .collect::<Vec<_>>();

        let params = BaseCircuitParams {
            k: gate_params.k,
            num_advice_per_phase: gate_params.num_advice_per_phase,
            num_fixed: gate_params.num_fixed,
            num_lookup_advice_per_phase,
            lookup_bits: self.lookup_bits(),
            num_instance_columns: ni,
        };
        self.config_params = params.clone();
        #[cfg(feature = "display")]
        {
            println!("Total range check advice cells to lookup per phase: {total_lookup_advice_per_phase:?}");
            log::debug!("Auto-calculated config params:\n {params:#?}");
        }
        params
    }

    /// Copies `assigned_instances` to the instance columns. Should only be called at the very end of
    /// `synthesize` after virtual `assigned_instances` have been assigned to physical circuit.
    pub fn assign_instances(
        &self,
        instance_columns: &[Column<Instance>],
        mut layouter: impl Layouter<F>,
    ) {
        if !self.core.witness_gen_only() {
            // expose public instances
            for (instances, instance_col) in self.assigned_instances.iter().zip_eq(instance_columns)
            {
                for (i, instance) in instances.iter().enumerate() {
                    let cell = instance.cell.unwrap();
                    let copy_manager = self.core.copy_manager.lock().unwrap();
                    let cell =
                        copy_manager.assigned_advices.get(&cell).expect("instance not assigned");
                    layouter.constrain_instance(*cell, *instance_col, i);
                }
            }
        }
    }

    /// Creates a new [RangeChip] sharing the same [LookupAnyManager]s as `self`.
    pub fn range_chip(&self) -> RangeChip<F> {
        RangeChip::new(
            self.config_params.lookup_bits.expect("lookup bits not set"),
            self.lookup_manager.clone(),
        )
    }

    /// Copies the queued cells to be range looked up in phase `phase` to special advice lookup columns
    /// using [LookupAnyManager].
    ///
    /// ## Special case
    /// Just for [RangeConfig], we have special handling for the case where there is a single (physical)
    /// advice column in [super::FlexGateConfig]. In this case, `RangeConfig` does not create extra lookup advice columns,
    /// the single advice column has lookup enabled, and there is a selector to toggle when lookup should
    /// be turned on.
    pub fn assign_lookups_in_phase(
        &self,
        config: &RangeConfig<F>,
        region: &mut Region<F>,
        phase: usize,
    ) {
        let lookup_manager = self.lookup_manager.get(phase).expect("too many phases");
        if lookup_manager.total_rows() == 0 {
            return;
        }
        if let Some(q_lookup) = config.q_lookup.get(phase).and_then(|q| *q) {
            // if q_lookup is Some, that means there should be a single advice column and it has lookup enabled
            assert_eq!(config.gate.basic_gates[phase].len(), 1);
            if !self.witness_gen_only() {
                let cells_to_lookup = lookup_manager.cells_to_lookup.lock().unwrap();
                for advice in cells_to_lookup.iter().flat_map(|(_, advices)| advices) {
                    let cell = advice[0].cell.as_ref().unwrap();
                    let copy_manager = self.core.copy_manager.lock().unwrap();
                    let acell = copy_manager.assigned_advices[cell];
                    assert!(
                        acell.row_offset < config.gate.max_rows,
                        "range lookup assigned to an unusable row"
                    );
                    assert_eq!(
                        acell.column,
                        config.gate.basic_gates[phase][0].value.into(),
                        "lookup column does not match"
                    );
                    q_lookup.enable(region, acell.row_offset).unwrap();
                }
            }
        } else {
            let lookup_cols = config
                .lookup_advice
                .get(phase)
                .expect("No special lookup advice columns")
                .iter()
                .map(|c| [*c])
                .collect_vec();
            assert!(!lookup_cols.is_empty(), "range lookups require lookup advice columns");
            let lookup_rows = lookup_manager.total_rows();
            let assigned_rows = lookup_rows.div_ceil(lookup_cols.len());
            assert!(
                assigned_rows <= config.gate.max_rows,
                "range lookups would be assigned to unusable rows"
            );
            lookup_manager.assign_raw(&lookup_cols, region);
        }
        let _ = lookup_manager.assigned.set(());
    }
}

/// Basic statistics
pub struct RangeStatistics {
    /// Number of advice cells for the basic gate and total constants used
    pub gate: GateStatistics,
    /// Total special advice cells that need to be looked up, per phase
    pub total_lookup_advice_per_phase: Vec<usize>,
}

impl<F: ScalarField> AsRef<BaseCircuitBuilder<F>> for BaseCircuitBuilder<F> {
    fn as_ref(&self) -> &BaseCircuitBuilder<F> {
        self
    }
}

impl<F: ScalarField> AsMut<BaseCircuitBuilder<F>> for BaseCircuitBuilder<F> {
    fn as_mut(&mut self) -> &mut BaseCircuitBuilder<F> {
        self
    }
}
