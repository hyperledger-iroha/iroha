//! Test-only exact upstream 56-partial-round Pasta Poseidon circuit candidate.
//!
//! Five advice columns constrain every round, the initial length capacity,
//! copied absorption inputs and exact zero padding. Constants come exclusively
//! from the shared leaf tables. No host hash callback supplies the relation.
//! Owned input bytes and working field cells clear on drop; Halo2 assignments,
//! arithmetic temporaries and compiler copies remain outside that claim.
//! TODO: Qualify a bounded multi-lane layout and complete semantic/BFV relation
//! before any production caller, relation identifier or proof-mode admission.

use ff::{Field, FromUniformBytes, PrimeField};
use halo2_proofs::{
    circuit::{Cell, Layouter, Region, Value},
    halo2curves::pasta::{Fp, Fq},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Fixed, Selector},
    poly::Rotation,
};
use iroha_zkp_poseidon::pasta;
use zeroize::{DefaultIsZeroes, Zeroize};

const PARAMETER_BYTES: usize = 201 * 32;
const ROUNDS: usize = 64;
const MAX_FIELDS: usize = 2054;

trait PastaField: PrimeField<Repr = [u8; 32]> + FromUniformBytes<64> + Ord {
    const PARAMETERS: &'static [u8; PARAMETER_BYTES];
    fn native_hash<const L: usize>(input: &[[u8; 32]; L]) -> [u8; 32];
}

impl PastaField for Fp {
    const PARAMETERS: &'static [u8; PARAMETER_BYTES] = pasta::fp::PARAMETER_BYTES;
    fn native_hash<const L: usize>(input: &[[u8; 32]; L]) -> [u8; 32] {
        pasta::fp::hash(input).expect("canonical test inputs")
    }
}

impl PastaField for Fq {
    const PARAMETERS: &'static [u8; PARAMETER_BYTES] = pasta::fq::PARAMETER_BYTES;
    fn native_hash<const L: usize>(input: &[[u8; 32]; L]) -> [u8; 32] {
        pasta::fq::hash(input).expect("canonical test inputs")
    }
}

#[derive(Clone, Debug)]
struct Parameters<F> {
    rounds: [[F; 3]; ROUNDS],
    mds: [[F; 3]; 3],
}

impl<F: PastaField> Parameters<F> {
    fn pinned() -> Self {
        let mut result = Self {
            rounds: [[F::ZERO; 3]; ROUNDS],
            mds: [[F::ZERO; 3]; 3],
        };
        for (cell, bytes) in result
            .rounds
            .iter_mut()
            .chain(result.mds.iter_mut())
            .flatten()
            .zip(F::PARAMETERS.chunks_exact(32))
        {
            *cell = Option::from(F::from_repr(bytes.try_into().expect("constant width")))
                .expect("pinned canonical parameter");
        }
        result
    }
}

#[derive(Clone, Debug)]
struct PoseidonConfig<F> {
    state: [Column<Advice>; 3],
    input: [Column<Advice>; 2],
    constants: [Column<Fixed>; 3],
    full: Selector,
    partial: Selector,
    absorb: Selector,
    initial: Selector,
    padding: Selector,
    parameters: Parameters<F>,
}

fn fifth<F: Field>(value: Expression<F>) -> Expression<F> {
    let square = value.clone() * value.clone();
    square.clone() * square * value
}

impl<F: PastaField> PoseidonConfig<F> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let state = std::array::from_fn(|_| meta.advice_column());
        let input = std::array::from_fn(|_| meta.advice_column());
        for &column in state.iter().chain(&input) {
            meta.enable_equality(column);
        }
        let constants = std::array::from_fn(|_| meta.fixed_column());
        let full = meta.complex_selector();
        let partial = meta.complex_selector();
        let absorb = meta.complex_selector();
        let initial = meta.complex_selector();
        let padding = meta.complex_selector();
        let parameters = Parameters::<F>::pinned();
        for (selector, is_full) in [(full, true), (partial, false)] {
            meta.create_gate("exact pinned Poseidon round", |meta| {
                let powered: [_; 3] = std::array::from_fn(|column| {
                    let value = meta.query_advice(state[column], Rotation::cur())
                        + meta.query_fixed(constants[column], Rotation::cur());
                    if is_full || column == 0 {
                        fifth(value)
                    } else {
                        value
                    }
                });
                (0..3)
                    .map(|row| {
                        let result = (0..3).fold(Expression::Constant(F::ZERO), |sum, column| {
                            sum + Expression::Constant(parameters.mds[row][column])
                                * powered[column].clone()
                        });
                        meta.query_selector(selector)
                            * (meta.query_advice(state[row], Rotation::next()) - result)
                    })
                    .collect::<Vec<_>>()
            });
        }
        meta.create_gate("copy-bound rate-two absorption", |meta| {
            (0..3)
                .map(|column| {
                    let delta = if column < 2 {
                        meta.query_advice(input[column], Rotation::cur())
                    } else {
                        Expression::Constant(F::ZERO)
                    };
                    meta.query_selector(absorb)
                        * (meta.query_advice(state[column], Rotation::next())
                            - meta.query_advice(state[column], Rotation::cur())
                            - delta)
                })
                .collect::<Vec<_>>()
        });
        meta.create_gate("fixed length capacity and zero initial rate", |meta| {
            (0..3)
                .map(|column| {
                    let expected = if column == 2 {
                        meta.query_fixed(constants[0], Rotation::cur())
                    } else {
                        Expression::Constant(F::ZERO)
                    };
                    meta.query_selector(initial)
                        * (meta.query_advice(state[column], Rotation::cur()) - expected)
                })
                .collect::<Vec<_>>()
        });
        meta.create_gate("fixed odd-length zero padding", |meta| {
            vec![meta.query_selector(padding) * meta.query_advice(input[1], Rotation::cur())]
        });
        Self {
            state,
            input,
            constants,
            full,
            partial,
            absorb,
            initial,
            padding,
            parameters,
        }
    }
}

#[derive(Clone, Copy)]
struct WorkingValues<F: PastaField> {
    state: [F; 3],
    powered: [F; 3],
    input: [F; 2],
}

impl<F: PastaField> Default for WorkingValues<F> {
    fn default() -> Self {
        Self {
            state: [F::ZERO; 3],
            powered: [F::ZERO; 3],
            input: [F::ZERO; 2],
        }
    }
}

impl<F: PastaField> DefaultIsZeroes for WorkingValues<F> {}

struct Working<F: PastaField>(WorkingValues<F>);

impl<F: PastaField> Drop for Working<F> {
    fn drop(&mut self) {
        self.0.zeroize();
        tests::observe_work_clear(
            self.0
                .state
                .iter()
                .chain(&self.0.powered)
                .chain(&self.0.input)
                .all(|value| bool::from(value.is_zero())),
        );
    }
}

// These mutation hooks exist only in this test-only module. Mutated values are
// propagated through later assignments so downstream output checks cannot mask
// a missing local transition, padding or source-copy constraint.
#[derive(Clone)]
enum Fault<F> {
    State { row: usize, column: usize, value: F },
    Input { index: usize, value: F },
    Copy { index: usize, source: usize },
    Padding(F),
}

fn assign_state<F: PastaField>(
    region: &mut Region<'_, F>,
    config: &PoseidonConfig<F>,
    row: usize,
    work: &mut Working<F>,
    faults: &[Fault<F>],
) -> [Cell; 3] {
    for fault in faults {
        if let Fault::State {
            row: at,
            column,
            value,
        } = *fault
        {
            if at == row {
                work.0.state[column] = value;
            }
        }
    }
    std::array::from_fn(|column| {
        region
            .assign_advice(
                config.state[column],
                row,
                Value::known(work.0.state[column]),
            )
            .cell()
    })
}

fn hash<F: PastaField, const L: usize>(
    config: &PoseidonConfig<F>,
    layouter: &mut impl Layouter<F>,
    source: &[Cell; L],
    values: &[[u8; 32]; L],
    faults: &[Fault<F>],
    fail_after_absorb: bool,
    panic_after_absorb: bool,
) -> Result<Cell, Error> {
    if L == 0 || L > MAX_FIELDS {
        return Err(Error::Synthesis);
    }
    layouter.assign_region(
        || "pinned ConstantLength Poseidon",
        |mut region| {
            let mut work = Working(WorkingValues::<F>::default());
            work.0.state[2] = F::from_u128((L as u128) << 64);
            config.initial.enable(&mut region, 0)?;
            region.assign_fixed(config.constants[0], 0, work.0.state[2]);
            let mut row = 0;
            let mut cells = assign_state(&mut region, config, row, &mut work, faults);
            for block in 0..L.div_ceil(2) {
                config.absorb.enable(&mut region, row)?;
                for column in 0..2 {
                    let index = 2 * block + column;
                    let mut from = index;
                    work.0.input[column] = if index < L {
                        Option::from(F::from_repr(values[index])).ok_or(Error::Synthesis)?
                    } else {
                        F::ZERO
                    };
                    for fault in faults {
                        match *fault {
                            Fault::Input { index: at, value } if index == at => {
                                work.0.input[column] = value
                            }
                            Fault::Copy { index: at, source } if index == at => from = source,
                            Fault::Padding(value) if index >= L => work.0.input[column] = value,
                            _ => {}
                        }
                    }
                    let input = region
                        .assign_advice(
                            config.input[column],
                            row,
                            Value::known(work.0.input[column]),
                        )
                        .cell();
                    if index < L {
                        region.constrain_equal(source[from], input);
                    } else {
                        config.padding.enable(&mut region, row)?;
                    }
                }
                for column in 0..2 {
                    work.0.state[column] += work.0.input[column];
                }
                row += 1;
                cells = assign_state(&mut region, config, row, &mut work, faults);
                if fail_after_absorb {
                    return Err(Error::Synthesis);
                }
                assert!(!panic_after_absorb, "test-only Poseidon assignment unwind");
                for round in 0..ROUNDS {
                    let full = matches!(round, 0..=3 | 60..=63);
                    (if full { config.full } else { config.partial }).enable(&mut region, row)?;
                    for column in 0..3 {
                        region.assign_fixed(
                            config.constants[column],
                            row,
                            config.parameters.rounds[round][column],
                        );
                        work.0.powered[column] =
                            work.0.state[column] + config.parameters.rounds[round][column];
                        if full || column == 0 {
                            work.0.powered[column] =
                                work.0.powered[column].square().square() * work.0.powered[column];
                        }
                    }
                    for output in 0..3 {
                        work.0.state[output] = (0..3).fold(F::ZERO, |sum, input| {
                            sum + config.parameters.mds[output][input] * work.0.powered[input]
                        });
                    }
                    row += 1;
                    cells = assign_state(&mut region, config, row, &mut work, faults);
                }
            }
            assert_eq!(row + 1, hash_rows(L));
            Ok(cells[0])
        },
    )
}

const fn hash_rows(length: usize) -> usize {
    length.div_ceil(2) * (ROUNDS + 1) + 1
}

#[path = "ram_lfe_poseidon_tests.rs"]
mod tests;
