//! Complete ordinary Guard equations, built identically for both Pasta fields.
//!
//! The normalized predecessor/successor/amount statement and financial-secret opening use the
//! maintained Guard semantic relation. Its data projection is copy-bound to every field of the
//! genuinely admitted Ed credential original; no provider-secret or P256 private scalar is used.
//! Full original DER/App Attest CBOR authenticates the wrapper's normalized statement and S.

use super::super::ordinary_issuer_config::OrdinaryIssuerTableV1;
use super::super::{
    DigestV1,
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{
        KagemushaAssignedGuardBundleV1, KagemushaGuardBundleRelationWitnessV1, assign_bytes,
        constant_bytes, constrain_guard_bundle_semantics_v1, hash,
    },
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    ordinary_app_guard_binding::{
        OrdinaryApprovalOriginalCellsV1, OrdinaryCredentialIssuerCellsV1,
        OrdinaryCredentialOriginalCellsV1, constrain_ordinary_approval_wrapper_v1,
    },
    ordinary_approval_proof_binding::{
        constrain_ordinary_authorization_proof_binding_v1,
        constrain_ordinary_selected_approval_proof_binding_v1,
    },
    ordinary_credential_union::{
        assign_ordinary_credential_union_v1, reconstruct_ordinary_credential_union_v1,
    },
    ordinary_integrity_union::constrain_ordinary_integrity_union_v1,
    ordinary_issuer_equation::constrain_ordinary_issuer_original_v1,
};
use super::{KagemushaOrdinaryAppGuardEpCircuitV1, KagemushaOrdinaryAppGuardEqCircuitV1};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaAppOperationApprovalV1, KagemushaHardwareSelectionSigningLayoutV1 as S,
    KagemushaOrdinaryAppCredentialV1, KagemushaPlayIntegrityRefreshLeaseV1,
};

/// Original bounded proof material; it is not an enrollment, Native owner, or spending capability.
/// The production caller must lend it from the actual selected witness/approval holder.
pub(crate) struct OrdinaryGuardWitnessV1<'a> {
    pub(crate) relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(crate) credential: &'a KagemushaOrdinaryAppCredentialV1,
    pub(crate) approval: &'a KagemushaAppOperationApprovalV1,
    pub(crate) previous_app_attest_counter: Option<u32>,
    pub(crate) integrity_lease: Option<&'a KagemushaPlayIntegrityRefreshLeaseV1>,
}

pub(crate) fn build_ordinary_app_guard_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: OrdinaryGuardWitnessV1<'_>,
    provider_policy_root: DigestV1,
    issuer_table: &OrdinaryIssuerTableV1,
) -> Result<
    (
        KagemushaOrdinaryAppGuardEqCircuitV1,
        KagemushaOrdinaryAppGuardEpCircuitV1,
    ),
    String,
> {
    let eq = build_ordinary_app_guard_eq_v1(
        eq_parameters,
        &witness,
        provider_policy_root,
        issuer_table,
    )?;
    let ep = build_ordinary_app_guard_ep_v1(
        ep_parameters,
        &witness,
        provider_policy_root,
        issuer_table,
    )?;
    Ok((eq, ep))
}

// The release generator consumes one complete graph before allocating the next parity.
// Both factories use the same complete build_half relation as the paired prover.
pub(crate) fn build_ordinary_app_guard_eq_v1(
    parameters: &ParamsIPA<EqAffine>,
    witness: &OrdinaryGuardWitnessV1<'_>,
    provider_policy_root: DigestV1,
    issuer_table: &OrdinaryIssuerTableV1,
) -> Result<KagemushaOrdinaryAppGuardEqCircuitV1, String> {
    require_provider(witness, provider_policy_root)?;
    let history = initial_kagemusha_eq_accumulator_v1(parameters).map_err(|e| e.to_string())?;
    let issuer_index = issuer_table.selected(witness.credential.subject.hardware_profile_id)?;
    let (builder, jobs, provider_cells, issuer_cells, profile_cells) =
        build_half::<Fp>(witness, history.as_bytes(), issuer_table)?;
    Ok(KagemushaOrdinaryAppGuardEqCircuitV1 {
        builder,
        jobs,
        provider_policy_root,
        provider_cells,
        issuer_table: issuer_table.clone(),
        issuer_index,
        issuer_cells,
        profile_cells,
    })
}

pub(crate) fn build_ordinary_app_guard_ep_v1(
    parameters: &ParamsIPA<EpAffine>,
    witness: &OrdinaryGuardWitnessV1<'_>,
    provider_policy_root: DigestV1,
    issuer_table: &OrdinaryIssuerTableV1,
) -> Result<KagemushaOrdinaryAppGuardEpCircuitV1, String> {
    require_provider(witness, provider_policy_root)?;
    let history = initial_kagemusha_ep_accumulator_v1(parameters).map_err(|e| e.to_string())?;
    let issuer_index = issuer_table.selected(witness.credential.subject.hardware_profile_id)?;
    let (builder, jobs, provider_cells, issuer_cells, profile_cells) =
        build_half::<Fq>(witness, history.as_bytes(), issuer_table)?;
    Ok(KagemushaOrdinaryAppGuardEpCircuitV1 {
        builder,
        jobs,
        provider_policy_root,
        provider_cells,
        issuer_table: issuer_table.clone(),
        issuer_index,
        issuer_cells,
        profile_cells,
    })
}

fn require_provider(
    witness: &OrdinaryGuardWitnessV1<'_>,
    provider_policy_root: DigestV1,
) -> Result<(), String> {
    if provider_policy_root == [0; 32]
        || witness.relation.statement.successor_hardware_policy_id != provider_policy_root
    {
        return Err("ordinary Guard provider policy differs".into());
    }
    Ok(())
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
    issuer_table: &OrdinaryIssuerTableV1,
) -> Result<
    (
        BaseCircuitBuilder<F>,
        PastaSha256JobsV1<F>,
        [AssignedValue<F>; 2],
        [AssignedValue<F>; 65],
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
    let issuer_index = issuer_table.selected(w.credential.subject.hardware_profile_id)?;
    let issuer_raw = issuer_table.slots[issuer_index].issuer_sec1;
    let issuer_cells = core::array::from_fn(|i| {
        builder
            .main(0)
            .load_witness(F::from(u64::from(issuer_raw[i])))
    });
    let guard = constrain_guard_bundle_semantics_v1(&mut builder, &mut jobs, w.relation)?;
    let mut union = assign_ordinary_credential_union_v1(&mut builder, w.credential)?;
    let range = builder.range_chip();
    let ed_digest = reconstruct_ordinary_credential_union_v1(
        &mut builder,
        &mut jobs,
        w.credential,
        &union,
        false,
    )?;
    let issuer_signature = assign_bytes(
        builder.main(0),
        &range,
        w.credential.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "ordinary issuer signature width")?;
    constrain_ordinary_issuer_original_v1(
        &mut builder,
        &mut jobs,
        1,
        &union.cells.fixed_digests[6],
        &union.cells.fixed_digests[7],
        &ed_digest,
        &issuer_signature,
        &w.credential.circuit_admission,
        &issuer_raw,
        &issuer_cells,
    )?;
    union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed_digest,
        signature: issuer_signature,
    });
    let credential_digest = reconstruct_ordinary_credential_union_v1(
        &mut builder,
        &mut jobs,
        w.credential,
        &union,
        true,
    )?;
    let apple = union.apple;
    let integrity = union.integrity;
    let credential = union.cells;
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
        .canonical_subject_signing_bytes()
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
    let lease_digest = constrain_ordinary_integrity_union_v1(
        &mut builder,
        &mut jobs,
        &credential,
        &credential_digest,
        integrity,
        w.integrity_lease,
        Some((&issuer_raw, &issuer_cells)),
        original.issued_at_ms,
        original.expires_at_ms,
    )?;
    let evidence = super::super::ordinary_platform_union::constrain_ordinary_platform_union_v1(
        &mut builder,
        &mut jobs,
        &credential,
        w.credential.subject.app_public_key.as_sec1_bytes(),
        w.credential.subject.app_release_digest,
        w.approval,
        &bound_wrapper,
        apple,
        w.previous_app_attest_counter,
    )?;
    let tag = range.gate().add(
        builder.main(0),
        apple,
        halo2_base::QuantumCell::Constant(F::ONE),
    );
    let approval_digest = constrain_ordinary_selected_approval_proof_binding_v1(
        &mut builder,
        &mut jobs,
        &bound_wrapper,
        tag,
        &evidence,
    )?;
    let authorization_digest = constrain_ordinary_authorization_proof_binding_v1(
        &mut builder,
        &mut jobs,
        &approval_digest,
        Some(&lease_digest),
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
        authorization_digest.to_vec(),
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
    Ok((
        builder,
        jobs,
        provider_cells,
        issuer_cells,
        guard.hardware_profile_id,
    ))
}

pub(crate) fn bind_credential<F: KagemushaPoseidonFieldV1>(
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
pub(crate) fn bind_subject<F: KagemushaPoseidonFieldV1>(
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
    ] {
        let bytes = assigned_uint_bytes_v1(ctx, range.gate(), value, bits);
        bind_slot(ctx, range, s, slot, bytes)?;
    }
    // Financial secure indexes are independent of the normalized Guard logical sequence.
    // Rotate resets the latter. The complete State/terminal consumer must join S indexes
    // to its actual assigned State; an original Guard proof alone supplies no financial grant.
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

#[cfg(test)]
#[path = "ordinary_guard_composition_tests.rs"]
mod original_guard_tests;
