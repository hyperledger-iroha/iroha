//! Data structures supporting Kaigi instructions.
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use derive_more::Display;
use getset::Getters;
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use crate::{account::AccountId, domain::DomainId, metadata::Metadata, name::Name};

/// Domain separation tag for Kaigi roster commitment leaves.
const KAIGI_ROSTER_LEAF_TAG: &[u8] = b"iroha:kaigi:roster:leaf:v1\x00";
/// Seed used for the deterministic empty Kaigi roster root.
const KAIGI_ROSTER_EMPTY_SEED: &[u8] = b"iroha:kaigi:roster:empty:v1\x00";

fn empty_roster_root() -> Hash {
    Hash::new(KAIGI_ROSTER_EMPTY_SEED)
}

fn roster_leaf_hash(commitment: &Hash) -> HashOf<[u8; 32]> {
    let mut buf = [0u8; KAIGI_ROSTER_LEAF_TAG.len() + Hash::LENGTH];
    buf[..KAIGI_ROSTER_LEAF_TAG.len()].copy_from_slice(KAIGI_ROSTER_LEAF_TAG);
    buf[KAIGI_ROSTER_LEAF_TAG.len()..].copy_from_slice(commitment.as_ref());
    HashOf::from_untyped_unchecked(Hash::new(buf))
}

fn compute_roster_root_from(commitments: &[KaigiParticipantCommitment]) -> Hash {
    if commitments.is_empty() {
        return empty_roster_root();
    }
    let mut tree = MerkleTree::<[u8; 32]>::default();
    for entry in commitments {
        tree.add(roster_leaf_hash(&entry.commitment));
    }
    tree.root().map_or_else(empty_roster_root, Hash::from)
}

/// Identifier for a Kaigi session scoped to a domain.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Display,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
#[display("{domain_id}:{call_name}")]
pub struct KaigiId {
    /// Domain that owns the call.
    pub domain_id: DomainId,
    /// Name of the call within the domain namespace.
    pub call_name: Name,
}

impl KaigiId {
    /// Construct a new call identifier.
    #[must_use]
    pub fn new(domain_id: DomainId, call_name: Name) -> Self {
        Self {
            domain_id,
            call_name,
        }
    }
}

/// Privacy configuration for Kaigi sessions.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", norito(tag = "mode", content = "state"))]
#[norito(reuse_archived)]
pub enum KaigiPrivacyMode {
    /// Participants are stored explicitly, matching the prior behaviour.
    Transparent,
    /// Participants use commitments/nullifiers for roster privacy.
    ZkRosterV1,
}

/// Room access policy that determines viewer authentication requirements.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    Default,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", norito(tag = "policy", content = "state"))]
#[norito(reuse_archived)]
pub enum KaigiRoomPolicy {
    /// Viewers may join without presenting authentication tokens.
    Public,
    /// Viewers must authenticate before the relay forwards traffic.
    #[default]
    Authenticated,
}

/// Commitment entry representing an anonymised participant.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct KaigiParticipantCommitment {
    /// Hash commitment to the participant identity.
    pub commitment: Hash,
    /// Optional alias tag surfaced to the host for diagnostics.
    pub alias_tag: Option<String>,
}

/// Nullifier entry ensuring joins/leaves occur only once.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
#[allow(missing_copy_implementations)]
pub struct KaigiParticipantNullifier {
    /// Hash derived from the join/leave witness for uniqueness.
    pub digest: Hash,
    /// Milliseconds since epoch when the nullifier was issued.
    pub issued_at_ms: u64,
}

/// Relay hop metadata.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRelayHop {
    /// Account offering relay services.
    pub relay_id: AccountId,
    /// HPKE public key bytes advertised by the relay.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub hpke_public_key: Vec<u8>,
    /// Relative weight for load-balancing decisions.
    pub weight: u8,
}

/// Relay manifest enumerating hops for onion routing.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRelayManifest {
    /// Relay hops to traverse for control/data packets.
    pub hops: Vec<KaigiRelayHop>,
    /// Timestamp after which the manifest must be refreshed.
    pub expiry_ms: u64,
}

/// Relay registration descriptor persisted in domain metadata.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRelayRegistration {
    /// Account advertising relay capabilities.
    pub relay_id: AccountId,
    /// HPKE public key bytes advertised by the relay.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub hpke_public_key: Vec<u8>,
    /// Bandwidth class signalled by the relay (larger value == higher capacity).
    pub bandwidth_class: u8,
}

/// Health indicator reported for a Kaigi relay.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "state"))]
#[norito(reuse_archived)]
pub enum KaigiRelayHealthStatus {
    /// Relay is operating as expected.
    Healthy,
    /// Relay is experiencing partial degradation but may still forward traffic.
    Degraded,
    /// Relay is unavailable and should be treated as failed.
    Unavailable,
}

impl KaigiRelayHealthStatus {
    /// Map the status to a deterministic metric value for gauges.
    #[must_use]
    pub const fn metric_value(self) -> i64 {
        match self {
            Self::Healthy => 0,
            Self::Degraded => 1,
            Self::Unavailable => 2,
        }
    }

    /// Human-readable label used for telemetry metrics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Signed health feedback persisted in domain metadata.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRelayFeedback {
    /// Relay being reported on.
    pub relay_id: AccountId,
    /// Kaigi session where the relay observation occurred.
    pub call: KaigiId,
    /// Account submitting the feedback (typically the host).
    pub reported_by: AccountId,
    /// Reported relay status.
    pub status: KaigiRelayHealthStatus,
    /// Timestamp (milliseconds since epoch) when the feedback was recorded.
    pub reported_at_ms: u64,
    /// Optional human-readable notes or reason for the update.
    pub notes: Option<String>,
}

/// Governance-managed allowlist restricting which relays may operate in a domain.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    Default,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRelayAllowlist {
    /// Set of relay accounts authorised by governance.
    pub allowed_relays: BTreeSet<AccountId>,
}

impl KaigiRelayAllowlist {
    /// Check whether the given relay subject is included in the allowlist.
    #[must_use]
    pub fn contains(&self, relay_id: &AccountId) -> bool {
        self.allowed_relays
            .iter()
            .any(|allowed| allowed.subject_id() == relay_id.subject_id())
    }
}

/// Configuration submitted by the host when creating a call.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Getters,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct NewKaigi {
    /// Identifier of the call.
    #[getset(get = "pub")]
    pub id: KaigiId,
    /// Account hosting the call.
    #[getset(get = "pub")]
    pub host: AccountId,
    /// Optional human readable title.
    #[getset(get = "pub")]
    pub title: Option<String>,
    /// Optional description of the session.
    #[getset(get = "pub")]
    pub description: Option<String>,
    /// Maximum number of concurrent participants (excluding the host).
    #[getset(get = "pub")]
    pub max_participants: Option<u32>,
    /// Gas rate charged per minute of call time (host provided).
    #[getset(get = "pub")]
    pub gas_rate_per_minute: u64,
    /// Optional per-call metadata for additional signalling.
    #[getset(get = "pub")]
    pub metadata: Metadata,
    /// Optional scheduled start timestamp (milliseconds since epoch).
    #[getset(get = "pub")]
    pub scheduled_start_ms: Option<u64>,
    /// Optional billing account used to settle call costs.
    #[getset(get = "pub")]
    pub billing_account: Option<AccountId>,
    /// Privacy configuration requested by the host.
    #[getset(get = "pub")]
    pub privacy_mode: KaigiPrivacyMode,
    /// Viewer authentication policy enforced by the relays.
    #[norito(default)]
    #[getset(get = "pub")]
    pub room_policy: KaigiRoomPolicy,
    /// Optional relay manifest snapshot carrying structured relay metadata.
    #[getset(get = "pub")]
    pub relay_manifest: Option<KaigiRelayManifest>,
}

impl NewKaigi {
    /// Create a call builder with default metadata.
    #[must_use]
    pub fn with_defaults(id: KaigiId, host: AccountId) -> Self {
        Self {
            id,
            host,
            title: None,
            description: None,
            max_participants: None,
            gas_rate_per_minute: 0,
            metadata: Metadata::default(),
            scheduled_start_ms: None,
            billing_account: None,
            privacy_mode: KaigiPrivacyMode::Transparent,
            room_policy: KaigiRoomPolicy::default(),
            relay_manifest: None,
        }
    }
}

/// Lifecycle state of a call.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "state"))]
#[norito(reuse_archived)]
pub enum KaigiStatus {
    /// Call has been created but not yet ended.
    Active,
    /// Call has concluded.
    Ended,
}

/// Stored call record tracked inside domain metadata.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[norito(reuse_archived)]
pub struct KaigiRecord {
    /// Identifier of the call.
    pub id: KaigiId,
    /// Host account that created the call.
    pub host: AccountId,
    /// Optional dedicated billing account.
    pub billing_account: Option<AccountId>,
    /// Optional human readable title.
    pub title: Option<String>,
    /// Optional description provided by the host.
    pub description: Option<String>,
    /// Maximum number of participants allowed (excluding host).
    pub max_participants: Option<u32>,
    /// Gas rate charged per minute of call time.
    pub gas_rate_per_minute: u64,
    /// Call metadata supplied by the host.
    pub metadata: Metadata,
    /// Optional scheduled start timestamp announced by the host.
    pub scheduled_start_ms: Option<u64>,
    /// Privacy configuration applied to the session.
    pub privacy_mode: KaigiPrivacyMode,
    /// Viewer authentication policy enforced by the relays.
    #[norito(default)]
    pub room_policy: KaigiRoomPolicy,
    /// Relay manifest snapshot carrying structured relay metadata.
    pub relay_manifest: Option<KaigiRelayManifest>,
    /// Deterministic Merkle root of the roster commitment tree.
    pub roster_root: Hash,
    /// Participant roster commitments used in privacy mode.
    pub roster_commitments: Vec<KaigiParticipantCommitment>,
    /// Nullifier log preventing duplicate joins in privacy mode.
    pub nullifier_log: Vec<KaigiParticipantNullifier>,
    /// Historical usage commitments recorded for privacy billing.
    pub usage_commitments: Vec<Hash>,
    /// Current status of the call.
    pub status: KaigiStatus,
    /// Milliseconds since epoch when the record was created.
    pub created_at_ms: u64,
    /// Milliseconds since epoch when the call was ended (if applicable).
    pub ended_at_ms: Option<u64>,
    /// Milliseconds of call time recorded so far.
    pub total_duration_ms: u64,
    /// Total gas billed for this session across all usage records.
    pub total_billed_gas: u64,
    /// Number of usage segments recorded.
    pub segments_recorded: u32,
    /// Participants currently associated with the call (excluding the host unless explicitly added).
    pub participants: Vec<AccountId>,
    /// Additional participant metadata (keyed by participant id) for future expansion.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::account_metadata_map")
    )]
    pub participant_metadata: BTreeMap<AccountId, Metadata>,
}

impl KaigiRecord {
    /// Construct a call record from a [`NewKaigi`] template.
    #[must_use]
    pub fn from_new(template: &NewKaigi, created_at_ms: u64) -> Self {
        Self {
            id: template.id.clone(),
            host: template.host.clone(),
            billing_account: template.billing_account.clone(),
            title: template.title.clone(),
            description: template.description.clone(),
            max_participants: template.max_participants,
            gas_rate_per_minute: template.gas_rate_per_minute,
            metadata: template.metadata.clone(),
            scheduled_start_ms: template.scheduled_start_ms,
            privacy_mode: template.privacy_mode,
            room_policy: template.room_policy,
            relay_manifest: template.relay_manifest.clone(),
            roster_root: empty_roster_root(),
            roster_commitments: Vec::new(),
            nullifier_log: Vec::new(),
            usage_commitments: Vec::new(),
            status: KaigiStatus::Active,
            created_at_ms,
            ended_at_ms: None,
            total_duration_ms: 0,
            total_billed_gas: 0,
            segments_recorded: 0,
            participants: Vec::new(),
            participant_metadata: BTreeMap::new(),
        }
    }

    /// Determine if the participant list already contains `account` by subject.
    #[must_use]
    pub fn has_participant(&self, account: &AccountId) -> bool {
        self.participants
            .iter()
            .any(|participant| participant.subject_id() == account.subject_id())
    }

    /// Determine if the roster already contains `commitment`.
    #[must_use]
    pub fn has_commitment(&self, commitment: &KaigiParticipantCommitment) -> bool {
        self.roster_commitments
            .iter()
            .any(|existing| existing.commitment == commitment.commitment)
    }

    /// Current Merkle root of the roster commitment tree.
    #[must_use]
    pub fn roster_root(&self) -> Hash {
        self.roster_root
    }

    /// Compute the roster root for an arbitrary commitment slice.
    #[must_use]
    pub fn compute_roster_root(commitments: &[KaigiParticipantCommitment]) -> Hash {
        compute_roster_root_from(commitments)
    }

    fn refresh_roster_root(&mut self) {
        self.roster_root = Self::compute_roster_root(&self.roster_commitments);
    }

    /// Append a roster commitment without additional deduplication.
    pub fn push_commitment(&mut self, commitment: KaigiParticipantCommitment) {
        self.roster_commitments.push(commitment);
        self.refresh_roster_root();
    }

    /// Remove a commitment if present.
    pub fn remove_commitment(&mut self, commitment: &KaigiParticipantCommitment) -> bool {
        let len_before = self.roster_commitments.len();
        self.roster_commitments
            .retain(|existing| existing.commitment != commitment.commitment);
        let removed = len_before != self.roster_commitments.len();
        if removed {
            self.refresh_roster_root();
        }
        removed
    }

    /// Check if a nullifier digest has already been recorded.
    #[must_use]
    pub fn has_nullifier(&self, nullifier: &KaigiParticipantNullifier) -> bool {
        self.nullifier_log
            .iter()
            .any(|existing| existing.digest == nullifier.digest)
    }

    /// Append a nullifier to the audit log.
    pub fn push_nullifier(&mut self, nullifier: KaigiParticipantNullifier) {
        self.nullifier_log.push(nullifier);
    }

    /// Append a usage commitment to the privacy usage log.
    pub fn push_usage_commitment(&mut self, commitment: Hash) {
        self.usage_commitments.push(commitment);
    }

    /// Append a participant, preserving uniqueness.
    pub fn push_participant(&mut self, account: AccountId) {
        if !self.has_participant(&account) {
            self.participants.push(account);
            self.participants.sort_unstable();
            self.participants.dedup();
        }
    }

    /// Remove a participant if present.
    pub fn remove_participant(&mut self, account: &AccountId) -> bool {
        let len_before = self.participants.len();
        self.participants
            .retain(|participant| participant.subject_id() != account.subject_id());
        len_before != self.participants.len()
    }

    /// Replace the relay manifest snapshot associated with the call.
    pub fn set_relay_manifest(&mut self, manifest: Option<KaigiRelayManifest>) {
        self.relay_manifest = manifest;
    }
}

/// Helper used for rendering deterministic metadata keys in domain storage.
///
/// # Errors
///
/// Returns [`ParseError`](crate::error::ParseError) if the composed key violates
/// the [`Name`] invariants enforced by the parser.
pub fn kaigi_metadata_key(call_name: &Name) -> Result<Name, crate::error::ParseError> {
    let composite = format!("kaigi__{}", call_name.as_ref());
    Name::from_str(&composite)
}

/// Helper used for storing Kaigi relay registrations inside domain metadata.
///
/// # Errors
///
/// Returns [`ParseError`](crate::error::ParseError) if the composed key violates [`Name`] rules.
pub fn kaigi_relay_metadata_key(relay_id: &AccountId) -> Result<Name, crate::error::ParseError> {
    let sanitized = relay_id
        .signatory()
        .to_string()
        .replace(['@', '#', '$'], "_");
    let key = format!("kaigi_relay__{sanitized}");
    Name::from_str(&key)
}

/// Helper used for storing Kaigi relay health feedback in domain metadata.
///
/// # Errors
///
/// Returns [`ParseError`](crate::error::ParseError) if the composed key violates [`Name`] rules.
pub fn kaigi_relay_feedback_key(relay_id: &AccountId) -> Result<Name, crate::error::ParseError> {
    let sanitized = relay_id
        .signatory()
        .to_string()
        .replace(['@', '#', '$'], "_");
    let key = format!("kaigi_relay_feedback__{sanitized}");
    Name::from_str(&key)
}

/// Metadata key storing the governance-managed relay allowlist.
///
/// # Errors
///
/// Returns [`ParseError`](crate::error::ParseError) if the key violates [`Name`] rules.
pub fn kaigi_relay_allowlist_key() -> Result<Name, crate::error::ParseError> {
    Name::from_str("kaigi_relay_allowlist")
}

/// Prelude re-export for Kaigi data structures.
pub mod prelude {
    pub use super::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRecord, KaigiRelayAllowlist, KaigiRelayFeedback, KaigiRelayHealthStatus,
        KaigiRelayHop, KaigiRelayManifest, KaigiRelayRegistration, KaigiRoomPolicy, KaigiStatus,
        NewKaigi, kaigi_metadata_key, kaigi_relay_allowlist_key, kaigi_relay_feedback_key,
        kaigi_relay_metadata_key,
    };
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::KeyPair;
    use norito::codec::{decode_adaptive, encode_adaptive};

    use super::*;

    #[test]
    fn metadata_key_prefixes_call_name() {
        let call_name = Name::from_str("demo").expect("valid name");
        let key = kaigi_metadata_key(&call_name).expect("metadata key");
        assert_eq!(key.as_ref(), "kaigi__demo");
    }

    #[test]
    fn call_record_participant_management_is_deterministic() {
        let domain_id = DomainId::from_str("nexus").expect("domain id");
        let host = AccountId::new(KeyPair::random().public_key().clone());
        let call_id = KaigiId::new(domain_id.clone(), Name::from_str("daily").unwrap());
        let template = NewKaigi::with_defaults(call_id, host);
        let mut record = KaigiRecord::from_new(&template, 1234);

        assert_eq!(record.status, KaigiStatus::Active);
        assert!(record.participants.is_empty());

        let bob = AccountId::new(KeyPair::random().public_key().clone());
        record.push_participant(bob.clone());
        record.push_participant(bob.clone());
        assert_eq!(record.participants.len(), 1);
        assert!(record.has_participant(&bob));

        assert!(record.remove_participant(&bob));
        assert!(!record.has_participant(&bob));
        assert!(!record.remove_participant(&bob));
        assert!(record.participants.is_empty());
    }

    #[test]
    fn call_record_preserves_creation_timestamp_and_schedule() {
        let domain_id = DomainId::from_str("nexus").expect("domain id");
        let host = AccountId::new(KeyPair::random().public_key().clone());
        let call_id = KaigiId::new(domain_id.clone(), Name::from_str("standup").unwrap());
        let mut template = NewKaigi::with_defaults(call_id, host);
        template.scheduled_start_ms = Some(7_777);
        template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
        template.relay_manifest = Some(KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: AccountId::new(KeyPair::random().public_key().clone()),
                hpke_public_key: vec![0, 1, 2],
                weight: 1,
            }],
            expiry_ms: 9_999,
        });

        let record = KaigiRecord::from_new(&template, 1_234);

        assert_eq!(record.created_at_ms, 1_234);
        assert_eq!(record.scheduled_start_ms, Some(7_777));
        assert_eq!(record.privacy_mode, KaigiPrivacyMode::ZkRosterV1);
        assert!(record.roster_commitments.is_empty());
        assert!(record.nullifier_log.is_empty());
        assert_eq!(
            record.relay_manifest.as_ref(),
            template.relay_manifest.as_ref()
        );
    }

    #[test]
    fn relay_hop_canonical_roundtrip() {
        let _domain = DomainId::from_str("relay-domain").expect("domain");
        let relay = AccountId::new(KeyPair::random().public_key().clone());
        let hop = KaigiRelayHop {
            relay_id: relay.clone(),
            hpke_public_key: vec![0xAA, 0xBB, 0xCC],
            weight: 3,
        };
        let bytes = encode_adaptive(&hop);
        let decoded: KaigiRelayHop = decode_adaptive(&bytes).expect("decode relay hop");
        assert_eq!(decoded, hop);
    }

    #[test]
    fn new_kaigi_canonical_roundtrip() {
        let domain = DomainId::from_str("wonderland").expect("domain");
        let host_keypair = KeyPair::random();
        let host = AccountId::new(host_keypair.public_key().clone());
        let id = KaigiId::new(
            domain.clone(),
            Name::from_str("weekly-sync").expect("call name"),
        );
        let relay = AccountId::new(KeyPair::random().public_key().clone());
        let manifest = KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: relay,
                hpke_public_key: vec![0x01; 32],
                weight: 5,
            }],
            expiry_ms: 1_700_111_000_000,
        };
        let mut metadata = Metadata::default();
        metadata.insert(Name::from_str("topic").expect("metadata key"), "status");
        let call = NewKaigi {
            id,
            host: host.clone(),
            title: Some("Weekly Sync".to_owned()),
            description: Some("Roadmap alignment".to_owned()),
            max_participants: Some(16),
            gas_rate_per_minute: 120,
            metadata,
            scheduled_start_ms: Some(1_700_000_000_000),
            billing_account: Some(host),
            privacy_mode: KaigiPrivacyMode::ZkRosterV1,
            room_policy: KaigiRoomPolicy::Authenticated,
            relay_manifest: Some(manifest),
        };

        let hop_bytes =
            encode_adaptive(&call.relay_manifest.as_ref().expect("manifest present").hops[0]);
        let _ = <AccountId as norito::core::DecodeFromSlice>::decode_from_slice(&hop_bytes);
        let manifest_bytes =
            encode_adaptive(call.relay_manifest.as_ref().expect("manifest present"));
        let _manifest_decoded: KaigiRelayManifest =
            decode_adaptive(&manifest_bytes).expect("decode manifest");
        let bytes = encode_adaptive(&call);
        let decoded: NewKaigi = decode_adaptive(&bytes).expect("decode create kaigi payload");
        assert_eq!(decoded, call);
    }

    #[test]
    fn participant_commitment_norito_roundtrip() {
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::new(b"commitment-bytes"),
            alias_tag: Some("relay-1".to_owned()),
        };

        let bytes = encode_adaptive(&commitment);
        let decoded: KaigiParticipantCommitment =
            decode_adaptive(&bytes).expect("decode commitment");
        assert_eq!(decoded, commitment);
    }

    #[test]
    fn roster_root_updates_with_commitment_changes() {
        let empty_root = KaigiRecord::compute_roster_root(&[]);
        assert_eq!(empty_root, empty_roster_root());

        let commitment = KaigiParticipantCommitment {
            commitment: Hash::prehashed([0xAA; Hash::LENGTH]),
            alias_tag: Some("participant".to_string()),
        };
        let populated_root = KaigiRecord::compute_roster_root(std::slice::from_ref(&commitment));
        assert_ne!(populated_root, empty_root);

        let domain = DomainId::from_str("kaigi").expect("domain");
        let host_key = KeyPair::random();
        let host = AccountId::new(host_key.public_key().clone());
        let call_id = KaigiId::new(domain.clone(), Name::from_str("privacy").expect("call"));
        let template = NewKaigi::with_defaults(call_id, host.clone());
        let mut record = KaigiRecord::from_new(&template, 0);
        assert_eq!(record.roster_root(), empty_roster_root());

        record.push_commitment(commitment.clone());
        assert_eq!(record.roster_root(), populated_root);
        assert!(record.has_commitment(&commitment));

        let removed = record.remove_commitment(&commitment);
        assert!(removed);
        assert!(record.roster_commitments.is_empty());
        assert_eq!(record.roster_root(), empty_roster_root());
    }

    #[test]
    fn relay_manifest_can_be_replaced_or_cleared() {
        let domain_id = DomainId::from_str("kaigi").expect("domain");
        let host_key = KeyPair::random();
        let host = AccountId::new(host_key.public_key().clone());
        let call_id = KaigiId::new(domain_id.clone(), Name::from_str("route").expect("call"));
        let mut template = NewKaigi::with_defaults(call_id, host.clone());
        let initial_manifest = KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: AccountId::new(KeyPair::random().public_key().clone()),
                hpke_public_key: vec![1, 2, 3],
                weight: 5,
            }],
            expiry_ms: 123,
        };
        template.relay_manifest = Some(initial_manifest.clone());

        let mut record = KaigiRecord::from_new(&template, 0);
        assert_eq!(record.relay_manifest.as_ref(), Some(&initial_manifest));

        let updated_manifest = KaigiRelayManifest {
            hops: vec![
                KaigiRelayHop {
                    relay_id: AccountId::new(KeyPair::random().public_key().clone()),
                    hpke_public_key: vec![4, 5, 6],
                    weight: 7,
                },
                KaigiRelayHop {
                    relay_id: AccountId::new(KeyPair::random().public_key().clone()),
                    hpke_public_key: vec![7, 8, 9],
                    weight: 3,
                },
            ],
            expiry_ms: 456,
        };

        record.set_relay_manifest(Some(updated_manifest.clone()));
        assert_eq!(record.relay_manifest.as_ref(), Some(&updated_manifest));

        record.set_relay_manifest(None);
        assert!(record.relay_manifest.is_none());
    }

    #[test]
    fn relay_registration_norito_roundtrip() {
        let _domain_id = DomainId::from_str("kaigi").expect("domain");
        let relay_id = AccountId::new(KeyPair::random().public_key().clone());
        let registration = KaigiRelayRegistration {
            relay_id: relay_id.clone(),
            hpke_public_key: vec![0xAA, 0xBB, 0xCC],
            bandwidth_class: 7,
        };

        let bytes = encode_adaptive(&registration);
        let decoded: KaigiRelayRegistration =
            decode_adaptive(&bytes).expect("decode relay registration");
        assert_eq!(decoded, registration);

        let key = kaigi_relay_metadata_key(&relay_id).expect("metadata key");
        assert!(key.as_ref().starts_with("kaigi_relay__"));
    }

    #[test]
    fn relay_feedback_key_uses_sanitized_signatory() {
        let relay = AccountId::new(KeyPair::random().public_key().clone());
        let key = kaigi_relay_feedback_key(&relay).expect("feedback key");
        let sanitized_fragment = relay.signatory().to_string();
        assert_eq!(
            key.as_ref(),
            format!("kaigi_relay_feedback__{sanitized_fragment}")
        );
    }

    #[test]
    fn allowlist_membership_checks_handle_present_and_missing_relays() {
        let relay = AccountId::new(KeyPair::random().public_key().clone());
        let mut allowlist = KaigiRelayAllowlist::default();
        allowlist.allowed_relays.insert(relay.clone());
        assert!(allowlist.contains(&relay));
        let other = AccountId::new(KeyPair::random().public_key().clone());
        assert!(!allowlist.contains(&other));
    }

    #[test]
    fn participant_nullifier_norito_roundtrip() {
        let nullifier = KaigiParticipantNullifier {
            digest: Hash::new(b"nullifier-seed"),
            issued_at_ms: 42,
        };

        let bytes = encode_adaptive(&nullifier);
        let decoded: KaigiParticipantNullifier = decode_adaptive(&bytes).expect("decode nullifier");
        assert_eq!(decoded, nullifier);
    }
}
