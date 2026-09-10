//! Test-only one-use native account/device possession bound to authenticated retail enrollment.
//!
//! Pending challenges cannot be decoded, copied or created from host paths, owner projections
//! or caller freshness claims. Completion consumes the pending instance. Its opaque result is
//! possession evidence at a native monotonic instant, not a Core machine, KYC approval, wallet
//! lease or monetary capability. The owner registry must separately install and serialize use.

use std::time::Duration;

use iroha_core::zk::kagemusha_v1_state::{
    DurabilityAnchorStatementV1, KagemushaRecoveryEnrollmentBindingV1,
};
use iroha_crypto::{Algorithm, HashOf, Signature, SignatureOf};
use iroha_data_model::kagemusha::{
    KagemushaDeviceReadCredentialCommandV1, KagemushaHardwareCredentialV1,
    KagemushaRetailEnrollmentOwnerV1, kagemusha_decode_device_success_response_v1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use super::initial_enrollment::FreshIssuerAdmissionV1;
use super::native_deadline::{NativeContinuousInstantV1, NativeDeadlineV1};
use super::startup_qualification::{
    NativeReadObservationV1, NativeStartupQualificationOwnerV1, ObservationDispositionV1,
    ObservationErrorV1,
};
use crate::kagemusha_device_bridge_v1::{
    qualification_projection_v1, sender_payload::hardware_authorization_key_reference_v1,
};

const ACCOUNT_DOMAIN: &str = "iroha:kagemusha:v1:enrolled-open-account-possession";
const INITIAL_CERTIFICATE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:enrolled-open-initial-certificate";
const CHALLENGE_MAX_BYTES: usize = 16 * 1024;
pub(super) const LIFETIME: Duration = Duration::from_secs(120);
const LIFETIME_MS: u64 = 120_000;

/// Authenticated source retained by native construction and committed by the account signer.
/// Decoding this projection supplies no certificate or checkpoint authority.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::enrolled_open::EnrolledOpenAuthoritySourceV1",
    frame = "iroha.kagemusha.v1.enrolled-open-authority-source"
)]
pub(super) enum EnrolledOpenAuthoritySourceV1 {
    /// Digest of the entire canonical initially verified issuer certificate.
    InitialCertificate { certificate_digest: [u8; 32] },
    /// Complete selected checkpoint identity and digest of its original terminal certificate.
    RecoveryCheckpoint {
        statement: DurabilityAnchorStatementV1,
        terminal_certificate_digest: [u8; 32],
    },
}

/// Exact typed account signing payload. Its digest uses `HashOf`, not SHA-256 of these bytes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::enrolled_open::EnrolledOpenAccountChallengeV1",
    frame = "iroha.kagemusha.v1.enrolled-open-account-challenge"
)]
#[norito(deny_unknown_fields)]
pub(super) struct EnrolledOpenAccountChallengeV1 {
    version: u16,
    domain: String,
    enrollment_id: [u8; 32],
    owner: KagemushaRetailEnrollmentOwnerV1,
    nonce: [u8; 32],
    authority_source: EnrolledOpenAuthoritySourceV1,
    release_id: [u8; 32],
    hardware_policy_digest: [u8; 32],
    core_authorization_key_reference: [u8; 32],
    lifetime_ms: u64,
}

/// Closed failures that never grant partial possession evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnrolledOpenErrorV1 {
    Encoding,
    AccountController,
    AccountSignature,
    DeviceBinding,
    Expired,
    Observation(ObservationErrorV1),
}

impl From<ObservationErrorV1> for EnrolledOpenErrorV1 {
    fn from(error: ObservationErrorV1) -> Self {
        Self::Observation(error)
    }
}

type Result<T> = std::result::Result<T, EnrolledOpenErrorV1>;

// Admission supplies its credential from an opaque certificate. Recovery tests use structural
// epoch selectors; these cannot be serialized or exposed as alternative construction inputs.
enum RequiredCredentialV1 {
    Initial(KagemushaHardwareCredentialV1),
    Recovered {
        generation: u128,
        epoch_id: [u8; 32],
        key_reference: [u8; 32],
    },
}

impl RequiredCredentialV1 {
    fn accepts(&self, credential: &KagemushaHardwareCredentialV1) -> bool {
        match self {
            Self::Initial(initial) => credential == initial,
            Self::Recovered {
                generation,
                epoch_id,
                key_reference,
            } => {
                u128::from(credential.hardware_epoch_generation) == *generation
                    && credential.hardware_epoch_id == *epoch_id
                    && credential.device_key_reference == *key_reference
            }
        }
    }
}

/// One native-owned, non-clonable open attempt with an OS-random operation-1 nonce.
pub(super) struct PendingEnrolledOpenV1 {
    observer: NativeStartupQualificationOwnerV1,
    challenge: EnrolledOpenAccountChallengeV1,
    challenge_bytes: Vec<u8>,
    command: Vec<u8>,
    required_credential: RequiredCredentialV1,
    initial_enrollment: Option<FreshIssuerAdmissionV1>,
    deadline: NativeDeadlineV1,
}

impl PendingEnrolledOpenV1 {
    /// Consume the fresh nonce-bound issuer admission before initial device possession.
    /// This preserves its original deadline and exact evidence; it supplies no hardware
    /// commit clock or monetary bootstrap authority. Existing Core owners require recovery.
    pub(super) fn from_fresh_issuer_admission(admission: FreshIssuerAdmissionV1) -> Result<Self> {
        let deadline = admission
            .deadline()
            .map_err(|_| EnrolledOpenErrorV1::Expired)?;
        let observer = NativeStartupQualificationOwnerV1::from_fresh_issuer_admission(&admission)?;
        let certificate = admission.evidence().certificate();
        let mut pending = Self::begin(
            observer,
            admission.enrollment_binding().clone(),
            EnrolledOpenAuthoritySourceV1::InitialCertificate {
                certificate_digest: digest(
                    INITIAL_CERTIFICATE_DOMAIN,
                    admission.canonical_certificate(),
                ),
            },
            RequiredCredentialV1::Initial(certificate.subject.issuance.credential),
            admission.release().release_id(),
            admission.release().hardware_policy_digest(),
            hardware_authorization_key_reference_v1(admission.native_authorization_public_key()),
            deadline,
        )?;
        pending.initial_enrollment = Some(admission);
        pending.require_unexpired()?;
        Ok(pending)
    }

    // Private shared kernel. Production callers above have already authenticated every input;
    // tests may exercise this kernel with explicitly structural fixtures, never fake evidence.
    fn begin(
        mut observer: NativeStartupQualificationOwnerV1,
        enrollment: KagemushaRecoveryEnrollmentBindingV1,
        authority_source: EnrolledOpenAuthoritySourceV1,
        required_credential: RequiredCredentialV1,
        release_id: [u8; 32],
        hardware_policy_digest: [u8; 32],
        core_authorization_key_reference: [u8; 32],
        deadline: NativeDeadlineV1,
    ) -> Result<Self> {
        deadline.check().map_err(|_| EnrolledOpenErrorV1::Expired)?;
        let account_key = enrollment
            .owner
            .account_id
            .controller()
            .single_signatory()
            .ok_or(EnrolledOpenErrorV1::AccountController)?;
        if account_key.algorithm() != Algorithm::Ed25519 {
            return Err(EnrolledOpenErrorV1::AccountController);
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes()
            .map_err(|_| EnrolledOpenErrorV1::Encoding)?;
        let nonce = observer.begin(1, &command)?;
        let challenge = EnrolledOpenAccountChallengeV1 {
            version: 1,
            domain: ACCOUNT_DOMAIN.to_owned(),
            enrollment_id: enrollment.enrollment_id,
            owner: enrollment.owner,
            nonce,
            authority_source,
            release_id,
            hardware_policy_digest,
            core_authorization_key_reference,
            lifetime_ms: LIFETIME_MS,
        };
        let challenge_bytes = encode_bounded(&challenge, CHALLENGE_MAX_BYTES)?;
        deadline.check().map_err(|_| EnrolledOpenErrorV1::Expired)?;
        Ok(Self {
            observer,
            challenge,
            challenge_bytes,
            command,
            required_credential,
            initial_enrollment: None,
            deadline,
        })
    }

    /// Exact bounded canonical account challenge retained by this native pending instance.
    pub(super) fn challenge_bytes(&self) -> &[u8] {
        &self.challenge_bytes
    }

    /// Exact typed `HashOf` message for the account's Ed25519 signer.
    pub(super) fn account_signing_message(&self) -> [u8; 32] {
        *HashOf::new(&self.challenge).as_ref()
    }

    /// Canonical operation-1 command body correlated by the separate native request identity.
    pub(super) fn device_command(&self) -> &[u8] {
        &self.command
    }

    /// Native-created device request identity; callers cannot replace it on completion.
    pub(super) fn nonce(&self) -> [u8; 32] {
        self.challenge.nonce
    }

    /// Exact immutable source owner retained by this pending instance.
    pub(super) fn enrollment_binding(&self) -> KagemushaRecoveryEnrollmentBindingV1 {
        KagemushaRecoveryEnrollmentBindingV1 {
            enrollment_id: self.challenge.enrollment_id,
            owner: self.challenge.owner.clone(),
        }
    }

    /// Exact authenticated certificate/checkpoint source selected at native construction.
    pub(super) fn authority_source(&self) -> &EnrolledOpenAuthoritySourceV1 {
        &self.challenge.authority_source
    }

    /// Check the native-owned continuous deadline before reserving a pending registry attempt.
    pub(super) fn require_unexpired(&self) -> Result<()> {
        self.deadline
            .check()
            .map(|_| ())
            .map_err(|_| EnrolledOpenErrorV1::Expired)
    }

    /// Consume the one-use account/device proof under the retained challenge and native clock.
    pub(super) fn complete(
        mut self,
        account_signature: &[u8],
        full_device_response: &[u8],
    ) -> Result<VerifiedEnrolledOpenV1> {
        self.require_unexpired()?;
        if account_signature.len() != 64 {
            return Err(EnrolledOpenErrorV1::AccountSignature);
        }
        let account_key = self
            .challenge
            .owner
            .account_id
            .controller()
            .single_signatory()
            .filter(|key| key.algorithm() == Algorithm::Ed25519)
            .ok_or(EnrolledOpenErrorV1::AccountController)?;
        SignatureOf::<EnrolledOpenAccountChallengeV1>::from_signature(Signature::from_bytes(
            account_signature,
        ))
        .verify(account_key, &self.challenge)
        .map_err(|_| EnrolledOpenErrorV1::AccountSignature)?;

        let response = kagemusha_decode_device_success_response_v1(
            full_device_response,
            1,
            self.challenge.nonce,
        )
        .map_err(|_| EnrolledOpenErrorV1::DeviceBinding)?;
        let qualification = qualification_projection_v1(response.payload)
            .ok_or(EnrolledOpenErrorV1::DeviceBinding)?;
        if !self.required_credential.accepts(&qualification.credential)
            || qualification.release_id != self.challenge.release_id
            || qualification.hardware_policy_digest != self.challenge.hardware_policy_digest
            || qualification.core_authorization_key_reference
                != self.challenge.core_authorization_key_reference
        {
            return Err(EnrolledOpenErrorV1::DeviceBinding);
        }
        let mut fields = vec![
            1_u32.to_le_bytes().to_vec(),
            qualification.release_id.to_vec(),
            norito::encode_canonical(&qualification.profile)
                .map_err(|_| EnrolledOpenErrorV1::Encoding)?,
            norito::encode_canonical(&qualification.credential)
                .map_err(|_| EnrolledOpenErrorV1::Encoding)?,
            0xffff_u32.to_le_bytes().to_vec(),
            qualification.hardware_policy_digest.to_vec(),
        ];
        self.observer.stage_qualification(&fields)?;
        fields.pop();
        let (disposition, observation) = self.observer.accept(
            1,
            self.challenge.nonce,
            &self.command,
            response.payload,
            response.authenticator,
            &fields,
        )?;
        if disposition != ObservationDispositionV1::Fresh {
            return Err(EnrolledOpenErrorV1::DeviceBinding);
        }
        let completed_at = self
            .deadline
            .check()
            .map_err(|_| EnrolledOpenErrorV1::Expired)?;
        Ok(VerifiedEnrolledOpenV1 {
            observer: self.observer,
            evidence: VerifiedEnrolledOpenEvidenceV1 {
                challenge: self.challenge,
                observation,
                account_signature: account_signature
                    .try_into()
                    .map_err(|_| EnrolledOpenErrorV1::AccountSignature)?,
                initial_enrollment: self.initial_enrollment,
                deadline: self.deadline,
                completed_at,
            },
        })
    }
}

/// Opaque completed account/device possession, never a wallet, Core or KYC capability.
pub(super) struct VerifiedEnrolledOpenV1 {
    observer: NativeStartupQualificationOwnerV1,
    evidence: VerifiedEnrolledOpenEvidenceV1,
}

impl VerifiedEnrolledOpenV1 {
    /// Borrow exact completed possession evidence without granting registry admission.
    pub(super) fn evidence(&self) -> &VerifiedEnrolledOpenEvidenceV1 {
        &self.evidence
    }

    /// Transfer the single observer and its opaque evidence into the native owner registry.
    /// The registry must atomically check owner/checkpoint currency and separate current initial
    /// certificate/KYC admission. This clock check only bounds the possession attempt.
    pub(super) fn into_parts(
        self,
    ) -> Result<(
        NativeStartupQualificationOwnerV1,
        VerifiedEnrolledOpenEvidenceV1,
    )> {
        self.evidence.require_unexpired()?;
        Ok((self.observer, self.evidence))
    }
}

/// Non-clonable possession evidence transferred only by consuming the completed attempt.
pub(super) struct VerifiedEnrolledOpenEvidenceV1 {
    challenge: EnrolledOpenAccountChallengeV1,
    observation: NativeReadObservationV1,
    account_signature: [u8; 64],
    initial_enrollment: Option<FreshIssuerAdmissionV1>,
    deadline: NativeDeadlineV1,
    completed_at: NativeContinuousInstantV1,
}

impl VerifiedEnrolledOpenEvidenceV1 {
    /// Exact immutable identity authenticated by the account signature and source authority.
    pub(super) fn enrollment_binding(&self) -> KagemushaRecoveryEnrollmentBindingV1 {
        KagemushaRecoveryEnrollmentBindingV1 {
            enrollment_id: self.challenge.enrollment_id,
            owner: self.challenge.owner.clone(),
        }
    }

    /// Exact source to compare against the registry's still-current native owner selection.
    pub(super) fn authority_source(&self) -> &EnrolledOpenAuthoritySourceV1 {
        &self.challenge.authority_source
    }

    /// Fresh command-bound observation authenticated during this possession attempt.
    pub(super) fn observation(&self) -> &NativeReadObservationV1 {
        &self.observation
    }

    /// Original exact account signature over the typed account challenge hash.
    pub(super) fn account_signature(&self) -> &[u8; 64] {
        &self.account_signature
    }

    /// Exact bounded canonical challenge that the account signed.
    pub(super) fn challenge_bytes(&self) -> Result<Vec<u8>> {
        encode_bounded(&self.challenge, CHALLENGE_MAX_BYTES)
    }

    /// Original one-use issuer admission, including the full possession proof and policy.
    /// Its signed issuance timestamp is historical evidence, never a hardware commit clock.
    /// Recovery retains its opaque Core source and returns `None` here.
    pub(super) fn initial_enrollment(&self) -> Option<&FreshIssuerAdmissionV1> {
        self.initial_enrollment.as_ref()
    }

    /// Native continuous instant when both signatures passed the final challenge deadline check.
    pub(super) fn completed_at(&self) -> NativeContinuousInstantV1 {
        self.completed_at
    }

    /// Reject evidence retained beyond its native-owned 120-second possession deadline.
    pub(super) fn require_unexpired(&self) -> Result<()> {
        self.deadline
            .check()
            .map(|_| ())
            .map_err(|_| EnrolledOpenErrorV1::Expired)
    }
}

fn encode_bounded<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value).map_err(|_| EnrolledOpenErrorV1::Encoding)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(EnrolledOpenErrorV1::Encoding);
    }
    Ok(bytes)
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update([0]);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

#[cfg(test)]
pub(super) mod tests;

#[cfg(test)]
mod explicit_schema_identity_tests {
    use super::*;

    macro_rules! identity {
        ($root:ty, $nominal:literal, $frame:literal) => {
            assert_eq!(<$root as norito::NoritoSchema>::nominal_name(), $nominal);
            assert_eq!(<$root as norito::NoritoSchema>::frame_name(), $frame);
            assert_eq!(
                norito::schema::identity::frame_hash::<$root>(),
                norito::core::schema_hash_for_name($frame)
            );
            assert_eq!(
                <Vec<$root> as norito::NoritoSchema>::nominal_name(),
                format!("alloc::vec::Vec<{}>", $nominal)
            );
        };
    }

    fn roundtrip<T>(value: &T) -> Vec<u8>
    where
        T: norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let frame = norito::encode_canonical(value).expect("canonical fixture frame");
        let header = norito::core::Header::read(frame.as_slice()).expect("typed frame header");
        assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
        let decoded: T = norito::decode_canonical(&frame).expect("same root canonical replay");
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
        assert!(matches!(
            norito::decode_canonical::<Vec<T>>(&frame),
            Err(norito::Error::SchemaMismatch)
        ));
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(norito::decode_canonical::<T>(&trailing).is_err());
        frame
    }

    #[test]
    fn framed_roots_keep_nominal_and_protocol_identities() {
        identity!(
            EnrolledOpenAccountChallengeV1,
            "connect_norito_bridge::kagemusha_core_coordinator_v1::enrolled_open::EnrolledOpenAccountChallengeV1",
            "iroha.kagemusha.v1.enrolled-open-account-challenge"
        );

        use crate::kagemusha_core_coordinator_v1::startup_qualification::tests as fixture;
        let qualification = fixture::qualification(1);
        let enrollment = fixture::enrollment_binding(&qualification);
        let challenge = EnrolledOpenAccountChallengeV1 {
            version: 1,
            domain: ACCOUNT_DOMAIN.to_owned(),
            enrollment_id: enrollment.enrollment_id,
            owner: enrollment.owner,
            nonce: [71; 32],
            authority_source: EnrolledOpenAuthoritySourceV1::InitialCertificate {
                certificate_digest: [77; 32],
            },
            release_id: qualification.release_id,
            hardware_policy_digest: qualification.hardware_policy_digest,
            core_authorization_key_reference: qualification.core_authorization_key_reference,
            lifetime_ms: LIFETIME_MS,
        };
        let frame = roundtrip(&challenge);
        assert_eq!(
            encode_bounded(&challenge, CHALLENGE_MAX_BYTES).unwrap(),
            frame
        );
        let replay: EnrolledOpenAccountChallengeV1 = norito::decode_canonical(&frame).unwrap();
        assert_eq!(HashOf::new(&challenge), HashOf::new(&replay));
        let mut altered = replay;
        altered.nonce[0] ^= 1;
        assert_ne!(HashOf::new(&challenge), HashOf::new(&altered));
    }
}
