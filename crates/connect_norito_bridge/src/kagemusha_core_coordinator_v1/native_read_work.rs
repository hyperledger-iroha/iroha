//! Read-only terminal recovery under original native index, authenticated device and current cut.
use super::super::archives::{
    KagemushaCoreSenderCandidateArchiveV1, KagemushaCoreSenderRecoveryArchiveV1,
};
use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{SenderPhaseV1, SenderReplyBodyV1};
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOutgoingOperationPhaseV1, KagemushaOutgoingOperationRecordV1,
};
use iroha_data_model::kagemusha::KagemushaAggregateStateCommitmentV1;

impl AuthenticatedRecoveredOwnerV1 {
    pub(in super::super) fn read_terminal_work(
        &self,
        method: super::super::KagemushaCoreCoordinatorMethodV1,
        fields: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        use super::super::KagemushaCoreCoordinatorMethodV1 as M;
        self.require_current()?;
        let result = match method {
            M::AcceptInstalledTerminal => self.accept_installed_terminal(fields),
            M::RecoverSender => self.recover_sender(fields),
            M::RecoverTerminalEnvelope => self.recover_terminal(fields),
            _ => Err(RegistryError::Rejected),
        }?;
        self.require_current()?;
        Ok(result)
    }

    fn indexed_record(&self, operation: [u8; 32]) -> Result<KagemushaOutgoingOperationRecordV1> {
        self.core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .outgoing_record_for_operation(operation)
            .map_err(|_| RegistryError::Rejected)?
            .ok_or(RegistryError::Rejected)
    }
    fn require_installed_reply(
        &self,
        native: &KagemushaOutgoingOperationRecordV1,
        operation: u8,
        original_reply: &[u8],
        envelope: &[u8],
    ) -> Result<()> {
        let signed = self.original_sender_reply(operation, native.operation_id, original_reply)?;
        let SenderReplyBodyV1::Lookup(Some(item)) = &signed.reply().body else {
            return Err(RegistryError::Rejected);
        };
        super::super::native_core_work::require_record_match(native, &item.record)
            .map_err(|_| RegistryError::Rejected)?;
        if item.record.phase != SenderPhaseV1::Installed
            || item.record.candidate_digest != native.candidate_digest
            || item.record.commit_certificate_digest != native.commit_certificate_digest
            || item.record.envelope_digest != native.envelope_digest
            || item.record.terminal_receipt_digest != native.terminal_receipt_digest
        {
            return Err(RegistryError::Rejected);
        }
        require_installed_reply_envelope(signed.command(), &item.canonical_envelope, envelope)
    }
    fn accept_installed_terminal(&self, fields: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        if fields.len() != 5 {
            return Err(RegistryError::Rejected);
        }
        let candidate = KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&fields[0])
            .map_err(|_| RegistryError::Rejected)?;
        let native = self.indexed_record(candidate.preparation.operation_id)?;
        if native.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || native.context != candidate.preparation.context
            || native.inputs_digest != candidate.preparation.inputs_digest
            || native.preparation_id != candidate.selector.preparation_id
            || native.candidate_digest != Some(candidate.candidate_digest)
        {
            return Err(RegistryError::Rejected);
        }
        let original = self
            .core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .original_terminal_envelope(native.operation_id)
            .map_err(|_| RegistryError::Rejected)?;
        if original != fields[1] {
            return Err(RegistryError::Rejected);
        }
        self.require_installed_reply(&native, 9, &fields[2], &original)?;
        self.require_installed_reply(&native, 10, &fields[3], &original)?;
        let (aggregate_wire, revision, pending, retries) = self
            .observer
            .authenticated_wallet_snapshot(&fields[4])
            .map_err(|_| RegistryError::Rejected)?;
        let aggregate =
            KagemushaAggregateStateCommitmentV1::decode_canonical_exact(&aggregate_wire)
                .map_err(|_| RegistryError::Rejected)?;
        let expected = self
            .core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .current_wallet_observation()
            .map_err(|_| RegistryError::Rejected)?;
        if aggregate != expected.aggregate
            || revision != expected.journal_revision
            || pending != expected.pending_credit_count
            || retries != expected.retry_outbox_count
        {
            return Err(RegistryError::Rejected);
        }
        Ok(vec![original, aggregate_wire])
    }
    fn recover_sender(&self, fields: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        self.require_current()?;
        let (selector, requested_kind) = sender_recovery_selector(fields)?;
        let qualification = self
            .observer
            .sender_qualification(&fields[3..])
            .map_err(|_| RegistryError::Rejected)?;
        let owner = self.core.selected().map_err(|_| RegistryError::Rejected)?;
        let before = owner
            .current_wallet_observation()
            .map_err(|_| RegistryError::Rejected)?;
        let current = owner
            .sender_context()
            .map_err(|_| RegistryError::Rejected)?;
        let release = owner
            .authenticated_release()
            .map_err(|_| RegistryError::Rejected)?;
        require_sender_recovery_qualification(
            &current,
            &qualification,
            release.provider_policy_root(),
        )?;
        let native = match selector {
            NativeSenderRecoverySelectorV1::Terminal(id) => owner.outgoing_record_for_terminal(id),
            NativeSenderRecoverySelectorV1::Operation(id) => {
                owner.outgoing_record_for_operation(id)
            }
        }
        .map_err(|_| RegistryError::Rejected)?;
        let recovery = match native {
            None => None,
            Some(native) => {
                let archive = KagemushaCoreSenderRecoveryArchiveV1 {
                    version: 1,
                    operation_id: native.operation_id,
                    terminal_id: native.outcome_id,
                    context: native.context,
                    inputs_digest: native.inputs_digest,
                };
                let recovery = sender_recovery_projection(
                    selector,
                    requested_kind,
                    native.operation_kind,
                    native.phase,
                    archive,
                    &current,
                )?;
                if let Some(archive) = &recovery {
                    let envelope = owner
                        .original_terminal_envelope(archive.operation_id)
                        .map_err(|_| RegistryError::Rejected)?;
                    let digest = crate::kagemusha_device_bridge_v1::sender_payload::terminal_envelope_digest_v1(&envelope)
                        .map_err(|_| RegistryError::Rejected)?;
                    if native.envelope_digest != Some(digest) {
                        return Err(RegistryError::Rejected);
                    }
                }
                recovery
            }
        };
        let after = owner
            .current_wallet_observation()
            .map_err(|_| RegistryError::Rejected)?;
        if before != after
            || owner
                .sender_context()
                .map_err(|_| RegistryError::Rejected)?
                != current
        {
            return Err(RegistryError::Rejected);
        }
        self.require_current()?;
        match recovery {
            None => Ok(Vec::new()),
            Some(recovery) => Ok(vec![
                recovery.operation_id.to_vec(),
                recovery.terminal_id.to_vec(),
                recovery
                    .encode_canonical()
                    .map_err(|_| RegistryError::Rejected)?,
            ]),
        }
    }
    fn recover_terminal(&self, fields: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        if fields.len() != 2 {
            return Err(RegistryError::Rejected);
        }
        let recovery = KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&fields[0])
            .map_err(|_| RegistryError::Rejected)?;
        let native = self.indexed_record(recovery.operation_id)?;
        if native.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || native.outcome_id != recovery.terminal_id
            || native.context != recovery.context
            || native.inputs_digest != recovery.inputs_digest
        {
            return Err(RegistryError::Rejected);
        }
        let envelope = self
            .core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .original_terminal_envelope(native.operation_id)
            .map_err(|_| RegistryError::Rejected)?;
        self.require_installed_reply(&native, 10, &fields[1], &envelope)?;
        Ok(vec![envelope])
    }
}

// Op9 acknowledges installation without exporting bytes; only op10 returns retry bytes.
// The original envelope here has already been reauthenticated by the selected native Core.
fn require_installed_reply_envelope(
    command: &crate::kagemusha_device_bridge_v1::sender_payload::SenderCommandV1,
    observed: &[u8],
    original: &[u8],
) -> Result<()> {
    use crate::kagemusha_device_bridge_v1::sender_payload::{
        SenderCommandBodyV1, SenderRecoverySelectorV1,
    };
    if original.is_empty() {
        return Err(RegistryError::Rejected);
    }
    match (&command.body, command.operation) {
        (SenderCommandBodyV1::Install { envelope, .. }, 9)
            if observed.is_empty() && envelope.as_slice() == original =>
        {
            Ok(())
        }
        (
            SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Lookup { .. },
            },
            10,
        ) if observed == original => Ok(()),
        _ => Err(RegistryError::Rejected),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeSenderRecoverySelectorV1 {
    Terminal([u8; 32]),
    Operation([u8; 32]),
}

fn sender_recovery_selector(
    fields: &[Vec<u8>],
) -> Result<(
    NativeSenderRecoverySelectorV1,
    iroha_data_model::kagemusha::KagemushaOperationKindV1,
)> {
    use iroha_data_model::kagemusha::KagemushaOperationKindV1 as Kind;
    if fields.len() != 8 {
        return Err(RegistryError::Rejected);
    }
    let id: [u8; 32] = fields[1]
        .as_slice()
        .try_into()
        .map_err(|_| RegistryError::Rejected)?;
    if id == [0; 32] {
        return Err(RegistryError::Rejected);
    }
    let selector = match fields[0].as_slice() {
        [0] => NativeSenderRecoverySelectorV1::Terminal(id),
        [1] => NativeSenderRecoverySelectorV1::Operation(id),
        _ => return Err(RegistryError::Rejected),
    };
    let kind = match fields[2].as_slice() {
        [0, 0, 0, 0] => Kind::SendSplit,
        [1, 0, 0, 0] => Kind::RedeemSplit,
        _ => return Err(RegistryError::Rejected),
    };
    Ok((selector, kind))
}

fn require_sender_recovery_qualification(
    current: &crate::kagemusha_device_bridge_v1::sender_payload::SenderWalletContextV1,
    qualification: &crate::kagemusha_device_bridge_v1::QualificationProjectionV1,
    provider_root: [u8; 32],
) -> Result<()> {
    if current.credential_id != qualification.credential.credential_id
        || current.release.release_id != qualification.release_id
        || current.release.hardware_profile_id != qualification.profile.hardware_profile_id
        || current.release.suite_id != qualification.credential.suite_id
        || current.release.policy_epoch != qualification.profile.policy_epoch
        || current.device_policy_binding.hardware_policy_id != provider_root
        || current.core_authorization_key_reference
            != qualification.core_authorization_key_reference
        || current.lane.network_id != qualification.credential.network_id
        || current.lane.device_lane_id != qualification.credential.lane_commitment
        || current.hardware_epoch.generation
            != u128::from(qualification.credential.hardware_epoch_generation)
        || current.hardware_epoch.epoch_id != qualification.credential.hardware_epoch_id
        || current.device_policy_binding.device_key_reference
            != qualification.credential.device_key_reference
    {
        return Err(RegistryError::Rejected);
    }
    Ok(())
}

// This kernel encodes only already authenticated native record fields. It creates no owner,
// lease, proof or monetary capability, and cannot resolve an absent operation itself.
fn sender_recovery_projection(
    selector: NativeSenderRecoverySelectorV1,
    requested_kind: iroha_data_model::kagemusha::KagemushaOperationKindV1,
    native_kind: iroha_data_model::kagemusha::KagemushaOperationKindV1,
    phase: iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingOperationPhaseV1,
    archive: KagemushaCoreSenderRecoveryArchiveV1,
    current: &crate::kagemusha_device_bridge_v1::sender_payload::SenderWalletContextV1,
) -> Result<Option<KagemushaCoreSenderRecoveryArchiveV1>> {
    use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingOperationPhaseV1 as Phase;
    archive
        .validate_shape()
        .map_err(|_| RegistryError::Rejected)?;
    archive
        .context
        .validate_retained_against_native(current)
        .map_err(|_| RegistryError::Rejected)?;
    if native_kind != requested_kind
        || match selector {
            NativeSenderRecoverySelectorV1::Terminal(id) => archive.terminal_id != id,
            NativeSenderRecoverySelectorV1::Operation(id) => archive.operation_id != id,
        }
    {
        return Err(RegistryError::Rejected);
    }
    match phase {
        Phase::Installed => {
            archive
                .encode_canonical()
                .map_err(|_| RegistryError::Rejected)?;
            Ok(Some(archive))
        }
        Phase::Released => Ok(None),
        Phase::Prepared | Phase::CandidatePersisted | Phase::Committed => {
            Err(RegistryError::Rejected)
        }
    }
}

#[cfg(test)]
#[path = "native_read_work/tests.rs"]
mod tests;
