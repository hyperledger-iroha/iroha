//! Actual assignment and proof identity against the copied pre-cutover HashMap assigner.

use super::coordinate;
use crate::{
    gates::flex_gate::{
        threads::single_phase::assign_with_constraints, BasicGateConfig, ThreadBreakPoints,
    },
    halo2_proofs::{
        circuit::{Cell, Layouter, Region, SimpleFloorPlanner, Value},
        dev::MockProver,
        halo2curves::bn256::{Bn256, Fr},
        plonk::{keygen_pk, keygen_vk, Circuit, Column, ConstraintSystem, Error, Fixed, Instance},
        poly::kzg::commitment::ParamsKZG,
        SerdeFormat,
    },
    utils::{
        halo2::{constrain_virtual_equals_external, raw_assign_fixed, raw_constrain_equal},
        testing::{check_proof_with_instances, gen_proof_with_instances},
        ScalarField,
    },
    virtual_region::{
        copy_constraints::{
            SharedCopyConstraintManager, EXTERNAL_CELL_TYPE_ID,
        },
        manager::VirtualRegionManager,
    },
    AssignedValue, Context, ContextCell, QuantumCell, FIRST_PHASE_CELL_TYPE_ID,
};
use rand::{rngs::StdRng, SeedableRng};
use rayon::prelude::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    ops::DerefMut,
    sync::{Arc, Mutex},
};

// Copied independently from the pinned preimage. Only the function name and
// physical-map parameter/access are renamed; row packing and overlap logic stay verbatim.
fn legacy_assign_with_constraints<F: ScalarField, const ROTATIONS: usize>(
    threads: &[Context<F>],
    basic_gates: &[BasicGateConfig<F>],
    region: &mut Region<F>,
    physical: &mut HashMap<ContextCell, Cell>,
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
            #[cfg(feature = "halo2-axiom")]
            let cell = region.assign_advice(column, row_offset, value).cell();
            #[cfg(not(feature = "halo2-axiom"))]
            let cell = region
                .assign_advice(|| "", column, row_offset, || value)
                .unwrap()
                .cell();
            if let Some(old_cell) =
                physical.insert(ContextCell::new(ctx.type_id, ctx.context_id, i), cell)
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
                #[cfg(feature = "halo2-axiom")]
                let ncell = region.assign_advice(column, row_offset, value);
                #[cfg(not(feature = "halo2-axiom"))]
                let ncell = region
                    .assign_advice(|| "", column, row_offset, || value)
                    .unwrap();
                raw_constrain_equal(region, ncell.cell(), cell);
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

// Copied original copy-manager assignment body; its HashMap resolution remains independent.
fn legacy_copy_constraints(
    manager: &SharedCopyConstraintManager<Fr>,
    config: &[Column<Fixed>],
    region: &mut Region<Fr>,
    physical: &HashMap<ContextCell, Cell>,
) {
    let mut guard = manager.lock().unwrap();
    let manager = guard.deref_mut();
    // BTreeMap iteration sorts constants deterministically. Sorting every
    // complete cell bucket reproduces the former flat
    // `(constant, ContextCell)` comparator exactly, including duplicates.
    manager.constant_equalities.canonicalize_cells();
    // Assign fixed cells, we go left to right, then top to bottom, to avoid needing to know number of rows here
    let mut fixed_col = 0;
    let mut fixed_offset = 0;
    for constant in manager.constant_equalities.constants() {
        // this will panic if you run out of rows
        let cell = raw_assign_fixed(region, config[fixed_col], fixed_offset, *constant);
        manager.assigned_constants.insert(*constant, cell);
        fixed_col += 1;
        if fixed_col >= config.len() {
            fixed_col = 0;
            fixed_offset += 1;
        }
    }

    // Just in case: we sort by ContextCell because the backend implementation of `raw_constrain_equal` (permutation argument) seems to depend on the order you specify copy constraints...
    manager.advice_equalities.par_sort_unstable();
    // Impose equality constraints between assigned advice cells
    // At this point we assume all cells have been assigned by other VirtualRegionManagers
    for (left, right) in &manager.advice_equalities {
        let left = physical.get(left).expect("virtual cell not assigned");
        let right = physical.get(right).expect("virtual cell not assigned");
        raw_constrain_equal(region, *left, *right);
    }
    for (constant, cells) in manager.constant_equalities.buckets() {
        let left = manager.assigned_constants[constant];
        for right in cells {
            let right = physical.get(right).expect("virtual cell not assigned");
            raw_constrain_equal(region, left, *right);
        }
    }
    // We can't clear advice_equalities and constant_equalities because keygen_vk and keygen_pk will call this function twice
    let _ = manager.assigned.set(());
    // When keygen_vk and keygen_pk are both run, you need to clear assigned constants
    // so the second run still assigns constants in the pk
    manager.assigned_constants.clear();
}

// Original Entry-based bridge, retained only as the independent reference.
fn legacy_bridge(
    region: &mut Region<Fr>,
    virtual_cell: AssignedValue<Fr>,
    external_cell: Cell,
    physical: &mut HashMap<ContextCell, Cell>,
) {
    let ctx_cell = virtual_cell.cell.unwrap();
    match physical.entry(ctx_cell) {
        Entry::Occupied(entry) => region.constrain_equal(*entry.get(), external_cell),
        Entry::Vacant(entry) => {
            assert_eq!(ctx_cell.type_id(), EXTERNAL_CELL_TYPE_ID);
            entry.insert(external_cell);
        }
    }
}

#[derive(Clone)]
struct Config {
    gates: Vec<BasicGateConfig<Fr>>,
    fixed: Vec<Column<Fixed>>,
    bridge: Column<crate::halo2_proofs::plonk::Advice>,
    instance: Column<Instance>,
}

type Snapshot = Vec<(
    ContextCell,
    (Column<crate::halo2_proofs::plonk::Any>, usize),
)>;

#[derive(Clone, Default)]
struct MappingCircuit<const REFERENCE: bool> {
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    missing_bridge: bool,
}

impl<const REFERENCE: bool> Circuit<Fr> for MappingCircuit<REFERENCE> {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<Fr>) -> Config {
        let gates = (0..4)
            .map(|_| BasicGateConfig::configure(meta, 0))
            .collect();
        let fixed = meta.fixed_column();
        meta.enable_equality(fixed);
        let bridge = meta.advice_column();
        meta.enable_equality(bridge);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        Config {
            gates,
            fixed: vec![fixed],
            bridge,
            instance,
        }
    }

    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fr>) -> Result<(), Error> {
        let manager = SharedCopyConstraintManager::<Fr>::default();
        let mut threads = Vec::new();
        for context_id in [0, 7, 13] {
            let mut ctx = Context::new(
                false,
                0,
                FIRST_PHASE_CELL_TYPE_ID,
                context_id,
                manager.clone(),
            );
            for _ in 0..11 {
                ctx.assign_region(
                    [1_u64, 2, 3, 7].map(|v| QuantumCell::Witness(Fr::from(v))),
                    [0],
                );
            }
            let constant = ctx.load_constant(Fr::from(1));
            ctx.constrain_equal(&ctx.get(0), &constant);
            threads.push(ctx);
        }
        {
            let mut guard = manager.lock().unwrap();
            // Deliberately unsorted, duplicated and cross-context virtual edges.
            for (a, b) in [(2, 0), (0, 1), (2, 1), (0, 1)] {
                guard.advice_equalities.push((
                    threads[a].get(0).cell.unwrap(),
                    threads[b].get(0).cell.unwrap(),
                ));
            }
        }
        let exposed = layouter.assign_region(
            || "coordinate-map oracle",
            |mut region| {
                let mut reference = HashMap::new();
                let break_points = if REFERENCE {
                    legacy_assign_with_constraints::<Fr, 4>(
                        &threads,
                        &config.gates,
                        &mut region,
                        &mut reference,
                        48,
                        false,
                    )
                } else {
                    assign_with_constraints::<Fr, 4>(
                        &threads,
                        &config.gates,
                        &mut region,
                        &mut manager.lock().unwrap(),
                        48,
                        false,
                    )
                };
                assert!(
                    break_points.len() >= 2,
                    "fixture crosses physical-column boundaries"
                );
                // Lookup-like and native bridge reads revisit out-of-order contexts.
                for (row, source) in [
                    threads[2].get(0),
                    threads[0].get(4),
                    threads[1].get(8),
                    threads[2].get(0),
                ]
                .into_iter()
                .enumerate()
                {
                    let raw = region
                        .assign_advice(config.bridge, row, Value::known(Fr::from(1)))
                        .cell();
                    if REFERENCE {
                        legacy_bridge(&mut region, source, raw, &mut reference);
                    } else {
                        constrain_virtual_equals_external(
                            &mut region,
                            source,
                            raw,
                            &mut manager.lock().unwrap(),
                        );
                    }
                }
                // First external binding inserts coordinates, the second emits an equality.
                let external = AssignedValue {
                    value: crate::halo2_proofs::plonk::Assigned::Trivial(Fr::from(1)),
                    cell: Some(ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, 17)),
                };
                for row in [4, 5] {
                    let raw = region
                        .assign_advice(config.bridge, row, Value::known(Fr::from(1)))
                        .cell();
                    if REFERENCE {
                        legacy_bridge(&mut region, external, raw, &mut reference);
                    } else {
                        constrain_virtual_equals_external(
                            &mut region,
                            external,
                            raw,
                            &mut manager.lock().unwrap(),
                        );
                    }
                }
                if self.missing_bridge {
                    let missing = AssignedValue {
                        cell: Some(ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 99, 0)),
                        ..external
                    };
                    let raw = region
                        .assign_advice(config.bridge, 6, Value::known(Fr::from(1)))
                        .cell();
                    if REFERENCE {
                        legacy_bridge(&mut region, missing, raw, &mut reference);
                    } else {
                        constrain_virtual_equals_external(
                            &mut region,
                            missing,
                            raw,
                            &mut manager.lock().unwrap(),
                        );
                    }
                }
                if REFERENCE {
                    legacy_copy_constraints(&manager, &config.fixed, &mut region, &reference);
                } else {
                    manager.assign_raw(&config.fixed, &mut region);
                }
                let mut keys = threads
                    .iter()
                    .flat_map(|ctx| {
                        (0..ctx.advice_len())
                            .map(|offset| ContextCell::new(ctx.type_id, ctx.context_id, offset))
                    })
                    .collect::<Vec<_>>();
                keys.push(external.cell.unwrap());
                let snapshot = keys
                    .into_iter()
                    .map(|key| {
                        let raw = if REFERENCE {
                            reference[&key]
                        } else {
                            manager
                                .lock()
                                .unwrap()
                                .assigned_advices
                                .resolve(&key)
                                .unwrap()
                        };
                        (key, coordinate(raw))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(snapshot.len(), 136);
                let exposed = if REFERENCE {
                    reference[&threads[0].get(0).cell.unwrap()]
                } else {
                    manager
                        .lock()
                        .unwrap()
                        .assigned_advices
                        .resolve(&threads[0].get(0).cell.unwrap())
                        .unwrap()
                };
                *self.snapshot.lock().unwrap() = Some(snapshot);
                Ok(exposed)
            },
        )?;
        layouter.constrain_instance(exposed, config.instance, 0);
        Ok(())
    }
}

#[test]
fn coordinate_runs_actual_base_assignment_and_external_bridge_match_hashmap() {
    let reference = MappingCircuit::<true>::default();
    let runs = MappingCircuit::<false>::default();
    MockProver::run(6, &reference, vec![vec![Fr::from(1)]])
        .unwrap()
        .assert_satisfied();
    MockProver::run(6, &runs, vec![vec![Fr::from(1)]])
        .unwrap()
        .assert_satisfied();
    assert_eq!(
        *reference.snapshot.lock().unwrap(),
        *runs.snapshot.lock().unwrap()
    );
    assert!(MockProver::run(6, &runs, vec![vec![Fr::from(2)]])
        .unwrap()
        .verify()
        .is_err());
}

#[test]
fn coordinate_runs_external_bridge_rejects_missing_base_identity_like_hashmap() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let reference = MappingCircuit::<true> {
        missing_bridge: true,
        ..Default::default()
    };
    let runs = MappingCircuit::<false> {
        missing_bridge: true,
        ..Default::default()
    };
    assert!(catch_unwind(AssertUnwindSafe(|| MockProver::run(
        6,
        &reference,
        vec![vec![Fr::from(1)]]
    )))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| MockProver::run(
        6,
        &runs,
        vec![vec![Fr::from(1)]]
    )))
    .is_err());
}

#[test]
fn coordinate_runs_actual_pk_vk_and_seeded_proof_equal_original_hashmap() {
    let params = ParamsKZG::<Bn256>::setup(6, StdRng::seed_from_u64(90210));
    let reference = MappingCircuit::<true>::default();
    let runs = MappingCircuit::<false>::default();
    let reference_vk = keygen_vk(&params, &reference).expect("reference VK");
    let runs_vk = keygen_vk(&params, &runs).expect("run VK");
    assert_eq!(
        reference_vk.to_bytes(SerdeFormat::Processed),
        runs_vk.to_bytes(SerdeFormat::Processed)
    );
    let reference_pk = keygen_pk(&params, reference_vk, &reference).expect("reference PK");
    let runs_pk = keygen_pk(&params, runs_vk, &runs).expect("run PK");
    // Processed here is an independent full semantic baseline, not a production artifact.
    assert_eq!(
        reference_pk.to_bytes(SerdeFormat::Processed),
        runs_pk.to_bytes(SerdeFormat::Processed)
    );
    let input = [Fr::from(1)];
    let reference_proof = gen_proof_with_instances(&params, &reference_pk, reference, &[&input]);
    let runs_proof = gen_proof_with_instances(&params, &runs_pk, runs, &[&input]);
    assert_eq!(reference_proof, runs_proof);
    check_proof_with_instances(&params, reference_pk.get_vk(), &runs_proof, &[&input], true);
    check_proof_with_instances(&params, runs_pk.get_vk(), &reference_proof, &[&input], true);
    check_proof_with_instances(
        &params,
        runs_pk.get_vk(),
        &runs_proof,
        &[&[Fr::from(2)]],
        false,
    );
    let mut corrupt = runs_proof;
    corrupt[0] ^= 1;
    check_proof_with_instances(&params, runs_pk.get_vk(), &corrupt, &[&input], false);
}
