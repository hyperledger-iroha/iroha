//! Genuine governed issuer equation for a private ordinary credential or refresh lease.
//!
//! The signing point must be selected by the release-fixed profile table. It is a separate
//! authority from the app's nonexportable approval key. Hashing an Ed original alone cannot
//! establish issuance; this equation authenticates that exact complete Ed-only original.

use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    ecc::EcPoint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::halo2curves::secp256r1::{Fp as P256Base, Fq as P256Scalar};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1,
    KagemushaOrdinaryIssuerCircuitAdmissionSigningLayoutV1 as I,
    KagemushaOrdinaryIssuerCircuitAdmissionV1,
};
use sha2::{Digest as _, Sha256};

use crate::{
    kagemusha_p256_curve_gadget::{
        P256_LIMB_BITS, P256_NUM_LIMBS, assert_p256_ecdsa_digest, p256_uint_bits_le,
    },
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

use super::guard_bundle::{constant_bytes, hash};

/// Assign the actual mandatory public signature and authenticate its precise governed subject.
/// The private issuer key is never present. The point cells are additionally fixed by release
/// table configuration and must also be copied to this profile's original credential cells.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_issuer_original_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    purpose: u8,
    release: &[PastaSha256ByteV1<F>; 32],
    profile: &[PastaSha256ByteV1<F>; 32],
    ed_original_sha256: &[PastaSha256ByteV1<F>; 32],
    original_signature: &[PastaSha256ByteV1<F>; 64],
    original: &KagemushaOrdinaryIssuerCircuitAdmissionV1,
    governed_key: &[u8; 65],
    governed_key_cells: &[AssignedValue<F>; 65],
) -> Result<[PastaSha256ByteV1<F>; I::TOTAL_BYTES], String> {
    fn field_be<T: ff::PrimeField>(raw: &[u8]) -> Result<T, String> {
        let mut repr = T::Repr::default();
        if repr.as_ref().len() != raw.len() {
            return Err("issuer P256 field width differs".into());
        }
        for (byte, original) in repr.as_mut().iter_mut().zip(raw.iter().rev()) {
            *byte = *original;
        }
        Option::<T>::from(T::from_repr(repr)).ok_or("issuer P256 field is not canonical".into())
    }
    let raw = original.signature.as_raw_bytes();
    let r = field_be::<P256Scalar>(&raw[..32])?;
    let s = field_be::<P256Scalar>(&raw[32..])?;
    let qx = field_be::<P256Base>(&governed_key[1..33])?;
    let qy = field_be::<P256Base>(&governed_key[33..])?;
    let digest: [u8; 32] = Sha256::digest(original.subject.canonical_signing_bytes()?).into();
    let z = digest.iter().fold(P256Scalar::ZERO, |value, byte| {
        value * P256Scalar::from(256) + P256Scalar::from(u64::from(*byte))
    });
    let mut little = digest;
    little.reverse();
    let quotient = u64::from(Option::<P256Scalar>::from(P256Scalar::from_repr(little)).is_none());
    let range = builder.range_chip();
    let base = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let scalar = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    let key = EcPoint::new(base.load_private(ctx, qx), base.load_private(ctx, qy));
    let r = scalar.load_private(ctx, r);
    let s = scalar.load_private(ctx, s);
    let z = scalar.load_private(ctx, z);
    let quotient = ctx.load_witness(F::from(quotient));
    constrain_ordinary_issuer_admission_v1(
        builder,
        jobs,
        purpose,
        release,
        profile,
        ed_original_sha256,
        original_signature,
        &OrdinaryIssuerSignatureCellsV1 {
            public_key: &key,
            fixed_profile_key_sec1: governed_key_cells,
            r: &r,
            s: &s,
            z: &z,
            digest_reduction_quotient: quotient,
        },
    )
}

/// Cells of the issuer's public signature, never either party's private signing key.
pub(super) struct OrdinaryIssuerSignatureCellsV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) public_key: &'a EcPoint<F, ProperCrtUint<F>>,
    pub(super) fixed_profile_key_sec1: &'a [AssignedValue<F>; 65],
    pub(super) r: &'a ProperCrtUint<F>,
    pub(super) s: &'a ProperCrtUint<F>,
    pub(super) z: &'a ProperCrtUint<F>,
    pub(super) digest_reduction_quotient: AssignedValue<F>,
}

/// Derive the sole signing frame from the same release/profile and canonical Ed-only SHA cells.
/// Credential and lease purposes have disjoint fixed constants; a raw witness cannot choose one.
pub(super) fn ordinary_issuer_message_v1<F: KagemushaPoseidonFieldV1>(
    purpose: u8,
    release: &[PastaSha256ByteV1<F>; 32],
    profile: &[PastaSha256ByteV1<F>; 32],
    ed_original_sha256: &[PastaSha256ByteV1<F>; 32],
) -> Result<[PastaSha256ByteV1<F>; I::TOTAL_BYTES], String> {
    if !matches!(purpose, 1 | 2) {
        return Err("ordinary issuer proof purpose rejected".into());
    }
    let mut bytes = constant_bytes(KAGEMUSHA_ORDINARY_ISSUER_CIRCUIT_ADMISSION_DOMAIN_V1);
    bytes.extend(constant_bytes(&99_u64.to_le_bytes()));
    bytes.extend(constant_bytes(&1_u16.to_le_bytes()));
    bytes.push(PastaSha256ByteV1::constant(purpose));
    bytes.extend_from_slice(release);
    bytes.extend_from_slice(profile);
    bytes.extend_from_slice(ed_original_sha256);
    bytes
        .try_into()
        .map_err(|_| "ordinary issuer signing width differs".into())
}

/// Bind the original fixed `r || s` carrier to the exact full-width ECDSA scalar cells.
/// No DER conversion, normalization or original-byte substitution is permitted.
fn bind_fixed_signature_v1<F: KagemushaPoseidonFieldV1>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut halo2_base::Context<F>,
    raw: &[PastaSha256ByteV1<F>; 64],
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
) {
    for (raw, value) in raw.chunks_exact(32).zip([r, s]) {
        let bits = p256_uint_bits_le(chip, ctx, value);
        for (index, raw_byte) in raw.iter().enumerate() {
            let first = (31 - index) * 8;
            let byte = chip.gate().inner_product(
                ctx,
                bits[first..first + 8].iter().copied(),
                (0..8).map(|bit| Constant(F::from(1_u64 << bit))),
            );
            let difference = chip.gate().sub(ctx, byte, raw_byte.quantum_cell());
            chip.gate().assert_is_const(ctx, &difference, &F::ZERO);
        }
    }
}

/// Authenticate the original mandatory issuer signature with the genuine low-S 256-bit equation.
/// Returned signing bytes are also copy-bound into the complete canonical admission carrier.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_issuer_admission_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    purpose: u8,
    release: &[PastaSha256ByteV1<F>; 32],
    profile: &[PastaSha256ByteV1<F>; 32],
    ed_original_sha256: &[PastaSha256ByteV1<F>; 32],
    original_signature: &[PastaSha256ByteV1<F>; 64],
    signature: &OrdinaryIssuerSignatureCellsV1<'_, F>,
) -> Result<[PastaSha256ByteV1<F>; I::TOTAL_BYTES], String> {
    let message = ordinary_issuer_message_v1(purpose, release, profile, ed_original_sha256)?;
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    // Every fixed signature byte has its own 8-bit admission, independently of scalar limbs.
    let raw = original_signature.map(|byte| match byte.assigned() {
        Some(cell) => PastaSha256ByteV1::range_checked(ctx, &range, cell),
        None => byte,
    });
    bind_fixed_signature_v1(&chip, ctx, &raw, signature.r, signature.s);
    let digest = hash(ctx, jobs, message.to_vec())?;
    let digest = digest.map(|byte| byte.assigned().expect("issuer SHA bytes assigned"));
    assert_p256_ecdsa_digest::<F, 256, true>(
        &chip,
        ctx,
        signature.public_key,
        signature.public_key,
        signature.fixed_profile_key_sec1,
        signature.r,
        signature.s,
        signature.z,
        &digest,
        signature.digest_reduction_quotient,
    );
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use der_parser::num_bigint::BigUint;
    use halo2_base::utils::modulus;
    use halo2_ecc::bigint::FixedCRTInteger;
    use halo2_proofs::dev::MockProver;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};
    use iroha_data_model::kagemusha::KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1;

    fn message_matches_model<F: KagemushaPoseidonFieldV1>() {
        let subject = KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1 {
            version: 1,
            purpose: 1,
            release_id: [3; 32],
            hardware_profile_id: [7; 32],
            ed_original_sha256: [11; 32],
        };
        let expected = subject.canonical_signing_bytes().unwrap();
        let message = ordinary_issuer_message_v1::<F>(
            1,
            &subject.release_id.map(PastaSha256ByteV1::constant),
            &subject.hardware_profile_id.map(PastaSha256ByteV1::constant),
            &subject.ed_original_sha256.map(PastaSha256ByteV1::constant),
        )
        .unwrap();
        // Constants retain the exact model-owned domain, purpose and all three original bindings.
        for (actual, expected) in message.iter().zip(expected) {
            assert_eq!(actual.test_value(), expected);
        }
        assert!(
            ordinary_issuer_message_v1::<F>(
                0,
                &subject.release_id.map(PastaSha256ByteV1::constant),
                &subject.hardware_profile_id.map(PastaSha256ByteV1::constant),
                &subject.ed_original_sha256.map(PastaSha256ByteV1::constant),
            )
            .is_err()
        );
        let lease = ordinary_issuer_message_v1::<F>(
            2,
            &subject.release_id.map(PastaSha256ByteV1::constant),
            &subject.hardware_profile_id.map(PastaSha256ByteV1::constant),
            &subject.ed_original_sha256.map(PastaSha256ByteV1::constant),
        )
        .unwrap();
        assert_ne!(
            message[I::PURPOSE].test_value(),
            lease[I::PURPOSE].test_value()
        );
    }

    #[test]
    fn issuer_signing_subject_matches_the_native_codec_in_both_fields() {
        message_matches_model::<Fp>();
        message_matches_model::<Fq>();
    }

    #[test]
    fn issuer_entry_points_require_full_width_low_s_signature_cells() {
        let _ = constrain_ordinary_issuer_admission_v1::<Fp>;
        let _ = constrain_ordinary_issuer_admission_v1::<Fq>;
    }

    fn fixed_signature_is_bound<F: KagemushaPoseidonFieldV1>(mutation: Option<usize>) -> bool {
        let mut raw = core::array::from_fn::<_, 64, _>(|i| (i as u8).wrapping_add(1));
        // Keep the scalar witnesses equal to the original bytes while altering the retained
        // carrier. In particular this detects changes in both the low and high scalar bytes.
        let r = BigUint::from_bytes_be(&raw[..32]);
        let s = BigUint::from_bytes_be(&raw[32..]);
        if let Some(index) = mutation {
            raw[index] ^= 1;
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(12)
            .use_lookup_bits(11)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
        let ctx = builder.main(0);
        let r = FixedCRTInteger::from_native(r, P256_NUM_LIMBS, P256_LIMB_BITS).assign(
            ctx,
            P256_LIMB_BITS,
            &modulus::<F>(),
        );
        let s = FixedCRTInteger::from_native(s, P256_NUM_LIMBS, P256_LIMB_BITS).assign(
            ctx,
            P256_LIMB_BITS,
            &modulus::<F>(),
        );
        let raw = raw.map(|byte| {
            let cell = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, &range, cell)
        });
        bind_fixed_signature_v1(&chip, ctx, &raw, &r, &s);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(12, &builder, vec![Vec::new()])
            .expect("fixed issuer signature shape synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn issuer_original_fixed_signature_cannot_change_scalar_bytes_in_either_field() {
        assert!(fixed_signature_is_bound::<Fp>(None));
        assert!(fixed_signature_is_bound::<Fq>(None));
        for index in [0, 15, 31, 32, 47, 63] {
            assert!(!fixed_signature_is_bound::<Fp>(Some(index)));
            assert!(!fixed_signature_is_bound::<Fq>(Some(index)));
        }
    }
}
