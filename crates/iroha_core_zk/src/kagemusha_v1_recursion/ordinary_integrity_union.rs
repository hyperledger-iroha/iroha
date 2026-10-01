//! Fixed initial/refresh Integrity union with one canonical stream per original.
//!
//! An absent lease uses a fixed public mathematical issuer pad. It never enters the authorization
//! transcript. A selected lease instead authenticates the original under the release-fixed key,
//! and its complete canonical digest and unchanged credential interval enter that transcript.

use super::{
    guard_bundle::assign_bytes,
    ordinary_app_guard_binding::{
        OrdinaryCredentialIssuerCellsV1, OrdinaryCredentialOriginalCellsV1,
    },
    ordinary_integrity_binding::{
        OrdinaryIntegrityLeaseCellsV1, constrain_ordinary_initial_integrity_interval_v1,
    },
    ordinary_integrity_stream::reconstruct_ordinary_integrity_stream_v1,
    ordinary_issuer_equation::constrain_ordinary_issuer_original_v1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
    utils::CurveAffineExt as _,
};
use halo2_proofs::halo2curves::{
    CurveAffine as _,
    secp256r1::{Fq as P256Scalar, Secp256r1Affine},
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaDeviceSignatureV1,
    KagemushaOrdinaryIssuerCircuitAdmissionV1, KagemushaPlayIntegrityBindingV1,
    KagemushaPlayIntegrityRefreshLeaseSubjectV1, KagemushaPlayIntegrityRefreshLeaseV1,
};
use sha2::{Digest as _, Sha256};

fn inactive_pad() -> Result<(KagemushaPlayIntegrityRefreshLeaseV1, [u8; 65]), String> {
    let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
        version: 1,
        credential_digest: [1; 32],
        challenge_digest: [1; 32],
        attested_key_id: [1; 32],
        release_id: [1; 32],
        hardware_profile_id: [1; 32],
        trust_policy_digest: [1; 32],
        app_authority_policy_digest: [1; 32],
        binding: KagemushaPlayIntegrityBindingV1 {
            request_hash: [1; 32],
            evidence_digest: [1; 32],
            policy_digest: [1; 32],
            verified_at_ms: 1,
            refresh_before_ms: 2,
        },
        possession_original_digest: Sha256::digest([0x5a; 8]).into(),
        policy_epoch: 1,
        hardware_epoch: 1,
        issued_at_ms: 1,
        expires_at_ms: 2,
    };
    let signature = iroha_crypto::Signature::from_bytes(&[0; 64]);
    let app_possession = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
        signature_der: vec![0x5a; 8],
    };
    let admission = KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
        &subject,
        &signature,
        &app_possession,
    )?;
    let (x, y) = Secp256r1Affine::generator().into_coordinates();
    let be = |v: &dyn AsRef<[u8]>| {
        let mut bytes = v.as_ref().to_vec();
        bytes.reverse();
        bytes
    };
    let mut key = [0; 65];
    key[0] = 4;
    key[1..33].copy_from_slice(&be(&x.to_repr()));
    key[33..].copy_from_slice(&be(&y.to_repr()));
    let mut le = [0; 32];
    le.copy_from_slice(&key[1..33]);
    le.reverse();
    let r = Option::<P256Scalar>::from(P256Scalar::from_repr(le))
        .ok_or("ordinary inactive issuer r shape")?;
    let z = Sha256::digest(admission.canonical_signing_bytes()?)
        .iter()
        .fold(P256Scalar::ZERO, |v, b| {
            v * P256Scalar::from(256) + P256Scalar::from(u64::from(*b))
        });
    // Fixed Q=G,k=1 public padding only. It is not a governed issuer and cannot be selected.
    let sig = p256::ecdsa::Signature::from_scalars(
        <[u8; 32]>::try_from(be(&r.to_repr())).unwrap(),
        <[u8; 32]>::try_from(be(&(z + r).to_repr())).unwrap(),
    )
    .map_err(|_| "ordinary inactive issuer signature shape")?;
    let sig = sig.normalize_s().unwrap_or(sig);
    Ok((
        KagemushaPlayIntegrityRefreshLeaseV1 {
            subject,
            signature,
            app_possession,
            circuit_admission: KagemushaOrdinaryIssuerCircuitAdmissionV1 {
                subject: admission,
                signature: KagemushaDeviceSignatureV1::from_raw_bytes(sig.to_bytes().as_ref())
                    .map_err(|e| e.to_string())?,
            },
        },
        key,
    ))
}

fn uint64<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) -> Result<AssignedValue<F>, String> {
    if bytes.len() != 8 {
        return Err("ordinary Integrity integer width differs".into());
    }
    let value = range.gate().inner_product(
        ctx,
        bytes.iter().map(|b| b.quantum_cell()),
        (0..8).map(|i| Constant(F::from(1u64 << (8 * i)))),
    );
    range.range_check(ctx, value, 64);
    Ok(value)
}

/// The issuer argument is fixed by the enclosing circuit type: Guard authenticates; State copies.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_integrity_union_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    credential_digest: &[PastaSha256ByteV1<F>; 32],
    integrity: AssignedValue<F>,
    raw: Option<&KagemushaPlayIntegrityRefreshLeaseV1>,
    issuer: Option<(&[u8; 65], &[AssignedValue<F>; 65])>,
    approval_issued: AssignedValue<F>,
    approval_expires: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let (pad, pad_key) = inactive_pad()?;
    let actual = raw.unwrap_or(&pad);
    let ed_layout = actual.ed_only_preimage_layout()?;
    let ed_raw = actual.ed_only_canonical_bytes()?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let active = ctx.load_witness(F::from(u64::from(raw.is_some())));
    range.gate().assert_bit(ctx, active);
    let inactive = range.gate().not(ctx, active);
    let no_policy = range.gate().mul_not(ctx, integrity, active);
    range.gate().assert_is_const(ctx, &no_policy, &F::ZERO);
    let mut positions = |indices: &[usize]| {
        assign_bytes(
            ctx,
            &range,
            &indices.iter().map(|i| ed_raw[*i]).collect::<Vec<_>>(),
        )
    };
    let mut cells = OrdinaryIntegrityLeaseCellsV1 {
        version: positions(&ed_layout.version_bytes)
            .try_into()
            .map_err(|_| "ordinary lease version width")?,
        fields: core::array::from_fn(|i| {
            positions(&ed_layout.fixed_digest_bytes[i])
                .try_into()
                .expect("lease raw32")
        }),
        scalars: core::array::from_fn(|i| {
            positions(&ed_layout.scalar_bytes[i])
                .try_into()
                .expect("lease raw64")
        }),
        signature: positions(&ed_layout.signature_bytes)
            .try_into()
            .map_err(|_| "ordinary lease Ed width")?,
        possession: Vec::new(),
        issuer_admission: None,
    };
    let der = match &actual.app_possession {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => signature_der,
        _ => return Err("ordinary Integrity original platform differs".into()),
    };
    if !(8..=72).contains(&der.len()) {
        return Err("ordinary Integrity DER width differs".into());
    }
    cells.possession = assign_bytes(
        ctx,
        &range,
        &(0..72)
            .map(|i| der.get(i).copied().unwrap_or(0))
            .collect::<Vec<_>>(),
    );
    let der_length = ctx.load_witness(F::from(der.len() as u64));
    let ed_digest = reconstruct_ordinary_integrity_stream_v1(
        builder,
        jobs,
        &actual.ed_only_canonical_stream_grammar()?,
        &cells,
        der_length,
    )?;
    let signature = assign_bytes(
        builder.main(0),
        &range,
        actual.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "ordinary lease issuer width")?;
    if let Some((gov_raw, gov_cells)) = issuer {
        let ctx = builder.main(0);
        let selected = core::array::from_fn(|i| {
            range.gate().select(
                ctx,
                gov_cells[i],
                Constant(F::from(u64::from(pad_key[i]))),
                active,
            )
        });
        let key = if raw.is_some() { gov_raw } else { &pad_key };
        constrain_ordinary_issuer_original_v1(
            builder,
            jobs,
            2,
            &cells.fields[3],
            &cells.fields[4],
            &ed_digest,
            &signature,
            &actual.circuit_admission,
            key,
            &selected,
        )?;
    }
    cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed_digest,
        signature,
    });
    let full_digest = reconstruct_ordinary_integrity_stream_v1(
        builder,
        jobs,
        &actual.original_canonical_stream_grammar()?,
        &cells,
        der_length,
    )?;
    let ctx = builder.main(0);
    // Original scope/epoch/interval joins are conditional, but every branch always executes.
    let equal_active = |ctx: &mut halo2_base::Context<F>,
                        a: &[PastaSha256ByteV1<F>],
                        b: &[PastaSha256ByteV1<F>]|
     -> Result<(), String> {
        if a.len() != b.len() {
            return Err("ordinary Integrity scope width differs".into());
        }
        for (a, b) in a.iter().zip(b) {
            let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
            let d = range.gate().mul(ctx, d, active);
            range.gate().assert_is_const(ctx, &d, &F::ZERO);
        }
        Ok(())
    };
    for (a, b) in [
        (&cells.fields[0], credential_digest),
        (&cells.fields[2], &credential.fixed_digests[13]),
        (&cells.fields[3], &credential.fixed_digests[6]),
        (&cells.fields[4], &credential.fixed_digests[7]),
        (&cells.fields[5], &credential.fixed_digests[9]),
        (&cells.fields[6], &credential.fixed_digests[10]),
    ] {
        equal_active(ctx, a, b)?;
    }
    let pi = credential
        .play_integrity_fields
        .as_ref()
        .ok_or("ordinary fixed Integrity cells absent")?;
    equal_active(ctx, &cells.fields[9], &pi[2])?;
    for i in 0..2 {
        equal_active(ctx, &cells.scalars[i], &credential.scalars[i])?;
    }
    for (a, b) in cells.version.iter().zip([1, 0]) {
        let d = range
            .gate()
            .sub(ctx, a.quantum_cell(), Constant(F::from(b)));
        range.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
    let times = core::array::from_fn::<_, 4, _>(|i| {
        uint64(ctx, &range, &cells.scalars[i + 2]).expect("lease time")
    });
    let ci = uint64(ctx, &range, &credential.scalars[2])?;
    let ce = uint64(ctx, &range, &credential.scalars[3])?;
    for (before, after) in [
        (times[0], times[2]),
        (times[2], approval_issued),
        (ci, times[2]),
        (approval_expires, times[1]),
        (approval_expires, times[3]),
        (times[3], times[1]),
        (times[3], ce),
    ] {
        let bad = range.is_less_than(ctx, after, before, 64);
        let bad = range.gate().mul(ctx, bad, active);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
    let live = range.is_less_than(ctx, times[2], times[3], 64);
    let dead = range.gate().mul_not(ctx, live, active);
    range.gate().assert_is_const(ctx, &dead, &F::ZERO);
    // The full issuer original hashes this exact DER stream. Native additionally verifies its
    // actual enrolled-key equation and Google source admission before lending the lease.
    let words = jobs.digest_bounded_constrained(ctx, &range, &cells.possession, der_length)?;
    let mut sha = Vec::new();
    for word in words {
        let bits = crate::pasta_sha256::PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            sha.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    equal_active(ctx, &sha, &cells.fields[10])?;
    let initial = range.gate().mul(ctx, integrity, inactive);
    let selected = full_digest.map(|b| {
        let v = range.gate().mul(ctx, b.quantum_cell(), active);
        PastaSha256ByteV1::range_checked(ctx, &range, v)
    });
    constrain_ordinary_initial_integrity_interval_v1(
        builder,
        credential,
        initial,
        approval_issued,
        approval_expires,
    )?;
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_public_issuer_pad_authenticates_only_its_fixed_message() {
        use p256::ecdsa::signature::Verifier as _;
        let (pad, key) = inactive_pad().unwrap();
        let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(&key).unwrap();
        let sig =
            p256::ecdsa::Signature::from_slice(pad.circuit_admission.signature.as_raw_bytes())
                .unwrap();
        let message = pad
            .circuit_admission
            .subject
            .canonical_signing_bytes()
            .unwrap();
        key.verify(&message, &sig).unwrap();
        let mut foreign = message;
        *foreign.last_mut().unwrap() ^= 1;
        assert!(key.verify(&foreign, &sig).is_err());
        assert!(pad.canonical_bytes().is_ok());
    }
}
