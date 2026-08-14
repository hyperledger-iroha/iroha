use crate::{
    FencedPrivacyPublicationDispositionV1, FencedPrivacyPublicationReceiptV1,
    FencedPrivacyPublicationRequestV1, FencedTransparencyHeadAncestryProofV1,
    FencedTransparencyPublicationInclusionV1, FencedTransparencyPublishErrorV1,
    FencedTransparencyPublisherV1, FencedTransparencyTargetHeadV1, GovernancePublishError,
    GovernancePublisher, GovernanceSubmissionProvenanceV1, PdpGovernanceArchiveV1,
    PdpRejectionReasonV1, PdpTerminalDecisionV1, PrivacyPublicationAuthorizationV1,
    governance_rooted_fs,
};
use axum::http::{Request, Version, header, request::Parts};
use ed25519_dalek::VerifyingKey as DalekVerifyingKey;
use hex::ToHex;
use iroha_config::parameters::{ProductionRuntimeHandleError, validate_production_runtime_handle};
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
#[cfg(test)]
use iroha_data_model::account::AccountId;
use norito::json::{self, Map as JsonMap, Value as JsonValue};
use norito::{
    core::DecodeLimits,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_car::{CarBuildPlan, CarWriter, FileEntry};
use sorafs_manifest::{
    GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1, GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1,
    GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1, GOVERNANCE_DAG_BLOCK_VERSION_V1,
    GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1, GOVERNANCE_DAG_HEAD_VERSION_V1,
    GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1, GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1,
    GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1, GOVERNANCE_LOG_VERSION_V1,
    GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1, GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1,
    GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1,
    GOVERNANCE_PUBLICATION_LABEL_TOTAL_MAX_BYTES_V1, GovernanceDagBlockV1, GovernanceDagHeadV1,
    GovernanceDagSubmissionProvenanceV1, GovernanceExternalPayloadV1, GovernanceLogNodeV1,
    GovernanceLogPayloadV1, GovernanceLogSignatureV1, GovernanceSignatureAlgorithm,
    MAX_REPUTATION_TRUST_EDGES, ModerationLedgerCyclePublicationV1,
    PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1, SignedReputationSnapshotV1,
    SoraFsAppealFinanceReportV1, SoraFsAppealFinanceSettlementReceiptV1,
    SoraFsAppealFinanceWeeklyRollupV1, SoraFsModerationBallotGovernanceEventV1,
    SorafsReconciliationReportV1,
    deal::{DealSettlementStatusV1, DealSettlementV1},
    governance_dag_block_cid_v1, governance_publication_source_pair_id_v1,
    por::{PorChallengePublicationV1, PorWeeklyReportV1},
    repair::GcAuditEventV1,
    validate_governance_dag_head_against_rotatable_chain_v1,
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
struct GovernanceCarBuffer {
    bytes: Vec<u8>,
}
impl GovernanceCarBuffer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
impl Write for GovernanceCarBuffer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("governance CAR archive length overflowed"))?;
        if next_len > GOVERNANCE_CAR_ARCHIVE_MAX_BYTES {
            return Err(io::Error::other(format!(
                "governance CAR archive exceeds {GOVERNANCE_CAR_ARCHIVE_MAX_BYTES} bytes"
            )));
        }
        self.bytes.try_reserve(buffer.len()).map_err(|_| {
            io::Error::other("failed to reserve bounded governance CAR archive bytes")
        })?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}
pub(crate) const GOVERNANCE_DAG_SINK_FILESYSTEM: &str = "filesystem";
const GOVERNANCE_PUBLICATION_STATE_FILE: &str = "governance-publication-state-v1.json";
const GOVERNANCE_PUBLICATION_INITIALIZED_FILE: &str = ".governance-publication-initialized-v1";
const GOVERNANCE_PUBLICATION_INITIALIZED_BODY: &[u8] =
    b"sorafs.governance_dag.publication_initialized.v1\n";
const GOVERNANCE_PUBLICATION_STATE_SCHEMA: &str =
    "sorafs.governance_dag.local_publication_state.v1";
const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
// Public index metadata is root-relative; the retained descriptor is the filesystem authority.
pub(crate) const GOVERNANCE_DAG_LOGICAL_ROOT: &str = ".";
const GOVERNANCE_CAR_QUEUE_FILE: &str = "car-queue.json";
const GOVERNANCE_CAR_QUEUE_SCHEMA: &str = "sorafs.governance_dag.local_car_queue.v1";
const GOVERNANCE_CAR_SEGMENT_SCHEMA: &str = "sorafs.governance_dag.local_car_segment.v1";
const GOVERNANCE_CAR_PLAN_SCHEMA: &str = "sorafs.governance_dag.local_car_plan.v1";
const GOVERNANCE_PUBLICATION_SOURCES_DIR: &str = "publication-sources";
const GOVERNANCE_CAR_SEGMENTS_DIR: &str = "car-segments";
const GOVERNANCE_RUNTIME_DAG_INDEX_FILE: &str = "runtime-dag-index.json";
pub(crate) const GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA: &str =
    "sorafs.governance_dag.runtime_signed_index.v1";
pub(crate) const GOVERNANCE_RUNTIME_DAG_INDEX_FIELDS_V1: &[&str] = &[
    "schema",
    "source",
    "root",
    "generated_at",
    "signer_handle",
    "publisher_peer_id",
    "publisher_peer_id_hex",
    "publisher_public_key_hex",
    "signer_revision",
    "signer_policy_digest_hex",
    "checkpoint_store_handle",
    "checkpoint_store_revision",
    "checkpoint_store_policy_digest_hex",
    "head_block_cid_hex",
    "head_generated_at",
    "block_count",
    "by_encoded_blake3",
    "by_source_payload_blake3",
    "by_payload_kind",
    "blocks",
];
pub(crate) const GOVERNANCE_RUNTIME_DAG_INDEX_BLOCK_FIELDS_V1: &[&str] = &[
    "position",
    "sequence",
    "payload_kind",
    "encoded_blake3",
    "encoded_len",
    "source_payload_blake3",
    "source_payload_len",
    "submission_publisher_account_digest_hex",
    "submission_origin",
    "encoded_path",
    "json_path",
    "node_cid_hex",
    "prev_node_cid_hex",
    "block_cid_hex",
    "prev_block_cid_hex",
    "block_path",
    "published_at_unix",
];
pub(crate) const GOVERNANCE_RUNTIME_DAG_DIR: &str = "runtime-dag";
pub(crate) const GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR: &str = "blocks";
const GOVERNANCE_RUNTIME_DAG_HEAD_FILE: &str = "head.to";
const GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR: &str = ".runtime-dag-producer-transaction-v1";
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_FILE: &str =
    "runtime-dag-qualification-history.to";
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR: &str =
    "runtime-dag-qualification-archives";
const GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_FILE: &str = "fenced-privacy-head.to";
const GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_FILE: &str = "fenced-privacy-head-sync.to";
const GOVERNANCE_FENCED_PRIVACY_PENDING_FILE: &str = "fenced-privacy-pending.to";
const GOVERNANCE_PUBLICATION_STORE_DIR_V1: &str = "governance-publication-authority-v1";
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_DIR_V1: &str =
    "governance-runtime-qualification-v1";
const GOVERNANCE_RUNTIME_DAG_STAGING_STORE_DIR_V1: &str = "governance-runtime-staging-v1";
const GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_DIR_V1: &str = "governance-runtime-committed-v1";
const GOVERNANCE_FENCED_PRIVACY_STORE_DIR_V1: &str = "governance-fenced-privacy-v1";
const GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_PENDING_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_STATE_VERSION_V1: u8 = 1;
const GOVERNANCE_PUBLISHER_LOCK_FILE: &str = ".governance-publisher.lock";
pub(crate) const GOVERNANCE_MUTABLE_INDEX_MAX_BYTES: usize = 64 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_HEAD_MAX_BYTES_V1: usize = 64 * 1024;
const GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_MAX_BYTES_V1: usize = 65 * 1024 * 1024;
const GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES: usize =
    GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1;
const GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES: usize = 64 * 1024 * 1024;
const GOVERNANCE_DIGEST_SIDECAR_BYTES: usize = 65;
const GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES: usize = GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES
    + GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES
    + 2 * GOVERNANCE_DIGEST_SIDECAR_BYTES;
const GOVERNANCE_CAR_ARCHIVE_MAX_BYTES: usize = 160 * 1024 * 1024;
const GOVERNANCE_PUBLICATION_STATE_MAX_BYTES: usize = 160 * 1024 * 1024;
const GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP: usize = 131_072;
const GOVERNANCE_PUBLICATION_INTERRUPTED_IDENTITY_ALLOWANCE: usize = 1;
const GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT: usize = 4;
const GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT: usize = 6;
const GOVERNANCE_PUBLICATION_ATOMIC_TEMP_ALLOWANCE: usize = 1;
const GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR: &str =
    ".governance-publication-recovery-quarantine-v1";
const GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP: usize = 16;
const GOVERNANCE_PUBLICATION_SOURCE_WRITE_ORDER: [&str; 4] = [
    "payload.to",
    "payload.to.blake3",
    "payload.json",
    "payload.json.blake3",
];
const GOVERNANCE_PUBLICATION_CAR_WRITE_ORDER: [&str; 6] = [
    ".car",
    ".car.blake3",
    ".plan.json",
    ".plan.json.blake3",
    ".json",
    ".json.blake3",
];
const GOVERNANCE_RELATIVE_PATH_MAX_BYTES: usize = 4_096;
const GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS: usize = 64;
const GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES: usize = 255;
const GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES: usize = GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1;
const GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES: usize =
    GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1;
pub(crate) const GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1: usize = 131_072;
const GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1: u64 = 1024 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_MAX_FUTURE_SKEW_SECS_V1: u64 = 60;
// Nested qualification histories need 20x variable headroom; small composite
// records need a 2 KiB floor for their 1,696 bytes of fixed decode overhead.
const GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1: usize = 2 * 1024;
const GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1: usize = 20;
const GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1: usize = 512 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_DECODE_MAX_TOTAL_ELEMENTS_V1: usize = 4_000_000;
pub(crate) const GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_STAGING_STATE_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TRANSITION_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1: usize = 64;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_TRANSITIONS_V1: usize = 64;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1: u64 = 64;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TOTAL_MAX_V1: u64 =
    GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1
        * GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_TRANSITIONS_V1 as u64
        + GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1 as u64;
const GOVERNANCE_DAG_SEALED_STATE_MAX_BYTES_V1: usize = 192 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_SEALED_MAX_BYTES_V1: usize = 64 * 1024;
const GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_SEALED_MAX_BYTES_V1: usize = 64 * 1024;
const GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_SEALED_MAX_BYTES_V1: usize = 256 * 1024;
#[derive(Debug, Clone, Copy)]
struct GovernanceTwoSlotStoreSpecV1 {
    directory_name: &'static str,
    semantic_domain: &'static [u8],
    stable_nonce: &'static [u8],
    max_payload_bytes: usize,
}
const GOVERNANCE_PUBLICATION_STORE_SPEC_V1: GovernanceTwoSlotStoreSpecV1 =
    GovernanceTwoSlotStoreSpecV1 {
        directory_name: GOVERNANCE_PUBLICATION_STORE_DIR_V1,
        semantic_domain: b"sorafs.governance.publication-authority.v1",
        stable_nonce: b"sorafs.governance.publication-authority.local-store.v1",
        max_payload_bytes: GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
    };
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_SPEC_V1: GovernanceTwoSlotStoreSpecV1 =
    GovernanceTwoSlotStoreSpecV1 {
        directory_name: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_DIR_V1,
        semantic_domain: b"sorafs.governance.runtime-dag.qualification-state.v1",
        stable_nonce: b"sorafs.governance.runtime-dag.qualification.local-store.v1",
        max_payload_bytes: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_MAX_BYTES_V1,
    };
const GOVERNANCE_RUNTIME_DAG_STAGING_STORE_SPEC_V1: GovernanceTwoSlotStoreSpecV1 =
    GovernanceTwoSlotStoreSpecV1 {
        directory_name: GOVERNANCE_RUNTIME_DAG_STAGING_STORE_DIR_V1,
        semantic_domain: b"sorafs.governance.runtime-dag.staging-transaction.v1",
        stable_nonce: b"sorafs.governance.runtime-dag.staging.local-store.v1",
        max_payload_bytes: governance_rooted_fs::TWO_SLOT_MAX_PAYLOAD_BYTES_V1,
    };
const GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_SPEC_V1: GovernanceTwoSlotStoreSpecV1 =
    GovernanceTwoSlotStoreSpecV1 {
        directory_name: GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_DIR_V1,
        semantic_domain: b"sorafs.governance.runtime-dag.committed-state.v1",
        stable_nonce: b"sorafs.governance.runtime-dag.committed.local-store.v1",
        max_payload_bytes: GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_MAX_BYTES_V1,
    };
const GOVERNANCE_FENCED_PRIVACY_STORE_SPEC_V1: GovernanceTwoSlotStoreSpecV1 =
    GovernanceTwoSlotStoreSpecV1 {
        directory_name: GOVERNANCE_FENCED_PRIVACY_STORE_DIR_V1,
        semantic_domain: b"sorafs.governance.fenced-privacy.state.v1",
        stable_nonce: b"sorafs.governance.fenced-privacy.local-store.v1",
        max_payload_bytes: 16 * 1024,
    };
/// Public, non-secret qualification returned by a Governance DAG runtime provider.
///
/// `revision` identifies the deployment-owned adapter/policy revision and
/// `policy_digest` binds the exact public provider policy. Runtime wrappers pin
/// this observation at startup and require it to remain identical on every
/// subsequent operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceDagRuntimeProviderQualificationV1 {
    /// Non-zero deployment policy revision.
    pub revision: u64,
    /// Non-zero digest of the public provider policy.
    pub policy_digest: [u8; 32],
}
impl GovernanceDagRuntimeProviderQualificationV1 {
    /// Construct one public provider qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }
    /// Whether both first-release qualification fields are non-zero.
    pub(crate) fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest.iter().any(|byte| *byte != 0)
    }
}
include!("governance/signing_purpose.rs");
/// Authenticated Governance DAG endpoint class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GovernanceDagAuthenticationScope {
    /// Kubo/IPFS control-plane request.
    Ipfs,
    /// Signed-head compare-and-swap request.
    SignedHead,
}
impl GovernanceDagAuthenticationScope {
    /// Canonical lowercase wire label for the endpoint class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ipfs => "ipfs",
            Self::SignedHead => "signed-head",
        }
    }
    const fn signing_tag(self) -> u8 {
        match self {
            Self::Ipfs => 1,
            Self::SignedHead => 2,
        }
    }
}
const GOVERNANCE_DAG_REQUEST_INGRESS_ENDPOINT_DOMAIN_V1: &[u8] =
    b"sorafs.governance-dag.request-ingress-endpoint.v1\0";
const GOVERNANCE_DAG_REQUEST_INGRESS_BINDING_DOMAIN_V1: &[u8] =
    b"sorafs.governance-dag.request-ingress-binding.v1\0";
/// Receiver posture required from every first-release Governance DAG endpoint.
///
/// There is deliberately no permissive or signer-only variant. A provider can
/// qualify only an endpoint whose backend is reachable exclusively through the
/// authenticated V1 receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceDagRequestIngressEnforcementV1 {
    /// The authenticated receiver is the backend's exclusive ingress.
    ExclusiveAuthenticatedReceiver = 1,
}
impl GovernanceDagRequestIngressEnforcementV1 {
    /// Stable first-release wire identifier.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        self as u8
    }
}
/// Replay posture required from every first-release Governance DAG receiver.
///
/// The store must implement one atomic nonce consume shared by every ingress
/// replica, seal committed evidence durably, and retain it until the signed
/// envelope expires. Process-local memory is not a qualifying implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceDagRequestReplayPostureV1 {
    /// Shared, sealed, atomic nonce consumption retained through expiry.
    SharedSealedAtomicConsumeUntilExpiry = 1,
}
impl GovernanceDagRequestReplayPostureV1 {
    /// Stable first-release wire identifier.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        self as u8
    }
}
/// Stable validation failure for a live Governance DAG ingress qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagRequestIngressQualificationErrorV1 {
    /// The canonical endpoint URL or endpoint digest is invalid.
    InvalidEndpointBinding,
    /// The runtime provider revision or public-policy digest is invalid.
    InvalidProviderQualification,
    /// The request-auth public key or timing policy is invalid.
    InvalidAuthenticationPolicy,
    /// The admitted request-body ceiling is zero.
    InvalidRequestBodyLimit,
    /// The receiver's public policy identity is zero.
    InvalidReceiverPolicy,
    /// The shared sealed replay namespace identity is zero.
    InvalidReplayNamespace,
    /// The complete ingress replica-set identity is zero.
    InvalidReplicaSet,
}
impl fmt::Display for GovernanceDagRequestIngressQualificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEndpointBinding => {
                "Governance DAG request-ingress endpoint binding is invalid"
            }
            Self::InvalidProviderQualification => {
                "Governance DAG request-ingress provider qualification is invalid"
            }
            Self::InvalidAuthenticationPolicy => {
                "Governance DAG request-ingress authentication policy is invalid"
            }
            Self::InvalidRequestBodyLimit => "Governance DAG request-ingress body limit is invalid",
            Self::InvalidReceiverPolicy => {
                "Governance DAG request-ingress receiver policy is invalid"
            }
            Self::InvalidReplayNamespace => {
                "Governance DAG request-ingress replay namespace is invalid"
            }
            Self::InvalidReplicaSet => "Governance DAG request-ingress replica set is invalid",
        })
    }
}
impl std::error::Error for GovernanceDagRequestIngressQualificationErrorV1 {}
/// Compute the exact public binding for one configured request-ingress endpoint.
///
/// IPFS endpoints bind their normalized base URL with exactly one trailing
/// slash. Signed-head endpoints bind the exact normalized URL. Credentials,
/// query strings, fragments, percent-escaped paths, non-HTTP schemes, and
/// hostless URLs are rejected.
/// The digest is domain-separated by endpoint scope.
///
/// # Errors
///
/// Returns a stable error when `endpoint` cannot name a canonical public
/// Governance DAG endpoint.
pub fn governance_dag_request_ingress_endpoint_binding_v1(
    scope: GovernanceDagAuthenticationScope,
    endpoint: &str,
) -> Result<[u8; 32], GovernanceDagRequestIngressQualificationErrorV1> {
    let url = canonical_governance_dag_request_ingress_endpoint_url_v1(scope, endpoint)?;
    let canonical = url.as_str().as_bytes();
    let canonical_len = u32::try_from(canonical.len())
        .map_err(|_| GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(GOVERNANCE_DAG_REQUEST_INGRESS_ENDPOINT_DOMAIN_V1);
    hasher.update(&[GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1]);
    hasher.update(&[scope.signing_tag()]);
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(canonical);
    Ok(*hasher.finalize().as_bytes())
}
fn canonical_governance_dag_request_ingress_endpoint_url_v1(
    scope: GovernanceDagAuthenticationScope,
    endpoint: &str,
) -> Result<Url, GovernanceDagRequestIngressQualificationErrorV1> {
    if endpoint.is_empty()
        || endpoint.trim() != endpoint
        || endpoint.contains('\\')
        || endpoint.chars().any(char::is_control)
    {
        return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding);
    }
    let mut url = Url::parse(endpoint)
        .map_err(|_| GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
        || url.port_or_known_default().is_none()
        || url.path().contains('%')
    {
        return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding);
    }
    if scope == GovernanceDagAuthenticationScope::Ipfs {
        let path = url.path().trim_end_matches('/');
        let normalized_path = if path.is_empty() {
            "/".to_owned()
        } else {
            format!("{path}/")
        };
        url.set_path(&normalized_path);
    }
    Ok(url)
}
/// Exact public request policy expected from one qualified ingress provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceDagRequestIngressBindingV1 {
    scope: GovernanceDagAuthenticationScope,
    endpoint_binding: [u8; 32],
    public_key: [u8; 32],
    max_body_bytes: u64,
    max_envelope_lifetime_secs: u64,
    max_future_skew_secs: u64,
}
impl GovernanceDagRequestIngressBindingV1 {
    /// Validate and construct one exact ingress binding.
    ///
    /// # Errors
    ///
    /// Rejects a zero endpoint binding or body limit, a malformed Ed25519 key,
    /// and request-auth timing outside the first-release bounds.
    pub fn try_new(
        scope: GovernanceDagAuthenticationScope,
        endpoint_binding: [u8; 32],
        public_key: [u8; 32],
        max_body_bytes: u64,
        max_envelope_lifetime_secs: u64,
        max_future_skew_secs: u64,
    ) -> Result<Self, GovernanceDagRequestIngressQualificationErrorV1> {
        if endpoint_binding == [0; 32] {
            return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding);
        }
        if max_body_bytes == 0 {
            return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidRequestBodyLimit);
        }
        GovernanceDagRequestAuthenticationPolicyV1::try_new(
            public_key,
            max_envelope_lifetime_secs,
            max_future_skew_secs,
        )
        .map_err(|_| {
            GovernanceDagRequestIngressQualificationErrorV1::InvalidAuthenticationPolicy
        })?;
        Ok(Self {
            scope,
            endpoint_binding,
            public_key,
            max_body_bytes,
            max_envelope_lifetime_secs,
            max_future_skew_secs,
        })
    }
    /// Endpoint class bound by this policy.
    #[must_use]
    pub const fn scope(self) -> GovernanceDagAuthenticationScope {
        self.scope
    }
    /// Domain-separated digest of the exact normalized endpoint.
    #[must_use]
    pub const fn endpoint_binding(self) -> [u8; 32] {
        self.endpoint_binding
    }
    /// Raw canonical Ed25519 request-auth key.
    #[must_use]
    pub const fn public_key(self) -> [u8; 32] {
        self.public_key
    }
    /// Maximum complete request body admitted by the receiver.
    #[must_use]
    pub const fn max_body_bytes(self) -> u64 {
        self.max_body_bytes
    }
    /// Maximum signed-envelope lifetime in seconds.
    #[must_use]
    pub const fn max_envelope_lifetime_secs(self) -> u64 {
        self.max_envelope_lifetime_secs
    }
    /// Maximum accepted future issuance skew in seconds.
    #[must_use]
    pub const fn max_future_skew_secs(self) -> u64 {
        self.max_future_skew_secs
    }
    /// Domain-separated digest of the complete endpoint, key, body, and timing policy.
    ///
    /// Rollout evidence uses this identity to bind deployment approval to the
    /// exact policy qualified by the runtime provider rather than to a
    /// collection of unanchored boolean claims.
    #[must_use]
    pub fn binding_digest(self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(GOVERNANCE_DAG_REQUEST_INGRESS_BINDING_DOMAIN_V1);
        hasher.update(&[GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1]);
        hasher.update(&[self.scope.signing_tag()]);
        hasher.update(&self.endpoint_binding);
        hasher.update(&self.public_key);
        hasher.update(&self.max_body_bytes.to_be_bytes());
        hasher.update(&self.max_envelope_lifetime_secs.to_be_bytes());
        hasher.update(&self.max_future_skew_secs.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}
/// Live provider proof that an exact endpoint enforces receiver authentication
/// and shared sealed replay consumption.
///
/// Construction has no signer-only or process-local posture. Providers must
/// return this value only after actively checking that the exact endpoint is
/// exclusively receiver-fronted and that every ingress replica atomically
/// consumes the same sealed replay namespace through envelope expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceDagRequestIngressQualificationV1 {
    provider: GovernanceDagRuntimeProviderQualificationV1,
    binding: GovernanceDagRequestIngressBindingV1,
    receiver_policy_digest: [u8; 32],
    replay_namespace_digest: [u8; 32],
    replica_set_digest: [u8; 32],
    enforcement: GovernanceDagRequestIngressEnforcementV1,
    replay_posture: GovernanceDagRequestReplayPostureV1,
}
impl GovernanceDagRequestIngressQualificationV1 {
    /// Validate and construct one live first-release ingress qualification.
    ///
    /// # Errors
    ///
    /// Rejects zero runtime, receiver-policy, replay-namespace, or replica-set
    /// identities. The only constructible posture is exclusive V1 receiver
    /// enforcement with shared sealed atomic replay consumption.
    pub fn try_new(
        provider: GovernanceDagRuntimeProviderQualificationV1,
        binding: GovernanceDagRequestIngressBindingV1,
        receiver_policy_digest: [u8; 32],
        replay_namespace_digest: [u8; 32],
        replica_set_digest: [u8; 32],
    ) -> Result<Self, GovernanceDagRequestIngressQualificationErrorV1> {
        if !provider.is_valid() {
            return Err(
                GovernanceDagRequestIngressQualificationErrorV1::InvalidProviderQualification,
            );
        }
        if receiver_policy_digest == [0; 32] {
            return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidReceiverPolicy);
        }
        if replay_namespace_digest == [0; 32] {
            return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidReplayNamespace);
        }
        if replica_set_digest == [0; 32] {
            return Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidReplicaSet);
        }
        Ok(Self {
            provider,
            binding,
            receiver_policy_digest,
            replay_namespace_digest,
            replica_set_digest,
            enforcement: GovernanceDagRequestIngressEnforcementV1::ExclusiveAuthenticatedReceiver,
            replay_posture:
                GovernanceDagRequestReplayPostureV1::SharedSealedAtomicConsumeUntilExpiry,
        })
    }
    /// Runtime adapter revision and public-policy identity.
    #[must_use]
    pub const fn provider(self) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.provider
    }
    /// Exact endpoint, key, body, and timing binding.
    #[must_use]
    pub const fn binding(self) -> GovernanceDagRequestIngressBindingV1 {
        self.binding
    }
    /// Public identity of the installed receiver policy.
    #[must_use]
    pub const fn receiver_policy_digest(self) -> [u8; 32] {
        self.receiver_policy_digest
    }
    /// Stable identity of the shared sealed replay namespace.
    #[must_use]
    pub const fn replay_namespace_digest(self) -> [u8; 32] {
        self.replay_namespace_digest
    }
    /// Public digest of the complete ingress replica set sharing that namespace.
    #[must_use]
    pub const fn replica_set_digest(self) -> [u8; 32] {
        self.replica_set_digest
    }
    /// Required exclusive receiver posture.
    #[must_use]
    pub const fn enforcement(self) -> GovernanceDagRequestIngressEnforcementV1 {
        self.enforcement
    }
    /// Required shared sealed replay posture.
    #[must_use]
    pub const fn replay_posture(self) -> GovernanceDagRequestReplayPostureV1 {
        self.replay_posture
    }
}
/// Version of the public Governance DAG request-authentication envelope.
pub const GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1: u8 = 1;
/// Maximum canonical absolute URL bytes authenticated by the V1 envelope.
pub const GOVERNANCE_DAG_REQUEST_AUTH_MAX_URL_BYTES_V1: usize = 4 * 1024;
/// Maximum number of selected public request headers authenticated by V1.
pub const GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1: usize = 8;
/// Maximum bytes in one selected public request-header value.
pub const GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_VALUE_BYTES_V1: usize = 1024;
/// Maximum aggregate bytes in selected public request-header names and values.
pub const GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_BYTES_V1: usize = 4 * 1024;
/// Maximum live nonce entries retained by one V1 request-auth replay cache.
pub const GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1: usize = 4_096;
/// Exact lowercase HTTP header names carrying one V1 authentication envelope.
pub const GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1: [&str; 8] = [
    "x-sorafs-governance-auth-version",
    "x-sorafs-governance-auth-scope",
    "x-sorafs-governance-auth-issued-at",
    "x-sorafs-governance-auth-expires-at",
    "x-sorafs-governance-auth-nonce",
    "x-sorafs-governance-auth-request-digest",
    "x-sorafs-governance-auth-public-key",
    "x-sorafs-governance-auth-signature",
];
/// Ordered lowercase HTTP header names selected into the V1 request digest.
pub const GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1: [&str; 6] = [
    "accept",
    "accept-encoding",
    "content-type",
    "if-match",
    "if-none-match",
    "user-agent",
];
const GOVERNANCE_DAG_REQUEST_AUTH_DOMAIN_V1: &[u8] =
    b"sorafs.governance-dag.http-request-auth.v1\0";
const GOVERNANCE_DAG_REQUEST_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.governance-dag.http-request-digest.v1\0";
const GOVERNANCE_DAG_REQUEST_AUTH_HEADER_PREFIX_V1: &str = "x-sorafs-governance-auth-";
/// Hard V1 ceiling for one signed request-authentication envelope lifetime.
pub(crate) const GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1: u64 = 300;
/// Hard V1 ceiling for future issuance skew in request authentication.
pub(crate) const GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1: u64 = 60;
/// One canonical, explicitly public HTTP header selected for authentication.
///
/// The V1 allow-list intentionally excludes authorization, cookies, proxy
/// credentials, API keys, forwarding metadata, and arbitrary extension
/// headers. Values are retained byte-for-byte as bounded visible ASCII.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GovernanceDagCanonicalRequestHeaderV1 {
    name: String,
    value: String,
}
impl GovernanceDagCanonicalRequestHeaderV1 {
    /// Construct one V1 selected public header.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free reason when the name is not in the fixed
    /// V1 allow-list or the value is empty, noncanonical, or oversized.
    pub fn try_new(name: &str, value: &str) -> Result<Self, &'static str> {
        if name.is_empty()
            || name.len() > 64
            || name.bytes().any(|byte| {
                !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !matches!(byte, b'-')
            })
            || GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1
                .binary_search(&name)
                .is_err()
        {
            return Err("Governance DAG request header name is not canonical");
        }
        if value.is_empty()
            || value.len() > GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_VALUE_BYTES_V1
            || value.trim() != value
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err("Governance DAG request header value is not canonical");
        }
        Ok(Self {
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }
    /// Canonical lowercase header name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Exact visible-ASCII header value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}
/// Bounded canonical descriptor of one complete Governance DAG HTTP request.
///
/// The descriptor contains public routing metadata and a body commitment only.
/// It cannot carry credentials, private keys, cookies, streaming bodies, or
/// process-local request authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagCanonicalRequestV1 {
    scope: GovernanceDagAuthenticationScope,
    method: String,
    canonical_url: String,
    selected_headers: Vec<GovernanceDagCanonicalRequestHeaderV1>,
    body_length: u64,
    body_blake3: [u8; 32],
    request_digest: [u8; 32],
}
impl GovernanceDagCanonicalRequestV1 {
    /// Construct a descriptor from exact bounded HTTP request parts.
    ///
    /// Callers must supply every present V1-selected public header exactly
    /// once. Names must already be lowercase and values must be visible ASCII.
    /// The constructor sorts them, rejects duplicates, and commits the exact
    /// body bytes without retaining them.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free reason when any request part is
    /// noncanonical or exceeds a V1 bound.
    pub fn try_from_http_parts<'a>(
        scope: GovernanceDagAuthenticationScope,
        method: &str,
        canonical_url: &str,
        selected_headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
        body: &[u8],
        max_body_bytes: u64,
    ) -> Result<Self, &'static str> {
        let body_length = u64::try_from(body.len())
            .map_err(|_| "Governance DAG request body length exceeds u64")?;
        if max_body_bytes == 0 || body_length > max_body_bytes {
            return Err(
                "Governance DAG request body commitment is noncanonical or exceeds the configured bound",
            );
        }
        let mut canonical_headers = selected_headers
            .into_iter()
            .map(|(name, value)| {
                let value = std::str::from_utf8(value)
                    .map_err(|_| "Governance DAG request header value is not canonical")?;
                GovernanceDagCanonicalRequestHeaderV1::try_new(name, value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        canonical_headers.sort_unstable();
        Self::try_new(
            scope,
            method,
            canonical_url,
            canonical_headers,
            body_length,
            *blake3::hash(body).as_bytes(),
            max_body_bytes,
        )
    }
    /// Construct a fully validated V1 descriptor and its request digest.
    ///
    /// `max_body_bytes` is the already validated deployment request bound.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free reason for malformed, noncanonical, or
    /// oversized public fields.
    pub fn try_new(
        scope: GovernanceDagAuthenticationScope,
        method: &str,
        canonical_url: &str,
        selected_headers: Vec<GovernanceDagCanonicalRequestHeaderV1>,
        body_length: u64,
        body_blake3: [u8; 32],
        max_body_bytes: u64,
    ) -> Result<Self, &'static str> {
        if !matches!(method, "GET" | "POST" | "PUT") {
            return Err("Governance DAG request method is not canonical");
        }
        if canonical_url.is_empty()
            || canonical_url.len() > GOVERNANCE_DAG_REQUEST_AUTH_MAX_URL_BYTES_V1
            || canonical_url.trim() != canonical_url
            || canonical_url.chars().any(char::is_control)
            || canonical_url.contains('\\')
            || !governance_request_auth_url_is_canonical(canonical_url)
        {
            return Err("Governance DAG request URL is not canonical");
        }
        if selected_headers.len() > GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1 {
            return Err("Governance DAG request has too many selected headers");
        }
        let mut previous_name = None;
        let mut header_bytes = 0_usize;
        for header in &selected_headers {
            if previous_name.is_some_and(|previous| previous >= header.name()) {
                return Err("Governance DAG request headers are not canonical");
            }
            previous_name = Some(header.name());
            header_bytes = header_bytes
                .checked_add(header.name().len())
                .and_then(|total| total.checked_add(header.value().len()))
                .ok_or("Governance DAG request header bytes overflow")?;
        }
        if header_bytes > GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_BYTES_V1 {
            return Err("Governance DAG request headers exceed the V1 bound");
        }
        if max_body_bytes == 0
            || body_length > max_body_bytes
            || body_blake3 == [0; 32]
            || (body_length == 0 && body_blake3 != *blake3::hash(b"").as_bytes())
        {
            return Err(
                "Governance DAG request body commitment is noncanonical or exceeds the configured bound",
            );
        }
        let mut descriptor = Self {
            scope,
            method: method.to_owned(),
            canonical_url: canonical_url.to_owned(),
            selected_headers,
            body_length,
            body_blake3,
            request_digest: [0; 32],
        };
        descriptor.request_digest = descriptor.compute_request_digest();
        Ok(descriptor)
    }
    /// Authenticated endpoint class.
    #[must_use]
    pub const fn scope(&self) -> GovernanceDagAuthenticationScope {
        self.scope
    }
    /// Exact canonical HTTP method.
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }
    /// Exact canonical absolute URL, including its canonical query.
    #[must_use]
    pub fn canonical_url(&self) -> &str {
        &self.canonical_url
    }
    /// Ordered selected public headers.
    #[must_use]
    pub fn selected_headers(&self) -> &[GovernanceDagCanonicalRequestHeaderV1] {
        &self.selected_headers
    }
    /// Exact request-body length in bytes.
    #[must_use]
    pub const fn body_length(&self) -> u64 {
        self.body_length
    }
    /// BLAKE3 commitment to the exact request-body bytes, including empty bodies.
    #[must_use]
    pub const fn body_blake3(&self) -> [u8; 32] {
        self.body_blake3
    }
    /// Domain-separated digest of every canonical descriptor field.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
    fn append_canonical_bytes(&self, bytes: &mut Vec<u8>) {
        bytes.push(GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1);
        bytes.push(self.scope.signing_tag());
        append_governance_request_auth_field(bytes, self.method.as_bytes());
        append_governance_request_auth_field(bytes, self.canonical_url.as_bytes());
        bytes.extend_from_slice(&(self.selected_headers.len() as u16).to_be_bytes());
        for header in &self.selected_headers {
            append_governance_request_auth_field(bytes, header.name.as_bytes());
            append_governance_request_auth_field(bytes, header.value.as_bytes());
        }
        bytes.extend_from_slice(&self.body_length.to_be_bytes());
        bytes.extend_from_slice(&self.body_blake3);
    }
    fn compute_request_digest(&self) -> [u8; 32] {
        let mut canonical = Vec::with_capacity(
            GOVERNANCE_DAG_REQUEST_DIGEST_DOMAIN_V1.len()
                + self.method.len()
                + self.canonical_url.len()
                + GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_BYTES_V1
                + 128,
        );
        canonical.extend_from_slice(GOVERNANCE_DAG_REQUEST_DIGEST_DOMAIN_V1);
        self.append_canonical_bytes(&mut canonical);
        *blake3::hash(&canonical).as_bytes()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GovernanceDagAuthenticationHeaderDispositionV1 {
    Reject,
    Retain,
}
/// Build one canonical outbound descriptor from a complete HTTP request.
///
/// Only the fixed V1 selected-public-header set is committed. Ordinary public
/// transport headers are deliberately excluded, while credential headers and
/// every Governance DAG authentication-prefix header are rejected. A
/// canonical `content-length`, when present, must occur exactly once and match
/// the complete byte body; `transfer-encoding` is never accepted because this
/// contract authenticates a finalized in-memory body rather than HTTP framing.
///
/// # Errors
///
/// Returns a stable, payload-free rejection for forbidden headers, ambiguous
/// framing, or any noncanonical request field.
pub fn canonicalize_governance_dag_outbound_http_request_v1<'a>(
    scope: GovernanceDagAuthenticationScope,
    method: &str,
    canonical_url: &str,
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    body: &[u8],
    max_body_bytes: u64,
) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagRequestAuthenticationErrorV1> {
    let (selected_headers, authentication_headers) = partition_governance_dag_http_headers_v1(
        headers,
        body,
        GovernanceDagAuthenticationHeaderDispositionV1::Reject,
    )?;
    debug_assert!(authentication_headers.is_empty());
    GovernanceDagCanonicalRequestV1::try_from_http_parts(
        scope,
        method,
        canonical_url,
        selected_headers,
        body,
        max_body_bytes,
    )
    .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)
}
/// A complete HTTP request authorized for Governance DAG backend dispatch.
///
/// Construction is restricted to [`GovernanceDagHttpRequestReceiverV1`]. The
/// receiver consumes the original typed HTTP request, verifies its transport
/// authority, signature, and replay state, and removes the public
/// authentication-envelope headers. Its returned URI is the canonical
/// origin-form path and query, so a backend cannot reinterpret a matching
/// absolute-form authority.
#[derive(Debug)]
pub struct GovernanceDagVerifiedHttpRequestV1<B> {
    request: Request<B>,
    descriptor: GovernanceDagCanonicalRequestV1,
}
impl<B> GovernanceDagVerifiedHttpRequestV1<B> {
    /// Exact canonical descriptor authenticated for this request.
    #[must_use]
    pub const fn descriptor(&self) -> &GovernanceDagCanonicalRequestV1 {
        &self.descriptor
    }
    /// Borrow the sanitized request that may be dispatched to the backend.
    #[must_use]
    pub const fn request(&self) -> &Request<B> {
        &self.request
    }
    /// Consume the authorization capability and recover the sanitized request.
    #[must_use]
    pub fn into_request(self) -> Request<B> {
        self.request
    }
}
/// Reusable receiver boundary for authenticated Governance DAG HTTP requests.
///
/// The receiver consumes one actual [`Request`] so the method, URI, headers,
/// and finalized body cannot be supplied from different request objects. It
/// derives the canonical absolute URL from the qualified endpoint and the
/// typed request parts, requires an unambiguous HTTP/1.x `Host`, validates any
/// URI authority against that host and endpoint, and rejects every unsigned
/// semantic header before signature or replay verification.
///
/// Replay state is deliberately caller-owned and borrowed for this receiver's
/// lifetime. A production implementation must supply an atomic shared sealed
/// store used by every replica in the qualified ingress set. The concrete
/// process-local cache in this crate is suitable only for isolated validation
/// and tests and cannot support a production ingress qualification.
#[derive(Debug)]
pub struct GovernanceDagHttpRequestReceiverV1<'a> {
    endpoint: Url,
    binding: GovernanceDagRequestIngressBindingV1,
    policy: GovernanceDagRequestAuthenticationPolicyV1,
    replay_store: &'a mut dyn GovernanceDagRequestAuthenticationReplayStoreV1,
}
impl<'a> GovernanceDagHttpRequestReceiverV1<'a> {
    /// Bind one exact endpoint policy and replay store.
    ///
    /// # Errors
    ///
    /// Rejects a noncanonical endpoint or a binding that does not commit to the
    /// exact normalized endpoint and authentication policy.
    pub fn try_new(
        endpoint: &str,
        binding: GovernanceDagRequestIngressBindingV1,
        replay_store: &'a mut dyn GovernanceDagRequestAuthenticationReplayStoreV1,
    ) -> Result<Self, GovernanceDagRequestAuthenticationErrorV1> {
        let endpoint =
            canonical_governance_dag_request_ingress_endpoint_url_v1(binding.scope(), endpoint)
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let endpoint_binding =
            governance_dag_request_ingress_endpoint_binding_v1(binding.scope(), endpoint.as_str())
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        if binding.endpoint_binding() != endpoint_binding {
            return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
        }
        let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            binding.public_key(),
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs(),
        )?;
        Ok(Self {
            endpoint,
            binding,
            policy,
            replay_store,
        })
    }
    /// Authenticate one complete HTTP request before backend dispatch.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free rejection and retains ownership of the
    /// request until every transport, structural, timing, signature, and replay
    /// check succeeds.
    pub fn verify_http_request<B: AsRef<[u8]>>(
        &mut self,
        request: Request<B>,
        now_unix_secs: u64,
    ) -> Result<GovernanceDagVerifiedHttpRequestV1<B>, GovernanceDagRequestAuthenticationErrorV1>
    {
        let (mut parts, body) = request.into_parts();
        let canonical_url = canonical_governance_dag_request_url_from_parts_v1(
            &self.endpoint,
            self.binding.scope(),
            &parts,
        )?;
        let canonical_url_parts = Url::parse(&canonical_url)
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let canonical_origin_form = match canonical_url_parts.query() {
            Some(query) => format!("{}?{query}", canonical_url_parts.path()),
            None => canonical_url_parts.path().to_owned(),
        };
        parts.uri = canonical_origin_form
            .parse()
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let body_bytes = body.as_ref();
        let (selected_headers, authentication_headers) = partition_governance_dag_http_headers_v1(
            parts
                .headers
                .iter()
                .filter(|(name, _)| name.as_str() != header::HOST.as_str())
                .map(|(name, value)| (name.as_str(), value.as_bytes())),
            body_bytes,
            GovernanceDagAuthenticationHeaderDispositionV1::Retain,
        )?;
        let descriptor = GovernanceDagCanonicalRequestV1::try_from_http_parts(
            self.binding.scope(),
            parts.method.as_str(),
            &canonical_url,
            selected_headers,
            body_bytes,
            self.binding.max_body_bytes(),
        )
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let envelope =
            parse_governance_dag_request_authentication_headers_v1(authentication_headers)?;
        verify_governance_dag_request_authentication_v1(
            &descriptor,
            &envelope,
            self.binding.scope(),
            &self.policy,
            now_unix_secs,
            self.replay_store,
        )?;
        for name in GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1 {
            parts.headers.remove(name);
        }
        Ok(GovernanceDagVerifiedHttpRequestV1 {
            request: Request::from_parts(parts, body),
            descriptor,
        })
    }
}
fn canonical_governance_dag_request_url_from_parts_v1(
    endpoint: &Url,
    scope: GovernanceDagAuthenticationScope,
    parts: &Parts,
) -> Result<String, GovernanceDagRequestAuthenticationErrorV1> {
    if parts.version == Version::HTTP_09 {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority);
    }
    let mut host_values = parts.headers.get_all(header::HOST).iter();
    let host = host_values.next();
    if host_values.next().is_some() {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority);
    }
    if matches!(parts.version, Version::HTTP_10 | Version::HTTP_11) && host.is_none() {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority);
    }
    let host_origin = host
        .map(|value| {
            value
                .to_str()
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority)
                .and_then(|authority| {
                    governance_dag_request_authority_origin_v1(endpoint, authority)
                })
        })
        .transpose()?;
    let uri_origin = parts
        .uri
        .authority()
        .map(|authority| governance_dag_request_authority_origin_v1(endpoint, authority.as_str()))
        .transpose()?;
    if parts
        .uri
        .scheme_str()
        .is_some_and(|scheme| scheme != endpoint.scheme())
        || host_origin
            .as_ref()
            .is_some_and(|origin| origin != &endpoint.origin())
        || uri_origin
            .as_ref()
            .is_some_and(|origin| origin != &endpoint.origin())
        || host_origin
            .as_ref()
            .zip(uri_origin.as_ref())
            .is_some_and(|(host, uri)| host != uri)
        || (host_origin.is_none() && uri_origin.is_none())
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::AuthorityMismatch);
    }
    let path_and_query = parts
        .uri
        .path_and_query()
        .ok_or(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?
        .as_str();
    if !path_and_query.starts_with('/') {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
    }
    let canonical_url = format!(
        "{}{path_and_query}",
        endpoint.origin().ascii_serialization()
    );
    if !governance_request_auth_url_is_canonical(&canonical_url) {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
    }
    let url = Url::parse(&canonical_url)
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    if url.path().contains('%') {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
    }
    let within_endpoint = match scope {
        GovernanceDagAuthenticationScope::SignedHead => url == *endpoint,
        GovernanceDagAuthenticationScope::Ipfs => url.path().starts_with(endpoint.path()),
    };
    if !within_endpoint {
        return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
    }
    Ok(canonical_url)
}
fn governance_dag_request_authority_origin_v1(
    endpoint: &Url,
    authority: &str,
) -> Result<url::Origin, GovernanceDagRequestAuthenticationErrorV1> {
    if authority.is_empty()
        || authority.trim() != authority
        || authority.contains(['/', '\\', '?', '#', '@'])
        || authority.chars().any(char::is_control)
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority);
    }
    let url = Url::parse(&format!("{}://{authority}/", endpoint.scheme()))
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority)?;
    if url.host_str().is_none()
        || url.port_or_known_default().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority);
    }
    Ok(url.origin())
}
type GovernanceDagHttpHeaderRefV1<'a> = (&'a str, &'a [u8]);
type GovernanceDagPartitionedHttpHeadersV1<'a> = (
    Vec<GovernanceDagHttpHeaderRefV1<'a>>,
    Vec<GovernanceDagHttpHeaderRefV1<'a>>,
);
fn partition_governance_dag_http_headers_v1<'a>(
    headers: impl IntoIterator<Item = GovernanceDagHttpHeaderRefV1<'a>>,
    body: &[u8],
    authentication_headers: GovernanceDagAuthenticationHeaderDispositionV1,
) -> Result<GovernanceDagPartitionedHttpHeadersV1<'a>, GovernanceDagRequestAuthenticationErrorV1> {
    let body_length = u64::try_from(body.len())
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::InvalidFraming)?;
    let mut selected = Vec::with_capacity(GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1);
    let mut authentication = Vec::with_capacity(GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1.len());
    let mut selected_header_bytes = 0_usize;
    let mut content_length = None;
    for (name, value) in headers {
        if governance_request_auth_is_forbidden_credential_header_v1(name) {
            return Err(GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader);
        }
        if governance_request_auth_header_has_prefix_v1(name) {
            if authentication_headers == GovernanceDagAuthenticationHeaderDispositionV1::Reject {
                return Err(GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader);
            }
            if authentication.len() == GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1.len() {
                return match parse_governance_dag_request_authentication_headers_v1(
                    authentication
                        .iter()
                        .copied()
                        .chain(std::iter::once((name, value))),
                ) {
                    Ok(_) => Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader),
                    Err(error) => Err(error),
                };
            }
            authentication.push((name, value));
            continue;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidFraming);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if name != "content-length" || content_length.is_some() {
                return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidFraming);
            }
            let parsed = parse_governance_request_content_length_v1(value)?;
            if parsed != body_length {
                return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidFraming);
            }
            content_length = Some(parsed);
            continue;
        }
        if GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1
            .binary_search(&name)
            .is_ok()
        {
            if selected.len() == GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1 {
                return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
            }
            selected_header_bytes = selected_header_bytes
                .checked_add(name.len())
                .and_then(|total| total.checked_add(value.len()))
                .ok_or(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
            if value.len() > GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_VALUE_BYTES_V1
                || selected_header_bytes > GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_BYTES_V1
            {
                return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
            }
            selected.push((name, value));
            continue;
        }
        if GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1
            .iter()
            .any(|selected_name| selected_name.eq_ignore_ascii_case(name))
        {
            return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
        }
        // The backend must never receive unsigned semantic-extension or proxy
        // metadata. HTTP framing is handled explicitly above; every other
        // accepted header is in the fixed signed allow-list.
        return Err(GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader);
    }
    Ok((selected, authentication))
}
fn parse_governance_request_content_length_v1(
    value: &[u8],
) -> Result<u64, GovernanceDagRequestAuthenticationErrorV1> {
    if value.is_empty()
        || (value.len() > 1 && value[0] == b'0')
        || !value.iter().all(u8::is_ascii_digit)
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidFraming);
    }
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(GovernanceDagRequestAuthenticationErrorV1::InvalidFraming)
}
fn governance_request_auth_is_forbidden_credential_header_v1(name: &str) -> bool {
    [
        "authorization",
        "proxy-authorization",
        "cookie",
        "cookie2",
        "set-cookie",
        "x-api-key",
        "x-auth-token",
    ]
    .iter()
    .any(|forbidden| forbidden.eq_ignore_ascii_case(name))
}
fn governance_request_auth_header_has_prefix_v1(name: &str) -> bool {
    name.get(..GOVERNANCE_DAG_REQUEST_AUTH_HEADER_PREFIX_V1.len())
        .is_some_and(|prefix| {
            prefix.eq_ignore_ascii_case(GOVERNANCE_DAG_REQUEST_AUTH_HEADER_PREFIX_V1)
        })
}
fn governance_request_auth_url_is_canonical(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.as_str() != value
        || governance_request_auth_url_has_noncanonical_percent_escape(value)
    {
        return false;
    }
    let query_pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if query_pairs.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return false;
    }
    let mut reconstructed = url.clone();
    reconstructed.set_query(None);
    if !query_pairs.is_empty() {
        let mut serializer = reconstructed.query_pairs_mut();
        for (key, value) in &query_pairs {
            serializer.append_pair(key, value);
        }
    }
    reconstructed.as_str() == value
}
fn governance_request_auth_url_has_noncanonical_percent_escape(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(encoded) = bytes.get(index + 1..index + 3) else {
            return true;
        };
        if !encoded.iter().all(|byte| byte.is_ascii_hexdigit())
            || encoded
                .iter()
                .any(|byte| byte.is_ascii_alphabetic() && !byte.is_ascii_uppercase())
        {
            return true;
        }
        let decoded = (governance_request_auth_hex_nibble(encoded[0]) << 4)
            | governance_request_auth_hex_nibble(encoded[1]);
        if decoded.is_ascii_alphanumeric() || matches!(decoded, b'-' | b'.' | b'_' | b'~') {
            return true;
        }
        index += 3;
    }
    false
}
fn governance_request_auth_hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("caller validates hexadecimal request-auth escapes"),
    }
}
/// Stable, payload-free rejection from the V1 request-authentication contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagRequestAuthenticationErrorV1 {
    /// The pinned Ed25519 public key is malformed or noncanonical.
    InvalidPolicyPublicKey,
    /// The configured timing bounds are outside the V1 policy.
    InvalidPolicyTiming,
    /// The requested replay-cache capacity is zero or above the V1 ceiling.
    InvalidReplayCacheCapacity,
    /// One of the eight fixed authentication headers is absent.
    MissingHeader,
    /// One fixed authentication header occurs more than once.
    DuplicateHeader,
    /// An unrecognized authentication-header alias or extension was supplied.
    UnknownHeader,
    /// An authentication header name or value is not in canonical wire form.
    NoncanonicalHeader,
    /// A credential or pre-existing authentication-prefix header was supplied.
    ForbiddenHeader,
    /// An unsigned header outside the fixed request contract was supplied.
    UnexpectedHeader,
    /// HTTP framing is ambiguous or disagrees with the finalized byte body.
    InvalidFraming,
    /// HTTP transport authority is absent, duplicated, or malformed.
    InvalidAuthority,
    /// HTTP `Host` or URI authority does not match the qualified endpoint.
    AuthorityMismatch,
    /// The method, URL, selected headers, or body commitment is noncanonical.
    NoncanonicalRequest,
    /// The envelope does not bind the receiver's exact request, scope, or key.
    RequestMismatch,
    /// The issuance interval is stale, future-dated, empty, or overlong.
    InvalidTiming,
    /// A required non-zero envelope field is zero.
    MalformedEnvelope,
    /// The Ed25519 public key is malformed or noncanonical.
    MalformedPublicKey,
    /// The Ed25519 signature encoding is malformed.
    MalformedSignature,
    /// The signature does not authenticate the exact canonical request.
    SignatureVerification,
    /// The nonce is already live in the caller-owned replay cache.
    Replay,
    /// The bounded replay cache cannot accept another live nonce.
    ReplayCacheFull,
    /// The deployment-owned shared sealed replay store is unavailable.
    ReplayStoreUnavailable,
}
impl fmt::Display for GovernanceDagRequestAuthenticationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPolicyPublicKey => {
                "Governance DAG request-auth policy public key is invalid"
            }
            Self::InvalidPolicyTiming => {
                "Governance DAG request-auth policy timing bounds are invalid"
            }
            Self::InvalidReplayCacheCapacity => {
                "Governance DAG request-auth replay-cache capacity is invalid"
            }
            Self::MissingHeader => {
                "Governance DAG request-auth required header is missing"
            }
            Self::DuplicateHeader => {
                "Governance DAG request-auth header is duplicated"
            }
            Self::UnknownHeader => {
                "Governance DAG request-auth header alias or extension is not recognized"
            }
            Self::NoncanonicalHeader => {
                "Governance DAG request-auth header is not canonical"
            }
            Self::ForbiddenHeader => {
                "Governance DAG request contains a forbidden credential or authentication header"
            }
            Self::UnexpectedHeader => {
                "Governance DAG request contains an unsigned header outside the fixed contract"
            }
            Self::InvalidFraming => {
                "Governance DAG request HTTP framing is ambiguous or inconsistent"
            }
            Self::InvalidAuthority => {
                "Governance DAG request HTTP transport authority is invalid"
            }
            Self::AuthorityMismatch => {
                "Governance DAG request HTTP transport authority does not match the qualified endpoint"
            }
            Self::NoncanonicalRequest => {
                "Governance DAG request is not canonical or bounded"
            }
            Self::RequestMismatch => {
                "Governance DAG request-auth envelope does not match the canonical request"
            }
            Self::InvalidTiming => {
                "Governance DAG request-auth envelope is stale, future-dated, overlong, or malformed"
            }
            Self::MalformedEnvelope => {
                "Governance DAG request-auth envelope is malformed"
            }
            Self::MalformedPublicKey => {
                "Governance DAG request-auth envelope contains a malformed public key"
            }
            Self::MalformedSignature => {
                "Governance DAG request-auth envelope contains a malformed signature"
            }
            Self::SignatureVerification => {
                "Governance DAG request-auth signature verification failed"
            }
            Self::Replay => {
                "Governance DAG request-auth envelope replay was rejected"
            }
            Self::ReplayCacheFull => {
                "Governance DAG request-auth replay state reached its bounded capacity"
            }
            Self::ReplayStoreUnavailable => {
                "Governance DAG request-auth shared sealed replay store is unavailable"
            }
        })
    }
}
impl std::error::Error for GovernanceDagRequestAuthenticationErrorV1 {}
/// Pinned receiver policy for V1 Governance DAG request authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceDagRequestAuthenticationPolicyV1 {
    public_key: [u8; 32],
    max_envelope_lifetime_secs: u64,
    max_future_skew_secs: u64,
}
impl GovernanceDagRequestAuthenticationPolicyV1 {
    /// Construct and validate one receiver policy.
    ///
    /// # Errors
    ///
    /// Returns a typed policy rejection when the key is not canonical Ed25519
    /// or the timing bounds exceed V1.
    pub fn try_new(
        public_key: [u8; 32],
        max_envelope_lifetime_secs: u64,
        max_future_skew_secs: u64,
    ) -> Result<Self, GovernanceDagRequestAuthenticationErrorV1> {
        if !(1..=GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1)
            .contains(&max_envelope_lifetime_secs)
            || max_future_skew_secs > GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1
            || max_future_skew_secs >= max_envelope_lifetime_secs
        {
            return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyTiming);
        }
        if public_key.iter().all(|byte| *byte == 0) {
            return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyPublicKey);
        }
        let key = PublicKey::from_bytes(Algorithm::Ed25519, &public_key)
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyPublicKey)?;
        let (algorithm, canonical_bytes) = key
            .try_to_bytes()
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyPublicKey)?;
        if algorithm != Algorithm::Ed25519 || canonical_bytes != public_key.as_slice() {
            return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyPublicKey);
        }
        Ok(Self {
            public_key,
            max_envelope_lifetime_secs,
            max_future_skew_secs,
        })
    }
    /// Raw canonical Ed25519 public key pinned by the receiver.
    #[must_use]
    pub const fn public_key(self) -> [u8; 32] {
        self.public_key
    }
    /// Maximum accepted envelope lifetime in seconds.
    #[must_use]
    pub const fn max_envelope_lifetime_secs(self) -> u64 {
        self.max_envelope_lifetime_secs
    }
    /// Maximum accepted issuance skew into the future in seconds.
    #[must_use]
    pub const fn max_future_skew_secs(self) -> u64 {
        self.max_future_skew_secs
    }
}
/// Replay-consumption boundary used by the V1 authenticated receiver.
///
/// Production implementations must atomically consume one nonce in a shared,
/// durably sealed namespace visible to every qualified ingress replica and
/// retain the evidence through `expires_at_unix_secs`. An unavailable or
/// ambiguous store must fail closed.
pub trait GovernanceDagRequestAuthenticationReplayStoreV1: fmt::Debug {
    /// Atomically reject or consume one live nonce.
    ///
    /// # Errors
    ///
    /// Returns [`GovernanceDagRequestAuthenticationErrorV1::Replay`] when the
    /// nonce was already consumed, and a fail-closed store error when durable
    /// consumption cannot be proven.
    fn consume_nonce(
        &mut self,
        nonce: [u8; 32],
        expires_at_unix_secs: u64,
        now_unix_secs: u64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1>;
}
/// Caller-owned process-local bounded live-nonce cache for V1 validation.
///
/// This cache never evicts a live nonce to admit another request; capacity
/// pressure therefore fails closed. It is useful for isolated receivers and
/// tests, but it is neither shared nor sealed and must not be cited as evidence
/// for a production [`GovernanceDagRequestIngressQualificationV1`].
#[derive(Debug)]
pub struct GovernanceDagRequestAuthenticationReplayCacheV1 {
    entries: BTreeMap<[u8; 32], u64>,
    capacity: usize,
}
impl GovernanceDagRequestAuthenticationReplayCacheV1 {
    /// Construct an empty cache with the governed V1 capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            capacity: GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1,
        }
    }
    /// Construct an empty cache with a smaller deterministic capacity.
    ///
    /// # Errors
    ///
    /// Rejects zero or a capacity above
    /// [`GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1`].
    pub fn try_with_capacity(
        capacity: usize,
    ) -> Result<Self, GovernanceDagRequestAuthenticationErrorV1> {
        if capacity == 0 || capacity > GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1 {
            return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidReplayCacheCapacity);
        }
        Ok(Self {
            entries: BTreeMap::new(),
            capacity,
        })
    }
    fn consume(
        &mut self,
        nonce: [u8; 32],
        expires_at_unix_secs: u64,
        now_unix_secs: u64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        self.entries
            .retain(|_, retained_expiry| *retained_expiry > now_unix_secs);
        if self.entries.contains_key(&nonce) {
            return Err(GovernanceDagRequestAuthenticationErrorV1::Replay);
        }
        if self.entries.len() >= self.capacity {
            return Err(GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull);
        }
        self.entries.insert(nonce, expires_at_unix_secs);
        Ok(())
    }
}
impl GovernanceDagRequestAuthenticationReplayStoreV1
    for GovernanceDagRequestAuthenticationReplayCacheV1
{
    fn consume_nonce(
        &mut self,
        nonce: [u8; 32],
        expires_at_unix_secs: u64,
        now_unix_secs: u64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        self.consume(nonce, expires_at_unix_secs, now_unix_secs)
    }
}
impl Default for GovernanceDagRequestAuthenticationReplayCacheV1 {
    fn default() -> Self {
        Self::new()
    }
}
/// Public HSM-signed authentication envelope for one canonical request.
///
/// The envelope deliberately exposes only fixed public authentication fields.
/// It contains no bearer token, cookie, mTLS identity, private key, or backend
/// diagnostic. The signature binds the complete descriptor plus the public
/// key, issuance interval, nonce, and request digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagRequestAuthenticationEnvelopeV1 {
    scope: GovernanceDagAuthenticationScope,
    issued_at_unix_secs: u64,
    expires_at_unix_secs: u64,
    nonce: [u8; 32],
    request_digest: [u8; 32],
    public_key: [u8; 32],
    signature: [u8; 64],
}
impl GovernanceDagRequestAuthenticationEnvelopeV1 {
    /// Construct one structurally canonical signed envelope.
    ///
    /// Freshness, pinned-key equality, request equality, replay, and signature
    /// verification are enforced by
    /// [`verify_governance_dag_request_authentication_v1`].
    ///
    /// # Errors
    ///
    /// Rejects zero or inverted timing fields and zero nonce, digest, key, or
    /// signature values.
    pub fn try_new(
        descriptor: &GovernanceDagCanonicalRequestV1,
        issued_at_unix_secs: u64,
        expires_at_unix_secs: u64,
        nonce: [u8; 32],
        public_key: [u8; 32],
        signature: [u8; 64],
    ) -> Result<Self, &'static str> {
        if issued_at_unix_secs == 0
            || expires_at_unix_secs <= issued_at_unix_secs
            || nonce.iter().all(|byte| *byte == 0)
            || descriptor.request_digest.iter().all(|byte| *byte == 0)
            || public_key.iter().all(|byte| *byte == 0)
            || signature.iter().all(|byte| *byte == 0)
        {
            return Err("Governance DAG request-auth envelope is malformed");
        }
        Ok(Self {
            scope: descriptor.scope,
            issued_at_unix_secs,
            expires_at_unix_secs,
            nonce,
            request_digest: descriptor.request_digest,
            public_key,
            signature,
        })
    }
    /// Build the exact domain-separated bytes an HSM must sign.
    #[must_use]
    pub fn signing_payload(
        descriptor: &GovernanceDagCanonicalRequestV1,
        issued_at_unix_secs: u64,
        expires_at_unix_secs: u64,
        nonce: [u8; 32],
        public_key: [u8; 32],
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(
            GOVERNANCE_DAG_REQUEST_AUTH_DOMAIN_V1.len()
                + descriptor.method.len()
                + descriptor.canonical_url.len()
                + GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_BYTES_V1
                + 256,
        );
        payload.extend_from_slice(GOVERNANCE_DAG_REQUEST_AUTH_DOMAIN_V1);
        descriptor.append_canonical_bytes(&mut payload);
        payload.extend_from_slice(&descriptor.request_digest);
        payload.extend_from_slice(&issued_at_unix_secs.to_be_bytes());
        payload.extend_from_slice(&expires_at_unix_secs.to_be_bytes());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&public_key);
        payload
    }
    /// Authenticated endpoint class.
    #[must_use]
    pub const fn scope(&self) -> GovernanceDagAuthenticationScope {
        self.scope
    }
    /// Inclusive issuance time as Unix seconds.
    #[must_use]
    pub const fn issued_at_unix_secs(&self) -> u64 {
        self.issued_at_unix_secs
    }
    /// Exclusive expiry time as Unix seconds.
    #[must_use]
    pub const fn expires_at_unix_secs(&self) -> u64 {
        self.expires_at_unix_secs
    }
    /// Public 256-bit one-use nonce.
    #[must_use]
    pub const fn nonce(&self) -> [u8; 32] {
        self.nonce
    }
    /// Signed canonical request digest.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
    /// Raw Ed25519 public key used by the runtime HSM.
    #[must_use]
    pub const fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    /// Raw canonical Ed25519 signature.
    #[must_use]
    pub const fn signature(&self) -> [u8; 64] {
        self.signature
    }
}
/// Render the exact eight canonical public HTTP authentication headers.
#[must_use]
pub fn governance_dag_request_authentication_headers_v1(
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
) -> [(&'static str, String); 8] {
    [
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[0],
            GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1.to_string(),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[1],
            envelope.scope().as_str().to_owned(),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[2],
            envelope.issued_at_unix_secs().to_string(),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[3],
            envelope.expires_at_unix_secs().to_string(),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[4],
            hex::encode(envelope.nonce()),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[5],
            hex::encode(envelope.request_digest()),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[6],
            hex::encode(envelope.public_key()),
        ),
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[7],
            hex::encode(envelope.signature()),
        ),
    ]
}
/// Parse exactly one V1 public authentication-envelope header set.
///
/// Ordinary HTTP headers are ignored. Every name using the Governance DAG
/// authentication prefix is part of this hard-cut contract: aliases,
/// extensions, case variants, duplicates, and missing fields are rejected.
/// Parsing alone grants no authority; receivers must pass the result to
/// [`verify_governance_dag_request_authentication_v1`] before dispatch.
///
/// # Errors
///
/// Returns a stable, payload-free header rejection without exposing header
/// values.
pub fn parse_governance_dag_request_authentication_headers_v1<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, GovernanceDagRequestAuthenticationErrorV1>
{
    let mut fields: [Option<&[u8]>; 8] = [None; 8];
    for (name, value) in headers {
        if !governance_request_auth_header_has_prefix_v1(name) {
            continue;
        }
        let Some(index) = GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1
            .iter()
            .position(|candidate| *candidate == name)
        else {
            if GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(name))
            {
                return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
            }
            return Err(GovernanceDagRequestAuthenticationErrorV1::UnknownHeader);
        };
        if fields[index].replace(value).is_some() {
            return Err(GovernanceDagRequestAuthenticationErrorV1::DuplicateHeader);
        }
    }
    let [
        Some(version),
        Some(scope),
        Some(issued_at),
        Some(expires_at),
        Some(nonce),
        Some(request_digest),
        Some(public_key),
        Some(signature),
    ] = fields
    else {
        return Err(GovernanceDagRequestAuthenticationErrorV1::MissingHeader);
    };
    if parse_governance_request_auth_decimal_header_v1(version)?
        != u64::from(GOVERNANCE_DAG_REQUEST_AUTH_VERSION_V1)
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
    }
    let scope = match scope {
        b"ipfs" => GovernanceDagAuthenticationScope::Ipfs,
        b"signed-head" => GovernanceDagAuthenticationScope::SignedHead,
        _ => return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader),
    };
    Ok(GovernanceDagRequestAuthenticationEnvelopeV1 {
        scope,
        issued_at_unix_secs: parse_governance_request_auth_decimal_header_v1(issued_at)?,
        expires_at_unix_secs: parse_governance_request_auth_decimal_header_v1(expires_at)?,
        nonce: parse_governance_request_auth_hex_header_v1(nonce)?,
        request_digest: parse_governance_request_auth_hex_header_v1(request_digest)?,
        public_key: parse_governance_request_auth_hex_header_v1(public_key)?,
        signature: parse_governance_request_auth_hex_header_v1(signature)?,
    })
}
/// Verify one parsed envelope before forwarding the request to its backend.
///
/// `now_unix_secs` is supplied by the receiver so tests and deployment time
/// sources remain explicit. The nonce is recorded only after every binding,
/// timing, key, and signature check succeeds.
///
/// # Errors
///
/// Returns a stable, payload-free rejection and leaves the replay cache
/// unchanged for every failure preceding nonce consumption.
pub fn verify_governance_dag_request_authentication_v1(
    request: &GovernanceDagCanonicalRequestV1,
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
    expected_scope: GovernanceDagAuthenticationScope,
    policy: &GovernanceDagRequestAuthenticationPolicyV1,
    now_unix_secs: u64,
    replay_store: &mut dyn GovernanceDagRequestAuthenticationReplayStoreV1,
) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
    verify_governance_dag_request_authentication_without_replay_v1(
        request,
        envelope,
        expected_scope,
        policy,
        now_unix_secs,
    )?;
    replay_store.consume_nonce(
        envelope.nonce(),
        envelope.expires_at_unix_secs(),
        now_unix_secs,
    )
}
/// Verify every request-auth property except receiver-side nonce consumption.
///
/// This exists only for the outbound service's non-authoritative signer sanity
/// check. An ingress receiver must call
/// [`verify_governance_dag_request_authentication_v1`] so the shared sealed
/// replay store is atomically consumed before backend dispatch.
pub(crate) fn verify_governance_dag_request_authentication_without_replay_v1(
    request: &GovernanceDagCanonicalRequestV1,
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
    expected_scope: GovernanceDagAuthenticationScope,
    policy: &GovernanceDagRequestAuthenticationPolicyV1,
    now_unix_secs: u64,
) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
    if request.scope() != expected_scope
        || envelope.scope() != expected_scope
        || envelope.request_digest() != request.request_digest()
        || envelope.public_key() != policy.public_key()
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
    }
    let issued_at = envelope.issued_at_unix_secs();
    let expires_at = envelope.expires_at_unix_secs();
    let lifetime = expires_at
        .checked_sub(issued_at)
        .ok_or(GovernanceDagRequestAuthenticationErrorV1::InvalidTiming)?;
    if issued_at == 0
        || lifetime == 0
        || lifetime > policy.max_envelope_lifetime_secs()
        || issued_at > now_unix_secs.saturating_add(policy.max_future_skew_secs())
        || expires_at <= now_unix_secs
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::InvalidTiming);
    }
    if envelope.nonce().iter().all(|byte| *byte == 0)
        || envelope.request_digest().iter().all(|byte| *byte == 0)
        || envelope.public_key().iter().all(|byte| *byte == 0)
        || envelope.signature().iter().all(|byte| *byte == 0)
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::MalformedEnvelope);
    }
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &envelope.public_key())
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::MalformedPublicKey)?;
    let signature = iroha_crypto::ed25519_parse_signature(&envelope.signature())
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::MalformedSignature)?;
    let signing_payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
        request,
        issued_at,
        expires_at,
        envelope.nonce(),
        envelope.public_key(),
    );
    signature
        .verify(&public_key, &signing_payload)
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::SignatureVerification)
}
fn parse_governance_request_auth_decimal_header_v1(
    value: &[u8],
) -> Result<u64, GovernanceDagRequestAuthenticationErrorV1> {
    if value.is_empty()
        || (value.len() > 1 && value[0] == b'0')
        || !value.iter().all(u8::is_ascii_digit)
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
    }
    let value = std::str::from_utf8(value)
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?
        .parse::<u64>()
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
    Ok(value)
}
fn parse_governance_request_auth_hex_header_v1<const N: usize>(
    value: &[u8],
) -> Result<[u8; N], GovernanceDagRequestAuthenticationErrorV1> {
    if value.len() != N.saturating_mul(2)
        || !value
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
    }
    let mut decoded = [0; N];
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
    Ok(decoded)
}
fn append_governance_request_auth_field(bytes: &mut Vec<u8>, field: &[u8]) {
    let length = u32::try_from(field.len()).expect("bounded request-auth fields fit u32");
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(field);
}
/// Rotation-aware, receiver-qualified runtime authenticator for Governance DAG publication.
///
/// Implementations own an Ed25519 HSM signing boundary and return only the
/// public signed envelope for a complete canonical request. The adapter never
/// receives a `reqwest` client, builder, body owner, or mutable header map and
/// therefore cannot inject bearer tokens, cookies, mTLS credentials, or other
/// opaque request authority. First-release providers must additionally own the
/// live deployment proof that the exact backend endpoint is exclusively
/// receiver-fronted and that all ingress replicas share one sealed atomic
/// replay namespace.
pub trait GovernanceDagRequestAuthenticator: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this authenticator.
    fn handle(&self) -> &str;
    /// Actively qualify the adapter, receiver, endpoint, and replay store.
    ///
    /// Implementations must probe the exact receiver and shared sealed replay
    /// namespace before returning. They must fail when the credential boundary,
    /// receiver, replica set, or replay store is unavailable, bypassable,
    /// revoked, stale, test-marked, process-local, or otherwise not
    /// production-ready. Returning configuration text without a live probe
    /// violates this trust-boundary contract.
    fn ingress_qualification(&self) -> Result<GovernanceDagRequestIngressQualificationV1, String>;
    /// Sign one exact bounded canonical outbound request descriptor.
    ///
    /// The returned envelope must use a fresh non-zero nonce and a short
    /// issuance interval. Implementations must redact backend diagnostics from
    /// errors; the service redacts them again at this trust boundary.
    fn authenticate(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String>;
}
/// Durable object class owned by the sealed Governance DAG checkpoint store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagSealedStateSlot {
    /// Last fully published and verified checkpoint.
    Checkpoint,
    /// Write-ahead publication intent.
    PublishIntent,
    /// Last fully committed local signed-producer root.
    ProducerCheckpoint,
    /// Exact local signed-producer filesystem transaction intent.
    ProducerPublishIntent,
    /// Live request-authentication nonces for Kubo/IPFS/IPNS operations.
    IpfsRequestReplay,
    /// Live request-authentication nonces for signed-head operations.
    SignedHeadRequestReplay,
}
impl GovernanceDagSealedStateSlot {
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Checkpoint => b"sorafs.governance_dag.sealed.checkpoint.v1",
            Self::PublishIntent => b"sorafs.governance_dag.sealed.publish_intent.v1",
            Self::ProducerCheckpoint => b"sorafs.governance_dag.sealed.producer-checkpoint.v1",
            Self::ProducerPublishIntent => {
                b"sorafs.governance_dag.sealed.producer-publish-intent.v1"
            }
            Self::IpfsRequestReplay => b"sorafs.governance_dag.sealed.ipfs-request-replay.v1",
            Self::SignedHeadRequestReplay => {
                b"sorafs.governance_dag.sealed.signed-head-request-replay.v1"
            }
        }
    }
}
/// Return the canonical V1 payload ceiling for one sealed-state slot.
///
/// Producer filesystem intents contain only checkpoint metadata and digests
/// for a durably staged transaction. Their deliberately small ceiling prevents
/// a checkpoint provider from forcing a full mutable index allocation during
/// sealed-record decoding.
#[must_use]
pub const fn governance_dag_sealed_state_payload_max_bytes_v1(
    slot: GovernanceDagSealedStateSlot,
) -> usize {
    match slot {
        GovernanceDagSealedStateSlot::ProducerCheckpoint => {
            GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_SEALED_MAX_BYTES_V1
        }
        GovernanceDagSealedStateSlot::ProducerPublishIntent => {
            GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_SEALED_MAX_BYTES_V1
        }
        GovernanceDagSealedStateSlot::IpfsRequestReplay
        | GovernanceDagSealedStateSlot::SignedHeadRequestReplay => {
            GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_SEALED_MAX_BYTES_V1
        }
        GovernanceDagSealedStateSlot::Checkpoint | GovernanceDagSealedStateSlot::PublishIntent => {
            GOVERNANCE_DAG_SEALED_STATE_MAX_BYTES_V1
        }
    }
}
/// Unsealed canonical record returned by the runtime checkpoint provider.
///
/// The provider must keep this payload authenticated and confidential at rest.
/// `revision` is a public content/CAS token checked again by the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagSealedStateRecord {
    /// Monotonic publication generation bound to the record.
    pub generation: u64,
    /// Deterministic content revision.
    pub revision: [u8; 32],
    /// Canonical Norito payload recovered by the provider.
    pub payload: Vec<u8>,
}
impl GovernanceDagSealedStateRecord {
    /// Construct a record and bind its public CAS revision.
    #[must_use]
    pub fn new(slot: GovernanceDagSealedStateSlot, generation: u64, payload: Vec<u8>) -> Self {
        let revision = governance_dag_sealed_state_revision(slot, generation, &payload);
        Self {
            generation,
            revision,
            payload,
        }
    }
    /// Verify the record's deterministic public CAS revision.
    #[must_use]
    pub fn has_valid_revision(&self, slot: GovernanceDagSealedStateSlot) -> bool {
        self.revision == governance_dag_sealed_state_revision(slot, self.generation, &self.payload)
    }
}
/// Derive the deterministic public CAS token for sealed Governance DAG state.
#[must_use]
pub fn governance_dag_sealed_state_revision(
    slot: GovernanceDagSealedStateSlot,
    generation: u64,
    payload: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(slot.domain());
    hasher.update(&generation.to_le_bytes());
    hasher.update(
        &u64::try_from(payload.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}
/// Runtime-only sealed, monotonic Governance DAG checkpoint storage.
///
/// Implementations must seal payloads at rest and enforce linearizable
/// compare-and-swap. A generation may stay equal while an in-flight publish
/// intent advances, but it must never decrease. Checkpoint generation must
/// strictly advance, request-replay generations must strictly advance, and
/// deletes must compare-and-swap the exact last transient-intent revision.
pub trait GovernanceDagSealedCheckpointStore: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this store.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    ///
    /// Implementations must fail when the sealed monotonic store is
    /// unavailable, revoked, stale, test-marked, or otherwise not
    /// production-ready.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;
    /// Load and unseal the latest record for `slot`.
    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, String>;
    /// Atomically store `next` if `expected_revision` is still current.
    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), String>;
    /// Atomically remove a transient record if its exact revision is current.
    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String>;
}
/// Deployment-owned authenticated reader for the authoritative privacy target head.
///
/// Implementations authenticate every target read using runtime-only credentials
/// and return only the current committed head. Credentials, endpoint tokens,
/// private keys, and provider diagnostics must never cross this boundary.
pub trait FencedTransparencyAuthoritativeHeadReaderV1: Send + Sync + fmt::Debug {
    /// Stable, credential-free deployment identity for this reader.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    ///
    /// Implementations must fail when the authenticated transport or readback
    /// provider is unavailable, revoked, stale, test-marked, or otherwise not
    /// production-ready.
    ///
    /// # Errors
    ///
    /// Returns a redacted diagnostic when the configured provider cannot prove
    /// its current public identity and policy qualification.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;
    /// Authenticate the current head and verify every exact requested ancestor
    /// and publication inclusion.
    ///
    /// Implementations must verify target-owned immutable-history or inclusion
    /// evidence. Generation and fencing-floor comparisons alone are never an
    /// ancestry proof. `None` is an authenticated genesis observation, not a
    /// local default. Production adapters must implement this complete operation;
    /// there is no read-only fallback that can defer inclusion verification until
    /// after an append.
    ///
    /// # Errors
    ///
    /// Returns a redacted diagnostic when the target cannot authenticate the
    /// read, prove that every requested head reaches its exact current head, or
    /// prove an exact stable publication identity and payload at its claimed
    /// inclusion head.
    fn read_authoritative_head_with_ancestry(
        &self,
        required_ancestors: &[FencedTransparencyTargetHeadV1],
        required_publications: &[FencedTransparencyPublicationInclusionV1],
    ) -> Result<FencedTransparencyHeadAncestryProofV1, String>;
}
#[derive(Debug, Clone)]
struct PublishIndexEntryForCar {
    position: usize,
    newly_inserted: bool,
    payload_kind: String,
    encoded_path: String,
    json_path: String,
    encoded_blake3: String,
    encoded_len: usize,
    json_blake3: String,
    json_len: usize,
}
struct PreparedGovernanceCarSegment {
    segment: JsonMap,
    car_path: PathBuf,
    plan_path: PathBuf,
    manifest_path: PathBuf,
    car_bytes: Vec<u8>,
    plan_body: String,
    manifest_body: String,
}
/// One weekly rollup recovered from the fully authenticated runtime DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthoritativeAppealFinanceWeeklyRollup {
    /// BLAKE3 digest of the exact canonical source payload.
    pub(crate) encoded_blake3: String,
    /// Typed payload authenticated by the signed runtime DAG.
    pub(crate) rollup: SoraFsAppealFinanceWeeklyRollupV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct CachedPrivacyPublicationAuthorizationV1 {
    lease_id: [u8; 32],
    scope_query_id: [u8; 32],
    scope_cycle_id: [u8; 16],
    scope_cycle_start_unix: u64,
    scope_cycle_end_unix: u64,
    scope_due_at_unix: u64,
    scope_holder_identity: [u8; 32],
    lease_fencing_token: u64,
    lease_issued_at_unix: u64,
    lease_expires_at_unix: u64,
    lease_provider_handle: String,
    lease_provider_revision: u64,
    lease_provider_policy_digest: [u8; 32],
    finalized_anchor_query_id: [u8; 32],
    finalized_anchor_sequence: u64,
    finalized_anchor_release_id: [u8; 16],
    finalized_anchor_record_digest: [u8; 32],
    finalized_anchor_latest_publication_block_hash: Option<[u8; 32]>,
    release_sequence: u64,
    release_record_digest: [u8; 32],
    payload_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FencedPrivacyPendingRequestV1 {
    version: u8,
    target_handle: String,
    target_revision: u64,
    target_policy_digest: [u8; 32],
    request_digest: [u8; 32],
    authorization_digest: [u8; 32],
    publication_idempotency_digest: [u8; 32],
    authorization: CachedPrivacyPublicationAuthorizationV1,
    payload_digest: [u8; 32],
    expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    fencing_token: u64,
    fencing_floor: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FencedPrivacyPublicationCacheV1 {
    version: u8,
    target_handle: String,
    target_revision: u64,
    target_policy_digest: [u8; 32],
    authoritative_head: FencedTransparencyTargetHeadV1,
    last_request_digest: [u8; 32],
    last_authorization_digest: [u8; 32],
    last_publication_idempotency_digest: [u8; 32],
    last_authorization: CachedPrivacyPublicationAuthorizationV1,
    last_payload_digest: [u8; 32],
    last_expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    last_fencing_token: u64,
    last_fencing_floor: u64,
    last_disposition: FencedPrivacyPublicationDispositionV1,
    last_included_head: FencedTransparencyTargetHeadV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FencedPrivacyAuthoritativeHeadSyncV1 {
    version: u8,
    reader_handle: String,
    reader_revision: u64,
    reader_policy_digest: [u8; 32],
    authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    ancestry_proof_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FencedPrivacyStateV1 {
    version: u8,
    pending: Option<FencedPrivacyPendingRequestV1>,
    publication_cache: Option<FencedPrivacyPublicationCacheV1>,
    authoritative_head_sync: Option<FencedPrivacyAuthoritativeHeadSyncV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProviderBindingV1 {
    signer_handle: String,
    signer_revision: u64,
    signer_policy_digest: [u8; 32],
    checkpoint_store_handle: String,
    checkpoint_store_revision: u64,
    checkpoint_store_policy_digest: [u8; 32],
    publisher_peer_id: Vec<u8>,
    publisher_public_key: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationTransitionBodyV1 {
    version: u8,
    root_digest: [u8; 32],
    generation: u64,
    predecessor_transition_digest: Option<[u8; 32]>,
    predecessor_checkpoint_revision: [u8; 32],
    previous: RuntimeDagProviderBindingV1,
    next: RuntimeDagProviderBindingV1,
    block_count: u64,
    head_block_cid: [u8; 32],
    head_bytes_digest: [u8; 32],
    predecessor_index_digest: [u8; 32],
    successor_index_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagKeyTransitionSigningPayloadV1 {
    version: u8,
    outgoing_segment_revision: u64,
    incoming_segment_revision: u64,
    transition_body_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagKeyTransitionEnvelopeV1 {
    version: u8,
    outgoing_segment_revision: u64,
    incoming_segment_revision: u64,
    transition_body_digest: [u8; 32],
    outgoing_signature: [u8; 64],
    incoming_signature: [u8; 64],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationTransitionV1 {
    body: RuntimeDagQualificationTransitionBodyV1,
    key_transition: RuntimeDagKeyTransitionEnvelopeV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationHistoryV1 {
    version: u8,
    root_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
    archived_through_generation: u64,
    archive_tail_transition_digest: [u8; 32],
    transitions: Vec<RuntimeDagQualificationTransitionV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationStateV1 {
    version: u8,
    history: Option<RuntimeDagQualificationHistoryV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationArchiveBodyV1 {
    version: u8,
    root_digest: [u8; 32],
    archive_generation: u64,
    predecessor_archive_digest: [u8; 32],
    predecessor_transition_digest: [u8; 32],
    first_transition_generation: u64,
    last_transition_generation: u64,
    tail_transition_digest: [u8; 32],
    signer: RuntimeDagProviderBindingV1,
    transitions: Vec<RuntimeDagQualificationTransitionV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationArchiveV1 {
    body: RuntimeDagQualificationArchiveBodyV1,
    signature: [u8; 64],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeDagQualificationSummary {
    transition_generation: u64,
    transition_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
}
impl RuntimeDagQualificationSummary {
    const EMPTY: Self = Self {
        transition_generation: 0,
        transition_digest: [0; 32],
        archive_generation: 0,
        archive_digest: [0; 32],
    };
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RuntimeDagProducerCheckpointV1 {
    pub(crate) version: u8,
    pub(crate) root_digest: [u8; 32],
    pub(crate) signer_handle: String,
    pub(crate) signer_revision: u64,
    pub(crate) signer_policy_digest: [u8; 32],
    pub(crate) checkpoint_store_handle: String,
    pub(crate) checkpoint_store_revision: u64,
    pub(crate) checkpoint_store_policy_digest: [u8; 32],
    pub(crate) publisher_peer_id: Vec<u8>,
    pub(crate) publisher_public_key: [u8; 32],
    pub(crate) block_count: u64,
    pub(crate) head_block_cid: [u8; 32],
    pub(crate) head_bytes_digest: [u8; 32],
    pub(crate) index_bytes_digest: [u8; 32],
    pub(crate) qualification_transition_generation: u64,
    pub(crate) qualification_transition_digest: [u8; 32],
    pub(crate) qualification_archive_generation: u64,
    pub(crate) qualification_archive_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedArtifactV1 {
    byte_len: u64,
    blake3: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerPublishIntentV1 {
    version: u8,
    checkpoint: RuntimeDagProducerCheckpointV1,
    previous_checkpoint_revision: Option<[u8; 32]>,
    staging_revision: [u8; 32],
    block: RuntimeDagProducerStagedArtifactV1,
    head: RuntimeDagProducerStagedArtifactV1,
    index: RuntimeDagProducerStagedArtifactV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedTransactionV1 {
    block_bytes: Vec<u8>,
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedEnvelopeV1 {
    intent: RuntimeDagProducerPublishIntentV1,
    transaction: RuntimeDagProducerStagedTransactionV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagingStateV1 {
    version: u8,
    staged: Option<RuntimeDagProducerStagedEnvelopeV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagCommittedStateV1 {
    version: u8,
    head_bytes: Option<Vec<u8>>,
    index_bytes: Option<Vec<u8>>,
}
/// One exact authenticated publication-authority generation for local readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GovernancePublicationSnapshotV1 {
    store_generation: u64,
    store_record_digest: [u8; 32],
    canonical_bytes: Vec<u8>,
}
impl GovernancePublicationSnapshotV1 {
    /// Return the fixed-store generation and complete record digest.
    #[cfg(test)]
    pub(crate) fn store_identity(&self) -> (u64, [u8; 32]) {
        (self.store_generation, self.store_record_digest)
    }
    /// Borrow the canonical authoritative publication JSON bytes.
    #[cfg(test)]
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Consume the snapshot without cloning its potentially large canonical
    /// publication body.
    pub(crate) fn into_parts(self) -> (Vec<u8>, u64, [u8; 32]) {
        (
            self.canonical_bytes,
            self.store_generation,
            self.store_record_digest,
        )
    }
}
/// One exact authenticated runtime-DAG head/index generation for local readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeDagCommittedSnapshotV1 {
    store_generation: u64,
    store_record_digest: [u8; 32],
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
}
impl RuntimeDagCommittedSnapshotV1 {
    /// Return the fixed-store generation and complete record digest.
    pub(crate) fn store_identity(&self) -> (u64, [u8; 32]) {
        (self.store_generation, self.store_record_digest)
    }
    /// Borrow the canonical signed-head bytes committed with the index.
    pub(crate) fn head_bytes(&self) -> &[u8] {
        &self.head_bytes
    }
    /// Borrow the canonical runtime-index bytes committed with the head.
    pub(crate) fn index_bytes(&self) -> &[u8] {
        &self.index_bytes
    }
}
/// One read-only runtime-DAG generation authenticated by an exact sealed
/// producer checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedRuntimeDagSnapshotV1 {
    committed: RuntimeDagCommittedSnapshotV1,
    checkpoint_generation: u64,
    checkpoint_revision: [u8; 32],
}
impl AuthenticatedRuntimeDagSnapshotV1 {
    /// Return the fixed-store generation and complete record digest.
    #[cfg(test)]
    pub(crate) fn store_identity(&self) -> (u64, [u8; 32]) {
        self.committed.store_identity()
    }
    /// Borrow the canonical signed-head bytes.
    #[cfg(test)]
    pub(crate) fn head_bytes(&self) -> &[u8] {
        self.committed.head_bytes()
    }
    /// Borrow the canonical runtime-index bytes.
    #[cfg(test)]
    pub(crate) fn index_bytes(&self) -> &[u8] {
        self.committed.index_bytes()
    }
    /// Return the sealed producer-checkpoint generation and revision digest.
    #[cfg(test)]
    pub(crate) fn checkpoint_identity(&self) -> (u64, [u8; 32]) {
        (self.checkpoint_generation, self.checkpoint_revision)
    }
    /// Consume the authenticated snapshot without cloning its signed head or
    /// potentially large runtime index.
    pub(crate) fn into_parts(self) -> (Vec<u8>, Vec<u8>, u64, [u8; 32], u64, [u8; 32]) {
        (
            self.committed.head_bytes,
            self.committed.index_bytes,
            self.committed.store_generation,
            self.committed.store_record_digest,
            self.checkpoint_generation,
            self.checkpoint_revision,
        )
    }
}
/// Persists governance artefacts on the filesystem for downstream ingestion.
#[derive(Debug)]
pub(crate) struct FilesystemGovernancePublisher {
    root: PathBuf,
    root_guard: GovernanceFilesystemRootGuard,
    publication_state_store: governance_rooted_fs::TwoSlotStoreV1,
    runtime_dag_signer: Option<GovernanceRuntimeDagSigner>,
    runtime_dag_checkpoint_store: Option<GovernanceRuntimeDagCheckpointStore>,
    fenced_privacy_publisher: Option<QualifiedFencedTransparencyPublisherV1>,
    fenced_privacy_head_reader: Option<QualifiedFencedTransparencyHeadReaderV1>,
    publication_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    runtime_dag_test_observed_timestamp: AtomicU64,
    _root_lock: File,
}
#[derive(Debug, Clone)]
/// Retained canonical root and platform-stable directory identities for fencing.
pub(crate) struct GovernanceFilesystemRootGuard {
    canonical_root: PathBuf,
    rooted_directory: governance_rooted_fs::RootedDirectory,
    #[cfg(unix)]
    ancestors: Vec<GovernanceFilesystemDirectoryIdentity>,
    #[cfg(unix)]
    effective_uid: u32,
    #[cfg(unix)]
    pinned_root_owner: u32,
    #[cfg(unix)]
    writer_root: bool,
}
#[cfg(unix)]
#[derive(Debug, Clone)]
struct GovernanceFilesystemDirectoryIdentity {
    path: PathBuf,
    handle: Arc<File>,
    device: u64,
    inode: u64,
    owner: u32,
    permissions: u32,
    is_root: bool,
}
#[derive(Clone)]
/// Startup-qualified signer pinned to one exact public provider policy.
pub(crate) struct GovernanceRuntimeDagSigner {
    handle: String,
    publisher_peer_id: Vec<u8>,
    public_key: [u8; 32],
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    verification_key: PublicKey,
    provider: Arc<dyn GovernanceDagRuntimeSigner>,
}
#[derive(Clone)]
/// Startup-qualified sealed store pinned to one exact public provider policy.
pub(crate) struct GovernanceRuntimeDagCheckpointStore {
    handle: String,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn GovernanceDagSealedCheckpointStore>,
}
/// Startup-qualified, identity-pinned fused privacy Governance publisher.
#[derive(Clone)]
pub struct QualifiedFencedTransparencyPublisherV1 {
    handle: String,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn FencedTransparencyPublisherV1>,
}
#[derive(Clone)]
/// Startup-qualified, identity-pinned authoritative privacy-head reader.
pub struct QualifiedFencedTransparencyHeadReaderV1 {
    handle: String,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1>,
}
#[derive(Debug)]
struct FencedPrivacyBoundaryFailure {
    error: GovernancePublishError,
    may_have_appended: bool,
}
impl fmt::Debug for QualifiedFencedTransparencyPublisherV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedFencedTransparencyPublisherV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("provider", &"<runtime-only>")
            .finish()
    }
}
impl fmt::Debug for QualifiedFencedTransparencyHeadReaderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedFencedTransparencyHeadReaderV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("provider", &"<runtime-only>")
            .finish()
    }
}
impl fmt::Debug for GovernanceRuntimeDagSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceRuntimeDagSigner")
            .field("handle", &self.handle)
            .field("publisher_peer_id", &self.publisher_peer_id)
            .field("public_key", &hex::encode(self.public_key))
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for GovernanceRuntimeDagCheckpointStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernanceRuntimeDagCheckpointStore")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("provider", &"<runtime-only>")
            .finish()
    }
}
impl FilesystemGovernancePublisher {
    /// Construct an unsigned base publisher for isolated tests.
    #[cfg(test)]
    pub(crate) fn try_new(root: PathBuf) -> io::Result<Self> {
        Self::try_new_with_publication_lock(root, Arc::new(Mutex::new(())))
    }
    /// Construct a publisher sharing its transaction fence with its owning node.
    pub(crate) fn try_new_with_publication_lock(
        root: PathBuf,
        publication_lock: Arc<Mutex<()>>,
    ) -> io::Result<Self> {
        validate_atomic_output_path(&root.join(".governance-root-probe"))?;
        fs::create_dir_all(&root)?;
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(&root)?;
        let root = root_guard.root().to_path_buf();
        validate_atomic_output_path(&root.join(".governance-root-probe"))?;
        let root_lock = acquire_governance_publisher_lock(&root)?;
        root_guard.revalidate()?;
        reject_governance_publication_recovery_quarantine(&root_guard).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("governance publication recovery is quarantined: {error}"),
            )
        })?;
        let (publication_state_store, marker_present) =
            initialize_governance_publication_authority_if_pristine(&root, &root_guard).map_err(
                |error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid governance publication initialization: {error}"),
                    )
                },
            )?;
        let (publication_state, _) = read_governance_publication_state(&publication_state_store)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid authoritative governance publication state: {error}"),
                )
            })?;
        reconcile_governance_publication_artifacts(&root_guard, &publication_state).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid governance publication artifact inventory: {error}"),
                )
            },
        )?;
        if !marker_present {
            write_governance_publication_initialization_marker(&root, &root_guard).map_err(
                |error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "failed to finalize governance publication initialization: {error}"
                        ),
                    )
                },
            )?;
        }
        Ok(Self {
            root,
            root_guard,
            publication_state_store,
            runtime_dag_signer: None,
            runtime_dag_checkpoint_store: None,
            fenced_privacy_publisher: None,
            fenced_privacy_head_reader: None,
            publication_lock,
            #[cfg(test)]
            runtime_dag_test_observed_timestamp: AtomicU64::new(u64::MAX),
            _root_lock: root_lock,
        })
    }
    /// Return the canonical filesystem root pinned by this publisher.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    /// Return the retained physical root/ancestor identity fence.
    pub(crate) fn root_guard(&self) -> &GovernanceFilesystemRootGuard {
        &self.root_guard
    }
    /// Atomically attach and reconcile the signed-producer runtime providers.
    ///
    /// Neither provider is installed until both qualifications and the sealed
    /// recovery transaction succeed. This keeps partial crash state recoverable:
    /// the signer never performs a standalone filesystem audit before the
    /// checkpoint store can replay its exact write-ahead intent.
    pub(crate) fn with_qualified_runtime_dag_providers(
        mut self,
        signer: GovernanceRuntimeDagSigner,
        checkpoint_store: GovernanceRuntimeDagCheckpointStore,
    ) -> Result<Self, GovernancePublishError> {
        if self.runtime_dag_signer.is_some() || self.runtime_dag_checkpoint_store.is_some() {
            self.transition_qualified_runtime_dag_providers(signer, checkpoint_store)?;
            return Ok(self);
        }
        self.root_guard.revalidate()?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        reconcile_runtime_dag_producer_state(
            &self.root,
            &self.root_guard,
            &signer,
            &checkpoint_store,
        )?;
        self.root_guard.revalidate()?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        self.runtime_dag_signer = Some(signer);
        self.runtime_dag_checkpoint_store = Some(checkpoint_store);
        Ok(self)
    }
    /// Authenticate and install one explicit signer/store qualification rotation.
    ///
    /// Every change advances one canonical authority segment. Both outgoing and
    /// incoming HSM authorities sign the exact predecessor/current-head
    /// transition, so signer-key or publisher-identity rotation remains
    /// continuous with the already retained block chain.
    pub(crate) fn transition_qualified_runtime_dag_providers(
        &mut self,
        next_signer: GovernanceRuntimeDagSigner,
        next_store: GovernanceRuntimeDagCheckpointStore,
    ) -> Result<(), GovernancePublishError> {
        let previous_signer = self.runtime_dag_signer.clone().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG provider rotation requires an installed predecessor signer",
            )
        })?;
        let previous_store = self.runtime_dag_checkpoint_store.clone().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG provider rotation requires an installed predecessor checkpoint store",
            )
        })?;
        let publication_lock = Arc::clone(&self.publication_lock);
        let _publication_guard = publication_lock.lock().map_err(|_| {
            GovernancePublishError::other(
                "filesystem governance publisher transaction lock is poisoned",
            )
        })?;
        self.root_guard.revalidate()?;
        previous_signer.assert_qualification()?;
        previous_store.assert_qualification()?;
        next_signer.assert_qualification()?;
        next_store.assert_qualification()?;
        let previous_binding = runtime_dag_provider_binding(&previous_signer, &previous_store);
        let next_binding = runtime_dag_provider_binding(&next_signer, &next_store);
        if previous_binding == next_binding {
            self.runtime_dag_signer = Some(next_signer);
            self.runtime_dag_checkpoint_store = Some(next_store);
            return Ok(());
        }
        if let Some((history, _)) =
            read_runtime_dag_qualification_history(&self.root, &self.root_guard, None)?
        {
            let tail =
                runtime_dag_history_tail_transition(&self.root, &history)?.ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG qualification history has no authenticated tail",
                    )
                })?;
            if tail.body.next == next_binding && tail.body.previous == previous_binding {
                let should_compact =
                    history.transitions.len() >= GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1;
                install_runtime_dag_provider_transition(
                    &self.root,
                    &self.root_guard,
                    &next_signer,
                    &next_store,
                    &tail,
                )?;
                self.runtime_dag_signer = Some(next_signer);
                self.runtime_dag_checkpoint_store = Some(next_store);
                drop(_publication_guard);
                if should_compact {
                    self.compact_runtime_dag_qualification_history(
                        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1 / 2,
                    )?;
                }
                return Ok(());
            }
            if tail.body.next != previous_binding {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG provider rotation does not extend the authenticated qualification tail",
                ));
            }
        }
        reconcile_runtime_dag_producer_state(
            &self.root,
            &self.root_guard,
            &previous_signer,
            &previous_store,
        )?;
        let predecessor_record = previous_store
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG provider rotation has no sealed predecessor checkpoint",
                )
            })?;
        let predecessor = decode_runtime_dag_producer_checkpoint_record(
            &predecessor_record,
            &self.root,
            &previous_signer,
            &previous_store,
        )?;
        let (predecessor_index_digest, successor_index_digest, _) =
            canonical_runtime_dag_index_for_transition(
                &self.root,
                &self.root_guard,
                &previous_binding,
                &next_binding,
                &predecessor,
            )?;
        let existing = read_runtime_dag_qualification_history(
            &self.root,
            &self.root_guard,
            Some(&previous_binding),
        )?;
        let (mut history, summary, predecessor_history) = match existing {
            Some((history, summary)) => {
                let predecessor = history.clone();
                (history, summary, Some(predecessor))
            }
            None => (
                RuntimeDagQualificationHistoryV1 {
                    version: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_VERSION_V1,
                    root_digest: runtime_dag_producer_root_digest(&self.root)?,
                    archive_generation: 0,
                    archive_digest: [0; 32],
                    archived_through_generation: 0,
                    archive_tail_transition_digest: [0; 32],
                    transitions: Vec::new(),
                },
                RuntimeDagQualificationSummary::EMPTY,
                None,
            ),
        };
        if history.transitions.len() >= GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1 {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification history must be compacted before another provider rotation",
            ));
        }
        let generation = summary
            .transition_generation
            .checked_add(1)
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG qualification transition generation exhausted",
                )
            })?;
        let body = RuntimeDagQualificationTransitionBodyV1 {
            version: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TRANSITION_VERSION_V1,
            root_digest: predecessor.root_digest,
            generation,
            predecessor_transition_digest: (summary.transition_generation != 0)
                .then_some(summary.transition_digest),
            predecessor_checkpoint_revision: predecessor_record.revision,
            previous: previous_binding,
            next: next_binding.clone(),
            block_count: predecessor.block_count,
            head_block_cid: predecessor.head_block_cid,
            head_bytes_digest: predecessor.head_bytes_digest,
            predecessor_index_digest,
            successor_index_digest,
            archive_generation: summary.archive_generation,
            archive_digest: summary.archive_digest,
        };
        let outgoing_segment_revision = generation;
        let incoming_segment_revision = generation.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG key-transition segment revision exhausted",
            )
        })?;
        let transition_body_digest = runtime_dag_transition_body_digest(&body)?;
        let signing_bytes = governance_dag_key_transition_signing_payload_v1(
            outgoing_segment_revision,
            incoming_segment_revision,
            transition_body_digest,
        )?;
        let transition = RuntimeDagQualificationTransitionV1 {
            body,
            key_transition: RuntimeDagKeyTransitionEnvelopeV1 {
                version: GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1,
                outgoing_segment_revision,
                incoming_segment_revision,
                transition_body_digest,
                outgoing_signature: runtime_dag_raw_signature(
                    &previous_signer,
                    GovernanceDagSigningPurposeV1::KeyTransition,
                    &signing_bytes,
                )?,
                incoming_signature: runtime_dag_raw_signature(
                    &next_signer,
                    GovernanceDagSigningPurposeV1::KeyTransition,
                    &signing_bytes,
                )?,
            },
        };
        validate_runtime_dag_qualification_transition(
            &transition,
            runtime_dag_producer_root_digest(&self.root)?,
        )?;
        history.transitions.push(transition.clone());
        let should_compact =
            history.transitions.len() >= GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1;
        write_runtime_dag_qualification_history(
            &self.root,
            &self.root_guard,
            &history,
            &next_binding,
            predecessor_history.as_ref(),
        )?;
        install_runtime_dag_provider_transition(
            &self.root,
            &self.root_guard,
            &next_signer,
            &next_store,
            &transition,
        )?;
        self.runtime_dag_signer = Some(next_signer);
        self.runtime_dag_checkpoint_store = Some(next_store);
        drop(_publication_guard);
        if should_compact {
            self.compact_runtime_dag_qualification_history(
                GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1 / 2,
            )?;
        }
        Ok(())
    }
    /// Archive an authenticated prefix and retain at most `retain_latest`
    /// live provider transitions.
    ///
    /// The immutable archive is installed and read back first, then its digest
    /// is advanced through sealed monotonic CAS. Only after that readback may
    /// the live journal prefix be pruned.
    pub(crate) fn compact_runtime_dag_qualification_history(
        &self,
        retain_latest: usize,
    ) -> Result<usize, GovernancePublishError> {
        let signer = self.runtime_dag_signer.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification compaction requires an installed signer",
            )
        })?;
        let store = self.runtime_dag_checkpoint_store.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification compaction requires an installed checkpoint store",
            )
        })?;
        let _publication_guard = self.lock_publication()?;
        reconcile_runtime_dag_producer_state(&self.root, &self.root_guard, signer, store)?;
        let binding = runtime_dag_provider_binding(signer, store);
        let Some((mut history, summary)) =
            read_runtime_dag_qualification_history(&self.root, &self.root_guard, Some(&binding))?
        else {
            return Ok(0);
        };
        if history.transitions.len() <= retain_latest {
            return Ok(0);
        }
        if history.archive_generation >= GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1 {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive chain reached its V1 bound",
            ));
        }
        let previous_record = store
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG qualification compaction has no sealed checkpoint",
                )
            })?;
        let previous = decode_runtime_dag_producer_checkpoint_record(
            &previous_record,
            &self.root,
            signer,
            store,
        )?;
        if previous.qualification_transition_generation != summary.transition_generation
            || previous.qualification_transition_digest != summary.transition_digest
            || previous.qualification_archive_generation != summary.archive_generation
            || previous.qualification_archive_digest != summary.archive_digest
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification compaction predecessor changed",
            ));
        }
        let prune_count = history
            .transitions
            .len()
            .saturating_sub(retain_latest)
            .min(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_TRANSITIONS_V1);
        let transitions = history.transitions[..prune_count].to_vec();
        let first_transition_generation = transitions
            .first()
            .expect("positive prune count")
            .body
            .generation;
        let last_transition_generation = transitions
            .last()
            .expect("positive prune count")
            .body
            .generation;
        let tail_transition_digest =
            runtime_dag_transition_digest(transitions.last().expect("positive prune count"))?;
        let archive_generation = history.archive_generation.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive generation exhausted",
            )
        })?;
        let body = RuntimeDagQualificationArchiveBodyV1 {
            version: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_VERSION_V1,
            root_digest: history.root_digest,
            archive_generation,
            predecessor_archive_digest: history.archive_digest,
            predecessor_transition_digest: history.archive_tail_transition_digest,
            first_transition_generation,
            last_transition_generation,
            tail_transition_digest,
            signer: binding.clone(),
            transitions,
        };
        let archive = RuntimeDagQualificationArchiveV1 {
            signature: runtime_dag_raw_signature(
                signer,
                GovernanceDagSigningPurposeV1::QualificationArchive,
                &runtime_dag_archive_signing_bytes(&body)?,
            )?,
            body,
        };
        let archive_digest =
            validate_runtime_dag_qualification_archive(&archive, history.root_digest)?;
        let archive_path =
            runtime_dag_qualification_archive_path(&self.root, archive_generation, archive_digest);
        write_runtime_dag_qualification_state(
            &self.root_guard,
            &archive_path,
            &archive,
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_BYTES_V1,
            true,
        )?;
        let archive_readback = read_runtime_dag_qualification_archive(
            &self.root,
            archive_generation,
            archive_digest,
            history.root_digest,
        )?;
        if archive_readback != archive {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive durable readback diverged",
            ));
        }
        let mut next = previous;
        next.qualification_archive_generation = archive_generation;
        next.qualification_archive_digest = archive_digest;
        let next_record = runtime_dag_producer_checkpoint_record(&next)?;
        store.compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            Some(previous_record.revision),
            next_record.clone(),
        )?;
        if store
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
            .as_ref()
            != Some(&next_record)
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive checkpoint readback diverged",
            ));
        }
        let predecessor_history = history.clone();
        history.transitions.drain(..prune_count);
        history.archive_generation = archive_generation;
        history.archive_digest = archive_digest;
        history.archived_through_generation = last_transition_generation;
        history.archive_tail_transition_digest = tail_transition_digest;
        write_runtime_dag_qualification_history(
            &self.root,
            &self.root_guard,
            &history,
            &binding,
            Some(&predecessor_history),
        )?;
        reconcile_runtime_dag_producer_state(&self.root, &self.root_guard, signer, store)?;
        Ok(prune_count)
    }
    /// Attach an already-qualified fused privacy publication provider.
    pub(crate) fn with_qualified_fenced_privacy_publisher(
        mut self,
        publisher: QualifiedFencedTransparencyPublisherV1,
    ) -> Result<Self, GovernancePublishError> {
        publisher.assert_qualification()?;
        if let Some(reader) = &self.fenced_privacy_head_reader {
            ensure_fenced_privacy_runtime_bindings_match(&publisher, reader)?;
        }
        self.fenced_privacy_publisher = Some(publisher);
        Ok(self)
    }
    /// Attach an already-qualified reader and bootstrap the local head cache.
    ///
    /// Standard node, Torii, and daemon launchers resolve this reader alongside
    /// the fused writer before enabling privacy publication. No production path
    /// may infer genesis from an empty root.
    pub(crate) fn with_qualified_fenced_privacy_head_reader(
        mut self,
        reader: QualifiedFencedTransparencyHeadReaderV1,
    ) -> Result<Self, GovernancePublishError> {
        reader.assert_qualification()?;
        if let Some(publisher) = &self.fenced_privacy_publisher {
            ensure_fenced_privacy_runtime_bindings_match(publisher, &reader)?;
        }
        {
            let _publication_guard = self.lock_publication()?;
            synchronize_fenced_privacy_authoritative_head(&self.root, &reader, None)?;
        }
        self.fenced_privacy_head_reader = Some(reader);
        Ok(self)
    }
    fn record_publish_index(
        &self,
        payload_kind: &str,
        encoded: &[u8],
        json_bytes: &[u8],
        labels: JsonMap,
    ) -> Result<(PathBuf, PathBuf), GovernancePublishError> {
        reject_governance_publication_recovery_quarantine(&self.root_guard)?;
        reject_legacy_governance_publication_authorities(&self.root, &self.root_guard)?;
        validate_governance_car_source_lengths(encoded.len(), json_bytes.len())?;
        let digest_hex = blake3::hash(encoded).to_hex().to_string();
        let json_blake3 = blake3::hash(json_bytes).to_hex().to_string();
        let encoded_len_u64 = u64::try_from(encoded.len()).map_err(|_| {
            GovernancePublishError::other("governance encoded source length exceeds u64")
        })?;
        let json_len_u64 = u64::try_from(json_bytes.len()).map_err(|_| {
            GovernancePublishError::other("governance JSON source length exceeds u64")
        })?;
        let (encoded_relative, json_relative) = governance_source_pair_relative_paths(
            payload_kind,
            encoded_len_u64,
            &digest_hex,
            json_len_u64,
            &json_blake3,
        )?;
        let encoded_path = resolve_index_path(&self.root, &encoded_relative)?;
        let json_path = resolve_index_path(&self.root, &json_relative)?;
        let (mut publication_state, publication_snapshot) =
            read_governance_publication_state(&self.publication_state_store)?;
        let publish_index = match publication_state.remove("publish_index") {
            Some(JsonValue::Object(index)) => index,
            _ => {
                return Err(GovernancePublishError::other(
                    "governance publication state is missing its publish index",
                ));
            }
        };
        let car_queue = match publication_state.remove("car_queue") {
            Some(JsonValue::Object(queue)) => queue,
            _ => {
                return Err(GovernancePublishError::other(
                    "governance publication state is missing its CAR queue",
                ));
            }
        };
        let (publish_index, entry) = update_publish_index(
            &self.root,
            publish_index,
            payload_kind,
            &encoded_path,
            &json_path,
            &digest_hex,
            encoded.len(),
            &json_blake3,
            json_bytes.len(),
            labels,
        )?;
        if !entry.newly_inserted {
            let persisted = persist_governance_source_pair(
                &self.root,
                &self.root_guard,
                payload_kind,
                encoded,
                json_bytes,
            )?;
            if persisted != (encoded_path.clone(), json_path.clone()) {
                return Err(GovernancePublishError::other(
                    "persisted duplicate governance source pair diverged from its identity",
                ));
            }
            let mut canonical_segment =
                assemble_governance_car_segment(&self.root, &self.root_guard, &entry)?;
            canonical_segment.insert(
                "queue_position".into(),
                JsonValue::from(u64::try_from(entry.position).map_err(|_| {
                    GovernancePublishError::other("duplicate CAR queue position exceeds u64")
                })?),
            );
            let existing_segment = car_queue
                .get("segments")
                .and_then(JsonValue::as_array)
                .and_then(|segments| segments.get(entry.position))
                .and_then(JsonValue::as_object)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "duplicate governance publication lost its committed CAR segment",
                    )
                })?;
            if existing_segment != &canonical_segment {
                return Err(GovernancePublishError::other(
                    "duplicate governance publication CAR segment diverges from its canonical immutable artifacts",
                ));
            }
            return Ok((encoded_path, json_path));
        }
        let (car_files, car_file_records) =
            governance_car_segment_files_from_source_bytes(&entry, encoded, json_bytes)?;
        let prepared_segment =
            prepare_governance_car_segment(&self.root, &entry, car_files, car_file_records)?;
        let car_queue =
            install_governance_car_segment(car_queue, &entry, prepared_segment.segment.clone())?;
        publication_state.insert("publish_index".into(), JsonValue::Object(publish_index));
        publication_state.insert("car_queue".into(), JsonValue::Object(car_queue));
        let prepared_state = prepare_governance_publication_state(publication_state)?;
        let persistence = (|| -> Result<(), GovernancePublishError> {
            let persisted = persist_governance_source_pair(
                &self.root,
                &self.root_guard,
                payload_kind,
                encoded,
                json_bytes,
            )?;
            if persisted != (encoded_path.clone(), json_path.clone()) {
                return Err(GovernancePublishError::other(
                    "persisted governance source pair diverged from its preflight identity",
                ));
            }
            persist_prepared_governance_car_segment(&self.root_guard, &prepared_segment)?;
            write_prepared_governance_publication_state(
                &self.publication_state_store,
                &publication_snapshot,
                &prepared_state,
            )
            .map(drop)
        })();
        if let Err(error) = persistence {
            if let Err(reconcile_error) = reconcile_current_governance_publication_artifacts(
                &self.root_guard,
                &self.publication_state_store,
            ) {
                return Err(GovernancePublishError::other(format!(
                    "governance publication failed ({error}); bounded orphan reconciliation also failed ({reconcile_error})"
                )));
            }
            return Err(error);
        }
        Ok((encoded_path, json_path))
    }
    fn record_runtime_signed_payload(
        &self,
        payload_kind: &str,
        payload: GovernanceLogPayloadV1,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
    ) -> Result<(), GovernancePublishError> {
        self.record_runtime_signed_payload_with_provenance(
            payload_kind,
            payload,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn record_runtime_signed_payload_with_provenance(
        &self,
        payload_kind: &str,
        payload: GovernanceLogPayloadV1,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let Some(signer) = &self.runtime_dag_signer else {
            return Ok(());
        };
        let checkpoint_store = self.runtime_dag_checkpoint_store.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "signed governance runtime DAG publication requires a sealed producer checkpoint store",
            )
        })?;
        let observed_timestamp = current_unix_timestamp_seconds();
        #[cfg(test)]
        let observed_timestamp = {
            let test_timestamp = self
                .runtime_dag_test_observed_timestamp
                .load(Ordering::SeqCst);
            if test_timestamp == u64::MAX {
                observed_timestamp
            } else {
                test_timestamp
            }
        };
        let provenance = provenance.map(GovernanceSubmissionProvenanceV1::to_dag_provenance);
        append_runtime_signed_dag_payload(
            &self.root,
            &self.root_guard,
            signer,
            checkpoint_store,
            payload_kind,
            payload,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
            observed_timestamp,
            provenance,
        )
    }
    fn preflight_runtime_signed_payload(
        &self,
        payload: &GovernanceLogPayloadV1,
        source_payload_len: usize,
    ) -> Result<(), GovernancePublishError> {
        let Some(signer) = &self.runtime_dag_signer else {
            return Ok(());
        };
        let checkpoint_store = self.runtime_dag_checkpoint_store.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "signed governance runtime DAG publication requires a sealed producer checkpoint store",
            )
        })?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        preflight_runtime_signed_dag_payload(payload, source_payload_len)
    }
    fn preflight_runtime_signed_payload_with_provenance(
        &self,
        payload: &GovernanceLogPayloadV1,
        source_payload_len: usize,
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let provenance = provenance.map(GovernanceSubmissionProvenanceV1::to_dag_provenance);
        payload
            .validate_submission_provenance(provenance.as_ref())
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "invalid authenticated governance submission provenance: {error}"
                ))
            })?;
        let Some(signer) = &self.runtime_dag_signer else {
            return Ok(());
        };
        let checkpoint_store = self.runtime_dag_checkpoint_store.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "signed governance runtime DAG publication requires a sealed producer checkpoint store",
            )
        })?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        preflight_runtime_signed_dag_payload(payload, source_payload_len)
    }
    #[cfg(test)]
    fn set_runtime_dag_observed_timestamp_for_test(&self, timestamp: u64) {
        self.runtime_dag_test_observed_timestamp
            .store(timestamp, Ordering::SeqCst);
    }
    fn lock_publication(&self) -> Result<MutexGuard<'_, ()>, GovernancePublishError> {
        self.root_guard.revalidate()?;
        let guard = self.publication_lock.lock().map_err(|_| {
            GovernancePublishError::other(
                "filesystem governance publisher transaction lock is poisoned",
            )
        })?;
        self.root_guard.revalidate()?;
        Ok(guard)
    }
}
fn acquire_governance_publisher_lock(root: &Path) -> io::Result<File> {
    let lock_path = root.join(GOVERNANCE_PUBLISHER_LOCK_FILE);
    validate_atomic_output_path(&lock_path)?;
    let before_open = match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => {
            validate_governance_lock_metadata(&lock_path, &metadata)?;
            Some(metadata)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    set_no_follow_flag(&mut options);
    let file = options.open(&lock_path)?;
    let opened_metadata = file.metadata()?;
    validate_governance_lock_metadata(&lock_path, &opened_metadata)?;
    if before_open
        .as_ref()
        .is_some_and(|metadata| !metadata_identifies_same_file(metadata, &opened_metadata))
    {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` changed between inspection and open",
            lock_path.display()
        )));
    }
    let after_open = fs::symlink_metadata(&lock_path)?;
    validate_governance_lock_metadata(&lock_path, &after_open)?;
    if !metadata_identifies_same_file(&opened_metadata, &after_open) {
        return Err(io::Error::other(format!(
            "governance publisher lock path `{}` changed while opening",
            lock_path.display()
        )));
    }
    validate_atomic_output_path(&lock_path)?;
    match file.try_lock() {
        Ok(()) => {
            let locked_path_metadata = fs::symlink_metadata(&lock_path)?;
            validate_governance_lock_metadata(&lock_path, &locked_path_metadata)?;
            if !metadata_identifies_same_file(&opened_metadata, &locked_path_metadata) {
                return Err(io::Error::other(format!(
                    "governance publisher lock path `{}` changed while locking",
                    lock_path.display()
                )));
            }
            validate_atomic_output_path(&lock_path)?;
            Ok(file)
        }
        Err(fs::TryLockError::WouldBlock) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "governance publisher directory is already in use: {}",
                root.display()
            ),
        )),
        Err(fs::TryLockError::Error(err)) => Err(io::Error::new(
            err.kind(),
            format!(
                "failed to lock governance publisher directory via `{}`: {err}",
                lock_path.display()
            ),
        )),
    }
}
fn validate_governance_lock_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` must have exactly one hard link",
            path.display()
        )));
    }
    Ok(())
}
impl GovernanceFilesystemRootGuard {
    /// Capture a root which this process is authorized to mutate.
    pub(crate) fn capture_writer(root: &Path) -> io::Result<Self> {
        Self::capture_with_role(root, true)
    }
    /// Capture a read-only producer root and pin its distinct owner identity.
    pub(crate) fn capture_source(root: &Path) -> io::Result<Self> {
        Self::capture_with_role(root, false)
    }
    fn capture_with_role(root: &Path, writer_root: bool) -> io::Result<Self> {
        #[cfg(unix)]
        let lexical_root = governance_absolute_lexical_root(root)?;
        #[cfg(windows)]
        let lexical_root = governance_absolute_lexical_root(root)?;
        #[cfg(not(any(unix, windows)))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Governance DAG filesystem roots are unsupported on this platform",
        ));
        let root_metadata = fs::symlink_metadata(&lexical_root)?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(io::Error::other(format!(
                "governance filesystem root `{}` must be a real directory",
                lexical_root.display()
            )));
        }
        #[cfg(windows)]
        let rooted_directory =
            governance_rooted_fs::RootedDirectory::open_root(&lexical_root, writer_root)?;
        #[cfg(windows)]
        rooted_directory.validate_acl()?;
        let canonical_root = fs::canonicalize(&lexical_root)?;
        #[cfg(unix)]
        if canonical_root != lexical_root {
            return Err(io::Error::other(format!(
                "governance filesystem root `{}` is not an exact canonical path; symlinked or aliased ancestors are forbidden",
                lexical_root.display()
            )));
        }
        #[cfg(unix)]
        let (ancestors, effective_uid, pinned_root_owner) = {
            let mut paths = canonical_root.ancestors().collect::<Vec<_>>();
            paths.reverse();
            let effective_uid = unsafe { geteuid() };
            let pinned_root_owner = if writer_root {
                effective_uid
            } else {
                root_metadata.uid()
            };
            let last = paths.len().saturating_sub(1);
            let ancestors = paths
                .into_iter()
                .enumerate()
                .map(|(position, path)| {
                    let is_root = position == last;
                    let metadata = fs::symlink_metadata(path)?;
                    validate_governance_directory_policy(
                        path,
                        &metadata,
                        is_root,
                        effective_uid,
                        pinned_root_owner,
                        writer_root,
                    )?;
                    let mut options = fs::OpenOptions::new();
                    options.read(true);
                    set_directory_no_follow_flags(&mut options);
                    let handle = Arc::new(options.open(path).map_err(|err| {
                        io::Error::new(
                            err.kind(),
                            format!(
                                "failed to retain governance filesystem directory `{}`: {err}",
                                path.display()
                            ),
                        )
                    })?);
                    let opened_metadata = handle.metadata()?;
                    validate_governance_directory_policy(
                        path,
                        &opened_metadata,
                        is_root,
                        effective_uid,
                        pinned_root_owner,
                        writer_root,
                    )?;
                    governance_rooted_fs::validate_retained_directory_acl(&handle, path)?;
                    if !metadata_identifies_same_file(&metadata, &opened_metadata) {
                        return Err(io::Error::other(format!(
                            "governance filesystem ancestor `{}` changed between inspection and directory-only open",
                            path.display()
                        )));
                    }
                    Ok(GovernanceFilesystemDirectoryIdentity {
                        path: path.to_path_buf(),
                        handle,
                        device: metadata.dev(),
                        inode: metadata.ino(),
                        owner: metadata.uid(),
                        permissions: metadata.permissions().mode() & 0o7777,
                        is_root,
                    })
                })
                .collect::<io::Result<Vec<_>>>()?;
            (ancestors, effective_uid, pinned_root_owner)
        };
        #[cfg(unix)]
        let rooted_directory = governance_rooted_fs::RootedDirectory::from_retained(
            canonical_root.clone(),
            Arc::clone(
                &ancestors
                    .last()
                    .ok_or_else(|| io::Error::other("governance root has no retained ancestor"))?
                    .handle,
            ),
            writer_root,
        )?;
        let guard = Self {
            canonical_root,
            rooted_directory,
            #[cfg(unix)]
            ancestors,
            #[cfg(unix)]
            effective_uid,
            #[cfg(unix)]
            pinned_root_owner,
            #[cfg(unix)]
            writer_root,
        };
        guard.revalidate()?;
        Ok(guard)
    }
    /// Return the exact canonical root bound by this guard.
    pub(crate) fn root(&self) -> &Path {
        &self.canonical_root
    }
    pub(crate) fn rooted_directory(&self) -> &governance_rooted_fs::RootedDirectory {
        &self.rooted_directory
    }
    /// Return a path-free digest of the retained physical root identity.
    pub(crate) fn identity_digest(&self) -> io::Result<[u8; 32]> {
        self.revalidate()?;
        let digest = self.rooted_directory.identity_digest()?;
        self.revalidate()?;
        Ok(digest)
    }
    /// Revalidate every retained ancestor and root identity.
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            let effective_uid = unsafe { geteuid() };
            if effective_uid != self.effective_uid {
                return Err(io::Error::other(format!(
                    "governance filesystem root `{}` effective user changed from {} to {}",
                    self.canonical_root.display(),
                    self.effective_uid,
                    effective_uid
                )));
            }
            for identity in &self.ancestors {
                revalidate_governance_directory_identity(
                    identity,
                    effective_uid,
                    self.pinned_root_owner,
                    self.writer_root,
                )?;
            }
            for identity in self.ancestors.iter().rev() {
                revalidate_governance_directory_identity(
                    identity,
                    effective_uid,
                    self.pinned_root_owner,
                    self.writer_root,
                )?;
            }
        }
        #[cfg(windows)]
        {
            self.rooted_directory.validate_acl()?;
            let metadata = fs::symlink_metadata(&self.canonical_root)?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.file_attributes() & 0x0000_0400 != 0
                || metadata.volume_serial_number().is_none()
                || metadata.file_index().is_none()
            {
                return Err(io::Error::other(format!(
                    "governance filesystem root `{}` is no longer a stable, non-reparse directory",
                    self.canonical_root.display()
                )));
            }
        }
        #[cfg(not(any(unix, windows)))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Governance DAG filesystem roots are unsupported on this platform",
        ));
        self.rooted_directory
            .verify_path_binding(&self.canonical_root)?;
        Ok(())
    }
}
#[cfg(unix)]
fn revalidate_governance_directory_identity(
    identity: &GovernanceFilesystemDirectoryIdentity,
    effective_uid: u32,
    pinned_root_owner: u32,
    writer_root: bool,
) -> io::Result<()> {
    let handle_metadata = identity.handle.metadata()?;
    validate_governance_directory_policy(
        &identity.path,
        &handle_metadata,
        identity.is_root,
        effective_uid,
        pinned_root_owner,
        writer_root,
    )?;
    governance_rooted_fs::validate_retained_directory_acl(&identity.handle, &identity.path)?;
    if handle_metadata.dev() != identity.device
        || handle_metadata.ino() != identity.inode
        || handle_metadata.uid() != identity.owner
        || handle_metadata.permissions().mode() & 0o7777 != identity.permissions
    {
        return Err(io::Error::other(format!(
            "retained governance filesystem ancestor `{}` changed identity, owner, or mode",
            identity.path.display()
        )));
    }
    let path_metadata = fs::symlink_metadata(&identity.path)?;
    validate_governance_directory_policy(
        &identity.path,
        &path_metadata,
        identity.is_root,
        effective_uid,
        pinned_root_owner,
        writer_root,
    )?;
    if !metadata_identifies_same_file(&handle_metadata, &path_metadata)
        || path_metadata.uid() != identity.owner
        || path_metadata.permissions().mode() & 0o7777 != identity.permissions
    {
        return Err(io::Error::other(format!(
            "governance filesystem ancestor path `{}` changed identity, owner, or mode",
            identity.path.display()
        )));
    }
    governance_rooted_fs::validate_retained_directory_acl(&identity.handle, &identity.path)?;
    Ok(())
}
#[cfg(unix)]
fn validate_governance_directory_policy(
    path: &Path,
    metadata: &fs::Metadata,
    is_root: bool,
    effective_uid: u32,
    pinned_root_owner: u32,
    writer_root: bool,
) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "governance filesystem ancestor `{}` must be a real directory",
            path.display()
        )));
    }
    let owner = metadata.uid();
    let permissions = metadata.permissions().mode() & 0o7777;
    if governance_directory_policy_accepts(
        owner,
        permissions,
        is_root,
        effective_uid,
        pinned_root_owner,
        writer_root,
    ) {
        return Ok(());
    }
    if is_root {
        let expected_owner = if writer_root {
            effective_uid
        } else {
            pinned_root_owner
        };
        return Err(io::Error::other(format!(
            "governance filesystem root `{}` must be owned by UID {} and must not be group/world writable",
            path.display(),
            expected_owner
        )));
    }
    Err(io::Error::other(format!(
        "governance filesystem ancestor `{}` must have a trusted owner and may be group/world writable only as a sticky trusted parent",
        path.display()
    )))
}
#[cfg(unix)]
fn governance_directory_policy_accepts(
    owner: u32,
    permissions: u32,
    is_root: bool,
    effective_uid: u32,
    pinned_root_owner: u32,
    writer_root: bool,
) -> bool {
    if is_root {
        let expected_owner = if writer_root {
            effective_uid
        } else {
            pinned_root_owner
        };
        return owner == expected_owner && permissions & 0o022 == 0;
    }
    let trusted_owner = owner == 0 || owner == effective_uid || owner == pinned_root_owner;
    let writable = permissions & 0o022 != 0;
    trusted_owner && (!writable || permissions & 0o1000 != 0)
}
#[cfg(any(unix, windows))]
fn governance_absolute_lexical_root(root: &Path) -> io::Result<PathBuf> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    if absolute.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(io::Error::other(format!(
            "governance filesystem root `{}` must not contain `.` or `..` components",
            root.display()
        )));
    }
    Ok(absolute)
}
#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}
#[cfg(not(any(unix, windows)))]
fn metadata_identifies_same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
fn status_label(status: DealSettlementStatusV1) -> &'static str {
    match status {
        DealSettlementStatusV1::WindowSettled => "window_settled",
        DealSettlementStatusV1::Completed => "completed",
        DealSettlementStatusV1::Cancelled => "cancelled",
        DealSettlementStatusV1::Defaulted => "defaulted",
    }
}
fn pdp_decision_label(decision: PdpTerminalDecisionV1) -> &'static str {
    match decision {
        PdpTerminalDecisionV1::Accepted => "accepted",
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired) => {
            "rejected_deadline_expired"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::SubmissionLate) => {
            "rejected_submission_late"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::FutureTimestamp) => {
            "rejected_future_timestamp"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof) => {
            "rejected_invalid_proof"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::AdmissionRevoked) => {
            "rejected_admission_revoked"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::AdmissionInactive) => {
            "rejected_admission_inactive"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::StorageUnavailable) => {
            "rejected_storage_unavailable"
        }
    }
}
fn governance_two_slot_label_digest_v1(kind: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.local-two-slot.binding-label.v1\0");
    hasher.update(
        &u64::try_from(kind.len())
            .expect("fixed governance two-slot label kind fits u64")
            .to_le_bytes(),
    );
    hasher.update(kind);
    hasher.update(
        &u64::try_from(value.len())
            .expect("fixed governance two-slot label value fits u64")
            .to_le_bytes(),
    );
    hasher.update(value);
    *hasher.finalize().as_bytes()
}
fn governance_two_slot_config_v1(
    spec: GovernanceTwoSlotStoreSpecV1,
) -> Result<governance_rooted_fs::TwoSlotStoreConfigV1, GovernancePublishError> {
    governance_rooted_fs::TwoSlotStoreConfigV1::try_new(
        spec.directory_name,
        governance_two_slot_label_digest_v1(b"domain", spec.semantic_domain),
        governance_two_slot_label_digest_v1(b"stable-caller-nonce", spec.stable_nonce),
        spec.max_payload_bytes,
    )
    .map_err(Into::into)
}
fn open_governance_two_slot_store_v1(
    root_guard: &GovernanceFilesystemRootGuard,
    spec: GovernanceTwoSlotStoreSpecV1,
    initial_payload: &[u8],
) -> Result<governance_rooted_fs::TwoSlotStoreV1, GovernancePublishError> {
    root_guard.revalidate()?;
    let store = root_guard
        .rooted_directory()
        .open_or_create_two_slot_store_v1(governance_two_slot_config_v1(spec)?, initial_payload)?;
    root_guard.revalidate()?;
    Ok(store)
}
fn load_governance_two_slot_store_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
    label: &str,
) -> Result<governance_rooted_fs::TwoSlotSnapshotV1, GovernancePublishError> {
    store.load().map_err(|error| {
        GovernancePublishError::other(format!("failed to load {label} two-slot state: {error}"))
    })
}
fn compare_and_swap_governance_two_slot_store_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
    expected: &governance_rooted_fs::TwoSlotSnapshotV1,
    payload: &[u8],
    label: &str,
) -> Result<governance_rooted_fs::TwoSlotSnapshotV1, GovernancePublishError> {
    store.compare_and_swap(expected, payload).map_err(|error| {
        GovernancePublishError::other(format!("failed to commit {label} two-slot state: {error}"))
    })
}
fn encode_governance_two_slot_value_v1<T: norito::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, GovernancePublishError> {
    norito::to_bytes(value).map_err(|error| {
        GovernancePublishError::other(format!("failed to encode {label}: {error}"))
    })
}
fn decode_governance_two_slot_value_v1<T>(
    snapshot: &governance_rooted_fs::TwoSlotSnapshotV1,
    label: &str,
) -> Result<T, GovernancePublishError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    decode_canonical_runtime_dag(snapshot.payload(), label)
}
#[cfg(test)]
fn write_atomic(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> io::Result<()> {
    write_rooted_atomic(root_guard, path, data)
}
fn write_immutable_governance_file(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
    max_bytes: usize,
) -> Result<(), GovernancePublishError> {
    if data.is_empty() || data.len() > max_bytes {
        return Err(GovernancePublishError::other(format!(
            "immutable governance artifact `{}` is outside its {max_bytes}-byte bound",
            path.display()
        )));
    }
    match read_rooted_governance_state_file(root_guard, path, max_bytes) {
        Ok(snapshot) => {
            if snapshot.bytes() != data {
                return Err(GovernancePublishError::other(format!(
                    "immutable governance artifact path `{}` is already occupied by different bytes",
                    path.display()
                )));
            }
            snapshot.binding().verify()?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if let Err(write_error) = write_rooted_atomic_expected(
                root_guard,
                path,
                data,
                governance_rooted_fs::ExpectedFile::Missing,
            ) {
                if write_error.kind() != io::ErrorKind::WouldBlock {
                    return Err(write_error.into());
                }
                let raced = read_rooted_governance_state_file(root_guard, path, max_bytes)
                    .map_err(|read_error| {
                        GovernancePublishError::other(format!(
                            "immutable governance artifact `{}` raced with another writer ({write_error}) and could not be verified: {read_error}",
                            path.display()
                        ))
                    })?;
                if raced.bytes() != data {
                    return Err(GovernancePublishError::other(format!(
                        "immutable governance artifact path `{}` was concurrently occupied by different bytes",
                        path.display()
                    )));
                }
                raced.binding().verify()?;
            }
        }
        Err(error) => return Err(error.into()),
    }
    let readback = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    if readback.bytes() != data {
        return Err(GovernancePublishError::other(format!(
            "immutable governance artifact `{}` durable readback diverged",
            path.display()
        )));
    }
    readback.binding().verify()?;
    Ok(())
}
fn write_immutable_governance_artifact(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
    max_bytes: usize,
) -> Result<(), GovernancePublishError> {
    write_immutable_governance_file(root_guard, path, data, max_bytes)?;
    let mut digest_body = blake3::hash(data).to_hex().to_string();
    digest_body.push('\n');
    write_immutable_governance_file(
        root_guard,
        &digest_sidecar_path_for(path),
        digest_body.as_bytes(),
        GOVERNANCE_DIGEST_SIDECAR_BYTES,
    )?;
    let readback = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    if readback.bytes() != data {
        return Err(GovernancePublishError::other(format!(
            "immutable governance artifact `{}` changed while binding its digest sidecar",
            path.display()
        )));
    }
    verify_rooted_digest_sidecar(root_guard, path, readback.bytes())?;
    readback.binding().verify()?;
    Ok(())
}
fn governance_source_pair_id(
    payload_kind: &str,
    encoded_len: u64,
    encoded_blake3: &str,
    json_len: u64,
    json_blake3: &str,
) -> Result<String, GovernancePublishError> {
    let encoded_blake3 = hex::decode(encoded_blake3)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| GovernancePublishError::other("encoded source digest is noncanonical"))?;
    let json_blake3 = hex::decode(json_blake3)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| GovernancePublishError::other("JSON source digest is noncanonical"))?;
    Ok(hex::encode(governance_publication_source_pair_id_v1(
        payload_kind,
        encoded_len,
        encoded_blake3,
        json_len,
        json_blake3,
    )))
}
pub(crate) fn governance_source_pair_relative_paths(
    payload_kind: &str,
    encoded_len: u64,
    encoded_blake3: &str,
    json_len: u64,
    json_blake3: &str,
) -> Result<(String, String), GovernancePublishError> {
    validate_governance_publication_payload_kind(payload_kind)?;
    let pair_id = governance_source_pair_id(
        payload_kind,
        encoded_len,
        encoded_blake3,
        json_len,
        json_blake3,
    )?;
    let root = format!("{GOVERNANCE_PUBLICATION_SOURCES_DIR}/{payload_kind}/{pair_id}");
    Ok((format!("{root}/payload.to"), format!("{root}/payload.json")))
}
fn validate_governance_publication_payload_kind(
    payload_kind: &str,
) -> Result<(), GovernancePublishError> {
    if payload_kind.is_empty()
        || matches!(payload_kind, "." | "..")
        || payload_kind.len() > 128
        || !payload_kind.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        return Err(GovernancePublishError::other(
            "governance publication source kind is noncanonical",
        ));
    }
    Ok(())
}
fn persist_governance_source_pair(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    payload_kind: &str,
    encoded: &[u8],
    json_bytes: &[u8],
) -> Result<(PathBuf, PathBuf), GovernancePublishError> {
    validate_governance_car_source_lengths(encoded.len(), json_bytes.len())?;
    validate_governance_publication_payload_kind(payload_kind)?;
    let encoded_blake3 = blake3::hash(encoded).to_hex().to_string();
    let json_blake3 = blake3::hash(json_bytes).to_hex().to_string();
    let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
        GovernancePublishError::other("governance encoded source length exceeds u64")
    })?;
    let json_len = u64::try_from(json_bytes.len())
        .map_err(|_| GovernancePublishError::other("governance JSON source length exceeds u64"))?;
    let (encoded_relative, json_relative) = governance_source_pair_relative_paths(
        payload_kind,
        encoded_len,
        &encoded_blake3,
        json_len,
        &json_blake3,
    )?;
    let encoded_path = resolve_index_path(root, &encoded_relative)?;
    let json_path = resolve_index_path(root, &json_relative)?;
    write_immutable_governance_artifact(
        root_guard,
        &encoded_path,
        encoded,
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
    )?;
    write_immutable_governance_artifact(
        root_guard,
        &json_path,
        json_bytes,
        GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
    )?;
    Ok((encoded_path, json_path))
}
fn rooted_target(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    create_directories: bool,
) -> io::Result<(governance_rooted_fs::RootedDirectory, OsString)> {
    root_guard.revalidate()?;
    let relative = path.strip_prefix(root_guard.root()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "governance target `{}` escapes retained root `{}`",
                path.display(),
                root_guard.root().display()
            ),
        )
    })?;
    let target = root_guard
        .rooted_directory()
        .resolve_parent(relative, create_directories)?;
    root_guard.revalidate()?;
    Ok(target)
}
fn rooted_atomic_temp_name(target: &OsStr) -> io::Result<OsString> {
    let target = target.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance atomic target name is not canonical UTF-8",
        )
    })?;
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(OsString::from(format!(
        ".{target}.tmp-{}-{counter}",
        std::process::id()
    )))
}
fn write_rooted_atomic(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> io::Result<()> {
    isolate_recoverable_atomic_state_for_target(
        root_guard,
        path,
        GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
        "mutable-state-recovery",
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("governance mutable-state recovery failed: {error}"),
        )
    })?;
    let (directory, name) = rooted_target(root_guard, path, true)?;
    name.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance atomic target name is not canonical UTF-8",
        )
    })?;
    // Crash-temporary reclamation is a recovery operation, not part of a new
    // write. The unique create-only name below cannot collide with an older
    // process, so a new transaction never needs to delete a stale pathname as
    // a side effect.
    let temporary_name = rooted_atomic_temp_name(&name)?;
    directory.atomic_replace_current(&name, &temporary_name, data)?;
    root_guard.revalidate()
}
fn write_rooted_atomic_expected(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
    expected: governance_rooted_fs::ExpectedFile,
) -> io::Result<()> {
    isolate_recoverable_atomic_state_for_target(
        root_guard,
        path,
        GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
        "mutable-state-recovery",
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("governance mutable-state recovery failed: {error}"),
        )
    })?;
    let (directory, name) = rooted_target(root_guard, path, true)?;
    name.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance atomic target name is not canonical UTF-8",
        )
    })?;
    // See `write_rooted_atomic`: stale-name recovery is deliberately separate
    // from the transaction that creates this process's unique temporary.
    let temporary_name = rooted_atomic_temp_name(&name)?;
    directory.atomic_write(&name, &temporary_name, data, expected)?;
    root_guard.revalidate()
}
pub(super) fn read_rooted_governance_state_file(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    max_bytes: usize,
) -> io::Result<governance_rooted_fs::FileSnapshot> {
    let (directory, name) = rooted_target(root_guard, path, false)?;
    let snapshot = directory.read_file(&name, max_bytes)?;
    root_guard.revalidate()?;
    Ok(snapshot)
}
fn write_rooted_digest_sidecar(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> io::Result<()> {
    let mut body = blake3::hash(data).to_hex().to_string();
    body.push('\n');
    write_rooted_atomic(root_guard, &digest_sidecar_path_for(path), body.as_bytes())
}
fn ensure_rooted_digest_sidecar_immutable(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> Result<(), GovernancePublishError> {
    let sidecar_path = digest_sidecar_path_for(path);
    let mut body = blake3::hash(data).to_hex().to_string();
    body.push('\n');
    match read_rooted_governance_state_file(
        root_guard,
        &sidecar_path,
        GOVERNANCE_DIGEST_SIDECAR_BYTES,
    ) {
        Ok(current) => {
            if current.bytes() != body.as_bytes() {
                return Err(GovernancePublishError::other(format!(
                    "immutable governance digest sidecar for `{}` is substituted",
                    path.display()
                )));
            }
            current.binding().verify()?;
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    write_rooted_atomic_expected(
        root_guard,
        &sidecar_path,
        body.as_bytes(),
        governance_rooted_fs::ExpectedFile::Missing,
    )?;
    let readback = read_rooted_governance_state_file(
        root_guard,
        &sidecar_path,
        GOVERNANCE_DIGEST_SIDECAR_BYTES,
    )?;
    if readback.bytes() != body.as_bytes() {
        return Err(GovernancePublishError::other(format!(
            "immutable governance digest sidecar for `{}` diverged after creation",
            path.display()
        )));
    }
    readback.binding().verify()?;
    Ok(())
}
fn verify_rooted_digest_sidecar(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> Result<(), GovernancePublishError> {
    let actual = read_rooted_governance_state_file(
        root_guard,
        &digest_sidecar_path_for(path),
        GOVERNANCE_DIGEST_SIDECAR_BYTES,
    )?;
    let mut expected = blake3::hash(data).to_hex().to_string();
    expected.push('\n');
    if actual.bytes() != expected.as_bytes() {
        return Err(GovernancePublishError::other(format!(
            "governance state digest sidecar does not match `{}`",
            path.display()
        )));
    }
    actual.binding().verify()?;
    Ok(())
}
#[cfg(test)]
fn write_atomic_with_directory_sync<F>(path: &Path, data: &[u8], sync_parent: F) -> io::Result<()>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    validate_atomic_output_path(path)?;
    fs::create_dir_all(parent).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to create output parent `{}`: {err}",
                parent.display()
            ),
        )
    })?;
    validate_atomic_output_path(path)?;
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = temp_path_for_atomic(path, std::process::id(), counter);
    let write_result = (|| -> io::Result<()> {
        let mut file = open_atomic_temp_file(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        validate_atomic_output_path(path)?;
        fs::rename(&tmp_path, path)?;
        sync_parent(parent)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}
fn write_digest_sidecar(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    data: &[u8],
) -> io::Result<()> {
    write_rooted_digest_sidecar(root_guard, path, data)
}
fn digest_sidecar_path_for(path: &Path) -> PathBuf {
    let suffix = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.blake3"),
        _ => "blake3".to_string(),
    };
    path.with_extension(suffix)
}
fn verify_digest_sidecar(path: &Path, data: &[u8]) -> Result<(), GovernancePublishError> {
    let digest_path = digest_sidecar_path_for(path);
    let actual = read_bounded_governance_state_file(&digest_path, GOVERNANCE_DIGEST_SIDECAR_BYTES)?;
    let mut expected = blake3::hash(data).to_hex().to_string();
    expected.push('\n');
    if actual != expected.as_bytes() {
        return Err(GovernancePublishError::other(format!(
            "governance state digest sidecar does not match `{}`",
            path.display()
        )));
    }
    Ok(())
}
#[cfg(test)]
fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64) -> PathBuf {
    let suffix = format!("tmp-{pid}-{counter}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}
#[cfg(test)]
fn open_atomic_temp_file(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow_flag(&mut options);
    let file = options.open(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create atomic temp `{}`: {err}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to inspect atomic temp `{}` after open: {err}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "atomic temp `{}` must be a regular file",
            path.display()
        )));
    }
    Ok(file)
}
fn validate_atomic_output_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a symlink",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a directory",
                    path.display()
                )));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to inspect output `{}`: {err}", path.display()),
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        )));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!(
                            "failed to inspect output parent `{}`: {err}",
                            ancestor.display()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(unix)]
fn set_directory_no_follow_flags(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag() | platform_directory_only_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}
#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("Governance DAG filesystem flags are not qualified for this Android architecture");
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Governance DAG filesystem flags are not qualified for this Unix target");
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
fn platform_no_follow_flag() -> i32 {
    0x400000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
fn platform_directory_only_flag() -> i32 {
    0x200000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
fn platform_directory_only_flag() -> i32 {
    0x4000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn platform_directory_only_flag() -> i32 {
    0x10000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
fn platform_directory_only_flag() -> i32 {
    0x4000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
fn platform_directory_only_flag() -> i32 {
    0x10000
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_directory_only_flag() -> i32 {
    0x0010_0000
}
#[cfg(target_os = "freebsd")]
fn platform_directory_only_flag() -> i32 {
    0x0002_0000
}
#[cfg(target_os = "dragonfly")]
fn platform_directory_only_flag() -> i32 {
    0x0800_0000
}
#[cfg(target_os = "openbsd")]
fn platform_directory_only_flag() -> i32 {
    0x0002_0000
}
#[cfg(target_os = "netbsd")]
fn platform_directory_only_flag() -> i32 {
    0x0020_0000
}
fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
impl QualifiedFencedTransparencyPublisherV1 {
    /// Qualify and pin one deployment-owned fused privacy publisher.
    ///
    /// # Errors
    ///
    /// Returns [`GovernancePublishError`] when the configured binding is
    /// malformed or test-marked, qualification fails, or the live provider
    /// identity, revision, or policy digest differs from the expected binding.
    pub fn try_new(
        expected_handle: String,
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        provider: Arc<dyn FencedTransparencyPublisherV1>,
    ) -> Result<Self, GovernancePublishError> {
        validate_runtime_handle(&expected_handle, "fenced transparency publisher")?;
        if !expected_qualification.is_valid() {
            return Err(GovernancePublishError::other(
                "configured fenced transparency publisher qualification is invalid",
            ));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency publisher is unavailable, stale, or unqualified",
            )
        })?;
        if provider.handle() != expected_handle || qualification != expected_qualification {
            return Err(GovernancePublishError::other(
                "fenced transparency publisher identity or policy does not match configuration",
            ));
        }
        let publisher = Self {
            handle: expected_handle,
            qualification: expected_qualification,
            provider,
        };
        publisher.assert_qualification()?;
        Ok(publisher)
    }
    /// Return the pinned opaque deployment handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }
    /// Return the pinned public adapter qualification.
    #[must_use]
    pub const fn qualification(&self) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.qualification
    }
    pub(crate) fn assert_qualification(&self) -> Result<(), GovernancePublishError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency publisher is unavailable, stale, or unqualified",
            )
        })?;
        if self.provider.handle() != self.handle || qualification != self.qualification {
            return Err(GovernancePublishError::other(
                "fenced transparency publisher identity or policy changed after qualification",
            ));
        }
        Ok(())
    }
    fn compare_and_append_privacy_classified(
        &self,
        request: &FencedPrivacyPublicationRequestV1,
    ) -> Result<FencedPrivacyPublicationReceiptV1, FencedPrivacyBoundaryFailure> {
        request
            .validate()
            .map_err(|error| FencedPrivacyBoundaryFailure {
                error: GovernancePublishError::other(error.to_string()),
                may_have_appended: false,
            })?;
        self.assert_qualification()
            .map_err(|error| FencedPrivacyBoundaryFailure {
                error,
                may_have_appended: false,
            })?;
        let result = self.provider.compare_and_append_privacy(request);
        self.assert_qualification()
            .map_err(|_| FencedPrivacyBoundaryFailure {
                error: GovernancePublishError::other(
                    "fenced transparency publisher identity changed during an external append; outcome is ambiguous",
                ),
                may_have_appended: true,
            })?;
        let receipt = result.map_err(|error| FencedPrivacyBoundaryFailure {
            error: GovernancePublishError::other(error.to_string()),
            may_have_appended: !matches!(
                error,
                FencedTransparencyPublishErrorV1::InvalidRequest
                    | FencedTransparencyPublishErrorV1::CompareConflict
                    | FencedTransparencyPublishErrorV1::PublicationConflict
                    | FencedTransparencyPublishErrorV1::StaleFencingToken
                    | FencedTransparencyPublishErrorV1::Rejected
            ),
        })?;
        receipt
            .validate_for_request(request, &self.handle, self.qualification)
            .map_err(|error| FencedPrivacyBoundaryFailure {
                error: GovernancePublishError::other(error.to_string()),
                may_have_appended: true,
            })?;
        Ok(receipt)
    }
}
impl QualifiedFencedTransparencyHeadReaderV1 {
    /// Qualify and pin one deployment-owned authenticated head reader.
    ///
    /// # Errors
    ///
    /// Returns [`GovernancePublishError`] when the expected binding is
    /// malformed or test-marked, provider qualification fails, or the provider
    /// identity, revision, or policy digest differs from the expected binding.
    pub fn try_new(
        expected_handle: String,
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        provider: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1>,
    ) -> Result<Self, GovernancePublishError> {
        validate_runtime_handle(&expected_handle, "fenced transparency head reader")?;
        if !expected_qualification.is_valid() {
            return Err(GovernancePublishError::other(
                "configured fenced transparency head reader qualification is invalid",
            ));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency head reader is unavailable, stale, or unqualified",
            )
        })?;
        if provider.handle() != expected_handle || qualification != expected_qualification {
            return Err(GovernancePublishError::other(
                "fenced transparency head reader identity or policy does not match configuration",
            ));
        }
        let reader = Self {
            handle: expected_handle,
            qualification: expected_qualification,
            provider,
        };
        reader.assert_qualification()?;
        Ok(reader)
    }
    /// Return the pinned opaque deployment handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }
    /// Return the pinned public adapter qualification.
    #[must_use]
    pub const fn qualification(&self) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.qualification
    }
    pub(crate) fn assert_qualification(&self) -> Result<(), GovernancePublishError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency head reader is unavailable, stale, or unqualified",
            )
        })?;
        if self.provider.handle() != self.handle || qualification != self.qualification {
            return Err(GovernancePublishError::other(
                "fenced transparency head reader identity or policy changed after qualification",
            ));
        }
        Ok(())
    }
    fn read_authoritative_head_with_ancestry(
        &self,
        required_ancestors: &[FencedTransparencyTargetHeadV1],
        required_publications: &[FencedTransparencyPublicationInclusionV1],
    ) -> Result<FencedTransparencyHeadAncestryProofV1, GovernancePublishError> {
        self.assert_qualification()?;
        let result = self
            .provider
            .read_authoritative_head_with_ancestry(required_ancestors, required_publications);
        self.assert_qualification().map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency head reader identity changed during authenticated readback",
            )
        })?;
        let proof = result.map_err(|_| {
            GovernancePublishError::other(
                "fenced transparency authoritative head readback or ancestry proof failed authentication",
            )
        })?;
        proof
            .validate_for_required_evidence(required_ancestors, required_publications)
            .map_err(|_| {
                GovernancePublishError::other(
                    "fenced transparency authoritative head ancestry or inclusion proof is malformed or substituted",
                )
            })?;
        Ok(proof)
    }
}
pub(crate) fn ensure_fenced_privacy_runtime_bindings_match(
    publisher: &QualifiedFencedTransparencyPublisherV1,
    reader: &QualifiedFencedTransparencyHeadReaderV1,
) -> Result<(), GovernancePublishError> {
    if publisher.handle() != reader.handle() || publisher.qualification() != reader.qualification()
    {
        return Err(GovernancePublishError::other(
            "fused privacy writer and authoritative-head reader bindings must share one exact identity, revision, and policy digest",
        ));
    }
    Ok(())
}
impl GovernanceRuntimeDagSigner {
    fn try_new(
        expected_handle: String,
        publisher_peer_id: Vec<u8>,
        expected_public_key: [u8; 32],
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        provider: Arc<dyn GovernanceDagRuntimeSigner>,
    ) -> Result<Self, GovernancePublishError> {
        validate_runtime_handle(&expected_handle, "governance runtime DAG signer")?;
        if !expected_qualification.is_valid() {
            return Err(GovernancePublishError::other(
                "configured governance runtime DAG signer policy qualification is invalid",
            ));
        }
        if publisher_peer_id.is_empty() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG publisher peer id must not be empty",
            ));
        }
        if publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG publisher peer id exceeds {GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1} bytes"
            )));
        }
        if expected_public_key.iter().all(|byte| *byte == 0) {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key must not be all zero",
            ));
        }
        let dalek_public_key =
            DalekVerifyingKey::from_bytes(&expected_public_key).map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer public key is not a canonical Ed25519 point",
                )
            })?;
        if dalek_public_key.to_bytes() != expected_public_key || dalek_public_key.is_weak() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key is non-canonical or weak",
            ));
        }
        let verification_key = PublicKey::from_bytes(Algorithm::Ed25519, &expected_public_key)
            .map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer public key is not canonical Ed25519",
                )
            })?;
        if provider.handle() != expected_handle {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer handle does not match configured handle",
            ));
        }
        if provider.publisher_peer_id() != publisher_peer_id {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer publisher identity does not match configured identity",
            ));
        }
        if provider.public_key() != expected_public_key {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key does not match configured public key",
            ));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer is unavailable, stale, or unqualified",
            )
        })?;
        if !qualification.is_valid() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer returned an invalid policy qualification",
            ));
        }
        if qualification != expected_qualification {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer policy qualification does not match configured revision and digest",
            ));
        }
        let rechecked_qualification = provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer is unavailable, stale, or unqualified",
            )
        })?;
        if provider.handle() != expected_handle
            || provider.publisher_peer_id() != publisher_peer_id
            || provider.public_key() != expected_public_key
            || rechecked_qualification != expected_qualification
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer identity or policy changed during startup qualification",
            ));
        }
        Ok(Self {
            handle: expected_handle,
            publisher_peer_id,
            public_key: expected_public_key,
            qualification: expected_qualification,
            verification_key,
            provider,
        })
    }
    fn sign(
        &self,
        purpose: GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<GovernanceLogSignatureV1, GovernancePublishError> {
        self.assert_qualification()?;
        let signature_result = self.provider.sign(purpose, payload);
        self.assert_qualification()?;
        let signature_bytes = signature_result.map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer refused the canonical payload",
            )
        })?;
        let signature = IrohaSignature::try_from_bytes(&signature_bytes).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer returned a malformed Ed25519 signature",
            )
        })?;
        signature
            .verify(&self.verification_key, payload)
            .map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer returned a signature for another key or payload",
                )
            })?;
        Ok(GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: self.public_key.to_vec(),
            signature: signature_bytes.to_vec(),
        })
    }
    /// Revalidate the pinned signer identity and public provider policy.
    pub(crate) fn assert_qualification(&self) -> Result<(), GovernancePublishError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer is unavailable, stale, or unqualified",
            )
        })?;
        if self.provider.handle() != self.handle
            || self.provider.publisher_peer_id() != self.publisher_peer_id
            || self.provider.public_key() != self.public_key
            || qualification != self.qualification
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer identity or policy changed after injection",
            ));
        }
        Ok(())
    }
    /// Return the exact retained, non-secret signer binding without exposing
    /// filesystem paths or the signer provider itself.
    #[must_use]
    pub(crate) fn binding(
        &self,
    ) -> (
        &str,
        GovernanceDagRuntimeProviderQualificationV1,
        &[u8],
        [u8; 32],
    ) {
        (
            &self.handle,
            self.qualification,
            &self.publisher_peer_id,
            self.public_key,
        )
    }
    fn publisher_peer_id_hex(&self) -> String {
        hex::encode(&self.publisher_peer_id)
    }
    fn publisher_public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }
}
impl GovernanceRuntimeDagCheckpointStore {
    fn try_new(
        expected_handle: String,
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        provider: Arc<dyn GovernanceDagSealedCheckpointStore>,
    ) -> Result<Self, GovernancePublishError> {
        validate_runtime_handle(&expected_handle, "governance runtime DAG checkpoint store")?;
        if !expected_qualification.is_valid() {
            return Err(GovernancePublishError::other(
                "configured governance runtime DAG checkpoint-store qualification is invalid",
            ));
        }
        if provider.handle() != expected_handle {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint-store handle does not match configuration",
            ));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG checkpoint store is unavailable, stale, or unqualified",
            )
        })?;
        if qualification != expected_qualification {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint-store qualification does not match configuration",
            ));
        }
        let store = Self {
            handle: expected_handle,
            qualification: expected_qualification,
            provider,
        };
        store.assert_qualification()?;
        Ok(store)
    }
    pub(crate) fn handle(&self) -> &str {
        &self.handle
    }
    pub(crate) const fn qualification(&self) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.qualification
    }
    pub(crate) fn assert_qualification(&self) -> Result<(), GovernancePublishError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG checkpoint store is unavailable, stale, or unqualified",
            )
        })?;
        if self.provider.handle() != self.handle || qualification != self.qualification {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint-store identity or policy changed after injection",
            ));
        }
        Ok(())
    }
    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, GovernancePublishError> {
        self.assert_qualification()?;
        let result = self.provider.load(slot);
        self.assert_qualification()?;
        let record = result.map_err(|_| {
            GovernancePublishError::other("governance runtime DAG checkpoint-store read failed")
        })?;
        if record.as_ref().is_some_and(|record| {
            record.generation == 0
                || record.payload.is_empty()
                || !record.has_valid_revision(slot)
                || record.payload.len() > governance_dag_sealed_state_payload_max_bytes_v1(slot)
        }) {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint store returned a malformed or oversized record",
            ));
        }
        Ok(record)
    }
    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), GovernancePublishError> {
        if next.generation == 0
            || next.payload.is_empty()
            || !next.has_valid_revision(slot)
            || next.payload.len() > governance_dag_sealed_state_payload_max_bytes_v1(slot)
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint-store write is malformed or oversized",
            ));
        }
        self.assert_qualification()?;
        let result = self
            .provider
            .compare_and_swap(slot, expected_revision, next.clone());
        self.assert_qualification()?;
        result.map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG checkpoint-store compare-and-swap failed",
            )
        })?;
        if self.load(slot)?.as_ref() != Some(&next) {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint-store readback diverged",
            ));
        }
        Ok(())
    }
    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), GovernancePublishError> {
        self.assert_qualification()?;
        let result = self.provider.delete(slot, expected_revision);
        self.assert_qualification()?;
        result.map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG checkpoint-store intent delete failed",
            )
        })?;
        if self.load(slot)?.is_some() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG checkpoint store retained a deleted intent",
            ));
        }
        Ok(())
    }
}
/// Qualify one exact runtime signer without opening the publisher filesystem.
pub(crate) fn qualify_governance_dag_runtime_signer_provider(
    expected_handle: String,
    publisher_peer_id: Vec<u8>,
    expected_public_key: [u8; 32],
    expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn GovernanceDagRuntimeSigner>,
) -> Result<GovernanceRuntimeDagSigner, GovernancePublishError> {
    GovernanceRuntimeDagSigner::try_new(
        expected_handle,
        publisher_peer_id,
        expected_public_key,
        expected_qualification,
        provider,
    )
}
/// Qualify one exact sealed local-producer store before opening state.
pub(crate) fn qualify_governance_dag_runtime_checkpoint_store(
    expected_handle: String,
    expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn GovernanceDagSealedCheckpointStore>,
) -> Result<GovernanceRuntimeDagCheckpointStore, GovernancePublishError> {
    GovernanceRuntimeDagCheckpointStore::try_new(expected_handle, expected_qualification, provider)
}
fn validate_runtime_handle(
    handle: &str,
    label: &'static str,
) -> Result<(), GovernancePublishError> {
    match validate_production_runtime_handle(handle) {
        Ok(()) => Ok(()),
        Err(ProductionRuntimeHandleError::InvalidSyntax) => Err(GovernancePublishError::other(
            format!("{label} handle is not a canonical credential-free production runtime handle"),
        )),
        Err(ProductionRuntimeHandleError::TestMarked) => Err(GovernancePublishError::other(
            format!("{label} handle is test-marked and cannot qualify a production adapter"),
        )),
    }
}
#[cfg(unix)]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    metadata_identifies_same_file(before, after)
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}
#[cfg(not(unix))]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    metadata_identifies_same_file(before, after)
        && before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
}
fn read_bounded_governance_state_file(path: &Path, max_bytes: usize) -> io::Result<Vec<u8>> {
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| io::Error::other("governance state byte limit exceeds u64"))?;
    validate_atomic_output_path(path)?;
    let before_open = fs::symlink_metadata(path)?;
    validate_governance_state_metadata(path, &before_open)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(path)?;
    let opened_metadata = file.metadata()?;
    validate_governance_state_metadata(path, &opened_metadata)?;
    if !metadata_identifies_same_file(&before_open, &opened_metadata) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while opening",
            path.display()
        )));
    }
    if opened_metadata.len() > max_bytes_u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance state `{}` exceeds {max_bytes} bytes",
                path.display()
            ),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened_metadata.len()).unwrap_or(max_bytes));
    (&mut file)
        .take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance state `{}` exceeds {max_bytes} bytes",
                path.display()
            ),
        ));
    }
    let after_read_file = file.metadata()?;
    if !metadata_stable_during_read(&opened_metadata, &after_read_file) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while reading",
            path.display()
        )));
    }
    let after_read = fs::symlink_metadata(path)?;
    validate_governance_state_metadata(path, &after_read)?;
    if !metadata_identifies_same_file(&opened_metadata, &after_read) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while reading",
            path.display()
        )));
    }
    validate_atomic_output_path(path)?;
    Ok(bytes)
}
fn validate_governance_state_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "governance state `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::other(format!(
            "governance state `{}` must have exactly one hard link",
            path.display()
        )));
    }
    Ok(())
}
fn empty_governance_publish_index() -> JsonMap {
    let mut index = JsonMap::new();
    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    index.insert("generated_at".into(), JsonValue::from(0_u64));
    index.insert("entry_count".into(), JsonValue::from(0_u64));
    index.insert(
        "payload_kind_counts".into(),
        JsonValue::Object(JsonMap::new()),
    );
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(JsonMap::new()),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(JsonMap::new()));
    index.insert("entries".into(), JsonValue::Array(Vec::new()));
    index
}
fn empty_governance_car_queue() -> JsonMap {
    let mut queue = JsonMap::new();
    queue.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
    );
    queue.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    queue.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    queue.insert("generated_at".into(), JsonValue::from(0_u64));
    queue.insert("segment_count".into(), JsonValue::from(0_u64));
    queue.insert("assembled_count".into(), JsonValue::from(0_u64));
    queue.insert("pending_count".into(), JsonValue::from(0_u64));
    queue.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(JsonMap::new()),
    );
    queue.insert("by_payload_kind".into(), JsonValue::Object(JsonMap::new()));
    queue.insert(
        "by_car_archive_blake3".into(),
        JsonValue::Object(JsonMap::new()),
    );
    queue.insert("segments".into(), JsonValue::Array(Vec::new()));
    queue
}
fn empty_governance_publication_state() -> JsonMap {
    let mut state = JsonMap::new();
    state.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_PUBLICATION_STATE_SCHEMA),
    );
    state.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    state.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    state.insert("generation".into(), JsonValue::from(0_u64));
    state.insert("generated_at".into(), JsonValue::from(0_u64));
    state.insert(
        "publish_index".into(),
        JsonValue::Object(empty_governance_publish_index()),
    );
    state.insert(
        "car_queue".into(),
        JsonValue::Object(empty_governance_car_queue()),
    );
    state
}
fn read_governance_publication_initialization_marker(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<bool, GovernancePublishError> {
    let marker_path = root.join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE);
    let snapshot = match read_rooted_governance_state_file(
        root_guard,
        &marker_path,
        GOVERNANCE_PUBLICATION_INITIALIZED_BODY.len(),
    ) {
        Ok(snapshot) => snapshot,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if snapshot.bytes() != GOVERNANCE_PUBLICATION_INITIALIZED_BODY {
        return Err(GovernancePublishError::other(
            "governance publication initialization marker is malformed",
        ));
    }
    snapshot.binding().verify()?;
    Ok(true)
}
fn governance_publication_artifact_roots_present(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<bool, GovernancePublishError> {
    let root_directory = root_guard.rooted_directory();
    for directory_name in [
        GOVERNANCE_PUBLICATION_SOURCES_DIR,
        GOVERNANCE_CAR_SEGMENTS_DIR,
    ] {
        match root_directory.open_directory(OsStr::new(directory_name)) {
            Ok(directory) => {
                drop(directory);
                return Ok(true);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    root_guard.revalidate()?;
    Ok(false)
}
fn initialize_governance_publication_authority_if_pristine(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(governance_rooted_fs::TwoSlotStoreV1, bool), GovernancePublishError> {
    reject_legacy_governance_publication_authorities(root, root_guard)?;
    let marker_present = read_governance_publication_initialization_marker(root, root_guard)?;
    let authority_present = match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_PUBLICATION_STORE_DIR_V1))
    {
        Ok(directory) => {
            drop(directory);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if !authority_present
        && (marker_present || governance_publication_artifact_roots_present(root_guard)?)
    {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state is missing from an initialized root",
        ));
    }
    let state = empty_governance_publication_state();
    validate_governance_publication_state(&state)?;
    let body = json::to_json_pretty(&JsonValue::Object(state)).map_err(|error| {
        GovernancePublishError::other(format!(
            "serialize initial governance publication state: {error}"
        ))
    })?;
    let store = open_governance_two_slot_store_v1(
        root_guard,
        GOVERNANCE_PUBLICATION_STORE_SPEC_V1,
        body.as_bytes(),
    )?;
    let _ = read_governance_publication_state(&store)?;
    Ok((store, marker_present))
}
fn write_governance_publication_initialization_marker(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    let marker_path = root.join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE);
    write_rooted_atomic_expected(
        root_guard,
        &marker_path,
        GOVERNANCE_PUBLICATION_INITIALIZED_BODY,
        governance_rooted_fs::ExpectedFile::Missing,
    )?;
    if !read_governance_publication_initialization_marker(root, root_guard)? {
        return Err(GovernancePublishError::other(
            "governance publication initialization marker did not persist",
        ));
    }
    Ok(())
}
fn reject_legacy_atomic_state_names(
    directory: &governance_rooted_fs::RootedDirectory,
    targets: &[&str],
    label: &str,
) -> Result<(), GovernancePublishError> {
    for name in directory.child_names_bounded(GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP)? {
        let Some(name_utf8) = name.to_str() else {
            continue;
        };
        let legacy = targets.iter().any(|target| {
            governance_rooted_fs::is_atomic_temp_candidate_for(name_utf8, target)
                || governance_rooted_fs::is_atomic_retained_candidate_for(name_utf8, target)
        });
        if legacy {
            return Err(GovernancePublishError::other(format!(
                "legacy {label} atomic state `{name_utf8}` is unsupported; archive or remove it offline before first-release initialization"
            )));
        }
    }
    Ok(())
}
fn reject_legacy_governance_publication_authorities(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    reject_legacy_atomic_state_names(
        root_guard.rooted_directory(),
        &[
            GOVERNANCE_PUBLICATION_STATE_FILE,
            GOVERNANCE_PUBLISH_INDEX_FILE,
            GOVERNANCE_CAR_QUEUE_FILE,
            "governance-publication-state-v1.json.blake3",
            "publish-index.json.blake3",
            "car-queue.json.blake3",
        ],
        "governance publication authority",
    )?;
    for file in [
        GOVERNANCE_PUBLICATION_STATE_FILE,
        GOVERNANCE_PUBLISH_INDEX_FILE,
        GOVERNANCE_CAR_QUEUE_FILE,
    ] {
        match read_rooted_governance_state_file(root_guard, &root.join(file), 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy governance publication authority `{file}` is unsupported; remove it before first-release initialization"
                )));
            }
        }
        let sidecar = digest_sidecar_path_for(&root.join(file));
        match read_rooted_governance_state_file(root_guard, &sidecar, 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy governance publication authority sidecar `{}` is unsupported; remove it before first-release initialization",
                    sidecar.display()
                )));
            }
        }
    }
    Ok(())
}
fn read_governance_publication_state(
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<(JsonMap, governance_rooted_fs::TwoSlotSnapshotV1), GovernancePublishError> {
    let snapshot = load_governance_two_slot_store_v1(store, "governance publication authority")?;
    let state = decode_governance_publication_state_snapshot(&snapshot)?;
    Ok((state, snapshot))
}
fn decode_governance_publication_state_snapshot(
    snapshot: &governance_rooted_fs::TwoSlotSnapshotV1,
) -> Result<JsonMap, GovernancePublishError> {
    let value: JsonValue = json::from_slice(snapshot.payload()).map_err(|error| {
        GovernancePublishError::other(format!(
            "failed to parse authoritative governance publication state: {error}"
        ))
    })?;
    let JsonValue::Object(state) = value else {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state root is not an object",
        ));
    };
    validate_governance_publication_state(&state)?;
    let canonical = json::to_json_pretty(&JsonValue::Object(state.clone())).map_err(|error| {
        GovernancePublishError::other(format!(
            "failed to canonicalize authoritative governance publication state: {error}"
        ))
    })?;
    if canonical.as_bytes() != snapshot.payload() {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state is not canonical JSON",
        ));
    }
    let logical_generation =
        required_governance_u64(&state, "generation", "governance publication state")?;
    if logical_generation.checked_add(1) != Some(snapshot.generation()) {
        return Err(GovernancePublishError::other(
            "authoritative governance publication generation does not match its fixed-store generation",
        ));
    }
    Ok(state)
}
/// Load one exact publication-authority generation through a retained root.
///
/// An entirely pristine root is reported as `None`; once initialization state
/// or immutable publication history exists, a missing typed authority fails
/// closed.
pub(crate) fn load_governance_publication_snapshot_v1(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<Option<GovernancePublicationSnapshotV1>, GovernancePublishError> {
    let root = root_guard.root();
    root_guard.revalidate()?;
    reject_legacy_governance_publication_authorities(root, root_guard)?;
    match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_PUBLICATION_STORE_DIR_V1))
    {
        Ok(directory) => drop(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if read_governance_publication_initialization_marker(root, root_guard)?
                || governance_publication_artifact_roots_present(root_guard)?
            {
                return Err(GovernancePublishError::other(
                    "authoritative governance publication state is missing from an initialized root",
                ));
            }
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    }
    let config = governance_two_slot_config_v1(GOVERNANCE_PUBLICATION_STORE_SPEC_V1)?;
    let snapshot = root_guard
        .rooted_directory()
        .load_existing_two_slot_store_v1(config)
        .map_err(|error| {
            GovernancePublishError::other(format!(
                "failed to load governance publication two-slot state: {error}"
            ))
        })?;
    decode_governance_publication_state_snapshot(&snapshot)?;
    root_guard.revalidate()?;
    Ok(Some(GovernancePublicationSnapshotV1 {
        store_generation: snapshot.generation(),
        store_record_digest: snapshot.record_digest(),
        canonical_bytes: snapshot.payload().to_vec(),
    }))
}
#[cfg(test)]
fn commit_governance_publication_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    state: JsonMap,
) -> Result<(), GovernancePublishError> {
    let (store, _) = initialize_governance_publication_authority_if_pristine(root, root_guard)?;
    let (current, snapshot) = read_governance_publication_state(&store)?;
    if current.get("generation") != state.get("generation") {
        return Err(GovernancePublishError::other(
            "governance publication state predecessor generation is stale",
        ));
    }
    let body = prepare_governance_publication_state(state)?;
    write_prepared_governance_publication_state(&store, &snapshot, &body).map(drop)
}
#[cfg(test)]
fn commit_governance_publication_state_with<F>(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    state: JsonMap,
    writer: F,
) -> Result<(), GovernancePublishError>
where
    F: FnOnce(&GovernanceFilesystemRootGuard, &Path, &[u8]) -> io::Result<()>,
{
    let (store, _) = initialize_governance_publication_authority_if_pristine(root, root_guard)?;
    let (current, snapshot) = read_governance_publication_state(&store)?;
    if current.get("generation") != state.get("generation") {
        return Err(GovernancePublishError::other(
            "governance publication state predecessor generation is stale",
        ));
    }
    let body = prepare_governance_publication_state(state)?;
    writer(
        root_guard,
        &root.join(GOVERNANCE_PUBLICATION_STATE_FILE),
        &body,
    )?;
    write_prepared_governance_publication_state(&store, &snapshot, &body).map(drop)
}
fn prepare_governance_publication_state(
    mut state: JsonMap,
) -> Result<Vec<u8>, GovernancePublishError> {
    let generation = state
        .get("generation")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance publication state generation is missing or invalid",
            )
        })?
        .checked_add(1)
        .ok_or_else(|| {
            GovernancePublishError::other("governance publication state generation exhausted")
        })?;
    state.insert("generation".into(), JsonValue::from(generation));
    state.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    validate_governance_publication_state(&state)?;
    let body = json::to_json_pretty(&JsonValue::Object(state)).map_err(|error| {
        GovernancePublishError::other(format!(
            "serialize authoritative governance publication state: {error}"
        ))
    })?;
    if body.is_empty() || body.len() > GOVERNANCE_PUBLICATION_STATE_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "authoritative governance publication state exceeds {GOVERNANCE_PUBLICATION_STATE_MAX_BYTES} bytes"
        )));
    }
    Ok(body.into_bytes())
}
fn write_prepared_governance_publication_state(
    store: &governance_rooted_fs::TwoSlotStoreV1,
    expected: &governance_rooted_fs::TwoSlotSnapshotV1,
    body: &[u8],
) -> Result<governance_rooted_fs::TwoSlotSnapshotV1, GovernancePublishError> {
    if body.is_empty() || body.len() > GOVERNANCE_PUBLICATION_STATE_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "prepared authoritative governance publication state exceeds {GOVERNANCE_PUBLICATION_STATE_MAX_BYTES} bytes"
        )));
    }
    let committed = compare_and_swap_governance_two_slot_store_v1(
        store,
        expected,
        body,
        "governance publication authority",
    )?;
    if committed.payload() != body {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state readback diverged",
        ));
    }
    Ok(committed)
}
fn require_exact_governance_fields(
    map: &JsonMap,
    expected: &[&str],
    context: &str,
) -> Result<(), GovernancePublishError> {
    if map.len() != expected.len() || !expected.iter().all(|field| map.contains_key(*field)) {
        return Err(GovernancePublishError::other(format!(
            "{context} fields do not match the first-release schema"
        )));
    }
    Ok(())
}
fn required_governance_string<'a>(
    map: &'a JsonMap,
    field: &str,
    context: &str,
) -> Result<&'a str, GovernancePublishError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| GovernancePublishError::other(format!("{context} is missing `{field}`")))
}
fn required_governance_u64(
    map: &JsonMap,
    field: &str,
    context: &str,
) -> Result<u64, GovernancePublishError> {
    map.get(field)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| GovernancePublishError::other(format!("{context} is missing `{field}`")))
}
fn validate_governance_lower_hex(
    value: &str,
    bytes: usize,
    context: &str,
) -> Result<(), GovernancePublishError> {
    if value.len() != bytes.saturating_mul(2)
        || hex::decode(value)
            .ok()
            .is_none_or(|decoded| decoded.len() != bytes || hex::encode(decoded) != value)
    {
        return Err(GovernancePublishError::other(format!(
            "{context} is not canonical lowercase hexadecimal"
        )));
    }
    Ok(())
}
fn validate_governance_publication_labels(
    labels: &JsonMap,
    context: &str,
) -> Result<(), GovernancePublishError> {
    if labels.len() > GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1 {
        return Err(GovernancePublishError::other(format!(
            "{context} exceeds the {GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1}-label hard cap"
        )));
    }
    let mut compact_bytes = 2_usize;
    for (position, (key, value)) in labels.iter().enumerate() {
        if key.is_empty()
            || key.len() > GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(GovernancePublishError::other(format!(
                "{context} contains a noncanonical label key"
            )));
        }
        if value.as_object().is_some() || value.as_array().is_some() {
            return Err(GovernancePublishError::other(format!(
                "{context} label `{key}` must be a scalar"
            )));
        }
        if value
            .as_str()
            .is_some_and(|string| string.len() > GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1)
        {
            return Err(GovernancePublishError::other(format!(
                "{context} label `{key}` exceeds the {GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1}-byte string bound"
            )));
        }
        let encoded_value = json::to_json(value).map_err(|error| {
            GovernancePublishError::other(format!(
                "{context} label `{key}` is not canonical JSON: {error}"
            ))
        })?;
        compact_bytes = compact_bytes
            .checked_add(usize::from(position != 0))
            .and_then(|bytes| bytes.checked_add(key.len()))
            .and_then(|bytes| bytes.checked_add(3))
            .and_then(|bytes| bytes.checked_add(encoded_value.len()))
            .ok_or_else(|| GovernancePublishError::other("publication label size overflowed"))?;
    }
    if compact_bytes > GOVERNANCE_PUBLICATION_LABEL_TOTAL_MAX_BYTES_V1 {
        return Err(GovernancePublishError::other(format!(
            "{context} labels exceed the {GOVERNANCE_PUBLICATION_LABEL_TOTAL_MAX_BYTES_V1}-byte aggregate bound"
        )));
    }
    Ok(())
}
#[derive(Debug)]
struct GovernancePublishIdentity {
    payload_kind: String,
    encoded_path: String,
    json_path: String,
    encoded_blake3: String,
    encoded_len: u64,
    json_blake3: String,
    json_len: u64,
}
fn validate_governance_publish_index_state(
    index: &JsonMap,
) -> Result<Vec<GovernancePublishIdentity>, GovernancePublishError> {
    const INDEX_FIELDS: [&str; 9] = [
        "schema",
        "source",
        "root",
        "generated_at",
        "entry_count",
        "payload_kind_counts",
        "by_encoded_blake3",
        "by_payload_kind",
        "entries",
    ];
    require_exact_governance_fields(index, &INDEX_FIELDS, "governance publish index")?;
    if required_governance_string(index, "schema", "governance publish index")?
        != GOVERNANCE_PUBLISH_INDEX_SCHEMA
        || required_governance_string(index, "source", "governance publish index")?
            != GOVERNANCE_DAG_SINK_FILESYSTEM
        || required_governance_string(index, "root", "governance publish index")?
            != GOVERNANCE_DAG_LOGICAL_ROOT
    {
        return Err(GovernancePublishError::other(
            "governance publish index has an unsupported identity",
        ));
    }
    required_governance_u64(index, "generated_at", "governance publish index")?;
    let entries = index
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("governance publish entries are missing"))?;
    if entries.len() > GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP {
        return Err(GovernancePublishError::other(
            "governance publish index exceeds its entry hard cap",
        ));
    }
    let mut identities = Vec::with_capacity(entries.len());
    let mut payload_kind_counts = JsonMap::new();
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut path_identities = BTreeMap::<String, (&'static str, u64, String)>::new();
    let mut source_pair_ids = BTreeSet::new();
    for (position, value) in entries.iter().enumerate() {
        let context = format!("governance publish entry {position}");
        let entry = value
            .as_object()
            .ok_or_else(|| GovernancePublishError::other(format!("{context} is not an object")))?;
        const ENTRY_FIELDS: [&str; 10] = [
            "position",
            "payload_kind",
            "encoded_path",
            "json_path",
            "encoded_blake3",
            "encoded_len",
            "json_blake3",
            "json_len",
            "published_at_unix",
            "labels",
        ];
        require_exact_governance_fields(entry, &ENTRY_FIELDS, &context)?;
        if required_governance_u64(entry, "position", &context)?
            != u64::try_from(position).map_err(|_| {
                GovernancePublishError::other("governance publish position exceeds u64")
            })?
        {
            return Err(GovernancePublishError::other(format!(
                "{context} position is noncanonical"
            )));
        }
        let labels = entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| {
                GovernancePublishError::other(format!("{context} labels are noncanonical"))
            })?;
        validate_governance_publication_labels(labels, &context)?;
        required_governance_u64(entry, "published_at_unix", &context)?;
        let payload_kind = required_governance_string(entry, "payload_kind", &context)?;
        if payload_kind.is_empty()
            || matches!(payload_kind, "." | "..")
            || payload_kind.len() > 128
            || !payload_kind.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            })
        {
            return Err(GovernancePublishError::other(format!(
                "{context} payload kind is noncanonical"
            )));
        }
        let encoded_path = required_governance_string(entry, "encoded_path", &context)?;
        let json_path = required_governance_string(entry, "json_path", &context)?;
        index_path_components(encoded_path)?;
        index_path_components(json_path)?;
        if !encoded_path.ends_with(".to")
            || !json_path.ends_with(".json")
            || encoded_path == json_path
        {
            return Err(GovernancePublishError::other(format!(
                "{context} source paths are noncanonical"
            )));
        }
        let encoded_blake3 = required_governance_string(entry, "encoded_blake3", &context)?;
        let json_blake3 = required_governance_string(entry, "json_blake3", &context)?;
        validate_governance_lower_hex(encoded_blake3, 32, "encoded publication digest")?;
        validate_governance_lower_hex(json_blake3, 32, "JSON publication digest")?;
        let encoded_len = required_governance_u64(entry, "encoded_len", &context)?;
        let json_len = required_governance_u64(entry, "json_len", &context)?;
        let encoded_len_usize = usize::try_from(encoded_len).map_err(|_| {
            GovernancePublishError::other("encoded publication length exceeds host limits")
        })?;
        let json_len_usize = usize::try_from(json_len).map_err(|_| {
            GovernancePublishError::other("JSON publication length exceeds host limits")
        })?;
        validate_governance_car_source_lengths(encoded_len_usize, json_len_usize)?;
        let (expected_encoded_path, expected_json_path) = governance_source_pair_relative_paths(
            payload_kind,
            encoded_len,
            encoded_blake3,
            json_len,
            json_blake3,
        )?;
        if encoded_path != expected_encoded_path || json_path != expected_json_path {
            return Err(GovernancePublishError::other(format!(
                "{context} source paths do not match their composite content identity"
            )));
        }
        let pair_id = governance_source_pair_id(
            payload_kind,
            encoded_len,
            encoded_blake3,
            json_len,
            json_blake3,
        )?;
        if !source_pair_ids.insert(pair_id) {
            return Err(GovernancePublishError::other(format!(
                "{context} duplicates a composite source-pair identity"
            )));
        }
        for (path, role, bytes, digest) in [
            (encoded_path, "encoded", encoded_len, encoded_blake3),
            (json_path, "json", json_len, json_blake3),
        ] {
            let identity = (role, bytes, digest.to_owned());
            if path_identities
                .insert(path.to_owned(), identity.clone())
                .is_some_and(|existing| existing != identity)
            {
                return Err(GovernancePublishError::other(format!(
                    "{context} reuses source path `{path}` for a different identity"
                )));
            }
        }
        let count = payload_kind_counts
            .get(payload_kind)
            .and_then(JsonValue::as_u64)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("payload-kind count overflowed"))?;
        payload_kind_counts.insert(payload_kind.to_owned(), JsonValue::from(count));
        append_index_position(&mut by_encoded_blake3, encoded_blake3, position);
        append_index_position(&mut by_payload_kind, payload_kind, position);
        identities.push(GovernancePublishIdentity {
            payload_kind: payload_kind.to_owned(),
            encoded_path: encoded_path.to_owned(),
            json_path: json_path.to_owned(),
            encoded_blake3: encoded_blake3.to_owned(),
            encoded_len,
            json_blake3: json_blake3.to_owned(),
            json_len,
        });
    }
    if required_governance_u64(index, "entry_count", "governance publish index")?
        != u64::try_from(entries.len()).unwrap_or(u64::MAX)
        || index.get("payload_kind_counts") != Some(&JsonValue::Object(payload_kind_counts))
        || index.get("by_encoded_blake3") != Some(&JsonValue::Object(by_encoded_blake3))
        || index.get("by_payload_kind") != Some(&JsonValue::Object(by_payload_kind))
    {
        return Err(GovernancePublishError::other(
            "governance publish index counters or lookups are stale",
        ));
    }
    Ok(identities)
}
fn validate_governance_car_segment_source_files(
    segment: &JsonMap,
    identity: &GovernancePublishIdentity,
    context: &str,
) -> Result<(), GovernancePublishError> {
    let files = segment
        .get("files")
        .and_then(JsonValue::as_array)
        .filter(|files| files.len() == 4)
        .ok_or_else(|| {
            GovernancePublishError::other(format!("{context} source files are invalid"))
        })?;
    let mut roles = BTreeMap::new();
    let mut total = 0_u64;
    for (position, value) in files.iter().enumerate() {
        let file_context = format!("{context} source file {position}");
        let file = value.as_object().ok_or_else(|| {
            GovernancePublishError::other(format!("{file_context} is not an object"))
        })?;
        const FILE_FIELDS: [&str; 4] = ["role", "path", "bytes", "blake3"];
        require_exact_governance_fields(file, &FILE_FIELDS, &file_context)?;
        let role = required_governance_string(file, "role", &file_context)?;
        if roles.insert(role.to_owned(), file).is_some() {
            return Err(GovernancePublishError::other(format!(
                "{context} has duplicate source roles"
            )));
        }
        index_path_components(required_governance_string(file, "path", &file_context)?)?;
        validate_governance_lower_hex(
            required_governance_string(file, "blake3", &file_context)?,
            32,
            "governance CAR source digest",
        )?;
        total = total
            .checked_add(required_governance_u64(file, "bytes", &file_context)?)
            .ok_or_else(|| GovernancePublishError::other("CAR source byte count overflowed"))?;
    }
    let expected = [
        (
            "encoded",
            identity.encoded_path.clone(),
            identity.encoded_len,
            identity.encoded_blake3.as_str(),
        ),
        (
            "encoded_blake3_sidecar",
            format!("{}.blake3", identity.encoded_path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES as u64,
            "",
        ),
        (
            "json",
            identity.json_path.clone(),
            identity.json_len,
            identity.json_blake3.as_str(),
        ),
        (
            "json_blake3_sidecar",
            format!("{}.blake3", identity.json_path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES as u64,
            "",
        ),
    ];
    for (role, expected_path, expected_bytes, expected_digest) in expected {
        let file = roles.get(role).ok_or_else(|| {
            GovernancePublishError::other(format!("{context} is missing source role `{role}`"))
        })?;
        if required_governance_string(file, "path", context)? != expected_path
            || required_governance_u64(file, "bytes", context)? != expected_bytes
            || (!expected_digest.is_empty()
                && required_governance_string(file, "blake3", context)? != expected_digest)
        {
            return Err(GovernancePublishError::other(format!(
                "{context} source role `{role}` is not bound to its publish entry"
            )));
        }
    }
    if total > GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES as u64
        || required_governance_u64(segment, "payload_bytes", context)? != total
    {
        return Err(GovernancePublishError::other(format!(
            "{context} source aggregate is inconsistent"
        )));
    }
    Ok(())
}
fn register_governance_artifact_owner(
    owners: &mut BTreeMap<String, String>,
    path: &str,
    owner: &str,
    context: &str,
) -> Result<(), GovernancePublishError> {
    index_path_components(path)?;
    if owners
        .insert(path.to_owned(), owner.to_owned())
        .is_some_and(|existing| existing != owner)
    {
        return Err(GovernancePublishError::other(format!(
            "{context} aliases governance artifact path `{path}` across distinct content identities"
        )));
    }
    Ok(())
}
fn validate_governance_car_queue_state(
    queue: &JsonMap,
    identities: &[GovernancePublishIdentity],
) -> Result<(), GovernancePublishError> {
    const QUEUE_FIELDS: [&str; 11] = [
        "schema",
        "source",
        "root",
        "generated_at",
        "segment_count",
        "assembled_count",
        "pending_count",
        "by_encoded_blake3",
        "by_payload_kind",
        "by_car_archive_blake3",
        "segments",
    ];
    require_exact_governance_fields(queue, &QUEUE_FIELDS, "governance CAR queue")?;
    if required_governance_string(queue, "schema", "governance CAR queue")?
        != GOVERNANCE_CAR_QUEUE_SCHEMA
        || required_governance_string(queue, "source", "governance CAR queue")?
            != GOVERNANCE_DAG_SINK_FILESYSTEM
        || required_governance_string(queue, "root", "governance CAR queue")?
            != GOVERNANCE_DAG_LOGICAL_ROOT
    {
        return Err(GovernancePublishError::other(
            "governance CAR queue has an unsupported identity",
        ));
    }
    required_governance_u64(queue, "generated_at", "governance CAR queue")?;
    let segments = queue
        .get("segments")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("governance CAR segments are missing"))?;
    if segments.len() != identities.len() || segments.len() > GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP
    {
        return Err(GovernancePublishError::other(
            "governance publish index and CAR queue are not one-to-one",
        ));
    }
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut by_car_archive_blake3 = JsonMap::new();
    let mut artifact_owners = BTreeMap::<String, String>::new();
    for (position, identity) in identities.iter().enumerate() {
        let pair_id = governance_source_pair_id(
            &identity.payload_kind,
            identity.encoded_len,
            &identity.encoded_blake3,
            identity.json_len,
            &identity.json_blake3,
        )?;
        let owner = format!("source-pair:{pair_id}");
        for path in [
            identity.encoded_path.clone(),
            format!("{}.blake3", identity.encoded_path),
            identity.json_path.clone(),
            format!("{}.blake3", identity.json_path),
        ] {
            register_governance_artifact_owner(
                &mut artifact_owners,
                &path,
                &owner,
                &format!("governance publish entry {position}"),
            )?;
        }
    }
    let mut car_archive_owners = BTreeMap::<String, usize>::new();
    for (position, (value, identity)) in segments.iter().zip(identities).enumerate() {
        let context = format!("governance CAR segment {position}");
        let segment = value
            .as_object()
            .ok_or_else(|| GovernancePublishError::other(format!("{context} is not an object")))?;
        const SEGMENT_FIELDS: [&str; 23] = [
            "schema",
            "queue_position",
            "status",
            "source",
            "source_publish_index_position",
            "payload_kind",
            "encoded_path",
            "json_path",
            "encoded_blake3",
            "encoded_len",
            "car_path",
            "plan_path",
            "manifest_path",
            "car_size",
            "car_archive_blake3",
            "car_payload_blake3",
            "car_cid_hex",
            "root_cids_hex",
            "dag_codec",
            "chunk_count",
            "payload_bytes",
            "files",
            "chunk_profile",
        ];
        require_exact_governance_fields(segment, &SEGMENT_FIELDS, &context)?;
        let position_u64 = u64::try_from(position)
            .map_err(|_| GovernancePublishError::other("CAR queue position exceeds u64"))?;
        if required_governance_string(segment, "schema", &context)? != GOVERNANCE_CAR_SEGMENT_SCHEMA
            || required_governance_string(segment, "status", &context)? != "assembled"
            || required_governance_string(segment, "source", &context)?
                != GOVERNANCE_DAG_SINK_FILESYSTEM
            || required_governance_u64(segment, "queue_position", &context)? != position_u64
            || required_governance_u64(segment, "source_publish_index_position", &context)?
                != position_u64
            || required_governance_string(segment, "payload_kind", &context)?
                != identity.payload_kind
            || required_governance_string(segment, "encoded_path", &context)?
                != identity.encoded_path
            || required_governance_string(segment, "json_path", &context)? != identity.json_path
            || required_governance_string(segment, "encoded_blake3", &context)?
                != identity.encoded_blake3
            || required_governance_u64(segment, "encoded_len", &context)? != identity.encoded_len
        {
            return Err(GovernancePublishError::other(format!(
                "{context} is not bound one-to-one to its publish entry"
            )));
        }
        let pair_id = governance_source_pair_id(
            &identity.payload_kind,
            identity.encoded_len,
            &identity.encoded_blake3,
            identity.json_len,
            &identity.json_blake3,
        )?;
        let base = format!("{GOVERNANCE_CAR_SEGMENTS_DIR}/{position:020}_{pair_id}");
        let artifact_owner = format!("car-segment:{position}:{pair_id}");
        for (field, expected_path) in [
            ("car_path", format!("{base}.car")),
            ("plan_path", format!("{base}.plan.json")),
            ("manifest_path", format!("{base}.json")),
        ] {
            let path = required_governance_string(segment, field, &context)?;
            if path != expected_path {
                return Err(GovernancePublishError::other(format!(
                    "{context} `{field}` is not its canonical composite-identity path"
                )));
            }
            register_governance_artifact_owner(
                &mut artifact_owners,
                path,
                &artifact_owner,
                &context,
            )?;
            register_governance_artifact_owner(
                &mut artifact_owners,
                &format!("{path}.blake3"),
                &artifact_owner,
                &context,
            )?;
        }
        let car_size = required_governance_u64(segment, "car_size", &context)?;
        if car_size == 0 || car_size > GOVERNANCE_CAR_ARCHIVE_MAX_BYTES as u64 {
            return Err(GovernancePublishError::other(format!(
                "{context} CAR size is outside its fixed bound"
            )));
        }
        let car_archive_blake3 =
            required_governance_string(segment, "car_archive_blake3", &context)?;
        validate_governance_lower_hex(car_archive_blake3, 32, "CAR archive digest")?;
        if car_archive_owners
            .insert(car_archive_blake3.to_owned(), position)
            .is_some()
        {
            return Err(GovernancePublishError::other(format!(
                "{context} reuses a CAR archive digest already bound to another segment"
            )));
        }
        validate_governance_lower_hex(
            required_governance_string(segment, "car_payload_blake3", &context)?,
            32,
            "CAR payload digest",
        )?;
        let car_cid = required_governance_string(segment, "car_cid_hex", &context)?;
        validate_governance_lower_hex(car_cid, 36, "CAR CID")?;
        if !car_cid.starts_with("01551f20") {
            return Err(GovernancePublishError::other(format!(
                "{context} CAR CID has a noncanonical codec"
            )));
        }
        let roots = segment
            .get("root_cids_hex")
            .and_then(JsonValue::as_array)
            .filter(|roots| !roots.is_empty() && roots.len() <= 64)
            .ok_or_else(|| GovernancePublishError::other(format!("{context} roots are invalid")))?;
        for root in roots {
            let root = root.as_str().ok_or_else(|| {
                GovernancePublishError::other(format!("{context} root CID is not a string"))
            })?;
            validate_governance_lower_hex(root, 36, "CAR root CID")?;
            if !root.starts_with("01711f20") {
                return Err(GovernancePublishError::other(format!(
                    "{context} root CID has a noncanonical codec"
                )));
            }
        }
        if required_governance_u64(segment, "dag_codec", &context)? != 0x71
            || required_governance_u64(segment, "chunk_count", &context)? == 0
        {
            return Err(GovernancePublishError::other(format!(
                "{context} CAR geometry is noncanonical"
            )));
        }
        let profile = segment
            .get("chunk_profile")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| {
                GovernancePublishError::other(format!("{context} profile is missing"))
            })?;
        const PROFILE_FIELDS: [&str; 4] = ["min_size", "target_size", "max_size", "break_mask"];
        require_exact_governance_fields(profile, &PROFILE_FIELDS, "governance CAR profile")?;
        let default = sorafs_chunker::ChunkProfile::DEFAULT;
        if required_governance_u64(profile, "min_size", &context)? != default.min_size as u64
            || required_governance_u64(profile, "target_size", &context)?
                != default.target_size as u64
            || required_governance_u64(profile, "max_size", &context)? != default.max_size as u64
            || required_governance_u64(profile, "break_mask", &context)? != default.break_mask
        {
            return Err(GovernancePublishError::other(format!(
                "{context} profile is not canonical V1"
            )));
        }
        validate_governance_car_segment_source_files(segment, identity, &context)?;
        append_index_position(&mut by_encoded_blake3, &identity.encoded_blake3, position);
        append_index_position(&mut by_payload_kind, &identity.payload_kind, position);
        append_index_position(&mut by_car_archive_blake3, car_archive_blake3, position);
    }
    let count = u64::try_from(segments.len()).unwrap_or(u64::MAX);
    if required_governance_u64(queue, "segment_count", "governance CAR queue")? != count
        || required_governance_u64(queue, "assembled_count", "governance CAR queue")? != count
        || required_governance_u64(queue, "pending_count", "governance CAR queue")? != 0
        || queue.get("by_encoded_blake3") != Some(&JsonValue::Object(by_encoded_blake3))
        || queue.get("by_payload_kind") != Some(&JsonValue::Object(by_payload_kind))
        || queue.get("by_car_archive_blake3") != Some(&JsonValue::Object(by_car_archive_blake3))
    {
        return Err(GovernancePublishError::other(
            "governance CAR queue counters or lookups are stale",
        ));
    }
    Ok(())
}
fn validate_governance_publication_state(state: &JsonMap) -> Result<(), GovernancePublishError> {
    const STATE_FIELDS: [&str; 7] = [
        "schema",
        "source",
        "root",
        "generation",
        "generated_at",
        "publish_index",
        "car_queue",
    ];
    require_exact_governance_fields(
        state,
        &STATE_FIELDS,
        "authoritative governance publication state",
    )?;
    if required_governance_string(state, "schema", "governance publication state")?
        != GOVERNANCE_PUBLICATION_STATE_SCHEMA
        || required_governance_string(state, "source", "governance publication state")?
            != GOVERNANCE_DAG_SINK_FILESYSTEM
        || required_governance_string(state, "root", "governance publication state")?
            != GOVERNANCE_DAG_LOGICAL_ROOT
    {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state has an unsupported identity",
        ));
    }
    required_governance_u64(state, "generation", "governance publication state")?;
    required_governance_u64(state, "generated_at", "governance publication state")?;
    let index = state
        .get("publish_index")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            GovernancePublishError::other("publication state publish index is missing")
        })?;
    let queue = state
        .get("car_queue")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| GovernancePublishError::other("publication state CAR queue is missing"))?;
    let identities = validate_governance_publish_index_state(index)?;
    validate_governance_car_queue_state(queue, &identities)
}
#[derive(Debug, Default)]
struct GovernancePublicationArtifactInventory {
    source_kind_dirs: BTreeSet<String>,
    source_pair_dirs: BTreeSet<String>,
    source_files: BTreeSet<String>,
    car_files: BTreeSet<String>,
    next_position: usize,
}
fn governance_publication_artifact_inventory(
    state: &JsonMap,
) -> Result<GovernancePublicationArtifactInventory, GovernancePublishError> {
    validate_governance_publication_state(state)?;
    let mut inventory = GovernancePublicationArtifactInventory::default();
    let entries = state
        .get("publish_index")
        .and_then(|index| index.get("entries"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("publication entries are missing"))?;
    inventory.next_position = entries.len();
    for (position, entry) in entries.iter().enumerate() {
        let entry = entry.as_object().ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance publish entry {position} is not an object"
            ))
        })?;
        let encoded_path = required_governance_string(
            entry,
            "encoded_path",
            "governance publication artifact inventory",
        )?;
        let json_path = required_governance_string(
            entry,
            "json_path",
            "governance publication artifact inventory",
        )?;
        let mut components = encoded_path.split('/');
        let (Some(source_root), Some(kind), Some(pair_id), Some(file), None) = (
            components.next(),
            components.next(),
            components.next(),
            components.next(),
            components.next(),
        ) else {
            return Err(GovernancePublishError::other(
                "governance publication source inventory path is noncanonical",
            ));
        };
        if source_root != GOVERNANCE_PUBLICATION_SOURCES_DIR
            || file != "payload.to"
            || json_path != format!("{source_root}/{kind}/{pair_id}/payload.json")
        {
            return Err(GovernancePublishError::other(
                "governance publication source inventory path is noncanonical",
            ));
        }
        let kind_dir = format!("{source_root}/{kind}");
        let pair_dir = format!("{kind_dir}/{pair_id}");
        inventory.source_kind_dirs.insert(kind_dir);
        inventory.source_pair_dirs.insert(pair_dir);
        for path in [encoded_path.to_owned(), json_path.to_owned()] {
            if !inventory.source_files.insert(path.clone())
                || !inventory.source_files.insert(format!("{path}.blake3"))
            {
                return Err(GovernancePublishError::other(
                    "governance publication source inventory aliases an artifact",
                ));
            }
        }
    }
    let segments = state
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("publication CAR segments are missing"))?;
    for (position, segment) in segments.iter().enumerate() {
        let segment = segment.as_object().ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance CAR segment {position} is not an object"
            ))
        })?;
        for field in ["car_path", "plan_path", "manifest_path"] {
            let path = required_governance_string(
                segment,
                field,
                "governance publication artifact inventory",
            )?;
            if !inventory.car_files.insert(path.to_owned())
                || !inventory.car_files.insert(format!("{path}.blake3"))
            {
                return Err(GovernancePublishError::other(
                    "governance publication CAR inventory aliases an artifact",
                ));
            }
        }
    }
    Ok(inventory)
}
fn is_canonical_governance_source_pair_directory(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
fn is_canonical_governance_source_artifact_name(name: &str) -> bool {
    matches!(
        name,
        "payload.to" | "payload.to.blake3" | "payload.json" | "payload.json.blake3"
    )
}
fn reject_governance_publication_recovery_quarantine(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    let root_directory = root_guard.rooted_directory();
    let quarantine = match root_directory
        .open_directory(OsStr::new(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR))
    {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let entry_count = quarantine
        .child_names_bounded(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP)
        .map(|entries| entries.len())
        .map_err(|error| {
            GovernancePublishError::other(format!(
                "governance publication recovery quarantine exceeds its {}-entry hard cap or cannot be inspected ({error}); stop the publisher and clear `{GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR}` offline",
                GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP
            ))
        })?;
    Err(GovernancePublishError::other(format!(
        "governance publication recovery quarantine contains {entry_count} preserved entries; stop the publisher, inspect them, and clear `{GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR}` offline before restart"
    )))
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn prepare_governance_publication_recovery_quarantine(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<governance_rooted_fs::RootedDirectory, GovernancePublishError> {
    reject_governance_publication_recovery_quarantine(root_guard)?;
    let root_directory = root_guard.rooted_directory();
    let quarantine = root_directory
        .open_or_create_directory(OsStr::new(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR))?;
    if !quarantine
        .child_names_bounded(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP)?
        .is_empty()
    {
        return Err(GovernancePublishError::other(
            "governance publication recovery quarantine was populated during creation; offline inspection is required",
        ));
    }
    root_guard.revalidate()?;
    Ok(quarantine)
}
fn governance_publication_atomic_temp_target_name(name: &str) -> Option<&str> {
    let name = name.strip_prefix('.')?;
    let (target_name, suffix) = name.rsplit_once(".tmp-")?;
    if target_name.is_empty() {
        return None;
    }
    let (pid, counter) = suffix.split_once('-')?;
    if pid.is_empty()
        || counter.is_empty()
        || !pid.bytes().all(|byte| byte.is_ascii_digit())
        || !counter.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(target_name)
}
fn governance_artifact_roles_form_write_prefix<const N: usize>(
    present: &BTreeSet<String>,
    write_order: &[&str; N],
) -> bool {
    present.len() <= N
        && write_order
            .iter()
            .take(present.len())
            .all(|role| present.contains(*role))
}
fn canonical_governance_car_artifact_base(name: &str) -> Option<&str> {
    const SUFFIXES: [&str; 6] = [
        ".plan.json.blake3",
        ".json.blake3",
        ".car.blake3",
        ".plan.json",
        ".json",
        ".car",
    ];
    let base = SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))?;
    let (position, pair_id) = base.split_once('_')?;
    (position.len() == 20
        && position.bytes().all(|byte| byte.is_ascii_digit())
        && is_canonical_governance_source_pair_directory(pair_id))
    .then_some(base)
}
#[cfg(test)]
fn is_canonical_governance_car_artifact_name(name: &str) -> bool {
    canonical_governance_car_artifact_base(name).is_some()
}
#[derive(Debug)]
struct GovernancePublicationPlannedFileRemoval {
    directory: governance_rooted_fs::RootedDirectory,
    binding: governance_rooted_fs::FileBinding,
    rollback_rank: usize,
    expected_bytes: Vec<u8>,
    quarantine_slot: OsString,
}
#[derive(Debug)]
struct GovernancePublicationPlannedDirectoryRemoval {
    parent: governance_rooted_fs::RootedDirectory,
    retained: governance_rooted_fs::RootedDirectory,
    quarantine_slot: OsString,
}
#[derive(Debug, Default)]
struct GovernancePublicationArtifactCleanupPlan {
    authority_files: Vec<GovernancePublicationPlannedFileRemoval>,
    source_files: Vec<GovernancePublicationPlannedFileRemoval>,
    source_pair_dirs: Vec<GovernancePublicationPlannedDirectoryRemoval>,
    source_kind_dirs: Vec<GovernancePublicationPlannedDirectoryRemoval>,
    source_root: Option<GovernancePublicationPlannedDirectoryRemoval>,
    car_files: Vec<GovernancePublicationPlannedFileRemoval>,
    car_root: Option<GovernancePublicationPlannedDirectoryRemoval>,
}
#[derive(Debug)]
struct GovernanceInterruptedPublicationIdentity {
    payload_kind: String,
    pair_id: String,
    source_roles_complete: bool,
    verified_source: Option<PublishIndexEntryForCar>,
}
impl GovernanceInterruptedPublicationIdentity {
    fn verified_source(&self) -> Result<&PublishIndexEntryForCar, GovernancePublishError> {
        self.verified_source.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "interrupted governance publication CAR persistence requires a complete verified source pair",
            )
        })
    }
}
fn plan_governance_publication_file_removal(
    directory: &governance_rooted_fs::RootedDirectory,
    name: &OsStr,
    max_bytes: usize,
    expected_bytes: Option<&[u8]>,
    rollback_rank: usize,
    quarantine_slot: OsString,
) -> Result<GovernancePublicationPlannedFileRemoval, GovernancePublishError> {
    let snapshot = directory.read_file(name, max_bytes)?;
    if let Some(expected_bytes) = expected_bytes {
        if snapshot.bytes() != expected_bytes {
            return Err(GovernancePublishError::other(
                "interrupted governance publication CAR role diverges from its canonical source projection",
            ));
        }
    }
    let expected_identity = snapshot.binding().identity();
    let expected_bytes = snapshot.bytes().to_vec();
    let binding = directory
        .removal_file_binding(name, max_bytes)?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "uncommitted governance publication artifact disappeared during reconciliation",
            )
        })?;
    if binding.identity() != expected_identity {
        return Err(GovernancePublishError::other(
            "interrupted governance publication artifact changed after exact comparison",
        ));
    }
    Ok(GovernancePublicationPlannedFileRemoval {
        directory: directory.clone(),
        binding,
        rollback_rank,
        expected_bytes,
        quarantine_slot,
    })
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn plan_private_governance_file_removal(
    directory: &governance_rooted_fs::RootedDirectory,
    name: &OsStr,
    max_bytes: usize,
    rollback_rank: usize,
    quarantine_slot: OsString,
) -> Result<GovernancePublicationPlannedFileRemoval, GovernancePublishError> {
    let snapshot = directory.read_private_file(name, max_bytes)?;
    let expected_identity = snapshot.binding().identity();
    let expected_bytes = snapshot.bytes().to_vec();
    let binding = directory
        .private_removal_file_binding(name, max_bytes)?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "private governance recovery artifact disappeared during reconciliation",
            )
        })?;
    if binding.identity() != expected_identity {
        return Err(GovernancePublishError::other(
            "private governance recovery artifact changed after exact comparison",
        ));
    }
    Ok(GovernancePublicationPlannedFileRemoval {
        directory: directory.clone(),
        binding,
        rollback_rank,
        expected_bytes,
        quarantine_slot,
    })
}
fn governance_source_artifact_max_bytes(target: &str) -> Option<usize> {
    match target {
        "payload.to" => Some(GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES),
        "payload.json" => Some(GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES),
        "payload.to.blake3" | "payload.json.blake3" => Some(GOVERNANCE_DIGEST_SIDECAR_BYTES),
        _ => None,
    }
}
fn governance_car_artifact_max_bytes(role: &str) -> Option<usize> {
    match role {
        ".car" => Some(GOVERNANCE_CAR_ARCHIVE_MAX_BYTES),
        ".plan.json" => Some(GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
        ".json" => Some(GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1),
        ".car.blake3" | ".plan.json.blake3" | ".json.blake3" => {
            Some(GOVERNANCE_DIGEST_SIDECAR_BYTES)
        }
        _ => None,
    }
}
fn governance_digest_sidecar_body(data: &[u8]) -> Vec<u8> {
    let mut body = blake3::hash(data).to_hex().to_string();
    body.push('\n');
    body.into_bytes()
}
fn expected_interrupted_governance_car_role<'a>(
    prepared: &'a PreparedGovernanceCarSegment,
    role: &str,
) -> Option<Cow<'a, [u8]>> {
    match role {
        ".car" => Some(Cow::Borrowed(prepared.car_bytes.as_slice())),
        ".car.blake3" => Some(Cow::Owned(governance_digest_sidecar_body(
            &prepared.car_bytes,
        ))),
        ".plan.json" => Some(Cow::Borrowed(prepared.plan_body.as_bytes())),
        ".plan.json.blake3" => Some(Cow::Owned(governance_digest_sidecar_body(
            prepared.plan_body.as_bytes(),
        ))),
        ".json" => Some(Cow::Borrowed(prepared.manifest_body.as_bytes())),
        ".json.blake3" => Some(Cow::Owned(governance_digest_sidecar_body(
            prepared.manifest_body.as_bytes(),
        ))),
        _ => None,
    }
}
fn governance_artifact_rollback_rank<const N: usize>(
    role: &str,
    is_temporary: bool,
    write_order: &[&str; N],
) -> Result<usize, GovernancePublishError> {
    let position = write_order
        .iter()
        .position(|candidate| *candidate == role)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "interrupted governance publication role is outside its canonical write order",
            )
        })?;
    Ok(if is_temporary { N + position } else { position })
}
fn verify_complete_interrupted_source_pair(
    root_guard: &GovernanceFilesystemRootGuard,
    identity: &mut GovernanceInterruptedPublicationIdentity,
    position: usize,
) -> Result<(), GovernancePublishError> {
    if !identity.source_roles_complete {
        return Ok(());
    }
    let pair_root = root_guard
        .root()
        .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
        .join(&identity.payload_kind)
        .join(&identity.pair_id);
    let encoded_path = pair_root.join("payload.to");
    let json_path = pair_root.join("payload.json");
    let encoded = read_rooted_governance_state_file(
        root_guard,
        &encoded_path,
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
    )?;
    let json = read_rooted_governance_state_file(
        root_guard,
        &json_path,
        GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
    )?;
    validate_governance_car_source_lengths(encoded.bytes().len(), json.bytes().len())?;
    verify_rooted_digest_sidecar(root_guard, &encoded_path, encoded.bytes())?;
    verify_rooted_digest_sidecar(root_guard, &json_path, json.bytes())?;
    let encoded_digest = blake3::hash(encoded.bytes()).to_hex().to_string();
    let json_digest = blake3::hash(json.bytes()).to_hex().to_string();
    let expected_pair_id = governance_source_pair_id(
        &identity.payload_kind,
        u64::try_from(encoded.bytes().len()).map_err(|_| {
            GovernancePublishError::other("interrupted encoded source length exceeds u64")
        })?,
        &encoded_digest,
        u64::try_from(json.bytes().len()).map_err(|_| {
            GovernancePublishError::other("interrupted JSON source length exceeds u64")
        })?,
        &json_digest,
    )?;
    if expected_pair_id != identity.pair_id {
        return Err(GovernancePublishError::other(
            "interrupted governance publication source bytes do not match their composite identity",
        ));
    }
    encoded.binding().verify()?;
    json.binding().verify()?;
    root_guard.revalidate()?;
    identity.verified_source = Some(PublishIndexEntryForCar {
        position,
        newly_inserted: true,
        payload_kind: identity.payload_kind.clone(),
        encoded_path: format!(
            "{GOVERNANCE_PUBLICATION_SOURCES_DIR}/{}/{}/payload.to",
            identity.payload_kind, identity.pair_id
        ),
        json_path: format!(
            "{GOVERNANCE_PUBLICATION_SOURCES_DIR}/{}/{}/payload.json",
            identity.payload_kind, identity.pair_id
        ),
        encoded_blake3: encoded_digest,
        encoded_len: encoded.bytes().len(),
        json_blake3: json_digest,
        json_len: json.bytes().len(),
    });
    Ok(())
}
fn plan_governance_publication_source_artifacts(
    root_guard: &GovernanceFilesystemRootGuard,
    inventory: &GovernancePublicationArtifactInventory,
) -> Result<
    (
        GovernancePublicationArtifactCleanupPlan,
        Option<GovernanceInterruptedPublicationIdentity>,
    ),
    GovernancePublishError,
> {
    let mut plan = GovernancePublicationArtifactCleanupPlan::default();
    let root_directory = root_guard.rooted_directory();
    let sources =
        match root_directory.open_directory(OsStr::new(GOVERNANCE_PUBLICATION_SOURCES_DIR)) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if inventory.source_files.is_empty() {
                    return Ok((plan, None));
                }
                return Err(GovernancePublishError::other(
                    "committed governance publication source directory is missing",
                ));
            }
            Err(error) => return Err(error.into()),
        };
    let kind_bound = inventory
        .source_kind_dirs
        .len()
        .checked_add(GOVERNANCE_PUBLICATION_INTERRUPTED_IDENTITY_ALLOWANCE)
        .ok_or_else(|| GovernancePublishError::other("publication source scan bound overflowed"))?
        .max(1);
    let mut seen = BTreeSet::new();
    let mut interrupted_identity = None;
    let mut interrupted_empty_kind = None::<String>;
    for kind_name in sources.child_names_bounded(kind_bound)? {
        let kind = kind_name.to_str().ok_or_else(|| {
            GovernancePublishError::other("governance publication source kind is not UTF-8")
        })?;
        validate_governance_publication_payload_kind(kind)?;
        let kind_directory = sources.open_directory(&kind_name)?;
        let kind_relative = format!("{GOVERNANCE_PUBLICATION_SOURCES_DIR}/{kind}");
        let expected_pair_count = inventory
            .source_pair_dirs
            .iter()
            .filter(|pair| pair.starts_with(&format!("{kind_relative}/")))
            .count();
        let pair_bound = expected_pair_count
            .checked_add(usize::from(
                interrupted_identity.is_none() && interrupted_empty_kind.is_none(),
            ))
            .ok_or_else(|| {
                GovernancePublishError::other("publication source-pair scan bound overflowed")
            })?
            .max(1);
        let pair_names = kind_directory.child_names_bounded(pair_bound)?;
        let kind_was_empty = pair_names.is_empty();
        for pair_name in pair_names {
            let pair = pair_name.to_str().ok_or_else(|| {
                GovernancePublishError::other(
                    "governance publication source-pair identity is not UTF-8",
                )
            })?;
            if !is_canonical_governance_source_pair_directory(pair) {
                return Err(GovernancePublishError::other(
                    "governance publication source-pair directory is noncanonical",
                ));
            }
            let pair_directory = kind_directory.open_directory(&pair_name)?;
            let pair_relative = format!("{kind_relative}/{pair}");
            let committed_pair = inventory.source_pair_dirs.contains(&pair_relative);
            if !committed_pair
                && (interrupted_identity.is_some() || interrupted_empty_kind.is_some())
            {
                return Err(GovernancePublishError::other(
                    "more than one interrupted governance publication source-pair identity is present",
                ));
            }
            if !committed_pair && inventory.next_position >= GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP {
                return Err(GovernancePublishError::other(
                    "interrupted governance publication exists after the publication entry hard cap",
                ));
            }
            let file_bound = GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT
                .checked_add(GOVERNANCE_PUBLICATION_ATOMIC_TEMP_ALLOWANCE)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "publication source artifact scan bound overflowed",
                    )
                })?;
            let mut canonical_source_files = BTreeSet::new();
            let mut source_temporary_target = None::<String>;
            for file_name in pair_directory.child_names_bounded(file_bound)? {
                let file = file_name.to_str().ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance publication source artifact name is not UTF-8",
                    )
                })?;
                let (target, is_temporary) =
                    if let Some(target) = governance_publication_atomic_temp_target_name(file) {
                        (target, true)
                    } else {
                        (file, false)
                    };
                if !is_canonical_governance_source_artifact_name(target) {
                    return Err(GovernancePublishError::other(
                        "governance publication source artifact name is noncanonical",
                    ));
                }
                let target_relative = format!("{pair_relative}/{target}");
                if committed_pair {
                    if is_temporary || !inventory.source_files.contains(&target_relative) {
                        return Err(GovernancePublishError::other(
                            "committed governance publication source directory contains an uncommitted artifact",
                        ));
                    }
                    if pair_directory.file_identity(&file_name)?.is_none() {
                        return Err(GovernancePublishError::other(
                            "committed governance publication source artifact disappeared during reconciliation",
                        ));
                    }
                    seen.insert(target_relative);
                } else {
                    if is_temporary {
                        if source_temporary_target.replace(target.to_owned()).is_some() {
                            return Err(GovernancePublishError::other(
                                "interrupted governance publication has more than one source atomic temporary",
                            ));
                        }
                    } else {
                        canonical_source_files.insert(target.to_owned());
                    }
                    let max_bytes =
                        governance_source_artifact_max_bytes(target).ok_or_else(|| {
                            GovernancePublishError::other(
                                "interrupted governance publication source role has no byte bound",
                            )
                        })?;
                    let rollback_rank = governance_artifact_rollback_rank(
                        target,
                        is_temporary,
                        &GOVERNANCE_PUBLICATION_SOURCE_WRITE_ORDER,
                    )?;
                    plan.source_files
                        .push(plan_governance_publication_file_removal(
                            &pair_directory,
                            &file_name,
                            max_bytes,
                            None,
                            rollback_rank,
                            OsString::from(format!("source-file-{rollback_rank:02}")),
                        )?);
                }
            }
            if !committed_pair {
                if !governance_artifact_roles_form_write_prefix(
                    &canonical_source_files,
                    &GOVERNANCE_PUBLICATION_SOURCE_WRITE_ORDER,
                ) {
                    return Err(GovernancePublishError::other(
                        "interrupted governance publication source roles are not an exact write prefix",
                    ));
                }
                if source_temporary_target
                    .as_ref()
                    .is_some_and(|temporary_target| {
                        GOVERNANCE_PUBLICATION_SOURCE_WRITE_ORDER.get(canonical_source_files.len())
                            != Some(&temporary_target.as_str())
                    })
                {
                    return Err(GovernancePublishError::other(
                        "interrupted governance publication source atomic temporary is not the exact next write role",
                    ));
                }
                let source_roles_complete = canonical_source_files.len()
                    == GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT;
                interrupted_identity = Some(GovernanceInterruptedPublicationIdentity {
                    payload_kind: kind.to_owned(),
                    pair_id: pair.to_owned(),
                    source_roles_complete,
                    verified_source: None,
                });
                plan.source_pair_dirs
                    .push(GovernancePublicationPlannedDirectoryRemoval {
                        parent: kind_directory.clone(),
                        retained: pair_directory,
                        quarantine_slot: OsString::from("source-pair"),
                    });
            }
        }
        if !inventory.source_kind_dirs.contains(&kind_relative) {
            if kind_was_empty {
                // `resolve_parent` durably creates each component in order, so
                // a crash may leave one new kind before its pair directory.
                if inventory.next_position >= GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP {
                    return Err(GovernancePublishError::other(
                        "interrupted governance publication source kind exists after the publication entry hard cap",
                    ));
                }
                if interrupted_identity.is_some() || interrupted_empty_kind.is_some() {
                    return Err(GovernancePublishError::other(
                        "more than one interrupted governance publication source prefix is present",
                    ));
                }
                interrupted_empty_kind = Some(kind.to_owned());
            } else if interrupted_identity
                .as_ref()
                .is_none_or(|identity| identity.payload_kind != kind)
            {
                return Err(GovernancePublishError::other(
                    "uncommitted governance publication source kind has no exact source-pair identity",
                ));
            }
            plan.source_kind_dirs
                .push(GovernancePublicationPlannedDirectoryRemoval {
                    parent: sources.clone(),
                    retained: kind_directory,
                    quarantine_slot: OsString::from("source-kind"),
                });
        }
    }
    if seen != inventory.source_files {
        return Err(GovernancePublishError::other(
            "one or more committed governance publication source artifacts are missing",
        ));
    }
    if let Some(identity) = interrupted_identity.as_mut() {
        verify_complete_interrupted_source_pair(root_guard, identity, inventory.next_position)?;
    }
    plan.source_files
        .sort_by(|left, right| right.rollback_rank.cmp(&left.rollback_rank));
    if inventory.source_files.is_empty() {
        plan.source_root = Some(GovernancePublicationPlannedDirectoryRemoval {
            parent: root_directory.clone(),
            retained: sources,
            quarantine_slot: OsString::from("source-root"),
        });
    }
    Ok((plan, interrupted_identity))
}
fn plan_governance_publication_car_artifacts(
    root_guard: &GovernanceFilesystemRootGuard,
    inventory: &GovernancePublicationArtifactInventory,
    interrupted_identity: Option<&GovernanceInterruptedPublicationIdentity>,
    plan: &mut GovernancePublicationArtifactCleanupPlan,
) -> Result<(), GovernancePublishError> {
    let root_directory = root_guard.rooted_directory();
    let car_segments = match root_directory.open_directory(OsStr::new(GOVERNANCE_CAR_SEGMENTS_DIR))
    {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if inventory.car_files.is_empty() {
                return Ok(());
            }
            return Err(GovernancePublishError::other(
                "committed governance CAR artifact directory is missing",
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let file_bound = inventory
        .car_files
        .len()
        .checked_add(GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT)
        .and_then(|bound| bound.checked_add(GOVERNANCE_PUBLICATION_ATOMIC_TEMP_ALLOWANCE))
        .ok_or_else(|| GovernancePublishError::other("publication CAR scan bound overflowed"))?
        .max(1);
    let mut seen = BTreeSet::new();
    let mut interrupted_car_base = None::<String>;
    let mut interrupted_car_roles = BTreeSet::new();
    let mut interrupted_car_temporary_role = None::<String>;
    let mut interrupted_car_artifacts = Vec::<(OsString, String, bool)>::new();
    for file_name in car_segments.child_names_bounded(file_bound)? {
        let file = file_name.to_str().ok_or_else(|| {
            GovernancePublishError::other("governance CAR artifact name is not UTF-8")
        })?;
        let (target, is_temporary) =
            if let Some(target) = governance_publication_atomic_temp_target_name(file) {
                (target, true)
            } else {
                (file, false)
            };
        let Some(base) = canonical_governance_car_artifact_base(target) else {
            return Err(GovernancePublishError::other(
                "governance CAR artifact name is noncanonical",
            ));
        };
        let target_relative = format!("{GOVERNANCE_CAR_SEGMENTS_DIR}/{target}");
        if !is_temporary && inventory.car_files.contains(&target_relative) {
            if car_segments.file_identity(&file_name)?.is_none() {
                return Err(GovernancePublishError::other(
                    "committed governance CAR artifact disappeared during reconciliation",
                ));
            }
            seen.insert(target_relative);
        } else {
            if inventory.car_files.contains(&target_relative) {
                return Err(GovernancePublishError::other(
                    "committed governance CAR artifact has an uncommitted atomic temporary",
                ));
            }
            if interrupted_car_base
                .as_ref()
                .is_some_and(|existing| existing != base)
            {
                return Err(GovernancePublishError::other(
                    "interrupted governance publication CAR roles span more than one artifact base",
                ));
            }
            interrupted_car_base = Some(base.to_owned());
            let role = target
                .strip_prefix(base)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "interrupted governance CAR role is not bound to its canonical base",
                    )
                })?
                .to_owned();
            if is_temporary {
                if interrupted_car_temporary_role
                    .replace(role.clone())
                    .is_some()
                {
                    return Err(GovernancePublishError::other(
                        "interrupted governance publication has more than one CAR atomic temporary",
                    ));
                }
            } else {
                interrupted_car_roles.insert(role.clone());
            }
            interrupted_car_artifacts.push((file_name, role, is_temporary));
        }
    }
    if seen != inventory.car_files {
        return Err(GovernancePublishError::other(
            "one or more committed governance CAR artifacts are missing",
        ));
    }
    if let Some(base) = interrupted_car_base {
        if !governance_artifact_roles_form_write_prefix(
            &interrupted_car_roles,
            &GOVERNANCE_PUBLICATION_CAR_WRITE_ORDER,
        ) {
            return Err(GovernancePublishError::other(
                "interrupted governance publication CAR roles are not an exact write prefix",
            ));
        }
        if interrupted_car_temporary_role
            .as_ref()
            .is_some_and(|temporary_role| {
                GOVERNANCE_PUBLICATION_CAR_WRITE_ORDER.get(interrupted_car_roles.len())
                    != Some(&temporary_role.as_str())
            })
        {
            return Err(GovernancePublishError::other(
                "interrupted governance publication CAR atomic temporary is not the exact next write role",
            ));
        }
        let (position, pair_id) = base.split_once('_').ok_or_else(|| {
            GovernancePublishError::other(
                "interrupted governance publication CAR base is noncanonical",
            )
        })?;
        let position = position.parse::<usize>().map_err(|_| {
            GovernancePublishError::other(
                "interrupted governance publication CAR position exceeds host limits",
            )
        })?;
        if position != inventory.next_position {
            return Err(GovernancePublishError::other(format!(
                "interrupted governance publication CAR position {position} is not the exact expected next position {}",
                inventory.next_position
            )));
        }
        let identity = interrupted_identity.ok_or_else(|| {
            GovernancePublishError::other(
                "interrupted governance publication CAR artifacts have no source-pair identity",
            )
        })?;
        if pair_id != identity.pair_id {
            return Err(GovernancePublishError::other(
                "interrupted governance publication source and CAR identities diverge",
            ));
        }
        let verified_source = identity.verified_source()?;
        // Every durable CAR role was atomically promoted and therefore must
        // equal the deterministic projection of the complete source pair.
        // Only the exact next atomic temporary may contain partial bytes.
        let (files, file_records) =
            governance_car_segment_files(root_guard.root(), root_guard, verified_source)?;
        let prepared = prepare_governance_car_segment(
            root_guard.root(),
            verified_source,
            files,
            file_records,
        )?;
        for (file_name, role, is_temporary) in interrupted_car_artifacts {
            let max_bytes = governance_car_artifact_max_bytes(&role).ok_or_else(|| {
                GovernancePublishError::other(
                    "interrupted governance publication CAR role has no byte bound",
                )
            })?;
            let expected = if is_temporary {
                None
            } else {
                Some(
                    expected_interrupted_governance_car_role(&prepared, &role).ok_or_else(
                        || {
                            GovernancePublishError::other(
                                "interrupted governance publication CAR role has no canonical source projection",
                            )
                        },
                    )?,
                )
            };
            let rollback_rank = governance_artifact_rollback_rank(
                &role,
                is_temporary,
                &GOVERNANCE_PUBLICATION_CAR_WRITE_ORDER,
            )?;
            plan.car_files
                .push(plan_governance_publication_file_removal(
                    &car_segments,
                    &file_name,
                    max_bytes,
                    expected.as_deref(),
                    rollback_rank,
                    OsString::from(format!("car-file-{rollback_rank:02}")),
                )?);
        }
        plan.car_files
            .sort_by(|left, right| right.rollback_rank.cmp(&left.rollback_rank));
    }
    if inventory.car_files.is_empty() {
        plan.car_root = Some(GovernancePublicationPlannedDirectoryRemoval {
            parent: root_directory.clone(),
            retained: car_segments,
            quarantine_slot: OsString::from("car-root"),
        });
    }
    Ok(())
}
fn apply_governance_publication_cleanup_plan(
    root_guard: &GovernanceFilesystemRootGuard,
    plan: GovernancePublicationArtifactCleanupPlan,
) -> Result<(), GovernancePublishError> {
    apply_governance_publication_cleanup_plan_with(root_guard, plan, |_| Ok(()))
}
#[cfg(windows)]
fn apply_governance_publication_cleanup_plan_with<AfterStep>(
    root_guard: &GovernanceFilesystemRootGuard,
    plan: GovernancePublicationArtifactCleanupPlan,
    mut after_step: AfterStep,
) -> Result<(), GovernancePublishError>
where
    AfterStep: FnMut(usize) -> Result<(), GovernancePublishError>,
{
    let GovernancePublicationArtifactCleanupPlan {
        authority_files,
        source_files,
        source_pair_dirs,
        source_kind_dirs,
        source_root,
        car_files,
        car_root,
    } = plan;
    root_guard.revalidate()?;
    let mut completed_steps = 0_usize;
    let mut record_step = || -> Result<(), GovernancePublishError> {
        completed_steps = completed_steps.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("governance publication cleanup step overflowed")
        })?;
        after_step(completed_steps)
    };
    for removal in authority_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes: _,
            quarantine_slot: _,
            ..
        } = removal;
        directory.remove_file_binding(binding)?;
        record_step()?;
    }
    // Persistence writes source roles before CAR roles. Rollback is the exact
    // inverse: discard the next CAR temporary, remove durable CAR roles in
    // reverse order, and only then unwind the source prefix.
    for removal in car_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes: _,
            quarantine_slot: _,
            ..
        } = removal;
        directory.remove_file_binding(binding)?;
        record_step()?;
    }
    if let Some(removal) = car_root {
        let GovernancePublicationPlannedDirectoryRemoval {
            parent,
            retained,
            quarantine_slot: _,
        } = removal;
        parent.remove_empty_directory_binding(retained)?;
        record_step()?;
    }
    for removal in source_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes: _,
            quarantine_slot: _,
            ..
        } = removal;
        directory.remove_file_binding(binding)?;
        record_step()?;
    }
    for removal in source_pair_dirs
        .into_iter()
        .chain(source_kind_dirs)
        .chain(source_root)
    {
        let GovernancePublicationPlannedDirectoryRemoval {
            parent,
            retained,
            quarantine_slot: _,
        } = removal;
        parent.remove_empty_directory_binding(retained)?;
        record_step()?;
    }
    root_guard.revalidate()?;
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn apply_governance_publication_cleanup_plan_with<AfterStep>(
    root_guard: &GovernanceFilesystemRootGuard,
    plan: GovernancePublicationArtifactCleanupPlan,
    mut after_step: AfterStep,
) -> Result<(), GovernancePublishError>
where
    AfterStep: FnMut(usize) -> Result<(), GovernancePublishError>,
{
    let planned_count = plan
        .authority_files
        .len()
        .checked_add(plan.source_files.len())
        .and_then(|count| count.checked_add(plan.source_pair_dirs.len()))
        .and_then(|count| count.checked_add(plan.source_kind_dirs.len()))
        .and_then(|count| count.checked_add(usize::from(plan.source_root.is_some())))
        .and_then(|count| count.checked_add(plan.car_files.len()))
        .and_then(|count| count.checked_add(usize::from(plan.car_root.is_some())))
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance publication recovery quarantine entry count overflowed",
            )
        })?;
    if planned_count > GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP {
        return Err(GovernancePublishError::other(format!(
            "governance publication recovery requires {planned_count} quarantine entries, above the {}-entry hard cap",
            GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP
        )));
    }
    let mut slots = BTreeSet::new();
    for removal in plan
        .authority_files
        .iter()
        .chain(&plan.source_files)
        .chain(&plan.car_files)
    {
        slots.insert(removal.quarantine_slot.clone());
    }
    for removal in plan
        .source_pair_dirs
        .iter()
        .chain(&plan.source_kind_dirs)
        .chain(plan.source_root.iter())
        .chain(plan.car_root.iter())
    {
        slots.insert(removal.quarantine_slot.clone());
    }
    if slots.len() != planned_count {
        return Err(GovernancePublishError::other(
            "governance publication recovery quarantine slots are not one-to-one",
        ));
    }
    if planned_count == 0 {
        root_guard.revalidate()?;
        return Ok(());
    }
    let quarantine = prepare_governance_publication_recovery_quarantine(root_guard)?;
    let GovernancePublicationArtifactCleanupPlan {
        authority_files,
        source_files,
        source_pair_dirs,
        source_kind_dirs,
        source_root,
        car_files,
        car_root,
    } = plan;
    root_guard.revalidate()?;
    let mut completed_steps = 0_usize;
    let mut record_step = || -> Result<(), GovernancePublishError> {
        completed_steps = completed_steps.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("governance publication cleanup step overflowed")
        })?;
        after_step(completed_steps)
    };
    for removal in authority_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes,
            quarantine_slot,
            ..
        } = removal;
        let isolated =
            directory.isolate_file_binding(binding, &quarantine, quarantine_slot.as_os_str())?;
        if isolated.bytes() != expected_bytes.as_slice() {
            return Err(GovernancePublishError::other(
                "isolated governance authority temporary changed after exact comparison; the quarantine was preserved for offline inspection",
            ));
        }
        isolated.binding().verify()?;
        record_step()?;
    }
    // Persistence writes source roles before CAR roles. Rollback isolates the
    // exact inverse prefix into a durable, bounded quarantine. POSIX has no
    // conditional unlink-by-descriptor, so no quarantined pathname is ever
    // unlinked while a same-UID process could substitute it.
    for removal in car_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes,
            quarantine_slot,
            ..
        } = removal;
        let isolated =
            directory.isolate_file_binding(binding, &quarantine, quarantine_slot.as_os_str())?;
        if isolated.bytes() != expected_bytes.as_slice() {
            return Err(GovernancePublishError::other(
                "isolated governance CAR artifact changed after exact comparison; the quarantine was preserved for offline inspection",
            ));
        }
        isolated.binding().verify()?;
        record_step()?;
    }
    if let Some(removal) = car_root {
        removal.parent.isolate_empty_directory_binding(
            removal.retained,
            &quarantine,
            removal.quarantine_slot.as_os_str(),
        )?;
        record_step()?;
    }
    for removal in source_files {
        let GovernancePublicationPlannedFileRemoval {
            directory,
            binding,
            expected_bytes,
            quarantine_slot,
            ..
        } = removal;
        let isolated =
            directory.isolate_file_binding(binding, &quarantine, quarantine_slot.as_os_str())?;
        if isolated.bytes() != expected_bytes.as_slice() {
            return Err(GovernancePublishError::other(
                "isolated governance source artifact changed after exact comparison; the quarantine was preserved for offline inspection",
            ));
        }
        isolated.binding().verify()?;
        record_step()?;
    }
    for removal in source_pair_dirs
        .into_iter()
        .chain(source_kind_dirs)
        .chain(source_root)
    {
        removal.parent.isolate_empty_directory_binding(
            removal.retained,
            &quarantine,
            removal.quarantine_slot.as_os_str(),
        )?;
        record_step()?;
    }
    root_guard.revalidate()?;
    let isolated_slots = quarantine
        .child_names_bounded(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if isolated_slots != slots {
        return Err(GovernancePublishError::other(
            "governance publication recovery quarantine changed during isolation; offline inspection is required",
        ));
    }
    let isolated_count = isolated_slots.len();
    Err(GovernancePublishError::other(format!(
        "isolated {isolated_count} interrupted governance publication entries into `{GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR}`; stop the publisher, inspect them, and clear the quarantine offline before restart"
    )))
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn apply_governance_publication_cleanup_plan_with<AfterStep>(
    _root_guard: &GovernanceFilesystemRootGuard,
    _plan: GovernancePublicationArtifactCleanupPlan,
    _after_step: AfterStep,
) -> Result<(), GovernancePublishError>
where
    AfterStep: FnMut(usize) -> Result<(), GovernancePublishError>,
{
    Err(GovernancePublishError::other(
        "governance publication recovery is unsupported on this platform",
    ))
}
fn governance_publish_entry_for_integrity(
    entry: &JsonMap,
    position: usize,
) -> Result<PublishIndexEntryForCar, GovernancePublishError> {
    let context = format!("governance publish entry {position}");
    let encoded_len = usize::try_from(required_governance_u64(entry, "encoded_len", &context)?)
        .map_err(|_| {
            GovernancePublishError::other(format!("{context} length exceeds host limits"))
        })?;
    let json_len =
        usize::try_from(required_governance_u64(entry, "json_len", &context)?).map_err(|_| {
            GovernancePublishError::other(format!("{context} JSON length exceeds host limits"))
        })?;
    validate_governance_car_source_lengths(encoded_len, json_len)?;
    Ok(PublishIndexEntryForCar {
        position,
        newly_inserted: false,
        payload_kind: required_governance_string(entry, "payload_kind", &context)?.to_owned(),
        encoded_path: required_governance_string(entry, "encoded_path", &context)?.to_owned(),
        json_path: required_governance_string(entry, "json_path", &context)?.to_owned(),
        encoded_blake3: required_governance_string(entry, "encoded_blake3", &context)?.to_owned(),
        encoded_len,
        json_blake3: required_governance_string(entry, "json_blake3", &context)?.to_owned(),
        json_len,
    })
}
fn verify_exact_governance_publication_artifact(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    expected: &[u8],
    max_bytes: usize,
    context: &str,
) -> Result<(), GovernancePublishError> {
    let snapshot =
        read_rooted_governance_state_file(root_guard, path, max_bytes).map_err(|error| {
            GovernancePublishError::other(format!(
                "read committed {context} `{}`: {error}",
                path.display()
            ))
        })?;
    if snapshot.bytes() != expected {
        return Err(GovernancePublishError::other(format!(
            "committed {context} `{}` diverges from its authoritative canonical bytes",
            path.display()
        )));
    }
    verify_rooted_digest_sidecar(root_guard, path, snapshot.bytes())?;
    snapshot.binding().verify()?;
    Ok(())
}
fn verify_governance_publication_artifact_integrity(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    state: &JsonMap,
) -> Result<(), GovernancePublishError> {
    let entries = state
        .get("publish_index")
        .and_then(|index| index.get("entries"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("publication entries are missing"))?;
    let segments = state
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| GovernancePublishError::other("publication CAR segments are missing"))?;
    if entries.len() != segments.len() {
        return Err(GovernancePublishError::other(
            "publication entry and CAR segment integrity inventories diverge",
        ));
    }
    for (position, (entry, segment)) in entries.iter().zip(segments).enumerate() {
        let entry = entry.as_object().ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance publish entry {position} is not an object"
            ))
        })?;
        let segment = segment.as_object().ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance CAR segment {position} is not an object"
            ))
        })?;
        let entry = governance_publish_entry_for_integrity(entry, position)?;
        let (files, file_records) = governance_car_segment_files(root, root_guard, &entry)?;
        let prepared = prepare_governance_car_segment(root, &entry, files, file_records)?;
        let mut canonical_segment = prepared.segment.clone();
        canonical_segment.insert(
            "queue_position".into(),
            JsonValue::from(u64::try_from(position).map_err(|_| {
                GovernancePublishError::other("governance CAR queue position exceeds u64")
            })?),
        );
        if segment != &canonical_segment {
            return Err(GovernancePublishError::other(format!(
                "committed governance CAR segment {position} diverges from its canonical source projection"
            )));
        }
        verify_exact_governance_publication_artifact(
            root_guard,
            &prepared.car_path,
            &prepared.car_bytes,
            GOVERNANCE_CAR_ARCHIVE_MAX_BYTES,
            "governance CAR archive",
        )?;
        verify_exact_governance_publication_artifact(
            root_guard,
            &prepared.plan_path,
            prepared.plan_body.as_bytes(),
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
            "governance CAR plan",
        )?;
        verify_exact_governance_publication_artifact(
            root_guard,
            &prepared.manifest_path,
            prepared.manifest_body.as_bytes(),
            GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1,
            "governance CAR segment manifest",
        )?;
    }
    root_guard.revalidate()?;
    Ok(())
}
fn reconcile_governance_publication_artifacts(
    root_guard: &GovernanceFilesystemRootGuard,
    state: &JsonMap,
) -> Result<(), GovernancePublishError> {
    let inventory = governance_publication_artifact_inventory(state)?;
    // Reconciliation is deliberately two-phase: retain an identity-bound,
    // read-only cleanup plan first, then prove every authority-bound byte
    // before applying any removal from an interrupted publication.
    let (mut cleanup_plan, interrupted_identity) =
        plan_governance_publication_source_artifacts(root_guard, &inventory)?;
    plan_governance_publication_car_artifacts(
        root_guard,
        &inventory,
        interrupted_identity.as_ref(),
        &mut cleanup_plan,
    )?;
    verify_governance_publication_artifact_integrity(root_guard.root(), root_guard, state)?;
    root_guard.revalidate()?;
    apply_governance_publication_cleanup_plan(root_guard, cleanup_plan)?;
    Ok(())
}
fn reconcile_current_governance_publication_artifacts(
    root_guard: &GovernanceFilesystemRootGuard,
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<(), GovernancePublishError> {
    let (state, _) = read_governance_publication_state(store)?;
    reconcile_governance_publication_artifacts(root_guard, &state)
}
pub(crate) fn validate_governance_car_source_lengths(
    encoded_len: usize,
    json_len: usize,
) -> Result<usize, GovernancePublishError> {
    if encoded_len == 0 || encoded_len > GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "governance encoded publication length must be in 1..={GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES} bytes"
        )));
    }
    if json_len == 0 || json_len > GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "governance JSON publication length must be in 1..={GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES} bytes"
        )));
    }
    let total = encoded_len
        .checked_add(json_len)
        .and_then(|bytes| bytes.checked_add(2 * GOVERNANCE_DIGEST_SIDECAR_BYTES))
        .ok_or_else(|| {
            GovernancePublishError::other("governance CAR source aggregate length overflowed")
        })?;
    if total > GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "governance CAR sources exceed the {GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES}-byte segment limit"
        )));
    }
    Ok(total)
}
#[expect(
    clippy::too_many_arguments,
    reason = "the publish index binds both exact source-file identities alongside their logical publication metadata"
)]
fn update_publish_index(
    root: &Path,
    mut index: JsonMap,
    payload_kind: &str,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
    json_blake3: &str,
    json_len: usize,
    labels: JsonMap,
) -> Result<(JsonMap, PublishIndexEntryForCar), GovernancePublishError> {
    validate_governance_car_source_lengths(encoded_len, json_len)?;
    validate_governance_publication_labels(&labels, "governance publication")?;
    let mut entries = match index.remove("entries") {
        Some(JsonValue::Array(entries)) => entries,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance publish index has non-array `entries`",
            ));
        }
        None => Vec::new(),
    };
    let encoded_path = index_path_string(root, encoded_path);
    let json_path = index_path_string(root, json_path);
    let labels = JsonValue::Object(labels);
    let duplicate_position = entries.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry.get("encoded_path").and_then(JsonValue::as_str) == Some(encoded_path.as_str())
            && entry.get("json_path").and_then(JsonValue::as_str) == Some(json_path.as_str())
    });
    if duplicate_position.is_none() && entries.len() >= GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP {
        return Err(GovernancePublishError::other(format!(
            "governance publish index reached its {GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP}-entry hard cap"
        )));
    }
    let encoded_len_u64 = u64::try_from(encoded_len).map_err(|_| {
        GovernancePublishError::other("governance encoded publication length exceeds u64")
    })?;
    let json_len_u64 = u64::try_from(json_len).map_err(|_| {
        GovernancePublishError::other("governance JSON publication length exceeds u64")
    })?;
    if let Some(position) = duplicate_position {
        let existing = &entries[position];
        if existing.get("encoded_blake3").and_then(JsonValue::as_str) != Some(digest_hex)
            || existing.get("encoded_len").and_then(JsonValue::as_u64) != Some(encoded_len_u64)
            || existing.get("json_blake3").and_then(JsonValue::as_str) != Some(json_blake3)
            || existing.get("json_len").and_then(JsonValue::as_u64) != Some(json_len_u64)
            || existing.get("labels") != Some(&labels)
        {
            return Err(GovernancePublishError::other(
                "duplicate governance publication changed its source identity or derived labels",
            ));
        }
        index.insert("entries".into(), JsonValue::Array(entries));
        return Ok((
            index,
            PublishIndexEntryForCar {
                position,
                newly_inserted: false,
                payload_kind: payload_kind.to_owned(),
                encoded_path,
                json_path,
                encoded_blake3: digest_hex.to_owned(),
                encoded_len,
                json_blake3: json_blake3.to_owned(),
                json_len,
            },
        ));
    }
    let position = entries.len();
    let mut entry = JsonMap::new();
    entry.insert("position".into(), JsonValue::from(position as u64));
    entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
    entry.insert("encoded_path".into(), JsonValue::from(encoded_path.clone()));
    entry.insert("json_path".into(), JsonValue::from(json_path.clone()));
    entry.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    entry.insert("encoded_len".into(), JsonValue::from(encoded_len_u64));
    entry.insert("json_blake3".into(), JsonValue::from(json_blake3));
    entry.insert("json_len".into(), JsonValue::from(json_len_u64));
    entry.insert(
        "published_at_unix".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    entry.insert("labels".into(), labels);
    entries.push(JsonValue::Object(entry));
    let index = rebuild_publish_index(index, entries)?;
    Ok((
        index,
        PublishIndexEntryForCar {
            position,
            newly_inserted: true,
            payload_kind: payload_kind.to_owned(),
            encoded_path,
            json_path,
            encoded_blake3: digest_hex.to_owned(),
            encoded_len,
            json_blake3: json_blake3.to_owned(),
            json_len,
        },
    ))
}
fn rebuild_publish_index(
    mut index: JsonMap,
    mut entries: Vec<JsonValue>,
) -> Result<JsonMap, GovernancePublishError> {
    let mut payload_kind_counts = JsonMap::new();
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    for (position, entry) in entries.iter_mut().enumerate() {
        let Some(entry_map) = entry.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is not an object",
            ));
        };
        entry_map.insert("position".into(), JsonValue::from(position as u64));
        let Some(payload_kind) = entry_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `payload_kind`",
            ));
        };
        let count = payload_kind_counts
            .get(&payload_kind)
            .and_then(JsonValue::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        payload_kind_counts.insert(payload_kind.clone(), JsonValue::from(count));
        append_index_position(&mut by_payload_kind, &payload_kind, position);
        let Some(digest_hex) = entry_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
    }
    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    index.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    index.insert("entry_count".into(), JsonValue::from(entries.len() as u64));
    index.insert(
        "payload_kind_counts".into(),
        JsonValue::Object(payload_kind_counts),
    );
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("entries".into(), JsonValue::Array(entries));
    Ok(index)
}
fn append_index_position(index: &mut JsonMap, key: &str, position: usize) {
    let position = JsonValue::from(position as u64);
    match index.get_mut(key).and_then(JsonValue::as_array_mut) {
        Some(positions) => positions.push(position),
        None => {
            index.insert(key.to_string(), JsonValue::Array(vec![position]));
        }
    }
}
fn index_path_string(root: &Path, path: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}
#[cfg(test)]
fn assemble_governance_car_queue(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    queue: JsonMap,
    entry: &PublishIndexEntryForCar,
) -> Result<JsonMap, GovernancePublishError> {
    let segment = assemble_governance_car_segment(root, root_guard, entry)?;
    install_governance_car_segment(queue, entry, segment)
}
fn install_governance_car_segment(
    mut queue: JsonMap,
    entry: &PublishIndexEntryForCar,
    segment: JsonMap,
) -> Result<JsonMap, GovernancePublishError> {
    let mut segments = match queue.remove("segments") {
        Some(JsonValue::Array(segments)) => segments,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance CAR queue has non-array `segments`",
            ));
        }
        None => Vec::new(),
    };
    let existing_position = segments.iter().position(|segment| {
        segment
            .get("source_publish_index_position")
            .and_then(JsonValue::as_u64)
            == Some(entry.position as u64)
            && segment.get("encoded_blake3").and_then(JsonValue::as_str)
                == Some(entry.encoded_blake3.as_str())
    });
    match existing_position {
        Some(position) => segments[position] = JsonValue::Object(segment),
        None => segments.push(JsonValue::Object(segment)),
    }
    rebuild_car_queue(queue, segments)
}
fn rebuild_car_queue(
    _previous_queue: JsonMap,
    mut segments: Vec<JsonValue>,
) -> Result<JsonMap, GovernancePublishError> {
    let mut queue = JsonMap::new();
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut by_car_archive_blake3 = JsonMap::new();
    let mut assembled_count = 0u64;
    for (position, segment) in segments.iter_mut().enumerate() {
        let Some(segment_map) = segment.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is not an object",
            ));
        };
        segment_map.insert("queue_position".into(), JsonValue::from(position as u64));
        if segment_map.get("schema").and_then(JsonValue::as_str)
            != Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment uses an unsupported schema",
            ));
        }
        if segment_map.get("status").and_then(JsonValue::as_str) != Some("assembled") {
            return Err(GovernancePublishError::other(
                "governance CAR queue contains a non-producible segment status",
            ));
        }
        let Some(payload_kind) = segment_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `payload_kind`",
            ));
        };
        append_index_position(&mut by_payload_kind, &payload_kind, position);
        let Some(digest_hex) = segment_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
        let Some(car_archive_blake3) = segment_map
            .get("car_archive_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "assembled governance CAR queue segment is missing `car_archive_blake3`",
            ));
        };
        append_index_position(&mut by_car_archive_blake3, &car_archive_blake3, position);
        assembled_count = assembled_count.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("governance CAR queue assembled count overflowed")
        })?;
    }
    queue.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
    );
    queue.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    queue.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    queue.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    queue.insert(
        "segment_count".into(),
        JsonValue::from(segments.len() as u64),
    );
    queue.insert("assembled_count".into(), JsonValue::from(assembled_count));
    let pending_count = 0_u64;
    queue.insert("pending_count".into(), JsonValue::from(pending_count));
    queue.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    queue.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    queue.insert(
        "by_car_archive_blake3".into(),
        JsonValue::Object(by_car_archive_blake3),
    );
    queue.insert("segments".into(), JsonValue::Array(segments));
    record_governance_dag_backlog(pending_count);
    Ok(queue)
}
fn assemble_governance_car_segment(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    entry: &PublishIndexEntryForCar,
) -> Result<JsonMap, GovernancePublishError> {
    let (files, file_records) = governance_car_segment_files(root, root_guard, entry)?;
    let prepared = prepare_governance_car_segment(root, entry, files, file_records)?;
    persist_prepared_governance_car_segment(root_guard, &prepared)?;
    Ok(prepared.segment)
}
fn prepare_governance_car_segment(
    root: &Path,
    entry: &PublishIndexEntryForCar,
    files: Vec<FileEntry>,
    file_records: Vec<JsonValue>,
) -> Result<PreparedGovernanceCarSegment, GovernancePublishError> {
    let (plan, payload) = CarBuildPlan::from_files(files).map_err(|err| {
        GovernancePublishError::other(format!("build governance CAR segment plan: {err}"))
    })?;
    if usize::try_from(plan.content_length).ok() != Some(payload.len())
        || payload.len() > GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES
    {
        return Err(GovernancePublishError::other(
            "governance CAR plan payload exceeds its fixed segment bound",
        ));
    }
    let mut car_output = GovernanceCarBuffer::new();
    let stats = CarWriter::new(&plan, &payload)
        .map_err(|err| GovernancePublishError::other(format!("initialise CAR writer: {err}")))?
        .write_to(&mut car_output)
        .map_err(|err| GovernancePublishError::other(format!("write CAR segment: {err}")))?;
    let car_bytes = car_output.into_bytes();
    if usize::try_from(stats.car_size).ok() != Some(car_bytes.len()) {
        return Err(GovernancePublishError::other(
            "governance CAR writer returned inconsistent bounded archive statistics",
        ));
    }
    let base_path = governance_car_segment_base_path(root, entry)?;
    let car_path = base_path.with_extension("car");
    let plan_path = base_path.with_extension("plan.json");
    let manifest_path = base_path.with_extension("json");
    let plan_json = governance_car_plan_json(entry, &plan, &stats, &file_records);
    let plan_body = json::to_json_pretty(&JsonValue::Object(plan_json)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance CAR plan: {err}"))
    })?;
    if plan_body.is_empty() || plan_body.len() > GOVERNANCE_MUTABLE_INDEX_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "governance CAR plan exceeds its fixed serialized bound",
        ));
    }
    let segment_json = governance_car_segment_json(
        root,
        entry,
        &stats,
        &file_records,
        &car_path,
        &plan_path,
        &manifest_path,
    );
    let segment_body =
        json::to_json_pretty(&JsonValue::Object(segment_json.clone())).map_err(|err| {
            GovernancePublishError::other(format!("serialize governance CAR segment: {err}"))
        })?;
    if segment_body.is_empty() || segment_body.len() > GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1
    {
        return Err(GovernancePublishError::other(
            "governance CAR segment manifest exceeds its fixed serialized bound",
        ));
    }
    Ok(PreparedGovernanceCarSegment {
        segment: segment_json,
        car_path,
        plan_path,
        manifest_path,
        car_bytes,
        plan_body,
        manifest_body: segment_body,
    })
}
fn persist_prepared_governance_car_segment(
    root_guard: &GovernanceFilesystemRootGuard,
    prepared: &PreparedGovernanceCarSegment,
) -> Result<(), GovernancePublishError> {
    write_immutable_governance_artifact(
        root_guard,
        &prepared.car_path,
        &prepared.car_bytes,
        GOVERNANCE_CAR_ARCHIVE_MAX_BYTES,
    )?;
    write_immutable_governance_artifact(
        root_guard,
        &prepared.plan_path,
        prepared.plan_body.as_bytes(),
        GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
    )?;
    write_immutable_governance_artifact(
        root_guard,
        &prepared.manifest_path,
        prepared.manifest_body.as_bytes(),
        GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1,
    )?;
    Ok(())
}
fn governance_car_segment_base_path(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<PathBuf, GovernancePublishError> {
    Ok(root.join(governance_car_segment_relative_base(entry)?))
}
fn governance_car_segment_relative_base(
    entry: &PublishIndexEntryForCar,
) -> Result<String, GovernancePublishError> {
    let pair_id = governance_source_pair_id(
        &entry.payload_kind,
        u64::try_from(entry.encoded_len)
            .map_err(|_| GovernancePublishError::other("encoded source length exceeds u64"))?,
        &entry.encoded_blake3,
        u64::try_from(entry.json_len)
            .map_err(|_| GovernancePublishError::other("JSON source length exceeds u64"))?,
        &entry.json_blake3,
    )?;
    Ok(format!(
        "{GOVERNANCE_CAR_SEGMENTS_DIR}/{:020}_{pair_id}",
        entry.position
    ))
}
fn governance_car_segment_files(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    entry: &PublishIndexEntryForCar,
) -> Result<(Vec<FileEntry>, Vec<JsonValue>), GovernancePublishError> {
    let expected_total = validate_governance_car_source_lengths(entry.encoded_len, entry.json_len)?;
    let encoded_path = resolve_index_path(root, &entry.encoded_path)?;
    let json_path = resolve_index_path(root, &entry.json_path)?;
    let encoded_sidecar = digest_sidecar_path_for(&encoded_path);
    let json_sidecar = digest_sidecar_path_for(&json_path);
    let encoded_sidecar_path = index_path_string(root, &encoded_sidecar);
    let json_sidecar_path = index_path_string(root, &json_sidecar);
    let specs = [
        (
            "encoded",
            entry.encoded_path.as_str(),
            encoded_path,
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
        ),
        (
            "encoded_blake3_sidecar",
            encoded_sidecar_path.as_str(),
            encoded_sidecar,
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
        (
            "json",
            entry.json_path.as_str(),
            json_path,
            GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
        ),
        (
            "json_blake3_sidecar",
            json_sidecar_path.as_str(),
            json_sidecar,
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
    ];
    let mut snapshots = Vec::with_capacity(specs.len());
    for (_, _, absolute_path, max_bytes) in &specs {
        let snapshot = read_rooted_governance_state_file(root_guard, absolute_path, *max_bytes)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "read governance CAR segment source `{}`: {err}",
                    absolute_path.display()
                ))
            })?;
        snapshots.push(snapshot);
    }
    let encoded_bytes = snapshots[0].bytes();
    let encoded_digest = blake3::hash(encoded_bytes).to_hex().to_string();
    if encoded_bytes.len() != entry.encoded_len || encoded_digest != entry.encoded_blake3 {
        return Err(GovernancePublishError::other(format!(
            "governance CAR segment encoded source does not match publish-index length/digest identity {}:{}",
            entry.encoded_len, entry.encoded_blake3
        )));
    }
    let json_bytes = snapshots[2].bytes();
    let json_digest = blake3::hash(json_bytes).to_hex().to_string();
    if json_bytes.len() != entry.json_len || json_digest != entry.json_blake3 {
        return Err(GovernancePublishError::other(format!(
            "governance CAR segment JSON source does not match publish-index length/digest identity {}:{}",
            entry.json_len, entry.json_blake3
        )));
    }
    let encoded_sidecar = format!("{encoded_digest}\n");
    if snapshots[1].bytes() != encoded_sidecar.as_bytes() {
        return Err(GovernancePublishError::other(
            "governance CAR segment encoded digest sidecar does not match retained source bytes",
        ));
    }
    let json_sidecar = format!("{json_digest}\n");
    if snapshots[3].bytes() != json_sidecar.as_bytes() {
        return Err(GovernancePublishError::other(
            "governance CAR segment JSON digest sidecar does not match retained source bytes",
        ));
    }
    for ((role, _, absolute_path, _), snapshot) in specs.iter().zip(&snapshots) {
        snapshot.binding().verify().map_err(|err| {
            GovernancePublishError::other(format!(
                "revalidate governance CAR segment {role} source `{}`: {err}",
                absolute_path.display()
            ))
        })?;
    }
    let actual_total = snapshots.iter().try_fold(0_usize, |total, snapshot| {
        total.checked_add(snapshot.bytes().len()).ok_or_else(|| {
            GovernancePublishError::other("governance CAR source aggregate length overflowed")
        })
    })?;
    if actual_total != expected_total || actual_total > GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "governance CAR source aggregate does not match its bounded publish-index identity",
        ));
    }
    let mut files = Vec::with_capacity(specs.len());
    let mut records = Vec::with_capacity(specs.len());
    for ((role, relative_path, _, _), snapshot) in specs.into_iter().zip(snapshots) {
        let bytes = snapshot.into_bytes();
        let digest_hex = blake3::hash(&bytes).to_hex().to_string();
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            GovernancePublishError::other("governance CAR segment source length exceeds u64")
        })?;
        let mut record = JsonMap::new();
        record.insert("role".into(), JsonValue::from(role));
        record.insert("path".into(), JsonValue::from(relative_path));
        record.insert("bytes".into(), JsonValue::from(byte_len));
        record.insert("blake3".into(), JsonValue::from(digest_hex));
        files.push(FileEntry {
            path: index_path_components(relative_path)?,
            data: bytes,
        });
        records.push(JsonValue::Object(record));
    }
    Ok((files, records))
}
fn governance_car_segment_files_from_source_bytes(
    entry: &PublishIndexEntryForCar,
    encoded: &[u8],
    json_bytes: &[u8],
) -> Result<(Vec<FileEntry>, Vec<JsonValue>), GovernancePublishError> {
    let expected_total = validate_governance_car_source_lengths(encoded.len(), json_bytes.len())?;
    if encoded.len() != entry.encoded_len
        || blake3::hash(encoded).to_hex().as_str() != entry.encoded_blake3
        || json_bytes.len() != entry.json_len
        || blake3::hash(json_bytes).to_hex().as_str() != entry.json_blake3
    {
        return Err(GovernancePublishError::other(
            "in-memory governance CAR sources diverge from their publish-index identity",
        ));
    }
    let encoded_sidecar = format!("{}\n", entry.encoded_blake3).into_bytes();
    let json_sidecar = format!("{}\n", entry.json_blake3).into_bytes();
    let specs = [
        ("encoded", entry.encoded_path.clone(), encoded.to_vec()),
        (
            "encoded_blake3_sidecar",
            format!("{}.blake3", entry.encoded_path),
            encoded_sidecar,
        ),
        ("json", entry.json_path.clone(), json_bytes.to_vec()),
        (
            "json_blake3_sidecar",
            format!("{}.blake3", entry.json_path),
            json_sidecar,
        ),
    ];
    let actual_total = specs.iter().try_fold(0_usize, |total, (_, _, bytes)| {
        total.checked_add(bytes.len()).ok_or_else(|| {
            GovernancePublishError::other("governance CAR source aggregate length overflowed")
        })
    })?;
    if actual_total != expected_total {
        return Err(GovernancePublishError::other(
            "in-memory governance CAR source aggregate is inconsistent",
        ));
    }
    let mut files = Vec::with_capacity(specs.len());
    let mut records = Vec::with_capacity(specs.len());
    for (role, relative_path, bytes) in specs {
        let digest_hex = blake3::hash(&bytes).to_hex().to_string();
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            GovernancePublishError::other("governance CAR source length exceeds u64")
        })?;
        let mut record = JsonMap::new();
        record.insert("role".into(), JsonValue::from(role));
        record.insert("path".into(), JsonValue::from(relative_path.clone()));
        record.insert("bytes".into(), JsonValue::from(byte_len));
        record.insert("blake3".into(), JsonValue::from(digest_hex));
        files.push(FileEntry {
            path: index_path_components(&relative_path)?,
            data: bytes,
        });
        records.push(JsonValue::Object(record));
    }
    Ok((files, records))
}
fn governance_car_plan_json(
    entry: &PublishIndexEntryForCar,
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
) -> JsonMap {
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(GOVERNANCE_CAR_PLAN_SCHEMA));
    root.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    root.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    root.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    root.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    root.insert(
        "content_length".into(),
        JsonValue::from(plan.content_length),
    );
    root.insert(
        "payload_blake3".into(),
        JsonValue::from(plan.payload_digest.to_hex().to_string()),
    );
    root.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    root.insert(
        "chunk_count".into(),
        JsonValue::from(plan.chunks.len() as u64),
    );
    root.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    root.insert("chunk_profile".into(), chunk_profile_json(plan));
    root.insert("chunks".into(), governance_car_chunks_json(plan));
    root
}
fn governance_car_segment_json(
    root: &Path,
    entry: &PublishIndexEntryForCar,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
    car_path: &Path,
    plan_path: &Path,
    manifest_path: &Path,
) -> JsonMap {
    let mut segment = JsonMap::new();
    segment.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_SEGMENT_SCHEMA),
    );
    segment.insert("status".into(), JsonValue::from("assembled"));
    segment.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    segment.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    segment.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    segment.insert(
        "encoded_path".into(),
        JsonValue::from(entry.encoded_path.clone()),
    );
    segment.insert("json_path".into(), JsonValue::from(entry.json_path.clone()));
    segment.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    segment.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    segment.insert(
        "car_path".into(),
        JsonValue::from(index_path_string(root, car_path)),
    );
    segment.insert(
        "plan_path".into(),
        JsonValue::from(index_path_string(root, plan_path)),
    );
    segment.insert(
        "manifest_path".into(),
        JsonValue::from(index_path_string(root, manifest_path)),
    );
    segment.insert("car_size".into(), JsonValue::from(stats.car_size));
    segment.insert(
        "car_archive_blake3".into(),
        JsonValue::from(stats.car_archive_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_payload_blake3".into(),
        JsonValue::from(stats.car_payload_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_cid_hex".into(),
        JsonValue::from(hex::encode(&stats.car_cid)),
    );
    segment.insert(
        "root_cids_hex".into(),
        JsonValue::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| JsonValue::from(hex::encode(cid)))
                .collect(),
        ),
    );
    segment.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    segment.insert(
        "chunk_count".into(),
        JsonValue::from(stats.chunk_count as u64),
    );
    segment.insert("payload_bytes".into(), JsonValue::from(stats.payload_bytes));
    segment.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    segment.insert("chunk_profile".into(), chunk_profile_json_from_stats(stats));
    segment
}
fn chunk_profile_json(plan: &CarBuildPlan) -> JsonValue {
    let profile = plan.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}
fn chunk_profile_json_from_stats(stats: &sorafs_car::CarWriteStats) -> JsonValue {
    let profile = stats.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}
fn governance_car_chunks_json(plan: &CarBuildPlan) -> JsonValue {
    JsonValue::Array(
        plan.chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let mut value = JsonMap::new();
                value.insert("index".into(), JsonValue::from(index as u64));
                value.insert("offset".into(), JsonValue::from(chunk.offset));
                value.insert("length".into(), JsonValue::from(chunk.length as u64));
                value.insert("blake3".into(), JsonValue::from(hex::encode(chunk.digest)));
                JsonValue::Object(value)
            })
            .collect(),
    )
}
fn resolve_index_path(root: &Path, relative_path: &str) -> Result<PathBuf, GovernancePublishError> {
    let components = index_path_components(relative_path)?;
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}
fn index_path_components(relative_path: &str) -> Result<Vec<String>, GovernancePublishError> {
    if relative_path.is_empty()
        || relative_path == "."
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
        || relative_path.len() > GOVERNANCE_RELATIVE_PATH_MAX_BYTES
    {
        return Err(GovernancePublishError::other(
            "governance CAR queue path must be a bounded relative slash-separated path",
        ));
    }
    let mut components = Vec::new();
    for component in relative_path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.len() > GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES
            || components.len() == GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS
        {
            return Err(GovernancePublishError::other(
                "governance CAR queue path contains an invalid or oversized component",
            ));
        }
        components.push(component.to_owned());
    }
    Ok(components)
}
#[derive(Debug, Clone)]
struct RuntimeDagTip {
    sequence: u64,
    block_cid: Vec<u8>,
    node_cid: Vec<u8>,
    timestamp: u64,
}
#[derive(Debug, Clone)]
struct RuntimeDagAuthoritySegmentV1 {
    activation_block_count: u64,
    revision: u64,
    binding: RuntimeDagProviderBindingV1,
}
#[derive(Debug, Clone)]
struct RuntimeDagAuthorityLineageV1 {
    segments: Vec<RuntimeDagAuthoritySegmentV1>,
    transitions: Vec<RuntimeDagQualificationTransitionV1>,
    qualification: RuntimeDagQualificationSummary,
}
// The runtime DAG append helper keeps the filesystem, signer, payload, and
// derived artifact metadata together so every publish path indexes identical
// evidence fields.
#[allow(clippy::too_many_arguments)]
fn append_runtime_signed_dag_payload(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    checkpoint_store: &GovernanceRuntimeDagCheckpointStore,
    payload_kind: &str,
    payload: GovernanceLogPayloadV1,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
    observed_timestamp: u64,
    submission_provenance: Option<GovernanceDagSubmissionProvenanceV1>,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    reconcile_runtime_dag_producer_state(root, root_guard, signer, checkpoint_store)?;
    root_guard.revalidate()?;
    validate_existing_runtime_dag_root(root, signer, checkpoint_store)?;
    root_guard.revalidate()?;
    let mut index = read_runtime_dag_index(root, root_guard, signer, checkpoint_store)?;
    let mut blocks = match index.remove("blocks") {
        Some(JsonValue::Array(blocks)) => blocks,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index has non-array `blocks`",
            ));
        }
        None => Vec::new(),
    };
    let expected_submission_account_digest = submission_provenance
        .as_ref()
        .map(|provenance| hex::encode(provenance.publisher_account_digest));
    let expected_submission_origin = submission_provenance
        .as_ref()
        .map(|provenance| provenance.origin.label());
    let duplicate_position = blocks.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry
                .get("source_payload_blake3")
                .and_then(JsonValue::as_str)
                == Some(digest_hex)
            && json_optional_string_matches(
                entry.get("submission_publisher_account_digest_hex"),
                expected_submission_account_digest.as_deref(),
            )
            && json_optional_string_matches(
                entry.get("submission_origin"),
                expected_submission_origin,
            )
    });
    if let Some(position) = duplicate_position {
        if runtime_dag_index_entry_files_exist(root, &blocks[position]) {
            record_governance_dag_head_age_from_index(&index);
            return Ok(());
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG index references a missing block file",
        ));
    }
    let tip = runtime_dag_tip_from_entries(&blocks)?;
    let sequence = match tip.as_ref() {
        Some(tip) => tip.sequence.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG sequence exhausted")
        })?,
        None => 0,
    };
    let timestamp = tip.as_ref().map_or(observed_timestamp, |tip| {
        observed_timestamp.max(tip.timestamp)
    });
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: tip.as_ref().map(|tip| tip.node_cid.clone()),
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        submission_provenance,
        payload,
        publisher_signature: empty_governance_ed25519_signature(),
    };
    node.node_cid = node.recompute_node_cid().map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG node CID: {err}"))
    })?;
    let node_payload = node.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG node signing payload: {err}"
        ))
    })?;
    node.publisher_signature =
        signer.sign(GovernanceDagSigningPurposeV1::LogNode, &node_payload)?;
    node.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG node: {err}"))
    })?;
    node.verify_publisher_signature().map_err(|err| {
        GovernancePublishError::other(format!(
            "verify governance runtime DAG node signature: {err}"
        ))
    })?;
    let prev_block_cid = tip.as_ref().map(|tip| tip.block_cid.clone());
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp,
        &signer.publisher_peer_id,
        &node,
    )
    .map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG block CID: {err}"))
    })?;
    let mut block = GovernanceDagBlockV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        block_cid,
        prev_block_cid,
        sequence,
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        node,
        block_signature: empty_governance_ed25519_signature(),
    };
    let block_payload = block.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG block signing payload: {err}"
        ))
    })?;
    block.block_signature = signer.sign(GovernanceDagSigningPurposeV1::DagBlock, &block_payload)?;
    block.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG block: {err}"))
    })?;
    let block_count = sequence.checked_add(1).ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG block count exhausted")
    })?;
    let checkpoint_cid = runtime_dag_checkpoint_cid(&blocks, block_count)?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: block.block_cid.clone(),
        block_count,
        generated_at: timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_ed25519_signature(),
    };
    let head_payload = head.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG head signing payload: {err}"
        ))
    })?;
    head.head_signature = signer.sign(GovernanceDagSigningPurposeV1::DagHead, &head_payload)?;
    head.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG head: {err}"))
    })?;
    let block_bytes = block.canonical_bytes().map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG block: {err}"))
    })?;
    let block_position = u64::try_from(blocks.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG block position exceeds u64")
    })?;
    let block_encoded_len = u64::try_from(block_bytes.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG block length exceeds u64")
    })?;
    let source_payload_len = u64::try_from(encoded_len).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG source payload length exceeds u64")
    })?;
    let block_digest_hex = blake3::hash(&block_bytes).to_hex().to_string();
    let block_cid_hex = hex::encode(&block.block_cid);
    let block_path = runtime_dag_block_path(root, sequence, &block_cid_hex);
    let head_bytes = norito::to_bytes(&head).map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG head: {err}"))
    })?;
    let mut entry = JsonMap::new();
    entry.insert("position".into(), JsonValue::from(block_position));
    entry.insert("sequence".into(), JsonValue::from(sequence));
    entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
    entry.insert("encoded_blake3".into(), JsonValue::from(block_digest_hex));
    entry.insert("encoded_len".into(), JsonValue::from(block_encoded_len));
    entry.insert(
        "source_payload_blake3".into(),
        JsonValue::from(digest_hex.to_owned()),
    );
    entry.insert(
        "source_payload_len".into(),
        JsonValue::from(source_payload_len),
    );
    entry.insert(
        "submission_publisher_account_digest_hex".into(),
        block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| JsonValue::from(hex::encode(provenance.publisher_account_digest)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "submission_origin".into(),
        block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| JsonValue::from(provenance.origin.label()))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "encoded_path".into(),
        JsonValue::from(index_path_string(root, encoded_path)),
    );
    entry.insert(
        "json_path".into(),
        JsonValue::from(index_path_string(root, json_path)),
    );
    entry.insert(
        "node_cid_hex".into(),
        JsonValue::from(hex::encode(&block.node.node_cid)),
    );
    entry.insert(
        "prev_node_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.node_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_cid_hex".into(),
        JsonValue::from(block_cid_hex.clone()),
    );
    entry.insert(
        "prev_block_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.block_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_path".into(),
        JsonValue::from(index_path_string(root, &block_path)),
    );
    entry.insert("published_at_unix".into(), JsonValue::from(timestamp));
    blocks.push(JsonValue::Object(entry));
    let index_bytes =
        build_runtime_dag_index_bytes(signer, checkpoint_store, index, blocks, &head)?;
    root_guard.revalidate()?;
    commit_runtime_dag_producer_transaction(
        root,
        root_guard,
        signer,
        checkpoint_store,
        &block_path,
        block_bytes,
        head_bytes,
        index_bytes,
    )?;
    root_guard.revalidate()?;
    record_governance_dag_head_age(head.generated_at);
    Ok(())
}
fn read_runtime_dag_index(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<JsonMap, GovernancePublishError> {
    let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    match committed.index_bytes {
        Some(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance runtime DAG committed index: {err}"
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str)
                != Some(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA)
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index uses an unsupported schema",
                ));
            }
            if map.contains_key("head_path") {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index contains the obsolete loose-head authority field",
                ));
            }
            if map.get("source").and_then(JsonValue::as_str) != Some(GOVERNANCE_DAG_SINK_FILESYSTEM)
                || map.get("root").and_then(JsonValue::as_str) != Some(GOVERNANCE_DAG_LOGICAL_ROOT)
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index source or logical root marker is invalid",
                ));
            }
            validate_runtime_dag_signer_fields(&map, signer)?;
            validate_runtime_dag_checkpoint_store_fields(&map, store)?;
            let canonical =
                json::to_json_pretty(&JsonValue::Object(map.clone())).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "failed to canonicalize governance runtime DAG index: {error}"
                    ))
                })?;
            if canonical.as_bytes() != bytes {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index is not canonical JSON",
                ));
            }
            Ok(map)
        }
        None => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
            insert_runtime_dag_signer_fields(&mut map, signer);
            insert_runtime_dag_checkpoint_store_fields(&mut map, store);
            map.insert("blocks".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
    }
}
fn validate_runtime_dag_signer_fields(
    index: &JsonMap,
    signer: &GovernanceRuntimeDagSigner,
) -> Result<(), GovernancePublishError> {
    let handle = index
        .get("signer_handle")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG index is missing `signer_handle`")
        })?;
    if handle != signer.handle {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index signer handle does not match configured signer",
        ));
    }
    let expected_peer = signer.publisher_peer_id_hex();
    let expected_public_key = signer.publisher_public_key_hex();
    let expected_peer_text = String::from_utf8_lossy(&signer.publisher_peer_id);
    if index.get("publisher_peer_id").and_then(JsonValue::as_str)
        != Some(expected_peer_text.as_ref())
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher peer text does not match configured signer",
        ));
    }
    let peer = index
        .get("publisher_peer_id_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_peer_id_hex`",
            )
        })?;
    if peer != expected_peer {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher peer id does not match configured signer",
        ));
    }
    let public_key = index
        .get("publisher_public_key_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_public_key_hex`",
            )
        })?;
    if public_key != expected_public_key {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher public key does not match configured signer",
        ));
    }
    let revision = index
        .get("signer_revision")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `signer_revision`",
            )
        })?;
    if revision != signer.qualification.revision {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index signer revision does not match configured signer",
        ));
    }
    let policy_digest = index
        .get("signer_policy_digest_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `signer_policy_digest_hex`",
            )
        })?;
    if policy_digest != hex::encode(signer.qualification.policy_digest) {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index signer policy digest does not match configured signer",
        ));
    }
    Ok(())
}
fn validate_runtime_dag_checkpoint_store_fields(
    index: &JsonMap,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    let handle = index
        .get("checkpoint_store_handle")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `checkpoint_store_handle`",
            )
        })?;
    let revision = index
        .get("checkpoint_store_revision")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `checkpoint_store_revision`",
            )
        })?;
    let policy_digest = index
        .get("checkpoint_store_policy_digest_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `checkpoint_store_policy_digest_hex`",
            )
        })?;
    if handle != store.handle
        || revision != store.qualification.revision
        || policy_digest != hex::encode(store.qualification.policy_digest)
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index checkpoint-store binding does not match the configured store",
        ));
    }
    Ok(())
}
fn insert_runtime_dag_signer_fields(index: &mut JsonMap, signer: &GovernanceRuntimeDagSigner) {
    index.insert(
        "signer_handle".into(),
        JsonValue::from(signer.handle.clone()),
    );
    index.insert(
        "publisher_peer_id".into(),
        JsonValue::from(String::from_utf8_lossy(&signer.publisher_peer_id).to_string()),
    );
    index.insert(
        "publisher_peer_id_hex".into(),
        JsonValue::from(signer.publisher_peer_id_hex()),
    );
    index.insert(
        "publisher_public_key_hex".into(),
        JsonValue::from(signer.publisher_public_key_hex()),
    );
    index.insert(
        "signer_revision".into(),
        JsonValue::from(signer.qualification.revision),
    );
    index.insert(
        "signer_policy_digest_hex".into(),
        JsonValue::from(hex::encode(signer.qualification.policy_digest)),
    );
}
fn insert_runtime_dag_checkpoint_store_fields(
    index: &mut JsonMap,
    store: &GovernanceRuntimeDagCheckpointStore,
) {
    index.insert(
        "checkpoint_store_handle".into(),
        JsonValue::from(store.handle.clone()),
    );
    index.insert(
        "checkpoint_store_revision".into(),
        JsonValue::from(store.qualification.revision),
    );
    index.insert(
        "checkpoint_store_policy_digest_hex".into(),
        JsonValue::from(hex::encode(store.qualification.policy_digest)),
    );
}
fn runtime_dag_index_provider_binding(
    index: &JsonMap,
) -> Result<RuntimeDagProviderBindingV1, GovernancePublishError> {
    let fixed_digest = |field: &str| -> Result<[u8; 32], GovernancePublishError> {
        required_runtime_hex(index, field)?
            .as_slice()
            .try_into()
            .map_err(|_| {
                GovernancePublishError::other(format!(
                    "governance runtime DAG index `{field}` is not a 32-byte digest"
                ))
            })
    };
    let publisher_peer_id = required_runtime_hex(index, "publisher_peer_id_hex")?;
    if index.get("publisher_peer_id").and_then(JsonValue::as_str)
        != Some(String::from_utf8_lossy(&publisher_peer_id).as_ref())
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher peer text and canonical bytes diverge",
        ));
    }
    let binding = RuntimeDagProviderBindingV1 {
        signer_handle: required_runtime_string(index, "signer_handle")?,
        signer_revision: required_runtime_u64(index, "signer_revision")?,
        signer_policy_digest: fixed_digest("signer_policy_digest_hex")?,
        checkpoint_store_handle: required_runtime_string(index, "checkpoint_store_handle")?,
        checkpoint_store_revision: required_runtime_u64(index, "checkpoint_store_revision")?,
        checkpoint_store_policy_digest: fixed_digest("checkpoint_store_policy_digest_hex")?,
        publisher_peer_id,
        publisher_public_key: fixed_digest("publisher_public_key_hex")?,
    };
    validate_runtime_dag_provider_binding(&binding)?;
    Ok(binding)
}
fn insert_runtime_dag_provider_binding_fields(
    index: &mut JsonMap,
    binding: &RuntimeDagProviderBindingV1,
) {
    index.insert(
        "signer_handle".into(),
        JsonValue::from(binding.signer_handle.clone()),
    );
    index.insert(
        "signer_revision".into(),
        JsonValue::from(binding.signer_revision),
    );
    index.insert(
        "signer_policy_digest_hex".into(),
        JsonValue::from(hex::encode(binding.signer_policy_digest)),
    );
    index.insert(
        "checkpoint_store_handle".into(),
        JsonValue::from(binding.checkpoint_store_handle.clone()),
    );
    index.insert(
        "checkpoint_store_revision".into(),
        JsonValue::from(binding.checkpoint_store_revision),
    );
    index.insert(
        "checkpoint_store_policy_digest_hex".into(),
        JsonValue::from(hex::encode(binding.checkpoint_store_policy_digest)),
    );
    index.insert(
        "publisher_peer_id".into(),
        JsonValue::from(String::from_utf8_lossy(&binding.publisher_peer_id).to_string()),
    );
    index.insert(
        "publisher_peer_id_hex".into(),
        JsonValue::from(hex::encode(&binding.publisher_peer_id)),
    );
    index.insert(
        "publisher_public_key_hex".into(),
        JsonValue::from(hex::encode(binding.publisher_public_key)),
    );
}
pub(crate) fn runtime_dag_payload_kind(payload: &GovernanceLogPayloadV1) -> &str {
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(_) => "provider_advert",
        GovernanceLogPayloadV1::ReplicationOrder(_) => "replication_order",
        GovernanceLogPayloadV1::PorChallengePublication(_) => "por_challenge_publication",
        GovernanceLogPayloadV1::PorProof(_) => "por_proof",
        GovernanceLogPayloadV1::PdpArchive(_) => "pdp_archive",
        GovernanceLogPayloadV1::AuditVerdict(_) => "audit_verdict",
        GovernanceLogPayloadV1::DealSettlement(_) => "deal_settlement",
        GovernanceLogPayloadV1::SignedReputationSnapshot(_) => "reputation_snapshot",
        GovernanceLogPayloadV1::ModerationBallotEvent(_) => "moderation_ballot_event",
        GovernanceLogPayloadV1::AppealFinanceReport(_) => "appeal_finance_report",
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(_) => "appeal_finance_weekly_rollup",
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(_) => {
            "appeal_finance_settlement_receipt"
        }
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(_) => "orderbook_settlement_receipt",
        GovernanceLogPayloadV1::ExternalPayload(payload) => payload.payload_kind.as_str(),
        GovernanceLogPayloadV1::PorWeeklyReport(_) => "por_weekly_report",
    }
}
pub(crate) fn runtime_dag_payload_kind_is_supported(kind: &str) -> bool {
    const SUPPORTED: &[&str] = &[
        "appeal_finance_report",
        "appeal_finance_settlement_receipt",
        "appeal_finance_weekly_rollup",
        "audit_verdict",
        "deal_settlement",
        "gc_audit",
        "moderation_ballot_event",
        "orderbook_settlement_receipt",
        "pdp_archive",
        "por_challenge_publication",
        "por_proof",
        "por_weekly_report",
        "proof_token_issuance",
        "provider_advert",
        "reconciliation",
        "repair_audit",
        "repair_slash",
        "replication_order",
        "reputation_snapshot",
        "transparency_ledger_publication",
    ];
    SUPPORTED.contains(&kind)
}
fn canonical_runtime_source_payload_len(
    payload: &GovernanceLogPayloadV1,
) -> Result<usize, GovernancePublishError> {
    fn encoded_bounded_len<T: norito::NoritoSerialize>(
        value: &T,
    ) -> Result<usize, GovernancePublishError> {
        let exact = norito::core::encoded_frame_len(value).map_err(|error| {
            GovernancePublishError::other(format!(
                "failed to size canonical governance source payload without allocation: {error}"
            ))
        })?;
        if exact > GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES {
            return Err(GovernancePublishError::other(format!(
                "canonical governance source payload exceeds the V1 producer byte limit of {GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES}"
            )));
        }
        Ok(exact)
    }
    macro_rules! encoded_len {
        ($value:expr) => {
            encoded_bounded_len($value)
        };
    }
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(value) => encoded_len!(value),
        GovernanceLogPayloadV1::ReplicationOrder(value) => encoded_len!(value),
        GovernanceLogPayloadV1::PorChallengePublication(value) => encoded_len!(value),
        GovernanceLogPayloadV1::PorProof(value) => encoded_len!(value),
        GovernanceLogPayloadV1::PdpArchive(value) => encoded_len!(value),
        GovernanceLogPayloadV1::AuditVerdict(value) => encoded_len!(value),
        GovernanceLogPayloadV1::DealSettlement(value) => encoded_len!(value.as_ref()),
        GovernanceLogPayloadV1::SignedReputationSnapshot(value) => encoded_len!(value),
        GovernanceLogPayloadV1::ModerationBallotEvent(value) => encoded_len!(value),
        GovernanceLogPayloadV1::AppealFinanceReport(value) => encoded_len!(value),
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => encoded_len!(value),
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => encoded_len!(value),
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(value) => encoded_len!(value),
        GovernanceLogPayloadV1::ExternalPayload(value) => {
            if value.encoded_payload.is_empty()
                || value.encoded_payload.len() > GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES
            {
                return Err(GovernancePublishError::other(format!(
                    "canonical governance source payload exceeds the V1 producer byte limit of {GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES}"
                )));
            }
            Ok(value.encoded_payload.len())
        }
        GovernanceLogPayloadV1::PorWeeklyReport(value) => encoded_len!(value),
    }
}
fn canonical_runtime_source_payload_bytes(
    payload: &GovernanceLogPayloadV1,
) -> Result<Vec<u8>, GovernancePublishError> {
    fn encode_exact<T: norito::NoritoSerialize>(
        value: &T,
        expected_len: usize,
    ) -> Result<Vec<u8>, GovernancePublishError> {
        let bytes = norito::to_bytes(value).map_err(|error| {
            GovernancePublishError::other(format!(
                "failed to encode canonical governance source payload: {error}"
            ))
        })?;
        if bytes.len() != expected_len {
            return Err(GovernancePublishError::other(
                "canonical governance source payload length changed between preflight and encoding",
            ));
        }
        Ok(bytes)
    }
    let expected_len = canonical_runtime_source_payload_len(payload)?;
    macro_rules! encode {
        ($value:expr) => {
            encode_exact($value, expected_len)
        };
    }
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(value) => encode!(value),
        GovernanceLogPayloadV1::ReplicationOrder(value) => encode!(value),
        GovernanceLogPayloadV1::PorChallengePublication(value) => encode!(value),
        GovernanceLogPayloadV1::PorProof(value) => encode!(value),
        GovernanceLogPayloadV1::PdpArchive(value) => encode!(value),
        GovernanceLogPayloadV1::AuditVerdict(value) => encode!(value),
        GovernanceLogPayloadV1::DealSettlement(value) => encode!(value.as_ref()),
        GovernanceLogPayloadV1::SignedReputationSnapshot(value) => encode!(value),
        GovernanceLogPayloadV1::ModerationBallotEvent(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceReport(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => encode!(value),
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(value) => encode!(value),
        GovernanceLogPayloadV1::ExternalPayload(value) => Ok(value.encoded_payload.clone()),
        GovernanceLogPayloadV1::PorWeeklyReport(value) => encode!(value),
    }
}
fn preflight_runtime_signed_dag_payload(
    payload: &GovernanceLogPayloadV1,
    source_payload_len: usize,
) -> Result<(), GovernancePublishError> {
    let canonical_source_len = canonical_runtime_source_payload_len(payload)?;
    if canonical_source_len == 0
        || canonical_source_len != source_payload_len
        || canonical_source_len > GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES
    {
        return Err(GovernancePublishError::other(format!(
            "canonical governance source payload is outside the V1 producer byte limit of {GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES}"
        )));
    }
    // Every variable-size source variant is semantically bounded before this
    // point, and `encoded_frame_len` above performs a real serialization into
    // a counting sink. The remaining node/block fields are fixed-width except
    // for the already qualified, 128-byte-bounded publisher identity. One
    // fixed envelope allowance therefore bounds the node, and a second bounds
    // the enclosing block, without cloning the payload or allocating dummy
    // node/block frames.
    let node_upper_bound = canonical_source_len
        .checked_add(GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1)
        .ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG node preflight length overflowed")
        })?;
    if node_upper_bound > GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1 {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG node preflight exceeds the V1 signing-payload byte limit of {GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1}"
        )));
    }
    let block_upper_bound = node_upper_bound
        .checked_add(GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG block preflight length overflowed",
            )
        })?;
    if block_upper_bound > GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG block preflight exceeds the V1 canonical byte limit of {GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES}"
        )));
    }
    Ok(())
}
fn decode_canonical_runtime_dag<T>(bytes: &[u8], label: &str) -> Result<T, GovernancePublishError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    let max = bytes.len().max(1);
    let value = norito::decode_from_bytes_with_limits(
        bytes,
        DecodeLimits::new(
            MAX_REPUTATION_TRUST_EDGES,
            max,
            GOVERNANCE_RUNTIME_DAG_DECODE_MAX_TOTAL_ELEMENTS_V1,
            runtime_dag_decode_allocation_limit(max),
            128,
        ),
    )
    .map_err(|error| {
        GovernancePublishError::other(format!("{label} canonical decode failed: {error}"))
    })?;
    let canonical = norito::to_bytes(&value).map_err(|error| {
        GovernancePublishError::other(format!("{label} canonical encode failed: {error}"))
    })?;
    if canonical != bytes {
        return Err(GovernancePublishError::other(format!(
            "{label} bytes are noncanonical"
        )));
    }
    Ok(value)
}
fn runtime_dag_decode_allocation_limit(input_bytes: usize) -> usize {
    input_bytes
        .saturating_mul(GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1)
        .max(GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1)
        .min(GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1)
}
fn add_runtime_dag_audit_bytes(total: &mut u64, len: usize) -> Result<(), GovernancePublishError> {
    let len = u64::try_from(len).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG artifact length exceeds u64")
    })?;
    *total = total.checked_add(len).ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG total byte count overflow")
    })?;
    if *total > GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1 {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG exceeds the {GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1} byte hard cap"
        )));
    }
    Ok(())
}
fn validate_runtime_dag_immutable_file_inventory(
    root: &Path,
) -> Result<(), GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_source(root)?;
    let runtime_root = root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))?;
    for name in runtime_root.child_names_bounded(1)? {
        if name == OsStr::new(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR) {
            continue;
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG immutable root contains an unexpected mutable or malformed artifact",
        ));
    }
    root_guard.revalidate()?;
    Ok(())
}
fn validate_existing_runtime_dag_root(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    authoritative_appeal_finance_weekly_rollups(root, signer, store)?;
    Ok(())
}
/// Authenticate the complete runtime DAG and return its signed weekly rollups.
pub(crate) fn authoritative_appeal_finance_weekly_rollups(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<Vec<AuthoritativeAppealFinanceWeeklyRollup>, GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    let committed_store = open_runtime_dag_committed_store_v1(root, &root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    let (Some(head_bytes), Some(index_bytes)) = (
        committed.head_bytes.as_ref(),
        committed.index_bytes.as_ref(),
    ) else {
        let runtime_root = root.join(GOVERNANCE_RUNTIME_DAG_DIR);
        if fs::symlink_metadata(&runtime_root).is_ok() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG artifacts exist without their signed index",
            ));
        }
        return Ok(Vec::new());
    };
    let index = read_runtime_dag_index(root, &root_guard, signer, store)?;
    let current_binding = runtime_dag_provider_binding(signer, store);
    let indexed_blocks = index
        .get("blocks")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index has non-array or missing `blocks`",
            )
        })?;
    if indexed_blocks.is_empty() || indexed_blocks.len() > GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1
    {
        return Err(GovernancePublishError::other(
            "persisted governance runtime DAG index block count is outside the hard limit",
        ));
    }
    let block_count = u64::try_from(indexed_blocks.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG block count exceeds u64")
    })?;
    let latest_allowed = current_unix_timestamp_seconds()
        .saturating_add(GOVERNANCE_RUNTIME_DAG_MAX_FUTURE_SKEW_SECS_V1);
    let mut total_bytes = 0_u64;
    add_runtime_dag_audit_bytes(&mut total_bytes, index_bytes.len())?;
    let mut blocks = Vec::with_capacity(indexed_blocks.len());
    let mut indexed_block_paths = Vec::with_capacity(indexed_blocks.len());
    let mut expected_by_encoded_blake3 = JsonMap::new();
    let mut expected_by_source_payload_blake3 = JsonMap::new();
    let mut expected_by_payload_kind = JsonMap::new();
    let mut authoritative_weekly_rollups = Vec::new();
    let mut authoritative_weekly_rollup_digests = BTreeSet::new();
    for (position, entry) in indexed_blocks.iter().enumerate() {
        let entry = entry.as_object().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index block entry is not an object",
            )
        })?;
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG position exceeds u64")
        })?;
        if required_runtime_u64(entry, "position")? != position_u64
            || required_runtime_u64(entry, "sequence")? != position_u64
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index position or sequence is noncanonical",
            ));
        }
        let block_path_string = required_runtime_string(entry, "block_path")?;
        let block_path = resolve_index_path(root, &block_path_string)?;
        let block_bytes = read_bounded_governance_state_file(
            &block_path,
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        )?;
        verify_digest_sidecar(&block_path, &block_bytes)?;
        add_runtime_dag_audit_bytes(&mut total_bytes, block_bytes.len())?;
        if required_runtime_u64(entry, "encoded_len")?
            != u64::try_from(block_bytes.len()).map_err(|_| {
                GovernancePublishError::other("governance runtime DAG block length exceeds u64")
            })?
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block length is substituted",
            ));
        }
        let block_digest_hex = blake3::hash(&block_bytes).to_hex().to_string();
        if required_runtime_string(entry, "encoded_blake3")? != block_digest_hex {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block digest is substituted",
            ));
        }
        let block: GovernanceDagBlockV1 =
            decode_canonical_runtime_dag(&block_bytes, "governance runtime DAG block")?;
        block.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "governance runtime DAG block validation failed: {error}"
            ))
        })?;
        let block_cid_hex = hex::encode(&block.block_cid);
        let node_cid_hex = hex::encode(&block.node.node_cid);
        let prev_block_cid_hex = block.prev_block_cid.as_ref().map(hex::encode);
        let prev_node_cid_hex = block.node.prev_cid.as_ref().map(hex::encode);
        if block.sequence != position_u64
            || block.timestamp > latest_allowed
            || required_runtime_string(entry, "block_cid_hex")? != block_cid_hex
            || required_runtime_string(entry, "node_cid_hex")? != node_cid_hex
            || optional_runtime_string(entry, "prev_block_cid_hex")? != prev_block_cid_hex
            || optional_runtime_string(entry, "prev_node_cid_hex")? != prev_node_cid_hex
            || required_runtime_u64(entry, "published_at_unix")? != block.timestamp
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block identity or parent lineage is substituted",
            ));
        }
        let canonical_block_path = runtime_dag_block_path(root, block.sequence, &block_cid_hex);
        if block_path != canonical_block_path
            || block_path_string != index_path_string(root, &canonical_block_path)
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block path is noncanonical",
            ));
        }
        let payload_kind = runtime_dag_payload_kind(&block.node.payload);
        if required_runtime_string(entry, "payload_kind")? != payload_kind {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index payload kind does not match the signed payload",
            ));
        }
        let submission_account_digest = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| hex::encode(provenance.publisher_account_digest));
        let submission_origin = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| provenance.origin.label().to_owned());
        if required_optional_runtime_string(entry, "submission_publisher_account_digest_hex")?
            != submission_account_digest
            || required_optional_runtime_string(entry, "submission_origin")? != submission_origin
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index submission provenance does not match the signed node",
            ));
        }
        let source_path_string = required_runtime_string(entry, "encoded_path")?;
        let source_path = resolve_index_path(root, &source_path_string)?;
        let source_bytes = read_bounded_governance_state_file(
            &source_path,
            GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES,
        )?;
        verify_digest_sidecar(&source_path, &source_bytes)?;
        add_runtime_dag_audit_bytes(&mut total_bytes, source_bytes.len())?;
        let source_len = u64::try_from(source_bytes.len()).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG source payload length exceeds u64",
            )
        })?;
        let source_digest = *blake3::hash(&source_bytes).as_bytes();
        let source_digest_hex = hex::encode(source_digest);
        if required_runtime_u64(entry, "source_payload_len")? != source_len
            || required_runtime_string(entry, "source_payload_blake3")?
                != source_digest_hex.as_str()
            || canonical_runtime_source_payload_bytes(&block.node.payload)? != source_bytes
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG source payload does not match its signed node",
            ));
        }
        if let GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup) = &block.node.payload {
            if source_path.extension().and_then(OsStr::to_str) != Some("to") {
                return Err(GovernancePublishError::other(
                    "signed appeal finance weekly rollup source path must use the canonical `.to` extension",
                ));
            }
            rollup.validate().map_err(|error| {
                GovernancePublishError::other(format!(
                    "signed appeal finance weekly rollup failed validation: {error}"
                ))
            })?;
            if !authoritative_weekly_rollup_digests.insert(source_digest_hex.clone()) {
                return Err(GovernancePublishError::other(
                    "signed governance runtime DAG contains a duplicate appeal finance weekly rollup",
                ));
            }
            authoritative_weekly_rollups.push(AuthoritativeAppealFinanceWeeklyRollup {
                encoded_blake3: source_digest_hex.clone(),
                rollup: rollup.clone(),
            });
        }
        let json_path_string = required_runtime_string(entry, "json_path")?;
        let json_path = resolve_index_path(root, &json_path_string)?;
        let json_bytes =
            read_bounded_governance_state_file(&json_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES)?;
        verify_digest_sidecar(&json_path, &json_bytes)?;
        add_runtime_dag_audit_bytes(&mut total_bytes, json_bytes.len())?;
        append_runtime_index_position(&mut expected_by_payload_kind, payload_kind, position_u64);
        append_runtime_index_position(
            &mut expected_by_encoded_blake3,
            &block_digest_hex,
            position_u64,
        );
        append_runtime_index_position(
            &mut expected_by_source_payload_blake3,
            &source_digest_hex,
            position_u64,
        );
        indexed_block_paths.push(block_path);
        blocks.push(block);
    }
    for (field, expected) in [
        ("by_encoded_blake3", expected_by_encoded_blake3),
        (
            "by_source_payload_blake3",
            expected_by_source_payload_blake3,
        ),
        ("by_payload_kind", expected_by_payload_kind),
    ] {
        if index.get(field) != Some(&JsonValue::Object(expected)) {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG index reverse map `{field}` is substituted"
            )));
        }
    }
    add_runtime_dag_audit_bytes(&mut total_bytes, head_bytes.len())?;
    let head: GovernanceDagHeadV1 =
        decode_canonical_runtime_dag(head_bytes, "governance runtime DAG head")?;
    head.validate().map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG head validation failed: {error}"
        ))
    })?;
    let authority_lineage = runtime_dag_authority_lineage(root, &current_binding)?;
    let authority_blocks = blocks.iter().collect::<Vec<_>>();
    validate_runtime_dag_authority_lineage_for_chain(&authority_lineage, &authority_blocks, &head)?;
    if head.generated_at > latest_allowed {
        return Err(GovernancePublishError::other(
            "governance runtime DAG head is future-dated",
        ));
    }
    validate_governance_dag_head_against_rotatable_chain_v1(&head, &blocks).map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG signed head does not authenticate its retained chain: {error}"
        ))
    })?;
    if required_runtime_u64(&index, "block_count")? != block_count
        || required_runtime_string(&index, "head_block_cid_hex")?
            != hex::encode(&head.head_block_cid)
        || required_runtime_u64(&index, "generated_at")? != head.generated_at
        || required_runtime_u64(&index, "head_generated_at")? != head.generated_at
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index and signed head are inconsistent",
        ));
    }
    let blocks_dir = root
        .join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR);
    let blocks_dir_metadata = fs::symlink_metadata(&blocks_dir)?;
    if !blocks_dir_metadata.file_type().is_dir() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG blocks path is not a directory",
        ));
    }
    for entry in fs::read_dir(&blocks_dir)? {
        let path = entry?.path();
        let expected = indexed_block_paths
            .iter()
            .any(|indexed| path == *indexed || path == digest_sidecar_path_for(indexed));
        if !expected {
            return Err(GovernancePublishError::other(
                "governance runtime DAG contains an unindexed block or sidecar and may have been rolled back",
            ));
        }
    }
    validate_runtime_dag_immutable_file_inventory(root)?;
    Ok(authoritative_weekly_rollups)
}
pub(crate) fn runtime_dag_producer_root_digest(
    root: &Path,
) -> Result<[u8; 32], GovernancePublishError> {
    let canonical = fs::canonicalize(root).map_err(|error| {
        GovernancePublishError::other(format!(
            "canonicalize governance runtime DAG producer root: {error}"
        ))
    })?;
    let canonical = canonical.to_str().ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG producer root is not canonical UTF-8")
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.local-producer-root.v1\0");
    hasher.update(
        &u64::try_from(canonical.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical.as_bytes());
    Ok(*hasher.finalize().as_bytes())
}
fn runtime_dag_provider_binding(
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> RuntimeDagProviderBindingV1 {
    RuntimeDagProviderBindingV1 {
        signer_handle: signer.handle.clone(),
        signer_revision: signer.qualification.revision,
        signer_policy_digest: signer.qualification.policy_digest,
        checkpoint_store_handle: store.handle.clone(),
        checkpoint_store_revision: store.qualification.revision,
        checkpoint_store_policy_digest: store.qualification.policy_digest,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        publisher_public_key: signer.public_key,
    }
}
fn validate_runtime_dag_provider_binding(
    binding: &RuntimeDagProviderBindingV1,
) -> Result<(), GovernancePublishError> {
    validate_runtime_handle(
        &binding.signer_handle,
        "governance runtime DAG transition signer",
    )?;
    validate_runtime_handle(
        &binding.checkpoint_store_handle,
        "governance runtime DAG transition checkpoint store",
    )?;
    if binding.signer_revision == 0
        || binding.signer_policy_digest == [0; 32]
        || binding.checkpoint_store_revision == 0
        || binding.checkpoint_store_policy_digest == [0; 32]
        || binding.publisher_peer_id.is_empty()
        || binding.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || binding.publisher_public_key == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition contains an invalid provider binding",
        ));
    }
    let key = DalekVerifyingKey::from_bytes(&binding.publisher_public_key).map_err(|_| {
        GovernancePublishError::other(
            "governance runtime DAG provider transition contains a malformed Ed25519 key",
        )
    })?;
    if key.to_bytes() != binding.publisher_public_key || key.is_weak() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition contains a noncanonical or weak Ed25519 key",
        ));
    }
    Ok(())
}
fn runtime_dag_transition_body_digest(
    body: &RuntimeDagQualificationTransitionBodyV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let canonical = norito::to_bytes(body).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG provider transition body: {error}"
        ))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.provider-transition-body.v1\0");
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}
/// Build the exact predecessor-bound Governance DAG key-transition payload.
pub fn governance_dag_key_transition_signing_payload_v1(
    outgoing_segment_revision: u64,
    incoming_segment_revision: u64,
    transition_body_digest: [u8; 32],
) -> Result<Vec<u8>, GovernancePublishError> {
    let payload = RuntimeDagKeyTransitionSigningPayloadV1 {
        version: GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1,
        outgoing_segment_revision,
        incoming_segment_revision,
        transition_body_digest,
    };
    let canonical = norito::to_bytes(&payload).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG key-transition signing payload: {error}"
        ))
    })?;
    let mut payload =
        Vec::with_capacity(b"sorafs.governance-dag.key-transition.v1\0".len() + canonical.len());
    payload.extend_from_slice(b"sorafs.governance-dag.key-transition.v1\0");
    payload.extend_from_slice(&canonical);
    Ok(payload)
}
fn runtime_dag_archive_signing_bytes(
    body: &RuntimeDagQualificationArchiveBodyV1,
) -> Result<Vec<u8>, GovernancePublishError> {
    let canonical = norito::to_bytes(body).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG qualification archive body: {error}"
        ))
    })?;
    let mut payload = Vec::with_capacity(
        b"sorafs.governance-dag.qualification-archive.v1\0".len() + canonical.len(),
    );
    payload.extend_from_slice(b"sorafs.governance-dag.qualification-archive.v1\0");
    payload.extend_from_slice(&canonical);
    Ok(payload)
}
fn runtime_dag_raw_signature(
    signer: &GovernanceRuntimeDagSigner,
    purpose: GovernanceDagSigningPurposeV1,
    payload: &[u8],
) -> Result<[u8; 64], GovernancePublishError> {
    signer
        .sign(purpose, payload)?
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer returned a noncanonical signature length",
            )
        })
}
fn verify_runtime_dag_binding_signature(
    binding: &RuntimeDagProviderBindingV1,
    payload: &[u8],
    signature: &[u8; 64],
    label: &str,
) -> Result<(), GovernancePublishError> {
    validate_runtime_dag_provider_binding(binding)?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &binding.publisher_public_key)
        .map_err(|_| {
            GovernancePublishError::other(format!(
                "{label} contains a malformed Ed25519 public key"
            ))
        })?;
    let signature = IrohaSignature::try_from_bytes(signature).map_err(|_| {
        GovernancePublishError::other(format!("{label} contains a malformed Ed25519 signature"))
    })?;
    signature.verify(&public_key, payload).map_err(|_| {
        GovernancePublishError::other(format!(
            "{label} signature does not authenticate its canonical bytes"
        ))
    })
}
fn runtime_dag_transition_digest(
    transition: &RuntimeDagQualificationTransitionV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let canonical = norito::to_bytes(transition).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG provider transition: {error}"
        ))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.provider-transition-digest.v1\0");
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}
fn runtime_dag_archive_digest(
    archive: &RuntimeDagQualificationArchiveV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let canonical = norito::to_bytes(archive).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG qualification archive: {error}"
        ))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.qualification-archive-digest.v1\0");
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}
fn validate_runtime_dag_qualification_transition(
    transition: &RuntimeDagQualificationTransitionV1,
    root_digest: [u8; 32],
) -> Result<[u8; 32], GovernancePublishError> {
    let body = &transition.body;
    let key_transition = &transition.key_transition;
    validate_runtime_dag_provider_binding(&body.previous)?;
    validate_runtime_dag_provider_binding(&body.next)?;
    let incoming_segment_revision = body.generation.checked_add(1).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG key-transition segment revision exhausted",
        )
    })?;
    let transition_body_digest = runtime_dag_transition_body_digest(body)?;
    let empty_head = body.block_count == 0;
    let empty_head_fields = body.head_block_cid == [0; 32]
        && body.head_bytes_digest == [0; 32]
        && body.predecessor_index_digest == [0; 32]
        && body.successor_index_digest == [0; 32];
    if body.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TRANSITION_VERSION_V1
        || body.root_digest != root_digest
        || body.generation == 0
        || body.predecessor_checkpoint_revision == [0; 32]
        || body.previous == body.next
        || key_transition.version != GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1
        || key_transition.outgoing_segment_revision != body.generation
        || key_transition.incoming_segment_revision != incoming_segment_revision
        || key_transition.transition_body_digest != transition_body_digest
        || empty_head != empty_head_fields
        || (!empty_head
            && (body.head_block_cid == [0; 32]
                || body.head_bytes_digest == [0; 32]
                || body.predecessor_index_digest == [0; 32]
                || body.successor_index_digest == [0; 32]
                || body.predecessor_index_digest == body.successor_index_digest))
        || (body.archive_generation == 0) != (body.archive_digest == [0; 32])
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition is malformed, substituted, or does not bind one exact head",
        ));
    }
    let payload = governance_dag_key_transition_signing_payload_v1(
        key_transition.outgoing_segment_revision,
        key_transition.incoming_segment_revision,
        key_transition.transition_body_digest,
    )?;
    verify_runtime_dag_binding_signature(
        &body.previous,
        &payload,
        &key_transition.outgoing_signature,
        "governance runtime DAG outgoing key transition",
    )?;
    verify_runtime_dag_binding_signature(
        &body.next,
        &payload,
        &key_transition.incoming_signature,
        "governance runtime DAG incoming key transition",
    )?;
    runtime_dag_transition_digest(transition)
}
fn validate_runtime_dag_qualification_archive(
    archive: &RuntimeDagQualificationArchiveV1,
    root_digest: [u8; 32],
) -> Result<[u8; 32], GovernancePublishError> {
    let body = &archive.body;
    validate_runtime_dag_provider_binding(&body.signer)?;
    if body.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_VERSION_V1
        || body.root_digest != root_digest
        || body.archive_generation == 0
        || body.archive_generation > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1
        || (body.archive_generation == 1) != (body.predecessor_archive_digest == [0; 32])
        || body.transitions.is_empty()
        || body.transitions.len() > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_TRANSITIONS_V1
        || body.first_transition_generation == 0
        || body.first_transition_generation > body.last_transition_generation
        || u64::try_from(body.transitions.len()).ok()
            != body
                .last_transition_generation
                .checked_sub(body.first_transition_generation)
                .and_then(|distance| distance.checked_add(1))
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive is malformed or outside its V1 bounds",
        ));
    }
    let payload = runtime_dag_archive_signing_bytes(body)?;
    verify_runtime_dag_binding_signature(
        &body.signer,
        &payload,
        &archive.signature,
        "governance runtime DAG qualification archive",
    )?;
    let mut expected_generation = body.first_transition_generation;
    let mut expected_predecessor = if body.predecessor_transition_digest == [0; 32] {
        None
    } else {
        Some(body.predecessor_transition_digest)
    };
    let mut previous_block_count = None;
    let mut tail_digest = [0; 32];
    for transition in &body.transitions {
        let digest = validate_runtime_dag_qualification_transition(transition, root_digest)?;
        if transition.body.generation != expected_generation
            || transition.body.predecessor_transition_digest != expected_predecessor
            || previous_block_count
                .is_some_and(|block_count| transition.body.block_count < block_count)
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive contains a fork, gap, duplicate, or rollback",
            ));
        }
        expected_generation = expected_generation.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification transition generation exhausted",
            )
        })?;
        expected_predecessor = Some(digest);
        previous_block_count = Some(transition.body.block_count);
        tail_digest = digest;
    }
    if body.last_transition_generation.checked_add(1) != Some(expected_generation)
        || body.tail_transition_digest != tail_digest
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive tail is substituted",
        ));
    }
    runtime_dag_archive_digest(archive)
}
fn runtime_dag_qualification_history_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_FILE)
}
fn runtime_dag_qualification_archive_path(
    root: &Path,
    generation: u64,
    digest: [u8; 32],
) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR)
        .join(format!("{generation:020}_{}.to", hex::encode(digest)))
}
fn parse_runtime_dag_qualification_archive_name(name: &str) -> Option<(u64, [u8; 32])> {
    let stem = name.strip_suffix(".to")?;
    let (generation, digest) = stem.split_once('_')?;
    if generation.len() != 20 || digest.len() != 64 {
        return None;
    }
    let generation = generation.parse().ok()?;
    let digest = hex::decode(digest).ok()?;
    Some((generation, digest.as_slice().try_into().ok()?))
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn runtime_dag_qualification_archive_temp_inventory(
    directory: &governance_rooted_fs::RootedDirectory,
    next_generation: u64,
) -> Result<(Vec<(OsString, usize)>, bool), GovernancePublishError> {
    let canonical_entries = usize::try_from(
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1.saturating_mul(2),
    )
    .map_err(|_| {
        GovernancePublishError::other(
            "governance runtime DAG qualification archive inventory bound exceeds host limits",
        )
    })?;
    let inventory_limit = canonical_entries
        .checked_add(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive recovery bound overflowed",
            )
        })?;
    let entries = directory.child_names_bounded(inventory_limit)?;
    let directory_was_empty = entries.is_empty();
    let mut temporaries = Vec::new();
    for name in entries {
        let name_utf8 = name.to_str().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive name is not UTF-8",
            )
        })?;
        let Some(target) = governance_publication_atomic_temp_target_name(name_utf8) else {
            if name_utf8.starts_with('.') && name_utf8.contains(".tmp-") {
                return Err(GovernancePublishError::other(format!(
                    "governance runtime DAG qualification archive temporary `{name_utf8}` is noncanonical; offline inspection is required"
                )));
            }
            continue;
        };
        let (archive_name, max_bytes) = target.strip_suffix(".blake3").map_or(
            (
                target,
                GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_BYTES_V1,
            ),
            |archive| (archive, GOVERNANCE_DIGEST_SIDECAR_BYTES),
        );
        let Some((generation, digest)) = parse_runtime_dag_qualification_archive_name(archive_name)
        else {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG qualification archive temporary `{name_utf8}` claims a noncanonical target; offline inspection is required"
            )));
        };
        let canonical = format!("{generation:020}_{}.to", hex::encode(digest));
        if generation != next_generation || archive_name != canonical {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG qualification archive temporary `{name_utf8}` is outside the exact next-generation namespace; offline inspection is required"
            )));
        }
        temporaries.push((name, max_bytes));
    }
    if temporaries.len() > GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive temporaries exceed the recovery quarantine bound",
        ));
    }
    Ok((temporaries, directory_was_empty))
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn isolate_runtime_dag_qualification_archive_temps(
    root_guard: &GovernanceFilesystemRootGuard,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<(), GovernancePublishError> {
    reject_governance_publication_recovery_quarantine(root_guard)?;
    let next_generation = checkpoint
        .qualification_archive_generation
        .checked_add(1)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive generation exhausted",
            )
        })?;
    let directory = match root_guard.rooted_directory().open_directory(OsStr::new(
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
    )) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let (temporaries, directory_was_empty) =
        runtime_dag_qualification_archive_temp_inventory(&directory, next_generation)?;
    if temporaries.is_empty() {
        if directory_was_empty {
            root_guard
                .rooted_directory()
                .remove_empty_directory_binding(directory)?;
        }
        root_guard.revalidate()?;
        return Ok(());
    }
    let mut plan = GovernancePublicationArtifactCleanupPlan::default();
    for (name, max_bytes) in temporaries {
        let rollback_rank = plan.authority_files.len();
        plan.authority_files
            .push(plan_private_governance_file_removal(
                &directory,
                &name,
                max_bytes,
                rollback_rank,
                OsString::from(format!(
                    "runtime-dag-qualification-archive-{rollback_rank:02}"
                )),
            )?);
    }
    root_guard.revalidate()?;
    apply_governance_publication_cleanup_plan(root_guard, plan)
}
#[cfg(windows)]
fn isolate_runtime_dag_qualification_archive_temps(
    root_guard: &GovernanceFilesystemRootGuard,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<(), GovernancePublishError> {
    let next_generation = checkpoint
        .qualification_archive_generation
        .checked_add(1)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive generation exhausted",
            )
        })?;
    let directory = match root_guard.rooted_directory().open_directory(OsStr::new(
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
    )) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let (temporaries, _) =
        runtime_dag_qualification_archive_temp_inventory(&directory, next_generation)?;
    for (name, _) in temporaries {
        let target = name
            .to_str()
            .and_then(governance_publication_atomic_temp_target_name)
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "validated qualification archive temporary lost its target",
                )
            })?;
        directory.remove_atomic_temps_for(target)?;
    }
    root_guard.revalidate()?;
    Ok(())
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn isolate_runtime_dag_qualification_archive_temps(
    _root_guard: &GovernanceFilesystemRootGuard,
    _checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<(), GovernancePublishError> {
    Err(GovernancePublishError::other(
        "governance runtime DAG qualification archive recovery is unsupported on this platform",
    ))
}
fn staged_runtime_dag_qualification_archive(
    root: &Path,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<Option<(u64, [u8; 32])>, GovernancePublishError> {
    let next_generation = checkpoint
        .qualification_archive_generation
        .checked_add(1)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive generation exhausted",
            )
        })?;
    let directory = root.join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR);
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut candidate = None;
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive name is not UTF-8",
            ));
        };
        let Some((generation, digest)) = parse_runtime_dag_qualification_archive_name(&name) else {
            continue;
        };
        if generation != next_generation {
            continue;
        }
        if candidate.replace((generation, digest)).is_some() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive directory contains duplicate staged successors",
            ));
        }
    }
    Ok(candidate)
}
fn write_runtime_dag_qualification_state<T: norito::NoritoSerialize>(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    value: &T,
    max_bytes: usize,
    immutable: bool,
) -> Result<Vec<u8>, GovernancePublishError> {
    let bytes = norito::to_bytes(value).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode canonical governance runtime DAG qualification state: {error}"
        ))
    })?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification state exceeds its canonical byte bound",
        ));
    }
    let replacement = match read_rooted_governance_state_file(root_guard, path, max_bytes) {
        Ok(current) if current.bytes() == bytes => {
            current.binding().verify()?;
            None
        }
        Ok(_) if immutable => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive path is already occupied by substituted bytes",
            ));
        }
        Ok(current) => Some(governance_rooted_fs::ExpectedFile::Identity(
            current.binding(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if immutable {
                match read_rooted_governance_state_file(
                    root_guard,
                    &digest_sidecar_path_for(path),
                    65,
                ) {
                    Ok(_) => {
                        return Err(GovernancePublishError::other(
                            "governance runtime DAG qualification archive has an orphan digest sidecar",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
            Some(governance_rooted_fs::ExpectedFile::Missing)
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(expected) = replacement {
        write_rooted_atomic_expected(root_guard, path, &bytes, expected)?;
    }
    if immutable {
        ensure_rooted_digest_sidecar_immutable(root_guard, path, &bytes)?;
    } else {
        write_digest_sidecar(root_guard, path, &bytes)?;
    }
    let readback = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    verify_rooted_digest_sidecar(root_guard, path, readback.bytes())?;
    if readback.bytes() != bytes {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification state durable readback diverged",
        ));
    }
    readback.binding().verify()?;
    Ok(bytes)
}
fn read_runtime_dag_qualification_archive(
    root: &Path,
    generation: u64,
    digest: [u8; 32],
    root_digest: [u8; 32],
) -> Result<RuntimeDagQualificationArchiveV1, GovernancePublishError> {
    let path = runtime_dag_qualification_archive_path(root, generation, digest);
    let bytes = read_bounded_governance_state_file(
        &path,
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_BYTES_V1,
    )?;
    let missing_sidecar = match verify_digest_sidecar(&path, &bytes) {
        Ok(()) => false,
        Err(_error)
            if fs::symlink_metadata(digest_sidecar_path_for(&path))
                .is_err_and(|sidecar_error| sidecar_error.kind() == io::ErrorKind::NotFound) =>
        {
            true
        }
        Err(error) => return Err(error),
    };
    let archive: RuntimeDagQualificationArchiveV1 =
        decode_canonical_runtime_dag(&bytes, "governance runtime DAG qualification archive")?;
    if validate_runtime_dag_qualification_archive(&archive, root_digest)? != digest
        || archive.body.archive_generation != generation
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive path, generation, or digest is substituted",
        ));
    }
    if missing_sidecar {
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
        ensure_rooted_digest_sidecar_immutable(&root_guard, &path, &bytes)?;
        verify_digest_sidecar(&path, &bytes)?;
    }
    Ok(archive)
}
fn read_runtime_dag_qualification_archive_read_only(
    root_guard: &GovernanceFilesystemRootGuard,
    generation: u64,
    digest: [u8; 32],
    root_digest: [u8; 32],
) -> Result<RuntimeDagQualificationArchiveV1, GovernancePublishError> {
    root_guard.revalidate()?;
    let path = runtime_dag_qualification_archive_path(root_guard.root(), generation, digest);
    let snapshot = read_rooted_governance_state_file(
        root_guard,
        &path,
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_BYTES_V1,
    )?;
    verify_rooted_digest_sidecar(root_guard, &path, snapshot.bytes())?;
    let archive: RuntimeDagQualificationArchiveV1 = decode_canonical_runtime_dag(
        snapshot.bytes(),
        "governance runtime DAG qualification archive",
    )?;
    if validate_runtime_dag_qualification_archive(&archive, root_digest)? != digest
        || archive.body.archive_generation != generation
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive path, generation, or digest is substituted",
        ));
    }
    snapshot.binding().verify()?;
    root_guard.revalidate()?;
    Ok(archive)
}
fn validate_runtime_dag_qualification_history(
    root: &Path,
    history: &RuntimeDagQualificationHistoryV1,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
    allowed_unindexed_archive: Option<(u64, [u8; 32])>,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
    validate_runtime_dag_qualification_history_with_guard(
        root,
        history,
        expected_binding,
        allowed_unindexed_archive,
        None,
    )
}
fn validate_runtime_dag_qualification_history_with_guard(
    root: &Path,
    history: &RuntimeDagQualificationHistoryV1,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
    allowed_unindexed_archive: Option<(u64, [u8; 32])>,
    read_only_root_guard: Option<&GovernanceFilesystemRootGuard>,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
    if read_only_root_guard.is_some_and(|guard| guard.root() != root) {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history root differs from its retained read root",
        ));
    }
    let root_digest = runtime_dag_producer_root_digest(root)?;
    if history.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_VERSION_V1
        || history.root_digest != root_digest
        || (history.archive_generation == 0 && history.transitions.is_empty())
        || history.transitions.len() > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ACTIVE_MAX_V1
        || (history.archive_generation == 0)
            != (history.archive_digest == [0; 32]
                && history.archived_through_generation == 0
                && history.archive_tail_transition_digest == [0; 32])
        || history.archive_generation > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history is malformed or outside its V1 bounds",
        ));
    }
    let mut archives = Vec::new();
    let mut archive_generation = history.archive_generation;
    let mut archive_digest = history.archive_digest;
    while archive_generation != 0 {
        let archive = match read_only_root_guard {
            Some(root_guard) => read_runtime_dag_qualification_archive_read_only(
                root_guard,
                archive_generation,
                archive_digest,
                root_digest,
            )?,
            None => read_runtime_dag_qualification_archive(
                root,
                archive_generation,
                archive_digest,
                root_digest,
            )?,
        };
        archive_digest = archive.body.predecessor_archive_digest;
        archive_generation = archive_generation.checked_sub(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification archive generation rolled back",
            )
        })?;
        archives.push(archive);
    }
    if archive_digest != [0; 32] {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive predecessor chain is truncated",
        ));
    }
    archives.reverse();
    let mut expected_archive_generation = 1_u64;
    let mut expected_archive_digest = [0; 32];
    let mut archive_digests = Vec::with_capacity(archives.len());
    let mut expected_generation = 1_u64;
    let mut expected_predecessor = None;
    let mut last_binding = None;
    let mut last_transition_block_count = None;
    let mut expected_archive_paths = Vec::new();
    for archive in &archives {
        let digest = runtime_dag_archive_digest(archive)?;
        if archive.body.archive_generation != expected_archive_generation
            || archive.body.predecessor_archive_digest != expected_archive_digest
            || archive.body.first_transition_generation != expected_generation
            || archive.body.predecessor_transition_digest != expected_predecessor.unwrap_or([0; 32])
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification archive chain contains a fork, gap, duplicate, or rollback",
            ));
        }
        for transition in &archive.body.transitions {
            let transition_digest =
                validate_runtime_dag_qualification_transition(transition, root_digest)?;
            if transition.body.generation != expected_generation
                || transition.body.predecessor_transition_digest != expected_predecessor
                || last_binding
                    .as_ref()
                    .is_some_and(|binding| *binding != transition.body.previous)
                || last_transition_block_count
                    .is_some_and(|block_count| transition.body.block_count < block_count)
                || transition.body.archive_generation >= archive.body.archive_generation
                || (transition.body.archive_generation == 0)
                    != (transition.body.archive_digest == [0; 32])
                || (transition.body.archive_generation != 0
                    && archive_digests
                        .iter()
                        .find(|(generation, _)| *generation == transition.body.archive_generation)
                        .is_none_or(|(_, digest)| *digest != transition.body.archive_digest))
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG qualification transition archive is not one canonical lineage",
                ));
            }
            expected_generation = expected_generation.checked_add(1).ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG qualification transition generation exhausted",
                )
            })?;
            expected_predecessor = Some(transition_digest);
            last_binding = Some(transition.body.next.clone());
            last_transition_block_count = Some(transition.body.block_count);
        }
        expected_archive_paths.push(runtime_dag_qualification_archive_path(
            root,
            archive.body.archive_generation,
            digest,
        ));
        archive_digests.push((archive.body.archive_generation, digest));
        expected_archive_generation =
            expected_archive_generation.checked_add(1).ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG qualification archive generation exhausted",
                )
            })?;
        expected_archive_digest = digest;
    }
    if history.archive_generation != 0
        && (!runtime_dag_generation_immediately_precedes(
            history.archived_through_generation,
            expected_generation,
        ) || history.archive_tail_transition_digest != expected_predecessor.unwrap_or([0; 32])
            || history.archive_digest != expected_archive_digest)
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history archive head is substituted",
        ));
    }
    for transition in &history.transitions {
        let digest = validate_runtime_dag_qualification_transition(transition, root_digest)?;
        if transition.body.generation != expected_generation
            || transition.body.predecessor_transition_digest != expected_predecessor
            || last_binding
                .as_ref()
                .is_some_and(|binding| *binding != transition.body.previous)
            || last_transition_block_count
                .is_some_and(|block_count| transition.body.block_count < block_count)
            || transition.body.archive_generation > history.archive_generation
            || (transition.body.archive_generation == 0)
                != (transition.body.archive_digest == [0; 32])
            || (transition.body.archive_generation != 0
                && archive_digests
                    .iter()
                    .find(|(generation, _)| *generation == transition.body.archive_generation)
                    .is_none_or(|(_, digest)| *digest != transition.body.archive_digest))
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification history contains a fork, gap, duplicate, rollback, or archive substitution",
            ));
        }
        expected_generation = expected_generation.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification transition generation exhausted",
            )
        })?;
        expected_predecessor = Some(digest);
        last_binding = Some(transition.body.next.clone());
        last_transition_block_count = Some(transition.body.block_count);
    }
    let transition_generation = expected_generation.saturating_sub(1);
    if transition_generation > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TOTAL_MAX_V1
        || expected_binding.is_some_and(|expected| last_binding.as_ref() != Some(expected))
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history is outside its total bound or ends at a substituted provider",
        ));
    }
    let archives_dir = root.join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR);
    let allowed_unindexed_archive = allowed_unindexed_archive.map(|(generation, digest)| {
        runtime_dag_qualification_archive_path(root, generation, digest)
    });
    if let Some(root_guard) = read_only_root_guard {
        let mut expected_names = BTreeSet::new();
        for archive in expected_archive_paths
            .iter()
            .chain(allowed_unindexed_archive.iter())
        {
            for path in [archive.clone(), digest_sidecar_path_for(archive)] {
                expected_names.insert(
                    path.file_name()
                        .ok_or_else(|| {
                            GovernancePublishError::other(
                                "governance runtime DAG qualification archive has no canonical file name",
                            )
                        })?
                        .to_os_string(),
                );
            }
        }
        match root_guard.rooted_directory().open_directory(OsStr::new(
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
        )) {
            Ok(directory) => {
                let bound = expected_names.len().checked_add(1).ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG qualification archive inventory bound overflowed",
                    )
                })?;
                let actual = directory
                    .child_names_bounded(bound)?
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                if actual != expected_names {
                    return Err(GovernancePublishError::other(
                        "governance runtime DAG qualification archive directory contains an unindexed, missing, or duplicate artifact",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && expected_names.is_empty() => {}
            Err(error) => return Err(error.into()),
        }
    } else {
        match fs::read_dir(&archives_dir) {
            Ok(entries) => {
                for entry in entries {
                    let path = entry?.path();
                    let expected = expected_archive_paths.iter().any(|archive| {
                        path == *archive || path == digest_sidecar_path_for(archive)
                    }) || allowed_unindexed_archive.as_ref().is_some_and(
                        |archive| path == *archive || path == digest_sidecar_path_for(archive),
                    );
                    if !expected {
                        return Err(GovernancePublishError::other(
                            "governance runtime DAG qualification archive directory contains an unindexed fork or duplicate",
                        ));
                    }
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::NotFound && expected_archive_paths.is_empty() => {
            }
            Err(error) => return Err(error.into()),
        }
    }
    if let Some(root_guard) = read_only_root_guard {
        root_guard.revalidate()?;
    }
    Ok(RuntimeDagQualificationSummary {
        transition_generation,
        transition_digest: expected_predecessor.unwrap_or([0; 32]),
        archive_generation: history.archive_generation,
        archive_digest: history.archive_digest,
    })
}
fn runtime_dag_generation_immediately_precedes(previous: u64, next: u64) -> bool {
    previous.checked_add(1) == Some(next)
}
fn reject_legacy_runtime_dag_qualification_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    reject_legacy_atomic_state_names(
        root_guard.rooted_directory(),
        &[
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_FILE,
            "runtime-dag-qualification-history.to.blake3",
        ],
        "governance runtime DAG qualification authority",
    )?;
    for path in [
        runtime_dag_qualification_history_path(root),
        digest_sidecar_path_for(&runtime_dag_qualification_history_path(root)),
    ] {
        match read_rooted_governance_state_file(root_guard, &path, 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy mutable governance runtime DAG qualification state `{}` is unsupported; remove it before first-release initialization",
                    path.display()
                )));
            }
        }
    }
    root_guard.revalidate()?;
    Ok(())
}
fn open_runtime_dag_qualification_store_v1(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<governance_rooted_fs::TwoSlotStoreV1, GovernancePublishError> {
    if root != root_guard.root() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification root differs from its retained root guard",
        ));
    }
    root_guard.revalidate()?;
    reject_legacy_runtime_dag_qualification_state(root, root_guard)?;
    let store_present = match root_guard.rooted_directory().open_directory(OsStr::new(
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_DIR_V1,
    )) {
        Ok(directory) => {
            drop(directory);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if !store_present {
        match root_guard.rooted_directory().open_directory(OsStr::new(
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
        )) {
            Ok(directory) => {
                drop(directory);
                return Err(GovernancePublishError::other(
                    "governance runtime DAG qualification archives exist without their typed history state",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let initial = encode_governance_two_slot_value_v1(
        &RuntimeDagQualificationStateV1 {
            version: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_VERSION_V1,
            history: None,
        },
        "initial governance runtime DAG qualification state",
    )?;
    open_governance_two_slot_store_v1(
        root_guard,
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_SPEC_V1,
        &initial,
    )
}
fn load_runtime_dag_qualification_state_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<
    (
        RuntimeDagQualificationStateV1,
        governance_rooted_fs::TwoSlotSnapshotV1,
    ),
    GovernancePublishError,
> {
    let snapshot =
        load_governance_two_slot_store_v1(store, "governance runtime DAG qualification state")?;
    let state: RuntimeDagQualificationStateV1 = decode_governance_two_slot_value_v1(
        &snapshot,
        "governance runtime DAG qualification state",
    )?;
    if state.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_VERSION_V1 {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification state version is unsupported",
        ));
    }
    Ok((state, snapshot))
}
fn read_runtime_dag_qualification_history(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
) -> Result<
    Option<(
        RuntimeDagQualificationHistoryV1,
        RuntimeDagQualificationSummary,
    )>,
    GovernancePublishError,
> {
    read_runtime_dag_qualification_history_allowing_archive(
        root,
        root_guard,
        expected_binding,
        None,
    )
}
fn read_runtime_dag_qualification_history_allowing_archive(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
    allowed_unindexed_archive: Option<(u64, [u8; 32])>,
) -> Result<
    Option<(
        RuntimeDagQualificationHistoryV1,
        RuntimeDagQualificationSummary,
    )>,
    GovernancePublishError,
> {
    let store = open_runtime_dag_qualification_store_v1(root, root_guard)?;
    let (state, _) = load_runtime_dag_qualification_state_v1(&store)?;
    let Some(history) = state.history else {
        match root_guard.rooted_directory().open_directory(OsStr::new(
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
        )) {
            Ok(directory) => {
                drop(directory);
                return Err(GovernancePublishError::other(
                    "governance runtime DAG qualification archives exist without their authenticated history head",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        return Ok(None);
    };
    let summary = validate_runtime_dag_qualification_history(
        root,
        &history,
        expected_binding,
        allowed_unindexed_archive,
    )?;
    Ok(Some((history, summary)))
}
fn read_existing_runtime_dag_qualification_history_v1(
    root_guard: &GovernanceFilesystemRootGuard,
    expected_binding: &RuntimeDagProviderBindingV1,
) -> Result<
    Option<(
        RuntimeDagQualificationHistoryV1,
        RuntimeDagQualificationSummary,
    )>,
    GovernancePublishError,
> {
    let root = root_guard.root();
    root_guard.revalidate()?;
    reject_legacy_runtime_dag_qualification_state(root, root_guard)?;
    match root_guard.rooted_directory().open_directory(OsStr::new(
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_DIR_V1,
    )) {
        Ok(directory) => drop(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match root_guard.rooted_directory().open_directory(OsStr::new(
                GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
            )) {
                Ok(directory) => {
                    drop(directory);
                    return Err(GovernancePublishError::other(
                        "governance runtime DAG qualification archives exist without their typed history state",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    }
    let config = governance_two_slot_config_v1(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_SPEC_V1)?;
    let snapshot = root_guard
        .rooted_directory()
        .load_existing_two_slot_store_v1(config)
        .map_err(|error| {
            GovernancePublishError::other(format!(
                "failed to load governance runtime DAG qualification state read-only: {error}"
            ))
        })?;
    let state: RuntimeDagQualificationStateV1 = decode_governance_two_slot_value_v1(
        &snapshot,
        "governance runtime DAG qualification state read-only snapshot",
    )?;
    if state.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_VERSION_V1 {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification state version is unsupported",
        ));
    }
    let Some(history) = state.history else {
        match root_guard.rooted_directory().open_directory(OsStr::new(
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR,
        )) {
            Ok(directory) => {
                drop(directory);
                return Err(GovernancePublishError::other(
                    "governance runtime DAG qualification archives exist without their authenticated history head",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        root_guard.revalidate()?;
        return Ok(None);
    };
    let summary = validate_runtime_dag_qualification_history_with_guard(
        root,
        &history,
        Some(expected_binding),
        None,
        Some(root_guard),
    )?;
    root_guard.revalidate()?;
    Ok(Some((history, summary)))
}
fn runtime_dag_qualification_summary(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    let binding = runtime_dag_provider_binding(signer, store);
    read_runtime_dag_qualification_history(root, &root_guard, Some(&binding)).map(|history| {
        history.map_or(RuntimeDagQualificationSummary::EMPTY, |(_, summary)| {
            summary
        })
    })
}
fn runtime_dag_history_tail_transition(
    root: &Path,
    history: &RuntimeDagQualificationHistoryV1,
) -> Result<Option<RuntimeDagQualificationTransitionV1>, GovernancePublishError> {
    if let Some(transition) = history.transitions.last() {
        return Ok(Some(transition.clone()));
    }
    if history.archive_generation == 0 {
        return Ok(None);
    }
    let archive = read_runtime_dag_qualification_archive(
        root,
        history.archive_generation,
        history.archive_digest,
        history.root_digest,
    )?;
    Ok(archive.body.transitions.last().cloned())
}
fn runtime_dag_full_transition_lineage(
    root_guard: &GovernanceFilesystemRootGuard,
    history: &RuntimeDagQualificationHistoryV1,
) -> Result<Vec<RuntimeDagQualificationTransitionV1>, GovernancePublishError> {
    root_guard.revalidate()?;
    let mut archives = Vec::new();
    let mut generation = history.archive_generation;
    let mut digest = history.archive_digest;
    while generation != 0 {
        let archive = read_runtime_dag_qualification_archive_read_only(
            root_guard,
            generation,
            digest,
            history.root_digest,
        )?;
        digest = archive.body.predecessor_archive_digest;
        generation = generation.checked_sub(1).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG key-transition archive generation rolled back",
            )
        })?;
        archives.push(archive);
    }
    if digest != [0; 32] {
        return Err(GovernancePublishError::other(
            "governance runtime DAG key-transition archive lineage is truncated",
        ));
    }
    archives.reverse();
    let capacity = usize::try_from(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TOTAL_MAX_V1)
        .unwrap_or(usize::MAX)
        .min(
            archives
                .iter()
                .map(|archive| archive.body.transitions.len())
                .sum::<usize>()
                .saturating_add(history.transitions.len()),
        );
    let mut transitions = Vec::with_capacity(capacity);
    for archive in archives {
        transitions.extend(archive.body.transitions);
    }
    transitions.extend(history.transitions.iter().cloned());
    if transitions.is_empty()
        || u64::try_from(transitions.len()).unwrap_or(u64::MAX)
            > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TOTAL_MAX_V1
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG key-transition lineage is empty or exceeds its V1 bound",
        ));
    }
    root_guard.revalidate()?;
    Ok(transitions)
}
fn runtime_dag_authority_lineage(
    root: &Path,
    current_binding: &RuntimeDagProviderBindingV1,
) -> Result<RuntimeDagAuthorityLineageV1, GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_source(root)?;
    runtime_dag_authority_lineage_read_only(&root_guard, current_binding)
}
fn runtime_dag_authority_lineage_read_only(
    root_guard: &GovernanceFilesystemRootGuard,
    current_binding: &RuntimeDagProviderBindingV1,
) -> Result<RuntimeDagAuthorityLineageV1, GovernancePublishError> {
    let Some((history, qualification)) =
        read_existing_runtime_dag_qualification_history_v1(root_guard, current_binding)?
    else {
        return Ok(RuntimeDagAuthorityLineageV1 {
            segments: vec![RuntimeDagAuthoritySegmentV1 {
                activation_block_count: 0,
                revision: 1,
                binding: current_binding.clone(),
            }],
            transitions: Vec::new(),
            qualification: RuntimeDagQualificationSummary::EMPTY,
        });
    };
    let transitions = runtime_dag_full_transition_lineage(root_guard, &history)?;
    let first = transitions.first().ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG key-transition history has no first authority segment",
        )
    })?;
    let mut segments = Vec::with_capacity(transitions.len().saturating_add(1));
    segments.push(RuntimeDagAuthoritySegmentV1 {
        activation_block_count: 0,
        revision: first.key_transition.outgoing_segment_revision,
        binding: first.body.previous.clone(),
    });
    for transition in &transitions {
        let previous = segments.last().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG key-transition authority lineage is empty",
            )
        })?;
        if transition.body.previous != previous.binding
            || transition.key_transition.outgoing_segment_revision != previous.revision
            || transition.key_transition.incoming_segment_revision
                != previous.revision.checked_add(1).ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG authority segment revision exhausted",
                    )
                })?
            || transition.body.block_count < previous.activation_block_count
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG key-transition authority segments contain a fork, rollback, or revision substitution",
            ));
        }
        segments.push(RuntimeDagAuthoritySegmentV1 {
            activation_block_count: transition.body.block_count,
            revision: transition.key_transition.incoming_segment_revision,
            binding: transition.body.next.clone(),
        });
    }
    if segments.last().map(|segment| &segment.binding) != Some(current_binding) {
        return Err(GovernancePublishError::other(
            "governance runtime DAG key-transition authority lineage ends at another binding",
        ));
    }
    Ok(RuntimeDagAuthorityLineageV1 {
        segments,
        transitions,
        qualification,
    })
}
fn validate_runtime_dag_authority_lineage_for_chain(
    authority_lineage: &RuntimeDagAuthorityLineageV1,
    blocks: &[&GovernanceDagBlockV1],
    head: &GovernanceDagHeadV1,
) -> Result<(), GovernancePublishError> {
    if blocks.is_empty() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG authority validation requires a retained block",
        ));
    }
    let block_count = u64::try_from(blocks.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG authority block count exceeds u64")
    })?;
    let mut authority_segment_index = 0_usize;
    let mut tip_authority_segment_index = 0_usize;
    for (position, block) in blocks.iter().enumerate() {
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG authority position exceeds u64")
        })?;
        while authority_segment_index + 1 < authority_lineage.segments.len()
            && authority_lineage.segments[authority_segment_index + 1].activation_block_count
                <= position_u64
        {
            authority_segment_index += 1;
        }
        let authority = authority_lineage
            .segments
            .get(authority_segment_index)
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG authority lineage has no segment for a retained block",
                )
            })?;
        if block.sequence != position_u64
            || block.publisher_peer_id != authority.binding.publisher_peer_id
            || block.node.publisher_peer_id != authority.binding.publisher_peer_id
            || block.block_signature.public_key != authority.binding.publisher_public_key
            || block.node.publisher_signature.public_key != authority.binding.publisher_public_key
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG block signer or publisher identity is outside its authenticated authority segment",
            ));
        }
        tip_authority_segment_index = authority_segment_index;
    }
    while authority_segment_index + 1 < authority_lineage.segments.len()
        && authority_lineage.segments[authority_segment_index + 1].activation_block_count
            <= block_count
    {
        authority_segment_index += 1;
    }
    if authority_segment_index + 1 != authority_lineage.segments.len() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG key-transition lineage activates beyond the retained block count",
        ));
    }
    for transition in &authority_lineage.transitions {
        if transition.body.block_count > block_count {
            return Err(GovernancePublishError::other(
                "governance runtime DAG key transition binds a future retained head",
            ));
        }
        if transition.body.block_count == 0 {
            continue;
        }
        let transition_tip = usize::try_from(transition.body.block_count - 1)
            .ok()
            .and_then(|position| blocks.get(position))
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG key transition references a missing retained head",
                )
            })?;
        if transition_tip.block_cid.as_slice() != transition.body.head_block_cid.as_slice() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG key transition head is substituted or rolled back",
            ));
        }
    }
    let tip_authority = authority_lineage
        .segments
        .get(tip_authority_segment_index)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG authority lineage has no segment for the retained head",
            )
        })?;
    if head.publisher_peer_id != tip_authority.binding.publisher_peer_id
        || head.head_signature.public_key != tip_authority.binding.publisher_public_key
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG head signer or publisher identity is outside the authenticated tip segment",
        ));
    }
    Ok(())
}
/// Validate one committed producer snapshot against its bounded, dual-signed
/// authority-segment lineage.
pub(crate) fn validate_runtime_dag_snapshot_authority_lineage<'a>(
    root: &Path,
    checkpoint: &RuntimeDagProducerCheckpointV1,
    blocks: impl IntoIterator<Item = &'a GovernanceDagBlockV1>,
    head: &GovernanceDagHeadV1,
) -> Result<(), GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_source(root)?;
    validate_runtime_dag_snapshot_authority_lineage_read_only(&root_guard, checkpoint, blocks, head)
}
fn validate_runtime_dag_snapshot_authority_lineage_read_only<'a>(
    root_guard: &GovernanceFilesystemRootGuard,
    checkpoint: &RuntimeDagProducerCheckpointV1,
    blocks: impl IntoIterator<Item = &'a GovernanceDagBlockV1>,
    head: &GovernanceDagHeadV1,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    let blocks = blocks.into_iter().collect::<Vec<_>>();
    let block_count = u64::try_from(blocks.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG snapshot block count exceeds u64")
    })?;
    if checkpoint.block_count != block_count
        || checkpoint.head_block_cid.as_slice() != head.head_block_cid.as_slice()
        || head.block_count != block_count
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG snapshot does not bind its sealed block/head boundary",
        ));
    }
    let authority_lineage =
        authenticated_runtime_dag_authority_lineage_read_only(root_guard, checkpoint)?;
    validate_runtime_dag_authority_lineage_for_chain(&authority_lineage, &blocks, head)?;
    root_guard.revalidate()?;
    Ok(())
}
fn authenticated_runtime_dag_authority_lineage_read_only(
    root_guard: &GovernanceFilesystemRootGuard,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<RuntimeDagAuthorityLineageV1, GovernancePublishError> {
    root_guard.revalidate()?;
    validate_runtime_dag_producer_checkpoint_shape(checkpoint, root_guard.root())?;
    let authority_lineage = runtime_dag_authority_lineage_read_only(
        root_guard,
        &runtime_dag_checkpoint_binding(checkpoint),
    )?;
    let qualification = authority_lineage.qualification;
    if checkpoint.qualification_transition_generation != qualification.transition_generation
        || checkpoint.qualification_transition_digest != qualification.transition_digest
        || checkpoint.qualification_archive_generation != qualification.archive_generation
        || checkpoint.qualification_archive_digest != qualification.archive_digest
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG snapshot authority lineage diverges from its sealed checkpoint",
        ));
    }
    root_guard.revalidate()?;
    Ok(authority_lineage)
}
type RuntimeDagIndexTransitionV1 = ([u8; 32], [u8; 32], Option<Vec<u8>>);
fn canonical_runtime_dag_index_for_transition(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    previous: &RuntimeDagProviderBindingV1,
    next: &RuntimeDagProviderBindingV1,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<RuntimeDagIndexTransitionV1, GovernancePublishError> {
    let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    if checkpoint.block_count == 0 {
        if committed.index_bytes.is_some() {
            return Err(GovernancePublishError::other(
                "empty governance runtime DAG provider transition found a substituted index",
            ));
        }
        return Ok(([0; 32], [0; 32], None));
    }
    let bytes = committed.index_bytes.ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG provider transition predecessor index is missing",
        )
    })?;
    if *blake3::hash(&bytes).as_bytes() != checkpoint.index_bytes_digest {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition predecessor index digest is substituted",
        ));
    }
    let value: JsonValue = json::from_slice(&bytes).map_err(|error| {
        GovernancePublishError::other(format!(
            "decode governance runtime DAG provider transition index: {error}"
        ))
    })?;
    let mut index = value.as_object().cloned().ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG provider transition index is not an object",
        )
    })?;
    let canonical = json::to_json_pretty(&value).map_err(|error| {
        GovernancePublishError::other(format!(
            "canonicalize governance runtime DAG provider transition index: {error}"
        ))
    })?;
    if canonical.as_bytes() != bytes
        || runtime_dag_index_provider_binding(&index)? != *previous
        || required_runtime_u64(&index, "block_count")? != checkpoint.block_count
        || required_runtime_string(&index, "head_block_cid_hex")?
            != hex::encode(checkpoint.head_block_cid)
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition index does not authenticate its predecessor binding and head",
        ));
    }
    insert_runtime_dag_provider_binding_fields(&mut index, next);
    let successor = json::to_json_pretty(&JsonValue::Object(index)).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG provider transition successor index: {error}"
        ))
    })?;
    if successor.is_empty() || successor.len() > GOVERNANCE_MUTABLE_INDEX_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition successor index exceeds its byte bound",
        ));
    }
    let predecessor_digest = *blake3::hash(&bytes).as_bytes();
    let successor_digest = *blake3::hash(successor.as_bytes()).as_bytes();
    if predecessor_digest == successor_digest {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition did not change the bound provider identity",
        ));
    }
    Ok((
        predecessor_digest,
        successor_digest,
        Some(successor.into_bytes()),
    ))
}
fn canonical_runtime_dag_successor_index_from_transition(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    transition: &RuntimeDagQualificationTransitionV1,
) -> Result<Option<Vec<u8>>, GovernancePublishError> {
    if transition.body.block_count == 0 {
        if transition.body.predecessor_index_digest != [0; 32]
            || transition.body.successor_index_digest != [0; 32]
        {
            return Err(GovernancePublishError::other(
                "empty governance runtime DAG transition carries an index digest",
            ));
        }
        return Ok(None);
    }
    let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    let bytes = committed.index_bytes.ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG transition recovery index is missing")
    })?;
    let current_digest = *blake3::hash(&bytes).as_bytes();
    let value: JsonValue = json::from_slice(&bytes).map_err(|error| {
        GovernancePublishError::other(format!(
            "decode governance runtime DAG transition recovery index: {error}"
        ))
    })?;
    let mut index = value.as_object().cloned().ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG transition recovery index is not an object",
        )
    })?;
    let canonical = json::to_json_pretty(&value).map_err(|error| {
        GovernancePublishError::other(format!(
            "canonicalize governance runtime DAG transition recovery index: {error}"
        ))
    })?;
    if canonical.as_bytes() != bytes
        || required_runtime_u64(&index, "block_count")? != transition.body.block_count
        || required_runtime_string(&index, "head_block_cid_hex")?
            != hex::encode(transition.body.head_block_cid)
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG transition recovery index is noncanonical or binds another head",
        ));
    }
    if current_digest == transition.body.successor_index_digest {
        if runtime_dag_index_provider_binding(&index)? != transition.body.next {
            return Err(GovernancePublishError::other(
                "governance runtime DAG transition successor index binding is substituted",
            ));
        }
        return Ok(Some(bytes));
    }
    if current_digest != transition.body.predecessor_index_digest
        || runtime_dag_index_provider_binding(&index)? != transition.body.previous
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG transition recovery found neither the exact predecessor nor successor index",
        ));
    }
    insert_runtime_dag_provider_binding_fields(&mut index, &transition.body.next);
    let successor = json::to_json_pretty(&JsonValue::Object(index)).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG transition recovery successor index: {error}"
        ))
    })?;
    if successor.is_empty()
        || successor.len() > GOVERNANCE_MUTABLE_INDEX_MAX_BYTES
        || *blake3::hash(successor.as_bytes()).as_bytes() != transition.body.successor_index_digest
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG transition successor index bytes are substituted",
        ));
    }
    Ok(Some(successor.into_bytes()))
}
fn runtime_dag_checkpoint_from_transition(
    transition: &RuntimeDagQualificationTransitionV1,
    transition_digest: [u8; 32],
) -> RuntimeDagProducerCheckpointV1 {
    let body = &transition.body;
    RuntimeDagProducerCheckpointV1 {
        version: GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1,
        root_digest: body.root_digest,
        signer_handle: body.next.signer_handle.clone(),
        signer_revision: body.next.signer_revision,
        signer_policy_digest: body.next.signer_policy_digest,
        checkpoint_store_handle: body.next.checkpoint_store_handle.clone(),
        checkpoint_store_revision: body.next.checkpoint_store_revision,
        checkpoint_store_policy_digest: body.next.checkpoint_store_policy_digest,
        publisher_peer_id: body.next.publisher_peer_id.clone(),
        publisher_public_key: body.next.publisher_public_key,
        block_count: body.block_count,
        head_block_cid: body.head_block_cid,
        head_bytes_digest: body.head_bytes_digest,
        index_bytes_digest: body.successor_index_digest,
        qualification_transition_generation: body.generation,
        qualification_transition_digest: transition_digest,
        qualification_archive_generation: body.archive_generation,
        qualification_archive_digest: body.archive_digest,
    }
}
fn validate_runtime_dag_transition_predecessor(
    transition: &RuntimeDagQualificationTransitionV1,
    transition_digest: [u8; 32],
    predecessor_record: &GovernanceDagSealedStateRecord,
    predecessor: &RuntimeDagProducerCheckpointV1,
) -> Result<(), GovernancePublishError> {
    let body = &transition.body;
    if predecessor_record.revision != body.predecessor_checkpoint_revision
        || runtime_dag_checkpoint_binding(predecessor) != body.previous
        || predecessor.root_digest != body.root_digest
        || predecessor.block_count != body.block_count
        || predecessor.head_block_cid != body.head_block_cid
        || predecessor.head_bytes_digest != body.head_bytes_digest
        || predecessor.index_bytes_digest != body.predecessor_index_digest
        || predecessor
            .qualification_transition_generation
            .checked_add(1)
            != Some(body.generation)
        || predecessor.qualification_transition_digest
            != body.predecessor_transition_digest.unwrap_or([0; 32])
        || predecessor.qualification_archive_generation != body.archive_generation
        || predecessor.qualification_archive_digest != body.archive_digest
        || transition_digest == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition does not authenticate the exact sealed predecessor checkpoint",
        ));
    }
    Ok(())
}
fn install_runtime_dag_provider_transition(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    transition: &RuntimeDagQualificationTransitionV1,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    signer.assert_qualification()?;
    store.assert_qualification()?;
    let root_digest = runtime_dag_producer_root_digest(root)?;
    let transition_digest = validate_runtime_dag_qualification_transition(transition, root_digest)?;
    if transition.body.next != runtime_dag_provider_binding(signer, store) {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition successor does not match the injected providers",
        ));
    }
    let successor_index =
        canonical_runtime_dag_successor_index_from_transition(root, root_guard, transition)?;
    let next_checkpoint = runtime_dag_checkpoint_from_transition(transition, transition_digest);
    validate_runtime_dag_producer_checkpoint_shape(&next_checkpoint, root)?;
    let next_record = runtime_dag_producer_checkpoint_record(&next_checkpoint)?;
    let current_record = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?;
    if current_record.as_ref() != Some(&next_record) {
        let expected_revision = match current_record.as_ref() {
            Some(record) => {
                let predecessor = decode_runtime_dag_unqualified_checkpoint_record(record, root)?;
                validate_runtime_dag_transition_predecessor(
                    transition,
                    transition_digest,
                    record,
                    &predecessor,
                )?;
                Some(record.revision)
            }
            None => None,
        };
        root_guard.revalidate()?;
        signer.assert_qualification()?;
        store.assert_qualification()?;
        store.compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            expected_revision,
            next_record.clone(),
        )?;
    }
    let readback = store
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG provider transition checkpoint disappeared after install",
            )
        })?;
    if readback != next_record {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition checkpoint readback is substituted",
        ));
    }
    if let Some(successor_index) = successor_index {
        root_guard.revalidate()?;
        let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
        let (current, snapshot) = load_runtime_dag_committed_state_v1(&committed_store)?;
        let head_bytes = current.head_bytes.ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG provider transition committed head is missing",
            )
        })?;
        let current_index = current.index_bytes.ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG provider transition committed index is missing",
            )
        })?;
        if *blake3::hash(&head_bytes).as_bytes() != transition.body.head_bytes_digest {
            return Err(GovernancePublishError::other(
                "governance runtime DAG provider transition committed head is substituted",
            ));
        }
        if current_index != successor_index {
            if *blake3::hash(&current_index).as_bytes() != transition.body.predecessor_index_digest
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG provider transition committed index is neither its predecessor nor successor",
                ));
            }
            let next = RuntimeDagCommittedStateV1 {
                version: GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1,
                head_bytes: Some(head_bytes),
                index_bytes: Some(successor_index),
            };
            let bytes = encode_governance_two_slot_value_v1(
                &next,
                "governance runtime DAG provider-transition committed state",
            )?;
            compare_and_swap_governance_two_slot_store_v1(
                &committed_store,
                &snapshot,
                &bytes,
                "governance runtime DAG provider-transition committed state",
            )?;
        }
    }
    root_guard.revalidate()?;
    validate_existing_runtime_dag_root(root, signer, store)?;
    let local = match local_runtime_dag_producer_checkpoint(root, signer, store)? {
        Some(local) => local,
        None => empty_runtime_dag_producer_checkpoint(root, signer, store)?,
    };
    if local != next_checkpoint {
        return Err(GovernancePublishError::other(
            "governance runtime DAG provider transition local readback diverged from its sealed checkpoint",
        ));
    }
    signer.assert_qualification()?;
    store.assert_qualification()
}
fn recover_runtime_dag_provider_transition(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    let Some((history, summary)) = read_runtime_dag_qualification_history(root, root_guard, None)?
    else {
        return Ok(());
    };
    let transition = runtime_dag_history_tail_transition(root, &history)?.ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG qualification history has no authenticated tail",
        )
    })?;
    if transition.body.next != runtime_dag_provider_binding(signer, store) {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history ends at another provider binding",
        ));
    }
    if let Some(record) = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)? {
        let checkpoint = decode_runtime_dag_unqualified_checkpoint_record(&record, root)?;
        if runtime_dag_checkpoint_binding(&checkpoint) == transition.body.next
            && checkpoint.qualification_transition_generation == summary.transition_generation
            && checkpoint.qualification_transition_digest == summary.transition_digest
            && checkpoint.block_count >= transition.body.block_count
            && (checkpoint.block_count != transition.body.block_count
                || (checkpoint.head_block_cid == transition.body.head_block_cid
                    && checkpoint.head_bytes_digest == transition.body.head_bytes_digest
                    && checkpoint.index_bytes_digest == transition.body.successor_index_digest))
        {
            if checkpoint.block_count > transition.body.block_count
                || checkpoint.qualification_archive_generation > transition.body.archive_generation
            {
                return Ok(());
            }
        }
    }
    install_runtime_dag_provider_transition(root, root_guard, signer, store, &transition)
}
fn write_runtime_dag_qualification_history(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    history: &RuntimeDagQualificationHistoryV1,
    expected_binding: &RuntimeDagProviderBindingV1,
    predecessor: Option<&RuntimeDagQualificationHistoryV1>,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
    let store = open_runtime_dag_qualification_store_v1(root, root_guard)?;
    let (current, snapshot) = load_runtime_dag_qualification_state_v1(&store)?;
    if current.history.as_ref() != predecessor {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history predecessor was substituted",
        ));
    }
    let summary =
        validate_runtime_dag_qualification_history(root, history, Some(expected_binding), None)?;
    let next = RuntimeDagQualificationStateV1 {
        version: GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_VERSION_V1,
        history: Some(history.clone()),
    };
    let bytes =
        encode_governance_two_slot_value_v1(&next, "governance runtime DAG qualification state")?;
    if bytes.len() > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STATE_MAX_BYTES_V1 {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification state exceeds its canonical byte bound",
        ));
    }
    let committed = compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &bytes,
        "governance runtime DAG qualification state",
    )?;
    let readback: RuntimeDagQualificationStateV1 = decode_governance_two_slot_value_v1(
        &committed,
        "governance runtime DAG qualification state readback",
    )?;
    if readback != next {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification history readback is substituted",
        ));
    }
    Ok(summary)
}
fn runtime_dag_history_after_archive(
    history: &RuntimeDagQualificationHistoryV1,
    archive: &RuntimeDagQualificationArchiveV1,
) -> Result<RuntimeDagQualificationHistoryV1, GovernancePublishError> {
    let body = &archive.body;
    if body.root_digest != history.root_digest
        || history.archive_generation.checked_add(1) != Some(body.archive_generation)
        || body.predecessor_archive_digest != history.archive_digest
        || body.predecessor_transition_digest != history.archive_tail_transition_digest
        || history.archived_through_generation.checked_add(1)
            != Some(body.first_transition_generation)
        || body.transitions.is_empty()
        || body.transitions.len() > history.transitions.len()
        || history.transitions[..body.transitions.len()] != body.transitions
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification archive is not the exact live-history prefix",
        ));
    }
    let mut next = history.clone();
    next.transitions.drain(..body.transitions.len());
    next.archive_generation = body.archive_generation;
    next.archive_digest = runtime_dag_archive_digest(archive)?;
    next.archived_through_generation = body.last_transition_generation;
    next.archive_tail_transition_digest = body.tail_transition_digest;
    Ok(next)
}
fn recover_runtime_dag_qualification_compaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    let Some(record) = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)? else {
        return Ok(());
    };
    let checkpoint = decode_runtime_dag_unqualified_checkpoint_record(&record, root)?;
    let binding = runtime_dag_provider_binding(signer, store);
    if runtime_dag_checkpoint_binding(&checkpoint) != binding {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification compaction checkpoint belongs to another provider binding",
        ));
    }
    isolate_runtime_dag_qualification_archive_temps(root_guard, &checkpoint)?;
    let allowed_archive = (checkpoint.qualification_archive_generation != 0).then_some((
        checkpoint.qualification_archive_generation,
        checkpoint.qualification_archive_digest,
    ));
    let (history, summary, staged_archive) =
        match read_runtime_dag_qualification_history_allowing_archive(
            root,
            root_guard,
            Some(&binding),
            allowed_archive,
        ) {
            Ok(Some((history, summary))) => (history, summary, None),
            Ok(None) => {
                if checkpoint.qualification_transition_generation == 0
                    && checkpoint.qualification_archive_generation == 0
                {
                    return Ok(());
                }
                return Err(GovernancePublishError::other(
                    "sealed governance runtime DAG qualification checkpoint has no authenticated history",
                ));
            }
            Err(primary_error) => {
                let Some(staged) = staged_runtime_dag_qualification_archive(root, &checkpoint)?
                else {
                    return Err(primary_error);
                };
                match read_runtime_dag_qualification_history_allowing_archive(
                    root,
                    root_guard,
                    Some(&binding),
                    Some(staged),
                ) {
                    Ok(Some((history, summary))) => (history, summary, Some(staged)),
                    _ => return Err(primary_error),
                }
            }
        };
    if let Some((archive_generation, archive_digest)) = staged_archive {
        if checkpoint.qualification_transition_generation != summary.transition_generation
            || checkpoint.qualification_transition_digest != summary.transition_digest
            || checkpoint.qualification_archive_generation != summary.archive_generation
            || checkpoint.qualification_archive_digest != summary.archive_digest
            || summary.archive_generation.checked_add(1) != Some(archive_generation)
        {
            return Err(GovernancePublishError::other(
                "staged governance runtime DAG qualification archive does not extend the sealed archive head",
            ));
        }
        let archive = read_runtime_dag_qualification_archive(
            root,
            archive_generation,
            archive_digest,
            history.root_digest,
        )?;
        if archive.body.signer != binding {
            return Err(GovernancePublishError::other(
                "staged governance runtime DAG qualification archive signer binding is substituted",
            ));
        }
        let next_history = runtime_dag_history_after_archive(&history, &archive)?;
        let mut next_checkpoint = checkpoint.clone();
        next_checkpoint.qualification_archive_generation = archive_generation;
        next_checkpoint.qualification_archive_digest = archive_digest;
        let next_record = runtime_dag_producer_checkpoint_record(&next_checkpoint)?;
        root_guard.revalidate()?;
        store.compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            Some(record.revision),
            next_record.clone(),
        )?;
        if store
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
            .as_ref()
            != Some(&next_record)
        {
            return Err(GovernancePublishError::other(
                "staged governance runtime DAG qualification archive checkpoint readback diverged",
            ));
        }
        write_runtime_dag_qualification_history(
            root,
            root_guard,
            &next_history,
            &binding,
            Some(&history),
        )?;
        return Ok(());
    }
    if checkpoint.qualification_transition_generation != summary.transition_generation
        || checkpoint.qualification_transition_digest != summary.transition_digest
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG qualification checkpoint and transition history diverge",
        ));
    }
    if checkpoint.qualification_archive_generation == summary.archive_generation
        && checkpoint.qualification_archive_digest == summary.archive_digest
    {
        return Ok(());
    }
    if summary.archive_generation.checked_add(1)
        != Some(checkpoint.qualification_archive_generation)
        || checkpoint.qualification_archive_digest == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG qualification archive head is stale, forked, or rolled back",
        ));
    }
    let archive = read_runtime_dag_qualification_archive(
        root,
        checkpoint.qualification_archive_generation,
        checkpoint.qualification_archive_digest,
        history.root_digest,
    )?;
    let next = runtime_dag_history_after_archive(&history, &archive)?;
    root_guard.revalidate()?;
    let recovered =
        write_runtime_dag_qualification_history(root, root_guard, &next, &binding, Some(&history))?;
    if recovered.transition_generation != checkpoint.qualification_transition_generation
        || recovered.transition_digest != checkpoint.qualification_transition_digest
        || recovered.archive_generation != checkpoint.qualification_archive_generation
        || recovered.archive_digest != checkpoint.qualification_archive_digest
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG qualification compaction recovery readback diverged",
        ));
    }
    Ok(())
}
fn empty_runtime_dag_producer_checkpoint(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<RuntimeDagProducerCheckpointV1, GovernancePublishError> {
    let qualification = runtime_dag_qualification_summary(root, signer, store)?;
    Ok(RuntimeDagProducerCheckpointV1 {
        version: GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1,
        root_digest: runtime_dag_producer_root_digest(root)?,
        signer_handle: signer.handle.clone(),
        signer_revision: signer.qualification.revision,
        signer_policy_digest: signer.qualification.policy_digest,
        checkpoint_store_handle: store.handle.clone(),
        checkpoint_store_revision: store.qualification.revision,
        checkpoint_store_policy_digest: store.qualification.policy_digest,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        publisher_public_key: signer.public_key,
        block_count: 0,
        head_block_cid: [0; 32],
        head_bytes_digest: [0; 32],
        index_bytes_digest: [0; 32],
        qualification_transition_generation: qualification.transition_generation,
        qualification_transition_digest: qualification.transition_digest,
        qualification_archive_generation: qualification.archive_generation,
        qualification_archive_digest: qualification.archive_digest,
    })
}
fn runtime_dag_producer_checkpoint(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    head_bytes: &[u8],
    index_bytes: &[u8],
) -> Result<RuntimeDagProducerCheckpointV1, GovernancePublishError> {
    let qualification = runtime_dag_qualification_summary(root, signer, store)?;
    let head: GovernanceDagHeadV1 =
        decode_canonical_runtime_dag(head_bytes, "governance runtime DAG producer head")?;
    head.validate().map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG producer head is invalid: {error}"
        ))
    })?;
    let head_block_cid: [u8; 32] = head.head_block_cid.as_slice().try_into().map_err(|_| {
        GovernancePublishError::other("governance runtime DAG producer head CID is not 32 bytes")
    })?;
    let value: JsonValue = json::from_slice(index_bytes).map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG producer index is invalid JSON: {error}"
        ))
    })?;
    let index = value.as_object().ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG producer index is not an object")
    })?;
    let canonical = json::to_json_pretty(&value).map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG producer index cannot be canonicalized: {error}"
        ))
    })?;
    if canonical.as_bytes() != index_bytes {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer index is noncanonical",
        ));
    }
    validate_runtime_dag_signer_fields(index, signer)?;
    validate_runtime_dag_checkpoint_store_fields(index, store)?;
    if required_runtime_u64(index, "block_count")? != head.block_count
        || required_runtime_string(index, "head_block_cid_hex")?
            != hex::encode(&head.head_block_cid)
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer index does not bind its signed head",
        ));
    }
    Ok(RuntimeDagProducerCheckpointV1 {
        version: GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1,
        root_digest: runtime_dag_producer_root_digest(root)?,
        signer_handle: signer.handle.clone(),
        signer_revision: signer.qualification.revision,
        signer_policy_digest: signer.qualification.policy_digest,
        checkpoint_store_handle: store.handle.clone(),
        checkpoint_store_revision: store.qualification.revision,
        checkpoint_store_policy_digest: store.qualification.policy_digest,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        publisher_public_key: signer.public_key,
        block_count: head.block_count,
        head_block_cid,
        head_bytes_digest: *blake3::hash(head_bytes).as_bytes(),
        index_bytes_digest: *blake3::hash(index_bytes).as_bytes(),
        qualification_transition_generation: qualification.transition_generation,
        qualification_transition_digest: qualification.transition_digest,
        qualification_archive_generation: qualification.archive_generation,
        qualification_archive_digest: qualification.archive_digest,
    })
}
fn validate_runtime_dag_producer_checkpoint_shape(
    checkpoint: &RuntimeDagProducerCheckpointV1,
    root: &Path,
) -> Result<(), GovernancePublishError> {
    let empty = checkpoint.block_count == 0;
    let empty_fields = checkpoint.head_block_cid == [0; 32]
        && checkpoint.head_bytes_digest == [0; 32]
        && checkpoint.index_bytes_digest == [0; 32];
    if checkpoint.version != GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1
        || checkpoint.root_digest != runtime_dag_producer_root_digest(root)?
        || checkpoint.signer_revision == 0
        || checkpoint.signer_policy_digest == [0; 32]
        || checkpoint.checkpoint_store_revision == 0
        || checkpoint.checkpoint_store_policy_digest == [0; 32]
        || checkpoint.publisher_peer_id.is_empty()
        || checkpoint.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || checkpoint.publisher_public_key == [0; 32]
        || empty != empty_fields
        || (!empty
            && (checkpoint.head_block_cid == [0; 32]
                || checkpoint.head_bytes_digest == [0; 32]
                || checkpoint.index_bytes_digest == [0; 32]))
        || (checkpoint.qualification_transition_generation == 0)
            != (checkpoint.qualification_transition_digest == [0; 32])
        || (checkpoint.qualification_archive_generation == 0)
            != (checkpoint.qualification_archive_digest == [0; 32])
        || checkpoint.qualification_archive_generation
            > checkpoint.qualification_transition_generation
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint is malformed or belongs to another root",
        ));
    }
    validate_runtime_dag_provider_binding(&RuntimeDagProviderBindingV1 {
        signer_handle: checkpoint.signer_handle.clone(),
        signer_revision: checkpoint.signer_revision,
        signer_policy_digest: checkpoint.signer_policy_digest,
        checkpoint_store_handle: checkpoint.checkpoint_store_handle.clone(),
        checkpoint_store_revision: checkpoint.checkpoint_store_revision,
        checkpoint_store_policy_digest: checkpoint.checkpoint_store_policy_digest,
        publisher_peer_id: checkpoint.publisher_peer_id.clone(),
        publisher_public_key: checkpoint.publisher_public_key,
    })?;
    Ok(())
}
fn runtime_dag_checkpoint_binding(
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> RuntimeDagProviderBindingV1 {
    RuntimeDagProviderBindingV1 {
        signer_handle: checkpoint.signer_handle.clone(),
        signer_revision: checkpoint.signer_revision,
        signer_policy_digest: checkpoint.signer_policy_digest,
        checkpoint_store_handle: checkpoint.checkpoint_store_handle.clone(),
        checkpoint_store_revision: checkpoint.checkpoint_store_revision,
        checkpoint_store_policy_digest: checkpoint.checkpoint_store_policy_digest,
        publisher_peer_id: checkpoint.publisher_peer_id.clone(),
        publisher_public_key: checkpoint.publisher_public_key,
    }
}
fn decode_runtime_dag_unqualified_checkpoint_record(
    record: &GovernanceDagSealedStateRecord,
    root: &Path,
) -> Result<RuntimeDagProducerCheckpointV1, GovernancePublishError> {
    if !record.has_valid_revision(GovernanceDagSealedStateSlot::ProducerCheckpoint) {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint revision is invalid",
        ));
    }
    let checkpoint: RuntimeDagProducerCheckpointV1 = decode_canonical_runtime_dag(
        &record.payload,
        "sealed governance runtime DAG producer checkpoint",
    )?;
    validate_runtime_dag_producer_checkpoint_shape(&checkpoint, root)?;
    if record.generation != runtime_dag_producer_checkpoint_generation(&checkpoint)? {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint generation is inconsistent",
        ));
    }
    Ok(checkpoint)
}
fn validate_runtime_dag_producer_checkpoint(
    checkpoint: &RuntimeDagProducerCheckpointV1,
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    validate_runtime_dag_producer_checkpoint_shape(checkpoint, root)?;
    let binding = runtime_dag_provider_binding(signer, store);
    let qualification = runtime_dag_qualification_summary(root, signer, store)?;
    if checkpoint.signer_handle != binding.signer_handle
        || checkpoint.signer_revision != binding.signer_revision
        || checkpoint.signer_policy_digest != binding.signer_policy_digest
        || checkpoint.checkpoint_store_handle != binding.checkpoint_store_handle
        || checkpoint.checkpoint_store_revision != binding.checkpoint_store_revision
        || checkpoint.checkpoint_store_policy_digest != binding.checkpoint_store_policy_digest
        || checkpoint.publisher_peer_id != binding.publisher_peer_id
        || checkpoint.publisher_public_key != binding.publisher_public_key
        || checkpoint.qualification_transition_generation != qualification.transition_generation
        || checkpoint.qualification_transition_digest != qualification.transition_digest
        || checkpoint.qualification_archive_generation != qualification.archive_generation
        || checkpoint.qualification_archive_digest != qualification.archive_digest
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint belongs to another provider binding or qualification lineage",
        ));
    }
    Ok(())
}
fn runtime_dag_producer_checkpoint_generation(
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<u64, GovernancePublishError> {
    checkpoint
        .block_count
        .checked_add(checkpoint.qualification_transition_generation)
        .and_then(|generation| generation.checked_add(checkpoint.qualification_archive_generation))
        .and_then(|generation| generation.checked_add(1))
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG producer checkpoint generation exhausted",
            )
        })
}
fn runtime_dag_producer_checkpoint_record(
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<GovernanceDagSealedStateRecord, GovernancePublishError> {
    let payload = norito::to_bytes(checkpoint).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG producer checkpoint: {error}"
        ))
    })?;
    let generation = runtime_dag_producer_checkpoint_generation(checkpoint)?;
    Ok(GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerCheckpoint,
        generation,
        payload,
    ))
}
fn decode_runtime_dag_producer_checkpoint_record(
    record: &GovernanceDagSealedStateRecord,
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<RuntimeDagProducerCheckpointV1, GovernancePublishError> {
    if !record.has_valid_revision(GovernanceDagSealedStateSlot::ProducerCheckpoint) {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint revision is invalid",
        ));
    }
    let checkpoint = decode_runtime_dag_unqualified_checkpoint_record(record, root)?;
    validate_runtime_dag_producer_checkpoint(&checkpoint, root, signer, store)?;
    Ok(checkpoint)
}
fn local_runtime_dag_producer_checkpoint(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<Option<RuntimeDagProducerCheckpointV1>, GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    let committed_store = open_runtime_dag_committed_store_v1(root, &root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    validate_existing_runtime_dag_root(root, signer, store)?;
    match (committed.head_bytes, committed.index_bytes) {
        (Some(head_bytes), Some(index_bytes)) => {
            runtime_dag_producer_checkpoint(root, signer, store, &head_bytes, &index_bytes)
                .map(Some)
        }
        (None, None) => Ok(None),
        _ => Err(GovernancePublishError::other(
            "governance runtime DAG committed head/index state is torn",
        )),
    }
}
fn validate_runtime_dag_producer_intent_bounds(
    root: &Path,
    intent: &RuntimeDagProducerPublishIntentV1,
    staged: &RuntimeDagProducerStagedTransactionV1,
) -> Result<(), GovernancePublishError> {
    validate_runtime_dag_producer_intent_metadata(intent)?;
    validate_runtime_dag_producer_file_lengths(
        staged.block_bytes.len(),
        staged.head_bytes.len(),
        staged.index_bytes.len(),
    )?;
    for (label, descriptor, bytes) in [
        ("block", &intent.block, staged.block_bytes.as_slice()),
        ("head", &intent.head, staged.head_bytes.as_slice()),
        ("index", &intent.index, staged.index_bytes.as_slice()),
    ] {
        let byte_len = usize::try_from(descriptor.byte_len).map_err(|_| {
            GovernancePublishError::other(format!(
                "governance runtime DAG staged {label} length exceeds host limits"
            ))
        })?;
        if byte_len != bytes.len() || descriptor.blake3 != *blake3::hash(bytes).as_bytes() {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG staged {label} descriptor is substituted"
            )));
        }
    }
    let value: JsonValue = json::from_slice(&staged.index_bytes).map_err(|error| {
        GovernancePublishError::other(format!(
            "governance runtime DAG producer intent index is invalid JSON: {error}"
        ))
    })?;
    let blocks = value
        .get("blocks")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG producer intent index has no block array",
            )
        })?;
    validate_runtime_dag_producer_entry_count(blocks.len(), intent.checkpoint.block_count)?;
    let mut total = 0_u64;
    add_runtime_dag_audit_bytes(&mut total, staged.index_bytes.len())?;
    add_runtime_dag_audit_bytes(&mut total, staged.head_bytes.len())?;
    for entry in blocks {
        let entry = entry.as_object().ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG producer intent index block entry is not an object",
            )
        })?;
        for (label, len, limit) in [
            (
                "block",
                required_runtime_u64(entry, "encoded_len")?,
                GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
            ),
            (
                "source payload",
                required_runtime_u64(entry, "source_payload_len")?,
                GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES,
            ),
        ] {
            if len > u64::try_from(limit).unwrap_or(u64::MAX) {
                return Err(GovernancePublishError::other(format!(
                    "governance runtime DAG producer intent {label} exceeds the per-file byte limit"
                )));
            }
            add_runtime_dag_audit_bytes(
                &mut total,
                usize::try_from(len).map_err(|_| {
                    GovernancePublishError::other(
                        "governance runtime DAG producer intent artifact length exceeds host limits",
                    )
                })?,
            )?;
        }
        let json_path = resolve_index_path(root, &required_runtime_string(entry, "json_path")?)?;
        let metadata = fs::symlink_metadata(&json_path)?;
        validate_governance_state_metadata(&json_path, &metadata)?;
        let json_len = usize::try_from(metadata.len()).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG producer intent JSON length exceeds host limits",
            )
        })?;
        if json_len > GOVERNANCE_MUTABLE_INDEX_MAX_BYTES {
            return Err(GovernancePublishError::other(
                "governance runtime DAG producer intent JSON exceeds the per-file byte limit",
            ));
        }
        add_runtime_dag_audit_bytes(&mut total, json_len)?;
    }
    Ok(())
}
fn validate_runtime_dag_producer_intent_metadata(
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<(), GovernancePublishError> {
    for (label, descriptor, limit) in [
        (
            "block",
            &intent.block,
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        ),
        (
            "head",
            &intent.head,
            GOVERNANCE_RUNTIME_DAG_HEAD_MAX_BYTES_V1,
        ),
        ("index", &intent.index, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
    ] {
        let byte_len = usize::try_from(descriptor.byte_len).map_err(|_| {
            GovernancePublishError::other(format!(
                "governance runtime DAG staged {label} length exceeds host limits"
            ))
        })?;
        if byte_len == 0 || byte_len > limit || descriptor.blake3 == [0; 32] {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG staged {label} descriptor is malformed"
            )));
        }
    }
    if intent.staging_revision
        != runtime_dag_producer_staging_revision(
            &intent.checkpoint,
            intent.previous_checkpoint_revision,
            &intent.block,
            &intent.head,
            &intent.index,
        )?
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer staging revision is substituted",
        ));
    }
    Ok(())
}
fn runtime_dag_producer_staged_artifact(
    bytes: &[u8],
) -> Result<RuntimeDagProducerStagedArtifactV1, GovernancePublishError> {
    if bytes.is_empty() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staged artifact is empty",
        ));
    }
    Ok(RuntimeDagProducerStagedArtifactV1 {
        byte_len: u64::try_from(bytes.len()).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG staged artifact length exceeds u64",
            )
        })?,
        blake3: *blake3::hash(bytes).as_bytes(),
    })
}
fn runtime_dag_producer_staging_revision(
    checkpoint: &RuntimeDagProducerCheckpointV1,
    previous_checkpoint_revision: Option<[u8; 32]>,
    block: &RuntimeDagProducerStagedArtifactV1,
    head: &RuntimeDagProducerStagedArtifactV1,
    index: &RuntimeDagProducerStagedArtifactV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let checkpoint_bytes = norito::to_bytes(checkpoint).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG staging checkpoint: {error}"
        ))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.local-producer-staging.v1\0");
    hasher.update(
        &u64::try_from(checkpoint_bytes.len())
            .map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG staging checkpoint length exceeds u64",
                )
            })?
            .to_le_bytes(),
    );
    hasher.update(&checkpoint_bytes);
    match previous_checkpoint_revision {
        Some(revision) => {
            hasher.update(&[1]);
            hasher.update(&revision);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    for artifact in [block, head, index] {
        hasher.update(&artifact.byte_len.to_le_bytes());
        hasher.update(&artifact.blake3);
    }
    Ok(*hasher.finalize().as_bytes())
}
fn reject_legacy_runtime_dag_mutable_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    reject_legacy_atomic_state_names(
        root_guard.rooted_directory(),
        &[
            GOVERNANCE_RUNTIME_DAG_INDEX_FILE,
            "runtime-dag-index.json.blake3",
        ],
        "governance runtime DAG committed authority",
    )?;
    for path in [
        root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
        digest_sidecar_path_for(&root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)),
        runtime_dag_head_path(root),
        digest_sidecar_path_for(&runtime_dag_head_path(root)),
    ] {
        match read_rooted_governance_state_file(root_guard, &path, 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy mutable governance runtime DAG state `{}` is unsupported; remove it before first-release initialization",
                    path.display()
                )));
            }
        }
    }
    match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR))
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(directory) => {
            drop(directory);
            return Err(GovernancePublishError::other(format!(
                "legacy mutable governance runtime DAG staging directory `{GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR}` is unsupported; remove it before first-release initialization"
            )));
        }
        Err(error) => return Err(error.into()),
    }
    match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))
    {
        Ok(runtime_directory) => reject_legacy_atomic_state_names(
            &runtime_directory,
            &[GOVERNANCE_RUNTIME_DAG_HEAD_FILE, "head.to.blake3"],
            "governance runtime DAG committed head authority",
        )?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    root_guard.revalidate()?;
    Ok(())
}
fn open_runtime_dag_staging_store_v1(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<governance_rooted_fs::TwoSlotStoreV1, GovernancePublishError> {
    if root != root_guard.root() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staging root differs from its retained root guard",
        ));
    }
    reject_legacy_runtime_dag_mutable_state(root, root_guard)?;
    let initial = encode_governance_two_slot_value_v1(
        &RuntimeDagProducerStagingStateV1 {
            version: GOVERNANCE_RUNTIME_DAG_STAGING_STATE_VERSION_V1,
            staged: None,
        },
        "initial governance runtime DAG staging state",
    )?;
    open_governance_two_slot_store_v1(
        root_guard,
        GOVERNANCE_RUNTIME_DAG_STAGING_STORE_SPEC_V1,
        &initial,
    )
}
fn load_runtime_dag_staging_state_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<
    (
        RuntimeDagProducerStagingStateV1,
        governance_rooted_fs::TwoSlotSnapshotV1,
    ),
    GovernancePublishError,
> {
    let snapshot =
        load_governance_two_slot_store_v1(store, "governance runtime DAG staging state")?;
    let state: RuntimeDagProducerStagingStateV1 =
        decode_governance_two_slot_value_v1(&snapshot, "governance runtime DAG staging state")?;
    if state.version != GOVERNANCE_RUNTIME_DAG_STAGING_STATE_VERSION_V1 {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staging state version is unsupported",
        ));
    }
    Ok((state, snapshot))
}
fn open_runtime_dag_committed_store_v1(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<governance_rooted_fs::TwoSlotStoreV1, GovernancePublishError> {
    if root != root_guard.root() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG committed root differs from its retained root guard",
        ));
    }
    reject_legacy_runtime_dag_mutable_state(root, root_guard)?;
    let initial = encode_governance_two_slot_value_v1(
        &RuntimeDagCommittedStateV1 {
            version: GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1,
            head_bytes: None,
            index_bytes: None,
        },
        "initial governance runtime DAG committed state",
    )?;
    open_governance_two_slot_store_v1(
        root_guard,
        GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_SPEC_V1,
        &initial,
    )
}
fn load_runtime_dag_committed_state_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<
    (
        RuntimeDagCommittedStateV1,
        governance_rooted_fs::TwoSlotSnapshotV1,
    ),
    GovernancePublishError,
> {
    let snapshot =
        load_governance_two_slot_store_v1(store, "governance runtime DAG committed state")?;
    let state: RuntimeDagCommittedStateV1 =
        decode_governance_two_slot_value_v1(&snapshot, "governance runtime DAG committed state")?;
    validate_runtime_dag_committed_state_v1(&state)?;
    Ok((state, snapshot))
}
fn validate_runtime_dag_committed_state_v1(
    state: &RuntimeDagCommittedStateV1,
) -> Result<(), GovernancePublishError> {
    if state.version != GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1
        || state.head_bytes.is_some() != state.index_bytes.is_some()
        || state.head_bytes.as_ref().is_some_and(|bytes| {
            bytes.is_empty() || bytes.len() > GOVERNANCE_RUNTIME_DAG_HEAD_MAX_BYTES_V1
        })
        || state.index_bytes.as_ref().is_some_and(|bytes| {
            bytes.is_empty() || bytes.len() > GOVERNANCE_MUTABLE_INDEX_MAX_BYTES
        })
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG committed state is malformed or outside its byte bounds",
        ));
    }
    Ok(())
}
/// Load one exact committed runtime-DAG generation through a retained root.
///
/// This is the sole read boundary for consumers of mutable head/index state.
/// The two values are selected from one fixed-slot record, and an empty
/// initialized store is reported as `None`.
pub(crate) fn load_runtime_dag_committed_snapshot_v1(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<Option<RuntimeDagCommittedSnapshotV1>, GovernancePublishError> {
    let root = root_guard.root();
    root_guard.revalidate()?;
    reject_legacy_runtime_dag_mutable_state(root, root_guard)?;
    match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_DIR_V1))
    {
        Ok(directory) => drop(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let config = governance_two_slot_config_v1(GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_SPEC_V1)?;
    let snapshot = match root_guard
        .rooted_directory()
        .load_existing_two_slot_store_v1(config)
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(GovernancePublishError::other(format!(
                "failed to load governance runtime DAG committed two-slot state: {error}"
            )));
        }
    };
    let state: RuntimeDagCommittedStateV1 = decode_governance_two_slot_value_v1(
        &snapshot,
        "governance runtime DAG committed reader snapshot",
    )?;
    validate_runtime_dag_committed_state_v1(&state)?;
    root_guard.revalidate()?;
    match (state.head_bytes, state.index_bytes) {
        (Some(head_bytes), Some(index_bytes)) => Ok(Some(RuntimeDagCommittedSnapshotV1 {
            store_generation: snapshot.generation(),
            store_record_digest: snapshot.record_digest(),
            head_bytes,
            index_bytes,
        })),
        (None, None) => Ok(None),
        _ => Err(GovernancePublishError::other(
            "governance runtime DAG committed state contains a split head/index generation",
        )),
    }
}
fn validate_authenticated_runtime_dag_semantics_v1(
    root_guard: &GovernanceFilesystemRootGuard,
    authority_lineage: &RuntimeDagAuthorityLineageV1,
    snapshot: &RuntimeDagCommittedSnapshotV1,
    index: &JsonMap,
    indexed_blocks: &[JsonValue],
    head: &GovernanceDagHeadV1,
) -> Result<(), GovernancePublishError> {
    if indexed_blocks.is_empty() || indexed_blocks.len() > GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1
    {
        return Err(GovernancePublishError::other(
            "authenticated governance runtime DAG block count is outside its V1 bound",
        ));
    }
    let root = root_guard.root();
    let latest_allowed = current_unix_timestamp_seconds()
        .saturating_add(GOVERNANCE_RUNTIME_DAG_MAX_FUTURE_SKEW_SECS_V1);
    let mut total_bytes = 0_u64;
    add_runtime_dag_audit_bytes(&mut total_bytes, snapshot.index_bytes().len())?;
    add_runtime_dag_audit_bytes(&mut total_bytes, snapshot.head_bytes().len())?;
    let mut decoded_blocks = Vec::with_capacity(indexed_blocks.len());
    let mut expected_by_encoded_blake3 = JsonMap::new();
    let mut expected_by_source_payload_blake3 = JsonMap::new();
    let mut expected_by_payload_kind = JsonMap::new();
    let mut expected_block_names = BTreeSet::new();
    let mut previous_block_cid: Option<Vec<u8>> = None;
    let mut previous_node_cid: Option<Vec<u8>> = None;
    for (position, entry) in indexed_blocks.iter().enumerate() {
        let entry = entry.as_object().ok_or_else(|| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG index block is not an object",
            )
        })?;
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG position exceeds u64",
            )
        })?;
        if required_runtime_u64(entry, "position")? != position_u64
            || required_runtime_u64(entry, "sequence")? != position_u64
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG index position or sequence is noncanonical",
            ));
        }
        let indexed_block_cid = required_runtime_string(entry, "block_cid_hex")?;
        let indexed_node_cid = required_runtime_string(entry, "node_cid_hex")?;
        let indexed_block_cid_bytes = required_runtime_hex(entry, "block_cid_hex")?;
        let indexed_node_cid_bytes = required_runtime_hex(entry, "node_cid_hex")?;
        if indexed_block_cid_bytes.len() != 32
            || indexed_node_cid_bytes.len() != 32
            || indexed_block_cid != hex::encode(&indexed_block_cid_bytes)
            || indexed_node_cid != hex::encode(&indexed_node_cid_bytes)
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG block or node CID is noncanonical",
            ));
        }
        let block_path_string = required_runtime_string(entry, "block_path")?;
        let canonical_block_path = runtime_dag_block_path(root, position_u64, &indexed_block_cid);
        if block_path_string != index_path_string(root, &canonical_block_path) {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG block path is noncanonical",
            ));
        }
        let block_snapshot = read_rooted_governance_state_file(
            root_guard,
            &canonical_block_path,
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        )?;
        verify_rooted_digest_sidecar(root_guard, &canonical_block_path, block_snapshot.bytes())?;
        add_runtime_dag_audit_bytes(&mut total_bytes, block_snapshot.bytes().len())?;
        let block_len = u64::try_from(block_snapshot.bytes().len()).map_err(|_| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG block length exceeds u64",
            )
        })?;
        let block_digest_hex = blake3::hash(block_snapshot.bytes()).to_hex().to_string();
        if required_runtime_u64(entry, "encoded_len")? != block_len
            || required_runtime_string(entry, "encoded_blake3")? != block_digest_hex
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG block length or digest is substituted",
            ));
        }
        let block: GovernanceDagBlockV1 = decode_canonical_runtime_dag(
            block_snapshot.bytes(),
            "authenticated governance runtime DAG block",
        )?;
        block.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "authenticated governance runtime DAG block is invalid: {error}"
            ))
        })?;
        if block.sequence != position_u64
            || block.timestamp > latest_allowed
            || indexed_block_cid != hex::encode(&block.block_cid)
            || indexed_node_cid != hex::encode(&block.node.node_cid)
            || optional_runtime_string(entry, "prev_block_cid_hex")?
                != block.prev_block_cid.as_ref().map(hex::encode)
            || optional_runtime_string(entry, "prev_node_cid_hex")?
                != block.node.prev_cid.as_ref().map(hex::encode)
            || block.prev_block_cid != previous_block_cid
            || block.node.prev_cid != previous_node_cid
            || required_runtime_u64(entry, "published_at_unix")? != block.timestamp
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG block identity or parent lineage is substituted",
            ));
        }
        previous_block_cid = Some(block.block_cid.clone());
        previous_node_cid = Some(block.node.node_cid.clone());
        let payload_kind = runtime_dag_payload_kind(&block.node.payload);
        if !runtime_dag_payload_kind_is_supported(payload_kind)
            || required_runtime_string(entry, "payload_kind")? != payload_kind
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG payload kind is unsupported or substituted",
            ));
        }
        let submission_account_digest = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| hex::encode(provenance.publisher_account_digest));
        let submission_origin = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| provenance.origin.label().to_owned());
        if required_optional_runtime_string(entry, "submission_publisher_account_digest_hex")?
            != submission_account_digest
            || required_optional_runtime_string(entry, "submission_origin")? != submission_origin
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG submission provenance is substituted",
            ));
        }
        let source_path_string = required_runtime_string(entry, "encoded_path")?;
        let source_path = resolve_index_path(root, &source_path_string)?;
        let source_snapshot = read_rooted_governance_state_file(
            root_guard,
            &source_path,
            GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES,
        )?;
        verify_rooted_digest_sidecar(root_guard, &source_path, source_snapshot.bytes())?;
        add_runtime_dag_audit_bytes(&mut total_bytes, source_snapshot.bytes().len())?;
        let source_len = u64::try_from(source_snapshot.bytes().len()).map_err(|_| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG source length exceeds u64",
            )
        })?;
        let source_digest_hex = blake3::hash(source_snapshot.bytes()).to_hex().to_string();
        if required_runtime_u64(entry, "source_payload_len")? != source_len
            || required_runtime_string(entry, "source_payload_blake3")? != source_digest_hex
            || canonical_runtime_source_payload_bytes(&block.node.payload)?
                != source_snapshot.bytes()
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG source payload is substituted",
            ));
        }
        let json_path_string = required_runtime_string(entry, "json_path")?;
        let json_path = resolve_index_path(root, &json_path_string)?;
        let json_snapshot = read_rooted_governance_state_file(
            root_guard,
            &json_path,
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        )?;
        verify_rooted_digest_sidecar(root_guard, &json_path, json_snapshot.bytes())?;
        add_runtime_dag_audit_bytes(&mut total_bytes, json_snapshot.bytes().len())?;
        validate_governance_car_source_lengths(
            source_snapshot.bytes().len(),
            json_snapshot.bytes().len(),
        )?;
        let json_len = u64::try_from(json_snapshot.bytes().len()).map_err(|_| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG JSON source length exceeds u64",
            )
        })?;
        let json_digest_hex = blake3::hash(json_snapshot.bytes()).to_hex().to_string();
        let expected_source_paths = governance_source_pair_relative_paths(
            payload_kind,
            source_len,
            &source_digest_hex,
            json_len,
            &json_digest_hex,
        )?;
        if source_path_string != expected_source_paths.0
            || json_path_string != expected_source_paths.1
        {
            return Err(GovernancePublishError::other(
                "authenticated governance runtime DAG source paths do not bind their immutable bytes",
            ));
        }
        append_runtime_index_position(
            &mut expected_by_encoded_blake3,
            &block_digest_hex,
            position_u64,
        );
        append_runtime_index_position(
            &mut expected_by_source_payload_blake3,
            &source_digest_hex,
            position_u64,
        );
        append_runtime_index_position(&mut expected_by_payload_kind, payload_kind, position_u64);
        let block_name = canonical_block_path.file_name().ok_or_else(|| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG block path has no file name",
            )
        })?;
        expected_block_names.insert(block_name.to_os_string());
        expected_block_names.insert(
            digest_sidecar_path_for(&canonical_block_path)
                .file_name()
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "authenticated governance runtime DAG block sidecar has no file name",
                    )
                })?
                .to_os_string(),
        );
        block_snapshot.binding().verify()?;
        source_snapshot.binding().verify()?;
        json_snapshot.binding().verify()?;
        decoded_blocks.push(block);
    }
    for (field, expected) in [
        ("by_encoded_blake3", expected_by_encoded_blake3),
        (
            "by_source_payload_blake3",
            expected_by_source_payload_blake3,
        ),
        ("by_payload_kind", expected_by_payload_kind),
    ] {
        if index.get(field) != Some(&JsonValue::Object(expected)) {
            return Err(GovernancePublishError::other(format!(
                "authenticated governance runtime DAG reverse map `{field}` is substituted"
            )));
        }
    }
    let runtime_root = root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))?;
    if runtime_root.child_names_bounded(2)?
        != vec![OsString::from(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)]
    {
        return Err(GovernancePublishError::other(
            "authenticated governance runtime DAG immutable root inventory is noncanonical",
        ));
    }
    let blocks_directory =
        runtime_root.open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR))?;
    let inventory_bound = GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1
        .checked_mul(2)
        .and_then(|bound| bound.checked_add(1))
        .ok_or_else(|| {
            GovernancePublishError::other(
                "authenticated governance runtime DAG inventory bound overflowed",
            )
        })?;
    let actual_block_names = blocks_directory
        .child_names_bounded(inventory_bound)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual_block_names != expected_block_names {
        return Err(GovernancePublishError::other(
            "authenticated governance runtime DAG block inventory contains an unindexed or missing artifact",
        ));
    }
    if head.generated_at > latest_allowed {
        return Err(GovernancePublishError::other(
            "authenticated governance runtime DAG head is future-dated",
        ));
    }
    validate_governance_dag_head_against_rotatable_chain_v1(head, &decoded_blocks).map_err(
        |error| {
            GovernancePublishError::other(format!(
                "authenticated governance runtime DAG head does not authenticate its chain: {error}"
            ))
        },
    )?;
    let authority_blocks = decoded_blocks.iter().collect::<Vec<_>>();
    validate_runtime_dag_authority_lineage_for_chain(authority_lineage, &authority_blocks, head)?;
    root_guard.revalidate()?;
    Ok(())
}
/// Load a runtime-DAG generation bracketed by one exact sealed producer
/// checkpoint without initializing or reconciling any local state.
///
/// An authenticated genesis checkpoint returns `None`. A non-genesis
/// checkpoint requires one typed head/index generation whose canonical bytes,
/// digests, count, CID, root, and provider bindings all match the checkpoint.
pub(crate) fn load_authenticated_runtime_dag_snapshot_v1(
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<Option<AuthenticatedRuntimeDagSnapshotV1>, GovernancePublishError> {
    let root = root_guard.root();
    root_guard.revalidate()?;
    signer.assert_qualification()?;
    store.assert_qualification()?;
    match root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_DIR_V1))
    {
        Ok(directory) => drop(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG checkpoint has no initialized typed committed store",
            ));
        }
        Err(error) => return Err(error.into()),
    }
    let checkpoint_record_a = store
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "sealed governance runtime DAG producer checkpoint is missing",
            )
        })?;
    if store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)?
        .is_some()
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer transaction is active",
        ));
    }
    let checkpoint = decode_runtime_dag_unqualified_checkpoint_record(&checkpoint_record_a, root)?;
    let expected_binding = runtime_dag_provider_binding(signer, store);
    if runtime_dag_checkpoint_binding(&checkpoint) != expected_binding {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint belongs to another qualified provider binding",
        ));
    }
    let authority_lineage =
        authenticated_runtime_dag_authority_lineage_read_only(root_guard, &checkpoint)?;
    // The fixed-store generation is intentionally not derived from the block
    // count. A qualified provider transition rewrites the authenticated index
    // binding without appending a block and therefore advances this store
    // independently. The sealed checkpoint instead binds the exact head/index
    // byte digests, block count, CID, and current provider identities below.
    let committed = load_runtime_dag_committed_snapshot_v1(root_guard)?;
    match committed.as_ref() {
        Some(snapshot)
            if checkpoint.block_count == 0
                || checkpoint.head_bytes_digest
                    != *blake3::hash(snapshot.head_bytes()).as_bytes()
                || checkpoint.index_bytes_digest
                    != *blake3::hash(snapshot.index_bytes()).as_bytes() =>
        {
            return Err(GovernancePublishError::other(
                "typed governance runtime DAG byte generation does not match its sealed producer checkpoint",
            ));
        }
        Some(_) => {}
        None if checkpoint.block_count != 0 => {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG producer checkpoint has no typed head/index generation",
            ));
        }
        None => match root_guard
            .rooted_directory()
            .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))
        {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(directory) => {
                drop(directory);
                return Err(GovernancePublishError::other(
                    "authenticated governance runtime DAG genesis has unindexed immutable artifacts",
                ));
            }
            Err(error) => return Err(error.into()),
        },
    }
    if let Some(snapshot) = committed.as_ref() {
        let head: GovernanceDagHeadV1 = decode_canonical_runtime_dag(
            snapshot.head_bytes(),
            "governance runtime DAG reader head",
        )?;
        head.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "governance runtime DAG reader head is invalid: {error}"
            ))
        })?;
        let index_value: JsonValue = json::from_slice(snapshot.index_bytes()).map_err(|error| {
            GovernancePublishError::other(format!(
                "governance runtime DAG reader index is invalid JSON: {error}"
            ))
        })?;
        let canonical_index = json::to_json_pretty(&index_value).map_err(|error| {
            GovernancePublishError::other(format!(
                "governance runtime DAG reader index cannot be canonicalized: {error}"
            ))
        })?;
        if canonical_index.as_bytes() != snapshot.index_bytes() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG reader index is noncanonical",
            ));
        }
        let index = index_value.as_object().ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG reader index is not an object")
        })?;
        require_exact_governance_fields(
            index,
            GOVERNANCE_RUNTIME_DAG_INDEX_FIELDS_V1,
            "governance runtime DAG index",
        )?;
        if required_runtime_string(index, "schema")? != GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA
            || required_runtime_string(index, "source")? != GOVERNANCE_DAG_SINK_FILESYSTEM
            || required_runtime_string(index, "root")? != GOVERNANCE_DAG_LOGICAL_ROOT
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG reader index identity is invalid",
            ));
        }
        validate_runtime_dag_signer_fields(index, signer)?;
        validate_runtime_dag_checkpoint_store_fields(index, store)?;
        let blocks = index
            .get("blocks")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG reader index blocks are missing",
                )
            })?;
        for block in blocks {
            let block = block.as_object().ok_or_else(|| {
                GovernancePublishError::other(
                    "governance runtime DAG reader index block is not an object",
                )
            })?;
            require_exact_governance_fields(
                block,
                GOVERNANCE_RUNTIME_DAG_INDEX_BLOCK_FIELDS_V1,
                "governance runtime DAG index block",
            )?;
        }
        let indexed_count = u64::try_from(blocks.len()).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG block count exceeds u64")
        })?;
        let head_cid: [u8; 32] = head.head_block_cid.as_slice().try_into().map_err(|_| {
            GovernancePublishError::other("governance runtime DAG reader head CID is not 32 bytes")
        })?;
        if checkpoint.block_count == 0
            || checkpoint.block_count != head.block_count
            || checkpoint.block_count != indexed_count
            || checkpoint.head_block_cid != head_cid
            || required_runtime_u64(index, "block_count")? != head.block_count
            || required_runtime_string(index, "head_block_cid_hex")?
                != hex::encode(&head.head_block_cid)
            || required_runtime_u64(index, "head_generated_at")? != head.generated_at
            || required_runtime_u64(index, "generated_at")? != head.generated_at
        {
            return Err(GovernancePublishError::other(
                "typed governance runtime DAG generation does not match its sealed producer checkpoint",
            ));
        }
        validate_authenticated_runtime_dag_semantics_v1(
            root_guard,
            &authority_lineage,
            snapshot,
            index,
            blocks,
            &head,
        )?;
    }
    let checkpoint_record_b = store
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "sealed governance runtime DAG producer checkpoint disappeared during read",
            )
        })?;
    if store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)?
        .is_some()
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer transaction changed during read",
        ));
    }
    if checkpoint_record_b != checkpoint_record_a {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint changed during read",
        ));
    }
    root_guard.revalidate()?;
    signer.assert_qualification()?;
    store.assert_qualification()?;
    Ok(
        committed.map(|committed| AuthenticatedRuntimeDagSnapshotV1 {
            committed,
            checkpoint_generation: checkpoint_record_a.generation,
            checkpoint_revision: checkpoint_record_a.revision,
        }),
    )
}
#[cfg(test)]
pub(crate) fn write_runtime_dag_committed_snapshot_fixture_v1(
    root: &Path,
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
) -> Result<(), GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    let store = open_runtime_dag_committed_store_v1(root_guard.root(), &root_guard)?;
    let (_, snapshot) = load_runtime_dag_committed_state_v1(&store)?;
    let state = RuntimeDagCommittedStateV1 {
        version: GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1,
        head_bytes: Some(head_bytes),
        index_bytes: Some(index_bytes),
    };
    validate_runtime_dag_committed_state_v1(&state)?;
    let bytes = encode_governance_two_slot_value_v1(
        &state,
        "governance runtime DAG committed test snapshot",
    )?;
    compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &bytes,
        "governance runtime DAG committed test snapshot",
    )?;
    Ok(())
}
fn stage_runtime_dag_producer_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
    staged: &RuntimeDagProducerStagedTransactionV1,
) -> Result<(), GovernancePublishError> {
    validate_runtime_dag_producer_intent_bounds(root, intent, staged)?;
    let store = open_runtime_dag_staging_store_v1(root, root_guard)?;
    let (current, snapshot) = load_runtime_dag_staging_state_v1(&store)?;
    let next = RuntimeDagProducerStagingStateV1 {
        version: GOVERNANCE_RUNTIME_DAG_STAGING_STATE_VERSION_V1,
        staged: Some(RuntimeDagProducerStagedEnvelopeV1 {
            intent: intent.clone(),
            transaction: staged.clone(),
        }),
    };
    if current == next {
        return Ok(());
    }
    let bytes =
        encode_governance_two_slot_value_v1(&next, "governance runtime DAG staging transaction")?;
    let committed = compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &bytes,
        "governance runtime DAG staging transaction",
    )?;
    let readback: RuntimeDagProducerStagingStateV1 = decode_governance_two_slot_value_v1(
        &committed,
        "governance runtime DAG staging transaction readback",
    )?;
    if readback != next {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staging transaction readback diverged",
        ));
    }
    Ok(())
}
fn load_runtime_dag_producer_staged_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<RuntimeDagProducerStagedTransactionV1, GovernancePublishError> {
    let store = open_runtime_dag_staging_store_v1(root, root_guard)?;
    let (state, _) = load_runtime_dag_staging_state_v1(&store)?;
    let envelope = state.staged.ok_or_else(|| {
        GovernancePublishError::other(
            "sealed governance runtime DAG producer intent has no staged transaction",
        )
    })?;
    if envelope.intent != *intent {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staged transaction belongs to another sealed intent",
        ));
    }
    let staged = envelope.transaction;
    validate_runtime_dag_producer_intent_bounds(root, intent, &staged)?;
    Ok(staged)
}
fn clear_runtime_dag_producer_staged_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<(), GovernancePublishError> {
    let store = open_runtime_dag_staging_store_v1(root, root_guard)?;
    let (state, snapshot) = load_runtime_dag_staging_state_v1(&store)?;
    let Some(envelope) = state.staged else {
        return Ok(());
    };
    if envelope.intent != *intent {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staging cleanup refuses another active transaction",
        ));
    }
    let cleared = RuntimeDagProducerStagingStateV1 {
        version: GOVERNANCE_RUNTIME_DAG_STAGING_STATE_VERSION_V1,
        staged: None,
    };
    let bytes = encode_governance_two_slot_value_v1(
        &cleared,
        "cleared governance runtime DAG staging state",
    )?;
    let committed = compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &bytes,
        "cleared governance runtime DAG staging state",
    )?;
    let readback: RuntimeDagProducerStagingStateV1 = decode_governance_two_slot_value_v1(
        &committed,
        "cleared governance runtime DAG staging state readback",
    )?;
    if readback != cleared {
        return Err(GovernancePublishError::other(
            "governance runtime DAG staging cleanup readback diverged",
        ));
    }
    Ok(())
}
fn runtime_dag_producer_block_path_from_intent(
    root: &Path,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<PathBuf, GovernancePublishError> {
    let sequence = intent
        .checkpoint
        .block_count
        .checked_sub(1)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "sealed governance runtime DAG producer intent has no successor block",
            )
        })?;
    Ok(runtime_dag_block_path(
        root,
        sequence,
        &hex::encode(intent.checkpoint.head_block_cid),
    ))
}
fn validate_runtime_dag_producer_file_lengths(
    block_len: usize,
    head_len: usize,
    index_len: usize,
) -> Result<(), GovernancePublishError> {
    for (label, len, limit) in [
        ("block", block_len, GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES),
        ("head", head_len, GOVERNANCE_RUNTIME_DAG_HEAD_MAX_BYTES_V1),
        ("index", index_len, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
    ] {
        if len == 0 || len > limit {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG producer intent {label} is outside the per-file byte limit"
            )));
        }
    }
    Ok(())
}
fn validate_runtime_dag_producer_entry_count(
    entries: usize,
    checkpoint_count: u64,
) -> Result<(), GovernancePublishError> {
    if entries == 0
        || entries > GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1
        || u64::try_from(entries).unwrap_or(u64::MAX) != checkpoint_count
    {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer intent block count is outside the hard limit",
        ));
    }
    Ok(())
}
fn decode_runtime_dag_producer_intent_metadata_record(
    record: &GovernanceDagSealedStateRecord,
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<RuntimeDagProducerPublishIntentV1, GovernancePublishError> {
    if !record.has_valid_revision(GovernanceDagSealedStateSlot::ProducerPublishIntent) {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent revision is invalid",
        ));
    }
    let intent: RuntimeDagProducerPublishIntentV1 = decode_canonical_runtime_dag(
        &record.payload,
        "sealed governance runtime DAG producer intent",
    )?;
    validate_runtime_dag_producer_checkpoint(&intent.checkpoint, root, signer, store)?;
    if intent.version != GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_VERSION_V1
        || record.generation != runtime_dag_producer_checkpoint_generation(&intent.checkpoint)?
        || intent.staging_revision == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent is malformed",
        ));
    }
    validate_runtime_dag_producer_intent_metadata(&intent)?;
    Ok(intent)
}
fn load_and_validate_runtime_dag_producer_staged_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<RuntimeDagProducerStagedTransactionV1, GovernancePublishError> {
    let staged = load_runtime_dag_producer_staged_transaction(root, root_guard, intent)?;
    let block: GovernanceDagBlockV1 = decode_canonical_runtime_dag(
        &staged.block_bytes,
        "sealed governance runtime DAG producer intent block",
    )?;
    let expected_path = runtime_dag_producer_block_path_from_intent(root, intent)?;
    if runtime_dag_block_path(root, block.sequence, &hex::encode(&block.block_cid)) != expected_path
        || runtime_dag_producer_checkpoint(
            root,
            signer,
            store,
            &staged.head_bytes,
            &staged.index_bytes,
        )? != intent.checkpoint
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent bytes or path are substituted",
        ));
    }
    Ok(staged)
}
#[cfg(windows)]
fn isolate_recoverable_atomic_state_for_target(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    _max_bytes: usize,
    _quarantine_slot_prefix: &str,
) -> Result<(), GovernancePublishError> {
    reject_governance_publication_recovery_quarantine(root_guard)?;
    let target_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG transaction target name is not canonical UTF-8",
            )
        })?;
    let (parent, _) = match rooted_target(root_guard, path, false) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    parent.remove_atomic_temps_for(target_name)?;
    root_guard.revalidate()?;
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn plan_recoverable_atomic_temps_for_target(
    parent: &governance_rooted_fs::RootedDirectory,
    target_name: &str,
    max_bytes: usize,
    quarantine_slot_prefix: &str,
    plan: &mut GovernancePublicationArtifactCleanupPlan,
) -> Result<(), GovernancePublishError> {
    for name in parent.child_names_bounded(GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP)? {
        let Some(name_utf8) = name.to_str() else {
            continue;
        };
        let decoded = governance_publication_atomic_temp_target_name(name_utf8);
        if decoded != Some(target_name) {
            if name_utf8
                .strip_prefix('.')
                .and_then(|name| name.strip_prefix(target_name))
                .is_some_and(|suffix| suffix.starts_with(".tmp-"))
            {
                return Err(GovernancePublishError::other(format!(
                    "governance atomic temporary name `{name_utf8}` is noncanonical; offline inspection is required"
                )));
            }
            continue;
        }
        let rollback_rank = plan.authority_files.len();
        let removal = plan_governance_publication_file_removal(
            parent,
            &name,
            max_bytes,
            None,
            rollback_rank,
            OsString::from(format!("{quarantine_slot_prefix}-{rollback_rank:06}")),
        )?;
        plan.authority_files.push(removal);
    }
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn isolate_recoverable_atomic_state_for_target(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    max_bytes: usize,
    quarantine_slot_prefix: &str,
) -> Result<(), GovernancePublishError> {
    reject_governance_publication_recovery_quarantine(root_guard)?;
    let target_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance atomic recovery target name is not canonical UTF-8",
            )
        })?;
    let (parent, _) = match rooted_target(root_guard, path, false) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut plan = GovernancePublicationArtifactCleanupPlan::default();
    plan_recoverable_atomic_temps_for_target(
        &parent,
        target_name,
        max_bytes,
        quarantine_slot_prefix,
        &mut plan,
    )?;
    root_guard.revalidate()?;
    apply_governance_publication_cleanup_plan(root_guard, plan)
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn isolate_recoverable_atomic_state_for_target(
    _root_guard: &GovernanceFilesystemRootGuard,
    _path: &Path,
    _max_bytes: usize,
    _quarantine_slot_prefix: &str,
) -> Result<(), GovernancePublishError> {
    Err(GovernancePublishError::other(
        "governance atomic recovery is unsupported on this platform",
    ))
}
fn validate_runtime_dag_producer_intent_successor(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    intent: &RuntimeDagProducerPublishIntentV1,
    staged: &RuntimeDagProducerStagedTransactionV1,
    previous: Option<&RuntimeDagProducerCheckpointV1>,
) -> Result<(), GovernancePublishError> {
    let previous_count = previous.map_or(0, |checkpoint| checkpoint.block_count);
    let expected_count = previous_count.checked_add(1).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG producer intent successor count exhausted",
        )
    })?;
    if intent.checkpoint.block_count != expected_count {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent is not the direct checkpoint successor",
        ));
    }
    let block: GovernanceDagBlockV1 = decode_canonical_runtime_dag(
        &staged.block_bytes,
        "sealed governance runtime DAG successor block",
    )?;
    block.validate().map_err(|error| {
        GovernancePublishError::other(format!(
            "sealed governance runtime DAG successor block is invalid: {error}"
        ))
    })?;
    let head: GovernanceDagHeadV1 = decode_canonical_runtime_dag(
        &staged.head_bytes,
        "sealed governance runtime DAG successor head",
    )?;
    head.validate().map_err(|error| {
        GovernancePublishError::other(format!(
            "sealed governance runtime DAG successor head is invalid: {error}"
        ))
    })?;
    head.verify_head_signature().map_err(|error| {
        GovernancePublishError::other(format!(
            "sealed governance runtime DAG successor head signature is invalid: {error}"
        ))
    })?;
    if block.sequence != previous_count
        || block.block_cid.as_slice() != intent.checkpoint.head_block_cid
        || head.block_count != expected_count
        || head.head_block_cid != block.block_cid
        || block.publisher_peer_id != signer.publisher_peer_id
        || block.node.publisher_peer_id != signer.publisher_peer_id
        || block.block_signature.public_key != signer.public_key.to_vec()
        || block.node.publisher_signature.public_key != signer.public_key.to_vec()
        || head.publisher_peer_id != signer.publisher_peer_id
        || head.head_signature.public_key != signer.public_key.to_vec()
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG successor block, head, or signer binding is substituted",
        ));
    }
    let new_value: JsonValue = json::from_slice(&staged.index_bytes).map_err(|error| {
        GovernancePublishError::other(format!(
            "sealed governance runtime DAG successor index is invalid: {error}"
        ))
    })?;
    let new_index = new_value.as_object().ok_or_else(|| {
        GovernancePublishError::other(
            "sealed governance runtime DAG successor index is not an object",
        )
    })?;
    let new_blocks = new_index
        .get("blocks")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "sealed governance runtime DAG successor index has no block array",
            )
        })?;
    if new_blocks.len() != usize::try_from(expected_count).unwrap_or(usize::MAX) {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG successor index block count is inconsistent",
        ));
    }
    let last = new_blocks
        .last()
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "sealed governance runtime DAG successor index has no final block entry",
            )
        })?;
    let expected_block_path =
        runtime_dag_block_path(root, block.sequence, &hex::encode(&block.block_cid));
    if required_runtime_u64(last, "sequence")? != block.sequence
        || required_runtime_u64(last, "encoded_len")?
            != u64::try_from(staged.block_bytes.len()).unwrap_or(u64::MAX)
        || required_runtime_string(last, "encoded_blake3")?
            != blake3::hash(&staged.block_bytes).to_hex().to_string()
        || required_runtime_string(last, "block_cid_hex")? != hex::encode(&block.block_cid)
        || required_runtime_string(last, "node_cid_hex")? != hex::encode(&block.node.node_cid)
        || required_runtime_string(last, "block_path")?
            != index_path_string(root, &expected_block_path)
        || runtime_dag_producer_block_path_from_intent(root, intent)? != expected_block_path
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG successor block does not match its final index entry",
        ));
    }
    let prior_tip = if previous_count == 0 {
        None
    } else {
        runtime_dag_tip_from_entries(
            new_blocks
                .get(..usize::try_from(previous_count).unwrap_or(usize::MAX))
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "sealed governance runtime DAG successor lost its prior index prefix",
                    )
                })?,
        )?
    };
    if block.prev_block_cid != prior_tip.as_ref().map(|tip| tip.block_cid.clone())
        || block.node.prev_cid != prior_tip.as_ref().map(|tip| tip.node_cid.clone())
        || previous.is_some_and(|checkpoint| {
            checkpoint.block_count != 0
                && block.prev_block_cid.as_deref() != Some(checkpoint.head_block_cid.as_slice())
        })
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG successor does not extend the exact prior tip",
        ));
    }
    let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
    let (committed, _) = load_runtime_dag_committed_state_v1(&committed_store)?;
    match committed.index_bytes.as_deref() {
        Some(current) if current == staged.index_bytes => {}
        Some(current)
            if previous.is_some_and(|checkpoint| {
                *blake3::hash(current).as_bytes() == checkpoint.index_bytes_digest
            }) =>
        {
            let previous_value: JsonValue = json::from_slice(current).map_err(|error| {
                GovernancePublishError::other(format!(
                    "prior governance runtime DAG index is invalid: {error}"
                ))
            })?;
            let previous_blocks = previous_value
                .get("blocks")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "prior governance runtime DAG index has no block array",
                    )
                })?;
            let prefix_len = usize::try_from(previous_count).map_err(|_| {
                GovernancePublishError::other(
                    "prior governance runtime DAG block count exceeds host limits",
                )
            })?;
            if previous_blocks.len() != prefix_len
                || new_blocks.get(..prefix_len) != Some(previous_blocks.as_slice())
            {
                return Err(GovernancePublishError::other(
                    "sealed governance runtime DAG successor does not preserve the exact prior index prefix",
                ));
            }
        }
        None if previous.is_none_or(|checkpoint| checkpoint.block_count == 0) => {}
        Some(_) | None => {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG successor cannot authenticate the current index boundary",
            ));
        }
    }
    match committed.head_bytes.as_deref() {
        Some(current) if current == staged.head_bytes => {}
        Some(current)
            if previous.is_some_and(|checkpoint| {
                *blake3::hash(current).as_bytes() == checkpoint.head_bytes_digest
            }) => {}
        None if previous.is_none_or(|checkpoint| checkpoint.block_count == 0) => {}
        Some(_) | None => {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG successor cannot authenticate the current head boundary",
            ));
        }
    }
    Ok(())
}
fn apply_runtime_dag_producer_intent(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    intent: &RuntimeDagProducerPublishIntentV1,
    staged: &RuntimeDagProducerStagedTransactionV1,
    previous: Option<&RuntimeDagProducerCheckpointV1>,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    let block_path = runtime_dag_producer_block_path_from_intent(root, intent)?;
    root_guard.revalidate()?;
    write_immutable_governance_artifact(
        root_guard,
        &block_path,
        &staged.block_bytes,
        GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
    )?;
    root_guard.revalidate()?;
    let committed_store = open_runtime_dag_committed_store_v1(root, root_guard)?;
    let (current, snapshot) = load_runtime_dag_committed_state_v1(&committed_store)?;
    let next = RuntimeDagCommittedStateV1 {
        version: GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_VERSION_V1,
        head_bytes: Some(staged.head_bytes.clone()),
        index_bytes: Some(staged.index_bytes.clone()),
    };
    if current != next {
        let predecessor_matches = match (
            previous,
            current.head_bytes.as_deref(),
            current.index_bytes.as_deref(),
        ) {
            (None, None, None) => true,
            (Some(previous), Some(head), Some(index)) => {
                *blake3::hash(head).as_bytes() == previous.head_bytes_digest
                    && *blake3::hash(index).as_bytes() == previous.index_bytes_digest
            }
            _ => false,
        };
        if !predecessor_matches {
            return Err(GovernancePublishError::other(
                "governance runtime DAG committed state is not the sealed intent predecessor",
            ));
        }
        let bytes = encode_governance_two_slot_value_v1(
            &next,
            "governance runtime DAG committed head/index transaction",
        )?;
        let committed = compare_and_swap_governance_two_slot_store_v1(
            &committed_store,
            &snapshot,
            &bytes,
            "governance runtime DAG committed head/index transaction",
        )?;
        let readback: RuntimeDagCommittedStateV1 = decode_governance_two_slot_value_v1(
            &committed,
            "governance runtime DAG committed head/index readback",
        )?;
        if readback != next {
            return Err(GovernancePublishError::other(
                "governance runtime DAG committed head/index readback diverged",
            ));
        }
    }
    root_guard.revalidate()?;
    validate_existing_runtime_dag_root(root, signer, store)?;
    let local = local_runtime_dag_producer_checkpoint(root, signer, store)?.ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG producer transaction did not create a local checkpoint",
        )
    })?;
    if local != intent.checkpoint {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer transaction readback diverged",
        ));
    }
    root_guard.revalidate()?;
    Ok(())
}
fn finish_runtime_dag_producer_intent(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    intent_record: GovernanceDagSealedStateRecord,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    let intent =
        decode_runtime_dag_producer_intent_metadata_record(&intent_record, root, signer, store)?;
    let current_record = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?;
    let current = current_record
        .as_ref()
        .map(|record| decode_runtime_dag_producer_checkpoint_record(record, root, signer, store))
        .transpose()?;
    let next_record = runtime_dag_producer_checkpoint_record(&intent.checkpoint)?;
    let already_checkpointed = current_record.as_ref() == Some(&next_record);
    if already_checkpointed {
        root_guard.revalidate()?;
        validate_existing_runtime_dag_root(root, signer, store)?;
        let local =
            local_runtime_dag_producer_checkpoint(root, signer, store)?.ok_or_else(|| {
                GovernancePublishError::other(
                    "sealed governance runtime DAG producer checkpoint has no local root",
                )
            })?;
        if local != intent.checkpoint {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG committed intent does not match the audited local root",
            ));
        }
        root_guard.revalidate()?;
        store.delete(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
            intent_record.revision,
        )?;
        clear_runtime_dag_producer_staged_transaction(root, root_guard, &intent)?;
        signer.assert_qualification()?;
        return store.assert_qualification();
    }
    if !already_checkpointed
        && current_record.as_ref().map(|record| record.revision)
            != intent.previous_checkpoint_revision
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint changed during intent recovery",
        ));
    }
    let staged = load_and_validate_runtime_dag_producer_staged_transaction(
        root, root_guard, signer, store, &intent,
    )?;
    validate_runtime_dag_producer_intent_successor(
        root,
        root_guard,
        signer,
        &intent,
        &staged,
        current.as_ref(),
    )?;
    root_guard.revalidate()?;
    let filesystem_previous = current
        .as_ref()
        .filter(|checkpoint| checkpoint.block_count != 0);
    apply_runtime_dag_producer_intent(
        root,
        root_guard,
        signer,
        store,
        &intent,
        &staged,
        filesystem_previous,
    )?;
    if !already_checkpointed {
        signer.assert_qualification()?;
        store.assert_qualification()?;
        store.compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            intent.previous_checkpoint_revision,
            next_record,
        )?;
    }
    store.delete(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
        intent_record.revision,
    )?;
    clear_runtime_dag_producer_staged_transaction(root, root_guard, &intent)?;
    signer.assert_qualification()?;
    store.assert_qualification()
}
fn reconcile_runtime_dag_producer_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    drop(open_runtime_dag_staging_store_v1(root, root_guard)?);
    drop(open_runtime_dag_committed_store_v1(root, root_guard)?);
    recover_runtime_dag_qualification_compaction(root, root_guard, signer, store)?;
    recover_runtime_dag_provider_transition(root, root_guard, signer, store)?;
    root_guard.revalidate()?;
    match store.load(GovernanceDagSealedStateSlot::ProducerPublishIntent)? {
        Some(intent) => {
            finish_runtime_dag_producer_intent(root, root_guard, signer, store, intent)?;
        }
        None => {
            // Staging is deliberately installed before its sealed intent. A
            // crash in that narrow window leaves an unauthoritative local
            // transaction which must not become an implicit intent on restart.
            // Conversely, a crash after deleting a completed sealed intent but
            // before this local clear leaves the same safe cleanup shape.
            let staging_store = open_runtime_dag_staging_store_v1(root, root_guard)?;
            let (staging, _) = load_runtime_dag_staging_state_v1(&staging_store)?;
            drop(staging_store);
            if let Some(envelope) = staging.staged {
                if store
                    .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)?
                    .is_some()
                {
                    return Err(GovernancePublishError::other(
                        "sealed governance runtime DAG producer intent appeared while reconciling unsealed staging",
                    ));
                }
                clear_runtime_dag_producer_staged_transaction(root, root_guard, &envelope.intent)?;
            }
        }
    }
    root_guard.revalidate()?;
    let sealed_record = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?;
    if sealed_record.is_none() {
        let local = local_runtime_dag_producer_checkpoint(root, signer, store)?;
        if local.is_some() {
            return Err(GovernancePublishError::other(
                "local governance runtime DAG exists without its sealed producer root binding",
            ));
        }
        let genesis = empty_runtime_dag_producer_checkpoint(root, signer, store)?;
        signer.assert_qualification()?;
        store.assert_qualification()?;
        store.compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            None,
            runtime_dag_producer_checkpoint_record(&genesis)?,
        )?;
    }
    let sealed_record = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?;
    let sealed = sealed_record
        .as_ref()
        .map(|record| decode_runtime_dag_producer_checkpoint_record(record, root, signer, store))
        .transpose()?;
    let local = local_runtime_dag_producer_checkpoint(root, signer, store)?;
    let local = match local {
        Some(local) => Some(local),
        None => Some(empty_runtime_dag_producer_checkpoint(root, signer, store)?),
    };
    if sealed != local {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer checkpoint does not match the audited local root",
        ));
    }
    root_guard.revalidate()?;
    Ok(())
}
/// Requalify both signed-producer providers and authenticate the complete local root.
pub(crate) fn revalidate_runtime_dag_producer_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<(), GovernancePublishError> {
    signer.assert_qualification()?;
    store.assert_qualification()?;
    reconcile_runtime_dag_producer_state(root, root_guard, signer, store)?;
    signer.assert_qualification()?;
    store.assert_qualification()
}
#[allow(clippy::too_many_arguments)]
fn commit_runtime_dag_producer_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    block_path: &Path,
    block_bytes: Vec<u8>,
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
) -> Result<(), GovernancePublishError> {
    root_guard.revalidate()?;
    reconcile_runtime_dag_producer_state(root, root_guard, signer, store)?;
    let previous_record = store.load(GovernanceDagSealedStateSlot::ProducerCheckpoint)?;
    let previous = previous_record
        .as_ref()
        .map(|record| decode_runtime_dag_producer_checkpoint_record(record, root, signer, store))
        .transpose()?;
    let checkpoint =
        runtime_dag_producer_checkpoint(root, signer, store, &head_bytes, &index_bytes)?;
    let expected_count = previous
        .as_ref()
        .map_or(Some(1), |checkpoint| checkpoint.block_count.checked_add(1))
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG producer checkpoint generation exhausted",
            )
        })?;
    if checkpoint.block_count != expected_count {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer transaction is not the direct checkpoint successor",
        ));
    }
    let staged = RuntimeDagProducerStagedTransactionV1 {
        block_bytes,
        head_bytes,
        index_bytes,
    };
    let block = runtime_dag_producer_staged_artifact(&staged.block_bytes)?;
    let head = runtime_dag_producer_staged_artifact(&staged.head_bytes)?;
    let index = runtime_dag_producer_staged_artifact(&staged.index_bytes)?;
    let previous_checkpoint_revision = previous_record.as_ref().map(|record| record.revision);
    let staging_revision = runtime_dag_producer_staging_revision(
        &checkpoint,
        previous_checkpoint_revision,
        &block,
        &head,
        &index,
    )?;
    let intent = RuntimeDagProducerPublishIntentV1 {
        version: GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_VERSION_V1,
        checkpoint,
        previous_checkpoint_revision,
        staging_revision,
        block,
        head,
        index,
    };
    if runtime_dag_producer_block_path_from_intent(root, &intent)? != block_path {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer transaction block path is substituted",
        ));
    }
    validate_runtime_dag_producer_intent_bounds(root, &intent, &staged)?;
    let intent_payload = norito::to_bytes(&intent).map_err(|error| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG producer intent: {error}"
        ))
    })?;
    if intent_payload.is_empty()
        || intent_payload.len()
            > governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerPublishIntent,
            )
    {
        return Err(GovernancePublishError::other(
            "encoded governance runtime DAG producer intent exceeds its sealed byte limit",
        ));
    }
    let intent_record = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
        runtime_dag_producer_checkpoint_generation(&intent.checkpoint)?,
        intent_payload,
    );
    if store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)?
        .is_some()
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent is already active",
        ));
    }
    root_guard.revalidate()?;
    stage_runtime_dag_producer_transaction(root, root_guard, &intent, &staged)?;
    let staged_readback = load_runtime_dag_producer_staged_transaction(root, root_guard, &intent)?;
    if staged_readback != staged {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer staging readback diverged",
        ));
    }
    if store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)?
        .is_some()
    {
        return Err(GovernancePublishError::other(
            "sealed governance runtime DAG producer intent appeared during staging",
        ));
    }
    signer.assert_qualification()?;
    store.assert_qualification()?;
    root_guard.revalidate()?;
    store.compare_and_swap(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
        None,
        intent_record.clone(),
    )?;
    root_guard.revalidate()?;
    finish_runtime_dag_producer_intent(root, root_guard, signer, store, intent_record)?;
    signer.assert_qualification()?;
    store.assert_qualification()
}
fn runtime_dag_tip_from_entries(
    blocks: &[JsonValue],
) -> Result<Option<RuntimeDagTip>, GovernancePublishError> {
    let Some(last) = blocks.last() else {
        return Ok(None);
    };
    let Some(map) = last.as_object() else {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index block entry is not an object",
        ));
    };
    Ok(Some(RuntimeDagTip {
        sequence: required_runtime_u64(map, "sequence")?,
        block_cid: required_runtime_hex(map, "block_cid_hex")?,
        node_cid: required_runtime_hex(map, "node_cid_hex")?,
        timestamp: required_runtime_u64(map, "published_at_unix")?,
    }))
}
fn runtime_dag_checkpoint_cid(
    blocks: &[JsonValue],
    block_count: u64,
) -> Result<Option<Vec<u8>>, GovernancePublishError> {
    let window = u64::try_from(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("governance DAG checkpoint window fits u64");
    if block_count <= window {
        return Ok(None);
    }
    let checkpoint_sequence = block_count.checked_sub(window).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint block count is smaller than its window",
        )
    })?;
    let checkpoint_position = usize::try_from(checkpoint_sequence).map_err(|_| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint sequence exceeds host limits",
        )
    })?;
    let checkpoint_entry = blocks.get(checkpoint_position).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG index is missing the checkpoint window root",
        )
    })?;
    let checkpoint_map = checkpoint_entry.as_object().ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint index entry is not an object",
        )
    })?;
    if required_runtime_u64(checkpoint_map, "sequence")? != checkpoint_sequence {
        return Err(GovernancePublishError::other(
            "governance runtime DAG checkpoint sequence does not match its index position",
        ));
    }
    required_runtime_hex(checkpoint_map, "block_cid_hex").map(Some)
}
fn build_runtime_dag_index_bytes(
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    mut index: JsonMap,
    mut blocks: Vec<JsonValue>,
    head: &GovernanceDagHeadV1,
) -> Result<Vec<u8>, GovernancePublishError> {
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_source_payload_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut previous_block_cid_hex: Option<String> = None;
    let mut previous_node_cid_hex: Option<String> = None;
    for (position, block) in blocks.iter_mut().enumerate() {
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG index position exceeds u64")
        })?;
        let Some(block_map) = block.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block entry is not an object",
            ));
        };
        block_map.insert("position".into(), JsonValue::from(position_u64));
        let sequence = required_runtime_u64(block_map, "sequence")?;
        if sequence != position_u64 {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index sequence does not match block position",
            ));
        }
        let payload_kind = required_runtime_string(block_map, "payload_kind")?;
        append_runtime_index_position(&mut by_payload_kind, &payload_kind, position_u64);
        let encoded_blake3 = required_runtime_string(block_map, "encoded_blake3")?;
        append_runtime_index_position(&mut by_encoded_blake3, &encoded_blake3, position_u64);
        let source_payload_blake3 = required_runtime_string(block_map, "source_payload_blake3")?;
        append_runtime_index_position(
            &mut by_source_payload_blake3,
            &source_payload_blake3,
            position_u64,
        );
        let block_cid_hex = required_runtime_string(block_map, "block_cid_hex")?;
        let node_cid_hex = required_runtime_string(block_map, "node_cid_hex")?;
        let prev_block_cid_hex = optional_runtime_string(block_map, "prev_block_cid_hex")?;
        let prev_node_cid_hex = optional_runtime_string(block_map, "prev_node_cid_hex")?;
        if prev_block_cid_hex != previous_block_cid_hex
            || prev_node_cid_hex != previous_node_cid_hex
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index parent links are inconsistent",
            ));
        }
        previous_block_cid_hex = Some(block_cid_hex);
        previous_node_cid_hex = Some(node_cid_hex);
    }
    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(GOVERNANCE_DAG_LOGICAL_ROOT));
    index.insert("generated_at".into(), JsonValue::from(head.generated_at));
    insert_runtime_dag_signer_fields(&mut index, signer);
    insert_runtime_dag_checkpoint_store_fields(&mut index, store);
    index.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&head.head_block_cid)),
    );
    index.insert(
        "head_generated_at".into(),
        JsonValue::from(head.generated_at),
    );
    index.insert("block_count".into(), JsonValue::from(head.block_count));
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert(
        "by_source_payload_blake3".into(),
        JsonValue::Object(by_source_payload_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("blocks".into(), JsonValue::Array(blocks));
    let body = json::to_json_pretty(&JsonValue::Object(index)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance runtime DAG index: {err}"))
    })?;
    Ok(body.into_bytes())
}
fn append_runtime_index_position(index: &mut JsonMap, key: &str, position: u64) {
    let position = JsonValue::from(position);
    match index.get_mut(key).and_then(JsonValue::as_array_mut) {
        Some(positions) => positions.push(position),
        None => {
            index.insert(key.to_owned(), JsonValue::Array(vec![position]));
        }
    }
}
fn runtime_dag_index_entry_files_exist(root: &Path, entry: &JsonValue) -> bool {
    entry
        .get("block_path")
        .and_then(JsonValue::as_str)
        .and_then(|path| resolve_index_path(root, path).ok())
        .is_some_and(|path| path.is_file())
}
fn runtime_dag_block_path(root: &Path, sequence: u64, block_cid_hex: &str) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
        .join(format!("{sequence:020}_{block_cid_hex}.to"))
}
fn runtime_dag_head_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_HEAD_FILE)
}
fn fenced_privacy_head_cache_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_FILE)
}
fn fenced_privacy_head_sync_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_FILE)
}
fn fenced_privacy_pending_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_FENCED_PRIVACY_PENDING_FILE)
}
impl CachedPrivacyPublicationAuthorizationV1 {
    fn from_authorization(authorization: &PrivacyPublicationAuthorizationV1) -> Self {
        let lease = authorization.leader_lease();
        let scope = lease.scope();
        let window = scope.window();
        let provider = lease.provider_binding();
        let qualification = provider.qualification();
        let anchor = authorization.finalized_anchor();
        Self {
            lease_id: lease.lease_id(),
            scope_query_id: scope.query_id(),
            scope_cycle_id: scope.cycle_id(),
            scope_cycle_start_unix: window.cycle_start_unix,
            scope_cycle_end_unix: window.cycle_end_unix,
            scope_due_at_unix: window.due_at_unix,
            scope_holder_identity: scope.holder_identity(),
            lease_fencing_token: lease.fencing_token(),
            lease_issued_at_unix: lease.issued_at_unix(),
            lease_expires_at_unix: lease.expires_at_unix(),
            lease_provider_handle: provider.handle().to_owned(),
            lease_provider_revision: qualification.revision(),
            lease_provider_policy_digest: qualification.policy_digest(),
            finalized_anchor_query_id: anchor.query_id(),
            finalized_anchor_sequence: anchor.sequence(),
            finalized_anchor_release_id: anchor.release_id(),
            finalized_anchor_record_digest: anchor.record_digest(),
            finalized_anchor_latest_publication_block_hash: anchor.latest_publication_block_hash(),
            release_sequence: authorization.release_sequence(),
            release_record_digest: authorization.release_record_digest(),
            payload_digest: authorization.payload_digest(),
        }
    }
    fn reconstruct(&self) -> Result<PrivacyPublicationAuthorizationV1, GovernancePublishError> {
        let malformed = || {
            GovernancePublishError::other(
                "fenced privacy local retry cache contains a malformed authorization",
            )
        };
        let scope = crate::TransparencyLeaderLeaseScopeV1::try_new(
            self.scope_query_id,
            crate::PrivacyAggregateCycleWindow {
                cycle_start_unix: self.scope_cycle_start_unix,
                cycle_end_unix: self.scope_cycle_end_unix,
                due_at_unix: self.scope_due_at_unix,
            },
            self.scope_holder_identity,
        )
        .map_err(|_| malformed())?;
        if scope.cycle_id() != self.scope_cycle_id {
            return Err(malformed());
        }
        let provider = crate::TransparencyRuntimeProviderBindingV1::try_new(
            self.lease_provider_handle.clone(),
            self.lease_provider_revision,
            self.lease_provider_policy_digest,
        )
        .map_err(|_| malformed())?;
        let lease = crate::TransparencyLeaderLeaseGrantV1::try_new(
            self.lease_id,
            scope,
            self.lease_fencing_token,
            self.lease_issued_at_unix,
            self.lease_expires_at_unix,
            provider,
        )
        .map_err(|_| malformed())?;
        let anchor = crate::PrivacyReleaseAnchorHeadV1::try_from_parts(
            self.finalized_anchor_query_id,
            self.finalized_anchor_sequence,
            self.finalized_anchor_release_id,
            self.finalized_anchor_record_digest,
            self.finalized_anchor_latest_publication_block_hash,
        )
        .map_err(|_| malformed())?;
        PrivacyPublicationAuthorizationV1::try_from_cached_parts(
            lease,
            anchor,
            self.release_sequence,
            self.release_record_digest,
            self.payload_digest,
        )
        .map_err(|_| malformed())
    }
}
impl FencedPrivacyPendingRequestV1 {
    fn from_request(
        request: &FencedPrivacyPublicationRequestV1,
        publisher: &QualifiedFencedTransparencyPublisherV1,
    ) -> Result<Self, GovernancePublishError> {
        request
            .validate()
            .map_err(|error| GovernancePublishError::other(error.to_string()))?;
        let pending = Self {
            version: GOVERNANCE_FENCED_PRIVACY_PENDING_VERSION_V1,
            target_handle: publisher.handle().to_owned(),
            target_revision: publisher.qualification().revision,
            target_policy_digest: publisher.qualification().policy_digest,
            request_digest: request.request_digest(),
            authorization_digest: request.authorization_digest(),
            publication_idempotency_digest: request.publication_idempotency_digest(),
            authorization: CachedPrivacyPublicationAuthorizationV1::from_authorization(
                request.authorization(),
            ),
            payload_digest: request.payload_digest(),
            expected_authoritative_head: request.expected_authoritative_head(),
            fencing_token: request.fencing_token(),
            fencing_floor: request.fencing_floor(),
        };
        if !pending.has_valid_shape() {
            return Err(GovernancePublishError::other(
                "fenced privacy request produced a malformed pending journal record",
            ));
        }
        Ok(pending)
    }
    fn has_valid_shape(&self) -> bool {
        let predecessor_is_valid = match self.expected_authoritative_head {
            Some(head) => head.is_valid(),
            None => true,
        };
        let expected_floor = self
            .expected_authoritative_head
            .map_or(0, FencedTransparencyTargetHeadV1::fencing_floor);
        let authorization_is_valid = match self.authorization.reconstruct() {
            Ok(authorization) => {
                authorization.binding_digest() == self.authorization_digest
                    && authorization.publication_idempotency_digest()
                        == self.publication_idempotency_digest
                    && authorization.payload_digest() == self.payload_digest
                    && authorization.leader_lease().fencing_token() == self.fencing_token
            }
            Err(_) => false,
        };
        self.version == GOVERNANCE_FENCED_PRIVACY_PENDING_VERSION_V1
            && validate_production_runtime_handle(&self.target_handle).is_ok()
            && self.target_revision != 0
            && self.target_policy_digest != [0; 32]
            && predecessor_is_valid
            && authorization_is_valid
            && self.fencing_floor == expected_floor
            && self.fencing_token != 0
    }
    fn reconstruct_request(
        &self,
        incoming_authorization: &PrivacyPublicationAuthorizationV1,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        publisher: &QualifiedFencedTransparencyPublisherV1,
    ) -> Result<FencedPrivacyPublicationRequestV1, GovernancePublishError> {
        if self.target_handle != publisher.handle()
            || self.target_revision != publisher.qualification().revision
            || self.target_policy_digest != publisher.qualification().policy_digest
        {
            return Err(GovernancePublishError::other(
                "fenced privacy pending request belongs to a different qualified target",
            ));
        }
        if incoming_authorization.publication_idempotency_digest()
            != self.publication_idempotency_digest
        {
            return Err(GovernancePublishError::other(
                "fenced privacy pending request belongs to different release evidence",
            ));
        }
        if *blake3::hash(encoded).as_bytes() != self.payload_digest {
            return Err(GovernancePublishError::other(
                "fenced privacy pending request belongs to a different canonical payload",
            ));
        }
        let authorization = self.authorization.reconstruct()?;
        let request = FencedPrivacyPublicationRequestV1::try_new(
            authorization,
            publication,
            encoded.to_vec(),
            self.expected_authoritative_head,
            self.fencing_floor,
        )
        .map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy pending journal cannot reconstruct its exact request",
            )
        })?;
        if request.request_digest() != self.request_digest
            || request.authorization_digest() != self.authorization_digest
            || request.publication_idempotency_digest() != self.publication_idempotency_digest
            || request.payload_digest() != self.payload_digest
            || request.fencing_token() != self.fencing_token
        {
            return Err(GovernancePublishError::other(
                "fenced privacy pending journal does not match the exact request",
            ));
        }
        Ok(request)
    }
}
impl FencedPrivacyPublicationCacheV1 {
    fn from_verified_receipt(
        request: &FencedPrivacyPublicationRequestV1,
        receipt: &FencedPrivacyPublicationReceiptV1,
        authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    ) -> Result<Self, GovernancePublishError> {
        request
            .validate()
            .map_err(|error| GovernancePublishError::other(error.to_string()))?;
        receipt
            .validate_for_request(
                request,
                &receipt.provider_handle,
                receipt.provider_qualification,
            )
            .map_err(|error| GovernancePublishError::other(error.to_string()))?;
        let authoritative_head = authoritative_head.ok_or_else(|| {
            GovernancePublishError::other(
                "fenced privacy publication receipt cannot synchronize to genesis",
            )
        })?;
        if receipt.included_head().generation() > authoritative_head.generation()
            || receipt.included_head().fencing_floor() > authoritative_head.fencing_floor()
            || (receipt.included_head().generation() == authoritative_head.generation()
                && receipt.included_head() != authoritative_head)
            || receipt.readback_head().generation() > authoritative_head.generation()
            || receipt.readback_head().fencing_floor() > authoritative_head.fencing_floor()
            || (receipt.readback_head().generation() == authoritative_head.generation()
                && receipt.readback_head() != authoritative_head)
        {
            return Err(GovernancePublishError::other(
                "fenced privacy publication receipt cannot seed the local retry cache",
            ));
        }
        let cache = Self {
            version: GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_VERSION_V1,
            target_handle: receipt.provider_handle.clone(),
            target_revision: receipt.provider_qualification.revision,
            target_policy_digest: receipt.provider_qualification.policy_digest,
            authoritative_head: receipt.included_head(),
            last_request_digest: request.request_digest(),
            last_authorization_digest: request.authorization_digest(),
            last_publication_idempotency_digest: request.publication_idempotency_digest(),
            last_authorization: CachedPrivacyPublicationAuthorizationV1::from_authorization(
                request.authorization(),
            ),
            last_payload_digest: request.payload_digest(),
            last_expected_authoritative_head: request.expected_authoritative_head(),
            last_fencing_token: request.fencing_token(),
            last_fencing_floor: request.fencing_floor(),
            last_disposition: receipt.disposition(),
            last_included_head: receipt.included_head(),
        };
        let cache = Self {
            authoritative_head,
            ..cache
        };
        if !cache.has_valid_shape() {
            return Err(GovernancePublishError::other(
                "fenced privacy publication receipt produced a malformed local retry cache",
            ));
        }
        Ok(cache)
    }
    fn has_valid_shape(&self) -> bool {
        let expected_floor = self
            .last_expected_authoritative_head
            .map_or(0, |head| head.fencing_floor());
        let expected_generation = self
            .last_expected_authoritative_head
            .map_or(Some(1), |head| head.generation().checked_add(1));
        let predecessor_is_valid = match self.last_expected_authoritative_head {
            Some(head) => head.is_valid(),
            None => true,
        };
        let authorization_is_valid = match self.last_authorization.reconstruct() {
            Ok(authorization) => {
                authorization.binding_digest() == self.last_authorization_digest
                    && authorization.publication_idempotency_digest()
                        == self.last_publication_idempotency_digest
                    && authorization.payload_digest() == self.last_payload_digest
                    && authorization.leader_lease().fencing_token() == self.last_fencing_token
            }
            Err(_) => false,
        };
        let included_head_is_valid = self.last_included_head.is_valid()
            && self.last_included_head.generation() <= self.authoritative_head.generation()
            && self.last_included_head.fencing_floor() <= self.authoritative_head.fencing_floor()
            && (self.last_included_head.generation() != self.authoritative_head.generation()
                || self.last_included_head == self.authoritative_head);
        self.version == GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_VERSION_V1
            && validate_production_runtime_handle(&self.target_handle).is_ok()
            && self.target_revision != 0
            && self.target_policy_digest != [0; 32]
            && self.authoritative_head.is_valid()
            && predecessor_is_valid
            && authorization_is_valid
            && included_head_is_valid
            && match self.last_disposition {
                FencedPrivacyPublicationDispositionV1::Appended => {
                    expected_generation == Some(self.last_included_head.generation())
                        && self.last_included_head.fencing_floor() == self.last_fencing_token
                        && self.last_fencing_token > self.last_fencing_floor
                }
                FencedPrivacyPublicationDispositionV1::AlreadyIncluded => {
                    self.last_fencing_token != 0
                }
            }
            && self.last_fencing_floor == expected_floor
    }
    fn exact_retry_request(
        &self,
        incoming_authorization: &PrivacyPublicationAuthorizationV1,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
    ) -> Result<Option<FencedPrivacyPublicationRequestV1>, GovernancePublishError> {
        let incoming_payload_digest = *blake3::hash(encoded).as_bytes();
        if incoming_payload_digest != self.last_payload_digest
            || incoming_authorization.publication_idempotency_digest()
                != self.last_publication_idempotency_digest
        {
            return Ok(None);
        }
        let authorization = self.last_authorization.reconstruct()?;
        let request = FencedPrivacyPublicationRequestV1::try_new(
            authorization,
            publication,
            encoded.to_vec(),
            self.last_expected_authoritative_head,
            self.last_fencing_floor,
        )
        .map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy local retry cache cannot reconstruct its exact request",
            )
        })?;
        let included_head_mismatch = match self.last_disposition {
            FencedPrivacyPublicationDispositionV1::Appended => {
                request.expected_successor_head().map_err(|_| {
                    GovernancePublishError::other(
                        "fenced privacy local retry cache cannot reconstruct its successor",
                    )
                })? != self.last_included_head
            }
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded => false,
        };
        if request.request_digest() != self.last_request_digest
            || request.publication_idempotency_digest() != self.last_publication_idempotency_digest
            || included_head_mismatch
        {
            return Err(GovernancePublishError::other(
                "fenced privacy local retry cache does not match the exact request",
            ));
        }
        Ok(Some(request))
    }
}
impl FencedPrivacyAuthoritativeHeadSyncV1 {
    fn from_authenticated_read(
        reader: &QualifiedFencedTransparencyHeadReaderV1,
        proof: &FencedTransparencyHeadAncestryProofV1,
    ) -> Result<Self, GovernancePublishError> {
        let sync = Self {
            version: GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1,
            reader_handle: reader.handle().to_owned(),
            reader_revision: reader.qualification().revision,
            reader_policy_digest: reader.qualification().policy_digest,
            authoritative_head: proof.authoritative_head(),
            ancestry_proof_digest: proof.adapter_proof_digest(),
        };
        if !sync.has_valid_shape() {
            return Err(GovernancePublishError::other(
                "authenticated fenced privacy head readback produced a malformed cache record",
            ));
        }
        Ok(sync)
    }
    fn has_valid_shape(&self) -> bool {
        self.version == GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1
            && validate_production_runtime_handle(&self.reader_handle).is_ok()
            && self.reader_revision != 0
            && self.reader_policy_digest.iter().any(|byte| *byte != 0)
            && self.ancestry_proof_digest != [0; 32]
            && self
                .authoritative_head
                .is_none_or(FencedTransparencyTargetHeadV1::is_valid)
    }
}
fn reject_legacy_fenced_privacy_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    reject_legacy_atomic_state_names(
        root_guard.rooted_directory(),
        &[
            GOVERNANCE_FENCED_PRIVACY_PENDING_FILE,
            GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_FILE,
            GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_FILE,
        ],
        "fenced privacy authority",
    )?;
    for path in [
        fenced_privacy_pending_path(root),
        fenced_privacy_head_cache_path(root),
        fenced_privacy_head_sync_path(root),
    ] {
        match read_rooted_governance_state_file(root_guard, &path, 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy mutable fenced privacy state `{}` is unsupported; remove it before first-release initialization",
                    path.display()
                )));
            }
        }
    }
    root_guard.revalidate()?;
    Ok(())
}
fn validate_fenced_privacy_state_v1(
    state: &FencedPrivacyStateV1,
) -> Result<(), GovernancePublishError> {
    if state.version != GOVERNANCE_FENCED_PRIVACY_STATE_VERSION_V1
        || state
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.has_valid_shape())
        || state
            .publication_cache
            .as_ref()
            .is_some_and(|cache| !cache.has_valid_shape())
        || state
            .authoritative_head_sync
            .as_ref()
            .is_some_and(|sync| !sync.has_valid_shape())
    {
        return Err(GovernancePublishError::other(
            "combined fenced privacy state is malformed",
        ));
    }
    if state.pending.as_ref().is_some_and(|pending| {
        state.publication_cache.as_ref().is_some_and(|cache| {
            pending.target_handle != cache.target_handle
                || pending.target_revision != cache.target_revision
                || pending.target_policy_digest != cache.target_policy_digest
        }) || state.authoritative_head_sync.as_ref().is_some_and(|sync| {
            pending.target_handle != sync.reader_handle
                || pending.target_revision != sync.reader_revision
                || pending.target_policy_digest != sync.reader_policy_digest
        })
    }) || state.publication_cache.as_ref().is_some_and(|cache| {
        state.authoritative_head_sync.as_ref().is_some_and(|sync| {
            cache.target_handle != sync.reader_handle
                || cache.target_revision != sync.reader_revision
                || cache.target_policy_digest != sync.reader_policy_digest
        })
    }) {
        return Err(GovernancePublishError::other(
            "combined fenced privacy records belong to different qualified targets",
        ));
    }
    Ok(())
}
fn open_fenced_privacy_store_v1(
    root: &Path,
) -> Result<governance_rooted_fs::TwoSlotStoreV1, GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    reject_legacy_fenced_privacy_state(root, &root_guard)?;
    let initial = encode_governance_two_slot_value_v1(
        &FencedPrivacyStateV1 {
            version: GOVERNANCE_FENCED_PRIVACY_STATE_VERSION_V1,
            pending: None,
            publication_cache: None,
            authoritative_head_sync: None,
        },
        "initial combined fenced privacy state",
    )?;
    open_governance_two_slot_store_v1(
        &root_guard,
        GOVERNANCE_FENCED_PRIVACY_STORE_SPEC_V1,
        &initial,
    )
}
fn load_fenced_privacy_state_v1(
    store: &governance_rooted_fs::TwoSlotStoreV1,
) -> Result<
    (
        FencedPrivacyStateV1,
        governance_rooted_fs::TwoSlotSnapshotV1,
    ),
    GovernancePublishError,
> {
    let snapshot = load_governance_two_slot_store_v1(store, "combined fenced privacy state")?;
    let state: FencedPrivacyStateV1 =
        decode_governance_two_slot_value_v1(&snapshot, "combined fenced privacy state")?;
    validate_fenced_privacy_state_v1(&state)?;
    Ok((state, snapshot))
}
fn update_fenced_privacy_state_v1(
    root: &Path,
    label: &str,
    update: impl FnOnce(&mut FencedPrivacyStateV1),
) -> Result<(), GovernancePublishError> {
    let store = open_fenced_privacy_store_v1(root)?;
    let (mut state, snapshot) = load_fenced_privacy_state_v1(&store)?;
    let predecessor = state.clone();
    update(&mut state);
    validate_fenced_privacy_state_v1(&state)?;
    if state == predecessor {
        return Ok(());
    }
    let bytes = encode_governance_two_slot_value_v1(&state, label)?;
    if bytes.len() > GOVERNANCE_FENCED_PRIVACY_STORE_SPEC_V1.max_payload_bytes {
        return Err(GovernancePublishError::other(
            "combined fenced privacy state exceeds its byte limit",
        ));
    }
    let committed =
        compare_and_swap_governance_two_slot_store_v1(&store, &snapshot, &bytes, label)?;
    let readback: FencedPrivacyStateV1 =
        decode_governance_two_slot_value_v1(&committed, "combined fenced privacy readback")?;
    if readback != state {
        return Err(GovernancePublishError::other(
            "combined fenced privacy state readback diverged",
        ));
    }
    Ok(())
}
fn read_fenced_privacy_pending_request(
    root: &Path,
) -> Result<Option<FencedPrivacyPendingRequestV1>, GovernancePublishError> {
    let store = open_fenced_privacy_store_v1(root)?;
    Ok(load_fenced_privacy_state_v1(&store)?.0.pending)
}
fn write_fenced_privacy_pending_request(
    root: &Path,
    pending: &FencedPrivacyPendingRequestV1,
) -> Result<(), GovernancePublishError> {
    if !pending.has_valid_shape() {
        return Err(GovernancePublishError::other(
            "fenced privacy pending request is malformed",
        ));
    }
    update_fenced_privacy_state_v1(root, "fenced privacy pending request", |state| {
        state.pending = Some(pending.clone());
    })
}
fn remove_fenced_privacy_pending_request(root: &Path) -> Result<(), GovernancePublishError> {
    update_fenced_privacy_state_v1(root, "cleared fenced privacy pending request", |state| {
        state.pending = None;
    })
}
fn read_fenced_privacy_head_cache(
    root: &Path,
) -> Result<Option<FencedPrivacyPublicationCacheV1>, GovernancePublishError> {
    let store = open_fenced_privacy_store_v1(root)?;
    Ok(load_fenced_privacy_state_v1(&store)?.0.publication_cache)
}
fn write_fenced_privacy_head_cache(
    root: &Path,
    cache: &FencedPrivacyPublicationCacheV1,
) -> Result<(), GovernancePublishError> {
    if !cache.has_valid_shape() {
        return Err(GovernancePublishError::other(
            "fenced privacy authoritative successor cache is malformed",
        ));
    }
    update_fenced_privacy_state_v1(root, "fenced privacy publication cache", |state| {
        state.publication_cache = Some(cache.clone());
    })
}
fn read_fenced_privacy_head_sync(
    root: &Path,
) -> Result<Option<FencedPrivacyAuthoritativeHeadSyncV1>, GovernancePublishError> {
    let store = open_fenced_privacy_store_v1(root)?;
    Ok(load_fenced_privacy_state_v1(&store)?
        .0
        .authoritative_head_sync)
}
fn write_fenced_privacy_head_sync(
    root: &Path,
    sync: &FencedPrivacyAuthoritativeHeadSyncV1,
) -> Result<(), GovernancePublishError> {
    if !sync.has_valid_shape() {
        return Err(GovernancePublishError::other(
            "fenced privacy authenticated head cache is malformed",
        ));
    }
    update_fenced_privacy_state_v1(root, "fenced privacy authenticated head sync", |state| {
        state.authoritative_head_sync = Some(sync.clone());
    })
}
fn retain_fenced_privacy_required_ancestor(
    required: &mut Vec<FencedTransparencyTargetHeadV1>,
    ancestor: Option<FencedTransparencyTargetHeadV1>,
) {
    if let Some(ancestor) = ancestor.filter(|candidate| !required.contains(candidate)) {
        required.push(ancestor);
    }
}
fn retain_fenced_privacy_required_publication(
    required: &mut Vec<FencedTransparencyPublicationInclusionV1>,
    publication: FencedTransparencyPublicationInclusionV1,
) {
    if !required.contains(&publication) {
        required.push(publication);
    }
}
/// Reauthenticate the persisted privacy head/cache against the exact target.
///
/// The caller must hold the shared filesystem-publisher transaction fence so
/// publication and preflight synchronization cannot race.
pub(crate) fn synchronize_fenced_privacy_authoritative_head(
    root: &Path,
    reader: &QualifiedFencedTransparencyHeadReaderV1,
    receipt: Option<&FencedPrivacyPublicationReceiptV1>,
) -> Result<Option<FencedTransparencyTargetHeadV1>, GovernancePublishError> {
    let publication_cache = read_fenced_privacy_head_cache(root)?;
    let prior_sync = read_fenced_privacy_head_sync(root)?;
    if prior_sync.as_ref().is_some_and(|sync| {
        sync.reader_handle != reader.handle()
            || sync.reader_revision != reader.qualification().revision
            || sync.reader_policy_digest != reader.qualification().policy_digest
    }) {
        return Err(GovernancePublishError::other(
            "persisted fenced privacy authoritative-head state belongs to a different qualified reader",
        ));
    }
    let mut required_ancestors = Vec::new();
    let mut required_publications = Vec::new();
    if let Some(cache) = publication_cache {
        if cache.target_handle != reader.handle()
            || cache.target_revision != reader.qualification().revision
            || cache.target_policy_digest != reader.qualification().policy_digest
        {
            return Err(GovernancePublishError::other(
                "persisted fenced privacy publication cache belongs to a different qualified target",
            ));
        }
        retain_fenced_privacy_required_ancestor(
            &mut required_ancestors,
            Some(cache.authoritative_head),
        );
        retain_fenced_privacy_required_ancestor(
            &mut required_ancestors,
            Some(cache.last_included_head),
        );
        let publication = FencedTransparencyPublicationInclusionV1::try_new(
            cache.last_publication_idempotency_digest,
            cache.last_payload_digest,
            cache.last_included_head,
        )
        .map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy publication cache contains malformed inclusion evidence",
            )
        })?;
        retain_fenced_privacy_required_publication(&mut required_publications, publication);
    }
    retain_fenced_privacy_required_ancestor(
        &mut required_ancestors,
        prior_sync.as_ref().and_then(|sync| sync.authoritative_head),
    );
    if let Some(receipt) = receipt {
        retain_fenced_privacy_required_ancestor(
            &mut required_ancestors,
            Some(receipt.included_head()),
        );
        retain_fenced_privacy_required_ancestor(
            &mut required_ancestors,
            Some(receipt.readback_head()),
        );
        let publication = FencedTransparencyPublicationInclusionV1::try_new(
            receipt.publication_idempotency_digest(),
            receipt.payload_digest(),
            receipt.included_head(),
        )
        .map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy receipt contains malformed publication inclusion evidence",
            )
        })?;
        retain_fenced_privacy_required_publication(&mut required_publications, publication);
    }
    let proof = reader
        .read_authoritative_head_with_ancestry(&required_ancestors, &required_publications)?;
    let authoritative_head = proof.authoritative_head();
    let next_sync = FencedPrivacyAuthoritativeHeadSyncV1::from_authenticated_read(reader, &proof)?;
    if prior_sync.as_ref() != Some(&next_sync) {
        write_fenced_privacy_head_sync(root, &next_sync)?;
    }
    Ok(authoritative_head)
}
fn empty_governance_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}
fn required_runtime_string(map: &JsonMap, field: &str) -> Result<String, GovernancePublishError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance runtime DAG index entry is missing `{field}`"
            ))
        })
}
fn optional_runtime_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernancePublishError> {
    match map.get(field) {
        Some(JsonValue::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "governance runtime DAG index entry field `{field}` is not a string or null"
                ))
            }),
    }
}
fn required_optional_runtime_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernancePublishError> {
    let value = map.get(field).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry is missing `{field}`"
        ))
    })?;
    match value {
        JsonValue::Null => Ok(None),
        value => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "governance runtime DAG index entry field `{field}` is not a string or null"
                ))
            }),
    }
}
fn json_optional_string_matches(value: Option<&JsonValue>, expected: Option<&str>) -> bool {
    match (value, expected) {
        (Some(JsonValue::Null), None) => true,
        (Some(value), Some(expected)) => value.as_str() == Some(expected),
        _ => false,
    }
}
fn required_runtime_u64(map: &JsonMap, field: &str) -> Result<u64, GovernancePublishError> {
    map.get(field).and_then(JsonValue::as_u64).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry is missing `{field}`"
        ))
    })
}
fn required_runtime_hex(map: &JsonMap, field: &str) -> Result<Vec<u8>, GovernancePublishError> {
    let value = required_runtime_string(map, field)?;
    if value.is_empty() {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is empty"
        )));
    }
    hex::decode(&value).map_err(|err| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is not hex: {err}"
        ))
    })
}
fn record_governance_dag_publish_result(
    payload_kind: &str,
    result: &Result<(), GovernancePublishError>,
    encoded_len: usize,
) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    let result_label = if result.is_ok() { "success" } else { "failure" };
    let encoded_len = u64::try_from(encoded_len).unwrap_or(u64::MAX);
    metrics.record_sorafs_governance_dag_publish(
        payload_kind,
        result_label,
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        encoded_len,
        current_unix_timestamp_seconds(),
    );
}
fn record_governance_dag_backlog(pending_count: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_backlog(GOVERNANCE_DAG_SINK_FILESYSTEM, pending_count);
}
fn record_governance_dag_head_age_from_index(index: &JsonMap) {
    if let Some(generated_at) = governance_dag_head_generated_at_from_index(index) {
        record_governance_dag_head_age(generated_at);
    }
}
fn governance_dag_head_generated_at_from_index(index: &JsonMap) -> Option<u64> {
    index
        .get("head_generated_at")
        .and_then(JsonValue::as_u64)
        .or_else(|| index.get("generated_at").and_then(JsonValue::as_u64))
}
fn record_governance_dag_head_age(generated_at: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_head_age_seconds(
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        governance_dag_head_age_seconds(generated_at, current_unix_timestamp_seconds()),
    );
}
fn governance_dag_head_age_seconds(generated_at: u64, now: u64) -> u64 {
    now.saturating_sub(generated_at)
}
fn ensure_canonical_governance_encoding<T: norito::NoritoSerialize>(
    value: &T,
    encoded: &[u8],
    payload_kind: &'static str,
) -> Result<(), GovernancePublishError> {
    let canonical = norito::to_bytes(value).map_err(|err| {
        GovernancePublishError::other(format!(
            "failed to canonically encode {payload_kind} before publication: {err}"
        ))
    })?;
    if canonical != encoded {
        return Err(GovernancePublishError::other(format!(
            "{payload_kind} publication bytes do not match the canonical header-bearing Norito payload"
        )));
    }
    Ok(())
}
fn bind_authenticated_submission_labels(
    labels: &mut JsonMap,
    provenance: Option<&GovernanceSubmissionProvenanceV1>,
) {
    let Some(provenance) = provenance else {
        return;
    };
    let signed_provenance = provenance.to_dag_provenance();
    labels.insert(
        "authenticated_publisher_account_digest_hex".into(),
        JsonValue::from(hex::encode(signed_provenance.publisher_account_digest)),
    );
    labels.insert(
        "authenticated_publisher_origin".into(),
        JsonValue::from(provenance.origin().label()),
    );
}
fn bind_authenticated_submission_json(
    json_body: String,
    provenance: Option<&GovernanceSubmissionProvenanceV1>,
) -> Result<String, GovernancePublishError> {
    let Some(provenance) = provenance else {
        return Ok(json_body);
    };
    let value: JsonValue = json::from_slice(json_body.as_bytes()).map_err(|error| {
        GovernancePublishError::other(format!(
            "decode governance JSON before binding authenticated provenance: {error}"
        ))
    })?;
    let JsonValue::Object(mut root) = value else {
        return Err(GovernancePublishError::other(
            "governance JSON root is not an object",
        ));
    };
    let metadata = root
        .get_mut("metadata")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| GovernancePublishError::other("governance JSON metadata is missing"))?;
    let signed_provenance = provenance.to_dag_provenance();
    metadata.insert(
        "authenticated_publisher_account_digest_hex".into(),
        JsonValue::from(hex::encode(signed_provenance.publisher_account_digest)),
    );
    metadata.insert(
        "authenticated_publisher_origin".into(),
        JsonValue::from(provenance.origin().label()),
    );
    json::to_json_pretty(&JsonValue::Object(root)).map_err(|error| {
        GovernancePublishError::other(format!(
            "serialize governance JSON with authenticated provenance: {error}"
        ))
    })
}
#[cfg(test)]
impl FilesystemGovernancePublisher {
    fn publish_transparency_ledger_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        authorization: Option<&PrivacyPublicationAuthorizationV1>,
    ) -> Result<(), GovernancePublishError> {
        let provenance = test_submission_provenance(
            crate::GovernanceSubmissionOriginV1::PrivacyAggregatePublishDue,
        );
        <Self as GovernancePublisher>::publish_transparency_ledger_publication(
            self,
            publication,
            encoded,
            authorization,
            Some(&provenance),
        )
    }
    fn publish_proof_token_issuance(
        &self,
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let provenance = test_submission_provenance(
            crate::GovernanceSubmissionOriginV1::TransparencyTokenIssuance,
        );
        <Self as GovernancePublisher>::publish_proof_token_issuance(
            self,
            issuance,
            encoded,
            Some(&provenance),
        )
    }
    fn publish_appeal_finance_report(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let provenance =
            test_submission_provenance(crate::GovernanceSubmissionOriginV1::AppealFinanceReport);
        <Self as GovernancePublisher>::publish_appeal_finance_report(
            self,
            report,
            encoded,
            &provenance,
        )
    }
    fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let provenance = test_submission_provenance(
            crate::GovernanceSubmissionOriginV1::AppealFinanceWeeklyRollup,
        );
        <Self as GovernancePublisher>::publish_appeal_finance_weekly_rollup(
            self,
            rollup,
            encoded,
            &provenance,
        )
    }
}
#[cfg(test)]
fn test_submission_provenance(
    origin: crate::GovernanceSubmissionOriginV1,
) -> GovernanceSubmissionProvenanceV1 {
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &[0xA7; 32])
        .expect("fixed test publisher key must be valid");
    GovernanceSubmissionProvenanceV1::new(AccountId::new(public_key), origin)
}
impl GovernancePublisher for FilesystemGovernancePublisher {
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(settlement, encoded, "deal settlement")?;
            settlement.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid deal settlement: {err}"))
            })?;
            let runtime_payload =
                GovernanceLogPayloadV1::DealSettlement(Box::new(settlement.clone()));
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let mut settlement_obj = JsonMap::new();
            settlement_obj.insert("version".into(), JsonValue::from(settlement.version as u64));
            settlement_obj.insert(
                "deal_id".into(),
                JsonValue::from(settlement.deal_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "settlement_id".into(),
                JsonValue::from(settlement.settlement_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "ledger_snapshot_id".into(),
                JsonValue::from(settlement.ledger.snapshot_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "ledger_sequence".into(),
                JsonValue::from(settlement.ledger.sequence),
            );
            settlement_obj.insert(
                "provider_id".into(),
                JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "client_id".into(),
                JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            settlement_obj.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
            settlement_obj.insert(
                "ledger_captured_at".into(),
                JsonValue::from(settlement.ledger.captured_at),
            );
            settlement_obj.insert(
                "window_start_epoch".into(),
                JsonValue::from(settlement.ledger.window_start_epoch),
            );
            settlement_obj.insert(
                "window_end_epoch".into(),
                JsonValue::from(settlement.ledger.window_end_epoch),
            );
            settlement_obj.insert(
                "settlement_window_epochs".into(),
                JsonValue::from(settlement.ledger.settlement_window_epochs),
            );
            settlement_obj.insert(
                "provider_accrual".into(),
                JsonValue::from(settlement.ledger.provider_accrual.to_string()),
            );
            settlement_obj.insert(
                "client_liability".into(),
                JsonValue::from(settlement.ledger.client_liability.to_string()),
            );
            settlement_obj.insert(
                "outstanding_liability".into(),
                JsonValue::from(settlement.ledger.outstanding_liability.to_string()),
            );
            settlement_obj.insert(
                "bond_total".into(),
                JsonValue::from(settlement.ledger.bond_total.to_string()),
            );
            settlement_obj.insert(
                "bond_locked".into(),
                JsonValue::from(settlement.ledger.bond_locked.to_string()),
            );
            settlement_obj.insert(
                "bond_slashed".into(),
                JsonValue::from(settlement.ledger.bond_slashed.to_string()),
            );
            settlement_obj.insert(
                "bond_released".into(),
                JsonValue::from(settlement.ledger.bond_released.to_string()),
            );
            if let Some(notes) = &settlement.audit_notes {
                settlement_obj.insert("audit_notes".into(), JsonValue::from(notes.clone()));
            }
            let mut payload = JsonMap::new();
            payload.insert("settlement".into(), JsonValue::Object(settlement_obj));
            let mut metadata = JsonMap::new();
            metadata.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            payload.insert("metadata".into(), JsonValue::Object(metadata));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!("serialize settlement json: {err}"))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "deal_id".into(),
                JsonValue::from(settlement.deal_id.encode_hex::<String>()),
            );
            labels.insert(
                "provider_id".into(),
                JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
            );
            labels.insert(
                "client_id".into(),
                JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
            );
            labels.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            labels.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
            let (encoded_path, json_path) = self.record_publish_index(
                "deal_settlement",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "deal_settlement",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("deal_settlement", &result, encoded.len());
        result
    }
    fn publish_pdp_archive(
        &self,
        archive: &PdpGovernanceArchiveV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(archive, encoded, "PDP governance archive")?;
            archive.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PDP governance archive: {error}"))
            })?;
            let runtime_payload = GovernanceLogPayloadV1::PdpArchive(archive.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let decision = pdp_decision_label(archive.decision);
            let mut payload = JsonMap::new();
            payload.insert(
                "version".into(),
                JsonValue::from(u64::from(archive.version)),
            );
            payload.insert("sequence".into(), JsonValue::from(archive.sequence));
            payload.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(archive.challenge_id)),
            );
            payload.insert(
                "commitment_digest_hex".into(),
                JsonValue::from(hex::encode(archive.commitment_digest)),
            );
            payload.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(archive.manifest_digest)),
            );
            payload.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(archive.provider_id)),
            );
            payload.insert("epoch_id".into(), JsonValue::from(archive.epoch_id));
            payload.insert("decision".into(), JsonValue::from(decision));
            payload.insert(
                "proof_digest_hex".into(),
                archive
                    .proof_digest
                    .map(hex::encode)
                    .map_or(JsonValue::Null, JsonValue::from),
            );
            payload.insert(
                "sampled_segments".into(),
                JsonValue::from(u64::from(archive.sampled_segments)),
            );
            payload.insert(
                "sampled_hot_leaves".into(),
                JsonValue::from(u64::from(archive.sampled_hot_leaves)),
            );
            payload.insert(
                "sampled_bytes".into(),
                JsonValue::from(archive.sampled_bytes),
            );
            payload.insert(
                "issued_at_unix".into(),
                JsonValue::from(archive.issued_at_unix),
            );
            payload.insert(
                "response_deadline_unix".into(),
                JsonValue::from(archive.response_deadline_unix),
            );
            payload.insert(
                "decided_at_unix".into(),
                JsonValue::from(archive.decided_at_unix),
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!("serialize PDP archive json: {error}"))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(archive.challenge_id)),
            );
            labels.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(archive.manifest_digest)),
            );
            labels.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(archive.provider_id)),
            );
            labels.insert("epoch_id".into(), JsonValue::from(archive.epoch_id));
            labels.insert("decision".into(), JsonValue::from(decision));
            labels.insert("sequence".into(), JsonValue::from(archive.sequence));
            let (encoded_path, json_path) =
                self.record_publish_index("pdp_archive", encoded, json_body.as_bytes(), labels)?;
            self.record_runtime_signed_payload(
                "pdp_archive",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("pdp_archive", &result, encoded.len());
        result
    }
    fn publish_por_challenge_publication(
        &self,
        publication: &PorChallengePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                publication,
                encoded,
                "PoR challenge publication",
            )?;
            publication.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PoR challenge publication: {error}"))
            })?;
            let runtime_payload =
                GovernanceLogPayloadV1::PorChallengePublication(publication.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let challenge = &publication.challenge;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let mut payload = JsonMap::new();
            payload.insert(
                "publication".into(),
                json::to_value(publication).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "serialize PoR challenge publication json: {error}"
                    ))
                })?,
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!(
                    "serialize PoR challenge publication json: {error}"
                ))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(challenge.challenge_id)),
            );
            labels.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(challenge.manifest_digest)),
            );
            labels.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(challenge.provider_id)),
            );
            labels.insert("epoch_id".into(), JsonValue::from(challenge.epoch_id));
            labels.insert(
                "duplicate_samples".into(),
                JsonValue::from(u64::from(publication.duplicate_samples)),
            );
            labels.insert("forced".into(), JsonValue::from(challenge.forced));
            let (encoded_path, json_path) = self.record_publish_index(
                "por_challenge_publication",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "por_challenge_publication",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("por_challenge_publication", &result, encoded.len());
        result
    }
    fn publish_por_weekly_report(
        &self,
        report: &PorWeeklyReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "PoR weekly report")?;
            report.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PoR weekly report: {error}"))
            })?;
            let runtime_payload = GovernanceLogPayloadV1::PorWeeklyReport(report.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let mut payload = JsonMap::new();
            payload.insert(
                "report".into(),
                json::to_value(report).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "serialize PoR weekly report json: {error}"
                    ))
                })?,
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!("serialize PoR weekly report json: {error}"))
            })?;
            let mut labels = JsonMap::new();
            labels.insert("cycle".into(), JsonValue::from(report.cycle.to_string()));
            labels.insert("generated_at".into(), JsonValue::from(report.generated_at));
            labels.insert(
                "challenges_total".into(),
                JsonValue::from(u64::from(report.challenges_total)),
            );
            labels.insert(
                "challenges_failed".into(),
                JsonValue::from(u64::from(report.challenges_failed)),
            );
            labels.insert(
                "forced_challenges".into(),
                JsonValue::from(u64::from(report.forced_challenges)),
            );
            let (encoded_path, json_path) = self.record_publish_index(
                "por_weekly_report",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "por_weekly_report",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("por_weekly_report", &result, encoded.len());
        result
    }
    fn publish_gc_audit_event(
        &self,
        event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(event, encoded, "GC audit event")?;
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid GC audit event: {err}"))
            })?;
            let external = GovernanceExternalPayloadV1::from_gc_audit(event, encoded)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            let runtime_payload = GovernanceLogPayloadV1::ExternalPayload(external);
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let mut payload = JsonMap::new();
            payload.insert(
                "event".into(),
                json::to_value(event).map_err(|err| {
                    GovernancePublishError::other(format!("serialize gc event: {err}"))
                })?,
            );
            let mut metadata = JsonMap::new();
            metadata.insert(
                "reason".into(),
                JsonValue::from(event.payload.reason.clone()),
            );
            if let Some(blocked) = &event.payload.blocked_reason {
                metadata.insert("blocked_reason".into(), JsonValue::from(blocked.clone()));
            }
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            payload.insert("metadata".into(), JsonValue::Object(metadata));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!("serialize gc audit json: {err}"))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "manifest".into(),
                JsonValue::from(hex::encode(event.payload.manifest_digest)),
            );
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(event.payload.provider_id)),
            );
            labels.insert(
                "reason".into(),
                JsonValue::from(event.payload.reason.clone()),
            );
            labels.insert("sequence".into(), JsonValue::from(event.header.sequence));
            labels.insert(
                "evicted_at_unix".into(),
                JsonValue::from(event.payload.evicted_at_unix),
            );
            let (encoded_path, json_path) =
                self.record_publish_index("gc_audit", encoded, json_body.as_bytes(), labels)?;
            self.record_runtime_signed_payload(
                "gc_audit",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("gc_audit", &result, encoded.len());
        result
    }
    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "reconciliation report")?;
            report.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid reconciliation report: {err}"))
            })?;
            let external = GovernanceExternalPayloadV1::from_reconciliation(report, encoded)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            let runtime_payload = GovernanceLogPayloadV1::ExternalPayload(external);
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let mut payload = JsonMap::new();
            payload.insert(
                "report".into(),
                json::to_value(report).map_err(|err| {
                    GovernancePublishError::other(format!("serialize reconciliation report: {err}"))
                })?,
            );
            let mut metadata = JsonMap::new();
            metadata.insert(
                "provider".into(),
                JsonValue::from(hex::encode(report.provider_id)),
            );
            metadata.insert(
                "generated_at_unix".into(),
                JsonValue::from(report.generated_at_unix),
            );
            metadata.insert(
                "repair_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.repair_snapshot_hash)),
            );
            metadata.insert(
                "retention_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.retention_snapshot_hash)),
            );
            metadata.insert(
                "gc_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.gc_snapshot_hash)),
            );
            metadata.insert(
                "divergence_count".into(),
                JsonValue::from(report.divergence_count as u64),
            );
            if let Some(appeal_finance) = &report.appeal_finance {
                metadata.insert(
                    "appeal_finance_rollup_snapshot_hash".into(),
                    JsonValue::from(hex::encode(appeal_finance.rollup_snapshot_hash)),
                );
                metadata.insert(
                    "appeal_finance_rollup_count".into(),
                    JsonValue::from(u64::from(appeal_finance.rollup_count)),
                );
                metadata.insert(
                    "appeal_finance_source_report_count".into(),
                    JsonValue::from(appeal_finance.source_report_count),
                );
                metadata.insert(
                    "appeal_finance_case_count".into(),
                    JsonValue::from(appeal_finance.case_count),
                );
                metadata.insert(
                    "appeal_finance_total_treasury_xor".into(),
                    JsonValue::from(appeal_finance.total_treasury_xor.to_string()),
                );
                metadata.insert(
                    "appeal_finance_total_rewards_forfeited_treasury_xor".into(),
                    JsonValue::from(
                        appeal_finance
                            .total_rewards_forfeited_treasury_xor
                            .to_string(),
                    ),
                );
            }
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            payload.insert("metadata".into(), JsonValue::Object(metadata));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!(
                    "serialize reconciliation report json: {err}"
                ))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(report.provider_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(report.generated_at_unix),
            );
            labels.insert(
                "divergence_count".into(),
                JsonValue::from(report.divergence_count as u64),
            );
            if let Some(appeal_finance) = &report.appeal_finance {
                labels.insert(
                    "appeal_finance_rollup_count".into(),
                    JsonValue::from(u64::from(appeal_finance.rollup_count)),
                );
                labels.insert(
                    "appeal_finance_source_report_count".into(),
                    JsonValue::from(appeal_finance.source_report_count),
                );
                labels.insert(
                    "appeal_finance_total_treasury_xor".into(),
                    JsonValue::from(appeal_finance.total_treasury_xor.to_string()),
                );
            }
            let (encoded_path, json_path) =
                self.record_publish_index("reconciliation", encoded, json_body.as_bytes(), labels)?;
            self.record_runtime_signed_payload(
                "reconciliation",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("reconciliation", &result, encoded.len());
        result
    }
    fn publish_reputation_snapshot(
        &self,
        envelope: &SignedReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            let canonical = envelope.canonical_bytes().map_err(|err| {
                GovernancePublishError::other(format!("invalid signed reputation snapshot: {err}"))
            })?;
            if canonical != encoded {
                return Err(GovernancePublishError::other(
                    "signed reputation snapshot bytes are not canonical",
                ));
            }
            let runtime_payload =
                GovernanceLogPayloadV1::SignedReputationSnapshot(envelope.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let snapshot = &envelope.snapshot;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = reputation_snapshot_json(envelope, encoded.len(), &digest_hex)?;
            let mut labels = JsonMap::new();
            labels.insert(
                "snapshot_id_hex".into(),
                JsonValue::from(hex::encode(snapshot.snapshot_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(snapshot.generated_at_unix),
            );
            labels.insert(
                "provider_count".into(),
                JsonValue::from(snapshot.providers.len() as u64),
            );
            labels.insert(
                "merkle_root_hex".into(),
                JsonValue::from(hex::encode(snapshot.merkle_root)),
            );
            labels.insert(
                "policy_digest_hex".into(),
                JsonValue::from(hex::encode(envelope.policy_digest)),
            );
            labels.insert(
                "scoring_evidence_digest_hex".into(),
                JsonValue::from(hex::encode(envelope.scoring_evidence_digest)),
            );
            labels.insert(
                "signature_count".into(),
                JsonValue::from(envelope.signatures.len() as u64),
            );
            let (encoded_path, json_path) = self.record_publish_index(
                "reputation_snapshot",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "reputation_snapshot",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("reputation_snapshot", &result, encoded.len());
        result
    }
    fn publish_moderation_ballot_event(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(event, encoded, "moderation ballot event")?;
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid moderation ballot event: {err}"))
            })?;
            let runtime_payload = GovernanceLogPayloadV1::ModerationBallotEvent(event.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = moderation_ballot_event_json(event, encoded, &digest_hex)?;
            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(event.case_id.clone()));
            labels.insert("round_id".into(), JsonValue::from(event.round_id.clone()));
            labels.insert("kind".into(), JsonValue::from(event.kind.as_str()));
            labels.insert("sequence".into(), JsonValue::from(event.sequence));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(event.generated_at_unix_ms),
            );
            labels.insert(
                "committed_count".into(),
                JsonValue::from(event.committed_count),
            );
            labels.insert(
                "revealed_count".into(),
                JsonValue::from(event.revealed_count),
            );
            if let Some(juror_id) = &event.juror_id {
                labels.insert("juror_id".into(), JsonValue::from(juror_id.clone()));
            }
            if let Some(tally) = &event.tally {
                labels.insert(
                    "votes_total".into(),
                    JsonValue::from(u64::from(tally.votes_total)),
                );
                labels.insert("quorum".into(), JsonValue::from(u64::from(tally.quorum)));
                labels.insert("contested".into(), JsonValue::from(tally.contested));
                if let Some(choice) = tally.winning_choice {
                    labels.insert("winning_choice".into(), JsonValue::from(choice.as_str()));
                }
            }
            let (encoded_path, json_path) = self.record_publish_index(
                "moderation_ballot_event",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "moderation_ballot_event",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("moderation_ballot_event", &result, encoded.len());
        result
    }
    fn publish_transparency_ledger_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        authorization: Option<&PrivacyPublicationAuthorizationV1>,
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                publication,
                encoded,
                "transparency ledger publication",
            )?;
            publication.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid transparency ledger publication: {err}"
                ))
            })?;
            let is_privacy_publication = !publication.privacy_aggregates.is_empty();
            let fenced_publication = if is_privacy_publication {
                let authorization = authorization.ok_or_else(|| {
                    GovernancePublishError::other(
                        "privacy transparency publication requires finalized authorization",
                    )
                })?;
                authorization.validate_publication(publication, encoded)?;
                let publisher = self.fenced_privacy_publisher.as_ref().ok_or_else(|| {
                    GovernancePublishError::other(
                        "privacy transparency publication requires a qualified fused target publisher",
                    )
                })?;
                let head_reader = self.fenced_privacy_head_reader.as_ref().ok_or_else(|| {
                    GovernancePublishError::other(
                        "privacy transparency publication requires a qualified authenticated authoritative-head reader",
                    )
                })?;
                ensure_fenced_privacy_runtime_bindings_match(publisher, head_reader)?;
                let authoritative_head =
                    synchronize_fenced_privacy_authoritative_head(&self.root, head_reader, None)?;
                let pending = read_fenced_privacy_pending_request(&self.root)?;
                let cached = read_fenced_privacy_head_cache(&self.root)?;
                let request = if let Some(pending) = pending {
                    pending.reconstruct_request(authorization, publication, encoded, publisher)?
                } else {
                    let exact_retry = match &cached {
                        Some(cache) => {
                            cache.exact_retry_request(authorization, publication, encoded)?
                        }
                        None => None,
                    };
                    if let Some(request) = exact_retry {
                        request
                    } else {
                        let fencing_floor = authoritative_head
                            .map_or(0, FencedTransparencyTargetHeadV1::fencing_floor);
                        FencedPrivacyPublicationRequestV1::try_new(
                            authorization.clone(),
                            publication,
                            encoded.to_vec(),
                            authoritative_head,
                            fencing_floor,
                        )
                        .map_err(|error| GovernancePublishError::other(error.to_string()))?
                    }
                };
                let pending = FencedPrivacyPendingRequestV1::from_request(&request, publisher)?;
                // This durable intent journal is neither a publication artifact
                // nor an authoritative-head cache. Persisting the exact request
                // before the external mutation closes the crash window while
                // every externally visible local artifact remains receipt-gated.
                write_fenced_privacy_pending_request(&self.root, &pending)?;
                let receipt = match publisher.compare_and_append_privacy_classified(&request) {
                    Ok(receipt) => receipt,
                    Err(failure) if failure.may_have_appended => return Err(failure.error),
                    Err(failure) => {
                        remove_fenced_privacy_pending_request(&self.root).map_err(
                            |cleanup_error| {
                                GovernancePublishError::other(format!(
                                    "{}; additionally failed to clear the definitive pending request: {cleanup_error}",
                                    failure.error
                                ))
                            },
                        )?;
                        return Err(failure.error);
                    }
                };
                let authoritative_head = synchronize_fenced_privacy_authoritative_head(
                    &self.root,
                    head_reader,
                    Some(&receipt),
                )?;
                let next_cache = FencedPrivacyPublicationCacheV1::from_verified_receipt(
                    &request,
                    &receipt,
                    authoritative_head,
                )?;
                write_fenced_privacy_head_cache(&self.root, &next_cache)?;
                Some((receipt, request.authorization().clone()))
            } else if authorization.is_some() {
                return Err(GovernancePublishError::other(
                    "non-privacy transparency publication must not receive privacy authorization",
                ));
            } else {
                None
            };
            let runtime_payload = if fenced_publication.is_none() {
                let external = GovernanceExternalPayloadV1::from_transparency_ledger_publication(
                    publication,
                    encoded,
                )
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
                let payload = GovernanceLogPayloadV1::ExternalPayload(external);
                self.preflight_runtime_signed_payload_with_provenance(
                    &payload,
                    encoded.len(),
                    provenance,
                )?;
                Some(payload)
            } else {
                None
            };
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = bind_authenticated_submission_json(
                transparency_ledger_publication_json(publication, encoded, &digest_hex)?,
                provenance,
            )?;
            let block_hash = publication.block.block_hash().map_err(|err| {
                GovernancePublishError::other(format!("hash transparency ledger block: {err}"))
            })?;
            let publication_hash = publication.publication_hash().map_err(|err| {
                GovernancePublishError::other(format!(
                    "hash transparency ledger publication: {err}"
                ))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "cycle_id_hex".into(),
                JsonValue::from(hex::encode(publication.block.cycle_id)),
            );
            labels.insert(
                "cycle_start_unix".into(),
                JsonValue::from(publication.block.cycle_start_unix),
            );
            labels.insert(
                "cycle_end_unix".into(),
                JsonValue::from(publication.block.cycle_end_unix),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(publication.block.generated_at_unix),
            );
            labels.insert(
                "entry_count".into(),
                JsonValue::from(publication.block.entry_count),
            );
            labels.insert(
                "entry_root_hex".into(),
                JsonValue::from(hex::encode(publication.block.entry_root)),
            );
            labels.insert(
                "block_hash_hex".into(),
                JsonValue::from(hex::encode(block_hash)),
            );
            labels.insert(
                "publication_hash_hex".into(),
                JsonValue::from(hex::encode(publication_hash)),
            );
            if let Some((_, authorization)) = &fenced_publication {
                let lease = authorization.leader_lease();
                labels.insert(
                    "leader_lease_id_hex".into(),
                    JsonValue::from(hex::encode(lease.lease_id())),
                );
                labels.insert(
                    "leader_lease_fencing_token".into(),
                    JsonValue::from(lease.fencing_token()),
                );
                labels.insert(
                    "leader_lease_provider_handle".into(),
                    JsonValue::from(lease.provider_binding().handle()),
                );
                labels.insert(
                    "leader_lease_provider_revision".into(),
                    JsonValue::from(lease.provider_binding().qualification().revision()),
                );
                labels.insert(
                    "leader_lease_provider_policy_digest_hex".into(),
                    JsonValue::from(hex::encode(
                        lease.provider_binding().qualification().policy_digest(),
                    )),
                );
                labels.insert(
                    "privacy_release_sequence".into(),
                    JsonValue::from(authorization.release_sequence()),
                );
                labels.insert(
                    "privacy_release_record_digest_hex".into(),
                    JsonValue::from(hex::encode(authorization.release_record_digest())),
                );
                labels.insert(
                    "privacy_finalized_anchor_sequence".into(),
                    JsonValue::from(authorization.finalized_anchor().sequence()),
                );
                labels.insert(
                    "privacy_finalized_anchor_record_digest_hex".into(),
                    JsonValue::from(hex::encode(
                        authorization.finalized_anchor().record_digest(),
                    )),
                );
            }
            if let Some((receipt, _)) = &fenced_publication {
                let included_head = receipt.included_head();
                labels.insert(
                    "fenced_publication_request_digest_hex".into(),
                    JsonValue::from(hex::encode(receipt.request_digest())),
                );
                labels.insert(
                    "fenced_publication_idempotency_digest_hex".into(),
                    JsonValue::from(hex::encode(receipt.publication_idempotency_digest())),
                );
                labels.insert(
                    "fenced_publication_disposition".into(),
                    JsonValue::from(match receipt.disposition() {
                        FencedPrivacyPublicationDispositionV1::Appended => "appended",
                        FencedPrivacyPublicationDispositionV1::AlreadyIncluded => {
                            "already_included"
                        }
                    }),
                );
                labels.insert(
                    "fenced_authoritative_generation".into(),
                    JsonValue::from(included_head.generation()),
                );
                labels.insert(
                    "fenced_authoritative_head_digest_hex".into(),
                    JsonValue::from(hex::encode(included_head.head_digest())),
                );
                labels.insert(
                    "fenced_readback_generation".into(),
                    JsonValue::from(receipt.readback_head().generation()),
                );
                labels.insert(
                    "fenced_readback_head_digest_hex".into(),
                    JsonValue::from(hex::encode(receipt.readback_head().head_digest())),
                );
                labels.insert(
                    "fenced_head_inclusion_digest_hex".into(),
                    JsonValue::from(hex::encode(receipt.head_inclusion_digest())),
                );
            }
            bind_authenticated_submission_labels(&mut labels, provenance);
            let (encoded_path, json_path) = self.record_publish_index(
                "transparency_ledger_publication",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            if let Some(runtime_payload) = runtime_payload {
                self.record_runtime_signed_payload_with_provenance(
                    "transparency_ledger_publication",
                    runtime_payload,
                    &encoded_path,
                    &json_path,
                    &digest_hex,
                    encoded.len(),
                    provenance,
                )?;
            } else {
                remove_fenced_privacy_pending_request(&self.root)?;
            }
            Ok(())
        })();
        record_governance_dag_publish_result(
            "transparency_ledger_publication",
            &result,
            encoded.len(),
        );
        result
    }
    fn publish_proof_token_issuance(
        &self,
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(issuance, encoded, "proof-token issuance")?;
            issuance.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid proof-token issuance: {err}"))
            })?;
            let external =
                GovernanceExternalPayloadV1::from_proof_token_issuance(issuance, encoded)
                    .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            let runtime_payload = GovernanceLogPayloadV1::ExternalPayload(external);
            self.preflight_runtime_signed_payload_with_provenance(
                &runtime_payload,
                encoded.len(),
                provenance,
            )?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = bind_authenticated_submission_json(
                proof_token_issuance_json(issuance, encoded, &digest_hex)?,
                provenance,
            )?;
            let mut labels = JsonMap::new();
            labels.insert(
                "token_id_hex".into(),
                JsonValue::from(hex::encode(issuance.token_id)),
            );
            labels.insert(
                "issued_at_unix".into(),
                JsonValue::from(issuance.issued_at_unix),
            );
            if let Some(expires_at_unix) = issuance.expires_at_unix {
                labels.insert("expires_at_unix".into(), JsonValue::from(expires_at_unix));
            }
            labels.insert(
                "moderation_action_code".into(),
                JsonValue::from(u64::from(issuance.moderation_action_code)),
            );
            labels.insert(
                "signer_key_hex".into(),
                JsonValue::from(hex::encode(issuance.signer_key)),
            );
            labels.insert(
                "token_blake3_hex".into(),
                JsonValue::from(hex::encode(issuance.token_blake3)),
            );
            labels.insert(
                "blinded_digest_hex".into(),
                JsonValue::from(hex::encode(issuance.blinded_digest)),
            );
            labels.insert(
                "entry_count".into(),
                JsonValue::from(issuance.entry_ids.len() as u64),
            );
            if let Some(first_entry_id) = issuance.entry_ids.first() {
                labels.insert(
                    "first_entry_id".into(),
                    JsonValue::from(first_entry_id.clone()),
                );
            }
            if let Some(evidence_digest) = issuance.evidence_digest {
                labels.insert(
                    "evidence_digest_hex".into(),
                    JsonValue::from(hex::encode(evidence_digest)),
                );
            }
            if let Some(policy_digest) = issuance.policy_digest {
                labels.insert(
                    "policy_digest_hex".into(),
                    JsonValue::from(hex::encode(policy_digest)),
                );
            }
            bind_authenticated_submission_labels(&mut labels, provenance);
            let (encoded_path, json_path) = self.record_publish_index(
                "proof_token_issuance",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload_with_provenance(
                "proof_token_issuance",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                provenance,
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("proof_token_issuance", &result, encoded.len());
        result
    }
    fn publish_appeal_finance_report(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
        provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "appeal finance report")?;
            report.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid appeal finance report: {err}"))
            })?;
            let runtime_payload = GovernanceLogPayloadV1::AppealFinanceReport(report.clone());
            self.preflight_runtime_signed_payload_with_provenance(
                &runtime_payload,
                encoded.len(),
                Some(provenance),
            )?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = bind_authenticated_submission_json(
                appeal_finance_report_json(report, encoded, &digest_hex)?,
                Some(provenance),
            )?;
            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(report.case_id.clone()));
            if let Some(round_id) = &report.round_id {
                labels.insert("round_id".into(), JsonValue::from(round_id.clone()));
            }
            labels.insert(
                "report_id_hex".into(),
                JsonValue::from(hex::encode(report.report_id)),
            );
            labels.insert("outcome".into(), JsonValue::from(report.outcome.as_str()));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(report.generated_at_unix_ms),
            );
            labels.insert(
                "appeal_finance_config_version".into(),
                JsonValue::from(report.appeal_finance_config_version.clone()),
            );
            labels.insert(
                "deposit_xor".into(),
                JsonValue::from(report.deposit_xor.to_string()),
            );
            labels.insert(
                "refund_xor".into(),
                JsonValue::from(report.refund.amount_xor.to_string()),
            );
            labels.insert(
                "treasury_xor".into(),
                JsonValue::from(report.treasury.amount_xor.to_string()),
            );
            labels.insert(
                "held_xor".into(),
                JsonValue::from(report.held.amount_xor.to_string()),
            );
            labels.insert(
                "panel_size".into(),
                JsonValue::from(u64::from(report.panel_size)),
            );
            labels.insert(
                "panel_reward_total_xor".into(),
                JsonValue::from(report.panel_reward_total_xor.to_string()),
            );
            labels.insert(
                "rewards_paid_total_xor".into(),
                JsonValue::from(report.rewards_paid_total_xor.to_string()),
            );
            labels.insert(
                "rewards_forfeited_treasury_xor".into(),
                JsonValue::from(report.rewards_forfeited_treasury_xor.to_string()),
            );
            labels.insert(
                "juror_payout_count".into(),
                JsonValue::from(report.juror_payouts.len() as u64),
            );
            labels.insert(
                "no_show_count".into(),
                JsonValue::from(report.no_show_juror_ids.len() as u64),
            );
            bind_authenticated_submission_labels(&mut labels, Some(provenance));
            let (encoded_path, json_path) = self.record_publish_index(
                "appeal_finance_report",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload_with_provenance(
                "appeal_finance_report",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                Some(provenance),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("appeal_finance_report", &result, encoded.len());
        result
    }
    fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
        provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(rollup, encoded, "appeal finance weekly rollup")?;
            rollup.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid appeal finance weekly rollup: {err}"
                ))
            })?;
            let runtime_payload = GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup.clone());
            self.preflight_runtime_signed_payload_with_provenance(
                &runtime_payload,
                encoded.len(),
                Some(provenance),
            )?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = bind_authenticated_submission_json(
                appeal_finance_weekly_rollup_json(rollup, encoded, &digest_hex)?,
                Some(provenance),
            )?;
            let mut labels = JsonMap::new();
            labels.insert("cycle".into(), JsonValue::from(rollup.cycle.to_string()));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(rollup.generated_at_unix_ms),
            );
            labels.insert("report_count".into(), JsonValue::from(rollup.report_count));
            labels.insert("case_count".into(), JsonValue::from(rollup.case_count));
            labels.insert(
                "config_version_count".into(),
                JsonValue::from(rollup.appeal_finance_config_versions.len() as u64),
            );
            labels.insert(
                "outcome_count".into(),
                JsonValue::from(rollup.outcomes.len() as u64),
            );
            labels.insert(
                "juror_payout_count".into(),
                JsonValue::from(rollup.juror_payout_count),
            );
            labels.insert(
                "no_show_count".into(),
                JsonValue::from(rollup.no_show_juror_count),
            );
            labels.insert(
                "total_treasury_xor".into(),
                JsonValue::from(rollup.total_treasury_xor.to_string()),
            );
            labels.insert(
                "total_rewards_forfeited_treasury_xor".into(),
                JsonValue::from(rollup.total_rewards_forfeited_treasury_xor.to_string()),
            );
            bind_authenticated_submission_labels(&mut labels, Some(provenance));
            let (encoded_path, json_path) = self.record_publish_index(
                "appeal_finance_weekly_rollup",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload_with_provenance(
                "appeal_finance_weekly_rollup",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                Some(provenance),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result(
            "appeal_finance_weekly_rollup",
            &result,
            encoded.len(),
        );
        result
    }
    fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                receipt,
                encoded,
                "appeal finance settlement receipt",
            )?;
            receipt.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid appeal finance settlement receipt: {err}"
                ))
            })?;
            let runtime_payload =
                GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(receipt.clone());
            self.preflight_runtime_signed_payload(&runtime_payload, encoded.len())?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let json_body = appeal_finance_settlement_receipt_json(receipt, encoded, &digest_hex)?;
            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(receipt.case_id.clone()));
            if let Some(round_id) = &receipt.round_id {
                labels.insert("round_id".into(), JsonValue::from(round_id.clone()));
            }
            labels.insert(
                "receipt_id_hex".into(),
                JsonValue::from(hex::encode(receipt.receipt_id)),
            );
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(receipt.generated_at_unix_ms),
            );
            labels.insert(
                "finalized_block_height".into(),
                JsonValue::from(receipt.finalized_block_height),
            );
            labels.insert(
                "finalized_block_hash_hex".into(),
                JsonValue::from(hex::encode(receipt.finalized_block_hash)),
            );
            labels.insert(
                "appeal_finance_config_version".into(),
                JsonValue::from(receipt.appeal_finance_config_version.clone()),
            );
            labels.insert(
                "appeal_finance_policy_digest_hex".into(),
                JsonValue::from(hex::encode(receipt.appeal_finance_policy_digest)),
            );
            labels.insert("outcome".into(), JsonValue::from(receipt.outcome.as_str()));
            labels.insert(
                "escrow_id_hex".into(),
                JsonValue::from(receipt.escrow_id_hex.clone()),
            );
            labels.insert(
                "submitted_step".into(),
                JsonValue::from(receipt.submitted_step.clone()),
            );
            labels.insert(
                "required_authority".into(),
                JsonValue::from(receipt.required_authority.clone()),
            );
            labels.insert(
                "tx_hash_hex".into(),
                JsonValue::from(receipt.tx_hash_hex.clone()),
            );
            labels.insert(
                "reconciliation_digest_hex".into(),
                JsonValue::from(receipt.reconciliation_digest_hex.clone()),
            );
            labels.insert(
                "reconciliation_status".into(),
                JsonValue::from(receipt.reconciliation_status.clone()),
            );
            labels.insert(
                "observed_lifecycle_status".into(),
                JsonValue::from(receipt.observed_lifecycle_status.clone()),
            );
            labels.insert(
                "amount_xor".into(),
                JsonValue::from(receipt.amount_xor.to_string()),
            );
            labels.insert(
                "deposit_xor".into(),
                JsonValue::from(receipt.deposit_xor.to_string()),
            );
            labels.insert(
                "refund_xor".into(),
                JsonValue::from(receipt.refund_xor.to_string()),
            );
            labels.insert(
                "treasury_xor".into(),
                JsonValue::from(receipt.treasury_xor.to_string()),
            );
            labels.insert(
                "held_xor".into(),
                JsonValue::from(receipt.held_xor.to_string()),
            );
            labels.insert(
                "panel_size".into(),
                JsonValue::from(u64::from(receipt.panel_size)),
            );
            labels.insert(
                "configured_signer_count".into(),
                JsonValue::from(u64::from(receipt.configured_signer_count)),
            );
            let (encoded_path, json_path) = self.record_publish_index(
                "appeal_finance_settlement_receipt",
                encoded,
                json_body.as_bytes(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "appeal_finance_settlement_receipt",
                runtime_payload,
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result(
            "appeal_finance_settlement_receipt",
            &result,
            encoded.len(),
        );
        result
    }
}
fn reputation_snapshot_json(
    envelope: &SignedReputationSnapshotV1,
    encoded_len: usize,
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    if encoded_len == 0 || encoded_len > GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "signed reputation snapshot encoded length must be in 1..={GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES} bytes"
        )));
    }
    let mut payload = JsonMap::new();
    payload.insert(
        "schema".into(),
        JsonValue::from("sorafs.reputation_snapshot.metadata.v1"),
    );
    let snapshot = &envelope.snapshot;
    let mut metadata = JsonMap::new();
    metadata.insert(
        "snapshot_id_hex".into(),
        JsonValue::from(hex::encode(snapshot.snapshot_id)),
    );
    metadata.insert(
        "generated_at_unix".into(),
        JsonValue::from(snapshot.generated_at_unix),
    );
    metadata.insert(
        "provider_count".into(),
        JsonValue::from(snapshot.providers.len() as u64),
    );
    metadata.insert(
        "merkle_root_hex".into(),
        JsonValue::from(hex::encode(snapshot.merkle_root)),
    );
    metadata.insert(
        "policy_digest_hex".into(),
        JsonValue::from(hex::encode(envelope.policy_digest)),
    );
    metadata.insert(
        "scoring_evidence_digest_hex".into(),
        JsonValue::from(hex::encode(envelope.scoring_evidence_digest)),
    );
    metadata.insert(
        "signature_count".into(),
        JsonValue::from(envelope.signatures.len() as u64),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(encoded_len).unwrap_or(u64::MAX)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize signed reputation snapshot json: {err}"))
    })
}
fn moderation_ballot_event_json(
    event: &SoraFsModerationBallotGovernanceEventV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "event".into(),
        json::to_value(event).map_err(|err| {
            GovernancePublishError::other(format!("serialize moderation ballot event: {err}"))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert("case_id".into(), JsonValue::from(event.case_id.clone()));
    metadata.insert("round_id".into(), JsonValue::from(event.round_id.clone()));
    metadata.insert("kind".into(), JsonValue::from(event.kind.as_str()));
    metadata.insert("sequence".into(), JsonValue::from(event.sequence));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(event.generated_at_unix_ms),
    );
    metadata.insert(
        "committed_count".into(),
        JsonValue::from(event.committed_count),
    );
    metadata.insert(
        "revealed_count".into(),
        JsonValue::from(event.revealed_count),
    );
    metadata.insert(
        "challenge_count".into(),
        JsonValue::from(event.challenge_count),
    );
    if let Some(juror_id) = &event.juror_id {
        metadata.insert("juror_id".into(), JsonValue::from(juror_id.clone()));
    }
    if let Some(challenge) = &event.challenge {
        metadata.insert(
            "challenge_id".into(),
            JsonValue::from(challenge.challenge_id.clone()),
        );
        metadata.insert(
            "challenge_kind".into(),
            JsonValue::from(challenge.kind.as_str()),
        );
        if let Some(decision) = challenge.decision {
            metadata.insert(
                "challenge_decision".into(),
                JsonValue::from(decision.as_str()),
            );
        }
    }
    if let Some(tally) = &event.tally {
        metadata.insert(
            "votes_total".into(),
            JsonValue::from(u64::from(tally.votes_total)),
        );
        metadata.insert("quorum".into(), JsonValue::from(u64::from(tally.quorum)));
        metadata.insert("contested".into(), JsonValue::from(tally.contested));
        if let Some(choice) = tally.winning_choice {
            metadata.insert("winning_choice".into(), JsonValue::from(choice.as_str()));
        }
    }
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize moderation ballot event json: {err}"))
    })
}
fn transparency_ledger_publication_json(
    publication: &ModerationLedgerCyclePublicationV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let block_hash = publication.block.block_hash().map_err(|err| {
        GovernancePublishError::other(format!("hash transparency ledger block: {err}"))
    })?;
    let publication_hash = publication.publication_hash().map_err(|err| {
        GovernancePublishError::other(format!("hash transparency ledger publication: {err}"))
    })?;
    let mut payload = JsonMap::new();
    payload.insert(
        "publication".into(),
        json::to_value(publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "serialize transparency ledger publication: {err}"
            ))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert(
        "cycle_id_hex".into(),
        JsonValue::from(hex::encode(publication.block.cycle_id)),
    );
    metadata.insert(
        "cycle_start_unix".into(),
        JsonValue::from(publication.block.cycle_start_unix),
    );
    metadata.insert(
        "cycle_end_unix".into(),
        JsonValue::from(publication.block.cycle_end_unix),
    );
    metadata.insert(
        "generated_at_unix".into(),
        JsonValue::from(publication.block.generated_at_unix),
    );
    metadata.insert(
        "entry_count".into(),
        JsonValue::from(publication.block.entry_count),
    );
    metadata.insert(
        "proof_count".into(),
        JsonValue::from(publication.proofs.len() as u64),
    );
    metadata.insert(
        "entry_root_hex".into(),
        JsonValue::from(hex::encode(publication.block.entry_root)),
    );
    metadata.insert(
        "block_hash_hex".into(),
        JsonValue::from(hex::encode(block_hash)),
    );
    metadata.insert(
        "publication_hash_hex".into(),
        JsonValue::from(hex::encode(publication_hash)),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize transparency ledger publication json: {err}"
        ))
    })
}
fn proof_token_issuance_json(
    issuance: &ProofTokenIssuanceV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "issuance".into(),
        json::to_value(issuance).map_err(|err| {
            GovernancePublishError::other(format!("serialize proof-token issuance: {err}"))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert(
        "payload_version".into(),
        JsonValue::from(u64::from(PROOF_TOKEN_ISSUANCE_VERSION_V1)),
    );
    metadata.insert(
        "token_id_hex".into(),
        JsonValue::from(hex::encode(issuance.token_id)),
    );
    metadata.insert(
        "issued_at_unix".into(),
        JsonValue::from(issuance.issued_at_unix),
    );
    if let Some(expires_at_unix) = issuance.expires_at_unix {
        metadata.insert("expires_at_unix".into(), JsonValue::from(expires_at_unix));
    }
    metadata.insert(
        "moderation_action_code".into(),
        JsonValue::from(u64::from(issuance.moderation_action_code)),
    );
    metadata.insert(
        "signer_key_hex".into(),
        JsonValue::from(hex::encode(issuance.signer_key)),
    );
    metadata.insert(
        "token_blake3_hex".into(),
        JsonValue::from(hex::encode(issuance.token_blake3)),
    );
    metadata.insert(
        "blinded_digest_hex".into(),
        JsonValue::from(hex::encode(issuance.blinded_digest)),
    );
    metadata.insert(
        "entry_count".into(),
        JsonValue::from(issuance.entry_ids.len() as u64),
    );
    metadata.insert(
        "entry_ids".into(),
        JsonValue::Array(
            issuance
                .entry_ids
                .iter()
                .cloned()
                .map(JsonValue::from)
                .collect(),
        ),
    );
    if let Some(evidence_digest) = issuance.evidence_digest {
        metadata.insert(
            "evidence_digest_hex".into(),
            JsonValue::from(hex::encode(evidence_digest)),
        );
    }
    if let Some(policy_digest) = issuance.policy_digest {
        metadata.insert(
            "policy_digest_hex".into(),
            JsonValue::from(hex::encode(policy_digest)),
        );
    }
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize proof-token issuance json: {err}"))
    })
}
fn appeal_finance_report_json(
    report: &SoraFsAppealFinanceReportV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "report".into(),
        json::to_value(report).map_err(|err| {
            GovernancePublishError::other(format!("serialize appeal finance report: {err}"))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert(
        "report_id_hex".into(),
        JsonValue::from(hex::encode(report.report_id)),
    );
    metadata.insert("case_id".into(), JsonValue::from(report.case_id.clone()));
    if let Some(round_id) = &report.round_id {
        metadata.insert("round_id".into(), JsonValue::from(round_id.clone()));
    }
    metadata.insert("outcome".into(), JsonValue::from(report.outcome.as_str()));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(report.generated_at_unix_ms),
    );
    metadata.insert(
        "appeal_finance_config_version".into(),
        JsonValue::from(report.appeal_finance_config_version.clone()),
    );
    metadata.insert(
        "deposit_xor".into(),
        JsonValue::from(report.deposit_xor.to_string()),
    );
    metadata.insert(
        "refund_xor".into(),
        JsonValue::from(report.refund.amount_xor.to_string()),
    );
    metadata.insert(
        "treasury_xor".into(),
        JsonValue::from(report.treasury.amount_xor.to_string()),
    );
    metadata.insert(
        "held_xor".into(),
        JsonValue::from(report.held.amount_xor.to_string()),
    );
    metadata.insert(
        "panel_size".into(),
        JsonValue::from(u64::from(report.panel_size)),
    );
    metadata.insert(
        "juror_payout_count".into(),
        JsonValue::from(report.juror_payouts.len() as u64),
    );
    metadata.insert(
        "no_show_count".into(),
        JsonValue::from(report.no_show_juror_ids.len() as u64),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize appeal finance report json: {err}"))
    })
}
fn appeal_finance_weekly_rollup_json(
    rollup: &SoraFsAppealFinanceWeeklyRollupV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "rollup".into(),
        json::to_value(rollup).map_err(|err| {
            GovernancePublishError::other(format!("serialize appeal finance weekly rollup: {err}"))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert("cycle".into(), JsonValue::from(rollup.cycle.to_string()));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(rollup.generated_at_unix_ms),
    );
    metadata.insert("report_count".into(), JsonValue::from(rollup.report_count));
    metadata.insert("case_count".into(), JsonValue::from(rollup.case_count));
    metadata.insert(
        "config_versions".into(),
        JsonValue::Array(
            rollup
                .appeal_finance_config_versions
                .iter()
                .cloned()
                .map(JsonValue::from)
                .collect(),
        ),
    );
    metadata.insert(
        "total_deposit_xor".into(),
        JsonValue::from(rollup.total_deposit_xor.to_string()),
    );
    metadata.insert(
        "total_refund_xor".into(),
        JsonValue::from(rollup.total_refund_xor.to_string()),
    );
    metadata.insert(
        "total_treasury_xor".into(),
        JsonValue::from(rollup.total_treasury_xor.to_string()),
    );
    metadata.insert(
        "total_held_xor".into(),
        JsonValue::from(rollup.total_held_xor.to_string()),
    );
    metadata.insert(
        "total_rewards_forfeited_treasury_xor".into(),
        JsonValue::from(rollup.total_rewards_forfeited_treasury_xor.to_string()),
    );
    metadata.insert(
        "juror_payout_count".into(),
        JsonValue::from(rollup.juror_payout_count),
    );
    metadata.insert(
        "no_show_count".into(),
        JsonValue::from(rollup.no_show_juror_count),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize appeal finance weekly rollup json: {err}"
        ))
    })
}
fn appeal_finance_settlement_receipt_json(
    receipt: &SoraFsAppealFinanceSettlementReceiptV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "receipt".into(),
        json::to_value(receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "serialize appeal finance settlement receipt: {err}"
            ))
        })?,
    );
    let mut metadata = JsonMap::new();
    metadata.insert(
        "receipt_id_hex".into(),
        JsonValue::from(hex::encode(receipt.receipt_id)),
    );
    metadata.insert("case_id".into(), JsonValue::from(receipt.case_id.clone()));
    if let Some(round_id) = &receipt.round_id {
        metadata.insert("round_id".into(), JsonValue::from(round_id.clone()));
    }
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(receipt.generated_at_unix_ms),
    );
    metadata.insert(
        "finalized_block_height".into(),
        JsonValue::from(receipt.finalized_block_height),
    );
    metadata.insert(
        "finalized_block_hash_hex".into(),
        JsonValue::from(hex::encode(receipt.finalized_block_hash)),
    );
    metadata.insert(
        "appeal_finance_config_version".into(),
        JsonValue::from(receipt.appeal_finance_config_version.clone()),
    );
    metadata.insert(
        "appeal_finance_policy_digest_hex".into(),
        JsonValue::from(hex::encode(receipt.appeal_finance_policy_digest)),
    );
    metadata.insert("outcome".into(), JsonValue::from(receipt.outcome.as_str()));
    metadata.insert(
        "escrow_id_hex".into(),
        JsonValue::from(receipt.escrow_id_hex.clone()),
    );
    metadata.insert(
        "submitted_step".into(),
        JsonValue::from(receipt.submitted_step.clone()),
    );
    metadata.insert(
        "required_authority".into(),
        JsonValue::from(receipt.required_authority.clone()),
    );
    metadata.insert(
        "tx_hash_hex".into(),
        JsonValue::from(receipt.tx_hash_hex.clone()),
    );
    metadata.insert(
        "reconciliation_digest_hex".into(),
        JsonValue::from(receipt.reconciliation_digest_hex.clone()),
    );
    metadata.insert(
        "reconciliation_status".into(),
        JsonValue::from(receipt.reconciliation_status.clone()),
    );
    metadata.insert(
        "observed_lifecycle_status".into(),
        JsonValue::from(receipt.observed_lifecycle_status.clone()),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    payload.insert("metadata".into(), JsonValue::Object(metadata));
    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize appeal finance settlement receipt json: {err}"
        ))
    })
}
#[cfg(test)]
mod tests {
    include!("governance/tests/support.rs");
    include!("governance/tests/support_publication.rs");
    include!("governance/tests/runtime_and_privacy_publication.rs");
    include!("governance/tests/runtime_and_privacy_publication_continued.rs");
    include!("governance/tests/publication_persistence.rs");
}
