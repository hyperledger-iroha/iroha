use serde::{Deserialize, Serialize};

use crate::utils::ScalarField;
use crate::{
    halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        plonk::{Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector},
    },
    virtual_region::manager::VirtualRegionManager,
};

use self::builder::BaseCircuitBuilder;

use super::flex_gate::{FlexGateConfig, FlexGateConfigParams};
use super::range::RangeConfig;

/// Module that helps auto-build circuits
pub mod builder;

/// A struct defining the configuration parameters for a halo2-base circuit
/// - this is used to configure [BaseConfig].
#[derive(Clone, Default, Debug, Hash, Serialize, Deserialize)]
pub struct BaseCircuitParams {
    // Keeping FlexGateConfigParams expanded for backwards compatibility
    /// Specifies the number of rows in the circuit to be 2<sup>k</sup>
    pub k: usize,
    /// The number of advice columns per phase
    pub num_advice_per_phase: Vec<usize>,
    /// The number of fixed columns
    pub num_fixed: usize,
    /// The number of bits that can be ranged checked using a special lookup table with values [0, 2<sup>lookup_bits</sup>), if using.
    /// The number of special advice columns that have range lookup enabled per phase
    pub num_lookup_advice_per_phase: Vec<usize>,
    /// This is `None` if no lookup table is used.
    pub lookup_bits: Option<usize>,
    /// Number of public instance columns
    #[serde(default)]
    pub num_instance_columns: usize,
}

impl BaseCircuitParams {
    fn gate_params(&self) -> FlexGateConfigParams {
        FlexGateConfigParams {
            k: self.k,
            num_advice_per_phase: self.num_advice_per_phase.clone(),
            num_fixed: self.num_fixed,
        }
    }
}

/// Configuration with [`BaseConfig`] with `NI` public instance columns.
#[derive(Clone, Debug)]
pub struct BaseConfig<F: ScalarField> {
    /// The underlying private gate/range configuration
    pub base: MaybeRangeConfig<F>,
    /// The public instance column
    pub instance: Vec<Column<Instance>>,
}

/// Smart Halo2 circuit config that has different variants depending on whether you need range checks or not.
/// The difference is that to enable range checks, the Halo2 config needs to add a lookup table.
#[derive(Clone, Debug)]
pub enum MaybeRangeConfig<F: ScalarField> {
    /// Config for a circuit that does not use range checks
    WithoutRange(FlexGateConfig<F>),
    /// Config for a circuit that does use range checks
    WithRange(RangeConfig<F>),
}

impl<F: ScalarField> BaseConfig<F> {
    /// Generates a new `BaseConfig` depending on `params`.
    /// - It will generate a `RangeConfig` is `params` has `lookup_bits` not None **and** `num_lookup_advice_per_phase` are not all empty or zero (i.e., if `params` indicates that the circuit actually requires a lookup table).
    /// - Otherwise it will generate a `FlexGateConfig`.
    pub fn configure(meta: &mut ConstraintSystem<F>, params: BaseCircuitParams) -> Self {
        let total_lookup_advice_cols = params.num_lookup_advice_per_phase.iter().sum::<usize>();
        let base = if params.lookup_bits.is_some() && total_lookup_advice_cols != 0 {
            // We only add a lookup table if lookup bits is not None
            MaybeRangeConfig::WithRange(RangeConfig::configure(
                meta,
                params.gate_params(),
                &params.num_lookup_advice_per_phase,
                params.lookup_bits.unwrap(),
            ))
        } else {
            MaybeRangeConfig::WithoutRange(FlexGateConfig::configure(meta, params.gate_params()))
        };
        let instance = (0..params.num_instance_columns)
            .map(|_| {
                let inst = meta.instance_column();
                meta.enable_equality(inst);
                inst
            })
            .collect();
        Self { base, instance }
    }

    /// Returns the inner [`FlexGateConfig`]
    pub fn gate(&self) -> &FlexGateConfig<F> {
        match &self.base {
            MaybeRangeConfig::WithoutRange(config) => config,
            MaybeRangeConfig::WithRange(config) => &config.gate,
        }
    }

    /// Returns the fixed columns for constants
    pub fn constants(&self) -> &Vec<Column<Fixed>> {
        match &self.base {
            MaybeRangeConfig::WithoutRange(config) => &config.constants,
            MaybeRangeConfig::WithRange(config) => &config.gate.constants,
        }
    }

    /// Returns a slice of the selector column to enable lookup -- this is only in the situation where there is a single advice column of any kind -- per phase
    /// Returns empty slice if there are no lookups enabled.
    pub fn q_lookup(&self) -> &[Option<Selector>] {
        match &self.base {
            MaybeRangeConfig::WithoutRange(_) => &[],
            MaybeRangeConfig::WithRange(config) => &config.q_lookup,
        }
    }

    /// Updates the number of usable rows in the circuit. Used if you mutate [ConstraintSystem] after `BaseConfig::configure` is called.
    pub fn set_usable_rows(&mut self, usable_rows: usize) {
        match &mut self.base {
            MaybeRangeConfig::WithoutRange(config) => config.max_rows = usable_rows,
            MaybeRangeConfig::WithRange(config) => config.gate.max_rows = usable_rows,
        }
    }

    /// Initialization of config at very beginning of `synthesize`.
    /// Loads fixed lookup table, if using.
    pub fn initialize(&self, layouter: &mut impl Layouter<F>) {
        // only load lookup table if we are actually doing lookups
        if let MaybeRangeConfig::WithRange(config) = &self.base {
            config.load_lookup_table(layouter).expect("load lookup table should not fail");
        }
    }
}

impl<F: ScalarField> Circuit<F> for BaseCircuitBuilder<F> {
    type Config = BaseConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.config_params.clone()
    }

    /// Creates a new instance of the [BaseCircuitBuilder] without witnesses by setting the witness_gen_only flag to false
    fn without_witnesses(&self) -> Self {
        unimplemented!()
    }

    /// Configures a new circuit using [`BaseCircuitParams`]
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        BaseConfig::configure(meta, params)
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("You must use configure_with_params");
    }

    /// Performs the actual computation on the circuit (e.g., witness generation), populating the lookup table and filling in all the advice values for a particular proof.
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // only load lookup table if we are actually doing lookups
        if let MaybeRangeConfig::WithRange(config) = &config.base {
            config.load_lookup_table(&mut layouter).expect("load lookup table should not fail");
        } else {
            assert!(
                self.lookup_manager.iter().all(|lookup_manager| lookup_manager.total_rows() == 0),
                "range lookups were queued but the circuit was configured without a RangeConfig"
            );
        }
        // Only FirstPhase (phase 0)
        layouter
            .assign_region(
                || "BaseCircuitBuilder generated circuit",
                |mut region| {
                    let usable_rows = config.gate().max_rows;
                    self.core.phase_manager[0].assign_raw(
                        &(config.gate().basic_gates[0].clone(), usable_rows),
                        &mut region,
                    );
                    // Only assign cells to lookup if we're sure we're doing range lookups
                    if let MaybeRangeConfig::WithRange(config) = &config.base {
                        self.assign_lookups_in_phase(config, &mut region, 0);
                    }
                    // Impose equality constraints
                    if !self.core.witness_gen_only() {
                        self.core.copy_manager.assign_raw(config.constants(), &mut region);
                    }
                    Ok(())
                },
            )
            .unwrap();

        self.assign_instances(&config.instance, layouter.namespace(|| "expose"));
        Ok(())
    }
}

/// Defines stage of circuit building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitBuilderStage {
    /// Keygen phase
    Keygen,
    /// Prover Circuit
    Prover,
    /// Mock Circuit
    Mock,
}

impl CircuitBuilderStage {
    /// Returns true if the circuit is used for witness generation only.
    pub fn witness_gen_only(&self) -> bool {
        matches!(self, CircuitBuilderStage::Prover)
    }
}

impl<F: ScalarField> AsRef<BaseConfig<F>> for BaseConfig<F> {
    fn as_ref(&self) -> &BaseConfig<F> {
        self
    }
}

impl<F: ScalarField> AsMut<BaseConfig<F>> for BaseConfig<F> {
    fn as_mut(&mut self) -> &mut BaseConfig<F> {
        self
    }
}
