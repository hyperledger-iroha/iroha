//! Fixed-size partial-MSM shard constraint and exact-carrier mutation checks.

use ff::Field as _;
use halo2_proofs::plonk::Circuit as _;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        CurveAffine,
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine},
    },
};

use super::{
    PARTIAL_MSM_SHARD_K_V1, PartialMsmShardCircuitV1, PartialMsmShardStatementV1,
    PartialMsmSourceV1,
};
use halo2_base::utils::CurveAffineExt;

fn statement<C>() -> PartialMsmShardStatementV1<C, 2>
where
    C: CurveAffineExt,
{
    let generator = C::generator();
    let second = (generator.to_curve() + generator.to_curve()).to_affine();
    let start = (generator.to_curve() * C::ScalarExt::from(7)).to_affine();
    let sources = [
        PartialMsmSourceV1 {
            point: generator,
            coefficient: C::ScalarExt::from(3),
        },
        PartialMsmSourceV1 {
            point: second,
            coefficient: C::ScalarExt::from(5),
        },
    ];
    let endpoint = (start.to_curve()
        + sources[0].point.to_curve() * sources[0].coefficient
        + sources[1].point.to_curve() * sources[1].coefficient)
        .to_affine();
    PartialMsmShardStatementV1 {
        source_start: 400,
        source_end: 402,
        start,
        endpoint,
        sources,
    }
}

fn assert_shard_matrix<C>()
where
    C: CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField
        + halo2_base::utils::ScalarField
        + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let original = statement::<C>();
    let public = original
        .public_instances()
        .expect("canonical source carrier");
    assert_eq!(public.len(), 14);
    let circuit = PartialMsmShardCircuitV1::build(&original).expect("valid shard circuit");
    MockProver::run(
        PARTIAL_MSM_SHARD_K_V1 as u32,
        &circuit,
        vec![public.clone()],
    )
    .expect("valid shard synthesis")
    .assert_satisfied();

    // A reordered chunk has the same group sum, so the original source-major
    // carrier must still reject it at the exact public-instance boundary.
    let mut reordered = original.clone();
    reordered.sources.swap(0, 1);
    let reordered_circuit =
        PartialMsmShardCircuitV1::build(&reordered).expect("reordered shard circuit");
    assert!(
        MockProver::run(
            PARTIAL_MSM_SHARD_K_V1 as u32,
            &reordered_circuit,
            vec![public.clone()],
        )
        .expect("reordered shard synthesis")
        .verify()
        .is_err()
    );

    let mut omitted = original.clone();
    omitted.sources[1].coefficient = C::ScalarExt::ZERO;
    let omitted_circuit = PartialMsmShardCircuitV1::build(&omitted).expect("omitted shard circuit");
    assert!(
        MockProver::run(
            PARTIAL_MSM_SHARD_K_V1 as u32,
            &omitted_circuit,
            vec![public.clone()],
        )
        .expect("omitted shard synthesis")
        .verify()
        .is_err()
    );

    // Even if a caller changes its claimed public endpoint along with the
    // witness, the dense relation must reject an invalid partial sum.
    let mut bad_endpoint = original.clone();
    bad_endpoint.endpoint =
        (bad_endpoint.endpoint.to_curve() + C::generator().to_curve()).to_affine();
    let bad_public = bad_endpoint
        .public_instances()
        .expect("mutated public endpoint");
    let bad_circuit =
        PartialMsmShardCircuitV1::build(&bad_endpoint).expect("invalid endpoint circuit");
    assert!(
        MockProver::run(
            PARTIAL_MSM_SHARD_K_V1 as u32,
            &bad_circuit,
            vec![bad_public]
        )
        .expect("endpoint mutation synthesis")
        .verify()
        .is_err()
    );

    let mut wrong_interval = public;
    wrong_interval[1] += C::Base::ONE;
    assert!(
        MockProver::run(
            PARTIAL_MSM_SHARD_K_V1 as u32,
            &circuit,
            vec![wrong_interval]
        )
        .expect("interval mutation synthesis")
        .verify()
        .is_err()
    );
}

#[test]
fn partial_msm_shard_eq_and_ep_bind_order_and_endpoint() {
    assert_shard_matrix::<EqAffine>();
    assert_shard_matrix::<EpAffine>();
}

#[test]
fn partial_msm_shard_rejects_noncanonical_interval() {
    let mut statement = statement::<EqAffine>();
    statement.source_end = statement.source_start + 1;
    assert!(statement.public_instances().is_err());
    assert!(PartialMsmShardCircuitV1::build(&statement).is_err());
}

#[test]
fn partial_msm_shard_full_32_source_shape_fits_k14() {
    let generator = EqAffine::generator();
    let start = (generator.to_curve() * <EqAffine as CurveAffine>::ScalarExt::from(7)).to_affine();
    let sources: [PartialMsmSourceV1<EqAffine>; 32] =
        std::array::from_fn(|index| PartialMsmSourceV1 {
            point: (generator.to_curve()
                * <EqAffine as CurveAffine>::ScalarExt::from(index as u64 + 2))
            .to_affine(),
            coefficient: <EqAffine as CurveAffine>::ScalarExt::ONE,
        });
    let endpoint = sources
        .iter()
        .fold(start.to_curve(), |sum, source| {
            sum + source.point.to_curve()
        })
        .to_affine();
    let statement = PartialMsmShardStatementV1 {
        source_start: 0,
        source_end: 32,
        start,
        endpoint,
        sources,
    };
    let circuit = PartialMsmShardCircuitV1::build(&statement)
        .expect("maximum shard must fit both Base packing and dense scheduling");
    assert_eq!(circuit.params().k, PARTIAL_MSM_SHARD_K_V1);
    let dense_rows = circuit
        .dense_jobs
        .required_rows_with_lanes(1)
        .expect("maximum shard dense rows");
    assert_eq!(dense_rows, (32 + 2) * 130 + 3);
    assert!(dense_rows <= (1 << PARTIAL_MSM_SHARD_K_V1) - 9);
    assert_eq!(statement.public_instances().unwrap().len(), 6 + 32 * 4);
}
