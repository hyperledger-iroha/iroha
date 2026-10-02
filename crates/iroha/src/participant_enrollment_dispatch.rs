//! Owned exact enrollment requests and issuer-policy projection joins.
//!
//! These are retention and consistency checks, never FI customer admission, Native
//! startup, signer custody or monetary authority. The real FI must retain its admitted
//! customer/native resources while a genuine installed issuer parent borrows this data.
use crate::participant_enrollment_request::{
    MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES, ParticipantEnrollmentOperationV1,
    VerifiedParticipantEnrollmentRequestV1,
};
use eyre::{Result, ensure};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KagemushaRetailEnrollmentIssuerPolicyV1, kagemusha_ordinary_retail_issuer_policy_digest_v1,
    },
};

/// Closed issuer purpose derived exclusively from the verified request operation.
/// Selecting this purpose grants no custody or right to send a worker command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticipantEnrollmentIssuerPurposeV1 {
    /// Native durable C reservation, dedicated signing and publication before response.
    NativePreparation,
    /// Full platform evidence admission in the existing private worker.
    RawAttestation,
    /// Final app identity credential in the existing private worker.
    Credential,
}
impl ParticipantEnrollmentIssuerPurposeV1 {
    /// Existing private worker phase. C preparation has a separate native producer.
    #[must_use]
    pub const fn worker_phase(self) -> Option<&'static str> {
        match self {
            Self::NativePreparation => None,
            Self::RawAttestation => Some("raw"),
            Self::Credential => Some("credential"),
        }
    }
}

/// Own the exact body together with its real signature/certified S/W request evidence.
/// No decoder, DTO, marker, status flag or body digest can construct this value.
/// This evidence alone supplies no FI customer or installed issuer capability.
pub struct RetainedParticipantEnrollmentRequestV1 {
    request: VerifiedParticipantEnrollmentRequestV1,
    original_body: Box<[u8]>,
}
impl RetainedParticipantEnrollmentRequestV1 {
    /// Retain received bytes after verification, without parsing or reserializing JSON.
    /// # Errors
    /// Rejects an empty/oversized body, changed bytes or expired challenged observation.
    pub fn retain(request: VerifiedParticipantEnrollmentRequestV1, body: Vec<u8>) -> Result<Self> {
        ensure!(
            !body.is_empty() && body.len() <= MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES,
            "retained enrollment body outside bound"
        );
        request.verify_original_body(&body)?;
        Ok(Self {
            request,
            original_body: body.into_boxed_slice(),
        })
    }
    /// Exact original signed subject; it remains owned until the full dispatch completes.
    #[must_use]
    pub fn request(&self) -> &VerifiedParticipantEnrollmentRequestV1 {
        &self.request
    }
    /// Recheck finite request evidence and borrow the immutable received body.
    /// # Errors
    /// Rejects an expired challenged observation or changed retained bytes.
    pub fn original_body(&self) -> Result<&[u8]> {
        self.request.verify_original_body(&self.original_body)?;
        Ok(&self.original_body)
    }
    /// Borrow a fixed-purpose dispatch while keeping the original request/body alive.
    /// The FI must independently recheck its actual current release/customer/native scope.
    /// # Errors
    /// Rejects an expired challenged observation or changed retained bytes.
    pub fn dispatch(&self) -> Result<ParticipantEnrollmentIssuerDispatchV1<'_>> {
        self.original_body()?;
        Ok(ParticipantEnrollmentIssuerDispatchV1 { retained: self })
    }
}

/// Borrowed original request and fixed purpose, not an authenticated worker channel.
/// Keeping this borrow prevents the retained request/body from being dropped during dispatch.
pub struct ParticipantEnrollmentIssuerDispatchV1<'a> {
    retained: &'a RetainedParticipantEnrollmentRequestV1,
}
impl ParticipantEnrollmentIssuerDispatchV1<'_> {
    /// Real original signed subject, including FI, actor, S/W, network and business identity.
    #[must_use]
    pub fn request(&self) -> &VerifiedParticipantEnrollmentRequestV1 {
        self.retained.request()
    }
    /// Select only the operation authenticated by the original client signature.
    #[must_use]
    pub fn purpose(&self) -> ParticipantEnrollmentIssuerPurposeV1 {
        match self.request().operation() {
            ParticipantEnrollmentOperationV1::Prepare => {
                ParticipantEnrollmentIssuerPurposeV1::NativePreparation
            }
            ParticipantEnrollmentOperationV1::RawAttestation => {
                ParticipantEnrollmentIssuerPurposeV1::RawAttestation
            }
            ParticipantEnrollmentOperationV1::Certificate => {
                ParticipantEnrollmentIssuerPurposeV1::Credential
            }
        }
    }
    /// Borrow exact received bytes again immediately before the real parent sends/produces.
    /// # Errors
    /// Rejects an expired challenged observation or changed retained bytes.
    pub fn original_body(&self) -> Result<&[u8]> {
        self.retained.original_body()
    }
}

/// Exact public C policy fields projected from a separately admitted native issuer policy.
/// This data value deliberately supplies no installation, signer or customer authority.
/// Only the genuine Native parent may select the input policy and publish this projection.
pub struct ParticipantIssuerPreparationProjectionV1 {
    issuer_policy_digest: [u8; 32],
    core_preparation_public_key: [u8; 32],
    network: NetworkId,
    authentication_namespace: String,
}
impl ParticipantIssuerPreparationProjectionV1 {
    /// Project the model's exact Ed25519 issuer key and canonical policy digest.
    /// The caller must have independently admitted the policy's authority/current interval.
    /// Shape validation never grants that authority, and app-authority/P256 keys are not inputs.
    /// # Errors
    /// Rejects invalid policy shape or a reserved/wrong-width preparation key.
    pub fn from_native_issuer_policy(
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<Self> {
        issuer.validate().map_err(|e| eyre::eyre!(e.to_string()))?;
        let (_, key) = issuer.issuer_public_key.to_bytes();
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| eyre::eyre!("preparation issuer key has wrong width"))?;
        ensure!(key != [0; 32], "reserved preparation issuer key");
        Ok(Self {
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(issuer)
                .map_err(|e| eyre::eyre!(e))?,
            core_preparation_public_key: key,
            network: issuer.runtime.network_id,
            authentication_namespace: issuer.runtime.authentication_namespace.to_string(),
        })
    }
    /// Canonical issuer-policy digest for the Python policy projection's exact field.
    #[must_use]
    pub fn issuer_policy_digest(&self) -> [u8; 32] {
        self.issuer_policy_digest
    }
    /// Exact native `issuer_public_key` for Python `core_preparation_public_key`.
    #[must_use]
    pub fn core_preparation_public_key(&self) -> [u8; 32] {
        self.core_preparation_public_key
    }
    /// Require the projected Python fields to equal this exact native issuer policy.
    /// Equality with an app-authority key is allowed if the governed originals permit it;
    /// it is never inferred or used to fill a missing C key.
    /// # Errors
    /// Rejects another policy digest or another C signing key.
    pub fn require_projected_fields(&self, policy_digest: [u8; 32], c_key: [u8; 32]) -> Result<()> {
        ensure!(
            policy_digest == self.issuer_policy_digest && c_key == self.core_preparation_public_key,
            "Python preparation projection differs from native issuer original"
        );
        Ok(())
    }
    /// Require the retained signed request to address this policy's native runtime namespace/network.
    /// This does not admit FI customer state, select an account/lane, or establish policy custody.
    /// # Errors
    /// Rejects stale evidence or another exact native network/authentication namespace.
    pub fn require_request_runtime(
        &self,
        dispatch: &ParticipantEnrollmentIssuerDispatchV1<'_>,
    ) -> Result<()> {
        dispatch.original_body()?;
        ensure!(
            dispatch.request().network_id() == &self.network
                && dispatch.request().namespace() == self.authentication_namespace,
            "issuer runtime differs from retained request"
        );
        Ok(())
    }
}
