//! Static guards for the uninhabited residual-commitment boundary.

use super::*;

#[test]
fn coefficient_gate_oracle_has_the_frozen_two_residuals() {
    let aggregate = Scalar::from_u64(3);
    let equality = Scalar::from_u64(5);
    let weighted = Scalar::from_u64(15);
    let terminal = Scalar::from_u64(7);
    assert_eq!(
        coefficient_gate_residuals_v1(aggregate, equality, weighted, terminal, weighted + terminal),
        [Scalar::zero(); 2]
    );
    assert_ne!(
        coefficient_gate_residuals_v1(
            aggregate,
            equality + Scalar::one(),
            weighted,
            terminal,
            weighted + terminal,
        )[0],
        Scalar::zero()
    );
}

#[test]
fn residual_commitment_fixture_points_are_exact_and_ordered() {
    let generator = Point::canonical_generator().expect("canonical generator");
    let expected = [
        hex_literal::hex!("804f62562dcd0e0d1f8c036f9dbcebf8efa2e53232f3a638cb7de434eaa1545923"),
        hex_literal::hex!("00687bf060defdc612b4def7534a81002c6b934793111c940b534748983b337f91"),
        hex_literal::hex!("00faf3d078d37c668f3c7cf837273792c932b9feebbd45a019ae85d24226ae0b33"),
    ];
    for ((statement, multiple), encoded) in POST_BATCH_RESIDUAL_STATEMENTS_V1
        .into_iter()
        .zip([53_u64, 54, 55])
        .zip(expected)
    {
        assert!([3, 5, 8].contains(&statement));
        assert_eq!(
            generator
                .mul_scalar(Scalar::from_u64(multiple))
                .to_non_identity_wire_bytes()
                .unwrap(),
            encoded
        );
    }
}

#[test]
#[rustfmt::skip]
fn source_guards_freeze_ordered_points_and_the_uninhabited_production_seal() {
    let source = include_str!("coefficient_residual_v1.rs");
    let parent = include_str!("../challenge_v1.rs");
    assert!(source.contains("POST_BATCH_RESIDUAL_STATEMENTS_V1"));
    assert!(source.contains("FRAME_COEFFICIENT_RESIDUAL_COORDINATE_V1"));
    assert!(source.contains("FRAME_COEFFICIENT_RESIDUAL_COMMITMENT_V1"));
    assert!(source.contains("commitments: [[u8; 33]; REQUIRED_POST_BATCH_RESIDUAL_COMMITMENTS_V1]"));
    assert!(source.contains("validate_endpoint_v1(&commitment)?"));
    assert!(source.contains("exact-length-2^14-vector-q_s[v]"));
    assert!(source.contains("binds-every-entry-to-the-frozen-q_3/q_5/q_8-Boolean-formula"));
    assert!(source.contains("A_s=Q_s(r_s)-and-opens-the-same-framed-q_s-commitment"));
    assert!(source.contains("blinded_residual_commitments: Infallible"));
    assert!(source.contains("vector_arithmetic_proofs: Infallible"));
    let transition = source.split("impl GlobalLookupTranscriptV1<CoefficientResidualCommitmentStageV1>").nth(1).unwrap();
    let validate = transition.find("validate_endpoint_v1(&commitment)?").unwrap();
    let coordinate = transition.find("FRAME_COEFFICIENT_RESIDUAL_COORDINATE_V1").unwrap();
    let commitment = transition.find("FRAME_COEFFICIENT_RESIDUAL_COMMITMENT_V1").unwrap();
    assert!(validate < coordinate && coordinate < commitment);
    assert!(parent.contains("Result<GlobalLookupTranscriptV1<CoefficientResidualCommitmentStageV1>"));
    assert!(!parent.contains("Result<GlobalLookupTranscriptV1<SumcheckStageV1>"));
    assert!(!source.contains("residual_digest"));
    assert!(!source.contains("pub struct"));
    assert!(source.lines().count() <= 180);
}
