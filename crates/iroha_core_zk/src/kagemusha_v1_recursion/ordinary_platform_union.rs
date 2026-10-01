//! Fixed Android/App Attest union of genuine original platform equations.
//!
//! Both equations execute. A credential-constrained selector sends the active original to its
//! equation and a fixed public mathematical pad to the other. Pads cannot enter the output
//! transcript or authorize a credential; they avoid exceptional invalid inactive curve inputs.

use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    ordinary_app_guard_binding::OrdinaryCredentialOriginalCellsV1,
    ordinary_platform_equation::{
        OrdinaryPlatformSignatureCellsV1, constrain_original_android_approval_stream_v1,
        constrain_original_apple_approval_stream_v1,
    },
};
use crate::{
    kagemusha_p256_curve_gadget::{P256_LIMB_BITS, P256_NUM_LIMBS},
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use ff::{Field as _, PrimeField};
use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
    utils::CurveAffineExt as _,
};
use halo2_ecc::{
    ecc::EcPoint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::halo2curves::{
    CurveAffine as _,
    secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1Affine},
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaAppOperationApprovalV1, kagemusha_app_attest_original_parts_v1,
};
use p256::ecdsa::Signature;
use sha2::{Digest as _, Sha256};

fn field_be<T: PrimeField>(raw: &[u8]) -> Result<T, String> {
    let mut repr = T::Repr::default();
    if raw.len() != repr.as_ref().len() {
        return Err("ordinary platform field width".into());
    }
    for (a, b) in repr.as_mut().iter_mut().zip(raw.iter().rev()) {
        *a = *b;
    }
    Option::<T>::from(T::from_repr(repr)).ok_or("ordinary platform noncanonical field".into())
}
fn scalar_sha(raw: &[u8]) -> (P256Scalar, u64) {
    let raw: [u8; 32] = Sha256::digest(raw).into();
    let z = raw.iter().fold(P256Scalar::ZERO, |x, b| {
        x * P256Scalar::from(256) + P256Scalar::from(u64::from(*b))
    });
    let mut le = raw;
    le.reverse();
    (
        z,
        u64::from(Option::<P256Scalar>::from(P256Scalar::from_repr(le)).is_none()),
    )
}
fn be<T: PrimeField>(v: T) -> Vec<u8> {
    let mut b = v.to_repr().as_ref().to_vec();
    b.reverse();
    b
}
struct Pad {
    wrapper: [u8; A::TOTAL_BYTES],
    key: [u8; 65],
    auth: [u8; 37],
    der: Vec<u8>,
    assertion: Vec<u8>,
}
fn pad(apple: bool) -> Result<Pad, String> {
    let mut wrapper = [0; A::TOTAL_BYTES];
    wrapper[A::DOMAIN]
        .copy_from_slice(iroha_data_model::kagemusha::KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1);
    wrapper[A::BODY_LENGTH].copy_from_slice(&(A::BODY.len() as u64).to_le_bytes());
    wrapper[A::VERSION].copy_from_slice(&1_u16.to_le_bytes());
    wrapper[A::PURPOSE.start] = 1;
    let (x, y) = Secp256r1Affine::generator().into_coordinates();
    let mut key = [0; 65];
    key[0] = 4;
    key[1..33].copy_from_slice(&be(x));
    key[33..].copy_from_slice(&be(y));
    let mut auth = [1; 37];
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&1_u32.to_be_bytes());
    let message = if apple {
        let mut bytes = auth.to_vec();
        bytes.extend(Sha256::digest(wrapper));
        Sha256::digest(bytes).to_vec()
    } else {
        wrapper.to_vec()
    };
    // Q=G and k=1 are fixed public padding only: r=G.x, s=SHA(message)+r. No key
    // generation or arbitrary signing API exists, and all selected output bytes exclude this pad.
    let r = field_be::<P256Scalar>(&key[1..33])?;
    let (z, _) = scalar_sha(&message);
    let s = z + r;
    let sig = Signature::from_scalars(
        <[u8; 32]>::try_from(be(r)).unwrap(),
        <[u8; 32]>::try_from(be(s)).unwrap(),
    )
    .map_err(|_| "ordinary public pad signature shape")?;
    let der = sig.to_der().as_bytes().to_vec();
    let mut assertion = vec![0xa2, 0x69];
    assertion.extend(b"signature");
    assertion.extend([0x58, der.len() as u8]);
    assertion.extend(&der);
    assertion.push(0x71);
    assertion.extend(b"authenticatorData");
    assertion.extend([0x58, 37]);
    assertion.extend(auth);
    Ok(Pad {
        wrapper,
        key,
        auth,
        der,
        assertion,
    })
}

/// Return only the original active platform stream. Its selector is bound to the full credential.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_platform_union_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    original_key: &[u8; 65],
    approval: &KagemushaAppOperationApprovalV1,
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    apple: AssignedValue<F>,
    previous_counter: Option<u32>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let actual_apple = matches!(
        &approval.evidence,
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { .. }
    );
    if actual_apple && previous_counter.is_none() {
        return Err("ordinary Apple independent counter floor absent".into());
    }
    let mut streams = Vec::new();
    for branch_apple in [false, true] {
        let pad = pad(branch_apple)?;
        let active = if branch_apple {
            apple
        } else {
            let range = builder.range_chip();
            range.gate().not(builder.main(0), apple)
        };
        let is_active = branch_apple == actual_apple;
        let (raw, auth, der) = if is_active {
            match &approval.evidence {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                    (signature_der.as_slice(), None, signature_der.as_slice())
                }
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                    let (a, d) = kagemusha_app_attest_original_parts_v1(raw_assertion)?;
                    (raw_assertion.as_slice(), Some(a), d)
                }
            }
        } else {
            (
                if branch_apple {
                    pad.assertion.as_slice()
                } else {
                    pad.der.as_slice()
                },
                Some(pad.auth.as_slice()),
                pad.der.as_slice(),
            )
        };
        let native_wrapper = if is_active {
            approval.challenge.canonical_signing_bytes()?
        } else {
            pad.wrapper.to_vec()
        };
        let native_key = if is_active { *original_key } else { pad.key };
        let sig = Signature::from_der(der).map_err(|_| "ordinary original DER shape")?;
        let r = field_be::<P256Scalar>(&sig.r().to_bytes())?;
        let s = field_be::<P256Scalar>(&sig.s().to_bytes())?;
        let digest_message = if branch_apple {
            let mut b = auth.ok_or("ordinary Apple auth absent")?.to_vec();
            b.extend(Sha256::digest(&native_wrapper));
            Sha256::digest(b).to_vec()
        } else {
            native_wrapper.clone()
        };
        let (z, quotient) = scalar_sha(&digest_message);
        let range = builder.range_chip();
        let base = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
        let scalar = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
        let ctx = builder.main(0);
        let key = EcPoint::new(
            base.load_private(ctx, field_be(&native_key[1..33])?),
            base.load_private(ctx, field_be(&native_key[33..])?),
        );
        let r = scalar.load_private(ctx, r);
        let s = scalar.load_private(ctx, s);
        let z = scalar.load_private(ctx, z);
        let selected_key = core::array::from_fn(|i| {
            range.gate().select(
                ctx,
                credential.app_public_key[i].quantum_cell(),
                Constant(F::from(u64::from(pad.key[i]))),
                active,
            )
        });
        let selected_wrapper = core::array::from_fn(|i| {
            let b = range.gate().select(
                ctx,
                wrapper[i].quantum_cell(),
                Constant(F::from(u64::from(pad.wrapper[i]))),
                active,
            );
            PastaSha256ByteV1::range_checked(ctx, &range, b)
        });
        let signature = OrdinaryPlatformSignatureCellsV1 {
            signature_public_key: &key,
            enrolled_public_key: &key,
            enrolled_public_key_sec1: &selected_key,
            r: &r,
            s: &s,
            z: &z,
            digest_reduction_quotient: ctx.load_witness(F::from(quotient)),
        };
        let stream = if branch_apple {
            let auth_raw: [u8; 37] = auth
                .ok_or("ordinary Apple authData absent")?
                .try_into()
                .map_err(|_| "ordinary Apple authData width")?;
            let auth_cells =
                core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(auth_raw[i]))));
            let rp = core::array::from_fn(|i| {
                range.gate().select(
                    ctx,
                    credential.fixed_digests[11][i].quantum_cell(),
                    Constant(F::from(u64::from(pad.auth[i]))),
                    active,
                )
            });
            let native_floor = previous_counter.unwrap_or(0);
            let floor_raw = ctx.load_witness(F::from(u64::from(native_floor)));
            let floor = range
                .gate()
                .select(ctx, floor_raw, Constant(F::ZERO), active);
            let credential_floor = range.gate().inner_product(
                ctx,
                credential.scalars[4].iter().map(|byte| byte.quantum_cell()),
                (0..4).map(|i| Constant(F::from(1_u64 << (8 * i)))),
            );
            range.range_check(ctx, credential_floor, 32);
            let below_enrollment = range.is_less_than(ctx, floor, credential_floor, 32);
            let active_below_enrollment = range.gate().mul(ctx, active, below_enrollment);
            range
                .gate()
                .assert_is_const(ctx, &active_below_enrollment, &F::ZERO);
            let counter = ctx.load_witness(F::from(u64::from(u32::from_be_bytes(
                auth_raw[33..].try_into().unwrap(),
            ))));
            let selected_wrapper = selected_wrapper.map(|b| b.assigned().unwrap());
            constrain_original_apple_approval_stream_v1(
                builder,
                jobs,
                raw,
                &selected_wrapper,
                &auth_cells,
                &rp,
                floor,
                counter,
                &signature,
            )?
        } else {
            constrain_original_android_approval_stream_v1(
                builder,
                jobs,
                raw,
                &selected_wrapper,
                &signature,
            )?
        };
        streams.push(stream);
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let android = &streams[0];
    let apple_stream = &streams[1];
    let length = range
        .gate()
        .select(ctx, apple_stream.actual_len(), android.actual_len(), apple);
    let bytes = (0..142)
        .map(|i| {
            let a = android
                .bytes()
                .get(i)
                .copied()
                .unwrap_or(PastaSha256ByteV1::constant(0));
            let b = apple_stream.bytes()[i];
            let selected = range
                .gate()
                .select(ctx, b.quantum_cell(), a.quantum_cell(), apple);
            PastaSha256ByteV1::range_checked(ctx, &range, selected)
        })
        .collect();
    KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{VerifyingKey, signature::Verifier as _};

    #[test]
    fn fixed_public_inactive_pads_verify_only_their_exact_constant_messages() {
        for apple in [false, true] {
            let pad = pad(apple).unwrap();
            let key = VerifyingKey::from_sec1_bytes(&pad.key).unwrap();
            let signature = Signature::from_der(&pad.der).unwrap();
            let message = if apple {
                let mut b = pad.auth.to_vec();
                b.extend(Sha256::digest(pad.wrapper));
                Sha256::digest(b).to_vec()
            } else {
                pad.wrapper.to_vec()
            };
            key.verify(&message, &signature).unwrap();
            let mut changed = message.clone();
            changed[0] ^= 1;
            assert!(key.verify(&changed, &signature).is_err());
            if apple {
                let (auth, der) = kagemusha_app_attest_original_parts_v1(&pad.assertion).unwrap();
                assert_eq!(auth, pad.auth);
                assert_eq!(der, pad.der);
            }
        }
    }
}
