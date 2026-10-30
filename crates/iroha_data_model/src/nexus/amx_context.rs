//! Canonical framing and strict reader for the signed Nexus/AMX context preimage.
//!
//! Signed genesis commits `Hash::new(preimage)` as `sumeragi_context.nexus_amx_context_hash`.
//! [`NexusAmxContextWriterV1`] is the sole framing used to build that preimage (in
//! `iroha_config::parameters::actual::sumeragi_nexus_amx_context_preimage`). The reader walks
//! the exact tag grammar of that projection, decodes only the physical catalog fields a light
//! client must join against (lanes, dataspaces, routing rules and the autoscale range) and
//! treats every other field as opaque bytes. Every decoded value must re-encode to its exact
//! original bytes. Golden tests in `iroha_config` keep the writer and this grammar aligned:
//! a field added to the projection must also be added to [`tag`], to the grammar here and to
//! the field-level rendering in [`nexus_amx_context_fields_v1`].
use super::{LaneConsensusProjectionV1, LaneVisibility};
use iroha_model_base::topology::{DataSpaceId, LaneId};
use norito::codec::{Decode, Encode};
use std::{collections::BTreeSet, fmt};

mod fields;
pub use fields::{
    NexusAmxContextFieldDiffV1, NexusAmxContextFieldOriginV1, NexusAmxContextFieldV1,
    diff_nexus_amx_context_v1, nexus_amx_context_fields_v1,
};

/// Domain prefix of every Nexus/AMX context preimage.
pub const NEXUS_AMX_CONTEXT_DOMAIN_V1: &[u8] = b"sumeragi:nexus-amx-context\0v1";
/// Upper bound on an admitted Nexus/AMX context preimage.
pub const MAX_NEXUS_AMX_CONTEXT_BYTES_V1: usize = 1 << 20;

/// Projection tags, in the exact order the preimage emits them.
pub mod tag {
    /// Exclusive lane-id bound of the lane catalog.
    pub const LANE_COUNT: &str = "nexus.lane_catalog.lane_count";
    /// Canonically ordered lane consensus projections.
    pub const LANES: &str = "nexus.lane_catalog.lanes";
    /// Number of retained lane-incarnation lineage entries.
    pub const LANE_LIFECYCLE_COUNT: &str = "nexus.lane_lifecycle.count";
    /// Lineage entry lane id.
    pub const LANE_LIFECYCLE_LANE_ID: &str = "nexus.lane_lifecycle.lane_id";
    /// Lineage entry generation.
    pub const LANE_LIFECYCLE_GENERATION: &str = "nexus.lane_lifecycle.generation";
    /// Lineage entry incarnation commitment.
    pub const LANE_LIFECYCLE_INCARNATION: &str = "nexus.lane_lifecycle.incarnation";
    /// Lineage entry activation height.
    pub const LANE_LIFECYCLE_ACTIVATION_HEIGHT: &str = "nexus.lane_lifecycle.activation_height";
    /// Number of dataspace catalog entries.
    pub const DATASPACE_COUNT: &str = "nexus.dataspace_catalog.count";
    /// Dataspace id.
    pub const DATASPACE_ID: &str = "nexus.dataspace.id";
    /// Dataspace alias.
    pub const DATASPACE_ALIAS: &str = "nexus.dataspace.alias";
    /// Dataspace fault tolerance.
    pub const DATASPACE_FAULT_TOLERANCE: &str = "nexus.dataspace.fault_tolerance";
    /// Routing default lane.
    pub const ROUTING_DEFAULT_LANE: &str = "nexus.routing.default_lane";
    /// Routing default dataspace.
    pub const ROUTING_DEFAULT_DATASPACE: &str = "nexus.routing.default_dataspace";
    /// Number of ordered routing rules.
    pub const ROUTING_RULE_COUNT: &str = "nexus.routing.rule_count";
    /// Routing rule lane.
    pub const ROUTING_RULE_LANE: &str = "nexus.routing.rule.lane";
    /// Routing rule dataspace override.
    pub const ROUTING_RULE_DATASPACE: &str = "nexus.routing.rule.dataspace";
    /// Routing rule account matcher.
    pub const ROUTING_RULE_ACCOUNT: &str = "nexus.routing.rule.account";
    /// Routing rule instruction matcher.
    pub const ROUTING_RULE_INSTRUCTION: &str = "nexus.routing.rule.instruction";
    /// Public-lane validator mode.
    pub const STAKING_PUBLIC_VALIDATOR_MODE: &str = "nexus.staking.public_validator_mode";
    /// Restricted-lane validator mode.
    pub const STAKING_RESTRICTED_VALIDATOR_MODE: &str = "nexus.staking.restricted_validator_mode";
    /// Maximum validators.
    pub const STAKING_MAX_VALIDATORS: &str = "nexus.staking.max_validators";
    /// Exact optional original root fee policy, including all typed custody and exemptions.
    /// Absence records only the actual pre-genesis state; runtime presence is enforced by Core.
    pub const ROOT_FEE_POLICY: &str = "root.fee_policy";
    /// Default fee sponsor program per dataspace.
    pub const DATASPACE_FEE_SPONSOR_PROGRAM_IDS: &str = "nexus.dataspace_fee_sponsor_program_ids";
    /// AXT slot length.
    pub const AXT_SLOT_LENGTH_MS: &str = "nexus.axt.slot_length_ms";
    /// AXT clock skew.
    pub const AXT_MAX_CLOCK_SKEW_MS: &str = "nexus.axt.max_clock_skew_ms";
    /// AXT proof cache TTL.
    pub const AXT_PROOF_CACHE_TTL_SLOTS: &str = "nexus.axt.proof_cache_ttl_slots";
    /// AXT replay retention.
    pub const AXT_REPLAY_RETENTION_SLOTS: &str = "nexus.axt.replay_retention_slots";
    /// Fusion floor.
    pub const FUSION_FLOOR_TEU: &str = "nexus.fusion.floor_teu";
    /// Fusion exit.
    pub const FUSION_EXIT_TEU: &str = "nexus.fusion.exit_teu";
    /// Fusion observation slots.
    pub const FUSION_OBSERVATION_SLOTS: &str = "nexus.fusion.observation_slots";
    /// Fusion window.
    pub const FUSION_MAX_WINDOW_SLOTS: &str = "nexus.fusion.max_window_slots";
    /// Whether autoscale is enabled.
    pub const AUTOSCALE_ENABLED: &str = "nexus.autoscale.enabled";
    /// Inclusive lower elastic lane bound.
    pub const AUTOSCALE_MIN_LANE_ID: &str = "nexus.autoscale.min_lane_id";
    /// Exclusive upper elastic lane bound.
    pub const AUTOSCALE_MAX_LANE_ID_EXCLUSIVE: &str = "nexus.autoscale.max_lane_id_exclusive";
    /// Autoscale target block interval.
    pub const AUTOSCALE_TARGET_BLOCK_MS: &str = "nexus.autoscale.target_block_ms";
    /// Autoscale scale-out latency ratio.
    pub const AUTOSCALE_SCALE_OUT_LATENCY_RATIO_BITS: &str =
        "nexus.autoscale.scale_out_latency_ratio_bits";
    /// Autoscale scale-in latency ratio.
    pub const AUTOSCALE_SCALE_IN_LATENCY_RATIO_BITS: &str =
        "nexus.autoscale.scale_in_latency_ratio_bits";
    /// Autoscale scale-out utilization ratio.
    pub const AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO_BITS: &str =
        "nexus.autoscale.scale_out_utilization_ratio_bits";
    /// Autoscale scale-in utilization ratio.
    pub const AUTOSCALE_SCALE_IN_UTILIZATION_RATIO_BITS: &str =
        "nexus.autoscale.scale_in_utilization_ratio_bits";
    /// Autoscale scale-out window.
    pub const AUTOSCALE_SCALE_OUT_WINDOW_BLOCKS: &str = "nexus.autoscale.scale_out_window_blocks";
    /// Autoscale scale-in window.
    pub const AUTOSCALE_SCALE_IN_WINDOW_BLOCKS: &str = "nexus.autoscale.scale_in_window_blocks";
    /// Autoscale cooldown.
    pub const AUTOSCALE_COOLDOWN_BLOCKS: &str = "nexus.autoscale.cooldown_blocks";
    /// Autoscale per-lane target throughput.
    pub const AUTOSCALE_PER_LANE_TARGET_TPS: &str = "nexus.autoscale.per_lane_target_tps";
    /// Autoscale last transition height.
    pub const AUTOSCALE_LAST_TRANSITION_HEIGHT: &str = "nexus.autoscale.last_transition_height";
    /// Commit window.
    pub const COMMIT_WINDOW_SLOTS: &str = "nexus.commit.window_slots";
    /// DA per-slot total.
    pub const DA_Q_IN_SLOT_TOTAL: &str = "nexus.da.q_in_slot_total";
    /// DA per-dataspace minimum.
    pub const DA_Q_IN_SLOT_PER_DS_MIN: &str = "nexus.da.q_in_slot_per_ds_min";
    /// DA base sample size.
    pub const DA_SAMPLE_SIZE_BASE: &str = "nexus.da.sample_size_base";
    /// DA maximum sample size.
    pub const DA_SAMPLE_SIZE_MAX: &str = "nexus.da.sample_size_max";
    /// DA base threshold.
    pub const DA_THRESHOLD_BASE: &str = "nexus.da.threshold_base";
    /// DA shards per attester.
    pub const DA_PER_ATTESTER_SHARDS: &str = "nexus.da.per_attester_shards";
    /// DA ingest quota window.
    pub const DA_INGEST_QUOTA_WINDOW_BLOCKS: &str = "nexus.da.ingest_quota_window_blocks";
    /// DA ingest count quota.
    pub const DA_INGEST_QUOTA_MAX_COUNT_PER_ACCOUNT: &str =
        "nexus.da.ingest_quota_max_count_per_account";
    /// DA ingest byte quota.
    pub const DA_INGEST_QUOTA_MAX_BYTES_PER_ACCOUNT: &str =
        "nexus.da.ingest_quota_max_bytes_per_account";
    /// DA audit sample size.
    pub const DA_AUDIT_SAMPLE_SIZE: &str = "nexus.da.audit.sample_size";
    /// DA audit window count.
    pub const DA_AUDIT_WINDOW_COUNT: &str = "nexus.da.audit.window_count";
    /// DA audit interval.
    pub const DA_AUDIT_INTERVAL_NS: &str = "nexus.da.audit.interval_ns";
    /// DA recovery timeout.
    pub const DA_RECOVERY_REQUEST_TIMEOUT_NS: &str = "nexus.da.recovery.request_timeout_ns";
    /// DA rotation hit bound.
    pub const DA_ROTATION_MAX_HITS_PER_WINDOW: &str = "nexus.da.rotation.max_hits_per_window";
    /// DA rotation window.
    pub const DA_ROTATION_WINDOW_SLOTS: &str = "nexus.da.rotation.window_slots";
    /// DA rotation seed tag.
    pub const DA_ROTATION_SEED_TAG: &str = "nexus.da.rotation.seed_tag";
    /// DA rotation latency decay.
    pub const DA_ROTATION_LATENCY_DECAY_BITS: &str = "nexus.da.rotation.latency_decay_bits";
    /// AMX per-dataspace budget.
    pub const PIPELINE_AMX_PER_DATASPACE_BUDGET_MS: &str = "pipeline.amx_per_dataspace_budget_ms";
    /// AMX group budget.
    pub const PIPELINE_AMX_GROUP_BUDGET_MS: &str = "pipeline.amx_group_budget_ms";
    /// AMX per-instruction cost.
    pub const PIPELINE_AMX_PER_INSTRUCTION_NS: &str = "pipeline.amx_per_instruction_ns";
    /// AMX per-memory-access cost.
    pub const PIPELINE_AMX_PER_MEMORY_ACCESS_NS: &str = "pipeline.amx_per_memory_access_ns";
    /// AMX per-syscall cost.
    pub const PIPELINE_AMX_PER_SYSCALL_NS: &str = "pipeline.amx_per_syscall_ns";
    /// Staged active public-lane validator records.
    pub const STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS: &str = "staged.active_public_lane_validators";

    /// Fixed-arity fields after the routing rules, in exact preimage order.
    pub const TAIL: [&str; 50] = [
        STAKING_PUBLIC_VALIDATOR_MODE,
        STAKING_RESTRICTED_VALIDATOR_MODE,
        STAKING_MAX_VALIDATORS,
        ROOT_FEE_POLICY,
        DATASPACE_FEE_SPONSOR_PROGRAM_IDS,
        AXT_SLOT_LENGTH_MS,
        AXT_MAX_CLOCK_SKEW_MS,
        AXT_PROOF_CACHE_TTL_SLOTS,
        AXT_REPLAY_RETENTION_SLOTS,
        FUSION_FLOOR_TEU,
        FUSION_EXIT_TEU,
        FUSION_OBSERVATION_SLOTS,
        FUSION_MAX_WINDOW_SLOTS,
        AUTOSCALE_ENABLED,
        AUTOSCALE_MIN_LANE_ID,
        AUTOSCALE_MAX_LANE_ID_EXCLUSIVE,
        AUTOSCALE_TARGET_BLOCK_MS,
        AUTOSCALE_SCALE_OUT_LATENCY_RATIO_BITS,
        AUTOSCALE_SCALE_IN_LATENCY_RATIO_BITS,
        AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO_BITS,
        AUTOSCALE_SCALE_IN_UTILIZATION_RATIO_BITS,
        AUTOSCALE_SCALE_OUT_WINDOW_BLOCKS,
        AUTOSCALE_SCALE_IN_WINDOW_BLOCKS,
        AUTOSCALE_COOLDOWN_BLOCKS,
        AUTOSCALE_PER_LANE_TARGET_TPS,
        AUTOSCALE_LAST_TRANSITION_HEIGHT,
        COMMIT_WINDOW_SLOTS,
        DA_Q_IN_SLOT_TOTAL,
        DA_Q_IN_SLOT_PER_DS_MIN,
        DA_SAMPLE_SIZE_BASE,
        DA_SAMPLE_SIZE_MAX,
        DA_THRESHOLD_BASE,
        DA_PER_ATTESTER_SHARDS,
        DA_INGEST_QUOTA_WINDOW_BLOCKS,
        DA_INGEST_QUOTA_MAX_COUNT_PER_ACCOUNT,
        DA_INGEST_QUOTA_MAX_BYTES_PER_ACCOUNT,
        DA_AUDIT_SAMPLE_SIZE,
        DA_AUDIT_WINDOW_COUNT,
        DA_AUDIT_INTERVAL_NS,
        DA_RECOVERY_REQUEST_TIMEOUT_NS,
        DA_ROTATION_MAX_HITS_PER_WINDOW,
        DA_ROTATION_WINDOW_SLOTS,
        DA_ROTATION_SEED_TAG,
        DA_ROTATION_LATENCY_DECAY_BITS,
        PIPELINE_AMX_PER_DATASPACE_BUDGET_MS,
        PIPELINE_AMX_GROUP_BUDGET_MS,
        PIPELINE_AMX_PER_INSTRUCTION_NS,
        PIPELINE_AMX_PER_MEMORY_ACCESS_NS,
        PIPELINE_AMX_PER_SYSCALL_NS,
        STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS,
    ];
}

/// Writer for the canonical tag-length-value Nexus/AMX context preimage.
///
/// Each field is framed as a little-endian `u32` tag length, the tag bytes, a little-endian
/// `u64` value length and the bare Norito encoding of the value.
#[derive(Debug, Clone)]
pub struct NexusAmxContextWriterV1(Vec<u8>);

impl Default for NexusAmxContextWriterV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusAmxContextWriterV1 {
    /// Start a preimage with the exact V1 domain prefix.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec())
    }

    /// Append one framed field.
    ///
    /// # Panics
    /// Panics only if a static tag exceeds `u32` or a value exceeds `u64` bytes.
    pub fn field<T: Encode>(&mut self, tag: &'static str, value: &T) {
        let bytes = value.encode();
        let tag_len = u32::try_from(tag.len()).expect("static projection tag fits in u32");
        let bytes_len = u64::try_from(bytes.len()).expect("projection field fits in u64");
        self.0.extend_from_slice(&tag_len.to_le_bytes());
        self.0.extend_from_slice(tag.as_bytes());
        self.0.extend_from_slice(&bytes_len.to_le_bytes());
        self.0.extend_from_slice(&bytes);
    }

    /// Complete the preimage.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// The exact Nexus/AMX context preimage a node derived, retained by a refusal as evidence.
///
/// `Debug` shows its length and hash, not its bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct NexusAmxContextPreimageV1(Vec<u8>);

impl NexusAmxContextPreimageV1 {
    /// Retain derived preimage bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The exact preimage bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The commitment of the preimage, as signed genesis records it.
    #[must_use]
    pub fn hash(&self) -> iroha_crypto::Hash {
        iroha_crypto::Hash::new(&self.0)
    }
}

impl fmt::Debug for NexusAmxContextPreimageV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NexusAmxContextPreimageV1")
            .field("len", &self.0.len())
            .field("hash", &self.hash())
            .finish()
    }
}

/// Why a Nexus/AMX context preimage was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NexusAmxContextError {
    /// The preimage is empty or exceeds [`MAX_NEXUS_AMX_CONTEXT_BYTES_V1`].
    #[error("Nexus/AMX context preimage exceeds its bound")]
    Oversize,
    /// The preimage does not start with [`NEXUS_AMX_CONTEXT_DOMAIN_V1`].
    #[error("Nexus/AMX context preimage has the wrong domain")]
    Domain,
    /// A field frame is incomplete or not UTF-8.
    #[error("Nexus/AMX context preimage is truncated or malformed")]
    Truncated,
    /// A field appears out of the exact grammar order, or is unknown.
    #[error("Nexus/AMX context field {found:?} appears where {expected:?} is required")]
    UnexpectedTag {
        /// Required tag.
        expected: &'static str,
        /// Tag found in the preimage, or `None` at end of input.
        found: Option<String>,
    },
    /// Fields remain after the complete grammar.
    #[error("Nexus/AMX context preimage has trailing field {0:?}")]
    TrailingField(String),
    /// A selected value is not its exact canonical encoding.
    #[error("Nexus/AMX context field {0} is not canonical")]
    NonCanonical(&'static str),
    /// The decoded catalog is structurally inconsistent.
    #[error("Nexus/AMX context catalog is inconsistent: {0}")]
    Structure(&'static str),
}

/// One dataspace catalog entry committed into the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusDataspaceProjectionV1 {
    id: DataSpaceId,
    alias: String,
    fault_tolerance: u32,
}

impl NexusDataspaceProjectionV1 {
    /// Dataspace identity.
    #[must_use]
    pub fn id(&self) -> DataSpaceId {
        self.id
    }
    /// Dataspace alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
    /// Fault tolerance `f`.
    #[must_use]
    pub fn fault_tolerance(&self) -> u32 {
        self.fault_tolerance
    }
}

/// One ordered physical routing rule committed into the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusRoutingRuleProjectionV1 {
    lane: LaneId,
    dataspace: Option<DataSpaceId>,
    account: Option<String>,
    instruction: Option<String>,
}

impl NexusRoutingRuleProjectionV1 {
    /// Target lane.
    #[must_use]
    pub fn lane(&self) -> LaneId {
        self.lane
    }
    /// Optional dataspace override.
    #[must_use]
    pub fn dataspace(&self) -> Option<DataSpaceId> {
        self.dataspace
    }
    /// Optional account matcher.
    #[must_use]
    pub fn account(&self) -> Option<&str> {
        self.account.as_deref()
    }
    /// Optional instruction matcher.
    #[must_use]
    pub fn instruction(&self) -> Option<&str> {
        self.instruction.as_deref()
    }
}

/// Committed elastic-lane range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NexusAutoscaleProjectionV1 {
    enabled: bool,
    min_lane_id: u32,
    max_lane_id_exclusive: u32,
}

impl NexusAutoscaleProjectionV1 {
    /// Whether consensus-driven lane autoscaling is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    /// Inclusive lower elastic lane bound.
    #[must_use]
    pub fn min_lane_id(&self) -> u32 {
        self.min_lane_id
    }
    /// Exclusive upper elastic lane bound.
    #[must_use]
    pub fn max_lane_id_exclusive(&self) -> u32 {
        self.max_lane_id_exclusive
    }
    /// Whether `lane` lies in the enabled elastic range.
    #[must_use]
    pub fn contains(&self, lane: LaneId) -> bool {
        self.enabled && (self.min_lane_id..self.max_lane_id_exclusive).contains(&lane.as_u32())
    }
}

/// The physical catalog selected from one exact, structurally checked context preimage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusAmxContextCatalogV1 {
    lane_count: u32,
    lanes: Vec<LaneConsensusProjectionV1>,
    dataspaces: Vec<NexusDataspaceProjectionV1>,
    default_lane: LaneId,
    default_dataspace: DataSpaceId,
    rules: Vec<NexusRoutingRuleProjectionV1>,
    autoscale: NexusAutoscaleProjectionV1,
}

impl NexusAmxContextCatalogV1 {
    /// Exclusive lane-id bound.
    #[must_use]
    pub fn lane_count(&self) -> u32 {
        self.lane_count
    }
    /// Lanes, strictly ascending by id.
    #[must_use]
    pub fn lanes(&self) -> &[LaneConsensusProjectionV1] {
        &self.lanes
    }
    /// Dataspaces, strictly ascending by id.
    #[must_use]
    pub fn dataspaces(&self) -> &[NexusDataspaceProjectionV1] {
        &self.dataspaces
    }
    /// Routing default lane.
    #[must_use]
    pub fn default_lane(&self) -> LaneId {
        self.default_lane
    }
    /// Routing default dataspace.
    #[must_use]
    pub fn default_dataspace(&self) -> DataSpaceId {
        self.default_dataspace
    }
    /// Ordered first-match routing rules.
    #[must_use]
    pub fn rules(&self) -> &[NexusRoutingRuleProjectionV1] {
        &self.rules
    }
    /// Elastic lane range.
    #[must_use]
    pub fn autoscale(&self) -> NexusAutoscaleProjectionV1 {
        self.autoscale
    }
    /// The lane with `id`.
    #[must_use]
    pub fn lane(&self, id: LaneId) -> Option<&LaneConsensusProjectionV1> {
        self.lanes
            .binary_search_by_key(&id, |lane| lane.id)
            .ok()
            .map(|index| &self.lanes[index])
    }
}

impl LaneConsensusProjectionV1 {
    /// Lane identifier.
    #[must_use]
    pub fn id(&self) -> LaneId {
        self.id
    }
    /// Lane alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
    /// Physical dataspace owning the lane.
    #[must_use]
    pub fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }
    /// Declarative visibility.
    #[must_use]
    pub fn visibility(&self) -> LaneVisibility {
        self.visibility
    }
    /// Governance module identifier.
    #[must_use]
    pub fn governance(&self) -> Option<&str> {
        self.governance.as_deref()
    }
}

struct Fields<'a> {
    fields: Vec<(&'a str, &'a [u8])>,
    next: usize,
}

impl<'a> Fields<'a> {
    fn tokenize(preimage: &'a [u8]) -> Result<Self, NexusAmxContextError> {
        if preimage.is_empty() || preimage.len() > MAX_NEXUS_AMX_CONTEXT_BYTES_V1 {
            return Err(NexusAmxContextError::Oversize);
        }
        let mut rest = preimage
            .strip_prefix(NEXUS_AMX_CONTEXT_DOMAIN_V1)
            .ok_or(NexusAmxContextError::Domain)?;
        let mut fields = Vec::new();
        while !rest.is_empty() {
            let tag_len = take(&mut rest, 4)?;
            let tag_len = usize::try_from(u32::from_le_bytes(
                tag_len.try_into().expect("four tag-length bytes"),
            ))
            .map_err(|_| NexusAmxContextError::Truncated)?;
            let tag = std::str::from_utf8(take(&mut rest, tag_len)?)
                .map_err(|_| NexusAmxContextError::Truncated)?;
            let value_len = take(&mut rest, 8)?;
            let value_len = usize::try_from(u64::from_le_bytes(
                value_len.try_into().expect("eight value-length bytes"),
            ))
            .map_err(|_| NexusAmxContextError::Truncated)?;
            fields.push((tag, take(&mut rest, value_len)?));
        }
        Ok(Self { fields, next: 0 })
    }

    fn expect(&mut self, tag: &'static str) -> Result<&'a [u8], NexusAmxContextError> {
        match self.fields.get(self.next) {
            Some((found, value)) if *found == tag => {
                self.next += 1;
                Ok(value)
            }
            found => Err(NexusAmxContextError::UnexpectedTag {
                expected: tag,
                found: found.map(|(found, _)| (*found).to_owned()),
            }),
        }
    }

    fn value<T: Decode + Encode>(&mut self, tag: &'static str) -> Result<T, NexusAmxContextError> {
        let bytes = self.expect(tag)?;
        let value = norito::codec::decode_adaptive::<T>(bytes)
            .map_err(|_| NexusAmxContextError::NonCanonical(tag))?;
        if value.encode() != bytes {
            return Err(NexusAmxContextError::NonCanonical(tag));
        }
        Ok(value)
    }

    /// A counted group: the count must fit the remaining fields before any iteration.
    fn count(&mut self, tag: &'static str, group: usize) -> Result<usize, NexusAmxContextError> {
        let count = self.value::<u64>(tag)?;
        let remaining = self.fields.len() - self.next;
        usize::try_from(count)
            .ok()
            .filter(|count| count.checked_mul(group).is_some_and(|n| n <= remaining))
            .ok_or(NexusAmxContextError::Structure(
                "counted group exceeds the preimage",
            ))
    }

    fn finish(self) -> Result<(), NexusAmxContextError> {
        match self.fields.get(self.next) {
            None => Ok(()),
            Some((tag, _)) => Err(NexusAmxContextError::TrailingField((*tag).to_owned())),
        }
    }
}

fn take<'a>(rest: &mut &'a [u8], len: usize) -> Result<&'a [u8], NexusAmxContextError> {
    if rest.len() < len {
        return Err(NexusAmxContextError::Truncated);
    }
    let (head, tail) = rest.split_at(len);
    *rest = tail;
    Ok(head)
}

/// Decode the physical catalog from one exact Nexus/AMX context preimage.
///
/// The caller must separately bind the preimage to its signed commitment
/// (`Hash::new(preimage) == Hash::prehashed(sumeragi_context.nexus_amx_context_hash)`).
///
/// # Errors
/// Oversized or truncated input, the wrong domain, any unknown, missing, reordered or trailing
/// field, a non-canonical selected value, or an inconsistent catalog (unordered or duplicate
/// lanes, out-of-bounds lane ids, duplicate aliases or dataspaces, a lane with an unknown
/// dataspace, or a rule whose lane is unknown or whose dataspace differs from its lane).
pub fn decode_nexus_amx_context_v1(
    preimage: &[u8],
) -> Result<NexusAmxContextCatalogV1, NexusAmxContextError> {
    use tag::*;
    let mut fields = Fields::tokenize(preimage)?;
    let lane_count = fields.value::<u32>(LANE_COUNT)?;
    let lanes = fields.value::<Vec<LaneConsensusProjectionV1>>(LANES)?;
    for _ in 0..fields.count(LANE_LIFECYCLE_COUNT, 4)? {
        for tag in [
            LANE_LIFECYCLE_LANE_ID,
            LANE_LIFECYCLE_GENERATION,
            LANE_LIFECYCLE_INCARNATION,
            LANE_LIFECYCLE_ACTIVATION_HEIGHT,
        ] {
            fields.expect(tag)?;
        }
    }
    let mut dataspaces = Vec::new();
    for _ in 0..fields.count(DATASPACE_COUNT, 3)? {
        dataspaces.push(NexusDataspaceProjectionV1 {
            id: fields.value(DATASPACE_ID)?,
            alias: fields.value(DATASPACE_ALIAS)?,
            fault_tolerance: fields.value(DATASPACE_FAULT_TOLERANCE)?,
        });
    }
    let default_lane = fields.value::<LaneId>(ROUTING_DEFAULT_LANE)?;
    let default_dataspace = fields.value::<DataSpaceId>(ROUTING_DEFAULT_DATASPACE)?;
    let mut rules = Vec::new();
    for _ in 0..fields.count(ROUTING_RULE_COUNT, 4)? {
        rules.push(NexusRoutingRuleProjectionV1 {
            lane: fields.value(ROUTING_RULE_LANE)?,
            dataspace: fields.value(ROUTING_RULE_DATASPACE)?,
            account: fields.value(ROUTING_RULE_ACCOUNT)?,
            instruction: fields.value(ROUTING_RULE_INSTRUCTION)?,
        });
    }
    let mut autoscale = NexusAutoscaleProjectionV1 {
        enabled: false,
        min_lane_id: 0,
        max_lane_id_exclusive: 0,
    };
    for tag in TAIL {
        match tag {
            AUTOSCALE_ENABLED => autoscale.enabled = fields.value(tag)?,
            AUTOSCALE_MIN_LANE_ID => autoscale.min_lane_id = fields.value(tag)?,
            AUTOSCALE_MAX_LANE_ID_EXCLUSIVE => {
                autoscale.max_lane_id_exclusive = fields.value(tag)?
            }
            _ => {
                fields.expect(tag)?;
            }
        }
    }
    fields.finish()?;
    let catalog = NexusAmxContextCatalogV1 {
        lane_count,
        lanes,
        dataspaces,
        default_lane,
        default_dataspace,
        rules,
        autoscale,
    };
    check_structure(&catalog)?;
    Ok(catalog)
}

fn check_structure(catalog: &NexusAmxContextCatalogV1) -> Result<(), NexusAmxContextError> {
    use NexusAmxContextError::Structure;
    let mut previous = None;
    let mut lane_aliases = BTreeSet::new();
    for lane in &catalog.lanes {
        if lane.version != LaneConsensusProjectionV1::VERSION {
            return Err(Structure("unsupported lane projection version"));
        }
        if previous.is_some_and(|previous| previous >= lane.id)
            || lane.id.as_u32() >= catalog.lane_count
        {
            return Err(Structure(
                "lanes are not strictly ascending below the lane count",
            ));
        }
        previous = Some(lane.id);
        if !lane_aliases.insert(lane.alias.as_str()) {
            return Err(Structure("duplicate lane alias"));
        }
    }
    let mut previous = None;
    let mut dataspace_aliases = BTreeSet::new();
    for dataspace in &catalog.dataspaces {
        if previous.is_some_and(|previous| previous >= dataspace.id) {
            return Err(Structure("dataspaces are not strictly ascending"));
        }
        previous = Some(dataspace.id);
        if !dataspace_aliases.insert(dataspace.alias.as_str()) {
            return Err(Structure("duplicate dataspace alias"));
        }
    }
    let known = |id: DataSpaceId| {
        catalog
            .dataspaces
            .binary_search_by_key(&id, |dataspace| dataspace.id)
            .is_ok()
    };
    if catalog.lanes.iter().any(|lane| !known(lane.dataspace_id)) {
        return Err(Structure("a lane names an unknown dataspace"));
    }
    if catalog.lane(catalog.default_lane).is_none() || !known(catalog.default_dataspace) {
        return Err(Structure("the routing default is not in the catalog"));
    }
    for rule in &catalog.rules {
        let lane = catalog
            .lane(rule.lane)
            .ok_or(Structure("a routing rule names an unknown lane"))?;
        if rule
            .dataspace
            .is_some_and(|dataspace| dataspace != lane.dataspace_id)
        {
            return Err(Structure(
                "a routing rule pairs a lane with a foreign dataspace",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::{LaneCatalog, LaneConfig};
    use std::num::NonZeroU32;

    fn lane(id: u32, alias: &str, dataspace: DataSpaceId) -> LaneConfig {
        LaneConfig {
            id: LaneId::new(id),
            dataspace_id: dataspace,
            alias: alias.to_owned(),
            ..LaneConfig::default()
        }
    }

    type TestRoutingRule<'a> = (u32, Option<u64>, Option<&'a str>, Option<&'a str>);

    /// A miniature writer-produced preimage with the complete grammar.
    fn preimage_with(
        lanes: &[LaneConfig],
        dataspaces: &[(u64, &str)],
        rules: &[TestRoutingRule<'_>],
        autoscale: (bool, u32, u32),
    ) -> Vec<u8> {
        use tag::*;
        let catalog = LaneCatalog::new(NonZeroU32::new(8).unwrap(), lanes.to_vec()).unwrap();
        let (count, projected) = catalog.consensus_projection();
        let mut writer = NexusAmxContextWriterV1::new();
        writer.field(LANE_COUNT, &count);
        writer.field(LANES, &projected);
        writer.field(LANE_LIFECYCLE_COUNT, &1_u64);
        writer.field(LANE_LIFECYCLE_LANE_ID, &LaneId::new(0));
        writer.field(LANE_LIFECYCLE_GENERATION, &0_u64);
        writer.field(
            LANE_LIFECYCLE_INCARNATION,
            &iroha_crypto::Hash::new(b"lane"),
        );
        writer.field(LANE_LIFECYCLE_ACTIVATION_HEIGHT, &1_u64);
        writer.field(DATASPACE_COUNT, &(dataspaces.len() as u64));
        for (id, alias) in dataspaces {
            writer.field(DATASPACE_ID, &DataSpaceId::new(*id));
            writer.field(DATASPACE_ALIAS, &(*alias).to_owned());
            writer.field(DATASPACE_FAULT_TOLERANCE, &1_u32);
        }
        writer.field(ROUTING_DEFAULT_LANE, &LaneId::new(0));
        writer.field(ROUTING_DEFAULT_DATASPACE, &DataSpaceId::UNIVERSAL);
        writer.field(ROUTING_RULE_COUNT, &(rules.len() as u64));
        for (lane, dataspace, account, instruction) in rules {
            writer.field(ROUTING_RULE_LANE, &LaneId::new(*lane));
            writer.field(ROUTING_RULE_DATASPACE, &dataspace.map(DataSpaceId::new));
            writer.field(ROUTING_RULE_ACCOUNT, &account.map(str::to_owned));
            writer.field(ROUTING_RULE_INSTRUCTION, &instruction.map(str::to_owned));
        }
        for tag in TAIL {
            match tag {
                AUTOSCALE_ENABLED => writer.field(tag, &autoscale.0),
                AUTOSCALE_MIN_LANE_ID => writer.field(tag, &autoscale.1),
                AUTOSCALE_MAX_LANE_ID_EXCLUSIVE => writer.field(tag, &autoscale.2),
                _ => writer.field(tag, &7_u64),
            }
        }
        writer.finish()
    }

    fn sample() -> Vec<u8> {
        preimage_with(
            &[
                lane(0, "core", DataSpaceId::UNIVERSAL),
                lane(5, "bpng", DataSpaceId::new(9)),
            ],
            &[(0, "universal"), (9, "bpng")],
            &[
                (0, Some(0), None, Some("governance")),
                (5, Some(9), Some("*@bpng"), None),
            ],
            (true, 6, 8),
        )
    }

    #[test]
    fn root_fee_policy_has_one_tag_and_retired_local_terms_are_rejected() {
        assert_eq!(
            tag::TAIL
                .iter()
                .filter(|&&name| name == tag::ROOT_FEE_POLICY)
                .count(),
            1
        );
        assert!(!tag::TAIL.iter().any(|name| name.starts_with("nexus.fees.")));
        let good = sample();
        let fields = Fields::tokenize(&good).unwrap().fields;
        for retired in [
            "nexus.fees.asset",
            "nexus.fees.sink",
            "nexus.fees.base",
            "nexus.fees.per_byte",
            "nexus.fees.per_instruction",
            "nexus.fees.per_gas_unit",
            "nexus.fees.sponsor_vault_custody_account_id",
            "nexus.fees.successful_claim_exempt_authorities",
        ] {
            let mut encoded = NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec();
            for (field, value) in &fields {
                let field = if *field == tag::ROOT_FEE_POLICY {
                    retired
                } else {
                    *field
                };
                encoded.extend_from_slice(&u32::try_from(field.len()).unwrap().to_le_bytes());
                encoded.extend_from_slice(field.as_bytes());
                encoded.extend_from_slice(&u64::try_from(value.len()).unwrap().to_le_bytes());
                encoded.extend_from_slice(value);
            }
            assert_eq!(
                decode_nexus_amx_context_v1(&encoded),
                Err(NexusAmxContextError::UnexpectedTag {
                    expected: tag::ROOT_FEE_POLICY,
                    found: Some(retired.to_owned()),
                })
            );
        }
    }

    #[test]
    fn monetary_staking_tags_cannot_reenter_the_node_context() {
        let staking: Vec<_> = tag::TAIL
            .iter()
            .copied()
            .filter(|name| name.starts_with("nexus.staking."))
            .collect();
        assert_eq!(
            staking,
            [
                tag::STAKING_PUBLIC_VALIDATOR_MODE,
                tag::STAKING_RESTRICTED_VALIDATOR_MODE,
                tag::STAKING_MAX_VALIDATORS
            ]
        );
        let good = sample();
        let fields = Fields::tokenize(&good).unwrap().fields;
        for retired in [
            "nexus.staking.min_validator_stake",
            "nexus.staking.max_stake_shares_per_validator",
            "nexus.staking.max_pending_unbonds_per_share",
            "nexus.staking.unbonding_delay_ns",
            "nexus.staking.max_slash_bps",
            "nexus.staking.stake_asset_id",
            "nexus.staking.stake_escrow_account_id",
            "nexus.staking.slash_sink_account_id",
        ] {
            let mut encoded = NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec();
            for (field, value) in &fields {
                if *field == tag::STAKING_MAX_VALIDATORS {
                    let mut insertion = NexusAmxContextWriterV1(encoded);
                    insertion.field(retired, &7_u64);
                    encoded = insertion.finish();
                }
                encoded.extend_from_slice(&u32::try_from(field.len()).unwrap().to_le_bytes());
                encoded.extend_from_slice(field.as_bytes());
                encoded.extend_from_slice(&u64::try_from(value.len()).unwrap().to_le_bytes());
                encoded.extend_from_slice(value);
            }
            assert_eq!(
                decode_nexus_amx_context_v1(&encoded),
                Err(NexusAmxContextError::UnexpectedTag {
                    expected: tag::STAKING_MAX_VALIDATORS,
                    found: Some(retired.to_owned()),
                })
            );
        }
    }

    #[test]
    fn writer_preimage_decodes_selected_catalog() {
        let catalog = decode_nexus_amx_context_v1(&sample()).unwrap();
        assert_eq!(catalog.lane_count(), 8);
        assert_eq!(catalog.lanes().len(), 2);
        let bpng = catalog.lane(LaneId::new(5)).unwrap();
        assert_eq!(
            (bpng.alias(), bpng.dataspace_id(), bpng.visibility()),
            ("bpng", DataSpaceId::new(9), LaneVisibility::Public)
        );
        assert_eq!(catalog.dataspaces()[1].alias(), "bpng");
        assert_eq!(catalog.dataspaces()[1].fault_tolerance(), 1);
        assert_eq!(catalog.rules()[1].account(), Some("*@bpng"));
        assert_eq!(catalog.rules()[0].instruction(), Some("governance"));
        assert!(catalog.autoscale().contains(LaneId::new(6)));
        assert!(!catalog.autoscale().contains(LaneId::new(5)));
        assert_eq!(
            (catalog.default_lane(), catalog.default_dataspace()),
            (LaneId::new(0), DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn framing_rejects_domain_truncation_trailing_and_oversize_input() {
        let good = sample();
        let mut wrong_domain = good.clone();
        wrong_domain[0] ^= 1;
        assert_eq!(
            decode_nexus_amx_context_v1(&wrong_domain),
            Err(NexusAmxContextError::Domain)
        );
        for cut in [good.len() - 1, NEXUS_AMX_CONTEXT_DOMAIN_V1.len() + 2] {
            assert_eq!(
                decode_nexus_amx_context_v1(&good[..cut]),
                Err(NexusAmxContextError::Truncated),
                "{cut}"
            );
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(
            decode_nexus_amx_context_v1(&trailing),
            Err(NexusAmxContextError::Truncated)
        );
        let mut extra = NexusAmxContextWriterV1(good.clone());
        extra.field(
            "nexus.committed_catalog_policy.v1",
            &iroha_crypto::Hash::new(b"x"),
        );
        assert_eq!(
            decode_nexus_amx_context_v1(&extra.finish()),
            Err(NexusAmxContextError::TrailingField(
                "nexus.committed_catalog_policy.v1".to_owned()
            ))
        );
        assert_eq!(
            decode_nexus_amx_context_v1(&[]),
            Err(NexusAmxContextError::Oversize)
        );
        let mut oversize = good;
        oversize.resize(MAX_NEXUS_AMX_CONTEXT_BYTES_V1 + 1, 0);
        assert_eq!(
            decode_nexus_amx_context_v1(&oversize),
            Err(NexusAmxContextError::Oversize)
        );
    }

    #[test]
    fn grammar_rejects_unknown_and_reordered_tags_and_noncanonical_values() {
        let good = sample();
        let fields = Fields::tokenize(&good).unwrap().fields;
        let rebuild = |fields: &[(&str, Vec<u8>)]| {
            let mut out = NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec();
            for (tag, value) in fields {
                out.extend_from_slice(&u32::try_from(tag.len()).unwrap().to_le_bytes());
                out.extend_from_slice(tag.as_bytes());
                out.extend_from_slice(&(value.len() as u64).to_le_bytes());
                out.extend_from_slice(value);
            }
            out
        };
        let owned: Vec<(&str, Vec<u8>)> = fields.iter().map(|(t, v)| (*t, v.to_vec())).collect();
        assert!(decode_nexus_amx_context_v1(&rebuild(&owned)).is_ok());
        let mut unknown = owned.clone();
        unknown[0].0 = "nexus.lane_catalog.lane_bound";
        assert!(matches!(
            decode_nexus_amx_context_v1(&rebuild(&unknown)),
            Err(NexusAmxContextError::UnexpectedTag {
                expected: tag::LANE_COUNT,
                ..
            })
        ));
        let mut reordered = owned.clone();
        reordered.swap(0, 1);
        assert!(matches!(
            decode_nexus_amx_context_v1(&rebuild(&reordered)),
            Err(NexusAmxContextError::UnexpectedTag { .. })
        ));
        let mut missing = owned.clone();
        missing.pop();
        assert!(matches!(
            decode_nexus_amx_context_v1(&rebuild(&missing)),
            Err(NexusAmxContextError::UnexpectedTag {
                expected: tag::STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS,
                found: None
            })
        ));
        let mut noncanonical = owned.clone();
        noncanonical[0].1.push(0);
        assert_eq!(
            decode_nexus_amx_context_v1(&rebuild(&noncanonical)),
            Err(NexusAmxContextError::NonCanonical(tag::LANE_COUNT))
        );
        let mut huge_count = owned;
        let index = huge_count
            .iter()
            .position(|(tag, _)| *tag == tag::ROUTING_RULE_COUNT)
            .unwrap();
        huge_count[index].1 = u64::MAX.encode();
        assert!(matches!(
            decode_nexus_amx_context_v1(&rebuild(&huge_count)),
            Err(NexusAmxContextError::Structure(_))
        ));
    }

    #[test]
    fn structure_rejects_duplicates_unknown_dataspaces_and_mismatched_rules() {
        let universal = DataSpaceId::UNIVERSAL;
        let bpng = DataSpaceId::new(9);
        let lanes = [lane(0, "core", universal), lane(5, "bpng", bpng)];
        for (dataspaces, rules, label) in [
            (
                vec![(0, "universal")],
                vec![],
                "lane with an unknown dataspace",
            ),
            (
                vec![(0, "universal"), (9, "universal")],
                vec![],
                "duplicate dataspace alias",
            ),
            (
                vec![(0, "universal"), (9, "bpng")],
                vec![(4, Some(9), Some("*@bpng"), None)],
                "rule names an unknown lane",
            ),
            (
                vec![(0, "universal"), (9, "bpng")],
                vec![(5, Some(0), Some("*@bpng"), None)],
                "rule pairs a lane with a foreign dataspace",
            ),
        ] {
            let preimage = preimage_with(&lanes, &dataspaces, &rules, (false, 1, 8));
            assert!(
                matches!(
                    decode_nexus_amx_context_v1(&preimage),
                    Err(NexusAmxContextError::Structure(_))
                ),
                "{label}"
            );
        }
        // Duplicate lane ids cannot come from a LaneCatalog; splice a duplicated projection.
        let catalog = LaneCatalog::new(NonZeroU32::new(8).unwrap(), lanes.to_vec()).unwrap();
        let (_, mut projected) = catalog.consensus_projection();
        projected.push(projected[1].clone());
        let good = preimage_with(&lanes, &[(0, "universal"), (9, "bpng")], &[], (false, 1, 8));
        let mut tokens: Vec<(&str, Vec<u8>)> = Fields::tokenize(&good)
            .unwrap()
            .fields
            .iter()
            .map(|(t, v)| (*t, v.to_vec()))
            .collect();
        tokens[1].1 = projected.encode();
        let mut out = NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec();
        for (tag, value) in &tokens {
            out.extend_from_slice(&u32::try_from(tag.len()).unwrap().to_le_bytes());
            out.extend_from_slice(tag.as_bytes());
            out.extend_from_slice(&(value.len() as u64).to_le_bytes());
            out.extend_from_slice(value);
        }
        assert!(matches!(
            decode_nexus_amx_context_v1(&out),
            Err(NexusAmxContextError::Structure(_))
        ));
    }
    #[test]
    fn retired_staking_and_fee_tags_are_rejected_by_the_canonical_grammar() {
        let preimage = sample();
        let fields = Fields::tokenize(&preimage).unwrap().fields;
        for (expected, retired) in [
            (
                tag::STAKING_MAX_VALIDATORS,
                "nexus.staking.min_validator_stake",
            ),
            (tag::ROOT_FEE_POLICY, "nexus.fees.asset"),
        ] {
            let mut changed = NEXUS_AMX_CONTEXT_DOMAIN_V1.to_vec();
            for &(field, value) in &fields {
                let field = if field == expected { retired } else { field };
                changed.extend_from_slice(&u32::try_from(field.len()).unwrap().to_le_bytes());
                changed.extend_from_slice(field.as_bytes());
                changed.extend_from_slice(&u64::try_from(value.len()).unwrap().to_le_bytes());
                changed.extend_from_slice(value);
            }
            assert_eq!(
                decode_nexus_amx_context_v1(&changed),
                Err(NexusAmxContextError::UnexpectedTag {
                    expected,
                    found: Some(retired.to_owned()),
                })
            );
        }
    }
}
