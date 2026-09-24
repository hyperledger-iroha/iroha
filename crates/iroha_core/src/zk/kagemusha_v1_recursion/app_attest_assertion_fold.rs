//! One bounded call surface ensuring raw CBOR, canonical DER, SHA and ECDSA share cells.
//!
//! This staged helper remains unused by the monetary fold. It does not by itself
//! authenticate the signed Core subject, governed RP, credential, or release policy.
// TODO: Wire this exact helper into both recursive parities only after Core/Guard,
// provider-policy and terminal commitment links are fully circuit-derived.

use halo2_base::{AssignedValue, gates::circuit::builder::BaseCircuitBuilder};
use halo2_ecc::{
    bigint::ProperCrtUint,
    ecc::EcPoint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::halo2curves::secp256r1::Fp as P256Base;

use crate::zk::{
    kagemusha_p256_curve_gadget::{
        P256_CRT_LIMB_BITS_V1, app_attest_der_gadget::constrain_p256_canonical_der_v1,
        assert_apple_assertion_ecdsa,
    },
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::PastaSha256JobsV1,
};

use super::app_attest_assertion_cbor::constrain_original_apple_assertion_37_v1;

/// Derive DER from `r,s`, prove original CBOR equality, then verify the Apple
/// assertion with those same `r,s` and authenticator/S cells. No host-parsed
/// DER pair can be substituted between the two relations.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged monetary assertion fold remains closed")
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_assertion_ecdsa_37_v1<
    F: KagemushaPoseidonFieldV1,
    const S_LEN: usize,
>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    original_raw_assertion: &[u8],
    canonical_s: &[AssignedValue<F>; S_LEN],
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_id_hash: &[AssignedValue<F>; 32],
    previous_secure_index: AssignedValue<F>,
    next_secure_index: AssignedValue<F>,
    signature_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key_sec1: &[AssignedValue<F>; 65],
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
    z: &ProperCrtUint<F>,
    digest_reduction_quotient: AssignedValue<F>,
) -> Result<(), String> {
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_CRT_LIMB_BITS_V1, 3);
    let der = constrain_p256_canonical_der_v1(&chip, builder.main(0), r, s);
    constrain_original_apple_assertion_37_v1(
        builder,
        original_raw_assertion,
        authenticator_data,
        &der,
    )?;
    let expected_flags = builder.main(0).load_constant(F::from(0x40_u64));
    assert_apple_assertion_ecdsa::<F, S_LEN, 256, 37, false>(
        &chip,
        builder.main(0),
        jobs,
        canonical_s,
        authenticator_data,
        governed_rp_id_hash,
        expected_flags,
        previous_secure_index,
        next_secure_index,
        signature_public_key,
        enrolled_public_key,
        enrolled_public_key_sec1,
        r,
        s,
        z,
        digest_reduction_quotient,
    )
}
