//! Opt-in real key/proof equivalence across an actual numerator codec rollover.
//!
//! This synthetic BN256 fixture preserves the original physical assignment
//! schedule and is separate from the default Pasta-field codec unit tests.

use crate::{
    COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN, QuantumCell,
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    halo2_proofs::{
        SerdeFormat,
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::bn256::Fr,
        plonk::{Assigned, Circuit, ConstraintSystem, Error, keygen_pk, keygen_vk},
        poly::kzg::commitment::ParamsKZG,
    },
    utils::testing::{check_proof, gen_proof},
};
use crate::{
    ContextCell, FIRST_PHASE_CELL_TYPE_ID,
    gates::flex_gate::{BasicGateConfig, ThreadBreakPoints},
    halo2_proofs::circuit::{Region, Value},
    utils::{
        ScalarField,
        halo2::{raw_assign_advice_cell, raw_constrain_equal},
    },
    virtual_region::{
        copy_constraints::{CopyConstraintManager, SharedCopyConstraintManager},
        manager::VirtualRegionManager,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use std::cell::RefCell;

const ROLLOVER_K: u32 = 16;
const ROLLOVER_ADVICE_LEN: usize = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN + 32;

fn rollover_params() -> BaseCircuitParams {
    BaseCircuitParams {
        k: ROLLOVER_K as usize,
        num_advice_per_phase: vec![2],
        num_fixed: 0,
        num_lookup_advice_per_phase: vec![0],
        lookup_bits: None,
        num_instance_columns: 0,
    }
}

// This reference owns exact uncompressed Assigned variants, independently
// of CompactAdvice. Its read facades exist only so the physical assignment
// oracle below retains the production schedule verbatim.
#[derive(Clone)]
struct RawAssignedTrace<F: ScalarField> {
    values: Vec<Assigned<F>>,
    selector: Vec<bool>,
    type_id: &'static str,
    context_id: usize,
    equalities: Vec<(ContextCell, ContextCell)>,
}

impl<F: ScalarField> RawAssignedTrace<F> {
    fn advice_len(&self) -> usize {
        self.values.len()
    }

    fn advice_values(&self) -> impl ExactSizeIterator<Item = Assigned<F>> + '_ {
        self.values.iter().copied()
    }
}

fn rollover_trace() -> RawAssignedTrace<Fr> {
    assert_eq!(ROLLOVER_ADVICE_LEN % 4, 0);
    let cases = [
        Assigned::Zero,
        Assigned::Trivial(Fr::from(0)),
        Assigned::Trivial(Fr::from(1)),
        Assigned::Trivial(Fr::from(2)),
        Assigned::Rational(Fr::from(0), Fr::from(3)),
        Assigned::Rational(Fr::from(1), Fr::from(4)),
        Assigned::Rational(Fr::from(6), Fr::from(2)),
        Assigned::Rational(Fr::from(11), Fr::from(0)),
    ];
    let mut trace = RawAssignedTrace {
        values: Vec::with_capacity(ROLLOVER_ADVICE_LEN),
        selector: Vec::with_capacity(ROLLOVER_ADVICE_LEN),
        type_id: FIRST_PHASE_CELL_TYPE_ID,
        context_id: 0,
        equalities: Vec::with_capacity(ROLLOVER_ADVICE_LEN / 4),
    };
    for block in 0..ROLLOVER_ADVICE_LEN / 4 {
        let value = cases[block % cases.len()];
        // Every four rows constrain value + 0 * 1 = value. Keeping both
        // copies exact also exercises x/0 -> 0 during real assignment.
        trace
            .values
            .extend([value, Assigned::Zero, Assigned::Trivial(Fr::from(1)), value]);
        trace.selector.extend([true, false, false, false]);
        trace.equalities.push((
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, block * 4),
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, block * 4 + 3),
        ));
    }
    trace
}

fn codec_rollover_circuit(trace: &RawAssignedTrace<Fr>) -> BaseCircuitBuilder<Fr> {
    let mut circuit = BaseCircuitBuilder::<Fr>::new(false).use_params(rollover_params());
    let ctx = circuit.main(0);
    for values in trace.values.chunks_exact(4) {
        ctx.assign_region(
            values.iter().copied().map(QuantumCell::WitnessFraction),
            [0],
        );
        let left = ctx.get(-4);
        let right = ctx.get(-1);
        ctx.constrain_equal(&left, &right);
    }
    assert_eq!(ctx.advice_len(), ROLLOVER_ADVICE_LEN);
    assert_eq!(ctx.advice_numerator_segment_count(), 2);
    assert_eq!(ctx.advice_numerator_storage().segment_counts, [1, 1]);
    assert!(
        ctx.advice_storage_capacities()[0] < 2 * COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN,
        "the real full-segment rollover must select compressed numerator storage"
    );
    assert_eq!(
        ctx.selector.iter().copied().collect::<Vec<_>>(),
        trace.selector
    );
    for (offset, (actual, expected)) in ctx.advice_values().zip(&trace.values).enumerate() {
        assert_exact_assigned(actual, *expected, offset);
        let historical = ctx.get(offset as isize);
        assert_exact_assigned(historical.value, *expected, offset);
        assert_eq!(
            historical.cell,
            Some(ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, offset))
        );
    }
    assert_eq!(
        ctx.copy_manager
            .lock()
            .expect("codec copy manager")
            .advice_equalities,
        trace.equalities
    );
    circuit
}

fn assert_exact_assigned(actual: Assigned<Fr>, expected: Assigned<Fr>, offset: usize) {
    // Assigned's field-level equality may normalize variants. Require the
    // same variant and exact numerator/denominator in this storage oracle.
    match (actual, expected) {
        (Assigned::Zero, Assigned::Zero) => {}
        (Assigned::Trivial(actual), Assigned::Trivial(expected)) => {
            assert_eq!(actual, expected, "trivial numerator at {offset}");
        }
        (Assigned::Rational(an, ad), Assigned::Rational(en, ed)) => {
            assert_eq!(an, en, "rational numerator at {offset}");
            assert_eq!(ad, ed, "rational denominator at {offset}");
        }
        (actual, expected) => {
            panic!("exact Assigned variant changed at {offset}: {actual:?} != {expected:?}")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssignmentReceipt {
    break_points: ThreadBreakPoints,
    coordinates: Vec<(usize, usize)>,
    equalities: Vec<(ContextCell, ContextCell)>,
}

fn assignment_receipt(
    manager: &CopyConstraintManager<Fr>,
    break_points: ThreadBreakPoints,
) -> AssignmentReceipt {
    assert_eq!(manager.assigned_advices.len(), ROLLOVER_ADVICE_LEN);
    AssignmentReceipt {
        break_points,
        coordinates: (0..ROLLOVER_ADVICE_LEN)
            .map(|offset| {
                let cell = manager
                    .assigned_advices
                    .resolve(&ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, offset))
                    .expect("every raw or codec virtual advice cell must be assigned");
                (cell.column.index(), cell.row_offset)
            })
            .collect(),
        equalities: manager.advice_equalities.clone(),
    }
}

// The function body is copied verbatim from assign_with_constraints at the
// reviewed source revision. Only the function name and input trace type
// differ. Compare every physical coordinate, breakpoint and canonical copy
// edge below, then compare complete key bytes and deterministic proof bytes.
fn assign_raw_assigned_reference_with_constraints<F: ScalarField, const ROTATIONS: usize>(
    threads: &[RawAssignedTrace<F>],
    basic_gates: &[BasicGateConfig<F>],
    region: &mut Region<F>,
    copy_manager: &mut CopyConstraintManager<F>,
    max_rows: usize,
    use_unknown: bool,
) -> ThreadBreakPoints {
    let mut break_points = vec![];
    let mut gate_index = 0;
    let mut row_offset = 0;
    for ctx in threads {
        if ctx.advice_len() == 0 {
            continue;
        }
        let mut basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
        assert_eq!(ctx.selector.len(), ctx.advice_len());

        for (i, (advice, &q)) in ctx.advice_values().zip(ctx.selector.iter()).enumerate() {
            let column = basic_gate.value;
            let value = if use_unknown {
                Value::unknown()
            } else {
                Value::known(advice)
            };
            let cell = raw_assign_advice_cell(region, column, row_offset, value);
            if let Some(old_cell) = copy_manager
                .assigned_advices
                .insert(ContextCell::new(ctx.type_id, ctx.context_id, i), cell)
            {
                assert!(
                    old_cell.row_offset == cell.row_offset && old_cell.column == cell.column,
                    "Trying to overwrite virtual cell with a different raw cell"
                );
            }

            // If selector enabled and row_offset is valid add break point, account for break point overlap, and enforce equality constraint for gate outputs.
            // ⚠️ This assumes overlap is of form: gate enabled at `i - delta` and `i`, where `delta = ROTATIONS - 1`. We currently do not support `delta < ROTATIONS - 1`.
            if (q && row_offset + ROTATIONS > max_rows) || row_offset >= max_rows - 1 {
                break_points.push(row_offset);
                row_offset = 0;
                gate_index += 1;

                // safety check: make sure selector is not enabled on `i - delta` for `0 < delta < ROTATIONS - 1`
                if ROTATIONS > 1 && i + 2 >= ROTATIONS {
                    for delta in 1..ROTATIONS - 1 {
                        assert!(
                            !ctx.selector[i - delta],
                            "We do not support overlaps with delta = {delta}"
                        );
                    }
                }
                // when there is a break point, because we may have two gates that overlap at the current cell, we must copy the current cell to the next column for safety
                basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
                let column = basic_gate.value;
                let ncell = raw_assign_advice_cell(region, column, row_offset, value);
                raw_constrain_equal(region, ncell, cell);
            }

            if q {
                basic_gate
                    .q_enable
                    .enable(region, row_offset)
                    .expect("enable selector should not fail");
            }

            row_offset += 1;
        }
    }
    break_points
}

struct RawAssignedReferenceCircuit {
    trace: RawAssignedTrace<Fr>,
    use_unknown: bool,
    receipt: RefCell<Option<AssignmentReceipt>>,
}

impl Circuit<Fr> for RawAssignedReferenceCircuit {
    type Config = BaseConfig<Fr>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        rollover_params()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            trace: self.trace.clone(),
            use_unknown: true,
            receipt: RefCell::new(None),
        }
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<Fr>,
        params: Self::Params,
    ) -> Self::Config {
        BaseConfig::configure(meta, params)
    }

    fn configure(_: &mut ConstraintSystem<Fr>) -> Self::Config {
        unreachable!("raw reference uses explicit circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        let copy_manager = SharedCopyConstraintManager::<Fr>::default();
        {
            let mut manager = copy_manager.lock().expect("raw reference copy manager");
            for &edge in &self.trace.equalities {
                manager.push_advice_equality(edge);
            }
        }
        let mut break_points = Vec::new();
        layouter.assign_region(
            || "raw Assigned physical reference",
            |mut region| {
                break_points = assign_raw_assigned_reference_with_constraints::<Fr, 4>(
                    std::slice::from_ref(&self.trace),
                    &config.gate().basic_gates[0],
                    &mut region,
                    &mut copy_manager.lock().expect("raw reference copy manager"),
                    config.gate().max_rows,
                    self.use_unknown,
                );
                copy_manager.assign_raw(config.constants(), &mut region);
                Ok(())
            },
        )?;
        let receipt = assignment_receipt(
            &copy_manager.lock().expect("raw reference copy manager"),
            break_points,
        );
        let mut previous = self.receipt.borrow_mut();
        if let Some(previous) = previous.as_ref() {
            assert_eq!(
                previous, &receipt,
                "raw schedule must survive repeated synthesis"
            );
        }
        *previous = Some(receipt);
        Ok(())
    }
}

#[test]
#[ignore = "opt-in k=16 KZG key/proof equivalence over the actual 65,536-cell codec rollover"]
fn numerator_codec_rollover_preserves_raw_assignment_keys_and_real_proofs() {
    let trace = rollover_trace();
    let codec = codec_rollover_circuit(&trace);
    let raw = RawAssignedReferenceCircuit {
        trace,
        use_unknown: false,
        receipt: RefCell::new(None),
    };
    let params = ParamsKZG::setup(ROLLOVER_K, StdRng::seed_from_u64(0x4e554d455241544f));
    let codec_vk = keygen_vk(&params, &codec).expect("codec verifying key");
    let raw_vk = keygen_vk(&params, &raw).expect("raw Assigned verifying key");
    assert_eq!(
        codec_vk.to_bytes(SerdeFormat::Processed),
        raw_vk.to_bytes(SerdeFormat::Processed),
        "numerator storage must preserve the complete verifying key"
    );
    let codec_pk = keygen_pk(&params, codec_vk, &codec).expect("codec proving key");
    let raw_pk = keygen_pk(&params, raw_vk, &raw).expect("raw Assigned proving key");
    assert_eq!(
        codec_pk.to_bytes(SerdeFormat::Processed),
        raw_pk.to_bytes(SerdeFormat::Processed),
        "numerator storage must preserve the complete proving key"
    );
    let codec_break_points = codec.break_points();
    assert_eq!(codec_break_points.len(), 1);
    assert_eq!(
        codec_break_points[0].len(),
        1,
        "fixture must cross physical advice columns"
    );
    let codec_receipt = assignment_receipt(
        &codec
            .core()
            .copy_manager
            .lock()
            .expect("codec copy manager"),
        codec_break_points[0].clone(),
    );
    assert_eq!(
        raw.receipt.borrow().as_ref(),
        Some(&codec_receipt),
        "raw and codec must preserve every physical coordinate and copy edge"
    );
    // gen_proof uses the same fixed StdRng seed for both circuits. Byte
    // equality therefore checks exact prover behavior, not just acceptance.
    let codec_proof = gen_proof(&params, &codec_pk, codec);
    let raw_proof = gen_proof(&params, &raw_pk, raw);
    assert_eq!(
        codec_proof, raw_proof,
        "same witnesses must produce the same proof bytes"
    );
    check_proof(&params, raw_pk.get_vk(), &codec_proof, true);
    check_proof(&params, codec_pk.get_vk(), &raw_proof, true);
}
