use std::{fmt, marker::PhantomData};

use ff::Field;

use crate::{
    circuit::{
        AssignedCell, Cell, Layouter, Region, RegionStart, Table, Value,
        layouter::{RegionColumn, RegionLayouter, RegionShape, SyncDeps, TableLayouter},
        table_layouter::{SimpleTableLayouter, compute_table_lengths},
    },
    plonk::{
        Advice, Any, Assigned, Assignment, Challenge, Circuit, Column, Error, Fixed, FloorPlanner,
        Instance, Selector, TableColumn,
    },
};

mod strategy;

/// The version 1 [`FloorPlanner`] provided by `halo2`.
///
/// - No column optimizations are performed. Circuit configuration is left entirely to the
///   circuit designer.
/// - A dual-pass layouter is used to measures regions prior to assignment.
/// - Regions are measured as rectangles, bounded on the cells they assign.
/// - Regions are laid out using a greedy first-fit strategy, after sorting regions by
///   their "advice area" (number of advice columns * rows).
#[derive(Debug)]
pub struct V1;

struct V1Plan<'a, F: Field, CS: Assignment<F> + 'a> {
    cs: &'a mut CS,
    /// Stores the starting row for each region.
    regions: Vec<RegionStart>,
    /// Stores the constants to be assigned, and the cells to which they are copied.
    constants: Vec<(Assigned<F>, Cell)>,
    /// Stores the table fixed columns.
    table_columns: Vec<TableColumn>,
}

impl<'a, F: Field, CS: Assignment<F> + 'a> fmt::Debug for V1Plan<'a, F, CS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("floor_planner::V1Plan").finish()
    }
}

impl<'a, F: Field, CS: Assignment<F> + SyncDeps> V1Plan<'a, F, CS> {
    /// Creates a new v1 layouter.
    pub fn new(cs: &'a mut CS) -> Result<Self, Error> {
        let ret = V1Plan {
            cs,
            regions: vec![],
            constants: vec![],
            table_columns: vec![],
        };
        Ok(ret)
    }
}

impl FloorPlanner for V1 {
    fn synthesize<F: Field, CS: Assignment<F> + SyncDeps, C: Circuit<F>>(
        cs: &mut CS,
        circuit: &C,
        config: C::Config,
        constants: Vec<Column<Fixed>>,
    ) -> Result<(), Error> {
        let mut plan = V1Plan::new(cs)?;

        // First pass: measure the regions within the circuit.
        let mut measure = MeasurementPass::new();
        {
            let pass = &mut measure;
            circuit.synthesize_for_measurement(config.clone(), V1Pass::<_, CS>::measure(pass))?;
        }

        // Planning:
        // - Position the regions.
        let (regions, column_allocations) = strategy::slot_in_biggest_advice_first(measure.regions);
        plan.regions = regions;

        // - Determine how many rows our planned circuit will require.
        let first_unassigned_row = column_allocations
            .values()
            .map(|a| a.unbounded_interval_start())
            .max()
            .unwrap_or(0);

        // - Position the constants within those rows.
        let fixed_allocations: Vec<_> = constants
            .into_iter()
            .map(|c| {
                (
                    c,
                    column_allocations
                        .get(&Column::<Any>::from(c).into())
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let constant_positions = || {
            fixed_allocations.iter().flat_map(|(c, a)| {
                let c = *c;
                a.free_intervals(0, Some(first_unassigned_row))
                    .flat_map(move |e| e.range().unwrap().map(move |i| (c, i)))
            })
        };

        // Second pass:
        // - Assign the regions.
        let mut assign = AssignmentPass::new(&mut plan);
        {
            let pass = &mut assign;
            circuit.synthesize(config, V1Pass::assign(pass))?;
        }

        // - Assign the constants.
        if constant_positions().count() < plan.constants.len() {
            return Err(Error::NotEnoughColumnsForConstants);
        }
        for ((fixed_column, fixed_row), (value, advice)) in
            constant_positions().zip(plan.constants.into_iter())
        {
            plan.cs.assign_fixed(fixed_column, fixed_row, value);
            plan.cs.copy(
                fixed_column.into(),
                fixed_row,
                advice.column,
                advice.row_offset,
            );
        }

        Ok(())
    }
}

#[derive(Debug)]
enum Pass<'p, 'a, F: Field, CS: Assignment<F> + 'a> {
    Measurement(&'p mut MeasurementPass),
    Assignment(&'p mut AssignmentPass<'p, 'a, F, CS>),
}

/// A single pass of the [`V1`] layouter.
#[derive(Debug)]
pub struct V1Pass<'p, 'a, F: Field, CS: Assignment<F> + 'a>(Pass<'p, 'a, F, CS>);

impl<'p, 'a, F: Field, CS: Assignment<F> + 'a> V1Pass<'p, 'a, F, CS> {
    fn measure(pass: &'p mut MeasurementPass) -> Self {
        V1Pass(Pass::Measurement(pass))
    }

    fn assign(pass: &'p mut AssignmentPass<'p, 'a, F, CS>) -> Self {
        V1Pass(Pass::Assignment(pass))
    }
}

impl<'p, 'a, F: Field, CS: Assignment<F> + SyncDeps> Layouter<F> for V1Pass<'p, 'a, F, CS> {
    type Root = Self;

    fn assign_region<A, AR, N, NR>(&mut self, name: N, assignment: A) -> Result<AR, Error>
    where
        A: FnOnce(Region<'_, F>) -> Result<AR, Error>,
        N: Fn() -> NR,
        NR: Into<String>,
    {
        match &mut self.0 {
            Pass::Measurement(pass) => pass.assign_region(assignment),
            Pass::Assignment(pass) => pass.assign_region(name, assignment),
        }
    }

    fn assign_table<A, N, NR>(&mut self, name: N, assignment: A) -> Result<(), Error>
    where
        A: FnMut(Table<'_, F>) -> Result<(), Error>,
        N: Fn() -> NR,
        NR: Into<String>,
    {
        match &mut self.0 {
            Pass::Measurement(_) => Ok(()),
            Pass::Assignment(pass) => pass.assign_table(name, assignment),
        }
    }

    fn constrain_instance(&mut self, cell: Cell, instance: Column<Instance>, row: usize) {
        match &mut self.0 {
            Pass::Measurement(_) => {}
            Pass::Assignment(pass) => pass.constrain_instance(cell, instance, row),
        }
    }

    fn next_phase(&mut self) {
        match &mut self.0 {
            Pass::Measurement(_) => {}
            Pass::Assignment(pass) => pass.plan.cs.next_phase(),
        }
    }

    fn get_challenge(&self, challenge: Challenge) -> Value<F> {
        match &self.0 {
            Pass::Measurement(_) => Value::unknown(),
            Pass::Assignment(pass) => pass.plan.cs.get_challenge(challenge),
        }
    }

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }

    fn push_namespace<NR, N>(&mut self, name_fn: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
        if let Pass::Assignment(pass) = &mut self.0 {
            pass.plan.cs.push_namespace(name_fn);
        }
    }

    fn pop_namespace(&mut self, gadget_name: Option<String>) {
        if let Pass::Assignment(pass) = &mut self.0 {
            pass.plan.cs.pop_namespace(gadget_name);
        }
    }
}

/// Measures the circuit.
#[derive(Debug)]
pub struct MeasurementPass {
    regions: Vec<RegionShape>,
}

impl MeasurementPass {
    fn new() -> Self {
        MeasurementPass { regions: vec![] }
    }

    fn assign_region<F: Field, A, AR>(&mut self, assignment: A) -> Result<AR, Error>
    where
        A: FnOnce(Region<'_, F>) -> Result<AR, Error>,
    {
        let region_index = self.regions.len();

        // Get shape of the region.
        let mut shape = RegionShape::new(region_index.into());
        let result = {
            let region: &mut dyn RegionLayouter<F> = &mut shape;
            assignment(region.into())
        }?;
        self.regions.push(shape);

        Ok(result)
    }
}

/// Assigns the circuit.
#[derive(Debug)]
pub struct AssignmentPass<'p, 'a, F: Field, CS: Assignment<F> + 'a> {
    plan: &'p mut V1Plan<'a, F, CS>,
    /// Counter tracking which region we need to assign next.
    region_index: usize,
}

impl<'p, 'a, F: Field, CS: Assignment<F> + SyncDeps> AssignmentPass<'p, 'a, F, CS> {
    fn new(plan: &'p mut V1Plan<'a, F, CS>) -> Self {
        AssignmentPass {
            plan,
            region_index: 0,
        }
    }

    fn assign_region<A, AR, N, NR>(&mut self, name: N, assignment: A) -> Result<AR, Error>
    where
        A: FnOnce(Region<'_, F>) -> Result<AR, Error>,
        N: Fn() -> NR,
        NR: Into<String>,
    {
        // Get the next region we are assigning.
        let region_index = self.region_index;
        self.region_index += 1;

        self.plan.cs.enter_region(name);
        let mut region = V1Region::new(self.plan, region_index);
        let result = {
            let region: &mut dyn RegionLayouter<F> = &mut region;
            assignment(region.into())
        }?;
        self.plan.cs.exit_region();

        Ok(result)
    }

    fn assign_table<A, AR, N, NR>(&mut self, name: N, mut assignment: A) -> Result<AR, Error>
    where
        A: FnMut(Table<'_, F>) -> Result<AR, Error>,
        N: Fn() -> NR,
        NR: Into<String>,
    {
        // Maintenance hazard: there is near-duplicate code in `SingleChipLayouter::assign_table`.

        // Assign table cells.
        self.plan.cs.enter_region(name);
        let mut table = SimpleTableLayouter::new(self.plan.cs, &self.plan.table_columns);
        let result = {
            let table: &mut dyn TableLayouter<F> = &mut table;
            assignment(table.into())
        }?;
        let default_and_assigned = table.default_and_assigned;
        self.plan.cs.exit_region();

        // Check that all table columns have the same length `first_unused`,
        // and all cells up to that length are assigned.
        let first_unused = compute_table_lengths(&default_and_assigned)?;

        // Record these columns so that we can prevent them from being used again.
        for column in default_and_assigned.keys() {
            self.plan.table_columns.push(*column);
        }

        for (col, (default_val, _)) in default_and_assigned {
            // default_val must be Some because we must have assigned
            // at least one cell in each column, and in that case we checked
            // that all cells up to first_unused were assigned.
            self.plan
                .cs
                .fill_from_row(col.inner(), first_unused, default_val.unwrap())?;
        }

        Ok(result)
    }

    fn constrain_instance(&mut self, cell: Cell, instance: Column<Instance>, row: usize) {
        self.plan
            .cs
            .copy(cell.column, cell.row_offset, instance.into(), row);
    }
}

struct V1Region<'r, 'a, F: Field, CS: Assignment<F> + 'a> {
    plan: &'r mut V1Plan<'a, F, CS>,
    region_start: RegionStart,
}

impl<'r, 'a, F: Field, CS: Assignment<F> + 'a> fmt::Debug for V1Region<'r, 'a, F, CS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("V1Region")
            .field("plan", &self.plan)
            .field("region_start", &self.region_start)
            .finish()
    }
}

impl<'r, 'a, F: Field, CS: Assignment<F> + 'a> V1Region<'r, 'a, F, CS> {
    fn new(plan: &'r mut V1Plan<'a, F, CS>, region_index: usize) -> Self {
        V1Region {
            region_start: plan.regions[region_index],
            plan,
        }
    }
}

impl<'r, 'a, F: Field, CS: Assignment<F> + SyncDeps> RegionLayouter<F> for V1Region<'r, 'a, F, CS> {
    fn enable_selector<'v>(
        &'v mut self,
        annotation: &'v (dyn Fn() -> String + 'v),
        selector: &Selector,
        offset: usize,
    ) -> Result<(), Error> {
        self.plan
            .cs
            .enable_selector(annotation, selector, *self.region_start + offset)
    }

    fn assign_advice<'v>(
        &mut self,
        column: Column<Advice>,
        offset: usize,
        to: Value<Assigned<F>>,
    ) -> AssignedCell<&'v Assigned<F>, F> {
        let row_offset = *self.region_start + offset;
        let value = self.plan.cs.assign_advice(column, row_offset, to);

        AssignedCell {
            value,
            cell: Cell {
                row_offset,
                column: column.into(),
            },
            _marker: PhantomData,
        }
    }

    fn assign_advice_from_constant<'v>(
        &'v mut self,
        _annotation: &'v (dyn Fn() -> String + 'v),
        column: Column<Advice>,
        offset: usize,
        constant: Assigned<F>,
    ) -> Result<Cell, Error> {
        let advice = self
            .assign_advice(column, offset, Value::known(constant))
            .cell();
        self.constrain_constant(advice, constant)?;

        Ok(advice)
    }

    fn assign_advice_from_instance<'v>(
        &mut self,
        _annotation: &'v (dyn Fn() -> String + 'v),
        instance: Column<Instance>,
        row: usize,
        advice: Column<Advice>,
        offset: usize,
    ) -> Result<(Cell, Value<F>), Error> {
        let value = self.plan.cs.query_instance(instance, row)?;

        let cell = self
            .assign_advice(advice, offset, value.map(Assigned::Trivial))
            .cell();

        self.plan
            .cs
            .copy(cell.column, cell.row_offset, instance.into(), row);

        Ok((cell, value))
    }

    fn instance_value(
        &mut self,
        instance: Column<Instance>,
        row: usize,
    ) -> Result<Value<F>, Error> {
        self.plan.cs.query_instance(instance, row)
    }

    fn assign_fixed(&mut self, column: Column<Fixed>, offset: usize, to: Assigned<F>) -> Cell {
        let row_offset = *self.region_start + offset;
        self.plan.cs.assign_fixed(column, row_offset, to);

        Cell {
            row_offset,
            column: column.into(),
        }
    }

    fn constrain_constant(&mut self, cell: Cell, constant: Assigned<F>) -> Result<(), Error> {
        self.plan.constants.push((constant, cell));
        Ok(())
    }

    fn name_column<'v>(
        &'v mut self,
        annotation: &'v (dyn Fn() -> String + 'v),
        column: Column<Any>,
    ) {
        self.plan.cs.annotate_column(annotation, column)
    }

    fn constrain_equal(&mut self, left: Cell, right: Cell) {
        self.plan
            .cs
            .copy(left.column, left.row_offset, right.column, right.row_offset);
    }

    fn get_challenge(&self, challenge: Challenge) -> Value<F> {
        self.plan.cs.get_challenge(challenge)
    }

    fn next_phase(&mut self) {
        self.plan.cs.next_phase()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use halo2curves::pasta::vesta;

    use crate::{
        circuit::{Layouter, Value},
        dev::MockProver,
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Selector},
        poly::Rotation,
    };

    #[test]
    fn not_enough_columns_for_constants() {
        struct MyCircuit {}

        impl Circuit<vesta::Scalar> for MyCircuit {
            type Config = Column<Advice>;
            type FloorPlanner = super::V1;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                MyCircuit {}
            }

            fn configure(meta: &mut crate::plonk::ConstraintSystem<vesta::Scalar>) -> Self::Config {
                meta.advice_column()
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl crate::circuit::Layouter<vesta::Scalar>,
            ) -> Result<(), crate::plonk::Error> {
                layouter.assign_region(
                    || "assign constant",
                    |mut region| {
                        region.assign_advice_from_constant(
                            || "one",
                            config,
                            0,
                            vesta::Scalar::from(1),
                        )
                    },
                )?;

                Ok(())
            }
        }

        let circuit = MyCircuit {};
        assert!(matches!(
            MockProver::run(3, &circuit, vec![]).unwrap_err(),
            Error::NotEnoughColumnsForConstants,
        ));
    }

    #[test]
    fn default_measurement_hook_still_uses_without_witnesses() {
        struct MeasurementCircuit {
            witnessless: bool,
            without_calls: Arc<AtomicUsize>,
            witnessless_synthesis: Arc<AtomicUsize>,
            witnessed_synthesis: Arc<AtomicUsize>,
        }

        impl Circuit<vesta::Scalar> for MeasurementCircuit {
            type Config = Column<Advice>;
            type FloorPlanner = super::V1;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                self.without_calls.fetch_add(1, Ordering::SeqCst);
                Self {
                    witnessless: true,
                    without_calls: Arc::clone(&self.without_calls),
                    witnessless_synthesis: Arc::clone(&self.witnessless_synthesis),
                    witnessed_synthesis: Arc::clone(&self.witnessed_synthesis),
                }
            }

            fn configure(meta: &mut ConstraintSystem<vesta::Scalar>) -> Self::Config {
                meta.advice_column()
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<vesta::Scalar>,
            ) -> Result<(), Error> {
                if self.witnessless {
                    self.witnessless_synthesis.fetch_add(1, Ordering::SeqCst);
                } else {
                    self.witnessed_synthesis.fetch_add(1, Ordering::SeqCst);
                }
                layouter.assign_region(
                    || "measurement hook",
                    |mut region| {
                        region.assign_advice(config, 0, Value::known(vesta::Scalar::from(1)));
                        Ok(())
                    },
                )
            }
        }

        let without_calls = Arc::new(AtomicUsize::new(0));
        let witnessless_synthesis = Arc::new(AtomicUsize::new(0));
        let witnessed_synthesis = Arc::new(AtomicUsize::new(0));
        let circuit = MeasurementCircuit {
            witnessless: false,
            without_calls: Arc::clone(&without_calls),
            witnessless_synthesis: Arc::clone(&witnessless_synthesis),
            witnessed_synthesis: Arc::clone(&witnessed_synthesis),
        };

        MockProver::run(3, &circuit, vec![]).expect("measurement hook circuit should synthesize");
        assert_eq!(without_calls.load(Ordering::SeqCst), 1);
        assert_eq!(witnessless_synthesis.load(Ordering::SeqCst), 1);
        assert_eq!(witnessed_synthesis.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn custom_measurement_hook_resets_on_success_and_error() {
        struct ReusableMeasurementCircuit {
            fail_measurement: bool,
            without_calls: Arc<AtomicUsize>,
            resets: Arc<AtomicUsize>,
            assignments: Arc<AtomicUsize>,
        }

        impl Circuit<vesta::Scalar> for ReusableMeasurementCircuit {
            type Config = Column<Advice>;
            type FloorPlanner = super::V1;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                self.without_calls.fetch_add(1, Ordering::SeqCst);
                Self {
                    fail_measurement: self.fail_measurement,
                    without_calls: Arc::clone(&self.without_calls),
                    resets: Arc::clone(&self.resets),
                    assignments: Arc::clone(&self.assignments),
                }
            }

            fn configure(meta: &mut ConstraintSystem<vesta::Scalar>) -> Self::Config {
                meta.advice_column()
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<vesta::Scalar>,
            ) -> Result<(), Error> {
                let result = if self.fail_measurement {
                    Err(Error::Synthesis)
                } else {
                    self.synthesize(config, layouter)
                };
                self.resets.fetch_add(1, Ordering::SeqCst);
                result
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<vesta::Scalar>,
            ) -> Result<(), Error> {
                self.assignments.fetch_add(1, Ordering::SeqCst);
                layouter.assign_region(
                    || "reusable measurement hook",
                    |mut region| {
                        region.assign_advice(config, 0, Value::known(vesta::Scalar::from(1)));
                        Ok(())
                    },
                )
            }
        }

        let run = |fail_measurement| {
            let without_calls = Arc::new(AtomicUsize::new(0));
            let resets = Arc::new(AtomicUsize::new(0));
            let assignments = Arc::new(AtomicUsize::new(0));
            let circuit = ReusableMeasurementCircuit {
                fail_measurement,
                without_calls: Arc::clone(&without_calls),
                resets: Arc::clone(&resets),
                assignments: Arc::clone(&assignments),
            };
            let result = MockProver::run(3, &circuit, vec![]);
            (result, without_calls, resets, assignments)
        };

        let (result, without_calls, resets, assignments) = run(false);
        result.expect("reusable measurement circuit should synthesize");
        assert_eq!(without_calls.load(Ordering::SeqCst), 0);
        assert_eq!(resets.load(Ordering::SeqCst), 1);
        assert_eq!(assignments.load(Ordering::SeqCst), 2);

        let (result, without_calls, resets, assignments) = run(true);
        assert!(matches!(result, Err(Error::Synthesis)));
        assert_eq!(without_calls.load(Ordering::SeqCst), 0);
        assert_eq!(resets.load(Ordering::SeqCst), 1);
        assert_eq!(assignments.load(Ordering::SeqCst), 0);
    }

    #[derive(Clone)]
    struct RegionSeparationConfig {
        shared: Column<Advice>,
        copied: Column<Advice>,
        first: Selector,
        second: Selector,
    }

    struct RegionSeparationCircuit {
        corrupt_copy: bool,
    }

    impl Circuit<vesta::Scalar> for RegionSeparationCircuit {
        type Config = RegionSeparationConfig;
        type FloorPlanner = super::V1;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                corrupt_copy: self.corrupt_copy,
            }
        }

        fn configure(meta: &mut ConstraintSystem<vesta::Scalar>) -> Self::Config {
            let shared = meta.advice_column();
            let copied = meta.advice_column();
            let first = meta.selector();
            let second = meta.selector();
            meta.enable_equality(shared);
            meta.enable_equality(copied);

            meta.create_gate("first region value", |meta| {
                let q = meta.query_selector(first);
                let value = meta.query_advice(shared, Rotation::cur());
                vec![q * (value - Expression::Constant(vesta::Scalar::from(1)))]
            });
            meta.create_gate("second region value", |meta| {
                let q = meta.query_selector(second);
                let value = meta.query_advice(shared, Rotation::cur());
                vec![q * (value - Expression::Constant(vesta::Scalar::from(2)))]
            });

            RegionSeparationConfig {
                shared,
                copied,
                first,
                second,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<vesta::Scalar>,
        ) -> Result<(), Error> {
            let first = layouter.assign_region(
                || "first region",
                |mut region| {
                    config.first.enable(&mut region, 0)?;
                    Ok(region
                        .assign_advice(config.shared, 0, Value::known(vesta::Scalar::from(1)))
                        .cell())
                },
            )?;

            layouter.assign_region(
                || "second region",
                |mut region| {
                    config.second.enable(&mut region, 0)?;
                    region.assign_advice(config.shared, 0, Value::known(vesta::Scalar::from(2)));
                    let copied = region.assign_advice(
                        config.copied,
                        0,
                        Value::known(vesta::Scalar::from(if self.corrupt_copy { 2 } else { 1 })),
                    );
                    region.constrain_equal(first, copied.cell());
                    Ok(())
                },
            )
        }
    }

    struct ReusableRegionSeparationCircuit(RegionSeparationCircuit);

    impl Circuit<vesta::Scalar> for ReusableRegionSeparationCircuit {
        type Config = RegionSeparationConfig;
        type FloorPlanner = super::V1;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self(self.0.without_witnesses())
        }

        fn configure(meta: &mut ConstraintSystem<vesta::Scalar>) -> Self::Config {
            RegionSeparationCircuit::configure(meta)
        }

        fn synthesize_for_measurement(
            &self,
            config: Self::Config,
            layouter: impl Layouter<vesta::Scalar>,
        ) -> Result<(), Error> {
            self.synthesize(config, layouter)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<vesta::Scalar>,
        ) -> Result<(), Error> {
            self.0.synthesize(config, layouter)
        }
    }

    #[test]
    fn regions_are_disjoint_and_cross_region_equality_is_enforced() {
        let valid = RegionSeparationCircuit {
            corrupt_copy: false,
        };
        MockProver::run(4, &valid, vec![])
            .expect("planner should synthesize two regions")
            .assert_satisfied();

        let corrupt = RegionSeparationCircuit { corrupt_copy: true };
        assert!(
            MockProver::run(4, &corrupt, vec![])
                .expect("planner should synthesize the corrupt circuit")
                .verify()
                .is_err(),
            "cross-region equality must reject a mismatched copy"
        );
    }

    #[test]
    fn reusable_measurement_preserves_v1_verifying_key_layout() {
        use crate::{
            SerdeFormat,
            plonk::keygen_vk,
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };

        let params = ParamsIPA::<vesta::Affine>::new(4);
        let default = RegionSeparationCircuit {
            corrupt_copy: false,
        };
        let reusable = ReusableRegionSeparationCircuit(RegionSeparationCircuit {
            corrupt_copy: false,
        });
        let default_vk = keygen_vk(&params, &default).expect("default V1 VK");
        let reusable_vk = keygen_vk(&params, &reusable).expect("reusable V1 VK");
        assert_eq!(
            reusable_vk.to_bytes(SerdeFormat::Processed),
            default_vk.to_bytes(SerdeFormat::Processed),
            "reusing the measurement graph must preserve V1 region placement"
        );
    }
}
