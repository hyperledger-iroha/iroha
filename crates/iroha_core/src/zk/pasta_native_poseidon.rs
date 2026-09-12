//! Fixed native Pasta Poseidon permutations with exact Base input/output bridges.
//!
//! This internal machine uses the existing width-3/rate-2, eight-full/57-partial-round
//! generator. Base retains ordered sponge absorption, domain-prefix constraints and one/zero
//! padding. Every permutation is constrained in a dedicated lane and its three input and output
//! state cells are bound through one equality bus to the original Base graph. It does not authenticate a standalone hash
//! or replace any caller's recursive history, source inventory or public binding.

use halo2_base::{
    AssignedValue, Context,
    gates::{GateChip, GateInstructions},
    halo2_proofs::{
        circuit::{Layouter, Value},
        plonk::{Advice, Column, ConstraintSystem, Error, Expression, Fixed},
        poly::Rotation,
    },
    poseidon::hasher::spec::OptimizedPoseidonSpec,
    virtual_region::copy_constraints::SharedCopyConstraintManager,
};

use super::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1;

const WIDTH: usize = 3;
const FULL_ROUNDS: usize = 8;
const PARTIAL_ROUNDS: usize = 57;
const ROUNDS: usize = FULL_ROUNDS + PARTIAL_ROUNDS;
const PERMUTATION_ROWS: usize = ROUNDS + 1;
const MAX_LANES: usize = 2;
const K16_USABLE_ROWS: usize = (1 << 16) - 9;

#[derive(Clone, Debug)]
struct RawSpec<F> {
    constants: Vec<[F; WIDTH]>,
    mds: [[F; WIDTH]; WIDTH],
}

impl<F: KagemushaPoseidonFieldV1> RawSpec<F> {
    fn new() -> Self {
        let (constants, mds) = OptimizedPoseidonSpec::<F, WIDTH, 2>::unoptimized_constants::<
            FULL_ROUNDS,
            PARTIAL_ROUNDS,
            0,
        >();
        assert_eq!(constants.len(), ROUNDS, "fixed Pasta round count");
        Self { constants, mds }
    }

    fn transition(&self, state: [F; WIDTH], round: usize) -> [F; WIDTH] {
        let mut powered: [F; WIDTH] = std::array::from_fn(|i| state[i] + self.constants[round][i]);
        for (i, value) in powered.iter_mut().enumerate() {
            if full_round(round) || i == 0 {
                *value = value.square().square() * *value;
            }
        }
        std::array::from_fn(|i| {
            (0..WIDTH).fold(F::ZERO, |sum, j| sum + self.mds[i][j] * powered[j])
        })
    }

    fn permutation(&self, mut state: [F; WIDTH]) -> [F; WIDTH] {
        for round in 0..ROUNDS {
            state = self.transition(state, round);
        }
        state
    }
}

fn full_round(round: usize) -> bool {
    round < FULL_ROUNDS / 2 || round >= FULL_ROUNDS / 2 + PARTIAL_ROUNDS
}

/// Native permutation lanes sharing round constants, fixed modes and one equality bus.
///
/// State columns are local to the permutation gates. A fixed four-way bridge schedule copies
/// all three inputs and outputs of each lane through one bus, preserving the 66-row blocks.
#[derive(Clone, Debug)]
pub(super) struct PastaNativePoseidonConfigV1 {
    lanes: Vec<[Column<Advice>; WIDTH]>,
    bus: Column<Advice>,
    constants: [Column<Fixed>; WIDTH],
    round_mode: Column<Fixed>,
    bridge_mode: Column<Fixed>,
}

fn bridge_start(lane: usize, output: bool) -> usize {
    if output {
        60 + WIDTH * lane
    } else {
        WIDTH * lane
    }
}

fn bridge_code(lane: usize, output: bool) -> u64 {
    if output {
        3 + lane as u64
    } else {
        1 + lane as u64
    }
}

impl PastaNativePoseidonConfigV1 {
    /// Configure one or two lanes with one shared equality bus and degree-seven round gates.
    pub(super) fn configure<F: KagemushaPoseidonFieldV1>(
        meta: &mut ConstraintSystem<F>,
        lane_count: usize,
    ) -> Self {
        assert!(
            (1..=MAX_LANES).contains(&lane_count),
            "native Poseidon lane count"
        );
        let lanes = (0..lane_count)
            .map(|_| std::array::from_fn(|_| meta.advice_column()))
            .collect::<Vec<_>>();
        let bus = meta.advice_column();
        meta.enable_equality(bus);
        let constants = std::array::from_fn(|_| meta.fixed_column());
        let round_mode = meta.fixed_column();
        let bridge_mode = meta.fixed_column();
        let spec = RawSpec::<F>::new();
        for columns in &lanes {
            meta.create_gate("native Pasta exact Poseidon rounds", |meta| {
                let state: [_; WIDTH] = std::array::from_fn(|i| {
                    meta.query_advice(columns[i], Rotation::cur())
                        + meta.query_fixed(constants[i], Rotation::cur())
                });
                let next: [_; WIDTH] =
                    std::array::from_fn(|i| meta.query_advice(columns[i], Rotation::next()));
                let pow5: [_; WIDTH] = std::array::from_fn(|i| {
                    let square = state[i].clone() * state[i].clone();
                    square.clone() * square * state[i].clone()
                });
                // The fixed mode is 0 off-schedule, 1 for partial and 2 for full rounds.
                // These exact quadratic indicators raise the round degree from six to seven;
                // both degrees use the same extended domain, while keys/protocols regenerate.
                let mode = meta.query_fixed(round_mode, Rotation::cur());
                let one = Expression::Constant(F::ONE);
                let two = Expression::Constant(F::from(2));
                let half = Expression::Constant(F::from(2).invert().unwrap());
                let full = mode.clone() * (mode.clone() - one) * half;
                let partial = mode.clone() * (two - mode);
                let mut constraints = Vec::with_capacity(2 * WIDTH);
                for i in 0..WIDTH {
                    let mut full_sum = Expression::Constant(F::ZERO);
                    let mut partial_sum = Expression::Constant(F::ZERO);
                    for j in 0..WIDTH {
                        let coefficient = Expression::Constant(spec.mds[i][j]);
                        full_sum = full_sum + coefficient.clone() * pow5[j].clone();
                        partial_sum = partial_sum
                            + coefficient
                                * if j == 0 {
                                    pow5[j].clone()
                                } else {
                                    state[j].clone()
                                };
                    }
                    constraints.push(full.clone() * (next[i].clone() - full_sum));
                    constraints.push(partial.clone() * (next[i].clone() - partial_sum));
                }
                constraints
            });
        }
        meta.create_gate("native Pasta Poseidon BUS bridges", |meta| {
            let code = meta.query_fixed(bridge_mode, Rotation::cur());
            let bus_values: [_; WIDTH] =
                std::array::from_fn(|i| meta.query_advice(bus, Rotation(i as i32)));
            let mut constraints = Vec::with_capacity(2 * WIDTH * lane_count);
            for (lane, columns) in lanes.iter().enumerate() {
                for output in [false, true] {
                    let enabled = bridge_code(lane, output);
                    let mut numerator = Expression::Constant(F::ONE);
                    let mut denominator = F::ONE;
                    // Fixed code zero disables every bridge, including all blinding rows.
                    // Each degree-four indicator selects exactly one of the four endpoints.
                    for other in 0..=4 {
                        if other != enabled {
                            numerator =
                                numerator * (code.clone() - Expression::Constant(F::from(other)));
                            denominator *= F::from(enabled) - F::from(other);
                        }
                    }
                    let indicator = numerator * Expression::Constant(denominator.invert().unwrap());
                    let target = if output { ROUNDS } else { 0 };
                    let rotation = Rotation(target as i32 - bridge_start(lane, output) as i32);
                    for i in 0..WIDTH {
                        constraints.push(
                            indicator.clone()
                                * (bus_values[i].clone() - meta.query_advice(columns[i], rotation)),
                        );
                    }
                }
            }
            constraints
        });
        Self {
            lanes,
            bus,
            constants,
            round_mode,
            bridge_mode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativePoseidonMutation {
    Trace {
        job: usize,
        round: usize,
        column: usize,
    },
    Bus {
        job: usize,
        output: bool,
        column: usize,
    },
}

#[derive(Clone, Debug)]
struct PermutationJob<F: KagemushaPoseidonFieldV1> {
    input: [AssignedValue<F>; WIDTH],
    output: [AssignedValue<F>; WIDTH],
}

#[derive(Clone, Debug)]
struct HashInventory {
    input_count: usize,
    first_permutation: usize,
    permutations: usize,
}

/// Complete ordered raw-sponge jobs, with checked capacity before graph mutation.
#[derive(Clone, Debug)]
pub(super) struct PastaNativePoseidonJobsV1<F: KagemushaPoseidonFieldV1> {
    jobs: Vec<PermutationJob<F>>,
    hashes: Vec<HashInventory>,
    lane_count: usize,
    usable_rows: usize,
    spec: RawSpec<F>,
    use_unknown: bool,
    unfinished_transcript: bool,
}

/// One fully reserved stateful transcript. Dropping it without `finish` poisons its queue.
///
/// Challenge generation is infallible in the verifier trait. Each squeeze therefore retains
/// genuine permutation witnesses locally, including on an invalid call schedule; it never
/// returns a placeholder. Only exact completion commits the jobs and permits synthesis.
pub(super) struct PastaNativePoseidonTranscriptV1<'jobs, F: KagemushaPoseidonFieldV1> {
    queue: &'jobs mut PastaNativePoseidonJobsV1<F>,
    expected_inputs: Vec<usize>,
    expected_permutations: usize,
    observed_inputs: Vec<usize>,
    pending: Vec<AssignedValue<F>>,
    state: [AssignedValue<F>; WIDTH],
    jobs: Vec<PermutationJob<F>>,
    hashes: Vec<HashInventory>,
    witness_gen_only: bool,
    failed: bool,
}

impl<F: KagemushaPoseidonFieldV1> PastaNativePoseidonTranscriptV1<'_, F> {
    /// Absorb original assigned cells into the next fixed squeeze segment.
    pub(super) fn absorb(&mut self, inputs: &[AssignedValue<F>]) -> Result<(), String> {
        let expected = self
            .expected_inputs
            .get(self.observed_inputs.len())
            .copied();
        let count = self.pending.len().checked_add(inputs.len());
        if self.failed
            || expected.is_none()
            || count
                .zip(expected)
                .is_none_or(|(count, expected)| count > expected)
            || (!self.witness_gen_only && inputs.iter().any(|cell| cell.cell.is_none()))
        {
            self.failed = true;
            return Err("native Poseidon transcript absorption schedule is invalid".to_owned());
        }
        self.pending.extend_from_slice(inputs);
        Ok(())
    }

    /// Squeeze the genuine persistent state and retain every permutation it computes.
    ///
    /// A short or extra squeeze makes `finish` fail and leaves the containing queue invalid.
    /// It does not cause a panic, skip a permutation, or fall back to a different circuit.
    pub(super) fn squeeze(&mut self, ctx: &mut Context<F>, gate: &GateChip<F>) -> AssignedValue<F> {
        let mut padded = std::mem::take(&mut self.pending);
        let input_count = padded.len();
        if self.expected_inputs.get(self.observed_inputs.len()) != Some(&input_count) {
            self.failed = true;
        }
        self.observed_inputs.push(input_count);
        let first_permutation = self.jobs.len();
        padded.push(ctx.load_constant(F::ONE));
        if padded.len() % 2 != 0 {
            padded.push(ctx.load_constant(F::ZERO));
        }
        for chunk in padded.chunks_exact(2) {
            self.state[1] = gate.add(ctx, self.state[1], chunk[0]);
            self.state[2] = gate.add(ctx, self.state[2], chunk[1]);
            let value = self
                .queue
                .spec
                .permutation(self.state.map(|cell| *cell.value()));
            let output = value.map(|value| ctx.load_witness(value));
            self.jobs.push(PermutationJob {
                input: self.state,
                output,
            });
            self.state = output;
        }
        self.hashes.push(HashInventory {
            input_count,
            first_permutation,
            permutations: self.jobs.len() - first_permutation,
        });
        self.state[1]
    }

    /// Commit only the complete reserved transcript; return its final constrained state cell.
    pub(super) fn finish(mut self) -> Result<AssignedValue<F>, String> {
        if self.failed
            || !self.pending.is_empty()
            || self.observed_inputs != self.expected_inputs
            || self.jobs.len() != self.expected_permutations
            || !self.queue.unfinished_transcript
        {
            return Err("native Poseidon transcript reservation is incomplete".to_owned());
        }
        let offset = self.queue.jobs.len();
        let complete = offset.checked_add(self.jobs.len()).ok_or_else(|| {
            "native Poseidon transcript permutation inventory overflow".to_owned()
        })?;
        if required_rows(complete, self.queue.lane_count)? > self.queue.usable_rows {
            return Err("native Poseidon transcript reservation exceeds lane capacity".to_owned());
        }
        for hash in &mut self.hashes {
            hash.first_permutation = hash
                .first_permutation
                .checked_add(offset)
                .ok_or_else(|| "native Poseidon transcript hash inventory overflow".to_owned())?;
        }
        self.queue.jobs.extend(self.jobs);
        self.queue.hashes.extend(self.hashes);
        self.queue.unfinished_transcript = false;
        self.queue.validate_inventory()?;
        Ok(self.state[1])
    }
}

impl<F: KagemushaPoseidonFieldV1> PastaNativePoseidonJobsV1<F> {
    /// Reserve one or two existing k16 lanes without increasing the containing row envelope.
    pub(super) fn new(lane_count: usize, usable_rows: usize) -> Result<Self, String> {
        if !(1..=MAX_LANES).contains(&lane_count)
            || !(PERMUTATION_ROWS..=K16_USABLE_ROWS).contains(&usable_rows)
        {
            return Err("native Poseidon lane/row envelope is invalid".to_owned());
        }
        Ok(Self {
            jobs: Vec::new(),
            hashes: Vec::new(),
            lane_count,
            usable_rows,
            spec: RawSpec::new(),
            use_unknown: false,
            unfinished_transcript: false,
        })
    }

    /// Reserve a complete stateful squeeze schedule before allocating any transcript advice.
    ///
    /// The schedule contains the exact number of absorbed native fields at every squeeze.
    /// Holding the returned reservation exclusively borrows this queue. A failed or abandoned
    /// transcript keeps the queue invalid, so no assigned challenge can lose its native gates.
    pub(super) fn begin_transcript<'jobs>(
        &'jobs mut self,
        ctx: &mut Context<F>,
        expected_inputs: &[usize],
    ) -> Result<PastaNativePoseidonTranscriptV1<'jobs, F>, String> {
        self.validate_inventory()?;
        if expected_inputs.is_empty() {
            return Err("native Poseidon transcript has no squeeze schedule".to_owned());
        }
        let expected_permutations = expected_inputs.iter().try_fold(0_usize, |total, &count| {
            count
                .checked_div(2)
                .and_then(|count| count.checked_add(1))
                .and_then(|count| total.checked_add(count))
                .ok_or_else(|| "native Poseidon transcript schedule overflow".to_owned())
        })?;
        let complete = self
            .jobs
            .len()
            .checked_add(expected_permutations)
            .ok_or_else(|| {
                "native Poseidon transcript permutation inventory overflow".to_owned()
            })?;
        if required_rows(complete, self.lane_count)? > self.usable_rows {
            return Err("native Poseidon transcript reservation exceeds lane capacity".to_owned());
        }
        self.unfinished_transcript = true;
        Ok(PastaNativePoseidonTranscriptV1 {
            queue: self,
            expected_inputs: expected_inputs.to_vec(),
            expected_permutations,
            observed_inputs: Vec::with_capacity(expected_inputs.len()),
            pending: Vec::new(),
            state: [
                ctx.load_constant(F::from_u128(1_u128 << 64)),
                ctx.load_constant(F::ZERO),
                ctx.load_constant(F::ZERO),
            ],
            jobs: Vec::with_capacity(expected_permutations),
            hashes: Vec::with_capacity(expected_inputs.len()),
            witness_gen_only: ctx.witness_gen_only(),
            failed: false,
        })
    }

    /// Queue the exact existing raw input list and return its constrained second state cell.
    ///
    /// `fixed_prefix` is already present in `inputs`; it is checked and constrained in place.
    /// This method never adds a domain or arity prefix. Input length fixes the complete padding
    /// schedule. Every job and three-cell state bridge is retained in deterministic order.
    pub(super) fn queue_raw(
        &mut self,
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
        inputs: Vec<AssignedValue<F>>,
        fixed_prefix: &[F],
    ) -> Result<AssignedValue<F>, String> {
        if fixed_prefix.len() > inputs.len()
            || inputs
                .iter()
                .zip(fixed_prefix)
                .any(|(cell, value)| cell.value() != value)
            || (!ctx.witness_gen_only() && inputs.iter().any(|cell| cell.cell.is_none()))
        {
            return Err("native Poseidon raw input prefix or cell identity is invalid".to_owned());
        }
        self.validate_inventory()?;
        let permutations = inputs
            .len()
            .checked_div(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| "native Poseidon input length overflow".to_owned())?;
        let complete = self
            .jobs
            .len()
            .checked_add(permutations)
            .ok_or_else(|| "native Poseidon permutation inventory overflow".to_owned())?;
        if required_rows(complete, self.lane_count)? > self.usable_rows {
            return Err(
                "native Poseidon complete permutation inventory exceeds lane capacity".to_owned(),
            );
        }
        for (cell, value) in inputs.iter().zip(fixed_prefix) {
            gate.assert_is_const(ctx, cell, value);
        }
        let input_count = inputs.len();
        let first_permutation = self.jobs.len();
        let mut padded = inputs;
        padded.push(ctx.load_constant(F::ONE));
        if padded.len() % 2 != 0 {
            padded.push(ctx.load_constant(F::ZERO));
        }
        let mut state = [
            ctx.load_constant(F::from_u128(1_u128 << 64)),
            ctx.load_constant(F::ZERO),
            ctx.load_constant(F::ZERO),
        ];
        for chunk in padded.chunks_exact(2) {
            state[1] = gate.add(ctx, state[1], chunk[0]);
            state[2] = gate.add(ctx, state[2], chunk[1]);
            let value = self.spec.permutation(state.map(|cell| *cell.value()));
            let output = value.map(|value| ctx.load_witness(value));
            self.jobs.push(PermutationJob {
                input: state,
                output,
            });
            state = output;
        }
        self.hashes.push(HashInventory {
            input_count,
            first_permutation,
            permutations,
        });
        Ok(state[1])
    }

    fn validate_inventory(&self) -> Result<(), String> {
        if self.unfinished_transcript {
            return Err("native Poseidon transcript reservation is unfinished".to_owned());
        }
        let mut next = 0_usize;
        for hash in &self.hashes {
            if hash.first_permutation != next || hash.permutations != hash.input_count / 2 + 1 {
                return Err("native Poseidon hash length or ordering changed".to_owned());
            }
            next = next
                .checked_add(hash.permutations)
                .ok_or_else(|| "native Poseidon hash inventory overflow".to_owned())?;
        }
        if next != self.jobs.len() {
            return Err("native Poseidon complete permutation inventory changed".to_owned());
        }
        Ok(())
    }

    /// Exact maximum lane rows, including each final state row.
    pub(super) fn required_rows(&self) -> Result<usize, String> {
        self.validate_inventory()?;
        required_rows(self.jobs.len(), self.lane_count)
    }

    /// Preserve every assignment identity and schedule while hiding advice during keygen.
    pub(super) fn unknown(mut self) -> Self {
        self.use_unknown = true;
        self
    }

    /// Synthesize after Base so every permutation bridge uses its original physical cells.
    pub(super) fn synthesize(
        &self,
        config: &PastaNativePoseidonConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        usable_rows: usize,
    ) -> Result<(), Error> {
        self.synthesize_inner(
            config,
            layouter,
            copy_manager,
            witness_gen_only,
            usable_rows,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn synthesize_inner(
        &self,
        config: &PastaNativePoseidonConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        usable_rows: usize,
        mutation: Option<NativePoseidonMutation>,
    ) -> Result<(), Error> {
        let rows = self.required_rows().map_err(|_| Error::Synthesis)?;
        if config.lanes.len() != self.lane_count
            || usable_rows != self.usable_rows
            || rows > usable_rows
        {
            return Err(Error::Synthesis);
        }
        if rows == 0 {
            return Ok(());
        }
        let physical = if witness_gen_only {
            None
        } else {
            Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
        };
        layouter.assign_region(
            || "native Pasta exact raw Poseidon",
            |mut region| {
                for row in 0..rows {
                    let round = row % PERMUTATION_ROWS;
                    let active = round < ROUNDS;
                    for i in 0..WIDTH {
                        region.assign_fixed(
                            config.constants[i],
                            row,
                            if active {
                                self.spec.constants[round][i]
                            } else {
                                F::ZERO
                            },
                        );
                    }
                    region.assign_fixed(
                        config.round_mode,
                        row,
                        if !active {
                            F::ZERO
                        } else if full_round(round) {
                            F::from(2)
                        } else {
                            F::ONE
                        },
                    );
                    let mut code = 0;
                    let mut bus_slot = false;
                    for lane in 0..self.lane_count {
                        for output in [false, true] {
                            let start = bridge_start(lane, output);
                            if round == start {
                                code = bridge_code(lane, output);
                            }
                            bus_slot |= (start..start + WIDTH).contains(&round);
                        }
                    }
                    region.assign_fixed(config.bridge_mode, row, F::from(code));
                    if !bus_slot {
                        region.assign_advice_discarding_value(
                            config.bus,
                            row,
                            if self.use_unknown {
                                Value::unknown()
                            } else {
                                Value::known(F::ZERO)
                            },
                        );
                    }
                }
                for (lane, columns) in config.lanes.iter().enumerate() {
                    for block in 0..rows / PERMUTATION_ROWS {
                        let job_index = block * self.lane_count + lane;
                        let job = self.jobs.get(job_index);
                        // The final unused lane still has its complete round trace and BUS
                        // bridges under the shared fixed schedule, with no external copies.
                        let mut state =
                            job.map_or([F::ZERO; WIDTH], |job| job.input.map(|cell| *cell.value()));
                        for round in 0..=ROUNDS {
                            let row = block * PERMUTATION_ROWS + round;
                            for i in 0..WIDTH {
                                let mut value = state[i];
                                if mutation
                                    == Some(NativePoseidonMutation::Trace {
                                        job: job_index,
                                        round,
                                        column: i,
                                    })
                                {
                                    value += F::ONE;
                                }
                                region.assign_advice_discarding_value(
                                    columns[i],
                                    row,
                                    if self.use_unknown {
                                        Value::unknown()
                                    } else {
                                        Value::known(value)
                                    },
                                );
                                if round == 0 || round == ROUNDS {
                                    let output = round == ROUNDS;
                                    let mut value = state[i];
                                    if mutation
                                        == Some(NativePoseidonMutation::Bus {
                                            job: job_index,
                                            output,
                                            column: i,
                                        })
                                    {
                                        value += F::ONE;
                                    }
                                    let bus_row =
                                        block * PERMUTATION_ROWS + bridge_start(lane, output) + i;
                                    let assigned = region.assign_advice_discarding_value(
                                        config.bus,
                                        bus_row,
                                        if self.use_unknown {
                                            Value::unknown()
                                        } else {
                                            Value::known(value)
                                        },
                                    );
                                    if let (Some(job), Some(physical)) = (job, &physical) {
                                        let bridge =
                                            if output { job.output[i] } else { job.input[i] };
                                        let virtual_cell = bridge.cell.ok_or(Error::Synthesis)?;
                                        let target = physical
                                            .assigned_advices
                                            .resolve(&virtual_cell)
                                            .ok_or(Error::Synthesis)?;
                                        // Only BUS participates in equality. Its fixed bridge gate
                                        // binds the original Base cell to the local state column.
                                        region.constrain_equal(assigned, target);
                                    }
                                }
                            }
                            if round < ROUNDS {
                                state = self.spec.transition(state, round);
                            }
                        }
                    }
                }
                Ok(())
            },
        )
    }
}

fn required_rows(permutations: usize, lanes: usize) -> Result<usize, String> {
    if !(1..=MAX_LANES).contains(&lanes) {
        return Err("native Poseidon lane count is invalid".to_owned());
    }
    permutations
        .div_ceil(lanes)
        .checked_mul(PERMUTATION_ROWS)
        .ok_or_else(|| "native Poseidon row count overflow".to_owned())
}

#[cfg(test)]
#[path = "pasta_native_poseidon_transcript_tests.rs"]
mod transcript_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::pasta_cycle_loader::pasta_poseidon_domain_elements_v1;
    use halo2_base::{
        gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
        halo2_proofs::{
            circuit::V1,
            dev::MockProver,
            halo2curves::pasta::{Fp, Fq},
            plonk::Circuit,
        },
    };
    use snark_verifier::{loader::native::NativeLoader, util::hash::Poseidon};

    const TEST_K: usize = 12;
    const TEST_ROWS: usize = (1 << TEST_K) - 9;
    const DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-hash-claim-deferred-batch";

    fn reference<F: KagemushaPoseidonFieldV1>(input: &[F]) -> F {
        let mut old = Poseidon::<F, F, WIDTH, 2>::from_spec(
            &NativeLoader,
            F::kagemusha_poseidon_spec_v1().clone(),
        );
        old.update(input);
        old.squeeze()
    }

    fn raw_reference<F: KagemushaPoseidonFieldV1>(input: &[F]) -> F {
        let spec = RawSpec::<F>::new();
        let mut padded = input.to_vec();
        padded.push(F::ONE);
        if padded.len() % 2 != 0 {
            padded.push(F::ZERO);
        }
        let mut state = [F::from_u128(1_u128 << 64), F::ZERO, F::ZERO];
        for chunk in padded.chunks_exact(2) {
            state[1] += chunk[0];
            state[2] += chunk[1];
            state = spec.permutation(state);
        }
        state[1]
    }

    fn vectors<F: KagemushaPoseidonFieldV1>() -> Vec<(Vec<F>, Vec<F>)> {
        let mut vectors = [0, 1, 2, 3, 8, 31]
            .into_iter()
            .map(|length| {
                (
                    (0..length)
                        .map(|i| F::from((i + 1) as u64))
                        .collect::<Vec<_>>(),
                    vec![],
                )
            })
            .collect::<Vec<_>>();
        let prefix = pasta_poseidon_domain_elements_v1::<F>(DOMAIN, 1);
        assert_eq!(prefix.len(), 6);
        for length in [0, 1, 2, 3] {
            let mut input = prefix.clone();
            input.extend((0..length).map(|i| F::from((i + 100) as u64)));
            vectors.push((input, prefix.clone()));
        }
        vectors
    }

    fn assert_raw_equivalence<F: KagemushaPoseidonFieldV1>() {
        for (input, _) in vectors::<F>() {
            assert_eq!(raw_reference(&input), reference(&input));
        }
        let mut input = vec![-F::ONE, F::ZERO, F::ONE];
        assert_eq!(raw_reference(&input), reference(&input));
        input.reverse();
        assert_eq!(raw_reference(&input), reference(&input));
    }

    fn assert_old_base_and_new_circuit<F: KagemushaPoseidonFieldV1>() {
        use halo2_base::poseidon::hasher::PoseidonHasher;

        let mut base = BaseCircuitBuilder::<F>::new(false).use_k(TEST_K);
        let gate = GateChip::<F>::default();
        let mut old = PoseidonHasher::new(F::kagemusha_poseidon_spec_v1().clone());
        old.initialize_consts(base.main(0), &gate);
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).expect("fixture capacity");
        for (input, prefix) in vectors::<F>().into_iter().take(4) {
            let assigned = input
                .iter()
                .map(|value| base.main(0).load_witness(*value))
                .collect::<Vec<_>>();
            let old_output = old.hash_fix_len_array(base.main(0), &gate, &assigned);
            let new_output = jobs
                .queue_raw(base.main(0), &gate, assigned, &prefix)
                .expect("new exact raw sponge");
            base.main(0).constrain_equal(&old_output, &new_output);
            gate.assert_is_const(base.main(0), &new_output, &reference(&input));
        }
        base.calculate_params(Some(9));
        let circuit = TestCircuit {
            base,
            jobs,
            mutation: None,
        };
        MockProver::run(TEST_K as u32, &circuit, vec![])
            .expect("old Base and new native sponge synthesis")
            .assert_satisfied();
    }

    #[derive(Clone, Debug)]
    struct TestConfig<F: KagemushaPoseidonFieldV1> {
        base: BaseConfig<F>,
        poseidon: PastaNativePoseidonConfigV1,
    }
    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        base: BaseCircuitBuilder<F>,
        jobs: PastaNativePoseidonJobsV1<F>,
        mutation: Option<NativePoseidonMutation>,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = TestConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;
        fn params(&self) -> Self::Params {
            self.base.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                base: self.base.deep_clone().unknown(true),
                jobs: self.jobs.clone().unknown(),
                mutation: None,
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("parameterized fixture")
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(TEST_ROWS);
            TestConfig {
                base,
                poseidon: PastaNativePoseidonConfigV1::configure::<F>(meta, 2),
            }
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.base.reset_synthesis_state();
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.base,
                config.base,
                layouter.namespace(|| "Base Poseidon bridges"),
            )?;
            self.jobs.synthesize_inner(
                &config.poseidon,
                &mut layouter,
                &self.base.core().copy_manager,
                self.base.witness_gen_only(),
                TEST_ROWS,
                self.mutation,
            )
        }
    }

    fn fixture<F: KagemushaPoseidonFieldV1>(vectors: &[(Vec<F>, Vec<F>)]) -> TestCircuit<F> {
        let mut base = BaseCircuitBuilder::<F>::new(false).use_k(TEST_K);
        let gate = GateChip::<F>::default();
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).expect("fixture capacity");
        for (input, prefix) in vectors {
            let assigned = input
                .iter()
                .map(|value| base.main(0).load_witness(*value))
                .collect::<Vec<_>>();
            let output = jobs
                .queue_raw(base.main(0), &gate, assigned, prefix)
                .expect("exact raw hash job");
            gate.assert_is_const(base.main(0), &output, &reference(input));
        }
        base.calculate_params(Some(9));
        TestCircuit {
            base,
            jobs,
            mutation: None,
        }
    }

    fn assert_circuit_bindings<F: KagemushaPoseidonFieldV1>() {
        let circuit = fixture(&vectors::<F>());
        MockProver::run(TEST_K as u32, &circuit, vec![])
            .expect("both-lane Poseidon fixture")
            .assert_satisfied();
        let unknown = circuit.without_witnesses();
        assert_eq!(unknown.jobs.required_rows(), circuit.jobs.required_rows());
        assert_eq!(unknown.jobs.jobs.len(), circuit.jobs.jobs.len());
        for (left, right) in unknown.jobs.jobs.iter().zip(&circuit.jobs.jobs) {
            assert_eq!(
                left.input.map(|cell| cell.cell),
                right.input.map(|cell| cell.cell)
            );
            assert_eq!(
                left.output.map(|cell| cell.cell),
                right.output.map(|cell| cell.cell)
            );
        }
        // Input, full-round, partial-round, final-state and second-lane mutations must fail.
        for mutation in [(0, 0, 0), (0, 1, 1), (0, 32, 0), (0, ROUNDS, 1), (1, 32, 2)] {
            let mut changed = fixture(&[(vec![F::ONE, F::from(2)], vec![])]);
            changed.mutation = Some(NativePoseidonMutation::Trace {
                job: mutation.0,
                round: mutation.1,
                column: mutation.2,
            });
            assert!(
                MockProver::run(TEST_K as u32, &changed, vec![])
                    .expect("mutated trace synthesis")
                    .verify()
                    .is_err()
            );
        }
        let mut missing = fixture(&[(vec![F::ONE], vec![])]);
        missing.jobs.jobs.pop();
        assert!(missing.jobs.required_rows().is_err());
        let mut length = fixture(&[(vec![F::ONE], vec![])]);
        length.jobs.hashes[0].input_count += 2;
        assert!(length.jobs.required_rows().is_err());
        let mut missing_identity = fixture(&[(vec![F::ONE], vec![])]);
        missing_identity.jobs.jobs[0].input[1].cell = None;
        assert!(MockProver::run(TEST_K as u32, &missing_identity, vec![]).is_err());
    }

    fn assert_bus_bridge_bindings<F: KagemushaPoseidonFieldV1>() {
        use halo2_base::halo2_proofs::dev::VerifyFailure;

        for lane in 0..2 {
            for output in [false, true] {
                for column in 0..WIDTH {
                    let mut changed = fixture(&[(vec![F::ONE, F::from(2)], vec![])]);
                    assert_eq!(changed.jobs.jobs.len(), 2);
                    changed.mutation = Some(NativePoseidonMutation::Bus {
                        job: lane,
                        output,
                        column,
                    });
                    let failures = MockProver::run(TEST_K as u32, &changed, vec![])
                        .expect("mutated BUS synthesis")
                        .verify()
                        .expect_err("every BUS endpoint is bound to its local state");
                    assert!(failures.iter().any(|failure| matches!(failure,
                        VerifyFailure::ConstraintNotSatisfied { constraint, .. }
                            if constraint.to_string().contains("native Pasta Poseidon BUS bridges")
                    )), "BUS mutation must fail its bridge gate, not only the Base copy");

                    // Each endpoint must also retain the original Base identity. In particular,
                    // output copies cannot disappear when local state no longer has equality.
                    let mut missing = fixture(&[(vec![F::ONE, F::from(2)], vec![])]);
                    let job = &mut missing.jobs.jobs[lane];
                    if output {
                        job.output[column].cell = None;
                    } else {
                        job.input[column].cell = None;
                    }
                    assert!(MockProver::run(TEST_K as u32, &missing, vec![]).is_err());
                }
            }
        }
    }

    #[test]
    fn fp_native_poseidon_shared_bus_binds_all_twelve_endpoints() {
        assert_bus_bridge_bindings::<Fp>();
    }

    #[test]
    fn fq_native_poseidon_shared_bus_binds_all_twelve_endpoints() {
        assert_bus_bridge_bindings::<Fq>();
    }

    #[test]
    fn native_poseidon_shared_bus_last_block_and_unused_lane_remain_constrained() {
        use halo2_base::halo2_proofs::dev::VerifyFailure;

        fn check<F: KagemushaPoseidonFieldV1>() {
            // 121 and 122 permutations occupy the last complete block below the unchanged
            // k12 usable-row ceiling. The odd case requires a complete second-lane dummy.
            for length in [240, 242] {
                let input = (0..length)
                    .map(|i| F::from((i + 1) as u64))
                    .collect::<Vec<_>>();
                let circuit = fixture(&[(input, vec![])]);
                assert_eq!(circuit.jobs.jobs.len(), length / 2 + 1);
                assert_eq!(circuit.jobs.required_rows(), Ok(4_026));
                MockProver::run(TEST_K as u32, &circuit, vec![])
                    .expect("last complete lane block synthesis")
                    .assert_satisfied();
                if length == 240 {
                    let mut changed = circuit.clone();
                    assert_eq!(changed.jobs.jobs.len() % 2, 1);
                    changed.mutation = Some(NativePoseidonMutation::Bus {
                        job: 121,
                        output: true,
                        column: 2,
                    });
                    let failures = MockProver::run(TEST_K as u32, &changed, vec![])
                        .expect("mutated unused-lane BUS synthesis")
                        .verify()
                        .expect_err("unused lane still follows shared fixed bridges");
                    assert!(failures.iter().any(|failure| matches!(failure,
                        VerifyFailure::ConstraintNotSatisfied { constraint, .. }
                            if constraint.to_string().contains("native Pasta Poseidon BUS bridges")
                    )));
                    changed.mutation = Some(NativePoseidonMutation::Trace {
                        job: 121,
                        round: 32,
                        column: 0,
                    });
                    assert!(
                        MockProver::run(TEST_K as u32, &changed, vec![])
                            .expect("mutated unused-lane round synthesis")
                            .verify()
                            .is_err()
                    );
                }
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    fn assert_prefix_capacity_and_padding<F: KagemushaPoseidonFieldV1>() {
        let gate = GateChip::<F>::default();
        let mut base = BaseCircuitBuilder::<F>::new(false).use_k(TEST_K);
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).expect("fixture capacity");
        let input = vec![base.main(0).load_witness(F::ONE)];
        let before = base.main(0).advice_len();
        assert!(
            jobs.queue_raw(base.main(0), &gate, input, &[F::from(2)])
                .is_err()
        );
        assert_eq!(base.main(0).advice_len(), before);
        assert_eq!(jobs.required_rows(), Ok(0));
        let too_long = vec![base.main(0).load_witness(F::ONE); 244];
        let before = base.main(0).advice_len();
        assert!(jobs.queue_raw(base.main(0), &gate, too_long, &[]).is_err());
        assert_eq!(base.main(0).advice_len(), before);
        assert_eq!(jobs.required_rows(), Ok(0));
        for length in [0, 1, 2, 3] {
            let input = (0..length)
                .map(|i| F::from((i + 1) as u64))
                .collect::<Vec<_>>();
            let padded_wrong = input.iter().copied().chain([F::ZERO]).collect::<Vec<_>>();
            assert_ne!(reference(&input), reference(&padded_wrong));
            let mut circuit = fixture(&[(input.clone(), vec![])]);
            let wrong = circuit.base.main(0).load_constant(reference(&padded_wrong));
            let result = circuit.jobs.jobs.last().expect("final permutation").output[1];
            circuit.base.main(0).constrain_equal(&result, &wrong);
            circuit.base.calculate_params(Some(9));
            assert!(
                MockProver::run(TEST_K as u32, &circuit, vec![])
                    .expect("wrong-padding digest fixture")
                    .verify()
                    .is_err()
            );
        }
    }

    #[test]
    fn fp_raw_generator_matches_existing_optimized_poseidon() {
        assert_raw_equivalence::<Fp>();
    }
    #[test]
    fn fq_raw_generator_matches_existing_optimized_poseidon() {
        assert_raw_equivalence::<Fq>();
    }
    #[test]
    fn fp_native_poseidon_matches_existing_base_circuit() {
        assert_old_base_and_new_circuit::<Fp>();
    }
    #[test]
    fn fq_native_poseidon_matches_existing_base_circuit() {
        assert_old_base_and_new_circuit::<Fq>();
    }
    #[test]
    fn fp_native_poseidon_circuit_binds_every_state_and_lane() {
        assert_circuit_bindings::<Fp>();
    }
    #[test]
    fn fq_native_poseidon_circuit_binds_every_state_and_lane() {
        assert_circuit_bindings::<Fq>();
    }
    #[test]
    fn fp_native_poseidon_prefix_padding_length_and_capacity_are_bound() {
        assert_prefix_capacity_and_padding::<Fp>();
    }
    #[test]
    fn fq_native_poseidon_prefix_padding_length_and_capacity_are_bound() {
        assert_prefix_capacity_and_padding::<Fq>();
    }
    #[test]
    fn native_poseidon_inventory_keeps_the_existing_source_ceiling() {
        assert_eq!(required_rows(1_984, 2), Ok(65_472));
        assert!(required_rows(1_985, 2).expect("checked rows") > K16_USABLE_ROWS);
        assert_eq!(required_rows(39 + 1_008 + 937, 2), Ok(65_472));
        assert_eq!(required_rows(39 + 1_008 + 7, 2), Ok(34_782));
        assert!(required_rows(usize::MAX, 2).is_err());
    }

    #[test]
    fn native_poseidon_shared_bus_geometry_and_degree_are_fixed_in_both_fields() {
        fn check<F: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>>() {
            let mut cs = ConstraintSystem::<F>::default();
            let config = PastaNativePoseidonConfigV1::configure::<F>(&mut cs, 2);
            assert_eq!(config.lanes.len(), 2);
            assert_eq!(cs.num_advice_columns(), 7);
            assert_eq!(cs.num_fixed_columns(), 5);
            assert_eq!(cs.permutation().get_columns().len(), 1);
            assert_eq!(cs.num_selectors(), 0);
            assert!(cs.lookups().is_empty());
            assert_eq!(cs.degree(), 7);
            assert_eq!(cs.blinding_factors(), 6);
            assert_eq!(cs.minimum_rows(), 9);
            assert_eq!(
                cs.permutation().get_columns(),
                vec![Column::<halo2_base::halo2_proofs::plonk::Any>::from(
                    config.bus
                )]
            );
            let rotations = |column| {
                let mut rotations = cs
                    .advice_queries()
                    .iter()
                    .filter_map(|(actual, rotation)| (*actual == column).then_some(rotation.0))
                    .collect::<Vec<_>>();
                rotations.sort_unstable();
                rotations
            };
            assert_eq!(rotations(config.bus), vec![0, 1, 2]);
            for column in config.lanes[0] {
                assert_eq!(rotations(column), vec![0, 1, 5]);
            }
            for column in config.lanes[1] {
                assert_eq!(rotations(column), vec![-3, 0, 1, 2]);
            }
            let old_domain = halo2_base::halo2_proofs::poly::EvaluationDomain::<F>::new(6, 16);
            let new_domain = halo2_base::halo2_proofs::poly::EvaluationDomain::<F>::new(7, 16);
            assert_eq!(old_domain.extended_k(), 19);
            assert_eq!(new_domain.extended_k(), 19);
            assert_eq!(old_domain.get_quotient_poly_degree(), 5);
            assert_eq!(new_domain.get_quotient_poly_degree(), 6);
            let mut single = ConstraintSystem::<F>::default();
            let single_config = PastaNativePoseidonConfigV1::configure::<F>(&mut single, 1);
            assert_eq!(single_config.lanes.len(), 1);
            assert_eq!(single.num_advice_columns(), 4);
            assert_eq!(single.num_fixed_columns(), 5);
            assert_eq!(
                single.permutation().get_columns(),
                vec![Column::<halo2_base::halo2_proofs::plonk::Any>::from(
                    single_config.bus
                )]
            );
            assert_eq!(single.degree(), 7);
            assert_eq!(single.blinding_factors(), 5);
            assert_eq!(single.minimum_rows(), 8);
            assert_eq!(required_rows(992, 1), Ok(65_472));
            assert!(required_rows(993, 1).unwrap() > K16_USABLE_ROWS);
        }
        check::<Fp>();
        check::<Fq>();
    }

    fn public_fixture<F: KagemushaPoseidonFieldV1>(
        witness_gen_only: bool,
        offset: u64,
    ) -> (TestCircuit<F>, Vec<F>) {
        let mut base = BaseCircuitBuilder::<F>::new(witness_gen_only)
            .use_k(TEST_K)
            .use_instance_columns(1);
        let gate = GateChip::<F>::default();
        let mut jobs = PastaNativePoseidonJobsV1::new(2, TEST_ROWS).unwrap();
        let prefix = pasta_poseidon_domain_elements_v1::<F>(DOMAIN, 1);
        let mut public = Vec::new();
        for length in [4, 6] {
            let mut input = prefix.clone();
            input.extend((0..length).map(|i| F::from(offset + i + 1)));
            let assigned = input
                .iter()
                .map(|value| base.main(0).load_witness(*value))
                .collect::<Vec<_>>();
            let output = jobs
                .queue_raw(base.main(0), &gate, assigned.clone(), &prefix)
                .expect("public raw Poseidon job");
            base.assigned_instances[0].extend(assigned);
            base.assigned_instances[0].push(output);
            public.extend(input.iter().copied());
            public.push(reference(&input));
        }
        base.calculate_params(Some(9));
        (
            TestCircuit {
                base,
                jobs,
                mutation: None,
            },
            public,
        )
    }

    #[test]
    fn native_poseidon_real_proofs_use_checked_keys_and_fixed_witness_schedule() {
        use ff::Field;
        use halo2_base::halo2_proofs::poly::{VerificationStrategy, commitment::ParamsProver as _};
        use halo2_base::halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::{EpAffine, EqAffine},
            plonk::{ProvingKey, VerifyingKey, keygen_pk2, keygen_vk_custom},
            poly::ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
            transcript::{
                Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer,
                TranscriptWriterBuffer,
            },
        };

        macro_rules! check {
            ($curve:ty, $field:ty, $compressed:expr) => {{
                let params = ParamsIPA::<$curve>::new(TEST_K as u32);
                let (circuit, _) = public_fixture::<$field>(false, 100);
                let circuit_params = circuit.params();
                assert_eq!(circuit.jobs.jobs.len() % 2, 1, "last lane block is unused");
                let pk = keygen_pk2(&params, &circuit, $compressed)
                    .expect("native Poseidon key generation");
                let break_points = circuit.base.break_points();
                let pk_bytes = pk.to_bytes(SerdeFormat::Processed);
                let vk_bytes = pk.get_vk().to_bytes(SerdeFormat::Processed);
                let unknown = circuit.without_witnesses();
                let unknown_vk = keygen_vk_custom(&params, &unknown, $compressed)
                    .expect("unknown native Poseidon key generation");
                assert_eq!(
                    unknown_vk.to_bytes(SerdeFormat::Processed),
                    vk_bytes,
                    "witness values must not alter fixed schedule or copy mapping"
                );
                let mut input = pk_bytes.as_slice();
                let restored_pk = ProvingKey::<$curve>::read_checked::<_, TestCircuit<$field>>(
                    &mut input,
                    SerdeFormat::Processed,
                    TEST_K as u32,
                    circuit_params.clone(),
                )
                .expect("checked native Poseidon PK reload");
                assert!(input.is_empty());
                assert_eq!(restored_pk.to_bytes(SerdeFormat::Processed), pk_bytes);
                let mut input = vk_bytes.as_slice();
                let restored_vk = VerifyingKey::<$curve>::read_checked::<_, TestCircuit<$field>>(
                    &mut input,
                    SerdeFormat::Processed,
                    TEST_K as u32,
                    circuit_params.clone(),
                )
                .expect("checked native Poseidon VK reload");
                assert!(input.is_empty());
                assert_eq!(restored_vk.to_bytes(SerdeFormat::Processed), vk_bytes);
                let cs = restored_vk.cs();
                assert_eq!(cs.degree(), 7);
                assert_eq!(cs.blinding_factors(), 6);
                let protocol = snark_verifier::system::halo2::compile(
                    &params,
                    &restored_vk,
                    snark_verifier::system::halo2::Config::ipa()
                        .with_num_instance(vec![circuit.base.assigned_instances[0].len()]),
                );
                assert_eq!(protocol.quotient.num_chunk(), 6);
                assert_eq!(
                    protocol.preprocessed.len(),
                    cs.num_fixed_columns() + cs.permutation().get_columns().len()
                );
                assert_eq!(
                    protocol.num_witness.iter().sum::<usize>(),
                    cs.num_advice_columns()
                        + 3 * cs.lookups().len()
                        + cs.permutation().get_columns().len().div_ceil(5)
                        + 1
                );
                // Build different witnesses using the same PK. The production witness-only path
                // relies on that PK's fixed permutation mapping and retains the complete trace.
                let (mut witness, public) = public_fixture::<$field>(true, 200);
                assert_eq!(witness.jobs.required_rows(), circuit.jobs.required_rows());
                assert_eq!(
                    witness.base.config_params.num_advice_per_phase,
                    circuit_params.num_advice_per_phase
                );
                witness.base.set_params(circuit_params);
                witness.base.set_break_points(break_points);
                let mut transcript =
                    Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                let columns: [&[$field]; 1] = [&public];
                halo2_base::halo2_proofs::plonk::create_proof::<
                    IPACommitmentScheme<$curve>,
                    ProverIPA<'_, $curve>,
                    _,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &restored_pk,
                    &[witness],
                    &[&columns],
                    rand_core_06::OsRng,
                    &mut transcript,
                )
                .expect("genuine native Poseidon proof");
                let proof = transcript.finalize();
                let verify = |public: &[$field], proof: &[u8]| {
                    let columns: [&[$field]; 1] = [public];
                    let mut transcript =
                        Blake2bRead::<_, $curve, Challenge255<$curve>>::init(proof);
                    halo2_base::halo2_proofs::plonk::verify_proof::<
                        IPACommitmentScheme<$curve>,
                        VerifierIPA<'_, $curve>,
                        _,
                        _,
                        _,
                    >(
                        &params,
                        &restored_vk,
                        SingleStrategy::<$curve>::new(&params),
                        &[&columns],
                        &mut transcript,
                    )
                };
                verify(&public, &proof).expect("checked native Poseidon proof verification");
                let mut changed_input = public.clone();
                changed_input[6] += <$field>::ONE;
                assert!(
                    verify(&changed_input, &proof).is_err(),
                    "changed input must fail"
                );
                let mut changed_output = public.clone();
                *changed_output.last_mut().unwrap() += <$field>::ONE;
                assert!(
                    verify(&changed_output, &proof).is_err(),
                    "changed digest must fail"
                );
                let mut corrupt = proof.clone();
                corrupt[0] ^= 1;
                assert!(
                    verify(&public, &corrupt).is_err(),
                    "corrupt proof must fail"
                );
                println!(
                    "native Poseidon {} compressed={} PK={} VK={} proof={}",
                    stringify!($curve),
                    $compressed,
                    pk_bytes.len(),
                    vk_bytes.len(),
                    proof.len()
                );
            }};
        }
        check!(EqAffine, Fp, false);
        check!(EqAffine, Fp, true);
        check!(EpAffine, Fq, false);
        check!(EpAffine, Fq, true);
    }
}
