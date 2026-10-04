//! Accept and reject suites for the constraint checker, driven through the
//! frontend (layouter, assembly) so both are exercised end to end.

use ff::Field;
use iroha_pasta::Fp;
use rand_chacha::ChaCha20Rng;
use rand_core_06::{RngCore, SeedableRng};

use super::*;
use crate::{
    cs::{Advice, Fixed, Instance, TableColumn},
    frontend::{
        Assembly, Assigned, Layouter, SimpleFloorPlanner, SingleChipLayouter, TableError, Value,
    },
};

/// Runs `body` on a fresh witness assembly for `cs` and returns its tables.
fn run(
    cs: &ConstraintSystem<Fp>,
    k: u32,
    instances: &[Vec<Fp>],
    body: impl FnOnce(&mut SingleChipLayouter<'_, Fp, Assembly<Fp>>) -> Result<(), Error>,
) -> Result<AssignedTables<Fp>, Error> {
    let mut assembly = Assembly::new(cs, k, Some(instances))?;
    {
        let mut layouter = SingleChipLayouter::new(&mut assembly, cs.constants().to_vec());
        body(&mut layouter)?;
    }
    assembly.finish()
}

/// Columns of the multiplication chain.
#[derive(Clone, Copy, Debug)]
struct MulConfig {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    s: Selector,
    q: Selector,
    constants: Column<Fixed>,
    instance: Column<Instance>,
    table: TableColumn,
}

/// `s * (a * b - c)`, a lookup `q * a` into `0..8`, copies `c[i] = a[i+1]`, a
/// constant `a[0] = 2` and the public output `c[last]`.
fn mul_cs() -> (ConstraintSystem<Fp>, MulConfig) {
    let mut cs = ConstraintSystem::<Fp>::new();
    let lhs = cs.advice_column();
    let rhs = cs.advice_column();
    let out = cs.advice_column();
    let constants = cs.fixed_column();
    cs.enable_constant(constants);
    let instance = cs.instance_column(1);
    for column in [lhs, rhs, out] {
        cs.enable_equality(column);
    }
    cs.enable_equality(instance);
    let mul = cs.selector();
    cs.create_gate("mul", |meta| {
        let enabled = meta.query_selector(mul);
        let left = meta.query_advice(lhs, Rotation::cur());
        let right = meta.query_advice(rhs, Rotation::cur());
        let product = meta.query_advice(out, Rotation::cur());
        vec![("a*b=c", enabled * (left * right - product))]
    });
    let range = cs.complex_selector();
    let table = cs.lookup_table_column();
    cs.lookup("range", |meta| {
        let enabled = meta.query_selector(range);
        let left = meta.query_advice(lhs, Rotation::cur());
        vec![(enabled * left, table)]
    });
    (
        cs,
        MulConfig {
            a: lhs,
            b: rhs,
            c: out,
            s: mul,
            q: range,
            constants,
            instance,
            table,
        },
    )
}

/// Loads the table `0..8`.
fn load_table(layouter: &mut impl Layouter<Fp>, table: TableColumn) -> Result<(), Error> {
    layouter.assign_table(
        || "range",
        |mut t| {
            for value in 0..8_u64 {
                let offset = usize::try_from(value).map_err(|_| Error::Synthesis)?;
                t.assign_cell(|| "v", table, offset, || Value::known(Fp::from(value)))?;
            }
            Ok(())
        },
    )
}

/// Witness for the chain: `a[0] = 2`, `b[i] = 1 + (i mod 3)`.
fn chain(rows: usize) -> Vec<[Fp; 3]> {
    let mut a = Fp::from(2);
    (0..rows)
        .map(|i| {
            let b = Fp::from(1 + (i as u64 % 3));
            let row = [a, b, a * b];
            a = row[2];
            row
        })
        .collect()
}

/// Lays the chain out; `edit` may change the witness row values first.
fn lay_chain(
    layouter: &mut SingleChipLayouter<'_, Fp, Assembly<Fp>>,
    config: MulConfig,
    witness: &[[Fp; 3]],
) -> Result<(), Error> {
    load_table(layouter, config.table)?;
    let last = layouter.assign_region(
        || "chain",
        |mut region| {
            let mut previous_c = None;
            let mut last = None;
            for (row, values) in witness.iter().enumerate() {
                config.s.enable(&mut region, row)?;
                if row < 2 {
                    region.enable_selector(|| "q", &config.q, row)?;
                }
                let a = if row == 0 {
                    region
                        .assign_advice_from_constant(|| "two", config.a, 0, Fp::from(2))?
                        .cell()
                } else {
                    region
                        .assign_advice(config.a, row, Value::known(values[0]))?
                        .cell()
                };
                if let Some(previous) = previous_c {
                    region.constrain_equal(previous, a)?;
                }
                region.assign_advice(config.b, row, Value::known(values[1]))?;
                let c = region.assign_advice(config.c, row, Value::known(values[2]))?;
                previous_c = Some(c.cell());
                last = Some(c.cell());
            }
            last.ok_or(Error::Synthesis)
        },
    )?;
    layouter.constrain_instance(last, config.instance, 0)
}

fn mul_tables(
    witness: &[[Fp; 3]],
    public: Fp,
) -> Result<(ConstraintSystem<Fp>, AssignedTables<Fp>), Error> {
    let (cs, config) = mul_cs();
    let tables = run(&cs, 5, &[vec![public]], |layouter| {
        lay_chain(layouter, config, witness)
    })?;
    Ok((cs, tables))
}

#[test]
fn honest_chain_is_satisfied_in_both_modes() {
    let witness = chain(6);
    let (cs, tables) = mul_tables(&witness, witness[5][2]).expect("synthesis");
    for mode in [CheckMode::Strict, CheckMode::Halo2Compatible] {
        let report = check(&cs, &tables, mode).expect("check");
        assert!(report.is_satisfied(), "{report}");
        assert_eq!(report.to_string(), "all constraints are satisfied");
        assert_eq!(report.into_result(), Ok(()));
    }
    // The constant 2 sits in the constants column at row 0 and is copied.
    assert_eq!(tables.fixed()[0][0], Fp::from(2));
    assert!(tables.permutation().is_copied(0, 0));
}

#[test]
fn wrong_product_is_reported_with_cells_and_region() {
    let mut witness = chain(6);
    witness[3][2] += Fp::ONE;
    let (cs, tables) = mul_tables(&witness, witness[5][2]).expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    let failures = report.failures();
    let CheckFailure::ConstraintNotSatisfied {
        gate,
        constraint_name,
        location,
        value,
        cells,
        ..
    } = &failures[0]
    else {
        panic!("expected a gate failure, got {failures:?}");
    };
    assert_eq!((*gate, constraint_name.as_str()), (0, "a*b=c"));
    assert_eq!(location.row, 3);
    assert_eq!(
        location.region.as_ref().map(|r| r.1.as_str()),
        Some("chain")
    );
    assert_eq!(*value, -Fp::ONE);
    assert_eq!(cells.len(), 3);
    assert!(
        cells
            .iter()
            .all(|cell| cell.issue.is_none() && cell.row == 3)
    );
    // The tampered c[3] no longer equals a[4].
    assert!(failures.iter().any(|f| matches!(
        f,
        CheckFailure::CopyMismatch { left_row: 3, .. }
            | CheckFailure::CopyMismatch { right_row: 3, .. }
    )));
    let text = report.to_string();
    assert!(text.contains("gate 0 `mul` constraint 0 `a*b=c`"), "{text}");
    assert!(text.contains("A2[+0]@3"), "{text}");
    assert!(text.contains("region 1 `chain`"), "{text}");
}

#[test]
fn wrong_public_input_breaks_the_instance_copy() {
    let witness = chain(4);
    let (cs, tables) = mul_tables(&witness, Fp::from(5)).expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    assert_eq!(report.failures().len(), 2, "{report}");
    assert!(
        report
            .failures()
            .iter()
            .all(|f| matches!(f, CheckFailure::CopyMismatch { .. }))
    );
    assert!(report.to_string().contains("I0@0"));
}

#[test]
fn lookup_input_outside_the_table() {
    let mut witness = chain(4);
    // Row 1 is looked up; 9 is outside 0..8. Keep the product consistent.
    witness[1][0] = Fp::from(9);
    witness[1][2] = witness[1][0] * witness[1][1];
    witness[0][2] = Fp::from(9);
    witness[0][1] = Fp::from(9) * Fp::from(2).invert().unwrap();
    let (cs, tables) = mul_tables(&witness, witness[3][2]).expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    let missing: Vec<_> = report
        .failures()
        .iter()
        .filter_map(|f| match f {
            CheckFailure::LookupInputMissing {
                location, input, ..
            } => Some((location.row, input.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(missing, vec![(1, vec![Fp::from(9)])], "{report}");
}

#[test]
fn unassigned_advice_is_poison_only_in_strict_mode() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    let b = cs.advice_column();
    let s = cs.selector();
    cs.create_gate("a*b", |meta| {
        let s = meta.query_selector(s);
        let a = meta.query_advice(a, Rotation::cur());
        let b = meta.query_advice(b, Rotation::cur());
        vec![s * a * b]
    });
    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "half",
            |mut region| {
                s.enable(&mut region, 2)?;
                region.assign_advice(a, 2, Value::known(Fp::ZERO))?;
                Ok(())
            },
        )
    })
    .expect("synthesis");
    let strict = check(&cs, &tables, CheckMode::Strict).expect("check");
    // a = 0 makes the product zero whatever b is: a disabled read is fine.
    assert!(strict.is_satisfied(), "{strict}");

    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "half",
            |mut region| {
                s.enable(&mut region, 2)?;
                region.assign_advice(a, 2, Value::known(Fp::ONE))?;
                Ok(())
            },
        )
    })
    .expect("synthesis");
    let strict = check(&cs, &tables, CheckMode::Strict).expect("check");
    let [
        CheckFailure::ConstraintPoisoned {
            location, cells, ..
        },
    ] = strict.failures()
    else {
        panic!("expected one poisoned gate: {strict}");
    };
    assert_eq!(location.row, 2);
    assert_eq!(cells[1].issue, Some(CellIssue::Unassigned));
    assert!(strict.to_string().contains("A1[+0]@2 Unassigned"));
    let compatible = check(&cs, &tables, CheckMode::Halo2Compatible).expect("check");
    assert!(
        compatible.is_satisfied(),
        "MockProver reads unassigned advice as zero"
    );
}

#[test]
fn wraparound_and_blinding_rows_are_poison() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    let s = cs.selector();
    cs.create_gate("step", |meta| {
        let s = meta.query_selector(s);
        let prev = meta.query_advice(a, Rotation::prev());
        let next = meta.query_advice(a, Rotation::next());
        vec![s * (next - prev)]
    });
    // k = 4: n = 16, b = 5, usable rows 0..10.
    let assign_all = |layouter: &mut SingleChipLayouter<'_, Fp, Assembly<Fp>>, enabled: usize| {
        layouter.assign_region(
            || "all",
            |mut region| {
                for row in 0..10 {
                    region.assign_advice(a, row, Value::known(Fp::ONE))?;
                }
                s.enable(&mut region, enabled)
            },
        )
    };
    let tables = run(&cs, 4, &[], |l| assign_all(l, 0)).expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Halo2Compatible).expect("check");
    let [CheckFailure::ConstraintPoisoned { cells, .. }] = report.failures() else {
        panic!("expected a wrapped read: {report}");
    };
    let poison: Vec<_> = cells.iter().filter(|cell| cell.issue.is_some()).collect();
    assert_eq!(poison.len(), 1);
    assert_eq!(
        (poison[0].issue, poison[0].row, poison[0].rotation),
        (Some(CellIssue::Wrapped), 15, Rotation::prev())
    );

    let tables = run(&cs, 4, &[], |l| assign_all(l, 9)).expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Halo2Compatible).expect("check");
    let [CheckFailure::ConstraintPoisoned { cells, .. }] = report.failures() else {
        panic!("expected a blinding-row read: {report}");
    };
    let poison: Vec<_> = cells.iter().filter(|cell| cell.issue.is_some()).collect();
    assert_eq!(poison.len(), 1);
    assert_eq!(
        (poison[0].issue, poison[0].row, poison[0].rotation),
        (Some(CellIssue::BeyondUsableRows), 10, Rotation::next())
    );

    let tables = run(&cs, 4, &[], |l| assign_all(l, 5)).expect("synthesis");
    assert!(
        check(&cs, &tables, CheckMode::Strict)
            .expect("check")
            .is_satisfied()
    );
}

#[test]
fn ungated_constraints_fail_on_blinding_rows() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    cs.create_gate("always", |meta| vec![meta.query_advice(a, Rotation::cur())]);
    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "zeros",
            |mut region| {
                for row in 0..10 {
                    region.assign_advice(a, row, Value::known(Fp::ZERO))?;
                }
                Ok(())
            },
        )
    })
    .expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    let rows: Vec<usize> = report
        .failures()
        .iter()
        .map(|f| match f {
            CheckFailure::ConstraintPoisoned { location, .. } => location.row,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(rows, (10..16).collect::<Vec<_>>());
}

#[test]
fn lookup_poison_on_both_sides() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let input = cs.advice_column();
    let table = cs.advice_column();
    cs.lookup_any("advice table", |meta| {
        let input = meta.query_advice(input, Rotation::cur());
        let table = meta.query_advice(table, Rotation::cur());
        vec![(input, table)]
    });
    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "partial",
            |mut region| {
                for row in 0..9 {
                    region.assign_advice(input, row, Value::known(Fp::ZERO))?;
                    region.assign_advice(table, row, Value::known(Fp::ZERO))?;
                }
                Ok(())
            },
        )
    })
    .expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    let kinds: Vec<(bool, usize)> = report
        .failures()
        .iter()
        .map(|f| match f {
            CheckFailure::LookupPoisoned {
                table, location, ..
            } => (*table, location.row),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(kinds, vec![(true, 9), (false, 9)]);
    assert!(
        report
            .to_string()
            .contains("table depends on a poison cell")
    );
    let compatible = check(&cs, &tables, CheckMode::Halo2Compatible).expect("check");
    assert!(compatible.is_satisfied(), "{compatible}");
}

#[test]
fn unassigned_copied_cells_are_reported_in_strict_mode() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    let b = cs.advice_column();
    cs.enable_equality(a);
    cs.enable_equality(b);
    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "copy",
            |mut region| {
                let cell = region.assign_advice(a, 0, Value::known(Fp::ZERO))?;
                region.constrain_equal(
                    cell.cell(),
                    crate::frontend::Cell {
                        row_offset: 1,
                        column: b.into(),
                    },
                )
            },
        )
    })
    .expect("synthesis");
    let report = check(&cs, &tables, CheckMode::Strict).expect("check");
    assert_eq!(
        report.failures(),
        &[CheckFailure::CopyUnassigned {
            column: b.into(),
            row: 1
        }]
    );
    assert!(report.to_string().contains("A1@1 was never assigned"));
    assert!(
        check(&cs, &tables, CheckMode::Halo2Compatible)
            .expect("check")
            .is_satisfied()
    );
}

#[test]
fn constants_need_a_constants_column() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    cs.enable_equality(a);
    let error = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "constant",
            |mut region| {
                region.assign_advice_from_constant(|| "one", a, 0, Fp::ONE)?;
                Ok(())
            },
        )
    })
    .unwrap_err();
    assert_eq!(error, Error::NotEnoughColumnsForConstants);
}

#[test]
fn constants_fill_the_first_constants_column_across_regions() {
    let (cs, config) = mul_cs();
    let tables = run(&cs, 5, &[vec![Fp::ZERO]], |layouter| {
        for (region, value) in [3_u64, 4, 5].into_iter().enumerate() {
            layouter.assign_region(
                || "constant",
                |mut region_handle| {
                    region_handle.assign_advice_from_constant(
                        || "c",
                        config.b,
                        region,
                        Fp::from(value),
                    )?;
                    Ok(())
                },
            )?;
        }
        Ok(())
    })
    .expect("synthesis");
    assert_eq!(
        tables.fixed()[config.constants.index()][..3],
        [Fp::from(3), Fp::from(4), Fp::from(5)]
    );
    // Each constant row is copied to its advice cell (constants column first).
    let constants_position = cs
        .permutation()
        .position(config.constants.into())
        .expect("equality");
    let b_position = cs
        .permutation()
        .position(config.b.into())
        .expect("equality");
    for row in 0..3 {
        assert_eq!(
            tables.permutation().mapping(constants_position, row),
            Some((b_position, row))
        );
    }
}

#[test]
fn table_errors() {
    let (cs, config) = mul_cs();
    let other = TableColumn::new(config.constants);
    let attempt = |body: &dyn Fn(&mut crate::frontend::Table<'_, Fp>) -> Result<(), Error>| {
        run(&cs, 5, &[vec![Fp::ZERO]], |layouter| {
            layouter.assign_table(|| "t", |mut table| body(&mut table))
        })
    };
    let gap = attempt(&|table| {
        table.assign_cell(|| "", config.table, 0, || Value::known(Fp::ONE))?;
        table.assign_cell(|| "", config.table, 2, || Value::known(Fp::ONE))
    });
    assert_eq!(
        gap.unwrap_err(),
        Error::Table(TableError::ColumnNotAssigned(config.table))
    );
    let overwrite = attempt(&|table| {
        table.assign_cell(|| "", config.table, 0, || Value::known(Fp::ONE))?;
        table.assign_cell(|| "", config.table, 0, || Value::known(Fp::ONE))
    });
    assert_eq!(
        overwrite.unwrap_err(),
        Error::Table(TableError::OverwriteDefault(config.table))
    );
    let uneven = attempt(&|table| {
        table.assign_cell(|| "", config.table, 0, || Value::known(Fp::ONE))?;
        table.assign_cell(|| "", other, 0, || Value::known(Fp::ONE))?;
        table.assign_cell(|| "", other, 1, || Value::known(Fp::ONE))
    });
    assert!(matches!(
        uneven.unwrap_err(),
        Error::Table(TableError::UnevenColumnLengths { .. })
    ));
    let unknown = attempt(&|table| table.assign_cell(|| "", config.table, 0, Value::<Fp>::unknown));
    assert_eq!(unknown.unwrap_err(), Error::Synthesis);
    let reused = run(&cs, 5, &[vec![Fp::ZERO]], |layouter| {
        load_table(layouter, config.table)?;
        load_table(layouter, config.table)
    });
    assert_eq!(
        reused.unwrap_err(),
        Error::Table(TableError::UsedColumn(config.table))
    );
    // A loaded table fills the remaining usable rows with its first value.
    let tables = run(&cs, 5, &[vec![Fp::ZERO]], |layouter| {
        load_table(layouter, config.table)
    })
    .expect("table");
    let column = &tables.fixed()[config.table.inner().index()];
    assert_eq!(column[7], Fp::from(7));
    assert_eq!(column[8], Fp::ZERO);
    assert_eq!(column[tables.usable_rows() - 1], Fp::ZERO);
    assert!(tables.fixed_assigned()[config.table.inner().index()][tables.usable_rows() - 1]);
}

#[test]
fn namespaces_prefix_region_names() {
    let (cs, config) = mul_cs();
    let tables = run(&cs, 5, &[vec![Fp::ZERO]], |layouter| {
        let mut chip = layouter.namespace(|| "chip");
        chip.assign_region(
            || "inner",
            |mut region| {
                region.assign_advice(config.b, 0, Value::known(Fp::ONE))?;
                Ok(())
            },
        )?;
        drop(chip);
        layouter.assign_region(
            || "outer",
            |mut region| {
                region.assign_advice(config.b, 1, Value::known(Fp::ONE))?;
                Ok(())
            },
        )
    })
    .expect("synthesis");
    let names: Vec<&str> = tables.regions().iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["chip::inner", "outer"]);
}

#[test]
fn check_rejects_keygen_and_mismatched_tables() {
    let (cs, config) = mul_cs();
    let mut keygen = Assembly::new(&cs, 5, None).expect("keygen");
    {
        let mut layouter = SingleChipLayouter::new(&mut keygen, cs.constants().to_vec());
        load_table(&mut layouter, config.table).expect("table");
    }
    let keygen = keygen.finish().expect("finish");
    assert_eq!(
        check(&cs, &keygen, CheckMode::Strict),
        Err(Error::WitnessRequired)
    );
    let witness = chain(2);
    let (cs, tables) = mul_tables(&witness, witness[1][2]).expect("synthesis");
    let finalized = cs
        .clone()
        .finalize(tables.selectors(), true)
        .expect("finalize");
    assert_eq!(
        check(finalized.constraint_system(), &tables, CheckMode::Strict),
        Err(Error::BoundsFailure),
        "the checker runs on the uncompressed constraint system"
    );
}

/// A circuit driving the full `Circuit` path.
#[derive(Clone)]
struct SquareCircuit {
    x: Value<Fp>,
}

impl Circuit<Fp> for SquareCircuit {
    type Config = (Column<Advice>, Column<Instance>, Selector);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            x: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let column = meta.advice_column();
        let out = meta.instance_column(1);
        meta.enable_equality(column);
        meta.enable_equality(out);
        let s = meta.selector();
        meta.create_gate("square", |cells| {
            let s = cells.query_selector(s);
            let x = cells.query_advice(column, Rotation::cur());
            let y = cells.query_advice(column, Rotation::next());
            vec![s * (x.clone() * x - y)]
        });
        (column, out, s)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let (x, out, s) = config;
        let y = layouter.assign_region(
            || "square",
            |mut region| {
                s.enable(&mut region, 0)?;
                let input = region.assign_advice(x, 0, self.x)?;
                let output = region.assign_advice(x, 1, self.x.map(|v| v.square()))?;
                let copied = input.copy_advice(&mut region, x, 2)?;
                region.constrain_equal(copied.cell(), input.cell())?;
                Ok(output.cell())
            },
        )?;
        layouter.constrain_instance(y, out, 0)
    }
}

#[test]
fn circuit_path_end_to_end() {
    let circuit = SquareCircuit {
        x: Value::known(Fp::from(3)),
    };
    let report =
        check_circuit(&circuit, 4, &[vec![Fp::from(9)]], CheckMode::Strict).expect("check");
    assert!(report.is_satisfied(), "{report}");
    let report =
        check_circuit(&circuit, 4, &[vec![Fp::from(10)]], CheckMode::Strict).expect("check");
    assert!(!report.is_satisfied());
    assert!(matches!(
        check_circuit(&circuit, 4, &[], CheckMode::Strict),
        Err(Error::InstanceShape { column: None, .. })
    ));
    assert_eq!(
        check_circuit(&circuit, 2, &[vec![Fp::from(9)]], CheckMode::Strict),
        Err(Error::NotEnoughRowsAvailable { current_k: 2 })
    );
    let keygen =
        crate::frontend::synthesize(&circuit.without_witnesses(), 4, None).expect("keygen");
    assert!(keygen.tables.advice().is_none());
    assert!(keygen.tables.selectors()[0][0]);
    assert_eq!(
        crate::frontend::synthesize(&circuit.without_witnesses(), 4, Some(&[vec![Fp::ONE]]))
            .unwrap_err(),
        Error::Synthesis,
        "proving needs every witness value"
    );
}

/// Evaluates a finalized (selector-free) polynomial at `row` with the
/// selector columns appended to the fixed columns.
fn evaluate_compressed(
    poly: &Expression<Fp>,
    tables: &AssignedTables<Fp>,
    fixed: &[Vec<Fp>],
    row: usize,
) -> Option<Fp> {
    let witness = Witness {
        n: tables.n(),
        usable: tables.usable_rows(),
        mode: CheckMode::Halo2Compatible,
        fixed,
        advice: tables.advice().expect("witness"),
        advice_assigned: tables.advice_assigned(),
        instance: tables.instance().expect("witness"),
        selectors: &[],
        tables,
    };
    match poly.evaluate(&mut RowEvaluator {
        witness: &witness,
        row,
    }) {
        Val::Real(value) => Some(value),
        Val::Poison => None,
    }
}

/// Differential test: the checker's naive evaluation of the source
/// expressions agrees with the evaluation of the selector-compressed
/// constraint system on random circuits and assignments (zero-ness and
/// poison on every row).
#[test]
fn checker_agrees_with_compressed_constraint_system() {
    let mut rng = ChaCha20Rng::from_seed([11; 32]);
    for case in 0..40 {
        let mut cs = ConstraintSystem::<Fp>::new();
        let advice: Vec<_> = (0..3).map(|_| cs.advice_column()).collect();
        let fixed = cs.fixed_column();
        let selectors: Vec<Selector> = (0..5)
            .map(|i| {
                if rng.next_u32() % 4 == 0 && i > 0 {
                    cs.complex_selector()
                } else {
                    cs.selector()
                }
            })
            .collect();
        for (index, selector) in selectors.iter().enumerate() {
            let degree = 1 + (rng.next_u32() % 3) as usize;
            let rotation = Rotation(i32::try_from(rng.next_u32() % 2).expect("small"));
            let column = advice[index % advice.len()];
            cs.create_gate(format!("g{index}"), |meta| {
                let s = meta.query_selector(*selector);
                let mut poly = meta.query_advice(column, rotation);
                for _ in 1..degree {
                    poly = poly * meta.query_advice(column, Rotation::cur());
                }
                let f = meta.query_fixed(fixed, Rotation::cur());
                vec![s * (poly - f)]
            });
        }
        let k = 5;
        let compress = case % 2 == 0;
        let tables = run(&cs, k, &[], |layouter| {
            layouter.assign_region(
                || "random",
                |mut region| {
                    for row in 0..20 {
                        for column in &advice {
                            let value = Fp::from(u64::from(rng.next_u32() % 3));
                            region.assign_advice(*column, row, Value::known(value))?;
                        }
                        region.assign_fixed(fixed, row, Fp::from(u64::from(rng.next_u32() % 2)))?;
                        for selector in &selectors {
                            if rng.next_u32() % 3 == 0 {
                                region.enable_selector(|| "", selector, row)?;
                            }
                        }
                    }
                    Ok(())
                },
            )
        })
        .expect("synthesis");
        let report = check(&cs, &tables, CheckMode::Halo2Compatible).expect("check");
        let finalized = cs
            .clone()
            .finalize(tables.selectors(), compress)
            .expect("finalize");
        let mut fixed_columns = tables.fixed().to_vec();
        fixed_columns.extend_from_slice(finalized.selector_columns());
        let compressed = finalized.constraint_system();
        for (gate_index, (source, target)) in cs.gates().iter().zip(compressed.gates()).enumerate()
        {
            let source_poly = &source.polynomials()[0];
            let target_poly = &target.polynomials()[0];
            for row in 0..tables.n() {
                let witness = Witness {
                    n: tables.n(),
                    usable: tables.usable_rows(),
                    mode: CheckMode::Halo2Compatible,
                    fixed: tables.fixed(),
                    advice: tables.advice().expect("witness"),
                    advice_assigned: tables.advice_assigned(),
                    instance: tables.instance().expect("witness"),
                    selectors: tables.selectors(),
                    tables: &tables,
                };
                let source_value = match source_poly.evaluate(&mut RowEvaluator {
                    witness: &witness,
                    row,
                }) {
                    Val::Real(value) => Some(value),
                    Val::Poison => None,
                };
                let target_value = evaluate_compressed(target_poly, &tables, &fixed_columns, row);
                assert_eq!(
                    source_value.map(|v| v.is_zero_vartime()),
                    target_value.map(|v| v.is_zero_vartime()),
                    "case {case} gate {gate_index} row {row}"
                );
                let reported = report.failures().iter().any(|failure| match failure {
                    CheckFailure::ConstraintNotSatisfied { gate, location, .. }
                    | CheckFailure::ConstraintPoisoned { gate, location, .. } => {
                        *gate == gate_index && location.row == row
                    }
                    _ => false,
                });
                assert_eq!(
                    reported,
                    !source_value.is_some_and(|v| v.is_zero_vartime()),
                    "case {case} gate {gate_index} row {row}"
                );
            }
        }
    }
}

#[test]
fn failure_display_covers_every_variant() {
    let cell = QueriedCell {
        column: Column::new(0, Any::Fixed),
        rotation: Rotation(-1),
        row: 3,
        value: Fp::ONE,
        issue: None,
    };
    let location = Location {
        row: 4,
        region: None,
    };
    let failures = [
        CheckFailure::LookupInputMissing {
            lookup: 0,
            name: "l".into(),
            location: location.clone(),
            input: vec![Fp::ONE],
            cells: vec![cell.clone()],
        },
        CheckFailure::LookupPoisoned {
            lookup: 0,
            name: "l".into(),
            table: false,
            location: location.clone(),
            cells: vec![cell.clone()],
        },
        CheckFailure::ConstraintPoisoned {
            gate: 1,
            gate_name: "g".into(),
            constraint: 0,
            constraint_name: String::new(),
            location,
            cells: vec![cell],
        },
    ];
    for failure in failures {
        let text = failure.to_string();
        assert!(text.contains("F0[-1]@3"), "{text}");
        assert!(text.contains("row 4"), "{text}");
    }
}

#[test]
fn assigned_fractions_reach_the_checker() {
    let mut cs = ConstraintSystem::<Fp>::new();
    let a = cs.advice_column();
    let s = cs.selector();
    cs.create_gate("half", |meta| {
        let s = meta.query_selector(s);
        let a = meta.query_advice(a, Rotation::cur());
        vec![s * (a * Fp::from(2) - Expression::Constant(Fp::ONE))]
    });
    let tables = run(&cs, 4, &[], |layouter| {
        layouter.assign_region(
            || "fraction",
            |mut region| {
                s.enable(&mut region, 0)?;
                region.assign_advice(
                    a,
                    0,
                    Value::known(Assigned::Rational(Fp::ONE, Fp::from(2))),
                )?;
                Ok(())
            },
        )
    })
    .expect("synthesis");
    assert!(
        check(&cs, &tables, CheckMode::Strict)
            .expect("check")
            .is_satisfied()
    );
}
