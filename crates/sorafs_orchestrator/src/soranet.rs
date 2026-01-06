//! SoraNet client helpers for guard selection, pinning, and persistence.
//!
//! The guard selection engine keeps entry relays sticky for a configurable
//! retention window, updates metadata from the latest directory consensus, and
//! persists selections using Norito so CLI tooling and SDKs can reuse guard
//! caches across sessions. This underpins SNNet-5 by providing deterministic
//! guard pinning ahead of the circuit manager and transport integration.

#![allow(unexpected_cfgs)]

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    convert::TryFrom,
    fmt,
    num::{NonZeroU64, NonZeroUsize},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::Hasher as Blake3Hasher;
use ed25519_dalek::VerifyingKey as Ed25519VerifyingKey;
use iroha_crypto::soranet::{
    certificate::{CertificateError, CertificateValidationPhase, RelayCertificateBundleV2},
    directory::{
        GuardDirectoryIssuerV1, GuardDirectorySnapshotV2, compute_issuer_fingerprint,
        decode_validation_phase,
    },
    handshake::HandshakeSuite,
};
use iroha_data_model::soranet::prelude::{RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayId};
use iroha_logger::info;
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};
use rand::random;
use thiserror::Error;

use crate::{AnonymityPolicy, SORANET_BANDWIDTH_UNIT_BYTES};

const GUARD_SET_VERSION_MIN: u8 = 1;
const GUARD_SET_VERSION: u8 = 7;
const DEFAULT_RETENTION_SECS: u64 = 30 * 24 * 60 * 60;
const CACHE_TAG_LABEL: &[u8] = b"soranet.guard_cache.tag.v1";
const CACHE_NONCE_LEN: usize = 16;
const CACHE_TAG_LEN: usize = 32;
const GUARD_CAPABILITY_PQ_SHIFT: u32 = 96;
const GUARD_CAPABILITY_GUARD_SHIFT: u32 = 64;
const GUARD_CAPABILITY_BANDWIDTH_SHIFT: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardCapabilityComponents {
    pq_rank: u8,
    pq_flag: bool,
    guard_weight: u32,
    bandwidth_units: u32,
    reputation_weight: u32,
}

impl GuardCapabilityComponents {
    pub(crate) fn from_descriptor(
        descriptor: &RelayDescriptor,
        policy: AnonymityPolicy,
        pq_capable: bool,
    ) -> Self {
        Self::from_inputs(
            guard_pq_rank(policy, pq_capable),
            pq_capable,
            descriptor.guard_weight,
            descriptor.bandwidth_bytes_per_sec,
            descriptor.reputation_weight,
        )
    }

    pub(crate) fn from_inputs(
        pq_rank: u8,
        pq_flag: bool,
        guard_weight: u32,
        bandwidth_bytes_per_sec: u64,
        reputation_weight: u32,
    ) -> Self {
        let bandwidth_units = if bandwidth_bytes_per_sec == 0 {
            0
        } else {
            bandwidth_bytes_per_sec
                .div_ceil(SORANET_BANDWIDTH_UNIT_BYTES)
                .min(u64::from(u32::MAX)) as u32
        };
        Self {
            pq_rank,
            pq_flag,
            guard_weight,
            bandwidth_units,
            reputation_weight,
        }
    }

    pub(crate) fn score(&self) -> u128 {
        let pq_bits = ((u128::from(self.pq_rank) << 1) | u128::from(self.pq_flag as u8))
            << GUARD_CAPABILITY_PQ_SHIFT;
        let guard_bits = u128::from(self.guard_weight) << GUARD_CAPABILITY_GUARD_SHIFT;
        let bandwidth_bits = u128::from(self.bandwidth_units) << GUARD_CAPABILITY_BANDWIDTH_SHIFT;
        let reputation_bits = u128::from(self.reputation_weight);

        pq_bits | guard_bits | bandwidth_bits | reputation_bits
    }

    pub(crate) fn cmp(&self, other: &Self) -> Ordering {
        other
            .pq_rank
            .cmp(&self.pq_rank)
            .then_with(|| (other.pq_flag as u8).cmp(&(self.pq_flag as u8)))
            .then_with(|| other.guard_weight.cmp(&self.guard_weight))
            .then_with(|| other.bandwidth_units.cmp(&self.bandwidth_units))
            .then_with(|| other.reputation_weight.cmp(&self.reputation_weight))
    }

    pub(crate) fn pq_rank(&self) -> u8 {
        self.pq_rank
    }

    pub(crate) fn has_pq_certificate(&self) -> bool {
        self.pq_flag
    }

    pub(crate) fn guard_weight(&self) -> u32 {
        self.guard_weight
    }

    pub(crate) fn bandwidth_units(&self) -> u32 {
        self.bandwidth_units
    }

    pub(crate) fn reputation_weight(&self) -> u32 {
        self.reputation_weight
    }
}

/// Generic endpoint descriptor used for both relay metadata and persisted guard entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Scheme-qualified endpoint URL (e.g. `soranet://relay1.example:443`).
    pub url: String,
    /// Lower values indicate higher priority. Ties fall back to lexical URL order.
    pub priority: u8,
    /// Stream tags advertised by this endpoint.
    pub tags: Vec<EndpointTag>,
}

impl Endpoint {
    /// Creates a new endpoint descriptor.
    #[must_use]
    pub fn new(url: impl Into<String>, priority: u8) -> Self {
        Self {
            url: url.into(),
            priority,
            tags: Vec::new(),
        }
    }

    /// Creates a new endpoint descriptor with explicit tags.
    #[must_use]
    pub fn with_tags(
        url: impl Into<String>,
        priority: u8,
        tags: impl Into<Vec<EndpointTag>>,
    ) -> Self {
        Self {
            url: url.into(),
            priority,
            tags: tags.into(),
        }
    }

    fn ordering_key(&self) -> (u8, &str) {
        (self.priority, self.url.as_str())
    }

    /// Returns the stream tags attached to this endpoint.
    #[must_use]
    pub fn tags(&self) -> &[EndpointTag] {
        &self.tags
    }

    /// Returns `true` when the endpoint advertises the provided tag.
    #[must_use]
    pub fn has_tag(&self, tag: EndpointTag) -> bool {
        self.tags.contains(&tag)
    }
}

/// Tags advertised by relay endpoints describing supported streaming overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointTag {
    /// Endpoint accepts Norito streaming over SoraNet.
    NoritoStream,
}

impl EndpointTag {
    const NORITO_STREAM_LABEL: &'static str = "norito-stream";

    /// Convert a textual label into an endpoint tag.
    pub fn from_label(label: &str) -> Result<Self, EndpointTagError> {
        match label.trim().to_ascii_lowercase().as_str() {
            Self::NORITO_STREAM_LABEL => Ok(Self::NoritoStream),
            other => Err(EndpointTagError::UnknownTag(other.to_string())),
        }
    }

    /// Returns the canonical label for this tag.
    #[must_use]
    pub const fn as_label(&self) -> &'static str {
        match self {
            Self::NoritoStream => Self::NORITO_STREAM_LABEL,
        }
    }
}

impl fmt::Display for EndpointTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

/// Errors produced while parsing endpoint tags.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EndpointTagError {
    /// Tag label was not recognised.
    #[error("unknown endpoint tag `{0}`")]
    UnknownTag(String),
}

/// Relay roles advertised in the SoraNet directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayRoles {
    /// Relay may act as an entry (guard) hop.
    pub entry: bool,
    /// Relay may act as a middle hop.
    pub middle: bool,
    /// Relay may act as an exit hop.
    pub exit: bool,
}

impl RelayRoles {
    /// Construct a new role descriptor.
    #[must_use]
    pub const fn new(entry: bool, middle: bool, exit: bool) -> Self {
        Self {
            entry,
            middle,
            exit,
        }
    }

    /// Returns `true` when the descriptor currently advertises exit capability.
    #[must_use]
    pub const fn exit(&self) -> bool {
        self.exit
    }

    /// Clear the exit capability flag.
    pub fn disable_exit(&mut self) {
        self.exit = false;
    }
}

/// Metadata describing a relay extracted from the directory consensus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayDescriptor {
    /// Stable relay identifier (descriptor hash or relay fingerprint).
    pub relay_id: [u8; 32],
    /// Weight used when scheduling guards; higher values are preferred.
    pub guard_weight: u32,
    /// Sustained bandwidth advertised by the relay in bytes per second.
    pub bandwidth_bytes_per_sec: u64,
    /// Reputation or staking weight advertised by the directory.
    pub reputation_weight: u32,
    /// Roles advertised by the relay.
    pub roles: RelayRoles,
    /// Transport endpoints suitable for establishing circuits.
    pub endpoints: Vec<Endpoint>,
    /// Optional ML-KEM public key advertised by the relay (Kyber-768).
    pub pq_kem_public: Option<Vec<u8>>,
    /// Relay certificate bundle (SRCv2) if supplied by the directory.
    pub certificate: Option<RelayCertificateBundleV2>,
    /// Optional topology metadata used to bias path selection.
    pub path_metadata: PathMetadata,
}

/// Topology metadata used when choosing relays and enforcing diversity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathMetadata {
    /// Approximate round-trip latency in milliseconds.
    pub avg_rtt_ms: Option<u32>,
    /// Region label (e.g., `us-west`, `eu-central`).
    pub region: Option<String>,
    /// Autonomous System Number advertised by the operator, if known.
    pub asn: Option<u32>,
    /// Whether the relay belongs to a validator-class fast lane.
    pub validator_lane: bool,
    /// Whether validator-only meshes may bypass MASQUE/obfs wrappers when using this relay.
    pub masque_bypass_allowed: bool,
}

/// Outcome when applying a collection of path hints to a guard directory.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PathHintReport {
    /// Number of hints that matched descriptors.
    pub applied: usize,
    /// Relay identifiers (hex) that did not match any descriptor.
    pub missing: Vec<String>,
}

impl PathMetadata {
    /// Apply a hint to override metadata fields.
    fn apply_hint(&mut self, hint: &RelayPathHint) {
        if let Some(asn) = hint.asn {
            self.asn = Some(asn);
        }
        if let Some(rtt) = hint.avg_rtt_ms {
            self.avg_rtt_ms = Some(rtt);
        }
        if let Some(region) = hint.region.clone() {
            let trimmed = region.trim();
            if !trimmed.is_empty() {
                self.region = Some(trimmed.to_string());
            }
        }
        if hint.validator_lane {
            self.validator_lane = true;
        }
        if hint.masque_bypass_allowed {
            self.masque_bypass_allowed = true;
        }
    }
}

/// Hint describing supplemental relay metadata not present in the directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayPathHint {
    /// Relay identifier targeted by the hint.
    pub relay_id: [u8; 32],
    /// Optional latency/region/ASN metadata to apply.
    pub avg_rtt_ms: Option<u32>,
    /// Region label (e.g., `us-west`, `eu-central`).
    pub region: Option<String>,
    /// Autonomous System Number advertised by the operator, if known.
    pub asn: Option<u32>,
    /// Whether the relay participates in the validator fast lane.
    pub validator_lane: bool,
    /// Whether validator-only meshes may bypass MASQUE/obfs wrappers when using this relay.
    pub masque_bypass_allowed: bool,
}

impl RelayPathHint {
    /// Construct a hint from a hex-encoded relay identifier.
    ///
    /// # Errors
    /// Returns an error when the supplied hex string does not decode to 32 bytes.
    pub fn from_hex(
        relay_id_hex: &str,
        avg_rtt_ms: Option<u32>,
        region: Option<String>,
        asn: Option<u32>,
        validator_lane: bool,
        masque_bypass_allowed: bool,
    ) -> Result<Self, String> {
        let trimmed = relay_id_hex.trim();
        if trimmed.len() != 64 {
            return Err(format!(
                "relay_id_hex must be 64 hex characters; got {}",
                trimmed.len()
            ));
        }
        let mut relay_id = [0u8; 32];
        hex::decode_to_slice(trimmed, &mut relay_id)
            .map_err(|_| "relay_id_hex must decode to 32 bytes".to_string())?;
        Ok(Self {
            relay_id,
            avg_rtt_ms,
            region,
            asn,
            validator_lane,
            masque_bypass_allowed,
        })
    }
}

impl RelayDescriptor {
    /// Returns true when the relay may serve as an entry guard.
    #[must_use]
    pub fn is_entry_guard(&self) -> bool {
        self.roles.entry
    }

    /// Returns the highest priority endpoint, if any.
    #[must_use]
    pub fn primary_endpoint(&self) -> Option<&Endpoint> {
        self.endpoints
            .iter()
            .min_by(|left, right| left.ordering_key().cmp(&right.ordering_key()))
    }

    /// Returns the endpoint matching the provided tag, falling back to primary if unavailable.
    pub fn preferred_endpoint(&self, preferred_tag: Option<EndpointTag>) -> Option<&Endpoint> {
        if let Some(tag) = preferred_tag {
            let mut tagged = self
                .endpoints
                .iter()
                .filter(|endpoint| endpoint.has_tag(tag))
                .collect::<Vec<_>>();
            tagged.sort_by(|left, right| left.ordering_key().cmp(&right.ordering_key()));
            if let Some(endpoint) = tagged.first() {
                return Some(endpoint);
            }
        }
        self.primary_endpoint()
    }

    /// Returns true when the descriptor advertises PQ-capable handshakes.
    ///
    /// This method does not validate certificate lifetimes; use
    /// [`RelayDescriptor::is_pq_capable_at`] when time bounds matter.
    #[must_use]
    pub fn is_pq_capable(&self) -> bool {
        self.pq_kem_public().is_some()
    }

    /// Returns true when the descriptor advertises PQ-capable handshakes and any
    /// embedded certificate is valid at `now_unix`.
    #[must_use]
    pub fn is_pq_capable_at(&self, now_unix: u64) -> bool {
        self.pq_kem_public_at(now_unix).is_some()
    }

    /// Iterate over endpoints that advertise the provided tag.
    pub fn endpoints_with_tag(&self, tag: EndpointTag) -> impl Iterator<Item = &Endpoint> {
        self.endpoints
            .iter()
            .filter(move |endpoint| endpoint.has_tag(tag))
    }

    /// Returns the advertised ML-KEM public key, if present.
    #[must_use]
    pub fn pq_kem_public(&self) -> Option<&[u8]> {
        if let Some(bundle) = self.certificate.as_ref()
            && !bundle.certificate.pq_kem_public.is_empty()
        {
            return Some(bundle.certificate.pq_kem_public.as_slice());
        }
        self.pq_kem_public.as_deref()
    }

    fn pq_kem_public_at(&self, now_unix: u64) -> Option<&[u8]> {
        if let Some(bundle) = self.certificate.as_ref() {
            if !is_certificate_valid_at(bundle, now_unix) {
                return None;
            }
            if !bundle.certificate.pq_kem_public.is_empty() {
                return Some(bundle.certificate.pq_kem_public.as_slice());
            }
            return None;
        }
        self.pq_kem_public.as_deref()
    }

    /// Returns the relay certificate bundle, if supplied.
    #[must_use]
    pub fn certificate(&self) -> Option<&RelayCertificateBundleV2> {
        self.certificate.as_ref()
    }

    /// Returns certificate metadata distilled for weighting and telemetry.
    #[must_use]
    pub fn certificate_metadata(&self) -> Option<GuardCertificateMetadata> {
        self.certificate
            .as_ref()
            .map(GuardCertificateMetadata::from_bundle)
    }

    /// Returns the certificate validity window when available.
    #[must_use]
    pub fn certificate_validity(&self) -> Option<(i64, i64)> {
        self.certificate.as_ref().map(|bundle| {
            (
                bundle.certificate.valid_after,
                bundle.certificate.valid_until,
            )
        })
    }
}

/// Resolver-friendly view over the directory consensus.
#[derive(Debug, Clone, Default)]
pub struct RelayDirectory {
    entries: Vec<RelayDescriptor>,
    directory_hash: Option<[u8; 32]>,
    published_at_unix: Option<i64>,
    valid_after_unix: Option<i64>,
    valid_until_unix: Option<i64>,
    validation_phase: Option<CertificateValidationPhase>,
}

/// Reason why an exit role was revoked while enforcing the bond policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDemotionReason {
    /// No ledger entry was found for the relay.
    MissingBond,
    /// Ledger entry exists but exit capability is currently disabled.
    ExitDisabledInLedger,
    /// Ledger entry uses an unexpected bond asset.
    BondAssetMismatch,
    /// Bond amount fell below the minimum exit requirement.
    BelowMinimum,
}

/// Outcome entry returned when exit capability is revoked for a relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitDemotion {
    /// Identifier of the relay that was demoted.
    pub relay_id: [u8; 32],
    /// Reason for the demotion.
    pub reason: ExitDemotionReason,
}

/// Errors surfaced while decoding or validating guard directory snapshots.
#[derive(Debug, Error)]
pub enum GuardDirectoryError {
    /// Norito payload failed to decode.
    #[error("failed to decode guard directory: {source}")]
    Decode {
        /// Underlying Norito error.
        #[source]
        source: norito::Error,
    },
    /// Snapshot version is not supported by the parser.
    #[error("guard directory version {version} is not supported")]
    UnsupportedVersion {
        /// Unsupported version value.
        version: u8,
    },
    /// Snapshot contained no relay entries.
    #[error("guard directory did not advertise any relays")]
    EmptyDirectory,
    /// Validation phase value was not recognised.
    #[error("guard directory validation phase {phase} is not recognised")]
    UnknownValidationPhase {
        /// Raw validation phase value.
        phase: u8,
    },
    /// Issuer fingerprint could not be recomputed from the provided keys.
    #[error("issuer fingerprint mismatch (expected {expected}, computed {computed})")]
    IssuerFingerprintMismatch {
        /// Fingerprint supplied by the snapshot.
        expected: String,
        /// Fingerprint recomputed locally.
        computed: String,
    },
    /// Issuer advertised more than once.
    #[error("guard directory advertised duplicate issuer {fingerprint}")]
    DuplicateIssuer {
        /// Fingerprint that was duplicated.
        fingerprint: String,
    },
    /// Issuer omitted ML-DSA keys required for the current validation phase.
    #[error("issuer {fingerprint} missing ML-DSA-65 public key for validation phase {phase:?}")]
    IssuerMissingMlDsa {
        /// Issuer fingerprint.
        fingerprint: String,
        /// Phase that triggered the requirement.
        phase: CertificateValidationPhase,
    },
    /// Certificate could not be decoded or verified.
    #[error("relay certificate decode or verification failed: {source}")]
    CertificateDecode {
        /// Underlying certificate error.
        #[source]
        source: CertificateError,
    },
    /// Certificate referenced unknown issuer fingerprint.
    #[error("relay certificate references unknown issuer {fingerprint}")]
    CertificateUnknownIssuer {
        /// Fingerprint missing from the issuer set.
        fingerprint: String,
    },
    /// Certificate directory hash did not match the snapshot hash.
    #[error("relay certificate directory hash mismatch (expected {expected}, got {found})")]
    CertificateDirectoryHashMismatch {
        /// Directory hash from the snapshot header.
        expected: String,
        /// Directory hash embedded in the certificate.
        found: String,
    },
    /// Issuer Ed25519 key failed validation.
    #[error("issuer {fingerprint} contains an invalid Ed25519 public key: {source}")]
    InvalidIssuerEd25519Key {
        /// Issuer fingerprint.
        fingerprint: String,
        /// Underlying parse error.
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    /// Certificate endpoint tags contained an unknown label.
    #[error("relay {relay} referenced unknown endpoint tag `{label}` ({reason})")]
    InvalidEndpointTag {
        /// Relay identifier (hex encoded).
        relay: String,
        /// Tag label that failed validation.
        label: String,
        /// Validation failure description.
        reason: String,
    },
}

fn parse_validation_phase(value: u8) -> Result<CertificateValidationPhase, GuardDirectoryError> {
    decode_validation_phase(value)
        .ok_or(GuardDirectoryError::UnknownValidationPhase { phase: value })
}

impl RelayDirectory {
    /// Build a directory from relay descriptors.
    #[must_use]
    pub fn new(entries: Vec<RelayDescriptor>) -> Self {
        Self {
            entries,
            directory_hash: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            validation_phase: None,
        }
    }

    /// Returns all relay descriptors.
    #[must_use]
    pub fn entries(&self) -> &[RelayDescriptor] {
        &self.entries
    }

    /// Returns the directory hash advertised by the snapshot.
    #[must_use]
    pub fn directory_hash(&self) -> Option<[u8; 32]> {
        self.directory_hash
    }

    /// Returns the published-at timestamp (Unix seconds) if known.
    #[must_use]
    pub fn published_at(&self) -> Option<i64> {
        self.published_at_unix
    }

    /// Returns the validity window start timestamp (Unix seconds) if known.
    #[must_use]
    pub fn valid_after(&self) -> Option<i64> {
        self.valid_after_unix
    }

    /// Returns the validity window end timestamp (Unix seconds) if known.
    #[must_use]
    pub fn valid_until(&self) -> Option<i64> {
        self.valid_until_unix
    }

    /// Returns the certificate validation phase advertised by the snapshot.
    #[must_use]
    pub fn validation_phase(&self) -> Option<CertificateValidationPhase> {
        self.validation_phase
    }

    /// Apply supplemental path hints to the relay descriptors in the directory.
    ///
    /// Returns the number of hints applied and any unmatched relay identifiers.
    pub fn apply_path_hints(&mut self, hints: &[RelayPathHint]) -> PathHintReport {
        let mut applied = 0usize;
        let mut missing = Vec::new();
        let mut by_id: HashMap<[u8; 32], &RelayPathHint> =
            hints.iter().map(|hint| (hint.relay_id, hint)).collect();

        for descriptor in &mut self.entries {
            if let Some(hint) = by_id.remove(&descriptor.relay_id) {
                descriptor.path_metadata.apply_hint(hint);
                applied += 1;
            }
        }

        for (relay_id, _) in by_id {
            missing.push(hex::encode(relay_id));
        }

        PathHintReport { applied, missing }
    }

    /// Decode a guard directory snapshot encoded with Norito and populate relay descriptors.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be decoded, if certificate verification fails,
    /// or when issuer metadata is inconsistent.
    pub fn from_guard_directory_bytes(bytes: &[u8]) -> Result<Self, GuardDirectoryError> {
        let snapshot = GuardDirectorySnapshotV2::from_bytes(bytes)
            .map_err(|source| GuardDirectoryError::Decode { source })?;
        Self::from_guard_directory_snapshot(snapshot)
    }

    fn from_guard_directory_snapshot(
        snapshot: GuardDirectorySnapshotV2,
    ) -> Result<Self, GuardDirectoryError> {
        if snapshot.version != 2 {
            return Err(GuardDirectoryError::UnsupportedVersion {
                version: snapshot.version,
            });
        }

        if snapshot.relays.is_empty() {
            return Err(GuardDirectoryError::EmptyDirectory);
        }

        let validation_phase = parse_validation_phase(snapshot.validation_phase)?;

        let mut issuers: HashMap<[u8; 32], GuardDirectoryIssuerV1> =
            HashMap::with_capacity(snapshot.issuers.len());
        for issuer in snapshot.issuers {
            let computed =
                compute_issuer_fingerprint(&issuer.ed25519_public, &issuer.mldsa65_public);
            if computed != issuer.fingerprint {
                return Err(GuardDirectoryError::IssuerFingerprintMismatch {
                    expected: hex::encode(issuer.fingerprint),
                    computed: hex::encode(computed),
                });
            }
            if validation_phase != CertificateValidationPhase::Phase1AllowSingle
                && issuer.mldsa65_public.is_empty()
            {
                return Err(GuardDirectoryError::IssuerMissingMlDsa {
                    fingerprint: hex::encode(issuer.fingerprint),
                    phase: validation_phase,
                });
            }
            let fingerprint = issuer.fingerprint;
            if issuers.insert(fingerprint, issuer).is_some() {
                return Err(GuardDirectoryError::DuplicateIssuer {
                    fingerprint: hex::encode(fingerprint),
                });
            }
        }

        let mut entries = Vec::with_capacity(snapshot.relays.len());
        for entry in snapshot.relays {
            let bundle = RelayCertificateBundleV2::from_cbor(&entry.certificate)
                .map_err(|source| GuardDirectoryError::CertificateDecode { source })?;

            if bundle.certificate.directory_hash != snapshot.directory_hash {
                return Err(GuardDirectoryError::CertificateDirectoryHashMismatch {
                    expected: hex::encode(snapshot.directory_hash),
                    found: hex::encode(bundle.certificate.directory_hash),
                });
            }

            let issuer_fingerprint = bundle.certificate.issuer_fingerprint;
            let issuer = issuers.get(&issuer_fingerprint).ok_or_else(|| {
                GuardDirectoryError::CertificateUnknownIssuer {
                    fingerprint: hex::encode(issuer_fingerprint),
                }
            })?;

            let ed25519_key =
                Ed25519VerifyingKey::from_bytes(&issuer.ed25519_public).map_err(|err| {
                    GuardDirectoryError::InvalidIssuerEd25519Key {
                        fingerprint: hex::encode(issuer_fingerprint),
                        source: err,
                    }
                })?;

            bundle
                .verify(&ed25519_key, &issuer.mldsa65_public, validation_phase)
                .map_err(|source| GuardDirectoryError::CertificateDecode { source })?;

            let mut endpoints = Vec::with_capacity(bundle.certificate.endpoints.len());
            for endpoint in &bundle.certificate.endpoints {
                let mut tags = Vec::with_capacity(endpoint.tags.len());
                for label in &endpoint.tags {
                    match EndpointTag::from_label(label) {
                        Ok(tag) => tags.push(tag),
                        Err(err) => {
                            return Err(GuardDirectoryError::InvalidEndpointTag {
                                relay: hex::encode(bundle.certificate.relay_id),
                                label: label.clone(),
                                reason: err.to_string(),
                            });
                        }
                    }
                }
                endpoints.push(Endpoint {
                    url: endpoint.url.clone(),
                    priority: endpoint.priority,
                    tags,
                });
            }

            entries.push(RelayDescriptor {
                relay_id: bundle.certificate.relay_id,
                guard_weight: bundle.certificate.guard_weight,
                bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
                reputation_weight: bundle.certificate.reputation_weight,
                roles: RelayRoles {
                    entry: bundle.certificate.roles.entry,
                    middle: bundle.certificate.roles.middle,
                    exit: bundle.certificate.roles.exit,
                },
                endpoints,
                pq_kem_public: None,
                certificate: Some(bundle),
                path_metadata: PathMetadata::default(),
            });
        }

        Ok(Self {
            entries,
            directory_hash: Some(snapshot.directory_hash),
            published_at_unix: Some(snapshot.published_at_unix),
            valid_after_unix: Some(snapshot.valid_after_unix),
            valid_until_unix: Some(snapshot.valid_until_unix),
            validation_phase: Some(validation_phase),
        })
    }

    #[cfg(test)]
    fn push(&mut self, descriptor: RelayDescriptor) {
        self.entries.push(descriptor);
    }

    /// Apply the relay bond policy to the directory, revoking exit roles that no longer satisfy it.
    ///
    /// Relays missing a bond ledger entry, using the wrong bond asset, or falling below the minimum
    /// exit requirement are demoted by clearing their exit flag. The function returns a list of
    /// demotions so callers can persist audit trails or surface telemetry.
    pub fn enforce_exit_bond_policy<'a, I>(
        &mut self,
        policy: &RelayBondPolicyV1,
        bonds: I,
    ) -> Vec<ExitDemotion>
    where
        I: IntoIterator<Item = (&'a RelayId, &'a RelayBondLedgerEntryV1)>,
    {
        let ledger: BTreeMap<[u8; 32], &RelayBondLedgerEntryV1> =
            bonds.into_iter().map(|(id, entry)| (*id, entry)).collect();

        let mut demotions = Vec::new();
        for descriptor in &mut self.entries {
            if !descriptor.roles.exit() {
                continue;
            }

            let Some(entry) = ledger.get(&descriptor.relay_id) else {
                descriptor.roles.disable_exit();
                demotions.push(ExitDemotion {
                    relay_id: descriptor.relay_id,
                    reason: ExitDemotionReason::MissingBond,
                });
                continue;
            };

            if !entry.exit_capable {
                descriptor.roles.disable_exit();
                demotions.push(ExitDemotion {
                    relay_id: descriptor.relay_id,
                    reason: ExitDemotionReason::ExitDisabledInLedger,
                });
                continue;
            }

            if entry.bond_asset_id != policy.bond_asset_id {
                descriptor.roles.disable_exit();
                demotions.push(ExitDemotion {
                    relay_id: descriptor.relay_id,
                    reason: ExitDemotionReason::BondAssetMismatch,
                });
                continue;
            }

            if !entry.meets_exit_minimum(policy) {
                descriptor.roles.disable_exit();
                demotions.push(ExitDemotion {
                    relay_id: descriptor.relay_id,
                    reason: ExitDemotionReason::BelowMinimum,
                });
            }
        }

        demotions
    }
}

/// Guard assignment with the pinned timestamp and endpoint metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRecord {
    /// Relay identifier selected as a guard.
    pub relay_id: [u8; 32],
    /// Unix timestamp (seconds) when the guard was first pinned.
    pub pinned_at_unix: u64,
    /// Endpoint used to build circuits through the guard.
    pub endpoint: Endpoint,
    /// Weighted bandwidth/reputation score advertised by the directory.
    pub guard_weight: u32,
    /// Sustained bandwidth advertised for the guard in bytes per second.
    pub bandwidth_bytes_per_sec: u64,
    /// Reputation or staking weight advertised for the guard.
    pub reputation_weight: u32,
    /// Cached ML-KEM public key advertised by the guard, if any.
    pub pq_kem_public: Option<Vec<u8>>,
    /// Relay certificate bundle if supplied when the guard was pinned.
    pub certificate: Option<RelayCertificateBundleV2>,
    /// Topology metadata used for path selection and diversity.
    pub path_metadata: PathMetadata,
}

/// Metadata distilled from a relay certificate for telemetry and weighting decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardCertificateMetadata {
    /// Publication timestamp included in the certificate payload.
    pub published_at: i64,
    /// Certificate validity window start (inclusive, Unix seconds).
    pub valid_after: i64,
    /// Certificate validity window end (exclusive, Unix seconds).
    pub valid_until: i64,
    /// Whether the bundle carries both Ed25519 and ML-DSA signatures.
    pub has_dual_signatures: bool,
    /// Advertised handshake suites ordered by preference.
    pub handshake_suites: Vec<HandshakeSuite>,
    /// Whether a PQ ML-KEM key was embedded in the certificate payload.
    pub has_pq_key: bool,
}

impl GuardCertificateMetadata {
    fn from_bundle(bundle: &RelayCertificateBundleV2) -> Self {
        let has_dual_signatures = bundle.signatures.mldsa65.is_some();
        let has_pq_key = !bundle.certificate.pq_kem_public.is_empty();
        Self {
            published_at: bundle.certificate.published_at,
            valid_after: bundle.certificate.valid_after,
            valid_until: bundle.certificate.valid_until,
            has_dual_signatures,
            handshake_suites: bundle.certificate.handshake_suites.clone(),
            has_pq_key,
        }
    }

    fn handshake_labels(&self) -> Vec<&'static str> {
        self.handshake_suites
            .iter()
            .map(|suite| suite.label())
            .collect()
    }
}

impl GuardRecord {
    /// Returns the certificate bundle if cached for this guard.
    #[must_use]
    pub fn certificate(&self) -> Option<&RelayCertificateBundleV2> {
        self.certificate.as_ref()
    }

    /// Returns metadata distilled from the cached certificate, when available.
    #[must_use]
    pub fn certificate_metadata(&self) -> Option<GuardCertificateMetadata> {
        self.certificate
            .as_ref()
            .map(GuardCertificateMetadata::from_bundle)
    }
}

/// Symmetric key used to authenticate persisted guard caches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardCacheKey([u8; 32]);

impl GuardCacheKey {
    /// Length of guard cache keys in bytes.
    pub const LENGTH: usize = 32;

    /// Construct a key from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse a guard cache key from a hex string.
    pub fn from_hex(hex: &str) -> Result<Self, GuardCacheKeyError> {
        let trimmed = hex.trim();
        if trimmed.len() != Self::LENGTH * 2 {
            return Err(GuardCacheKeyError::InvalidLength {
                expected: Self::LENGTH * 2,
                actual: trimmed.len(),
            });
        }
        let mut bytes = [0u8; Self::LENGTH];
        hex::decode_to_slice(trimmed, &mut bytes)
            .map_err(|_| GuardCacheKeyError::InvalidHex(trimmed.to_string()))?;
        Ok(Self(bytes))
    }

    /// Return the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Render the key as an uppercase hex string.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode_upper(self.0)
    }
}

impl fmt::Display for GuardCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Errors encountered while parsing guard cache keys.
#[derive(Debug, Error)]
pub enum GuardCacheKeyError {
    /// The supplied key did not decode from hex.
    #[error("guard cache key must be 64 hexadecimal characters; got `{0}`")]
    InvalidHex(String),
    /// The supplied key did not have the expected length.
    #[error("guard cache key must be {expected} characters; got {actual}")]
    InvalidLength {
        /// Expected number of characters.
        expected: usize,
        /// Actual number of characters observed.
        actual: usize,
    },
}

/// Authentication tag persisted alongside guard caches.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GuardCacheTag {
    nonce: [u8; CACHE_NONCE_LEN],
    mac: [u8; CACHE_TAG_LEN],
}

impl GuardCacheTag {
    fn generate(key: &GuardCacheKey, payload: &[u8]) -> Self {
        let nonce: [u8; CACHE_NONCE_LEN] = random();
        let mut hasher = Blake3Hasher::new_keyed(key.as_bytes());
        hasher.update(CACHE_TAG_LABEL);
        hasher.update(&nonce);
        hasher.update(payload);
        let mac = hasher.finalize();
        let mut mac_bytes = [0u8; CACHE_TAG_LEN];
        mac_bytes.copy_from_slice(mac.as_bytes());
        Self {
            nonce,
            mac: mac_bytes,
        }
    }

    fn verify(&self, key: &GuardCacheKey, payload: &[u8]) -> bool {
        let mut hasher = Blake3Hasher::new_keyed(key.as_bytes());
        hasher.update(CACHE_TAG_LABEL);
        hasher.update(&self.nonce);
        hasher.update(payload);
        let mac = hasher.finalize();
        mac.as_bytes() == &self.mac
    }

    fn to_hex(&self) -> String {
        let mut buf = [0u8; CACHE_NONCE_LEN + CACHE_TAG_LEN];
        buf[..CACHE_NONCE_LEN].copy_from_slice(&self.nonce);
        buf[CACHE_NONCE_LEN..].copy_from_slice(&self.mac);
        hex::encode_upper(buf)
    }

    fn from_hex(hex: &str) -> Result<Self, GuardSetPersistenceError> {
        let trimmed = hex.trim();
        if trimmed.len() != (CACHE_NONCE_LEN + CACHE_TAG_LEN) * 2 {
            return Err(GuardSetPersistenceError::InvalidCacheTagLength {
                expected: (CACHE_NONCE_LEN + CACHE_TAG_LEN) * 2,
                actual: trimmed.len(),
            });
        }
        let mut bytes = vec![0u8; CACHE_NONCE_LEN + CACHE_TAG_LEN];
        hex::decode_to_slice(trimmed, &mut bytes)
            .map_err(|_| GuardSetPersistenceError::InvalidCacheTagHex(trimmed.to_string()))?;
        let mut nonce = [0u8; CACHE_NONCE_LEN];
        let mut mac = [0u8; CACHE_TAG_LEN];
        nonce.copy_from_slice(&bytes[..CACHE_NONCE_LEN]);
        mac.copy_from_slice(&bytes[CACHE_NONCE_LEN..]);
        Ok(Self { nonce, mac })
    }
}

/// Guard set persisted across client sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardSet {
    guards: Vec<GuardRecord>,
}

impl GuardSet {
    /// Creates a new guard set with the supplied records.
    #[must_use]
    pub fn new(guards: Vec<GuardRecord>) -> Self {
        Self { guards }
    }

    /// Returns the guard entries in the set.
    #[must_use]
    pub fn guards(&self) -> &[GuardRecord] {
        &self.guards
    }

    /// Number of guards in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.guards.len()
    }

    /// Returns `true` when the guard set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }

    /// Iterate over guard records.
    pub fn iter(&self) -> impl Iterator<Item = &GuardRecord> {
        self.guards.iter()
    }

    /// Serialise the guard set to Norito bytes without authentication.
    pub fn encode(&self) -> Result<Vec<u8>, GuardSetPersistenceError> {
        self.encode_with_key(None)
    }

    /// Serialise the guard set to Norito bytes with an authentication tag.
    pub fn encode_with_key(
        &self,
        key: Option<&GuardCacheKey>,
    ) -> Result<Vec<u8>, GuardSetPersistenceError> {
        let mut persist = GuardSetPersist {
            version: GUARD_SET_VERSION,
            guards: self.guards.iter().map(GuardRecordPersist::from).collect(),
            cache_tag_hex: None,
        };
        if let Some(key) = key {
            let payload = to_bytes(&persist).map_err(GuardSetPersistenceError::Encode)?;
            let tag = GuardCacheTag::generate(key, &payload);
            persist.cache_tag_hex = Some(tag.to_hex());
        }
        to_bytes(&persist).map_err(GuardSetPersistenceError::Encode)
    }

    /// Deserialise a guard set from Norito bytes without verifying an authentication tag.
    pub fn decode(bytes: &[u8]) -> Result<Self, GuardSetPersistenceError> {
        Self::decode_with_key(bytes, None)
    }

    /// Deserialise a guard set from Norito bytes, verifying the authentication tag when present.
    pub fn decode_with_key(
        bytes: &[u8],
        key: Option<&GuardCacheKey>,
    ) -> Result<Self, GuardSetPersistenceError> {
        let payload: GuardSetPersist =
            decode_from_bytes(bytes).map_err(GuardSetPersistenceError::Decode)?;
        if payload.version < GUARD_SET_VERSION_MIN || payload.version > GUARD_SET_VERSION {
            return Err(GuardSetPersistenceError::UnsupportedVersion {
                version: payload.version,
            });
        }

        if let Some(tag_hex) = payload.cache_tag_hex.as_deref() {
            let key = key.ok_or(GuardSetPersistenceError::MissingCacheKey)?;
            let tag = GuardCacheTag::from_hex(tag_hex)?;
            let mut bare_payload = payload.clone();
            bare_payload.cache_tag_hex = None;
            let bare_bytes = to_bytes(&bare_payload).map_err(GuardSetPersistenceError::Encode)?;
            if !tag.verify(key, &bare_bytes) {
                return Err(GuardSetPersistenceError::InvalidCacheTag);
            }
        } else if key.is_some() && payload.version >= 3 {
            return Err(GuardSetPersistenceError::MissingCacheTag);
        }

        let guards = payload.guards.into_iter().map(GuardRecord::from).collect();
        Ok(Self { guards })
    }
}

/// Controls how long guard assignments remain sticky.
#[derive(Debug, Clone, Copy)]
pub struct GuardRetention {
    retention_secs: NonZeroU64,
}

impl GuardRetention {
    /// Creates a new retention policy.
    #[must_use]
    pub const fn new(retention_secs: NonZeroU64) -> Self {
        Self { retention_secs }
    }

    /// Returns the retention window in seconds.
    #[must_use]
    pub fn as_secs(self) -> u64 {
        self.retention_secs.get()
    }
}

impl Default for GuardRetention {
    fn default() -> Self {
        Self {
            retention_secs: NonZeroU64::new(DEFAULT_RETENTION_SECS)
                .expect("default retention must be non-zero"),
        }
    }
}

fn guard_capability_score(
    descriptor: &RelayDescriptor,
    policy: AnonymityPolicy,
    pq_capable: bool,
) -> u128 {
    GuardCapabilityComponents::from_descriptor(descriptor, policy, pq_capable).score()
}

fn guard_pq_rank(policy: AnonymityPolicy, pq_capable: bool) -> u8 {
    if !pq_capable {
        return 0;
    }
    match policy {
        AnonymityPolicy::StrictPq => 3,
        AnonymityPolicy::MajorityPq => 2,
        AnonymityPolicy::GuardPq => 1,
    }
}

/// Deterministic guard selector that honours existing pins and retention policy.
#[derive(Debug, Clone)]
pub struct GuardSelector {
    desired_count: NonZeroUsize,
    retention: GuardRetention,
}

impl GuardSelector {
    /// Creates a selector targeting `desired_count` entry guards.
    #[must_use]
    pub fn new(desired_count: NonZeroUsize) -> Self {
        Self {
            desired_count,
            retention: GuardRetention::default(),
        }
    }

    /// Overrides the retention policy applied to guards.
    #[must_use]
    pub fn with_retention(mut self, retention: GuardRetention) -> Self {
        self.retention = retention;
        self
    }

    /// Returns the configured retention policy.
    #[must_use]
    pub fn retention(&self) -> GuardRetention {
        self.retention
    }

    /// Returns the desired guard count.
    #[must_use]
    pub fn desired_count(&self) -> NonZeroUsize {
        self.desired_count
    }

    /// Selects guards based on the latest directory, preserving existing pins when possible.
    pub fn select(
        &self,
        directory: &RelayDirectory,
        existing: Option<&GuardSet>,
        now_unix: u64,
        policy: AnonymityPolicy,
    ) -> GuardSet {
        let mut selected = Vec::new();
        let mut selected_ids = Vec::<[u8; 32]>::new();
        let retention_secs = self.retention.as_secs();
        let max_count = self.desired_count.get();
        let pq_available = directory
            .entries()
            .iter()
            .filter(|descriptor| {
                descriptor.is_entry_guard() && descriptor.is_pq_capable_at(now_unix)
            })
            .count();
        let max_classical = allowed_classical(policy, max_count, pq_available);
        let mut selected_classical = 0usize;
        let mut used_asn: HashSet<u32> = HashSet::new();
        let mut used_regions: HashSet<String> = HashSet::new();

        let directory_map: BTreeMap<[u8; 32], &RelayDescriptor> = directory
            .entries()
            .iter()
            .map(|descriptor| (descriptor.relay_id, descriptor))
            .collect();

        if let Some(previous) = existing {
            for record in previous.iter() {
                if selected.len() >= max_count {
                    break;
                }
                if record.pinned_at_unix > now_unix {
                    continue;
                }
                let age = now_unix.saturating_sub(record.pinned_at_unix);
                if age > retention_secs {
                    continue;
                }
                let descriptor = match directory_map.get(&record.relay_id) {
                    Some(descriptor) => *descriptor,
                    None => continue,
                };
                if !descriptor.is_entry_guard() {
                    continue;
                }
                let endpoint = match descriptor.primary_endpoint() {
                    Some(endpoint) => endpoint.clone(),
                    None => continue,
                };
                if selected_ids.contains(&record.relay_id) {
                    continue;
                }
                let certificate = descriptor
                    .certificate()
                    .cloned()
                    .or_else(|| record.certificate().cloned());
                let certificate_candidate = certificate.is_some();
                let certificate =
                    certificate.and_then(|bundle| enforce_certificate_validity(bundle, now_unix));
                let pq_allowed = !certificate_candidate || certificate.is_some();
                let descriptor_pq = if pq_allowed {
                    descriptor.pq_kem_public_at(now_unix)
                } else {
                    None
                };
                let record_pq = if pq_allowed {
                    record.pq_kem_public.as_deref()
                } else {
                    None
                };
                let pq_kem_public =
                    preferred_pq_kem_public(certificate.as_ref(), descriptor_pq, record_pq);
                let is_pq = pq_kem_public.is_some();
                if !is_pq && selected_classical >= max_classical {
                    continue;
                }
                if policy == AnonymityPolicy::StrictPq && !is_pq {
                    continue;
                }
                selected_ids.push(record.relay_id);
                let path_metadata = reconcile_path_metadata(descriptor, Some(record));
                selected.push(GuardRecord {
                    relay_id: record.relay_id,
                    pinned_at_unix: record.pinned_at_unix,
                    endpoint,
                    guard_weight: descriptor.guard_weight,
                    bandwidth_bytes_per_sec: descriptor.bandwidth_bytes_per_sec,
                    reputation_weight: descriptor.reputation_weight,
                    pq_kem_public,
                    certificate,
                    path_metadata: path_metadata.clone(),
                });
                record_path_usage(&path_metadata, &mut used_asn, &mut used_regions);
                if !is_pq {
                    selected_classical += 1;
                }
            }
        }

        if selected.len() >= max_count {
            emit_guard_selection_telemetry(&selected);
            return GuardSet::new(selected);
        }

        let mut candidates: Vec<(PathSortKey, u128, &RelayDescriptor)> = directory
            .entries()
            .iter()
            .filter(|descriptor| descriptor.is_entry_guard())
            .filter(|descriptor| descriptor.primary_endpoint().is_some())
            .map(|descriptor| {
                let pq_capable = descriptor.is_pq_capable_at(now_unix);
                (
                    PathSortKey {
                        validator_lane: descriptor.path_metadata.validator_lane,
                        rtt_ms: descriptor.path_metadata.avg_rtt_ms,
                    },
                    guard_capability_score(descriptor, policy, pq_capable),
                    descriptor,
                )
            })
            .collect();
        candidates.sort_by(
            |(left_path, left_score, left_descriptor),
             (right_path, right_score, right_descriptor)| {
                right_path
                    .validator_lane
                    .cmp(&left_path.validator_lane)
                    .then_with(|| compare_rtt(left_path.rtt_ms, right_path.rtt_ms))
                    .then_with(|| right_score.cmp(left_score))
                    .then_with(|| left_descriptor.relay_id.cmp(&right_descriptor.relay_id))
            },
        );

        for (idx, (_path, _score, descriptor)) in candidates.iter().enumerate() {
            if selected.len() >= max_count {
                break;
            }
            if selected_ids.contains(&descriptor.relay_id) {
                continue;
            }
            if let Some(endpoint) = descriptor.primary_endpoint() {
                let certificate = descriptor.certificate().cloned();
                let certificate_candidate = certificate.is_some();
                let certificate =
                    certificate.and_then(|bundle| enforce_certificate_validity(bundle, now_unix));
                let pq_allowed = !certificate_candidate || certificate.is_some();
                let descriptor_pq = if pq_allowed {
                    descriptor.pq_kem_public_at(now_unix)
                } else {
                    None
                };
                let pq_kem_public =
                    preferred_pq_kem_public(certificate.as_ref(), descriptor_pq, None);
                let is_pq = pq_kem_public.is_some();
                if !is_pq && selected_classical >= max_classical {
                    continue;
                }
                if policy == AnonymityPolicy::StrictPq && !is_pq {
                    continue;
                }
                let path_metadata = reconcile_path_metadata(descriptor, None);

                let asn_collision = path_metadata
                    .asn
                    .map(|asn| used_asn.contains(&asn))
                    .unwrap_or(false);
                let region_collision = path_metadata
                    .region
                    .as_ref()
                    .map(|region| used_regions.contains(region))
                    .unwrap_or(false);
                if (asn_collision || region_collision)
                    && has_diverse_future(
                        &candidates,
                        idx,
                        &used_asn,
                        &used_regions,
                        now_unix,
                        policy,
                        max_classical,
                        selected_classical,
                        selected.len(),
                        max_count,
                    )
                {
                    continue;
                }

                selected_ids.push(descriptor.relay_id);
                selected.push(GuardRecord {
                    relay_id: descriptor.relay_id,
                    pinned_at_unix: now_unix,
                    endpoint: endpoint.clone(),
                    guard_weight: descriptor.guard_weight,
                    bandwidth_bytes_per_sec: descriptor.bandwidth_bytes_per_sec,
                    reputation_weight: descriptor.reputation_weight,
                    pq_kem_public,
                    certificate,
                    path_metadata: path_metadata.clone(),
                });
                record_path_usage(&path_metadata, &mut used_asn, &mut used_regions);
                if !is_pq {
                    selected_classical += 1;
                }
            }
        }

        emit_guard_selection_telemetry(&selected);
        GuardSet::new(selected)
    }
}

fn certificate_pq_kem_public(bundle: &RelayCertificateBundleV2) -> Option<Vec<u8>> {
    if bundle.certificate.pq_kem_public.is_empty() {
        None
    } else {
        Some(bundle.certificate.pq_kem_public.clone())
    }
}

fn preferred_pq_kem_public(
    certificate: Option<&RelayCertificateBundleV2>,
    descriptor_pq: Option<&[u8]>,
    record_pq: Option<&[u8]>,
) -> Option<Vec<u8>> {
    if let Some(bundle) = certificate
        && let Some(bytes) = certificate_pq_kem_public(bundle)
    {
        return Some(bytes);
    }
    if let Some(bytes) = descriptor_pq {
        return Some(bytes.to_vec());
    }
    record_pq.map(|value| value.to_vec())
}

fn is_certificate_valid_at(bundle: &RelayCertificateBundleV2, now_unix: u64) -> bool {
    let now_i64 = i64::try_from(now_unix).unwrap_or(i64::MAX);
    if bundle.certificate.valid_after > now_i64 {
        return false;
    }
    if bundle.certificate.valid_until > 0 && bundle.certificate.valid_until <= now_i64 {
        return false;
    }
    true
}

fn enforce_certificate_validity(
    bundle: RelayCertificateBundleV2,
    now_unix: u64,
) -> Option<RelayCertificateBundleV2> {
    if is_certificate_valid_at(&bundle, now_unix) {
        Some(bundle)
    } else {
        None
    }
}

fn emit_guard_selection_telemetry(guards: &[GuardRecord]) {
    for guard in guards {
        let relay_id_hex = hex::encode(guard.relay_id);
        let path = &guard.path_metadata;
        if let Some(metadata) = guard.certificate_metadata() {
            let handshake_joined = metadata.handshake_labels().join(",");
            info!(
                target: "telemetry::sorafs.guard",
                event = "guard_selected",
                relay_id = relay_id_hex.as_str(),
                certificate = true,
                published_at = metadata.published_at,
                valid_after = metadata.valid_after,
                valid_until = metadata.valid_until,
                has_dual_signatures = metadata.has_dual_signatures,
                has_pq_key = metadata.has_pq_key,
                handshake_suites = handshake_joined.as_str(),
                path_avg_rtt_ms = ?path.avg_rtt_ms,
                path_region = ?path.region,
                path_asn = ?path.asn,
                path_validator_lane = path.validator_lane,
                path_masque_bypass_allowed = path.masque_bypass_allowed,
            );
        } else {
            info!(
                target: "telemetry::sorafs.guard",
                event = "guard_selected",
                relay_id = relay_id_hex.as_str(),
                certificate = false,
                path_avg_rtt_ms = ?path.avg_rtt_ms,
                path_region = ?path.region,
                path_asn = ?path.asn,
                path_validator_lane = path.validator_lane,
                path_masque_bypass_allowed = path.masque_bypass_allowed,
            );
        }
    }
}

fn emit_circuit_build_telemetry(guard: &GuardRecord) {
    let relay_id_hex = hex::encode(guard.relay_id);
    if let Some(metadata) = guard.certificate_metadata() {
        let handshake_joined = metadata.handshake_labels().join(",");
        info!(
            target: "telemetry::sorafs.circuit",
            event = "circuit_build",
            relay_id = relay_id_hex.as_str(),
            certificate = true,
            published_at = metadata.published_at,
            valid_after = metadata.valid_after,
            valid_until = metadata.valid_until,
            has_dual_signatures = metadata.has_dual_signatures,
            has_pq_key = metadata.has_pq_key,
            handshake_suites = handshake_joined.as_str(),
            path_avg_rtt_ms = ?guard.path_metadata.avg_rtt_ms,
            path_region = ?guard.path_metadata.region,
            path_asn = ?guard.path_metadata.asn,
            path_validator_lane = guard.path_metadata.validator_lane,
            path_masque_bypass_allowed = guard.path_metadata.masque_bypass_allowed,
        );
    } else {
        info!(
            target: "telemetry::sorafs.circuit",
            event = "circuit_build",
            relay_id = relay_id_hex.as_str(),
            certificate = false,
            path_avg_rtt_ms = ?guard.path_metadata.avg_rtt_ms,
            path_region = ?guard.path_metadata.region,
            path_asn = ?guard.path_metadata.asn,
            path_validator_lane = guard.path_metadata.validator_lane,
            path_masque_bypass_allowed = guard.path_metadata.masque_bypass_allowed,
        );
    }
}

fn allowed_classical(policy: AnonymityPolicy, target: usize, pq_available: usize) -> usize {
    match policy {
        AnonymityPolicy::GuardPq | AnonymityPolicy::MajorityPq => {
            if pq_available == 0 {
                target
            } else {
                let capped_pq = pq_available.min(target);
                target.saturating_sub(capped_pq)
            }
        }
        AnonymityPolicy::StrictPq => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PathSortKey {
    validator_lane: bool,
    rtt_ms: Option<u32>,
}

fn compare_rtt(left: Option<u32>, right: Option<u32>) -> Ordering {
    match (left, right) {
        (Some(lhs), Some(rhs)) => lhs.cmp(&rhs),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn avoids_asn(descriptor: &RelayDescriptor, avoid_asn: &HashSet<u32>) -> bool {
    descriptor
        .path_metadata
        .asn
        .map(|asn| !avoid_asn.contains(&asn))
        .unwrap_or(true)
}

fn record_path_usage(
    metadata: &PathMetadata,
    used_asn: &mut HashSet<u32>,
    used_regions: &mut HashSet<String>,
) {
    if let Some(asn) = metadata.asn {
        used_asn.insert(asn);
    }
    if let Some(region) = metadata.region.as_ref()
        && !region.is_empty()
    {
        used_regions.insert(region.clone());
    }
}

#[allow(clippy::too_many_arguments)]
fn has_diverse_future(
    candidates: &[(PathSortKey, u128, &RelayDescriptor)],
    start_index: usize,
    used_asn: &HashSet<u32>,
    used_regions: &HashSet<String>,
    now_unix: u64,
    policy: AnonymityPolicy,
    max_classical: usize,
    selected_classical: usize,
    selected: usize,
    max_count: usize,
) -> bool {
    let slots_remaining = max_count.saturating_sub(selected);
    if slots_remaining == 0 {
        return false;
    }
    let classical_budget = max_classical.saturating_sub(selected_classical);
    for (_, _, descriptor) in candidates.iter().skip(start_index + 1) {
        if !descriptor.is_entry_guard() || descriptor.primary_endpoint().is_none() {
            continue;
        }
        let is_pq = descriptor.is_pq_capable_at(now_unix);
        if matches!(policy, AnonymityPolicy::StrictPq) && !is_pq {
            continue;
        }
        if !is_pq && classical_budget == 0 {
            continue;
        }
        if slots_remaining == 1 && !is_pq && classical_budget == 0 {
            continue;
        }
        let has_asn_diversity = descriptor
            .path_metadata
            .asn
            .map(|asn| !used_asn.contains(&asn))
            .unwrap_or(false);
        let has_region_diversity = descriptor
            .path_metadata
            .region
            .as_ref()
            .map(|region| !used_regions.contains(region))
            .unwrap_or(false);
        if has_asn_diversity || has_region_diversity {
            return true;
        }
    }
    false
}

fn reconcile_path_metadata(
    descriptor: &RelayDescriptor,
    record: Option<&GuardRecord>,
) -> PathMetadata {
    let mut metadata = descriptor.path_metadata.clone();
    if metadata.avg_rtt_ms.is_none() {
        metadata.avg_rtt_ms = record.and_then(|guard| guard.path_metadata.avg_rtt_ms);
    }
    metadata.region = metadata
        .region
        .take()
        .map(|region| region.trim().to_string())
        .filter(|region| !region.is_empty())
        .or_else(|| {
            record
                .and_then(|guard| guard.path_metadata.region.clone())
                .map(|region| region.trim().to_string())
                .filter(|region| !region.is_empty())
        });
    if metadata.asn.is_none() {
        metadata.asn = record.and_then(|guard| guard.path_metadata.asn);
    }
    if !metadata.validator_lane {
        metadata.validator_lane = record
            .map(|guard| guard.path_metadata.validator_lane)
            .unwrap_or(false);
    }
    if !metadata.masque_bypass_allowed {
        metadata.masque_bypass_allowed = record
            .map(|guard| guard.path_metadata.masque_bypass_allowed)
            .unwrap_or(false);
    }
    metadata
}

/// Errors surfaced when persisting guard sets.
#[derive(Debug, Error)]
pub enum GuardSetPersistenceError {
    /// Guard set encoding failed.
    #[error("failed to encode guard set: {0}")]
    Encode(norito::Error),
    /// Guard set decoding failed.
    #[error("failed to decode guard set: {0}")]
    Decode(norito::Error),
    /// Guard set version is unsupported.
    #[error("unsupported guard set version {version}")]
    UnsupportedVersion { version: u8 },
    /// Guard cache tag was present but no key was supplied.
    #[error("guard cache key required to decode tagged cache")]
    MissingCacheKey,
    /// Guard cache key was supplied but the payload lacked a tag.
    #[error("guard cache tag missing from payload")]
    MissingCacheTag,
    /// Guard cache tag validation failed.
    #[error("guard cache tag did not match encoded payload")]
    InvalidCacheTag,
    /// Guard cache tag contained invalid hex.
    #[error("guard cache tag must be hexadecimal; got `{0}`")]
    InvalidCacheTagHex(String),
    /// Guard cache tag length was unexpected.
    #[error("guard cache tag must be {expected} characters; got {actual}")]
    InvalidCacheTagLength {
        /// Expected number of characters.
        expected: usize,
        /// Actual number of characters received.
        actual: usize,
    },
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct GuardSetPersist {
    version: u8,
    guards: Vec<GuardRecordPersist>,
    #[norito(default)]
    cache_tag_hex: Option<String>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct GuardRecordPersist {
    relay_id: [u8; 32],
    pinned_at_unix: u64,
    endpoint: GuardEndpointPersistV1,
    #[norito(default)]
    guard_weight: u32,
    #[norito(default)]
    bandwidth_bytes_per_sec: u64,
    #[norito(default)]
    reputation_weight: u32,
    pq_kem_public_hex: Option<String>,
    #[norito(default)]
    certificate_base64: Option<String>,
    #[norito(default)]
    path_avg_rtt_ms: Option<u32>,
    #[norito(default)]
    path_region: Option<String>,
    #[norito(default)]
    path_asn: Option<u32>,
    #[norito(default)]
    path_validator_lane: bool,
    #[norito(default)]
    path_masque_bypass_allowed: bool,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct GuardEndpointPersistV1 {
    url: String,
    priority: u8,
    #[norito(default)]
    tags: Vec<String>,
}

impl From<&GuardRecord> for GuardRecordPersist {
    fn from(record: &GuardRecord) -> Self {
        Self {
            relay_id: record.relay_id,
            pinned_at_unix: record.pinned_at_unix,
            endpoint: GuardEndpointPersistV1 {
                url: record.endpoint.url.clone(),
                priority: record.endpoint.priority,
                tags: record
                    .endpoint
                    .tags
                    .iter()
                    .map(EndpointTag::as_label)
                    .map(str::to_owned)
                    .collect(),
            },
            guard_weight: record.guard_weight,
            bandwidth_bytes_per_sec: record.bandwidth_bytes_per_sec,
            reputation_weight: record.reputation_weight,
            pq_kem_public_hex: record.pq_kem_public.as_ref().map(hex::encode),
            certificate_base64: record
                .certificate()
                .map(|bundle| BASE64_STANDARD.encode(bundle.to_cbor())),
            path_avg_rtt_ms: record.path_metadata.avg_rtt_ms,
            path_region: record.path_metadata.region.clone(),
            path_asn: record.path_metadata.asn,
            path_validator_lane: record.path_metadata.validator_lane,
            path_masque_bypass_allowed: record.path_metadata.masque_bypass_allowed,
        }
    }
}

impl From<GuardRecordPersist> for GuardRecord {
    fn from(persist: GuardRecordPersist) -> Self {
        let GuardRecordPersist {
            relay_id,
            pinned_at_unix,
            endpoint,
            guard_weight,
            bandwidth_bytes_per_sec,
            reputation_weight,
            pq_kem_public_hex,
            certificate_base64,
            path_avg_rtt_ms,
            path_region,
            path_asn,
            path_validator_lane,
            path_masque_bypass_allowed,
        } = persist;

        let pq_kem_public = pq_kem_public_hex.and_then(|hex| hex::decode(hex).ok());
        let certificate = certificate_base64.and_then(|encoded| {
            BASE64_STANDARD
                .decode(encoded.as_bytes())
                .ok()
                .and_then(|bytes| RelayCertificateBundleV2::from_cbor(&bytes).ok())
        });
        let tags = endpoint
            .tags
            .iter()
            .filter_map(|label| EndpointTag::from_label(label).ok())
            .collect();
        Self {
            relay_id,
            pinned_at_unix,
            endpoint: Endpoint {
                url: endpoint.url,
                priority: endpoint.priority,
                tags,
            },
            guard_weight,
            bandwidth_bytes_per_sec,
            reputation_weight,
            pq_kem_public,
            certificate,
            path_metadata: PathMetadata {
                avg_rtt_ms: path_avg_rtt_ms,
                region: path_region.map(|region| region.trim().to_string()),
                asn: path_asn,
                validator_lane: path_validator_lane,
                masque_bypass_allowed: path_masque_bypass_allowed,
            },
        }
    }
}

/// Stable identifier allocated to each circuit managed by [`CircuitManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CircuitId(u64);

impl CircuitId {
    /// Construct a circuit identifier from a raw number.
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric identifier.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Relay descriptor retained within a circuit path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitRelay {
    /// Relay identifier (descriptor hash).
    pub relay_id: [u8; 32],
    /// Endpoint preferred when building the hop.
    pub endpoint: Endpoint,
    /// Whether the relay advertises PQ-capable transport.
    pub pq_capable: bool,
}

#[derive(Debug, Clone)]
struct Circuit {
    id: CircuitId,
    guard: GuardRecord,
    entry: CircuitRelay,
    middle: CircuitRelay,
    exit: CircuitRelay,
    built_at_unix: u64,
    expires_at_unix: u64,
    latency: LatencyWindow,
    /// Whether the circuit may bypass MASQUE/obfs layers for validator-class meshes.
    masque_bypass: bool,
}

impl Circuit {
    fn info(&self) -> CircuitInfo {
        CircuitInfo {
            id: self.id,
            built_at_unix: self.built_at_unix,
            expires_at_unix: self.expires_at_unix,
            entry_pinned_at_unix: self.guard.pinned_at_unix,
            entry: self.entry.clone(),
            middle: self.middle.clone(),
            exit: self.exit.clone(),
            masque_bypass: self.masque_bypass,
        }
    }
}

/// Snapshot of an active circuit exposed to callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitInfo {
    /// Circuit identifier.
    pub id: CircuitId,
    /// Unix timestamp when the circuit was established.
    pub built_at_unix: u64,
    /// Unix timestamp when the circuit should be renewed.
    pub expires_at_unix: u64,
    /// Unix timestamp when the entry guard was pinned.
    pub entry_pinned_at_unix: u64,
    /// Entry relay metadata.
    pub entry: CircuitRelay,
    /// Middle relay metadata.
    pub middle: CircuitRelay,
    /// Exit relay metadata.
    pub exit: CircuitRelay,
    /// Whether MASQUE/obfs is bypassed for this circuit (validator mesh fast-path).
    pub masque_bypass: bool,
}

/// Reason why a circuit left the active set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitRetirementReason {
    /// Circuit reached its configured TTL.
    Expired,
    /// Guard pin was removed from the guard set.
    EntryRemoved,
    /// Caller requested shutdown.
    ManualTeardown,
    /// Guard metadata changed and a new circuit replaced the previous one.
    MetadataChanged(CircuitRenewalReason),
}

/// Reason why a circuit was renewed while the guard remained selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitRenewalReason {
    /// Guard pin rotated (fresh retention window).
    GuardRotation,
    /// Guard endpoint changed.
    EndpointUpdated,
    /// Guard PQ material changed (e.g., new ML-KEM key).
    PqKeyUpdated,
}

/// Aggregated latency data collected while the circuit was active.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitLatencySnapshot {
    /// Number of latency samples recorded.
    pub samples: u64,
    /// Average latency in milliseconds across the samples.
    pub average_ms: f64,
    /// Minimum latency observed in milliseconds.
    pub min_ms: u64,
    /// Maximum latency observed in milliseconds.
    pub max_ms: u64,
}

/// Historical entry describing a retired circuit.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitRotationRecord {
    /// Circuit metadata captured immediately before retirement.
    pub info: CircuitInfo,
    /// Unix timestamp when the circuit was retired.
    pub retired_at_unix: u64,
    /// Retirement reason.
    pub reason: CircuitRetirementReason,
    /// Latency snapshot gathered while the circuit was active.
    pub latency: Option<CircuitLatencySnapshot>,
}

/// Configuration applied to the circuit lifecycle manager.
#[derive(Debug, Clone)]
pub struct CircuitManagerConfig {
    circuit_ttl: Duration,
    validator_masque_bypass: bool,
}

impl CircuitManagerConfig {
    /// Creates a configuration with the supplied circuit TTL.
    #[must_use]
    pub const fn new(circuit_ttl: Duration) -> Self {
        Self {
            circuit_ttl,
            validator_masque_bypass: false,
        }
    }

    /// Enable or disable MASQUE/obfs bypass for validator-class meshes.
    #[must_use]
    pub const fn with_validator_masque_bypass(mut self, enabled: bool) -> Self {
        self.validator_masque_bypass = enabled;
        self
    }

    /// Returns the configured circuit TTL.
    #[must_use]
    pub const fn circuit_ttl(&self) -> Duration {
        self.circuit_ttl
    }

    /// Returns whether MASQUE/obfs bypass is enabled for validator meshes.
    #[must_use]
    pub const fn validator_masque_bypass(&self) -> bool {
        self.validator_masque_bypass
    }
}

impl Default for CircuitManagerConfig {
    fn default() -> Self {
        Self {
            circuit_ttl: Duration::from_mins(15),
            validator_masque_bypass: false,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct LatencyWindow {
    samples: u64,
    sum_ms: u128,
    min_ms: Option<u64>,
    max_ms: Option<u64>,
}

impl LatencyWindow {
    fn record(&mut self, latency: Duration) {
        let millis = latency.as_millis();
        self.samples = self.samples.saturating_add(1);
        self.sum_ms = self.sum_ms.saturating_add(millis);
        let millis_u64 = millis as u64;
        self.min_ms = Some(match self.min_ms {
            Some(current) => current.min(millis_u64),
            None => millis_u64,
        });
        self.max_ms = Some(match self.max_ms {
            Some(current) => current.max(millis_u64),
            None => millis_u64,
        });
    }

    fn snapshot(&self) -> Option<CircuitLatencySnapshot> {
        if self.samples == 0 {
            return None;
        }
        let average_ms = (self.sum_ms as f64) / (self.samples as f64);
        Some(CircuitLatencySnapshot {
            samples: self.samples,
            average_ms,
            min_ms: self.min_ms.expect("window min must exist"),
            max_ms: self.max_ms.expect("window max must exist"),
        })
    }
}

/// Errors raised by [`CircuitManager::refresh`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CircuitManagerError {
    /// Guard set was empty when attempting to refresh circuits.
    #[error("guard set is empty")]
    EmptyGuardSet,
    /// Guard entry did not contain a usable endpoint.
    #[error("guard {guard_id:?} missing endpoint")]
    MissingGuardEndpoint { guard_id: [u8; 32] },
    /// No viable middle relay was found for the provided guard.
    #[error("no viable middle relay found for guard {guard_id:?}")]
    NoMiddleRelay { guard_id: [u8; 32] },
    /// No viable exit relay was found for the provided guard.
    #[error("no viable exit relay found for guard {guard_id:?}")]
    NoExitRelay { guard_id: [u8; 32] },
}

/// Event emitted while refreshing circuits.
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitEvent {
    /// New circuit became active.
    Built { info: CircuitInfo },
    /// Circuit was retired.
    Retired { record: CircuitRotationRecord },
}

/// Lifecycle manager that keeps SoraNet circuits aligned with the guard set.
#[derive(Debug)]
pub struct CircuitManager {
    config: CircuitManagerConfig,
    circuits: Vec<Circuit>,
    rotation_history: Vec<CircuitRotationRecord>,
    next_circuit_id: u64,
}

impl CircuitManager {
    /// Creates a new circuit manager.
    #[must_use]
    pub fn new(config: CircuitManagerConfig) -> Self {
        Self {
            config,
            circuits: Vec::new(),
            rotation_history: Vec::new(),
            next_circuit_id: 1,
        }
    }

    /// Returns the configuration currently in use.
    #[must_use]
    pub const fn config(&self) -> &CircuitManagerConfig {
        &self.config
    }

    /// Returns the active circuits.
    #[must_use]
    pub fn active_circuits(&self) -> Vec<CircuitInfo> {
        let mut infos: Vec<_> = self.circuits.iter().map(Circuit::info).collect();
        infos.sort_by(|left, right| left.entry.relay_id.cmp(&right.entry.relay_id));
        infos
    }

    /// Returns the recorded rotation history.
    #[must_use]
    pub fn rotation_history(&self) -> &[CircuitRotationRecord] {
        &self.rotation_history
    }

    /// Records a latency sample for the specified circuit.
    ///
    /// Returns `true` when the circuit exists and the sample was recorded.
    pub fn record_latency(&mut self, circuit_id: CircuitId, latency: Duration) -> bool {
        if let Some(circuit) = self.circuits.iter_mut().find(|c| c.id == circuit_id) {
            circuit.latency.record(latency);
            return true;
        }
        false
    }

    /// Refreshes the active circuit set so it matches the guard set.
    ///
    /// Expired circuits are rotated out, new guards trigger fresh circuits, and telemetry snapshots
    /// are captured for retired paths. Returns lifecycle events describing the performed actions.
    pub fn refresh(
        &mut self,
        directory: &RelayDirectory,
        guard_set: &GuardSet,
        now_unix: u64,
        policy: AnonymityPolicy,
    ) -> Result<Vec<CircuitEvent>, CircuitManagerError> {
        if guard_set.is_empty() {
            return Err(CircuitManagerError::EmptyGuardSet);
        }

        let mut events = Vec::new();

        let guard_map: HashMap<[u8; 32], &GuardRecord> = guard_set
            .iter()
            .map(|record| (record.relay_id, record))
            .collect();

        let drained: Vec<_> = self.circuits.drain(..).collect();
        let mut retained = Vec::with_capacity(drained.len());
        for mut circuit in drained {
            let guard_id = circuit.guard.relay_id;

            let Some(updated_guard) = guard_map.get(&guard_id) else {
                let record = self.record_retirement(
                    circuit,
                    now_unix,
                    CircuitRetirementReason::EntryRemoved,
                );
                events.push(CircuitEvent::Retired { record });
                continue;
            };

            if now_unix >= circuit.expires_at_unix {
                let record =
                    self.record_retirement(circuit, now_unix, CircuitRetirementReason::Expired);
                events.push(CircuitEvent::Retired { record });
                continue;
            }

            let endpoint_changed = updated_guard.endpoint != circuit.guard.endpoint;
            let pq_changed =
                updated_guard.pq_kem_public.as_deref() != circuit.guard.pq_kem_public.as_deref();
            let pinned_newer = updated_guard.pinned_at_unix > circuit.guard.pinned_at_unix;

            let renewal_reason = if endpoint_changed {
                Some(CircuitRenewalReason::EndpointUpdated)
            } else if pq_changed {
                Some(CircuitRenewalReason::PqKeyUpdated)
            } else if pinned_newer {
                Some(CircuitRenewalReason::GuardRotation)
            } else {
                None
            };

            if let Some(reason) = renewal_reason {
                let record = self.record_retirement(
                    circuit,
                    now_unix,
                    CircuitRetirementReason::MetadataChanged(reason),
                );
                events.push(CircuitEvent::Retired { record });
                continue;
            }

            circuit.guard = (*updated_guard).clone();
            circuit.entry.endpoint = circuit.guard.endpoint.clone();
            circuit.entry.pq_capable = circuit.guard.pq_kem_public.is_some();
            retained.push(circuit);
        }
        self.circuits = retained;

        for guard in guard_set.iter() {
            if self
                .circuits
                .iter()
                .any(|circuit| circuit.guard.relay_id == guard.relay_id)
            {
                continue;
            }

            let circuit = self.build_circuit(directory, guard, now_unix, policy)?;
            let info = circuit.info();
            events.push(CircuitEvent::Built { info: info.clone() });
            self.circuits.push(circuit);
        }

        self.circuits
            .sort_by(|left, right| left.entry.relay_id.cmp(&right.entry.relay_id));

        Ok(events)
    }

    /// Tears down every active circuit, returning the recorded rotation results.
    pub fn teardown_all(&mut self, now_unix: u64) -> Vec<CircuitRotationRecord> {
        let drained: Vec<_> = self.circuits.drain(..).collect();
        let mut records = Vec::with_capacity(drained.len());
        for circuit in drained {
            let record =
                self.record_retirement(circuit, now_unix, CircuitRetirementReason::ManualTeardown);
            records.push(record);
        }
        records
    }

    fn build_circuit(
        &mut self,
        directory: &RelayDirectory,
        guard: &GuardRecord,
        now_unix: u64,
        policy: AnonymityPolicy,
    ) -> Result<Circuit, CircuitManagerError> {
        let endpoint = guard.endpoint.clone();
        if endpoint.url.is_empty() {
            return Err(CircuitManagerError::MissingGuardEndpoint {
                guard_id: guard.relay_id,
            });
        }

        let mut avoid_asn: HashSet<u32> = HashSet::new();
        if let Some(asn) = guard.path_metadata.asn {
            avoid_asn.insert(asn);
        }

        let middle_descriptor = select_relay(
            directory,
            |descriptor| descriptor.roles.middle && descriptor.relay_id != guard.relay_id,
            now_unix,
            policy,
            &avoid_asn,
        )
        .ok_or(CircuitManagerError::NoMiddleRelay {
            guard_id: guard.relay_id,
        })?;
        if let Some(asn) = middle_descriptor.path_metadata.asn {
            avoid_asn.insert(asn);
        }
        let exit_descriptor = select_relay(
            directory,
            |descriptor| {
                descriptor.roles.exit()
                    && descriptor.relay_id != guard.relay_id
                    && descriptor.relay_id != middle_descriptor.relay_id
            },
            now_unix,
            policy,
            &avoid_asn,
        )
        .ok_or(CircuitManagerError::NoExitRelay {
            guard_id: guard.relay_id,
        })?;

        let entry = CircuitRelay {
            relay_id: guard.relay_id,
            endpoint,
            pq_capable: guard.pq_kem_public.is_some(),
        };
        let middle = descriptor_to_circuit_relay(middle_descriptor, None, now_unix);
        let exit =
            descriptor_to_circuit_relay(exit_descriptor, Some(EndpointTag::NoritoStream), now_unix);
        let masque_bypass = self.config.validator_masque_bypass()
            && guard.path_metadata.validator_lane
            && guard.path_metadata.masque_bypass_allowed
            && middle_descriptor.path_metadata.masque_bypass_allowed
            && exit_descriptor.path_metadata.masque_bypass_allowed;

        let id = CircuitId(self.next_circuit_id);
        self.next_circuit_id = self.next_circuit_id.wrapping_add(1);
        let ttl_secs = self.config.circuit_ttl.as_secs();
        let expires_at_unix = now_unix.saturating_add(ttl_secs);

        let circuit = Circuit {
            id,
            guard: guard.clone(),
            entry,
            middle,
            exit,
            built_at_unix: now_unix,
            expires_at_unix,
            latency: LatencyWindow::default(),
            masque_bypass,
        };
        emit_circuit_build_telemetry(&circuit.guard);
        Ok(circuit)
    }

    fn record_retirement(
        &mut self,
        circuit: Circuit,
        retired_at_unix: u64,
        reason: CircuitRetirementReason,
    ) -> CircuitRotationRecord {
        let info = circuit.info();
        let latency = circuit.latency.snapshot();
        let record = CircuitRotationRecord {
            info,
            retired_at_unix,
            reason,
            latency,
        };
        self.rotation_history.push(record.clone());
        record
    }
}

fn descriptor_to_circuit_relay(
    descriptor: &RelayDescriptor,
    preferred_tag: Option<EndpointTag>,
    now_unix: u64,
) -> CircuitRelay {
    CircuitRelay {
        relay_id: descriptor.relay_id,
        endpoint: descriptor
            .preferred_endpoint(preferred_tag)
            .cloned()
            .expect("descriptor should expose primary endpoint"),
        pq_capable: descriptor.is_pq_capable_at(now_unix),
    }
}

fn select_relay<'a>(
    directory: &'a RelayDirectory,
    predicate: impl Fn(&'a RelayDescriptor) -> bool,
    now_unix: u64,
    policy: AnonymityPolicy,
    avoid_asn: &HashSet<u32>,
) -> Option<&'a RelayDescriptor> {
    let mut candidates: Vec<_> = directory
        .entries()
        .iter()
        .filter(|descriptor| predicate(descriptor))
        .filter(|descriptor| descriptor.primary_endpoint().is_some())
        .collect();

    if matches!(policy, AnonymityPolicy::StrictPq) {
        candidates.retain(|descriptor| descriptor.is_pq_capable_at(now_unix));
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|left, right| {
        let left_avoids_asn = avoids_asn(left, avoid_asn);
        let right_avoids_asn = avoids_asn(right, avoid_asn);

        right
            .path_metadata
            .validator_lane
            .cmp(&left.path_metadata.validator_lane)
            .then_with(|| right_avoids_asn.cmp(&left_avoids_asn))
            .then_with(|| {
                compare_rtt(
                    left.path_metadata.avg_rtt_ms,
                    right.path_metadata.avg_rtt_ms,
                )
            })
            .then_with(|| {
                right
                    .is_pq_capable_at(now_unix)
                    .cmp(&left.is_pq_capable_at(now_unix))
            })
            .then_with(|| right.guard_weight.cmp(&left.guard_weight))
            .then_with(|| left.relay_id.cmp(&right.relay_id))
    });

    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        str::FromStr,
        time::Duration,
    };

    use ed25519_dalek::SigningKey;
    use iroha_crypto::soranet::{
        certificate::{
            CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
            RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
        },
        directory::{GuardDirectoryRelayEntryV2, encode_validation_phase},
        handshake::HandshakeSuite,
    };
    use iroha_data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        soranet::prelude::{RelayBondLedgerEntryV1, RelayBondPolicyV1},
    };
    use iroha_primitives::numeric::Numeric;
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

    use super::*;

    fn build_directory_snapshot(
        validation_phase: CertificateValidationPhase,
        directory_hash: [u8; 32],
    ) -> (GuardDirectorySnapshotV2, RelayCertificateBundleV2) {
        let mut rng = StdRng::seed_from_u64(0xC0FFEE);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let ed_signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = Ed25519VerifyingKey::from(&ed_signing_key).to_bytes();

        let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(&ed_public, mldsa_keys.public_key());

        let certificate = RelayCertificateV2 {
            relay_id: [0x11; 32],
            identity_ed25519: ed_public,
            identity_mldsa65: vec![0x44; 1952],
            descriptor_commit: [0x22; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 180,
            bandwidth_bytes_per_sec: 2_500_000,
            reputation_weight: 90,
            endpoints: vec![RelayEndpointV2 {
                url: "soranet://relay.v2.example:443".to_string(),
                priority: 0,
                tags: vec![EndpointTag::NoritoStream.as_label().to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 2,
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash,
            issuer_fingerprint: fingerprint,
            pq_kem_public: vec![0x55; 1184],
        };

        let published_at = certificate.published_at;
        let valid_after = certificate.valid_after;
        let valid_until = certificate.valid_until;
        let validation_phase_raw = encode_validation_phase(validation_phase);

        let bundle = certificate
            .issue(&ed_signing_key, mldsa_keys.secret_key())
            .expect("certificate issuance");

        let snapshot = GuardDirectorySnapshotV2 {
            version: 2,
            directory_hash,
            published_at_unix: published_at,
            valid_after_unix: valid_after,
            valid_until_unix: valid_until,
            validation_phase: validation_phase_raw,
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public: ed_public,
                mldsa65_public: mldsa_keys.public_key().to_vec(),
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: bundle.to_cbor(),
            }],
        };

        (snapshot, bundle)
    }

    #[test]
    fn guard_directory_decodes_snapshot_v2() {
        let directory_hash = [0xAB; 32];
        let (snapshot, bundle) = build_directory_snapshot(
            CertificateValidationPhase::Phase3RequireDual,
            directory_hash,
        );
        let bytes = to_bytes(&snapshot).expect("encode snapshot");

        let directory =
            RelayDirectory::from_guard_directory_bytes(&bytes).expect("decode guard directory");
        assert_eq!(directory.directory_hash(), Some(directory_hash));
        assert_eq!(
            directory.validation_phase(),
            Some(CertificateValidationPhase::Phase3RequireDual)
        );
        assert_eq!(directory.published_at(), Some(snapshot.published_at_unix));
        assert_eq!(directory.valid_after(), Some(snapshot.valid_after_unix));
        assert_eq!(directory.valid_until(), Some(snapshot.valid_until_unix));

        let descriptor = directory.entries().first().expect("descriptor");
        assert_eq!(descriptor.relay_id, bundle.certificate.relay_id);
        assert_eq!(descriptor.guard_weight, bundle.certificate.guard_weight);
        assert!(descriptor.is_entry_guard());
        assert!(descriptor.is_pq_capable());
        assert_eq!(
            descriptor.certificate_validity(),
            Some((
                bundle.certificate.valid_after,
                bundle.certificate.valid_until
            ))
        );
        assert!(descriptor.certificate().is_some());
    }

    #[test]
    fn guard_directory_detects_hash_mismatch() {
        let (mut snapshot, _) =
            build_directory_snapshot(CertificateValidationPhase::Phase3RequireDual, [0xAA; 32]);
        snapshot.directory_hash = [0xBB; 32];
        let bytes = to_bytes(&snapshot).expect("encode snapshot");

        let err =
            RelayDirectory::from_guard_directory_bytes(&bytes).expect_err("hash mismatch expected");
        assert!(matches!(
            err,
            GuardDirectoryError::CertificateDirectoryHashMismatch { .. }
        ));
    }

    fn relay_id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn entry_descriptor(id_byte: u8, weight: u32, endpoints: Vec<Endpoint>) -> RelayDescriptor {
        RelayDescriptor {
            relay_id: relay_id(id_byte),
            guard_weight: weight,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }
    }

    fn pq_entry_descriptor(id_byte: u8, weight: u32, endpoints: Vec<Endpoint>) -> RelayDescriptor {
        let mut descriptor = entry_descriptor(id_byte, weight, endpoints);
        descriptor.pq_kem_public = Some(vec![0xAB; 4]);
        descriptor
    }

    fn middle_descriptor(id_byte: u8, weight: u32, pq: bool, priority: u8) -> RelayDescriptor {
        let mut descriptor = RelayDescriptor {
            relay_id: relay_id(id_byte),
            guard_weight: weight,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(false, true, false),
            endpoints: vec![Endpoint::new(
                format!("soranet://middle-{id_byte:02x}"),
                priority,
            )],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        if pq {
            descriptor.pq_kem_public = Some(vec![0xBA; 4]);
        }
        descriptor
    }

    fn exit_descriptor(id_byte: u8, weight: u32, pq: bool, priority: u8) -> RelayDescriptor {
        let mut descriptor = RelayDescriptor {
            relay_id: relay_id(id_byte),
            guard_weight: weight,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(false, false, true),
            endpoints: vec![Endpoint::new(
                format!("soranet://exit-{id_byte:02x}"),
                priority,
            )],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        if pq {
            descriptor.pq_kem_public = Some(vec![0xCD; 4]);
        }
        descriptor
    }

    fn asset_definition(name: &str) -> AssetDefinitionId {
        let domain = DomainId::from_str("sora").expect("domain id");
        let asset_name = Name::from_str(name).expect("asset name");
        AssetDefinitionId::new(domain, asset_name)
    }

    fn bond_policy(minimum: &str, asset: &AssetDefinitionId) -> RelayBondPolicyV1 {
        RelayBondPolicyV1 {
            minimum_exit_bond: Numeric::from_str(minimum).expect("numeric"),
            bond_asset_id: asset.clone(),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        }
    }

    fn bond_entry(
        relay_id: [u8; 32],
        amount: &str,
        asset: &AssetDefinitionId,
        exit_capable: bool,
    ) -> RelayBondLedgerEntryV1 {
        RelayBondLedgerEntryV1 {
            relay_id,
            bonded_amount: Numeric::from_str(amount).expect("numeric"),
            bond_asset_id: asset.clone(),
            bonded_since_unix: 123,
            exit_capable,
        }
    }

    fn guard_record(id_byte: u8, pinned_at_unix: u64, endpoint: &str) -> GuardRecord {
        GuardRecord {
            relay_id: relay_id(id_byte),
            pinned_at_unix,
            endpoint: Endpoint::new(endpoint.to_string(), 0),
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }
    }

    fn guard_record_with_pq(
        id_byte: u8,
        pinned_at_unix: u64,
        endpoint: &str,
        pq_bytes: Option<Vec<u8>>,
    ) -> GuardRecord {
        GuardRecord {
            relay_id: relay_id(id_byte),
            pinned_at_unix,
            endpoint: Endpoint::new(endpoint.to_string(), 0),
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: pq_bytes,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }
    }

    #[test]
    fn guard_cache_key_roundtrip() {
        let key = GuardCacheKey::from_hex(
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        )
        .expect("parse key");
        assert_eq!(
            key.to_hex(),
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF"
        );
    }

    #[test]
    fn guard_set_roundtrip_with_key() {
        let key = GuardCacheKey::from_hex(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        )
        .expect("parse key");
        let mut guard = guard_record(0x01, 42, "soranet://relay-1");
        guard.bandwidth_bytes_per_sec = 4 * 1024 * 1024;
        guard.reputation_weight = 77;
        let set = GuardSet::new(vec![guard]);
        let encoded = set.encode_with_key(Some(&key)).expect("encode guard set");
        let decoded = GuardSet::decode_with_key(&encoded, Some(&key)).expect("decode guard set");
        assert_eq!(decoded.guards().len(), 1);
        assert_eq!(decoded.guards()[0].relay_id, relay_id(0x01));
        assert_eq!(decoded.guards()[0].bandwidth_bytes_per_sec, 4 * 1024 * 1024);
        assert_eq!(decoded.guards()[0].reputation_weight, 77);
    }

    #[test]
    fn guard_selector_filters_expired_certificates() {
        let (_, mut bundle) =
            build_directory_snapshot(CertificateValidationPhase::Phase3RequireDual, [0xDD; 32]);
        bundle.certificate.valid_after = 0;
        bundle.certificate.valid_until = 40;
        let directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: bundle.certificate.relay_id,
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            roles: RelayRoles::new(true, false, false),
            endpoints: vec![Endpoint::new("soranet://expired-cert", 0)],
            pq_kem_public: None,
            certificate: Some(bundle),
            path_metadata: PathMetadata::default(),
        }]);
        let selector = GuardSelector::new(NonZeroUsize::new(1).expect("non-zero"));
        let guards = selector.select(&directory, None, 100, AnonymityPolicy::GuardPq);
        assert_eq!(guards.guards().len(), 1);
        let guard = &guards.guards()[0];
        assert!(guard.certificate().is_none());
        assert!(guard.pq_kem_public.is_none());
    }

    #[test]
    fn guard_selector_ignores_stale_record_pq_keys() {
        let (_, mut bundle) =
            build_directory_snapshot(CertificateValidationPhase::Phase3RequireDual, [0xAA; 32]);
        bundle.certificate.valid_after = 0;
        bundle.certificate.valid_until = 40;
        let relay_id = bundle.certificate.relay_id;
        let directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id,
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            roles: RelayRoles::new(true, false, false),
            endpoints: vec![Endpoint::new("soranet://expired-cert", 0)],
            pq_kem_public: None,
            certificate: Some(bundle),
            path_metadata: PathMetadata::default(),
        }]);
        let existing = GuardSet::new(vec![GuardRecord {
            relay_id,
            pinned_at_unix: 10,
            endpoint: Endpoint::new("soranet://expired-cert", 0),
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAB; 4]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let selector = GuardSelector::new(NonZeroUsize::new(1).expect("non-zero"));
        let guards = selector.select(&directory, Some(&existing), 100, AnonymityPolicy::GuardPq);
        assert_eq!(guards.guards().len(), 1);
        let guard = &guards.guards()[0];
        assert!(guard.certificate().is_none());
        assert!(guard.pq_kem_public.is_none());
    }

    #[test]
    fn guard_set_persists_certificate_bundle() {
        let (_, bundle) =
            build_directory_snapshot(CertificateValidationPhase::Phase3RequireDual, [0xCC; 32]);
        let guard = GuardRecord {
            relay_id: bundle.certificate.relay_id,
            pinned_at_unix: 321,
            endpoint: Endpoint::new("soranet://bundle-guard", 0),
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            pq_kem_public: Some(bundle.certificate.pq_kem_public.clone()),
            certificate: Some(bundle.clone()),
            path_metadata: PathMetadata::default(),
        };
        let set = GuardSet::new(vec![guard]);
        let encoded = set.encode().expect("encode guard set");
        let decoded = GuardSet::decode(&encoded).expect("decode guard set");
        let decoded_guard = decoded.guards().first().expect("guard");
        assert!(decoded_guard.certificate().is_some());
        assert_eq!(decoded_guard.certificate().expect("bundle"), &bundle);
        assert!(
            decoded_guard
                .pq_kem_public
                .as_ref()
                .is_some_and(|key| key == &bundle.certificate.pq_kem_public)
        );
    }

    #[test]
    fn guard_selector_prefers_validator_lane_with_path_hints() {
        let mut directory = RelayDirectory::new(vec![
            entry_descriptor(0x01, 100, vec![Endpoint::new("soranet://relay-1", 0)]),
            entry_descriptor(0x02, 120, vec![Endpoint::new("soranet://relay-2", 0)]),
        ]);
        let hints = vec![
            RelayPathHint {
                relay_id: relay_id(0x01),
                avg_rtt_ms: Some(20),
                region: Some("us-west".to_string()),
                asn: Some(64512),
                validator_lane: true,
                masque_bypass_allowed: true,
            },
            RelayPathHint {
                relay_id: relay_id(0x02),
                avg_rtt_ms: Some(5),
                region: Some("us-east".to_string()),
                asn: Some(64513),
                validator_lane: false,
                masque_bypass_allowed: false,
            },
        ];
        let report = directory.apply_path_hints(&hints);
        assert_eq!(report.applied, 2);
        assert!(report.missing.is_empty());

        let selector = GuardSelector::new(NonZeroUsize::new(1).expect("non-zero"));
        let guards = selector.select(&directory, None, 0, AnonymityPolicy::GuardPq);
        let guard = guards.guards().first().expect("selected guard");
        assert_eq!(guard.relay_id, relay_id(0x01));
        assert_eq!(guard.path_metadata.avg_rtt_ms, Some(20));
        assert!(guard.path_metadata.validator_lane);
        assert!(guard.path_metadata.masque_bypass_allowed);
    }

    #[test]
    fn guard_set_tag_verification_fails_without_key() {
        let key = GuardCacheKey::from_hex(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        )
        .expect("parse key");
        let set = GuardSet::new(vec![guard_record(0x02, 10, "soranet://relay-2")]);
        let encoded = set.encode_with_key(Some(&key)).expect("encode guard set");
        let err = GuardSet::decode(&encoded).expect_err("decode should fail");
        match err {
            GuardSetPersistenceError::MissingCacheKey => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn guard_set_tag_verification_detects_tampering() {
        let key = GuardCacheKey::from_hex(
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        )
        .expect("parse key");
        let set = GuardSet::new(vec![guard_record(0x03, 5, "soranet://relay-3")]);
        let mut encoded = set.encode_with_key(Some(&key)).expect("encode guard set");
        // Flip one byte in the payload to trigger verification failure.
        if let Some(last_byte) = encoded.last_mut() {
            *last_byte ^= 0xFF;
        } else {
            panic!("guard set encoding produced empty payload");
        }
        let err =
            GuardSet::decode_with_key(&encoded, Some(&key)).expect_err("verification should fail");
        match err {
            GuardSetPersistenceError::InvalidCacheTag | GuardSetPersistenceError::Decode(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn exit_bond_policy_demotes_missing_bond() {
        let asset = asset_definition("xor");
        let policy = bond_policy("100", &asset);
        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: relay_id(0xAA),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);
        let ledger: BTreeMap<RelayId, RelayBondLedgerEntryV1> = BTreeMap::new();

        let demotions = directory.enforce_exit_bond_policy(&policy, ledger.iter());

        assert_eq!(demotions.len(), 1);
        assert_eq!(demotions[0].relay_id, relay_id(0xAA));
        assert_eq!(demotions[0].reason, ExitDemotionReason::MissingBond);
        assert!(!directory.entries()[0].roles.exit());
    }

    #[test]
    fn exit_bond_policy_demotes_disabled_in_ledger() {
        let asset = asset_definition("xor");
        let policy = bond_policy("100", &asset);
        let entry = bond_entry(relay_id(0xBB), "150", &asset, false);
        let mut ledger = BTreeMap::new();
        ledger.insert(entry.relay_id, entry);

        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: relay_id(0xBB),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let demotions = directory.enforce_exit_bond_policy(&policy, ledger.iter());

        assert_eq!(demotions.len(), 1);
        assert_eq!(
            demotions[0].reason,
            ExitDemotionReason::ExitDisabledInLedger
        );
        assert!(!directory.entries()[0].roles.exit());
    }

    #[test]
    fn exit_bond_policy_demotes_below_minimum() {
        let asset = asset_definition("xor");
        let policy = bond_policy("100", &asset);
        let entry = bond_entry(relay_id(0xCC), "80", &asset, true);
        let mut ledger = BTreeMap::new();
        ledger.insert(entry.relay_id, entry);

        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: relay_id(0xCC),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let demotions = directory.enforce_exit_bond_policy(&policy, ledger.iter());

        assert_eq!(demotions.len(), 1);
        assert_eq!(demotions[0].reason, ExitDemotionReason::BelowMinimum);
        assert!(!directory.entries()[0].roles.exit());
    }

    #[test]
    fn exit_bond_policy_demotes_asset_mismatch() {
        let asset = asset_definition("xor");
        let other = asset_definition("usd");
        let policy = bond_policy("100", &asset);
        let entry = bond_entry(relay_id(0xDD), "200", &other, true);
        let mut ledger = BTreeMap::new();
        ledger.insert(entry.relay_id, entry);

        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: relay_id(0xDD),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let demotions = directory.enforce_exit_bond_policy(&policy, ledger.iter());

        assert_eq!(demotions.len(), 1);
        assert_eq!(demotions[0].reason, ExitDemotionReason::BondAssetMismatch);
        assert!(!directory.entries()[0].roles.exit());
    }

    #[test]
    fn exit_bond_policy_retains_compliant_exit() {
        let asset = asset_definition("xor");
        let policy = bond_policy("100", &asset);
        let entry = bond_entry(relay_id(0xEE), "250", &asset, true);
        let mut ledger = BTreeMap::new();
        ledger.insert(entry.relay_id, entry);

        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id: relay_id(0xEE),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let demotions = directory.enforce_exit_bond_policy(&policy, ledger.iter());

        assert!(demotions.is_empty());
        assert!(directory.entries()[0].roles.exit());
    }

    fn sample_directory() -> RelayDirectory {
        RelayDirectory::new(vec![
            entry_descriptor(0x01, 120, vec![Endpoint::new("soranet://relay-1", 0)]),
            entry_descriptor(0x02, 110, vec![Endpoint::new("soranet://relay-2", 0)]),
            entry_descriptor(0x03, 100, vec![Endpoint::new("soranet://relay-3", 0)]),
            entry_descriptor(0x04, 90, vec![Endpoint::new("soranet://relay-4", 0)]),
        ])
    }

    #[test]
    fn circuit_manager_builds_initial_circuits() {
        let directory = sample_directory();
        let guard_set = GuardSet::new(vec![guard_record(0x01, 0, "soranet://relay-1")]);
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let events = manager
            .refresh(&directory, &guard_set, 0, AnonymityPolicy::GuardPq)
            .expect("refresh");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], CircuitEvent::Built { .. }));
        assert_eq!(manager.active_circuits().len(), 1);
    }

    #[test]
    fn circuit_manager_sets_masque_bypass_for_validator_mesh() {
        let mut entry = entry_descriptor(0x01, 120, vec![Endpoint::new("soranet://entry", 0)]);
        entry.path_metadata.validator_lane = true;
        entry.path_metadata.masque_bypass_allowed = true;

        let mut middle = middle_descriptor(0x02, 80, false, 0);
        middle.path_metadata.masque_bypass_allowed = true;

        let exit = RelayDescriptor {
            relay_id: relay_id(0x03),
            guard_weight: 80,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(false, false, true),
            endpoints: vec![Endpoint::new("soranet://exit", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata {
                masque_bypass_allowed: true,
                ..PathMetadata::default()
            },
        };

        let directory = RelayDirectory::new(vec![entry, middle, exit]);
        let mut guard = guard_record(0x01, 0, "soranet://entry");
        guard.path_metadata.validator_lane = true;
        guard.path_metadata.masque_bypass_allowed = true;
        let guard_set = GuardSet::new(vec![guard]);
        let mut manager =
            CircuitManager::new(CircuitManagerConfig::default().with_validator_masque_bypass(true));

        let events = manager
            .refresh(&directory, &guard_set, 0, AnonymityPolicy::GuardPq)
            .expect("refresh");
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CircuitEvent::Built { .. })),
            "expected a circuit build event"
        );

        let circuits = manager.active_circuits();
        let circuit = circuits.first().expect("circuit");
        assert!(circuit.masque_bypass, "validator mesh should bypass MASQUE");
    }

    #[test]
    fn circuit_manager_rotates_when_guard_pinned_updates() {
        let directory = sample_directory();
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let initial_guard = guard_record(0x01, 10, "soranet://relay-1");
        manager
            .refresh(
                &directory,
                &GuardSet::new(vec![initial_guard]),
                10,
                AnonymityPolicy::GuardPq,
            )
            .expect("initial refresh");

        for circuit in manager.active_circuits() {
            assert!(manager.record_latency(circuit.id, Duration::from_millis(50)));
        }

        let rotated_guard = guard_record(0x01, 200, "soranet://relay-1");
        let events = manager
            .refresh(
                &directory,
                &GuardSet::new(vec![rotated_guard]),
                200,
                AnonymityPolicy::GuardPq,
            )
            .expect("rotation refresh");

        assert!(events.iter().any(|event| match event {
            CircuitEvent::Retired { record } => matches!(
                record.reason,
                CircuitRetirementReason::MetadataChanged(CircuitRenewalReason::GuardRotation)
            ),
            _ => false,
        }));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, CircuitEvent::Built { .. }))
        );
    }

    #[test]
    fn circuit_manager_rotates_when_endpoint_changes() {
        let directory = sample_directory();
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let initial_guard = guard_record(0x01, 0, "soranet://relay-1");
        manager
            .refresh(
                &directory,
                &GuardSet::new(vec![initial_guard]),
                0,
                AnonymityPolicy::GuardPq,
            )
            .expect("initial refresh");

        let updated_guard = guard_record(0x01, 0, "soranet://relay-1-alt");
        let events = manager
            .refresh(
                &directory,
                &GuardSet::new(vec![updated_guard]),
                10,
                AnonymityPolicy::GuardPq,
            )
            .expect("refresh with endpoint change");

        assert!(events.iter().any(|event| match event {
            CircuitEvent::Retired { record } => matches!(
                record.reason,
                CircuitRetirementReason::MetadataChanged(CircuitRenewalReason::EndpointUpdated)
            ),
            _ => false,
        }));
    }

    #[test]
    fn circuit_manager_rotates_when_pq_material_changes() {
        let directory = sample_directory();
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let initial_guard = guard_record_with_pq(0x01, 0, "soranet://relay-1", Some(vec![0xAA; 4]));
        manager
            .refresh(
                &directory,
                &GuardSet::new(vec![initial_guard]),
                0,
                AnonymityPolicy::GuardPq,
            )
            .expect("initial refresh");

        let updated_guard = guard_record_with_pq(0x01, 0, "soranet://relay-1", Some(vec![0xBB; 4]));
        let events = manager
            .refresh(
                &directory,
                &GuardSet::new(vec![updated_guard]),
                20,
                AnonymityPolicy::GuardPq,
            )
            .expect("refresh with pq change");

        assert!(events.iter().any(|event| match event {
            CircuitEvent::Retired { record } => matches!(
                record.reason,
                CircuitRetirementReason::MetadataChanged(CircuitRenewalReason::PqKeyUpdated)
            ),
            _ => false,
        }));
    }

    #[test]
    fn circuit_manager_retires_removed_guards() {
        let directory = sample_directory();
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let guard_a = guard_record(0x01, 0, "soranet://relay-1");
        let guard_b = guard_record(0x02, 0, "soranet://relay-2");
        let guard_b_id = guard_b.relay_id;

        manager
            .refresh(
                &directory,
                &GuardSet::new(vec![guard_a.clone(), guard_b]),
                0,
                AnonymityPolicy::GuardPq,
            )
            .expect("initial refresh");

        let events = manager
            .refresh(
                &directory,
                &GuardSet::new(vec![guard_a]),
                60,
                AnonymityPolicy::GuardPq,
            )
            .expect("refresh removing guard");

        assert!(events.iter().any(|event| match event {
            CircuitEvent::Retired { record } => {
                record.info.entry.relay_id == guard_b_id
                    && record.reason == CircuitRetirementReason::EntryRemoved
            }
            _ => false,
        }));
    }

    #[test]
    fn circuit_manager_latency_soak_remains_stable_across_rotations() {
        let directory = sample_directory();
        let mut manager = CircuitManager::new(CircuitManagerConfig::default());

        let rotations = [
            GuardSet::new(vec![
                guard_record(0x01, 0, "soranet://relay-1"),
                guard_record(0x02, 0, "soranet://relay-2"),
            ]),
            GuardSet::new(vec![
                guard_record(0x01, 100, "soranet://relay-1"),
                guard_record(0x02, 100, "soranet://relay-2"),
            ]),
            GuardSet::new(vec![
                guard_record(0x01, 200, "soranet://relay-1"),
                guard_record(0x02, 200, "soranet://relay-2"),
            ]),
            GuardSet::new(vec![
                guard_record(0x01, 300, "soranet://relay-1"),
                guard_record(0x02, 300, "soranet://relay-2"),
            ]),
        ];

        let mut averages = Vec::new();
        let latency_samples = [48u64, 49, 47, 48];

        for (idx, guard_set) in rotations.iter().enumerate() {
            let events = manager
                .refresh(
                    &directory,
                    guard_set,
                    (idx as u64) * 100,
                    AnonymityPolicy::GuardPq,
                )
                .expect("refresh succeeds");

            if idx > 0 {
                let mut rotation_latencies = Vec::new();
                for event in &events {
                    if let CircuitEvent::Retired { record } = event
                        && let Some(snapshot) = &record.latency
                    {
                        rotation_latencies.push(snapshot.average_ms);
                    }
                }
                assert!(!rotation_latencies.is_empty());
                let avg = rotation_latencies.iter().sum::<f64>() / rotation_latencies.len() as f64;
                averages.push(avg);
            }

            for circuit in manager.active_circuits() {
                assert!(
                    manager.record_latency(circuit.id, Duration::from_millis(latency_samples[idx]))
                );
            }
        }

        assert!(
            averages.len() >= 3,
            "expected at least three rotation averages, got {averages:?}"
        );

        let min_avg = averages
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap_or_default();
        let max_avg = averages
            .iter()
            .copied()
            .reduce(f64::max)
            .unwrap_or_default();
        let spread = max_avg - min_avg;
        assert!(
            spread <= 2.0,
            "latency spread {spread} across rotations exceeds threshold; averages={averages:?}"
        );
    }

    #[test]
    fn selector_prefers_highest_weight_entry_guards() {
        let mut directory = RelayDirectory::default();
        directory.push(entry_descriptor(
            0x01,
            100,
            vec![Endpoint::new("soranet://relay-1", 1)],
        ));
        directory.push(entry_descriptor(
            0x02,
            300,
            vec![Endpoint::new("soranet://relay-2", 0)],
        ));
        directory.push(entry_descriptor(
            0x03,
            200,
            vec![Endpoint::new("soranet://relay-3", 0)],
        ));

        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guards = selector.select(&directory, None, 5, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, relay_id(0x02));
        assert_eq!(guards.guards()[1].relay_id, relay_id(0x03));
        assert_eq!(guards.guards()[0].endpoint.url, "soranet://relay-2");
        assert_eq!(guards.guards()[1].endpoint.url, "soranet://relay-3");
        assert_eq!(guards.guards()[0].pinned_at_unix, 5);
        assert_eq!(guards.guards()[1].pinned_at_unix, 5);
    }

    #[test]
    fn selector_prefers_higher_bandwidth_on_weight_tie() {
        let mut high_bw =
            pq_entry_descriptor(0x10, 150, vec![Endpoint::new("soranet://high-bw", 0)]);
        high_bw.bandwidth_bytes_per_sec = 9 * 1024 * 1024;
        high_bw.reputation_weight = 50;
        let mut low_bw = pq_entry_descriptor(0x11, 150, vec![Endpoint::new("soranet://low-bw", 0)]);
        low_bw.bandwidth_bytes_per_sec = 5 * 1024 * 1024;
        low_bw.reputation_weight = 90;

        let directory = RelayDirectory::new(vec![high_bw.clone(), low_bw.clone()]);
        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guards = selector.select(&directory, None, 5, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, high_bw.relay_id);
        assert_eq!(guards.guards()[1].relay_id, low_bw.relay_id);
        assert_eq!(guards.guards()[0].bandwidth_bytes_per_sec, 9 * 1024 * 1024);
        assert_eq!(guards.guards()[1].bandwidth_bytes_per_sec, 5 * 1024 * 1024);
    }

    #[test]
    fn selector_prefers_higher_reputation_after_bandwidth() {
        let mut first = pq_entry_descriptor(0x20, 180, vec![Endpoint::new("soranet://first", 0)]);
        first.bandwidth_bytes_per_sec = 7 * 1024 * 1024;
        first.reputation_weight = 60;
        let mut second = pq_entry_descriptor(0x21, 180, vec![Endpoint::new("soranet://second", 0)]);
        second.bandwidth_bytes_per_sec = first.bandwidth_bytes_per_sec;
        second.reputation_weight = 90;

        let directory = RelayDirectory::new(vec![first.clone(), second.clone()]);
        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guards = selector.select(&directory, None, 42, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, second.relay_id);
        assert_eq!(guards.guards()[1].relay_id, first.relay_id);
        assert_eq!(guards.guards()[0].reputation_weight, 90);
        assert_eq!(guards.guards()[1].reputation_weight, 60);
    }

    #[test]
    fn selector_prefers_pq_guard_over_heavier_classical_in_compatible_policy() {
        let mut pq_guard =
            pq_entry_descriptor(0x40, 150, vec![Endpoint::new("soranet://pq-guard", 0)]);
        pq_guard.bandwidth_bytes_per_sec = 6 * 1024 * 1024;
        pq_guard.reputation_weight = 110;

        let mut classical_guard = entry_descriptor(
            0x41,
            220,
            vec![Endpoint::new("soranet://classical-guard", 0)],
        );
        classical_guard.bandwidth_bytes_per_sec = 10 * 1024 * 1024;
        classical_guard.reputation_weight = 190;

        let directory = RelayDirectory::new(vec![classical_guard.clone(), pq_guard.clone()]);
        let selector = GuardSelector::new(NonZeroUsize::new(1).expect("non-zero"));
        let guards = selector.select(&directory, None, 5, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 1);
        assert_eq!(guards.guards()[0].relay_id, pq_guard.relay_id);
        assert!(guards.guards()[0].pq_kem_public.is_some());
    }

    #[test]
    fn selector_is_deterministic_for_equivalent_descriptors() {
        let directory = RelayDirectory::new(vec![
            entry_descriptor(0x30, 120, vec![Endpoint::new("soranet://relay-30", 0)]),
            entry_descriptor(0x10, 120, vec![Endpoint::new("soranet://relay-10", 0)]),
            entry_descriptor(0x20, 120, vec![Endpoint::new("soranet://relay-20", 0)]),
        ]);

        let selector = GuardSelector::new(NonZeroUsize::new(3).expect("non-zero"));
        let guards = selector.select(&directory, None, 7, AnonymityPolicy::GuardPq);
        let ordered: Vec<[u8; 32]> = guards.iter().map(|record| record.relay_id).collect();

        assert_eq!(
            ordered,
            vec![relay_id(0x10), relay_id(0x20), relay_id(0x30)]
        );
    }

    #[test]
    fn selector_retains_pinned_guards_within_retention() {
        let directory = RelayDirectory::new(vec![
            entry_descriptor(0x01, 10, vec![Endpoint::new("soranet://guard-1", 0)]),
            entry_descriptor(0x02, 50, vec![Endpoint::new("soranet://guard-2", 0)]),
        ]);

        let existing = GuardSet::new(vec![GuardRecord {
            relay_id: relay_id(0x01),
            pinned_at_unix: 10,
            endpoint: Endpoint::new("soranet://guard-1-old", 5),
            guard_weight: 10,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let retention = GuardRetention::new(NonZeroU64::new(100).expect("non-zero"));
        let selector =
            GuardSelector::new(NonZeroUsize::new(2).expect("non-zero")).with_retention(retention);
        let guards = selector.select(&directory, Some(&existing), 50, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, relay_id(0x01));
        assert_eq!(guards.guards()[0].pinned_at_unix, 10);
        assert_eq!(guards.guards()[0].endpoint.url, "soranet://guard-1");
        assert_eq!(guards.guards()[1].relay_id, relay_id(0x02));
        assert_eq!(guards.guards()[1].pinned_at_unix, 50);
    }

    #[test]
    fn selector_drops_expired_or_missing_guards() {
        let directory = RelayDirectory::new(vec![
            entry_descriptor(0x10, 70, vec![Endpoint::new("soranet://fresh-1", 0)]),
            entry_descriptor(0x11, 60, vec![Endpoint::new("soranet://fresh-2", 0)]),
        ]);

        let existing = GuardSet::new(vec![
            GuardRecord {
                relay_id: relay_id(0x01),
                pinned_at_unix: 0,
                endpoint: Endpoint::new("soranet://stale", 0),
                guard_weight: 5,
                bandwidth_bytes_per_sec: 0,
                reputation_weight: 0,
                pq_kem_public: None,
                certificate: None,
                path_metadata: PathMetadata::default(),
            },
            GuardRecord {
                relay_id: relay_id(0x02),
                pinned_at_unix: 5,
                endpoint: Endpoint::new("soranet://missing", 0),
                guard_weight: 5,
                bandwidth_bytes_per_sec: 0,
                reputation_weight: 0,
                pq_kem_public: None,
                certificate: None,
                path_metadata: PathMetadata::default(),
            },
        ]);

        let retention = GuardRetention::new(NonZeroU64::new(10).expect("non-zero"));
        let selector =
            GuardSelector::new(NonZeroUsize::new(2).expect("non-zero")).with_retention(retention);
        let guards = selector.select(&directory, Some(&existing), 20, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, relay_id(0x10));
        assert_eq!(guards.guards()[1].relay_id, relay_id(0x11));
        assert_eq!(guards.guards()[0].pinned_at_unix, 20);
        assert_eq!(guards.guards()[1].pinned_at_unix, 20);
    }

    #[test]
    fn selector_prioritises_pq_guards_when_policy_requires() {
        let directory = RelayDirectory::new(vec![
            pq_entry_descriptor(0x01, 10, vec![Endpoint::new("soranet://pq-1", 0)]),
            entry_descriptor(0x02, 20, vec![Endpoint::new("soranet://classical-1", 0)]),
            pq_entry_descriptor(0x03, 5, vec![Endpoint::new("soranet://pq-2", 0)]),
        ]);

        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guards = selector.select(&directory, None, 100, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert!(guards.guards()[0].pq_kem_public.is_some());
        assert!(guards.guards()[1].pq_kem_public.is_some());
    }

    #[test]
    fn selector_replaces_classical_guard_when_pq_available() {
        let directory = RelayDirectory::new(vec![
            pq_entry_descriptor(0x01, 50, vec![Endpoint::new("soranet://pq-guard", 0)]),
            entry_descriptor(
                0x02,
                100,
                vec![Endpoint::new("soranet://classical-guard", 0)],
            ),
        ]);

        let existing = GuardSet::new(vec![GuardRecord {
            relay_id: relay_id(0x02),
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://classical-old", 0),
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let selector = GuardSelector::new(NonZeroUsize::new(1).expect("non-zero"));
        let guards = selector.select(&directory, Some(&existing), 10, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 1);
        assert_eq!(guards.guards()[0].relay_id, relay_id(0x01));
        assert!(guards.guards()[0].pq_kem_public.is_some());
    }

    #[test]
    fn selector_ignores_classical_when_pq_supply_sufficient() {
        let directory = RelayDirectory::new(vec![
            pq_entry_descriptor(0x01, 80, vec![Endpoint::new("soranet://pq-1", 0)]),
            pq_entry_descriptor(0x02, 70, vec![Endpoint::new("soranet://pq-2", 0)]),
            pq_entry_descriptor(0x03, 60, vec![Endpoint::new("soranet://pq-3", 0)]),
            entry_descriptor(
                0x04,
                90,
                vec![Endpoint::new("soranet://classical-guard", 0)],
            ),
        ]);

        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guard_set = selector.select(&directory, None, 5, AnonymityPolicy::GuardPq);
        assert_eq!(guard_set.len(), 2);
        assert!(
            guard_set
                .guards()
                .iter()
                .all(|record| record.pq_kem_public.is_some())
        );

        let guard_set_majority = selector.select(&directory, None, 5, AnonymityPolicy::MajorityPq);
        assert_eq!(guard_set_majority.len(), 2);
        assert!(
            guard_set_majority
                .guards()
                .iter()
                .all(|record| record.pq_kem_public.is_some())
        );
    }

    #[test]
    fn guard_set_persistence_roundtrip() {
        let guards = GuardSet::new(vec![
            GuardRecord {
                relay_id: relay_id(0xAA),
                pinned_at_unix: 42,
                endpoint: Endpoint::new("soranet://persist-1", 0),
                guard_weight: 90,
                bandwidth_bytes_per_sec: 0,
                reputation_weight: 0,
                pq_kem_public: Some(vec![0xAA, 0xBB, 0xCC]),
                certificate: None,
                path_metadata: PathMetadata::default(),
            },
            GuardRecord {
                relay_id: relay_id(0xBB),
                pinned_at_unix: 84,
                endpoint: Endpoint::new("soranet://persist-2", 2),
                guard_weight: 80,
                bandwidth_bytes_per_sec: 0,
                reputation_weight: 0,
                pq_kem_public: None,
                certificate: None,
                path_metadata: PathMetadata::default(),
            },
        ]);

        let bytes = guards.encode().expect("encode guard set");
        let decoded = GuardSet::decode(&bytes).expect("decode guard set");

        assert_eq!(decoded, guards);
    }

    #[test]
    fn circuit_manager_builds_circuits_per_guard() {
        let guard_a = GuardRecord {
            relay_id: relay_id(0x01),
            pinned_at_unix: 100,
            endpoint: Endpoint::new("soranet://guard-a", 0),
            guard_weight: 120,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_b = GuardRecord {
            relay_id: relay_id(0x02),
            pinned_at_unix: 200,
            endpoint: Endpoint::new("soranet://guard-b", 1),
            guard_weight: 110,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_a.clone(), guard_b.clone()]);

        let directory = RelayDirectory::new(vec![
            middle_descriptor(0x10, 50, true, 0),
            exit_descriptor(0x20, 60, true, 0),
            exit_descriptor(0x21, 55, true, 1),
        ]);

        let mut manager = CircuitManager::new(CircuitManagerConfig::new(Duration::from_secs(60)));
        let events = manager
            .refresh(&directory, &guard_set, 1_000, AnonymityPolicy::GuardPq)
            .expect("build circuits");
        assert_eq!(events.len(), 2);

        let circuits = manager.active_circuits();
        assert_eq!(circuits.len(), 2);
        let entry_ids: BTreeSet<[u8; 32]> =
            circuits.iter().map(|info| info.entry.relay_id).collect();
        assert!(entry_ids.contains(&guard_a.relay_id));
        assert!(entry_ids.contains(&guard_b.relay_id));
    }

    #[test]
    fn circuit_manager_rotates_on_ttl() {
        let guard = GuardRecord {
            relay_id: relay_id(0x03),
            pinned_at_unix: 50,
            endpoint: Endpoint::new("soranet://guard-rot", 0),
            guard_weight: 115,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard.clone()]);

        let directory = RelayDirectory::new(vec![
            middle_descriptor(0x12, 45, true, 0),
            exit_descriptor(0x22, 70, true, 0),
        ]);

        let ttl = Duration::from_secs(5);
        let mut manager = CircuitManager::new(CircuitManagerConfig::new(ttl));
        let first_events = manager
            .refresh(&directory, &guard_set, 10, AnonymityPolicy::GuardPq)
            .expect("initial build");
        assert_eq!(first_events.len(), 1);

        let initial_circuit = manager
            .active_circuits()
            .into_iter()
            .next()
            .expect("active circuit");
        assert_eq!(initial_circuit.entry.relay_id, guard.relay_id);

        assert!(manager.record_latency(initial_circuit.id, Duration::from_millis(48)));
        assert!(manager.record_latency(initial_circuit.id, Duration::from_millis(52)));

        let later = 10 + ttl.as_secs() + 1;
        let events = manager
            .refresh(&directory, &guard_set, later, AnonymityPolicy::GuardPq)
            .expect("rotate circuit");
        let retired = events
            .iter()
            .filter(|event| matches!(event, CircuitEvent::Retired { .. }))
            .count();
        let built = events
            .iter()
            .filter(|event| matches!(event, CircuitEvent::Built { .. }))
            .count();
        assert_eq!(retired, 1);
        assert_eq!(built, 1);

        let history = manager.rotation_history();
        assert_eq!(history.len(), 1);
        let record = &history[0];
        assert_eq!(record.reason, CircuitRetirementReason::Expired);
        let latency = record
            .latency
            .as_ref()
            .expect("latency snapshot present after samples");
        assert_eq!(latency.samples, 2);
        assert!(latency.average_ms >= 48.0 && latency.average_ms <= 52.0);
        assert_eq!(latency.min_ms, 48);
        assert_eq!(latency.max_ms, 52);
    }

    #[test]
    fn circuit_manager_teardown_all_records_manual_shutdown() {
        let guard_a = GuardRecord {
            relay_id: relay_id(0x05),
            pinned_at_unix: 120,
            endpoint: Endpoint::new("soranet://guard-a", 0),
            guard_weight: 105,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_b = GuardRecord {
            relay_id: relay_id(0x06),
            pinned_at_unix: 180,
            endpoint: Endpoint::new("soranet://guard-b", 0),
            guard_weight: 95,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_a.clone(), guard_b.clone()]);

        let directory = RelayDirectory::new(vec![
            middle_descriptor(0x30, 70, true, 0),
            exit_descriptor(0x40, 80, true, 0),
            exit_descriptor(0x41, 75, true, 1),
        ]);

        let mut manager = CircuitManager::new(CircuitManagerConfig::new(Duration::from_secs(120)));
        manager
            .refresh(&directory, &guard_set, 1_000, AnonymityPolicy::GuardPq)
            .expect("build initial circuits");

        let active = manager.active_circuits();
        assert_eq!(active.len(), 2);

        let mut expected_latencies = BTreeMap::new();
        for (idx, circuit) in active.iter().enumerate() {
            let millis = if idx == 0 { 55 } else { 62 };
            assert!(manager.record_latency(circuit.id, Duration::from_millis(millis)));
            expected_latencies.insert(circuit.entry.relay_id, millis);
        }

        let retired_at = 2_000;
        let records = manager.teardown_all(retired_at);
        assert_eq!(records.len(), expected_latencies.len());
        assert!(manager.active_circuits().is_empty());

        for record in &records {
            assert_eq!(record.reason, CircuitRetirementReason::ManualTeardown);
            assert_eq!(record.retired_at_unix, retired_at);
            let snapshot = record
                .latency
                .as_ref()
                .expect("latency snapshot recorded during teardown");
            assert_eq!(snapshot.samples, 1);
            let expected = expected_latencies
                .get(&record.info.entry.relay_id)
                .copied()
                .expect("expected latency entry for relay");
            assert_eq!(snapshot.min_ms, expected);
            assert_eq!(snapshot.max_ms, expected);
            assert!(
                (snapshot.average_ms - expected as f64).abs() < f64::EPSILON,
                "expected average {} for relay {:?}, got {}",
                expected,
                record.info.entry.relay_id,
                snapshot.average_ms
            );
        }

        assert_eq!(manager.rotation_history().len(), records.len());
        for record in manager.rotation_history() {
            assert_eq!(record.reason, CircuitRetirementReason::ManualTeardown);
            assert!(record.latency.is_some());
        }
    }

    #[test]
    fn circuit_manager_soak_maintains_latency_stability() {
        let guard = GuardRecord {
            relay_id: relay_id(0x04),
            pinned_at_unix: 75,
            endpoint: Endpoint::new("soranet://guard-soak", 0),
            guard_weight: 102,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard.clone()]);

        let directory = RelayDirectory::new(vec![
            middle_descriptor(0x13, 55, true, 0),
            exit_descriptor(0x23, 65, true, 0),
        ]);

        let ttl = Duration::from_secs(2);
        let mut manager = CircuitManager::new(CircuitManagerConfig::new(ttl));
        let mut now = 0u64;
        let mut averages = Vec::new();

        for rotation in 0..3 {
            if manager.active_circuits().is_empty() {
                let events = manager
                    .refresh(&directory, &guard_set, now, AnonymityPolicy::GuardPq)
                    .expect("build circuit");
                assert!(
                    events
                        .iter()
                        .any(|event| matches!(event, CircuitEvent::Built { .. }))
                );
            }

            let circuit = manager
                .active_circuits()
                .into_iter()
                .next()
                .expect("circuit available");
            let base = 50 + rotation as u64;
            for sample in [base, base + 2, base - 1] {
                assert!(manager.record_latency(circuit.id, Duration::from_millis(sample)));
            }

            now += ttl.as_secs() + 1;
            let events = manager
                .refresh(&directory, &guard_set, now, AnonymityPolicy::GuardPq)
                .expect("rotate circuit");
            let retired_avg = events
                .iter()
                .filter_map(|event| match event {
                    CircuitEvent::Retired { record } => {
                        record.latency.as_ref().map(|snap| snap.average_ms)
                    }
                    _ => None,
                })
                .next()
                .expect("retired circuit latency present");
            averages.push(retired_avg);
        }

        assert_eq!(averages.len(), 3);
        let min = averages
            .iter()
            .fold(f64::INFINITY, |acc, &value| acc.min(value));
        let max = averages
            .iter()
            .fold(f64::NEG_INFINITY, |acc, &value| acc.max(value));
        assert!(
            (max - min) <= 5.0,
            "latency drift exceeded threshold: {max} - {min} > 5ms"
        );
        assert!(
            manager.rotation_history().len() >= 3,
            "expected at least three rotation records"
        );
    }

    #[test]
    fn provider_support_helpers_match_metadata() {
        use sorafs_car::multi_fetch::{FetchProvider, ProviderMetadata, TransportHint};

        let mut pq_metadata = ProviderMetadata::new();
        pq_metadata.capability_names = vec!["soranet_pq_strict".into()];
        pq_metadata.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        let pq_provider = FetchProvider::new("pq-provider").with_metadata(pq_metadata);

        let mut classical_metadata = ProviderMetadata::new();
        classical_metadata.capability_names = vec!["soranet_classical".into()];
        classical_metadata.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        let classical_provider =
            FetchProvider::new("classic-provider").with_metadata(classical_metadata);

        assert!(crate::provider_supports_soranet(&pq_provider));
        assert!(crate::provider_supports_pq(&pq_provider));
        assert!(crate::provider_supports_soranet(&classical_provider));
        assert!(!crate::provider_supports_pq(&classical_provider));
    }

    #[test]
    fn endpoint_tag_roundtrip() {
        let tag = EndpointTag::from_label("norito-stream").expect("parse norito-stream tag");
        assert_eq!(tag, EndpointTag::NoritoStream);
        assert_eq!(tag.as_label(), "norito-stream");
        assert!(EndpointTag::from_label("unknown-tag").is_err());
    }

    #[test]
    fn descriptor_prefers_tagged_endpoint_for_exit() {
        let descriptor = RelayDescriptor {
            relay_id: relay_id(0xA1),
            guard_weight: 5,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(false, false, true),
            endpoints: vec![
                Endpoint::new("soranet://exit-default", 0),
                Endpoint::with_tags("soranet://exit-norito", 1, vec![EndpointTag::NoritoStream]),
            ],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let relay = descriptor_to_circuit_relay(&descriptor, Some(EndpointTag::NoritoStream), 0);
        assert_eq!(relay.endpoint.url, "soranet://exit-norito");

        let fallback = descriptor_to_circuit_relay(&descriptor, None, 0);
        assert_eq!(fallback.endpoint.url, "soranet://exit-default");
    }

    #[test]
    fn path_hints_enrich_directory_metadata() {
        let relay_id = relay_id(0xEF);
        let mut directory = RelayDirectory::new(vec![RelayDescriptor {
            relay_id,
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://hinted", 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);
        let hint = RelayPathHint {
            relay_id,
            avg_rtt_ms: Some(22),
            region: Some("us-west".to_string()),
            asn: Some(64_512),
            validator_lane: true,
            masque_bypass_allowed: true,
        };

        let report = directory.apply_path_hints(&[hint]);
        assert_eq!(report.applied, 1);
        assert!(report.missing.is_empty());

        let metadata = &directory.entries()[0].path_metadata;
        assert_eq!(metadata.avg_rtt_ms, Some(22));
        assert_eq!(metadata.region.as_deref(), Some("us-west"));
        assert_eq!(metadata.asn, Some(64_512));
        assert!(metadata.validator_lane);
        assert!(metadata.masque_bypass_allowed);
    }

    #[test]
    fn selector_prioritises_validator_and_asn_diversity() {
        let validator = RelayDescriptor {
            relay_id: relay_id(0x01),
            guard_weight: 160,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://validator", 0)],
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata {
                avg_rtt_ms: Some(25),
                region: Some("us-east".to_string()),
                asn: Some(64_512),
                validator_lane: true,
                masque_bypass_allowed: true,
            },
        };
        let same_asn = RelayDescriptor {
            relay_id: relay_id(0x02),
            guard_weight: 170,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://classic-fast", 0)],
            pq_kem_public: Some(vec![0xBB]),
            certificate: None,
            path_metadata: PathMetadata {
                avg_rtt_ms: Some(10),
                region: Some("us-east".to_string()),
                asn: Some(64_512),
                validator_lane: false,
                masque_bypass_allowed: false,
            },
        };
        let diverse_asn = RelayDescriptor {
            relay_id: relay_id(0x03),
            guard_weight: 150,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints: vec![Endpoint::new("soranet://diverse", 0)],
            pq_kem_public: Some(vec![0xCC]),
            certificate: None,
            path_metadata: PathMetadata {
                avg_rtt_ms: Some(12),
                region: Some("us-west".to_string()),
                asn: Some(64_570),
                validator_lane: false,
                masque_bypass_allowed: false,
            },
        };

        let directory = RelayDirectory::new(vec![validator, same_asn, diverse_asn]);
        let selector = GuardSelector::new(NonZeroUsize::new(2).expect("non-zero"));
        let guards = selector.select(&directory, None, 5, AnonymityPolicy::GuardPq);

        assert_eq!(guards.len(), 2);
        assert_eq!(guards.guards()[0].relay_id, relay_id(0x01));
        assert!(guards.guards()[0].path_metadata.validator_lane);
        assert_eq!(guards.guards()[1].relay_id, relay_id(0x03));
        assert_eq!(guards.guards()[1].path_metadata.asn, Some(64_570));
    }
}
