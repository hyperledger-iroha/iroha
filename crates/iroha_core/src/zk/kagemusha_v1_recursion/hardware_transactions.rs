//! Governed device certificates for the independent inbox and recovery transactions.
//!
//! A qualified applet signs these records only after its atomic journal operation. They never
//! replace the recursive Guard proof for Bootstrap, MintFold, SendSplit, ReceiveFold, RedeemSplit
//! or Rotate. The release's exact governed profile authenticates the applet contract and key.

use std::sync::Arc;

use iroha_data_model::kagemusha::{
    KagemushaAuthenticatedReleaseV1, KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
};
use norito::{Decode, Encode};
use rand::{TryRngCore as _, rngs::OsRng};

use crate::zk::kagemusha_v1_state::{
    CreditStageStatementV1, DurabilityAnchorStatementV1, KagemushaLaneIdV1,
    KagemushaRecoveryCheckpointStatementV1, KagemushaRecoveryJournalsV1,
    MintReservationStatementV1, MintStageStatementV1,
};

const DOMAIN: &str = "iroha:kagemusha:v1:hardware-journal-certificate";
/// Maximum one independent hardware journal certificate, unrelated to history length.
pub const KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1: usize = 64 * 1024;

/// Complete transaction certified by the governed non-forking device service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::hardware_transactions::KagemushaHardwareTransactionV1"
)]
pub enum KagemushaHardwareTransactionV1 {
    /// Atomic reservation of sealed mint opening, key custody and inbox capacity.
    MintReservation(MintReservationStatementV1),
    /// Irreversible staging of the exact finalized mint into that reservation.
    MintStage(MintStageStatementV1),
    /// Irreversible staging of the exact peer credit and receipt.
    CreditStage(CreditStageStatementV1),
    /// Historical seal of the exact complete private recovery snapshot.
    DurabilityAnchor(DurabilityAnchorStatementV1),
    /// Atomic comparison and replacement of the complete current recovery checkpoint.
    RecoveryCheckpoint(KagemushaRecoveryCheckpointStatementV1),
    /// Fresh read of the current checkpoint and its selected durable journal prefixes.
    CurrentCheckpoint {
        /// Exact latest hardware-selected state, never an application-selected old checkpoint.
        statement: DurabilityAnchorStatementV1,
        /// Exact journal prefixes selected atomically with that state.
        journals: KagemushaRecoveryJournalsV1,
        /// Fresh challenge supplied by the verifier for this read only.
        challenge: [u8; 32],
    },
}

impl KagemushaHardwareTransactionV1 {
    /// Reject malformed transaction intent before durable staging or device dispatch.
    /// This checks structure only; certificates and actual applet execution remain mandatory.
    pub fn validate(&self) -> Result<(), String> {
        let (lane, generation, epoch_id) = self.lane_and_epoch();
        if lane.network_id.as_bytes() == &[0; 32]
            || lane.device_lane_id == [0; 32]
            || lane.scale > iroha_data_model::kagemusha::KAGEMUSHA_ASSET_SCALE_MAX_V1
            || generation == 0
            || epoch_id == [0; 32]
        {
            return Err("invalid Kagemusha hardware transaction lane or epoch".to_owned());
        }
        let valid = match self {
            Self::MintReservation(value) => {
                value.version == 1
                    && value.inbox_revision_before.checked_add(1)
                        == Some(value.inbox_revision_after)
                    && nonzero(&[
                        value.state_commitment,
                        value.reservation_digest,
                        value.predecessor_journal_commitment,
                        value.successor_journal_commitment,
                        value.successor_capacity_commitment,
                    ])
                    && value.predecessor_journal_commitment != value.successor_journal_commitment
            }
            Self::MintStage(value) => {
                value.version == 1
                    && value.inbox_revision_before.checked_add(1)
                        == Some(value.inbox_revision_after)
                    && nonzero(&[
                        value.state_commitment,
                        value.reservation_digest,
                        value.credit_id.0,
                        value.envelope_digest,
                        value.predecessor_journal_commitment,
                        value.successor_journal_commitment,
                        value.successor_capacity_commitment,
                    ])
                    && value.predecessor_journal_commitment != value.successor_journal_commitment
            }
            Self::CreditStage(value) => {
                value.version == 1
                    && value.journal_revision_before.checked_add(1)
                        == Some(value.journal_revision_after)
                    && nonzero(&[
                        value.receiver_state_commitment,
                        value.receiver_state_nonce_commitment,
                        value.receiver_device_policy_binding.device_key_reference,
                        value.receiver_device_policy_binding.hardware_policy_id,
                        value.credit_id.0,
                        value.envelope_digest,
                    ])
            }
            Self::DurabilityAnchor(value) => valid_anchor(value),
            Self::RecoveryCheckpoint(value) => {
                valid_anchor(&value.successor)
                    && value.operation_id != [0; 32]
                    && value.previous.revision.checked_add(1)
                        == Some(value.successor.metadata_revision)
                    && (value.previous.revision == 0)
                        == (value.previous.snapshot_commitment == [0; 32])
                    && value.previous.snapshot_commitment != value.successor.snapshot_commitment
            }
            Self::CurrentCheckpoint {
                statement,
                journals,
                challenge,
            } => {
                valid_anchor(statement)
                    && *challenge != [0; 32]
                    && journals.coordinator.sequence != 0
                    && journals.coordinator.byte_len != 0
                    && journals.responses.sequence != 0
                    && journals.responses.byte_len != 0
                    && nonzero(&[
                        journals.coordinator.head,
                        journals.responses.head,
                        journals.response_history_root,
                        journals.retirement_transition_id,
                    ])
            }
        };
        if !valid {
            return Err("invalid Kagemusha hardware transaction statement".to_owned());
        }
        Ok(())
    }

    fn lane_and_epoch(&self) -> (&KagemushaLaneIdV1, u128, [u8; 32]) {
        let (lane, epoch) = match self {
            Self::MintReservation(value) => (&value.lane, &value.hardware_epoch),
            Self::MintStage(value) => (&value.lane, &value.hardware_epoch),
            Self::CreditStage(value) => (&value.recipient_lane, &value.receiver_hardware_epoch),
            Self::DurabilityAnchor(value)
            | Self::CurrentCheckpoint {
                statement: value, ..
            } => (&value.lane, &value.hardware_epoch),
            Self::RecoveryCheckpoint(value) => {
                (&value.successor.lane, &value.successor.hardware_epoch)
            }
        };
        (lane, epoch.generation, epoch.epoch_id)
    }
}

fn nonzero(digests: &[[u8; 32]]) -> bool {
    digests.iter().all(|digest| *digest != [0; 32])
}

fn valid_anchor(value: &DurabilityAnchorStatementV1) -> bool {
    value.version == 1
        && nonzero(&[
            value.state_commitment,
            value.state_nonce_commitment,
            value.snapshot_commitment,
            value.device_policy_binding.device_key_reference,
            value.device_policy_binding.hardware_policy_id,
        ])
}

/// Canonical signed subject; all fields are covered by the hardware signature.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::hardware_transactions::KagemushaHardwareTransactionSubjectV1",
    frame = "iroha.kagemusha.v1.hardware-journal-subject"
)]
pub struct KagemushaHardwareTransactionSubjectV1 {
    /// Sole subject version.
    pub version: u16,
    /// Must equal the fixed hardware journal domain.
    pub domain: String,
    /// Original retry identity consumed by the device's durable transaction index.
    pub request_id: [u8; 32],
    /// Exact admitting release, not the host's most recently downloaded release.
    pub release_id: [u8; 32],
    /// Exact governed profile-list digest.
    pub hardware_policy_digest: [u8; 32],
    /// Original governance-signed credential of the device that committed the transaction.
    pub credential: KagemushaHardwareCredentialV1,
    /// Trusted hardware commit/read time, authenticated by the same signature.
    pub committed_at_ms: u64,
    /// Complete operation-specific statement, including all predecessor/successor commitments.
    pub transaction: KagemushaHardwareTransactionV1,
}

impl KagemushaHardwareTransactionSubjectV1 {
    /// Construct the fixed-domain subject in the qualified hardware service.
    pub fn new(
        request_id: [u8; 32],
        release_id: [u8; 32],
        hardware_policy_digest: [u8; 32],
        credential: KagemushaHardwareCredentialV1,
        committed_at_ms: u64,
        transaction: KagemushaHardwareTransactionV1,
    ) -> Self {
        Self {
            version: 1,
            domain: DOMAIN.to_owned(),
            request_id,
            release_id,
            hardware_policy_digest,
            credential,
            committed_at_ms,
            transaction,
        }
    }

    /// Exact canonical signature payload. Creating bytes conveys no hardware authority.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        if bytes.len() > KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1 {
            return Err("Kagemusha hardware transaction exceeds its byte bound".to_owned());
        }
        Ok(bytes)
    }
}

/// Hardware-signed independent transaction evidence; decoding alone supplies no authority.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::hardware_transactions::KagemushaHardwareTransactionCertificateV1",
    frame = "iroha.kagemusha.v1.hardware-journal-certificate"
)]
pub struct KagemushaHardwareTransactionCertificateV1 {
    /// Exact original signed subject.
    pub subject: KagemushaHardwareTransactionSubjectV1,
    /// Low-S P-256 signature under the subject's governed device key.
    pub signature: KagemushaDeviceSignatureV1,
}

/// Platform I/O for a verifier-owned fresh checkpoint read.
///
/// This interface returns untrusted bytes; the verifier authenticates the complete certificate
/// and challenge itself. It grants neither signing nor monetary authority to its implementation.
pub trait KagemushaHardwareCheckpointTransportV1: Send + Sync {
    /// Read the latest applet checkpoint using this exact fresh challenge.
    fn read_current_checkpoint(&self, challenge: [u8; 32]) -> Result<Vec<u8>, String>;
}

/// Immutable release and wallet pins for independent hardware transaction verification.
#[derive(Clone)]
pub struct KagemushaHardwareTransactionVerifierV1 {
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    lane: KagemushaLaneIdV1,
    profile_id: [u8; 32],
    transport: Arc<dyn KagemushaHardwareCheckpointTransportV1>,
}

impl KagemushaHardwareTransactionVerifierV1 {
    /// Bind a platform transport to one externally authenticated release, profile and wallet.
    /// The transport is never an authority source, and no key is read from the host journal.
    pub fn new(
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        lane: KagemushaLaneIdV1,
        profile_id: [u8; 32],
        transport: Arc<dyn KagemushaHardwareCheckpointTransportV1>,
    ) -> Result<Self, String> {
        if release.enabled_profile(profile_id).is_none()
            || lane.device_lane_id == [0; 32]
            || lane.network_id.as_bytes() == &[0; 32]
            || lane.scale > iroha_data_model::kagemusha::KAGEMUSHA_ASSET_SCALE_MAX_V1
        {
            return Err(
                "Kagemusha hardware transaction profile or lane is not admitted".to_owned(),
            );
        }
        Ok(Self {
            release,
            lane,
            profile_id,
            transport,
        })
    }

    pub(super) fn release(&self) -> &KagemushaAuthenticatedReleaseV1 {
        &self.release
    }

    /// Bind durable native storage to the exact authority, artifacts, profile and wallet.
    pub fn storage_binding(&self) -> Result<[u8; 32], String> {
        use sha2::{Digest as _, Sha256};
        let mut digest = Sha256::new();
        digest.update(b"iroha:kagemusha:v1:hardware-journal-storage\0");
        digest.update(self.release.attestation_digest());
        digest.update(self.release.authority_policy_digest());
        digest.update(self.profile_id);
        digest.update(norito::encode_canonical(&self.lane).map_err(|error| error.to_string())?);
        Ok(digest.finalize().into())
    }

    /// Authenticate exact expected evidence, including its original governed credential and time.
    /// Historical evidence is checked at commit time, so delayed delivery cannot expire money.
    pub fn verify(
        &self,
        expected: &KagemushaHardwareTransactionV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.verify_inner(None, expected, bytes)
    }

    /// Verify both the exact device retry identity and complete transaction statement.
    pub fn verify_for_request(
        &self,
        request_id: [u8; 32],
        expected: &KagemushaHardwareTransactionV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.verify_inner(Some(request_id), expected, bytes)
    }

    fn verify_inner(
        &self,
        request_id: Option<[u8; 32]>,
        expected: &KagemushaHardwareTransactionV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        expected.validate()?;
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1 {
            return Err("invalid Kagemusha hardware certificate length".to_owned());
        }
        let maximum = KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1;
        let certificate: KagemushaHardwareTransactionCertificateV1 =
            norito::decode_canonical_with_limits(
                bytes,
                norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
            )
            .map_err(|error| error.to_string())?;
        if norito::encode_canonical(&certificate).map_err(|error| error.to_string())? != bytes {
            return Err("noncanonical Kagemusha hardware certificate".to_owned());
        }
        let subject = &certificate.subject;
        let credential = &subject.credential;
        let profile = self
            .release
            .enabled_profile(self.profile_id)
            .ok_or_else(|| "Kagemusha hardware profile is absent".to_owned())?;
        credential
            .validate_against_profile(&profile.hardware_profile)
            .map_err(|error| error.to_string())?;
        let (lane, generation, epoch_id) = expected.lane_and_epoch();
        if subject.version != 1
            || subject.domain != DOMAIN
            || subject.request_id == [0; 32]
            || request_id.is_some_and(|id| id != subject.request_id)
            || subject.release_id != self.release.release_id()
            || subject.hardware_policy_digest != self.release.hardware_policy_digest()
            || &subject.transaction != expected
            || lane != &self.lane
            || credential.network_id != self.lane.network_id
            || credential.lane_commitment != self.lane.device_lane_id
            || credential.hardware_profile_id != self.profile_id
            || credential.suite_id != profile.suite_id
            || u128::from(credential.hardware_epoch_generation) != generation
            || credential.hardware_epoch_id != epoch_id
            || subject.committed_at_ms < credential.issued_at_ms
            || subject.committed_at_ms >= credential.expires_at_ms
        {
            return Err("Kagemusha hardware certificate binding mismatch".to_owned());
        }
        let anchor = match expected {
            KagemushaHardwareTransactionV1::DurabilityAnchor(value)
            | KagemushaHardwareTransactionV1::CurrentCheckpoint {
                statement: value, ..
            } => Some(value),
            KagemushaHardwareTransactionV1::RecoveryCheckpoint(value) => Some(&value.successor),
            _ => None,
        };
        if anchor.is_some_and(|anchor| {
            anchor.device_policy_binding.device_key_reference != credential.device_key_reference
                || anchor.device_policy_binding.hardware_policy_id
                    != self.release.provider_policy_root()
        }) {
            return Err("Kagemusha checkpoint credential substitution".to_owned());
        }
        if let KagemushaHardwareTransactionV1::CreditStage(value) = expected {
            if value.receiver_device_policy_binding.device_key_reference
                != credential.device_key_reference
                || value.receiver_device_policy_binding.hardware_policy_id
                    != self.release.provider_policy_root()
            {
                return Err("Kagemusha credit staging credential substitution".to_owned());
            }
        }
        let staged_at = match expected {
            KagemushaHardwareTransactionV1::MintStage(value) => Some(value.staged_at_ms),
            KagemushaHardwareTransactionV1::CreditStage(value) => Some(value.staged_at_ms),
            _ => None,
        };
        if staged_at.is_some_and(|time| time != subject.committed_at_ms) {
            return Err("Kagemusha staging time differs from hardware commit time".to_owned());
        }
        if let KagemushaHardwareTransactionV1::RecoveryCheckpoint(value) = expected {
            if value.operation_id != subject.request_id {
                return Err("Kagemusha checkpoint operation identity mismatch".to_owned());
            }
        }
        certificate
            .signature
            .verify(&credential.device_public_key, &subject.signing_bytes()?)
            .map_err(|error| error.to_string())
    }

    /// Authenticate a newly challenged current checkpoint and the exact journal selections.
    /// Each call consumes independent OS entropy; persisted signatures never establish freshness.
    pub fn verify_current(
        &self,
        statement: &DurabilityAnchorStatementV1,
        journals: &KagemushaRecoveryJournalsV1,
    ) -> Result<(), String> {
        let mut challenge = [0; 32];
        OsRng
            .try_fill_bytes(&mut challenge)
            .map_err(|error| error.to_string())?;
        if challenge == [0; 32] {
            return Err("Kagemusha checkpoint entropy unavailable".to_owned());
        }
        let bytes = self.transport.read_current_checkpoint(challenge)?;
        self.verify_for_request(
            challenge,
            &KagemushaHardwareTransactionV1::CurrentCheckpoint {
                statement: statement.clone(),
                journals: journals.clone(),
                challenge,
            },
            &bytes,
        )
    }
}
