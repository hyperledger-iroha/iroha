//! DA ingest authorization, request, receipt, and shared durable spool types.

use crate::parameter::CustomParameter;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    da::types::{
        BlobClass, BlobCodec, BlobDigest, Compression, DaRentQuote, ErasureProfile, ExtraMetadata,
        FecScheme, MetadataEncryption, MetadataVisibility, RetentionPolicy, StorageTicketId,
    },
    parameter::CustomParameterId,
    sorafs::pin_registry::{ManifestDigest, StorageClass},
};
use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_model_base::topology::LaneId;

use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;
/// Domain separator for version-one DA ingest request signatures.
pub const DA_INGEST_REQUEST_SIGNING_DOMAIN_V1: &[u8] = b"iroha:da-ingest-request:v1\0";
/// Domain separator for the immutable request-content commitment carried into consensus.
pub const DA_INGEST_REQUEST_CONTENT_DOMAIN_V1: &[u8] = b"iroha:da-ingest-request:content:v1\0";
/// Domain separator for the producer's exact post-ingest pin-scope authorization.
pub const DA_PIN_SCOPE_SIGNING_DOMAIN_V1: &[u8] = b"iroha:da:pin-scope:v1\0";
/// Consensus-wide ceiling for lane/epoch windows retained by DA admission.
pub const MAX_DA_INGEST_ADMISSION_WINDOWS_V1: usize = 1_024;
/// Consensus-wide ceiling for lane records retained by DA admission.
pub const MAX_DA_INGEST_ADMISSION_LANES_V1: usize = 1_024;
/// Consensus-wide ceiling for producer identities retained by DA admission.
pub const MAX_DA_INGEST_ADMISSION_PRODUCERS_V1: usize = 4_096;
/// One incarnation-bound lane entry in the governed DA ingest policy.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestAdmissionLaneV1")]
pub struct DaIngestAdmissionLaneV1 {
    /// Exact lane governed by this entry.
    pub lane_id: LaneId,
    /// Non-zero commitment identifying the exact active lane incarnation.
    pub lane_incarnation: Hash,
    /// Canonically ordered accounts allowed to produce for the admitted epochs.
    ///
    /// An empty list is a durable tombstone. Tombstones preserve the epoch
    /// floor when a lane is disabled or retired, but admit no requests and
    /// consume no replay-window capacity.
    pub producers: Vec<AccountId>,
    /// Current governed producer epoch.
    pub current_epoch: u64,
    /// Optional immediately preceding grace epoch.
    #[norito(required)]
    pub grace_epoch: Option<u64>,
}
impl DaIngestAdmissionLaneV1 {
    /// Return whether this entry admits an exact producer and epoch.
    #[must_use]
    pub fn authorizes(&self, owner: &AccountId, epoch: u64) -> bool {
        self.admits_epoch(epoch) && self.producers.binary_search(owner).is_ok()
    }

    /// Return whether this entry retains an exact replay window.
    #[must_use]
    pub fn admits_epoch(&self, epoch: u64) -> bool {
        !self.producers.is_empty()
            && (epoch == self.current_epoch || self.grace_epoch == Some(epoch))
    }
}
/// Versioned, consensus-replayed producer and epoch policy for DA ingest.
///
/// The predecessor commitment gives updates compare-and-swap semantics. Lane
/// entries are never dropped: an empty producer list acts as a bounded durable
/// tombstone, preventing an old signed epoch from becoming valid again after a
/// lane id is retired and later reused.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestAdmissionPolicyV1")]
pub struct DaIngestAdmissionPolicyV1 {
    /// Payload layout version. This must be [`Self::VERSION`].
    pub version: u8,
    /// Strictly increasing policy revision, starting at one.
    pub revision: u64,
    /// Exact predecessor policy commitment, absent only for revision one.
    #[norito(required)]
    pub expected_previous_policy_hash: Option<Hash>,
    /// Canonically lane-ordered admission entries and tombstones.
    pub lanes: Vec<DaIngestAdmissionLaneV1>,
}
impl DaIngestAdmissionPolicyV1 {
    /// Supported admission-policy layout version.
    pub const VERSION: u8 = 1;
    /// Reserved custom-parameter identifier for DA ingest admission.
    pub const PARAMETER_ID_STR: &'static str = "iroha:da_ingest_admission_policy_v1";
    /// Domain separator for policy commitments.
    pub const HASH_DOMAIN_V1: &'static [u8] = b"iroha:da-ingest-admission-policy:v1\0";

    /// Construct the reserved on-chain custom-parameter identifier.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid DA ingest admission custom parameter identifier")
    }

    /// Compute the canonical domain-separated commitment to this policy.
    #[must_use]
    pub fn policy_hash(&self) -> Hash {
        let encoded = self.encode();
        Hash::new_from_chunks(&[Self::HASH_DOMAIN_V1, encoded.as_slice()])
    }

    /// Validate the bounded, canonical shape of this policy.
    ///
    /// # Errors
    ///
    /// Returns [`DaIngestAdmissionPolicyError`] when the version, ordering,
    /// incarnation, epoch, or resource bounds are invalid.
    pub fn validate(&self) -> Result<(), DaIngestAdmissionPolicyError> {
        if self.version != Self::VERSION {
            return Err(DaIngestAdmissionPolicyError::UnsupportedVersion {
                actual: self.version,
                expected: Self::VERSION,
            });
        }
        if self.revision == 0 {
            return Err(DaIngestAdmissionPolicyError::ZeroRevision);
        }
        if self.lanes.len() > MAX_DA_INGEST_ADMISSION_LANES_V1 {
            return Err(DaIngestAdmissionPolicyError::TooManyLanes {
                actual: self.lanes.len(),
                maximum: MAX_DA_INGEST_ADMISSION_LANES_V1,
            });
        }
        if self
            .lanes
            .windows(2)
            .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(DaIngestAdmissionPolicyError::NonCanonicalLaneOrder);
        }
        let mut producers = 0_usize;
        let mut windows = 0_usize;
        for lane in &self.lanes {
            if lane.lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
                return Err(DaIngestAdmissionPolicyError::ZeroLaneIncarnation {
                    lane_id: lane.lane_id,
                });
            }
            if lane.current_epoch == u64::MAX {
                return Err(DaIngestAdmissionPolicyError::TerminalEpoch {
                    lane_id: lane.lane_id,
                });
            }
            if lane.producers.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(DaIngestAdmissionPolicyError::NonCanonicalProducerOrder {
                    lane_id: lane.lane_id,
                });
            }
            producers = producers.saturating_add(lane.producers.len());
            if producers > MAX_DA_INGEST_ADMISSION_PRODUCERS_V1 {
                return Err(DaIngestAdmissionPolicyError::TooManyProducers {
                    actual: producers,
                    maximum: MAX_DA_INGEST_ADMISSION_PRODUCERS_V1,
                });
            }
            if let Some(grace_epoch) = lane.grace_epoch
                && grace_epoch.checked_add(1) != Some(lane.current_epoch)
            {
                return Err(
                    DaIngestAdmissionPolicyError::GraceEpochNotImmediatelyPrevious {
                        lane_id: lane.lane_id,
                        grace_epoch,
                        current_epoch: lane.current_epoch,
                    },
                );
            }
            if !lane.producers.is_empty() {
                windows = windows.saturating_add(1 + usize::from(lane.grace_epoch.is_some()));
                if windows > MAX_DA_INGEST_ADMISSION_WINDOWS_V1 {
                    return Err(DaIngestAdmissionPolicyError::TooManyWindows {
                        actual: windows,
                        maximum: MAX_DA_INGEST_ADMISSION_WINDOWS_V1,
                    });
                }
            }
        }
        Ok(())
    }

    /// Validate optimistic-concurrency and non-reuse rules against a predecessor.
    ///
    /// # Errors
    ///
    /// Returns [`DaIngestAdmissionPolicyError`] when a revision is stale, a
    /// previous lane tombstone is dropped, or a changed lane does not advance
    /// its epoch floor.
    pub fn validate_transition(
        &self,
        previous: Option<&Self>,
    ) -> Result<(), DaIngestAdmissionPolicyError> {
        self.validate()?;
        let Some(previous) = previous else {
            if self.revision != 1 {
                return Err(DaIngestAdmissionPolicyError::InitialRevisionMismatch {
                    actual: self.revision,
                });
            }
            if self.expected_previous_policy_hash.is_some() {
                return Err(DaIngestAdmissionPolicyError::UnexpectedPreviousPolicyHash);
            }
            return Ok(());
        };
        previous.validate()?;
        let expected_revision = previous.revision.checked_add(1).ok_or(
            DaIngestAdmissionPolicyError::RevisionOverflow {
                previous: previous.revision,
            },
        )?;
        if self.revision != expected_revision {
            return Err(DaIngestAdmissionPolicyError::RevisionMismatch {
                actual: self.revision,
                expected: expected_revision,
            });
        }
        let expected_hash = previous.policy_hash();
        if self.expected_previous_policy_hash != Some(expected_hash) {
            return Err(DaIngestAdmissionPolicyError::PreviousPolicyHashMismatch {
                actual: self.expected_previous_policy_hash,
                expected: expected_hash,
            });
        }
        for prior in &previous.lanes {
            let Ok(index) = self
                .lanes
                .binary_search_by_key(&prior.lane_id, |lane| lane.lane_id)
            else {
                return Err(DaIngestAdmissionPolicyError::PriorLaneDropped {
                    lane_id: prior.lane_id,
                });
            };
            let next = &self.lanes[index];
            if next == prior {
                continue;
            }
            if next.current_epoch <= prior.current_epoch {
                return Err(DaIngestAdmissionPolicyError::EpochDidNotAdvance {
                    lane_id: prior.lane_id,
                    previous: prior.current_epoch,
                    next: next.current_epoch,
                });
            }
            if next.lane_incarnation != prior.lane_incarnation && next.grace_epoch.is_some() {
                return Err(DaIngestAdmissionPolicyError::CrossIncarnationGrace {
                    lane_id: prior.lane_id,
                });
            }
            if let Some(grace_epoch) = next.grace_epoch
                && grace_epoch != prior.current_epoch
            {
                return Err(DaIngestAdmissionPolicyError::GraceEpochNotPredecessor {
                    lane_id: prior.lane_id,
                    grace_epoch,
                    previous_epoch: prior.current_epoch,
                });
            }
        }
        Ok(())
    }

    /// Find the exact canonical entry for a lane.
    #[must_use]
    pub fn lane(&self, lane_id: LaneId) -> Option<&DaIngestAdmissionLaneV1> {
        self.lanes
            .binary_search_by_key(&lane_id, |lane| lane.lane_id)
            .ok()
            .map(|index| &self.lanes[index])
    }

    /// Return whether the committed policy authorizes an exact request scope.
    #[must_use]
    pub fn authorizes(
        &self,
        owner: &AccountId,
        lane_id: LaneId,
        lane_incarnation: Hash,
        epoch: u64,
    ) -> bool {
        self.lane(lane_id).is_some_and(|lane| {
            lane.lane_incarnation == lane_incarnation && lane.authorizes(owner, epoch)
        })
    }

    /// Return whether the committed policy retains an exact replay window.
    #[must_use]
    pub fn retains(&self, lane_id: LaneId, epoch: u64) -> bool {
        self.lane(lane_id)
            .is_some_and(|lane| lane.admits_epoch(epoch))
    }

    /// Convert this policy into the reserved custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode and structurally validate the reserved custom parameter.
    ///
    /// Non-matching identifiers return `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns [`norito::json::Error`] for malformed, unsupported, unbounded,
    /// or non-canonical payloads.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, norito::json::Error> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        let policy = norito::json::from_str::<Self>(custom.payload().get())?;
        policy
            .validate()
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        Ok(Some(policy))
    }
}
/// Validation failures for a governed DA ingest admission policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DaIngestAdmissionPolicyError {
    /// The payload used an unsupported layout version.
    #[error("unsupported DA ingest admission policy version {actual}; expected {expected}")]
    UnsupportedVersion {
        /// Observed version.
        actual: u8,
        /// Supported version.
        expected: u8,
    },
    /// Revision zero is not a valid committed revision.
    #[error("DA ingest admission policy revision must start at one")]
    ZeroRevision,
    /// The initial policy did not use revision one.
    #[error("initial DA ingest admission policy revision must be one, got {actual}")]
    InitialRevisionMismatch {
        /// Observed initial revision.
        actual: u64,
    },
    /// An initial policy unexpectedly named a predecessor.
    #[error("initial DA ingest admission policy must not name a predecessor hash")]
    UnexpectedPreviousPolicyHash,
    /// A revision counter overflowed.
    #[error("DA ingest admission policy revision cannot advance past {previous}")]
    RevisionOverflow {
        /// Previous revision.
        previous: u64,
    },
    /// A successor carried the wrong revision.
    #[error("DA ingest admission policy revision {actual} does not follow {expected}")]
    RevisionMismatch {
        /// Observed revision.
        actual: u64,
        /// Required revision.
        expected: u64,
    },
    /// A successor did not bind the exact predecessor.
    #[error("DA ingest admission policy predecessor hash mismatch")]
    PreviousPolicyHashMismatch {
        /// Observed predecessor commitment.
        actual: Option<Hash>,
        /// Required predecessor commitment.
        expected: Hash,
    },
    /// The lane vector exceeded the protocol bound.
    #[error("DA ingest admission policy has {actual} lanes; maximum is {maximum}")]
    TooManyLanes {
        /// Observed lane count.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// Lane entries were not strictly ordered.
    #[error("DA ingest admission lanes are not in canonical lane-id order")]
    NonCanonicalLaneOrder,
    /// A lane carried an all-zero incarnation.
    #[error("DA ingest admission lane {lane_id} has an all-zero incarnation")]
    ZeroLaneIncarnation {
        /// Invalid lane.
        lane_id: LaneId,
    },
    /// A lane selected the terminal epoch and could never advance or retire.
    #[error("DA ingest admission lane {lane_id} cannot use terminal epoch u64::MAX")]
    TerminalEpoch {
        /// Invalid lane.
        lane_id: LaneId,
    },
    /// Producer identities were not strictly ordered and unique.
    #[error("DA ingest admission lane {lane_id} producers are not canonically ordered")]
    NonCanonicalProducerOrder {
        /// Invalid lane.
        lane_id: LaneId,
    },
    /// Producer identities exceeded the protocol bound.
    #[error("DA ingest admission policy has {actual} producers; maximum is {maximum}")]
    TooManyProducers {
        /// Observed producer count.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// A grace epoch was not immediately before the current epoch.
    #[error(
        "DA ingest admission lane {lane_id} grace epoch {grace_epoch} is not immediately before current epoch {current_epoch}"
    )]
    GraceEpochNotImmediatelyPrevious {
        /// Invalid lane.
        lane_id: LaneId,
        /// Observed grace epoch.
        grace_epoch: u64,
        /// Current epoch.
        current_epoch: u64,
    },
    /// Retained replay windows exceeded the protocol bound.
    #[error("DA ingest admission policy retains {actual} windows; maximum is {maximum}")]
    TooManyWindows {
        /// Observed retained window count.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// A successor omitted a durable lane tombstone.
    #[error("DA ingest admission policy dropped prior lane {lane_id}")]
    PriorLaneDropped {
        /// Omitted lane.
        lane_id: LaneId,
    },
    /// A changed lane did not advance its epoch floor.
    #[error(
        "DA ingest admission lane {lane_id} changed without advancing epoch {previous}; got {next}"
    )]
    EpochDidNotAdvance {
        /// Changed lane.
        lane_id: LaneId,
        /// Previous current epoch.
        previous: u64,
        /// Successor current epoch.
        next: u64,
    },
    /// A successor carried a grace epoch across a lane incarnation boundary.
    #[error("DA ingest admission lane {lane_id} cannot carry grace across an incarnation change")]
    CrossIncarnationGrace {
        /// Changed lane.
        lane_id: LaneId,
    },
    /// A successor grace epoch did not equal its predecessor current epoch.
    #[error(
        "DA ingest admission lane {lane_id} grace epoch {grace_epoch} does not equal predecessor epoch {previous_epoch}"
    )]
    GraceEpochNotPredecessor {
        /// Changed lane.
        lane_id: LaneId,
        /// Observed grace epoch.
        grace_epoch: u64,
        /// Previous current epoch.
        previous_epoch: u64,
    },
}
/// One canonical account-controller signature over a DA ingest authorization.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestSignatureV1")]
pub struct DaIngestSignatureV1 {
    /// Account-controller key that produced the signature.
    pub signer: PublicKey,
    /// Signature over [`DaIngestAuthorizationV1::signing_digest`].
    pub signature: Signature,
}
/// One canonical account-controller signature over an exact DA pin scope.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaPinScopeSignatureV1")]
pub struct DaPinScopeSignatureV1 {
    /// Account-controller key that approved the exact pin scope.
    pub signer: PublicKey,
    /// Signature over [`DaPinScopeV1::signing_digest`].
    pub signature: Signature,
}
/// Minimal immutable DA admission authorization committed into a block sidecar.
///
/// The request-content commitment keeps the consensus payload compact while the
/// signed quota identity remains independently verifiable by every validator.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestAuthorizationV1")]
pub struct DaIngestAuthorizationV1 {
    /// Exact genesis-derived network identity authorising this admission.
    pub network_id: NetworkId,
    /// Account whose deterministic consensus quota is charged.
    pub owner: AccountId,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)` and used as the replay nonce.
    pub sequence: u64,
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
    /// Exact canonical payload length charged to the owner's quota.
    pub payload_bytes: u64,
    /// Commitment to every remaining signed request field.
    pub request_content_hash: Hash,
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaIngestSignatureV1>,
}
impl DaIngestAuthorizationV1 {
    /// Compute the exact digest signed by every account-controller witness.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DA_INGEST_REQUEST_SIGNING_DOMAIN_V1);
        hasher.update(self.network_id.as_bytes());
        let owner = self
            .owner
            .to_account_address()
            .and_then(|address| address.canonical_bytes())
            .expect("a validated AccountId must have canonical controller bytes");
        hash_len_prefixed(&mut hasher, &owner);
        hasher.update(&self.lane_id.as_u32().to_le_bytes());
        hasher.update(&self.epoch.to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.payload_hash.as_bytes());
        hasher.update(&self.payload_bytes.to_le_bytes());
        hasher.update(self.request_content_hash.as_ref());
        *hasher.finalize().as_bytes()
    }
    /// Return whether witnesses are non-empty, strictly signer ordered, and individually valid.
    #[must_use]
    pub fn has_valid_canonical_signatures(&self) -> bool {
        if self.signatures.is_empty()
            || self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return false;
        }
        let digest = self.signing_digest();
        self.signatures
            .iter()
            .all(|witness| witness.signature.verify(&witness.signer, &digest).is_ok())
    }
}
/// Exact post-ingest scope that a producer must approve before pin publication.
///
/// Torii computes the storage ticket and canonical manifest digest before this
/// scope exists. The producer signs the returned scope and retries the same
/// ingest request with those witnesses, preventing a block proposer from
/// substituting any index-bearing pin field.
///
/// This is the frozen stored layout of the baseline `world.da_pin_intents_by_ticket`
/// value. The witnesses also cover the digest of the complete paired commitment
/// ([`DaCommitmentRecord::pin_scope_digest`](super::commitment::DaCommitmentRecord::pin_scope_digest)),
/// which is not stored here: every verifier supplies it from the paired record. Torii hands
/// producers the scope together with that digest as a [`DaPinScopeProposalV1`].
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaPinScopeV1")]
pub struct DaPinScopeV1 {
    /// Exact genesis-derived network identity of the original ingest authorization.
    pub network_id: NetworkId,
    /// Account that signed the original request and this exact pin scope.
    pub owner: AccountId,
    /// Exact raw BLAKE3 digest signed by the original ingest authorization witnesses.
    /// Every bit is significant; no entity-hash marker is applied.
    pub request_authorization_digest: BlobDigest,
    /// Nexus lane carried by both the request and resulting pin intent.
    pub lane_id: LaneId,
    /// Epoch carried by both the request and resulting pin intent.
    pub epoch: u64,
    /// Sequence carried by both the request and resulting pin intent.
    pub sequence: u64,
    /// Exact durable storage ticket assigned by Torii.
    pub storage_ticket: StorageTicketId,
    /// Exact digest of the durable canonical manifest.
    pub manifest_hash: ManifestDigest,
    /// Exact normalized registry alias, when requested.
    #[norito(required)]
    pub alias: Option<String>,
}
impl DaPinScopeV1 {
    /// Construct the exact scope corresponding to an ingest authorization and durable outputs.
    #[must_use]
    pub fn new(
        authorization: &DaIngestAuthorizationV1,
        storage_ticket: StorageTicketId,
        manifest_hash: ManifestDigest,
        alias: Option<String>,
    ) -> Self {
        Self {
            network_id: authorization.network_id,
            owner: authorization.owner.clone(),
            request_authorization_digest: BlobDigest::new(authorization.signing_digest()),
            lane_id: authorization.lane_id,
            epoch: authorization.epoch,
            sequence: authorization.sequence,
            storage_ticket,
            manifest_hash,
            alias,
        }
    }

    /// Return whether this scope names the exact original ingest authorization.
    #[must_use]
    pub fn matches_authorization(&self, authorization: &DaIngestAuthorizationV1) -> bool {
        self.network_id == authorization.network_id
            && self.owner == authorization.owner
            && *self.request_authorization_digest.as_bytes() == authorization.signing_digest()
            && self.lane_id == authorization.lane_id
            && self.epoch == authorization.epoch
            && self.sequence == authorization.sequence
    }

    /// Compute the domain-separated digest signed by every pin-scope witness.
    ///
    /// `commitment_digest` is the paired record's
    /// [`DaCommitmentRecord::pin_scope_digest`](super::commitment::DaCommitmentRecord::pin_scope_digest).
    /// It binds the complete proposed commitment, including negotiated retention and the
    /// service acknowledgement, without becoming part of this stored scope.
    #[must_use]
    pub fn signing_digest(&self, commitment_digest: &BlobDigest) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DA_PIN_SCOPE_SIGNING_DOMAIN_V1);
        hasher.update(self.network_id.as_bytes());
        let owner = self
            .owner
            .to_account_address()
            .and_then(|address| address.canonical_bytes())
            .expect("a validated AccountId must have canonical controller bytes");
        hash_len_prefixed(&mut hasher, &owner);
        hasher.update(self.request_authorization_digest.as_ref());
        hasher.update(&self.lane_id.as_u32().to_le_bytes());
        hasher.update(&self.epoch.to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.storage_ticket.as_ref());
        hasher.update(self.manifest_hash.as_bytes());
        hasher.update(commitment_digest.as_bytes());
        match &self.alias {
            Some(alias) => {
                hasher.update(&[1]);
                hash_len_prefixed(&mut hasher, alias.as_bytes());
            }
            None => {
                hasher.update(&[0]);
            }
        }
        *hasher.finalize().as_bytes()
    }
}
/// Pin scope that Torii hands a producer for approval, with the digest it must also sign.
///
/// The JSON keys and binary field order are those of the pin scope Torii returns to
/// producers: the [`DaPinScopeV1`] fields with `commitment_digest` after `manifest_hash`.
/// Only the [`DaPinScopeV1`] projection is stored; verifiers recompute the digest from
/// the paired [`DaCommitmentRecord`](super::commitment::DaCommitmentRecord).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaPinScopeProposalV1")]
pub struct DaPinScopeProposalV1 {
    /// Exact genesis-derived network identity of the original ingest authorization.
    pub network_id: NetworkId,
    /// Account that signed the original request and must sign this exact pin scope.
    pub owner: AccountId,
    /// Exact raw BLAKE3 digest signed by the original ingest authorization witnesses.
    pub request_authorization_digest: BlobDigest,
    /// Nexus lane carried by both the request and resulting pin intent.
    pub lane_id: LaneId,
    /// Epoch carried by both the request and resulting pin intent.
    pub epoch: u64,
    /// Sequence carried by both the request and resulting pin intent.
    pub sequence: u64,
    /// Exact durable storage ticket assigned by Torii.
    pub storage_ticket: StorageTicketId,
    /// Exact digest of the durable canonical manifest.
    pub manifest_hash: ManifestDigest,
    /// Digest of the complete proposed on-chain commitment, including negotiated retention
    /// and acknowledgement ([`DaCommitmentRecord::pin_scope_digest`](super::commitment::DaCommitmentRecord::pin_scope_digest)).
    pub commitment_digest: BlobDigest,
    /// Exact normalized registry alias, when requested.
    #[norito(required)]
    pub alias: Option<String>,
}
impl DaPinScopeProposalV1 {
    /// Offer one stored scope together with the digest of its paired commitment.
    #[must_use]
    pub fn new(scope: DaPinScopeV1, commitment_digest: BlobDigest) -> Self {
        let DaPinScopeV1 {
            network_id,
            owner,
            request_authorization_digest,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            manifest_hash,
            alias,
        } = scope;
        Self {
            network_id,
            owner,
            request_authorization_digest,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            manifest_hash,
            commitment_digest,
            alias,
        }
    }

    /// The stored scope projection, without the commitment digest.
    #[must_use]
    pub fn scope(&self) -> DaPinScopeV1 {
        DaPinScopeV1 {
            network_id: self.network_id,
            owner: self.owner.clone(),
            request_authorization_digest: self.request_authorization_digest,
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            storage_ticket: self.storage_ticket,
            manifest_hash: self.manifest_hash,
            alias: self.alias.clone(),
        }
    }

    /// Return whether this proposal names the exact original ingest authorization.
    #[must_use]
    pub fn matches_authorization(&self, authorization: &DaIngestAuthorizationV1) -> bool {
        self.scope().matches_authorization(authorization)
    }

    /// Digest every producer witness signs for this proposal.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        self.scope().signing_digest(&self.commitment_digest)
    }
}
/// Producer authorization over one exact durable DA pin scope.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaPinScopeAuthorizationV1")]
pub struct DaPinScopeAuthorizationV1 {
    /// Exact scope approved by the producer.
    pub scope: DaPinScopeV1,
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaPinScopeSignatureV1>,
}
impl DaPinScopeAuthorizationV1 {
    /// Sign one exact scope and its paired commitment digest with an account-controller key.
    ///
    /// # Errors
    ///
    /// Returns an error when the signing backend rejects the key.
    pub fn try_sign(
        scope: DaPinScopeV1,
        commitment_digest: &BlobDigest,
        key_pair: &KeyPair,
    ) -> Result<Self, iroha_crypto::Error> {
        let signature = Signature::try_new(
            key_pair.private_key(),
            &scope.signing_digest(commitment_digest),
        )?;
        Ok(Self {
            scope,
            signatures: vec![DaPinScopeSignatureV1 {
                signer: key_pair.public_key().clone(),
                signature,
            }],
        })
    }

    /// Add one account-controller witness and restore canonical signer ordering.
    ///
    /// # Errors
    ///
    /// Returns an error when signing fails or the signer is already present.
    pub fn try_add_signature(
        &mut self,
        commitment_digest: &BlobDigest,
        key_pair: &KeyPair,
    ) -> Result<(), iroha_crypto::Error> {
        let signer = key_pair.public_key();
        if self
            .signatures
            .iter()
            .any(|witness| &witness.signer == signer)
        {
            return Err(iroha_crypto::Error::Other(
                "duplicate DA pin-scope authorization signer".to_owned(),
            ));
        }
        let signature = Signature::try_new(
            key_pair.private_key(),
            &self.scope.signing_digest(commitment_digest),
        )?;
        self.signatures.push(DaPinScopeSignatureV1 {
            signer: signer.clone(),
            signature,
        });
        self.signatures
            .sort_by(|left, right| left.signer.cmp(&right.signer));
        Ok(())
    }

    /// Return whether witnesses are non-empty, strictly ordered, and individually valid
    /// over this scope and the digest of its paired commitment.
    #[must_use]
    pub fn has_valid_canonical_signatures(&self, commitment_digest: &BlobDigest) -> bool {
        if self.signatures.is_empty()
            || self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return false;
        }
        let digest = self.scope.signing_digest(commitment_digest);
        self.signatures
            .iter()
            .all(|witness| witness.signature.verify(&witness.signer, &digest).is_ok())
    }
}
/// Summary of the 2D erasure layout captured in DA manifests/receipts.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    Default,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct DaStripeLayout {
    /// Total row stripes (data + column parity).
    pub total_stripes: u32,
    /// Total shards per stripe (data + row parity).
    pub shards_per_stripe: u32,
    /// Number of column-parity stripes across the matrix.
    pub row_parity_stripes: u16,
}
/// Norito payload accepted by the Torii `/v1/da/ingest` endpoint.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestRequest")]
pub struct DaIngestRequest {
    /// Exact genesis-derived network identity authorising this request.
    pub network_id: NetworkId,
    /// Authenticated account whose consensus DA quota is charged.
    pub owner: AccountId,
    /// Caller-supplied blob identifier (BLAKE3 digest or equivalent).
    pub client_blob_id: BlobDigest,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)` used for replay detection.
    pub sequence: u64,
    /// Semantic classification of the blob.
    pub blob_class: BlobClass,
    /// Codec label describing the payload.
    pub codec: BlobCodec,
    /// Erasure profile requested for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy requested/negotiated for the blob.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes (power-of-two, aligned with erasure profile).
    pub chunk_size: u32,
    /// Total payload size in bytes.
    pub total_size: u64,
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
    /// Compression applied to the payload.
    pub compression: Compression,
    /// Optional pre-generated Norito manifest supplied by the caller.
    #[norito(json = "crate::json_helpers::base64_vec::option")]
    #[norito(required)]
    pub norito_manifest: Option<Vec<u8>>,
    /// Raw payload bytes to be chunked and replicated.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub payload: Vec<u8>,
    /// Additional metadata entries for governance/analytics.
    pub metadata: ExtraMetadata,
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaIngestSignatureV1>,
    /// Post-ingest witnesses over the exact scope returned by Torii.
    ///
    /// This vector is empty on the prepare request and deliberately excluded
    /// from the primary request digest. A retry adds canonical scope witnesses
    /// without invalidating the original request authorization.
    pub pin_scope_signatures: Vec<DaPinScopeSignatureV1>,
}
/// Canonical version-one intent signed by a DA ingest submitter.
///
/// Signer witnesses live on [`DaIngestRequest`] so every controller key signs
/// one identical digest. Every request field that can affect admission,
/// storage, accounting, or the resulting manifest is committed.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestRequestIntentV1")]
pub struct DaIngestRequestIntentV1 {
    /// Exact genesis-derived network identity authorising this intent.
    pub network_id: NetworkId,
    /// Authenticated account whose consensus DA quota is charged.
    pub owner: AccountId,
    /// Caller-supplied blob identifier.
    pub client_blob_id: BlobDigest,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)`.
    pub sequence: u64,
    /// Semantic classification of the blob.
    pub blob_class: BlobClass,
    /// Codec label describing the payload.
    pub codec: BlobCodec,
    /// Erasure profile requested for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy requested for the blob.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes.
    pub chunk_size: u32,
    /// Canonical payload size in bytes.
    pub total_size: u64,
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
    /// Compression applied to the transported payload.
    pub compression: Compression,
    /// Optional caller-provided Norito manifest.
    #[norito(required)]
    pub norito_manifest: Option<Vec<u8>>,
    /// Transported payload bytes.
    pub payload: Vec<u8>,
    /// Additional governance and analytics metadata.
    pub metadata: ExtraMetadata,
}
#[derive(Clone, Copy)]
struct DaIngestRequestIntentRefV1<'a> {
    client_blob_id: &'a BlobDigest,
    blob_class: BlobClass,
    codec: &'a BlobCodec,
    erasure_profile: ErasureProfile,
    retention_policy: &'a RetentionPolicy,
    chunk_size: u32,
    compression: Compression,
    norito_manifest: &'a Option<Vec<u8>>,
    payload: &'a Vec<u8>,
    metadata: &'a ExtraMetadata,
}
impl<'a> From<&'a DaIngestRequestIntentV1> for DaIngestRequestIntentRefV1<'a> {
    fn from(intent: &'a DaIngestRequestIntentV1) -> Self {
        Self {
            client_blob_id: &intent.client_blob_id,
            blob_class: intent.blob_class,
            codec: &intent.codec,
            erasure_profile: intent.erasure_profile,
            retention_policy: &intent.retention_policy,
            chunk_size: intent.chunk_size,
            compression: intent.compression,
            norito_manifest: &intent.norito_manifest,
            payload: &intent.payload,
            metadata: &intent.metadata,
        }
    }
}
impl<'a> From<&'a DaIngestRequest> for DaIngestRequestIntentRefV1<'a> {
    fn from(request: &'a DaIngestRequest) -> Self {
        Self {
            client_blob_id: &request.client_blob_id,
            blob_class: request.blob_class,
            codec: &request.codec,
            erasure_profile: request.erasure_profile,
            retention_policy: &request.retention_policy,
            chunk_size: request.chunk_size,
            compression: request.compression,
            norito_manifest: &request.norito_manifest,
            payload: &request.payload,
            metadata: &request.metadata,
        }
    }
}
fn hash_len_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).expect("in-memory DA field length must fit into u64");
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
}
fn hash_tagged_u16(hasher: &mut blake3::Hasher, tag: u8, value: u16) {
    hasher.update(&[tag]);
    hasher.update(&value.to_le_bytes());
}
fn da_ingest_request_content_hash(intent: &DaIngestRequestIntentRefV1<'_>) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(DA_INGEST_REQUEST_CONTENT_DOMAIN_V1);
    hasher.update(intent.client_blob_id.as_bytes());
    match intent.blob_class {
        BlobClass::TaikaiSegment => hash_tagged_u16(&mut hasher, 0, 0),
        BlobClass::NexusLaneSidecar => hash_tagged_u16(&mut hasher, 1, 0),
        BlobClass::GovernanceArtifact => hash_tagged_u16(&mut hasher, 2, 0),
        BlobClass::Custom(value) => hash_tagged_u16(&mut hasher, 3, value),
    }
    hash_len_prefixed(&mut hasher, intent.codec.0.as_bytes());
    hasher.update(&intent.erasure_profile.data_shards.to_le_bytes());
    hasher.update(&intent.erasure_profile.parity_shards.to_le_bytes());
    hasher.update(&intent.erasure_profile.row_parity_stripes.to_le_bytes());
    hasher.update(&intent.erasure_profile.chunk_alignment.to_le_bytes());
    match intent.erasure_profile.fec_scheme {
        FecScheme::Rs12_10 => hash_tagged_u16(&mut hasher, 0, 0),
        FecScheme::RsWin14_10 => hash_tagged_u16(&mut hasher, 1, 0),
        FecScheme::Rs18_14 => hash_tagged_u16(&mut hasher, 2, 0),
        FecScheme::Custom(value) => hash_tagged_u16(&mut hasher, 3, value),
    }
    hasher.update(&intent.retention_policy.hot_retention_secs.to_le_bytes());
    hasher.update(&intent.retention_policy.cold_retention_secs.to_le_bytes());
    hasher.update(&intent.retention_policy.required_replicas.to_le_bytes());
    let storage_class = match intent.retention_policy.storage_class {
        StorageClass::Hot => 0,
        StorageClass::Warm => 1,
        StorageClass::Cold => 2,
    };
    hasher.update(&[storage_class]);
    hash_len_prefixed(
        &mut hasher,
        intent.retention_policy.governance_tag.0.as_bytes(),
    );
    hasher.update(&intent.chunk_size.to_le_bytes());
    let compression = match intent.compression {
        Compression::Identity => 0,
        Compression::Gzip => 1,
        Compression::Deflate => 2,
        Compression::Zstd => 3,
    };
    hasher.update(&[compression]);
    match intent.norito_manifest {
        Some(manifest) => {
            hasher.update(&[1]);
            hash_len_prefixed(&mut hasher, manifest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hash_len_prefixed(&mut hasher, intent.payload);
    let metadata_count =
        u64::try_from(intent.metadata.items.len()).expect("DA metadata count must fit into u64");
    hasher.update(&metadata_count.to_le_bytes());
    for entry in &intent.metadata.items {
        hash_len_prefixed(&mut hasher, entry.key.as_bytes());
        hash_len_prefixed(&mut hasher, &entry.value);
        let visibility = match entry.visibility {
            MetadataVisibility::Public => 0,
            MetadataVisibility::GovernanceOnly => 1,
        };
        hasher.update(&[visibility]);
        match &entry.encryption {
            MetadataEncryption::None => {
                hasher.update(&[0]);
            }
            MetadataEncryption::ChaCha20Poly1305(envelope) => {
                hasher.update(&[1]);
                match &envelope.key_label {
                    Some(label) => {
                        hasher.update(&[1]);
                        hash_len_prefixed(&mut hasher, label.as_bytes());
                    }
                    None => {
                        hasher.update(&[0]);
                    }
                }
            }
        }
    }
    Hash::prehashed(*hasher.finalize().as_bytes())
}
impl DaIngestRequestIntentV1 {
    fn unsigned_authorization(&self) -> DaIngestAuthorizationV1 {
        DaIngestAuthorizationV1 {
            network_id: self.network_id,
            owner: self.owner.clone(),
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            payload_hash: self.payload_hash,
            payload_bytes: self.total_size,
            request_content_hash: da_ingest_request_content_hash(&self.into()),
            signatures: Vec::new(),
        }
    }
    /// Compute the domain-separated digest signed by each account-controller key.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        self.unsigned_authorization().signing_digest()
    }
    /// Sign this intent and construct the corresponding ingest request.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured signing backend rejects the key or
    /// cannot create a signature.
    pub fn try_sign(self, key_pair: &KeyPair) -> Result<DaIngestRequest, iroha_crypto::Error> {
        let signature = Signature::try_new(key_pair.private_key(), &self.signing_digest())?;
        Ok(DaIngestRequest {
            network_id: self.network_id,
            owner: self.owner,
            client_blob_id: self.client_blob_id,
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            blob_class: self.blob_class,
            codec: self.codec,
            erasure_profile: self.erasure_profile,
            retention_policy: self.retention_policy,
            chunk_size: self.chunk_size,
            total_size: self.total_size,
            payload_hash: self.payload_hash,
            compression: self.compression,
            norito_manifest: self.norito_manifest,
            payload: self.payload,
            metadata: self.metadata,
            signatures: vec![DaIngestSignatureV1 {
                signer: key_pair.public_key().clone(),
                signature,
            }],
            pin_scope_signatures: Vec::new(),
        })
    }
}
impl DaIngestRequest {
    /// Project the compact immutable authorization committed into the pin-intent sidecar.
    #[must_use]
    pub fn authorization(&self) -> DaIngestAuthorizationV1 {
        DaIngestAuthorizationV1 {
            network_id: self.network_id,
            owner: self.owner.clone(),
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            payload_hash: self.payload_hash,
            payload_bytes: self.total_size,
            request_content_hash: da_ingest_request_content_hash(&self.into()),
            signatures: self.signatures.clone(),
        }
    }
    /// Compute the domain-separated digest covering every signable request field.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        self.authorization().signing_digest()
    }
    /// Add one account-controller witness and restore canonical signer ordering.
    ///
    /// # Errors
    ///
    /// Returns an error when signing fails or the signer is already present.
    pub fn try_add_signature(&mut self, key_pair: &KeyPair) -> Result<(), iroha_crypto::Error> {
        let signer = key_pair.public_key();
        if self
            .signatures
            .iter()
            .any(|witness| &witness.signer == signer)
        {
            return Err(iroha_crypto::Error::Other(
                "duplicate DA ingest authorization signer".to_owned(),
            ));
        }
        let signature = Signature::try_new(key_pair.private_key(), &self.signing_digest())?;
        self.signatures.push(DaIngestSignatureV1 {
            signer: signer.clone(),
            signature,
        });
        self.signatures
            .sort_by(|left, right| left.signer.cmp(&right.signer));
        Ok(())
    }
    /// Add one witness approving an exact scope proposal returned for this request.
    ///
    /// The witness covers the stored scope and the proposal's commitment digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the scope belongs to another request, signing
    /// fails, or the signer is already present.
    pub fn try_add_pin_scope_signature(
        &mut self,
        scope: &DaPinScopeProposalV1,
        key_pair: &KeyPair,
    ) -> Result<(), iroha_crypto::Error> {
        if !scope.matches_authorization(&self.authorization()) {
            return Err(iroha_crypto::Error::Other(
                "DA pin scope does not match the ingest request authorization".to_owned(),
            ));
        }
        let signer = key_pair.public_key();
        if self
            .pin_scope_signatures
            .iter()
            .any(|witness| &witness.signer == signer)
        {
            return Err(iroha_crypto::Error::Other(
                "duplicate DA pin-scope authorization signer".to_owned(),
            ));
        }
        let signature = Signature::try_new(key_pair.private_key(), &scope.signing_digest())?;
        self.pin_scope_signatures.push(DaPinScopeSignatureV1 {
            signer: signer.clone(),
            signature,
        });
        self.pin_scope_signatures
            .sort_by(|left, right| left.signer.cmp(&right.signer));
        Ok(())
    }

    /// Project the producer's witnesses onto an exact pin scope.
    #[must_use]
    pub fn pin_scope_authorization(&self, scope: DaPinScopeV1) -> DaPinScopeAuthorizationV1 {
        DaPinScopeAuthorizationV1 {
            scope,
            signatures: self.pin_scope_signatures.clone(),
        }
    }
    /// Verify that every request witness is canonical and cryptographically valid.
    ///
    /// # Errors
    ///
    /// Returns [`iroha_crypto::Error::BadSignature`] for an empty, duplicate,
    /// non-canonical, or invalid witness set.
    pub fn verify_signatures(&self) -> Result<(), iroha_crypto::Error> {
        if self.authorization().has_valid_canonical_signatures() {
            Ok(())
        } else {
            Err(iroha_crypto::Error::BadSignature)
        }
    }
}
/// Ingest receipt returned once Torii accepts the blob.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::da::ingest::DaIngestReceipt")]
pub struct DaIngestReceipt {
    /// Caller-supplied blob identifier echoed back to the submitter.
    pub client_blob_id: BlobDigest,
    /// Nexus lane associated with the blob.
    pub lane_id: LaneId,
    /// Epoch recorded for the blob.
    pub epoch: u64,
    /// Blake3 digest of the raw payload.
    pub blob_hash: BlobDigest,
    /// Merkle root computed from chunk commitments.
    pub chunk_root: BlobDigest,
    /// Blake3 digest of the canonical Norito manifest.
    pub manifest_hash: BlobDigest,
    /// Storage ticket identifier issued by the orchestrator.
    pub storage_ticket: StorageTicketId,
    /// Norito-encoded PDP commitment derived from the accepted payload.
    #[norito(json = "crate::json_helpers::base64_vec::option")]
    #[norito(required)]
    pub pdp_commitment: Option<Vec<u8>>,
    /// Erasure layout summary for the admitted manifest.
    pub stripe_layout: DaStripeLayout,
    /// Unix timestamp (seconds) when the blob was accepted.
    pub queued_at_unix: u64,
    /// Effective policy applied to the stored manifest and on-chain commitment.
    pub retention_policy: RetentionPolicy,
    /// Rent and incentive breakdown quoted at ingest time.
    pub rent_quote: DaRentQuote,
    /// Signature generated by the Torii DA service.
    pub operator_signature: Signature,
}

// The signed frame's declared schema, version, sequence and nonzero signature slot
// are shared by receipt producers, restart verification and offline pin authorizers.
#[derive(Debug, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::persistence::DaReceiptSigningPayload")]
struct DaReceiptSigningPayload {
    version: u16,
    sequence: u64,
    receipt: DaIngestReceipt,
}

impl DaIngestReceipt {
    /// Version of the operator-signed receipt preimage.
    ///
    /// Version 2 is the receipt that carries `retention_policy`. Version 1 signed the
    /// earlier 12-field receipt and is never produced or verified.
    pub const SIGNING_PAYLOAD_VERSION: u16 = 2;

    /// Canonical signature slot before the service signs a receipt or a local assignment.
    #[must_use]
    pub fn signature_placeholder() -> Signature {
        Signature::try_from_bytes(&[0xA5; 64]).expect("fixed nonzero DA signature placeholder")
    }

    /// Canonical framed bytes authenticated by the operator, including the request sequence.
    ///
    /// # Errors
    /// Returns the Norito encoding error if the receipt cannot be encoded.
    pub fn operator_signing_bytes(&self, sequence: u64) -> Result<Vec<u8>, norito::Error> {
        let mut receipt = self.clone();
        receipt.operator_signature = Self::signature_placeholder();
        norito::encode_canonical(&DaReceiptSigningPayload {
            version: Self::SIGNING_PAYLOAD_VERSION,
            sequence,
            receipt,
        })
    }

    /// Verify the complete receipt and sequence under an independently trusted service key.
    ///
    /// # Errors
    /// Returns an error for malformed signatures, encoding failure or an invalid signature.
    pub fn verify_operator_signature(
        &self,
        sequence: u64,
        operator: &PublicKey,
    ) -> Result<(), iroha_crypto::Error> {
        let signature = self.operator_signature.payload();
        let parsed = match operator.try_algorithm()? {
            iroha_crypto::Algorithm::Ed25519 => {
                Some(iroha_crypto::ed25519_parse_signature(signature))
            }
            iroha_crypto::Algorithm::MlDsa => {
                Some(iroha_crypto::mldsa65_parse_signature(signature))
            }
            _ => None,
        };
        if let Some(parsed) = parsed {
            parsed.map_err(|err| {
                iroha_crypto::Error::Other(format!(
                    "DA receipt signature material is malformed: {err}"
                ))
            })?;
        }
        let bytes = self.operator_signing_bytes(sequence).map_err(|err| {
            iroha_crypto::Error::Other(format!("failed to encode DA receipt: {err}"))
        })?;
        self.operator_signature.verify(operator, &bytes)
    }

    /// Check the receipt against the original payload and the complete record named by a
    /// pin-scope proposal. The caller also authenticates the scope against the producer's
    /// signed request.
    #[must_use]
    pub fn matches_request_and_pin_scope(
        &self,
        request: &DaIngestRequest,
        scope: &DaPinScopeProposalV1,
    ) -> bool {
        // The receipt's blob hash is the digest of the request's canonical payload.
        let same_payload = self.blob_hash == request.payload_hash;
        request
            .metadata
            .registry_alias()
            .is_ok_and(|alias| alias == scope.alias.as_deref())
            && self.client_blob_id == request.client_blob_id
            && self.lane_id == request.lane_id
            && self.epoch == request.epoch
            && same_payload
            && self.storage_ticket == scope.storage_ticket
            && self.manifest_hash.as_bytes() == scope.manifest_hash.as_bytes()
            && self
                .commitment_record(
                    request.sequence,
                    super::commitment::DaProofScheme::SorafsPorBlake3,
                )
                .pin_scope_digest()
                == scope.commitment_digest
    }

    /// Reconstruct the complete commitment authorized in the producer's pin scope.
    #[must_use]
    pub fn commitment_record(
        &self,
        sequence: u64,
        proof_scheme: super::commitment::DaProofScheme,
    ) -> super::commitment::DaCommitmentRecord {
        super::commitment::DaCommitmentRecord::new(
            self.lane_id,
            self.epoch,
            sequence,
            self.client_blob_id,
            ManifestDigest::new(*self.manifest_hash.as_bytes()),
            proof_scheme,
            self.chunk_root,
            self.pdp_commitment.as_deref().map(Hash::new),
            self.retention_policy.clone(),
            self.storage_ticket,
            self.operator_signature.clone(),
        )
    }

    /// Digest of the complete commitment this receipt proposes, as producers sign it in
    /// their pin scope ([`DaPinScopeV1::signing_digest`]).
    #[must_use]
    pub fn pin_scope_commitment_digest(&self, sequence: u64) -> BlobDigest {
        self.commitment_record(sequence, super::commitment::DaProofScheme::SorafsPorBlake3)
            .pin_scope_digest()
    }
}

/// Durable DA spool record written by Torii and read by Core block assembly.
///
/// Both consumers use this single declared frame owner. The embedded receipt
/// remains a payload field; its API frame is not a spool-record envelope.
///
/// Version 2 carries the receipt with `retention_policy`; its nominal identity
/// `iroha_torii::da::persistence::StoredDaReceiptV2` makes the Norito header declare
/// that layout. The retired version-1 frame (`iroha_torii::da::persistence::StoredDaReceipt`,
/// a 12-field receipt) is refused by its exact header identity in [`Self::decode_frame`];
/// it is never decoded.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::persistence::StoredDaReceiptV2")]
pub struct StoredDaReceipt {
    /// Exact supported spool payload version.
    pub version: u16,
    /// Monotonic sequence within the receipt's lane and epoch.
    pub sequence: u64,
    /// Signed ingest acknowledgement.
    pub receipt: DaIngestReceipt,
}
impl StoredDaReceipt {
    /// The only supported durable receipt payload version.
    pub const VERSION: u16 = 2;

    /// Nominal frame identity of the retired version-1 spool record.
    pub const RETIRED_V1_FRAME_NAME: &'static str = "iroha_torii::da::persistence::StoredDaReceipt";

    /// Decode one durable spool frame of the supported version.
    ///
    /// # Errors
    ///
    /// Returns [`StoredDaReceiptFrameError::RetiredVersion1`] for a frame whose header
    /// names the retired version-1 record, without decoding its body;
    /// [`StoredDaReceiptFrameError::UnsupportedVersion`] for any other version; and
    /// [`StoredDaReceiptFrameError::Codec`] for every malformed or foreign frame.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, StoredDaReceiptFrameError> {
        if norito::core::Header::read(bytes).is_ok_and(|header| {
            header.schema == norito::core::schema_hash_for_name(Self::RETIRED_V1_FRAME_NAME)
        }) {
            return Err(StoredDaReceiptFrameError::RetiredVersion1);
        }
        let stored = norito::decode_from_bytes::<Self>(bytes)?;
        if stored.version != Self::VERSION {
            return Err(StoredDaReceiptFrameError::UnsupportedVersion {
                found: stored.version,
                expected: Self::VERSION,
            });
        }
        Ok(stored)
    }
}

/// Refusal of a durable DA receipt spool frame.
#[derive(Debug, Error)]
pub enum StoredDaReceiptFrameError {
    /// The frame is the retired version-1 record written before receipts carried retention.
    #[error(
        "retired version-1 DA receipt frame (`iroha_torii::da::persistence::StoredDaReceipt`); \
         this build reads only version 2, so drain or archive the spool written by the earlier build"
    )]
    RetiredVersion1,
    /// The frame decoded but declares another payload version.
    #[error("unsupported DA receipt version {found} (expected {expected})")]
    UnsupportedVersion {
        /// Version carried by the frame.
        found: u16,
        /// The only supported version.
        expected: u16,
    },
    /// The frame is malformed, truncated, non-canonical or names another owner.
    #[error("malformed DA receipt frame: {0}")]
    Codec(#[from] norito::Error),
}

#[cfg(test)]
mod pin_scope_tests {
    use super::*;
    use crate::block::BlockHeader;
    use iroha_crypto::{Algorithm, HashOf};

    fn signed_authorization() -> (KeyPair, DaIngestAuthorizationV1) {
        let key_pair = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
            .expect("valid deterministic pin-scope key");
        let mut authorization = DaIngestAuthorizationV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x92; 32])),
            ),
            owner: AccountId::new(key_pair.public_key().clone()),
            lane_id: LaneId::new(3),
            epoch: 5,
            sequence: 7,
            payload_hash: BlobDigest::new([0x93; 32]),
            payload_bytes: 11,
            request_content_hash: Hash::prehashed([0x94; 32]),
            signatures: Vec::new(),
        };
        authorization.signatures.push(DaIngestSignatureV1 {
            signer: key_pair.public_key().clone(),
            signature: Signature::try_new(key_pair.private_key(), &authorization.signing_digest())
                .expect("sign deterministic ingest authorization"),
        });
        (key_pair, authorization)
    }

    const COMMITMENT: BlobDigest = BlobDigest::new([0xC7; 32]);

    #[test]
    fn pin_scope_preserves_every_authorization_digest_bit() {
        let (key_pair, mut authorization) = signed_authorization();
        let mut covered_low_bits = [false; 2];
        for sequence in 0..64 {
            authorization.sequence = sequence;
            let digest = authorization.signing_digest();
            authorization.signatures[0].signature =
                Signature::try_new(key_pair.private_key(), &digest)
                    .expect("sign the current authorization fixture");
            covered_low_bits[usize::from(digest[31] & 1)] = true;
            let scope = DaPinScopeV1::new(
                &authorization,
                StorageTicketId::new([0xA1; 32]),
                ManifestDigest::new([0xA2; 32]),
                None,
            );
            assert_eq!(scope.request_authorization_digest.as_bytes(), &digest);
            assert!(scope.matches_authorization(&authorization));

            let frame = norito::to_bytes(&scope).expect("encode exact pin scope");
            let decoded: DaPinScopeV1 = norito::decode_from_bytes(&frame)
                .expect("decode pin scope with either digest low bit");
            assert_eq!(decoded, scope);
            assert_eq!(
                norito::to_bytes(&decoded).expect("re-encode pin scope"),
                frame
            );

            {
                let json = norito::json::to_json(&scope).expect("encode pin scope JSON");
                let decoded: DaPinScopeV1 = norito::json::from_str(&json)
                    .expect("decode pin scope JSON with either digest low bit");
                assert_eq!(decoded, scope);
            }

            let mut signed = DaPinScopeAuthorizationV1::try_sign(scope, &COMMITMENT, &key_pair)
                .expect("sign exact pin scope");
            assert!(signed.has_valid_canonical_signatures(&COMMITMENT));
            signed.scope.request_authorization_digest.0[31] ^= 1;
            assert!(!signed.scope.matches_authorization(&authorization));
            assert!(!signed.has_valid_canonical_signatures(&COMMITMENT));
        }
        assert_eq!(covered_low_bits, [true, true]);
    }

    #[test]
    fn pin_scope_signatures_bind_ticket_manifest_and_alias() {
        let (key_pair, authorization) = signed_authorization();
        let scope = DaPinScopeV1::new(
            &authorization,
            StorageTicketId::new([0xA1; 32]),
            ManifestDigest::new([0xA2; 32]),
            Some("video.example".to_owned()),
        );
        assert!(scope.matches_authorization(&authorization));
        let signed = DaPinScopeAuthorizationV1::try_sign(scope, &COMMITMENT, &key_pair)
            .expect("sign deterministic pin scope");
        assert!(signed.has_valid_canonical_signatures(&COMMITMENT));

        let mut mutated_ticket = signed.clone();
        mutated_ticket.scope.storage_ticket = StorageTicketId::new([0xB1; 32]);
        assert!(!mutated_ticket.has_valid_canonical_signatures(&COMMITMENT));

        let mut mutated_manifest = signed.clone();
        mutated_manifest.scope.manifest_hash = ManifestDigest::new([0xB2; 32]);
        assert!(!mutated_manifest.has_valid_canonical_signatures(&COMMITMENT));

        let mut mutated_alias = signed;
        mutated_alias.scope.alias = Some("other.example".to_owned());
        assert!(!mutated_alias.has_valid_canonical_signatures(&COMMITMENT));
    }

    #[test]
    fn pin_scope_signatures_bind_the_supplied_commitment_digest() {
        let (key_pair, authorization) = signed_authorization();
        let scope = DaPinScopeV1::new(
            &authorization,
            StorageTicketId::new([0xA1; 32]),
            ManifestDigest::new([0xA2; 32]),
            Some("video.example".to_owned()),
        );
        let signed = DaPinScopeAuthorizationV1::try_sign(scope.clone(), &COMMITMENT, &key_pair)
            .expect("sign deterministic pin scope");
        let mut other = COMMITMENT;
        other.0[31] ^= 1;
        assert!(signed.has_valid_canonical_signatures(&COMMITMENT));
        assert!(!signed.has_valid_canonical_signatures(&other));
        assert_ne!(
            scope.signing_digest(&COMMITMENT),
            scope.signing_digest(&other)
        );

        // The proposal handed to producers signs the same digest, and its stored
        // projection is exactly the scope: the digest never enters the stored layout.
        let proposal = DaPinScopeProposalV1::new(scope.clone(), COMMITMENT);
        assert_eq!(proposal.scope(), scope);
        assert_eq!(proposal.signing_digest(), scope.signing_digest(&COMMITMENT));
        assert!(proposal.matches_authorization(&authorization));
        let offered = norito::encode_canonical(&proposal).expect("scope proposal");
        assert!(norito::decode_canonical::<DaPinScopeV1>(&offered).is_err());
        let json = norito::json::to_value(&proposal).expect("proposal JSON");
        let object = json.as_object().expect("proposal object");
        assert_eq!(
            object
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            [
                "network_id",
                "owner",
                "request_authorization_digest",
                "lane_id",
                "epoch",
                "sequence",
                "storage_ticket",
                "manifest_hash",
                "commitment_digest",
                "alias",
            ]
            .into_iter()
            .collect()
        );
        let decoded: DaPinScopeProposalV1 =
            norito::json::from_value(json).expect("decode proposal JSON");
        assert_eq!(decoded, proposal);
        let stored_json = norito::json::to_value(&scope).expect("scope JSON");
        assert!(
            !stored_json
                .as_object()
                .expect("scope object")
                .contains_key("commitment_digest")
        );
    }

    #[test]
    fn pin_scope_authorization_adds_canonical_witnesses() {
        let (key_pair, authorization) = signed_authorization();
        let second = KeyPair::try_from_seed(vec![0x95; 32], Algorithm::Ed25519)
            .expect("valid second pin-scope key");
        let scope = DaPinScopeV1::new(
            &authorization,
            StorageTicketId::new([0xA3; 32]),
            ManifestDigest::new([0xA4; 32]),
            None,
        );
        let mut signed = DaPinScopeAuthorizationV1::try_sign(scope, &COMMITMENT, &key_pair)
            .expect("sign deterministic pin scope");
        signed
            .try_add_signature(&COMMITMENT, &second)
            .expect("add second deterministic pin-scope witness");
        assert!(signed.has_valid_canonical_signatures(&COMMITMENT));
        assert!(signed.signatures[0].signer < signed.signatures[1].signer);
        assert!(signed.try_add_signature(&COMMITMENT, &second).is_err());
    }
}

#[cfg(test)]
mod admission_policy_tests {
    use super::*;
    use iroha_crypto::Algorithm;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn lane(
        lane_id: u32,
        incarnation: u8,
        producers: Vec<AccountId>,
        current_epoch: u64,
        grace_epoch: Option<u64>,
    ) -> DaIngestAdmissionLaneV1 {
        DaIngestAdmissionLaneV1 {
            lane_id: LaneId::new(lane_id),
            lane_incarnation: Hash::prehashed([incarnation; Hash::LENGTH]),
            producers,
            current_epoch,
            grace_epoch,
        }
    }

    fn initial_policy() -> DaIngestAdmissionPolicyV1 {
        DaIngestAdmissionPolicyV1 {
            version: DaIngestAdmissionPolicyV1::VERSION,
            revision: 1,
            expected_previous_policy_hash: None,
            lanes: vec![lane(0, 0xA1, vec![account(0x11)], 7, Some(6))],
        }
    }

    #[test]
    fn admission_policy_roundtrips_and_rejects_arbitrary_epochs() {
        let policy = initial_policy();
        policy
            .validate_transition(None)
            .expect("canonical initial policy");
        let custom = policy.clone().into_custom_parameter();
        let decoded = DaIngestAdmissionPolicyV1::from_custom_parameter(&custom)
            .expect("decode policy")
            .expect("matching custom parameter");
        assert_eq!(decoded, policy);
        let producer = account(0x11);
        let incarnation = Hash::prehashed([0xA1; Hash::LENGTH]);
        assert!(policy.authorizes(&producer, LaneId::new(0), incarnation, 7));
        assert!(policy.authorizes(&producer, LaneId::new(0), incarnation, 6));
        assert!(!policy.authorizes(&producer, LaneId::new(0), incarnation, 8));
        assert!(!policy.authorizes(&account(0x12), LaneId::new(0), incarnation, 7));
    }

    #[test]
    fn changed_lane_scope_must_advance_epoch() {
        let previous = initial_policy();
        let mut next = previous.clone();
        next.revision = 2;
        next.expected_previous_policy_hash = Some(previous.policy_hash());
        next.lanes[0].producers = vec![account(0x12)];
        assert!(matches!(
            next.validate_transition(Some(&previous)),
            Err(DaIngestAdmissionPolicyError::EpochDidNotAdvance { .. })
        ));
        next.lanes[0].current_epoch = 8;
        next.lanes[0].grace_epoch = Some(7);
        next.validate_transition(Some(&previous))
            .expect("producer rotation with an epoch advance");
    }

    #[test]
    fn incarnation_rotation_forbids_epoch_reuse_and_grace() {
        let previous = initial_policy();
        let mut next = previous.clone();
        next.revision = 2;
        next.expected_previous_policy_hash = Some(previous.policy_hash());
        next.lanes[0].lane_incarnation = Hash::prehashed([0xA2; Hash::LENGTH]);
        next.lanes[0].current_epoch = 8;
        next.lanes[0].grace_epoch = Some(7);
        assert!(matches!(
            next.validate_transition(Some(&previous)),
            Err(DaIngestAdmissionPolicyError::CrossIncarnationGrace { .. })
        ));
        next.lanes[0].grace_epoch = None;
        next.validate_transition(Some(&previous))
            .expect("incarnation rotation with a fresh epoch and no grace");
    }

    #[test]
    fn lane_tombstones_cannot_be_dropped_or_retain_capacity() {
        let previous = initial_policy();
        let mut tombstone = previous.clone();
        tombstone.revision = 2;
        tombstone.expected_previous_policy_hash = Some(previous.policy_hash());
        tombstone.lanes[0].producers.clear();
        tombstone.lanes[0].current_epoch = 8;
        tombstone.lanes[0].grace_epoch = None;
        tombstone
            .validate_transition(Some(&previous))
            .expect("bounded lane tombstone");
        assert!(!tombstone.retains(LaneId::new(0), 8));
        let dropped = DaIngestAdmissionPolicyV1 {
            version: DaIngestAdmissionPolicyV1::VERSION,
            revision: 3,
            expected_previous_policy_hash: Some(tombstone.policy_hash()),
            lanes: Vec::new(),
        };
        assert!(matches!(
            dropped.validate_transition(Some(&tombstone)),
            Err(DaIngestAdmissionPolicyError::PriorLaneDropped { .. })
        ));
    }

    #[test]
    fn terminal_epoch_is_rejected() {
        let mut policy = initial_policy();
        policy.lanes[0].current_epoch = u64::MAX;
        policy.lanes[0].grace_epoch = Some(u64::MAX - 1);
        assert_eq!(
            policy.validate_transition(None),
            Err(DaIngestAdmissionPolicyError::TerminalEpoch {
                lane_id: LaneId::new(0),
            })
        );
    }
}

#[cfg(test)]
mod captured_ingest_schema_tests;

#[cfg(test)]
mod stored_receipt_frame_tests {
    use super::*;

    pub(super) fn fixture(pdp_commitment: Option<Vec<u8>>) -> StoredDaReceipt {
        let signer = KeyPair::try_from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("deterministic signer");
        StoredDaReceipt {
            version: StoredDaReceipt::VERSION,
            sequence: 7,
            receipt: DaIngestReceipt {
                client_blob_id: BlobDigest::new([1; 32]),
                lane_id: LaneId::new(3),
                epoch: 5,
                blob_hash: BlobDigest::new([2; 32]),
                chunk_root: BlobDigest::new([3; 32]),
                manifest_hash: BlobDigest::new([4; 32]),
                storage_ticket: StorageTicketId::new([5; 32]),
                pdp_commitment,
                retention_policy: Default::default(),
                stripe_layout: DaStripeLayout::default(),
                queued_at_unix: 1234,
                rent_quote: DaRentQuote::default(),
                operator_signature: Signature::try_new(signer.private_key(), b"receipt fixture")
                    .expect("sign fixture"),
            },
        }
    }

    #[test]
    fn stored_receipt_has_one_shared_frame_owner_and_exact_roundtrip() {
        let stored = fixture(None);
        assert_eq!(
            <StoredDaReceipt as norito::NoritoSchema>::nominal_name(),
            "iroha_torii::da::persistence::StoredDaReceiptV2"
        );
        assert_eq!(
            <StoredDaReceipt as norito::NoritoSchema>::frame_name(),
            "iroha_torii::da::persistence::StoredDaReceiptV2"
        );
        let bytes = norito::encode_canonical(&stored).expect("shared receipt frame");
        // The version-2 writer's captured frames (`fixtures/da/stored_receipt_frames.v2.json`)
        // independently confirm this sole frame identity and the root/container frames.
        assert_eq!(
            norito::schema::identity::frame_hash::<StoredDaReceipt>(),
            norito::core::schema_hash_for_name("iroha_torii::da::persistence::StoredDaReceiptV2")
        );
        assert_eq!(
            bytes[6..22],
            norito::schema::identity::frame_hash::<StoredDaReceipt>()
        );
        let decoded: StoredDaReceipt = norito::decode_canonical(&bytes).expect("exact roundtrip");
        assert_eq!(decoded, stored);
        assert_eq!(
            StoredDaReceipt::decode_frame(&bytes).expect("supported spool frame"),
            stored
        );
        // The retired version-1 owner is refused by its exact header identity, before
        // any body decoding, even though this envelope and payload are otherwise valid.
        let retired_v1 = [
            0xf4, 0xb0, 0x55, 0xd4, 0xb6, 0xa0, 0x6e, 0xd2, 0xff, 0x40, 0xfb, 0x14, 0xe1, 0x40,
            0x0f, 0xb3,
        ];
        assert_eq!(
            norito::core::schema_hash_for_name(StoredDaReceipt::RETIRED_V1_FRAME_NAME),
            retired_v1
        );
        let mut relabelled = bytes.clone();
        relabelled[6..22].copy_from_slice(&retired_v1);
        assert!(matches!(
            StoredDaReceipt::decode_frame(&relabelled),
            Err(StoredDaReceiptFrameError::RetiredVersion1)
        ));
        let mut future = stored.clone();
        future.version = StoredDaReceipt::VERSION + 1;
        assert!(matches!(
            StoredDaReceipt::decode_frame(&norito::encode_canonical(&future).unwrap()),
            Err(StoredDaReceiptFrameError::UnsupportedVersion { found, expected })
                if found == StoredDaReceipt::VERSION + 1 && expected == StoredDaReceipt::VERSION
        ));
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode"),
            bytes
        );
        let receipt_frame = norito::encode_canonical(&stored.receipt).expect("API receipt frame");
        assert!(matches!(
            norito::decode_canonical::<StoredDaReceipt>(&receipt_frame),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::decode_canonical::<DaIngestReceipt>(&bytes),
            Err(norito::Error::SchemaMismatch)
        ));
        // The removed Core duplicate and the superseded model-path declaration
        // are different owners, never alternative spellings accepted on disk.
        for wrong_owner in [
            [
                0x91, 0xd1, 0xb4, 0x31, 0x94, 0x8e, 0x90, 0x5b, 0xb3, 0xb2, 0x88, 0xbe, 0x0a, 0x3e,
                0x09, 0x7b,
            ],
            [
                0x14, 0x48, 0x06, 0x51, 0x39, 0x29, 0xcb, 0xdf, 0xaf, 0xff, 0x96, 0x39, 0x90, 0x59,
                0x83, 0x40,
            ],
        ] {
            let mut substituted = bytes.clone();
            substituted[6..22].copy_from_slice(&wrong_owner);
            norito::core::from_bytes_view(&substituted)
                .expect("only the declared owner differs; envelope and payload remain valid");
            assert!(matches!(
                norito::decode_canonical::<StoredDaReceipt>(&substituted),
                Err(norito::Error::SchemaMismatch)
            ));
        }
        assert!(norito::decode_canonical::<StoredDaReceipt>(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(norito::decode_canonical::<StoredDaReceipt>(&trailing).is_err());
    }
}

#[cfg(test)]
mod stored_receipt_original_frame_tests;
