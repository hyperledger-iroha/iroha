//! Bounded carrier-RLC row generation with the original arithmetic and assignment schedule.
//!
//! The row, two physical halves, running integers, and fixed-size quotient-pack buffer have
//! owned cleanup guards. No complete logical-row vector is created by this production path.
//! Caller witness owners, arithmetic temporaries/copies, backend storage, and allocator/RSS
//! behavior remain outside these guards. TODO: qualify the complete Claim producer lifetime
//! and full proof on the unchanged production memory/time/device gates.

use super::*;
use halo2_base::ContextCell;
use zeroize::Zeroize;

const MAX_PACKS: usize = KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);

/// Nonsecret identity of the existing BUS equality target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Binding {
    /// The original virtual cell; absence fails when constraint copies are required.
    Virtual(Option<ContextCell>),
    /// Store one completed ternary quotient pack for its later BUS copy.
    PackStore { carrier: usize, pack: usize },
    /// Load the corresponding completed ternary quotient pack.
    PackLoad { carrier: usize, pack: usize },
}

/// Plain scalar payload cleared by the non-Copy streamed-row owner.
///
/// No enum or AssignedValue payload shares this storage. The default explicitly
/// overwrites every initialized field, independently of binding variant layout.
#[derive(Clone, Copy)]
pub(super) struct RowValues<F: KagemushaPoseidonFieldV1> {
    /// Logical arithmetic fields in the unchanged frozen-oracle index order.
    pub(super) values: [F; CLAIM_RLC_COLUMNS],
    /// Fixed schedule power for a preprocess row, zero in every other mode.
    pub(super) ternary_power: F,
}

impl<F: KagemushaPoseidonFieldV1> Default for RowValues<F> {
    fn default() -> Self {
        Self {
            values: [F::ZERO; CLAIM_RLC_COLUMNS],
            ternary_power: F::ZERO,
        }
    }
}

impl<F: KagemushaPoseidonFieldV1> zeroize::DefaultIsZeroes for RowValues<F> {}

/// One emitted logical record, owning scalar cleanup and only cell/copy metadata.
pub(super) struct StreamRow<F: KagemushaPoseidonFieldV1> {
    payload: RowValues<F>,
    /// The unchanged arithmetic/boundary opcode.
    pub(super) mode: ClaimRlcRowModeV1,
    /// An EvaluateB row that completes a quotient pack.
    pub(super) store_pack: bool,
    /// An EvaluateA row that evaluates a previously stored quotient pack.
    pub(super) load_pack: bool,
    /// Only virtual-cell identity or pack coordinates; never an assigned scalar.
    pub(super) binding: Option<Binding>,
}

impl<F: KagemushaPoseidonFieldV1> std::ops::Deref for StreamRow<F> {
    type Target = RowValues<F>;
    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

impl<F: KagemushaPoseidonFieldV1> std::ops::DerefMut for StreamRow<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.payload
    }
}

impl<F: KagemushaPoseidonFieldV1> StreamRow<F> {
    fn new(state: ClaimRlcStateV1, mode: ClaimRlcRowModeV1, binding: Option<Binding>) -> Self {
        #[cfg(test)]
        LIVE_ROWS.with(|live| {
            live.set(live.get() + 1);
            CLEARS.with(|counts| {
                let mut record = counts.get();
                record[5] = record[5].max(live.get());
                counts.set(record);
            });
        });
        let mut row = Self {
            payload: RowValues::default(),
            mode,
            store_pack: false,
            load_pack: false,
            binding,
        };
        row.values[CLAIM_RLC_CHALLENGE_A] = F::from_u128(state.challenge_a);
        row.values[CLAIM_RLC_CHALLENGE_B] = F::from_u128(state.challenge_b);
        row.values[CLAIM_RLC_ACCUMULATOR_A] = F::from_u128(state.accumulator_a);
        row.values[CLAIM_RLC_ACCUMULATOR_B] = F::from_u128(state.accumulator_b);
        row.values[CLAIM_RLC_QUOTIENT_PACK] = F::from_u128(state.quotient_pack);
        row.values[CLAIM_RLC_COEFFICIENT] = F::from_u128(state.coefficient);
        row
    }

    /// The same three fixed-column values as the retained vector encoder.
    pub(super) fn fixed_encoding(&self) -> Result<[F; 3], PlonkError> {
        if self.store_pack && !matches!(self.mode, ClaimRlcRowModeV1::EvaluateB)
            || self.load_pack && !matches!(self.mode, ClaimRlcRowModeV1::EvaluateA)
            || !matches!(self.mode, ClaimRlcRowModeV1::Preprocess) && self.ternary_power != F::ZERO
        {
            return Err(PlonkError::Synthesis);
        }
        Ok(match self.mode {
            ClaimRlcRowModeV1::StartA => [F::ZERO, F::ONE, F::ZERO],
            ClaimRlcRowModeV1::StartB => [F::ZERO, F::ONE, F::ONE],
            ClaimRlcRowModeV1::Preprocess => {
                if self.ternary_power == F::ZERO {
                    return Err(PlonkError::Synthesis);
                }
                [F::ONE, F::ZERO, self.ternary_power]
            }
            ClaimRlcRowModeV1::EvaluateA => [
                F::ONE,
                F::ONE,
                if self.load_pack { F::ONE } else { F::ZERO },
            ],
            ClaimRlcRowModeV1::EvaluateB => [
                F::ONE,
                F::ONE,
                if self.store_pack {
                    F::ZERO - F::ONE
                } else {
                    F::from(2)
                },
            ],
            ClaimRlcRowModeV1::EndA => [F::ZERO, F::ONE, F::from(2)],
            ClaimRlcRowModeV1::EndB => [F::ZERO, F::ONE, F::from(3)],
        })
    }

    fn set_range_limbs(&mut self, first: u128, second: u128) -> Result<(), PlonkError> {
        let first_top_bits = match self.mode {
            ClaimRlcRowModeV1::Preprocess => 8,
            ClaimRlcRowModeV1::EvaluateA | ClaimRlcRowModeV1::EvaluateB => 6,
            _ => return Err(PlonkError::Synthesis),
        };
        self.values[CLAIM_RLC_SCALED_FIRST_TOP] =
            F::from_u128((first >> 120) << (CLAIM_RLC_RADIX_BITS - first_top_bits));
        self.values[CLAIM_RLC_SCALED_SECOND_TOP] =
            F::from_u128((second >> 120) << (CLAIM_RLC_RADIX_BITS - 7));
        for (half, mut value) in [first, second].into_iter().enumerate() {
            for limb in 0..9 {
                self.values[CLAIM_RLC_RANGE_START + half * 9 + limb] =
                    F::from_u128(value & (CLAIM_RLC_RADIX - 1));
                value >>= CLAIM_RLC_RADIX_BITS;
            }
            debug_assert_eq!(value, 0);
        }
        Ok(())
    }

    fn project_into(
        &self,
        values: &mut [[F; CLAIM_RLC_PHYSICAL_COLUMNS]; CLAIM_RLC_ROWS_PER_LOGICAL_ROW],
    ) {
        for (logical, physical, half) in CLAIM_RLC_STATE_LAYOUT {
            values[half][physical] = self.values[logical];
        }
        for (half, values) in values.iter_mut().enumerate() {
            values[CLAIM_RLC_PHYSICAL_STATE_COLUMNS..CLAIM_RLC_PHYSICAL_COLUMNS - 1]
                .copy_from_slice(
                    &self.values
                        [CLAIM_RLC_RANGE_START + 9 * half..CLAIM_RLC_RANGE_START + 9 * (half + 1)],
                );
            values[CLAIM_RLC_PHYSICAL_COLUMNS - 1] = self.values[CLAIM_RLC_SCALED_FIRST_TOP + half];
        }
    }

    /// Copy the physical projection solely for frozen-oracle test comparisons.
    /// Production instead projects directly into its owned physical cleanup guard.
    #[cfg(test)]
    pub(super) fn physical_values(
        &self,
    ) -> [[F; CLAIM_RLC_PHYSICAL_COLUMNS]; CLAIM_RLC_ROWS_PER_LOGICAL_ROW] {
        let mut values = [[F::ZERO; CLAIM_RLC_PHYSICAL_COLUMNS]; CLAIM_RLC_ROWS_PER_LOGICAL_ROW];
        self.project_into(&mut values);
        values
    }
}

impl<F: KagemushaPoseidonFieldV1> Drop for StreamRow<F> {
    fn drop(&mut self) {
        self.payload.zeroize();
        #[cfg(test)]
        {
            record_clear(
                0,
                self.ternary_power == F::ZERO && self.values.iter().all(|value| *value == F::ZERO),
            );
            LIVE_ROWS.with(|live| live.set(live.get() - 1));
        }
    }
}

struct GuardedState(ClaimRlcStateV1);

impl Drop for GuardedState {
    fn drop(&mut self) {
        self.0.challenge_a.zeroize();
        self.0.challenge_b.zeroize();
        self.0.accumulator_a.zeroize();
        self.0.accumulator_b.zeroize();
        self.0.quotient_pack.zeroize();
        self.0.coefficient.zeroize();
        #[cfg(test)]
        record_clear(
            1,
            [
                self.0.challenge_a,
                self.0.challenge_b,
                self.0.accumulator_a,
                self.0.accumulator_b,
                self.0.quotient_pack,
                self.0.coefficient,
            ]
            .iter()
            .all(|value| *value == 0),
        );
    }
}

struct GuardedPacks {
    values: [u128; MAX_PACKS],
    len: usize,
    ternary_power: u128,
}

impl GuardedPacks {
    fn new() -> Self {
        Self {
            values: [0; MAX_PACKS],
            len: 0,
            ternary_power: 1,
        }
    }

    fn push(&mut self, value: u128) -> Result<usize, PlonkError> {
        let index = self.len;
        *self.values.get_mut(index).ok_or(PlonkError::Synthesis)? = value;
        self.len += 1;
        Ok(index)
    }
}

impl Drop for GuardedPacks {
    fn drop(&mut self) {
        self.values.zeroize();
        self.ternary_power.zeroize();
        #[cfg(test)]
        record_clear(
            2,
            self.ternary_power == 0 && self.values.iter().all(|value| *value == 0),
        );
    }
}

#[derive(Clone, Copy)]
struct PhysicalValues<F: KagemushaPoseidonFieldV1> {
    values: [[F; CLAIM_RLC_PHYSICAL_COLUMNS]; CLAIM_RLC_ROWS_PER_LOGICAL_ROW],
    fixed: [F; 3],
}

impl<F: KagemushaPoseidonFieldV1> Default for PhysicalValues<F> {
    fn default() -> Self {
        Self {
            values: [[F::ZERO; CLAIM_RLC_PHYSICAL_COLUMNS]; CLAIM_RLC_ROWS_PER_LOGICAL_ROW],
            fixed: [F::ZERO; 3],
        }
    }
}

impl<F: KagemushaPoseidonFieldV1> zeroize::DefaultIsZeroes for PhysicalValues<F> {}

struct GuardedPhysical<F: KagemushaPoseidonFieldV1>(PhysicalValues<F>);

impl<F: KagemushaPoseidonFieldV1> GuardedPhysical<F> {
    fn new(row: &StreamRow<F>) -> Result<Self, PlonkError> {
        let mut physical = Self(PhysicalValues::default());
        physical.0.fixed = row.fixed_encoding()?;
        row.project_into(&mut physical.0.values);
        Ok(physical)
    }
}

impl<F: KagemushaPoseidonFieldV1> Drop for GuardedPhysical<F> {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        record_clear(
            3,
            self.0
                .values
                .iter()
                .flatten()
                .chain(self.0.fixed.iter())
                .all(|value| *value == F::ZERO),
        );
    }
}

struct Emitter<'sink, F, S>
where
    F: KagemushaPoseidonFieldV1,
    S: FnMut(usize, &StreamRow<F>) -> Result<(), PlonkError>,
{
    sink: &'sink mut S,
    next: usize,
    expected: usize,
    marker: std::marker::PhantomData<F>,
}

impl<F, S> Emitter<'_, F, S>
where
    F: KagemushaPoseidonFieldV1,
    S: FnMut(usize, &StreamRow<F>) -> Result<(), PlonkError>,
{
    fn row(&mut self, row: StreamRow<F>) -> Result<(), PlonkError> {
        if self.next >= self.expected {
            return Err(PlonkError::Synthesis);
        }
        (self.sink)(self.next, &row)?;
        self.next = self.next.checked_add(1).ok_or(PlonkError::Synthesis)?;
        // The row guard is destroyed before the caller constructs its next logical record.
        Ok(())
    }

    fn evaluations(
        &mut self,
        state: &mut ClaimRlcStateV1,
        pack_load: Option<(usize, usize)>,
        pack_store: Option<(usize, usize)>,
    ) -> Result<(), PlonkError> {
        let (quotient_a, remainder_a) =
            claim_rlc_native_step_v1(state.accumulator_a, state.challenge_a, state.coefficient)
                .map_err(|_| PlonkError::Synthesis)?;
        let binding = pack_load.map(|(carrier, pack)| Binding::PackLoad { carrier, pack });
        let mut evaluate_a = StreamRow::new(*state, ClaimRlcRowModeV1::EvaluateA, binding);
        evaluate_a.values[CLAIM_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_a);
        evaluate_a.values[CLAIM_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_a);
        evaluate_a.values[CLAIM_RLC_REMAINDER_INVERSE] =
            claim_rlc_non_modulus_inverse_v1::<F>(remainder_a)
                .map_err(|_| PlonkError::Synthesis)?;
        if pack_load.is_some() {
            evaluate_a.load_pack = true;
            evaluate_a.values[CLAIM_RLC_BUS] = F::from_u128(state.coefficient);
        }
        evaluate_a.set_range_limbs(quotient_a, remainder_a)?;
        self.row(evaluate_a)?;
        state.accumulator_a = remainder_a;

        let (quotient_b, remainder_b) =
            claim_rlc_native_step_v1(state.accumulator_b, state.challenge_b, state.coefficient)
                .map_err(|_| PlonkError::Synthesis)?;
        let mut evaluate_b = StreamRow::new(*state, ClaimRlcRowModeV1::EvaluateB, None);
        evaluate_b.values[CLAIM_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_b);
        evaluate_b.values[CLAIM_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_b);
        evaluate_b.values[CLAIM_RLC_REMAINDER_INVERSE] =
            claim_rlc_non_modulus_inverse_v1::<F>(remainder_b)
                .map_err(|_| PlonkError::Synthesis)?;
        evaluate_b.set_range_limbs(quotient_b, remainder_b)?;
        // The frozen emitter patches its just-pushed EvaluateB row with these fields. The
        // same pack boundary is known here before dispatch, so no old row must be retained.
        if let Some((carrier, pack)) = pack_store {
            evaluate_b.store_pack = true;
            evaluate_b.values[CLAIM_RLC_BUS] = F::from_u128(state.quotient_pack);
            evaluate_b.binding = Some(Binding::PackStore { carrier, pack });
        }
        self.row(evaluate_b)?;
        state.accumulator_b = remainder_b;
        Ok(())
    }
}

/// Emit the existing complete schedule, releasing each guarded row before its successor.
///
/// The production caller always supplies the unchanged capacity 4090. Smaller capacities are
/// used only by existing fixed-schedule tests; the buffer retains at most 52 quotient packs.
/// A sink failure or unwind drops all live row/state/pack guards and stops the successful prefix.
pub(super) fn emit_rows_with_capacity<F: KagemushaPoseidonFieldV1>(
    machine: &KagemushaClaimCarrierRlcMachineV1<F>,
    fixed_capacity: usize,
    mut sink: impl FnMut(usize, &StreamRow<F>) -> Result<(), PlonkError>,
) -> Result<usize, PlonkError> {
    if fixed_capacity == 0 || fixed_capacity > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 {
        return Err(PlonkError::Synthesis);
    }
    let challenge_a = if machine.use_unknown {
        1
    } else {
        assigned_u128_cell_v1(machine.challenge_a, "claim RLC challenge A")
            .map_err(|_| PlonkError::Synthesis)?
    };
    let challenge_b = if machine.use_unknown {
        1
    } else {
        assigned_u128_cell_v1(machine.challenge_b, "claim RLC challenge B")
            .map_err(|_| PlonkError::Synthesis)?
    };
    if challenge_a == 0
        || challenge_b == 0
        || challenge_a > (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1)
        || challenge_b > (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1)
    {
        return Err(PlonkError::Synthesis);
    }
    let physical_rows = machine
        .required_rows_with_capacity(fixed_capacity)
        .map_err(|_| PlonkError::Synthesis)?;
    let expected = physical_rows / CLAIM_RLC_ROWS_PER_LOGICAL_ROW;
    let mut emit = Emitter {
        sink: &mut sink,
        next: 0,
        expected,
        marker: std::marker::PhantomData,
    };
    for (carrier_index, carrier) in machine.carriers.iter().enumerate() {
        let mut state = GuardedState(ClaimRlcStateV1 {
            challenge_a,
            challenge_b,
            accumulator_a: 0,
            accumulator_b: 0,
            quotient_pack: 0,
            coefficient: 0,
        });
        let mut packs = GuardedPacks::new();
        let mut start_a = StreamRow::new(
            state.0,
            ClaimRlcRowModeV1::StartA,
            Some(Binding::Virtual(machine.challenge_a.cell)),
        );
        start_a.values[CLAIM_RLC_BUS] = F::from_u128(challenge_a);
        emit.row(start_a)?;
        let mut start_b = StreamRow::new(
            state.0,
            ClaimRlcRowModeV1::StartB,
            Some(Binding::Virtual(machine.challenge_b.cell)),
        );
        start_b.values[CLAIM_RLC_BUS] = F::from_u128(challenge_b);
        emit.row(start_b)?;
        for (value_index, assigned) in carrier.values.iter().copied().enumerate() {
            let value = if machine.use_unknown {
                0
            } else {
                assigned_u128_cell_v1(assigned, "claim RLC carrier value")
                    .map_err(|_| PlonkError::Synthesis)?
            };
            let quotient = value / CLAIM_CARRIER_RLC_MODULUS_V1;
            let remainder = value % CLAIM_CARRIER_RLC_MODULUS_V1;
            if quotient >= CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1 {
                return Err(PlonkError::Synthesis);
            }
            let mut preprocess = StreamRow::new(
                state.0,
                ClaimRlcRowModeV1::Preprocess,
                Some(Binding::Virtual(assigned.cell)),
            );
            preprocess.values[CLAIM_RLC_BUS] = F::from_u128(value);
            preprocess.values[CLAIM_RLC_VALUE] = F::from_u128(value);
            preprocess.values[CLAIM_RLC_QUOTIENT_BIT_0] = F::from_u128(quotient & 1);
            preprocess.values[CLAIM_RLC_QUOTIENT_BIT_1] = F::from_u128(quotient >> 1);
            preprocess.values[CLAIM_RLC_RAW_REMAINDER] = F::from_u128(remainder);
            preprocess.values[CLAIM_RLC_REMAINDER_INVERSE] =
                claim_rlc_non_modulus_inverse_v1::<F>(remainder)
                    .map_err(|_| PlonkError::Synthesis)?;
            preprocess.ternary_power = F::from_u128(packs.ternary_power);
            preprocess.set_range_limbs(value, remainder)?;
            emit.row(preprocess)?;
            state.0.quotient_pack = state
                .0
                .quotient_pack
                .checked_add(
                    quotient
                        .checked_mul(packs.ternary_power)
                        .ok_or(PlonkError::Synthesis)?,
                )
                .ok_or(PlonkError::Synthesis)?;
            state.0.coefficient = remainder;
            let pack_end = (value_index + 1) % CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1 == 0
                || value_index + 1 == carrier.values.len();
            let pack_store = pack_end.then_some((carrier_index, packs.len));
            emit.evaluations(&mut state.0, None, pack_store)?;
            if pack_end {
                packs.push(state.0.quotient_pack)?;
                state.0.quotient_pack = 0;
                packs.ternary_power = 1;
            } else {
                packs.ternary_power = packs
                    .ternary_power
                    .checked_mul(CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1)
                    .ok_or(PlonkError::Synthesis)?;
            }
        }
        for pack_index in 0..packs.len {
            state.0.coefficient = packs.values[pack_index];
            emit.evaluations(&mut state.0, Some((carrier_index, pack_index)), None)?;
        }
        if packs.len != fixed_capacity.div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
            return Err(PlonkError::Synthesis);
        }
        let expected_a = if machine.use_unknown {
            state.0.accumulator_a
        } else {
            assigned_u128_cell_v1(carrier.expected_a, "claim RLC expected A")
                .map_err(|_| PlonkError::Synthesis)?
        };
        let expected_b = if machine.use_unknown {
            state.0.accumulator_b
        } else {
            assigned_u128_cell_v1(carrier.expected_b, "claim RLC expected B")
                .map_err(|_| PlonkError::Synthesis)?
        };
        if expected_a >= CLAIM_CARRIER_RLC_MODULUS_V1 || expected_b >= CLAIM_CARRIER_RLC_MODULUS_V1
        {
            return Err(PlonkError::Synthesis);
        }
        let mut end_a = StreamRow::new(
            state.0,
            ClaimRlcRowModeV1::EndA,
            Some(Binding::Virtual(carrier.expected_a.cell)),
        );
        end_a.values[CLAIM_RLC_BUS] = F::from_u128(expected_a);
        emit.row(end_a)?;
        let mut end_b = StreamRow::new(
            state.0,
            ClaimRlcRowModeV1::EndB,
            Some(Binding::Virtual(carrier.expected_b.cell)),
        );
        end_b.values[CLAIM_RLC_BUS] = F::from_u128(expected_b);
        emit.row(end_b)?;
    }
    if emit.next != expected {
        return Err(PlonkError::Synthesis);
    }
    Ok(emit.next)
}

/// Assign a complete streamed schedule after the caller loads the unchanged fixed range table.
/// This preserves both half-row coordinates and the final sorted pack copy schedule.
pub(super) fn synthesize_with_capacity<F: KagemushaPoseidonFieldV1>(
    machine: &KagemushaClaimCarrierRlcMachineV1<F>,
    config: &KagemushaClaimCarrierRlcConfigV1,
    layouter: &mut impl Layouter<F>,
    copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
    witness_gen_only: bool,
    fixed_capacity: usize,
) -> Result<(), PlonkError> {
    let physical_cells = if witness_gen_only {
        None
    } else {
        Some(copy_manager.lock().map_err(|_| PlonkError::Synthesis)?)
    };
    layouter.assign_region(
        || "Kagemusha claim carrier fixed-row RLC",
        |mut region| {
            let mut pack_stores = std::collections::BTreeMap::<(usize, usize), Cell>::new();
            let mut pack_loads = std::collections::BTreeMap::<(usize, usize), Cell>::new();
            emit_rows_with_capacity(machine, fixed_capacity, |logical_row, row| {
                let physical_start = logical_row
                    .checked_mul(CLAIM_RLC_ROWS_PER_LOGICAL_ROW)
                    .ok_or(PlonkError::Synthesis)?;
                let physical = GuardedPhysical::new(row)?;
                let mut bus = None;
                for (half, values) in physical.0.values.iter().enumerate() {
                    let row_index = physical_start
                        .checked_add(half)
                        .ok_or(PlonkError::Synthesis)?;
                    let fixed = if half == 0 {
                        physical.0.fixed
                    } else {
                        [F::ZERO; 3]
                    };
                    for (position, column) in [config.mode_bit_0, config.mode_bit_1, config.payload]
                        .into_iter()
                        .enumerate()
                    {
                        region.assign_fixed(column, row_index, fixed[position]);
                    }
                    for (column_index, column) in config.advice.iter().copied().enumerate() {
                        let value = if machine.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(values[column_index])
                        };
                        let assigned =
                            region.assign_advice_discarding_value(column, row_index, value);
                        if half == 0 && column_index == 0 {
                            bus = Some(assigned);
                        }
                    }
                }
                let bus = bus.ok_or(PlonkError::Synthesis)?;
                if let Some(binding) = row.binding {
                    match binding {
                        Binding::Virtual(virtual_cell) => {
                            if let Some(physical_cells) = &physical_cells {
                                let virtual_cell = virtual_cell.ok_or(PlonkError::Synthesis)?;
                                let physical = physical_cells
                                    .assigned_advices
                                    .resolve(&virtual_cell)
                                    .ok_or(PlonkError::Synthesis)?;
                                region.constrain_equal(bus, physical);
                            }
                        }
                        Binding::PackStore { carrier, pack } => {
                            if pack_stores.insert((carrier, pack), bus).is_some() {
                                return Err(PlonkError::Synthesis);
                            }
                        }
                        Binding::PackLoad { carrier, pack } => {
                            if pack_loads.insert((carrier, pack), bus).is_some() {
                                return Err(PlonkError::Synthesis);
                            }
                        }
                    }
                }
                Ok(())
            })?;
            if pack_stores.len() != pack_loads.len() {
                return Err(PlonkError::Synthesis);
            }
            for (key, stored) in pack_stores {
                let loaded = pack_loads.remove(&key).ok_or(PlonkError::Synthesis)?;
                region.constrain_equal(stored, loaded);
            }
            if !pack_loads.is_empty() {
                return Err(PlonkError::Synthesis);
            }
            Ok(())
        },
    )
}

#[cfg(test)]
std::thread_local! {
    // Row/state/pack/physical clears, nonzero post-wipe observations, maximum live rows.
    static CLEARS: std::cell::Cell<[usize; 6]> = const { std::cell::Cell::new([0; 6]) };
    static LIVE_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_clear(kind: usize, zero: bool) {
    CLEARS.with(|counts| {
        let mut record = counts.get();
        record[kind] += 1;
        record[4] += usize::from(!zero);
        counts.set(record);
    });
}

/// Reset only thread-local nonsecret cleanup counters for the focused regression tests.
#[cfg(test)]
pub(super) fn reset_cleanup_counts() {
    CLEARS.with(|counts| counts.set([0; 6]));
    LIVE_ROWS.with(|live| assert_eq!(live.get(), 0));
}

/// Read drop/zero-observation counters without reading freed memory or retaining secrets.
#[cfg(test)]
pub(super) fn cleanup_counts() -> [usize; 6] {
    CLEARS.with(std::cell::Cell::get)
}

/// Hold public nonzero fields in the actual physical guard during a test closure.
///
/// The closure can succeed, return an error, or unwind. It receives no witness
/// references; cleanup counters can inspect the guard only after its drop.
#[cfg(test)]
pub(super) fn with_physical_guard_for_test<F: KagemushaPoseidonFieldV1>(
    run: impl FnOnce() -> Result<(), PlonkError>,
) -> Result<(), PlonkError> {
    let mut physical = GuardedPhysical(PhysicalValues::<F>::default());
    physical.0.values.fill([F::ONE; CLAIM_RLC_PHYSICAL_COLUMNS]);
    physical.0.fixed.fill(F::ONE);
    let result = run();
    drop(physical);
    result
}
