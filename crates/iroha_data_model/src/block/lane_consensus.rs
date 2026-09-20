//! Native lane consensus wire values and immutable authority commitments.
//!
//! Decoding, shape validation, PoP verification and hashing do not authenticate
//! finality or current membership. State owns that boundary; Core owns the shared
//! reducer, signature authentication, readiness and physical WAL custody.
//! No type in this module grants permission to sign or apply a value.
//!
//! Schema identities use their single canonical data-model owner. This is an
//! intentional first-release identity update: earlier goal-checkpoint wire/WAL
//! bytes cannot be resumed by this candidate. Semantic signing/hash domains
//! retain their distinct roles; no legacy decoder or signature translation exists.

use super::{consensus_v2 as wire, execution_context::MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK};
use crate::{NetworkId, nexus::MAX_ACTIVE_EXECUTION_LANES};
use iroha_crypto::Hash;
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Invalid structural carrier position, without any admission/finality verdict.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct LaneAdmissionPriorityError(
    /// Structural rejection detail.
    pub String,
);

/// Total order of first canonical QueuePlan admission within one network.
///
/// The carrier owns this position; the original signed ingress binding does not.
/// It is immutable when the same certificate is carried again. Structural
/// validity alone is not evidence of finalized admission or permission to vote.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::QueuePlanAdmissionPriorityV1")]
pub struct QueuePlanAdmissionPriorityV1 {
    /// Actual global carrier of the first successful registry insertion.
    pub carrier_height: u64,
    /// Zero-based position in that carrier's strict registry-key-ordered controls.
    pub admission_index: u32,
}

impl QueuePlanAdmissionPriorityV1 {
    /// Check the protocol bounds of a carrier-derived position.
    pub fn new(
        carrier_height: u64,
        admission_index: usize,
    ) -> Result<Self, LaneAdmissionPriorityError> {
        if carrier_height == 0 || admission_index >= MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK {
            return Err(LaneAdmissionPriorityError(
                "QueuePlan admission priority has an invalid carrier height or index".to_owned(),
            ));
        }
        let admission_index = u32::try_from(admission_index).map_err(|_| {
            LaneAdmissionPriorityError("QueuePlan admission priority index exceeds u32".to_owned())
        })?;
        Ok(Self {
            carrier_height,
            admission_index,
        })
    }

    /// Reject a zero carrier or an out-of-range position.
    pub fn validate(self) -> Result<(), LaneAdmissionPriorityError> {
        let index = usize::try_from(self.admission_index).map_err(|_| {
            LaneAdmissionPriorityError(
                "QueuePlan admission priority index exceeds usize".to_owned(),
            )
        })?;
        Self::new(self.carrier_height, index).map(|_| ())
    }
}

const CONTEXT_HASH_DOMAIN: &[u8] = b"iroha:lane-consensus:frozen-context:v1\0";
const CONTEXTS_HASH_DOMAIN: &[u8] = b"iroha:lane-consensus:open-contexts:v1\0";

/// One lane slot's immutable, post-carrier authority projection.
///
/// The opening carrier's own subject/hash is deliberately absent: this value is
/// committed by that carrier. A native consumer must separately authenticate
/// finality and exact inclusion before deriving a reducer instance identity.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1")]
pub struct FrozenLaneConsensusContextV1 {
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Exact consensus protocol revision.
    pub protocol_version: u16,
    /// Global carrier whose post-state opened this instance.
    pub opening_global_height: u64,
    /// Authenticated global height context used by the opening carrier.
    pub opening_global_context_id: wire::HeightContextId,
    /// Exact oldest unresolved admitted atomic group pinned by this instance.
    pub admitted_binding_hash: Hash,
    /// First canonical carrier position of the pinned group.
    pub admission_priority: QueuePlanAdmissionPriorityV1,
    /// Frozen epoch label; later global epochs do not retag this instance.
    pub epoch: u64,
    /// Frozen committee selection mode, with one vote per validator.
    pub mode: wire::ConsensusMode,
    /// Exact lane route.
    pub lane_id: LaneId,
    /// Dataspace bound to the route.
    pub dataspace_id: DataSpaceId,
    /// Exact lifecycle incarnation.
    pub lane_incarnation: Hash,
    /// Contiguous lane height owned by this instance.
    pub next_lane_height: u64,
    /// Canonically applied predecessor lane height, or zero for an empty frontier.
    pub predecessor_height: u64,
    /// Exact predecessor descriptor identity, absent only for the empty frontier.
    #[norito(required)]
    pub predecessor_hash: Option<Hash>,
    /// Actual predecessor application carrier height, zero only for an empty frontier.
    pub predecessor_applied_global_height: u64,
    /// Strictly ordered exact `3f + 1` committee.
    pub committee: Vec<PeerId>,
    /// Native BLS proofs aligned one-for-one with the complete committee.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Frozen Nexus/AMX policy commitment.
    pub nexus_amx_context_hash: Hash,
    /// Frozen deterministic execution policy commitment.
    pub execution_policy_hash: Hash,
    /// Mandatory signed RS16 layout.
    pub da_layout: wire::DataAvailabilityLayout,
    /// Frozen deterministic leader rotation seed.
    pub leader_seed: [u8; Hash::LENGTH],
}

/// Complete bounded open-instance set committed by one global carrier.
///
/// Empty is an explicit state. Snapshot deserialization must require the field;
/// [`Default`] is only a constructor for a newly initialized empty store.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneConsensusContextsV1")]
pub struct LaneConsensusContextsV1 {
    /// Strict route order, with at most one open context for each active lane.
    pub contexts: Vec<FrozenLaneConsensusContextV1>,
}

/// Rejection of malformed frozen authority; this is not a finality verdict.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LaneConsensusContextError {
    /// The stored revision is not the single supported production protocol.
    #[error("unsupported frozen lane protocol version {0}")]
    ProtocolVersion(u16),
    /// An exact identity uses the reserved zero-hash sentinel.
    #[error("frozen lane {0} identity is zero")]
    ZeroIdentity(&'static str),
    /// No finalized carrier can exist at height zero.
    #[error("frozen lane opening global height is zero")]
    OpeningHeight,
    /// The next slot, predecessor identity, or application anchor is inconsistent.
    #[error("frozen lane predecessor is absent, noncontiguous, or after its opening carrier")]
    Predecessor,
    /// The selected group's first admission must belong to the frozen opening cut.
    #[error("frozen lane admission priority is invalid or after the opening carrier")]
    AdmissionPriority,
    /// Roster count, order, or uniqueness violates exact production geometry.
    #[error("frozen lane committee is not a strictly ordered bounded exact 3f+1 roster")]
    Committee,
    /// PoP count or individual encoded proof length exceeds the native bounds.
    #[error("frozen lane proof-of-possession alignment or size is invalid")]
    ProofShape,
    /// The native proof verifier rejected a key or its aligned proof.
    #[error("frozen lane proof-of-possession verification failed: {0}")]
    ProofVerification(String),
    /// Signed layout or execution-policy structure is invalid.
    #[error("frozen lane signed policy or RS16 layout is invalid: {0}")]
    Policy(String),
    /// The complete set exceeds the existing active execution-lane bound.
    #[error("frozen lane context count {0} exceeds the active execution-lane bound")]
    TooManyContexts(usize),
    /// Ordering, duplicate routes, or multiple incarnations for one lane are invalid.
    #[error("frozen lane context set is unordered or repeats a lane")]
    ContextOrder,
    /// A full snapshot cannot combine different networks.
    #[error("frozen lane context set contains different networks")]
    MixedNetworks,
    /// Canonical Norito encoding failed.
    #[error("frozen lane context canonical encoding failed: {0}")]
    Encoding(String),
}

impl FrozenLaneConsensusContextV1 {
    /// Canonical route/incarnation ordering key.
    pub fn route_key(&self) -> (LaneId, DataSpaceId, Hash) {
        (self.lane_id, self.dataspace_id, self.lane_incarnation)
    }

    /// Validate structure and native PoPs without granting finality authority.
    pub fn validate(&self) -> Result<(), LaneConsensusContextError> {
        if self.protocol_version != wire::PROTOCOL_VERSION {
            return Err(LaneConsensusContextError::ProtocolVersion(
                self.protocol_version,
            ));
        }
        if self.opening_global_height == 0 {
            return Err(LaneConsensusContextError::OpeningHeight);
        }
        let zero = Hash::prehashed([0; Hash::LENGTH]);
        for (name, identity) in [
            ("network", self.network_id.as_bytes()),
            ("opening context", self.opening_global_context_id.0.as_ref()),
            ("admitted binding", self.admitted_binding_hash.as_ref()),
            ("incarnation", self.lane_incarnation.as_ref()),
            ("Nexus/AMX", self.nexus_amx_context_hash.as_ref()),
            ("execution policy", self.execution_policy_hash.as_ref()),
        ] {
            if identity == zero.as_ref() || *identity == [0; Hash::LENGTH] {
                return Err(LaneConsensusContextError::ZeroIdentity(name));
            }
        }
        if self.admission_priority.validate().is_err()
            || self.admission_priority.carrier_height > self.opening_global_height
        {
            return Err(LaneConsensusContextError::AdmissionPriority);
        }
        if self.predecessor_height.checked_add(1) != Some(self.next_lane_height)
            || match (self.predecessor_height, self.predecessor_hash) {
                (0, None) => self.predecessor_applied_global_height != 0,
                (0, Some(_)) | (_, None) => true,
                (_, Some(hash)) => {
                    hash == zero
                        || self.predecessor_applied_global_height == 0
                        || self.predecessor_applied_global_height > self.opening_global_height
                }
            }
        {
            return Err(LaneConsensusContextError::Predecessor);
        }
        if !wire::is_valid_committee_size(self.committee.len())
            || self.committee.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(LaneConsensusContextError::Committee);
        }
        if self.validator_set_pops.len() != self.committee.len()
            || self.validator_set_pops.iter().any(|proof| {
                proof.is_empty() || proof.len() > wire::finality::MAX_VALIDATOR_POP_BYTES
            })
        {
            return Err(LaneConsensusContextError::ProofShape);
        }
        wire::SumeragiV2GenesisContextParameters {
            da_layout: self.da_layout,
            nexus_amx_context_hash: *self.nexus_amx_context_hash.as_ref(),
            execution_policy_hash: *self.execution_policy_hash.as_ref(),
        }
        .validate()
        .map_err(|error| LaneConsensusContextError::Policy(error.to_string()))?;
        let roster = self
            .committee
            .iter()
            .cloned()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        wire::finality::verify_validator_power_roster_pops(&roster, &self.validator_set_pops)
            .map_err(|error| LaneConsensusContextError::ProofVerification(error.to_string()))
    }

    /// Return the exact `2f + 1` quorum after validation.
    pub fn minimum_signer_count(&self) -> Result<usize, LaneConsensusContextError> {
        self.validate()?;
        Ok(2 * ((self.committee.len() - 1) / 3) + 1)
    }

    /// Hash every canonical frozen field; this does not authenticate an opening.
    pub fn canonical_hash(&self) -> Result<Hash, LaneConsensusContextError> {
        self.validate()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|error| LaneConsensusContextError::Encoding(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[CONTEXT_HASH_DOMAIN, &bytes]))
    }
}

impl LaneConsensusContextsV1 {
    /// Validate an already canonically ordered set; never silently sort it.
    pub fn new(
        contexts: Vec<FrozenLaneConsensusContextV1>,
    ) -> Result<Self, LaneConsensusContextError> {
        let value = Self { contexts };
        value.validate()?;
        Ok(value)
    }

    /// Validate the complete set and every native committee without proving finality.
    pub fn validate(&self) -> Result<(), LaneConsensusContextError> {
        if self.contexts.len() > MAX_ACTIVE_EXECUTION_LANES {
            return Err(LaneConsensusContextError::TooManyContexts(
                self.contexts.len(),
            ));
        }
        if self.contexts.windows(2).any(|pair| {
            pair[0].route_key() >= pair[1].route_key() || pair[0].lane_id == pair[1].lane_id
        }) {
            return Err(LaneConsensusContextError::ContextOrder);
        }
        if self.contexts.first().is_some_and(|first| {
            self.contexts
                .iter()
                .any(|context| context.network_id != first.network_id)
        }) {
            return Err(LaneConsensusContextError::MixedNetworks);
        }
        self.contexts
            .iter()
            .try_for_each(FrozenLaneConsensusContextV1::validate)
    }

    /// Commit the complete canonical set, including the explicit empty state.
    pub fn canonical_hash(&self) -> Result<Hash, LaneConsensusContextError> {
        self.validate()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|error| LaneConsensusContextError::Encoding(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[CONTEXTS_HASH_DOMAIN, &bytes]))
    }
}

/// Native lane message-envelope revision.
pub const LANE_MESSAGE_VERSION_V1: u16 = 1;
const AVAILABILITY_DOMAIN: &[u8] = b"iroha:lane-reducer:availability:v1\0";
const VALUE_DOMAIN: &[u8] = b"iroha:lane-reducer:value:v1\0";
const VOTE_DOMAIN: &[u8] = b"iroha:lane-reducer:vote:v1\0";
const TIMEOUT_DOMAIN: &[u8] = b"iroha:lane-reducer:timeout:v1\0";
const PROPOSAL_DOMAIN: &[u8] = b"iroha:lane-reducer:proposal:v1\0";

/// Native shape/encoding rejection, without a finality or signature verdict.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid native lane wire value: {0}")]
pub struct LaneWireError(
    /// Native shape or canonical-codec rejection detail.
    pub String,
);

fn bad(reason: impl ToString) -> LaneWireError {
    LaneWireError(reason.to_string())
}
fn preimage<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<Vec<u8>, LaneWireError> {
    let frame = norito::encode_canonical(value).map_err(bad)?;
    let mut bytes = Vec::with_capacity(domain.len() + frame.len());
    bytes.extend_from_slice(domain);
    bytes.extend(frame);
    Ok(bytes)
}

/// Voting round within one immutable authenticated opening instance.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneRoundV1")]
pub struct LaneRoundV1 {
    /// Authenticated opening instance identity; raw bytes are not an authority token.
    pub instance_id: Hash,
    /// Exact lane slot height within the instance.
    pub lane_height: u64,
    /// Voting round; independent from the immutable value origin.
    pub voting_view: u64,
}

/// Role of the one pinned admitted group at this route.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneValueKindV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LaneValueKindV1 {
    /// An independently admitted economic input.
    Execution,
    /// Coordinator/participant roles at the same route must share this value.
    AtomicGroup,
}

/// Immutable value reference; reproposing it never rewrites its origin.
///
/// The referenced canonical body contains the full exact group route/slot list
/// and zero-effect participant controls. Those inputs must be available without
/// waiting for another route's decision; the global carrier joins decisions later.
/// Ready validation must authenticate all-route head eligibility and check these hashes against those bytes before voting.
/// Economic execution base/results are committed later by the global merge
/// candidate. A lane Decision does not promise successful economic execution.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneValueRefV1")]
pub struct LaneValueRefV1 {
    /// Authenticated opening instance identity; raw bytes are not an authority token.
    pub instance_id: Hash,
    /// Exact State-admitted group pinned by the frozen instance.
    pub admitted_binding_hash: Hash,
    /// Execution or atomic-group role of the canonical input.
    pub kind: LaneValueKindV1,
    /// Round in which the immutable value was first produced.
    pub origin_view: u64,
    /// Index of the original producer in the frozen committee.
    pub origin_producer: u32,
    /// Hash of the canonical immutable input descriptor.
    pub descriptor_hash: Hash,
    /// Hash of the exact canonical input payload.
    pub payload_hash: Hash,
    /// Hash of the complete immutable RS16 layout, root, length and count.
    /// Every Prepare/Commit statement commits this before a manifest is fetched.
    pub availability_hash: Hash,
}

impl LaneValueRefV1 {
    /// Commit every immutable native value field without granting authority.
    pub fn subject_hash(&self) -> Result<Hash, LaneWireError> {
        Ok(Hash::new(preimage(VALUE_DOMAIN, self)?))
    }
}

/// Signed RS16 manifest for the same stable value in every voting round.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneManifestV1")]
pub struct LaneManifestV1 {
    /// Immutable native input reference.
    pub value: LaneValueRefV1,
    /// Mandatory signed RS16 layout from the frozen context.
    pub layout: wire::DataAvailabilityLayout,
    /// Root of the exact encoded payload chunks.
    pub chunk_root: Hash,
    /// Length of the canonical unencoded payload.
    pub byte_len: u64,
    /// Exact chunk count derived from the signed RS16 layout.
    pub chunk_count: u32,
}

/// Commit the exact bounded RS16 manifest fields independently of a value.
///
/// A builder computes this before the value subject; no recursive hash or
/// signature is needed. This checks geometry, not chunk contents or authority.
///
/// # Errors
/// Rejects an invalid signed layout, zero root or inconsistent length/count.
pub fn lane_availability_hash(
    layout: wire::DataAvailabilityLayout,
    chunk_root: Hash,
    byte_len: u64,
    chunk_count: u32,
) -> Result<Hash, LaneWireError> {
    let expected = wire::expected_encoded_chunk_count(byte_len, layout).map_err(bad)?;
    if chunk_count != expected || chunk_root == Hash::prehashed([0; Hash::LENGTH]) {
        return Err(bad("manifest differs from exact signed RS16 geometry"));
    }
    Ok(Hash::new(preimage(
        AVAILABILITY_DOMAIN,
        &(layout, chunk_root, byte_len, chunk_count),
    )?))
}

impl LaneManifestV1 {
    /// Require the value's signed availability commitment to match this manifest.
    /// Geometry/hash equality grants neither body readiness nor signature authority.
    pub fn validate_availability(&self) -> Result<(), LaneWireError> {
        let expected = lane_availability_hash(
            self.layout,
            self.chunk_root,
            self.byte_len,
            self.chunk_count,
        )?;
        if self.value.availability_hash != expected {
            return Err(bad(
                "manifest differs from the voted availability commitment",
            ));
        }
        Ok(())
    }
}

/// Native Prepare/Commit phase.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LanePhaseV1")]
#[norito(
    tag = "phase",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LanePhaseV1 {
    /// Prepare evidence for the value at this voting round.
    Prepare,
    /// Commit evidence deciding the prepared value.
    Commit,
}

/// One common statement; signer identity is authenticated by its BLS key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneVoteStatementV1")]
pub struct LaneVoteStatementV1 {
    /// Exact instance, slot and voting round.
    pub round: LaneRoundV1,
    /// Prepare or Commit signing domain discriminator.
    pub phase: LanePhaseV1,
    /// Immutable native input reference.
    pub value: LaneValueRefV1,
}

/// One frozen-roster native signature.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneSignatureShareV1")]
pub struct LaneSignatureShareV1 {
    /// Index in the exact frozen committee.
    pub signer: u32,
    /// Native BLS-normal signature bytes.
    pub signature: Vec<u8>,
}

/// A signed native Prepare or Commit statement.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneVoteV1")]
pub struct LaneVoteV1 {
    /// Common native statement authenticated by every share.
    pub statement: LaneVoteStatementV1,
    /// One exact signer and its native signature.
    pub share: LaneSignatureShareV1,
}

/// Replay-complete exact quorum; individual signatures avoid an aggregate-token
/// registry in this first bounded draft. They remain real BLS-normal signatures.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneQcV1")]
pub struct LaneQcV1 {
    /// Common native statement authenticated by every share.
    pub statement: LaneVoteStatementV1,
    /// Strictly ordered distinct shares containing exactly 2f+1 signers.
    pub shares: Vec<LaneSignatureShareV1>,
}

/// Durable timeout statement with complete highest-Prepare evidence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneTimeoutBodyV1")]
pub struct LaneTimeoutBodyV1 {
    /// Exact instance, slot and voting round.
    pub round: LaneRoundV1,
    /// Full native certificate evidence, including exact signers/signatures.
    #[norito(required)]
    pub highest_prepare: Option<LaneQcV1>,
}

impl LaneTimeoutBodyV1 {
    /// Sign the stable Prepare statement, excluding an incidental QC signer set.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, LaneWireError> {
        preimage(
            TIMEOUT_DOMAIN,
            &(
                self.round,
                self.highest_prepare.as_ref().map(|qc| qc.statement.clone()),
            ),
        )
    }
}

/// Signed native timeout statement.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneTimeoutVoteV1")]
pub struct LaneTimeoutVoteV1 {
    /// Native signed statement body.
    pub body: LaneTimeoutBodyV1,
    /// One exact signer and its native signature.
    pub share: LaneSignatureShareV1,
}

/// Votes are strictly ordered by signer and contain exactly the native quorum.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneTcV1")]
pub struct LaneTcV1 {
    /// Exact instance, slot and voting round.
    pub round: LaneRoundV1,
    /// Strictly ordered distinct timeout voters containing exactly 2f+1 signers.
    pub votes: Vec<LaneTimeoutVoteV1>,
}

/// Proposal justification for one shared-reducer voting round.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneJustificationV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LaneJustificationV1 {
    /// View zero opens from the authenticated external State anchor.
    Opening,
    /// Complete exact previous-round timeout certificate.
    Timeout(LaneTcV1),
}

/// Unsigned proposal body persisted before native signing.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneProposalBodyV1")]
pub struct LaneProposalBodyV1 {
    /// Exact instance, slot and voting round.
    pub round: LaneRoundV1,
    /// Current voting-round leader index.
    pub proposer: u32,
    /// Exact value and mandatory RS16 availability description.
    pub manifest: LaneManifestV1,
    /// Opening authority or authenticated previous-round Timeout certificate.
    pub justification: LaneJustificationV1,
}

/// Signed proposal carrying the complete RS16 manifest.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneProposalV1")]
pub struct LaneProposalV1 {
    /// Native signed statement body.
    pub body: LaneProposalBodyV1,
    /// Native BLS-normal signature bytes.
    pub signature: Vec<u8>,
}

/// Canonical native lane consensus message.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneMessageV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LaneMessageV1 {
    /// Signed native proposal.
    Proposal(LaneProposalV1),
    /// Signed Prepare or Commit vote.
    Vote(LaneVoteV1),
    /// Exact native Prepare or Commit quorum.
    QuorumCertificate(LaneQcV1),
    /// Signed timeout with complete highest-Prepare evidence.
    TimeoutVote(LaneTimeoutVoteV1),
    /// Exact native timeout quorum.
    TimeoutCertificate(LaneTcV1),
}

/// Versioned canonical native message envelope.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneMessageEnvelopeV1")]
pub struct LaneMessageEnvelopeV1 {
    /// Native lane envelope revision; only revision one is supported.
    pub version: u16,
    /// One native lane protocol message.
    pub message: LaneMessageV1,
}

/// Replay-complete native decision over one immutable admitted input.
///
/// The Commit QC authenticates the value; a separate Prepare QC or origin
/// proposal is not a decision-availability prerequisite. Prepare evidence stays
/// with the reducer's durable lock/recovery owner where required.
///
/// This is untrusted evidence, not an authority capability. TODO: replace the
/// old MergeLaneExecution authority fields with this value only when the native
/// finalized-context proof owner and exact canonical input/body verifier are
/// wired through Kura, merge validation and snapshot recovery. Until then,
/// callers must not accept this DTO as independently portable finality proof.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_consensus::LaneDecisionV1")]
pub struct LaneDecisionV1 {
    /// Exact immutable value and mandatory signed RS16 layout.
    pub manifest: LaneManifestV1,
    /// Exactly 2f+1 native Commit shares over this same immutable value.
    pub commit_qc: LaneQcV1,
}

impl LaneVoteStatementV1 {
    /// Native signing bytes; includes phase and voting round, excluding signer.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, LaneWireError> {
        preimage(VOTE_DOMAIN, self)
    }
}

impl LaneProposalBodyV1 {
    /// Native proposal signing bytes, including manifest and timeout evidence.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, LaneWireError> {
        preimage(PROPOSAL_DOMAIN, self)
    }
}

fn committee_quorum(committee_len: usize) -> Result<usize, LaneWireError> {
    if !wire::is_valid_committee_size(committee_len) {
        return Err(bad("committee is not a bounded exact 3f+1 roster"));
    }
    Ok(2 * ((committee_len - 1) / 3) + 1)
}

fn round_shape(round: LaneRoundV1) -> Result<(), LaneWireError> {
    if round.instance_id == Hash::prehashed([0; Hash::LENGTH]) || round.lane_height == 0 {
        return Err(bad("zero instance or lane height"));
    }
    Ok(())
}

fn value_shape(
    value: &LaneValueRefV1,
    round: LaneRoundV1,
    committee_len: usize,
) -> Result<(), LaneWireError> {
    round_shape(round)?;
    if value.instance_id != round.instance_id
        || value.origin_view > round.voting_view
        || value.origin_producer as usize >= committee_len
        || [
            value.admitted_binding_hash,
            value.descriptor_hash,
            value.payload_hash,
            value.availability_hash,
        ]
        .contains(&Hash::prehashed([0; Hash::LENGTH]))
    {
        return Err(bad(
            "value differs from instance or has malformed immutable origin",
        ));
    }
    Ok(())
}

fn share_shape(share: &LaneSignatureShareV1, committee_len: usize) -> Result<(), LaneWireError> {
    // Existing native BLS-normal signatures have exactly 96 bytes. This checks
    // shape only: Core must authenticate the bytes with the frozen PoP roster.
    if share.signer as usize >= committee_len || share.signature.len() != 96 {
        return Err(bad(
            "signer index or native BLS signature length is invalid",
        ));
    }
    Ok(())
}

fn statement_shape(
    statement: &LaneVoteStatementV1,
    committee_len: usize,
) -> Result<(), LaneWireError> {
    value_shape(&statement.value, statement.round, committee_len)
}

fn qc_shape(qc: &LaneQcV1, committee_len: usize) -> Result<(), LaneWireError> {
    let quorum = committee_quorum(committee_len)?;
    statement_shape(&qc.statement, committee_len)?;
    if qc.shares.len() != quorum
        || qc
            .shares
            .windows(2)
            .any(|pair| pair[0].signer >= pair[1].signer)
    {
        return Err(bad("QC requires exactly 2f+1 ordered distinct shares"));
    }
    for share in &qc.shares {
        share_shape(share, committee_len)?;
    }
    Ok(())
}

fn timeout_shape(body: &LaneTimeoutBodyV1, committee_len: usize) -> Result<(), LaneWireError> {
    round_shape(body.round)?;
    if let Some(high) = &body.highest_prepare {
        qc_shape(high, committee_len)?;
        let round = high.statement.round;
        if high.statement.phase != LanePhaseV1::Prepare
            || round.instance_id != body.round.instance_id
            || round.lane_height != body.round.lane_height
            || round.voting_view > body.round.voting_view
        {
            return Err(bad(
                "timeout carries foreign, future or non-Prepare evidence",
            ));
        }
    }
    Ok(())
}

fn tc_shape(tc: &LaneTcV1, committee_len: usize) -> Result<(), LaneWireError> {
    let quorum = committee_quorum(committee_len)?;
    round_shape(tc.round)?;
    if tc.votes.len() != quorum
        || tc
            .votes
            .windows(2)
            .any(|pair| pair[0].share.signer >= pair[1].share.signer)
    {
        return Err(bad("TC requires exactly 2f+1 ordered distinct voters"));
    }
    for vote in &tc.votes {
        if vote.body.round != tc.round {
            return Err(bad("TC contains mixed rounds"));
        }
        timeout_shape(&vote.body, committee_len)?;
        share_shape(&vote.share, committee_len)?;
    }
    // Highest-Prepare compatibility and safe proposal selection are owned by
    // the shared reducer. Do not replicate its safety relation in this codec.
    Ok(())
}

fn manifest_shape(
    manifest: &LaneManifestV1,
    round: LaneRoundV1,
    committee_len: usize,
) -> Result<(), LaneWireError> {
    value_shape(&manifest.value, round, committee_len)?;
    manifest.validate_availability()
}

impl LaneMessageV1 {
    /// Check bounded native shape only, without leader, signature or finality authority.
    ///
    /// The committee count must come from the separately authenticated context.
    /// This method does not choose a highest Prepare or implement a vote guard.
    pub fn validate_shape(&self, committee_len: usize) -> Result<(), LaneWireError> {
        committee_quorum(committee_len)?;
        match self {
            Self::Proposal(proposal) => {
                let body = &proposal.body;
                manifest_shape(&body.manifest, body.round, committee_len)?;
                if body.proposer as usize >= committee_len || proposal.signature.len() != 96 {
                    return Err(bad("proposer index or native signature length is invalid"));
                }
                match &body.justification {
                    LaneJustificationV1::Opening if body.round.voting_view == 0 => {}
                    LaneJustificationV1::Timeout(tc)
                        if tc.round.instance_id == body.round.instance_id
                            && tc.round.lane_height == body.round.lane_height
                            && tc.round.voting_view.checked_add(1)
                                == Some(body.round.voting_view) =>
                    {
                        tc_shape(tc, committee_len)?;
                    }
                    _ => return Err(bad("proposal lacks exact opening or previous-round TC")),
                }
            }
            Self::Vote(vote) => {
                statement_shape(&vote.statement, committee_len)?;
                share_shape(&vote.share, committee_len)?;
            }
            Self::QuorumCertificate(qc) => qc_shape(qc, committee_len)?,
            Self::TimeoutVote(vote) => {
                timeout_shape(&vote.body, committee_len)?;
                share_shape(&vote.share, committee_len)?;
            }
            Self::TimeoutCertificate(tc) => tc_shape(tc, committee_len)?,
        }
        Ok(())
    }
}

impl LaneMessageEnvelopeV1 {
    /// Decode advertised canonical Norito framing within the caller's ingress bound.
    ///
    /// This returns untrusted native evidence. The caller must authenticate the
    /// exact current context and signatures before admitting a reducer event.
    pub fn decode_canonical(
        bytes: &[u8],
        maximum_bytes: usize,
        committee_len: usize,
    ) -> Result<Self, LaneWireError> {
        if bytes.len() > maximum_bytes {
            return Err(bad("native evidence exceeds ingress bound"));
        }
        let envelope: Self = norito::decode_canonical(bytes).map_err(bad)?;
        if envelope.version != LANE_MESSAGE_VERSION_V1 {
            return Err(bad("unsupported native lane envelope revision"));
        }
        envelope.message.validate_shape(committee_len)?;
        Ok(envelope)
    }
}

impl LaneDecisionV1 {
    /// Exact immutable input reference decided by the native Commit quorum.
    pub fn value(&self) -> &LaneValueRefV1 {
        &self.manifest.value
    }

    /// Check native shape and matching frozen policy; never grant authority.
    ///
    /// A verified current/opening context must separately bind `instance_id`.
    /// Core must verify every signature, original leader, and exact body hashes.
    /// Globally cancelled/closed pre-merge evidence must not become live here.
    pub fn validate_shape(
        &self,
        frozen: &FrozenLaneConsensusContextV1,
    ) -> Result<(), LaneWireError> {
        frozen.validate().map_err(bad)?;
        qc_shape(&self.commit_qc, frozen.committee.len())?;
        if self.commit_qc.statement.phase != LanePhaseV1::Commit
            || self.commit_qc.statement.value != self.manifest.value
            || self.commit_qc.statement.round.lane_height != frozen.next_lane_height
            || self.manifest.value.admitted_binding_hash != frozen.admitted_binding_hash
            || self.manifest.layout != frozen.da_layout
        {
            return Err(bad(
                "decision differs from its exact Commit subject or frozen input policy",
            ));
        }
        manifest_shape(
            &self.manifest,
            self.commit_qc.statement.round,
            frozen.committee.len(),
        )
    }
}

#[cfg(test)]
#[path = "lane_consensus_tests.rs"]
mod tests;
