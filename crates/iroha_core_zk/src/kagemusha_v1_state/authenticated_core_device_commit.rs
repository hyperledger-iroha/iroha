//! Independent original op7 authentication before any concrete Core funds mutation.

use super::*;
use crate::kagemusha_sender_wire::{
    SENDER_REPLY_MAX_BYTES_V1, SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationV1,
    SenderPhaseV1, SenderReplyBodyV1, SenderReplyV1,
};
use iroha_data_model::kagemusha::{
    KagemushaCommitCertificateV1, kagemusha_verify_device_response_v1,
};

/// Exact original command and complete signed hardware success frame.
/// Raw bytes grant no authority until the actual Core independently verifies them.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OriginalOutgoingHardwareCommitV1")]
pub struct KagemushaOriginalOutgoingHardwareCommitV1 {
    /// Exact bounded canonical operation7 command, including the original Core authorization.
    pub canonical_command: Vec<u8>,
    /// Complete original command-correlated signed IKGMJRS1 success frame.
    pub original_response: Vec<u8>,
}

pub(super) fn verify(
    machine: &Machine,
    operation_id: DigestV1,
    candidate: &PersistedOutgoingCandidateV1,
    certificate: &KagemushaCommitCertificateV1,
    original: &KagemushaOriginalOutgoingHardwareCommitV1,
) -> Result<(), KagemushaStateErrorV1> {
    let rejected = || KagemushaStateErrorV1::HardwareCertificateMismatch;
    let retained_command = verify_command(
        machine,
        operation_id,
        candidate,
        &original.canonical_command,
    )?;
    let committed =
        CommittedOutgoingCandidateV1::from_hardware_commit(candidate.clone(), certificate.clone())?;
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or_else(rejected)?;
    let release = machine
        .guard_verifier
        .authenticated_release()
        .map_err(|_| rejected())?;
    let profile = release
        .enabled_profile(candidate.prepared.predecessor_state.hardware_profile_id)
        .ok_or_else(rejected)?;
    let credential = machine
        .recovery_metadata
        .accepted_credential
        .oem_original()?;
    let (command, reply) = verify_signed_reply(
        original,
        operation_id,
        &record.context,
        release.hardware_policy_digest(),
        profile.hardware_profile.qualification_report_digest,
        &credential.device_public_key,
    )?;
    if command != retained_command {
        return Err(rejected());
    }
    correlate_observed_record(record, &candidate.prepared, candidate, &committed, &reply)
}

// Canonical original command authentication grants no money or new owner. The same native
// candidate/context/signature binding is required before command exposure and completion.
pub(super) fn verify_command(
    machine: &Machine,
    operation_id: DigestV1,
    candidate: &PersistedOutgoingCandidateV1,
    canonical_command: &[u8],
) -> Result<SenderCommandV1, KagemushaStateErrorV1> {
    let rejected = || KagemushaStateErrorV1::HardwareCertificateMismatch;
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or_else(rejected)?;
    let prepared = &candidate.prepared;
    record
        .validate_against_prepared(prepared)
        .map_err(|_| rejected())?;
    if !matches!(
        record.phase,
        KagemushaOutgoingOperationPhaseV1::CandidatePersisted
            | KagemushaOutgoingOperationPhaseV1::Committed
    ) {
        return Err(rejected());
    }
    let terminal = candidate.hardware_terminal_body()?;
    let release = machine
        .guard_verifier
        .authenticated_release()
        .map_err(|_| rejected())?;
    let credential = machine
        .recovery_metadata
        .accepted_credential
        .oem_original()?;
    release
        .enabled_profile(prepared.predecessor_state.hardware_profile_id)
        .ok_or_else(rejected)?;
    if record.context.credential_id != credential.credential_id
        || record.context.release.release_id != release.release_id()
        || credential.hardware_profile_id != prepared.predecessor_state.hardware_profile_id
        || u128::from(credential.hardware_epoch_generation)
            != prepared.predecessor_state.hardware_epoch.generation
        || credential.hardware_epoch_id != prepared.predecessor_state.hardware_epoch.epoch_id
        || credential.device_key_reference
            != prepared
                .predecessor_state
                .device_policy_binding
                .device_key_reference
        || prepared
            .predecessor_state
            .device_policy_binding
            .hardware_policy_id
            != release.provider_policy_root()
        || record.context.core_authorization_key_reference
            != machine
                .enrollment_binding()
                .core_authorization_key_reference
    {
        return Err(rejected());
    }
    let command = SenderCommandV1::decode_canonical_exact(7, operation_id, canonical_command)
        .map_err(|_| rejected())?;
    if command.context != record.context {
        return Err(rejected());
    }
    let SenderCommandBodyV1::Commit {
        selector,
        candidate_digest,
        hardware_authorization,
    } = &command.body
    else {
        return Err(rejected());
    };
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(|_| rejected())?;
    if selector.inputs_digest != record.inputs_digest
        || selector.preparation_id != prepared.preparation_id
        || *candidate_digest != candidate.candidate_envelope_digest
        || authorization.hardware_transition_statement != prepared.hardware_statement()
        || authorization.prepared_one_use_authorization_digest
            != prepared.prepared_one_use_authorization_digest
        || authorization.outbox_reservation_commitment != terminal.outbox_reservation_commitment
        || authorization.outcome_id != record.outcome_id
        || authorization.transition_nullifier != terminal.transition_nullifier
        || authorization.hardware_one_use_nonce == [0; 32]
    {
        return Err(rejected());
    }
    Ok(command)
}

// Private pure original-byte authentication shared with genuine-signature diagnostic tests.
// This returns parsed data only and cannot construct a concrete owner or monetary capability.
pub(super) fn verify_signed_reply(
    original: &KagemushaOriginalOutgoingHardwareCommitV1,
    operation_id: DigestV1,
    expected_context: &crate::kagemusha_sender_wire::SenderWalletContextV1,
    hardware_policy_digest: DigestV1,
    qualification_report_digest: DigestV1,
    device_public_key: &iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
) -> Result<(SenderCommandV1, SenderReplyV1), KagemushaStateErrorV1> {
    let rejected = || KagemushaStateErrorV1::HardwareCertificateMismatch;
    let command =
        SenderCommandV1::decode_canonical_exact(7, operation_id, &original.canonical_command)
            .map_err(|_| rejected())?;
    if command.context != *expected_context {
        return Err(rejected());
    }
    let response = kagemusha_verify_device_response_v1(
        &original.original_response,
        &original.canonical_command,
        7,
        operation_id,
        hardware_policy_digest,
        qualification_report_digest,
        device_public_key,
    )
    .map_err(|_| rejected())?;
    if response.payload.len() > SENDER_REPLY_MAX_BYTES_V1 {
        return Err(rejected());
    }
    let reply: SenderReplyV1 = norito::decode_canonical_with_limits(
        response.payload,
        norito::DecodeLimits::new(
            SENDER_REPLY_MAX_BYTES_V1,
            SENDER_REPLY_MAX_BYTES_V1,
            SENDER_REPLY_MAX_BYTES_V1 * 4,
            SENDER_REPLY_MAX_BYTES_V1 * 8,
            32,
        ),
    )
    .map_err(|_| rejected())?;
    reply
        .validate_against(&command, expected_context)
        .map_err(|_| rejected())?;
    Ok((command, reply))
}

pub(super) fn correlate_observed_record(
    record: &KagemushaOutgoingOperationRecordV1,
    prepared: &PreparedOutgoingCandidateV1,
    candidate: &PersistedOutgoingCandidateV1,
    committed: &CommittedOutgoingCandidateV1,
    reply: &SenderReplyV1,
) -> Result<(), KagemushaStateErrorV1> {
    correlate_observed_fields(
        record,
        prepared.preparation_id,
        prepared.outbox_reservation.reservation_id,
        candidate.candidate_envelope_digest,
        committed.commit_certificate_digest,
        reply,
    )
}

// Pure projection equality only. The shipping caller derives every expected field from its
// authenticated native candidate and independently verified complete commit certificate.
pub(super) fn correlate_observed_fields(
    record: &KagemushaOutgoingOperationRecordV1,
    preparation_id: DigestV1,
    outbox_reservation_id: DigestV1,
    candidate_digest: DigestV1,
    commit_certificate_digest: DigestV1,
    reply: &SenderReplyV1,
) -> Result<(), KagemushaStateErrorV1> {
    let operation_id = record.operation_id;
    let rejected = || KagemushaStateErrorV1::HardwareCertificateMismatch;
    let SenderReplyBodyV1::Lookup(Some(item)) = &reply.body else {
        return Err(rejected());
    };
    let observed = &item.record;
    if observed.phase != SenderPhaseV1::Committed
        || observed.operation_id != operation_id
        || observed.context != record.context
        || observed.inputs_digest != record.inputs_digest
        || observed.operation_kind != record.operation_kind
        || observed.preparation_id != preparation_id
        || observed.outbox_reservation_id != outbox_reservation_id
        || observed.outcome_id != record.outcome_id
        || observed.candidate_digest != Some(candidate_digest)
        || observed.commit_certificate_digest != Some(commit_certificate_digest)
        || (record.phase == KagemushaOutgoingOperationPhaseV1::Committed
            && record.commit_certificate_digest != Some(commit_certificate_digest))
    {
        return Err(rejected());
    }
    Ok(())
}

#[cfg(test)]
#[path = "authenticated_core_device_commit_tests.rs"]
mod tests;
