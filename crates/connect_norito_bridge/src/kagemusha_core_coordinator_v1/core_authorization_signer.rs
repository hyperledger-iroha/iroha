//! Native signing custody for an independently installed Core P256 authorization key.
//! Signature bytes remain untrusted until native verification; public keys supply no signer.

use super::KagemushaCoreCoordinatorBackendErrorV1 as Error;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationPreimageV1,
    SenderPreparationSelectorV1, SenderWalletContextV1,
};
use iroha_data_model::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};
use std::sync::Arc;

/// Independently provisioned Rust-only signing owner. C/JNI callers cannot install it.
/// The platform must retain actual native private-key custody and original selection.
pub trait KagemushaNativeCoreAuthorizationSignerV1: Send + Sync + 'static {
    /// Recheck retained original policy, key custody and platform availability.
    fn recheck_originals(&self) -> Result<(), Error>;
    /// Return the exact retained public key; the native coordinator independently pins it.
    fn original_public_key(&self) -> Result<KagemushaDevicePublicKeyV1, Error>;
    /// Produce the original fixed-width low-S ECDSA-P256-SHA256 signature over this ID.
    /// This is the maintained 32-byte authorization ID, not an application-selected payload.
    fn sign_authorization_id(&self, authorization_id: [u8; 32]) -> Result<Vec<u8>, Error>;
}

pub(super) struct RetainedCoreAuthorizationSignerV1 {
    key: KagemushaDevicePublicKeyV1,
    source: Arc<dyn KagemushaNativeCoreAuthorizationSignerV1>,
}
impl RetainedCoreAuthorizationSignerV1 {
    pub(super) fn new(
        original_installed_key: KagemushaDevicePublicKeyV1,
        source: Arc<dyn KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self, Error> {
        original_installed_key
            .validate()
            .map_err(|_| Error::Rejected)?;
        let retained = Self {
            key: original_installed_key,
            source,
        };
        retained.recheck()?;
        Ok(retained)
    }
    pub(super) fn recheck(&self) -> Result<(), Error> {
        self.source.recheck_originals()?;
        if self.source.original_public_key()? != self.key {
            return Err(Error::Rejected);
        }
        self.source.recheck_originals()
    }

    // The sole production candidate intake is an exclusive native-owned commit. A decoded
    // PersistedOutgoingCandidate or public archive cannot reach the signing callback.
    pub(super) fn sign_outgoing_commit(
        &self,
        commit: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutgoingCommitV1,
        original_hardware_one_use_nonce: [u8; 32],
    ) -> Result<Vec<u8>, Error> {
        self.recheck()?;
        let record = commit.operation_record().map_err(|_| Error::Rejected)?;
        let prepared = commit.prepared().map_err(|_| Error::Rejected)?;
        let terminal = commit
            .candidate()
            .map_err(|_| Error::Rejected)?
            .hardware_terminal_body()
            .map_err(|_| Error::Rejected)?;
        if record.preparation_id != prepared.preparation_id
            || record.outbox_reservation_id != prepared.outbox_reservation.reservation_id
            || record.candidate_digest != Some(terminal.candidate_envelope_digest)
            || record.phase
                != iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingOperationPhaseV1::CandidatePersisted
            || original_hardware_one_use_nonce == [0; 32]
        {
            return Err(Error::Rejected);
        }
        let unsigned = SenderHardwareAuthorizationPreimageV1 {
            version: 1,
            purpose: crate::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationPurposeV1::Commit,
            operation_id: record.operation_id,
            inputs_digest: record.inputs_digest,
            preparation_id: prepared.preparation_id,
            candidate_digest: terminal.candidate_envelope_digest,
            release_id: record.context.release.release_id,
            hardware_transition_statement: prepared.hardware_statement(),
            prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
            outbox_reservation_commitment: terminal.outbox_reservation_commitment,
            outcome_id: record.outcome_id,
            transition_nullifier: terminal.transition_nullifier,
            envelope_digest: None,
            terminal_receipt_digest: None,
            hardware_one_use_nonce: original_hardware_one_use_nonce,
            authorization_public_key: self.key,
        };
        let bytes = self.sign_command7(unsigned, &record.context)?;
        if commit.operation_record().map_err(|_| Error::Rejected)? != record {
            return Err(Error::Rejected);
        }
        self.recheck()?;
        Ok(bytes)
    }
    // Only this borrowed actual installed payment can supply release signing claims.
    pub(super) fn sign_payment_release(
        &self,
        selection: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedPaymentReleaseSelectionV1<'_>,
        original_nonce: [u8; 32],
    ) -> Result<Vec<u8>, Error> {
        use crate::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationPurposeV1;
        use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1;
        self.recheck()?;
        selection.recheck().map_err(|_| Error::Rejected)?;
        let record = selection.record().map_err(|_| Error::Rejected)?;
        let prepared = selection.prepared().map_err(|_| Error::Rejected)?;
        let terminal = selection
            .hardware_terminal_body()
            .map_err(|_| Error::Rejected)?;
        let receipt_digest = selection
            .terminal_receipt_digest()
            .map_err(|_| Error::Rejected)?;
        let envelope_digest = record.envelope_digest.ok_or(Error::Rejected)?;
        if original_nonce == [0;32] || record.context.core_authorization_key_reference !=
            crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1(&self.key)
        { return Err(Error::Rejected); }
        let unsigned = SenderHardwareAuthorizationPreimageV1 {
            version: 1,
            purpose: SenderHardwareAuthorizationPurposeV1::Release,
            operation_id: record.operation_id,
            inputs_digest: record.inputs_digest,
            preparation_id: prepared.preparation_id,
            candidate_digest: terminal.candidate_envelope_digest,
            release_id: record.context.release.release_id,
            hardware_transition_statement: prepared.hardware_statement(),
            prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
            outbox_reservation_commitment: terminal.outbox_reservation_commitment,
            outcome_id: record.outcome_id,
            transition_nullifier: terminal.transition_nullifier,
            envelope_digest: Some(envelope_digest),
            terminal_receipt_digest: Some(receipt_digest),
            hardware_one_use_nonce: original_nonce,
            authorization_public_key: self.key,
        };
        let id = unsigned.authorization_id().map_err(|_| Error::Rejected)?;
        let signature = self.source.sign_authorization_id(id)?;
        self.recheck()?;
        let signature =
            KagemushaDeviceSignatureV1::from_raw_bytes(&signature).map_err(|_| Error::Rejected)?;
        signature
            .verify(&self.key, &id)
            .map_err(|_| Error::Rejected)?;
        let authorization = unsigned
            .with_signature(signature)
            .map_err(|_| Error::Rejected)?;
        let bytes = SenderHardwareAuthorizationPreimageV1::encode_verified(&authorization)
            .map_err(|_| Error::Rejected)?;
        // The selector supplies original inputs/ACK; command construction is performed by the
        // native work owner, then the consuming release intake independently re-verifies it.
        if !matches!(
            record.inputs,
            Some(KagemushaOutgoingPublicInputsV1::SendSplit { .. })
        ) {
            return Err(Error::Rejected);
        }
        selection.recheck().map_err(|_| Error::Rejected)?;
        if selection.record().map_err(|_| Error::Rejected)? != record {
            return Err(Error::Rejected);
        }
        self.recheck()?;
        Ok(bytes)
    }
    // Only a full native-finality admission for an actual installed voucher reaches signing.
    pub(super) fn sign_redemption_release(
        &self,
        selection: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedRedemptionFinalitySelectionV1<'_>,
        original_nonce: [u8; 32],
        source: &dyn super::native_core_work::KagemushaNativeRedemptionFinalitySourceV1,
    ) -> Result<Vec<u8>, Error> {
        use crate::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationPurposeV1;
        use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1;
        self.recheck()?;
        source.recheck_originals()?;
        let now = source.current_trusted_native_time_ms()?;
        let record = selection
            .record_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        let prepared = selection
            .prepared_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        let terminal = selection
            .hardware_terminal_body_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        let receipt = selection
            .terminal_receipt_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        if original_nonce == [0; 32] || !matches!(record.inputs, Some(KagemushaOutgoingPublicInputsV1::RedeemSplit { .. }))
            || record.context.core_authorization_key_reference != crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1(&self.key) {
            return Err(Error::Rejected);
        }
        let unsigned = SenderHardwareAuthorizationPreimageV1 {
            version: 1,
            purpose: SenderHardwareAuthorizationPurposeV1::Release,
            operation_id: record.operation_id,
            inputs_digest: record.inputs_digest,
            preparation_id: prepared.preparation_id,
            candidate_digest: terminal.candidate_envelope_digest,
            release_id: record.context.release.release_id,
            hardware_transition_statement: prepared.hardware_statement(),
            prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
            outbox_reservation_commitment: terminal.outbox_reservation_commitment,
            outcome_id: record.outcome_id,
            transition_nullifier: terminal.transition_nullifier,
            envelope_digest: Some(record.envelope_digest.ok_or(Error::Rejected)?),
            terminal_receipt_digest: Some(receipt.canonical_digest().map_err(|_| Error::Rejected)?),
            hardware_one_use_nonce: original_nonce,
            authorization_public_key: self.key,
        };
        let id = unsigned.authorization_id().map_err(|_| Error::Rejected)?;
        let signature = self.source.sign_authorization_id(id)?;
        self.recheck()?;
        let signature =
            KagemushaDeviceSignatureV1::from_raw_bytes(&signature).map_err(|_| Error::Rejected)?;
        signature
            .verify(&self.key, &id)
            .map_err(|_| Error::Rejected)?;
        let bytes = SenderHardwareAuthorizationPreimageV1::encode_verified(
            &unsigned
                .with_signature(signature)
                .map_err(|_| Error::Rejected)?,
        )
        .map_err(|_| Error::Rejected)?;
        let after = source.current_trusted_native_time_ms()?;
        if after < now
            || selection
                .record_at_trusted_time(after)
                .map_err(|_| Error::Rejected)?
                != record
        {
            return Err(Error::Rejected);
        }
        source.recheck_originals()?;
        self.recheck()?;
        Ok(bytes)
    }
    // Only the native coordinator can call this after consuming the actual verified candidate.
    // Public preimage fields and a cryptographic signature alone are never candidate admission.
    pub(super) fn sign_command7(
        &self,
        unsigned: SenderHardwareAuthorizationPreimageV1,
        context: &SenderWalletContextV1,
    ) -> Result<Vec<u8>, Error> {
        self.recheck()?;
        if unsigned.authorization_public_key != self.key {
            return Err(Error::Rejected);
        }
        unsigned
            .validate_commit_context(context)
            .map_err(|_| Error::Rejected)?;
        let id = unsigned.authorization_id().map_err(|_| Error::Rejected)?;
        let original = self.source.sign_authorization_id(id)?;
        self.recheck()?;
        let signature =
            KagemushaDeviceSignatureV1::from_raw_bytes(&original).map_err(|_| Error::Rejected)?;
        signature
            .verify(&self.key, &id)
            .map_err(|_| Error::Rejected)?;
        let authorization = unsigned
            .with_signature(signature)
            .map_err(|_| Error::Rejected)?;
        let bytes = SenderHardwareAuthorizationPreimageV1::encode_verified(&authorization)
            .map_err(|_| Error::Rejected)?;
        // Reuse the exact operation7 verifier before forwarding/exposing any signed bytes.
        let command = SenderCommandV1 {
            version: 1,
            operation: 7,
            operation_id: authorization.operation_id,
            context: context.clone(),
            body: SenderCommandBodyV1::Commit {
                selector: SenderPreparationSelectorV1 {
                    inputs_digest: authorization.inputs_digest,
                    preparation_id: authorization.preparation_id,
                },
                candidate_digest: authorization.candidate_digest,
                hardware_authorization: bytes.clone(),
            },
        };
        let wire = command.encode_canonical().map_err(|_| Error::Rejected)?;
        SenderCommandV1::decode_canonical_exact(7, authorization.operation_id, &wire)
            .map_err(|_| Error::Rejected)?;
        self.recheck()?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;
