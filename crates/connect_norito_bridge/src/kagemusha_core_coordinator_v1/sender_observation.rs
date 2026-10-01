//! Original sender response authentication under the installed native owner.
//! This token retains signed public bytes; it is never a recursive proof or spend capability.

use super::KagemushaCoreCoordinatorBackendErrorV1 as Error;
use crate::kagemusha_device_bridge_v1::{
    QualificationProjectionV1,
    sender_payload::{
        SENDER_REPLY_MAX_BYTES_V1, SenderCommandV1, SenderReplyV1, SenderWalletContextV1,
    },
};
use iroha_data_model::kagemusha::{
    KagemushaDeviceSignatureV1, kagemusha_device_response_signing_bytes_v1,
};
use norito::DecodeLimits;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedSenderReplyV1 {
    command: SenderCommandV1,
    original_command: Vec<u8>,
    original_reply: Vec<u8>,
    original_authenticator: Vec<u8>,
    original_response: Option<Vec<u8>>,
    reply: SenderReplyV1,
}

impl AuthenticatedSenderReplyV1 {
    // The caller supplies only selections borrowed from the independently installed native
    // Core and its accepted op1 observer. No exported from-parts constructor exists.
    pub(super) fn authenticate(
        operation: u8,
        request_id: [u8; 32],
        command: &[u8],
        reply: &[u8],
        authenticator: &[u8],
        native_context: &SenderWalletContextV1,
        qualification: &QualificationProjectionV1,
        admitted_provider_policy_root: [u8; 32],
    ) -> Result<Self, Error> {
        let decoded = SenderCommandV1::decode_canonical_exact(operation, request_id, command)
            .map_err(|_| Error::Rejected)?;
        if native_context.credential_id != qualification.credential.credential_id
            || native_context.release.release_id != qualification.release_id
            || native_context.release.hardware_profile_id
                != qualification.profile.hardware_profile_id
            || native_context.device_policy_binding.hardware_policy_id
                != admitted_provider_policy_root
            || native_context.core_authorization_key_reference
                != qualification.core_authorization_key_reference
            || native_context.lane.network_id != qualification.credential.network_id
            || native_context.lane.device_lane_id != qualification.credential.lane_commitment
            || native_context.hardware_epoch.generation
                != u128::from(qualification.credential.hardware_epoch_generation)
            || native_context.hardware_epoch.epoch_id != qualification.credential.hardware_epoch_id
            || native_context.device_policy_binding.device_key_reference
                != qualification.credential.device_key_reference
            || reply.is_empty()
            || reply.len() > SENDER_REPLY_MAX_BYTES_V1
        {
            return Err(Error::Rejected);
        }
        let decoded_reply: SenderReplyV1 = norito::decode_canonical_with_limits(
            reply,
            DecodeLimits::new(
                SENDER_REPLY_MAX_BYTES_V1,
                SENDER_REPLY_MAX_BYTES_V1,
                SENDER_REPLY_MAX_BYTES_V1 * 4,
                SENDER_REPLY_MAX_BYTES_V1 * 8,
                32,
            ),
        )
        .map_err(|_| Error::Rejected)?;
        decoded_reply
            .validate_against(&decoded, native_context)
            .map_err(|_| Error::Rejected)?;
        let transcript = kagemusha_device_response_signing_bytes_v1(
            operation,
            request_id,
            command,
            reply,
            qualification.hardware_policy_digest,
            qualification.profile.qualification_report_digest,
        )
        .map_err(|_| Error::Rejected)?;
        KagemushaDeviceSignatureV1::from_raw_bytes(authenticator)
            .and_then(|signature| {
                signature.verify(&qualification.credential.device_public_key, &transcript)
            })
            .map_err(|_| Error::Rejected)?;
        Ok(Self {
            command: decoded,
            original_command: command.to_vec(),
            original_reply: reply.to_vec(),
            original_authenticator: authenticator.to_vec(),
            original_response: None,
            reply: decoded_reply,
        })
    }

    // Retain only the complete original response already authenticated by this token.
    // Exact framing equality supplies no additional signature or release authority.
    pub(super) fn retain_original_response(mut self, original: &[u8]) -> Result<Self, Error> {
        if self.command.operation != 12 {
            return Err(Error::Rejected);
        }
        require_original_response(
            self.command.operation,
            self.command.operation_id,
            &self.original_reply,
            &self.original_authenticator,
            original,
        )?;
        self.original_response = Some(original.to_vec());
        Ok(self)
    }
    pub(super) fn original_response(&self) -> Result<&[u8], Error> {
        self.original_response.as_deref().ok_or(Error::Rejected)
    }
    pub(super) fn original_command(&self) -> &[u8] {
        &self.original_command
    }
    pub(super) fn original_reply(&self) -> &[u8] {
        &self.original_reply
    }
    pub(super) fn original_authenticator(&self) -> &[u8] {
        &self.original_authenticator
    }
    pub(super) fn command(&self) -> &SenderCommandV1 {
        &self.command
    }
    pub(super) fn reply(&self) -> &SenderReplyV1 {
        &self.reply
    }
    pub(super) fn require_original_reply(&self, bytes: &[u8]) -> Result<(), Error> {
        if self.original_reply == bytes {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
}

// Pure original-frame correlation; native response authentication and consuming release
// admission remain mandatory. Never rebuild a frame from its projected fields.
pub(super) fn require_original_response(
    operation: u8,
    request_id: [u8; 32],
    reply: &[u8],
    authenticator: &[u8],
    original: &[u8],
) -> Result<(), Error> {
    let decoded = iroha_data_model::kagemusha::kagemusha_decode_device_success_response_v1(
        original, operation, request_id,
    )
    .map_err(|_| Error::Rejected)?;
    if decoded.payload != reply || decoded.authenticator != authenticator {
        return Err(Error::Rejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
