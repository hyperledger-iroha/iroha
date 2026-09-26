//! Fixed five-slice interval, padding, source and endpoint constraint checks.

use ff::Field as _;
use halo2_base::utils::CurveAffineExt;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        CurveAffine,
        group::Curve as _,
        pasta::{EpAffine, EqAffine},
    },
    plonk::{Circuit as _, ConstraintSystem},
};

use super::{
    CLAIM_SLICE_K_V1, CLAIM_SLICE_SLOTS_V1, PartialMsmFiveSliceCircuitV1,
    PartialMsmFiveSliceStatementV1, PartialMsmSourceV1,
};

fn statement<C, const I: usize>(total_sources: u32) -> PartialMsmFiveSliceStatementV1<C>
where
    C: CurveAffineExt,
{
    let source_start = I as u32 * total_sources / 5;
    let source_end = (I as u32 + 1) * total_sources / 5;
    let active_sources = source_end - source_start;
    let generator = C::generator();
    let start = (generator.to_curve() * C::ScalarExt::from(7)).to_affine();
    let mut sources = [PartialMsmSourceV1 {
        point: generator,
        coefficient: C::ScalarExt::ZERO,
    }; CLAIM_SLICE_SLOTS_V1];
    for (index, source) in sources.iter_mut().take(active_sources as usize).enumerate() {
        *source = PartialMsmSourceV1 {
            point: (generator.to_curve() * C::ScalarExt::from(index as u64 + 2)).to_affine(),
            coefficient: C::ScalarExt::from(index as u64 + 3),
        };
    }
    let endpoint = sources
        .iter()
        .take(active_sources as usize)
        .fold(start.to_curve(), |sum, source| {
            sum + source.point.to_curve() * source.coefficient
        })
        .to_affine();
    PartialMsmFiveSliceStatementV1 {
        total_sources,
        source_start,
        source_end,
        active_sources,
        start,
        endpoint,
        sources,
    }
}

fn assert_rejected<C, const I: usize>(
    circuit: &PartialMsmFiveSliceCircuitV1<C, I>,
    public: Vec<C::Base>,
) where
    C: CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField
        + halo2_base::utils::ScalarField
        + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    assert!(
        MockProver::run(CLAIM_SLICE_K_V1 as u32, circuit, vec![public])
            .expect("slice synthesis")
            .verify()
            .is_err()
    );
}

fn assert_slice_matrix<C>()
where
    C: CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField
        + halo2_base::utils::ScalarField
        + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    const I: usize = 2;
    let original = statement::<C, I>(7);
    let public = original.public_instances::<I>().expect("exact carrier");
    assert_eq!(public.len(), 9 + 202 * 4);
    let circuit =
        PartialMsmFiveSliceCircuitV1::<C, I>::build(&original).expect("bounded five-slice circuit");
    assert_eq!(circuit.params().k, CLAIM_SLICE_K_V1);
    let params = circuit.params();
    let mut meta = ConstraintSystem::<C::Base>::default();
    <PartialMsmFiveSliceCircuitV1<C, I> as halo2_proofs::plonk::Circuit<C::Base>>::configure_with_params(
        &mut meta,
        params.clone(),
    );
    println!(
        "five-slice {}: advice={}, fixed={}, permutation={}, Base gate advice={:?}, Base lookup advice={:?}, Base fixed={}",
        std::any::type_name::<C>(),
        meta.num_advice_columns(),
        meta.num_fixed_columns(),
        meta.permutation().get_columns().len(),
        params.num_advice_per_phase,
        params.num_lookup_advice_per_phase,
        params.num_fixed,
    );
    assert_eq!(
        circuit.dense_jobs.required_rows_with_lanes(1).unwrap(),
        (202 + 2) * 130 + 3
    );
    MockProver::run(CLAIM_SLICE_K_V1 as u32, &circuit, vec![public.clone()])
        .expect("valid slice synthesis")
        .assert_satisfied();

    // Same group sum but changed source order must not match the original
    // public source-major carrier.
    let mut reordered = original;
    reordered.sources.swap(0, 1);
    let reordered_circuit = PartialMsmFiveSliceCircuitV1::<C, I>::build(&reordered).unwrap();
    assert_rejected(&reordered_circuit, public);

    let mut changed_coefficient = original;
    changed_coefficient.sources[1].coefficient += C::ScalarExt::ONE;
    changed_coefficient.endpoint = (changed_coefficient.endpoint.to_curve()
        + changed_coefficient.sources[1].point.to_curve())
    .to_affine();
    let changed_coefficient_circuit =
        PartialMsmFiveSliceCircuitV1::<C, I>::build(&changed_coefficient).unwrap();
    assert_rejected(
        &changed_coefficient_circuit,
        original.public_instances::<I>().unwrap(),
    );

    // The circuit itself rejects an incorrect floor interval even if the
    // attacker's supplied public cells agree with its malformed witness.
    let mut wrong_interval = original;
    wrong_interval.total_sources = 8;
    assert!(wrong_interval.public_instances::<I>().is_err());
    let wrong_interval_circuit =
        PartialMsmFiveSliceCircuitV1::<C, I>::build_unchecked_for_test(&wrong_interval).unwrap();
    assert_rejected(
        &wrong_interval_circuit,
        wrong_interval.public_instances_unchecked::<I>(),
    );

    // This changed padding point still contributes zero to the MSM, so only
    // the explicit padding predicate can reject it.
    let mut bad_padding = original;
    bad_padding.sources[2].point = (C::generator().to_curve() * C::ScalarExt::from(2)).to_affine();
    assert!(bad_padding.public_instances::<I>().is_err());
    let bad_padding_circuit =
        PartialMsmFiveSliceCircuitV1::<C, I>::build_unchecked_for_test(&bad_padding).unwrap();
    assert_rejected(
        &bad_padding_circuit,
        bad_padding.public_instances_unchecked::<I>(),
    );

    // Giving an inactive canonical generator a nonzero coefficient and
    // adjusting the endpoint preserves the MSM equation. Tail gating alone
    // must reject the changed coefficient.
    let mut bad_padding_coefficient = original;
    bad_padding_coefficient.sources[2].coefficient = C::ScalarExt::ONE;
    bad_padding_coefficient.endpoint =
        (bad_padding_coefficient.endpoint.to_curve() + C::generator().to_curve()).to_affine();
    assert!(bad_padding_coefficient.public_instances::<I>().is_err());
    let bad_padding_coefficient_circuit =
        PartialMsmFiveSliceCircuitV1::<C, I>::build_unchecked_for_test(&bad_padding_coefficient)
            .unwrap();
    assert_rejected(
        &bad_padding_coefficient_circuit,
        bad_padding_coefficient.public_instances_unchecked::<I>(),
    );

    let mut bad_endpoint = original;
    bad_endpoint.endpoint =
        (bad_endpoint.endpoint.to_curve() + C::generator().to_curve()).to_affine();
    let bad_endpoint_public = bad_endpoint.public_instances::<I>().unwrap();
    let bad_endpoint_circuit = PartialMsmFiveSliceCircuitV1::<C, I>::build(&bad_endpoint).unwrap();
    assert_rejected(&bad_endpoint_circuit, bad_endpoint_public);
}

#[test]
fn five_slice_eq_and_ep_bind_interval_padding_source_order_and_endpoint() {
    assert_slice_matrix::<EqAffine>();
    assert_slice_matrix::<EpAffine>();
}

#[test]
fn five_slice_maximum_inventory_covers_exactly_1008_sources() {
    let expected = [(0, 201), (201, 403), (403, 604), (604, 806), (806, 1_008)];
    for (index, (start, end)) in expected.into_iter().enumerate() {
        let actual = match index {
            0 => statement::<EqAffine, 0>(1_008),
            1 => statement::<EqAffine, 1>(1_008),
            2 => statement::<EqAffine, 2>(1_008),
            3 => statement::<EqAffine, 3>(1_008),
            _ => statement::<EqAffine, 4>(1_008),
        };
        assert_eq!((actual.source_start, actual.source_end), (start, end));
        assert_eq!(actual.active_sources, end - start);
        assert_eq!(actual.sources.len(), CLAIM_SLICE_SLOTS_V1);
    }
}

#[test]
fn five_slice_full_202_active_sources_prove_in_both_parities() {
    fn prove<C>()
    where
        C: CurveAffineExt,
        C::Base: halo2_base::utils::BigPrimeField
            + halo2_base::utils::ScalarField
            + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    {
        let value = statement::<C, 4>(1_008);
        assert_eq!(value.active_sources, 202);
        let public = value.public_instances::<4>().unwrap();
        let circuit = PartialMsmFiveSliceCircuitV1::<C, 4>::build(&value).unwrap();
        MockProver::run(CLAIM_SLICE_K_V1 as u32, &circuit, vec![public])
            .expect("full slice synthesis")
            .assert_satisfied();
    }
    prove::<EqAffine>();
    prove::<EpAffine>();
}

#[test]
fn five_slice_zero_active_prefix_proves_in_both_parities() {
    fn prove<C>()
    where
        C: CurveAffineExt,
        C::Base: halo2_base::utils::BigPrimeField
            + halo2_base::utils::ScalarField
            + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    {
        let value = statement::<C, 0>(1);
        assert_eq!(value.active_sources, 0);
        assert_eq!(value.start, value.endpoint);
        let public = value.public_instances::<0>().unwrap();
        let circuit = PartialMsmFiveSliceCircuitV1::<C, 0>::build(&value).unwrap();
        MockProver::run(CLAIM_SLICE_K_V1 as u32, &circuit, vec![public])
            .expect("empty-prefix slice synthesis")
            .assert_satisfied();
    }
    prove::<EqAffine>();
    prove::<EpAffine>();
}

#[test]
fn five_slice_rejects_overlarge_inventory_wrong_count_and_nonzero_padding() {
    let mut value = statement::<EqAffine, 4>(7);
    value.total_sources = 1_009;
    assert!(value.public_instances::<4>().is_err());
    assert!(PartialMsmFiveSliceCircuitV1::<EqAffine, 4>::build(&value).is_err());

    let mut value = statement::<EqAffine, 4>(7);
    value.active_sources += 1;
    assert!(value.public_instances::<4>().is_err());
    assert!(PartialMsmFiveSliceCircuitV1::<EqAffine, 4>::build(&value).is_err());

    let mut value = statement::<EqAffine, 4>(7);
    value.sources[value.active_sources as usize].coefficient =
        <EqAffine as CurveAffine>::ScalarExt::ONE;
    assert!(value.public_instances::<4>().is_err());
    assert!(PartialMsmFiveSliceCircuitV1::<EqAffine, 4>::build(&value).is_err());
}
