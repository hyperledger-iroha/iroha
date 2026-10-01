//! Fixed None/Some Integrity and Android/Apple original credential topology.
//!
//! Both canonical option frames are reconstructed from the same assigned semantic bytes before
//! a constrained option selector chooses the SHA. No witness-specific framing enters a key.

use super::{
    guard_bundle::assign_bytes,
    ordinary_app_guard_binding::{
        OrdinaryCredentialOriginalCellsV1, reconstruct_ordinary_credential_original_v1,
    },
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KagemushaAppKeySecurityLevelV1, KagemushaHardwarePlatformClassV1,
    KagemushaOrdinaryAppCredentialV1, KagemushaPlayIntegrityBindingV1,
};

pub(super) struct OrdinaryCredentialUnionV1<F: KagemushaPoseidonFieldV1> {
    pub(super) cells: OrdinaryCredentialOriginalCellsV1<F>,
    pub(super) apple: AssignedValue<F>,
    pub(super) integrity: AssignedValue<F>,
}

pub(super) fn assign_ordinary_credential_union_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    raw: &KagemushaOrdinaryAppCredentialV1,
) -> Result<OrdinaryCredentialUnionV1<F>, String> {
    let layout = raw.original_preimage_layout()?;
    let mut preimage = layout.bytes[..layout.original.start]
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .ok_or("ordinary original prefix differs")?;
    preimage.extend(raw.canonical_bytes()?);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut positions = |indices: &[usize]| {
        assign_bytes(
            ctx,
            &range,
            &indices.iter().map(|i| preimage[*i]).collect::<Vec<_>>(),
        )
    };
    let mut cells = OrdinaryCredentialOriginalCellsV1 {
        version: positions(&layout.version_bytes)
            .try_into()
            .map_err(|_| "ordinary version width")?,
        platform_class: positions(&layout.platform_class_bytes),
        security_level: positions(&layout.security_level_bytes),
        fixed_digests: core::array::from_fn(|i| {
            positions(&layout.fixed_digest_bytes[i])
                .try_into()
                .expect("model raw32")
        }),
        app_public_key: positions(&layout.app_public_key_bytes)
            .try_into()
            .map_err(|_| "ordinary SEC1 width")?,
        scalars: core::array::from_fn(|i| positions(&layout.scalar_bytes[i])),
        original_ed_signature: positions(&layout.signature_bytes)
            .try_into()
            .map_err(|_| "ordinary Ed width")?,
        play_integrity_fields: None,
        issuer_admission: None,
    };
    let pi = raw.subject.play_integrity;
    let pi = pi.unwrap_or(KagemushaPlayIntegrityBindingV1 {
        request_hash: [0; 32],
        evidence_digest: [0; 32],
        policy_digest: [0; 32],
        verified_at_ms: 0,
        refresh_before_ms: 0,
    });
    cells.play_integrity_fields = Some([
        assign_bytes(ctx, &range, &pi.request_hash),
        assign_bytes(ctx, &range, &pi.evidence_digest),
        assign_bytes(ctx, &range, &pi.policy_digest),
        assign_bytes(ctx, &range, &pi.verified_at_ms.to_le_bytes()),
        assign_bytes(ctx, &range, &pi.refresh_before_ms.to_le_bytes()),
    ]);
    let apple = ctx.load_witness(F::from(u64::from(
        raw.subject.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest,
    )));
    let strongbox = ctx.load_witness(F::from(u64::from(
        raw.subject.security_level == KagemushaAppKeySecurityLevelV1::StrongBox,
    )));
    let integrity = ctx.load_witness(F::from(u64::from(raw.subject.play_integrity.is_some())));
    let gate = range.gate();
    for bit in [apple, strongbox, integrity] {
        gate.assert_bit(ctx, bit);
    }
    for other in [strongbox, integrity] {
        let forbidden = gate.mul(ctx, apple, other);
        gate.assert_is_const(ctx, &forbidden, &F::ZERO);
    }
    let mut android = raw.clone();
    android.subject.platform_class = KagemushaHardwarePlatformClassV1::AndroidKeyMint;
    android.subject.security_level = KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment;
    let mut apple_raw = android.clone();
    apple_raw.subject.platform_class = KagemushaHardwarePlatformClassV1::AppleAppAttest;
    apple_raw.subject.security_level = KagemushaAppKeySecurityLevelV1::AppleAppAttest;
    let mut strongbox_raw = android.clone();
    strongbox_raw.subject.security_level = KagemushaAppKeySecurityLevelV1::StrongBox;
    fn tags(raw: &KagemushaOrdinaryAppCredentialV1) -> Result<(Vec<u8>, Vec<u8>), String> {
        let l = raw.ed_only_preimage_layout()?;
        let b = raw.ed_only_canonical_bytes()?;
        Ok((
            l.platform_class_bytes.iter().map(|i| b[*i]).collect(),
            l.security_level_bytes.iter().map(|i| b[*i]).collect(),
        ))
    }
    let (android_tag, tee_tag) = tags(&android)?;
    let (apple_tag, apple_level) = tags(&apple_raw)?;
    let (_, strongbox_level) = tags(&strongbox_raw)?;
    if android_tag.len() != apple_tag.len()
        || android_tag.len() != cells.platform_class.len()
        || tee_tag.len() != apple_level.len()
        || tee_tag.len() != strongbox_level.len()
        || tee_tag.len() != cells.security_level.len()
    {
        return Err("ordinary platform enum union encoder widths differ".into());
    }
    for ((cell, a), b) in cells.platform_class.iter().zip(android_tag).zip(apple_tag) {
        let expected = gate.select(
            ctx,
            Constant(F::from(u64::from(b))),
            Constant(F::from(u64::from(a))),
            apple,
        );
        let difference = gate.sub(ctx, cell.quantum_cell(), expected);
        gate.assert_is_const(ctx, &difference, &F::ZERO);
    }
    for (((cell, tee), sb), app) in cells
        .security_level
        .iter()
        .zip(tee_tag)
        .zip(strongbox_level)
        .zip(apple_level)
    {
        let android_level = gate.select(
            ctx,
            Constant(F::from(u64::from(sb))),
            Constant(F::from(u64::from(tee))),
            strongbox,
        );
        let expected = gate.select(ctx, Constant(F::from(u64::from(app))), android_level, apple);
        let difference = gate.sub(ctx, cell.quantum_cell(), expected);
        gate.assert_is_const(ctx, &difference, &F::ZERO);
    }
    for byte in cells
        .play_integrity_fields
        .as_ref()
        .expect("fixed union PI cells")
        .iter()
        .flatten()
    {
        let absent = gate.mul_not(ctx, integrity, byte.quantum_cell());
        gate.assert_is_const(ctx, &absent, &F::ZERO);
    }
    Ok(OrdinaryCredentialUnionV1 {
        cells,
        apple,
        integrity,
    })
}

pub(super) fn select_ordinary_digest_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    bit: AssignedValue<F>,
    yes: &[PastaSha256ByteV1<F>; 32],
    no: &[PastaSha256ByteV1<F>; 32],
) -> [PastaSha256ByteV1<F>; 32] {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    core::array::from_fn(|i| {
        let selected = range
            .gate()
            .select(ctx, yes[i].quantum_cell(), no[i].quantum_cell(), bit);
        PastaSha256ByteV1::range_checked(ctx, &range, selected)
    })
}

/// Compute both model-owned canonical frames, including their CRCs and full issuer originals.
/// `full` is a caller-fixed relation choice, never a witness-selected circuit construction branch.
pub(super) fn reconstruct_ordinary_credential_union_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw: &KagemushaOrdinaryAppCredentialV1,
    union: &OrdinaryCredentialUnionV1<F>,
    full: bool,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let mut none = raw.clone();
    none.subject.play_integrity = None;
    let mut some = raw.clone();
    some.subject.play_integrity = Some(KagemushaPlayIntegrityBindingV1 {
        request_hash: [1; 32],
        evidence_digest: [1; 32],
        policy_digest: [1; 32],
        verified_at_ms: 1,
        refresh_before_ms: 2,
    });
    // The enum values only occupy assigned selector positions, so freeze all remaining framing
    // with one fixed Android enum specimen. The semantic enum bytes still select Android/Apple.
    for specimen in [&mut none, &mut some] {
        specimen.subject.platform_class = KagemushaHardwarePlatformClassV1::AndroidKeyMint;
        specimen.subject.security_level =
            KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment;
    }
    // Only codec metadata is borrowed from these inert specimens. The actual assigned
    // selectors, Ed digest and issuer signature supply every semantic byte and the CRC.
    let none_layout = if full {
        none.original_preimage_layout_for_specimen()?
    } else {
        none.ed_only_preimage_layout()?
    };
    let some_layout = if full {
        some.original_preimage_layout_for_specimen()?
    } else {
        some.ed_only_preimage_layout()?
    };
    let mut none_cells = union.cells.clone();
    none_cells.play_integrity_fields = None;
    let no = reconstruct_ordinary_credential_original_v1(builder, jobs, &none_layout, &none_cells)?;
    let yes =
        reconstruct_ordinary_credential_original_v1(builder, jobs, &some_layout, &union.cells)?;
    Ok(select_ordinary_digest_v1(
        builder,
        union.integrity,
        &yes,
        &no,
    ))
}

#[cfg(test)]
mod tests {
    use super::super::ordinary_app_guard_binding::OrdinaryCredentialIssuerCellsV1;
    use super::*;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

    fn shape<F: KagemushaPoseidonFieldV1>(
        apple: bool,
        pi: bool,
    ) -> (Vec<usize>, Vec<usize>, usize, usize, usize) {
        let f = if pi {
            assert!(!apple, "Integrity belongs to the Android fixture");
            KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity()
        } else {
            KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple)
        };
        let token = f.verify(300).unwrap();
        let raw = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
            token.app_credential().original(),
        )
        .unwrap();
        assert_eq!(raw.subject.play_integrity.is_some(), pi);
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(17)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let mut jobs = PastaSha256JobsV1::default();
        let mut union = assign_ordinary_credential_union_v1(&mut builder, &raw).unwrap();
        let ed =
            reconstruct_ordinary_credential_union_v1(&mut builder, &mut jobs, &raw, &union, false)
                .unwrap();
        let range = builder.range_chip();
        let signature = assign_bytes(
            builder.main(0),
            &range,
            raw.circuit_admission.signature.as_raw_bytes(),
        )
        .try_into()
        .unwrap();
        union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
            ed_original_sha256: ed,
            signature,
        });
        reconstruct_ordinary_credential_union_v1(&mut builder, &mut jobs, &raw, &union, true)
            .unwrap();
        builder.calculate_params(Some(9));
        let params = builder.config_params;
        let (job_count, compression_blocks, required_rows) = jobs.capacity_profile().unwrap();
        (
            params.num_advice_per_phase,
            params.num_lookup_advice_per_phase,
            job_count,
            compression_blocks,
            required_rows,
        )
    }
    #[test]
    fn specimen_layout_does_not_bypass_strict_actual_credential_assignment() {
        for apple in [false, true] {
            let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
            let token = f.verify(300).unwrap();
            let raw = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
                token.app_credential().original(),
            )
            .unwrap();
            let mut specimen = raw.clone();
            specimen.subject.play_integrity = Some(KagemushaPlayIntegrityBindingV1 {
                request_hash: [3; 32],
                evidence_digest: [4; 32],
                policy_digest: [5; 32],
                verified_at_ms: 10,
                refresh_before_ms: 100,
            });
            specimen.original_preimage_layout_for_specimen().unwrap();
            assert!(specimen.canonical_bytes().is_err());
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(17)
                .use_lookup_bits(16);
            assert_eq!(
                assign_ordinary_credential_union_v1(&mut builder, &specimen)
                    .err()
                    .unwrap(),
                "ordinary issuer admission original differs"
            );
            assert_eq!(
                raw.canonical_bytes().unwrap(),
                token.app_credential().original()
            );
        }
    }

    #[test]
    fn android_apple_and_integrity_option_frames_share_one_fixed_graph() {
        for apple in [false, true] {
            assert_eq!(shape::<Fp>(false, false), shape::<Fp>(apple, false));
            assert_eq!(shape::<Fq>(false, false), shape::<Fq>(apple, false));
        }
        assert_eq!(shape::<Fp>(false, false), shape::<Fp>(false, true));
        assert_eq!(shape::<Fq>(false, false), shape::<Fq>(false, true));
    }
}
