//! The toy relation shared by the fold tests: a small circuit with every
//! relation feature (selectors, a fixed coefficient, a rotation, a degree-three
//! gate, a tuple lookup into a fixed table, two single-column lookups sharing one
//! table, a dynamic advice table, copies to an instance cell and to a constant).

#![allow(dead_code)]

pub mod qstep;
pub mod step;

use iroha_pasta::PastaField;
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Expression, Fixed, Instance, Rotation, Selector, TableColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
};

pub const K: u32 = 7;
pub const ROWS: usize = 24;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    d: Column<Advice>,
    e: Column<Advice>,
    cubes: Column<Advice>,
    coeff: Column<Fixed>,
    instance: Column<Instance>,
    mul: Selector,
    add: Selector,
    rot: Selector,
    cube: Selector,
    lookup: Selector,
    values: TableColumn,
    squares: TableColumn,
}

/// Row `i` holds `x_i, x_i^2, x_i^3`, `d_i = b_{i+1} - b_i`, `e_i = a_i + 5 b_i`.
#[derive(Clone, Copy, Debug)]
pub struct Toy {
    pub seed: u64,
}

impl Toy {
    pub fn x(self, row: usize) -> u64 {
        if row == 0 { 3 } else { (row as u64 * 5 + self.seed) % 15 }
    }
    pub fn instances<F: PastaField>(self) -> Vec<Vec<F>> {
        let last = self.x(ROWS - 1);
        vec![vec![F::from(self.x(0)), F::from(last * last * last)]]
    }
}

// Column names follow the gate equations (a, b, c, d, e).
#[allow(clippy::many_single_char_names)]
impl<F: PastaField> Circuit<F> for Toy {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Config {
        let [a, b, c, d, e, cubes] = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let coeff = meta.fixed_column();
        let instance = meta.instance_column(2);
        for column in [a, b, c] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let (mul, add, rot, cube) = (meta.selector(), meta.selector(), meta.selector(), meta.selector());
        let lookup = meta.complex_selector();
        let values = meta.lookup_table_column();
        let squares = meta.lookup_table_column();
        meta.create_gate("mul", |v| {
            let s = v.query_selector(mul);
            let (a, b, c) = (v.query_advice(a, Rotation::cur()), v.query_advice(b, Rotation::cur()), v.query_advice(c, Rotation::cur()));
            vec![s * (a * b - c)]
        });
        meta.create_gate("add", |v| {
            let s = v.query_selector(add);
            let k = v.query_fixed(coeff, Rotation::cur());
            let (a, b, e) = (v.query_advice(a, Rotation::cur()), v.query_advice(b, Rotation::cur()), v.query_advice(e, Rotation::cur()));
            vec![s * (a + k * b - e)]
        });
        meta.create_gate("rot", |v| {
            let s = v.query_selector(rot);
            let (b, next, d) = (v.query_advice(b, Rotation::cur()), v.query_advice(b, Rotation::next()), v.query_advice(d, Rotation::cur()));
            vec![s * (next - b - d)]
        });
        meta.create_gate("cube", |v| {
            let s = v.query_selector(cube);
            let (a, c) = (v.query_advice(a, Rotation::cur()), v.query_advice(c, Rotation::cur()));
            vec![s * (c - a.clone() * a.clone() * a)]
        });
        meta.lookup("pairs", |v| {
            let s = v.query_selector(lookup);
            let (a, b) = (v.query_advice(a, Rotation::cur()), v.query_advice(b, Rotation::cur()));
            vec![(s.clone() * a, values), (s * b, squares)]
        });
        meta.lookup_any("range a", |v| {
            let s = v.query_selector(lookup);
            let a = v.query_advice(a, Rotation::cur());
            vec![(s * (a + Expression::Constant(F::ONE)), v.query_fixed(values.inner(), Rotation::cur()))]
        });
        meta.lookup_any("range d", |v| {
            let s = v.query_selector(lookup);
            let a = v.query_advice(a, Rotation::cur());
            vec![(s * (a * Expression::Constant(F::ONE)), v.query_fixed(values.inner(), Rotation::cur()))]
        });
        meta.lookup_any("cubes", |v| {
            let s = v.query_selector(lookup);
            let c = v.query_advice(c, Rotation::cur());
            vec![(s * c, v.query_advice(cubes, Rotation::cur()))]
        });
        Config { a, b, c, d, e, cubes, coeff, instance, mul, add, rot, cube, lookup, values, squares }
    }

    fn synthesize(&self, config: Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        layouter.assign_table(
            || "squares",
            |mut table| {
                for i in 0..16u64 {
                    let row = usize::try_from(i).expect("small");
                    table.assign_cell(|| "i", config.values, row, || Value::known(F::from(i)))?;
                    table.assign_cell(|| "i^2", config.squares, row, || Value::known(F::from(i * i)))?;
                }
                Ok(())
            },
        )?;
        let last = layouter.assign_region(
            || "rows",
            |mut region| {
                let mut last = None;
                for row in 0..ROWS {
                    let x = self.x(row);
                    let (b, c) = (x * x, x * x * x);
                    config.mul.enable(&mut region, row)?;
                    config.add.enable(&mut region, row)?;
                    config.cube.enable(&mut region, row)?;
                    region.enable_selector(|| "lookup", &config.lookup, row)?;
                    region.assign_fixed(config.coeff, row, F::from(5))?;
                    if row == 0 {
                        region.assign_advice_from_instance(|| "x0", config.instance, 0, config.a, 0)?;
                        region.assign_advice_from_constant(|| "nine", config.b, 0, F::from(9))?;
                    } else {
                        region.assign_advice(config.a, row, Value::known(F::from(x)))?;
                        region.assign_advice(config.b, row, Value::known(F::from(b)))?;
                    }
                    last = Some(region.assign_advice(config.c, row, Value::known(F::from(c)))?.cell());
                    region.assign_advice(config.e, row, Value::known(F::from(x + 5 * b)))?;
                    if row + 1 < ROWS {
                        config.rot.enable(&mut region, row)?;
                        let next = self.x(row + 1);
                        let d = F::from(next * next) - F::from(b);
                        region.assign_advice(config.d, row, Value::known(d))?;
                    }
                }
                for i in 0..16u64 {
                    region.assign_advice(config.cubes, usize::try_from(i).expect("small"), Value::known(F::from(i * i * i)))?;
                }
                last.ok_or(Error::Synthesis)
            },
        )?;
        layouter.constrain_instance(last, config.instance, 1)
    }
}

