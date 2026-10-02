//! Same-original State-side binding for the private ordinary Guard public columns.
//!
//! This data relation creates no issuer or platform authorization. The enclosing State must
//! consume the exact five columns of a genuine, release-pinned OrdinaryAppGuard proof. Keeping
//! these cells bound to its own assigned Guard prevents a host-verified digest from becoming
//! an unrelated free witness. The phone signature is proved only by that mandatory helper.

use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaAppOperationApprovalV1, KagemushaHardwareSelectionSigningLayoutV1 as S,
    KagemushaOrdinaryAppCredentialV1, KagemushaPlayIntegrityRefreshLeaseV1,
};

use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    composite::assigned_digest_bytes_v1,
    guard_bundle::{KagemushaAssignedGuardBundleV1, assign_bytes, constant_bytes, hash},
    ordinary_app_guard_binding::{
        OrdinaryApprovalOriginalCellsV1, OrdinaryCredentialIssuerCellsV1,
        constrain_ordinary_approval_wrapper_v1,
    },
    ordinary_approval_proof_binding::{
        constrain_ordinary_authorization_proof_binding_v1,
        constrain_ordinary_selected_approval_proof_binding_v1,
    },
    ordinary_credential_union::{
        assign_ordinary_credential_union_v1, reconstruct_ordinary_credential_union_v1,
    },
    ordinary_guard_circuit::{bind_credential, bind_subject},
    ordinary_integrity_union::constrain_ordinary_integrity_union_v1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

/// Mandatory copied identities of the same original credential, platform transcript and subject.
pub(crate) struct KagemushaOrdinaryGuardDataBindingV1<F: KagemushaPoseidonFieldV1> {
    /// Normalized statement, credential, authorization transcript, complete S, provider root.
    pub(crate) digests: [[PastaSha256ByteV1<F>; 32]; 5],
    /// Actual account binding copied from the same issuer-authenticated original C.
    pub(crate) account_binding: [PastaSha256ByteV1<F>; 32],
    /// Exact full S signing bytes; State additionally joins its real proof statement/candidate.
    pub(crate) canonical_subject: [AssignedValue<F>; S::TOTAL_BYTES],
    /// Same signed purpose cell inside the whole platform-approved wrapper transcript.
    pub(crate) approval_purpose: AssignedValue<F>,
    /// Same operation-ID bytes copy-bound inside the complete signed W wrapper.
    pub(crate) approval_operation_id: [PastaSha256ByteV1<F>; 32],
    /// Same original nonce bytes copy-bound inside the whole platform-approved wrapper.
    pub(crate) approval_nonce: [PastaSha256ByteV1<F>; 32],
    /// Same original W issuance cell, never a detached terminal time witness.
    pub(crate) approval_issued_at_ms: AssignedValue<F>,
    /// Same immutable original W expiry cell.
    pub(crate) approval_expires_at_ms: AssignedValue<F>,
}

/// Reconstruct exact model originals and join every scope/key/financial field to assigned State.
/// The canonical original byte layouts are model-owned, including the mandatory issuer carrier.
/// No field supplied here is a capability; consuming the release-pinned helper is mandatory.
pub(super) fn constrain_ordinary_guard_data_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    guard: &KagemushaAssignedGuardBundleV1<F>,
    credential: &KagemushaOrdinaryAppCredentialV1,
    approval: &KagemushaAppOperationApprovalV1,
    integrity_lease: Option<&KagemushaPlayIntegrityRefreshLeaseV1>,
) -> Result<KagemushaOrdinaryGuardDataBindingV1<F>, String> {
    let mut union = assign_ordinary_credential_union_v1(builder, credential)?;
    let range = builder.range_chip();
    let ed_digest =
        reconstruct_ordinary_credential_union_v1(builder, jobs, credential, &union, false)?;
    let signature = assign_bytes(
        builder.main(0),
        &range,
        credential.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "ordinary issuer width")?;
    union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed_digest,
        signature,
    });
    let credential_digest =
        reconstruct_ordinary_credential_union_v1(builder, jobs, credential, &union, true)?;
    let apple = union.apple;
    let integrity = union.integrity;
    let cells = union.cells;
    let mut static_bytes = constant_bytes(b"iroha:kagemusha:v1:ordinary-app-static-binding\0");
    static_bytes.extend(constant_bytes(&416_u64.to_le_bytes()));
    for field in &cells.fixed_digests[3..16] {
        static_bytes.extend_from_slice(field);
    }
    let static_binding = hash(builder.main(0), jobs, static_bytes)?;
    bind_credential(
        builder,
        jobs,
        guard,
        &cells,
        &credential_digest,
        &static_binding,
    )?;
    let s_raw = approval
        .challenge
        .canonical_subject_signing_bytes()
        .map_err(|e| e.to_string())?;
    let ctx = builder.main(0);
    let signed_s = core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(s_raw[i]))));
    bind_subject(
        ctx,
        &range,
        guard,
        &credential_digest,
        &static_binding,
        &signed_s,
    )?;
    let wrapper_raw = approval.challenge.canonical_signing_bytes()?;
    let wrapper: [AssignedValue<F>; A::TOTAL_BYTES] =
        core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(wrapper_raw[i]))));
    let original = OrdinaryApprovalOriginalCellsV1 {
        operation_id: assign_bytes(ctx, &range, &approval.challenge.operation_id)
            .try_into()
            .map_err(|_| "ordinary operation width")?,
        nonce: assign_bytes(ctx, &range, &approval.challenge.nonce)
            .try_into()
            .map_err(|_| "ordinary nonce width")?,
        account_binding: cells.fixed_digests[3],
        authority_policy_digest: cells.fixed_digests[10],
        attested_key_id: cells.fixed_digests[13],
        enrollment_digest: credential_digest,
        normalized_guard_digest: guard.guard_digest,
        issued_at_ms: ctx.load_witness(F::from(approval.challenge.issued_at_ms)),
        expires_at_ms: ctx.load_witness(F::from(approval.challenge.expires_at_ms)),
    };
    let wrapper =
        constrain_ordinary_approval_wrapper_v1(builder, jobs, &wrapper, &signed_s, &original)?;
    let approval_purpose = wrapper[A::PURPOSE.start]
        .assigned()
        .expect("assigned ordinary approval purpose");
    let lease_digest = constrain_ordinary_integrity_union_v1(
        builder,
        jobs,
        &cells,
        &credential_digest,
        integrity,
        integrity_lease,
        None,
        original.issued_at_ms,
        original.expires_at_ms,
    )?;
    let raw = match &approval.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            signature_der.as_slice()
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            raw_assertion.as_slice()
        }
    };
    if raw.is_empty() || raw.len() > 311 {
        return Err("ordinary original platform evidence exceeds fixed capacity".into());
    }
    let ctx = builder.main(0);
    let bytes = (0..311)
        .map(|i| {
            let byte = ctx.load_witness(F::from(u64::from(raw.get(i).copied().unwrap_or(0))));
            PastaSha256ByteV1::range_checked(ctx, &range, byte)
        })
        .collect();
    let length = ctx.load_witness(F::from(raw.len() as u64));
    let evidence = KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, length)?;
    let tag = range.gate().add(
        builder.main(0),
        apple,
        halo2_base::QuantumCell::Constant(F::ONE),
    );
    let approval_digest = constrain_ordinary_selected_approval_proof_binding_v1(
        builder, jobs, &wrapper, tag, &evidence,
    )?;
    let authorization_digest = constrain_ordinary_authorization_proof_binding_v1(
        builder,
        jobs,
        &approval_digest,
        Some(&lease_digest),
    )?;
    let subject_bytes = signed_s
        .map(|byte| PastaSha256ByteV1::range_checked(builder.main(0), &range, byte))
        .to_vec();
    let subject_digest = hash(builder.main(0), jobs, subject_bytes)?;
    let provider = assigned_digest_bytes_v1(builder.main(0), range.gate(), guard.successor_policy)
        .try_into()
        .map_err(|_| "ordinary provider root width")?;
    Ok(KagemushaOrdinaryGuardDataBindingV1 {
        digests: [
            guard.guard_digest,
            credential_digest,
            authorization_digest,
            subject_digest,
            provider,
        ],
        account_binding: cells.fixed_digests[3],
        canonical_subject: signed_s,
        approval_purpose,
        approval_operation_id: original.operation_id,
        approval_nonce: original.nonce,
        approval_issued_at_ms: original.issued_at_ms,
        approval_expires_at_ms: original.expires_at_ms,
    })
}
