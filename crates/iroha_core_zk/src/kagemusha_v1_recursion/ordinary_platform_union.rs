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
        constrain_original_android_signed_message_stream_v1,
        constrain_original_apple_approval_stream_v1,
        constrain_original_apple_signed_message_stream_v1,
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
use halo2_proofs::halo2curves::secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1Affine};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaAppOperationApprovalV1, kagemusha_ordinary_apple_original_parts_v1,
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
    wrapper: Vec<u8>,
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
    pad_for_message(apple, wrapper.to_vec())
}
/// Fixed public inactive signature equation. Its point/scalar are never an app/receiver key.
fn pad_for_message(apple: bool, wrapper: Vec<u8>) -> Result<Pad, String> {
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
        bytes.extend(Sha256::digest(&wrapper));
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
    expected_release_digest: [u8; 32],
    approval: &KagemushaAppOperationApprovalV1,
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    apple: AssignedValue<F>,
    previous_counter: Option<u32>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let native = approval.challenge.canonical_signing_bytes()?;
    constrain_ordinary_signed_message_union_v1(
        builder,
        jobs,
        credential,
        original_key,
        expected_release_digest,
        &approval.evidence,
        &native,
        wrapper,
        apple,
        previous_counter,
        None,
        true,
    )
    .map(|streams| streams.active_original)
}

/// Private deterministic codec operand for the disabled receiver branch only. Its generator
/// signature is mathematical padding, never an original receiver/platform assertion or grant.
pub(super) fn inactive_receiver_codec_evidence_v1(
    apple: bool,
) -> Result<KagemushaAppOperationApprovalEvidenceV1, String> {
    let pad = pad_for_message(
        apple,
        vec![
            0;
            iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_DOMAIN_V1.len()
                + 8
                + 390
        ],
    )?;
    Ok(if apple {
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
            raw_assertion: pad.assertion,
        }
    } else {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: pad.der,
        }
    })
}

/// Mathematical stream outputs. Only active_original can carry an original receiver grant.
/// Disabled mathematical_codec_original contains a fixed public padding equation's bytes;
/// enclosing public digest bindings must independently require the assigned Send selector.
pub(super) struct OrdinarySignedMessageStreamsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) active_original: KagemushaBoundedByteStreamV1<F>,
    pub(super) mathematical_codec_original: KagemushaBoundedByteStreamV1<F>,
}

/// Fixed two-platform equation over a separately framed, fully assigned signing message.
/// Optional enabled only selects a zero inactive output; public pads grant no receiver authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_signed_message_union_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    original_key: &[u8; 65],
    expected_release_digest: [u8; 32],
    evidence: &KagemushaAppOperationApprovalEvidenceV1,
    native_message: &[u8],
    wrapper: &[PastaSha256ByteV1<F>],
    apple: AssignedValue<F>,
    previous_counter: Option<u32>,
    enabled: Option<(AssignedValue<F>, bool)>,
    operation_wrapper: bool,
) -> Result<OrdinarySignedMessageStreamsV1<F>, String> {
    let actual_apple = matches!(
        evidence,
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { .. }
    );
    let native_enabled = enabled.is_none_or(|(_, v)| v);
    if native_message.len() != wrapper.len() || wrapper.is_empty() || wrapper.len() > 1024 {
        return Err("ordinary signed message fixed capacity differs".into());
    }
    if actual_apple && native_enabled && previous_counter.is_none() {
        return Err("ordinary Apple independent counter floor absent".into());
    }
    let mut streams = Vec::new();
    for branch_apple in [false, true] {
        let pad = if operation_wrapper {
            pad(branch_apple)?
        } else {
            pad_for_message(branch_apple, vec![0; wrapper.len()])?
        };
        let active = if branch_apple {
            apple
        } else {
            let range = builder.range_chip();
            range.gate().not(builder.main(0), apple)
        };
        let active = if let Some((enabled, _)) = enabled {
            let range = builder.range_chip();
            range.gate().assert_bit(builder.main(0), enabled);
            range.gate().mul(builder.main(0), active, enabled)
        } else {
            active
        };
        let is_active = native_enabled && branch_apple == actual_apple;
        let (raw, auth, der) = if is_active {
            match evidence {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                    (signature_der.as_slice(), None, signature_der.as_slice())
                }
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                    let parts = kagemusha_ordinary_apple_original_parts_v1(
                        raw_assertion,
                        expected_release_digest,
                    )?;
                    (
                        raw_assertion.as_slice(),
                        Some(parts.authenticator_data),
                        parts.signature_der,
                    )
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
            native_message.to_vec()
        } else {
            pad.wrapper.clone()
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
        let selected_wrapper = wrapper
            .iter()
            .enumerate()
            .map(|(i, byte)| {
                let b = range.gate().select(
                    ctx,
                    byte.quantum_cell(),
                    Constant(F::from(u64::from(pad.wrapper[i]))),
                    active,
                );
                PastaSha256ByteV1::range_checked(ctx, &range, b)
            })
            .collect::<Vec<_>>();
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
                .get(..37)
                .ok_or("ordinary Apple authData header absent")?
                .try_into()
                .map_err(|_| "ordinary Apple authData header width")?;
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
            // The active branch receives the independently governed original credential cells;
            // inactive public padding retains explicit unavailable release measurement.
            let release = core::array::from_fn(|i| {
                range.gate().select(
                    ctx,
                    credential.fixed_digests[12][i].quantum_cell(),
                    Constant(F::ZERO),
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
            let selected_wrapper = selected_wrapper
                .into_iter()
                .map(|b| b.assigned().unwrap())
                .collect::<Vec<_>>();
            if operation_wrapper {
                constrain_original_apple_approval_stream_v1(
                    builder,
                    jobs,
                    raw,
                    selected_wrapper
                        .as_slice()
                        .try_into()
                        .map_err(|_| "ordinary approval message width")?,
                    &auth_cells,
                    &rp,
                    if is_active {
                        expected_release_digest
                    } else {
                        [0; 32]
                    },
                    &release,
                    floor,
                    counter,
                    &signature,
                )?
            } else {
                constrain_original_apple_signed_message_stream_v1(
                    builder,
                    jobs,
                    raw,
                    &selected_wrapper,
                    None,
                    &auth_cells,
                    &rp,
                    if is_active {
                        expected_release_digest
                    } else {
                        [0; 32]
                    },
                    &release,
                    floor,
                    counter,
                    &signature,
                )?
            }
        } else if operation_wrapper {
            constrain_original_android_approval_stream_v1(
                builder,
                jobs,
                raw,
                selected_wrapper
                    .as_slice()
                    .try_into()
                    .map_err(|_| "ordinary approval message width")?,
                &signature,
            )?
        } else {
            constrain_original_android_signed_message_stream_v1(
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
    let bytes = (0..311)
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
    let mathematical_codec_original =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, length)?;
    let active_original = if let Some((enabled, _)) = enabled {
        let bytes = mathematical_codec_original
            .bytes()
            .iter()
            .map(|b| {
                let cell = range
                    .gate()
                    .select(ctx, b.quantum_cell(), Constant(F::ZERO), enabled);
                PastaSha256ByteV1::range_checked(ctx, &range, cell)
            })
            .collect();
        let length = range.gate().select(
            ctx,
            mathematical_codec_original.actual_len(),
            Constant(F::ZERO),
            enabled,
        );
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, length)?
    } else {
        mathematical_codec_original.clone()
    };
    Ok(OrdinarySignedMessageStreamsV1 {
        active_original,
        mathematical_codec_original,
    })
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
                b.extend(Sha256::digest(&pad.wrapper));
                Sha256::digest(b).to_vec()
            } else {
                pad.wrapper.to_vec()
            };
            key.verify(&message, &signature).unwrap();
            let mut changed = message.clone();
            changed[0] ^= 1;
            assert!(key.verify(&changed, &signature).is_err());
            if apple {
                let parts =
                    kagemusha_ordinary_apple_original_parts_v1(&pad.assertion, [0; 32]).unwrap();
                assert_eq!(parts.authenticator_data, pad.auth);
                assert_eq!(parts.signature_der, pad.der);
                assert_eq!(
                    parts.release_measurement,
                    iroha_data_model::kagemusha::KagemushaAppAttestReleaseMeasurementV1::Unavailable,
                );
            }
        }
    }
    #[test]
    fn fixed_receiver_message_pads_verify_only_exact_public_math_messages() {
        for width in [390, 448, 511] {
            let message = (0..width)
                .map(|i| (i as u8).wrapping_mul(7))
                .collect::<Vec<_>>();
            for apple in [false, true] {
                let pad = pad_for_message(apple, message.clone()).unwrap();
                let key = VerifyingKey::from_sec1_bytes(&pad.key).unwrap();
                let sig = Signature::from_der(&pad.der).unwrap();
                let mut signing = message.clone();
                if apple {
                    let mut b = pad.auth.to_vec();
                    b.extend(Sha256::digest(&signing));
                    signing = Sha256::digest(b).to_vec();
                }
                key.verify(&signing, &sig).unwrap();
                signing[0] ^= 1;
                assert!(key.verify(&signing, &sig).is_err());
            }
        }
    }
}
