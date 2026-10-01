//! Complete ordinary Guard equations, built identically for both Pasta fields.
//!
//! The normalized predecessor/successor/amount statement and financial-secret opening use the
//! maintained Guard semantic relation. Its data projection is copy-bound to every field of the
//! genuinely admitted Ed credential original; no provider-secret or P256 private scalar is used.
//! Full original DER/App Attest CBOR authenticates the wrapper's normalized statement and S.

use super::super::{
    DigestV1,
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{
        KagemushaAssignedGuardBundleV1, KagemushaGuardBundleRelationWitnessV1, assign_bytes,
        constant_bytes, constrain_guard_bundle_semantics_v1, hash,
    },
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    ordinary_app_guard_binding::{
        OrdinaryApprovalOriginalCellsV1, OrdinaryCredentialOriginalCellsV1,
        constrain_ordinary_approval_wrapper_v1, constrain_ordinary_credential_original_v1,
    },
    ordinary_approval_proof_binding::constrain_ordinary_approval_proof_binding_v1,
    ordinary_platform_equation::{
        OrdinaryPlatformSignatureCellsV1, constrain_original_android_approval_stream_v1,
        constrain_original_apple_approval_stream_v1,
    },
};
use super::{KagemushaOrdinaryAppGuardEpCircuitV1, KagemushaOrdinaryAppGuardEqCircuitV1};
use crate::{
    kagemusha_p256_curve_gadget::{P256_LIMB_BITS, P256_NUM_LIMBS},
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use halo2_ecc::{
    ecc::EcPoint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::{
    halo2curves::{
        pasta::{EpAffine, EqAffine, Fp, Fq},
        secp256r1::{Fp as P256Base, Fq as P256Scalar},
    },
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KagemushaAppOperationApprovalEvidenceV1,
    KagemushaAppOperationApprovalSigningLayoutV1 as A, KagemushaAppOperationApprovalV1,
    KagemushaHardwareSelectionSigningLayoutV1 as S, KagemushaOrdinaryAppCredentialV1,
    kagemusha_app_attest_original_parts_v1,
};
use p256::ecdsa::Signature;
use sha2::{Digest as _, Sha256};

/// Original bounded proof material; it is not an enrollment, Native owner, or spending capability.
/// The production caller must lend it from the actual selected witness/approval holder.
pub(crate) struct OrdinaryGuardWitnessV1<'a> {
    pub(crate) relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(crate) credential: &'a KagemushaOrdinaryAppCredentialV1,
    pub(crate) approval: &'a KagemushaAppOperationApprovalV1,
    pub(crate) previous_app_attest_counter: Option<u32>,
}

pub(crate) fn build_ordinary_app_guard_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: OrdinaryGuardWitnessV1<'_>,
    provider_policy_root: DigestV1,
) -> Result<
    (
        KagemushaOrdinaryAppGuardEqCircuitV1,
        KagemushaOrdinaryAppGuardEpCircuitV1,
    ),
    String,
> {
    if provider_policy_root == [0; 32]
        || witness.relation.statement.successor_hardware_policy_id != provider_policy_root
    {
        return Err("ordinary Guard provider policy differs".into());
    }
    let eq_history =
        initial_kagemusha_eq_accumulator_v1(eq_parameters).map_err(|e| e.to_string())?;
    let ep_history =
        initial_kagemusha_ep_accumulator_v1(ep_parameters).map_err(|e| e.to_string())?;
    let (eq_builder, eq_jobs, eq_provider) = build_half::<Fp>(&witness, eq_history.as_bytes())?;
    let (ep_builder, ep_jobs, ep_provider) = build_half::<Fq>(&witness, ep_history.as_bytes())?;
    Ok((
        KagemushaOrdinaryAppGuardEqCircuitV1 {
            builder: eq_builder,
            jobs: eq_jobs,
            provider_policy_root,
            provider_cells: eq_provider,
        },
        KagemushaOrdinaryAppGuardEpCircuitV1 {
            builder: ep_builder,
            jobs: ep_jobs,
            provider_policy_root,
            provider_cells: ep_provider,
        },
    ))
}

fn equal_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    left: &[PastaSha256ByteV1<F>],
    right: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if left.len() != right.len() {
        return Err("ordinary Guard field width differs".into());
    }
    for (a, b) in left.iter().zip(right) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
    Ok(())
}
fn build_half<F: KagemushaPoseidonFieldV1>(
    w: &OrdinaryGuardWitnessV1<'_>,
    history: &[u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<
    (
        BaseCircuitBuilder<F>,
        PastaSha256JobsV1<F>,
        [AssignedValue<F>; 2],
    ),
    String,
> {
    w.relation.validate()?;
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(KAGEMUSHA_HALO2_K_V1 as usize)
        .use_lookup_bits((KAGEMUSHA_HALO2_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let guard = constrain_guard_bundle_semantics_v1(&mut builder, &mut jobs, w.relation)?;
    let layout = w.credential.original_preimage_layout()?;
    let mut preimage = layout.bytes[..layout.original.start]
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .ok_or("ordinary original prefix differs")?;
    preimage.extend(w.credential.canonical_bytes()?);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut positions = |indices: &[usize]| {
        let bytes = indices.iter().map(|i| preimage[*i]).collect::<Vec<_>>();
        assign_bytes(ctx, &range, &bytes)
    };
    let credential = OrdinaryCredentialOriginalCellsV1 {
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
            .map_err(|_| "ordinary Ed signature width")?,
        play_integrity_fields: layout
            .play_integrity_bytes
            .as_ref()
            .map(|p| core::array::from_fn(|i| positions(&p[i]))),
    };
    let raw_digest = w.credential.canonical_digest()?;
    let expected = assign_bytes(ctx, &range, &raw_digest)
        .try_into()
        .map_err(|_| "ordinary digest width")?;
    let credential_digest = constrain_ordinary_credential_original_v1(
        &mut builder,
        &mut jobs,
        &layout,
        &credential,
        &expected,
    )?;
    let static_binding = {
        let mut data = constant_bytes(b"iroha:kagemusha:v1:ordinary-app-static-binding\0");
        data.extend(constant_bytes(&416_u64.to_le_bytes()));
        for bytes in &credential.fixed_digests[3..16] {
            data.extend_from_slice(bytes);
        }
        hash(builder.main(0), &mut jobs, data)?
    };
    bind_credential(
        &mut builder,
        &mut jobs,
        &guard,
        &credential,
        &credential_digest,
        &static_binding,
    )?;
    let s_bytes = w
        .approval
        .challenge
        .subject
        .canonical_signing_bytes()
        .map_err(|e| e.to_string())?;
    let ctx = builder.main(0);
    let signed_s: [AssignedValue<F>; S::TOTAL_BYTES] =
        core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(s_bytes[i]))));
    bind_subject(
        ctx,
        &range,
        &guard,
        &credential_digest,
        &static_binding,
        &signed_s,
    )?;
    let wrapper_bytes = w.approval.challenge.canonical_signing_bytes()?;
    let ctx = builder.main(0);
    let wrapper: [AssignedValue<F>; A::TOTAL_BYTES] =
        core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(wrapper_bytes[i]))));
    let op = assign_bytes(ctx, &range, &w.approval.challenge.operation_id)
        .try_into()
        .map_err(|_| "operation ID")?;
    let nonce = assign_bytes(ctx, &range, &w.approval.challenge.nonce)
        .try_into()
        .map_err(|_| "native nonce")?;
    let original = OrdinaryApprovalOriginalCellsV1 {
        operation_id: op,
        nonce,
        account_binding: credential.fixed_digests[3],
        authority_policy_digest: credential.fixed_digests[10],
        attested_key_id: credential.fixed_digests[13],
        enrollment_digest: credential_digest,
        normalized_guard_digest: guard.guard_digest,
        issued_at_ms: ctx.load_witness(F::from(w.approval.challenge.issued_at_ms)),
        expires_at_ms: ctx.load_witness(F::from(w.approval.challenge.expires_at_ms)),
    };
    let bound_wrapper = constrain_ordinary_approval_wrapper_v1(
        &mut builder,
        &mut jobs,
        &wrapper,
        &signed_s,
        &original,
    )?;
    let (der, auth, tag) = match &w.approval.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            (signature_der.as_slice(), None, 1_u8)
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            let (auth, der) = kagemusha_app_attest_original_parts_v1(raw_assertion)?;
            (der, Some(auth), 2_u8)
        }
    };
    let signature = Signature::from_der(der).map_err(|_| "ordinary original DER shape")?;
    let r = field_be::<P256Scalar>(&signature.r().to_bytes())?;
    let s = field_be::<P256Scalar>(&signature.s().to_bytes())?;
    let sec1 = w.credential.subject.app_public_key.as_sec1_bytes();
    let qx = field_be::<P256Base>(&sec1[1..33])?;
    let qy = field_be::<P256Base>(&sec1[33..65])?;
    let mut prehash: DigestV1 = Sha256::digest(&wrapper_bytes).into();
    if let Some(auth) = auth {
        let mut bytes = auth.to_vec();
        bytes.extend_from_slice(&prehash);
        prehash = Sha256::digest(bytes).into();
    }
    let z = prehash.iter().fold(P256Scalar::ZERO, |value, b| {
        value * P256Scalar::from(256) + P256Scalar::from(u64::from(*b))
    });
    let mut little = prehash;
    little.reverse();
    let quotient = u64::from(Option::<P256Scalar>::from(P256Scalar::from_repr(little)).is_none());
    let base_chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let scalar_chip = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    let key = EcPoint::new(
        base_chip.load_private(ctx, qx),
        base_chip.load_private(ctx, qy),
    );
    let r = scalar_chip.load_private(ctx, r);
    let s = scalar_chip.load_private(ctx, s);
    let z = scalar_chip.load_private(ctx, z);
    let key_cells: [AssignedValue<F>; 65] = credential
        .app_public_key
        .map(|b| b.assigned().expect("assigned original SEC1"));
    let signature = OrdinaryPlatformSignatureCellsV1 {
        signature_public_key: &key,
        enrolled_public_key: &key,
        enrolled_public_key_sec1: &key_cells,
        r: &r,
        s: &s,
        z: &z,
        digest_reduction_quotient: ctx.load_witness(F::from(quotient)),
    };
    let evidence = match &w.approval.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            constrain_original_android_approval_stream_v1(
                &mut builder,
                &mut jobs,
                signature_der,
                &bound_wrapper,
                &signature,
            )?
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            let auth = auth.ok_or("ordinary Apple authData absent")?;
            let auth: [u8; 37] = auth
                .try_into()
                .map_err(|_| "ordinary App Attest authData must be exact37")?;
            let ctx = builder.main(0);
            let auth_cells =
                core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(auth[i]))));
            let rp = credential.fixed_digests[11]
                .map(|b| b.assigned().expect("assigned governed RP digest"));
            let floor = ctx.load_witness(F::from(u64::from(
                w.previous_app_attest_counter
                    .ok_or("ordinary Apple counter floor absent")?,
            )));
            let counter = ctx.load_witness(F::from(u64::from(u32::from_be_bytes(
                auth[33..37].try_into().expect("auth counter"),
            ))));
            constrain_original_apple_approval_stream_v1(
                &mut builder,
                &mut jobs,
                raw_assertion,
                &wrapper,
                &auth_cells,
                &rp,
                floor,
                counter,
                &signature,
            )?
        }
    };
    let approval_digest = constrain_ordinary_approval_proof_binding_v1(
        &mut builder,
        &mut jobs,
        &bound_wrapper,
        tag,
        &evidence,
    )?;
    let subject_bytes = signed_s
        .map(|b| PastaSha256ByteV1::range_checked(builder.main(0), &range, b))
        .to_vec();
    let subject_digest = hash(builder.main(0), &mut jobs, subject_bytes)?;
    let ctx = builder.main(0);
    let provider = assigned_digest_bytes_v1(ctx, range.gate(), guard.successor_policy);
    let provider_cells = guard.successor_policy;
    let mut public = Vec::new();
    for digest in [
        guard.guard_digest.to_vec(),
        credential_digest.to_vec(),
        approval_digest.to_vec(),
        subject_digest.to_vec(),
        provider,
    ] {
        public.extend(super::super::guard_bundle::digest_limbs_assigned(
            ctx,
            &digest.try_into().map_err(|_| "ordinary digest width")?,
        ));
    }
    public.extend(history.chunks_exact(16).map(|p| {
        ctx.load_constant(from_u128::<F>(u128::from_le_bytes(
            p.try_into().expect("history limb"),
        )))
    }));
    builder.assigned_instances = vec![public];
    super::super::base_packing::finalize_base_params_v1(&mut builder, 9)?;
    jobs.validate_capacity((1_usize << KAGEMUSHA_HALO2_K_V1) - 9)?;
    Ok((builder, jobs, provider_cells))
}

fn field_be<F: ff::PrimeField>(bytes: &[u8]) -> Result<F, String> {
    let mut repr = F::Repr::default();
    if repr.as_ref().len() != bytes.len() {
        return Err("P256 field width".into());
    }
    for (a, b) in repr.as_mut().iter_mut().zip(bytes.iter().rev()) {
        *a = *b;
    }
    Option::<F>::from(F::from_repr(repr)).ok_or("noncanonical P256 field".into())
}

fn bind_credential<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    g: &KagemushaAssignedGuardBundleV1<F>,
    c: &OrdinaryCredentialOriginalCellsV1<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
    static_binding: &[PastaSha256ByteV1<F>; 32],
) -> Result<(), String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for (raw, limbs) in [
        (c.fixed_digests[4], g.network_id),
        (c.fixed_digests[5], g.lane_id),
        (c.fixed_digests[6], g.release_id),
        (c.fixed_digests[7], g.hardware_profile_id),
        (c.fixed_digests[8], g.successor_suite_id),
        (c.fixed_digests[14], g.successor_key),
    ] {
        let bytes = assigned_digest_bytes_v1(ctx, range.gate(), limbs);
        equal_bytes(ctx, &range, &raw, &bytes)?;
    }
    let policy = assigned_uint_bytes_v1(ctx, range.gate(), g.policy_epoch, 64);
    equal_bytes(ctx, &range, &c.scalars[0], &policy)?;
    let epoch = assigned_uint_bytes_v1(ctx, range.gate(), g.successor_generation, 64);
    equal_bytes(ctx, &range, &c.scalars[1], &epoch)?;
    for slot in 0..2 {
        equal_bytes(ctx, &range, digest, &g.credential_issuance_digests[slot])?;
        equal_bytes(
            ctx,
            &range,
            static_binding,
            &g.credential_app_policy_binding_digests[slot],
        )?;
        equal_bytes(
            ctx,
            &range,
            &c.app_public_key,
            &g.credential_device_public_keys[slot],
        )?;
        equal_bytes(
            ctx,
            &range,
            &c.fixed_digests[15],
            &g.credential_financial_authority_commitments[slot],
        )?;
    }
    let key_sha = hash(ctx, jobs, c.app_public_key.to_vec())?;
    equal_bytes(ctx, &range, &key_sha, &c.fixed_digests[13])?;
    let mut epoch_preimage = constant_bytes(b"iroha:kagemusha:v1:ordinary-financial-epoch\0");
    epoch_preimage.extend(constant_bytes(&200_u64.to_le_bytes()));
    for i in [0, 4, 5, 6, 7, 15] {
        epoch_preimage.extend_from_slice(&c.fixed_digests[i]);
    }
    epoch_preimage.extend_from_slice(&c.scalars[1]);
    let epoch_sha = hash(ctx, jobs, epoch_preimage)?;
    let expected = assigned_digest_bytes_v1(ctx, range.gate(), g.successor_epoch);
    equal_bytes(ctx, &range, &epoch_sha, &expected)
}
fn bind_subject<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    g: &KagemushaAssignedGuardBundleV1<F>,
    credential: &[PastaSha256ByteV1<F>; 32],
    static_binding: &[PastaSha256ByteV1<F>; 32],
    s: &[AssignedValue<F>; S::TOTAL_BYTES],
) -> Result<(), String> {
    bind_slot(ctx, range, s, S::CREDENTIAL_ID, credential.to_vec())?;
    bind_slot(ctx, range, s, S::APP_POLICY_DIGEST, static_binding.to_vec())?;
    for (slot, limbs) in [
        (S::RELEASE_ID, g.release_id),
        (S::PROVIDER_POLICY_ROOT, g.successor_policy),
        (S::NETWORK_ID, g.network_id),
        (S::LANE_COMMITMENT, g.lane_id),
        (S::HARDWARE_PROFILE_ID, g.hardware_profile_id),
        (S::HARDWARE_EPOCH_ID, g.successor_epoch),
    ] {
        let bytes = assigned_digest_bytes_v1(ctx, range.gate(), limbs);
        bind_slot(ctx, range, s, slot, bytes)?;
    }
    for (slot, value, bits) in [
        (S::POLICY_EPOCH, g.policy_epoch, 64),
        (S::HARDWARE_EPOCH_GENERATION, g.successor_generation, 64),
        (S::OPERATION_TAG, g.operation, 8),
        (S::SECURE_INDEX_BEFORE, g.predecessor_sequence, 128),
        (S::SECURE_INDEX_AFTER, g.successor_sequence, 128),
    ] {
        let bytes = assigned_uint_bytes_v1(ctx, range.gate(), value, bits);
        bind_slot(ctx, range, s, slot, bytes)?;
    }
    // The native selected holder independently rederives the complete transition/candidate/body
    // digest. It compares SHA(fullS); the wrapper also signs this complete normalized Guard,
    // whose amount, operation and predecessor/successor fields are constrained above.
    Ok(())
}

fn bind_slot<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    subject: &[AssignedValue<F>; S::TOTAL_BYTES],
    slot: core::ops::Range<usize>,
    expected: Vec<PastaSha256ByteV1<F>>,
) -> Result<(), String> {
    let actual = subject[slot]
        .iter()
        .map(|v| PastaSha256ByteV1::range_checked(ctx, range, *v))
        .collect::<Vec<_>>();
    equal_bytes(ctx, range, &actual, &expected)
}
