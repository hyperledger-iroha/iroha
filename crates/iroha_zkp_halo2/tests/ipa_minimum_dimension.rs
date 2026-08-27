//! Regressions for the minimum safe IPA opening dimension.

use iroha_zkp_halo2::{
    Bn254Params, Error, IpaBackend, IpaGroup, IpaParams, IpaProofData, IpaScalar,
    OpenVerifyEnvelope, Params, backend::bn254::Bn254Backend, backend::pallas::PallasBackend,
    batch, norito_helpers,
};

fn forged_zero_round_envelope<B: IpaBackend>() -> OpenVerifyEnvelope {
    let one = B::Scalar::one();
    let a_final = one.add(one);
    let t = one;
    let g = B::derive_group_elem(b"G", 1, 0);
    let u = B::derive_group_elem(b"U", 1, 0);
    let p_g = g.pow(a_final).mul(u.pow(a_final.sub(t)));
    OpenVerifyEnvelope {
        params: IpaParams {
            version: 1,
            curve_id: B::CURVE_ID.as_u16(),
            n: 1,
        },
        public: norito_helpers::poly_open_public::<B>(1, one, t, p_g),
        proof: IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: a_final.to_bytes(),
            b_final: one.to_bytes(),
        },
        transcript_label: "forged-zero-round".into(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

fn assert_forged_envelope_is_rejected<B: IpaBackend>() {
    let results = batch::verify_open_batch_with_options(
        &[forged_zero_round_envelope::<B>()],
        &batch::BatchOptions::sequential(),
    );
    assert!(matches!(results.first(), Some(Err(Error::InvalidN(1)))));
}

#[test]
fn constructors_reject_zero_round_dimensions() {
    assert!(matches!(Params::new(1), Err(Error::InvalidN(1))));
    assert!(matches!(Bn254Params::new(1), Err(Error::InvalidN(1))));
}

#[test]
fn pallas_zero_round_forgery_is_rejected() {
    assert_forged_envelope_is_rejected::<PallasBackend>();
}

#[test]
fn bn254_zero_round_forgery_is_rejected() {
    assert_forged_envelope_is_rejected::<Bn254Backend>();
}
