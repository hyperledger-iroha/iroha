//! Real assignment regressions for KAGEMUSHA's exact Base column sizing.

use halo2_base::{QuantumCell::Witness, gates::RangeInstructions as _};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};

use super::*;
use halo2_base::gates::circuit::BaseConfig;

const TEST_K: usize = 6;
const RESERVED_ROWS: usize = 9;
const USABLE_ROWS: usize = 55;

#[derive(Clone)]
struct PackingCircuit<F: ScalarField>(BaseCircuitBuilder<F>);

impl<F: ScalarField> Circuit<F> for PackingCircuit<F> {
    type Config = BaseConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BaseCircuitParams;

    fn without_witnesses(&self) -> Self {
        Self(self.0.deep_clone().unknown(true))
    }

    fn params(&self) -> Self::Params {
        self.0.config_params.clone()
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut config = BaseConfig::configure(meta, params);
        config.set_usable_rows(USABLE_ROWS);
        config
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("packing regressions use explicit Base parameters")
    }

    fn synthesize(&self, config: Self::Config, layouter: impl Layouter<F>) -> Result<(), Error> {
        self.0.synthesize(config, layouter)
    }
}

fn builder<F: ScalarField>() -> BaseCircuitBuilder<F> {
    BaseCircuitBuilder::new(false).use_k(TEST_K)
}

fn check_final_boundary<F: ScalarField>() {
    for (count, columns, breaks) in [
        (USABLE_ROWS - 1, 1, vec![]),
        (USABLE_ROWS, 2, vec![USABLE_ROWS - 1]),
        (USABLE_ROWS + 1, 2, vec![USABLE_ROWS - 1]),
    ] {
        let mut builder = builder::<F>();
        for _ in 0..count {
            builder.main(0).load_witness(F::from(7));
        }
        finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("finalize exact capacity");
        assert_eq!(builder.config_params.num_advice_per_phase, [columns]);
        let circuit = PackingCircuit(builder);
        MockProver::run(TEST_K as u32, &circuit, vec![])
            .expect("actual Base assignment")
            .assert_satisfied();
        assert_eq!(circuit.0.break_points(), vec![breaks]);
    }
}

#[test]
fn base_packing_counts_the_final_boundary_duplicate_in_both_fields() {
    check_final_boundary::<Fp>();
    check_final_boundary::<Fq>();
}

fn check_selected_boundaries<F: ScalarField>() {
    for invalid_output in [false, true] {
        let mut builder = builder::<F>();
        for start in [USABLE_ROWS - 3, 2 * USABLE_ROWS - 6] {
            let ctx = builder.main(0);
            while ctx.advice_len() < start {
                ctx.load_witness(F::ZERO);
            }
            ctx.assign_region(
                [
                    Witness(F::from(2)),
                    Witness(F::from(3)),
                    Witness(F::ONE),
                    Witness(F::from(if invalid_output { 6 } else { 5 })),
                ],
                [0],
            );
        }
        let before = format!("{:?}", builder.core());
        finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("finalize selected gates");
        assert_eq!(before, format!("{:?}", builder.core()));
        assert_eq!(builder.config_params.num_advice_per_phase, [3]);
        let circuit = PackingCircuit(builder);
        let prover = MockProver::run(TEST_K as u32, &circuit, vec![])
            .expect("actual rotated gate assignment");
        assert_eq!(
            circuit.0.break_points(),
            vec![vec![USABLE_ROWS - 3, USABLE_ROWS - 3]]
        );
        assert_eq!(prover.verify().is_ok(), !invalid_output);
    }
}

#[test]
fn base_packing_preserves_rotated_gate_constraints_and_rejects_tampering() {
    check_selected_boundaries::<Fp>();
    check_selected_boundaries::<Fq>();
}

fn check_context_boundaries<F: ScalarField>() {
    let mut builder = builder::<F>();
    for _ in 0..USABLE_ROWS - 1 {
        builder.main(0).load_witness(F::ONE);
    }
    builder.new_thread(0); // An empty context must not reset the physical cursor.
    builder.new_thread(0).load_witness(F::ONE);
    finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("finalize context packing");
    assert_eq!(builder.config_params.num_advice_per_phase, [2]);
    let circuit = PackingCircuit(builder);
    MockProver::run(TEST_K as u32, &circuit, vec![])
        .expect("actual context boundary assignment")
        .assert_satisfied();
    assert_eq!(circuit.0.break_points(), vec![vec![USABLE_ROWS - 1]]);
}

#[test]
fn base_packing_keeps_the_cursor_across_empty_and_nonempty_contexts() {
    check_context_boundaries::<Fp>();
    check_context_boundaries::<Fq>();
}

fn check_fixed_rows<F: ScalarField>() {
    let mut builder = builder::<F>();
    for value in 0..64 {
        builder.main(0).load_constant(F::from(value));
    }
    builder.calculate_params(Some(RESERVED_ROWS));
    assert_eq!(
        builder.config_params.num_fixed, 1,
        "old domain-sized estimate"
    );
    let before = format!("{:?}", builder.core());
    finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("finalize fixed columns");
    assert_eq!(builder.config_params.num_fixed, 2);
    assert_eq!(before, format!("{:?}", builder.core()));
    MockProver::run(TEST_K as u32, &PackingCircuit(builder), vec![])
        .expect("fixed equalities fit without using reserved rows")
        .assert_satisfied();
}

#[test]
fn base_packing_reserves_rows_for_fixed_constants_in_both_fields() {
    check_fixed_rows::<Fp>();
    check_fixed_rows::<Fq>();
}

#[test]
fn base_packing_preserves_lookup_public_cells_and_repeated_finalization() {
    fn check<F: ScalarField>() {
        let mut builder = builder::<F>().use_lookup_bits(5).use_instance_columns(1);
        let range = builder.range_chip();
        let value = builder.main(0).load_witness(F::from(17));
        range.range_check(builder.main(0), value, 5);
        builder.assigned_instances = vec![vec![value]];
        let before = format!("{:?}", builder.core());
        let lookup_rows = builder.statistics().total_lookup_advice_per_phase;
        finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("complete Base sizing");
        assert_eq!(builder.config_params.k, TEST_K);
        assert_eq!(builder.config_params.lookup_bits, Some(5));
        assert_eq!(builder.config_params.num_instance_columns, 1);
        assert_eq!(builder.assigned_instances[0][0].cell, value.cell);
        assert_eq!(before, format!("{:?}", builder.core()));
        assert_eq!(
            builder.statistics().total_lookup_advice_per_phase,
            lookup_rows
        );
        let params = format!("{:?}", builder.config_params);
        finalize_base_params_v1(&mut builder, RESERVED_ROWS).expect("repeat complete sizing");
        assert_eq!(params, format!("{:?}", builder.config_params));
        let circuit = PackingCircuit(builder);
        MockProver::run(TEST_K as u32, &circuit, vec![vec![F::from(17)]])
            .expect("lookup and public assignments")
            .assert_satisfied();
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn base_packing_handles_zero_shapes_and_preserves_independent_phases() {
    let mut empty = builder::<Fp>();
    finalize_base_params_v1(&mut empty, RESERVED_ROWS).expect("empty graph");
    assert_eq!(empty.config_params.num_advice_per_phase, [0]);
    assert_eq!(empty.config_params.num_fixed, 0);
    assert!(
        empty
            .config_params
            .num_lookup_advice_per_phase
            .iter()
            .all(|count| *count == 0)
    );
    assert_eq!(
        validate_base_gate_capacity_v1(&empty, RESERVED_ROWS)
            .expect("empty capacity")
            .maximum_advice_rows,
        0
    );
    MockProver::run(TEST_K as u32, &PackingCircuit(empty), vec![])
        .expect("actual empty assignment")
        .assert_satisfied();

    let mut phases = builder::<Fp>();
    phases.main(0).load_witness(Fp::from(1));
    for _ in 0..USABLE_ROWS {
        phases.main(1).load_witness(Fp::from(1));
    }
    phases.new_thread(2);
    finalize_base_params_v1(&mut phases, RESERVED_ROWS).expect("independent phase sizing");
    assert_eq!(phases.config_params.num_advice_per_phase, [1, 2, 0]);
    assert!(validate_base_gate_capacity_v1(&phases, RESERVED_ROWS).is_ok());
    phases.config_params.num_advice_per_phase[1] = 1;
    assert!(validate_base_gate_capacity_v1(&phases, RESERVED_ROWS).is_err());
    phases.config_params.num_advice_per_phase.pop();
    assert!(validate_base_gate_capacity_v1(&phases, RESERVED_ROWS).is_err());
}

#[test]
fn base_packing_rejects_incomplete_graphs_and_bad_domains_without_mutation() {
    for (k, reserve) in [(0, 9), (usize::BITS as usize, 9), (6, usize::MAX), (2, 1)] {
        let mut builder = builder::<Fp>().use_k(k);
        let params = format!("{:?}", builder.config_params);
        assert!(finalize_base_params_v1(&mut builder, reserve).is_err());
        assert_eq!(params, format!("{:?}", builder.config_params));
    }
    let mut witness_only = BaseCircuitBuilder::<Fp>::new(true).use_k(TEST_K);
    assert!(finalize_base_params_v1(&mut witness_only, RESERVED_ROWS).is_err());
    let mut incomplete = builder::<Fp>();
    incomplete.main(0).load_witness(Fp::from(1));
    incomplete.main(0).selector.resize(0, false);
    let params = format!("{:?}", incomplete.config_params);
    assert!(finalize_base_params_v1(&mut incomplete, RESERVED_ROWS).is_err());
    assert_eq!(params, format!("{:?}", incomplete.config_params));
}
