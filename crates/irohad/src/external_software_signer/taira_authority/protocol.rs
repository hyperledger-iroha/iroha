//! Canonical IRTAUT01 frames, identities, manifests, and durable records.

use super::super::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerLiveProvenanceV1, SoftwareSignerPublicBindingV1,
    SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1, SoftwareSignerSignatureReceiptV1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::{
    fmt,
    path::{Component, PathBuf},
    str::FromStr,
};

/// Canonical magic at the start of every decoded Taira authority frame.
pub const TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1: [u8; 8] = *b"IRTAUT01";
pub(super) const TAIRA_AUTHORITY_BINDING_MAGIC_V1: [u8; 8] = *b"IRTAUB01";
pub(super) const TAIRA_AUTHORITY_PROTOCOL_VERSION_V1: u16 = 1;
pub(super) const TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1: usize = 34 * 1024 * 1024;
pub(super) const TAIRA_AUTHORITY_MAX_JSON_BYTES_V1: usize = 32 * 1024 * 1024;
pub(super) const TAIRA_AUTHORITY_MAX_ARTIFACTS_V1: usize = 256;
pub(super) const TAIRA_AUTHORITY_MAX_ARTIFACT_BYTES_V1: u64 = 16 * 1024 * 1024 * 1024;
pub(super) const TAIRA_AUTHORITY_MAX_TOTAL_ARTIFACT_BYTES_V1: u64 = 24 * 1024 * 1024 * 1024;

pub(super) const FRAME_QUALIFY_REQUEST_V1: u8 = 1;
pub(super) const FRAME_QUALIFY_RESPONSE_V1: u8 = 2;
pub(super) const FRAME_AUTHORIZE_REQUEST_V1: u8 = 3;
pub(super) const FRAME_AUTHORIZE_RESPONSE_V1: u8 = 4;
pub(super) const FRAME_VERIFY_REQUEST_V1: u8 = 5;
pub(super) const FRAME_VERIFY_RESPONSE_V1: u8 = 6;
pub(super) const FRAME_ADMIN_REQUEST_V1: u8 = 7;
pub(super) const FRAME_ADMIN_RESPONSE_V1: u8 = 8;
const QUALIFY_RESPONSE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:taira:authority-qualify-response:v1\0";

/// Closed registry of Taira release-authority roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, clap::ValueEnum)]
#[repr(u8)]
pub enum TairaAuthorityRoleV1 {
    /// Independent native release-evidence admission.
    NativeEvidence = 1,
    /// Privacy-protocol controller-origin evidence.
    PrivacyProtocolOrigin = 2,
    /// Retained-genesis privacy governance transaction signing.
    PrivacyGovernance = 3,
    /// Native candidate qualification.
    Qualification = 4,
    /// Short-lived deployment authorization issuance.
    DeployIssuance = 5,
    /// Independent rollout plan/result observation.
    RolloutObservation = 6,
    /// Public-soak observation signing.
    PublicSoakObservation = 7,
    /// Public-soak consume-once replay admission.
    PublicSoakReplayAdmission = 8,
}

impl TairaAuthorityRoleV1 {
    /// All roles in canonical registry order.
    pub const ALL: [Self; 8] = [
        Self::NativeEvidence,
        Self::PrivacyProtocolOrigin,
        Self::PrivacyGovernance,
        Self::Qualification,
        Self::DeployIssuance,
        Self::RolloutObservation,
        Self::PublicSoakObservation,
        Self::PublicSoakReplayAdmission,
    ];

    /// Stable production role label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeEvidence => "native-evidence",
            Self::PrivacyProtocolOrigin => "privacy-protocol-origin",
            Self::PrivacyGovernance => "privacy-governance",
            Self::Qualification => "qualification",
            Self::DeployIssuance => "deploy-issuance",
            Self::RolloutObservation => "rollout-observation",
            Self::PublicSoakObservation => "public-soak-observation",
            Self::PublicSoakReplayAdmission => "public-soak-replay-admission",
        }
    }

    pub(super) const fn envelope_schema(self) -> &'static str {
        match self {
            Self::NativeEvidence => "iroha.taira.independent-native-evidence-authority.v1",
            Self::PrivacyProtocolOrigin => {
                "iroha.taira.privacy-protocol-controller-origin-authority.v1"
            }
            Self::PrivacyGovernance => "iroha.taira.privacy_governance_authority.v1",
            Self::Qualification => "iroha.taira.linux-native-qualification-authority.v1",
            Self::DeployIssuance => "iroha.taira.deploy-issuance-authority.v1",
            Self::RolloutObservation => {
                "iroha.taira.authenticated-rollout-observation-authority.v1"
            }
            Self::PublicSoakObservation => "iroha.taira.public-v2-24h-soak-authority-envelope.v1",
            Self::PublicSoakReplayAdmission => {
                "iroha.taira.public-v2-24h-soak-durable-admission-receipt.v1"
            }
        }
    }

    pub(super) const fn replay_namespace(self) -> &'static str {
        match self {
            Self::NativeEvidence => "iroha.taira.independent-native-evidence-authority-replay.v1",
            Self::PrivacyProtocolOrigin => {
                "iroha.taira.privacy-protocol-controller-origin-replay.v1"
            }
            Self::PrivacyGovernance => "iroha.taira.privacy_governance_authority_replay.v1",
            Self::Qualification => "iroha.taira.native-qualification-replay.v1",
            Self::DeployIssuance => "iroha.taira.deploy-issuance-replay.v1",
            Self::RolloutObservation => "iroha.taira.authenticated-rollout-observation-replay.v1",
            Self::PublicSoakObservation | Self::PublicSoakReplayAdmission => {
                "iroha.taira.public-v2-24h-soak-authority-replay.v1"
            }
        }
    }
}

impl fmt::Display for TairaAuthorityRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TairaAuthorityRoleV1 {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|role| role.as_str() == value)
            .ok_or(())
    }
}

/// Immutable public identity for one role service.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TairaAuthorityPublicBindingV1 {
    /// Binding format marker.
    pub magic: [u8; 8],
    /// Binding format version.
    pub version: u16,
    /// Exact isolated role.
    pub role: TairaAuthorityRoleV1,
    /// Reused encrypted-software-signer identity and public key.
    pub signer: SoftwareSignerPublicBindingV1,
}

impl TairaAuthorityPublicBindingV1 {
    /// Validate the complete role/key/identity binding.
    pub fn validate(&self) -> Result<(), ()> {
        let SoftwareSignerPurposeBindingV1::TairaAuthority { role } = &self.signer.purpose_binding
        else {
            return Err(());
        };
        if self.magic != TAIRA_AUTHORITY_BINDING_MAGIC_V1
            || self.version != TAIRA_AUTHORITY_PROTOCOL_VERSION_V1
            || self.signer.role != SoftwareSignerRoleV1::TairaAuthority
            || self.signer.key_algorithm != SoftwareSignerKeyAlgorithmV1::Ed25519
            || role != self.role.as_str()
            || self.signer.validate().is_err()
        {
            return Err(());
        }
        Ok(())
    }

    /// SHA-256 of the canonical installed binding.
    pub fn sha256(&self) -> Result<[u8; 32], ()> {
        self.validate()?;
        let encoded = norito::encode_canonical(self).map_err(|_| ())?;
        Ok(sha256(&encoded))
    }
}

/// Validate the complete installed eight-role registry and reject any reused
/// role, key, handle, identity, or UID.
pub fn validate_taira_authority_registry_v1(
    bindings: &[TairaAuthorityPublicBindingV1],
) -> Result<(), ()> {
    if bindings.len() != TairaAuthorityRoleV1::ALL.len() {
        return Err(());
    }
    let mut roles = BTreeSet::new();
    let mut handles = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut uids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for binding in bindings {
        binding.validate()?;
        if !roles.insert(binding.role)
            || !handles.insert(binding.signer.handle.clone())
            || !identities.insert(binding.signer.service_id.clone())
            || !identities.insert(binding.signer.administrator_id.clone())
            || !uids.insert(binding.signer.service_uid)
            || !uids.insert(binding.signer.client_uid)
            || !uids.insert(binding.signer.administrator_uid)
            || !keys.insert(binding.signer.public_key_digest)
        {
            return Err(());
        }
    }
    if roles != TairaAuthorityRoleV1::ALL.into_iter().collect() {
        return Err(());
    }
    Ok(())
}

/// One role's complete filesystem and public-identity installation record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TairaAuthorityInstallationV1 {
    /// The exact reviewed public role binding.
    pub binding: TairaAuthorityPublicBindingV1,
    /// Isolated persistent state directory.
    pub state_directory: PathBuf,
    /// Client request socket.
    pub request_socket: PathBuf,
    /// Independent administrator socket.
    pub administrator_socket: PathBuf,
}

/// Validate all eight installations and reject every cross-role path or
/// public-identity alias, including observation-signer/replay-broker reuse.
pub fn validate_taira_authority_installations_v1(
    installations: &[TairaAuthorityInstallationV1],
) -> Result<(), ()> {
    validate_taira_authority_registry_v1(
        &installations
            .iter()
            .map(|installation| installation.binding.clone())
            .collect::<Vec<_>>(),
    )?;
    let mut state_directories = BTreeSet::new();
    let mut sockets = BTreeSet::new();
    for installation in installations {
        if !absolute_normal_path(&installation.state_directory)
            || !absolute_normal_path(&installation.request_socket)
            || !absolute_normal_path(&installation.administrator_socket)
            || installation.request_socket == installation.administrator_socket
            || !state_directories.insert(installation.state_directory.clone())
            || !sockets.insert(installation.request_socket.clone())
            || !sockets.insert(installation.administrator_socket.clone())
        {
            return Err(());
        }
    }
    Ok(())
}

fn absolute_normal_path(path: &std::path::Path) -> bool {
    path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
}

/// One path-free artifact identity paired with one ordered SCM_RIGHTS descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TairaAuthorityArtifactManifestEntryV1 {
    /// Contiguous zero-based descriptor ordinal.
    pub ordinal: u16,
    /// Logical basename; never an authority-supplied filesystem path.
    pub name: String,
    /// Exact immutable file length.
    pub size: u64,
    /// SHA-256 of the exact descriptor bytes.
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct AuthorityFrameV1 {
    pub magic: [u8; 8],
    pub version: u16,
    pub kind: u8,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct QualifyRequestV1 {
    pub binding_sha256: [u8; 32],
    pub client_nonce: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct QualifyResponseV1 {
    pub client_nonce: [u8; 32],
    pub server_nonce: [u8; 32],
    pub provenance: SoftwareSignerLiveProvenanceV1,
    pub status_json: Vec<u8>,
    pub response_digest: [u8; 32],
    pub response_attestation: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct AuthorizeRequestV1 {
    pub binding_sha256: [u8; 32],
    pub request_json: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[repr(u8)]
pub(super) enum OperationStatusV1 {
    Ok = 0,
    Replayed = 1,
    Rejected = 2,
    Conflict = 3,
    Unavailable = 4,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct OperationResponseV1 {
    pub status: OperationStatusV1,
    pub result_json: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct VerifyRequestV1 {
    pub binding_sha256: [u8; 32],
    pub request_json: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) enum AuthorityAdminCommandV1 {
    AssignRun {
        assignment_json: Vec<u8>,
    },
    Status,
    Rotate {
        operation_id: [u8; 32],
        expected_audit_head: [u8; 32],
        expected_key_revision: u64,
        new_key_revision: u64,
        new_policy_revision: u64,
        new_policy_digest: [u8; 32],
    },
    Revoke {
        operation_id: [u8; 32],
        expected_audit_head: [u8; 32],
        expected_key_revision: u64,
        reason_digest: [u8; 32],
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct AuthorityAdminRequestV1 {
    pub binding_sha256: [u8; 32],
    pub command: AuthorityAdminCommandV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct RunAssignmentV1 {
    pub role: TairaAuthorityRoleV1,
    pub run_id: [u8; 32],
    pub subject_sha256: [u8; 32],
    pub artifact_manifest_sha256: [u8; 32],
    pub issued_at_unix_millis: u64,
    pub not_before_unix_millis: u64,
    pub expires_at_unix_millis: u64,
    pub key_revision: u64,
    pub policy_revision: u64,
    pub policy_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct StoredRunAssignmentV1 {
    pub assignment: RunAssignmentV1,
    pub assignment_json: Vec<u8>,
    pub signing_payload: Vec<u8>,
    pub receipt: SoftwareSignerSignatureReceiptV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct ReplayConsumptionV1 {
    pub run_id: [u8; 32],
    pub operation_id: [u8; 32],
    pub request_sha256: [u8; 32],
    pub subject_sha256: [u8; 32],
    pub artifact_manifest_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct StoredAuthorizationV1 {
    pub consumption: ReplayConsumptionV1,
    pub admitted_at_unix_millis: u64,
    pub authority_envelope_json: Vec<u8>,
    pub durable_receipt_json: Vec<u8>,
    pub envelope_signing_payload: Vec<u8>,
    pub envelope_receipt: SoftwareSignerSignatureReceiptV1,
    pub receipt_signing_payload: Vec<u8>,
    pub durable_receipt: SoftwareSignerSignatureReceiptV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct StoredDeploymentFinalizationV1 {
    pub operation_id: [u8; 32],
    pub apply_request_sha256: [u8; 32],
    pub finalization_request_sha256: [u8; 32],
    pub outcome: String,
    pub result_sha256: [u8; 32],
    pub finalized_at_unix_millis: u64,
    pub signing_payload: Vec<u8>,
    pub receipt: SoftwareSignerSignatureReceiptV1,
    pub result_json: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct FinalizePrivacyGenesisV1 {
    pub operation_id: [u8; 32],
    pub binding_sha256: [u8; 32],
    pub previous_audit_head: [u8; 32],
    pub signing_payload: Vec<u8>,
    pub receipt: SoftwareSignerSignatureReceiptV1,
}

pub(super) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(super) fn qualify_response_digest(response: &QualifyResponseV1) -> Result<[u8; 32], ()> {
    let body = norito::encode_canonical(&(
        response.client_nonce,
        response.server_nonce,
        response.provenance.clone(),
        response.status_json.clone(),
    ))
    .map_err(|_| ())?;
    let mut digest = Sha256::new();
    digest.update(QUALIFY_RESPONSE_DIGEST_DOMAIN_V1);
    digest.update(u64::try_from(body.len()).map_err(|_| ())?.to_be_bytes());
    digest.update(body);
    Ok(digest.finalize().into())
}

pub(super) fn encode_frame<T: norito::NoritoSerialize>(kind: u8, body: &T) -> Result<Vec<u8>, ()> {
    let body = norito::encode_canonical(body).map_err(|_| ())?;
    if body.len() > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 {
        return Err(());
    }
    norito::encode_canonical(&AuthorityFrameV1 {
        magic: TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1,
        version: TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
        kind,
        body,
    })
    .map_err(|_| ())
}

pub(super) fn decode_frame(bytes: &[u8]) -> Result<AuthorityFrameV1, ()> {
    if bytes.is_empty() || bytes.len() > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 {
        return Err(());
    }
    let frame: AuthorityFrameV1 = norito::decode_canonical(bytes).map_err(|_| ())?;
    if frame.magic != TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1
        || frame.version != TAIRA_AUTHORITY_PROTOCOL_VERSION_V1
        || frame.body.len() > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1
    {
        return Err(());
    }
    Ok(frame)
}

pub(super) fn decode_body<T>(bytes: &[u8]) -> Result<T, ()>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical(bytes).map_err(|_| ())
}
