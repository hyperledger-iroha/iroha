//! Witness generation: runs a circuit's hint program over every row.
//!
//! Rows are independent once each row's starting state is known, so they are generated in
//! parallel. The application supplies the state sequence from its native reference execution;
//! every row checks that the circuit's next-state expressions reproduce the following state, so a divergence between
//! the circuit and the caller's reference semantics is caught before any proving work.

use rayon::prelude::*;

use crate::circuit::{
    Circuit, Hint, PUBLIC_ACTIVE, PUBLIC_CHECKPOINT, PUBLIC_FIRST, PUBLIC_LAST, PUBLIC_ROW, Var,
};
use crate::field::Fp;

/// Witness generation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TraceError {
    /// The statement claims more steps than the circuit holds.
    #[error("{steps} steps exceed the circuit capacity {capacity}")]
    TooManySteps {
        /// Claimed steps.
        steps: usize,
        /// Maximum steps.
        capacity: usize,
    },
    /// A supplied matrix has the wrong shape.
    #[error("{0} has the wrong shape")]
    Shape(&'static str),
    /// A public value lies outside its declared interval.
    #[error("public column {column} row {row} value {value} is outside its declared interval")]
    PublicOutOfRange {
        /// Column.
        column: usize,
        /// Row.
        row: usize,
        /// Value.
        value: i64,
    },
    /// A state value lies outside its register's interval.
    #[error("register {register} row {row} value {value} is outside its declared interval")]
    StateOutOfRange {
        /// Register.
        register: usize,
        /// Row.
        row: usize,
        /// Value.
        value: i64,
    },
    /// The circuit's transition disagrees with the supplied next state.
    #[error(
        "row {row}: circuit computes register {register} = {circuit}, simulation says {native}"
    )]
    Divergence {
        /// Row.
        row: usize,
        /// Register.
        register: usize,
        /// Circuit value.
        circuit: i64,
        /// Supplied value.
        native: i64,
    },
    /// A recorded interval fact does not hold (a circuit bug).
    #[error("row {row}: assumption {assumption} does not hold")]
    Assumption {
        /// Row.
        row: usize,
        /// Assumption index.
        assumption: usize,
    },
    /// A range-checked value is outside its range (the step's arithmetic left its proven bounds).
    #[error("row {row}: lookup {lookup} value {value} is out of range")]
    LookupOutOfRange {
        /// Row.
        row: usize,
        /// Lookup index.
        lookup: usize,
        /// Value.
        value: i128,
    },
}

/// Builds every public column (engine columns first) for a statement with `steps` steps and an
/// optional checkpoint step.
///
/// `public_inputs[c][row]` supplies the application's own public columns for all rows.
///
/// # Errors
/// Rejects wrong shapes, too many steps, a checkpoint beyond the last step and values outside
/// declared intervals.
pub fn public_columns(
    circuit: &Circuit,
    steps: usize,
    checkpoint: Option<usize>,
    public_inputs: &[Vec<i64>],
) -> Result<Vec<Vec<i64>>, TraceError> {
    let n = circuit.rows();
    if steps > circuit.max_steps() {
        return Err(TraceError::TooManySteps {
            steps,
            capacity: circuit.max_steps(),
        });
    }
    if checkpoint.is_some_and(|step| step > steps) {
        return Err(TraceError::Shape("checkpoint beyond the last step"));
    }
    let engine = crate::circuit::ENGINE_PUBLIC_COLUMNS as usize;
    if public_inputs.len() + engine != circuit.public.len()
        || public_inputs.iter().any(|c| c.len() != n)
    {
        return Err(TraceError::Shape("public columns"));
    }
    let mut out = Vec::with_capacity(circuit.public.len());
    for column in 0..engine as u32 {
        out.push(
            (0..n)
                .map(|row| match column {
                    PUBLIC_ACTIVE => i64::from(row < steps),
                    PUBLIC_FIRST => i64::from(row == 0),
                    PUBLIC_LAST => i64::from(row == steps),
                    PUBLIC_ROW => i64::try_from(row).expect("trace height fits i64"),
                    PUBLIC_CHECKPOINT => i64::from(checkpoint == Some(row)),
                    _ => unreachable!(),
                })
                .collect(),
        );
    }
    out.extend(public_inputs.iter().cloned());
    for (column, (values, spec)) in out.iter().zip(&circuit.public).enumerate() {
        if let Some((row, value)) = values
            .iter()
            .enumerate()
            .find(|(_, v)| **v < spec.lo || **v > spec.hi)
        {
            return Err(TraceError::PublicOutOfRange {
                column,
                row,
                value: *value,
            });
        }
    }
    Ok(out)
}

/// The main trace, column-major, with the range-table multiplicities of its lookups.
#[derive(Clone, Debug)]
pub struct MainTrace {
    /// `columns[c][row]`.
    pub columns: Vec<Vec<Fp>>,
    /// `multiplicities[v]` = number of lookups of `v` over all rows (empty without lookups).
    pub multiplicities: Vec<u64>,
}

fn eval_row(
    circuit: &Circuit,
    row: usize,
    public: &[Vec<i64>],
    state: &[i64],
    values: &mut [i128],
    field: &mut [Fp],
) {
    let lookup = |values: &[i128], var: Var| -> i128 {
        match var {
            Var::Main(i) => values[i as usize],
            Var::Public(i) => i128::from(public[i as usize][row]),
            Var::Next(_) => unreachable!("hints never read the next row"),
        }
    };
    for (index, column) in circuit.columns.iter().enumerate() {
        let (value, field_value) = match &column.hint {
            Hint::State(r) => (i128::from(state[*r as usize]), None),
            Hint::Quad(q) => (q.eval_i(|v| lookup(values, v)), None),
            Hint::LessThan(a, b) => (
                i128::from(a.eval_i(|v| lookup(values, v)) < b.eval_i(|v| lookup(values, v))),
                None,
            ),
            Hint::Limb { value, shift, bits } => {
                let v = value.eval_i(|x| lookup(values, x));
                ((v >> shift) & ((1i128 << bits) - 1), None)
            }
            Hint::DivTrunc(a, c) => (a.eval_i(|v| lookup(values, v)) / i128::from(*c), None),
            Hint::DivFloor(a, c) => (
                a.eval_i(|v| lookup(values, v)).div_euclid(i128::from(*c)),
                None,
            ),
            Hint::Equals(a, k) => (
                i128::from(a.eval_i(|v| lookup(values, v)) == i128::from(*k)),
                None,
            ),
            Hint::QuotientEquals { value, divisor, k } => (
                i128::from(
                    value
                        .eval_i(|v| lookup(values, v))
                        .div_euclid(i128::from(*divisor))
                        == i128::from(*k),
                ),
                None,
            ),
            Hint::Inverse(q) => {
                let v = Fp::from_i128(q.eval_i(|x| lookup(values, x)));
                (0, Some(v.inverse().unwrap_or(Fp::ZERO)))
            }
        };
        values[index] = value;
        field[index] = field_value.unwrap_or_else(|| Fp::from_i128(value));
    }
}

/// Generates the main trace.
///
/// `states[row]` is the state before step `row`; rows `steps..rows` repeat the final state.
///
/// # Errors
/// Rejects shape errors, divergence from the supplied states and failed range checks.
pub fn generate(
    circuit: &Circuit,
    public: &[Vec<i64>],
    steps: usize,
    states: &[Vec<i64>],
) -> Result<MainTrace, TraceError> {
    generate_inner(circuit, public, steps, states, true)
}

/// [`generate`] with an optional next-state check; with `check_transitions` off, `states` may also
/// hold one entry per row (padding included). Tests use it to build forged traces.
pub(crate) fn generate_inner(
    circuit: &Circuit,
    public: &[Vec<i64>],
    steps: usize,
    states: &[Vec<i64>],
    check_transitions: bool,
) -> Result<MainTrace, TraceError> {
    let n = circuit.rows();
    let registers = circuit.registers.len();
    let per_row = !check_transitions && states.len() == n;
    if (!per_row && states.len() != steps + 1) || states.iter().any(|s| s.len() != registers) {
        return Err(TraceError::Shape("states"));
    }
    for (row, state) in states.iter().enumerate() {
        for (register, (value, spec)) in state.iter().zip(&circuit.registers).enumerate() {
            if *value < spec.lo || *value > spec.hi {
                return Err(TraceError::StateOutOfRange {
                    register,
                    row,
                    value: *value,
                });
            }
        }
    }
    let width = circuit.columns.len();
    let limb_limit = 1i128 << circuit.range_bits;
    let has_lookups = !circuit.lookups.is_empty();
    let rows: Vec<Result<(Vec<Fp>, Vec<u32>), TraceError>> = (0..n)
        .into_par_iter()
        .map(|row| {
            let state = &states[if per_row { row } else { row.min(steps) }];
            let mut values = vec![0i128; width];
            let mut field = vec![Fp::ZERO; width];
            eval_row(circuit, row, public, state, &mut values, &mut field);
            // Next-state consistency (rows past the end are frozen).
            let next_state = &states[(row + 1).min(states.len() - 1)];
            if check_transitions && row + 1 < n {
                for (register, spec) in circuit.registers.iter().enumerate() {
                    let expected = i128::from(next_state[register]);
                    let computed = if row < steps {
                        spec.next.eval_i(|v| match v {
                            Var::Main(i) => values[i as usize],
                            Var::Public(i) => i128::from(public[i as usize][row]),
                            Var::Next(_) => unreachable!(),
                        })
                    } else {
                        values[spec.column as usize]
                    };
                    if computed != expected {
                        return Err(TraceError::Divergence {
                            row,
                            register,
                            circuit: i64::try_from(computed).unwrap_or(i64::MAX),
                            native: next_state[register],
                        });
                    }
                }
            }
            for (index, fact) in circuit.assumptions.iter().enumerate() {
                let v = fact.value.eval_i(|var| match var {
                    Var::Main(i) => values[i as usize],
                    Var::Public(i) => i128::from(public[i as usize][row]),
                    Var::Next(_) => unreachable!(),
                });
                if v < fact.lo || v > fact.hi {
                    return Err(TraceError::Assumption {
                        row,
                        assumption: index,
                    });
                }
            }
            let mut lookups = Vec::new();
            if has_lookups {
                for (index, lin) in circuit.lookups.iter().enumerate() {
                    let v = lin.eval_i(|var| match var {
                        Var::Main(i) => values[i as usize],
                        Var::Public(i) => i128::from(public[i as usize][row]),
                        Var::Next(_) => unreachable!(),
                    });
                    if v < 0 || v >= limb_limit {
                        return Err(TraceError::LookupOutOfRange {
                            row,
                            lookup: index,
                            value: v,
                        });
                    }
                    lookups.push(v as u32);
                }
            }
            Ok((field, lookups))
        })
        .collect();
    let mut columns = vec![vec![Fp::ZERO; n]; width];
    let mut counts = vec![
        0u64;
        if has_lookups {
            1usize << circuit.range_bits
        } else {
            0
        }
    ];
    for (row, result) in rows.into_iter().enumerate() {
        let (field, lookups) = result?;
        for (column, value) in columns.iter_mut().zip(field) {
            column[row] = value;
        }
        for v in lookups {
            counts[v as usize] += 1;
        }
    }
    Ok(MainTrace {
        columns,
        multiplicities: counts,
    })
}
