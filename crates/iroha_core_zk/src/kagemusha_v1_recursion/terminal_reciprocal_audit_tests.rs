//! Terminal audit-cell regressions using the actual reciprocal curve relation without Table8.

use super::*;
use ff::PrimeField as _;
use halo2_proofs::{dev::MockProver, halo2curves::group::Curve as _};
use snark_verifier::loader::halo2::EccInstructions as _;

const TEST_K: usize = 12;

fn claim_tail() -> [u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1] {
    std::array::from_fn(|index| (1_u128 << 96) + 1701 + index as u128)
}

#[derive(Clone, Debug)]
struct AuditConfig<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

#[derive(Clone)]
struct AuditCircuit<C: CurveAffineExt>
where
    C::Base: BigPrimeField,
{
    builder: BaseCircuitBuilder<C::Base>,
    jobs: PastaDenseMsmJobsV1<C>,
}

impl<C> Circuit<C::Base> for AuditCircuit<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    type Config = AuditConfig<C::Base>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<C::Base>,
        params: Self::Params,
    ) -> Self::Config {
        let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        AuditConfig {
            base,
            dense: PastaDenseMsmConfigV1::configure::<C>(meta),
        }
    }

    fn configure(_: &mut ConstraintSystem<C::Base>) -> Self::Config {
        unreachable!("terminal audit tests use explicit Base parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), PlonkError> {
        <BaseCircuitBuilder<C::Base> as Circuit<C::Base>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "terminal audit Base"),
        )?;
        self.jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            (1_usize << TEST_K) - MINIMUM_UNUSABLE_ROWS,
        )
    }
}

fn scalar_audit<C>(invalid_equation: bool) -> KagemushaDeferredParentOutputV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1);
    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let generator = C::generator();
        let right = if invalid_equation {
            (generator.to_curve() + generator.to_curve()).to_affine()
        } else {
            generator
        };
        // Distinct sources and nonzero coefficients test G-G=0 (or G-2G!=0), not an
        // empty/disabled equation. The scalar half only records this deferred curve claim.
        let left = chip.assign_point(&mut ctx, generator);
        let right = chip.assign_point(&mut ctx, right);
        chip.assert_equal(&mut ctx, &left, &right);
    }
    let binding = claim_tail().map(|value| {
        loader
            .ctx_mut()
            .main()
            .load_witness(C::ScalarExt::from_u128(value))
    });
    let audit = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        CANDIDATE_EQUATION_TAG_V1,
        &binding,
    )
    .expect("real scalar-side tagged audit with all fourteen claim cells");
    assert_eq!(audit.bound_u128_values, claim_tail());
    assert_eq!(audit.audit.source_count(), 2);
    assert_eq!(audit.audit.equation_count(), 1);
    assert_eq!(audit.equation_selectors, [true]);
    audit
}

fn audit_limbs<C>(audit: &KagemushaDeferredParentOutputV1<C>) -> [C::Base; 2]
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    audit.audit_digest_limbs.map(|limb| {
        let value = fe_to_biguint(limb.value());
        assert!(value.bits() <= 128);
        let digits = value.to_u64_digits();
        C::Base::from_u128(
            u128::from(digits.first().copied().unwrap_or(0))
                | (u128::from(digits.get(1).copied().unwrap_or(0)) << 64),
        )
    })
}

fn reciprocal_circuit<C>(
    audit: &KagemushaDeferredParentOutputV1<C>,
    parity: KagemushaPastaParityV1,
    public: &[C::Base],
) -> Result<AuditCircuit<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    reciprocal_circuit_with_tail(audit, parity, public, &claim_tail())
}

fn reciprocal_circuit_with_tail<C>(
    audit: &KagemushaDeferredParentOutputV1<C>,
    parity: KagemushaPastaParityV1,
    public: &[C::Base],
    local_claim_tail: &[u128],
) -> Result<AuditCircuit<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let mut builder = BaseCircuitBuilder::<C::Base>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(1);
    builder.assigned_instances = vec![
        public
            .iter()
            .map(|value| builder.main(0).load_witness(*value))
            .collect(),
    ];
    let binding = local_claim_tail
        .iter()
        .map(|value| builder.main(0).load_witness(C::Base::from_u128(*value)))
        .collect::<Vec<_>>();
    let mut jobs = PastaDenseMsmJobsV1::default();
    constrain_terminal_reciprocal_audit_v1(&mut builder, parity, audit, &binding, &mut jobs)?;
    jobs.validate_capacity((1_usize << TEST_K) - MINIMUM_UNUSABLE_ROWS)?;
    super::super::base_packing::finalize_base_params_v1(&mut builder, MINIMUM_UNUSABLE_ROWS)?;
    Ok(AuditCircuit { builder, jobs })
}

#[test]
fn terminal_reciprocal_audit_binds_39_41_in_both_parities_without_table8() {
    fn check<C>(parity: KagemushaPastaParityV1, offset: usize)
    where
        C: CurveAffineExt,
        C::Base: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
    {
        let audit = scalar_audit::<C>(false);
        let limbs = audit_limbs(&audit);
        let mut public = vec![C::Base::ZERO; TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1];
        public[offset..offset + 2].copy_from_slice(&limbs);
        // Both old State offsets are deliberately different. These are merely unconstrained
        // history placeholders in this isolated audit test, never a valid terminal history.
        for (index, old) in [48, 49, 50, 51].into_iter().enumerate() {
            public[old] = C::Base::from(700 + index as u64);
        }
        let circuit =
            reciprocal_circuit(&audit, parity, &public).expect("correct terminal audit cells");
        let positive = MockProver::run(TEST_K as u32, &circuit, vec![public.clone()])
            .expect("actual reciprocal audit without SHA");
        // Keep a serial/parallel acceptance comparison in each field. Both APIs check every
        // usable gate/lookup row and every copy constraint; the remaining matrix uses workers.
        let parallel = positive.verify_par();
        assert_eq!(positive.verify().is_ok(), parallel.is_ok());
        assert!(parallel.is_ok(), "positive reciprocal audit: {parallel:?}");
        drop(positive);
        drop(circuit);
        for limb in 0..2 {
            let mut changed = public.clone();
            changed[offset + limb] += C::Base::ONE;
            // Regenerate the witness with the changed public limb: rejection must come from
            // the actual reciprocal Poseidon/copy relation, not a public-to-witness mismatch.
            let circuit = reciprocal_circuit(&audit, parity, &changed).unwrap();
            assert!(
                MockProver::run(TEST_K as u32, &circuit, vec![changed])
                    .expect("changed audit limb circuit")
                    .verify_par()
                    .is_err()
            );
        }
        let mut relocated = public.clone();
        let state_offset = match parity {
            KagemushaPastaParityV1::Eq => 48,
            KagemushaPastaParityV1::Ep => 50,
        };
        relocated[state_offset..state_offset + 2].copy_from_slice(&limbs);
        relocated[offset] += C::Base::ONE;
        let circuit = reciprocal_circuit(&audit, parity, &relocated).unwrap();
        assert!(
            MockProver::run(TEST_K as u32, &circuit, vec![relocated])
                .expect("relocated old State audit")
                .verify_par()
                .is_err()
        );
        assert!(reciprocal_circuit(&audit, parity, &public[..80]).is_err());
        let mut extended = public.clone();
        extended.push(C::Base::ZERO);
        assert!(reciprocal_circuit(&audit, parity, &extended).is_err());

        // A recomputed, correctly located digest must still not authorize a false curve equation.
        let invalid = scalar_audit::<C>(true);
        public[offset..offset + 2].copy_from_slice(&audit_limbs(&invalid));
        match reciprocal_circuit(&invalid, parity, &public) {
            Err(_) => {} // The dense planner may reject a nonidentity residual immediately.
            Ok(circuit) => assert!(
                MockProver::run(TEST_K as u32, &circuit, vec![public])
                    .expect("false enabled reciprocal equation")
                    .verify_par()
                    .is_err()
            ),
        }
    }
    // The debug reciprocal arithmetic graph exceeds libtest's default thread stack. Keep this
    // test-local allocation explicit; it does not change circuit or production resource limits.
    // Each field owns its circuit and audit state inside one worker. Join both workers before
    // inspecting either result, including a thread-start failure, so neither can be left running.
    let eq_worker = std::thread::Builder::new()
        .name("kagemusha-terminal-reciprocal-audit-eq".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check::<EqAffine>(KagemushaPastaParityV1::Eq, 39));
    let ep_worker = std::thread::Builder::new()
        .name("kagemusha-terminal-reciprocal-audit-ep".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check::<EpAffine>(KagemushaPastaParityV1::Ep, 41));
    let eq_result = eq_worker.map(|worker| worker.join());
    let ep_result = ep_worker.map(|worker| worker.join());
    eq_result
        .expect("start Eq terminal reciprocal audit regression")
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    ep_result
        .expect("start Ep terminal reciprocal audit regression")
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

#[test]
fn terminal_reciprocal_audit_binds_every_claim_carrier_tail_cell_in_both_parities() {
    fn check<C>(parity: KagemushaPastaParityV1, offset: usize)
    where
        C: CurveAffineExt,
        C::Base: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
    {
        let mut audit = scalar_audit::<C>(false);
        let mut public = vec![C::Base::ZERO; TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1];
        public[offset..offset + 2].copy_from_slice(&audit_limbs(&audit));
        let original = claim_tail();
        let circuit = reciprocal_circuit_with_tail(&audit, parity, &public, &original).unwrap();
        MockProver::run(TEST_K as u32, &circuit, vec![public.clone()])
            .expect("fourteen-field bound reciprocal audit")
            .assert_satisfied_par();
        drop(circuit);
        for index in 0..original.len() {
            let mut local = original;
            local[index] += 1;
            // Keep the opposite output and digest unchanged. Only the original local cell
            // differs, so the cross-parity copy constraint must reject every tail position.
            let circuit = reciprocal_circuit_with_tail(&audit, parity, &public, &local).unwrap();
            let changed = MockProver::run(TEST_K as u32, &circuit, vec![public.clone()])
                .expect("mixed paired claim tail");
            let parallel = changed.verify_par();
            if index == 0 {
                // Also compare rejection in each field, retaining the full serial check.
                assert_eq!(changed.verify().is_err(), parallel.is_err());
            }
            assert!(parallel.is_err(), "unbound local carrier tail cell {index}");
        }
        for local in [&original[..0], &original[..13]] {
            assert!(reciprocal_circuit_with_tail(&audit, parity, &public, local).is_err());
        }
        let mut extended = original.to_vec();
        extended.push(1);
        assert!(reciprocal_circuit_with_tail(&audit, parity, &public, &extended).is_err());

        // Matching a substituted local tail cannot bypass the scalar audit transcript:
        // mutate both copies while retaining the correctly placed original audit digest.
        let mut substituted = original;
        substituted[13] += 1;
        audit.bound_u128_values = substituted.to_vec();
        let circuit = reciprocal_circuit_with_tail(&audit, parity, &public, &substituted).unwrap();
        assert!(
            MockProver::run(TEST_K as u32, &circuit, vec![public.clone()])
                .expect("substituted matching tails with original scalar audit")
                .verify_par()
                .is_err()
        );
        for length in [0, 13, 15] {
            audit.bound_u128_values = vec![1; length];
            assert!(reciprocal_circuit_with_tail(&audit, parity, &public, &original).is_err());
        }
    }
    // Each field owns its circuit and audit state inside one worker. Join both workers before
    // inspecting either result, including a thread-start failure, so neither can be left running.
    let eq_worker = std::thread::Builder::new()
        .name("kagemusha-terminal-claim-carrier-binding-eq".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check::<EqAffine>(KagemushaPastaParityV1::Eq, 39));
    let ep_worker = std::thread::Builder::new()
        .name("kagemusha-terminal-claim-carrier-binding-ep".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check::<EpAffine>(KagemushaPastaParityV1::Ep, 41));
    let eq_result = eq_worker.map(|worker| worker.join());
    let ep_result = ep_worker.map(|worker| worker.join());
    eq_result
        .expect("start Eq terminal claim carrier binding regression")
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    ep_result
        .expect("start Ep terminal claim carrier binding regression")
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

#[test]
fn terminal_parallel_mock_verifier_matches_serial_for_padding_gates_lookups_and_copies() {
    use halo2_proofs::{
        circuit::{SimpleFloorPlanner, Value},
        plonk::{Advice, Assigned, Column, Expression, Instance, Selector, TableColumn},
        poly::Rotation,
    };

    #[derive(Clone)]
    struct Probe<F> {
        gate_value: Assigned<F>,
        lookup_value: Assigned<F>,
    }

    impl<F: ff::PrimeField> Circuit<F> for Probe<F> {
        type Config = (Column<Advice>, TableColumn, Selector, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            self.clone()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let padding = meta.advice_column();
            let advice = meta.advice_column();
            let table = meta.lookup_table_column();
            let enabled = meta.complex_selector();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            meta.create_gate(
                "parallel parity with zero-weight padding rotation",
                |meta| {
                    let q = meta.query_selector(enabled);
                    let value = meta.query_advice(advice, Rotation::cur());
                    let unused = meta.query_advice(padding, Rotation(7));
                    vec![
                        q * (value - Expression::Constant(F::from(3))
                            + Expression::Constant(F::ZERO) * unused),
                    ]
                },
            );
            meta.lookup("parallel parity rotated lookup", |meta| {
                let q = meta.query_selector(enabled);
                let value = meta.query_advice(advice, Rotation::next());
                vec![(q * value, table)]
            });
            (advice, table, enabled, instance)
        }

        fn synthesize(
            &self,
            (advice, table, enabled, instance): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            layouter.assign_table(
                || "parallel parity range",
                |mut region| {
                    for row in 0..4 {
                        region.assign_cell(
                            || "range value",
                            table,
                            row,
                            || Value::known(F::from(row as u64)),
                        )?;
                    }
                    Ok(())
                },
            )?;
            let cell = layouter.assign_region(
                || "parallel parity witnesses",
                |mut region| {
                    enabled.enable(&mut region, 0)?;
                    let cell = region
                        .assign_advice(advice, 0, Value::known(self.gate_value))
                        .cell();
                    let _ = region.assign_advice(advice, 1, Value::known(self.lookup_value));
                    Ok(cell)
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    fn check<F: KagemushaPoseidonFieldV1>() {
        let assigned = |value| Assigned::Trivial(F::from(value));
        let rational_zero = Assigned::Rational(F::ONE, F::ZERO);
        // A zero denominator denotes the field value zero, not an invalid assignment.
        // It must preserve lookup membership and copy values without skipping bad gates.
        for (gate, lookup, instance, valid) in [
            (assigned(3), assigned(3), 3, true),
            (assigned(4), assigned(3), 4, false),
            (assigned(3), assigned(9), 3, false),
            (assigned(3), assigned(3), 4, false),
            (assigned(3), rational_zero, 3, true),
            (assigned(3), Assigned::Rational(F::ZERO, F::ZERO), 3, true),
            (assigned(4), rational_zero, 4, false),
            (assigned(3), rational_zero, 4, false),
            (rational_zero, assigned(3), 0, false),
            (
                Assigned::Rational(F::from(6), F::from(2)),
                assigned(3),
                3,
                true,
            ),
        ] {
            let circuit = Probe {
                gate_value: gate,
                lookup_value: lookup,
            };
            let prover = MockProver::run(6, &circuit, vec![vec![F::from(instance)]]).unwrap();
            let serial = prover.verify();
            let parallel = prover.verify_par();
            assert_eq!(serial.is_ok(), valid, "serial result: {serial:?}");
            assert_eq!(parallel.is_ok(), valid, "parallel result: {parallel:?}");
        }
    }

    check::<Fp>();
    check::<Fq>();
}
