use getset::CopyGetters;
use itertools::Itertools;

use crate::{
    gates::{circuit::CircuitBuilderStage, flex_gate::FlexGateConfigParams},
    utils::ScalarField,
    virtual_region::copy_constraints::SharedCopyConstraintManager,
    Context,
};

use super::SinglePhaseCoreManager;

/// Virtual region manager for [`FlexGateConfig`](super::super::FlexGateConfig) in multiple phases.
#[derive(Clone, Debug, Default, CopyGetters)]
pub struct MultiPhaseCoreManager<F: ScalarField> {
    /// Virtual region for each challenge phase. These cannot be shared across threads while keeping circuit deterministic.
    pub phase_manager: Vec<SinglePhaseCoreManager<F>>,
    /// Global shared copy manager
    pub copy_manager: SharedCopyConstraintManager<F>,
    /// Flag for witness generation. If true, the gate thread builder is used for witness generation only.
    #[getset(get_copy = "pub")]
    witness_gen_only: bool,
    /// The `unknown` flag is used during key generation. If true, during key generation witness `Value`s are replaced with `Value::unknown()` for safety.
    #[getset(get_copy = "pub")]
    use_unknown: bool,
}

impl<F: ScalarField> MultiPhaseCoreManager<F> {
    /// Creates a new [MultiPhaseCoreManager] with a default [SinglePhaseCoreManager] in phase 0.
    /// Creates an empty [SharedCopyConstraintManager] and sets `witness_gen_only` flag.
    /// * `witness_gen_only`: If true, the [MultiPhaseCoreManager] is used for witness generation only.
    ///     * If true, the gate thread builder only does witness asignments and does not store constraint information -- this should only be used for the real prover.
    ///     * If false, the gate thread builder is used for keygen and mock prover (it can also be used for real prover) and the builder stores circuit information (e.g. copy constraints, fixed columns, enabled selectors).
    ///         * These values are fixed for the circuit at key generation time, and they do not need to be re-computed by the prover in the actual proving phase.
    pub fn new(witness_gen_only: bool) -> Self {
        let copy_manager = SharedCopyConstraintManager::default();
        let phase_manager =
            vec![SinglePhaseCoreManager::new(witness_gen_only, copy_manager.clone())];
        Self { phase_manager, witness_gen_only, use_unknown: false, copy_manager }
    }

    /// Creates a new [MultiPhaseCoreManager] depending on the stage of circuit building. If the stage is [CircuitBuilderStage::Prover], the [MultiPhaseCoreManager] is used for witness generation only.
    pub fn from_stage(stage: CircuitBuilderStage) -> Self {
        Self::new(stage.witness_gen_only()).unknown(stage == CircuitBuilderStage::Keygen)
    }

    /// Mutates `self` to use the given copy manager in all phases and all threads.
    pub fn set_copy_manager(&mut self, copy_manager: SharedCopyConstraintManager<F>) {
        for pm in &mut self.phase_manager {
            pm.set_copy_manager(copy_manager.clone());
        }
        self.copy_manager = copy_manager;
    }

    /// Returns `self` with a given copy manager
    pub fn use_copy_manager(mut self, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        self.set_copy_manager(copy_manager);
        self
    }

    /// Creates a new [MultiPhaseCoreManager] with `use_unknown` flag set.
    /// * `use_unknown`: If true, during key generation witness values are replaced with `Value::unknown()` for safety.
    pub fn unknown(mut self, use_unknown: bool) -> Self {
        self.use_unknown = use_unknown;
        for pm in &mut self.phase_manager {
            pm.use_unknown = use_unknown;
        }
        self
    }

    /// Clears all threads in all phases and copy manager.
    pub fn clear(&mut self) {
        for pm in &mut self.phase_manager {
            pm.clear();
        }
        self.copy_manager.lock().unwrap().clear();
    }

    /// Returns a mutable reference to the [Context] of a gate thread. Spawns a new thread for the given phase, if none exists.
    /// * `phase`: The challenge phase (as an index) of the gate thread.
    pub fn main(&mut self, phase: usize) -> &mut Context<F> {
        self.touch(phase);
        self.phase_manager[phase].main()
    }

    /// Spawns a new thread for a new given `phase`. Returns a mutable reference to the [Context] of the new thread.
    /// * `phase`: The phase (index) of the gate thread.
    pub fn new_thread(&mut self, phase: usize) -> &mut Context<F> {
        self.touch(phase);
        self.phase_manager[phase].new_thread()
    }

    /// Returns a mutable reference to the [SinglePhaseCoreManager] of a given `phase`.
    pub fn in_phase(&mut self, phase: usize) -> &mut SinglePhaseCoreManager<F> {
        self.phase_manager.get_mut(phase).unwrap()
    }

    /// Populate `self` up to Phase `phase` (inclusive)
    pub(crate) fn touch(&mut self, phase: usize) {
        while self.phase_manager.len() <= phase {
            let _phase = self.phase_manager.len();
            let pm = SinglePhaseCoreManager::new(self.witness_gen_only, self.copy_manager.clone())
                .in_phase(_phase);
            self.phase_manager.push(pm);
        }
    }

    /// Returns some statistics about the virtual region.
    pub fn statistics(&self) -> GateStatistics {
        let total_advice_per_phase =
            self.phase_manager.iter().map(|pm| pm.total_advice()).collect::<Vec<_>>();

        let total_fixed: usize = self
            .copy_manager
            .lock()
            .unwrap()
            .constant_equalities
            .iter()
            .map(|(c, _)| *c)
            .sorted()
            .dedup()
            .count();

        GateStatistics { total_advice_per_phase, total_fixed }
    }

    /// Auto-calculates configuration parameters for the circuit
    ///
    /// * `k`: The number of in the circuit (i.e. numeber of rows = 2<sup>k</sup>)
    /// * `minimum_rows`: The minimum number of rows in the circuit that cannot be used for witness assignments and contain random `blinding factors` to ensure zk property, defaults to 0.
    pub fn calculate_params(&self, k: usize, minimum_rows: Option<usize>) -> FlexGateConfigParams {
        let max_rows = (1 << k) - minimum_rows.unwrap_or(0);
        let stats = self.statistics();
        // we do a rough estimate by taking ceil(advice_cells_per_phase / 2^k )
        // if this is too small, manual configuration will be needed
        let num_advice_per_phase = stats
            .total_advice_per_phase
            .iter()
            .map(|count| count.div_ceil(max_rows))
            .collect::<Vec<_>>();
        let num_fixed = (stats.total_fixed + (1 << k) - 1) >> k;

        let params = FlexGateConfigParams { num_advice_per_phase, num_fixed, k };
        #[cfg(feature = "display")]
        {
            for (phase, num_advice) in stats.total_advice_per_phase.iter().enumerate() {
                println!("Gate Chip | Phase {phase}: {num_advice} advice cells",);
            }
            println!("Total {} fixed cells", stats.total_fixed);
            log::debug!("Auto-calculated config params:\n {params:#?}");
        }
        params
    }
}

/// Basic statistics
pub struct GateStatistics {
    /// Total advice cell count per phase
    pub total_advice_per_phase: Vec<usize>,
    /// Total distinct constants used
    pub total_fixed: usize,
}
