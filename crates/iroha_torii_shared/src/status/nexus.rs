//! Nexus status wire records.
use iroha_schema::IntoSchema;
use norito::{
    core::DecodeFromSlice,
    derive::{NoritoDeserialize, NoritoSerialize},
    json::JsonSerialize,
};

/// TEU bucket contributions for a lane envelope (per slot).
#[allow(missing_copy_implementations)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct NexusLaneTeuBuckets {
    /// TEU sourced from configured per-lane floor allocation.
    pub floor: u64,
    /// TEU sourced from headroom scheduling after floor reservations.
    pub headroom: u64,
    /// TEU consumed by the must-serve slice (starvation guard).
    pub must_serve: u64,
    /// TEU consumed after circuit-breaker adjustments (caps lowered).
    pub circuit_breaker: u64,
}
impl NexusLaneTeuBuckets {
    const LABELS: [&'static str; 4] = ["floor", "headroom", "must_serve", "circuit_breaker"];
    /// Returns an iterator over bucket labels paired with their TEU amounts.
    pub fn iter(self) -> impl Iterator<Item = (&'static str, u64)> {
        [
            (Self::LABELS[0], self.floor),
            (Self::LABELS[1], self.headroom),
            (Self::LABELS[2], self.must_serve),
            (Self::LABELS[3], self.circuit_breaker),
        ]
        .into_iter()
    }
}
impl norito::core::NoritoSerialize for NexusLaneTeuBuckets {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusLaneTeuBuckets")
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.floor,
            self.headroom,
            self.must_serve,
            self.circuit_breaker,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneTeuBuckets {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusLaneTeuBuckets")
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (floor, headroom, must_serve, circuit_breaker) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            floor,
            headroom,
            must_serve,
            circuit_breaker,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusLaneTeuBuckets {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((floor, headroom, must_serve, circuit_breaker), used) =
            <(u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                floor,
                headroom,
                must_serve,
                circuit_breaker,
            },
            used,
        ))
    }
}
/// Fixed-length histogram for scheduler layer widths.
#[derive(Clone, Copy, Debug, Default, IntoSchema)]
pub struct SchedulerLayerWidthBuckets {
    buckets: [u64; 8],
}
impl SchedulerLayerWidthBuckets {
    /// Construct from an exact array of buckets.
    pub const fn new(buckets: [u64; 8]) -> Self {
        Self { buckets }
    }
    /// Construct from an arbitrary slice, truncating or zero-padding as needed.
    pub fn from_slice(values: &[u64]) -> Self {
        let mut buckets = [0u64; 8];
        let len = values.len().min(8);
        buckets[..len].copy_from_slice(&values[..len]);
        Self { buckets }
    }
    /// Convert into the inner array.
    pub const fn into_inner(self) -> [u64; 8] {
        self.buckets
    }
    /// Borrow the buckets slice.
    pub const fn as_slice(&self) -> &[u64; 8] {
        &self.buckets
    }
    /// Return the buckets as a `Vec`.
    pub fn to_vec(self) -> Vec<u64> {
        self.buckets.to_vec()
    }
}
impl From<[u64; 8]> for SchedulerLayerWidthBuckets {
    fn from(value: [u64; 8]) -> Self {
        Self::new(value)
    }
}
impl norito::json::FastJsonWrite for SchedulerLayerWidthBuckets {
    fn write_json(&self, out: &mut String) {
        out.push('[');
        for (idx, value) in self.buckets.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            value.json_serialize(out);
        }
        out.push(']');
    }
}
impl norito::json::JsonDeserialize for SchedulerLayerWidthBuckets {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let values = Vec::<u64>::json_deserialize(parser)?;
        if values.len() != 8 {
            return Err(norito::json::Error::Message(format!(
                "expected 8 histogram buckets, got {}",
                values.len()
            )));
        }
        let mut buckets = [0u64; 8];
        buckets.copy_from_slice(values.as_slice());
        Ok(Self { buckets })
    }
}
impl norito::core::NoritoSerialize for SchedulerLayerWidthBuckets {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::SchedulerLayerWidthBuckets")
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.buckets[0],
            self.buckets[1],
            self.buckets[2],
            self.buckets[3],
            self.buckets[4],
            self.buckets[5],
            self.buckets[6],
            self.buckets[7],
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for SchedulerLayerWidthBuckets {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::SchedulerLayerWidthBuckets")
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (b0, b1, b2, b3, b4, b5, b6, b7) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            buckets: [b0, b1, b2, b3, b4, b5, b6, b7],
        }
    }
}
impl<'a> DecodeFromSlice<'a> for SchedulerLayerWidthBuckets {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((b0, b1, b2, b3, b4, b5, b6, b7), used) =
            <(u64, u64, u64, u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                buckets: [b0, b1, b2, b3, b4, b5, b6, b7],
            },
            used,
        ))
    }
}
impl std::ops::Index<usize> for SchedulerLayerWidthBuckets {
    type Output = u64;
    fn index(&self, index: usize) -> &Self::Output {
        &self.buckets[index]
    }
}
/// TEU deferral counters per lane.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct NexusLaneTeuDeferrals {
    /// Deferred because the lane exceeded its configured TEU cap.
    pub cap_exceeded: u64,
    /// Deferred because the slot envelope hit a hard limit (e.g., bytes, witnesses).
    pub envelope_limit: u64,
    /// Deferred because per-dataspace or per-group quota limits triggered.
    pub quota: u64,
    /// Deferred because a circuit-breaker lowered the cap.
    pub circuit_breaker: u64,
}
impl NexusLaneTeuDeferrals {
    /// Increments the deferral counter corresponding to the provided reason.
    pub fn increment(&mut self, reason: &str, amount: u64) {
        match reason {
            "cap_exceeded" => self.cap_exceeded = self.cap_exceeded.saturating_add(amount),
            "envelope_limit" => {
                self.envelope_limit = self.envelope_limit.saturating_add(amount);
            }
            "quota" => self.quota = self.quota.saturating_add(amount),
            "circuit_breaker" => {
                self.circuit_breaker = self.circuit_breaker.saturating_add(amount);
            }
            _ => {}
        }
    }
}
impl norito::core::NoritoSerialize for NexusLaneTeuDeferrals {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusLaneTeuDeferrals")
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.cap_exceeded,
            self.envelope_limit,
            self.quota,
            self.circuit_breaker,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneTeuDeferrals {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusLaneTeuDeferrals")
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (cap_exceeded, envelope_limit, quota, circuit_breaker) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            cap_exceeded,
            envelope_limit,
            quota,
            circuit_breaker,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusLaneTeuDeferrals {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((cap_exceeded, envelope_limit, quota, circuit_breaker), used) =
            <(u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                cap_exceeded,
                envelope_limit,
                quota,
                circuit_breaker,
            },
            used,
        ))
    }
}
/// Snapshot of per-lane TEU scheduling state exposed via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusLaneTeuStatus")]
pub struct NexusLaneTeuStatus {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Configured TEU capacity for the current slot.
    pub capacity: u64,
    /// TEU committed in the latest slot envelope for this lane.
    pub committed: u64,
    /// Bucket breakdown for committed TEU.
    pub buckets: NexusLaneTeuBuckets,
    /// Aggregated TEU deferral counters.
    pub deferrals: NexusLaneTeuDeferrals,
    /// Number of times the must-serve slice was truncated (cumulative).
    pub must_serve_truncations: u64,
    /// Current circuit-breaker trigger level (0 = normal).
    pub trigger_level: u64,
    /// Starvation bound configured for this lane (in slots).
    pub starvation_bound_slots: u64,
    /// Latest block height recorded for this lane.
    pub block_height: u64,
    /// Slots since this lane last reached the global head height.
    pub finality_lag_slots: u64,
    /// Pending settlement backlog for this lane (micro XOR units).
    pub settlement_backlog_xor_micro: u128,
    /// Transactions executed in the latest block for this lane.
    pub tx_vertices: u64,
    /// Conflict edges among transactions executed for this lane.
    pub tx_edges: u64,
    /// Overlay chunks applied for this lane.
    pub overlay_count: u64,
    /// Total overlay instructions executed for this lane.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed for this lane.
    pub overlay_bytes_total: u64,
    /// Approximate number of RBC chunks attributed to this lane.
    pub rbc_chunks: u64,
    /// Approximate total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
    /// Peak scheduler layer width observed for this lane.
    pub peak_layer_width: u64,
    /// Number of scheduler layers executed for this lane.
    pub layer_count: u64,
    /// Average scheduler layer width (rounded) for this lane.
    pub avg_layer_width: u64,
    /// Median scheduler layer width for this lane.
    pub median_layer_width: u64,
    /// Scheduler utilization percentage (0..100) for this lane.
    pub scheduler_utilization_pct: u64,
    /// Histogram buckets for scheduler layer widths (le = [1,2,4,8,16,32,64,128]).
    pub layer_width_buckets: SchedulerLayerWidthBuckets,
    /// Detached overlay executions prepared in the latest block.
    pub detached_prepared: u64,
    /// Detached overlay merges applied in the latest block.
    pub detached_merged: u64,
    /// Detached overlay fallbacks applied in the latest block.
    pub detached_fallback: u64,
    /// Quarantine transactions executed for this lane.
    pub quarantine_executed: u64,
    /// Whether the lane's governance configuration requires a manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded for the lane.
    pub manifest_ready: bool,
    /// Human-readable alias for the lane.
    pub alias: String,
    /// Dataspace identifier associated with the lane.
    pub dataspace_id: u64,
    /// Dataspace alias associated with the lane.
    pub dataspace_alias: Option<String>,
    /// Declarative lane visibility derived from configuration.
    pub visibility: Option<String>,
    /// Storage profile configured for the lane.
    pub storage_profile: String,
    /// Declarative lane profile/type derived from configuration.
    pub lane_type: Option<String>,
    /// Governance module identifier attached to the lane.
    pub governance: Option<String>,
    /// Settlement policy identifier attached to the lane.
    pub settlement: Option<String>,
    /// Optional scheduler TEU capacity override advertised via lane metadata.
    pub scheduler_teu_capacity_override: Option<u64>,
    /// Optional scheduler starvation bound override advertised via lane metadata.
    pub scheduler_starvation_bound_override: Option<u64>,
    /// Source path of the active governance manifest, if available.
    pub manifest_path: Option<String>,
    /// Validators declared in the lane's governance manifest.
    pub manifest_validators: Vec<String>,
    /// Validator-account, peer, and Torii bindings declared by the manifest.
    pub manifest_validator_bindings: Vec<NexusLaneManifestValidatorBindingStatus>,
    /// Validator quorum required by the lane manifest.
    pub manifest_quorum: Option<u32>,
    /// Protected namespaces enforced by the lane manifest.
    pub manifest_protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook snapshot when configured.
    pub manifest_runtime_upgrade: Option<NexusLaneRuntimeUpgradeHookStatus>,
}
/// Configured dataspace entry exposed through `/status` for preflight checks.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusDataspaceCatalogStatus")]
pub struct NexusDataspaceCatalogStatus {
    /// Numeric lane identifier that services this dataspace.
    pub lane_id: u32,
    /// Human-readable lane alias.
    pub lane_alias: String,
    /// Numeric dataspace identifier.
    pub dataspace_id: u64,
    /// Human-readable dataspace alias.
    pub alias: String,
    /// Declarative lane visibility.
    pub visibility: String,
    /// Storage profile configured for the lane.
    pub storage_profile: String,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether the required governance manifest is loaded.
    pub manifest_ready: bool,
    /// Whether the lane is sealed because the manifest is not ready.
    pub sealed: bool,
    /// Source path of the active governance manifest, if available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    /// Protected namespaces enforced by the lane manifest.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub protected_namespaces: Vec<String>,
}
/// Effective Nexus routing policy exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusRoutingPolicyStatus")]
pub struct NexusRoutingPolicyStatus {
    /// Lane used when no policy rule matches.
    pub default_lane: u32,
    /// Dataspace used when no policy rule overrides it explicitly.
    pub default_dataspace: u64,
    /// Ordered routing rules evaluated by Nexus.
    pub rules: Vec<NexusRoutingRuleStatus>,
}
/// Effective Nexus routing rule exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusRoutingRuleStatus")]
pub struct NexusRoutingRuleStatus {
    /// Target lane identifier for the rule.
    pub lane: u32,
    /// Target dataspace identifier for the rule, when explicitly configured.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace_id: Option<u64>,
    /// Rule matcher.
    pub matcher: NexusRoutingMatcherStatus,
}
/// Nexus routing rule matcher exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusRoutingMatcherStatus")]
pub struct NexusRoutingMatcherStatus {
    /// Optional authority/account string match.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Optional instruction label match.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Optional operator-facing description.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
/// Nexus status snapshot exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "iroha_telemetry::metrics::NexusStatus")]
pub struct NexusStatus {
    /// Effective routing policy enforced by Nexus routing.
    pub routing_policy: NexusRoutingPolicyStatus,
}
/// Snapshot of per-dataspace scheduler state exposed via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct NexusDataspaceTeuStatus {
    /// Numeric lane identifier for the dataspace queue.
    pub lane_id: u32,
    /// Numeric dataspace identifier within the lane.
    pub dataspace_id: u64,
    /// Fault tolerance value (f) used to size lane relay committees.
    pub fault_tolerance: u32,
    /// Pending TEU demand left after scheduling the slot envelope.
    pub backlog: u64,
    /// Slots since the dataspace was last served.
    pub age_slots: u64,
    /// Latest SFQ virtual-finish tag for audit/debugging.
    pub virtual_finish: u64,
    /// Cumulative transactions executed for this dataspace since node start.
    pub tx_served: u64,
    /// Human-readable alias for the dataspace.
    pub alias: String,
    /// Optional description provided in configuration.
    pub description: Option<String>,
}
impl norito::core::NoritoSerialize for NexusDataspaceTeuStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusDataspaceTeuStatus")
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.lane_id,
            self.dataspace_id,
            self.fault_tolerance,
            self.backlog,
            self.age_slots,
            self.virtual_finish,
            self.tx_served,
            self.alias.clone(),
            self.description.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusDataspaceTeuStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name("iroha_telemetry::metrics::NexusDataspaceTeuStatus")
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (
            lane_id,
            dataspace_id,
            fault_tolerance,
            backlog,
            age_slots,
            virtual_finish,
            tx_served,
            alias,
            description,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            lane_id,
            dataspace_id,
            fault_tolerance,
            backlog,
            age_slots,
            virtual_finish,
            tx_served,
            alias,
            description,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusDataspaceTeuStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                lane_id,
                dataspace_id,
                fault_tolerance,
                backlog,
                age_slots,
                virtual_finish,
                tx_served,
                alias,
                description,
            ),
            used,
        ) = <(u32, u64, u32, u64, u64, u64, u64, String, Option<String>)>::decode_from_slice(
            bytes,
        )?;
        Ok((
            Self {
                lane_id,
                dataspace_id,
                fault_tolerance,
                backlog,
                age_slots,
                virtual_finish,
                tx_served,
                alias,
                description,
            },
            used,
        ))
    }
}

/// Schema-closed manifest validator binding exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct NexusLaneManifestValidatorBindingStatus {
    /// Canonical validator authority account.
    pub validator: String,
    /// Canonical consensus and routed-traffic peer identity.
    pub peer_id: String,
    /// Torii base URL declared for authoritative HTTP routing.
    #[norito(required)]
    pub torii_url: Option<String>,
}

impl norito::core::NoritoSerialize for NexusLaneManifestValidatorBindingStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(
            "iroha_telemetry::metrics::manifest_status::NexusLaneManifestValidatorBindingStatus",
        )
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.validator.clone(),
            self.peer_id.clone(),
            self.torii_url.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneManifestValidatorBindingStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(
            "iroha_telemetry::metrics::manifest_status::NexusLaneManifestValidatorBindingStatus",
        )
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (validator, peer_id, torii_url) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            validator,
            peer_id,
            torii_url,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for NexusLaneManifestValidatorBindingStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((validator, peer_id, torii_url), used) =
            <(String, String, Option<String>)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                validator,
                peer_id,
                torii_url,
            },
            used,
        ))
    }
}

/// Snapshot of the runtime-upgrade governance hook declared in a lane manifest.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct NexusLaneRuntimeUpgradeHookStatus {
    /// Whether runtime-upgrade instructions are permitted.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include manifest metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata identifiers declared by the manifest.
    pub allowed_ids: Vec<String>,
}

impl norito::core::NoritoSerialize for NexusLaneRuntimeUpgradeHookStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(
            "iroha_telemetry::metrics::manifest_status::NexusLaneRuntimeUpgradeHookStatus",
        )
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.allow,
            self.require_metadata,
            self.metadata_key.clone(),
            self.allowed_ids.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(
            "iroha_telemetry::metrics::manifest_status::NexusLaneRuntimeUpgradeHookStatus",
        )
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (allow, require_metadata, metadata_key, allowed_ids) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            allow,
            require_metadata,
            metadata_key,
            allowed_ids,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((allow, require_metadata, metadata_key, allowed_ids), used) =
            <(bool, bool, Option<String>, Vec<String>)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                allow,
                require_metadata,
                metadata_key,
                allowed_ids,
            },
            used,
        ))
    }
}
