use super::*;
use crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1;

fn zero_responses_v1() -> Box<[i64; WITNESS_CHUNK_COEFFICIENTS_V1]> {
    vec![0_i64; WITNESS_CHUNK_COEFFICIENTS_V1]
        .into_boxed_slice()
        .try_into()
        .expect("exact direct-response fixture")
}

fn basis_v1()
-> crate::generalized_bulletproof::ProofGeneratorView<'static, ZkAmsT256BulletproofSuiteV1> {
    ZkAmsT256BulletproofSuiteV1::generators()
        .reduce(WITNESS_CHUNK_COEFFICIENTS_V1)
        .expect("release T256 basis")
}

#[test]
fn canonical_identity_and_single_term_vectors_are_exact() {
    let basis = basis_v1();
    let mut responses = zero_responses_v1();
    let identity =
        reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &basis.g_bold[0])
            .expect("identity result is canonically admitted");
    let mut identity_wire = [0_u8; 33];
    identity_wire[0] = 0x40;
    assert_eq!(identity.as_bytes(), &identity_wire);

    responses[0] = 1;
    assert_eq!(
        reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &basis.g_bold[1],)
            .expect("positive unit")
            .as_bytes(),
        &basis.g_bold[0].to_non_identity_wire_bytes().unwrap()
    );
    responses[0] = -1;
    assert_eq!(
        reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &basis.g_bold[1],)
            .expect("negative unit")
            .as_bytes(),
        &basis.g_bold[0]
            .negate()
            .to_non_identity_wire_bytes()
            .unwrap()
    );
}

#[test]
fn challenge_subtracts_the_source_commitment_and_mixed_vector_matches_direct_arithmetic() {
    let basis = basis_v1();
    let mut responses = zero_responses_v1();
    let source = basis.g_bold[4];
    let negative = reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 1, &source)
        .expect("negative source commitment");
    assert_eq!(
        negative.as_bytes(),
        &source.negate().to_non_identity_wire_bytes().unwrap()
    );

    responses[0] = 7;
    responses[17] = -9;
    responses[16_383] = 13;
    let blind = Scalar::from_u64(11);
    let challenge = 19_u32;
    let observed = reconstruct_direct_response_first_message_v1(
        &responses,
        &blind.to_be_bytes(),
        challenge,
        &source,
    )
    .expect("mixed response vector");
    let expected = basis.g_bold[0].mul_scalar(Scalar::from_u64(7))
        + basis.g_bold[17].mul_scalar(-Scalar::from_u64(9))
        + basis.g_bold[16_383].mul_scalar(Scalar::from_u64(13))
        + basis.h.mul_scalar(blind)
        + source.mul_scalar(-Scalar::from_u64(u64::from(challenge)));
    assert_eq!(
        observed.as_bytes(),
        &expected.to_non_identity_wire_bytes().unwrap()
    );
    assert_eq!(
        hex::encode(observed.as_bytes()),
        "00354a70252c7be6a80793fbd2492fcc9618c20b8a55781a8488552a50538a62e2"
    );
}

#[test]
fn exact_response_bounds_and_scalar_encoding_fail_closed() {
    let basis = basis_v1();
    let mut responses = zero_responses_v1();
    responses[0] = -RESPONSE_COEFFICIENT_BOUND_V1;
    responses[WITNESS_CHUNK_COEFFICIENTS_V1 - 1] = RESPONSE_COEFFICIENT_BOUND_V1;
    reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &basis.g_bold[0])
        .expect("both response endpoints are admitted");
    responses[73] = RESPONSE_COEFFICIENT_BOUND_V1 + 1;
    assert_eq!(
        reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &basis.g_bold[0],),
        Err(DirectResponseCommitmentErrorV1::ResponseOutOfRange { index: 73 })
    );
    responses[73] = 0;
    assert_eq!(
        reconstruct_direct_response_first_message_v1(
            &responses,
            &VEGA_T256_SCALAR_MODULUS_BE_V1,
            0,
            &basis.g_bold[0],
        ),
        Err(DirectResponseCommitmentErrorV1::BlindScalarEncoding)
    );
    let mut endian_swapped_modulus = VEGA_T256_SCALAR_MODULUS_BE_V1;
    endian_swapped_modulus.reverse();
    assert_eq!(
        reconstruct_direct_response_first_message_v1(
            &responses,
            &endian_swapped_modulus,
            0,
            &basis.g_bold[0],
        ),
        Err(DirectResponseCommitmentErrorV1::BlindScalarEncoding)
    );
    assert_eq!(
        reconstruct_direct_response_first_message_v1(&responses, &[0; 32], 0, &Point::identity(),),
        Err(DirectResponseCommitmentErrorV1::SourceCommitmentIdentity)
    );
}

#[test]
fn response_point_owner_clears_during_unwind() {
    let basis = basis_v1();
    let unwind = std::panic::catch_unwind(|| {
        let scalar = ZeroizingT256ScalarCopyV1::new(Scalar::from_u64(1));
        let mut terms = SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(1)
            .expect("one-term response owner fixture");
        terms
            .push(scalar.as_ref(), &basis.g_bold[0])
            .expect("one-term response owner input");
        let _owned = ZeroizingDirectResponsePointV1(
            terms
                .evaluate()
                .expect("one-term response owner evaluation"),
        );
        panic!("injected response-owner unwind");
    });
    assert!(unwind.is_err());
}

#[test]
fn source_guards_freeze_owned_arithmetic_and_inert_authority() {
    const SOURCE: &str = include_str!("response_commitment_v1.rs");
    const PARENT: &str = include_str!("../direct_relation_wire_v1.rs");
    const ACTIVE: &str = include_str!("../../active_exact_binding.rs");
    assert!(SOURCE.contains(
        "responses: &[i64; WITNESS_CHUNK_COEFFICIENTS_V1],\n    blind_response_be: &[u8; 32],"
    ));
    assert!(SOURCE.contains("DIRECT_RESPONSE_MSM_TERMS_V1 == 16_386"));
    assert!(SOURCE.contains("SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new("));
    assert!(SOURCE.contains("ZeroizingT256ScalarCopyV1::new(-Scalar::from_u64("));
    assert!(SOURCE.contains("Scalar::from_be_bytes_exact_ref(blind_response_be)"));
    assert!(
        SOURCE.contains("let reconstructed = ZeroizingDirectResponsePointV1(terms.evaluate()?);")
    );
    assert!(SOURCE.contains("struct ZeroizingDirectResponsePointV1(SecretPoint<Point>);"));
    assert!(SOURCE.contains(
        "let encoded = SecretT256PointEncodingV1::new_allow_identity(self.0.expose_ref())?;"
    ));
    assert!(SOURCE.contains("let public = *encoded.as_ref();"));
    assert!(SOURCE.contains("drop(encoded);"));
    assert!(!SOURCE.contains("<Point as ProofPoint>::encode(*self.0.expose_ref())"));
    assert!(!SOURCE.contains(".to_non_identity_wire_bytes()"));
    assert!(!SOURCE.contains("Vec<Scalar>"));
    assert!(!SOURCE.contains("multiexp("));
    assert!(!SOURCE.contains("responses.iter().copied()"));
    assert!(!SOURCE.contains("pub fn"));
    let canonical_owner = SOURCE
        .split_once("pub(in super::super) struct CanonicalDirectResponsePointV1")
        .expect("canonical response owner")
        .0
        .rsplit_once("#[derive(")
        .expect("canonical response derive")
        .1;
    assert!(!canonical_owner.contains("Clone"));
    assert!(!canonical_owner.contains("Copy"));
    assert!(PARENT.contains("pub(super) mod response_commitment_v1;"));
    assert!(ACTIVE.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
}
