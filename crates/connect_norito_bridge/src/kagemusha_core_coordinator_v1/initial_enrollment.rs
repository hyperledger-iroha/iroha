//! One-use native issuer enrollment before an account/device wallet-open ceremony.
//!
//! The native owner starts one suspend-inclusive deadline and creates its client nonce before
//! HTTP. Completing this object proves a recent response by the independently pinned issuer
//! and both possession signatures. It supplies neither a hardware commit clock nor monetary
//! authority. Consuming challenge and proof phases retain the selected qualification and derive
//! signing inputs locally; certificate completion cannot substitute another proof. See the
//! adjacent `initial_enrollment/README.md` for the integration and validation boundary.
//! The owned pending phase retains the bounded journal's original live ticket. TODO: install a
//! qualified native backend for the sole public lifecycle ABI; there is no host approval fallback.

use std::sync::Arc;

use iroha_core::zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1;
use iroha_crypto::{Algorithm, Signature, SignatureOf};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1, KagemushaAppAttestationAuthorityPolicyV1,
    KagemushaAppDevicePolicyBindingV1, KagemushaAppEnrollmentCertificateV1,
    KagemushaAppEnrollmentSelectionV1, KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1,
    KagemushaDeviceQualificationReplyV1, KagemushaDeviceReadCredentialCommandV1,
    KagemushaRetailEnrollmentAccountProofV1, KagemushaRetailEnrollmentCertificateV1,
    KagemushaRetailEnrollmentChallengeV1, KagemushaRetailEnrollmentIssuerPolicyV1,
    KagemushaRetailEnrollmentOwnerV1, KagemushaRetailEnrollmentPossessionProofV1,
    KagemushaRetailEnrollmentSelectionV1, KagemushaVerifiedAppEnrollmentV1,
    KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1, kagemusha_device_key_reference_v1,
    kagemusha_verify_device_response_v1,
};
#[cfg(test)]
use rand::{TryRngCore as _, rngs::OsRng};
use sha2::{Digest as _, Sha256};

use super::native_deadline::NativeDeadlineV1;
use super::{
    enrollment_attempt_journal::{
        KagemushaEnrollmentJournalErrorV1, KagemushaEnrollmentLiveSelectionV1,
    },
    signed_app_preparation::{SignedAppPreparationPinsV1, verify_signed_app_preparation_v1},
};
use crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1;

#[cfg(test)]
const LIFETIME: std::time::Duration = std::time::Duration::from_secs(120);

// Raw Android attestation envelopes may be 128 KiB under the verifier's bounded wire contract.
// They stay in the Rust-only context provider and are never a coordinator frame field.
const PLATFORM_EVIDENCE_MAX_BYTES_V1: usize = 128 * 1024;

/// Closed initial-ceremony failures; no rejected result exposes a partial admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialEnrollmentErrorV1 {
    Encoding,
    Binding,
    Authority,
    RandomUnavailable,
    Expired,
}

type Result<T> = std::result::Result<T, InitialEnrollmentErrorV1>;

fn map_journal_error(error: KagemushaEnrollmentJournalErrorV1) -> InitialEnrollmentErrorV1 {
    match error {
        KagemushaEnrollmentJournalErrorV1::Expired => InitialEnrollmentErrorV1::Expired,
        KagemushaEnrollmentJournalErrorV1::Unavailable => {
            InitialEnrollmentErrorV1::RandomUnavailable
        }
        KagemushaEnrollmentJournalErrorV1::Store => InitialEnrollmentErrorV1::Authority,
        _ => InitialEnrollmentErrorV1::Binding,
    }
}

/// Rust-owned pending attempt with independent immutable issuer/catalog/Core-key pins.
/// It cannot be decoded, cloned or restored from a host cache after process restart.
pub struct PendingIssuerEnrollmentV1 {
    policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    native_authorization_public_key: KagemushaDevicePublicKeyV1,
    qualification: KagemushaDeviceQualificationReplyV1,
    canonical_qualification: Vec<u8>,
    client_nonce: [u8; 32],
    expected_app_digest: Option<[u8; 32]>,
    live_selection: Option<KagemushaEnrollmentLiveSelectionV1>,
    #[cfg(test)]
    deadline: Option<NativeDeadlineV1>,
}

impl PendingIssuerEnrollmentV1 {
    /// Consume the original phase-1 native selection after app attestation and qualification.
    ///
    /// The qualified backend must obtain `live_selection` from its process-local journal,
    /// authenticate `release` and both policies independently, and supply trusted service time.
    /// The signed preparation is checked against the provisional selected key ID before the
    /// verifier certificate binds that ID to the attested device point. This constructor never
    /// creates or renews a nonce, ticket, lane, or deadline.
    pub fn begin_selected(
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
        app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        owner: KagemushaRetailEnrollmentOwnerV1,
        native_authorization_public_key: KagemushaDevicePublicKeyV1,
        selected_attested_key_id: [u8; 32],
        signed_preparation: &[u8],
        raw_platform_evidence: &[u8],
        canonical_app_certificate: &[u8],
        canonical_qualification: &[u8],
        trusted_now_ms: u64,
    ) -> Result<Self> {
        let selection = live_selection.require_live().map_err(map_journal_error)?;
        let (enrollment, qualification) = Self::validate_context(
            &policy,
            &app_policy,
            &release,
            &owner,
            &native_authorization_public_key,
            canonical_qualification,
        )?;
        let app_policy_digest = app_policy
            .canonical_digest()
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if selection.account_i105
            != owner
                .account_id
                .canonical_i105()
                .map_err(|_| InitialEnrollmentErrorV1::Binding)?
            || selection.client_nonce == [0; 32]
            || selection.release_id != release.release_id()
            || selection.hardware_profile_id != qualification.credential.hardware_profile_id
            || selection.lane_id != owner.lane_id
            || live_selection.pins().issuer_policy_id != policy.issuer_policy_id
            || live_selection.pins().app_policy_digest != app_policy_digest
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let prep = verify_signed_app_preparation_v1(
            signed_preparation,
            SignedAppPreparationPinsV1 {
                policy: &policy,
                account_id: &owner.account_id,
                platform_class: app_policy.platform_class,
                selected_attested_key_id,
                client_nonce: selection.client_nonce,
                release_id: selection.release_id,
                profile_id: selection.hardware_profile_id,
                lane_id: selection.lane_id,
                trusted_now_ms,
            },
        )
        .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if raw_platform_evidence.is_empty()
            || raw_platform_evidence.len() > PLATFORM_EVIDENCE_MAX_BYTES_V1
            || canonical_app_certificate.is_empty()
            || canonical_app_certificate.len() > 96 * 1024
        {
            return Err(InitialEnrollmentErrorV1::Encoding);
        }
        let certificate: KagemushaAppEnrollmentCertificateV1 =
            norito::decode_canonical(canonical_app_certificate)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let credential = &qualification.credential;
        let point_key_id: [u8; 32] =
            Sha256::digest(credential.device_public_key.as_sec1_bytes()).into();
        let evidence_digest: [u8; 32] = Sha256::digest(raw_platform_evidence).into();
        if point_key_id == [0; 32]
            || credential.device_key_reference
                != kagemusha_device_key_reference_v1(&credential.device_public_key)
            || certificate.assertion.platform_evidence_digest != evidence_digest
            || certificate.assertion.device_key_reference != credential.device_key_reference
            || certificate.assertion.attested_key_id != point_key_id
            || (app_policy.platform_class
                == iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::AppleAppAttest
                && prep.attested_key_id != point_key_id)
            || (app_policy.platform_class
                == iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::AndroidKeyMint
                && prep.attested_key_id != [0; 32])
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let expected = KagemushaAppEnrollmentSelectionV1::for_credential(
            selection.client_nonce,
            prep.server_nonce,
            selection.release_id,
            credential,
        );
        let verified_app = certificate
            .authenticate(&app_policy, expected, trusted_now_ms)
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let client_nonce = selection.client_nonce;
        let pending = Self {
            policy,
            app_policy,
            release,
            enrollment,
            native_authorization_public_key,
            qualification,
            canonical_qualification: canonical_qualification.to_vec(),
            client_nonce,
            expected_app_digest: Some(verified_app.digest()),
            live_selection: Some(live_selection),
            #[cfg(test)]
            deadline: None,
        };
        pending.require_unexpired()?;
        Ok(pending)
    }

    /// Test-only construction for fixed-signature kernel vectors. Production callers
    /// must use `begin_selected`, which consumes the journal's original nonce and deadline.
    #[cfg(test)]
    pub fn begin(
        policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
        app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        owner: KagemushaRetailEnrollmentOwnerV1,
        native_authorization_public_key: KagemushaDevicePublicKeyV1,
        canonical_qualification: &[u8],
    ) -> Result<Self> {
        let deadline =
            NativeDeadlineV1::start(LIFETIME).map_err(|_| InitialEnrollmentErrorV1::Expired)?;
        let (enrollment, qualification) = Self::validate_context(
            &policy,
            &app_policy,
            &release,
            &owner,
            &native_authorization_public_key,
            canonical_qualification,
        )?;
        let mut client_nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut client_nonce)
            .map_err(|_| InitialEnrollmentErrorV1::RandomUnavailable)?;
        if client_nonce == [0; 32] {
            return Err(InitialEnrollmentErrorV1::RandomUnavailable);
        }
        deadline
            .check()
            .map_err(|_| InitialEnrollmentErrorV1::Expired)?;
        Ok(Self {
            policy,
            app_policy,
            release,
            enrollment,
            native_authorization_public_key,
            qualification,
            canonical_qualification: canonical_qualification.to_vec(),
            client_nonce,
            expected_app_digest: None,
            live_selection: None,
            deadline: Some(deadline),
        })
    }

    fn validate_context(
        policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
        app_policy: &KagemushaAppAttestationAuthorityPolicyV1,
        release: &KagemushaAuthenticatedReleaseV1,
        owner: &KagemushaRetailEnrollmentOwnerV1,
        native_authorization_public_key: &KagemushaDevicePublicKeyV1,
        canonical_qualification: &[u8],
    ) -> Result<(
        KagemushaRecoveryEnrollmentBindingV1,
        KagemushaDeviceQualificationReplyV1,
    )> {
        policy
            .validate()
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if app_policy.authority_key.algorithm() != Algorithm::Ed25519
            || app_policy.app_signing_identity_digest == [0; 32]
            || app_policy.app_release_digest == [0; 32]
            || app_policy.maximum_lifetime_ms == 0
        {
            return Err(InitialEnrollmentErrorV1::Authority);
        }
        native_authorization_public_key
            .validate()
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if owner.runtime != policy.runtime
            || owner
                .account_id
                .controller()
                .single_signatory()
                .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let enrollment_id = owner
            .enrollment_id()
            .map_err(|_| InitialEnrollmentErrorV1::Binding)?;
        let qualification =
            KagemushaDeviceQualificationReplyV1::decode_canonical_exact(canonical_qualification)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let enabled = release
            .enabled_profile(qualification.credential.hardware_profile_id)
            .ok_or(InitialEnrollmentErrorV1::Binding)?;
        if qualification.release_id != release.release_id()
            || qualification.hardware_policy_digest != release.hardware_policy_digest()
            || qualification.core_authorization_key_reference
                != hardware_authorization_key_reference_v1(native_authorization_public_key)
            || qualification.profile != enabled.hardware_profile
            || qualification.credential.suite_id != enabled.suite_id
            || qualification.credential.network_id != owner.runtime.network_id
            || qualification.credential.lane_commitment != owner.lane_id
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        qualification
            .credential
            .validate_app_policy_binding_for_release(
                &enabled.hardware_profile,
                release.release_id(),
                app_policy,
            )
            .map_err(|_| InitialEnrollmentErrorV1::Binding)?;
        Ok((
            KagemushaRecoveryEnrollmentBindingV1 {
                enrollment_id,
                owner: owner.clone(),
            },
            qualification,
        ))
    }

    /// Read only the original native nonce; reading it never starts a new lifetime.
    pub fn client_nonce(&self) -> Result<[u8; 32]> {
        self.require_unexpired()?;
        Ok(self.client_nonce)
    }

    /// Exact selected qualification body for the start request; reading never renews time.
    pub fn canonical_qualification(&self) -> Result<&[u8]> {
        self.require_unexpired()?;
        Ok(&self.canonical_qualification)
    }

    pub(super) fn deadline(&self) -> Result<NativeDeadlineV1> {
        self.require_unexpired()?;
        if let Some(live) = &self.live_selection {
            return live.deadline().map_err(map_journal_error);
        }
        #[cfg(test)]
        {
            return self
                .deadline
                .clone()
                .ok_or(InitialEnrollmentErrorV1::Binding);
        }
        #[cfg(not(test))]
        {
            Err(InitialEnrollmentErrorV1::Binding)
        }
    }

    fn require_unexpired(&self) -> Result<()> {
        if let Some(live) = &self.live_selection {
            live.require_live().map_err(map_journal_error)?;
            return Ok(());
        }
        #[cfg(test)]
        if let Some(deadline) = &self.deadline {
            return deadline
                .check()
                .map(|_| ())
                .map_err(|_| InitialEnrollmentErrorV1::Expired);
        }
        Err(InitialEnrollmentErrorV1::Binding)
    }

    /// Consume the start attempt before inspecting the issuer response. Projections supplied
    /// by HTTP are checked against locally derived bytes and never authorize signing.
    /// Native C/JNI must copy bounded inputs and consume its exact registry ticket first.
    pub fn accept_challenge(
        self,
        canonical_challenge: &[u8],
        projection: IssuerChallengeProjectionV1<'_>,
        verified_app: KagemushaVerifiedAppEnrollmentV1,
    ) -> Result<AcceptedIssuerChallengeV1> {
        self.require_unexpired()?;
        let challenge =
            KagemushaRetailEnrollmentChallengeV1::decode_canonical_exact(canonical_challenge)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.require_challenge_binding(&challenge)?;
        self.require_verified_app_binding(&challenge, &verified_app)?;
        let account_signing_message = challenge
            .account_signing_message()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let device_request_id = challenge
            .device_request_id()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let canonical_device_command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        if projection.challenge_id != device_request_id
            || projection.device_request_id != device_request_id
            || projection.account_signing_message != account_signing_message
            || projection.canonical_device_command != canonical_device_command
            || projection.expires_at_ms != challenge.expires_at_ms
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        self.require_unexpired()?;
        Ok(AcceptedIssuerChallengeV1 {
            pending: self,
            challenge,
            account_signing_message,
            device_request_id,
            canonical_device_command,
            verified_app,
        })
    }

    /// Authenticate the exact signed app certificate carried by phase 2 before accepting the
    /// issuer challenge. The original selected credential and client nonce, rather than app
    /// projections, determine the certificate's expected scope. Production attempts created by
    /// `begin_selected` additionally pin this certificate's digest to the raw evidence checked
    /// at selection time.
    pub fn accept_challenge_with_certificate(
        self,
        canonical_challenge: &[u8],
        projection: IssuerChallengeProjectionV1<'_>,
        canonical_app_certificate: &[u8],
        trusted_now_ms: u64,
    ) -> Result<AcceptedIssuerChallengeV1> {
        self.require_unexpired()?;
        if canonical_app_certificate.is_empty() || canonical_app_certificate.len() > 96 * 1024 {
            return Err(InitialEnrollmentErrorV1::Encoding);
        }
        let challenge =
            KagemushaRetailEnrollmentChallengeV1::decode_canonical_exact(canonical_challenge)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let certificate: KagemushaAppEnrollmentCertificateV1 =
            norito::decode_canonical(canonical_app_certificate)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let expected = KagemushaAppEnrollmentSelectionV1::for_credential(
            self.client_nonce,
            challenge.server_nonce,
            self.qualification.release_id,
            &self.qualification.credential,
        );
        let verified_app = certificate
            .authenticate(&self.app_policy, expected, trusted_now_ms)
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        self.accept_challenge(canonical_challenge, projection, verified_app)
    }

    fn require_challenge_binding(
        &self,
        challenge: &KagemushaRetailEnrollmentChallengeV1,
    ) -> Result<()> {
        let issuance = &challenge.issuance;
        if challenge.client_nonce != self.client_nonce
            || challenge.owner != self.enrollment.owner
            || challenge.issuer_policy_id != self.policy.issuer_policy_id
            || challenge.issuer_audience != self.policy.issuer_audience
            || issuance.release_id != self.qualification.release_id
            || issuance.hardware_policy_digest != self.qualification.hardware_policy_digest
            || issuance.core_authorization_key_reference
                != self.qualification.core_authorization_key_reference
            || issuance.credential != self.qualification.credential
            || challenge.issued_at_ms < self.policy.valid_from_ms
            || challenge.expires_at_ms > self.policy.expires_at_ms
            || challenge.issued_at_ms < self.qualification.credential.issued_at_ms
            || challenge.expires_at_ms > self.qualification.credential.expires_at_ms
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        // These are interval-shape checks only. No unsigned challenge timestamp becomes
        // trusted UTC; freshness remains the original native continuous deadline. Only
        // the later authenticated issuer decision checks historical issuance validity.
        Ok(())
    }

    fn require_verified_app_binding(
        &self,
        challenge: &KagemushaRetailEnrollmentChallengeV1,
        verified_app: &KagemushaVerifiedAppEnrollmentV1,
    ) -> Result<()> {
        let selected_app = verified_app.selection();
        let credential = &self.qualification.credential;
        let point_key_id: [u8; 32] =
            Sha256::digest(credential.device_public_key.as_sec1_bytes()).into();
        let pinned_binding = KagemushaAppDevicePolicyBindingV1 {
            app_signing_identity_digest: self.app_policy.app_signing_identity_digest,
            app_release_digest: self.app_policy.app_release_digest,
            release_id: self.qualification.release_id,
            hardware_profile_id: credential.hardware_profile_id,
            device_key_reference: credential.device_key_reference,
            lane_id: self.enrollment.owner.lane_id,
        }
        .canonical_digest()
        .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if verified_app.authority_policy() != self.app_policy.as_ref()
            || self
                .expected_app_digest
                .is_some_and(|digest| digest != verified_app.digest())
            || verified_app.static_binding_digest() != pinned_binding
            || credential.app_policy_binding_digest != pinned_binding
            || challenge.app_attestation_digest != verified_app.digest()
            || selected_app.client_nonce != self.client_nonce
            || selected_app.server_nonce != challenge.server_nonce
            || selected_app.release_id != self.qualification.release_id
            || selected_app.hardware_profile_id != credential.hardware_profile_id
            || selected_app.device_key_reference != credential.device_key_reference
            || selected_app.attested_key_id != point_key_id
            || selected_app.lane_id != self.enrollment.owner.lane_id
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        Ok(())
    }

    /// Internal final step. Only a prepared proof reaches this through the callable phase
    /// API; the host cannot replace proof bytes when supplying the issuer certificate.
    fn complete(
        self,
        canonical_proof: &[u8],
        canonical_certificate: &[u8],
        verified_app: &KagemushaVerifiedAppEnrollmentV1,
    ) -> Result<FreshIssuerAdmissionV1> {
        self.require_unexpired()?;
        let proof =
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(canonical_proof)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let certificate =
            KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(canonical_certificate)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.require_challenge_binding(&proof.challenge)?;
        self.require_verified_app_binding(&proof.challenge, verified_app)?;
        let issuance = &proof.challenge.issuance;
        // Initial credential identity comes from both possession proofs under the enabled
        // governed profile. This is allowed only before an existing Core/registry owner exists;
        // recovery must instead retain its opaque Core source and stronger credential floor.
        let expected = KagemushaRetailEnrollmentSelectionV1 {
            enrollment_id: self.enrollment.enrollment_id,
            account_id: self.enrollment.owner.account_id.clone(),
            lane_id: self.enrollment.owner.lane_id,
            issuance: issuance.clone(),
        };
        let evidence = proof
            .authenticate_issuer_evidence(
                &certificate,
                &self.policy,
                &self.release,
                &expected,
                self.client_nonce,
                verified_app,
            )
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let canonical_proof = canonical_proof.to_vec();
        let canonical_certificate = canonical_certificate.to_vec();
        self.require_unexpired()?;
        Ok(FreshIssuerAdmissionV1 {
            pending: self,
            evidence,
            canonical_proof,
            canonical_certificate,
        })
    }
}

/// Untrusted response projections, decoded from the strict HTTP response without authority.
/// The native phase compares both independently supplied identifiers with its own derivation.
pub struct IssuerChallengeProjectionV1<'a> {
    /// Digest identifying the issuer challenge.
    pub challenge_id: [u8; 32],
    /// Exact account-signing message, independently rederived by Core.
    pub account_signing_message: [u8; 32],
    /// Exact secure-device request identity.
    pub device_request_id: [u8; 32],
    /// Canonical command expected by the secure device.
    pub canonical_device_command: &'a [u8],
    /// Claimed issuer challenge expiry; Core checks it against the challenge.
    pub expires_at_ms: u64,
}

/// One retained signing request, not an authenticated issuer decision or device admission.
/// This state cannot be cloned, deserialized, or supplied a replacement challenge.
pub struct AcceptedIssuerChallengeV1 {
    pending: PendingIssuerEnrollmentV1,
    challenge: KagemushaRetailEnrollmentChallengeV1,
    account_signing_message: [u8; 32],
    device_request_id: [u8; 32],
    canonical_device_command: Vec<u8>,
    verified_app: KagemushaVerifiedAppEnrollmentV1,
}

impl AcceptedIssuerChallengeV1 {
    /// Require phase 3 to use the exact process-local journal owner that created this challenge.
    pub(super) fn require_same_live_selection(
        &self,
        live_selection: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<()> {
        let original = self
            .pending
            .live_selection
            .as_ref()
            .ok_or(InitialEnrollmentErrorV1::Binding)?;
        if !original
            .same_attempt(live_selection)
            .map_err(map_journal_error)?
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        Ok(())
    }

    pub fn account_signing_message(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.account_signing_message)
    }

    pub fn device_request_id(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.device_request_id)
    }

    pub fn canonical_device_command(&self) -> Result<&[u8]> {
        self.pending.require_unexpired()?;
        Ok(&self.canonical_device_command)
    }

    /// Verify the account signature and complete command-bound device frame, then retain
    /// the sole canonical proof for exact HTTP retries. No host time is used or returned.
    pub fn prepare_proof(
        self,
        raw_account_signature: &[u8],
        device_response: &[u8],
    ) -> Result<PreparedIssuerProofV1> {
        self.pending.require_unexpired()?;
        if raw_account_signature.len() != 64
            || device_response.is_empty()
            || device_response.len() > KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
        {
            return Err(InitialEnrollmentErrorV1::Encoding);
        }
        let account_signature: SignatureOf<KagemushaRetailEnrollmentAccountProofV1> =
            SignatureOf::from_signature(
                Signature::try_from_bytes(raw_account_signature)
                    .map_err(|_| InitialEnrollmentErrorV1::Authority)?,
            );
        let account_key = self
            .pending
            .enrollment
            .owner
            .account_id
            .controller()
            .single_signatory()
            .ok_or(InitialEnrollmentErrorV1::Authority)?;
        account_signature
            .verify(
                account_key,
                &self
                    .challenge
                    .account_signing_payload()
                    .map_err(|_| InitialEnrollmentErrorV1::Encoding)?,
            )
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let expected = &self.pending.qualification;
        let response = kagemusha_verify_device_response_v1(
            device_response,
            &self.canonical_device_command,
            1,
            self.device_request_id,
            expected.hardware_policy_digest,
            expected.profile.qualification_report_digest,
            &expected.credential.device_public_key,
        )
        .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let qualification =
            KagemushaDeviceQualificationReplyV1::decode_canonical_exact(response.payload)
                .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if qualification != *expected {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let proof = KagemushaRetailEnrollmentPossessionProofV1 {
            challenge: self.challenge,
            account_signature,
            device_response: device_response.to_vec(),
        };
        let canonical_proof = proof
            .canonical_bytes()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.pending.require_unexpired()?;
        Ok(PreparedIssuerProofV1 {
            pending: self.pending,
            challenge_id: self.device_request_id,
            canonical_proof,
            verified_app: self.verified_app,
        })
    }
}

/// Exact verified possession signatures awaiting the issuer decision. It confers no issuer
/// approval, trusted UTC or monetary authority. Retrying reads these same immutable bytes.
pub struct PreparedIssuerProofV1 {
    pending: PendingIssuerEnrollmentV1,
    challenge_id: [u8; 32],
    canonical_proof: Vec<u8>,
    verified_app: KagemushaVerifiedAppEnrollmentV1,
}

impl PreparedIssuerProofV1 {
    pub fn challenge_id(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.challenge_id)
    }

    pub fn canonical_proof(&self) -> Result<&[u8]> {
        self.pending.require_unexpired()?;
        Ok(&self.canonical_proof)
    }

    /// Consume this exact proof and the original native deadline before issuer admission.
    pub fn complete(self, canonical_certificate: &[u8]) -> Result<FreshIssuerAdmissionV1> {
        self.pending.complete(
            &self.canonical_proof,
            canonical_certificate,
            &self.verified_app,
        )
    }
}

/// Recent nonce-bound issuer response, not current UTC or a monetary bootstrap grant.
/// Construction consumes a native pending attempt and checks its original clock before and
/// after all three signatures. Registry selection/current-owner checks remain mandatory.
/// No constructor, deserializer or Clone permits a host to manufacture or duplicate this value.
pub struct FreshIssuerAdmissionV1 {
    pending: PendingIssuerEnrollmentV1,
    evidence: KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1,
    canonical_proof: Vec<u8>,
    canonical_certificate: Vec<u8>,
}

impl FreshIssuerAdmissionV1 {
    /// Recheck the original live ticket and deadline before this admission is used.
    pub(super) fn require_live(&self) -> Result<()> {
        self.pending.require_unexpired()
    }

    pub fn evidence(&self) -> &KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1 {
        &self.evidence
    }
    pub fn release(&self) -> &KagemushaAuthenticatedReleaseV1 {
        &self.pending.release
    }
    pub fn issuer_policy(&self) -> &KagemushaRetailEnrollmentIssuerPolicyV1 {
        &self.pending.policy
    }
    pub fn native_authorization_public_key(&self) -> &KagemushaDevicePublicKeyV1 {
        &self.pending.native_authorization_public_key
    }
    pub fn canonical_proof(&self) -> &[u8] {
        &self.canonical_proof
    }
    pub fn canonical_certificate(&self) -> &[u8] {
        &self.canonical_certificate
    }
    pub fn enrollment_binding(&self) -> &KagemushaRecoveryEnrollmentBindingV1 {
        &self.pending.enrollment
    }
    pub(super) fn deadline(&self) -> Result<NativeDeadlineV1> {
        self.pending.deadline()
    }
}

#[cfg(test)]
pub(super) mod tests;
