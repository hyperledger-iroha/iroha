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

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

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
use url::Url;

use crate::{
    FencedPrivacyPublicationDispositionV1, FencedPrivacyPublicationReceiptV1,
    FencedPrivacyPublicationRequestV1, FencedTransparencyHeadAncestryProofV1,
    FencedTransparencyPublicationInclusionV1, FencedTransparencyPublishErrorV1,
    FencedTransparencyPublisherV1, FencedTransparencyTargetHeadV1, GovernancePublishError,
    GovernancePublisher, GovernanceSubmissionProvenanceV1, PdpGovernanceArchiveV1,
    PdpRejectionReasonV1, PdpTerminalDecisionV1, PrivacyPublicationAuthorizationV1,
    governance_rooted_fs,
};

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
const GOVERNANCE_DAG_SINK_FILESYSTEM: &str = "filesystem";
const GOVERNANCE_PUBLICATION_STATE_FILE: &str = "governance-publication-state-v1.json";
const GOVERNANCE_PUBLICATION_INITIALIZED_FILE: &str = ".governance-publication-initialized-v1";
const GOVERNANCE_PUBLICATION_INITIALIZED_BODY: &[u8] =
    b"sorafs.governance_dag.publication_initialized.v1\n";
const GOVERNANCE_PUBLICATION_STATE_SCHEMA: &str =
    "sorafs.governance_dag.local_publication_state.v1";
const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
// Public index metadata is root-relative; the retained descriptor is the filesystem authority.
const GOVERNANCE_DAG_LOGICAL_ROOT: &str = ".";
const GOVERNANCE_CAR_QUEUE_FILE: &str = "car-queue.json";
const GOVERNANCE_CAR_QUEUE_SCHEMA: &str = "sorafs.governance_dag.local_car_queue.v1";
const GOVERNANCE_CAR_SEGMENT_SCHEMA: &str = "sorafs.governance_dag.local_car_segment.v1";
const GOVERNANCE_CAR_PLAN_SCHEMA: &str = "sorafs.governance_dag.local_car_plan.v1";
const GOVERNANCE_PUBLICATION_SOURCES_DIR: &str = "publication-sources";
const GOVERNANCE_CAR_SEGMENTS_DIR: &str = "car-segments";
const GOVERNANCE_RUNTIME_DAG_INDEX_FILE: &str = "runtime-dag-index.json";
const GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA: &str = "sorafs.governance_dag.runtime_signed_index.v1";
const GOVERNANCE_RUNTIME_DAG_DIR: &str = "runtime-dag";
const GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR: &str = "blocks";
const GOVERNANCE_RUNTIME_DAG_HEAD_FILE: &str = "head.to";
const GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR: &str = ".runtime-dag-producer-transaction-v1";
const GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_BLOCK_FILE: &str = "block.to";
const GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_HEAD_FILE: &str = "head.to";
const GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_INDEX_FILE: &str = "index.json";
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_FILE: &str =
    "runtime-dag-qualification-history.to";
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR: &str =
    "runtime-dag-qualification-archives";
const GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_FILE: &str = "fenced-privacy-head.to";
const GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_FILE: &str = "fenced-privacy-head-sync.to";
const GOVERNANCE_FENCED_PRIVACY_PENDING_FILE: &str = "fenced-privacy-pending.to";
const GOVERNANCE_FENCED_PRIVACY_HEAD_CACHE_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_PENDING_VERSION_V1: u8 = 1;
const GOVERNANCE_FENCED_PRIVACY_PENDING_JOURNAL_VERSION_V1: u8 = 1;
const GOVERNANCE_PUBLISHER_LOCK_FILE: &str = ".governance-publisher.lock";
const GOVERNANCE_MUTABLE_INDEX_MAX_BYTES: usize = 64 * 1024 * 1024;
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
const GOVERNANCE_FENCED_PRIVACY_HEAD_MAX_BYTES: usize = 4 * 1024;
const GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_MAX_BYTES: usize = 4 * 1024;
const GOVERNANCE_FENCED_PRIVACY_PENDING_MAX_BYTES: usize = 4 * 1024;
const GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1: usize = 131_072;
const GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1: u64 = 1024 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_MAX_FUTURE_SKEW_SECS_V1: u64 = 60;
const GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1: usize = 16;
const GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1: usize = 512 * 1024 * 1024;
const GOVERNANCE_RUNTIME_DAG_DECODE_MAX_TOTAL_ELEMENTS_V1: usize = 4_000_000;
pub(crate) const GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_TRANSITION_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_VERSION_V1: u8 = 1;
const GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
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

/// Runtime-only signing boundary for the local Governance DAG publisher.
///
/// Production implementations are expected to delegate to PKCS#11, an HSM, or
/// a managed signing service. Private key bytes must never be returned to the
/// caller, persisted below the publisher root, or sourced from
/// [`iroha_config`](iroha_config).
pub trait GovernanceDagRuntimeSigner: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this signer.
    fn handle(&self) -> &str;

    /// Qualify the active adapter and its public policy revision.
    ///
    /// Implementations must fail when the HSM/KMS adapter is unavailable,
    /// revoked, stale, test-marked, or otherwise not production-ready. Provider
    /// diagnostics can contain secrets and are therefore always redacted by the
    /// caller.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;

    /// Governed publisher peer identity bound to this signer.
    fn publisher_peer_id(&self) -> &[u8];

    /// Raw Ed25519 public key bound to the opaque handle.
    fn public_key(&self) -> [u8; 32];

    /// Sign one exact canonical Governance DAG payload.
    ///
    /// Implementations must not include credentials or provider diagnostics in
    /// the returned error. This crate nevertheless redacts every provider error
    /// at the trust boundary.
    fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String>;
}

/// Authenticated Governance DAG endpoint class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GovernanceDagAuthenticationScope {
    /// Kubo/IPFS/IPNS control-plane request.
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
const GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1: u64 = 300;
const GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1: u64 = 60;

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

/// Reusable receiver boundary for authenticated Governance DAG HTTP requests.
///
/// The receiver partitions real header pairs into the fixed
/// selected-public-header set and exactly eight authentication fields, derives
/// each canonical descriptor from its exact byte body, and verifies request
/// binding, timing, the pinned Ed25519 signature, and one-use replay state
/// before returning. It is transport-agnostic and does not itself install a
/// receiver in Kubo, IPFS, IPNS, or a head service.
///
/// Replay state is deliberately caller-owned, bounded, and borrowed for this
/// receiver's lifetime. Deployments must retain one receiver/cache for the
/// lifetime of the corresponding pinned policy.
// Production qualification remains blocked until deployment-owned Kubo/head
// ingress installs this boundary and sealed cross-replica state replaces the
// process-local replay memory.
#[derive(Debug)]
pub struct GovernanceDagHttpRequestReceiverV1<'a> {
    scope: GovernanceDagAuthenticationScope,
    max_body_bytes: u64,
    policy: &'a GovernanceDagRequestAuthenticationPolicyV1,
    replay_cache: &'a mut GovernanceDagRequestAuthenticationReplayCacheV1,
}

impl<'a> GovernanceDagHttpRequestReceiverV1<'a> {
    /// Bind one endpoint scope, request-size ceiling, policy, and replay cache.
    ///
    /// # Errors
    ///
    /// Rejects a zero request-size ceiling before any request can be accepted.
    pub fn try_new(
        scope: GovernanceDagAuthenticationScope,
        max_body_bytes: u64,
        policy: &'a GovernanceDagRequestAuthenticationPolicyV1,
        replay_cache: &'a mut GovernanceDagRequestAuthenticationReplayCacheV1,
    ) -> Result<Self, GovernanceDagRequestAuthenticationErrorV1> {
        if max_body_bytes == 0 {
            return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest);
        }
        Ok(Self {
            scope,
            max_body_bytes,
            policy,
            replay_cache,
        })
    }

    /// Authenticate one complete HTTP request before backend dispatch.
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free rejection and does not return a
    /// descriptor until every structural, timing, signature, and replay check
    /// succeeds.
    pub fn verify_http_request<'h>(
        &mut self,
        method: &str,
        canonical_url: &str,
        headers: impl IntoIterator<Item = (&'h str, &'h [u8])>,
        body: &[u8],
        now_unix_secs: u64,
    ) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagRequestAuthenticationErrorV1> {
        let (selected_headers, authentication_headers) = partition_governance_dag_http_headers_v1(
            headers,
            body,
            GovernanceDagAuthenticationHeaderDispositionV1::Retain,
        )?;
        let request = GovernanceDagCanonicalRequestV1::try_from_http_parts(
            self.scope,
            method,
            canonical_url,
            selected_headers,
            body,
            self.max_body_bytes,
        )
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let envelope =
            parse_governance_dag_request_authentication_headers_v1(authentication_headers)?;
        verify_governance_dag_request_authentication_v1(
            &request,
            &envelope,
            self.scope,
            self.policy,
            now_unix_secs,
            self.replay_cache,
        )?;
        Ok(request)
    }
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
    let mut content_length = None;
    for (name, value) in headers {
        if governance_request_auth_is_forbidden_credential_header_v1(name) {
            return Err(GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader);
        }
        if governance_request_auth_header_has_prefix_v1(name) {
            if authentication_headers == GovernanceDagAuthenticationHeaderDispositionV1::Reject {
                return Err(GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader);
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
            selected.push((name, value));
            continue;
        }
        if GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1
            .iter()
            .any(|selected_name| selected_name.eq_ignore_ascii_case(name))
        {
            return Err(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader);
        }
        // Every remaining header is ordinary public HTTP metadata. It is
        // intentionally excluded from the signed descriptor.
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
    /// HTTP framing is ambiguous or disagrees with the finalized byte body.
    InvalidFraming,
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
            Self::InvalidFraming => {
                "Governance DAG request HTTP framing is ambiguous or inconsistent"
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

/// Caller-owned bounded live-nonce cache for V1 replay rejection.
///
/// Receivers should retain one cache for each independently pinned
/// authentication policy. The cache never evicts a live nonce to admit another
/// request; capacity pressure therefore fails closed.
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
    replay_cache: &mut GovernanceDagRequestAuthenticationReplayCacheV1,
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
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::SignatureVerification)?;
    replay_cache.consume(envelope.nonce(), expires_at, now_unix_secs)
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

/// Rotation-aware runtime authenticator for Governance DAG publication.
///
/// Implementations own an Ed25519 HSM signing boundary and return only the
/// public signed envelope for a complete canonical request. The adapter never
/// receives a `reqwest` client, builder, body owner, or mutable header map and
/// therefore cannot inject bearer tokens, cookies, mTLS credentials, or other
/// opaque request authority.
pub trait GovernanceDagRequestAuthenticator: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this authenticator.
    fn handle(&self) -> &str;

    /// Qualify the active adapter and its public policy revision.
    ///
    /// Implementations must fail when the credential boundary is unavailable,
    /// revoked, stale, test-marked, or otherwise not production-ready.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;

    /// Raw Ed25519 public key bound to the opaque HSM handle.
    fn public_key(&self) -> [u8; 32];

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
/// strictly advance, and deletes must compare-and-swap the exact last revision.
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
struct FencedPrivacyPendingJournalV1 {
    version: u8,
    pending: Option<FencedPrivacyPendingRequestV1>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDagProducerStagedTransactionV1 {
    block_bytes: Vec<u8>,
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
}

/// Persists governance artefacts on the filesystem for downstream ingestion.
#[derive(Debug)]
pub(crate) struct FilesystemGovernancePublisher {
    root: PathBuf,
    root_guard: GovernanceFilesystemRootGuard,
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
        let authority_temp_cleanup =
            plan_governance_publication_authority_temp_cleanup(&root_guard).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid governance publication authority temporary: {error}"),
                )
            })?;
        apply_governance_publication_cleanup_plan(&root_guard, authority_temp_cleanup).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("governance publication authority recovery failed: {error}"),
                )
            },
        )?;
        let marker_present =
            initialize_governance_publication_authority_if_pristine(&root, &root_guard).map_err(
                |error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid governance publication initialization: {error}"),
                    )
                },
            )?;
        let (publication_state, _) = read_governance_publication_state(&root, &root_guard)
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

        if let Some((history, _)) = read_runtime_dag_qualification_history(&self.root, None)? {
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
                &previous_binding,
                &next_binding,
                &predecessor,
            )?;
        let existing = read_runtime_dag_qualification_history(&self.root, Some(&previous_binding))?;
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
        let signing_bytes = runtime_dag_key_transition_signing_bytes(
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
                outgoing_signature: runtime_dag_raw_signature(&previous_signer, &signing_bytes)?,
                incoming_signature: runtime_dag_raw_signature(&next_signer, &signing_bytes)?,
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
            read_runtime_dag_qualification_history(&self.root, Some(&binding))?
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

        let (mut publication_state, _current_state_bytes) =
            read_governance_publication_state(&self.root, &self.root_guard)?;
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
                &self.root,
                &self.root_guard,
                &prepared_state,
                write_atomic,
            )
        })();
        if let Err(error) = persistence {
            if let Err(reconcile_error) =
                reconcile_current_governance_publication_artifacts(&self.root, &self.root_guard)
            {
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

fn governance_source_pair_relative_paths(
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

    fn sign(&self, payload: &[u8]) -> Result<GovernanceLogSignatureV1, GovernancePublishError> {
        self.assert_qualification()?;
        let signature_result = self.provider.sign(payload);
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
) -> Result<bool, GovernancePublishError> {
    reject_legacy_governance_publication_authorities(root, root_guard)?;
    let marker_present = read_governance_publication_initialization_marker(root, root_guard)?;
    let authority_present = root_guard
        .rooted_directory()
        .file_identity(OsStr::new(GOVERNANCE_PUBLICATION_STATE_FILE))?
        .is_some();
    if authority_present {
        return Ok(marker_present);
    }
    if marker_present || governance_publication_artifact_roots_present(root_guard)? {
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
    write_prepared_governance_publication_state(
        root,
        root_guard,
        body.as_bytes(),
        |root_guard, path, body| {
            write_rooted_atomic_expected(
                root_guard,
                path,
                body,
                governance_rooted_fs::ExpectedFile::Missing,
            )
        },
    )?;
    Ok(false)
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

fn reject_legacy_governance_publication_authorities(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    for file in [GOVERNANCE_PUBLISH_INDEX_FILE, GOVERNANCE_CAR_QUEUE_FILE] {
        match read_rooted_governance_state_file(root_guard, &root.join(file), 1) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => {
                return Err(GovernancePublishError::other(format!(
                    "legacy governance publication authority `{file}` is unsupported; remove it before first-release initialization"
                )));
            }
        }
    }
    Ok(())
}

fn read_governance_publication_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(JsonMap, usize), GovernancePublishError> {
    reject_legacy_governance_publication_authorities(root, root_guard)?;
    let path = root.join(GOVERNANCE_PUBLICATION_STATE_FILE);
    let snapshot = match read_rooted_governance_state_file(
        root_guard,
        &path,
        GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(GovernancePublishError::other(
                "authoritative governance publication state is missing after initialization",
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let value: JsonValue = json::from_slice(snapshot.bytes()).map_err(|error| {
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
    snapshot.binding().verify()?;
    Ok((state, snapshot.bytes().len()))
}

#[cfg(test)]
fn commit_governance_publication_state(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    state: JsonMap,
) -> Result<(), GovernancePublishError> {
    commit_governance_publication_state_with(root, root_guard, state, write_atomic)
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
    let body = prepare_governance_publication_state(state)?;
    write_prepared_governance_publication_state(root, root_guard, &body, writer)
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

fn write_prepared_governance_publication_state<F>(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    body: &[u8],
    writer: F,
) -> Result<(), GovernancePublishError>
where
    F: FnOnce(&GovernanceFilesystemRootGuard, &Path, &[u8]) -> io::Result<()>,
{
    if body.is_empty() || body.len() > GOVERNANCE_PUBLICATION_STATE_MAX_BYTES {
        return Err(GovernancePublishError::other(format!(
            "prepared authoritative governance publication state exceeds {GOVERNANCE_PUBLICATION_STATE_MAX_BYTES} bytes"
        )));
    }
    let path = root.join(GOVERNANCE_PUBLICATION_STATE_FILE);
    writer(root_guard, &path, body)?;
    let readback = read_rooted_governance_state_file(
        root_guard,
        &path,
        GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
    )?;
    if readback.bytes() != body {
        return Err(GovernancePublishError::other(
            "authoritative governance publication state readback diverged",
        ));
    }
    readback.binding().verify()?;
    Ok(())
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

fn governance_atomic_recovery_target(name: &str) -> Option<&str> {
    governance_publication_atomic_temp_target_name(name)
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

fn plan_governance_publication_authority_temp_cleanup(
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<GovernancePublicationArtifactCleanupPlan, GovernancePublishError> {
    let root_directory = root_guard.rooted_directory();
    let mut plan = GovernancePublicationArtifactCleanupPlan::default();
    let mut seen_targets = BTreeSet::new();
    for name in root_directory.child_names_bounded(GOVERNANCE_PUBLICATION_ENTRY_HARD_CAP)? {
        let Some(name_utf8) = name.to_str() else {
            continue;
        };
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if let Some(target) = governance_rooted_fs::atomic_retained_target_name(name_utf8) {
            let max_bytes = match target {
                GOVERNANCE_PUBLICATION_STATE_FILE => GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
                GOVERNANCE_PUBLICATION_INITIALIZED_FILE => {
                    GOVERNANCE_PUBLICATION_INITIALIZED_BODY.len()
                }
                _ => continue,
            };
            // Successful replacement keeps the exact predecessor under a
            // bounded V1 name. It is immutable online and may only be archived
            // or cleared while the publisher is stopped.
            root_directory
                .file_binding(&name, max_bytes)?
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "retained governance authority generation disappeared during startup",
                    )
                })?
                .verify()?;
            continue;
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if [
            GOVERNANCE_PUBLICATION_STATE_FILE,
            GOVERNANCE_PUBLICATION_INITIALIZED_FILE,
        ]
        .into_iter()
        .any(|target| governance_rooted_fs::is_atomic_retained_candidate_for(name_utf8, target))
        {
            return Err(GovernancePublishError::other(format!(
                "governance publication authority retained-generation name `{name_utf8}` is noncanonical; offline inspection is required"
            )));
        }
        let Some(target) = governance_atomic_recovery_target(name_utf8) else {
            continue;
        };
        let (max_bytes, quarantine_slot) = match target {
            GOVERNANCE_PUBLICATION_STATE_FILE => (
                GOVERNANCE_PUBLICATION_STATE_MAX_BYTES,
                "authority-state-temp",
            ),
            GOVERNANCE_PUBLICATION_INITIALIZED_FILE => (
                GOVERNANCE_PUBLICATION_INITIALIZED_BODY.len(),
                "authority-marker-temp",
            ),
            _ => continue,
        };
        if !seen_targets.insert(target.to_owned()) {
            return Err(GovernancePublishError::other(format!(
                "more than one interrupted governance publication authority recovery artifact targets `{target}`"
            )));
        }
        let rollback_rank = plan.authority_files.len();
        let removal = plan_governance_publication_file_removal(
            &root_directory,
            &name,
            max_bytes,
            None,
            rollback_rank,
            OsString::from(quarantine_slot),
        )?;
        plan.authority_files.push(removal);
    }
    plan.authority_files
        .sort_by(|left, right| left.quarantine_slot.cmp(&right.quarantine_slot));
    root_guard.revalidate()?;
    Ok(plan)
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
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    let (state, _) = read_governance_publication_state(root, root_guard)?;
    reconcile_governance_publication_artifacts(root_guard, &state)
}

fn validate_governance_car_source_lengths(
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
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let mut index = read_runtime_dag_index(signer, checkpoint_store, &index_path)?;
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
    node.publisher_signature = signer.sign(&node_payload)?;
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
    block.block_signature = signer.sign(&block_payload)?;
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
    head.head_signature = signer.sign(&head_payload)?;
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
    let head_path = runtime_dag_head_path(root);

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

    let index_bytes = build_runtime_dag_index_bytes(
        root,
        signer,
        checkpoint_store,
        index,
        blocks,
        &head,
        &head_path,
    )?;
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
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    index_path: &Path,
) -> Result<JsonMap, GovernancePublishError> {
    match read_bounded_governance_state_file(index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance runtime DAG index `{}`: {err}",
                    index_path.display()
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
            verify_digest_sidecar(index_path, &bytes)?;
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
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
        Err(err) => Err(err.into()),
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

fn validate_runtime_dag_mutable_file_inventory(
    root: &Path,
    head_path: &Path,
) -> Result<(), GovernancePublishError> {
    let root_guard = GovernanceFilesystemRootGuard::capture_source(root)?;
    let runtime_root = root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))?;
    let head_name = head_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG head name is not canonical UTF-8")
        })?;
    let head_sidecar_path = digest_sidecar_path_for(head_path);
    let head_sidecar_name = head_sidecar_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG head sidecar name is not canonical UTF-8",
            )
        })?;
    let expected_retained = [
        (head_name, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
        (head_sidecar_name, GOVERNANCE_DIGEST_SIDECAR_BYTES),
    ];
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    let inventory_limit = expected_retained
        .len()
        .checked_mul(governance_rooted_fs::ATOMIC_RETAINED_SLOT_COUNT_V1)
        .and_then(|retained| retained.checked_add(3))
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG mutable-file inventory bound overflowed",
            )
        })?;
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let inventory_limit = 3;

    let mut retained_bytes = 0_u64;
    for name in runtime_root.child_names_bounded(inventory_limit)? {
        if name == OsStr::new(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
            || name == OsStr::new(head_name)
            || name == OsStr::new(head_sidecar_name)
        {
            continue;
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let mut retained = None;
            for (target, max_bytes) in expected_retained.iter().copied() {
                if let Some(len) =
                    runtime_root.atomic_retained_file_len(&name, target, max_bytes, true)?
                {
                    retained = Some(len);
                    break;
                }
            }
            if let Some(len) = retained {
                retained_bytes = retained_bytes.checked_add(len).ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG retained-head byte total overflowed",
                    )
                })?;
                if retained_bytes > governance_rooted_fs::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 {
                    return Err(GovernancePublishError::other(format!(
                        "governance runtime DAG retained head predecessors exceed the {}-byte V1 bound; stop the writer and inspect, archive, or clear them offline",
                        governance_rooted_fs::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1
                    )));
                }
                continue;
            }
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG contains an orphan or malformed head transaction artifact",
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
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let index_exists = match fs::symlink_metadata(&index_path) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if !index_exists {
        let runtime_root = root.join(GOVERNANCE_RUNTIME_DAG_DIR);
        if fs::symlink_metadata(&runtime_root).is_ok() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG artifacts exist without their signed index",
            ));
        }
        return Ok(Vec::new());
    }

    let index = read_runtime_dag_index(signer, store, &index_path)?;
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
    add_runtime_dag_audit_bytes(
        &mut total_bytes,
        usize::try_from(fs::metadata(&index_path)?.len()).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG index length exceeds usize")
        })?,
    )?;
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

    let head_path = runtime_dag_head_path(root);
    let head_bytes =
        read_bounded_governance_state_file(&head_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES)?;
    verify_digest_sidecar(&head_path, &head_bytes)?;
    add_runtime_dag_audit_bytes(&mut total_bytes, head_bytes.len())?;
    let head: GovernanceDagHeadV1 =
        decode_canonical_runtime_dag(&head_bytes, "governance runtime DAG head")?;
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
        || required_runtime_string(&index, "head_path")? != index_path_string(root, &head_path)
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
    validate_runtime_dag_mutable_file_inventory(root, &head_path)?;
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

fn runtime_dag_key_transition_signing_bytes(
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
    payload: &[u8],
) -> Result<[u8; 64], GovernancePublishError> {
    signer
        .sign(payload)?
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
    let payload = runtime_dag_key_transition_signing_bytes(
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

fn validate_runtime_dag_qualification_history(
    root: &Path,
    history: &RuntimeDagQualificationHistoryV1,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
    allowed_unindexed_archive: Option<(u64, [u8; 32])>,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
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
        let archive = read_runtime_dag_qualification_archive(
            root,
            archive_generation,
            archive_digest,
            root_digest,
        )?;
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
        && (history.archived_through_generation + 1 != expected_generation
            || history.archive_tail_transition_digest != expected_predecessor.unwrap_or([0; 32])
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
    match fs::read_dir(&archives_dir) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                let expected = expected_archive_paths
                    .iter()
                    .any(|archive| path == *archive || path == digest_sidecar_path_for(archive))
                    || allowed_unindexed_archive.as_ref().is_some_and(|archive| {
                        path == *archive || path == digest_sidecar_path_for(archive)
                    });
                if !expected {
                    return Err(GovernancePublishError::other(
                        "governance runtime DAG qualification archive directory contains an unindexed fork or duplicate",
                    ));
                }
            }
        }
        Err(error)
            if error.kind() == io::ErrorKind::NotFound && expected_archive_paths.is_empty() => {}
        Err(error) => return Err(error.into()),
    }

    Ok(RuntimeDagQualificationSummary {
        transition_generation,
        transition_digest: expected_predecessor.unwrap_or([0; 32]),
        archive_generation: history.archive_generation,
        archive_digest: history.archive_digest,
    })
}

fn read_runtime_dag_qualification_history(
    root: &Path,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
) -> Result<
    Option<(
        RuntimeDagQualificationHistoryV1,
        RuntimeDagQualificationSummary,
    )>,
    GovernancePublishError,
> {
    read_runtime_dag_qualification_history_allowing_archive(root, expected_binding, None)
}

fn read_runtime_dag_qualification_history_allowing_archive(
    root: &Path,
    expected_binding: Option<&RuntimeDagProviderBindingV1>,
    allowed_unindexed_archive: Option<(u64, [u8; 32])>,
) -> Result<
    Option<(
        RuntimeDagQualificationHistoryV1,
        RuntimeDagQualificationSummary,
    )>,
    GovernancePublishError,
> {
    let path = runtime_dag_qualification_history_path(root);
    let bytes = match read_bounded_governance_state_file(
        &path,
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_MAX_BYTES_V1,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::symlink_metadata(root.join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR))
            {
                Ok(_) => {
                    return Err(GovernancePublishError::other(
                        "governance runtime DAG qualification archives exist without their authenticated history head",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
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
    let history: RuntimeDagQualificationHistoryV1 =
        decode_canonical_runtime_dag(&bytes, "governance runtime DAG qualification history")?;
    let summary = validate_runtime_dag_qualification_history(
        root,
        &history,
        expected_binding,
        allowed_unindexed_archive,
    )?;
    if missing_sidecar {
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
        write_digest_sidecar(&root_guard, &path, &bytes)?;
        verify_digest_sidecar(&path, &bytes)?;
    }
    Ok(Some((history, summary)))
}

fn runtime_dag_qualification_summary(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
) -> Result<RuntimeDagQualificationSummary, GovernancePublishError> {
    let binding = runtime_dag_provider_binding(signer, store);
    read_runtime_dag_qualification_history(root, Some(&binding)).map(|history| {
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
    root: &Path,
    history: &RuntimeDagQualificationHistoryV1,
) -> Result<Vec<RuntimeDagQualificationTransitionV1>, GovernancePublishError> {
    let mut archives = Vec::new();
    let mut generation = history.archive_generation;
    let mut digest = history.archive_digest;
    while generation != 0 {
        let archive =
            read_runtime_dag_qualification_archive(root, generation, digest, history.root_digest)?;
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
    Ok(transitions)
}

fn runtime_dag_authority_lineage(
    root: &Path,
    current_binding: &RuntimeDagProviderBindingV1,
) -> Result<RuntimeDagAuthorityLineageV1, GovernancePublishError> {
    let Some((history, qualification)) =
        read_runtime_dag_qualification_history(root, Some(current_binding))?
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
    let transitions = runtime_dag_full_transition_lineage(root, &history)?;
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
    validate_runtime_dag_producer_checkpoint_shape(checkpoint, root)?;
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
        runtime_dag_authority_lineage(root, &runtime_dag_checkpoint_binding(checkpoint))?;
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
    validate_runtime_dag_authority_lineage_for_chain(&authority_lineage, &blocks, head)
}

fn canonical_runtime_dag_index_for_transition(
    root: &Path,
    previous: &RuntimeDagProviderBindingV1,
    next: &RuntimeDagProviderBindingV1,
    checkpoint: &RuntimeDagProducerCheckpointV1,
) -> Result<([u8; 32], [u8; 32], Option<Vec<u8>>), GovernancePublishError> {
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    if checkpoint.block_count == 0 {
        if fs::symlink_metadata(&index_path).is_ok() {
            return Err(GovernancePublishError::other(
                "empty governance runtime DAG provider transition found a substituted index",
            ));
        }
        return Ok(([0; 32], [0; 32], None));
    }
    let bytes =
        read_bounded_governance_state_file(&index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES)?;
    verify_digest_sidecar(&index_path, &bytes)?;
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
    let path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let bytes = read_bounded_governance_state_file(&path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES)?;
    verify_digest_sidecar(&path, &bytes)?;
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
    let successor_index = canonical_runtime_dag_successor_index_from_transition(root, transition)?;
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
        write_runtime_dag_transaction_file(
            root_guard,
            &root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
            &successor_index,
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
            Some(transition.body.predecessor_index_digest),
            false,
        )?;
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
    let Some((history, summary)) = read_runtime_dag_qualification_history(root, None)? else {
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
    let path = runtime_dag_qualification_history_path(root);
    match (
        predecessor,
        read_bounded_governance_state_file(
            &path,
            GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_MAX_BYTES_V1,
        ),
    ) {
        (Some(predecessor), Ok(bytes)) => {
            verify_digest_sidecar(&path, &bytes)?;
            let current: RuntimeDagQualificationHistoryV1 = decode_canonical_runtime_dag(
                &bytes,
                "governance runtime DAG qualification history predecessor",
            )?;
            if current != *predecessor {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG qualification history predecessor was substituted",
                ));
            }
        }
        (None, Err(error)) if error.kind() == io::ErrorKind::NotFound => {}
        (Some(_), Err(error)) if error.kind() == io::ErrorKind::NotFound => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification history predecessor disappeared",
            ));
        }
        (None, Ok(_)) => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG qualification history creation refuses an existing predecessor",
            ));
        }
        (_, Err(error)) => return Err(error.into()),
    }
    write_runtime_dag_qualification_state(
        root_guard,
        &path,
        history,
        GOVERNANCE_RUNTIME_DAG_QUALIFICATION_HISTORY_MAX_BYTES_V1,
        false,
    )?;
    let (readback, summary) = read_runtime_dag_qualification_history(root, Some(expected_binding))?
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG qualification history disappeared after install",
            )
        })?;
    if readback != *history {
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
    let allowed_archive = (checkpoint.qualification_archive_generation != 0).then_some((
        checkpoint.qualification_archive_generation,
        checkpoint.qualification_archive_digest,
    ));
    let (history, summary, staged_archive) =
        match read_runtime_dag_qualification_history_allowing_archive(
            root,
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
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    match fs::symlink_metadata(&index_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            validate_existing_runtime_dag_root(root, signer, store)?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    validate_existing_runtime_dag_root(root, signer, store)?;
    let index_bytes =
        read_bounded_governance_state_file(&index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES)?;
    let head_bytes = read_bounded_governance_state_file(
        &runtime_dag_head_path(root),
        GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
    )?;
    runtime_dag_producer_checkpoint(root, signer, store, &head_bytes, &index_bytes).map(Some)
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
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG producer intent JSON exceeds the per-file byte limit"
            )));
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
        ("head", &intent.head, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
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

fn runtime_dag_producer_staging_paths(root: &Path) -> [PathBuf; 3] {
    let staging_root = root.join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR);
    [
        staging_root.join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_BLOCK_FILE),
        staging_root.join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_HEAD_FILE),
        staging_root.join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGED_INDEX_FILE),
    ]
}

fn ensure_runtime_dag_producer_staging_root(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
) -> Result<(), GovernancePublishError> {
    let _ = runtime_dag_producer_staging_directory(root, root_guard, true)?;
    Ok(())
}

fn runtime_dag_producer_staging_directory(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    create: bool,
) -> Result<governance_rooted_fs::RootedDirectory, GovernancePublishError> {
    if root != root_guard.root() {
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer root differs from its retained root guard",
        ));
    }
    root_guard.revalidate()?;
    let staging = if create {
        root_guard
            .rooted_directory()
            .open_or_create_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR))?
    } else {
        root_guard
            .rooted_directory()
            .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR))?
    };
    if create {
        // Persist both the empty/new staging directory and its entry in the
        // producer root before a sealed intent can reference child artifacts.
        staging.sync_all()?;
        root_guard.rooted_directory().sync_all()?;
    }
    root_guard.revalidate()?;
    Ok(staging)
}

fn write_runtime_dag_producer_staged_artifact(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), GovernancePublishError> {
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG staged artifact `{}` is outside its byte limit",
            path.display()
        )));
    }
    write_rooted_atomic(root_guard, path, bytes)?;
    write_rooted_digest_sidecar(root_guard, path, bytes)?;
    let readback = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    verify_rooted_digest_sidecar(root_guard, path, readback.bytes())?;
    if readback.bytes() != bytes {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG staged artifact `{}` readback diverged",
            path.display()
        )));
    }
    Ok(())
}

fn stage_runtime_dag_producer_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    staged: &RuntimeDagProducerStagedTransactionV1,
) -> Result<(), GovernancePublishError> {
    ensure_runtime_dag_producer_staging_root(root, root_guard)?;
    let paths = runtime_dag_producer_staging_paths(root);
    for (path, bytes, max_bytes) in [
        (
            &paths[0],
            staged.block_bytes.as_slice(),
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        ),
        (
            &paths[1],
            staged.head_bytes.as_slice(),
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        ),
        (
            &paths[2],
            staged.index_bytes.as_slice(),
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        ),
    ] {
        root_guard.revalidate()?;
        write_runtime_dag_producer_staged_artifact(root_guard, path, bytes, max_bytes)?;
    }
    root_guard.revalidate()?;
    Ok(())
}

fn validate_runtime_dag_producer_staging_inventory(
    root_guard: &GovernanceFilesystemRootGuard,
    staging: &governance_rooted_fs::RootedDirectory,
    paths: &[PathBuf; 3],
) -> Result<(), GovernancePublishError> {
    let expected = [
        (
            paths[0].file_name().map(OsStr::to_os_string),
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        ),
        (
            digest_sidecar_path_for(&paths[0])
                .file_name()
                .map(OsStr::to_os_string),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
        (
            paths[1].file_name().map(OsStr::to_os_string),
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        ),
        (
            digest_sidecar_path_for(&paths[1])
                .file_name()
                .map(OsStr::to_os_string),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
        (
            paths[2].file_name().map(OsStr::to_os_string),
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        ),
        (
            digest_sidecar_path_for(&paths[2])
                .file_name()
                .map(OsStr::to_os_string),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
    ];
    let expected = expected
        .into_iter()
        .map(|(name, max_bytes)| name.map(|name| (name, max_bytes)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG producer staging path has no canonical file name",
            )
        })?;
    for (name, _) in &expected {
        if name.to_str().is_none() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG producer staging file name is not canonical UTF-8",
            ));
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    let inventory_limit = expected
        .len()
        .checked_mul(
            governance_rooted_fs::ATOMIC_RETAINED_SLOT_COUNT_V1
                .checked_add(1)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG staging inventory bound overflowed",
                    )
                })?,
        )
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG staging inventory bound overflowed",
            )
        })?;
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let inventory_limit = expected.len();

    let mut retained_bytes = 0_u64;
    for name in staging.child_names_bounded(inventory_limit)? {
        if expected.iter().any(|(expected, _)| expected == &name) {
            continue;
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let mut retained = None;
            for (target, max_bytes) in &expected {
                let target = target.to_str().ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG staging target is not canonical UTF-8",
                    )
                })?;
                if let Some(len) =
                    staging.atomic_retained_file_len(&name, target, *max_bytes, true)?
                {
                    retained = Some(len);
                    break;
                }
            }
            if let Some(len) = retained {
                retained_bytes = retained_bytes.checked_add(len).ok_or_else(|| {
                    GovernancePublishError::other(
                        "governance runtime DAG staging retained byte total overflowed",
                    )
                })?;
                if retained_bytes > governance_rooted_fs::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 {
                    return Err(GovernancePublishError::other(format!(
                        "governance runtime DAG staging retained predecessors exceed the {}-byte V1 bound; stop the writer and inspect, archive, or clear them offline",
                        governance_rooted_fs::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1
                    )));
                }
                continue;
            }
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG producer staging root contains an unexpected or malformed artifact",
        ));
    }
    root_guard.revalidate()?;
    Ok(())
}

fn read_runtime_dag_producer_staged_artifact(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    descriptor: &RuntimeDagProducerStagedArtifactV1,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, GovernancePublishError> {
    let byte_len = usize::try_from(descriptor.byte_len).map_err(|_| {
        GovernancePublishError::other(format!(
            "sealed governance runtime DAG staged {label} length exceeds host limits"
        ))
    })?;
    if byte_len == 0 || byte_len > max_bytes || descriptor.blake3 == [0; 32] {
        return Err(GovernancePublishError::other(format!(
            "sealed governance runtime DAG staged {label} descriptor is malformed"
        )));
    }
    let snapshot = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    verify_rooted_digest_sidecar(root_guard, path, snapshot.bytes())?;
    if snapshot.bytes().len() != byte_len
        || *blake3::hash(snapshot.bytes()).as_bytes() != descriptor.blake3
    {
        return Err(GovernancePublishError::other(format!(
            "sealed governance runtime DAG staged {label} is substituted"
        )));
    }
    Ok(snapshot.into_bytes())
}

fn load_runtime_dag_producer_staged_transaction(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<RuntimeDagProducerStagedTransactionV1, GovernancePublishError> {
    let paths = runtime_dag_producer_staging_paths(root);
    let staging = runtime_dag_producer_staging_directory(root, root_guard, false)?;
    validate_runtime_dag_producer_staging_inventory(root_guard, &staging, &paths)?;
    let staged = RuntimeDagProducerStagedTransactionV1 {
        block_bytes: read_runtime_dag_producer_staged_artifact(
            root_guard,
            &paths[0],
            &intent.block,
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
            "block",
        )?,
        head_bytes: read_runtime_dag_producer_staged_artifact(
            root_guard,
            &paths[1],
            &intent.head,
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
            "head",
        )?,
        index_bytes: read_runtime_dag_producer_staged_artifact(
            root_guard,
            &paths[2],
            &intent.index,
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
            "index",
        )?,
    };
    validate_runtime_dag_producer_intent_bounds(root, intent, &staged)?;
    Ok(staged)
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
        ("head", head_len, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
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

fn write_runtime_dag_transaction_file(
    root_guard: &GovernanceFilesystemRootGuard,
    path: &Path,
    bytes: &[u8],
    max_bytes: usize,
    previous_digest: Option<[u8; 32]>,
    immutable_new: bool,
) -> Result<(), GovernancePublishError> {
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG transaction target `{}` is outside its byte limit",
            path.display()
        )));
    }
    let replacement = match read_rooted_governance_state_file(root_guard, path, max_bytes) {
        Ok(current) if current.bytes() == bytes => {
            current.binding().verify()?;
            None
        }
        Ok(current)
            if !immutable_new
                && previous_digest
                    .is_some_and(|digest| digest == *blake3::hash(current.bytes()).as_bytes()) =>
        {
            Some(governance_rooted_fs::ExpectedFile::Identity(
                current.binding(),
            ))
        }
        Ok(_) => {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG transaction refuses to overwrite substituted `{}`",
                path.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if immutable_new {
                match read_rooted_governance_state_file(
                    root_guard,
                    &digest_sidecar_path_for(path),
                    GOVERNANCE_DIGEST_SIDECAR_BYTES,
                ) {
                    Ok(_) => {
                        return Err(GovernancePublishError::other(format!(
                            "immutable governance runtime DAG transaction target `{}` has an orphan digest sidecar",
                            path.display()
                        )));
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
            // A retained sealed intent proves the exact target bytes and its
            // predecessor revision. A crash after rename but before the
            // directory entry became durable may therefore be recovered by
            // recreating a missing mutable target from those authenticated
            // bytes. Substituted extant bytes remain non-overwritable above.
            Some(governance_rooted_fs::ExpectedFile::Missing)
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(expected) = replacement {
        write_rooted_atomic_expected(root_guard, path, bytes, expected)?;
    }
    if immutable_new {
        ensure_rooted_digest_sidecar_immutable(root_guard, path, bytes)?;
    } else {
        write_rooted_digest_sidecar(root_guard, path, bytes)?;
    }
    let readback = read_rooted_governance_state_file(root_guard, path, max_bytes)?;
    verify_rooted_digest_sidecar(root_guard, path, readback.bytes())?;
    if readback.bytes() != bytes {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG transaction target `{}` durable readback diverged",
            path.display()
        )));
    }
    readback.binding().verify()?;
    Ok(())
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

#[cfg(windows)]
fn remove_recoverable_runtime_dag_transaction_temps(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<(), GovernancePublishError> {
    let block_path = runtime_dag_producer_block_path_from_intent(root, intent)?;
    let head_path = runtime_dag_head_path(root);
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    for (position, (path, max_bytes)) in [
        (&block_path, GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES),
        (&head_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
        (&index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
    ]
    .into_iter()
    .enumerate()
    {
        isolate_recoverable_atomic_state_for_target(
            root_guard,
            path,
            max_bytes,
            &format!("runtime-dag-target-{position}"),
        )?;
        isolate_recoverable_atomic_state_for_target(
            root_guard,
            &digest_sidecar_path_for(path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
            &format!("runtime-dag-digest-{position}"),
        )?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn remove_recoverable_runtime_dag_transaction_temps(
    root: &Path,
    root_guard: &GovernanceFilesystemRootGuard,
    intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<(), GovernancePublishError> {
    reject_governance_publication_recovery_quarantine(root_guard)?;
    let block_path = runtime_dag_producer_block_path_from_intent(root, intent)?;
    let head_path = runtime_dag_head_path(root);
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let targets = [
        (block_path.clone(), GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES),
        (
            digest_sidecar_path_for(&block_path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
        (head_path.clone(), GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
        (
            digest_sidecar_path_for(&head_path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
        (index_path.clone(), GOVERNANCE_MUTABLE_INDEX_MAX_BYTES),
        (
            digest_sidecar_path_for(&index_path),
            GOVERNANCE_DIGEST_SIDECAR_BYTES,
        ),
    ];
    let mut plan = GovernancePublicationArtifactCleanupPlan::default();
    for (position, (path, max_bytes)) in targets.iter().enumerate() {
        let target_name = path.file_name().and_then(OsStr::to_str).ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG recovery target name is not canonical UTF-8",
            )
        })?;
        let (parent, _) = match rooted_target(root_guard, path, false) {
            Ok(target) => target,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        plan_recoverable_atomic_temps_for_target(
            &parent,
            target_name,
            *max_bytes,
            &format!("runtime-dag-boundary-{position}"),
            &mut plan,
        )?;
    }
    root_guard.revalidate()?;
    apply_governance_publication_cleanup_plan(root_guard, plan)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn remove_recoverable_runtime_dag_transaction_temps(
    _root: &Path,
    _root_guard: &GovernanceFilesystemRootGuard,
    _intent: &RuntimeDagProducerPublishIntentV1,
) -> Result<(), GovernancePublishError> {
    Err(GovernancePublishError::other(
        "governance runtime DAG temporary recovery is unsupported on this platform",
    ))
}

fn validate_runtime_dag_producer_intent_successor(
    root: &Path,
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

    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    match read_bounded_governance_state_file(&index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(current) if current == staged.index_bytes => {}
        Ok(current)
            if previous.is_some_and(|checkpoint| {
                *blake3::hash(&current).as_bytes() == checkpoint.index_bytes_digest
            }) =>
        {
            let previous_value: JsonValue = json::from_slice(&current).map_err(|error| {
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
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The sealed intent was admitted only against the exact predecessor
            // checkpoint revision and carries the complete successor index.
            // Recovery may reconstruct a directory entry lost after rename but
            // before its parent-directory sync became durable.
        }
        Ok(_) | Err(_) => {
            return Err(GovernancePublishError::other(
                "sealed governance runtime DAG successor cannot authenticate the current index boundary",
            ));
        }
    }

    let head_path = runtime_dag_head_path(root);
    match read_bounded_governance_state_file(&head_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(current) if current == staged.head_bytes => {}
        Ok(current)
            if previous.is_some_and(|checkpoint| {
                *blake3::hash(&current).as_bytes() == checkpoint.head_bytes_digest
            }) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The signed successor head and sealed predecessor revision bind
            // the recovery bytes even when the renamed directory entry was
            // lost at the crash boundary.
        }
        Ok(_) | Err(_) => {
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
    write_runtime_dag_transaction_file(
        root_guard,
        &block_path,
        &staged.block_bytes,
        GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES,
        None,
        true,
    )?;
    root_guard.revalidate()?;
    write_runtime_dag_transaction_file(
        root_guard,
        &runtime_dag_head_path(root),
        &staged.head_bytes,
        GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        previous.map(|checkpoint| checkpoint.head_bytes_digest),
        false,
    )?;
    root_guard.revalidate()?;
    write_runtime_dag_transaction_file(
        root_guard,
        &root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
        &staged.index_bytes,
        GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        previous.map(|checkpoint| checkpoint.index_bytes_digest),
        false,
    )?;
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
        signer,
        &intent,
        &staged,
        current.as_ref(),
    )?;
    root_guard.revalidate()?;
    remove_recoverable_runtime_dag_transaction_temps(root, root_guard, &intent)?;
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
    recover_runtime_dag_qualification_compaction(root, root_guard, signer, store)?;
    recover_runtime_dag_provider_transition(root, root_guard, signer, store)?;
    root_guard.revalidate()?;
    if let Some(intent) = store.load(GovernanceDagSealedStateSlot::ProducerPublishIntent)? {
        finish_runtime_dag_producer_intent(root, root_guard, signer, store, intent)?;
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
    stage_runtime_dag_producer_transaction(root, root_guard, &staged)?;
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
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    store: &GovernanceRuntimeDagCheckpointStore,
    mut index: JsonMap,
    mut blocks: Vec<JsonValue>,
    head: &GovernanceDagHeadV1,
    head_path: &Path,
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
    index.insert(
        "head_path".into(),
        JsonValue::from(index_path_string(root, head_path)),
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

fn read_fenced_privacy_pending_request(
    root: &Path,
) -> Result<Option<FencedPrivacyPendingRequestV1>, GovernancePublishError> {
    let path = fenced_privacy_pending_path(root);
    let bytes = match read_bounded_governance_state_file(
        &path,
        GOVERNANCE_FENCED_PRIVACY_PENDING_MAX_BYTES,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let journal =
        norito::decode_from_bytes::<FencedPrivacyPendingJournalV1>(&bytes).map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy pending-request journal is not canonical Norito",
            )
        })?;
    if norito::to_bytes(&journal).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy pending-request journal")
    })? != bytes
        || journal.version != GOVERNANCE_FENCED_PRIVACY_PENDING_JOURNAL_VERSION_V1
        || journal
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.has_valid_shape())
    {
        return Err(GovernancePublishError::other(
            "fenced privacy pending-request journal is malformed",
        ));
    }
    Ok(journal.pending)
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
    let journal = FencedPrivacyPendingJournalV1 {
        version: GOVERNANCE_FENCED_PRIVACY_PENDING_JOURNAL_VERSION_V1,
        pending: Some(pending.clone()),
    };
    let bytes = norito::to_bytes(&journal).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy pending-request journal")
    })?;
    if bytes.len() > GOVERNANCE_FENCED_PRIVACY_PENDING_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "fenced privacy pending request exceeds its byte limit",
        ));
    }
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    write_atomic(&root_guard, &fenced_privacy_pending_path(root), &bytes)?;
    Ok(())
}

fn remove_fenced_privacy_pending_request(root: &Path) -> Result<(), GovernancePublishError> {
    let path = fenced_privacy_pending_path(root);
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    let journal = FencedPrivacyPendingJournalV1 {
        version: GOVERNANCE_FENCED_PRIVACY_PENDING_JOURNAL_VERSION_V1,
        pending: None,
    };
    let bytes = norito::to_bytes(&journal).map_err(|_| {
        GovernancePublishError::other("encode cleared fenced privacy pending-request journal")
    })?;
    if bytes.len() > GOVERNANCE_FENCED_PRIVACY_PENDING_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "cleared fenced privacy pending-request journal exceeds its byte limit",
        ));
    }
    write_atomic(&root_guard, &path, &bytes)?;
    Ok(())
}

fn read_fenced_privacy_head_cache(
    root: &Path,
) -> Result<Option<FencedPrivacyPublicationCacheV1>, GovernancePublishError> {
    let path = fenced_privacy_head_cache_path(root);
    let bytes =
        match read_bounded_governance_state_file(&path, GOVERNANCE_FENCED_PRIVACY_HEAD_MAX_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
    let cache =
        norito::decode_from_bytes::<FencedPrivacyPublicationCacheV1>(&bytes).map_err(|_| {
            GovernancePublishError::other(
                "fenced privacy authoritative-head cache is not canonical Norito",
            )
        })?;
    if norito::to_bytes(&cache).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy authoritative-head cache")
    })? != bytes
        || !cache.has_valid_shape()
    {
        return Err(GovernancePublishError::other(
            "fenced privacy authoritative-head cache is malformed",
        ));
    }
    Ok(Some(cache))
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
    let bytes = norito::to_bytes(cache).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy authoritative successor")
    })?;
    if bytes.len() > GOVERNANCE_FENCED_PRIVACY_HEAD_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "fenced privacy authoritative-head cache exceeds its byte limit",
        ));
    }
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    write_atomic(&root_guard, &fenced_privacy_head_cache_path(root), &bytes)?;
    Ok(())
}

fn read_fenced_privacy_head_sync(
    root: &Path,
) -> Result<Option<FencedPrivacyAuthoritativeHeadSyncV1>, GovernancePublishError> {
    let path = fenced_privacy_head_sync_path(root);
    let bytes = match read_bounded_governance_state_file(
        &path,
        GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_MAX_BYTES,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let sync = norito::decode_from_bytes::<FencedPrivacyAuthoritativeHeadSyncV1>(&bytes).map_err(
        |_| {
            GovernancePublishError::other(
                "fenced privacy authenticated head cache is not canonical Norito",
            )
        },
    )?;
    if norito::to_bytes(&sync).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy authenticated head cache")
    })? != bytes
        || !sync.has_valid_shape()
    {
        return Err(GovernancePublishError::other(
            "fenced privacy authenticated head cache is malformed",
        ));
    }
    Ok(Some(sync))
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
    let bytes = norito::to_bytes(sync).map_err(|_| {
        GovernancePublishError::other("encode fenced privacy authenticated head cache")
    })?;
    if bytes.len() > GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_MAX_BYTES {
        return Err(GovernancePublishError::other(
            "fenced privacy authenticated head cache exceeds its byte limit",
        ));
    }
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)?;
    write_atomic(&root_guard, &fenced_privacy_head_sync_path(root), &bytes)?;
    Ok(())
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
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &[0xA5; 32])
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
    use std::{
        collections::BTreeMap,
        fs, io,
        panic::{AssertUnwindSafe, catch_unwind},
        path::{Path, PathBuf},
        sync::{
            Arc, Condvar, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use iroha_data_model::sorafs::transparency::{
        MODERATION_PRIVACY_AGGREGATE_VERSION_V1, MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
        ModerationLedgerMetadataV1, ModerationPrivacyAggregateMetricV1,
        ModerationPrivacyAggregateV1, ModerationPrivacyModeV1, ModerationPrivacyNoiseSourceV1,
        ModerationPrivacyParametersV1, ModerationPrivacyThresholdPrfCommitmentV1,
    };
    use norito::codec::Encode;
    use sorafs_manifest::PorReportIsoWeek;
    use sorafs_manifest::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1, XorQuantity,
    };
    use sorafs_manifest::por::{
        POR_CHALLENGE_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PorChallengeV1,
        derive_challenge_id, derive_challenge_seed,
    };
    use sorafs_manifest::repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GC_AUDIT_SIGNER_V1, GcAuditEventV1,
        GcAuditPayloadV1, SorafsAuditHeaderV1, gc_audit_payload_digest_v1,
    };
    use sorafs_manifest::{
        GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogPayloadV1,
        MODERATION_LEDGER_PUBLICATION_VERSION_V1, REPUTATION_PROVIDER_INPUT_VERSION_V1,
        REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationWeightsV1,
        SIGNED_REPUTATION_SNAPSHOT_VERSION_V1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        SORAFS_RECONCILIATION_REPORT_VERSION_V1, SignedReputationSnapshotV1,
        SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
        SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
        SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
        SoraFsModerationVoteCountsV1, SorafsReconciliationReportV1, build_reputation_snapshot,
        validate_governance_dag_head_against_chain_v1,
    };
    use tempfile::TempDir;

    use super::*;

    fn read_publication_state_fixture(root: &Path) -> JsonValue {
        json::from_slice(
            &fs::read(root.join(GOVERNANCE_PUBLICATION_STATE_FILE))
                .expect("read authoritative governance publication state"),
        )
        .expect("decode authoritative governance publication state")
    }

    fn read_publication_section_fixture(root: &Path, section: &str) -> JsonValue {
        read_publication_state_fixture(root)
            .get(section)
            .cloned()
            .unwrap_or_else(|| panic!("publication state section `{section}`"))
    }

    fn published_source_paths_fixture(root: &Path, payload_kind: &str) -> Vec<(PathBuf, PathBuf)> {
        read_publication_section_fixture(root, "publish_index")
            .get("entries")
            .and_then(JsonValue::as_array)
            .expect("publication entries")
            .iter()
            .filter(|entry| {
                entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            })
            .map(|entry| {
                let encoded = entry
                    .get("encoded_path")
                    .and_then(JsonValue::as_str)
                    .expect("encoded source path");
                let json = entry
                    .get("json_path")
                    .and_then(JsonValue::as_str)
                    .expect("JSON source path");
                (root.join(encoded), root.join(json))
            })
            .collect()
    }

    fn only_published_source_paths(root: &Path, payload_kind: &str) -> (PathBuf, PathBuf) {
        let paths = published_source_paths_fixture(root, payload_kind);
        assert_eq!(paths.len(), 1, "expected one `{payload_kind}` publication");
        paths.into_iter().next().expect("one publication path")
    }

    #[test]
    fn runtime_dag_decode_allocation_budget_is_scaled_and_absolutely_capped() {
        assert_eq!(
            runtime_dag_decode_allocation_limit(1),
            GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1
        );
        let cap_input = GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
            / GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1;
        assert_eq!(
            runtime_dag_decode_allocation_limit(cap_input),
            GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
        );
        assert_eq!(
            runtime_dag_decode_allocation_limit(cap_input.saturating_add(1)),
            GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
        );
        assert_eq!(
            runtime_dag_decode_allocation_limit(usize::MAX),
            GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
        );
    }

    // Keep one target-gated assertion for every ABI branch. Overlapping branches
    // fail with duplicate definitions; missing branches fail to resolve the flag.
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
    #[test]
    fn linux_directory_open_flags_match_low_flag_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
        assert_eq!(platform_directory_only_flag(), 0x4000);
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
    #[test]
    fn linux_directory_open_flags_match_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
        assert_eq!(platform_directory_only_flag(), 0x10000);
    }

    #[cfg(all(
        target_os = "android",
        any(target_arch = "aarch64", target_arch = "arm")
    ))]
    #[test]
    fn android_arm_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
        assert_eq!(platform_directory_only_flag(), 0x4000);
    }

    #[cfg(all(
        target_os = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[test]
    fn android_x86_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
        assert_eq!(platform_directory_only_flag(), 0x10000);
    }

    #[cfg(all(target_os = "android", target_arch = "riscv64"))]
    #[test]
    fn android_riscv64_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x400000);
        assert_eq!(platform_directory_only_flag(), 0x200000);
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "riscv32", target_arch = "riscv64")
    ))]
    #[test]
    fn linux_riscv_directory_open_flags_remain_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
        assert_eq!(platform_directory_only_flag(), 0x10000);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn apple_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0010_0000);
    }

    #[cfg(target_os = "freebsd")]
    #[test]
    fn freebsd_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0002_0000);
    }

    #[cfg(target_os = "dragonfly")]
    #[test]
    fn dragonfly_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0800_0000);
    }

    #[cfg(target_os = "openbsd")]
    #[test]
    fn openbsd_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0002_0000);
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn netbsd_directory_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0020_0000);
    }

    const TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST: [u8; 32] = [0x71; 32];
    const TEST_RUNTIME_DAG_STORE_POLICY_DIGEST: [u8; 32] = [0x73; 32];

    fn test_runtime_dag_signer_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST)
    }

    #[derive(Debug)]
    struct TestRuntimeDagCheckpointStoreState {
        records: [Option<GovernanceDagSealedStateRecord>; 4],
        generation_floors: [u64; 4],
    }

    impl Default for TestRuntimeDagCheckpointStoreState {
        fn default() -> Self {
            Self {
                records: std::array::from_fn(|_| None),
                generation_floors: [0; 4],
            }
        }
    }

    #[derive(Debug, Default)]
    struct TestRuntimeDagCheckpointStore {
        state: Mutex<TestRuntimeDagCheckpointStoreState>,
        fail_after_next_intent_cas: AtomicBool,
        fail_before_next_checkpoint_cas: AtomicBool,
        fail_after_next_checkpoint_cas: AtomicBool,
    }

    impl TestRuntimeDagCheckpointStore {
        const HANDLE: &'static str = "kms:governance-dag:producer-checkpoint-primary";

        const fn qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
            GovernanceDagRuntimeProviderQualificationV1::new(
                1,
                TEST_RUNTIME_DAG_STORE_POLICY_DIGEST,
            )
        }

        const fn slot_index(slot: GovernanceDagSealedStateSlot) -> usize {
            match slot {
                GovernanceDagSealedStateSlot::Checkpoint => 0,
                GovernanceDagSealedStateSlot::PublishIntent => 1,
                GovernanceDagSealedStateSlot::ProducerCheckpoint => 2,
                GovernanceDagSealedStateSlot::ProducerPublishIntent => 3,
            }
        }
    }

    impl GovernanceDagSealedCheckpointStore for TestRuntimeDagCheckpointStore {
        fn handle(&self) -> &str {
            Self::HANDLE
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(Self::qualification())
        }

        fn load(
            &self,
            slot: GovernanceDagSealedStateSlot,
        ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
            let state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            Ok(state.records[Self::slot_index(slot)].clone())
        }

        fn compare_and_swap(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: Option<[u8; 32]>,
            next: GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
                && self
                    .fail_before_next_checkpoint_cas
                    .swap(false, Ordering::SeqCst)
            {
                return Err("checkpoint CAS refused before install".to_owned());
            }
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision) != expected_revision {
                return Err("compare-and-swap conflict".to_owned());
            }
            if next.generation <= state.generation_floors[index]
                || next.payload.is_empty()
                || !next.has_valid_revision(slot)
            {
                return Err("invalid or non-monotonic record".to_owned());
            }
            state.generation_floors[index] = next.generation;
            state.records[index] = Some(next);
            drop(state);
            if slot == GovernanceDagSealedStateSlot::ProducerPublishIntent
                && self
                    .fail_after_next_intent_cas
                    .swap(false, Ordering::SeqCst)
            {
                return Err("ambiguous intent CAS response".to_owned());
            }
            if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
                && self
                    .fail_after_next_checkpoint_cas
                    .swap(false, Ordering::SeqCst)
            {
                return Err("ambiguous checkpoint CAS response".to_owned());
            }
            Ok(())
        }

        fn delete(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: [u8; 32],
        ) -> Result<(), String> {
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision)
                != Some(expected_revision)
            {
                return Err("delete conflict".to_owned());
            }
            state.records[index] = None;
            Ok(())
        }
    }

    const TEST_FENCED_PUBLISHER_HANDLE: &str = "hsm:governance:fenced-privacy-primary";
    const TEST_FENCED_PUBLISHER_POLICY_DIGEST: [u8; 32] = [0x72; 32];
    const TEST_FENCED_HEAD_READER_HANDLE: &str = TEST_FENCED_PUBLISHER_HANDLE;
    const TEST_FENCED_HEAD_READER_POLICY_DIGEST: [u8; 32] = TEST_FENCED_PUBLISHER_POLICY_DIGEST;
    const TEST_PRIVACY_QUERY_ID: [u8; 32] = [0x91; 32];
    const TEST_PRIVACY_CYCLE_START: u64 = 1_800_000_000;
    const TEST_PRIVACY_CYCLE_END: u64 = 1_800_604_800;

    fn test_fenced_publisher_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_FENCED_PUBLISHER_POLICY_DIGEST)
    }

    fn test_fenced_head_reader_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_FENCED_HEAD_READER_POLICY_DIGEST)
    }

    #[derive(Debug, Default)]
    struct TestFencedPublisherState {
        head: Option<FencedTransparencyTargetHeadV1>,
        fencing_floor: u64,
        publications:
            BTreeMap<([u8; 32], [u8; 16]), ([u8; 32], [u8; 32], FencedTransparencyTargetHeadV1)>,
        receipts: BTreeMap<
            [u8; 32],
            (
                FencedPrivacyPublicationRequestV1,
                FencedPrivacyPublicationReceiptV1,
            ),
        >,
        history: Vec<FencedTransparencyTargetHeadV1>,
        append_count: usize,
    }

    #[derive(Debug, Default)]
    struct TestFencedPublisherPause {
        reached: bool,
        released: bool,
    }

    #[derive(Debug)]
    struct TestFencedTransparencyPublisher {
        state: Mutex<TestFencedPublisherState>,
        pause_token: AtomicU64,
        pause: Mutex<TestFencedPublisherPause>,
        pause_changed: Condvar,
        substitute_receipt: AtomicBool,
    }

    #[derive(Debug)]
    struct TestFencedTransparencyHeadReader {
        target: Arc<TestFencedTransparencyPublisher>,
        handle: String,
        revision: AtomicU64,
        policy_digest: [u8; 32],
        head_override: Mutex<Option<Option<FencedTransparencyTargetHeadV1>>>,
        fail_read: AtomicBool,
    }

    impl TestFencedTransparencyPublisher {
        fn new() -> Self {
            Self {
                state: Mutex::new(TestFencedPublisherState::default()),
                pause_token: AtomicU64::new(0),
                pause: Mutex::new(TestFencedPublisherPause::default()),
                pause_changed: Condvar::new(),
                substitute_receipt: AtomicBool::new(false),
            }
        }

        fn pause_fencing_token(&self, fencing_token: u64) {
            self.pause_token.store(fencing_token, Ordering::Release);
            *self.pause.lock().expect("fenced publisher pause") =
                TestFencedPublisherPause::default();
        }

        fn wait_until_paused(&self) {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut pause = self.pause.lock().expect("fenced publisher pause");
            while !pause.reached {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .expect("fenced publisher reached pause deadline");
                let (next, wait) = self
                    .pause_changed
                    .wait_timeout(pause, remaining)
                    .expect("fenced publisher pause");
                pause = next;
                assert!(!wait.timed_out(), "fenced publisher did not pause");
            }
        }

        fn release_paused(&self) {
            self.pause.lock().expect("fenced publisher pause").released = true;
            self.pause_changed.notify_all();
        }

        fn set_substitute_receipt(&self, substitute: bool) {
            self.substitute_receipt.store(substitute, Ordering::Release);
        }

        fn append_count(&self) -> usize {
            self.state
                .lock()
                .expect("fenced publisher state")
                .append_count
        }

        fn head(&self) -> Option<FencedTransparencyTargetHeadV1> {
            self.state.lock().expect("fenced publisher state").head
        }

        fn pause_if_requested(&self, fencing_token: u64) {
            if self.pause_token.load(Ordering::Acquire) != fencing_token {
                return;
            }
            let mut pause = self.pause.lock().expect("fenced publisher pause");
            pause.reached = true;
            self.pause_changed.notify_all();
            while !pause.released {
                pause = self
                    .pause_changed
                    .wait(pause)
                    .expect("fenced publisher pause");
            }
            self.pause_token.store(0, Ordering::Release);
        }
    }

    impl TestFencedTransparencyHeadReader {
        fn new(target: Arc<TestFencedTransparencyPublisher>) -> Self {
            Self {
                target,
                handle: TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
                revision: AtomicU64::new(1),
                policy_digest: TEST_FENCED_HEAD_READER_POLICY_DIGEST,
                head_override: Mutex::new(None),
                fail_read: AtomicBool::new(false),
            }
        }

        fn with_handle(
            target: Arc<TestFencedTransparencyPublisher>,
            handle: impl Into<String>,
        ) -> Self {
            Self {
                handle: handle.into(),
                ..Self::new(target)
            }
        }

        fn with_binding(
            target: Arc<TestFencedTransparencyPublisher>,
            handle: impl Into<String>,
            revision: u64,
            policy_digest: [u8; 32],
        ) -> Self {
            Self {
                target,
                handle: handle.into(),
                revision: AtomicU64::new(revision),
                policy_digest,
                head_override: Mutex::new(None),
                fail_read: AtomicBool::new(false),
            }
        }

        fn set_revision(&self, revision: u64) {
            self.revision.store(revision, Ordering::Release);
        }

        fn override_head(&self, head: Option<FencedTransparencyTargetHeadV1>) {
            *self.head_override.lock().expect("head reader override") = Some(head);
        }

        fn set_fail_read(&self, fail: bool) {
            self.fail_read.store(fail, Ordering::Release);
        }
    }

    impl FencedTransparencyPublisherV1 for TestFencedTransparencyPublisher {
        fn handle(&self) -> &str {
            TEST_FENCED_PUBLISHER_HANDLE
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(test_fenced_publisher_qualification())
        }

        fn compare_and_append_privacy(
            &self,
            request: &FencedPrivacyPublicationRequestV1,
        ) -> Result<FencedPrivacyPublicationReceiptV1, FencedTransparencyPublishErrorV1> {
            request.validate()?;
            self.pause_if_requested(request.fencing_token());
            let mut state = self
                .state
                .lock()
                .map_err(|_| FencedTransparencyPublishErrorV1::UnqualifiedProvider)?;
            if let Some((retained_request, receipt)) = state.receipts.get(&request.request_digest())
            {
                return if retained_request == request {
                    Ok(receipt.clone())
                } else {
                    Err(FencedTransparencyPublishErrorV1::Rejected)
                };
            }
            if let Some((idempotency_digest, payload_digest, included_head)) = state
                .publications
                .get(&request.publication_scope())
                .copied()
            {
                if idempotency_digest != request.publication_idempotency_digest()
                    || payload_digest != request.payload_digest()
                {
                    return Err(FencedTransparencyPublishErrorV1::PublicationConflict);
                }
                let readback_head = state
                    .head
                    .ok_or(FencedTransparencyPublishErrorV1::InvalidReceipt)?;
                let receipt = FencedPrivacyPublicationReceiptV1::from_verified_existing(
                    request,
                    TEST_FENCED_PUBLISHER_HANDLE,
                    test_fenced_publisher_qualification(),
                    included_head,
                    readback_head,
                )?;
                state
                    .receipts
                    .insert(request.request_digest(), (request.clone(), receipt.clone()));
                return Ok(receipt);
            }
            if request.fencing_token() <= state.fencing_floor {
                return Err(FencedTransparencyPublishErrorV1::StaleFencingToken);
            }
            if request.expected_authoritative_head() != state.head {
                return Err(FencedTransparencyPublishErrorV1::CompareConflict);
            }
            let receipt = FencedPrivacyPublicationReceiptV1::from_verified_append(
                request,
                TEST_FENCED_PUBLISHER_HANDLE,
                test_fenced_publisher_qualification(),
            )?;
            state.head = Some(receipt.included_head());
            state.fencing_floor = request.fencing_token();
            state.append_count += 1;
            state.history.push(receipt.included_head());
            state.publications.insert(
                request.publication_scope(),
                (
                    request.publication_idempotency_digest(),
                    request.payload_digest(),
                    receipt.included_head(),
                ),
            );
            state
                .receipts
                .insert(request.request_digest(), (request.clone(), receipt.clone()));
            if self.substitute_receipt.load(Ordering::Acquire) {
                let mut substituted = receipt;
                substituted.head_inclusion_digest[0] ^= 0x80;
                Ok(substituted)
            } else {
                Ok(receipt)
            }
        }
    }

    impl FencedTransparencyAuthoritativeHeadReaderV1 for TestFencedTransparencyHeadReader {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                self.revision.load(Ordering::Acquire),
                self.policy_digest,
            ))
        }

        fn read_authoritative_head_with_ancestry(
            &self,
            required_ancestors: &[FencedTransparencyTargetHeadV1],
            required_publications: &[FencedTransparencyPublicationInclusionV1],
        ) -> Result<FencedTransparencyHeadAncestryProofV1, String> {
            if self.fail_read.load(Ordering::Acquire) {
                return Err("redacted test read failure".to_owned());
            }
            let observed =
                if let Some(head) = *self.head_override.lock().expect("head reader override") {
                    head
                } else {
                    self.target.head()
                };
            let state = self
                .target
                .state
                .lock()
                .map_err(|_| "redacted test target failure".to_owned())?;
            if observed != state.head {
                return Err("redacted test ancestry failure".to_owned());
            }
            let current_index = observed
                .map(|head| {
                    state
                        .history
                        .iter()
                        .position(|candidate| *candidate == head)
                        .ok_or_else(|| "redacted test current-head proof failure".to_owned())
                })
                .transpose()?;
            for ancestor in required_ancestors {
                let ancestor_index = state
                    .history
                    .iter()
                    .position(|candidate| candidate == ancestor)
                    .ok_or_else(|| "redacted test ancestry failure".to_owned())?;
                if current_index.is_none_or(|current| ancestor_index > current) {
                    return Err("redacted test ancestry failure".to_owned());
                }
            }
            for publication in required_publications {
                if !state.publications.values().any(
                    |(publication_idempotency_digest, payload_digest, included_head)| {
                        *publication_idempotency_digest
                            == publication.publication_idempotency_digest()
                            && *payload_digest == publication.payload_digest()
                            && *included_head == publication.included_head()
                    },
                ) {
                    return Err("redacted test publication inclusion failure".to_owned());
                }
            }
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"sorafs.test.fenced-head-ancestry-proof.v1");
            crate::fenced_privacy_digest_head(&mut hasher, observed);
            for ancestor in required_ancestors {
                crate::fenced_privacy_digest_head(&mut hasher, Some(*ancestor));
            }
            for publication in required_publications {
                hasher.update(&publication.publication_idempotency_digest());
                hasher.update(&publication.payload_digest());
                crate::fenced_privacy_digest_head(&mut hasher, Some(publication.included_head()));
            }
            FencedTransparencyHeadAncestryProofV1::try_new(
                observed,
                required_ancestors.to_vec(),
                required_publications.to_vec(),
                *hasher.finalize().as_bytes(),
            )
            .map_err(|_| "redacted test ancestry proof encoding failure".to_owned())
        }
    }

    fn qualified_test_fenced_publisher(
        provider: Arc<TestFencedTransparencyPublisher>,
    ) -> QualifiedFencedTransparencyPublisherV1 {
        let provider: Arc<dyn FencedTransparencyPublisherV1> = provider;
        QualifiedFencedTransparencyPublisherV1::try_new(
            TEST_FENCED_PUBLISHER_HANDLE.to_owned(),
            test_fenced_publisher_qualification(),
            provider,
        )
        .expect("qualify test fused publisher")
    }

    fn qualified_test_fenced_head_reader(
        reader: Arc<TestFencedTransparencyHeadReader>,
    ) -> QualifiedFencedTransparencyHeadReaderV1 {
        let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = reader;
        QualifiedFencedTransparencyHeadReaderV1::try_new(
            TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
            test_fenced_head_reader_qualification(),
            reader,
        )
        .expect("qualify test fused head reader")
    }

    fn test_fenced_head_reader(
        provider: Arc<TestFencedTransparencyPublisher>,
    ) -> Arc<TestFencedTransparencyHeadReader> {
        Arc::new(TestFencedTransparencyHeadReader::new(provider))
    }

    fn xor(value: &str) -> sorafs_manifest::deal::XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    #[derive(Clone, Copy)]
    struct SamplePrivacyReleaseSpec {
        query_id: [u8; 32],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        release_sequence: u64,
        release_record_digest: [u8; 32],
    }

    impl SamplePrivacyReleaseSpec {
        const fn primary() -> Self {
            Self {
                query_id: TEST_PRIVACY_QUERY_ID,
                cycle_start_unix: TEST_PRIVACY_CYCLE_START,
                cycle_end_unix: TEST_PRIVACY_CYCLE_END,
                release_sequence: 1,
                release_record_digest: [0x98; 32],
            }
        }

        const fn next() -> Self {
            let cycle_seconds = TEST_PRIVACY_CYCLE_END - TEST_PRIVACY_CYCLE_START;
            Self {
                query_id: TEST_PRIVACY_QUERY_ID,
                cycle_start_unix: TEST_PRIVACY_CYCLE_END,
                cycle_end_unix: TEST_PRIVACY_CYCLE_END + cycle_seconds,
                release_sequence: 2,
                release_record_digest: [0xA8; 32],
            }
        }
    }

    #[derive(Clone, Copy)]
    struct SampleFinalizedAnchorSpec {
        sequence: u64,
        release_id: [u8; 16],
        record_digest: [u8; 32],
        latest_publication_block_hash: Option<[u8; 32]>,
    }

    fn sample_privacy_publication_for(
        spec: SamplePrivacyReleaseSpec,
    ) -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
        let cycle_id = crate::privacy_aggregate_cycle_id(
            spec.query_id,
            spec.cycle_start_unix,
            spec.cycle_end_unix,
        );
        let aggregate = ModerationPrivacyAggregateV1 {
            version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
            aggregate_id: format!("sfm4c-fenced-publication-{}", spec.release_sequence),
            window_start_unix: spec.cycle_start_unix,
            window_end_unix: spec.cycle_end_unix,
            generated_at_unix: spec.cycle_end_unix,
            population_label: "fenced-population".to_owned(),
            population_digest: [0x92; 32],
            source_commitment: [0x91; 32],
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_numerator: Some(4),
                epsilon_denominator: Some(5),
                delta_ppb: Some(0),
                per_subject_metric_cap: Some(1),
                suppression_threshold: Some(25),
            },
            noise_source: ModerationPrivacyNoiseSourceV1::ThresholdPrf(
                ModerationPrivacyThresholdPrfCommitmentV1 {
                    commitment: [0x93; 32],
                },
            ),
            metrics: vec![ModerationPrivacyAggregateMetricV1 {
                key: "moderation_actions".to_owned(),
                value: 7,
                unit: "count".to_owned(),
            }],
            policy_digest: [0x94; 32],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "publisher".to_owned(),
                value: "fenced-runtime".to_owned(),
            }],
        };
        let publication = crate::NodeHandle::build_privacy_aggregate_publication(
            cycle_id,
            spec.cycle_start_unix,
            spec.cycle_end_unix,
            spec.cycle_end_unix,
            None,
            vec![aggregate],
        )
        .expect("build privacy publication");
        let encoded = norito::to_bytes(&publication).expect("encode privacy publication");
        (publication, encoded)
    }

    fn sample_privacy_publication() -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
        sample_privacy_publication_for(SamplePrivacyReleaseSpec::primary())
    }

    fn sample_privacy_authorization_for(
        spec: SamplePrivacyReleaseSpec,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        fencing_token: u64,
        finalized_anchor: Option<SampleFinalizedAnchorSpec>,
    ) -> PrivacyPublicationAuthorizationV1 {
        let cycle_seconds = spec.cycle_end_unix - spec.cycle_start_unix;
        let window = crate::PrivacyAggregateCycleWindow {
            cycle_start_unix: spec.cycle_start_unix,
            cycle_end_unix: spec.cycle_end_unix,
            due_at_unix: spec.cycle_end_unix,
        };
        let scope =
            crate::TransparencyLeaderLeaseScopeV1::try_new(spec.query_id, window, [0x95; 32])
                .expect("privacy leader scope");
        assert_eq!(scope.cycle_id(), publication.block.cycle_id);
        let lease_binding = crate::TransparencyRuntimeProviderBindingV1::try_new(
            "hsm:transparency:leader-primary",
            1,
            [0x96; 32],
        )
        .expect("privacy leader provider binding");
        let mut lease_id = [0x97; 32];
        lease_id[..8].copy_from_slice(&fencing_token.to_le_bytes());
        let lease = crate::TransparencyLeaderLeaseGrantV1::try_new(
            lease_id,
            scope,
            fencing_token,
            spec.cycle_end_unix,
            spec.cycle_end_unix + 300,
            lease_binding,
        )
        .expect("privacy leader lease");
        let payload_digest = *blake3::hash(encoded).as_bytes();
        let block_hash = publication
            .block
            .block_hash()
            .expect("privacy publication block hash");
        let release = crate::transparency::PrivacyReleaseRecordV1 {
            sequence: spec.release_sequence,
            release_id: publication.block.cycle_id,
            query_id: spec.query_id,
            first_cycle_start_unix: spec.cycle_start_unix,
            cycle_seconds,
            publish_delay_seconds: 0,
            cycle_start_unix: spec.cycle_start_unix,
            cycle_end_unix: spec.cycle_end_unix,
            due_at_unix: spec.cycle_end_unix,
            private_source_digest: [0x99; 32],
            policy_digest: [0x94; 32],
            population_inventory_digest: [0x9A; 32],
            metric_schema_digest: [0x9B; 32],
            privacy: publication.privacy_aggregates[0].privacy,
            prf_request_binding: Some([0x9C; 32]),
            prf_commitment: Some([0x93; 32]),
            budget_charge_digest: None,
            publication_payload_digest: Some(payload_digest),
            published_aggregate_inventory_digest: Some([0x9D; 32]),
            previous_publication_block_hash: None,
            publication_block_hash: Some(block_hash),
            status: crate::transparency::PrivacyReleaseStatusV1::Published,
            previous_record_digest: None,
            record_digest: spec.release_record_digest,
        };
        let finalized_anchor = finalized_anchor.unwrap_or(SampleFinalizedAnchorSpec {
            sequence: spec.release_sequence,
            release_id: publication.block.cycle_id,
            record_digest: spec.release_record_digest,
            latest_publication_block_hash: Some(block_hash),
        });
        let anchor = crate::PrivacyReleaseAnchorHeadV1::try_from_parts(
            spec.query_id,
            finalized_anchor.sequence,
            finalized_anchor.release_id,
            finalized_anchor.record_digest,
            finalized_anchor.latest_publication_block_hash,
        )
        .expect("privacy finalized anchor");
        PrivacyPublicationAuthorizationV1::try_new(&lease, anchor, &release, payload_digest)
            .expect("privacy publication authorization")
    }

    fn sample_privacy_authorization(
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        fencing_token: u64,
    ) -> PrivacyPublicationAuthorizationV1 {
        sample_privacy_authorization_for(
            SamplePrivacyReleaseSpec::primary(),
            publication,
            encoded,
            fencing_token,
            None,
        )
    }

    fn sample_fenced_request(
        fencing_token: u64,
        expected_head: Option<FencedTransparencyTargetHeadV1>,
    ) -> FencedPrivacyPublicationRequestV1 {
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, fencing_token);
        FencedPrivacyPublicationRequestV1::try_new(
            authorization,
            &publication,
            encoded,
            expected_head,
            expected_head.map_or(0, |head| head.fencing_floor()),
        )
        .expect("fenced privacy request")
    }

    fn assert_empty_publication_authority(root: &Path) {
        let state = read_publication_state_fixture(root);
        assert_eq!(state.get("generation").and_then(JsonValue::as_u64), Some(0));
        assert_eq!(
            state
                .get("publish_index")
                .and_then(|index| index.get("entries"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            state
                .get("car_queue")
                .and_then(|queue| queue.get("segments"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            fs::read(root.join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE))
                .expect("read publication initialization marker"),
            GOVERNANCE_PUBLICATION_INITIALIZED_BODY
        );
    }

    fn assert_no_privacy_publication_side_effects(root: &Path) {
        assert!(
            !root.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists()
                && !root.join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
            "privacy artifacts must remain absent"
        );
        assert_empty_publication_authority(root);
        assert!(
            !root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE).exists(),
            "runtime DAG index must remain absent"
        );
        assert!(
            !fenced_privacy_head_cache_path(root).exists(),
            "authoritative-head cache must remain absent"
        );
    }

    fn assert_fenced_privacy_pending_logically_cleared(root: &Path) {
        assert!(
            fenced_privacy_pending_path(root).is_file(),
            "logical deletion retains a typed tombstone journal"
        );
        assert_eq!(
            read_fenced_privacy_pending_request(root).expect("read pending tombstone"),
            None
        );
    }

    struct CanonicalTempDir {
        _inner: TempDir,
        path: PathBuf,
    }

    impl CanonicalTempDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    fn tempdir() -> std::io::Result<CanonicalTempDir> {
        let inner = tempfile::tempdir()?;
        let path = inner.path().canonicalize()?;
        Ok(CanonicalTempDir {
            _inner: inner,
            path,
        })
    }

    fn canonical_temp_path(dir: &CanonicalTempDir) -> PathBuf {
        dir.path().to_path_buf()
    }

    fn sample_settlement() -> (DealSettlementV1, Vec<u8>) {
        let deal_id = [0xAB; 32];
        let provider_id = [0xCD; 32];
        let client_id = [0xEF; 32];
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id,
            terms_digest: [0xA4; 32],
            provider_id,
            client_id,
            deal_start_epoch: 1_699_999_990,
            deal_end_epoch: 1_699_999_999,
            settlement_window_epochs: 10,
            window_start_epoch: 1_699_999_990,
            window_end_epoch: 1_700_000_000,
            provider_accrual: xor("0.5"),
            client_liability: xor("0.5"),
            micropayment_credit_generated: XorQuantity::zero(),
            micropayment_credit_applied: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor("0.5"),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor("1"),
            bond_locked: XorQuantity::zero(),
            bond_slashed: XorQuantity::zero(),
            bond_released: xor("1"),
            window_expected_charge: xor("0.5"),
            window_micropayment_generated: XorQuantity::zero(),
            window_micropayment_applied: XorQuantity::zero(),
            window_client_debit: xor("0.5"),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: xor("1"),
            captured_at: 1_700_000_000,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id,
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_000_000,
            audit_notes: None,
        };
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        let encoded = norito::to_bytes(&settlement).expect("encode settlement");
        (settlement, encoded)
    }

    fn sample_por_challenge_publication() -> (PorChallengePublicationV1, Vec<u8>) {
        let manifest_digest = [0x41; 32];
        let provider_id = [0x42; 32];
        let epoch_id = 7;
        let drand_round = 11;
        let drand_randomness = [0x43; 32];
        let seed = derive_challenge_seed(&drand_randomness, None, &manifest_digest, epoch_id);
        let challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id: derive_challenge_id(
                &seed,
                &manifest_digest,
                &provider_id,
                epoch_id,
                drand_round,
            ),
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0x44; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
            vrf_output: None,
            vrf_proof: None,
            forced: true,
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            seed,
            sample_tier: 1,
            sample_count: 3,
            sample_indices: vec![5, 5, 9],
            issued_at: 1_800_000_000,
            deadline_at: 1_800_000_900,
        };
        let publication =
            PorChallengePublicationV1::try_new(challenge, 1).expect("challenge publication");
        let encoded = norito::to_bytes(&publication).expect("encode challenge publication");
        (publication, encoded)
    }

    fn sample_por_weekly_report() -> (PorWeeklyReportV1, Vec<u8>) {
        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle: PorReportIsoWeek {
                year: 2026,
                week: 30,
            },
            generated_at: 1_800_604_800,
            challenges_total: 3,
            challenges_verified: 2,
            challenges_failed: 1,
            forced_challenges: 1,
            repairs_enqueued: 1,
            repairs_completed: 1,
            mean_latency_ms: Some(75),
            p95_latency_ms: Some(120),
            slashing_events: Vec::new(),
            providers_missing_vrf: vec![[0x42; 32]],
            top_offenders: Vec::new(),
            notes: None,
        };
        report.validate().expect("weekly report");
        let encoded = norito::to_bytes(&report).expect("encode weekly report");
        (report, encoded)
    }

    fn sample_reputation_snapshot() -> (SignedReputationSnapshotV1, Vec<u8>) {
        let metrics = ReputationProviderMetricsV1 {
            version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
            por_success_bps: 9_800,
            pdp_success_bps: 9_700,
            potr_success_bps: 9_600,
            latency_health_bps: 9_000,
            dispute_rate_bps: 100,
            token_violation_rate_bps: 50,
            repair_breach_rate_bps: 0,
        };
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics,
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let inputs = vec![input];
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &inputs,
            None,
        )
        .expect("reputation snapshot");
        let scoring_evidence = ReputationScoringEvidenceV1 {
            version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
            provider_inputs: inputs,
            trust_edges: Vec::new(),
        };
        let mut envelope = SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: [0xA5; 32],
            snapshot,
            scoring_evidence_digest: scoring_evidence
                .canonical_digest()
                .expect("scoring evidence digest"),
            scoring_evidence,
            signatures: Vec::new(),
        };
        let signing_key = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("derive reputation signing key");
        let signature = IrohaSignature::try_new(
            signing_key.private_key(),
            &envelope.signing_digest().expect("signing digest"),
        )
        .expect("sign reputation snapshot");
        envelope.signatures.push(ReputationSnapshotSignatureV1 {
            signer_id: "council-1".to_owned(),
            signature: signature
                .payload()
                .try_into()
                .expect("Ed25519 signature is fixed-width"),
        });
        let encoded = envelope
            .canonical_bytes()
            .expect("encode signed reputation snapshot");
        (envelope, encoded)
    }

    fn sample_moderation_ballot_event() -> (SoraFsModerationBallotGovernanceEventV1, Vec<u8>) {
        let event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 6,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_030_000,
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 2,
            challenge_count: 0,
            tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
                case_id: "case-42".to_string(),
                round_id: "round-1".to_string(),
                counts: SoraFsModerationVoteCountsV1 {
                    uphold: 2,
                    overturn: 0,
                    modify: 0,
                    escalate: 0,
                },
                votes_total: 2,
                quorum: 2,
                winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
                contested: false,
                tallied_at_unix_ms: 1_800_000_030_000,
            }),
            challenge: None,
        };
        let encoded = norito::to_bytes(&event).expect("encode moderation ballot event");
        (event, encoded)
    }

    fn sample_transparency_ledger_publication() -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_LEDGER_ENTRY_VERSION_V1, ModerationLedgerEntryKindV1,
            ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
        };

        let cycle_id = *b"cycle-2026-wk-03";
        let entries = [
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x32; 16],
                sequence: 2,
                occurred_at_unix: 1_800_000_032,
                kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
                subject: "gar-receipt-32".to_string(),
                subject_digest: [0x32; 32],
                payload_digest: [0x33; 32],
                summary_digest: [0x34; 32],
                policy_digest: Some([0x35; 32]),
                evidence_uris: vec!["sora://transparency/32".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "gar".to_string(),
                }],
            },
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x31; 16],
                sequence: 1,
                occurred_at_unix: 1_800_000_031,
                kind: ModerationLedgerEntryKindV1::ModerationAction,
                subject: "moderation-case-31".to_string(),
                subject_digest: [0x31; 32],
                payload_digest: [0x32; 32],
                summary_digest: [0x33; 32],
                policy_digest: Some([0x34; 32]),
                evidence_uris: vec!["sora://transparency/31".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "moderation".to_string(),
                }],
            },
        ];
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
            &entries,
        )
        .expect("transparency ledger publication");
        let encoded =
            norito::to_bytes(&publication).expect("encode transparency ledger publication");
        (publication, encoded)
    }

    fn sample_proof_token_issuance() -> (ProofTokenIssuanceV1, Vec<u8>) {
        let issuance = ProofTokenIssuanceV1 {
            version: PROOF_TOKEN_ISSUANCE_VERSION_V1,
            token_id: [0x61; 16],
            issued_at_unix: 1_800_000_030,
            expires_at_unix: Some(1_800_086_430),
            moderation_action_code: 2,
            signer_key: [0x62; 32],
            token_blake3: [0x63; 32],
            blinded_digest: [0x64; 32],
            entry_ids: vec!["denylist/global".to_string(), "gar/policy/42".to_string()],
            evidence_digest: Some([0x65; 32]),
            policy_digest: Some([0x66; 32]),
            metadata: Vec::new(),
        };
        let encoded = norito::to_bytes(&issuance).expect("encode proof-token issuance");
        (issuance, encoded)
    }

    fn sample_appeal_finance_report() -> (SoraFsAppealFinanceReportV1, Vec<u8>) {
        let report = SoraFsAppealFinanceReportV1 {
            version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
            report_id: [0x42; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_031_000,
            appeal_finance_config_version: "baseline-v1".to_string(),
            evidence_bundle_digest: Some([0xA7; 32]),
            outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
            deposit_xor: xor("420"),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: xor("420"),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: xor("50"),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: xor("0"),
            },
            panel_size: 3,
            panel_reward_total_xor: xor("85"),
            rewards_paid_total_xor: xor("60"),
            rewards_forfeited_treasury_xor: xor("25"),
            juror_payouts: vec![
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-a".to_string(),
                    stipend_xor: xor("25"),
                    bonus_xor: xor("5"),
                    total_xor: xor("30"),
                },
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-b".to_string(),
                    stipend_xor: xor("25"),
                    bonus_xor: xor("5"),
                    total_xor: xor("30"),
                },
            ],
            no_show_juror_ids: vec!["juror-c".to_string()],
        };
        let encoded = norito::to_bytes(&report).expect("encode appeal finance report");
        (report, encoded)
    }

    fn sample_appeal_finance_weekly_rollup() -> (SoraFsAppealFinanceWeeklyRollupV1, Vec<u8>) {
        let (report, _) = sample_appeal_finance_report();
        let rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            PorReportIsoWeek {
                year: 2026,
                week: 26,
            },
            1_800_000_100_000,
            &[report],
        )
        .expect("appeal finance weekly rollup");
        let encoded = norito::to_bytes(&rollup).expect("encode appeal finance weekly rollup");
        (rollup, encoded)
    }

    fn sample_appeal_finance_settlement_receipt()
    -> (SoraFsAppealFinanceSettlementReceiptV1, Vec<u8>) {
        let receipt = SoraFsAppealFinanceSettlementReceiptV1 {
            version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x52; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_032_000,
            finalized_block_height: 42,
            finalized_block_hash: [0x43; 32],
            appeal_finance_config_version: "baseline-v1".to_string(),
            appeal_finance_policy_digest: [0x44; 32],
            outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
            escrow_id_hex: "11".repeat(32),
            payer_account: "payer-account".to_string(),
            destination_account: "escrow-account".to_string(),
            release_authority_account: Some("release-authority".to_string()),
            submitted_step: "drawdown_non_refund".to_string(),
            required_authority: "release-authority".to_string(),
            amount_xor: xor("420"),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "settled".to_string(),
            observed_lifecycle_status: "drawn_down".to_string(),
            observed_remaining_xor: xor("0"),
            deposit_xor: xor("420"),
            refund_xor: xor("0"),
            treasury_xor: xor("210"),
            held_xor: xor("210"),
            panel_size: 7,
            configured_signer_count: 1,
        };
        let encoded = norito::to_bytes(&receipt).expect("encode appeal finance settlement receipt");
        (receipt, encoded)
    }

    #[test]
    fn governance_car_queue_rejects_non_producible_pending_segments() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain governance root");
        let mut pending = JsonMap::new();
        pending.insert(
            "schema".into(),
            JsonValue::from(GOVERNANCE_CAR_SEGMENT_SCHEMA),
        );
        pending.insert("status".into(), JsonValue::from("pending"));

        let error = rebuild_car_queue(JsonMap::new(), vec![JsonValue::Object(pending)])
            .expect_err("pending CAR segment must fail closed");
        assert!(error.to_string().contains("non-producible"));
        assert!(!temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists());
        root_guard
            .revalidate()
            .expect("retained root remains valid");
    }

    fn write_car_segment_source_fixture_for_kind(
        root: &Path,
        payload_kind: &str,
        encoded: &[u8],
    ) -> PublishIndexEntryForCar {
        let json = br#"{"status":"ready"}"#;
        let encoded_blake3 = blake3::hash(encoded).to_hex().to_string();
        let json_blake3 = blake3::hash(json).to_hex().to_string();
        let (encoded_relative, json_relative) = governance_source_pair_relative_paths(
            payload_kind,
            u64::try_from(encoded.len()).expect("encoded length"),
            &encoded_blake3,
            u64::try_from(json.len()).expect("JSON length"),
            &json_blake3,
        )
        .expect("derive canonical source fixture paths");
        let encoded_path = root.join(&encoded_relative);
        let json_path = root.join(&json_relative);
        fs::create_dir_all(encoded_path.parent().expect("encoded source parent"))
            .expect("create CAR source directory");
        fs::write(&encoded_path, encoded).expect("write encoded CAR source");
        fs::write(&json_path, json).expect("write JSON CAR source");
        for (path, bytes) in [(&encoded_path, encoded), (&json_path, json.as_slice())] {
            let mut digest = blake3::hash(bytes).to_hex().to_string();
            digest.push('\n');
            fs::write(digest_sidecar_path_for(path), digest).expect("write CAR source sidecar");
        }
        PublishIndexEntryForCar {
            position: 0,
            newly_inserted: true,
            payload_kind: payload_kind.to_owned(),
            encoded_path: encoded_relative,
            json_path: json_relative,
            encoded_blake3,
            encoded_len: encoded.len(),
            json_blake3,
            json_len: json.len(),
        }
    }

    fn write_car_segment_source_fixture(root: &Path, encoded: &[u8]) -> PublishIndexEntryForCar {
        write_car_segment_source_fixture_for_kind(root, "test_payload", encoded)
    }

    fn publication_artifact_paths_for_fixture(
        root: &Path,
        entry: &PublishIndexEntryForCar,
    ) -> Vec<PathBuf> {
        let encoded = root.join(&entry.encoded_path);
        let json = root.join(&entry.json_path);
        let base = root
            .join(governance_car_segment_relative_base(entry).expect("derive fixture CAR base"));
        let car = base.with_extension("car");
        let plan = base.with_extension("plan.json");
        let manifest = base.with_extension("json");
        vec![
            encoded.clone(),
            digest_sidecar_path_for(&encoded),
            json.clone(),
            digest_sidecar_path_for(&json),
            car.clone(),
            digest_sidecar_path_for(&car),
            plan.clone(),
            digest_sidecar_path_for(&plan),
            manifest.clone(),
            digest_sidecar_path_for(&manifest),
        ]
    }

    fn seed_complete_uncommitted_publication_fixture(
        root: &Path,
        payload_kind: &str,
        encoded: &[u8],
        position: usize,
    ) -> (PublishIndexEntryForCar, Vec<(PathBuf, Vec<u8>)>) {
        let mut entry = write_car_segment_source_fixture_for_kind(root, payload_kind, encoded);
        entry.position = position;
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)
            .expect("retain publication fixture root");
        assemble_governance_car_queue(root, &root_guard, empty_governance_car_queue(), &entry)
            .expect("assemble uncommitted publication fixture");
        drop(root_guard);
        let snapshots = publication_artifact_paths_for_fixture(root, &entry)
            .into_iter()
            .map(|path| {
                let bytes = fs::read(&path).expect("snapshot uncommitted publication artifact");
                (path, bytes)
            })
            .collect();
        (entry, snapshots)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn recovery_quarantine_path(root: &Path) -> PathBuf {
        root.join(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn clear_recovery_quarantine_offline(root: &Path) {
        let quarantine = recovery_quarantine_path(root);
        assert!(
            quarantine.is_dir(),
            "offline cleanup requires a preserved recovery quarantine"
        );
        fs::remove_dir_all(quarantine)
            .expect("clear recovery quarantine while publisher is stopped");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn finish_recovery_after_offline_quarantine_cleanup(
        root: &Path,
    ) -> FilesystemGovernancePublisher {
        for _ in 0..3 {
            match FilesystemGovernancePublisher::try_new(root.to_path_buf()) {
                Ok(publisher) => return publisher,
                Err(error)
                    if error
                        .to_string()
                        .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR) =>
                {
                    clear_recovery_quarantine_offline(root);
                }
                Err(error) => panic!("restart after offline quarantine cleanup failed: {error}"),
            }
        }
        panic!("recovery did not converge after bounded offline quarantine cleanup")
    }

    fn committed_publication_artifact_paths(
        root: &Path,
        state: &JsonMap,
    ) -> Vec<(&'static str, PathBuf)> {
        let entry = state
            .get("publish_index")
            .and_then(|index| index.get("entries"))
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("committed publish entry");
        let segment = state
            .get("car_queue")
            .and_then(|queue| queue.get("segments"))
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(JsonValue::as_object)
            .expect("committed CAR segment");
        let encoded = root.join(
            entry
                .get("encoded_path")
                .and_then(JsonValue::as_str)
                .expect("committed encoded path"),
        );
        let json = root.join(
            entry
                .get("json_path")
                .and_then(JsonValue::as_str)
                .expect("committed JSON path"),
        );
        let car = root.join(
            segment
                .get("car_path")
                .and_then(JsonValue::as_str)
                .expect("committed CAR path"),
        );
        let plan = root.join(
            segment
                .get("plan_path")
                .and_then(JsonValue::as_str)
                .expect("committed CAR plan path"),
        );
        let manifest = root.join(
            segment
                .get("manifest_path")
                .and_then(JsonValue::as_str)
                .expect("committed CAR manifest path"),
        );
        vec![
            ("encoded source", encoded.clone()),
            ("encoded source sidecar", digest_sidecar_path_for(&encoded)),
            ("JSON source", json.clone()),
            ("JSON source sidecar", digest_sidecar_path_for(&json)),
            ("CAR archive", car.clone()),
            ("CAR archive sidecar", digest_sidecar_path_for(&car)),
            ("CAR plan", plan.clone()),
            ("CAR plan sidecar", digest_sidecar_path_for(&plan)),
            ("CAR manifest", manifest.clone()),
            ("CAR manifest sidecar", digest_sidecar_path_for(&manifest)),
        ]
    }

    #[test]
    fn governance_car_segment_sources_require_recorded_length_digest_and_file_caps() {
        let temp = tempdir().expect("tempdir");
        let encoded = b"canonical-payload";
        let entry = write_car_segment_source_fixture(temp.path(), encoded);
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain CAR source root");

        let (files, records) = governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect("read canonical CAR sources");
        assert_eq!(files.len(), 4);
        assert_eq!(records.len(), 4);

        let mut wrong_length = entry.clone();
        wrong_length.encoded_len += 1;
        let error = governance_car_segment_files(temp.path(), &root_guard, &wrong_length)
            .expect_err("shorter encoded source must not satisfy its recorded length");
        assert!(error.to_string().contains("encoded source"));

        let encoded_path = temp.path().join(&entry.encoded_path);
        let substituted_encoded = b"tampered!-payload";
        assert_eq!(substituted_encoded.len(), encoded.len());
        fs::write(&encoded_path, substituted_encoded).expect("substitute encoded source");
        let mut substituted_sidecar = blake3::hash(substituted_encoded).to_hex().to_string();
        substituted_sidecar.push('\n');
        fs::write(digest_sidecar_path_for(&encoded_path), substituted_sidecar)
            .expect("substitute matching encoded sidecar");
        let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect_err("same-length encoded plus sidecar substitution must fail closed");
        assert!(error.to_string().contains("encoded source"));

        let entry = write_car_segment_source_fixture(temp.path(), encoded);
        let json_path = temp.path().join(&entry.json_path);
        let substituted_json = br#"{"status":"owned"}"#;
        assert_eq!(substituted_json.len(), entry.json_len);
        fs::write(&json_path, substituted_json).expect("substitute JSON source");
        let mut substituted_sidecar = blake3::hash(substituted_json).to_hex().to_string();
        substituted_sidecar.push('\n');
        fs::write(digest_sidecar_path_for(&json_path), substituted_sidecar)
            .expect("substitute matching JSON sidecar");
        let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect_err("same-length JSON plus sidecar substitution must fail closed");
        assert!(error.to_string().contains("JSON source"));

        let entry = write_car_segment_source_fixture(temp.path(), encoded);
        fs::write(
            digest_sidecar_path_for(&temp.path().join(&entry.encoded_path)),
            format!("{}\n", "0".repeat(64)),
        )
        .expect("substitute encoded digest sidecar");
        let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect_err("a mismatched retained digest sidecar must fail closed");
        assert!(error.to_string().contains("digest sidecar"));

        let entry = write_car_segment_source_fixture(temp.path(), encoded);
        fs::write(
            digest_sidecar_path_for(&temp.path().join(&entry.encoded_path)),
            vec![b'0'; GOVERNANCE_DIGEST_SIDECAR_BYTES + 1],
        )
        .expect("write oversized digest sidecar");
        let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect_err("oversized digest sidecar must fail closed");
        assert!(
            error
                .to_string()
                .contains(&format!("exceeds {GOVERNANCE_DIGEST_SIDECAR_BYTES} bytes"))
        );

        let mut corrupted_index_entry = entry;
        corrupted_index_entry.encoded_len = GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES + 1;
        let error = governance_car_segment_files(temp.path(), &root_guard, &corrupted_index_entry)
            .expect_err("corrupted publish-index length must fail before its source read");
        assert!(error.to_string().contains("encoded publication length"));
    }

    #[test]
    fn governance_car_source_limits_cover_each_file_and_the_checked_segment_total() {
        assert_eq!(
            validate_governance_car_source_lengths(
                GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
                GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
            )
            .expect("boundary lengths are valid"),
            GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES
        );
        for (encoded_len, json_len) in [
            (GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES + 1, 1),
            (1, GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES + 1),
            (0, 1),
            (1, 0),
        ] {
            validate_governance_car_source_lengths(encoded_len, json_len)
                .expect_err("outside-boundary governance CAR source lengths must fail");
        }
    }

    #[test]
    fn governance_immutable_artifacts_are_exact_idempotent_and_non_overwritable() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        let path = temp.path().join("sources").join("identity.to");
        let canonical = b"canonical-source";

        write_immutable_governance_artifact(
            &root_guard,
            &path,
            canonical,
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
        )
        .expect("create immutable source");
        write_immutable_governance_artifact(
            &root_guard,
            &path,
            canonical,
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
        )
        .expect("exact replay is idempotent");
        let error = write_immutable_governance_artifact(
            &root_guard,
            &path,
            b"substituted-source",
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
        )
        .expect_err("divergent replay must not replace immutable source bytes");
        assert!(error.to_string().contains("occupied by different bytes"));
        assert_eq!(fs::read(path).expect("read immutable source"), canonical);
    }

    #[test]
    fn governance_publish_index_rejects_labels_above_the_fixed_cap() {
        let temp = tempdir().expect("tempdir");
        let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
        let mut labels = JsonMap::new();
        for index in 0..=GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1 {
            labels.insert(format!("label_{index}"), JsonValue::from(index as u64));
        }

        let error = update_publish_index(
            temp.path(),
            empty_governance_publish_index(),
            &fixture.payload_kind,
            &temp.path().join(&fixture.encoded_path),
            &temp.path().join(&fixture.json_path),
            &fixture.encoded_blake3,
            fixture.encoded_len,
            &fixture.json_blake3,
            fixture.json_len,
            labels,
        )
        .expect_err("publish entries above the label cap must fail before CAR assembly");
        assert!(error.to_string().contains(&format!(
            "{GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1}-label hard cap"
        )));
    }

    #[test]
    fn governance_publication_labels_enforce_canonical_scalar_and_byte_bounds() {
        let mut boundary = JsonMap::new();
        boundary.insert(
            "a".repeat(GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1),
            JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1)),
        );
        boundary.insert("boolean".into(), JsonValue::from(true));
        boundary.insert("null".into(), JsonValue::Null);
        boundary.insert("number".into(), JsonValue::from(7_u64));
        validate_governance_publication_labels(&boundary, "test publication")
            .expect("labels at the per-field boundaries remain valid");

        for key in [
            String::new(),
            "bad/key".to_owned(),
            "a".repeat(GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1 + 1),
        ] {
            let mut labels = JsonMap::new();
            labels.insert(key, JsonValue::from("value"));
            let error = validate_governance_publication_labels(&labels, "test publication")
                .expect_err("noncanonical label keys must fail closed");
            assert!(error.to_string().contains("noncanonical label key"));
        }

        for value in [
            JsonValue::Array(Vec::new()),
            JsonValue::Object(JsonMap::new()),
        ] {
            let mut labels = JsonMap::new();
            labels.insert("nested".into(), value);
            let error = validate_governance_publication_labels(&labels, "test publication")
                .expect_err("structured label values must fail closed");
            assert!(error.to_string().contains("must be a scalar"));
        }

        let mut oversized_string = JsonMap::new();
        oversized_string.insert(
            "value".into(),
            JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1 + 1)),
        );
        let error = validate_governance_publication_labels(&oversized_string, "test publication")
            .expect_err("oversized label strings must fail closed");
        assert!(error.to_string().contains("string bound"));

        let mut oversized_aggregate = JsonMap::new();
        for index in 0..16 {
            oversized_aggregate.insert(
                format!("label_{index:02}"),
                JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1)),
            );
        }
        let error =
            validate_governance_publication_labels(&oversized_aggregate, "test publication")
                .expect_err("oversized aggregate label metadata must fail closed");
        assert!(error.to_string().contains("aggregate bound"));
    }

    #[test]
    fn governance_index_paths_enforce_fixed_byte_and_component_bounds() {
        let boundary = std::iter::repeat_n("a", GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS)
            .collect::<Vec<_>>()
            .join("/");
        assert_eq!(
            index_path_components(&boundary)
                .expect("path at the component-count boundary is valid")
                .len(),
            GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS
        );
        assert!(
            index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES))
                .is_ok(),
            "component at the byte boundary is valid"
        );

        let too_many_components = format!("{boundary}/a");
        assert!(index_path_components(&too_many_components).is_err());
        assert!(
            index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES + 1))
                .is_err()
        );
        assert!(
            index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_MAX_BYTES + 1)).is_err()
        );
    }

    #[test]
    fn governance_publication_artifact_names_are_canonical_and_bounded() {
        let digest = "11".repeat(32);
        let oversized_kind = "a".repeat(129);
        for kind in [
            "",
            ".",
            "..",
            "../escape",
            "bad/kind",
            "Uppercase",
            oversized_kind.as_str(),
        ] {
            assert!(
                governance_source_pair_relative_paths(kind, 1, &digest, 1, &digest).is_err(),
                "publication kind `{kind}` must not become path authority"
            );
        }
        let (encoded, json) =
            governance_source_pair_relative_paths(&"a".repeat(128), 1, &digest, 1, &digest)
                .expect("publication kind at the byte boundary");
        assert!(encoded.ends_with("/payload.to"));
        assert!(json.ends_with("/payload.json"));

        let pair_id = "ab".repeat(32);
        assert!(is_canonical_governance_source_pair_directory(&pair_id));
        assert!(!is_canonical_governance_source_pair_directory(
            &pair_id.to_uppercase()
        ));
        for suffix in [
            ".car",
            ".car.blake3",
            ".plan.json",
            ".plan.json.blake3",
            ".json",
            ".json.blake3",
        ] {
            assert!(is_canonical_governance_car_artifact_name(&format!(
                "{:020}_{pair_id}{suffix}",
                7
            )));
        }
        assert!(!is_canonical_governance_car_artifact_name(&format!(
            "7_{pair_id}.car"
        )));
        assert!(!is_canonical_governance_car_artifact_name(&format!(
            "{:020}_{pair_id}.tmp",
            7
        )));
    }

    #[test]
    fn governance_publication_state_commit_failure_preserves_immutable_orphans() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
        let encoded_path = temp.path().join(&fixture.encoded_path);
        let json_path = temp.path().join(&fixture.json_path);
        let (publish_index, entry) = update_publish_index(
            temp.path(),
            empty_governance_publish_index(),
            &fixture.payload_kind,
            &encoded_path,
            &json_path,
            &fixture.encoded_blake3,
            fixture.encoded_len,
            &fixture.json_blake3,
            fixture.json_len,
            JsonMap::new(),
        )
        .expect("prepare bounded publish index");
        let queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect("qualify CAR artifacts before commit");
        let car_path = resolve_index_path(
            temp.path(),
            queue
                .get("segments")
                .and_then(JsonValue::as_array)
                .and_then(|segments| segments.first())
                .and_then(|segment| segment.get("car_path"))
                .and_then(JsonValue::as_str)
                .expect("qualified CAR path"),
        )
        .expect("resolve qualified CAR path");
        let canonical_car = fs::read(&car_path).expect("read qualified CAR");
        let mut state = empty_governance_publication_state();
        state.insert(
            "publish_index".into(),
            JsonValue::Object(publish_index.clone()),
        );
        state.insert("car_queue".into(), JsonValue::Object(queue));

        commit_governance_publication_state_with(
            temp.path(),
            &root_guard,
            state,
            |_guard, _path, _bytes| Err(io::Error::other("injected commit failure")),
        )
        .expect_err("injected authoritative rename must fail");
        assert!(
            !temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists(),
            "failed commit must not expose either nested index"
        );

        let retried_queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect("retry reuses exact immutable orphan artifacts");
        assert_eq!(fs::read(&car_path).expect("read reused CAR"), canonical_car);

        fs::write(&car_path, b"substituted orphan").expect("substitute unreachable orphan");
        let error = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect_err("retry must not replace a divergent immutable orphan");
        assert!(error.to_string().contains("occupied by different bytes"));

        fs::remove_file(&car_path).expect("remove divergent unreachable orphan");
        let repaired_queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect("retry recreates a missing immutable orphan from canonical sources");
        for field in [
            "segments",
            "by_encoded_blake3",
            "by_payload_kind",
            "by_car_archive_blake3",
            "segment_count",
            "assembled_count",
            "pending_count",
        ] {
            assert_eq!(
                retried_queue.get(field),
                repaired_queue.get(field),
                "canonical retry diverged at `{field}`"
            );
        }
        assert_eq!(
            fs::read(&car_path).expect("read recreated CAR"),
            canonical_car
        );
        let mut retry_state = empty_governance_publication_state();
        retry_state.insert("publish_index".into(), JsonValue::Object(publish_index));
        retry_state.insert("car_queue".into(), JsonValue::Object(repaired_queue));
        commit_governance_publication_state(temp.path(), &root_guard, retry_state)
            .expect("single authoritative retry commit");
        let committed = read_publication_state_fixture(temp.path());
        assert_eq!(
            committed.get("generation").and_then(JsonValue::as_u64),
            Some(1)
        );
        validate_governance_publication_state(
            committed.as_object().expect("committed publication state"),
        )
        .expect("committed cross-sections remain one-to-one");
    }

    #[test]
    fn governance_publication_failed_successor_commit_preserves_exact_predecessor() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");

        let first = write_car_segment_source_fixture(temp.path(), b"publication-a");
        let (first_index, first_entry) = update_publish_index(
            temp.path(),
            empty_governance_publish_index(),
            &first.payload_kind,
            &temp.path().join(&first.encoded_path),
            &temp.path().join(&first.json_path),
            &first.encoded_blake3,
            first.encoded_len,
            &first.json_blake3,
            first.json_len,
            JsonMap::new(),
        )
        .expect("prepare publication A index");
        let first_queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &first_entry,
        )
        .expect("prepare publication A CAR");
        let mut first_state = empty_governance_publication_state();
        first_state.insert("publish_index".into(), JsonValue::Object(first_index));
        first_state.insert("car_queue".into(), JsonValue::Object(first_queue));
        commit_governance_publication_state(temp.path(), &root_guard, first_state)
            .expect("commit publication A");

        let authority_path = temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE);
        let predecessor_bytes = fs::read(&authority_path).expect("read publication A authority");
        let predecessor: JsonValue =
            json::from_slice(&predecessor_bytes).expect("decode publication A authority");
        assert_eq!(
            predecessor.get("generation").and_then(JsonValue::as_u64),
            Some(1)
        );

        let second = write_car_segment_source_fixture(temp.path(), b"publication-b");
        let mut successor = predecessor
            .as_object()
            .expect("publication A authority object")
            .clone();
        let predecessor_index = successor
            .remove("publish_index")
            .and_then(|value| value.as_object().cloned())
            .expect("publication A index");
        let predecessor_queue = successor
            .remove("car_queue")
            .and_then(|value| value.as_object().cloned())
            .expect("publication A CAR queue");
        let (successor_index, second_entry) = update_publish_index(
            temp.path(),
            predecessor_index,
            &second.payload_kind,
            &temp.path().join(&second.encoded_path),
            &temp.path().join(&second.json_path),
            &second.encoded_blake3,
            second.encoded_len,
            &second.json_blake3,
            second.json_len,
            JsonMap::new(),
        )
        .expect("prepare publication B index");
        let successor_queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            predecessor_queue,
            &second_entry,
        )
        .expect("prepare publication B CAR");
        successor.insert("publish_index".into(), JsonValue::Object(successor_index));
        successor.insert("car_queue".into(), JsonValue::Object(successor_queue));

        commit_governance_publication_state_with(
            temp.path(),
            &root_guard,
            successor.clone(),
            |_guard, _path, _bytes| Err(io::Error::other("injected successor commit failure")),
        )
        .expect_err("publication B authoritative swap must fail");
        assert_eq!(
            fs::read(&authority_path).expect("reread authority after failed B commit"),
            predecessor_bytes,
            "a failed successor swap must preserve publication A byte-for-byte"
        );
        let visible = read_publication_state_fixture(temp.path());
        assert_eq!(
            visible.get("generation").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            visible
                .get("publish_index")
                .and_then(|index| index.get("entry_count"))
                .and_then(JsonValue::as_u64),
            Some(1)
        );

        commit_governance_publication_state(temp.path(), &root_guard, successor)
            .expect("retry publication B with the exact prepared successor");
        let committed = read_publication_state_fixture(temp.path());
        assert_eq!(
            committed.get("generation").and_then(JsonValue::as_u64),
            Some(2)
        );
        assert_eq!(
            committed
                .get("publish_index")
                .and_then(|index| index.get("entry_count"))
                .and_then(JsonValue::as_u64),
            Some(2)
        );
        validate_governance_publication_state(
            committed
                .as_object()
                .expect("committed publication B state"),
        )
        .expect("publication B commits both nested indexes together");
    }

    #[test]
    fn governance_publication_state_rejects_cross_section_substitution() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
        let (publish_index, entry) = update_publish_index(
            temp.path(),
            empty_governance_publish_index(),
            &fixture.payload_kind,
            &temp.path().join(&fixture.encoded_path),
            &temp.path().join(&fixture.json_path),
            &fixture.encoded_blake3,
            fixture.encoded_len,
            &fixture.json_blake3,
            fixture.json_len,
            JsonMap::new(),
        )
        .expect("prepare publish index");
        let mut queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect("prepare CAR queue");
        queue
            .get_mut("segments")
            .and_then(JsonValue::as_array_mut)
            .and_then(|segments| segments.first_mut())
            .and_then(JsonValue::as_object_mut)
            .expect("first CAR segment")
            .insert(
                "encoded_len".into(),
                JsonValue::from(u64::try_from(fixture.encoded_len + 1).expect("small fixture")),
            );
        let mut state = empty_governance_publication_state();
        state.insert("publish_index".into(), JsonValue::Object(publish_index));
        state.insert("car_queue".into(), JsonValue::Object(queue));

        let error = validate_governance_publication_state(&state)
            .expect_err("cross-section substitution must fail closed");
        assert!(error.to_string().contains("one-to-one"));
    }

    #[test]
    fn governance_publication_state_rejects_noncanonical_car_artifact_paths() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
        let (publish_index, entry) = update_publish_index(
            temp.path(),
            empty_governance_publish_index(),
            &fixture.payload_kind,
            &temp.path().join(&fixture.encoded_path),
            &temp.path().join(&fixture.json_path),
            &fixture.encoded_blake3,
            fixture.encoded_len,
            &fixture.json_blake3,
            fixture.json_len,
            JsonMap::new(),
        )
        .expect("prepare publish index");
        let mut queue = assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &entry,
        )
        .expect("prepare CAR queue");
        queue
            .get_mut("segments")
            .and_then(JsonValue::as_array_mut)
            .and_then(|segments| segments.first_mut())
            .and_then(JsonValue::as_object_mut)
            .expect("first CAR segment")
            .insert(
                "car_path".into(),
                JsonValue::from("car-segments/00000000000000000000_substituted.car"),
            );
        let mut state = empty_governance_publication_state();
        state.insert("publish_index".into(), JsonValue::Object(publish_index));
        state.insert("car_queue".into(), JsonValue::Object(queue));

        let error = validate_governance_publication_state(&state)
            .expect_err("CAR paths must be derived from the exact position/source identity");
        assert!(
            error
                .to_string()
                .contains("canonical composite-identity path")
        );
    }

    #[cfg(unix)]
    #[test]
    fn governance_car_segment_sources_reject_linked_path_components() {
        let temp = tempdir().expect("tempdir");
        let mut entry = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain CAR source root");
        let encoded_path = temp.path().join(&entry.encoded_path);
        let source_dir = encoded_path.parent().expect("canonical source directory");
        std::os::unix::fs::symlink(source_dir, temp.path().join("linked"))
            .expect("create linked source directory");
        entry.encoded_path = "linked/payload.to".to_owned();

        governance_car_segment_files(temp.path(), &root_guard, &entry)
            .expect_err("descriptor-rooted CAR reads must reject linked components");
    }

    #[test]
    fn governance_car_segment_source_reader_stays_rooted_and_per_file_bounded() {
        let source = include_str!("governance.rs");
        let start = source
            .find("fn governance_car_segment_files(")
            .expect("CAR source reader definition");
        let end = source[start..]
            .find("\nfn governance_car_plan_json(")
            .map(|offset| start + offset)
            .expect("end of CAR source reader definition");
        let reader = &source[start..end];

        assert!(reader.contains("read_rooted_governance_state_file"));
        assert!(reader.contains("entry.encoded_len"));
        assert!(reader.contains("entry.encoded_blake3"));
        assert!(reader.contains("entry.json_len"));
        assert!(reader.contains("entry.json_blake3"));
        assert!(reader.contains("GOVERNANCE_DIGEST_SIDECAR_BYTES"));
        assert!(reader.contains("GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES"));
        assert!(reader.contains("snapshot.binding().verify()"));
        assert!(reader.contains("digest sidecar does not match retained source bytes"));
        assert!(
            !reader.contains("fs::read("),
            "CAR source reads must remain descriptor-rooted"
        );
    }

    #[test]
    fn governance_dag_head_age_seconds_saturates_for_future_heads() {
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_000, 1_800_000_045),
            45
        );
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_100, 1_800_000_045),
            0
        );
    }

    #[test]
    fn governance_dag_head_generated_at_from_index_prefers_head_timestamp() {
        let mut index = JsonMap::new();
        assert_eq!(governance_dag_head_generated_at_from_index(&index), None);

        index.insert("generated_at".into(), JsonValue::from(1_800_000_000u64));
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_000)
        );

        index.insert(
            "head_generated_at".into(),
            JsonValue::from(1_800_000_045u64),
        );
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_045)
        );
    }

    #[test]
    fn bounded_governance_state_reader_rejects_oversized_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("index.json");
        fs::write(&path, b"123456789").expect("write oversized state");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("oversized governance state must fail before allocation");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds 8 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_governance_state_reader_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("target.json");
        let path = temp.path().join("index.json");
        fs::write(&target, b"{}").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("create index symlink");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("governance state symlink must fail closed");
        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_governance_state_reader_rejects_hard_link() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("target.json");
        let path = temp.path().join("index.json");
        fs::write(&target, b"{}").expect("write target");
        fs::hard_link(&target, &path).expect("create index hard link");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("hard-linked governance state must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
    }

    struct TestRuntimeDagSigner {
        handle: String,
        publisher_peer_id: Vec<u8>,
        key_pair: KeyPair,
        public_key_override: Option<[u8; 32]>,
        qualification_revision: AtomicU64,
        qualification_reads: AtomicU64,
        drift_on_second_qualification_read: AtomicBool,
        qualification_error: Option<String>,
        drift_during_sign: AtomicBool,
        refuse_with: Option<String>,
        corrupt_signature: bool,
    }

    impl fmt::Debug for TestRuntimeDagSigner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestRuntimeDagSigner")
                .field("handle", &self.handle)
                .field("publisher_peer_id", &self.publisher_peer_id)
                .finish_non_exhaustive()
        }
    }

    impl TestRuntimeDagSigner {
        fn new(handle: &str, publisher_peer_id: &[u8], seed: u8) -> Self {
            Self {
                handle: handle.to_owned(),
                publisher_peer_id: publisher_peer_id.to_vec(),
                key_pair: KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive test runtime DAG signer"),
                public_key_override: None,
                qualification_revision: AtomicU64::new(1),
                qualification_reads: AtomicU64::new(0),
                drift_on_second_qualification_read: AtomicBool::new(false),
                qualification_error: None,
                drift_during_sign: AtomicBool::new(false),
                refuse_with: None,
                corrupt_signature: false,
            }
        }

        fn public_key_bytes(&self) -> [u8; 32] {
            let (algorithm, bytes) = self
                .key_pair
                .public_key()
                .try_to_bytes()
                .expect("serialize test public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            bytes.try_into().expect("Ed25519 public key is fixed-width")
        }
    }

    impl GovernanceDagRuntimeSigner for TestRuntimeDagSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            if let Some(error) = &self.qualification_error {
                return Err(error.clone());
            }
            let read_index = self.qualification_reads.fetch_add(1, Ordering::SeqCst);
            let revision = self.qualification_revision.load(Ordering::SeqCst);
            let revision = if self
                .drift_on_second_qualification_read
                .load(Ordering::SeqCst)
                && read_index == 1
            {
                revision.saturating_add(1)
            } else {
                revision
            };
            Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                revision,
                TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
            ))
        }

        fn publisher_peer_id(&self) -> &[u8] {
            &self.publisher_peer_id
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key_override
                .unwrap_or_else(|| self.public_key_bytes())
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            if self.drift_during_sign.swap(false, Ordering::SeqCst) {
                self.qualification_revision.fetch_add(1, Ordering::SeqCst);
            }
            if let Some(error) = &self.refuse_with {
                return Err(error.clone());
            }
            let mut signature: [u8; 64] =
                IrohaSignature::try_new(self.key_pair.private_key(), payload)
                    .expect("test runtime signer can sign")
                    .payload()
                    .try_into()
                    .expect("Ed25519 signature is fixed-width");
            if self.corrupt_signature {
                signature[0] ^= 0x80;
            }
            Ok(signature)
        }
    }

    fn qualified_test_runtime_dag_signer(revision: u64, seed: u8) -> GovernanceRuntimeDagSigner {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            seed,
        ));
        signer
            .qualification_revision
            .store(revision, Ordering::SeqCst);
        let public_key = signer.public_key();
        GovernanceRuntimeDagSigner::try_new(
            "pkcs11:governance-dag:primary".to_owned(),
            peer_id,
            public_key,
            GovernanceDagRuntimeProviderQualificationV1::new(
                revision,
                TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
            ),
            signer,
        )
        .expect("qualify runtime DAG signer")
    }

    fn qualified_test_runtime_dag_checkpoint_store(
        store: Arc<TestRuntimeDagCheckpointStore>,
    ) -> GovernanceRuntimeDagCheckpointStore {
        GovernanceRuntimeDagCheckpointStore::try_new(
            TestRuntimeDagCheckpointStore::HANDLE.to_owned(),
            TestRuntimeDagCheckpointStore::qualification(),
            store,
        )
        .expect("qualify runtime DAG checkpoint store")
    }

    fn signed_runtime_publisher_with_store(
        root: &Path,
        store: Arc<TestRuntimeDagCheckpointStore>,
    ) -> FilesystemGovernancePublisher {
        FilesystemGovernancePublisher::try_new(root.to_path_buf())
            .expect("publisher")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(1, 0x31),
                qualified_test_runtime_dag_checkpoint_store(store),
            )
            .expect("runtime DAG providers")
    }

    fn signed_runtime_publisher(root: &Path) -> FilesystemGovernancePublisher {
        signed_runtime_publisher_with_store(
            root,
            Arc::new(TestRuntimeDagCheckpointStore::default()),
        )
    }

    fn runtime_index(root: &Path) -> JsonValue {
        let bytes =
            fs::read(root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)).expect("runtime index exists");
        let index: JsonValue = norito::json::from_slice(&bytes).expect("runtime index parses");
        assert_eq!(
            index.get("root").and_then(JsonValue::as_str),
            Some(GOVERNANCE_DAG_LOGICAL_ROOT),
            "public runtime index must not disclose its host filesystem root"
        );
        index
    }

    fn runtime_blocks_from_index(root: &Path, index: &JsonValue) -> Vec<GovernanceDagBlockV1> {
        index
            .get("blocks")
            .and_then(JsonValue::as_array)
            .expect("runtime blocks")
            .iter()
            .map(|entry| {
                let block_path = entry
                    .get("block_path")
                    .and_then(JsonValue::as_str)
                    .expect("block path");
                let block_path = resolve_index_path(root, block_path).expect("resolve block path");
                let bytes = fs::read(block_path).expect("read runtime block");
                norito::decode_from_bytes(&bytes).expect("decode runtime block")
            })
            .collect()
    }

    fn assert_single_runtime_external(root: &Path, kind: &str, encoded: &[u8]) {
        let index = runtime_index(root);
        let blocks = runtime_blocks_from_index(root, &index);
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::ExternalPayload(payload) => {
                payload.validate().expect("external payload validates");
                assert_eq!(payload.payload_kind, kind);
                assert_eq!(payload.encoded_payload, encoded);
                assert_eq!(payload.encoded_blake3, *blake3::hash(encoded).as_bytes());
            }
            other => panic!("expected external runtime payload, found {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_rejects_noncanonical_or_mismatched_payload_bytes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, canonical) = sample_settlement();

        let bare = settlement.encode();
        let error = publisher
            .publish_deal_settlement(&settlement, &bare)
            .expect_err("bare payload without a Norito header must fail");
        assert!(error.to_string().contains("canonical header-bearing"));

        let mut conflicting = settlement.clone();
        conflicting.audit_notes = Some("different typed payload".to_owned());
        let error = publisher
            .publish_deal_settlement(&conflicting, &canonical)
            .expect_err("typed payload and canonical bytes must match");
        assert!(error.to_string().contains("do not match"));
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "validation must fail before any governance artifact is written"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_semantically_invalid_payload_before_writes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (mut settlement, _) = sample_settlement();
        settlement.deal_id[0] ^= 0x80;
        let encoded = norito::to_bytes(&settlement).expect("encode invalid settlement");

        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("ledger and settlement deal identifiers must match");
        assert!(error.to_string().contains("invalid deal settlement"));
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "semantic validation must fail before any governance artifact is written"
        );
    }

    #[test]
    fn filesystem_publisher_writes_por_payloads_into_one_signed_canonical_chain() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (publication, publication_encoded) = sample_por_challenge_publication();
        let (report, report_encoded) = sample_por_weekly_report();

        publisher
            .publish_por_challenge_publication(&publication, &publication_encoded)
            .expect("publish PoR challenge");
        publisher
            .publish_por_weekly_report(&report, &report_encoded)
            .expect("publish PoR weekly report");

        let challenge_path = temp
            .path()
            .join("por")
            .join("challenges")
            .join(format!("{:020}", publication.challenge.epoch_id))
            .join(hex::encode(publication.challenge.challenge_id))
            .with_extension("to");
        assert_eq!(
            fs::read(&challenge_path).expect("read canonical challenge publication"),
            publication_encoded
        );

        let report_digest = blake3::hash(&report_encoded).to_hex().to_string();
        let report_path = temp
            .path()
            .join("por")
            .join("reports")
            .join(format!(
                "{:04}-W{:02}_{:020}_{}",
                report.cycle.year,
                report.cycle.week,
                report.generated_at,
                &report_digest[..16],
            ))
            .with_extension("to");
        assert_eq!(
            fs::read(&report_path).expect("read canonical weekly report"),
            report_encoded
        );

        let index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
        assert_eq!(
            blocks[1].node.prev_cid,
            Some(blocks[0].node.node_cid.clone())
        );
        assert_eq!(
            blocks[0].node.payload,
            GovernanceLogPayloadV1::PorChallengePublication(publication)
        );
        assert_eq!(
            blocks[1].node.payload,
            GovernanceLogPayloadV1::PorWeeklyReport(report)
        );
        let head_bytes =
            fs::read(runtime_dag_head_path(temp.path())).expect("read signed runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode signed runtime head");
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("PoR runtime chain and head validate");
    }

    #[test]
    fn filesystem_publisher_root_has_a_single_process_owner() {
        let temp = tempdir().expect("tempdir");
        let owner = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("acquire publisher root");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("a second publisher must not share mutable index state");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("already in use"));

        drop(owner);
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher root ownership releases on drop");
    }

    #[test]
    fn filesystem_publisher_restart_rejects_runtime_signer_revision_substitution() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        {
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (settlement, encoded) = sample_settlement();
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect("seed signed runtime DAG");
        }
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("signer_revision").and_then(JsonValue::as_u64),
            Some(1)
        );
        let expected_policy_digest = hex::encode(TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST);
        assert_eq!(
            index
                .get("signer_policy_digest_hex")
                .and_then(JsonValue::as_str),
            Some(expected_policy_digest.as_str())
        );

        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let provider = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        provider.qualification_revision.store(2, Ordering::SeqCst);
        let signer = GovernanceRuntimeDagSigner::try_new(
            "pkcs11:governance-dag:primary".to_owned(),
            peer_id,
            provider.public_key(),
            GovernanceDagRuntimeProviderQualificationV1::new(
                2,
                TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
            ),
            provider,
        )
        .expect("qualify rotated runtime signer");
        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher root")
            .with_qualified_runtime_dag_providers(
                signer,
                qualified_test_runtime_dag_checkpoint_store(checkpoint_store),
            )
            .expect_err("implicit signer revision rotation must fail startup");
        assert!(
            error.to_string().contains("malformed")
                || error.to_string().contains("another root or signer")
                || error.to_string().contains("provider binding")
        );
    }

    #[test]
    fn filesystem_publisher_replays_authenticated_provider_transition_after_ambiguous_cas() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let mut publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("seed signed runtime DAG");

        checkpoint_store
            .fail_after_next_checkpoint_cas
            .store(true, Ordering::SeqCst);
        let next_store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("checkpoint store")
            .clone();
        let error = publisher
            .transition_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(2, 0x31),
                next_store,
            )
            .expect_err("ambiguous provider-transition checkpoint CAS must surface");
        assert!(error.to_string().contains("compare-and-swap failed"));
        drop(publisher);

        let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(2, 0x31),
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect("replay exact signed provider transition");
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("signer_revision").and_then(JsonValue::as_u64),
            Some(2)
        );
        let binding = runtime_dag_provider_binding(
            recovered
                .runtime_dag_signer
                .as_ref()
                .expect("recovered signer"),
            recovered
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("recovered store"),
        );
        let (_, summary) = read_runtime_dag_qualification_history(temp.path(), Some(&binding))
            .expect("read transition history")
            .expect("transition history exists");
        assert_eq!(summary.transition_generation, 1);
        assert_ne!(summary.transition_digest, [0; 32]);
        drop(recovered);
    }

    #[test]
    fn filesystem_publisher_rotates_signing_keys_with_authenticated_authority_segments() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let initial_signer = qualified_test_runtime_dag_signer(1, 0x31);
        let initial_public_key = initial_signer.public_key;
        let mut publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher")
            .with_qualified_runtime_dag_providers(
                initial_signer,
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect("initial runtime DAG providers");
        let (settlement, settlement_encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &settlement_encoded)
            .expect("publish under the outgoing authority");

        let next_signer = qualified_test_runtime_dag_signer(2, 0x32);
        let next_public_key = next_signer.public_key;
        assert_ne!(initial_public_key, next_public_key);
        let next_store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("checkpoint store")
            .clone();
        publisher
            .transition_qualified_runtime_dag_providers(next_signer, next_store)
            .expect("rotate to a distinct authenticated signing key");

        let current_signer = publisher
            .runtime_dag_signer
            .as_ref()
            .expect("rotated signer");
        let current_store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("rotated checkpoint store");
        validate_existing_runtime_dag_root(temp.path(), current_signer, current_store)
            .expect("an outgoing-signed tip remains valid until the incoming key appends");
        let current_binding = runtime_dag_provider_binding(current_signer, current_store);
        let lineage = runtime_dag_authority_lineage(temp.path(), &current_binding)
            .expect("read authenticated authority lineage");
        assert_eq!(lineage.segments.len(), 2);
        assert_eq!(lineage.transitions.len(), 1);
        assert_eq!(lineage.segments[0].activation_block_count, 0);
        assert_eq!(lineage.segments[0].revision, 1);
        assert_eq!(
            lineage.segments[0].binding.publisher_public_key,
            initial_public_key
        );
        assert_eq!(lineage.segments[1].activation_block_count, 1);
        assert_eq!(lineage.segments[1].revision, 2);
        assert_eq!(
            lineage.segments[1].binding.publisher_public_key,
            next_public_key
        );
        validate_runtime_dag_qualification_transition(
            &lineage.transitions[0],
            runtime_dag_producer_root_digest(temp.path()).expect("root digest"),
        )
        .expect("both continuity signatures authenticate the key transition");

        let (publication, publication_encoded) = sample_por_challenge_publication();
        publisher
            .publish_por_challenge_publication(&publication, &publication_encoded)
            .expect("publish under the incoming authority");
        let index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0].block_signature.public_key,
            initial_public_key.to_vec()
        );
        assert_eq!(
            blocks[1].block_signature.public_key,
            next_public_key.to_vec()
        );
        let head_bytes =
            fs::read(runtime_dag_head_path(temp.path())).expect("read rotated signed head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode rotated signed head");
        assert_eq!(head.head_signature.public_key, next_public_key.to_vec());
        validate_existing_runtime_dag_root(temp.path(), current_signer, current_store)
            .expect("segmented chain validates after the incoming key appends");
        drop(publisher);

        let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher after key rotation")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(2, 0x32),
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect("recover using only the current runtime providers");
        validate_existing_runtime_dag_root(
            temp.path(),
            recovered
                .runtime_dag_signer
                .as_ref()
                .expect("recovered signer"),
            recovered
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("recovered checkpoint store"),
        )
        .expect("bounded recovery authenticates every historical signer segment");
    }

    #[test]
    fn qualification_compaction_seals_archive_before_prune_and_recovers_idempotently() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let mut publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("seed signed runtime DAG");
        for revision in 2..=4 {
            let next_store = publisher
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("checkpoint store")
                .clone();
            publisher
                .transition_qualified_runtime_dag_providers(
                    qualified_test_runtime_dag_signer(revision, 0x31),
                    next_store,
                )
                .expect("append provider transition");
        }

        checkpoint_store
            .fail_before_next_checkpoint_cas
            .store(true, Ordering::SeqCst);
        let error = publisher
            .compact_runtime_dag_qualification_history(1)
            .expect_err("archive checkpoint refusal must surface after durable archive install");
        assert!(error.to_string().contains("compare-and-swap failed"));
        drop(publisher);

        let mut recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(4, 0x31),
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect("finish archive prune from sealed checkpoint");
        let binding = runtime_dag_provider_binding(
            recovered
                .runtime_dag_signer
                .as_ref()
                .expect("recovered signer"),
            recovered
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("recovered store"),
        );
        let (history, summary) =
            read_runtime_dag_qualification_history(temp.path(), Some(&binding))
                .expect("read compacted history")
                .expect("compacted history exists");
        assert_eq!(history.transitions.len(), 1);
        assert_eq!(history.archived_through_generation, 2);
        assert_eq!(summary.transition_generation, 3);
        assert_eq!(summary.archive_generation, 1);
        assert_ne!(summary.archive_digest, [0; 32]);
        let archive_path = runtime_dag_qualification_archive_path(
            temp.path(),
            summary.archive_generation,
            summary.archive_digest,
        );
        fs::remove_file(digest_sidecar_path_for(&archive_path))
            .expect("simulate crash before archive sidecar install");
        let archive = read_runtime_dag_qualification_archive(
            temp.path(),
            summary.archive_generation,
            summary.archive_digest,
            history.root_digest,
        )
        .expect("read signed qualification archive");
        assert!(
            digest_sidecar_path_for(&archive_path).is_file(),
            "authenticated archive replay restores its missing sidecar"
        );
        let mut tampered_archive = archive;
        tampered_archive.signature[0] ^= 0x80;
        assert!(
            validate_runtime_dag_qualification_archive(&tampered_archive, history.root_digest,)
                .is_err()
        );
        assert_eq!(
            recovered
                .compact_runtime_dag_qualification_history(1)
                .expect("idempotent compaction replay"),
            0
        );
        for revision in 5..=6 {
            let next_store = recovered
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("checkpoint store")
                .clone();
            recovered
                .transition_qualified_runtime_dag_providers(
                    qualified_test_runtime_dag_signer(revision, 0x31),
                    next_store,
                )
                .expect("append post-archive provider transition");
        }
        checkpoint_store
            .fail_after_next_checkpoint_cas
            .store(true, Ordering::SeqCst);
        let error = recovered
            .compact_runtime_dag_qualification_history(1)
            .expect_err("ambiguous post-CAS archive checkpoint response must surface");
        assert!(error.to_string().contains("compare-and-swap failed"));
        drop(recovered);

        let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher after post-CAS crash")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(6, 0x31),
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect("finish post-CAS archive prune");
        let binding = runtime_dag_provider_binding(
            recovered
                .runtime_dag_signer
                .as_ref()
                .expect("recovered signer"),
            recovered
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("recovered store"),
        );
        let (history, summary) =
            read_runtime_dag_qualification_history(temp.path(), Some(&binding))
                .expect("read twice-compacted history")
                .expect("twice-compacted history exists");
        assert_eq!(history.transitions.len(), 1);
        assert_eq!(summary.transition_generation, 5);
        assert_eq!(summary.archive_generation, 2);
        assert_eq!(
            recovered
                .compact_runtime_dag_qualification_history(1)
                .expect("second idempotent compaction replay"),
            0
        );
        drop(recovered);
    }

    #[test]
    fn qualification_history_rejects_tamper_fork_duplicate_rollback_and_bad_bytes() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let mut publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("seed signed runtime DAG");
        for revision in 2..=4 {
            let next_store = publisher
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("checkpoint store")
                .clone();
            publisher
                .transition_qualified_runtime_dag_providers(
                    qualified_test_runtime_dag_signer(revision, 0x31),
                    next_store,
                )
                .expect("append provider transition");
        }
        let binding = runtime_dag_provider_binding(
            publisher
                .runtime_dag_signer
                .as_ref()
                .expect("current signer"),
            publisher
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("current store"),
        );
        let (history, _) = read_runtime_dag_qualification_history(temp.path(), Some(&binding))
            .expect("read qualification history")
            .expect("qualification history exists");
        assert_eq!(history.transitions.len(), 3);
        let history_path = runtime_dag_qualification_history_path(temp.path());
        fs::remove_file(digest_sidecar_path_for(&history_path))
            .expect("simulate crash before history sidecar install");
        read_runtime_dag_qualification_history(temp.path(), Some(&binding))
            .expect("replay signed history with a missing sidecar")
            .expect("signed history remains present");
        assert!(
            digest_sidecar_path_for(&history_path).is_file(),
            "authenticated history replay restores its missing sidecar"
        );

        let mut tampered = history.clone();
        tampered.transitions[1].key_transition.incoming_signature[0] ^= 0x80;
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &tampered,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut outgoing_tampered = history.clone();
        outgoing_tampered.transitions[1]
            .key_transition
            .outgoing_signature[0] ^= 0x80;
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &outgoing_tampered,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut segment_revision_rollback = history.clone();
        let outgoing_revision = segment_revision_rollback.transitions[1]
            .key_transition
            .outgoing_segment_revision;
        segment_revision_rollback.transitions[1]
            .key_transition
            .incoming_segment_revision = outgoing_revision;
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &segment_revision_rollback,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut replayed_envelope = history.clone();
        replayed_envelope.transitions[1].key_transition =
            replayed_envelope.transitions[0].key_transition.clone();
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &replayed_envelope,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut forked = history.clone();
        forked.transitions.swap(1, 2);
        assert!(
            validate_runtime_dag_qualification_history(temp.path(), &forked, Some(&binding), None,)
                .is_err()
        );

        let mut duplicated = history.clone();
        duplicated
            .transitions
            .insert(1, duplicated.transitions[0].clone());
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &duplicated,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut rolled_back = history.clone();
        rolled_back.transitions.pop();
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &rolled_back,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let mut substituted = history.clone();
        substituted.transitions[2].body.next.signer_revision += 1;
        assert!(
            validate_runtime_dag_qualification_history(
                temp.path(),
                &substituted,
                Some(&binding),
                None,
            )
            .is_err()
        );

        let bytes = norito::to_bytes(&history).expect("encode canonical history");
        assert!(
            decode_canonical_runtime_dag::<RuntimeDagQualificationHistoryV1>(
                &bytes[..bytes.len() - 1],
                "truncated qualification history",
            )
            .is_err()
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            decode_canonical_runtime_dag::<RuntimeDagQualificationHistoryV1>(
                &trailing,
                "qualification history with trailing bytes",
            )
            .is_err()
        );
    }

    #[test]
    fn filesystem_publisher_recovers_all_atomic_temp_boundaries_from_sealed_intent() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        {
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (settlement, encoded) = sample_settlement();
            let error = publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect_err("ambiguous intent CAS response must surface");
            assert!(error.to_string().contains("compare-and-swap failed"));
        }

        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load sealed producer intent")
            .expect("ambiguous CAS retained producer intent");
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
        let block_path = runtime_dag_producer_block_path_from_intent(temp.path(), &intent)
            .expect("resolve intent block path");
        let head_path = runtime_dag_head_path(temp.path());
        let index_path = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
        let mut stale_temps = Vec::new();
        for (target_index, target) in [&block_path, &head_path, &index_path]
            .into_iter()
            .flat_map(|target| [target.clone(), digest_sidecar_path_for(target)])
            .enumerate()
        {
            fs::create_dir_all(target.parent().expect("transaction target parent"))
                .expect("create transaction target parent");
            let stale = temp_path_for_atomic(
                &target,
                40_000 + u32::try_from(target_index).expect("small target index"),
                u64::try_from(target_index).expect("small target index"),
            );
            fs::write(&stale, b"crash-before-rename").expect("seed stale atomic temp");
            stale_temps.push(stale);
        }

        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("reopen publisher before sealed-intent recovery")
                .with_qualified_runtime_dag_providers(
                    qualified_test_runtime_dag_signer(1, 0x31),
                    qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
                )
                .expect_err("Unix recovery must preserve all interrupted temps offline");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected combined recovery error: {error}"
            );
            assert_eq!(
                fs::read_dir(recovery_quarantine_path(temp.path()))
                    .expect("read combined transaction quarantine")
                    .count(),
                6,
                "one recovery transaction must isolate all six exact boundaries"
            );
        }
        for stale in &stale_temps {
            assert!(
                !stale.exists(),
                "sealed recovery must remove its exact stale transaction temp from the live namespace"
            );
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        clear_recovery_quarantine_offline(temp.path());
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload producer intent")
                .is_none()
        );
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        drop(publisher);
    }

    #[test]
    fn filesystem_publisher_reconstructs_each_missing_transaction_boundary() {
        for missing_boundary in 0..6 {
            let temp = tempdir().expect("tempdir");
            let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (first, first_encoded) = sample_settlement();
            publisher
                .publish_deal_settlement(&first, &first_encoded)
                .expect("seed predecessor runtime DAG block");

            let mut successor = first;
            successor.deal_id = [0x42; 32];
            successor.ledger.deal_id = successor.deal_id;
            successor.ledger.snapshot_id = successor
                .ledger
                .derive_snapshot_id()
                .expect("reseal successor ledger snapshot");
            successor.settlement_id = successor
                .derive_settlement_id()
                .expect("reseal successor settlement");
            let successor_encoded =
                norito::to_bytes(&successor).expect("encode successor settlement");
            checkpoint_store
                .fail_after_next_intent_cas
                .store(true, Ordering::SeqCst);
            publisher
                .publish_deal_settlement(&successor, &successor_encoded)
                .expect_err("retain sealed successor intent before filesystem apply");

            let intent_record = checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("load retained producer intent")
                .expect("producer intent exists");
            let intent: RuntimeDagProducerPublishIntentV1 =
                norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
            let staged = load_runtime_dag_producer_staged_transaction(
                temp.path(),
                publisher.root_guard(),
                &intent,
            )
            .expect("load exact staged transaction");
            let signer = publisher
                .runtime_dag_signer
                .as_ref()
                .expect("test publisher signer");
            let store = publisher
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("test publisher checkpoint store");
            let previous_record = store
                .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
                .expect("load predecessor checkpoint")
                .expect("predecessor checkpoint exists");
            let previous = decode_runtime_dag_producer_checkpoint_record(
                &previous_record,
                temp.path(),
                signer,
                store,
            )
            .expect("decode predecessor checkpoint");
            validate_runtime_dag_producer_intent_successor(
                temp.path(),
                signer,
                &intent,
                &staged,
                Some(&previous),
            )
            .expect("authenticate successor before applying it");
            apply_runtime_dag_producer_intent(
                temp.path(),
                &publisher.root_guard,
                signer,
                store,
                &intent,
                &staged,
                Some(&previous),
            )
            .expect("materialize all successor transaction files without checkpoint CAS");

            let block_path = runtime_dag_producer_block_path_from_intent(temp.path(), &intent)
                .expect("resolve successor block path");
            let head_path = runtime_dag_head_path(temp.path());
            let index_path = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
            let boundaries = [
                block_path.clone(),
                digest_sidecar_path_for(&block_path),
                head_path.clone(),
                digest_sidecar_path_for(&head_path),
                index_path.clone(),
                digest_sidecar_path_for(&index_path),
            ];
            let missing = boundaries
                .get(missing_boundary)
                .expect("six transaction boundaries");
            fs::remove_file(missing).expect("simulate lost renamed transaction boundary");
            drop(publisher);

            let recovered =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            assert!(
                missing.is_file(),
                "sealed recovery did not reconstruct boundary {missing_boundary}: {}",
                missing.display()
            );
            for primary in [&block_path, &head_path, &index_path] {
                let max_bytes = if primary == &block_path {
                    GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES
                } else {
                    GOVERNANCE_MUTABLE_INDEX_MAX_BYTES
                };
                let bytes = read_bounded_governance_state_file(primary, max_bytes)
                    .expect("read reconstructed primary");
                verify_digest_sidecar(primary, &bytes)
                    .expect("reconstructed primary has its exact digest sidecar");
            }
            assert!(
                checkpoint_store
                    .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                    .expect("reload producer intent")
                    .is_none()
            );
            assert_eq!(
                runtime_index(temp.path())
                    .get("block_count")
                    .and_then(JsonValue::as_u64),
                Some(2)
            );
            drop(recovered);
        }
    }

    #[test]
    fn filesystem_publisher_clamps_clock_regression_before_sealing_and_recovers() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (first, first_encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&first, &first_encoded)
            .expect("seed predecessor runtime DAG block");
        let predecessor_timestamp = runtime_index(temp.path())
            .get("head_generated_at")
            .and_then(JsonValue::as_u64)
            .expect("predecessor head timestamp");
        publisher
            .set_runtime_dag_observed_timestamp_for_test(predecessor_timestamp.saturating_sub(1));

        let mut successor = first;
        successor.deal_id = [0x43; 32];
        successor.ledger.deal_id = successor.deal_id;
        successor.ledger.snapshot_id = successor
            .ledger
            .derive_snapshot_id()
            .expect("reseal successor ledger snapshot");
        successor.settlement_id = successor
            .derive_settlement_id()
            .expect("reseal successor settlement");
        let successor_encoded = norito::to_bytes(&successor).expect("encode successor settlement");
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        publisher
            .publish_deal_settlement(&successor, &successor_encoded)
            .expect_err("retain the monotonically timestamped successor intent");
        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load retained producer intent")
            .expect("producer intent exists");
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
        let staged = load_runtime_dag_producer_staged_transaction(
            temp.path(),
            publisher.root_guard(),
            &intent,
        )
        .expect("load staged clock-regression successor");
        let block: GovernanceDagBlockV1 =
            decode_canonical_runtime_dag(&staged.block_bytes, "clock-regression successor block")
                .expect("decode successor block");
        assert_eq!(block.timestamp, predecessor_timestamp);
        assert_eq!(block.node.timestamp, predecessor_timestamp);
        drop(publisher);

        let recovered =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload producer intent")
                .is_none(),
            "restart must complete the clamped successor rather than wedge on timestamp regression"
        );
        assert_eq!(
            runtime_index(temp.path())
                .get("block_count")
                .and_then(JsonValue::as_u64),
            Some(2)
        );
        drop(recovered);
    }

    #[test]
    fn filesystem_publisher_recovers_checkpoint_cas_applied_response_error() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        checkpoint_store
            .fail_after_next_checkpoint_cas
            .store(true, Ordering::SeqCst);
        {
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (settlement, encoded) = sample_settlement();
            let error = publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect_err("ambiguous checkpoint CAS response must surface");
            assert!(error.to_string().contains("compare-and-swap failed"));
        }
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("load retained producer intent")
                .is_some()
        );
        fs::remove_dir_all(
            temp.path()
                .join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR),
        )
        .expect("simulate loss of staging bytes after committed checkpoint CAS");

        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload producer intent")
                .is_none(),
            "restart must authenticate the committed target and delete its retained intent"
        );
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        drop(publisher);
    }

    #[test]
    fn runtime_dag_producer_bounds_accept_exact_limits_and_reject_successors() {
        let mutable_limit = GOVERNANCE_MUTABLE_INDEX_MAX_BYTES;
        let block_limit = GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES;
        validate_runtime_dag_producer_file_lengths(block_limit, mutable_limit, mutable_limit)
            .expect("exact per-file limits are accepted");
        assert!(
            validate_runtime_dag_producer_file_lengths(block_limit + 1, 1, 1).is_err(),
            "block limit + 1 must fail before sealing"
        );
        assert!(
            validate_runtime_dag_producer_file_lengths(1, mutable_limit + 1, 1).is_err(),
            "head limit + 1 must fail before sealing"
        );
        assert!(
            validate_runtime_dag_producer_file_lengths(1, 1, mutable_limit + 1).is_err(),
            "index limit + 1 must fail before sealing"
        );
        assert!(
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES
                > GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES
        );
        validate_runtime_dag_producer_entry_count(
            GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1,
            u64::try_from(GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1).expect("entry cap fits u64"),
        )
        .expect("exact entry cap is accepted");
        assert!(
            validate_runtime_dag_producer_entry_count(
                GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1 + 1,
                u64::try_from(GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1 + 1)
                    .expect("entry cap successor fits u64"),
            )
            .is_err(),
            "entry cap + 1 must fail before sealing"
        );
        let mut total = GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1 - 1;
        add_runtime_dag_audit_bytes(&mut total, 1).expect("exact root byte cap is accepted");
        assert!(
            add_runtime_dag_audit_bytes(&mut total, 1).is_err(),
            "root byte cap + 1 must fail before sealing"
        );
        assert_eq!(
            governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerCheckpoint,
            ),
            64 * 1024
        );
        assert_eq!(
            governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerPublishIntent,
            ),
            64 * 1024
        );
        assert!(
            governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerPublishIntent,
            ) < governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::PublishIntent,
            ),
            "the digest-only producer intent must retain its small independent ceiling"
        );
        assert!(
            governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerCheckpoint,
            ) < governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::Checkpoint,
            ),
            "the bounded producer checkpoint must retain its small independent ceiling"
        );
    }

    #[test]
    fn runtime_dag_producer_intent_is_digest_only_and_stage_tamper_fails_closed() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        {
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (settlement, encoded) = sample_settlement();
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect_err("retain the digest-only producer intent");
        }
        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load retained producer intent")
            .expect("producer intent exists");
        assert!(
            intent_record.payload.len()
                <= governance_dag_sealed_state_payload_max_bytes_v1(
                    GovernanceDagSealedStateSlot::ProducerPublishIntent,
                )
        );
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode digest-only intent");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain producer root");
        let staged =
            load_runtime_dag_producer_staged_transaction(temp.path(), &root_guard, &intent)
                .expect("authenticate the durable staged transaction");
        assert_eq!(
            intent.index.byte_len,
            u64::try_from(staged.index_bytes.len()).expect("staged index length fits u64")
        );

        let index_path = runtime_dag_producer_staging_paths(temp.path())[2].clone();
        let mut substituted = staged.index_bytes;
        substituted[0] ^= 0x80;
        fs::write(&index_path, &substituted).expect("substitute staged index");
        write_digest_sidecar(&root_guard, &index_path, &substituted)
            .expect("refresh only the unauthenticated sidecar");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher root")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(1, 0x31),
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
            )
            .expect_err("staged index substitution must fail closed");
        assert!(error.to_string().contains("staged index is substituted"));
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload retained producer intent")
                .is_some(),
            "a failed staged readback must not erase the recovery intent"
        );
    }

    #[test]
    fn runtime_dag_checkpoint_wrapper_rejects_oversized_producer_records() {
        for slot in [
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
        ] {
            let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
            let oversized_payload =
                vec![0xA5; governance_dag_sealed_state_payload_max_bytes_v1(slot) + 1];
            let record = GovernanceDagSealedStateRecord::new(slot, 1, oversized_payload);
            let mut state = checkpoint_store
                .state
                .lock()
                .expect("lock test checkpoint store");
            state.records[TestRuntimeDagCheckpointStore::slot_index(slot)] = Some(record);
            drop(state);
            let qualified =
                qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store));
            let error = qualified
                .load(slot)
                .expect_err("oversized producer record must fail before canonical decode");
            assert!(error.to_string().contains("oversized record"));
        }
    }

    #[test]
    fn runtime_dag_staging_root_is_created_through_the_retained_root() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        ensure_runtime_dag_producer_staging_root(temp.path(), publisher.root_guard())
            .expect("create and synchronize the producer staging root");
        publisher
            .root_guard()
            .rooted_directory()
            .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR))
            .expect("staging root remains bound below the retained producer root");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn runtime_dag_staging_retained_generations_survive_successive_cycle_and_restart() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (first, first_encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&first, &first_encoded)
            .expect("publish first staging cycle");

        let mut successor = first;
        successor.deal_id = [0xA6; 32];
        successor.ledger.deal_id = successor.deal_id;
        successor.ledger.snapshot_id = successor
            .ledger
            .derive_snapshot_id()
            .expect("reseal successor ledger snapshot");
        successor.settlement_id = successor
            .derive_settlement_id()
            .expect("reseal successor settlement");
        let successor_encoded = norito::to_bytes(&successor).expect("encode successor settlement");
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        publisher
            .publish_deal_settlement(&successor, &successor_encoded)
            .expect_err("retain the second sealed staging cycle");

        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load second-cycle intent")
            .expect("second-cycle intent exists");
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode second-cycle intent");
        load_runtime_dag_producer_staged_transaction(temp.path(), publisher.root_guard(), &intent)
            .expect("retained predecessor inventory must not obscure live staged artifacts");
        let paths = runtime_dag_producer_staging_paths(temp.path());
        let staging =
            runtime_dag_producer_staging_directory(temp.path(), publisher.root_guard(), false)
                .expect("open second-cycle staging directory");
        let names = staging
            .child_names()
            .expect("inventory second staging cycle");
        assert_eq!(names.len(), 12, "six live files retain six predecessors");
        assert_eq!(
            names
                .iter()
                .filter(|name| {
                    name.to_str()
                        .and_then(governance_rooted_fs::atomic_retained_target_name)
                        .is_some()
                })
                .count(),
            6
        );
        validate_runtime_dag_producer_staging_inventory(publisher.root_guard(), &staging, &paths)
            .expect("explicit second-cycle inventory validation");
        drop(staging);
        drop(publisher);

        let restarted =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let staging =
            runtime_dag_producer_staging_directory(temp.path(), restarted.root_guard(), false)
                .expect("reopen staging directory after intent recovery");
        validate_runtime_dag_producer_staging_inventory(restarted.root_guard(), &staging, &paths)
            .expect("restart preserves and validates the bounded retained inventory");
        assert_eq!(
            runtime_index(temp.path())
                .get("block_count")
                .and_then(JsonValue::as_u64),
            Some(2)
        );
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload second-cycle intent")
                .is_none()
        );
    }

    #[test]
    fn runtime_dag_payload_preflight_counts_without_allocating_dummy_envelopes() {
        let (settlement, encoded) = sample_settlement();
        let payload = GovernanceLogPayloadV1::DealSettlement(Box::new(settlement));

        assert_eq!(
            canonical_runtime_source_payload_len(&payload).expect("count canonical source"),
            encoded.len()
        );
        preflight_runtime_signed_dag_payload(&payload, encoded.len())
            .expect("small canonical payload fits every runtime DAG envelope");
        assert!(
            preflight_runtime_signed_dag_payload(&payload, encoded.len().saturating_add(1))
                .is_err(),
            "source-length substitution must fail before publication"
        );
    }

    #[test]
    fn runtime_dag_audit_rejects_substituted_generated_at_with_fresh_sidecar() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("seed signed runtime DAG");
        let index_path = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
        let mut index = runtime_index(temp.path());
        let head_generated_at = index
            .get("head_generated_at")
            .and_then(JsonValue::as_u64)
            .expect("head timestamp");
        index.as_object_mut().expect("runtime index object").insert(
            "generated_at".to_owned(),
            JsonValue::from(head_generated_at.saturating_add(1)),
        );
        let bytes = json::to_json_pretty(&index)
            .expect("encode tampered runtime index")
            .into_bytes();
        fs::write(&index_path, &bytes).expect("replace runtime index");
        write_digest_sidecar(publisher.root_guard(), &index_path, &bytes)
            .expect("replace index sidecar");

        let error = validate_existing_runtime_dag_root(
            temp.path(),
            publisher
                .runtime_dag_signer
                .as_ref()
                .expect("signed publisher"),
            publisher
                .runtime_dag_checkpoint_store
                .as_ref()
                .expect("signed publisher store"),
        )
        .expect_err("unchecked generated_at substitution must fail");
        assert!(error.to_string().contains("index and signed head"));
    }

    #[cfg(windows)]
    #[test]
    fn atomic_temp_recovery_deletes_the_exact_opened_windows_object() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
        let stale = temp_path_for_atomic(&target, 42_000, 1);
        fs::write(&stale, b"recover-exact-object").expect("seed matching crash temp");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain Windows producer root");
        isolate_recoverable_atomic_state_for_target(
            &root_guard,
            &target,
            GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
            "windows-runtime-dag-temp",
        )
        .expect("delete exact matching crash temp");
        assert!(!stale.exists());
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_temp_recovery_never_follows_substituted_parent() {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        {
            let publisher =
                signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
            let (settlement, encoded) = sample_settlement();
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect_err("retain sealed producer intent");
        }
        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load sealed producer intent")
            .expect("producer intent exists");
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
        let block_path = runtime_dag_producer_block_path_from_intent(temp.path(), &intent)
            .expect("resolve block path");
        let outside = temp.path().join("outside-runtime");
        let outside_blocks = outside.join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR);
        fs::create_dir_all(&outside_blocks).expect("create outside blocks directory");
        let outside_target = outside_blocks.join(
            block_path
                .file_name()
                .expect("producer block has a file name"),
        );
        let outside_temp = temp_path_for_atomic(&outside_target, 41_000, 9);
        fs::write(&outside_temp, b"must-remain-outside").expect("seed outside temp");
        std::os::unix::fs::symlink(&outside, temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR))
            .expect("substitute runtime parent");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("reopen publisher root")
            .with_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(1, 0x31),
                qualified_test_runtime_dag_checkpoint_store(checkpoint_store),
            )
            .expect_err("substituted parent must fail closed");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("real directory")
        );
        assert_eq!(
            fs::read(&outside_temp).expect("outside temp remains"),
            b"must-remain-outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_root_lock_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        std::os::unix::fs::symlink(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
            .expect("create publisher lock symlink");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("publisher lock symlink must fail closed");
        assert!(error.to_string().contains("must not be a symlink"));
        assert_eq!(
            fs::read(&target).expect("read lock target"),
            b"must remain untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_root_lock_rejects_hard_link() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        fs::hard_link(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
            .expect("create publisher lock hard link");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("publisher lock hard link must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
        assert_eq!(
            fs::read(&target).expect("read lock target"),
            b"must remain untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn governance_directory_policy_enforces_role_owner_and_sticky_ancestor_matrix() {
        let effective_uid = 42;
        let producer_uid = 77;
        let unrelated_uid = 99;

        assert!(governance_directory_policy_accepts(
            effective_uid,
            0o755,
            true,
            effective_uid,
            effective_uid,
            true,
        ));
        assert!(!governance_directory_policy_accepts(
            0,
            0o755,
            true,
            effective_uid,
            effective_uid,
            true,
        ));
        assert!(!governance_directory_policy_accepts(
            effective_uid,
            0o1777,
            true,
            effective_uid,
            effective_uid,
            true,
        ));
        assert!(governance_directory_policy_accepts(
            producer_uid,
            0o755,
            true,
            effective_uid,
            producer_uid,
            false,
        ));
        assert!(!governance_directory_policy_accepts(
            effective_uid,
            0o755,
            true,
            effective_uid,
            producer_uid,
            false,
        ));

        for owner in [0, effective_uid, producer_uid] {
            assert!(governance_directory_policy_accepts(
                owner,
                0o755,
                false,
                effective_uid,
                producer_uid,
                false,
            ));
        }
        assert!(governance_directory_policy_accepts(
            0,
            0o1777,
            false,
            effective_uid,
            producer_uid,
            false,
        ));
        assert!(!governance_directory_policy_accepts(
            unrelated_uid,
            0o1777,
            false,
            effective_uid,
            producer_uid,
            false,
        ));
        assert!(!governance_directory_policy_accepts(
            effective_uid,
            0o775,
            false,
            effective_uid,
            producer_uid,
            false,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn governance_root_guard_accepts_exact_canonical_root_and_trusted_sticky_parent() {
        let temp = tempdir().expect("canonical tempdir");
        let writer_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("canonical writer root");
        let source_guard = GovernanceFilesystemRootGuard::capture_source(temp.path())
            .expect("canonical source root");
        assert_eq!(writer_guard.root(), temp.path());
        assert_eq!(source_guard.root(), temp.path());
        writer_guard
            .revalidate()
            .expect("writer root remains pinned");
        source_guard
            .revalidate()
            .expect("source root remains pinned");

        let sticky = temp.path().join("sticky-parent");
        fs::create_dir(&sticky).expect("create sticky parent");
        fs::set_permissions(&sticky, fs::Permissions::from_mode(0o1777))
            .expect("set sticky-parent mode");
        let child = sticky.join("writer-root");
        fs::create_dir(&child).expect("create writer root");
        fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).expect("secure writer root");
        GovernanceFilesystemRootGuard::capture_writer(&child)
            .expect("trusted sticky ancestor is accepted")
            .revalidate()
            .expect("sticky-root identity remains pinned");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn governance_root_guard_rejects_descriptor_bound_acl_mutation_grant() {
        let temp = tempdir().expect("canonical tempdir");
        let status = std::process::Command::new("chmod")
            .args(["+a", "everyone allow add_file"])
            .arg(temp.path())
            .status()
            .expect("install macOS ACL mutation grant");
        assert!(status.success(), "install macOS ACL mutation grant");
        let result = GovernanceFilesystemRootGuard::capture_writer(temp.path());
        let cleanup = std::process::Command::new("chmod")
            .arg("-RN")
            .arg(temp.path())
            .status()
            .expect("remove macOS ACL mutation grant");
        assert!(cleanup.success(), "remove macOS ACL mutation grant");
        let error = result.expect_err("ACL mutation grant must fail root capture");
        assert!(
            error.to_string().contains("ACL mutation grant"),
            "unexpected ACL rejection: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn governance_root_guard_rejects_lexical_symlink_ancestor() {
        let temp = tempdir().expect("canonical tempdir");
        let real_parent = temp.path().join("real-parent");
        let real_root = real_parent.join("producer");
        fs::create_dir_all(&real_root).expect("create real producer root");
        let linked_parent = temp.path().join("linked-parent");
        std::os::unix::fs::symlink(&real_parent, &linked_parent)
            .expect("create lexical ancestor symlink");

        let error = GovernanceFilesystemRootGuard::capture_writer(&linked_parent.join("producer"))
            .expect_err("lexical symlink ancestor must fail closed");
        assert!(
            error.to_string().contains("canonical")
                || error.to_string().contains("symlink")
                || error.to_string().contains("real directory"),
            "unexpected lexical-symlink error: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_rejects_root_mode_drift_before_publication() {
        let temp = tempdir().expect("canonical tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o777))
            .expect("make root unsafe");
        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("unsafe root mode must fail before publication");
        assert!(
            error.to_string().contains("mode")
                || error.to_string().contains("group/world writable"),
            "unexpected mode-drift error: {error}"
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .expect("restore root mode for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_rejects_root_rename_replacement_without_touching_replacement() {
        let temp = tempdir().expect("canonical tempdir");
        let root = temp.path().join("producer");
        fs::create_dir(&root).expect("create producer root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("secure producer root");
        let publisher = FilesystemGovernancePublisher::try_new(root.clone()).expect("publisher");
        let detached = temp.path().join("producer.detached");
        fs::rename(&root, &detached).expect("detach pinned producer root");
        fs::create_dir(&root).expect("create replacement producer root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("secure replacement root");
        let marker = root.join("must-remain");
        fs::write(&marker, b"replacement").expect("seed replacement marker");

        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("root replacement must fail before publication");
        assert!(
            error.to_string().contains("identity") || error.to_string().contains("changed"),
            "unexpected root-replacement error: {error}"
        );
        assert_eq!(
            fs::read(&marker).expect("replacement marker remains"),
            b"replacement"
        );
        assert!(!root.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists());
        assert!(!detached.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists());
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_rejects_ancestor_replacement_and_symlink_without_writing_target() {
        let temp = tempdir().expect("canonical tempdir");
        let ancestor = temp.path().join("ancestor");
        let root = ancestor.join("producer");
        fs::create_dir_all(&root).expect("create producer root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("secure producer root");
        let publisher = FilesystemGovernancePublisher::try_new(root.clone()).expect("publisher");
        let detached = temp.path().join("ancestor.detached");
        fs::rename(&ancestor, &detached).expect("detach pinned ancestor");
        fs::create_dir(&ancestor).expect("create replacement ancestor");
        std::os::unix::fs::symlink(detached.join("producer"), &root)
            .expect("substitute producer root symlink");
        let marker = detached.join("producer").join("must-remain");
        fs::write(&marker, b"detached").expect("seed detached marker");

        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("ancestor replacement must fail before publication");
        assert!(
            error.to_string().contains("identity")
                || error.to_string().contains("changed")
                || error.to_string().contains("real directory"),
            "unexpected ancestor-replacement error: {error}"
        );
        assert_eq!(
            fs::read(&marker).expect("detached marker remains"),
            b"detached"
        );
        assert!(!detached.join("producer/settlements").exists());
    }

    #[test]
    fn runtime_dag_signer_rejects_invalid_handle_and_oversized_identity() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        let public_key = signer.public_key();

        validate_runtime_handle(
            "pkcs11:prod/governance-dag.primary-v1_slot-a",
            "governance runtime DAG signer",
        )
        .expect("canonical production runtime handle");
        for handle in [
            "contains whitespace",
            "https://operator:secret@governance-signer",
            "https://governance-signer/path?credential=secret",
            "https://governance-signer/path#fragment",
            "pkcs11:prod/%67overnance-signer",
            "pkcs11:prod\\governance-signer",
        ] {
            let error = GovernanceRuntimeDagSigner::try_new(
                handle.to_owned(),
                peer_id.clone(),
                public_key,
                test_runtime_dag_signer_qualification(),
                signer.clone(),
            )
            .expect_err("forbidden runtime-handle character must fail closed");
            assert!(error.to_string().contains("canonical credential-free"));
        }

        let error = GovernanceRuntimeDagSigner::try_new(
            signer.handle().to_owned(),
            vec![0x41; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1],
            public_key,
            test_runtime_dag_signer_qualification(),
            signer,
        )
        .expect_err("oversized governance publisher identity must fail closed");
        assert!(
            error
                .to_string()
                .contains("publisher peer id exceeds 128 bytes")
        );
    }

    #[test]
    fn runtime_dag_signer_rejects_test_marked_stale_and_drifting_provider() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        let error = GovernanceRuntimeDagSigner::try_new(
            "pkcs11:governance-dag:test".to_owned(),
            peer_id.clone(),
            signer.public_key(),
            test_runtime_dag_signer_qualification(),
            signer,
        )
        .expect_err("test-marked configured handle must fail closed");
        assert!(error.to_string().contains("test-marked"));

        let mut stale = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
        stale.qualification_error = Some("hsm_token=must-never-escape".to_owned());
        let stale = Arc::new(stale);
        let error = GovernanceRuntimeDagSigner::try_new(
            stale.handle().to_owned(),
            peer_id.clone(),
            stale.public_key(),
            test_runtime_dag_signer_qualification(),
            stale,
        )
        .expect_err("stale provider must fail startup qualification");
        assert!(error.to_string().contains("stale"));
        assert!(!error.to_string().contains("must-never-escape"));

        let invalid = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        invalid.qualification_revision.store(0, Ordering::SeqCst);
        let error = GovernanceRuntimeDagSigner::try_new(
            invalid.handle().to_owned(),
            peer_id.clone(),
            invalid.public_key(),
            test_runtime_dag_signer_qualification(),
            invalid,
        )
        .expect_err("zero provider revision must fail startup qualification");
        assert!(error.to_string().contains("invalid policy qualification"));

        for expected_qualification in [
            GovernanceDagRuntimeProviderQualificationV1::new(
                2,
                TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
            ),
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x72; 32]),
        ] {
            let substituted = Arc::new(TestRuntimeDagSigner::new(
                "pkcs11:governance-dag:primary",
                &peer_id,
                0x31,
            ));
            let error = GovernanceRuntimeDagSigner::try_new(
                substituted.handle().to_owned(),
                peer_id.clone(),
                substituted.public_key(),
                expected_qualification,
                substituted,
            )
            .expect_err("substituted configured qualification must fail startup");
            assert!(
                error
                    .to_string()
                    .contains("does not match configured revision and digest")
            );
        }

        let drifting = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        drifting
            .drift_on_second_qualification_read
            .store(true, Ordering::SeqCst);
        let error = GovernanceRuntimeDagSigner::try_new(
            drifting.handle().to_owned(),
            peer_id.clone(),
            drifting.public_key(),
            test_runtime_dag_signer_qualification(),
            drifting,
        )
        .expect_err("qualification drift on the second startup read must fail closed");
        assert!(
            error
                .to_string()
                .contains("policy changed during startup qualification")
        );

        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            signer.handle().to_owned(),
            peer_id,
            signer.public_key(),
            test_runtime_dag_signer_qualification(),
            signer.clone(),
        )
        .expect("qualify stable signer");
        signer.qualification_revision.store(2, Ordering::SeqCst);
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("provider policy drift must fail closed");
        assert!(error.to_string().contains("policy changed"));

        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            b"12D3KooWRuntimeDagPublisher",
            0x31,
        ));
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            signer.handle().to_owned(),
            signer.publisher_peer_id().to_vec(),
            signer.public_key(),
            test_runtime_dag_signer_qualification(),
            signer.clone(),
        )
        .expect("qualify stable signer");
        signer.drift_during_sign.store(true, Ordering::SeqCst);
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("provider policy drift during signing must discard the signature");
        assert!(error.to_string().contains("policy changed"));
    }

    #[test]
    fn runtime_dag_signer_rejects_handle_peer_and_public_key_mismatch() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        let public_key = signer.public_key();
        let mismatched_public_key =
            TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x32).public_key();

        for (handle, peer, key, expected) in [
            (
                "pkcs11:governance-dag:other",
                peer_id.clone(),
                public_key,
                "handle does not match",
            ),
            (
                signer.handle(),
                b"12D3KooWOtherPublisher".to_vec(),
                public_key,
                "publisher identity does not match",
            ),
            (
                signer.handle(),
                peer_id.clone(),
                mismatched_public_key,
                "public key does not match",
            ),
        ] {
            let error = GovernanceRuntimeDagSigner::try_new(
                handle.to_owned(),
                peer,
                key,
                test_runtime_dag_signer_qualification(),
                signer.clone(),
            )
            .expect_err("mismatched runtime signer must fail closed");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn runtime_dag_signer_rejects_malformed_and_weak_ed25519_keys() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        for (public_key, expected) in [
            ([0xFF; 32], "not canonical Ed25519"),
            (
                {
                    let mut identity = [0_u8; 32];
                    identity[0] = 1;
                    identity
                },
                "non-canonical or weak",
            ),
        ] {
            let mut signer =
                TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
            signer.public_key_override = Some(public_key);
            let signer = Arc::new(signer);
            let error = GovernanceRuntimeDagSigner::try_new(
                signer.handle().to_owned(),
                peer_id.clone(),
                public_key,
                test_runtime_dag_signer_qualification(),
                signer,
            )
            .expect_err("malformed or weak Ed25519 key must fail during provider binding");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn runtime_dag_signer_redacts_provider_error_and_rejects_wrong_signature() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let mut refusing =
            TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
        refusing.refuse_with = Some("bearer=must-never-escape".to_owned());
        let refusing = Arc::new(refusing);
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            refusing.handle().to_owned(),
            peer_id.clone(),
            refusing.public_key(),
            test_runtime_dag_signer_qualification(),
            refusing,
        )
        .expect("bind refusing test provider");
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("provider outage must fail closed");
        assert!(error.to_string().contains("refused"));
        assert!(!error.to_string().contains("must-never-escape"));

        let mut corrupt =
            TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
        corrupt.corrupt_signature = true;
        let corrupt = Arc::new(corrupt);
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            corrupt.handle().to_owned(),
            peer_id,
            corrupt.public_key(),
            test_runtime_dag_signer_qualification(),
            corrupt,
        )
        .expect("bind corrupt test provider");
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("wrong signature must fail closed");
        assert!(error.to_string().contains("another key or payload"));
    }

    #[test]
    fn filesystem_publisher_serializes_concurrent_index_and_signed_head_updates() {
        const PUBLICATION_COUNT: usize = 16;

        let temp = tempdir().expect("tempdir");
        let publisher = Arc::new(signed_runtime_publisher(temp.path()));
        let (template, _) = sample_settlement();
        let threads = (0..PUBLICATION_COUNT)
            .map(|index| {
                let publisher = Arc::clone(&publisher);
                let mut settlement = template.clone();
                let marker = u8::try_from(index + 1).expect("small publication count");
                settlement.deal_id = [marker; 32];
                settlement.ledger.deal_id = settlement.deal_id;
                settlement.ledger.snapshot_id = settlement
                    .ledger
                    .derive_snapshot_id()
                    .expect("reseal ledger snapshot");
                settlement.settlement_id = settlement
                    .derive_settlement_id()
                    .expect("reseal settlement");
                thread::spawn(move || {
                    let encoded = norito::to_bytes(&settlement).expect("encode settlement");
                    publisher
                        .publish_deal_settlement(&settlement, &encoded)
                        .expect("publish settlement concurrently");
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().expect("publisher thread");
        }

        let publish_index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            publish_index.get("entry_count").and_then(JsonValue::as_u64),
            Some(PUBLICATION_COUNT as u64)
        );
        let entries = publish_index
            .get("entries")
            .and_then(JsonValue::as_array)
            .expect("publish index entries");
        assert_eq!(entries.len(), PUBLICATION_COUNT);
        for (expected_position, entry) in entries.iter().enumerate() {
            assert_eq!(
                entry.get("position").and_then(JsonValue::as_u64),
                Some(expected_position as u64)
            );
        }

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index.get("block_count").and_then(JsonValue::as_u64),
            Some(PUBLICATION_COUNT as u64)
        );
        assert_eq!(
            runtime_index
                .get("blocks")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(PUBLICATION_COUNT)
        );
    }

    #[test]
    fn filesystem_publisher_poisoned_transaction_lock_fails_before_writes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            let _guard = publisher
                .publication_lock
                .lock()
                .expect("publication lock starts healthy");
            panic!("poison publication transaction lock");
        }));
        assert!(poisoned.is_err());

        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("poisoned publisher must fail closed");
        assert!(error.to_string().contains("transaction lock is poisoned"));
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "poison detection must happen before artifact writes"
        );
        assert_empty_publication_authority(temp.path());
    }

    #[test]
    fn filesystem_publisher_appends_signed_runtime_dag_for_supported_payloads() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish settlement into runtime DAG");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("duplicate publish is idempotent");
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("deal_settlement"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let (snapshot, snapshot_encoded) = sample_reputation_snapshot();
        publisher
            .publish_reputation_snapshot(&snapshot, &snapshot_encoded)
            .expect("publish reputation snapshot into runtime DAG");

        let (finance_report, finance_encoded) = sample_appeal_finance_report();
        publisher
            .publish_appeal_finance_report(&finance_report, &finance_encoded)
            .expect("publish appeal finance report into runtime DAG");

        let (finance_rollup, rollup_encoded) = sample_appeal_finance_weekly_rollup();
        publisher
            .publish_appeal_finance_weekly_rollup(&finance_rollup, &rollup_encoded)
            .expect("publish appeal finance weekly rollup into runtime DAG");

        let (finance_receipt, receipt_encoded) = sample_appeal_finance_settlement_receipt();
        publisher
            .publish_appeal_finance_settlement_receipt(&finance_receipt, &receipt_encoded)
            .expect("publish appeal finance settlement receipt into runtime DAG");

        let (transparency_publication, transparency_encoded) =
            sample_transparency_ledger_publication();
        publisher
            .publish_transparency_ledger_publication(
                &transparency_publication,
                &transparency_encoded,
                None,
            )
            .expect("publish transparency ledger publication into runtime DAG");

        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(6)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("reputation_snapshot"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[2].sequence, 2);
        assert_eq!(blocks[3].sequence, 3);
        assert_eq!(blocks[4].sequence, 4);
        assert_eq!(blocks[5].sequence, 5);
        assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
        assert_eq!(blocks[2].prev_block_cid, Some(blocks[1].block_cid.clone()));
        assert_eq!(blocks[3].prev_block_cid, Some(blocks[2].block_cid.clone()));
        assert_eq!(blocks[4].prev_block_cid, Some(blocks[3].block_cid.clone()));
        assert_eq!(blocks[5].prev_block_cid, Some(blocks[4].block_cid.clone()));
        assert_eq!(
            blocks[1].node.prev_cid,
            Some(blocks[0].node.node_cid.clone())
        );
        assert_eq!(
            blocks[2].node.prev_cid,
            Some(blocks[1].node.node_cid.clone())
        );
        assert_eq!(
            blocks[3].node.prev_cid,
            Some(blocks[2].node.node_cid.clone())
        );
        assert_eq!(
            blocks[4].node.prev_cid,
            Some(blocks[3].node.node_cid.clone())
        );
        assert_eq!(
            blocks[5].node.prev_cid,
            Some(blocks[4].node.node_cid.clone())
        );
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::DealSettlement(value) => {
                assert_eq!(value.deal_id, settlement.deal_id);
            }
            other => panic!("unexpected first runtime DAG payload: {other:?}"),
        }
        match &blocks[1].node.payload {
            GovernanceLogPayloadV1::SignedReputationSnapshot(value) => {
                assert_eq!(value.snapshot.snapshot_id, snapshot.snapshot.snapshot_id);
            }
            other => panic!("unexpected second runtime DAG payload: {other:?}"),
        }
        match &blocks[2].node.payload {
            GovernanceLogPayloadV1::AppealFinanceReport(value) => {
                assert_eq!(value.report_id, finance_report.report_id);
                assert_eq!(value.case_id, finance_report.case_id);
            }
            other => panic!("unexpected third runtime DAG payload: {other:?}"),
        }
        match &blocks[3].node.payload {
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
                assert_eq!(value.cycle, finance_rollup.cycle);
                assert_eq!(value.report_count, finance_rollup.report_count);
                assert_eq!(value.total_deposit_xor, finance_rollup.total_deposit_xor);
            }
            other => panic!("unexpected fourth runtime DAG payload: {other:?}"),
        }
        match &blocks[4].node.payload {
            GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
                assert_eq!(value.receipt_id, finance_receipt.receipt_id);
                assert_eq!(value.tx_hash_hex, finance_receipt.tx_hash_hex);
                assert_eq!(
                    value.reconciliation_digest_hex,
                    finance_receipt.reconciliation_digest_hex
                );
            }
            other => panic!("unexpected fifth runtime DAG payload: {other:?}"),
        }
        match &blocks[5].node.payload {
            GovernanceLogPayloadV1::ExternalPayload(value) => {
                assert_eq!(value.payload_kind, "transparency_ledger_publication");
                assert_eq!(
                    value.payload_version,
                    MODERATION_LEDGER_PUBLICATION_VERSION_V1
                );
                assert_eq!(
                    value.encoded_blake3,
                    *blake3::hash(&transparency_encoded).as_bytes()
                );
                assert_eq!(value.encoded_len, transparency_encoded.len() as u64);
                assert_eq!(value.encoded_payload, transparency_encoded);
                assert_eq!(
                    value
                        .metadata
                        .iter()
                        .map(|item| item.key.as_str())
                        .collect::<Vec<_>>(),
                    vec![
                        "block_hash_hex",
                        "cycle_id_hex",
                        "entry_count",
                        "entry_root_hex",
                        "publication_hash_hex"
                    ]
                );
            }
            other => panic!("unexpected sixth runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_keeps_full_history_and_signs_checkpoint_window_with_one_identity() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (template, _) = sample_settlement();

        for marker in 1_u8..=GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u8 {
            let mut settlement = template.clone();
            settlement.deal_id = [marker; 32];
            settlement.ledger.deal_id = settlement.deal_id;
            settlement.ledger.snapshot_id = settlement
                .ledger
                .derive_snapshot_id()
                .expect("reseal ledger snapshot");
            settlement.settlement_id = settlement
                .derive_settlement_id()
                .expect("reseal settlement");
            let encoded = norito::to_bytes(&settlement).expect("encode settlement");
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect("publish settlement into runtime DAG");
        }

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head_at_window: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        assert_eq!(
            head_at_window.block_count,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u64
        );
        assert_eq!(head_at_window.checkpoint_cid, None);

        let mut settlement = template;
        settlement.deal_id = [0xFF; 32];
        settlement.ledger.deal_id = settlement.deal_id;
        settlement.ledger.snapshot_id = settlement
            .ledger
            .derive_snapshot_id()
            .expect("reseal ledger snapshot");
        settlement.settlement_id = settlement
            .derive_settlement_id()
            .expect("reseal settlement");
        let encoded = norito::to_bytes(&settlement).expect("encode settlement");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish first checkpointed settlement");

        let index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        assert_eq!(
            blocks.len(),
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
            "checkpointing must not truncate the root history"
        );
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[0].prev_block_cid, None);
        assert_eq!(blocks[0].node.prev_cid, None);
        for (position, pair) in blocks.windows(2).enumerate() {
            assert_eq!(pair[1].sequence, (position + 1) as u64);
            assert_eq!(pair[1].prev_block_cid, Some(pair[0].block_cid.clone()));
            assert_eq!(pair[1].node.prev_cid, Some(pair[0].node.node_cid.clone()));
        }

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        assert_eq!(head.block_count, blocks.len() as u64);
        assert_eq!(head.checkpoint_cid, Some(blocks[1].block_cid.clone()));
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("full root chain validates against checkpointed head");
        validate_governance_dag_head_against_chain_v1(
            &head,
            &blocks[blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1..],
        )
        .expect("canonical checkpoint tail validates against checkpointed head");

        let governed_public_key = &head.head_signature.public_key;
        assert_eq!(
            head.head_signature.algorithm,
            GovernanceSignatureAlgorithm::Ed25519
        );
        for block in &blocks {
            assert_eq!(block.publisher_peer_id, head.publisher_peer_id);
            assert_eq!(block.node.publisher_peer_id, head.publisher_peer_id);
            assert_eq!(
                block.block_signature.algorithm,
                GovernanceSignatureAlgorithm::Ed25519
            );
            assert_eq!(
                block.node.publisher_signature.algorithm,
                GovernanceSignatureAlgorithm::Ed25519
            );
            assert_eq!(&block.block_signature.public_key, governed_public_key);
            assert_eq!(
                &block.node.publisher_signature.public_key,
                governed_public_key
            );
        }
    }

    #[test]
    fn filesystem_publisher_writes_moderation_ballot_event_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (event, encoded) = sample_moderation_ballot_event();

        publisher
            .publish_moderation_ballot_event(&event, &encoded)
            .expect("publish moderation ballot event");

        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "moderation_ballot_event");
        let bytes = fs::read(&encoded_path).expect("read moderation event payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsModerationBallotGovernanceEventV1 =
            norito::decode_from_bytes(&bytes).expect("decode moderation event payload");
        assert_eq!(decoded, event);
        assert!(json_path.exists());

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("moderation_ballot_event"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("moderation_ballot_event"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        let expected_provenance =
            test_submission_provenance(crate::GovernanceSubmissionOriginV1::AppealFinanceReport)
                .to_dag_provenance();
        assert_eq!(
            blocks[0].node.submission_provenance.as_ref(),
            Some(&expected_provenance)
        );
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::ModerationBallotEvent(value) => {
                assert_eq!(value.case_id, event.case_id);
                assert_eq!(value.round_id, event.round_id);
                assert_eq!(value.kind, event.kind);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn fused_privacy_publisher_retries_the_exact_request_idempotently() {
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
        let request = sample_fenced_request(7, None);

        let first = publisher
            .compare_and_append_privacy_classified(&request)
            .expect("first fused append");
        let retried = publisher
            .compare_and_append_privacy_classified(&request)
            .expect("idempotent fused retry");

        assert_eq!(retried, first);
        assert_eq!(provider.append_count(), 1);
        assert_eq!(provider.head(), Some(first.included_head()));
    }

    #[test]
    fn fused_privacy_target_deduplicates_same_lease_before_fencing() {
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
        let first_request = sample_fenced_request(7, None);
        let first = publisher
            .compare_and_append_privacy_classified(&first_request)
            .expect("first fused append");
        let (publication, encoded) = sample_privacy_publication();
        let same_lease_authorization =
            sample_privacy_authorization(&publication, &encoded, first_request.fencing_token());
        let same_lease_request = FencedPrivacyPublicationRequestV1::try_new(
            same_lease_authorization,
            &publication,
            encoded,
            Some(first.included_head()),
            first.included_head().fencing_floor(),
        )
        .expect("same-lease lookup request remains structurally valid");

        let duplicate = publisher
            .compare_and_append_privacy_classified(&same_lease_request)
            .expect("stable scope lookup precedes stale-fence rejection");

        assert_eq!(
            duplicate.disposition(),
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded
        );
        assert_eq!(duplicate.included_head(), first.included_head());
        assert_eq!(duplicate.readback_head(), first.readback_head());
        assert_eq!(provider.append_count(), 1);
    }

    #[test]
    fn fused_privacy_target_rejects_conflicting_release_evidence_for_scope() {
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
        let first_request = sample_fenced_request(7, None);
        let first = publisher
            .compare_and_append_privacy_classified(&first_request)
            .expect("first fused append");
        let conflicting_spec = SamplePrivacyReleaseSpec {
            release_record_digest: [0xB8; 32],
            ..SamplePrivacyReleaseSpec::primary()
        };
        let (publication, encoded) = sample_privacy_publication();
        let conflicting_authorization =
            sample_privacy_authorization_for(conflicting_spec, &publication, &encoded, 8, None);
        let conflicting_request = FencedPrivacyPublicationRequestV1::try_new(
            conflicting_authorization,
            &publication,
            encoded,
            Some(first.included_head()),
            first.included_head().fencing_floor(),
        )
        .expect("conflicting stable-scope request");

        let error = publisher
            .compare_and_append_privacy_classified(&conflicting_request)
            .expect_err("one release scope cannot change its release evidence");

        assert!(
            error
                .error
                .to_string()
                .contains("identity conflicts with an existing publication")
        );
        assert!(!error.may_have_appended);
        assert_eq!(provider.append_count(), 1);
        assert_eq!(provider.head(), Some(first.included_head()));
    }

    #[test]
    fn fenced_head_reader_qualification_rejects_substitution_staleness_and_test_markers() {
        let target = Arc::new(TestFencedTransparencyPublisher::new());

        let substituted = Arc::new(TestFencedTransparencyHeadReader::with_handle(
            Arc::clone(&target),
            "https-pinned:governance:fenced-privacy-head-secondary",
        ));
        let substituted: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = substituted;
        let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
            TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
            test_fenced_head_reader_qualification(),
            substituted,
        )
        .expect_err("substituted reader identity must fail");
        assert!(error.to_string().contains("does not match configuration"));

        let stale = test_fenced_head_reader(Arc::clone(&target));
        stale.set_revision(2);
        let stale: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = stale;
        let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
            TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
            test_fenced_head_reader_qualification(),
            stale,
        )
        .expect_err("stale reader policy must fail");
        assert!(error.to_string().contains("does not match configuration"));

        let test_marked_handle = "https-pinned:governance:fenced-privacy-head-test";
        let test_marked = Arc::new(TestFencedTransparencyHeadReader::with_handle(
            target,
            test_marked_handle,
        ));
        let test_marked: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = test_marked;
        let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
            test_marked_handle.to_owned(),
            test_fenced_head_reader_qualification(),
            test_marked,
        )
        .expect_err("test-marked reader must fail");
        assert!(error.to_string().contains("test-marked"));
    }

    #[test]
    fn fused_writer_and_head_reader_require_one_exact_runtime_binding() {
        let target = Arc::new(TestFencedTransparencyPublisher::new());
        let writer = qualified_test_fenced_publisher(Arc::clone(&target));
        let cases = [
            (
                "hsm:governance:fenced-privacy-secondary",
                GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    TEST_FENCED_PUBLISHER_POLICY_DIGEST,
                ),
            ),
            (
                TEST_FENCED_PUBLISHER_HANDLE,
                GovernanceDagRuntimeProviderQualificationV1::new(
                    2,
                    TEST_FENCED_PUBLISHER_POLICY_DIGEST,
                ),
            ),
            (
                TEST_FENCED_PUBLISHER_HANDLE,
                GovernanceDagRuntimeProviderQualificationV1::new(1, [0x74; 32]),
            ),
        ];

        for (handle, qualification) in cases {
            let reader = Arc::new(TestFencedTransparencyHeadReader::with_binding(
                Arc::clone(&target),
                handle,
                qualification.revision,
                qualification.policy_digest,
            ));
            let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = reader;
            let reader = QualifiedFencedTransparencyHeadReaderV1::try_new(
                handle.to_owned(),
                qualification,
                reader,
            )
            .expect("independently qualify mismatched reader");
            let error = ensure_fenced_privacy_runtime_bindings_match(&writer, &reader)
                .expect_err("writer and reader binding mismatch must fail");
            assert!(error.to_string().contains("one exact identity"));
        }
    }

    #[test]
    fn authenticated_head_bootstrap_rejects_read_failure_and_malformed_head_without_cache() {
        let failed_root = tempdir().expect("failed root");
        let failed_target = Arc::new(TestFencedTransparencyPublisher::new());
        let failed_reader = test_fenced_head_reader(failed_target);
        let qualified_failed_reader = qualified_test_fenced_head_reader(Arc::clone(&failed_reader));
        failed_reader.set_fail_read(true);
        let error = FilesystemGovernancePublisher::try_new(failed_root.path().to_path_buf())
            .expect("failed publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_failed_reader)
            .expect_err("failed authenticated read must abort bootstrap");
        assert!(error.to_string().contains("failed authentication"));
        assert!(!fenced_privacy_head_sync_path(failed_root.path()).exists());

        let malformed_root = tempdir().expect("malformed root");
        let malformed_target = Arc::new(TestFencedTransparencyPublisher::new());
        let malformed_reader = test_fenced_head_reader(malformed_target);
        malformed_reader.override_head(Some(FencedTransparencyTargetHeadV1 {
            version: crate::FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            generation: 0,
            head_digest: [0xA1; 32],
            fencing_floor: 1,
        }));
        let error = FilesystemGovernancePublisher::try_new(malformed_root.path().to_path_buf())
            .expect("malformed publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                malformed_reader,
            ))
            .expect_err("malformed authoritative head must abort bootstrap");
        assert!(error.to_string().contains("failed authentication"));
        assert!(!fenced_privacy_head_sync_path(malformed_root.path()).exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn fenced_privacy_pending_logical_delete_is_typed_and_idempotent() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = qualified_test_fenced_publisher(provider);
        let request = sample_fenced_request(7, None);
        let pending = FencedPrivacyPendingRequestV1::from_request(&request, &publisher)
            .expect("build pending request");

        write_fenced_privacy_pending_request(temp.path(), &pending).expect("persist pending");
        assert_eq!(
            read_fenced_privacy_pending_request(temp.path()).expect("read pending"),
            Some(pending)
        );
        remove_fenced_privacy_pending_request(temp.path()).expect("write pending tombstone");
        assert_fenced_privacy_pending_logically_cleared(temp.path());
        remove_fenced_privacy_pending_request(temp.path()).expect("repeat pending tombstone");
        assert_fenced_privacy_pending_logically_cleared(temp.path());
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert!(
                temp.path()
                    .join(".fenced-privacy-pending.to.retained-v1-0000")
                    .is_file(),
                "the exact pending generation remains retained"
            );
            assert!(
                !temp
                    .path()
                    .join(".fenced-privacy-pending.to.retained-v1-0001")
                    .exists(),
                "an exact tombstone retry must not consume another retained slot"
            );
        }
    }

    #[test]
    fn persisted_pending_and_head_sync_reject_qualified_target_rotation() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
        let reader =
            qualified_test_fenced_head_reader(test_fenced_head_reader(Arc::clone(&provider)));
        let (publication, encoded) = sample_privacy_publication();
        let request = sample_fenced_request(7, None);
        let mut pending = FencedPrivacyPendingRequestV1::from_request(&request, &publisher)
            .expect("build pending request");
        pending.target_handle = "hsm:governance:fenced-privacy-retired".to_owned();
        write_fenced_privacy_pending_request(temp.path(), &pending)
            .expect("persist old-target pending request");
        let restored = read_fenced_privacy_pending_request(temp.path())
            .expect("read pending request")
            .expect("pending request exists");

        let error = restored
            .reconstruct_request(request.authorization(), &publication, &encoded, &publisher)
            .expect_err("pending request must remain bound to its qualified target");
        assert!(
            error
                .to_string()
                .contains("belongs to a different qualified target")
        );

        let receipt = FencedPrivacyPublicationReceiptV1::from_verified_append(
            &request,
            TEST_FENCED_PUBLISHER_HANDLE,
            test_fenced_publisher_qualification(),
        )
        .expect("build verified cache receipt");
        let mut retired_cache = FencedPrivacyPublicationCacheV1::from_verified_receipt(
            &request,
            &receipt,
            Some(receipt.included_head()),
        )
        .expect("build verified publication cache");
        retired_cache.target_handle = "hsm:governance:fenced-privacy-retired".to_owned();
        write_fenced_privacy_head_cache(temp.path(), &retired_cache)
            .expect("persist retired target cache");
        let error = synchronize_fenced_privacy_authoritative_head(temp.path(), &reader, None)
            .expect_err("persisted publication cache must not rotate targets implicitly");
        assert!(
            error
                .to_string()
                .contains("publication cache belongs to a different qualified target")
        );
        fs::remove_file(fenced_privacy_head_cache_path(temp.path()))
            .expect("remove retired cache before reader-binding check");

        let retired_sync = FencedPrivacyAuthoritativeHeadSyncV1 {
            version: GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1,
            reader_handle: "https-pinned:governance:fenced-privacy-retired".to_owned(),
            reader_revision: 1,
            reader_policy_digest: [0x73; 32],
            authoritative_head: None,
            ancestry_proof_digest: [0x74; 32],
        };
        write_fenced_privacy_head_sync(temp.path(), &retired_sync)
            .expect("persist retired reader binding");
        let error = synchronize_fenced_privacy_authoritative_head(temp.path(), &reader, None)
            .expect_err("persisted reader binding must not rotate implicitly");
        assert!(
            error
                .to_string()
                .contains("belongs to a different qualified reader")
        );
        assert_eq!(provider.append_count(), 0);
        assert!(provider.head().is_none());
    }

    #[test]
    fn authenticated_head_sync_rejects_rollbacks_forks_and_stale_reader() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let qualified_writer = qualified_test_fenced_publisher(Arc::clone(&provider));
        let first_request = sample_fenced_request(7, None);
        let first_receipt = qualified_writer
            .compare_and_append_privacy_classified(&first_request)
            .expect("seed first authoritative head");
        let next_spec = SamplePrivacyReleaseSpec::next();
        let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
        let next_authorization =
            sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 8, None);
        let second_request = FencedPrivacyPublicationRequestV1::try_new(
            next_authorization,
            &next_publication,
            next_encoded,
            Some(first_receipt.included_head()),
            first_receipt.included_head().fencing_floor(),
        )
        .expect("second distinct fenced privacy request");
        let second_receipt = qualified_writer
            .compare_and_append_privacy_classified(&second_request)
            .expect("seed second authoritative head");
        let authoritative_head = second_receipt.included_head();
        let head_reader = test_fenced_head_reader(Arc::clone(&provider));
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher")
            .with_qualified_fenced_privacy_publisher(qualified_writer)
            .expect("attach fused publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                Arc::clone(&head_reader),
            ))
            .expect("bootstrap current authoritative head");
        assert_eq!(
            read_fenced_privacy_head_sync(temp.path())
                .expect("read synchronized head")
                .and_then(|sync| sync.authoritative_head),
            Some(authoritative_head)
        );
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, 9);

        head_reader.override_head(Some(first_receipt.included_head()));
        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("generation rollback must fail");
        assert!(error.to_string().contains("failed authentication"));

        head_reader.override_head(None);
        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("genesis rollback must fail");
        assert!(error.to_string().contains("failed authentication"));

        head_reader.override_head(Some(
            FencedTransparencyTargetHeadV1::try_new(
                authoritative_head.generation(),
                [0xA2; 32],
                authoritative_head.fencing_floor(),
            )
            .expect("valid substituted head"),
        ));
        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("same-generation substitution must fail");
        assert!(error.to_string().contains("failed authentication"));

        head_reader.override_head(Some(
            FencedTransparencyTargetHeadV1::try_new(
                authoritative_head.generation() + 1,
                [0xA3; 32],
                authoritative_head.fencing_floor(),
            )
            .expect("structurally valid non-monotonic head"),
        ));
        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("unproven higher fork must fail");
        assert!(error.to_string().contains("failed authentication"));

        head_reader.override_head(Some(authoritative_head));
        head_reader.set_revision(2);
        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("stale reader qualification must fail");
        assert!(error.to_string().contains("changed after qualification"));

        assert_no_privacy_publication_side_effects(temp.path());
        assert!(!fenced_privacy_pending_path(temp.path()).exists());
        assert_eq!(
            read_fenced_privacy_head_sync(temp.path())
                .expect("read retained synchronized head")
                .and_then(|sync| sync.authoritative_head),
            Some(authoritative_head),
            "rejected reads must not roll back the authenticated cache"
        );
    }

    #[test]
    fn authenticated_head_sync_rejects_publication_at_unrelated_valid_ancestor() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let writer = qualified_test_fenced_publisher(Arc::clone(&provider));
        let first_request = sample_fenced_request(7, None);
        let first_receipt = writer
            .compare_and_append_privacy_classified(&first_request)
            .expect("seed first release");

        let next_spec = SamplePrivacyReleaseSpec::next();
        let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
        let next_authorization =
            sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 8, None);
        let second_request = FencedPrivacyPublicationRequestV1::try_new(
            next_authorization,
            &next_publication,
            next_encoded,
            Some(first_receipt.included_head()),
            first_receipt.included_head().fencing_floor(),
        )
        .expect("second release request");
        let second_receipt = writer
            .compare_and_append_privacy_classified(&second_request)
            .expect("seed unrelated later release");

        let (publication, encoded) = sample_privacy_publication();
        let duplicate_authorization = sample_privacy_authorization(&publication, &encoded, 9);
        let duplicate_request = FencedPrivacyPublicationRequestV1::try_new(
            duplicate_authorization,
            &publication,
            encoded,
            Some(second_receipt.included_head()),
            second_receipt.included_head().fencing_floor(),
        )
        .expect("duplicate release lookup request");
        let forged_receipt = FencedPrivacyPublicationReceiptV1::from_verified_existing(
            &duplicate_request,
            TEST_FENCED_PUBLISHER_HANDLE,
            test_fenced_publisher_qualification(),
            second_receipt.included_head(),
            second_receipt.included_head(),
        )
        .expect("structurally valid receipt at an unrelated ancestor");
        let reader =
            qualified_test_fenced_head_reader(test_fenced_head_reader(Arc::clone(&provider)));

        let error = synchronize_fenced_privacy_authoritative_head(
            temp.path(),
            &reader,
            Some(&forged_receipt),
        )
        .expect_err("ancestry alone must not prove a different publication identity");

        assert!(error.to_string().contains("failed authentication"));
        assert!(!fenced_privacy_head_sync_path(temp.path()).exists());
        assert_eq!(provider.append_count(), 2);
        assert_ne!(
            first_receipt.included_head(),
            second_receipt.included_head()
        );
    }

    #[test]
    fn filesystem_privacy_publication_replays_cached_request_after_lease_rotation() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let head_reader = test_fenced_head_reader(Arc::clone(&provider));
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher")
            .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
                &provider,
            )))
            .expect("attach fused publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                head_reader,
            ))
            .expect("attach authenticated head reader");
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, 8);
        let rotated_authorization = sample_privacy_authorization(&publication, &encoded, 9);

        publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect("first filesystem publication");
        let first_cache = read_fenced_privacy_head_cache(temp.path())
            .expect("read first cache")
            .expect("first cache exists");
        publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&rotated_authorization),
            )
            .expect("filesystem exact retry after lease rotation");
        let retry_cache = read_fenced_privacy_head_cache(temp.path())
            .expect("read retry cache")
            .expect("retry cache exists");

        assert_eq!(retry_cache, first_cache);
        assert_eq!(retry_cache.last_fencing_token, 8);
        assert_eq!(retry_cache.authoritative_head.fencing_floor(), 8);
        assert_eq!(provider.append_count(), 1);
        assert_eq!(provider.head(), Some(retry_cache.authoritative_head));

        let conflicting_spec = SamplePrivacyReleaseSpec {
            release_record_digest: [0xB8; 32],
            ..SamplePrivacyReleaseSpec::primary()
        };
        let conflicting_authorization =
            sample_privacy_authorization_for(conflicting_spec, &publication, &encoded, 10, None);
        let error = publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&conflicting_authorization),
            )
            .expect_err("cached payload must not mask conflicting release evidence");
        assert!(
            error
                .to_string()
                .contains("identity conflicts with an existing publication")
        );
        assert_eq!(provider.append_count(), 1);
        assert_fenced_privacy_pending_logically_cleared(temp.path());
        assert_eq!(
            read_fenced_privacy_head_cache(temp.path())
                .expect("read cache after conflict")
                .expect("cache survives conflict"),
            first_cache
        );
    }

    #[test]
    fn filesystem_privacy_publication_without_fused_adapter_fails_before_side_effects() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, 8);

        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("privacy publication must require fused adapter");

        assert!(
            error
                .to_string()
                .contains("requires a qualified fused target publisher")
        );
        assert_no_privacy_publication_side_effects(temp.path());
        assert!(!fenced_privacy_pending_path(temp.path()).exists());
    }

    #[test]
    fn fresh_filesystem_root_without_authenticated_head_reader_fails_closed() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher")
            .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(provider))
            .expect("attach fused publisher");
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, 8);

        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("fresh root must not infer authoritative genesis");

        assert!(
            error
                .to_string()
                .contains("requires a qualified authenticated authoritative-head reader")
        );
        assert_no_privacy_publication_side_effects(temp.path());
        assert!(!fenced_privacy_head_sync_path(temp.path()).exists());
        assert!(!fenced_privacy_pending_path(temp.path()).exists());
    }

    #[test]
    fn filesystem_privacy_publication_rejects_substituted_receipt_before_side_effects() {
        let temp = tempdir().expect("tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let head_reader = test_fenced_head_reader(Arc::clone(&provider));
        provider.set_substitute_receipt(true);
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher")
            .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
                &provider,
            )))
            .expect("attach fused publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                head_reader,
            ))
            .expect("attach authenticated head reader");
        let (publication, encoded) = sample_privacy_publication();
        let authorization = sample_privacy_authorization(&publication, &encoded, 9);

        let error = publisher
            .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
            .expect_err("substituted receipt must fail closed");

        assert!(error.to_string().contains("publication receipt is invalid"));
        assert_eq!(provider.append_count(), 1);
        assert_no_privacy_publication_side_effects(temp.path());
        assert!(
            fenced_privacy_pending_path(temp.path()).exists(),
            "ambiguous append must retain its exact pending request"
        );

        provider.set_substitute_receipt(false);
        let rotated_authorization = sample_privacy_authorization(&publication, &encoded, 10);
        publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&rotated_authorization),
            )
            .expect("recover exact request after malformed receipt");

        assert_eq!(provider.append_count(), 1);
        assert_fenced_privacy_pending_logically_cleared(temp.path());
        let cache = read_fenced_privacy_head_cache(temp.path())
            .expect("read recovered cache")
            .expect("recovered cache exists");
        assert_eq!(cache.last_fencing_token, 9);
        let index = read_publication_section_fixture(temp.path(), "publish_index");
        let labels = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .and_then(|entry| entry.get("labels"))
            .and_then(JsonValue::as_object)
            .expect("recovered privacy labels");
        assert_eq!(
            labels
                .get("leader_lease_fencing_token")
                .and_then(JsonValue::as_u64),
            Some(9)
        );
    }

    #[test]
    fn fresh_roots_deduplicate_release_across_leases_and_later_heads() {
        let first_root = tempdir().expect("first tempdir");
        let same_lease_root = tempdir().expect("same-lease tempdir");
        let later_anchor_root = tempdir().expect("later-anchor tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let first_reader = test_fenced_head_reader(Arc::clone(&provider));
        let same_lease_reader = test_fenced_head_reader(Arc::clone(&provider));
        let first_publisher =
            FilesystemGovernancePublisher::try_new(first_root.path().to_path_buf())
                .expect("first publisher")
                .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(
                    Arc::clone(&provider),
                ))
                .expect("attach first fused publisher")
                .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                    first_reader,
                ))
                .expect("attach first authenticated head reader");
        let same_lease_publisher =
            FilesystemGovernancePublisher::try_new(same_lease_root.path().to_path_buf())
                .expect("same-lease publisher")
                .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(
                    Arc::clone(&provider),
                ))
                .expect("attach same-lease fused publisher")
                .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                    same_lease_reader,
                ))
                .expect("attach same-lease authenticated head reader");
        let (publication, encoded) = sample_privacy_publication();
        let first_authorization = sample_privacy_authorization(&publication, &encoded, 10);
        let same_lease_authorization = sample_privacy_authorization(&publication, &encoded, 10);
        first_publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&first_authorization),
            )
            .expect("first root publishes from authenticated genesis");
        let first_head = provider.head().expect("first authoritative head");
        same_lease_publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&same_lease_authorization),
            )
            .expect("fresh root recognizes the same lease and stable release");

        assert_eq!(provider.append_count(), 1);
        assert_eq!(provider.head(), Some(first_head));
        assert_eq!(
            read_fenced_privacy_head_cache(first_root.path())
                .expect("first cached head")
                .map(|cache| cache.authoritative_head),
            Some(first_head)
        );
        let same_lease_cache = read_fenced_privacy_head_cache(same_lease_root.path())
            .expect("same-lease cached head")
            .expect("same-lease cache exists");
        assert_eq!(same_lease_cache.authoritative_head, first_head);
        assert_eq!(same_lease_cache.last_included_head, first_head);
        assert_eq!(
            same_lease_cache.last_disposition,
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded
        );
        assert_eq!(same_lease_cache.last_fencing_token, 10);
        same_lease_publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&same_lease_authorization),
            )
            .expect("same fresh root replays its already-included cache");
        assert_eq!(provider.append_count(), 1);
        assert_eq!(
            read_fenced_privacy_head_cache(same_lease_root.path())
                .expect("same-root retry cached head")
                .expect("same-root retry cache exists")
                .last_disposition,
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded
        );

        let next_spec = SamplePrivacyReleaseSpec::next();
        let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
        let next_authorization =
            sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 11, None);
        first_publisher
            .publish_transparency_ledger_publication(
                &next_publication,
                &next_encoded,
                Some(&next_authorization),
            )
            .expect("a genuinely distinct finalized release appends");
        let advanced_head = provider.head().expect("advanced authoritative head");
        assert_ne!(advanced_head, first_head);
        assert_eq!(provider.append_count(), 2);

        assert!(
            !first_root
                .path()
                .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
                .exists()
        );
        assert!(
            !same_lease_root
                .path()
                .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
                .exists()
        );

        let later_anchor_reader = test_fenced_head_reader(Arc::clone(&provider));
        let later_anchor_publisher =
            FilesystemGovernancePublisher::try_new(later_anchor_root.path().to_path_buf())
                .expect("later-anchor publisher")
                .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(
                    Arc::clone(&provider),
                ))
                .expect("attach later-anchor fused publisher")
                .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                    later_anchor_reader,
                ))
                .expect("bootstrap authoritative head");
        let advanced_block_hash = next_publication
            .block
            .block_hash()
            .expect("advanced publication block hash");
        let later_anchor_authorization = sample_privacy_authorization_for(
            SamplePrivacyReleaseSpec::primary(),
            &publication,
            &encoded,
            12,
            Some(SampleFinalizedAnchorSpec {
                sequence: next_spec.release_sequence,
                release_id: next_publication.block.cycle_id,
                record_digest: next_spec.release_record_digest,
                latest_publication_block_hash: Some(advanced_block_hash),
            }),
        );
        assert_eq!(
            first_authorization.publication_idempotency_digest(),
            later_anchor_authorization.publication_idempotency_digest(),
            "later finalized-head advancement must not change the release identity"
        );
        later_anchor_publisher
            .publish_transparency_ledger_publication(
                &publication,
                &encoded,
                Some(&later_anchor_authorization),
            )
            .expect("fresh root recognizes a release under a later finalized anchor");

        assert_eq!(provider.append_count(), 2);
        assert_eq!(provider.head(), Some(advanced_head));
        let later_cache = read_fenced_privacy_head_cache(later_anchor_root.path())
            .expect("later-anchor cached head")
            .expect("later-anchor cache exists");
        assert_eq!(later_cache.authoritative_head, advanced_head);
        assert_eq!(later_cache.last_included_head, first_head);
        assert_eq!(
            later_cache.last_disposition,
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded
        );
        assert_fenced_privacy_pending_logically_cleared(later_anchor_root.path());
    }

    #[test]
    fn newer_fencing_token_wins_while_paused_predecessor_has_zero_side_effects() {
        let stale_root = tempdir().expect("stale tempdir");
        let winner_root = tempdir().expect("winner tempdir");
        let provider = Arc::new(TestFencedTransparencyPublisher::new());
        let stale_reader = test_fenced_head_reader(Arc::clone(&provider));
        let winner_reader = test_fenced_head_reader(Arc::clone(&provider));
        provider.pause_fencing_token(20);
        let stale_publisher =
            FilesystemGovernancePublisher::try_new(stale_root.path().to_path_buf())
                .expect("stale publisher")
                .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(
                    Arc::clone(&provider),
                ))
                .expect("attach stale fused publisher")
                .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                    stale_reader,
                ))
                .expect("attach stale authenticated head reader");
        let winner_publisher =
            FilesystemGovernancePublisher::try_new(winner_root.path().to_path_buf())
                .expect("winner publisher")
                .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(
                    Arc::clone(&provider),
                ))
                .expect("attach winner fused publisher")
                .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                    winner_reader,
                ))
                .expect("attach winner authenticated head reader");
        let (publication, encoded) = sample_privacy_publication();
        let stale_authorization = sample_privacy_authorization(&publication, &encoded, 20);
        let winner_spec = SamplePrivacyReleaseSpec::next();
        let (winner_publication, winner_encoded) = sample_privacy_publication_for(winner_spec);
        let winner_authorization = sample_privacy_authorization_for(
            winner_spec,
            &winner_publication,
            &winner_encoded,
            21,
            None,
        );
        let stale_publication = publication.clone();
        let stale_encoded = encoded.clone();
        let stale = thread::spawn(move || {
            stale_publisher.publish_transparency_ledger_publication(
                &stale_publication,
                &stale_encoded,
                Some(&stale_authorization),
            )
        });
        provider.wait_until_paused();

        let winner_result = winner_publisher.publish_transparency_ledger_publication(
            &winner_publication,
            &winner_encoded,
            Some(&winner_authorization),
        );
        provider.release_paused();
        winner_result.expect("newer fencing token wins");
        let stale_error = stale
            .join()
            .expect("stale publication thread")
            .expect_err("paused stale token must fail");

        assert!(stale_error.to_string().contains("fencing token is stale"));
        assert_eq!(provider.append_count(), 1);
        assert_no_privacy_publication_side_effects(stale_root.path());
        assert_fenced_privacy_pending_logically_cleared(stale_root.path());
        assert_eq!(
            read_fenced_privacy_head_cache(winner_root.path())
                .expect("winner cached head")
                .map(|cache| cache.authoritative_head),
            provider.head()
        );
        assert!(
            !winner_root
                .path()
                .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
                .exists()
        );
    }

    #[test]
    fn filesystem_publisher_writes_transparency_ledger_publication_files_and_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (publication, encoded) = sample_transparency_ledger_publication();

        publisher
            .publish_transparency_ledger_publication(&publication, &encoded, None)
            .expect("publish transparency ledger publication");

        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "transparency_ledger_publication");
        let bytes = fs::read(&encoded_path).expect("read transparency ledger payload");
        assert_eq!(bytes, encoded);
        let decoded: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&bytes).expect("decode transparency ledger publication");
        assert_eq!(decoded, publication);
        assert!(json_path.exists());

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("publish index entry");
        let labels = entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .expect("publish labels");
        let expected_cycle_id = hex::encode(publication.block.cycle_id);
        assert_eq!(
            labels.get("cycle_id_hex").and_then(JsonValue::as_str),
            Some(expected_cycle_id.as_str())
        );
        assert_eq!(
            labels.get("entry_count").and_then(JsonValue::as_u64),
            Some(u64::from(publication.block.entry_count))
        );

        let queue = read_publication_section_fixture(temp.path(), "car_queue");
        assert_eq!(
            queue
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
    }

    #[test]
    fn filesystem_publisher_writes_proof_token_issuance_files_and_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (issuance, encoded) = sample_proof_token_issuance();

        publisher
            .publish_proof_token_issuance(&issuance, &encoded)
            .expect("publish proof-token issuance");

        let token_id_hex = hex::encode(issuance.token_id);
        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "proof_token_issuance");
        let bytes = fs::read(&encoded_path).expect("read proof-token issuance payload");
        assert_eq!(bytes, encoded);
        let decoded: ProofTokenIssuanceV1 =
            norito::decode_from_bytes(&bytes).expect("decode proof-token issuance");
        assert_eq!(decoded, issuance);

        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read proof-token issuance json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("issuance json");
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("token_id_hex"))
                .and_then(JsonValue::as_str),
            Some(token_id_hex.as_str())
        );

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("proof_token_issuance"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("publish index entry");
        let labels = entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .expect("publish labels");
        assert_eq!(
            labels.get("token_id_hex").and_then(JsonValue::as_str),
            Some(token_id_hex.as_str())
        );
        assert_eq!(
            labels.get("entry_count").and_then(JsonValue::as_u64),
            Some(2)
        );
        assert_single_runtime_external(temp.path(), "proof_token_issuance", &encoded);

        let queue = read_publication_section_fixture(temp.path(), "car_queue");
        assert_eq!(
            queue
                .get("by_payload_kind")
                .and_then(|value| value.get("proof_token_issuance"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_report_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (report, encoded) = sample_appeal_finance_report();

        publisher
            .publish_appeal_finance_report(&report, &encoded)
            .expect("publish appeal finance report");

        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "appeal_finance_report");
        let bytes = fs::read(&encoded_path).expect("read appeal finance report payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceReportV1 =
            norito::decode_from_bytes(&bytes).expect("decode appeal finance report");
        assert_eq!(decoded, report);
        assert!(json_path.exists());

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceReport(value) => {
                assert_eq!(value.report_id, report.report_id);
                assert_eq!(value.case_id, report.case_id);
                assert_eq!(value.outcome, report.outcome);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn signed_runtime_dag_rejects_missing_authenticated_submission_provenance_before_writes() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (report, encoded) = sample_appeal_finance_report();
        let payload = GovernanceLogPayloadV1::AppealFinanceReport(report);

        let error = publisher
            .preflight_runtime_signed_payload_with_provenance(&payload, encoded.len(), None)
            .expect_err("signed caller-supplied payload must retain authenticated provenance");
        assert!(
            error
                .to_string()
                .contains("requires authenticated submission provenance")
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );
        assert_empty_publication_authority(temp.path());
        assert!(!temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR).exists());
    }

    #[test]
    fn authenticated_submission_identity_participates_in_publication_idempotency() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (report, encoded) = sample_appeal_finance_report();
        let first =
            test_submission_provenance(crate::GovernanceSubmissionOriginV1::AppealFinanceReport);
        let other_key = PublicKey::from_bytes(Algorithm::Ed25519, &[0xA6; 32])
            .expect("fixed second publisher key must be valid");
        let second = GovernanceSubmissionProvenanceV1::new(
            AccountId::new(other_key),
            crate::GovernanceSubmissionOriginV1::AppealFinanceReport,
        );

        for provenance in [&first, &second] {
            <FilesystemGovernancePublisher as GovernancePublisher>::publish_appeal_finance_report(
                &publisher, &report, &encoded, provenance,
            )
            .expect("distinct authenticated publisher is a distinct attestation");
        }

        let publish_index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            publish_index
                .get("entries")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(2)
        );

        let runtime_index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        assert_eq!(blocks.len(), 2);
        assert_ne!(
            blocks[0].node.submission_provenance,
            blocks[1].node.submission_provenance
        );
        assert_ne!(blocks[0].node.node_cid, blocks[1].node.node_cid);
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_weekly_rollup_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (rollup, encoded) = sample_appeal_finance_weekly_rollup();

        publisher
            .publish_appeal_finance_weekly_rollup(&rollup, &encoded)
            .expect("publish appeal finance weekly rollup");

        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "appeal_finance_weekly_rollup");
        let bytes = fs::read(&encoded_path).expect("read appeal finance weekly rollup payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceWeeklyRollupV1 =
            norito::decode_from_bytes(&bytes).expect("decode appeal finance weekly rollup");
        assert_eq!(decoded, rollup);
        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read appeal finance weekly rollup json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("weekly rollup json");
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("cycle"))
                .and_then(JsonValue::as_str),
            Some("2026-W26")
        );

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
                assert_eq!(value.cycle, rollup.cycle);
                assert_eq!(value.report_count, rollup.report_count);
                assert_eq!(value.total_deposit_xor, rollup.total_deposit_xor);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn appeal_finance_settlement_receipt_source_identity_binds_finalized_cursor() {
        let (receipt, encoded) = sample_appeal_finance_settlement_receipt();
        let source_identity = |receipt: &SoraFsAppealFinanceSettlementReceiptV1, encoded: &[u8]| {
            let encoded_blake3 = blake3::hash(encoded).to_hex().to_string();
            let json = appeal_finance_settlement_receipt_json(receipt, encoded, &encoded_blake3)
                .expect("encode receipt JSON");
            governance_source_pair_relative_paths(
                "appeal_finance_settlement_receipt",
                u64::try_from(encoded.len()).expect("encoded length"),
                &encoded_blake3,
                u64::try_from(json.len()).expect("JSON length"),
                &blake3::hash(json.as_bytes()).to_hex().to_string(),
            )
            .expect("derive composite source identity")
        };
        let path = source_identity(&receipt, &encoded);

        let mut changed_height = receipt.clone();
        changed_height.finalized_block_height += 1;
        let changed_height_encoded =
            norito::to_bytes(&changed_height).expect("encode changed-height receipt");
        let changed_height_path = source_identity(&changed_height, &changed_height_encoded);
        assert_ne!(changed_height_path, path);

        let mut changed_hash = receipt;
        changed_hash.finalized_block_hash[0] ^= 0x01;
        let changed_hash_encoded =
            norito::to_bytes(&changed_hash).expect("encode changed-hash receipt");
        let changed_hash_path = source_identity(&changed_hash, &changed_hash_encoded);
        assert_ne!(changed_hash_path, path);
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_settlement_receipt_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (receipt, encoded) = sample_appeal_finance_settlement_receipt();

        publisher
            .publish_appeal_finance_settlement_receipt(&receipt, &encoded)
            .expect("publish appeal finance settlement receipt");

        let (encoded_path, json_path) =
            only_published_source_paths(temp.path(), "appeal_finance_settlement_receipt");
        let bytes = fs::read(&encoded_path).expect("read settlement receipt payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceSettlementReceiptV1 =
            norito::decode_from_bytes(&bytes).expect("decode settlement receipt");
        assert_eq!(decoded, receipt);
        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read settlement receipt json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("receipt json");
        let expected_policy_digest_hex = hex::encode(receipt.appeal_finance_policy_digest);
        let expected_finalized_block_hash_hex = hex::encode(receipt.finalized_block_hash);
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("tx_hash_hex"))
                .and_then(JsonValue::as_str),
            Some(receipt.tx_hash_hex.as_str())
        );
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("appeal_finance_policy_digest_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_policy_digest_hex.as_str())
        );
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("finalized_block_height"))
                .and_then(JsonValue::as_u64),
            Some(receipt.finalized_block_height)
        );
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("finalized_block_hash_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_finalized_block_hash_hex.as_str())
        );

        let index = read_publication_section_fixture(temp.path(), "publish_index");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("entries")
                .and_then(JsonValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("labels"))
                .and_then(|labels| labels.get("appeal_finance_policy_digest_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_policy_digest_hex.as_str())
        );
        assert_eq!(
            index
                .get("entries")
                .and_then(JsonValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("labels"))
                .and_then(|labels| labels.get("finalized_block_height"))
                .and_then(JsonValue::as_u64),
            Some(receipt.finalized_block_height)
        );
        assert_eq!(
            index
                .get("entries")
                .and_then(JsonValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("labels"))
                .and_then(|labels| labels.get("finalized_block_hash_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_finalized_block_hash_hex.as_str())
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
                assert_eq!(value.receipt_id, receipt.receipt_id);
                assert_eq!(value.case_id, receipt.case_id);
                assert_eq!(value.submitted_step, receipt.submitted_step);
                assert_eq!(value.finalized_block_height, receipt.finalized_block_height);
                assert_eq!(value.finalized_block_hash, receipt.finalized_block_hash);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_runtime_dag_index() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        fs::write(
            temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
            br#"{"schema":"sorafs.governance_dag.wrong","blocks":[]}"#,
        )
        .expect("write bad runtime index");
        let (settlement, encoded) = sample_settlement();

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("malformed runtime DAG index must fail closed");
        assert!(
            err.to_string().contains("unsupported schema"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn filesystem_publisher_writes_settlement_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish");

        let (encoded_path, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
        assert_eq!(
            fs::read(&encoded_path).expect("read encoded"),
            encoded,
            "encoded payload must match original bytes"
        );

        let json_bytes = fs::read(&json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let status = value
            .get("metadata")
            .and_then(|meta| meta.get("status"))
            .and_then(JsonValue::as_str)
            .expect("status");
        assert_eq!(status, "completed");

        let encoded_digest = fs::read_to_string(digest_sidecar_path_for(&encoded_path))
            .expect("read encoded digest");
        let encoded_digest = encoded_digest.trim();
        assert_eq!(encoded_digest, blake3::hash(&encoded).to_hex().as_str());

        let json_digest =
            fs::read_to_string(digest_sidecar_path_for(&json_path)).expect("read json digest");
        let json_digest = json_digest.trim();
        assert_eq!(json_digest, blake3::hash(&json_bytes).to_hex().as_str());

        let publication_path = temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE);
        let publication_bytes = fs::read(&publication_path).expect("read publication state");
        let publication: JsonValue =
            norito::json::from_slice(&publication_bytes).expect("publication state json");
        assert_eq!(
            publication.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_PUBLICATION_STATE_SCHEMA)
        );
        assert_eq!(
            publication.get("generation").and_then(JsonValue::as_u64),
            Some(1)
        );
        let index = publication
            .get("publish_index")
            .cloned()
            .expect("nested publish index");
        assert_eq!(
            index.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
        );
        assert_eq!(
            index.get("root").and_then(JsonValue::as_str),
            Some(GOVERNANCE_DAG_LOGICAL_ROOT)
        );
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("payload_kind_counts")
                .and_then(JsonValue::as_object)
                .and_then(|counts| counts.get("deal_settlement"))
                .and_then(JsonValue::as_u64),
            Some(1)
        );
        let digest_hex = blake3::hash(&encoded).to_hex().to_string();
        let digest_positions = index
            .get("by_encoded_blake3")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get(digest_hex.as_str()))
            .and_then(JsonValue::as_array)
            .expect("digest lookup");
        assert_eq!(digest_positions.len(), 1);
        assert_eq!(digest_positions[0].as_u64(), Some(0));
        let kind_positions = index
            .get("by_payload_kind")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get("deal_settlement"))
            .and_then(JsonValue::as_array)
            .expect("kind lookup");
        assert_eq!(kind_positions[0].as_u64(), Some(0));
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("first index entry");
        assert_eq!(
            entry.get("payload_kind").and_then(JsonValue::as_str),
            Some("deal_settlement")
        );
        assert_eq!(
            entry.get("encoded_path").and_then(JsonValue::as_str),
            Some(index_path_string(temp.path(), &encoded_path).as_str())
        );
        assert_eq!(
            entry.get("json_len").and_then(JsonValue::as_u64),
            Some(json_bytes.len() as u64)
        );
        assert_eq!(
            entry.get("json_blake3").and_then(JsonValue::as_str),
            Some(blake3::hash(&json_bytes).to_hex().as_str())
        );
        assert_eq!(
            entry
                .get("labels")
                .and_then(JsonValue::as_object)
                .and_then(|labels| labels.get("status"))
                .and_then(JsonValue::as_str),
            Some("completed")
        );
        assert!(!temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE).exists());
        assert!(!temp.path().join(GOVERNANCE_CAR_QUEUE_FILE).exists());

        let queue = publication
            .get("car_queue")
            .cloned()
            .expect("nested CAR queue");
        assert_eq!(
            queue.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
        );
        assert_eq!(
            queue.get("root").and_then(JsonValue::as_str),
            Some(GOVERNANCE_DAG_LOGICAL_ROOT)
        );
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        let segment = queue
            .get("segments")
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(JsonValue::as_object)
            .expect("first CAR segment");
        assert_eq!(
            segment.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );
        assert_eq!(
            segment.get("status").and_then(JsonValue::as_str),
            Some("assembled")
        );
        assert_eq!(
            segment
                .get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            segment.get("encoded_blake3").and_then(JsonValue::as_str),
            Some(digest_hex.as_str())
        );
        let car_path = resolve_index_path(
            temp.path(),
            segment
                .get("car_path")
                .and_then(JsonValue::as_str)
                .expect("car path"),
        )
        .expect("resolve car path");
        let car_bytes = fs::read(&car_path).expect("read CAR segment");
        let car_archive_digest_hex = blake3::hash(&car_bytes).to_hex().to_string();
        assert_eq!(
            segment.get("car_size").and_then(JsonValue::as_u64),
            Some(car_bytes.len() as u64)
        );
        assert_eq!(
            segment
                .get("car_archive_blake3")
                .and_then(JsonValue::as_str),
            Some(car_archive_digest_hex.as_str())
        );
        let archive_positions = queue
            .get("by_car_archive_blake3")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get(car_archive_digest_hex.as_str()))
            .and_then(JsonValue::as_array)
            .expect("CAR archive digest lookup");
        assert_eq!(archive_positions.as_slice(), [JsonValue::from(0_u64)]);
        let car_digest =
            fs::read_to_string(digest_sidecar_path_for(&car_path)).expect("read car sidecar");
        assert_eq!(
            car_digest.trim(),
            blake3::hash(&car_bytes).to_hex().as_str()
        );

        let plan_path = resolve_index_path(
            temp.path(),
            segment
                .get("plan_path")
                .and_then(JsonValue::as_str)
                .expect("plan path"),
        )
        .expect("resolve plan path");
        let plan_bytes = fs::read(&plan_path).expect("read CAR plan");
        let plan: JsonValue = norito::json::from_slice(&plan_bytes).expect("plan json");
        assert_eq!(
            plan.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_PLAN_SCHEMA)
        );
        assert_eq!(
            plan.get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            plan.get("files")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(4)
        );
        assert!(
            plan.get("chunks")
                .and_then(JsonValue::as_array)
                .is_some_and(|chunks| !chunks.is_empty()),
            "CAR plan should expose deterministic chunks"
        );
        let manifest_path = resolve_index_path(
            temp.path(),
            segment
                .get("manifest_path")
                .and_then(JsonValue::as_str)
                .expect("manifest path"),
        )
        .expect("resolve segment manifest path");
        let manifest_bytes = fs::read(&manifest_path).expect("read segment manifest");
        assert!(manifest_bytes.len() <= GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1);
        let manifest: JsonValue =
            norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
        assert_eq!(
            manifest.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("exact duplicate publication is a no-op");
        assert_eq!(
            fs::read(&publication_path).expect("reread publication state after duplicate"),
            publication_bytes,
            "an exact duplicate must not advance or rewrite the authority envelope"
        );
        assert_eq!(
            fs::read(&car_path).expect("reread duplicate CAR"),
            car_bytes
        );

        fs::write(&car_path, b"substituted archive").expect("substitute retained CAR artifact");
        fs::write(
            digest_sidecar_path_for(&car_path),
            format!("{}\n", blake3::hash(b"substituted archive").to_hex()),
        )
        .expect("substitute retained CAR sidecar");
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("duplicate publication must reject a substituted immutable CAR");
        assert!(
            error.to_string().contains("occupied by different bytes"),
            "unexpected duplicate substitution error: {error}"
        );
        assert_eq!(
            fs::read(&publication_path).expect("reread authority after substituted duplicate"),
            publication_bytes,
            "a rejected duplicate must leave the authority envelope unchanged"
        );
        assert_eq!(
            fs::read(&car_path).expect("read rejected substituted CAR"),
            b"substituted archive",
            "the publisher must not conceal immutable-artifact substitution by overwriting it"
        );

        let publication = read_publication_state_fixture(temp.path());
        assert_eq!(
            publication.get("generation").and_then(JsonValue::as_u64),
            Some(1)
        );
        let index = publication
            .get("publish_index")
            .expect("republished nested index");
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1),
            "duplicate attempts must not duplicate the index entry"
        );
        let queue = publication
            .get("car_queue")
            .expect("republished nested queue");
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1),
            "duplicate attempts must not duplicate the CAR queue segment"
        );
    }

    #[test]
    fn filesystem_publisher_settlement_json_preserves_exact_wide_quantities() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (mut settlement, _) = sample_settlement();
        let wide = xor("340282366920938463463374607431768211456");
        let sub_micro = xor("0.0000001");
        let applied = xor("0.00000004");
        let client_debit = xor("0.00000006");
        let slash = xor("0.000000001");
        let satisfied_without_outstanding = applied
            .checked_add(&client_debit)
            .and_then(|amount| amount.checked_add(&slash))
            .expect("fixture liability components");
        let outstanding = wide
            .checked_sub(&satisfied_without_outstanding)
            .expect("wide liability exceeds fixture payments");
        settlement.status = DealSettlementStatusV1::WindowSettled;
        settlement.ledger.deal_end_epoch = settlement.ledger.window_end_epoch + 10;
        settlement.ledger.provider_accrual = "0.0000001".parse().expect("sub-micro quantity");
        settlement.ledger.client_liability = wide.clone();
        settlement.ledger.micropayment_credit_generated = applied.clone();
        settlement.ledger.micropayment_credit_applied = applied.clone();
        settlement.ledger.micropayment_credit_carry = XorQuantity::zero();
        settlement.ledger.client_debit = client_debit.clone();
        settlement.ledger.outstanding_liability = outstanding;
        settlement.ledger.bond_total = xor("1.000000002");
        settlement.ledger.bond_locked = xor("1.000000001");
        settlement.ledger.bond_slashed = slash.clone();
        settlement.ledger.bond_released = XorQuantity::zero();
        settlement.ledger.window_expected_charge = wide;
        settlement.ledger.window_micropayment_generated = applied.clone();
        settlement.ledger.window_micropayment_applied = applied;
        settlement.ledger.window_client_debit = client_debit;
        settlement.ledger.window_bond_slashed = slash;
        settlement.ledger.window_bond_released = XorQuantity::zero();
        settlement.audit_notes = Some("exact wide-quantity settlement fixture".to_owned());
        assert_eq!(settlement.ledger.provider_accrual, sub_micro);
        settlement.ledger.snapshot_id = settlement.ledger.derive_snapshot_id().expect("ledger id");
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        settlement
            .validate_transition(None)
            .expect("coherent exact settlement fixture");
        let encoded = norito::to_bytes(&settlement).expect("encode canonical settlement");

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish exact settlement");

        let (_, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
        let body = fs::read(json_path).expect("read settlement json");
        let value: JsonValue = json::from_slice(&body).expect("parse settlement json");
        let object = value
            .get("settlement")
            .and_then(JsonValue::as_object)
            .expect("settlement object");
        for (field, expected) in [
            ("provider_accrual", "0.0000001"),
            (
                "client_liability",
                "340282366920938463463374607431768211456",
            ),
            ("bond_locked", "1.000000001"),
            ("bond_slashed", "0.000000001"),
        ] {
            assert_eq!(
                object.get(field).and_then(JsonValue::as_str),
                Some(expected),
                "exact quantity field {field}"
            );
        }
        for retired in [
            "provider_accrual_micro",
            "client_liability_micro",
            "bond_locked_micro",
            "bond_slashed_micro",
        ] {
            assert!(!object.contains_key(retired), "retired field {retired}");
        }
    }

    #[test]
    fn filesystem_publisher_rejects_legacy_separate_car_queue_authority() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        fs::write(
            temp.path().join(GOVERNANCE_CAR_QUEUE_FILE),
            br#"{"schema":"wrong","segments":[]}"#,
        )
        .expect("write malformed queue");

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("legacy CAR queue authority must fail closed");
        assert!(
            err.to_string()
                .contains("legacy governance publication authority"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_publication_authority_before_artifact_writes() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE),
            br#"{"schema":"substituted"}"#,
        )
        .expect("write malformed authoritative publication state");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("malformed authority must reject publisher startup");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "startup validation must not create immutable source artifacts"
        );
        assert!(
            !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
            "startup validation must not create immutable CAR artifacts"
        );
    }

    #[test]
    fn filesystem_publisher_reclaims_bounded_uncommitted_artifacts_at_startup() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        {
            let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
                .expect("retain publication root");
            let orphan = write_car_segment_source_fixture(temp.path(), b"orphan-publication");
            assemble_governance_car_queue(
                temp.path(),
                &root_guard,
                empty_governance_car_queue(),
                &orphan,
            )
            .expect("assemble orphan CAR artifacts");
        }
        assert!(
            temp.path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );
        assert!(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("Unix recovery must isolate interrupted artifacts before startup");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected quarantine error: {error}"
            );
            let quarantine = recovery_quarantine_path(temp.path());
            let isolated = fs::read_dir(&quarantine)
                .expect("read bounded recovery quarantine")
                .map(|entry| entry.expect("quarantine entry").file_name())
                .collect::<BTreeSet<_>>();
            let expected = [
                "car-file-00",
                "car-file-01",
                "car-file-02",
                "car-file-03",
                "car-file-04",
                "car-file-05",
                "car-root",
                "source-file-00",
                "source-file-01",
                "source-file-02",
                "source-file-03",
                "source-kind",
                "source-pair",
                "source-root",
            ]
            .map(OsString::from)
            .into_iter()
            .collect::<BTreeSet<_>>();
            assert_eq!(isolated, expected, "quarantine slots are deterministic");
            clear_recovery_quarantine_offline(temp.path());
            drop(
                FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("startup succeeds after offline quarantine cleanup"),
            );
        }
        #[cfg(windows)]
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("Windows exact-handle cleanup reconciles the publication"),
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "unreferenced source files and their empty directories must be reclaimed"
        );
        assert!(
            !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
            "unreferenced CAR files and their empty directory must be reclaimed"
        );
    }

    #[test]
    fn filesystem_publisher_reclaims_only_the_exact_next_car_atomic_temp() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let orphan = write_car_segment_source_fixture(temp.path(), b"orphan-publication");
        let car_base = temp
            .path()
            .join(governance_car_segment_relative_base(&orphan).expect("CAR base"));
        let car_target = car_base.with_extension("car");
        let car_directory = car_target.parent().expect("CAR directory");
        fs::create_dir_all(car_directory).expect("create interrupted CAR directory");
        let car_target_name = car_target
            .file_name()
            .and_then(OsStr::to_str)
            .expect("canonical CAR target name");
        fs::write(
            car_directory.join(format!(".{car_target_name}.tmp-42000-1")),
            b"interrupted CAR temp",
        )
        .expect("seed exact next CAR temp");

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("Unix recovery must isolate the exact interrupted temp");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected quarantine error: {error}"
            );
            clear_recovery_quarantine_offline(temp.path());
            drop(
                FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("startup succeeds after offline quarantine cleanup"),
            );
        }
        #[cfg(windows)]
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("Windows exact-handle cleanup reconciles the interrupted temp"),
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );
        assert!(!temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());
    }

    #[test]
    fn filesystem_publisher_verifies_every_committed_role_before_orphan_cleanup() {
        #[derive(Clone, Copy, Debug)]
        enum Mutation {
            Missing,
            Corrupt,
        }

        for mutation in [Mutation::Missing, Mutation::Corrupt] {
            for role_index in 0..10 {
                let temp = tempdir().expect("tempdir");
                let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("publisher");
                let (settlement, encoded) = sample_settlement();
                publisher
                    .publish_deal_settlement(&settlement, &encoded)
                    .expect("publish committed settlement");
                let state = read_publication_state_fixture(temp.path());
                let committed = committed_publication_artifact_paths(
                    temp.path(),
                    state.as_object().expect("publication state object"),
                );
                drop(publisher);

                let (_, orphan_snapshots) = seed_complete_uncommitted_publication_fixture(
                    temp.path(),
                    "interrupted_test_payload",
                    b"interrupted-publication",
                    1,
                );
                let (role, committed_path) = committed
                    .into_iter()
                    .nth(role_index)
                    .expect("committed role index");
                match mutation {
                    Mutation::Missing => {
                        fs::remove_file(&committed_path)
                            .expect("remove one committed publication role");
                    }
                    Mutation::Corrupt => {
                        fs::write(&committed_path, b"corrupt committed publication artifact")
                            .expect("corrupt one committed publication role");
                    }
                }

                let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect_err(
                        "startup must reject a missing or corrupt committed publication role",
                    );
                assert_eq!(
                    error.kind(),
                    io::ErrorKind::InvalidData,
                    "unexpected error kind for {mutation:?} {role}: {error}"
                );
                for (orphan_path, expected) in &orphan_snapshots {
                    let actual = fs::read(orphan_path).unwrap_or_else(|error| {
                        panic!(
                            "{mutation:?} {role} deleted orphan `{}` before failing: {error}",
                            orphan_path.display()
                        )
                    });
                    assert_eq!(
                        actual.as_slice(),
                        expected.as_slice(),
                        "{mutation:?} {role} changed orphan `{}` before failing",
                        orphan_path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn filesystem_publisher_rejects_multiple_interrupted_source_pairs_without_cleanup() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let first = write_car_segment_source_fixture_for_kind(
            temp.path(),
            "interrupted_alpha",
            b"interrupted-alpha",
        );
        let second = write_car_segment_source_fixture_for_kind(
            temp.path(),
            "interrupted_beta",
            b"interrupted-beta",
        );
        let snapshots = [first, second]
            .into_iter()
            .flat_map(|entry| {
                let encoded = temp.path().join(entry.encoded_path);
                let json = temp.path().join(entry.json_path);
                [
                    encoded.clone(),
                    digest_sidecar_path_for(&encoded),
                    json.clone(),
                    digest_sidecar_path_for(&json),
                ]
            })
            .map(|path| {
                let bytes = fs::read(&path).expect("snapshot interrupted source role");
                (path, bytes)
            })
            .collect::<Vec<_>>();

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("multiple interrupted source identities must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        for (path, expected) in snapshots {
            assert_eq!(
                fs::read(&path).expect("multiple-source rejection preserves every artifact"),
                expected,
                "multiple-source rejection changed `{}`",
                path.display()
            );
        }
    }

    #[test]
    fn filesystem_publisher_rejects_split_interrupted_car_bases_without_cleanup() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let (entry, _) = seed_complete_uncommitted_publication_fixture(
            temp.path(),
            "interrupted_split_car",
            b"interrupted-split-car",
            0,
        );
        let original_base = temp
            .path()
            .join(governance_car_segment_relative_base(&entry).expect("derive original CAR base"));
        let pair_id = original_base
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|base| base.split_once('_'))
            .map(|(_, pair_id)| pair_id)
            .expect("fixture CAR pair identity");
        let alternate_base = temp
            .path()
            .join(GOVERNANCE_CAR_SEGMENTS_DIR)
            .join(format!("{:020}_{pair_id}", 1));
        for suffix in ["json", "json.blake3"] {
            let source = original_base.with_extension(suffix);
            let target = alternate_base.with_extension(suffix);
            fs::rename(&source, &target).expect("split CAR role across another base");
        }
        let snapshots = fs::read_dir(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR))
            .expect("read split CAR directory")
            .map(|entry| {
                let path = entry.expect("split CAR entry").path();
                let bytes = fs::read(&path).expect("snapshot split CAR role");
                (path, bytes)
            })
            .chain(
                publication_artifact_paths_for_fixture(temp.path(), &entry)
                    .into_iter()
                    .take(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
                    .map(|path| {
                        let bytes = fs::read(&path).expect("snapshot split source role");
                        (path, bytes)
                    }),
            )
            .collect::<Vec<_>>();

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("CAR roles split across bases must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("more than one artifact base"),
            "unexpected error: {error}"
        );
        for (path, expected) in snapshots {
            assert_eq!(
                fs::read(&path).expect("split-base rejection preserves every artifact"),
                expected,
                "split-base rejection changed `{}`",
                path.display()
            );
        }
    }

    #[test]
    fn filesystem_publisher_rejects_non_next_or_uncorrelated_interrupted_car_without_cleanup() {
        for (case, replacement_position, replacement_pair_id) in [
            ("non-next", 1_usize, None),
            ("uncorrelated", 0_usize, Some("ab".repeat(32))),
        ] {
            let temp = tempdir().expect("tempdir");
            drop(
                FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("initialize empty publication authority"),
            );
            let (entry, _) = seed_complete_uncommitted_publication_fixture(
                temp.path(),
                "interrupted_identity_check",
                b"interrupted-identity-check",
                0,
            );
            let original_base = temp.path().join(
                governance_car_segment_relative_base(&entry).expect("derive original CAR base"),
            );
            let original_pair_id = original_base
                .file_name()
                .and_then(OsStr::to_str)
                .and_then(|base| base.split_once('_'))
                .map(|(_, pair_id)| pair_id)
                .expect("fixture CAR pair identity");
            let replacement_pair_id = replacement_pair_id.as_deref().unwrap_or(original_pair_id);
            let replacement_base = temp
                .path()
                .join(GOVERNANCE_CAR_SEGMENTS_DIR)
                .join(format!("{replacement_position:020}_{replacement_pair_id}"));
            for suffix in [
                "car",
                "car.blake3",
                "plan.json",
                "plan.json.blake3",
                "json",
                "json.blake3",
            ] {
                fs::rename(
                    original_base.with_extension(suffix),
                    replacement_base.with_extension(suffix),
                )
                .expect("move CAR role to a single invalid interrupted base");
            }
            let snapshots = fs::read_dir(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR))
                .expect("read invalid CAR directory")
                .map(|entry| {
                    let path = entry.expect("invalid CAR entry").path();
                    let bytes = fs::read(&path).expect("snapshot invalid CAR role");
                    (path, bytes)
                })
                .chain(
                    publication_artifact_paths_for_fixture(temp.path(), &entry)
                        .into_iter()
                        .take(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
                        .map(|path| {
                            let bytes = fs::read(&path).expect("snapshot source role");
                            (path, bytes)
                        }),
                )
                .collect::<Vec<_>>();

            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("invalid interrupted CAR identity must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            let message = error.to_string();
            assert!(
                message.contains("exact expected next position")
                    || message.contains("source and CAR identities diverge"),
                "unexpected {case} error: {error}"
            );
            for (path, expected) in snapshots {
                assert_eq!(
                    fs::read(&path).expect("identity rejection preserves every artifact"),
                    expected,
                    "{case} rejection changed `{}`",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn filesystem_publisher_cleanup_is_restart_safe_after_every_rollback_step() {
        const CLEANUP_STEPS: usize = GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT
            + 1
            + GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT
            + 3;

        for interrupted_after in 1..=CLEANUP_STEPS {
            let temp = tempdir().expect("tempdir");
            drop(
                FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("initialize empty publication authority"),
            );
            seed_complete_uncommitted_publication_fixture(
                temp.path(),
                "interrupted_rollback_boundary",
                b"interrupted-rollback-boundary",
                0,
            );
            let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
                .expect("retain rollback fixture root");
            let state = read_publication_state_fixture(temp.path());
            let state = state.as_object().expect("publication state object");
            let inventory = governance_publication_artifact_inventory(state)
                .expect("derive rollback fixture inventory");
            let (mut cleanup_plan, interrupted_identity) =
                plan_governance_publication_source_artifacts(&root_guard, &inventory)
                    .expect("plan interrupted source rollback");
            plan_governance_publication_car_artifacts(
                &root_guard,
                &inventory,
                interrupted_identity.as_ref(),
                &mut cleanup_plan,
            )
            .expect("plan interrupted CAR rollback");
            verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
                .expect("verify empty committed authority");
            let observed_step = std::cell::Cell::new(0_usize);
            let error =
                apply_governance_publication_cleanup_plan_with(&root_guard, cleanup_plan, |step| {
                    observed_step.set(step);
                    if step == interrupted_after {
                        Err(GovernancePublishError::other(
                            "injected cleanup interruption",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .expect_err("injected rollback interruption must stop cleanup");
            assert!(error.to_string().contains("injected cleanup interruption"));
            assert_eq!(observed_step.get(), interrupted_after);
            drop(root_guard);

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            let publisher = {
                let quarantine = recovery_quarantine_path(temp.path());
                let before_restart = fs::read_dir(&quarantine)
                    .expect("read interrupted recovery quarantine")
                    .count();
                assert_eq!(before_restart, interrupted_after);
                let restart_error =
                    FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                        .expect_err("restart must stop at a preserved recovery quarantine");
                assert!(
                    restart_error
                        .to_string()
                        .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                    "unexpected restart error: {restart_error}"
                );
                assert_eq!(
                    fs::read_dir(&quarantine)
                        .expect("reread preserved recovery quarantine")
                        .count(),
                    before_restart,
                    "restart must not mutate a preserved quarantine"
                );
                clear_recovery_quarantine_offline(temp.path());
                finish_recovery_after_offline_quarantine_cleanup(temp.path())
            };
            #[cfg(windows)]
            let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .unwrap_or_else(|error| {
                    panic!(
                        "restart after cleanup step {interrupted_after}/{CLEANUP_STEPS} failed: {error}"
                    )
                });
            assert!(
                !temp
                    .path()
                    .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                    .exists(),
                "source residue remained after restarting cleanup step {interrupted_after}"
            );
            assert!(
                !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
                "CAR residue remained after restarting cleanup step {interrupted_after}"
            );
            drop(publisher);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn filesystem_publisher_quarantines_same_inode_byte_changes_after_cleanup_planning() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let (entry, _) = seed_complete_uncommitted_publication_fixture(
            temp.path(),
            "interrupted_byte_change",
            b"interrupted-byte-change",
            0,
        );
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain byte-change fixture root");
        let state = read_publication_state_fixture(temp.path());
        let state = state.as_object().expect("publication state object");
        let inventory = governance_publication_artifact_inventory(state)
            .expect("derive byte-change fixture inventory");
        let (mut cleanup_plan, interrupted_identity) =
            plan_governance_publication_source_artifacts(&root_guard, &inventory)
                .expect("plan interrupted source rollback");
        plan_governance_publication_car_artifacts(
            &root_guard,
            &inventory,
            interrupted_identity.as_ref(),
            &mut cleanup_plan,
        )
        .expect("plan interrupted CAR rollback");
        verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
            .expect("verify empty committed authority");

        let car_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
            .into_iter()
            .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
            .collect::<Vec<_>>();
        let first_rollback_role = car_roles
            .last()
            .expect("complete CAR fixture has a final rollback role");
        let original = fs::read(first_rollback_role).expect("read planned CAR role");
        let substituted = vec![b'x'; original.len()];
        fs::write(first_rollback_role, &substituted)
            .expect("change planned CAR bytes without replacing its inode");

        let error = apply_governance_publication_cleanup_plan(&root_guard, cleanup_plan)
            .expect_err("post-plan byte change must stop recovery after isolation");
        assert!(
            error.to_string().contains("changed after exact comparison"),
            "unexpected byte-comparison error: {error}"
        );
        assert!(
            !first_rollback_role.exists(),
            "the changed live binding must be isolated without unlinking"
        );
        assert_eq!(
            fs::read(recovery_quarantine_path(temp.path()).join("car-file-05"))
                .expect("read preserved changed CAR role"),
            substituted,
            "the changed same-inode bytes must remain available for offline inspection"
        );
    }

    #[test]
    fn filesystem_publisher_rolls_back_the_next_atomic_temp_before_durable_roles() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let (entry, _) = seed_complete_uncommitted_publication_fixture(
            temp.path(),
            "interrupted_temp_rollback",
            b"interrupted-temp-rollback",
            0,
        );
        let car_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
            .into_iter()
            .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
            .collect::<Vec<_>>();
        for path in car_roles.iter().skip(1) {
            fs::remove_file(path).expect("truncate CAR prefix after its archive");
        }
        let next_target = &car_roles[1];
        let next_name = next_target
            .file_name()
            .and_then(OsStr::to_str)
            .expect("next CAR role name");
        let next_temp = next_target
            .parent()
            .expect("CAR role parent")
            .join(format!(".{next_name}.tmp-42000-1"));
        fs::write(&next_temp, b"partially-written-sidecar").expect("seed next atomic temporary");

        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain temporary rollback root");
        let state = read_publication_state_fixture(temp.path());
        let state = state.as_object().expect("publication state object");
        let inventory = governance_publication_artifact_inventory(state)
            .expect("derive temporary rollback inventory");
        let (mut cleanup_plan, interrupted_identity) =
            plan_governance_publication_source_artifacts(&root_guard, &inventory)
                .expect("plan temporary source rollback");
        plan_governance_publication_car_artifacts(
            &root_guard,
            &inventory,
            interrupted_identity.as_ref(),
            &mut cleanup_plan,
        )
        .expect("plan temporary CAR rollback");
        verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
            .expect("verify empty committed authority");
        apply_governance_publication_cleanup_plan_with(&root_guard, cleanup_plan, |step| {
            if step == 1 {
                Err(GovernancePublishError::other(
                    "injected post-temporary interruption",
                ))
            } else {
                Ok(())
            }
        })
        .expect_err("cleanup must stop immediately after removing the next temporary");
        assert!(
            !next_temp.exists(),
            "the next temporary must leave the live namespace first"
        );
        assert!(
            car_roles[0].exists(),
            "the durable CAR prefix must remain after the first rollback step"
        );
        drop(root_guard);

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let publisher = {
            let quarantine = recovery_quarantine_path(temp.path());
            assert_eq!(
                fs::read_dir(&quarantine)
                    .expect("read temporary recovery quarantine")
                    .count(),
                1,
                "the exact next temporary is the first isolated slot"
            );
            let restart_error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("restart must require offline cleanup of the isolated temp");
            assert!(
                restart_error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected restart error: {restart_error}"
            );
            clear_recovery_quarantine_offline(temp.path());
            finish_recovery_after_offline_quarantine_cleanup(temp.path())
        };
        #[cfg(windows)]
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("restart accepts the preserved durable CAR prefix");
        assert!(!temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );
        drop(publisher);
    }

    #[test]
    fn filesystem_publisher_accepts_one_empty_interrupted_kind_and_rejects_two() {
        let accepted = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(accepted.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        fs::create_dir_all(
            accepted
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .join("interrupted_empty_kind"),
        )
        .expect("seed one durably created empty source kind");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let error = FilesystemGovernancePublisher::try_new(accepted.path().to_path_buf())
                .expect_err("one legitimate empty prefix must be isolated on Unix");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected quarantine error: {error}"
            );
            clear_recovery_quarantine_offline(accepted.path());
            drop(
                FilesystemGovernancePublisher::try_new(accepted.path().to_path_buf())
                    .expect("startup succeeds after offline quarantine cleanup"),
            );
        }
        #[cfg(windows)]
        drop(
            FilesystemGovernancePublisher::try_new(accepted.path().to_path_buf())
                .expect("one empty source-kind prefix is a legitimate interrupted write"),
        );
        assert!(
            !accepted
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists()
        );

        let rejected = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(rejected.path().to_path_buf())
                .expect("initialize second empty publication authority"),
        );
        let source_root = rejected.path().join(GOVERNANCE_PUBLICATION_SOURCES_DIR);
        for kind in ["interrupted_empty_alpha", "interrupted_empty_beta"] {
            fs::create_dir_all(source_root.join(kind)).expect("seed excess empty source kind");
        }
        let error = FilesystemGovernancePublisher::try_new(rejected.path().to_path_buf())
            .expect_err("more than one empty source-kind prefix must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        for kind in ["interrupted_empty_alpha", "interrupted_empty_beta"] {
            assert!(
                source_root.join(kind).is_dir(),
                "excess-prefix rejection removed `{kind}`"
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn filesystem_publisher_rejects_empty_recovery_quarantine_until_offline_cleanup() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let quarantine = recovery_quarantine_path(temp.path());
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain empty-quarantine fixture root");
        drop(
            prepare_governance_publication_recovery_quarantine(&root_guard)
                .expect("simulate a crash after durable quarantine creation"),
        );
        drop(root_guard);

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("an empty retained quarantine must still block restart");
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected empty-quarantine error: {error}"
        );
        assert_eq!(
            fs::read_dir(&quarantine)
                .expect("reread empty recovery quarantine")
                .count(),
            0,
            "restart must preserve an empty quarantine for explicit offline cleanup"
        );
        clear_recovery_quarantine_offline(temp.path());
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("restart succeeds after removing the empty quarantine offline"),
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn filesystem_publisher_rejects_saturated_recovery_quarantine_without_mutation() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize empty publication authority"),
        );
        let quarantine = recovery_quarantine_path(temp.path());
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain saturated-quarantine fixture root");
        drop(
            prepare_governance_publication_recovery_quarantine(&root_guard)
                .expect("create durable saturated-quarantine fixture"),
        );
        drop(root_guard);
        for position in 0..=GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP {
            fs::write(
                quarantine.join(format!("preserved-{position:02}")),
                position.to_le_bytes(),
            )
            .expect("seed preserved quarantine entry");
        }

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("a saturated recovery quarantine must block startup");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("hard cap")
                && error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected saturation error: {error}"
        );
        assert_eq!(
            fs::read_dir(&quarantine)
                .expect("reread saturated quarantine")
                .count(),
            GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP + 1,
            "startup must not mutate a saturated quarantine"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_foreign_car_bytes_at_the_expected_base_without_cleanup() {
        let donor = tempdir().expect("donor tempdir");
        let (donor_entry, _) = seed_complete_uncommitted_publication_fixture(
            donor.path(),
            "foreign_interrupted_payload",
            b"foreign-interrupted-publication",
            0,
        );
        let donor_roles = publication_artifact_paths_for_fixture(donor.path(), &donor_entry)
            .into_iter()
            .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
            .map(|path| fs::read(path).expect("read foreign CAR role"))
            .collect::<Vec<_>>();
        assert_eq!(donor_roles.len(), GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT);

        for substituted_role in 0..GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT {
            let temp = tempdir().expect("tempdir");
            drop(
                FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                    .expect("initialize empty publication authority"),
            );
            let (entry, _) = seed_complete_uncommitted_publication_fixture(
                temp.path(),
                "expected_interrupted_payload",
                b"expected-interrupted-publication",
                0,
            );
            let target_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
                .into_iter()
                .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
                .collect::<Vec<_>>();
            assert_ne!(
                fs::read(&target_roles[substituted_role]).expect("read expected CAR role"),
                donor_roles[substituted_role],
                "foreign role fixture unexpectedly matches role {substituted_role}"
            );
            fs::write(
                &target_roles[substituted_role],
                &donor_roles[substituted_role],
            )
            .expect("substitute foreign bytes at expected CAR base");
            let snapshots = publication_artifact_paths_for_fixture(temp.path(), &entry)
                .into_iter()
                .map(|path| {
                    let bytes = fs::read(&path).expect("snapshot substituted publication role");
                    (path, bytes)
                })
                .collect::<Vec<_>>();

            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("foreign CAR bytes at the expected base must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(
                error
                    .to_string()
                    .contains("diverges from its canonical source projection"),
                "unexpected role {substituted_role} error: {error}"
            );
            for (path, expected) in snapshots {
                assert_eq!(
                    fs::read(&path).expect("content-correlation rejection preserves role"),
                    expected,
                    "content-correlation rejection changed `{}` for role {substituted_role}",
                    path.display()
                );
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn governance_atomic_writes_reconcile_stale_names_before_mutation() {
        let temp = tempdir().expect("tempdir");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain atomic-write fixture root");
        let replace_target = temp.path().join("replace-state");
        let replace_stale = temp.path().join(".replace-state.tmp-42000-1");
        fs::write(&replace_stale, b"older failed write").expect("seed replacement stale temp");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let error = write_rooted_atomic(&root_guard, &replace_target, b"current write")
                .expect_err("Unix must quarantine a stale replacement before writing");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected replacement quarantine error: {error}"
            );
            assert!(!replace_target.exists());
            assert!(!replace_stale.exists());
            assert_eq!(
                fs::read(
                    recovery_quarantine_path(temp.path()).join("mutable-state-recovery-000000")
                )
                .expect("read isolated replacement temp"),
                b"older failed write"
            );
            clear_recovery_quarantine_offline(temp.path());
        }
        #[cfg(windows)]
        {
            write_rooted_atomic(&root_guard, &replace_target, b"current write")
                .expect("Windows removes the exact opened stale temp before writing");
            assert_eq!(
                fs::read(&replace_target).expect("read replacement"),
                b"current write"
            );
            assert!(!replace_stale.exists());
        }

        let create_target = temp.path().join("create-state");
        let create_stale = temp.path().join(".create-state.tmp-42000-2");
        fs::write(&create_stale, b"older failed create").expect("seed create stale temp");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let error = write_rooted_atomic_expected(
                &root_guard,
                &create_target,
                b"current create",
                governance_rooted_fs::ExpectedFile::Missing,
            )
            .expect_err("Missing writes must quarantine a stale create before mutation");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected create quarantine error: {error}"
            );
            assert!(!create_target.exists());
            assert!(!create_stale.exists());
            assert_eq!(
                fs::read(
                    recovery_quarantine_path(temp.path()).join("mutable-state-recovery-000000")
                )
                .expect("read isolated create temp"),
                b"older failed create"
            );
            clear_recovery_quarantine_offline(temp.path());
        }
        #[cfg(windows)]
        {
            write_rooted_atomic_expected(
                &root_guard,
                &create_target,
                b"current create",
                governance_rooted_fs::ExpectedFile::Missing,
            )
            .expect("Windows removes the exact opened stale temp before create");
            assert_eq!(
                fs::read(&create_target).expect("read created target"),
                b"current create"
            );
            assert!(!create_stale.exists());
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn filesystem_publisher_reclaims_interrupted_authority_temp_at_startup() {
        let temp = tempdir().expect("tempdir");
        let stale_temp = temp
            .path()
            .join(format!(".{GOVERNANCE_PUBLICATION_STATE_FILE}.tmp-42000-1"));
        let stale_marker_temp = temp.path().join(format!(
            ".{GOVERNANCE_PUBLICATION_INITIALIZED_FILE}.tmp-42000-2"
        ));
        fs::write(&stale_temp, b"interrupted authoritative state")
            .expect("seed interrupted authority temp");
        fs::write(&stale_marker_temp, b"interrupted initialization marker")
            .expect("seed interrupted marker temp");

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let publisher = {
            let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("Unix startup must quarantine interrupted authority temporaries");
            assert!(
                error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected authority-temporary quarantine error: {error}"
            );
            assert!(!stale_temp.exists());
            assert!(!stale_marker_temp.exists());
            let quarantine = recovery_quarantine_path(temp.path());
            assert_eq!(
                fs::read(quarantine.join("authority-state-temp"))
                    .expect("read quarantined authority-state temporary"),
                b"interrupted authoritative state"
            );
            assert_eq!(
                fs::read(quarantine.join("authority-marker-temp"))
                    .expect("read quarantined authority-marker temporary"),
                b"interrupted initialization marker"
            );
            let restart_error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect_err("restart must retain quarantined authority temporaries");
            assert!(
                restart_error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected authority-temporary restart error: {restart_error}"
            );
            clear_recovery_quarantine_offline(temp.path());
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("startup succeeds after offline authority-temporary cleanup")
        };
        #[cfg(windows)]
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("Windows exact-handle cleanup reclaims authority temporaries");
        assert!(
            !stale_temp.exists(),
            "the canonical authoritative-state temp must be reclaimed without weakening authoritative validation"
        );
        assert!(
            !stale_marker_temp.exists(),
            "the canonical initialization-marker temp must be reclaimed without weakening authoritative validation"
        );
        drop(publisher);
    }

    #[test]
    fn filesystem_publisher_persists_explicit_empty_authority_and_marker() {
        let temp = tempdir().expect("tempdir");
        let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("initialize publication authority");
        assert!(
            temp.path()
                .join(GOVERNANCE_PUBLICATION_STATE_FILE)
                .is_file(),
            "a pristine root must gain an explicit empty authority"
        );
        assert_eq!(
            fs::read(temp.path().join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE))
                .expect("read initialization marker"),
            GOVERNANCE_PUBLICATION_INITIALIZED_BODY
        );
        let state = read_publication_state_fixture(temp.path());
        assert_eq!(state.get("generation").and_then(JsonValue::as_u64), Some(0));
        drop(publisher);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn filesystem_publisher_restart_accepts_bounded_retained_authority_generations() {
        let temp = tempdir().expect("tempdir");
        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("initialize publication authority"),
        );
        let state_path = temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE);
        let state_bytes = fs::read(&state_path).expect("read empty authority bytes");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        write_rooted_atomic(&root_guard, &state_path, &state_bytes)
            .expect("replace authority while retaining its exact predecessor");
        drop(root_guard);
        let retained = temp.path().join(format!(
            ".{GOVERNANCE_PUBLICATION_STATE_FILE}.retained-v1-0000"
        ));
        assert_eq!(
            fs::read(&retained).expect("read retained authority generation"),
            state_bytes
        );

        drop(
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
                .expect("restart accepts canonical bounded retained generations"),
        );
        assert_eq!(
            fs::read(&retained).expect("reread retained authority generation"),
            state_bytes,
            "startup must not mutate a retained predecessor"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_missing_authority_without_deleting_history() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish committed settlement");
        let (encoded_path, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
        let state = read_publication_state_fixture(temp.path());
        let car_paths = state
            .get("car_queue")
            .and_then(|queue| queue.get("segments"))
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(JsonValue::as_object)
            .map(|segment| {
                ["car_path", "plan_path", "manifest_path"].map(|field| {
                    temp.path().join(
                        segment
                            .get(field)
                            .and_then(JsonValue::as_str)
                            .expect("committed CAR artifact path"),
                    )
                })
            })
            .expect("committed CAR segment");
        drop(publisher);
        fs::remove_file(temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE))
            .expect("remove authority fixture");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("missing initialized authority must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("state is missing"));
        for path in [encoded_path, json_path].into_iter().chain(car_paths) {
            assert!(
                path.is_file(),
                "missing authority must not reclaim committed artifact `{}`",
                path.display()
            );
        }
    }

    #[test]
    fn filesystem_publisher_rejects_authority_bound_source_corruption_at_startup() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish committed settlement");
        let (encoded_path, _) = only_published_source_paths(temp.path(), "deal_settlement");
        drop(publisher);

        let substituted = b"substituted committed source";
        fs::write(&encoded_path, substituted).expect("substitute committed source");
        fs::write(
            digest_sidecar_path_for(&encoded_path),
            format!("{}\n", blake3::hash(substituted).to_hex()),
        )
        .expect("substitute matching unauthoritative sidecar");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("authority-bound source corruption must fail startup");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("does not match publish-index")
                || error.to_string().contains("canonical source projection"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read(&encoded_path).expect("read preserved substituted source"),
            substituted,
            "startup must fail closed without rewriting corrupted immutable history"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_authority_bound_car_corruption_at_startup() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish committed settlement");
        let car_path = read_publication_state_fixture(temp.path())
            .get("car_queue")
            .and_then(|queue| queue.get("segments"))
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(|segment| segment.get("car_path"))
            .and_then(JsonValue::as_str)
            .map(|path| temp.path().join(path))
            .expect("committed CAR path");
        drop(publisher);

        let substituted = b"substituted committed CAR";
        fs::write(&car_path, substituted).expect("substitute committed CAR");
        fs::write(
            digest_sidecar_path_for(&car_path),
            format!("{}\n", blake3::hash(substituted).to_hex()),
        )
        .expect("substitute matching unauthoritative CAR sidecar");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("authority-bound CAR corruption must fail startup");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("authoritative canonical bytes"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read(&car_path).expect("read preserved substituted CAR"),
            substituted,
            "startup must fail closed without rewriting corrupted immutable history"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_missing_committed_publication_artifacts_at_startup() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish committed settlement");
        let state = read_publication_state_fixture(temp.path());
        let car_path = state
            .get("car_queue")
            .and_then(|queue| queue.get("segments"))
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(|segment| segment.get("car_path"))
            .and_then(JsonValue::as_str)
            .expect("committed CAR path")
            .to_owned();
        drop(publisher);
        fs::remove_file(temp.path().join(car_path)).expect("remove committed CAR artifact");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("startup must reject a missing committed artifact");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("committed governance CAR artifacts are missing")
        );
    }

    #[test]
    fn atomic_temp_path_preserves_extensions_and_hides_file() {
        let base = Path::new("/tmp/settlement/artifact.norito.to");
        let tmp = temp_path_for_atomic(base, 42, 7);
        let tmp_name = tmp
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(
            tmp_name.starts_with(".artifact.norito.to.tmp-42-7"),
            "tmp name should keep extensions and add suffix, got {tmp_name}"
        );
        assert!(
            tmp.as_os_str()
                .to_string_lossy()
                .ends_with(".norito.to.tmp-42-7"),
            "tmp path should append to existing extensions"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_output() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let root_guard =
            GovernanceFilesystemRootGuard::capture_writer(&temp_path).expect("retain test root");
        let target_path = temp_path.join("target.to");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("governance.to");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err =
            write_atomic(&root_guard, &output_path, b"replace").expect_err("reject symlink output");
        let message = err.to_string();

        assert!(
            message.contains("regular file") || message.contains("reparse"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[test]
    fn write_atomic_surfaces_post_rename_directory_sync_failure() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("governance.to");
        let error = write_atomic_with_directory_sync(&output_path, b"committed", |_| {
            Err(io::Error::other("injected directory sync failure"))
        })
        .expect_err("directory sync failure must be reported");

        assert!(
            error
                .to_string()
                .contains("injected directory sync failure")
        );
        assert_eq!(
            fs::read(&output_path).expect("renamed output remains visible"),
            b"committed",
            "the caller must treat this as committed-unknown and retry idempotently"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_parent() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let root_guard =
            GovernanceFilesystemRootGuard::capture_writer(&temp_path).expect("retain test root");
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("governance.to");

        let err =
            write_atomic(&root_guard, &output_path, b"replace").expect_err("reject symlink parent");
        let message = err.to_string();

        assert!(
            message.contains("directory")
                || message.contains("symbolic")
                || message.contains("reparse"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join("governance.to").exists(),
            "symlink parent should not receive output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_atomic_temp_file_rejects_preexisting_symlink() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let target_path = temp_path.join("target.tmp");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let tmp_path = temp_path.join(".governance.to.tmp");
        std::os::unix::fs::symlink(&target_path, &tmp_path).expect("create symlink");

        let err = open_atomic_temp_file(&tmp_path).expect_err("reject temp symlink");
        let message = err.to_string();

        assert!(
            message.contains("failed to create atomic temp"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[test]
    fn filesystem_publisher_writes_gc_audit_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let payload = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: [0x33; 32],
            provider_id: [0x44; 32],
            evicted_at_unix: 1_700_000_333,
            freed_bytes: 4_096,
            reason: "retention_expired".into(),
            blocked_reason: None,
        };
        let header = SorafsAuditHeaderV1 {
            sequence: 7,
            occurred_at_unix: payload.evicted_at_unix,
            signer: GC_AUDIT_SIGNER_V1.into(),
            payload_digest: gc_audit_payload_digest_v1(&payload).expect("audit digest"),
        };
        let event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let encoded = norito::to_bytes(&event).expect("encode GC audit event");

        publisher
            .publish_gc_audit_event(&event, &encoded)
            .expect("publish gc audit");

        let (_, json_path) = only_published_source_paths(temp.path(), "gc_audit");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let reason = value
            .get("metadata")
            .and_then(|meta| meta.get("reason"))
            .and_then(JsonValue::as_str)
            .expect("reason");
        assert_eq!(reason, "retention_expired");
        assert_single_runtime_external(temp.path(), "gc_audit", &encoded);
    }

    #[test]
    fn filesystem_publisher_writes_reconciliation_report_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id: [0x55; 32],
            generated_at_unix: 1_700_000_444,
            repair_snapshot_hash: [0x01; 32],
            retention_snapshot_hash: [0x02; 32],
            gc_snapshot_hash: [0x03; 32],
            repair_task_count: 2,
            retention_manifest_count: 3,
            gc_evictions_total: 4,
            gc_freed_bytes_total: 5,
            divergence_count: 1,
            appeal_finance: None,
        };
        let encoded = norito::to_bytes(&report).expect("encode reconciliation report");

        publisher
            .publish_reconciliation_report(&report, &encoded)
            .expect("publish reconciliation report");

        let (_, json_path) = only_published_source_paths(temp.path(), "reconciliation");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        let provider = metadata
            .get("provider")
            .and_then(JsonValue::as_str)
            .expect("provider");
        let divergence = metadata
            .get("divergence_count")
            .and_then(JsonValue::as_u64)
            .expect("divergence_count");
        assert_eq!(provider, hex::encode(report.provider_id));
        assert_eq!(divergence, 1);
        assert_single_runtime_external(temp.path(), "reconciliation", &encoded);
    }

    #[test]
    fn filesystem_publisher_writes_reputation_snapshot_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (snapshot, encoded) = sample_reputation_snapshot();

        publisher
            .publish_reputation_snapshot(&snapshot, &encoded)
            .expect("publish reputation snapshot");

        let snapshot_hex = hex::encode(snapshot.snapshot.snapshot_id);
        let (_, json_path) = only_published_source_paths(temp.path(), "reputation_snapshot");
        assert!(!temp.path().join("reputation").join("latest.to").exists());
        assert!(!temp.path().join("reputation").join("latest.json").exists());
        let json_bytes = fs::read(json_path).expect("read reputation json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        assert_eq!(
            metadata.get("snapshot_id_hex").and_then(JsonValue::as_str),
            Some(snapshot_hex.as_str())
        );
        assert_eq!(
            metadata.get("provider_count").and_then(JsonValue::as_u64),
            Some(snapshot.snapshot.providers.len() as u64)
        );
    }

    #[test]
    fn reputation_snapshot_metadata_supports_the_full_encoded_bound_without_payload_duplication() {
        let (snapshot, _) = sample_reputation_snapshot();
        let digest_hex = "a5".repeat(32);
        let body = reputation_snapshot_json(
            &snapshot,
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
            &digest_hex,
        )
        .expect("project maximum-length snapshot metadata");
        assert!(
            body.len() <= GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
            "bounded metadata must fit the JSON source limit"
        );
        let value: JsonValue = json::from_str(&body).expect("decode snapshot metadata");
        assert_eq!(
            value.get("schema").and_then(JsonValue::as_str),
            Some("sorafs.reputation_snapshot.metadata.v1")
        );
        assert!(
            value.get("signed_snapshot").is_none(),
            "the canonical payload belongs only in payload.to"
        );
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("snapshot metadata");
        assert_eq!(
            metadata.get("encoded_len").and_then(JsonValue::as_u64),
            Some(GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES as u64)
        );
        assert!(
            !metadata.contains_key("encoded_base64"),
            "JSON metadata must not duplicate the canonical encoded payload"
        );
    }
}
