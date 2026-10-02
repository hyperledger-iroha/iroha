//! Actual paired ordinary Guard verification under genuine Native selections.
//!
//! Signature admission remains in the descriptor-held logical journal. This boundary derives
//! every public column from that journal and the actual financial preview, verifies both real IPA
//! proofs under the separate signed ordinary roles, and terminally decides their whole histories.

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaEpAccumulatorV1,
    KagemushaEqAccumulatorV1, decide_kagemusha_ep_accumulator_v1,
    decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    ordinary_guard_circuit::ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    kagemusha_v1_state::{
        KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
        KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1,
        KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1,
        KagemushaAuthenticatedOrdinaryHistoricalApprovalV1, KagemushaStateErrorV1,
    },
};
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::KagemushaReleasePurposeV1;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use snark_verifier::verifier::plonk::PlonkProtocol;

#[path = "ordinary_cash_guard_verifier.rs"]
mod cash_guard;
pub(super) use cash_guard::preparation_digests;
pub(crate) use cash_guard::{
    KagemushaAuthenticatedOrdinaryPreparationGuardV1, verify_ordinary_preparation_guard_v1,
};

#[path = "ordinary_cash_terminal_guard_verifier.rs"]
mod terminal_guard;
pub(super) use terminal_guard::terminal_digests;
pub(crate) use terminal_guard::{
    KagemushaAuthenticatedOrdinaryTerminalGuardV1, verify_ordinary_terminal_guard_v1,
};

type History = [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
type Result<T> = core::result::Result<T, KagemushaStateErrorV1>;

pub(super) struct OrdinaryGuardMaterialV1<'a> {
    pub(super) eq_parameters: &'a ParamsIPA<EqAffine>,
    pub(super) ep_parameters: &'a ParamsIPA<EpAffine>,
    pub(super) eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) release_id: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
}

#[derive(Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryGuardProofV1",
    frame = "iroha.kagemusha.core.v1.ordinary-app-guard-proof"
)]
pub(super) struct OrdinaryGuardProofWireV1 {
    pub(super) version: u16,
    pub(super) release_id: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
    pub(super) normalized_guard_digest: DigestV1,
    pub(super) credential_digest: DigestV1,
    pub(super) authorization_transcript_digest: DigestV1,
    pub(super) subject_signing_digest: DigestV1,
    pub(super) provider_policy_root: DigestV1,
    pub(super) eq_proof: Vec<u8>,
    pub(super) ep_proof: Vec<u8>,
    pub(super) eq_history: History,
    pub(super) ep_history: History,
}

/// Successful ordinary paired proof admission. Only the verifier constructs it.
/// It grants no current wallet, checkpoint, lease, or monetary mutation by itself.
pub(crate) struct KagemushaAuthenticatedOrdinaryBootstrapGuardV1 {
    normalized: DigestV1,
    credential: DigestV1,
    approval: DigestV1,
    subject: DigestV1,
    provider: DigestV1,
    eq_history: History,
    ep_history: History,
    original: Vec<u8>,
}
impl KagemushaAuthenticatedOrdinaryBootstrapGuardV1 {
    pub(crate) fn normalized_guard_digest(&self) -> DigestV1 {
        self.normalized
    }
    pub(crate) fn credential_digest(&self) -> DigestV1 {
        self.credential
    }
    pub(crate) fn authorization_transcript_digest(&self) -> DigestV1 {
        self.approval
    }
    pub(crate) fn subject_signing_digest(&self) -> DigestV1 {
        self.subject
    }
    pub(crate) fn provider_policy_root(&self) -> DigestV1 {
        self.provider
    }
    pub(crate) fn eq_history(&self) -> &History {
        &self.eq_history
    }
    pub(crate) fn ep_history(&self) -> &History {
        &self.ep_history
    }
    pub(crate) fn original(&self) -> &[u8] {
        &self.original
    }
}

pub(crate) fn verify_ordinary_bootstrap_guard_v1(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
    paired_guard: &[u8],
    trusted_native_now_ms: u64,
) -> Result<KagemushaAuthenticatedOrdinaryBootstrapGuardV1> {
    selection.recheck_at_trusted_time(trusted_native_now_ms)?;
    approval.recheck_captured_bootstrap_at_native_time(trusted_native_now_ms)?;
    if !core::ptr::eq(
        selection.enrollment(),
        approval.retained_enrollment().as_ref(),
    ) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let admitted = verify_selected_original(
        selection,
        approval.challenge(),
        approval.authorization_binding_digest()?,
        approval.retained_release(),
        paired_guard,
    )?;
    approval.recheck_captured_bootstrap_at_native_time(trusted_native_now_ms)?;
    selection.recheck_at_trusted_time(trusted_native_now_ms)?;
    Ok(admitted)
}

/// A historical proof result can only recheck an already selected publication. It cannot be
/// converted into the live approval result or used to reserve a new financial operation.
pub(crate) struct KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1 {
    verified: KagemushaAuthenticatedOrdinaryBootstrapGuardV1,
    approval_admission_time_ms: u64,
}
impl KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1 {
    pub(crate) fn normalized_guard_digest(&self) -> DigestV1 {
        self.verified.normalized_guard_digest()
    }
    pub(crate) fn credential_digest(&self) -> DigestV1 {
        self.verified.credential_digest()
    }
    pub(crate) fn authorization_transcript_digest(&self) -> DigestV1 {
        self.verified.authorization_transcript_digest()
    }
    pub(crate) fn subject_signing_digest(&self) -> DigestV1 {
        self.verified.subject_signing_digest()
    }
    pub(crate) fn provider_policy_root(&self) -> DigestV1 {
        self.verified.provider_policy_root()
    }
    pub(crate) fn eq_history(&self) -> &History {
        self.verified.eq_history()
    }
    pub(crate) fn ep_history(&self) -> &History {
        self.verified.ep_history()
    }
    pub(crate) fn original(&self) -> &[u8] {
        self.verified.original()
    }
    pub(crate) fn approval_admission_time_ms(&self) -> u64 {
        self.approval_admission_time_ms
    }
}

/// Reverify the same original platform equation and both ordinary proof histories at the exact
/// selected WAL publication time. Current enrollment and integrity freshness are independently
/// rechecked by the historical journal holder at native now; no original expiry is renewed.
pub(crate) fn verify_ordinary_bootstrap_guard_historical_v1(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    approval: &KagemushaAuthenticatedOrdinaryHistoricalApprovalV1<'_>,
    paired_guard: &[u8],
    approval_admission_time_ms: u64,
    trusted_native_now_ms: u64,
) -> Result<KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1> {
    approval.recheck_originals(approval_admission_time_ms, trusted_native_now_ms)?;
    // The original approval's own lease is checked at its retained publication instant by the
    // historical holder. The selection's latest lease must be checked at actual native now;
    // a later refresh must never be backdated to the initial publication time.
    selection.recheck_at_trusted_time(trusted_native_now_ms)?;
    if !core::ptr::eq(
        selection.enrollment(),
        approval.retained_enrollment().as_ref(),
    ) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let verified = verify_selected_original(
        selection,
        approval.challenge(),
        approval.authorization_binding_digest()?,
        approval.retained_release(),
        paired_guard,
    )?;
    selection.recheck_at_trusted_time(trusted_native_now_ms)?;
    approval.recheck_originals(approval_admission_time_ms, trusted_native_now_ms)?;
    Ok(KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1 {
        verified,
        approval_admission_time_ms,
    })
}

fn verify_selected_original(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    challenge: &iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1,
    proof_binding_digest: DigestV1,
    retained_release: &iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1,
    paired_guard: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryBootstrapGuardV1> {
    let release = selection.authenticated_release()?;
    if release.purpose() != KagemushaReleasePurposeV1::Production
        || release.release_id() != retained_release.release_id()
        || release.attestation_digest() != retained_release.attestation_digest()
    {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let preview = selection.preview()?;
    let normalized = preview
        .normalized_guard_statement
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;

    let subject: DigestV1 = Sha256::digest(
        challenge
            .subject
            .canonical_signing_bytes()
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?,
    )
    .into();
    if challenge.normalized_guard_digest != normalized
        || challenge.subject.secure_index_before != 0
        || challenge.subject.secure_index_after != 0
        || challenge.subject.transition_statement_digest
            != preview.statement.proof_statement_digest()?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let material = selection
        .recursive_verifier()
        .ordinary_guard_verifier_material();
    let wire = decode_exact(paired_guard, &material)?;
    let expected = [
        normalized,
        selection.enrollment().app_credential().digest(),
        proof_binding_digest,
        subject,
        release.provider_policy_root(),
    ];
    if [
        wire.normalized_guard_digest,
        wire.credential_digest,
        wire.authorization_transcript_digest,
        wire.subject_signing_digest,
        wire.provider_policy_root,
    ] != expected
        || expected.contains(&[0; 32])
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    verify_wire(&wire, &material)?;
    Ok(KagemushaAuthenticatedOrdinaryBootstrapGuardV1 {
        normalized: expected[0],
        credential: expected[1],
        approval: expected[2],
        subject: expected[3],
        provider: expected[4],
        eq_history: wire.eq_history,
        ep_history: wire.ep_history,
        original: paired_guard.to_vec(),
    })
}

/// Mathematical original verification only; no historical/current Native loan is produced.
pub(super) fn verify_stateless_original_v1(
    original: &[u8],
    material: &OrdinaryGuardMaterialV1<'_>,
    expected: [DigestV1; 5],
) -> Result<()> {
    if expected.contains(&[0; 32]) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let wire = decode_exact(original, material)?;
    if [
        wire.normalized_guard_digest,
        wire.credential_digest,
        wire.authorization_transcript_digest,
        wire.subject_signing_digest,
        wire.provider_policy_root,
    ] != expected
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    verify_wire(&wire, material)
}

fn decode_exact(
    bytes: &[u8],
    material: &OrdinaryGuardMaterialV1<'_>,
) -> Result<OrdinaryGuardProofWireV1> {
    if bytes.is_empty()
        || bytes.len() > KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
        || material.eq_protocol.num_instance != [ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1]
        || material.ep_protocol.num_instance != [ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1]
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let eq = ordinary_ipa_proof_profile_v1(material.eq_protocol)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        .byte_len;
    let ep = ordinary_ipa_proof_profile_v1(material.ep_protocol)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        .byte_len;
    if eq == 0
        || ep == 0
        || eq
            .checked_add(ep)
            .is_none_or(|n| n > KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let wire: OrdinaryGuardProofWireV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    if norito::encode_canonical(&wire).map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        != bytes
        || wire.version != 1
        || wire.eq_proof.len() != eq
        || wire.ep_proof.len() != ep
        || wire.release_id != material.release_id
        || wire.artifact_manifest_digest != material.artifact_manifest_digest
        || wire.eq_protocol_digest != material.eq_protocol_digest
        || wire.ep_protocol_digest != material.ep_protocol_digest
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(wire)
}

pub(super) fn public_column<F: KagemushaPoseidonFieldV1>(
    digests: [DigestV1; 5],
    history: &History,
) -> Vec<F> {
    let mut values = digests
        .into_iter()
        .flat_map(digest_limbs::<F>)
        .collect::<Vec<_>>();
    values.extend(history.chunks_exact(16).map(|part| {
        from_u128::<F>(u128::from_le_bytes(
            part.try_into().expect("fixed history limb"),
        ))
    }));
    values
}
fn verify_wire(w: &OrdinaryGuardProofWireV1, m: &OrdinaryGuardMaterialV1<'_>) -> Result<()> {
    let digests = [
        w.normalized_guard_digest,
        w.credential_digest,
        w.authorization_transcript_digest,
        w.subject_signing_digest,
        w.provider_policy_root,
    ];
    let eq_instances = public_column::<Fp>(digests, &w.eq_history);
    let ep_instances = public_column::<Fq>(digests, &w.ep_history);
    let eq =
        verify_eq_succinct_protocol(m.eq_parameters, m.eq_protocol, &w.eq_proof, &eq_instances)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let ep =
        verify_ep_succinct_protocol(m.ep_parameters, m.ep_protocol, &w.ep_proof, &ep_instances)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let eq = KagemushaEqAccumulatorV1::from_native(&eq)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let ep = KagemushaEpAccumulatorV1::from_native(&ep)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&w.eq_history)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&w.ep_history)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eq)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eq_history))
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &ep)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &ep_history))
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)
}
