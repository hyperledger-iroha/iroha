//! Both-Pasta mutation checks for the proof-visible global source carrier.

use ff::{Field as _, WithSmallOrderMulGroup};
use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::{MockProver, VerifyFailure},
    halo2curves::pasta::{Fp, Fq},
};
use snark_verifier::loader::halo2::EccInstructions as _;

use super::*;

#[path = "mint_hash_claim_inventory_proof_tests.rs"]
mod proof;

const TEST_K: usize = 13;
const TEST_SOURCES: usize = 2;

fn fixture<F: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>>(
    parity: KagemushaPastaParityV1,
) -> (BaseCircuitBuilder<F>, Vec<Vec<F>>) {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(3);
    let semantic = (0..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1)
        .map(|value| builder.main(0).load_witness(F::from(value as u64)))
        .collect();
    builder.assigned_instances = vec![semantic];
    let common = (0..KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1)
        .map(|index| builder.main(0).load_witness(F::from(index as u64 + 201)))
        .collect::<Vec<_>>();
    let mut own = [11_u128, 12, 13, 14, 15, 16, 17, 18]
        .map(|value| builder.main(0).load_witness(F::from_u128(value)))
        .to_vec();
    own.extend(common.iter().copied());
    let mut opposite = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
    opposite[..8].copy_from_slice(&[21, 22, 23, 24, 25, 26, 27, 28]);
    for (index, value) in opposite[8..8 + common.len()].iter_mut().enumerate() {
        *value = index as u128 + 201;
    }
    // These known integers are the exact canonical u128 values of the common cells.
    attach_global_inventory_instances_v1(
        &mut builder,
        parity,
        own,
        TEST_SOURCES,
        &opposite,
        TEST_SOURCES,
        &common,
    )
    .expect("two-source global inventory carrier");
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let instances = builder
        .assigned_instances
        .iter()
        .map(|column| column.iter().map(|cell| *cell.value()).collect())
        .collect();
    (builder, instances)
}

fn check_parity<F: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>>(
    parity: KagemushaPastaParityV1,
) {
    let (builder, baseline) = fixture::<F>(parity);
    assert_eq!(
        baseline.iter().map(Vec::len).collect::<Vec<_>>(),
        [113, 4090, 4090]
    );
    assert_eq!(baseline[0][INVENTORY_EQ_SOURCE_COUNT], F::from(2));
    assert_eq!(baseline[0][INVENTORY_EP_SOURCE_COUNT], F::from(2));
    MockProver::run(TEST_K as u32, &builder, baseline.clone())
        .expect("global inventory public binding")
        .assert_satisfied();

    let own_column = match parity {
        KagemushaPastaParityV1::Eq => 1,
        KagemushaPastaParityV1::Ep => 2,
    };
    let opposite_column = 3 - own_column;
    for (column, index) in [
        (own_column, 0), // source point
        (own_column, 2), // aggregate coefficient
        (own_column, 4), // second source, preserving count
        (opposite_column, 0),
        (opposite_column, 8),  // shared 58-cell bound tail
        (opposite_column, 90), // zero padding
        (0, INVENTORY_EQ_SOURCE_COUNT),
        (0, INVENTORY_EP_SOURCE_COUNT),
    ] {
        let mut changed = baseline.clone();
        changed[column][index] += F::ONE;
        assert!(
            MockProver::run(TEST_K as u32, &builder, changed)
                .expect("mutated global inventory public cell")
                .verify()
                .is_err(),
            "parity {parity:?} column {column} cell {index} escaped the proof boundary"
        );
    }
    let mut reordered = baseline;
    reordered[own_column].swap(0, 4);
    reordered[own_column].swap(1, 5);
    reordered[own_column].swap(2, 6);
    reordered[own_column].swap(3, 7);
    assert!(
        MockProver::run(TEST_K as u32, &builder, reordered)
            .expect("reordered global inventory sources")
            .verify()
            .is_err()
    );

    // Both the witness and public count agree on the false value. The source
    // count must still be constrained to the actual graph cardinality.
    let count_circuit = |claimed_sources| {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let count = constrain_inventory_source_count_v1(
            builder.main(0),
            &range,
            TEST_SOURCES,
            claimed_sources,
        )
        .expect("bounded graph count");
        builder.assigned_instances = vec![vec![count]];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        builder
    };
    MockProver::run(
        TEST_K as u32,
        &count_circuit(TEST_SOURCES),
        vec![vec![F::from(TEST_SOURCES as u64)]],
    )
    .expect("honest graph count")
    .assert_satisfied();
    assert!(
        MockProver::run(TEST_K as u32, &count_circuit(3), vec![vec![F::from(3)]])
            .expect("same false count in witness and public instance")
            .verify()
            .is_err()
    );
}

#[test]
fn global_inventory_public_sources_coefficients_counts_and_padding_are_bound_in_both_fields() {
    check_parity::<Fp>(KagemushaPastaParityV1::Eq);
    check_parity::<Fq>(KagemushaPastaParityV1::Ep);
}

#[test]
fn global_inventory_rejects_nonzero_reciprocal_padding_and_invalid_counts() {
    for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_instance_columns(3);
        builder.assigned_instances = vec![
            (0..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1)
                .map(|_| builder.main(0).load_witness(Fp::ZERO))
                .collect(),
        ];
        let common = (0..KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1)
            .map(|_| builder.main(0).load_witness(Fp::ZERO))
            .collect::<Vec<_>>();
        let mut own = (0..8)
            .map(|_| builder.main(0).load_witness(Fp::ONE))
            .collect::<Vec<_>>();
        own.extend(common.iter().copied());
        let mut opposite = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        opposite[100] = 1;
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own.clone(),
                2,
                &opposite,
                2,
                &common
            )
            .is_err()
        );
        opposite[100] = 0;
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own.clone(),
                0,
                &opposite,
                2,
                &common
            )
            .is_err()
        );
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own,
                2,
                &opposite,
                1009,
                &common
            )
            .is_err()
        );
    }
}

/// Build an actual inventory Circuit around a small, true deferred equation.
/// This exercises the production inventory finalizer, native source-challenge
/// queue and full 4,090-cell reciprocal RLC schedule. It deliberately omits the
/// parent/shard recursive verifier graph and does not qualify a monetary Claim.
fn full_inventory_fixture<C>(
    parity: KagemushaPastaParityV1,
    transcript_input: u64,
    corrupt_rlc_evaluation: bool,
    carrier_commitments: Option<ClaimCarrierCommitmentsV1>,
) -> KagemushaClaimGlobalInventoryProofInputV1<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits(CLAIM_RLC_RADIX_BITS)
        .use_instance_columns(3);
    // Synthesis-only cases use explicit points; proof-reader cases supply the
    // actual commitments computed from these carriers and authenticated IPA parameters.
    let commitments = carrier_commitments.unwrap_or_else(|| ClaimCarrierCommitmentsV1 {
        eq_proof_eq_carrier: EqAffine::generator(),
        eq_proof_ep_carrier: (EqAffine::generator() * Fp::from(2)).to_affine(),
        ep_proof_eq_carrier: EpAffine::generator(),
        ep_proof_ep_carrier: (EpAffine::generator() * Fq::from(2)).to_affine(),
    });
    let eq_challenge =
        native_claim_carrier_challenge_v1::<Fp>(commitments, KagemushaPastaParityV1::Eq);
    let ep_challenge =
        native_claim_carrier_challenge_v1::<Fq>(commitments, KagemushaPastaParityV1::Ep);
    let mut semantic_values = (0..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1)
        .map(|value| C::ScalarExt::from(value as u64 + 1))
        .collect::<Vec<_>>();
    append_inner_carrier_binding_v1(
        &mut semantic_values,
        ClaimCarrierBindingV1 {
            commitments,
            eq_challenge,
            ep_challenge,
            eq_at_eq_challenge: 0,
            eq_at_ep_challenge: 0,
            ep_at_eq_challenge: 0,
            ep_at_ep_challenge: 0,
        },
    )
    .expect("synthetic inventory semantic tail");
    let public = semantic_values
        .into_iter()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    let common_cells = common_public_cells_v1(&public);
    assert_eq!(
        common_cells.len(),
        KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
    );
    builder.assigned_instances = vec![public.clone()];
    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    constrain_claim_carrier_challenge_v1(&loader, &public, parity)
        .expect("native parity challenge binds the eight commitment limbs");
    let (bindings, selector) = {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let generator = C::generator();
        let first = chip.assign_point(&mut ctx, generator);
        let second = chip.assign_point(
            &mut ctx,
            (generator.to_curve() * C::ScalarExt::from(2)).to_affine(),
        );
        let sum = chip.sum_with_const(&mut ctx, &[&first, &first], C::identity());
        chip.assert_equal(&mut ctx, &second, &sum);
        let bindings = [transcript_input, 19, 23, 29, 1]
            .map(|value| ctx.main().load_witness(C::ScalarExt::from(value)));
        let selector = ctx.main().load_constant(C::ScalarExt::ONE);
        (bindings, selector)
    };
    assert_eq!(loader.ecc_chip().equation_count(), 1);
    let mut native_poseidon_jobs = PastaNativePoseidonJobsV1::new(
        KAGEMUSHA_MINT_HASH_CLAIM_NATIVE_POSEIDON_LANES_V1,
        (1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS,
    )
    .expect("full native Poseidon row envelope");
    let output = derive_mint_hash_claim_native_deferred_batch_v1(
        &mut builder,
        loader,
        vec![CLAIM_SHARD_EQUATION_TAG_V1],
        vec![selector],
        &bindings,
        &common_cells,
        &mut native_poseidon_jobs,
    )
    .expect("true two-source equation with the complete source challenge");
    assert_eq!(output.batch.source_count(), TEST_SOURCES);
    let own_carrier = padded_claim_carrier_u128_values_v1(&output)
        .expect("derived, canonical source-major carrier");
    let audit = assigned_digest_bytes_v1(&output.challenge_limbs).expect("derived source audit");
    let audit_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_AUDIT_LO,
    };
    for (index, limb) in output.challenge_limbs.iter().enumerate() {
        let assigned = builder.main(0).load_witness(*limb.value());
        builder.assigned_instances[0][audit_offset + index] = assigned;
    }
    let mut opposite_carrier = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
    opposite_carrier[..8].copy_from_slice(&[31, 37, 41, 43, 47, 53, 59, 61]);
    for (target, source) in opposite_carrier[8..8 + common_cells.len()]
        .iter_mut()
        .zip(&common_cells)
    {
        *target = assigned_u128_cell_v1(*source, "synthetic common cell")
            .expect("canonical common value");
    }
    let (eq_carrier, ep_carrier) = match parity {
        KagemushaPastaParityV1::Eq => (&own_carrier, &opposite_carrier),
        KagemushaPastaParityV1::Ep => (&opposite_carrier, &own_carrier),
    };
    for (offset, values, challenge) in [
        (
            public_instance::EQ_CARRIER_AT_EQ_CHALLENGE,
            eq_carrier,
            eq_challenge,
        ),
        (
            public_instance::EQ_CARRIER_AT_EP_CHALLENGE,
            eq_carrier,
            ep_challenge,
        ),
        (
            public_instance::EP_CARRIER_AT_EQ_CHALLENGE,
            ep_carrier,
            eq_challenge,
        ),
        (
            public_instance::EP_CARRIER_AT_EP_CHALLENGE,
            ep_carrier,
            ep_challenge,
        ),
    ] {
        let mut value = native_claim_carrier_rlc_v1(values, challenge)
            .expect("independent native RLC evaluation");
        if corrupt_rlc_evaluation && offset == public_instance::EQ_CARRIER_AT_EQ_CHALLENGE {
            value = (value + 1) % CLAIM_CARRIER_RLC_MODULUS_V1;
        }
        let assigned = builder.main(0).load_witness(C::ScalarExt::from_u128(value));
        builder.assigned_instances[0][offset] = assigned;
    }
    finish_global_inventory_v1::<C>(
        ClaimScalarHalfV1 {
            builder,
            output,
            common_cells,
            native_poseidon_jobs,
        },
        parity,
        audit,
        audit,
        &own_carrier,
        &opposite_carrier,
        TEST_SOURCES,
    )
    .expect("actual inventory circuit with complete native regions")
}

fn check_full_inventory_synthesis<C>(parity: KagemushaPastaParityV1)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let baseline = full_inventory_fixture::<C>(parity, 17, false, None);
    assert_eq!(
        baseline.instances.iter().map(Vec::len).collect::<Vec<_>>(),
        [113, 4090, 4090]
    );
    assert!(
        baseline
            .circuit
            .native_poseidon_jobs
            .required_rows()
            .unwrap()
            > 0
    );
    assert!(baseline.circuit.carrier_rlc.required_rows().unwrap() > 0);
    MockProver::run(
        KAGEMUSHA_RECURSION_IPA_K_V1,
        &baseline.circuit,
        baseline.instances.clone(),
    )
    .expect("complete inventory Circuit synthesis")
    .assert_satisfied();
    baseline.circuit.builder.reset_synthesis_state();

    let own_audit = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_AUDIT_LO,
    };
    let mut changed_public = baseline.instances.clone();
    changed_public[0][own_audit] += C::ScalarExt::ONE;
    assert!(
        MockProver::run(
            KAGEMUSHA_RECURSION_IPA_K_V1,
            &baseline.circuit,
            changed_public
        )
        .expect("changed full inventory public audit")
        .verify()
        .is_err()
    );
    baseline.circuit.builder.reset_synthesis_state();

    // Both the Base witness and public evaluation agree on the same false RLC
    // value. Base alone accepts it; the actual inventory RLC region must reject.
    let wrong_rlc = full_inventory_fixture::<C>(parity, 17, true, None);
    MockProver::run(
        KAGEMUSHA_RECURSION_IPA_K_V1,
        &wrong_rlc.circuit.builder,
        wrong_rlc.instances.clone(),
    )
    .expect("Base does not stand in for the RLC machine")
    .assert_satisfied();
    wrong_rlc.circuit.builder.reset_synthesis_state();
    assert!(
        MockProver::run(
            KAGEMUSHA_RECURSION_IPA_K_V1,
            &wrong_rlc.circuit,
            wrong_rlc.instances
        )
        .expect("false RLC evaluation in the full inventory Circuit")
        .verify()
        .is_err()
    );

    // Keep the honest Base, audit, public carriers and RLC, but splice in the
    // same-shape Poseidon queue for a different verifier-input transcript. The
    // cross-region copies must reject it. This catches an omitted native region
    // while the Poseidon gate's own algebra is exercised in its dedicated tests.
    let foreign = full_inventory_fixture::<C>(parity, 31, false, None);
    MockProver::run(
        KAGEMUSHA_RECURSION_IPA_K_V1,
        &foreign.circuit,
        foreign.instances.clone(),
    )
    .expect("honest foreign full inventory Circuit synthesis")
    .assert_satisfied();
    foreign.circuit.builder.reset_synthesis_state();
    assert_eq!(
        baseline.circuit.native_poseidon_jobs.required_rows(),
        foreign.circuit.native_poseidon_jobs.required_rows()
    );
    assert_ne!(
        baseline.instances[0][own_audit],
        foreign.instances[0][own_audit]
    );
    let mut wrong_poseidon = baseline.circuit;
    wrong_poseidon.native_poseidon_jobs = foreign.circuit.native_poseidon_jobs;
    let failures = MockProver::run(
        KAGEMUSHA_RECURSION_IPA_K_V1,
        &wrong_poseidon,
        baseline.instances,
    )
    .expect("foreign native Poseidon queue in the full inventory Circuit")
    .verify()
    .expect_err("the foreign Poseidon queue must violate its Base copy bindings");
    assert!(
        !failures.is_empty()
            && failures
                .iter()
                .all(|failure| matches!(failure, VerifyFailure::Permutation { .. })),
        "two satisfied fixtures must fail only at their transplanted copy bindings: {failures:?}"
    );
}

#[test]
fn global_inventory_full_circuit_synthesizes_poseidon_and_rlc_in_both_fields() {
    check_full_inventory_synthesis::<EqAffine>(KagemushaPastaParityV1::Eq);
    check_full_inventory_synthesis::<EpAffine>(KagemushaPastaParityV1::Ep);
}
