#[cfg(feature = "json")]
use crate::parameter::CustomParameter;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    da::types::{
        BlobClass, BlobCodec, BlobDigest, Compression, DaRentQuote, ErasureProfile, ExtraMetadata,
        FecScheme, MetadataEncryption, MetadataVisibility, RetentionPolicy, StorageTicketId,
    },
    nexus::LaneId,
    parameter::CustomParameterId,
    sorafs::pin_registry::{ManifestDigest, StorageClass},
};
use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
#[cfg(feature = "json")]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
    #[cfg(feature = "json")]
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
    #[cfg(feature = "json")]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DaIngestSignatureV1 {
    /// Account-controller key that produced the signature.
    pub signer: PublicKey,
    /// Signature over [`DaIngestAuthorizationV1::signing_digest`].
    pub signature: Signature,
}
/// One canonical account-controller signature over an exact DA pin scope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DaPinScopeV1 {
    /// Exact genesis-derived network identity of the original ingest authorization.
    pub network_id: NetworkId,
    /// Account that signed the original request and this exact pin scope.
    pub owner: AccountId,
    /// Digest signed by the original ingest authorization witnesses.
    pub request_authorization_digest: Hash,
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
            request_authorization_digest: Hash::prehashed(authorization.signing_digest()),
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
            && *self.request_authorization_digest.as_ref() == authorization.signing_digest()
            && self.lane_id == authorization.lane_id
            && self.epoch == authorization.epoch
            && self.sequence == authorization.sequence
    }

    /// Compute the domain-separated digest signed by every pin-scope witness.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
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
/// Producer authorization over one exact durable DA pin scope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DaPinScopeAuthorizationV1 {
    /// Exact scope approved by the producer.
    pub scope: DaPinScopeV1,
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaPinScopeSignatureV1>,
}
impl DaPinScopeAuthorizationV1 {
    /// Sign one exact scope with an account-controller key.
    ///
    /// # Errors
    ///
    /// Returns an error when the signing backend rejects the key.
    pub fn try_sign(scope: DaPinScopeV1, key_pair: &KeyPair) -> Result<Self, iroha_crypto::Error> {
        let signature = Signature::try_new(key_pair.private_key(), &scope.signing_digest())?;
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
    pub fn try_add_signature(&mut self, key_pair: &KeyPair) -> Result<(), iroha_crypto::Error> {
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
        let signature = Signature::try_new(key_pair.private_key(), &self.scope.signing_digest())?;
        self.signatures.push(DaPinScopeSignatureV1 {
            signer: signer.clone(),
            signature,
        });
        self.signatures
            .sort_by(|left, right| left.signer.cmp(&right.signer));
        Ok(())
    }

    /// Return whether witnesses are non-empty, strictly ordered, and individually valid.
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
        let digest = self.scope.signing_digest();
        self.signatures
            .iter()
            .all(|witness| witness.signature.verify(&witness.signer, &digest).is_ok())
    }
}
/// Summary of the 2D erasure layout captured in DA manifests/receipts.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[norito(deny_unknown_fields)]
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
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::base64_vec::option")
    )]
    #[norito(required)]
    pub norito_manifest: Option<Vec<u8>>,
    /// Raw payload bytes to be chunked and replicated.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
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
    #[norito(default)]
    pub pin_scope_signatures: Vec<DaPinScopeSignatureV1>,
}
/// Canonical version-one intent signed by a DA ingest submitter.
///
/// Signer witnesses live on [`DaIngestRequest`] so every controller key signs
/// one identical digest. Every request field that can affect admission,
/// storage, accounting, or the resulting manifest is committed.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
    /// Add one witness approving an exact scope returned for this request.
    ///
    /// # Errors
    ///
    /// Returns an error when the scope belongs to another request, signing
    /// fails, or the signer is already present.
    pub fn try_add_pin_scope_signature(
        &mut self,
        scope: &DaPinScopeV1,
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[norito(deny_unknown_fields)]
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
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::base64_vec::option")
    )]
    #[norito(required)]
    pub pdp_commitment: Option<Vec<u8>>,
    /// Erasure layout summary for the admitted manifest.
    pub stripe_layout: DaStripeLayout,
    /// Unix timestamp (seconds) when the blob was accepted.
    pub queued_at_unix: u64,
    /// Rent and incentive breakdown quoted at ingest time.
    pub rent_quote: DaRentQuote,
    /// Signature generated by the Torii DA service.
    pub operator_signature: Signature,
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
        let signed = DaPinScopeAuthorizationV1::try_sign(scope, &key_pair)
            .expect("sign deterministic pin scope");
        assert!(signed.has_valid_canonical_signatures());

        let mut mutated_ticket = signed.clone();
        mutated_ticket.scope.storage_ticket = StorageTicketId::new([0xB1; 32]);
        assert!(!mutated_ticket.has_valid_canonical_signatures());

        let mut mutated_manifest = signed.clone();
        mutated_manifest.scope.manifest_hash = ManifestDigest::new([0xB2; 32]);
        assert!(!mutated_manifest.has_valid_canonical_signatures());

        let mut mutated_alias = signed;
        mutated_alias.scope.alias = Some("other.example".to_owned());
        assert!(!mutated_alias.has_valid_canonical_signatures());
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
        let mut signed = DaPinScopeAuthorizationV1::try_sign(scope, &key_pair)
            .expect("sign deterministic pin scope");
        signed
            .try_add_signature(&second)
            .expect("add second deterministic pin-scope witness");
        assert!(signed.has_valid_canonical_signatures());
        assert!(signed.signatures[0].signer < signed.signatures[1].signer);
        assert!(signed.try_add_signature(&second).is_err());
    }
}

#[cfg(all(test, feature = "json"))]
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
