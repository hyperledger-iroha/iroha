/// Active opt-in validator host capability advert for authoritative Inrou placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouHostCapabilityRecordV1 {
    /// Schema version; must equal [`SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Validator account that owns this host advert.
    pub validator_account_id: AccountId,
    /// Peer identifier used for Torii routing.
    pub peer_id: String,
    /// Supported guest ISAs for locally materialized replicas.
    pub supported_guest_isas: BTreeSet<SoraInrouGuestIsaV1>,
    /// Exact operator-approved guest artifact accepted by this host.
    pub trusted_guest_artifact: SoraPublishedInrouGuestImageArtifactV1,
    /// Maximum number of concurrently hosted placed replicas.
    pub max_hosted_replica_capacity: u16,
    /// Maximum aggregate physical host CPU reservation, including VMM overhead, in millicores.
    pub max_cpu_millis: u32,
    /// Maximum aggregate physical host memory reservation, including VMM overhead, in bytes.
    pub max_memory_bytes: u64,
    /// Maximum aggregate hosted writable storage budget in bytes.
    pub max_storage_bytes: u64,
    /// Timestamp when the advert was last refreshed.
    pub advertised_at_ms: u64,
    /// Timestamp after which the advert is no longer eligible without a refresh.
    pub heartbeat_expires_at_ms: u64,
}
impl SoraInrouHostCapabilityRecordV1 {
    /// Validate the authoritative Inrou host advert.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required fields are empty or capacity
    /// invariants are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou host capability record",
            self.schema_version,
            SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
        )?;
        validate_validator_account_peer_id(
            "sora inrou host capability record",
            &self.validator_account_id,
            &self.peer_id,
        )?;
        if self.supported_guest_isas.len() != 1 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "supported_guest_isas",
                "must contain exactly the qualified host-native guest ISA",
            ));
        }
        self.trusted_guest_artifact.validate()?;
        if self.advertised_at_ms == 0 || self.heartbeat_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "advertised_at_ms",
                "advertised_at_ms and heartbeat_expires_at_ms must be greater than zero",
            ));
        }
        if self.heartbeat_expires_at_ms <= self.advertised_at_ms {
            return Err(invalid_field(
                "sora inrou host capability record",
                "heartbeat_expires_at_ms",
                "must be greater than advertised_at_ms",
            ));
        }
        if self.max_hosted_replica_capacity != SORA_INROU_HOSTED_REPLICA_CAPACITY_V1 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "max_hosted_replica_capacity",
                "must equal the first-release capacity of one",
            ));
        }
        let minimum_cpu_millis =
            u64::from(SORA_INROU_MIN_CPU_MILLIS_V1) + SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1;
        if u64::from(self.max_cpu_millis) < minimum_cpu_millis {
            return Err(invalid_field(
                "sora inrou host capability record",
                "max_cpu_millis",
                "must cover at least one minimum guest plus fixed VMM CPU overhead",
            ));
        }
        let minimum_memory_bytes =
            SORA_INROU_MIN_MEMORY_BYTES_V1 + SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1;
        if self.max_memory_bytes < minimum_memory_bytes {
            return Err(invalid_field(
                "sora inrou host capability record",
                "max_memory_bytes",
                "must cover at least one minimum guest plus fixed VMM memory overhead",
            ));
        }
        if self.max_storage_bytes < SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1 {
            return Err(invalid_field(
                "sora inrou host capability record",
                "max_storage_bytes",
                "must cover at least one minimum guest ephemeral-storage allocation",
            ));
        }
        Ok(())
    }
    /// Return whether the advert remains eligible at the supplied timestamp.
    #[must_use]
    pub fn is_active_at(&self, now_ms: u64) -> bool {
        self.heartbeat_expires_at_ms > now_ms
    }
    /// Return whether the advert may host placed replicas at the supplied timestamp.
    #[must_use]
    pub fn can_host_replicas_at(&self, now_ms: u64) -> bool {
        self.validate().is_ok() && self.is_active_at(now_ms)
    }
}
/// Whether the exact host assigned to an Inrou replica remains eligible to run it.
///
/// Inrou V1 never reassigns a stateful replica during its economic lease. An unavailable
/// assignment therefore retains its validator, peer, guest ISA, and placement incarnation so the
/// exact original host can reactivate it after recovering eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "availability", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraInrouReplicaHostAvailabilityV1 {
    /// The exact assigned host advert is currently eligible for this replica.
    Available,
    /// The exact assigned host advert is not currently eligible; the assignment is fail-stop.
    Unavailable,
}
impl SoraInrouReplicaHostAvailabilityV1 {
    /// Return whether the exact assigned host may run and serve this replica.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}
/// Authoritative host assignment for one placed Inrou replica slot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouReplicaPlacementV1 {
    /// One-based replica slot within the selected service revision.
    pub replica_slot: u16,
    /// Explicit clock domain for the encoded economic lease incarnation.
    pub economic_clock: SoraServiceLeaseClockV1,
    /// Canonical block height identifying the economic lease incarnation.
    pub lease_started_height: u64,
    /// Transaction-bound incarnation of this slot's host assignment within the active service lease.
    pub placement_incarnation: Hash,
    /// Current eligibility of this exact assigned host.
    pub host_availability: SoraInrouReplicaHostAvailabilityV1,
    /// Validator assigned to materialize the replica.
    pub validator_account_id: AccountId,
    /// Peer identifier used for Torii proxy routing.
    pub peer_id: String,
    /// Guest ISA selected locally on the assigned host.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
}
impl SoraInrouReplicaPlacementV1 {
    /// Validate one placed Inrou replica assignment.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required routing metadata is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.replica_slot == 0 {
            return Err(invalid_field(
                "sora inrou replica placement",
                "replica_slot",
                "must be greater than zero",
            ));
        }
        if self.lease_started_height == 0 {
            return Err(invalid_field(
                "sora inrou replica placement",
                "lease_started_height",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora inrou replica placement",
            "placement_incarnation",
            self.placement_incarnation,
        )?;
        validate_validator_account_peer_id(
            "sora inrou replica placement",
            &self.validator_account_id,
            &self.peer_id,
        )?;
        Ok(())
    }
}
/// Authoritative per-revision placement record for hosted Inrou replicas.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouServicePlacementRecordV1 {
    /// Schema version; must equal [`SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose placed replicas are being tracked.
    pub service_name: Name,
    /// Service revision/version covered by this record.
    pub service_version: String,
    /// Desired replica count declared by the admitted service manifest.
    pub desired_replica_count: u16,
    /// Number of currently active eligible validators considered during reconciliation.
    pub eligible_validator_count: u32,
    /// Assigned replica placements in deterministic slot order.
    pub placements: Vec<SoraInrouReplicaPlacementV1>,
    /// Timestamp of the last placement reconciliation.
    pub reconciled_at_ms: u64,
    /// Latest placement error, when not all requested slots could be assigned.
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraInrouServicePlacementRecordV1 {
    /// Validate the authoritative Inrou placement record.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or slot
    /// assignments are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou service placement record",
            self.schema_version,
            SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
        )?;
        validate_exact_token(
            "sora inrou service placement record",
            "service_version",
            &self.service_version,
        )?;
        if self.desired_replica_count == 0 {
            return Err(invalid_field(
                "sora inrou service placement record",
                "desired_replica_count",
                "must be greater than zero",
            ));
        }
        if self.reconciled_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou service placement record",
                "reconciled_at_ms",
                "must be greater than zero",
            ));
        }
        if self.placements.len() > usize::from(self.desired_replica_count) {
            return Err(invalid_field(
                "sora inrou service placement record",
                "placements",
                "placement count must not exceed desired_replica_count",
            ));
        }
        let available_placement_count = self
            .placements
            .iter()
            .filter(|placement| placement.host_availability.is_available())
            .count();
        if u32::try_from(available_placement_count)
            .expect("available placement count was bounded by desired_replica_count")
            > self.eligible_validator_count
        {
            return Err(invalid_field(
                "sora inrou service placement record",
                "placements",
                "available placement count must not exceed eligible_validator_count",
            ));
        }
        let mut seen_validators = BTreeSet::new();
        let mut seen_peer_ids = BTreeSet::new();
        let mut previous_slot = 0_u16;
        for placement in &self.placements {
            placement.validate()?;
            if placement.replica_slot <= previous_slot
                || placement.replica_slot > self.desired_replica_count
            {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora inrou service placement record",
                    field: "placements",
                    reason: format!(
                        "placements must use strictly increasing unique replica slots bounded by desired_replica_count {}; previous replica_slot {previous_slot}, found {}",
                        self.desired_replica_count, placement.replica_slot
                    ),
                });
            }
            previous_slot = placement.replica_slot;
            if !seen_validators.insert(placement.validator_account_id.clone()) {
                return Err(invalid_field(
                    "sora inrou service placement record",
                    "placements",
                    "each placement must use a distinct validator account",
                ));
            }
            if !seen_peer_ids.insert(placement.peer_id.clone()) {
                return Err(invalid_field(
                    "sora inrou service placement record",
                    "placements",
                    "each placement must use a distinct peer ID",
                ));
            }
        }
        Ok(())
    }
}
/// Authoritative canonical Hugging Face registry metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSourceRecordV1 {
    /// Schema version; must equal [`SORA_HF_SOURCE_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Stable canonical source identifier.
    pub source_id: Hash,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision used for this canonical source.
    pub resolved_revision: String,
    /// Block timestamp when the source was first admitted.
    pub created_at_ms: u64,
    /// Block timestamp of the last lifecycle mutation.
    pub updated_at_ms: u64,
}
impl SoraHfSourceRecordV1 {
    /// Validate canonical Hugging Face source metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf source record",
            self.schema_version,
            SORA_HF_SOURCE_RECORD_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf source record", "source_id", self.source_id)?;
        if !is_canonical_hf_repo_id_v1(&self.repo_id) {
            return Err(invalid_field(
                "sora hf source record",
                "repo_id",
                "must be one exact fully-qualified `namespace/repository` identifier",
            ));
        }
        if !is_canonical_hf_commit_oid_v1(&self.resolved_revision) {
            return Err(invalid_field(
                "sora hf source record",
                "resolved_revision",
                "must be the full 40-character lowercase hexadecimal commit OID",
            ));
        }
        let expected_source_id = derive_hf_source_id_v1(&self.repo_id, &self.resolved_revision)?;
        if self.source_id != expected_source_id {
            return Err(invalid_field(
                "sora hf source record",
                "source_id",
                "must equal the canonical repository-and-commit source identifier",
            ));
        }
        if self.created_at_ms == 0 || self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora hf source record",
                "created_at_ms",
                "created_at_ms and updated_at_ms must be greater than zero",
            ));
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(invalid_field(
                "sora hf source record",
                "updated_at_ms",
                "must be >= created_at_ms",
            ));
        }
        Ok(())
    }
}
/// Shared-lease pool lifecycle state for a canonical HF source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseStatusV1 {
    /// The pool is accepting joins against the current window.
    Active,
    /// The pool is draining after the last member left.
    Draining,
    /// The current window expired and requires a new sponsor.
    Expired,
    /// The pool was explicitly retired.
    Retired,
}
/// Shared-lease membership lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseMemberStatusV1 {
    /// The account actively participates in the current window.
    Active,
    /// The account left the pool or was expired out of the current window.
    Left,
}
/// Audit action recorded for shared Hugging Face lease windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraHfSharedLeaseActionV1 {
    /// A brand new window was opened by a sponsor.
    CreateWindow,
    /// An account joined an active window.
    Join,
    /// An active membership left the current window.
    Leave,
    /// A future or fresh window was sponsored.
    Renew,
    /// A queued window became active.
    Activate,
    /// A queued window reached activation but could not become active.
    ActivationFailed,
    /// The current window was retired early.
    Retire,
}
/// Queued next-window sponsorship metadata for a shared Hugging Face lease pool.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseQueuedWindowV1 {
    /// Account that sponsored the queued window and prepaid its storage charge.
    pub sponsor_account_id: AccountId,
    /// Settlement asset definition for the queued window.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal storage price charged at sponsorship.
    pub base_fee: Quantity,
    /// Timestamp when the queued sponsorship was recorded.
    pub sponsored_at_ms: u64,
    /// Planned start timestamp for the queued window.
    pub window_started_at_ms: u64,
    /// Planned expiry timestamp for the queued window.
    pub window_expires_at_ms: u64,
    /// Service binding that should be preserved for the queued sponsor.
    pub service_name: Name,
    /// Optional apartment binding that should be preserved for the queued sponsor.
    #[norito(required)]
    pub apartment_name: Option<Name>,
}
impl SoraHfSharedLeaseQueuedWindowV1 {
    /// Validate queued shared-lease sponsorship metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when timestamps, prices, or names are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.base_fee.is_zero() {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "base_fee",
                "must be greater than zero",
            ));
        }
        if self.sponsored_at_ms == 0
            || self.window_started_at_ms == 0
            || self.window_expires_at_ms == 0
        {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "sponsored_at_ms",
                "queued-window timestamps must be greater than zero",
            ));
        }
        if self.window_started_at_ms < self.sponsored_at_ms {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "window_started_at_ms",
                "must be greater than or equal to sponsored_at_ms",
            ));
        }
        if self.window_expires_at_ms <= self.window_started_at_ms {
            return Err(invalid_field(
                "sora hf shared lease queued window",
                "window_expires_at_ms",
                "must be greater than window_started_at_ms",
            ));
        }
        Ok(())
    }
}
/// Shared-lease pool metadata keyed by canonical source and pricing dimensions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeasePoolV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_POOL_VERSION_V1`].
    pub schema_version: u16,
    /// Stable pool identifier.
    pub pool_id: Hash,
    /// Canonical admitted source identifier.
    pub source_id: Hash,
    /// Storage class used by the shared lease.
    pub storage_class: StorageClass,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal price in `lease_asset_definition_id`.
    pub base_fee: Quantity,
    /// Shared window length in milliseconds.
    pub lease_term_ms: u64,
    /// Start timestamp for the active window.
    pub window_started_at_ms: u64,
    /// Expiry timestamp for the active window.
    pub window_expires_at_ms: u64,
    /// Number of currently active members.
    pub active_member_count: u32,
    /// Pool lifecycle status.
    pub status: SoraHfSharedLeaseStatusV1,
    /// Optional sponsorship for the next window after the current one expires.
    #[norito(required)]
    pub queued_next_window: Option<SoraHfSharedLeaseQueuedWindowV1>,
}
impl SoraHfSharedLeasePoolV1 {
    /// Validate shared-lease pool metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// time/price fields are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease pool",
            self.schema_version,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf shared lease pool", "pool_id", self.pool_id)?;
        validate_soracloud_digest_hash("sora hf shared lease pool", "source_id", self.source_id)?;
        if self.base_fee.is_zero() {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "base_fee",
                "must be greater than zero",
            ));
        }
        if self.lease_term_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "lease_term_ms",
                "must be greater than zero",
            ));
        }
        if self.window_started_at_ms == 0 || self.window_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "window_started_at_ms",
                "window timestamps must be greater than zero",
            ));
        }
        if self.window_expires_at_ms <= self.window_started_at_ms {
            return Err(invalid_field(
                "sora hf shared lease pool",
                "window_expires_at_ms",
                "must be greater than window_started_at_ms",
            ));
        }
        if let Some(next_window) = &self.queued_next_window {
            next_window.validate()?;
            if self.status != SoraHfSharedLeaseStatusV1::Active {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window",
                    "may only be set while the current window is active",
                ));
            }
            if next_window.window_started_at_ms != self.window_expires_at_ms {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window.window_started_at_ms",
                    "must match window_expires_at_ms",
                ));
            }
            if next_window.window_expires_at_ms
                != next_window
                    .window_started_at_ms
                    .saturating_add(self.lease_term_ms)
            {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window.window_expires_at_ms",
                    "must equal queued window_started_at_ms + lease_term_ms",
                ));
            }
            if next_window.lease_asset_definition_id != self.lease_asset_definition_id {
                return Err(invalid_field(
                    "sora hf shared lease pool",
                    "queued_next_window.lease_asset_definition_id",
                    "must match the pool settlement asset",
                ));
            }
        }
        Ok(())
    }
}
/// Account-scoped shared-lease membership plus free service/apartment bindings.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseMemberV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1`].
    pub schema_version: u16,
    /// Pool this membership belongs to.
    pub pool_id: Hash,
    /// Canonical admitted source identifier.
    pub source_id: Hash,
    /// Member account.
    pub account_id: AccountId,
    /// Membership lifecycle status.
    pub status: SoraHfSharedLeaseMemberStatusV1,
    /// Timestamp when the account first joined the current or previous windows.
    pub joined_at_ms: u64,
    /// Timestamp of the last mutation to this membership.
    pub updated_at_ms: u64,
    /// Total nominal amount charged to this member across joins/renewals.
    pub total_paid: Quantity,
    /// Total nominal amount refunded to this member by later joiners.
    pub total_refunded: Quantity,
    /// Most recent nominal direct charge applied to this member.
    pub last_charge: Quantity,
    /// Bound Soracloud services that reuse this membership.
    pub service_bindings: BTreeSet<String>,
    /// Bound Soracloud agent apartments that reuse this membership.
    pub apartment_bindings: BTreeSet<String>,
}
impl SoraHfSharedLeaseMemberV1 {
    /// Validate shared-lease membership metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// bindings contain invalid names.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease member",
            self.schema_version,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
        )?;
        validate_soracloud_digest_hash("sora hf shared lease member", "pool_id", self.pool_id)?;
        validate_soracloud_digest_hash("sora hf shared lease member", "source_id", self.source_id)?;
        if self.joined_at_ms == 0 || self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease member",
                "joined_at_ms",
                "joined_at_ms and updated_at_ms must be greater than zero",
            ));
        }
        if self.updated_at_ms < self.joined_at_ms {
            return Err(invalid_field(
                "sora hf shared lease member",
                "updated_at_ms",
                "must be >= joined_at_ms",
            ));
        }
        if self.total_refunded > self.total_paid {
            return Err(invalid_field(
                "sora hf shared lease member",
                "total_refunded",
                "must not exceed total_paid",
            ));
        }
        if self.last_charge > self.total_paid {
            return Err(invalid_field(
                "sora hf shared lease member",
                "last_charge",
                "must not exceed total_paid",
            ));
        }
        for service_name in &self.service_bindings {
            validate_exact_name_token(
                "sora hf shared lease member",
                "service_bindings",
                service_name,
            )?;
        }
        for apartment_name in &self.apartment_bindings {
            validate_exact_name_token(
                "sora hf shared lease member",
                "apartment_bindings",
                apartment_name,
            )?;
        }
        Ok(())
    }
}
/// Audit record for shared Hugging Face lease lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraHfSharedLeaseAuditEventV1 {
    /// Schema version; must equal [`SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Audit action that produced the event.
    pub action: SoraHfSharedLeaseActionV1,
    /// Pool affected by the event.
    pub pool_id: Hash,
    /// Canonical admitted source identifier.
    pub source_id: Hash,
    /// Account responsible for the lifecycle mutation.
    pub account_id: AccountId,
    /// Block timestamp of the mutation.
    pub occurred_at_ms: u64,
    /// Number of active members after the mutation.
    pub active_member_count: u32,
    /// Direct nominal amount charged to the acting account.
    pub charged: Quantity,
    /// Direct nominal refund amount recorded for the acting account.
    pub refunded: Quantity,
    /// Expiry of the lease window affected by the event.
    pub lease_expires_at_ms: u64,
    /// Terminal activation failure reason, present only for [`SoraHfSharedLeaseActionV1::ActivationFailed`].
    #[norito(default)]
    pub failure_reason: Option<String>,
    /// Optional service binding touched by the mutation.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Optional apartment binding touched by the mutation.
    #[norito(required)]
    pub apartment_name: Option<String>,
}
impl SoraHfSharedLeaseAuditEventV1 {
    /// Validate shared-lease audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// optional bindings contain empty names.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora hf shared lease audit event",
            self.schema_version,
            SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "sora hf shared lease audit event",
            "pool_id",
            self.pool_id,
        )?;
        validate_soracloud_digest_hash(
            "sora hf shared lease audit event",
            "source_id",
            self.source_id,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        if self.occurred_at_ms == 0 || self.lease_expires_at_ms == 0 {
            return Err(invalid_field(
                "sora hf shared lease audit event",
                "occurred_at_ms",
                "occurred_at_ms and lease_expires_at_ms must be greater than zero",
            ));
        }
        match self.action {
            SoraHfSharedLeaseActionV1::ActivationFailed => {
                if self
                    .failure_reason
                    .as_ref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(invalid_field(
                        "sora hf shared lease audit event",
                        "failure_reason",
                        "must be non-empty for activation failures",
                    ));
                }
            }
            _ if self.failure_reason.is_some() => {
                return Err(invalid_field(
                    "sora hf shared lease audit event",
                    "failure_reason",
                    "must be omitted unless action is activation_failed",
                ));
            }
            _ => {}
        }
        if let Some(service_name) = self.service_name.as_deref() {
            validate_exact_name_token(
                "sora hf shared lease audit event",
                "service_name",
                service_name,
            )?;
        }
        if let Some(apartment_name) = self.apartment_name.as_deref() {
            validate_exact_name_token(
                "sora hf shared lease audit event",
                "apartment_name",
                apartment_name,
            )?;
        }
        Ok(())
    }
}
/// Audit record for model-artifact lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelArtifactAuditEventV1 {
    /// Schema version; must equal [`SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Model-artifact action that produced the event.
    pub action: SoraModelArtifactActionV1,
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Active service revision when the event was emitted.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Training job associated with the artifact.
    pub training_job_id: String,
    /// Model weight version that consumed the artifact, when any.
    #[norito(required)]
    pub consumed_by_version: Option<String>,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
}
impl SoraModelArtifactAuditEventV1 {
    /// Validate model-artifact audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model artifact audit event",
            self.schema_version,
            SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora model artifact audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_name", self.model_name.as_str()),
            ("training_job_id", self.training_job_id.as_str()),
        ] {
            validate_exact_token("sora model artifact audit event", field, value)?;
        }
        if let Some(consumed_by_version) = self.consumed_by_version.as_deref() {
            validate_exact_token(
                "sora model artifact audit event",
                "consumed_by_version",
                consumed_by_version,
            )?;
        }
        Ok(())
    }
}
/// Audit action recorded for authoritative Soracloud agent-apartment state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAgentApartmentActionV1 {
    /// A new apartment was deployed.
    Deploy,
    /// An apartment lease was renewed.
    LeaseRenew,
    /// An apartment process was restarted.
    Restart,
    /// A wallet spend request was created but not yet approved.
    WalletSpendRequested,
    /// A wallet spend request was approved and applied.
    WalletSpendApproved,
    /// A policy capability was revoked.
    PolicyRevoked,
    /// A mailbox message was enqueued for delivery.
    MessageEnqueued,
    /// A mailbox message was acknowledged and consumed.
    MessageAcknowledged,
    /// An autonomy artifact was allowlisted.
    ArtifactAllowed,
    /// An autonomy run was approved and recorded.
    AutonomyRunApproved,
    /// An approved autonomy run completed execution and recorded a runtime outcome.
    AutonomyRunExecuted,
}
/// Runtime status of an authoritative Soracloud agent apartment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAgentRuntimeStatusV1 {
    /// Apartment lease is active and the process is considered runnable.
    Running,
    /// Apartment lease expired and must be renewed before further work.
    LeaseExpired,
}
/// Pending wallet spend request tracked inside an authoritative apartment record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentWalletSpendRequestV1 {
    /// Caller-supplied, replay-safe request identifier.
    pub request_id: String,
    /// Asset definition constrained by the apartment policy.
    pub asset_definition: String,
    /// Requested nominal spend amount.
    pub amount: Quantity,
    /// Audit sequence that created the request.
    pub created_sequence: u64,
}
/// Daily wallet-spend aggregate for an asset/day bucket pair.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentWalletDailySpendEntryV1 {
    /// Asset definition constrained by the apartment policy.
    pub asset_definition: String,
    /// Deterministic day bucket.
    pub day_bucket: u64,
    /// Total nominal quantity spent in that bucket.
    pub spent: Quantity,
}
/// Mailbox message queued for deterministic apartment-to-apartment delivery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentMailboxMessageV1 {
    /// Deterministic message identifier.
    pub message_id: String,
    /// Apartment that sent the message.
    pub from_apartment: String,
    /// Logical mailbox channel.
    pub channel: String,
    /// Message payload.
    pub payload: String,
    /// Canonical hash of the payload.
    pub payload_hash: Hash,
    /// Audit sequence that enqueued the message.
    pub enqueued_sequence: u64,
}
/// Allowlist entry authorizing an autonomy artifact for an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentArtifactAllowRuleV1 {
    /// Artifact hash that was approved.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Audit sequence that added the rule.
    pub added_sequence: u64,
}
/// Historical autonomy-run approval recorded for an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentAutonomyRunRecordV1 {
    /// Deterministic run identifier.
    pub run_id: String,
    /// Artifact hash executed by the run.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Approved budget units for the run.
    pub budget_units: u64,
    /// Human-readable run label.
    pub run_label: String,
    /// Optional canonical JSON input committed to the approved autonomy workflow.
    #[norito(required)]
    pub workflow_input_json: Option<String>,
    /// Apartment process generation active when the run was approved.
    pub approved_process_generation: u64,
    /// Deterministic runtime request commitment bound to the approved run.
    pub request_commitment: Hash,
    /// Audit sequence that approved the run.
    pub approved_sequence: u64,
}
/// Authoritative persistent-state accounting attached to an apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentPersistentStateV1 {
    /// Total bytes consumed by apartment-owned state.
    pub total_bytes: u64,
    /// Per-key size accounting for deterministic quota tracking.
    pub key_sizes: BTreeMap<String, u64>,
}
/// Authoritative runtime record for a Soracloud agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentApartmentRecordV1 {
    /// Schema version; must equal [`SORA_AGENT_APARTMENT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Admitted apartment manifest.
    pub manifest: AgentApartmentManifestV1,
    /// Canonical hash of the manifest.
    pub manifest_hash: Hash,
    /// Audit sequence that deployed the apartment.
    pub deployed_sequence: u64,
    /// Consensus block height when this apartment lease incarnation began.
    pub lease_started_height: u64,
    /// Consensus block height when the lease expires.
    pub lease_expires_height: u64,
    /// Consensus block height of the latest lease renewal.
    pub last_renewed_height: u64,
    /// Deterministic restart count.
    pub restart_count: u32,
    /// Audit sequence of the last restart, when any.
    #[norito(required)]
    pub last_restart_sequence: Option<u64>,
    /// Human-readable reason for the last restart, when any.
    #[norito(required)]
    pub last_restart_reason: Option<String>,
    /// Monotonic local-process generation.
    pub process_generation: u64,
    /// Audit sequence that started the current process generation.
    pub process_started_sequence: u64,
    /// Audit sequence of the most recent activity.
    pub last_active_sequence: u64,
    /// Audit sequence of the latest checkpoint, when any.
    #[norito(required)]
    pub last_checkpoint_sequence: Option<u64>,
    /// Number of recorded checkpoints.
    pub checkpoint_count: u32,
    /// Deterministic persistent-state accounting.
    pub persistent_state: SoraAgentPersistentStateV1,
    /// Revoked policy capabilities.
    pub revoked_policy_capabilities: BTreeSet<String>,
    /// Pending wallet requests keyed by request id.
    pub pending_wallet_requests: BTreeMap<String, SoraAgentWalletSpendRequestV1>,
    /// Daily wallet-spend aggregates keyed by `<asset>:<day_bucket>`.
    pub wallet_daily_spend: BTreeMap<String, SoraAgentWalletDailySpendEntryV1>,
    /// Pending mailbox queue for the apartment.
    pub mailbox_queue: Vec<SoraAgentMailboxMessageV1>,
    /// Total autonomy budget allocated to the apartment.
    pub autonomy_budget_ceiling_units: u64,
    /// Remaining autonomy budget units.
    pub autonomy_budget_remaining_units: u64,
    /// Approved artifact allowlist keyed by artifact hash.
    pub artifact_allowlist: BTreeMap<String, SoraAgentArtifactAllowRuleV1>,
    /// Historical autonomy-run approvals.
    pub autonomy_run_history: Vec<SoraAgentAutonomyRunRecordV1>,
}
impl SoraAgentApartmentRecordV1 {
    /// Derive the apartment runtime status in its current committed state view.
    ///
    /// The record must be paired with the current height of the same state
    /// view. It is not a historical query surface: after a renewal, the row no
    /// longer contains the prior expiry needed to reconstruct earlier lease
    /// gaps. Pairing a post-renewal row with an older view therefore fails
    /// closed.
    ///
    /// Lease intervals are half-open. A row is runnable at and after its
    /// latest renewal height, and strictly before `lease_expires_height`.
    #[must_use]
    pub fn runtime_status_at_current_height(
        &self,
        current_height: u64,
    ) -> SoraAgentRuntimeStatusV1 {
        if current_height >= self.last_renewed_height && current_height < self.lease_expires_height
        {
            SoraAgentRuntimeStatusV1::Running
        } else {
            SoraAgentRuntimeStatusV1::LeaseExpired
        }
    }

    /// Validate apartment lifecycle and deterministic-accounting invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the embedded manifest is
    /// invalid, or the recorded lifecycle/accounting state is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_required_fields()?;
        self.validate_restart_fields()?;
        self.validate_budget_fields()?;
        self.validate_collection_fields()
    }
    fn validate_required_fields(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora agent apartment record",
            self.schema_version,
            SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        )?;
        self.manifest.validate()?;
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "manifest_hash",
            self.manifest_hash,
        )?;
        if self.manifest_hash != self.manifest.manifest_hash() {
            return Err(invalid_field(
                "sora agent apartment record",
                "manifest_hash",
                "must match the canonical apartment manifest hash",
            ));
        }
        for (field, value) in [
            ("process_generation", self.process_generation),
            ("deployed_sequence", self.deployed_sequence),
            ("lease_started_height", self.lease_started_height),
            ("lease_expires_height", self.lease_expires_height),
            ("last_renewed_height", self.last_renewed_height),
            ("process_started_sequence", self.process_started_sequence),
            ("last_active_sequence", self.last_active_sequence),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora agent apartment record",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        if self.lease_expires_height <= self.lease_started_height {
            return Err(invalid_field(
                "sora agent apartment record",
                "lease_expires_height",
                "must be greater than lease_started_height",
            ));
        }
        if self.last_renewed_height < self.lease_started_height {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_renewed_height",
                "must be >= lease_started_height",
            ));
        }
        if self.last_renewed_height >= self.lease_expires_height {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_renewed_height",
                "must be less than lease_expires_height",
            ));
        }
        Ok(())
    }
    fn validate_restart_fields(&self) -> Result<(), SoracloudManifestError> {
        if self
            .last_restart_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_restart_reason",
                "must not be empty when provided",
            ));
        }
        if self.last_restart_sequence.is_some() != self.last_restart_reason.is_some() {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_restart",
                "last_restart_sequence and last_restart_reason must be populated together",
            ));
        }
        if self
            .last_checkpoint_sequence
            .is_some_and(|sequence| sequence == 0)
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "last_checkpoint_sequence",
                "must be greater than zero when provided",
            ));
        }
        Ok(())
    }
    fn validate_budget_fields(&self) -> Result<(), SoracloudManifestError> {
        if self.autonomy_budget_ceiling_units == 0 {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_budget_ceiling_units",
                "must be greater than zero",
            ));
        }
        if self.autonomy_budget_remaining_units > self.autonomy_budget_ceiling_units {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_budget_remaining_units",
                "must not exceed autonomy_budget_ceiling_units",
            ));
        }
        Ok(())
    }
    fn validate_collection_fields(&self) -> Result<(), SoracloudManifestError> {
        for revoked in &self.revoked_policy_capabilities {
            validate_nonempty_no_control(
                "sora agent apartment record",
                "revoked_policy_capabilities",
                revoked,
            )?;
        }
        for (request_id, request) in &self.pending_wallet_requests {
            Self::validate_pending_wallet_request(request_id, request)?;
        }
        for (key, entry) in &self.wallet_daily_spend {
            Self::validate_wallet_daily_spend_entry(key, entry)?;
        }
        for message in &self.mailbox_queue {
            Self::validate_mailbox_message(message)?;
        }
        for (artifact_hash, rule) in &self.artifact_allowlist {
            Self::validate_artifact_allowlist_rule(artifact_hash, rule)?;
        }
        for run in &self.autonomy_run_history {
            Self::validate_autonomy_run(run)?;
        }
        Ok(())
    }
    fn validate_pending_wallet_request(
        request_id: &str,
        request: &SoraAgentWalletSpendRequestV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_nonempty_no_control(
            "sora agent apartment record",
            "pending_wallet_requests.asset_definition",
            &request.asset_definition,
        )?;
        if request_id != request.request_id
            || !is_canonical_agent_wallet_request_id_v1(&request.request_id)
            || request.amount.is_zero()
            || request.created_sequence == 0
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "pending_wallet_requests",
                "wallet request entries must use non-empty matching request ids and valid metadata",
            ));
        }
        Ok(())
    }
    fn validate_wallet_daily_spend_entry(
        key: &str,
        entry: &SoraAgentWalletDailySpendEntryV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_nonempty_no_control("sora agent apartment record", "wallet_daily_spend.key", key)?;
        validate_nonempty_no_control(
            "sora agent apartment record",
            "wallet_daily_spend.asset_definition",
            &entry.asset_definition,
        )?;
        Ok(())
    }
    fn validate_mailbox_message(
        message: &SoraAgentMailboxMessageV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "mailbox_queue.payload_hash",
            message.payload_hash,
        )?;
        if message.payload_hash != Hash::new(message.payload.as_bytes()) {
            return Err(invalid_field(
                "sora agent apartment record",
                "mailbox_queue.payload_hash",
                "must match the canonical mailbox payload hash",
            ));
        }
        for (field, value) in [
            ("mailbox_queue.message_id", message.message_id.as_str()),
            (
                "mailbox_queue.from_apartment",
                message.from_apartment.as_str(),
            ),
            ("mailbox_queue.channel", message.channel.as_str()),
        ] {
            validate_nonempty_no_control("sora agent apartment record", field, value)?;
        }
        if message.enqueued_sequence == 0 {
            return Err(invalid_field(
                "sora agent apartment record",
                "mailbox_queue",
                "mailbox messages must use non-empty ids/origins/channels and valid sequences",
            ));
        }
        Ok(())
    }
    fn validate_artifact_allowlist_rule(
        artifact_hash: &str,
        rule: &SoraAgentArtifactAllowRuleV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_nonempty_no_control(
            "sora agent apartment record",
            "artifact_allowlist.artifact_hash",
            &rule.artifact_hash,
        )?;
        if let Some(provenance_hash) = rule.provenance_hash.as_deref() {
            validate_nonempty_no_control(
                "sora agent apartment record",
                "artifact_allowlist.provenance_hash",
                provenance_hash,
            )?;
        }
        if artifact_hash != rule.artifact_hash || rule.added_sequence == 0 {
            return Err(invalid_field(
                "sora agent apartment record",
                "artifact_allowlist",
                "allowlist entries must use non-empty matching artifact hashes and valid metadata",
            ));
        }
        Ok(())
    }
    fn validate_autonomy_run(
        run: &SoraAgentAutonomyRunRecordV1,
    ) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "sora agent apartment record",
            "autonomy_run_history.request_commitment",
            run.request_commitment,
        )?;
        for (field, value) in [
            ("autonomy_run_history.run_id", run.run_id.as_str()),
            (
                "autonomy_run_history.artifact_hash",
                run.artifact_hash.as_str(),
            ),
            ("autonomy_run_history.run_label", run.run_label.as_str()),
        ] {
            validate_nonempty_no_control("sora agent apartment record", field, value)?;
        }
        if let Some(provenance_hash) = run.provenance_hash.as_deref() {
            validate_nonempty_no_control(
                "sora agent apartment record",
                "autonomy_run_history.provenance_hash",
                provenance_hash,
            )?;
        }
        if run.budget_units == 0
            || run.approved_process_generation == 0
            || run.approved_sequence == 0
        {
            return Err(invalid_field(
                "sora agent apartment record",
                "autonomy_run_history",
                "autonomy run entries must use non-empty ids/hash/label plus positive budgets, process generations, and sequences",
            ));
        }
        if let Some(workflow_input_json) = run.workflow_input_json.as_deref() {
            if workflow_input_json.is_empty() || workflow_input_json.trim() != workflow_input_json {
                return Err(invalid_field(
                    "sora agent apartment record",
                    "autonomy_run_history",
                    "workflow_input_json must be a nonempty canonical JSON string",
                ));
            }
            let parsed = norito::json::from_str::<norito::json::Value>(workflow_input_json)
                .map_err(|error| SoracloudManifestError::InvalidField {
                    manifest: "sora agent apartment record",
                    field: "autonomy_run_history",
                    reason: format!("workflow_input_json must be valid JSON: {error}"),
                })?;
            let canonical = norito::json::to_json(&parsed).map_err(|error| {
                SoracloudManifestError::InvalidField {
                    manifest: "sora agent apartment record",
                    field: "autonomy_run_history",
                    reason: format!("workflow_input_json must serialize canonically: {error}"),
                }
            })?;
            if canonical != workflow_input_json {
                return Err(invalid_field(
                    "sora agent apartment record",
                    "autonomy_run_history",
                    "workflow_input_json must equal its canonical Norito JSON serialization",
                ));
            }
        }
        Ok(())
    }
}
/// Audit record for authoritative agent-apartment state transitions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAgentApartmentAuditEventV1 {
    /// Schema version; must equal [`SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Consensus block height that committed the transition.
    pub block_height: u64,
    /// Consensus block timestamp used for UTC-day spend accounting.
    pub block_timestamp_ms: u64,
    /// Agent-apartment action that produced the event.
    pub action: SoraAgentApartmentActionV1,
    /// Logical apartment identifier.
    pub apartment_name: Name,
    /// Resulting runtime status.
    pub status: SoraAgentRuntimeStatusV1,
    /// Lease-expiry consensus height after the event.
    pub lease_expires_height: u64,
    /// Hash of the admitted apartment manifest.
    pub manifest_hash: Hash,
    /// Restart count after the event.
    pub restart_count: u32,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
    /// Optional wallet request id associated with the event.
    #[norito(required)]
    pub request_id: Option<String>,
    /// Optional asset definition associated with the event.
    #[norito(required)]
    pub asset_definition: Option<String>,
    /// Optional nominal wallet amount associated with the event.
    #[norito(required)]
    pub amount: Option<Quantity>,
    /// Optional capability associated with the event.
    #[norito(required)]
    pub capability: Option<String>,
    /// Optional reason associated with the event.
    #[norito(required)]
    pub reason: Option<String>,
    /// Optional sender apartment associated with the event.
    #[norito(required)]
    pub from_apartment: Option<String>,
    /// Optional recipient apartment associated with the event.
    #[norito(required)]
    pub to_apartment: Option<String>,
    /// Optional mailbox channel associated with the event.
    #[norito(required)]
    pub channel: Option<String>,
    /// Optional payload hash associated with the event.
    #[norito(required)]
    pub payload_hash: Option<Hash>,
    /// Optional artifact hash associated with the event.
    #[norito(required)]
    pub artifact_hash: Option<String>,
    /// Optional provenance hash associated with the event.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Optional run id associated with the event.
    #[norito(required)]
    pub run_id: Option<String>,
    /// Optional run label associated with the event.
    #[norito(required)]
    pub run_label: Option<String>,
    /// Optional run budget associated with the event.
    #[norito(required)]
    pub budget_units: Option<u64>,
    /// Optional generated service name used for execution.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Optional generated service version used for execution.
    #[norito(required)]
    pub service_version: Option<String>,
    /// Optional service handler used for execution.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Optional execution result commitment recorded for the run.
    #[norito(required)]
    pub result_commitment: Option<Hash>,
    /// Optional authoritative runtime receipt identifier recorded for the run.
    #[norito(required)]
    pub runtime_receipt_id: Option<Hash>,
    /// Optional node-local journal artifact hash recorded for the run.
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    /// Optional checkpoint artifact hash recorded for the run.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional success flag recorded for executed autonomy runs.
    #[norito(required)]
    pub succeeded: Option<bool>,
}
impl SoraAgentApartmentAuditEventV1 {
    /// Validate agent-apartment audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora agent apartment audit event",
            self.schema_version,
            SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 || self.block_height == 0 || self.block_timestamp_ms == 0 {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "sequence",
                "sequence, block_height, and block_timestamp_ms must be greater than zero",
            ));
        }
        if self.lease_expires_height == 0 {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "lease_expires_height",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora agent apartment audit event",
            "manifest_hash",
            self.manifest_hash,
        )?;
        for (field, digest) in [
            ("payload_hash", self.payload_hash),
            ("result_commitment", self.result_commitment),
            ("runtime_receipt_id", self.runtime_receipt_id),
            ("journal_artifact_hash", self.journal_artifact_hash),
            ("checkpoint_artifact_hash", self.checkpoint_artifact_hash),
        ] {
            if let Some(digest) = digest {
                validate_soracloud_digest_hash("sora agent apartment audit event", field, digest)?;
            }
        }
        for (field, value) in [
            ("request_id", self.request_id.as_deref()),
            ("asset_definition", self.asset_definition.as_deref()),
            ("capability", self.capability.as_deref()),
            ("from_apartment", self.from_apartment.as_deref()),
            ("to_apartment", self.to_apartment.as_deref()),
            ("channel", self.channel.as_deref()),
            ("artifact_hash", self.artifact_hash.as_deref()),
            ("provenance_hash", self.provenance_hash.as_deref()),
            ("run_id", self.run_id.as_deref()),
            ("run_label", self.run_label.as_deref()),
            ("service_name", self.service_name.as_deref()),
            ("service_version", self.service_version.as_deref()),
            ("handler_name", self.handler_name.as_deref()),
        ] {
            if let Some(value) = value {
                validate_nonempty_no_control("sora agent apartment audit event", field, value)?;
            }
        }
        if self
            .reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "reason",
                "must not be empty when provided",
            ));
        }
        if self.budget_units.is_some_and(|budget| budget == 0) {
            return Err(invalid_field(
                "sora agent apartment audit event",
                "budget_units",
                "must be greater than zero when provided",
            ));
        }
        if matches!(
            self.action,
            SoraAgentApartmentActionV1::WalletSpendRequested
                | SoraAgentApartmentActionV1::WalletSpendApproved
        ) {
            let Some(request_id) = self.request_id.as_deref() else {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "request_id",
                    "wallet spend events require a canonical request_id",
                ));
            };
            if !is_canonical_agent_wallet_request_id_v1(request_id) {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "request_id",
                    "wallet spend events require a canonical V1 request_id",
                ));
            }
            if self
                .asset_definition
                .as_deref()
                .is_none_or(|asset_definition| asset_definition.trim().is_empty())
            {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "asset_definition",
                    "wallet spend events require an asset_definition",
                ));
            }
            if self.amount.as_ref().is_none_or(Quantity::is_zero) {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "amount",
                    "wallet spend events require an amount greater than zero",
                ));
            }
        }
        if self.action == SoraAgentApartmentActionV1::AutonomyRunExecuted {
            if self.run_id.is_none()
                || self.request_id.as_deref() != self.run_id.as_deref()
                || self.artifact_hash.is_none()
                || self.run_label.is_none()
                || self.budget_units.is_none()
            {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "run_id",
                    "autonomy execution events require matching request/run ids and complete approved-run attribution",
                ));
            }
            if self.result_commitment.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "result_commitment",
                    "autonomy execution events require a result_commitment",
                ));
            }
            if self.succeeded.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "succeeded",
                    "autonomy execution events require a success flag",
                ));
            }
            let service_context_count = [
                self.service_name.is_some(),
                self.service_version.is_some(),
                self.handler_name.is_some(),
            ]
            .into_iter()
            .filter(|present| *present)
            .count();
            if !matches!(service_context_count, 0 | 3) {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "service_name",
                    "autonomy execution service_name, service_version, and handler_name must be populated together",
                ));
            }
            if self.journal_artifact_hash.is_none() {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "journal_artifact_hash",
                    "autonomy execution events require their node-local execution-summary artifact hash",
                ));
            }
            match self.succeeded {
                Some(true)
                    if self.reason.is_some()
                        || self.runtime_receipt_id.is_none()
                        || self.checkpoint_artifact_hash.is_none()
                        || service_context_count != 3 =>
                {
                    return Err(invalid_field(
                        "sora agent apartment audit event",
                        "succeeded",
                        "successful autonomy execution requires complete service context, runtime receipt, journal, and checkpoint attribution without an error",
                    ));
                }
                Some(false)
                    if self.reason.is_none()
                        || self.runtime_receipt_id.is_some()
                        || self.checkpoint_artifact_hash.is_some() =>
                {
                    return Err(invalid_field(
                        "sora agent apartment audit event",
                        "succeeded",
                        "failed autonomy execution requires an error and journal but cannot claim a runtime receipt or checkpoint",
                    ));
                }
                _ => {}
            }
            if self.asset_definition.is_some()
                || self.amount.is_some()
                || self.capability.is_some()
                || self.from_apartment.is_some()
                || self.to_apartment.is_some()
                || self.channel.is_some()
                || self.payload_hash.is_some()
            {
                return Err(invalid_field(
                    "sora agent apartment audit event",
                    "action",
                    "autonomy execution events cannot carry wallet, policy, or mailbox attribution",
                ));
            }
        }
        Ok(())
    }
}
fn validate_exact_token(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonempty_no_control(manifest, field, value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(invalid_field(
            manifest,
            field,
            "must not contain whitespace",
        ));
    }
    Ok(())
}
fn validate_exact_name_token(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_exact_token(manifest, field, value)?;
    let parsed = value.parse::<Name>().map_err(|error| {
        invalid_field(
            manifest,
            field,
            format!("must be a canonical Name: {error}"),
        )
    })?;
    if parsed.as_ref() != value {
        return Err(invalid_field(
            manifest,
            field,
            "must use the exact canonical Name spelling",
        ));
    }
    Ok(())
}
fn validate_public_host(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_exact_token(manifest, field, value)?;
    if let Ok(address) = value.parse::<std::net::IpAddr>() {
        if address.to_string() == value {
            return Ok(());
        }
        return Err(invalid_field(
            manifest,
            field,
            "IP literals must use their exact canonical spelling",
        ));
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(invalid_field(
            manifest,
            field,
            "numeric hosts must use exact canonical IPv4 spelling",
        ));
    }
    if !value.is_ascii()
        || value.len() > 253
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
        || value.starts_with('.')
        || value.ends_with('.')
    {
        return Err(invalid_field(
            manifest,
            field,
            "must be a lowercase canonical DNS host name or canonical IP literal",
        ));
    }
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(invalid_field(
                manifest,
                field,
                "must be a lowercase canonical DNS host name or canonical IP literal",
            ));
        }
    }
    Ok(())
}
fn validate_internal_service_url(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_exact_token(manifest, field, value)?;
    let remainder = value.strip_prefix("soracloud://").ok_or_else(|| {
        invalid_field(
            manifest,
            field,
            "must use the exact `soracloud://<service>:<port>/<path>` scheme",
        )
    })?;
    let (authority, path_suffix) = remainder.split_once('/').ok_or_else(|| {
        invalid_field(
            manifest,
            field,
            "must include a canonical absolute service path",
        )
    })?;
    let (service_name, port_text) = authority.rsplit_once(':').ok_or_else(|| {
        invalid_field(
            manifest,
            field,
            "must include one canonical service name and TCP port",
        )
    })?;
    validate_exact_name_token(manifest, field, service_name)?;
    let port = port_text.parse::<u16>().map_err(|error| {
        invalid_field(
            manifest,
            field,
            format!("must include a valid TCP port: {error}"),
        )
    })?;
    if port == 0 || port.to_string() != port_text {
        return Err(invalid_field(
            manifest,
            field,
            "TCP port must use its exact nonzero decimal spelling",
        ));
    }
    let path = format!("/{path_suffix}");
    validate_absolute_path(manifest, field, &path)
}
fn validate_public_url(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonempty_no_control(manifest, field, value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(invalid_field(
            manifest,
            field,
            "must not contain whitespace",
        ));
    }
    let (remainder, default_port) = if let Some(remainder) = value.strip_prefix("https://") {
        (remainder, 443_u16)
    } else if let Some(remainder) = value.strip_prefix("http://") {
        (remainder, 80_u16)
    } else {
        return Err(invalid_field(
            manifest,
            field,
            "must start with exact lowercase http:// or https://",
        ));
    };
    let authority_end = remainder
        .find(|character| matches!(character, '/' | '?' | '#'))
        .unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Err(invalid_field(
            manifest,
            field,
            "must include a nonempty public host without user information",
        ));
    }
    let (host, port_text) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some(closing_offset) = bracketed.find(']') else {
            return Err(invalid_field(
                manifest,
                field,
                "IPv6 hosts must use exact canonical bracketed spelling",
            ));
        };
        let host = &bracketed[..closing_offset];
        let address = host.parse::<std::net::Ipv6Addr>().map_err(|error| {
            invalid_field(
                manifest,
                field,
                format!("must include a canonical IPv6 host: {error}"),
            )
        })?;
        if address.to_string() != host {
            return Err(invalid_field(
                manifest,
                field,
                "IPv6 hosts must use exact canonical bracketed spelling",
            ));
        }
        let suffix = &bracketed[closing_offset + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(suffix.strip_prefix(':').ok_or_else(|| {
                invalid_field(
                    manifest,
                    field,
                    "IPv6 host suffix must be one canonical TCP port",
                )
            })?)
        };
        (None, port)
    } else {
        if authority.matches(':').count() > 1 {
            return Err(invalid_field(
                manifest,
                field,
                "IPv6 hosts must use exact canonical bracketed spelling",
            ));
        }
        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        (Some(host), port)
    };
    if let Some(host) = host {
        validate_public_host(manifest, field, host)?;
    }
    if let Some(port_text) = port_text {
        let port = port_text.parse::<u16>().map_err(|error| {
            invalid_field(
                manifest,
                field,
                format!("must include a valid canonical TCP port: {error}"),
            )
        })?;
        if port == 0 || port.to_string() != port_text || port == default_port {
            return Err(invalid_field(
                manifest,
                field,
                "TCP port must be nonzero, non-default, and use exact decimal spelling",
            ));
        }
    }
    let suffix = &remainder[authority_end..];
    if suffix.contains('#') || suffix.contains('\\') || suffix.contains('%') || !suffix.is_ascii() {
        return Err(invalid_field(
            manifest,
            field,
            "URL path and query must be exact ASCII without fragments, escapes, or backslashes",
        ));
    }
    let (path, query) = suffix
        .split_once('?')
        .map_or((suffix, None), |(path, query)| (path, Some(query)));
    if let Some(query) = query {
        if query.is_empty() || path.is_empty() {
            return Err(invalid_field(
                manifest,
                field,
                "URL queries require a nonempty query and explicit canonical path",
            ));
        }
    } else if path == "/" {
        return Err(invalid_field(
            manifest,
            field,
            "root URLs must omit the trailing slash",
        ));
    }
    if !path.is_empty() {
        validate_absolute_path(manifest, field, path)?;
    }
    Ok(())
}
fn validate_absolute_path(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonempty_no_control(manifest, field, value)?;
    if !value.starts_with('/')
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
        || value.chars().any(char::is_whitespace)
    {
        return Err(invalid_field(
            manifest,
            field,
            "must be an exact absolute URL path without whitespace, query, fragment, or backslash",
        ));
    }
    if value != "/" {
        if value.ends_with('/')
            || value[1..]
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(invalid_field(
                manifest,
                field,
                "must not contain empty, `.` or `..` components or a trailing slash",
            ));
        }
    }
    Ok(())
}
/// Lifecycle action recorded for an app-level Soracloud topology transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraAppInfraActionV1 {
    /// First-time admission of an app topology.
    Deploy,
    /// Upgrade of an already admitted app topology.
    Upgrade,
}
/// Static-site binding attached to a Soracloud app.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppStaticSiteBindingV1 {
    /// Schema version; must equal [`SORA_APP_STATIC_SITE_BINDING_VERSION_V1`].
    pub schema_version: u16,
    /// Public URL exposed to browsers.
    pub public_url: String,
    /// Optional content-addressed site CID.
    #[norito(required)]
    pub content_cid: Option<String>,
    /// Optional static-site manifest digest.
    #[norito(required)]
    pub manifest_digest_hex: Option<String>,
    /// URL path where static content is mounted.
    pub mount_path: String,
    /// Optional API base path consumed by the static frontend.
    #[norito(required)]
    pub api_base_path: Option<String>,
}
impl SoraAppStaticSiteBindingV1 {
    /// Validate static-site binding fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version or URL/path fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app static site binding",
            self.schema_version,
            SORA_APP_STATIC_SITE_BINDING_VERSION_V1,
        )?;
        validate_public_url(
            "sora app static site binding",
            "public_url",
            &self.public_url,
        )?;
        validate_absolute_path(
            "sora app static site binding",
            "mount_path",
            &self.mount_path,
        )?;
        if let Some(api_base_path) = self.api_base_path.as_ref() {
            validate_absolute_path(
                "sora app static site binding",
                "api_base_path",
                api_base_path,
            )?;
        }
        match (
            self.content_cid.as_deref(),
            self.manifest_digest_hex.as_deref(),
        ) {
            (Some(content_cid), Some(manifest_digest_hex)) => {
                validate_canonical_sorafs_content_cid(
                    "sora app static site binding",
                    "content_cid",
                    content_cid,
                )?;
                validate_canonical_lower_hex_32(
                    "sora app static site binding",
                    "manifest_digest_hex",
                    manifest_digest_hex,
                )?;
                Ok(())
            }
            (None, None) => Ok(()),
            _ => Err(invalid_field(
                "sora app static site binding",
                "content_cid",
                "content_cid and manifest_digest_hex must be provided together",
            )),
        }
    }
}
/// Route projection exposed by a service inside an app topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppRouteProjectionV1 {
    /// Schema version; must equal [`SORA_APP_ROUTE_PROJECTION_VERSION_V1`].
    pub schema_version: u16,
    /// Public host name when the route is externally reachable.
    #[norito(required)]
    pub public_host: Option<String>,
    /// Public or internal path prefix.
    pub path_prefix: String,
    /// Optional internal service URL routed by Soracloud.
    #[norito(required)]
    pub internal_url: Option<String>,
}
impl SoraAppRouteProjectionV1 {
    /// Validate app route projection fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version or route fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app route projection",
            self.schema_version,
            SORA_APP_ROUTE_PROJECTION_VERSION_V1,
        )?;
        validate_absolute_path(
            "sora app route projection",
            "path_prefix",
            &self.path_prefix,
        )?;
        if let Some(public_host) = self.public_host.as_deref() {
            validate_public_host("sora app route projection", "public_host", public_host)?;
        }
        if let Some(internal_url) = self.internal_url.as_deref() {
            validate_internal_service_url(
                "sora app route projection",
                "internal_url",
                internal_url,
            )?;
        }
        Ok(())
    }
}
/// Reference to an admitted Soracloud service revision in an app topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraServiceRefV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_SERVICE_REF_VERSION_V1`].
    pub schema_version: u16,
    /// Service name.
    pub service_name: Name,
    /// Admitted service revision.
    pub service_version: String,
    /// Hash of the service manifest for the referenced revision.
    pub service_manifest_hash: Hash,
    /// Hash of the container manifest for the referenced revision.
    pub container_manifest_hash: Hash,
    /// Expected execution plane for the referenced service.
    pub execution_plane: SoraServiceExecutionPlaneV1,
    /// Expected container runtime for the referenced service.
    pub runtime: SoraContainerRuntimeV1,
    /// Routes projected from this service into the app.
    pub routes: Vec<SoraAppRouteProjectionV1>,
    /// Optional persistent lease-volume names the app topology depends on.
    pub lease_volumes: Vec<Name>,
    /// Optional shard identifier for horizontally sharded app services.
    #[norito(required)]
    pub shard: Option<String>,
}
impl SoraAppInfraServiceRefV1 {
    /// Validate service reference fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version, revision, or routes are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra service ref",
            self.schema_version,
            SORA_APP_INFRA_SERVICE_REF_VERSION_V1,
        )?;
        validate_exact_token(
            "sora app infra service ref",
            "service_version",
            &self.service_version,
        )?;
        validate_soracloud_digest_hash(
            "sora app infra service ref",
            "service_manifest_hash",
            self.service_manifest_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora app infra service ref",
            "container_manifest_hash",
            self.container_manifest_hash,
        )?;
        if let Some(shard) = self.shard.as_deref() {
            validate_exact_token("sora app infra service ref", "shard", shard)?;
        }
        let mut route_paths = BTreeSet::new();
        for route in &self.routes {
            route.validate()?;
            if !route_paths.insert((route.public_host.clone(), route.path_prefix.clone())) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora app infra service ref",
                    field: "routes",
                    reason: format!(
                        "duplicate route projection for host {:?} and path `{}`",
                        route.public_host, route.path_prefix
                    ),
                });
            }
        }
        let mut volumes = BTreeSet::new();
        for volume in &self.lease_volumes {
            if !volumes.insert(volume.clone()) {
                return Err(SoracloudManifestError::DuplicateLeaseVolume {
                    volume: volume.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Canonical app-level Soracloud infrastructure manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraManifestV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Stable application name.
    pub app_name: Name,
    /// Application topology revision.
    pub app_version: String,
    /// Public URL for the application.
    pub public_url: String,
    /// Optional static-site binding.
    #[norito(required)]
    pub static_site: Option<SoraAppStaticSiteBindingV1>,
    /// Authoritative service topology.
    pub services: Vec<SoraAppInfraServiceRefV1>,
}
impl SoraAppInfraManifestV1 {
    /// Compute the canonical app-infra manifest hash.
    #[must_use]
    pub fn manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(self))
    }
    /// Validate app topology invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the topology is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra manifest",
            self.schema_version,
            SORA_APP_INFRA_MANIFEST_VERSION_V1,
        )?;
        validate_exact_token("sora app infra manifest", "app_version", &self.app_version)?;
        validate_public_url("sora app infra manifest", "public_url", &self.public_url)?;
        if self.services.is_empty() {
            return Err(invalid_field(
                "sora app infra manifest",
                "services",
                "at least one service reference is required",
            ));
        }
        if let Some(static_site) = self.static_site.as_ref() {
            static_site.validate()?;
        }
        let mut service_names = BTreeSet::new();
        for service in &self.services {
            service.validate()?;
            if !service_names.insert(service.service_name.clone()) {
                return Err(SoracloudManifestError::DuplicateAppService {
                    service: service.service_name.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Exact authoritative app topology observed before a signed upgrade.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoraAppInfraExactCurrentRevisionPreconditionV1 {
    /// Active app version observed by the signer.
    pub app_version: String,
    /// Active topology-manifest hash observed by the signer.
    pub manifest_hash: Hash,
    /// Positive authoritative revision count observed by the signer.
    pub revision_count: u32,
}
/// Signed compare-and-set condition for an app topology deploy or upgrade.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "condition", content = "value"))]
pub enum SoraAppInfraMutationPreconditionV1 {
    /// A first deployment is valid only while the app name has no state.
    AppAbsent,
    /// An upgrade is valid only while the observed topology revision remains current.
    ExactCurrentRevision(SoraAppInfraExactCurrentRevisionPreconditionV1),
}
/// Authoritative app-level Soracloud infrastructure state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraStateV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Stable application name.
    pub app_name: Name,
    /// Active app topology revision.
    pub current_app_version: String,
    /// Hash of the active app topology.
    pub current_manifest_hash: Hash,
    /// Number of admitted app topology revisions.
    pub revision_count: u32,
    /// Audit sequence that deployed this topology.
    pub deployed_sequence: u64,
    /// Audit sequence that last updated this topology.
    pub updated_sequence: u64,
    /// Active app topology manifest.
    pub manifest: SoraAppInfraManifestV1,
}
impl SoraAppInfraStateV1 {
    /// Validate app topology state.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the state is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra state",
            self.schema_version,
            SORA_APP_INFRA_STATE_VERSION_V1,
        )?;
        self.manifest.validate()?;
        if self.app_name != self.manifest.app_name {
            return Err(invalid_field(
                "sora app infra state",
                "app_name",
                "must match embedded manifest app_name",
            ));
        }
        if self.current_app_version != self.manifest.app_version {
            return Err(invalid_field(
                "sora app infra state",
                "current_app_version",
                "must match embedded manifest app_version",
            ));
        }
        if self.current_manifest_hash != self.manifest.manifest_hash() {
            return Err(invalid_field(
                "sora app infra state",
                "current_manifest_hash",
                "must match embedded manifest hash",
            ));
        }
        if self.revision_count == 0 || self.deployed_sequence == 0 || self.updated_sequence == 0 {
            return Err(invalid_field(
                "sora app infra state",
                "sequence",
                "revision_count and audit sequences must be greater than zero",
            ));
        }
        Ok(())
    }
}
/// Audit record for an authoritative Soracloud app topology event.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraAppInfraAuditEventV1 {
    /// Schema version; must equal [`SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Monotonic audit/event sequence.
    pub sequence: u64,
    /// App topology action.
    pub action: SoraAppInfraActionV1,
    /// Application affected by the transition.
    pub app_name: Name,
    /// Previous active app version, when applicable.
    #[norito(required)]
    pub from_version: Option<String>,
    /// Resulting active app version.
    pub to_version: String,
    /// Hash of the admitted app topology.
    pub app_manifest_hash: Hash,
    /// Number of service refs in the topology.
    pub service_count: u32,
    /// Provenance signer that authorized the topology transition.
    pub signer: PublicKey,
}
impl SoraAppInfraAuditEventV1 {
    /// Validate app topology audit records.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when event fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora app infra audit event",
            self.schema_version,
            SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora app infra audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        validate_exact_token("sora app infra audit event", "to_version", &self.to_version)?;
        validate_soracloud_digest_hash(
            "sora app infra audit event",
            "app_manifest_hash",
            self.app_manifest_hash,
        )?;
        if self.service_count == 0 {
            return Err(invalid_field(
                "sora app infra audit event",
                "service_count",
                "must be greater than zero",
            ));
        }
        if let Some(from_version) = self.from_version.as_deref() {
            validate_exact_token("sora app infra audit event", "from_version", from_version)?;
        }
        Ok(())
    }
}
/// Audit record for an authoritative Soracloud lifecycle event.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceAuditEventV1 {
    /// Schema version; must equal [`SORA_SERVICE_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Monotonic audit/event sequence.
    pub sequence: u64,
    /// Consensus block height that committed the transition.
    pub block_height: u64,
    /// Consensus block timestamp that committed the transition.
    pub block_timestamp_ms: u64,
    /// Lifecycle action that produced the event.
    pub action: SoraServiceLifecycleActionV1,
    /// Service affected by the transition.
    pub service_name: Name,
    /// Previous active version; the wire key is explicitly null when not applicable.
    #[norito(required)]
    pub from_version: Option<String>,
    /// Resulting active version after the transition.
    pub to_version: String,
    /// Service manifest hash bound to the transition.
    pub service_manifest_hash: Hash,
    /// Container manifest hash bound to the transition.
    pub container_manifest_hash: Hash,
    /// Post-transition process generation.
    pub process_generation: u64,
    /// Post-transition config materialization generation.
    pub config_generation: u64,
    /// Post-transition secret materialization generation.
    pub secret_generation: u64,
    /// Commitment to the complete post-transition config projection.
    pub config_snapshot_hash: Hash,
    /// Commitment to the complete post-transition encrypted-secret projection.
    pub secret_snapshot_hash: Hash,
    /// Governance transaction hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub governance_tx_hash: Option<Hash>,
    /// Binding associated with this event; the wire key is explicitly null when absent.
    #[norito(required)]
    pub binding_name: Option<Name>,
    /// State key associated with this event; the wire key is explicitly null when absent.
    #[norito(required)]
    pub state_key: Option<String>,
    /// Canonically ordered config deltas needed to replay this transition exactly.
    pub config_mutations: Vec<SoraServiceConfigMutationV1>,
    /// Canonically ordered encrypted-secret deltas needed to replay this transition exactly.
    pub secret_mutations: Vec<SoraServiceSecretMutationV1>,
    /// Complete post-transition rollout state; the wire key is explicitly null when absent.
    #[norito(required)]
    pub rollout_state: Option<SoraServiceRolloutStateV1>,
    /// Decryption policy name; the wire key is explicitly null when absent.
    #[norito(required)]
    pub policy_name: Option<Name>,
    /// Snapshotted policy hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub policy_snapshot_hash: Option<Hash>,
    /// Jurisdiction/compliance tag; the wire key is explicitly null when absent.
    #[norito(required)]
    pub jurisdiction_tag: Option<String>,
    /// Consent-evidence commitment; the wire key is explicitly null when absent.
    #[norito(required)]
    pub consent_evidence_hash: Option<Hash>,
    /// Break-glass flag; the wire key is explicitly null when not applicable.
    #[norito(required)]
    pub break_glass: Option<bool>,
    /// Break-glass justification; the wire key is explicitly null when absent.
    #[norito(required)]
    pub break_glass_reason: Option<String>,
    /// Accepted lease-usage transition input; null for non-lease actions.
    #[norito(required)]
    pub lease_usage: Option<SoraServiceLeaseUsageAuditV1>,
    /// Commitment to the complete post-transition hosted-service lease; null when unchanged.
    #[norito(required)]
    pub service_lease_commitment: Option<Hash>,
    /// Hosted-service reporting-epoch rollover payload; the wire key is
    /// explicitly null for every other lifecycle action.
    #[norito(required)]
    pub lease_reporting_epoch_rollover: Option<SoraServiceLeaseReportingEpochRolloverV1>,
    /// Provenance signer that authorized the lifecycle action.
    pub signer: PublicKey,
}
impl SoraServiceAuditEventV1 {
    /// Validate Soracloud lifecycle audit records.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when event sequencing or version fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_required_fields()?;
        self.validate_optional_fields()?;
        self.validate_action_fields()?;
        self.validate_break_glass_fields()?;
        self.validate_reporting_epoch_rollover()
    }
    fn validate_required_fields(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service audit event",
            self.schema_version,
            SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 || self.block_height == 0 || self.block_timestamp_ms == 0 {
            return Err(invalid_field(
                "sora service audit event",
                "sequence",
                "sequence, block_height, and block_timestamp_ms must be greater than zero",
            ));
        }
        if let Some(from_version) = self.from_version.as_deref() {
            validate_exact_token("sora service audit event", "from_version", from_version)?;
        }
        validate_exact_token("sora service audit event", "to_version", &self.to_version)?;
        validate_soracloud_digest_hash(
            "sora service audit event",
            "service_manifest_hash",
            self.service_manifest_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora service audit event",
            "container_manifest_hash",
            self.container_manifest_hash,
        )?;
        if self.process_generation == 0 {
            return Err(invalid_field(
                "sora service audit event",
                "process_generation",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora service audit event",
            "config_snapshot_hash",
            self.config_snapshot_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora service audit event",
            "secret_snapshot_hash",
            self.secret_snapshot_hash,
        )?;
        Ok(())
    }
    fn validate_optional_fields(&self) -> Result<(), SoracloudManifestError> {
        if let Some(rollout) = self.rollout_state.as_ref() {
            rollout.validate()?;
        }
        if let Some(lease_usage) = self.lease_usage.as_ref() {
            lease_usage.validate()?;
        }
        if let Some(service_lease_commitment) = self.service_lease_commitment {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "service_lease_commitment",
                service_lease_commitment,
            )?;
        }
        if let Some(state_key) = self.state_key.as_deref() {
            validate_absolute_path("sora service audit event", "state_key", state_key)?;
        }
        let mut previous_config_name = None;
        for mutation in &self.config_mutations {
            mutation.validate_at_sequence(self.sequence)?;
            let config_name = mutation.config_name();
            if previous_config_name.is_some_and(|previous| previous >= config_name) {
                return Err(invalid_field(
                    "sora service audit event",
                    "config_mutations",
                    "must be strictly sorted by config name without duplicates",
                ));
            }
            previous_config_name = Some(config_name);
        }
        let mut previous_secret_name = None;
        for mutation in &self.secret_mutations {
            mutation.validate_at_sequence(self.sequence)?;
            let secret_name = mutation.secret_name();
            if previous_secret_name.is_some_and(|previous| previous >= secret_name) {
                return Err(invalid_field(
                    "sora service audit event",
                    "secret_mutations",
                    "must be strictly sorted by secret name without duplicates",
                ));
            }
            previous_secret_name = Some(secret_name);
        }
        if let Some(jurisdiction_tag) = self.jurisdiction_tag.as_deref() {
            validate_exact_token(
                "sora service audit event",
                "jurisdiction_tag",
                jurisdiction_tag,
            )?;
        }
        if let Some(governance_tx_hash) = self.governance_tx_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "governance_tx_hash",
                governance_tx_hash,
            )?;
        }
        if let Some(policy_snapshot_hash) = self.policy_snapshot_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "policy_snapshot_hash",
                policy_snapshot_hash,
            )?;
        }
        if let Some(consent_evidence_hash) = self.consent_evidence_hash {
            validate_soracloud_digest_hash(
                "sora service audit event",
                "consent_evidence_hash",
                consent_evidence_hash,
            )?;
        }
        Ok(())
    }
    fn validate_action_fields(&self) -> Result<(), SoracloudManifestError> {
        use SoraServiceLifecycleActionV1 as Action;

        let (required, allowed): (&[&str], &[&str]) = match self.action {
            Action::Deploy => (&[], &["service_lease_commitment"]),
            Action::Upgrade => (
                &["rollout_state"],
                &["rollout_state", "service_lease_commitment"],
            ),
            Action::LeaseUsage | Action::LeaseReportingEpochRollover => (
                &["lease_usage", "service_lease_commitment"],
                &["lease_usage", "service_lease_commitment"],
            ),
            Action::ConfigMutation | Action::SecretMutation => (&[], &[]),
            Action::StateMutation => (
                &["governance_tx_hash", "binding_name", "state_key"],
                &["governance_tx_hash", "binding_name", "state_key"],
            ),
            Action::FheJobRun => (
                &[
                    "governance_tx_hash",
                    "binding_name",
                    "state_key",
                    "policy_name",
                    "policy_snapshot_hash",
                ],
                &[
                    "governance_tx_hash",
                    "binding_name",
                    "state_key",
                    "policy_name",
                    "policy_snapshot_hash",
                ],
            ),
            Action::FhePolicyRegister | Action::FhePolicyRotate | Action::FhePolicyRevoke => (
                &["governance_tx_hash", "policy_name", "policy_snapshot_hash"],
                &["governance_tx_hash", "policy_name", "policy_snapshot_hash"],
            ),
            Action::DecryptionRequest => (
                &[
                    "governance_tx_hash",
                    "binding_name",
                    "state_key",
                    "policy_name",
                    "policy_snapshot_hash",
                    "jurisdiction_tag",
                    "break_glass",
                ],
                &[
                    "governance_tx_hash",
                    "binding_name",
                    "state_key",
                    "policy_name",
                    "policy_snapshot_hash",
                    "jurisdiction_tag",
                    "consent_evidence_hash",
                    "break_glass",
                    "break_glass_reason",
                ],
            ),
            Action::Rollout => (
                &["governance_tx_hash", "rollout_state"],
                &["governance_tx_hash", "rollout_state"],
            ),
            Action::Rollback => (
                &[],
                &[
                    "governance_tx_hash",
                    "rollout_state",
                    "service_lease_commitment",
                ],
            ),
            Action::CiphertextQuery => {
                return Err(invalid_field(
                    "sora service audit event",
                    "action",
                    "CiphertextQuery is a read-only response action and must not be persisted as a lifecycle event",
                ));
            }
        };
        let presence = [
            ("governance_tx_hash", self.governance_tx_hash.is_some()),
            ("binding_name", self.binding_name.is_some()),
            ("state_key", self.state_key.is_some()),
            ("rollout_state", self.rollout_state.is_some()),
            ("policy_name", self.policy_name.is_some()),
            ("policy_snapshot_hash", self.policy_snapshot_hash.is_some()),
            ("jurisdiction_tag", self.jurisdiction_tag.is_some()),
            (
                "consent_evidence_hash",
                self.consent_evidence_hash.is_some(),
            ),
            ("break_glass", self.break_glass.is_some()),
            ("break_glass_reason", self.break_glass_reason.is_some()),
            ("lease_usage", self.lease_usage.is_some()),
            (
                "service_lease_commitment",
                self.service_lease_commitment.is_some(),
            ),
        ];
        for (field, present) in presence {
            if required.contains(&field) && !present {
                return Err(invalid_field(
                    "sora service audit event",
                    field,
                    "must be present for this lifecycle action",
                ));
            }
            if present && !allowed.contains(&field) {
                return Err(invalid_field(
                    "sora service audit event",
                    field,
                    "must be null for this lifecycle action",
                ));
            }
        }

        let material_shape_valid = match self.action {
            Action::Deploy | Action::Upgrade => {
                self.config_mutations
                    .iter()
                    .all(|mutation| matches!(mutation, SoraServiceConfigMutationV1::Upsert(_)))
                    && self
                        .secret_mutations
                        .iter()
                        .all(|mutation| matches!(mutation, SoraServiceSecretMutationV1::Upsert(_)))
            }
            Action::ConfigMutation => {
                self.config_mutations.len() == 1 && self.secret_mutations.is_empty()
            }
            Action::SecretMutation => {
                self.config_mutations.is_empty() && self.secret_mutations.len() == 1
            }
            _ => self.config_mutations.is_empty() && self.secret_mutations.is_empty(),
        };
        if !material_shape_valid {
            return Err(invalid_field(
                "sora service audit event",
                "config_mutations",
                "material deltas must be exact for the lifecycle action and admissions may only upsert",
            ));
        }

        match self.action {
            Action::Deploy
            | Action::ConfigMutation
            | Action::SecretMutation
            | Action::StateMutation
            | Action::FheJobRun
            | Action::FhePolicyRegister
            | Action::FhePolicyRotate
            | Action::FhePolicyRevoke
            | Action::DecryptionRequest => {
                if self.from_version.is_some() {
                    return Err(invalid_field(
                        "sora service audit event",
                        "from_version",
                        "must be null for this lifecycle action",
                    ));
                }
            }
            Action::Upgrade | Action::Rollback => {
                let Some(from_version) = self.from_version.as_deref() else {
                    return Err(invalid_field(
                        "sora service audit event",
                        "from_version",
                        "must be present for this lifecycle action",
                    ));
                };
                if from_version == self.to_version {
                    return Err(invalid_field(
                        "sora service audit event",
                        "from_version",
                        "must differ from to_version for this lifecycle action",
                    ));
                }
            }
            Action::Rollout => {
                if self.from_version.as_deref() != Some(self.to_version.as_str()) {
                    return Err(invalid_field(
                        "sora service audit event",
                        "from_version",
                        "rollout progress must bind an unchanged active version",
                    ));
                }
            }
            Action::LeaseUsage | Action::LeaseReportingEpochRollover => {
                if self.from_version.as_deref() != Some(self.to_version.as_str()) {
                    return Err(invalid_field(
                        "sora service audit event",
                        "from_version",
                        "lease accounting must bind an unchanged deployment version",
                    ));
                }
            }
            Action::CiphertextQuery => {}
        }
        if self.action == Action::Rollback
            && (self.governance_tx_hash.is_some() != self.rollout_state.is_some())
        {
            return Err(invalid_field(
                "sora service audit event",
                "rollout_state",
                "rollback governance_tx_hash and rollout_state must be populated together",
            ));
        }
        if self.action == Action::Rollback
            && self.rollout_state.is_some()
            && self.service_lease_commitment.is_some()
        {
            return Err(invalid_field(
                "sora service audit event",
                "service_lease_commitment",
                "automatic rollout rollback must preserve rather than replace lease state",
            ));
        }
        if let Some(rollout) = self.rollout_state.as_ref() {
            let handle = rollout.rollout_handle.as_str();
            let expected_prefix = format!("{}:rollout:", self.service_name);
            let Some(sequence) = handle.strip_prefix(&expected_prefix) else {
                return Err(invalid_field(
                    "sora service audit event",
                    "rollout_state.rollout_handle",
                    "must use the canonical `<service>:rollout:<creation-sequence>` namespace",
                ));
            };
            let parsed_sequence = sequence.parse::<u64>().map_err(|_| {
                invalid_field(
                    "sora service audit event",
                    "rollout_state.rollout_handle",
                    "must end in a canonical positive creation sequence",
                )
            })?;
            if parsed_sequence == 0 || parsed_sequence.to_string() != sequence {
                return Err(invalid_field(
                    "sora service audit event",
                    "rollout_state.rollout_handle",
                    "must end in a canonical positive creation sequence",
                ));
            }
            if parsed_sequence != rollout.created_sequence {
                return Err(invalid_field(
                    "sora service audit event",
                    "rollout_state.created_sequence",
                    "must equal the creation sequence encoded in rollout_handle",
                ));
            }
            let rollout_shape_valid = match self.action {
                Action::Upgrade => {
                    rollout.created_sequence == self.sequence
                        && rollout.updated_sequence == self.sequence
                        && self.from_version.as_deref() == Some(rollout.baseline_version.as_str())
                        && self.to_version == rollout.candidate_version
                        && matches!(
                            rollout.stage,
                            SoraRolloutStageV1::Canary | SoraRolloutStageV1::Promoted
                        )
                }
                Action::Rollout => {
                    rollout.updated_sequence == self.sequence
                        && self.to_version == rollout.candidate_version
                        && matches!(
                            rollout.stage,
                            SoraRolloutStageV1::Canary | SoraRolloutStageV1::Promoted
                        )
                }
                Action::Rollback => {
                    rollout.updated_sequence == self.sequence
                        && self.from_version.as_deref() == Some(rollout.candidate_version.as_str())
                        && self.to_version == rollout.baseline_version
                        && rollout.stage == SoraRolloutStageV1::RolledBack
                }
                _ => false,
            };
            if !rollout_shape_valid {
                return Err(invalid_field(
                    "sora service audit event",
                    "rollout_state",
                    "must be the exact post-transition rollout state for this lifecycle action",
                ));
            }
        }
        Ok(())
    }
    fn validate_break_glass_fields(&self) -> Result<(), SoracloudManifestError> {
        if self
            .break_glass_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must not be empty when provided",
            ));
        }
        if self.break_glass == Some(false) && self.break_glass_reason.is_some() {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must be null when break_glass=false",
            ));
        }
        if self.break_glass == Some(true) && self.break_glass_reason.is_none() {
            return Err(invalid_field(
                "sora service audit event",
                "break_glass_reason",
                "must be provided when break_glass=true",
            ));
        }
        Ok(())
    }
    fn validate_reporting_epoch_rollover(&self) -> Result<(), SoracloudManifestError> {
        let usage = self.lease_usage.as_ref();
        if let Some(usage) = usage
            && usage
                .assignment
                .placement
                .validator_account_id
                .try_signatory()
                != Some(&self.signer)
        {
            return Err(invalid_field(
                "sora service audit event",
                "lease_usage.assignment.placement.validator_account_id",
                "single-signatory reporter must match the audit signer",
            ));
        }
        match self.action {
            SoraServiceLifecycleActionV1::LeaseUsage => {
                if self.lease_reporting_epoch_rollover.is_some() {
                    return Err(invalid_field(
                        "sora service audit event",
                        "lease_reporting_epoch_rollover",
                        "must be null for same-epoch LeaseUsage",
                    ));
                }
                Ok(())
            }
            SoraServiceLifecycleActionV1::LeaseReportingEpochRollover => {
                let Some(rollover) = self.lease_reporting_epoch_rollover.as_ref() else {
                    return Err(invalid_field(
                        "sora service audit event",
                        "lease_reporting_epoch_rollover",
                        "must be present for LeaseReportingEpochRollover",
                    ));
                };
                rollover.validate()?;
                let usage = usage.expect("rollover action-field validation requires usage");
                if usage.reporting_epoch != rollover.new_reporting_epoch
                    || usage.assignment.service_version != rollover.active_service_version
                    || usage.assignment.placement.replica_slot != rollover.replica_slot
                    || usage.assignment.placement.validator_account_id
                        != rollover.reporter_account_id
                    || usage.replica_accounted_egress_bytes != 0
                    || usage.finalize_reporter
                {
                    return Err(invalid_field(
                        "sora service audit event",
                        "lease_reporting_epoch_rollover",
                        "usage, settlement, and successor opener must describe one exact rollover",
                    ));
                }
                Ok(())
            }
            _ if self.lease_reporting_epoch_rollover.is_some() => Err(invalid_field(
                "sora service audit event",
                "lease_reporting_epoch_rollover",
                "must be null for non-rollover lifecycle actions",
            )),
            _ => Ok(()),
        }
    }
}
/// Runtime health status observed for a materialized service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "health_status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceHealthStatusV1 {
    /// Revision is still hydrating bundles or replay state.
    Hydrating,
    /// Revision is serving normally.
    #[default]
    Healthy,
    /// Revision is serving but under elevated failure/load pressure.
    Degraded,
    /// Revision is not fit to serve traffic.
    Unavailable,
}
/// Authoritative runtime state for the active revision of a service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceRuntimeStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_RUNTIME_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose runtime state is being tracked.
    pub service_name: Name,
    /// Active service revision/version on this node/ledger view.
    pub active_service_version: String,
    /// Current health classification.
    pub health_status: SoraServiceHealthStatusV1,
    /// Load factor in basis points (`0..=10_000`).
    pub load_factor_bps: u16,
    /// Active materialized bundle hash.
    pub materialized_bundle_hash: Hash,
}
impl SoraServiceRuntimeStateV1 {
    /// Validate runtime-state bounds and formatting.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// runtime-state fields are out of range.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service runtime state",
            self.schema_version,
            SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        )?;
        validate_exact_token(
            "sora service runtime state",
            "active_service_version",
            &self.active_service_version,
        )?;
        if self.load_factor_bps > 10_000 {
            return Err(invalid_field(
                "sora service runtime state",
                "load_factor_bps",
                "must be within 0..=10_000",
            ));
        }
        validate_soracloud_digest_hash(
            "sora service runtime state",
            "materialized_bundle_hash",
            self.materialized_bundle_hash,
        )?;
        Ok(())
    }
}
/// Authoritative runtime state for one placed Inrou replica slot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraInrouReplicaRuntimeStateV1 {
    /// Schema version; must equal [`SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose placed replica is being tracked.
    pub service_name: Name,
    /// Service revision currently materialized for the placed replica.
    pub service_version: String,
    /// One-based placed replica slot.
    pub replica_slot: u16,
    /// Exact host-assignment incarnation materialized by this runtime state.
    pub placement_incarnation: Hash,
    /// Validator currently hosting the replica.
    pub validator_account_id: AccountId,
    /// Peer identifier currently hosting the replica.
    pub peer_id: String,
    /// Guest ISA currently materializing the replica.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Current health classification for the placed replica.
    pub health_status: SoraServiceHealthStatusV1,
    /// Load factor in basis points (`0..=10_000`) for the placed replica.
    pub load_factor_bps: u16,
    /// Active materialized bundle hash.
    pub materialized_bundle_hash: Hash,
    /// Hosted-service reporting epoch acknowledged before this replica serves.
    pub reporting_epoch: u64,
    /// Total authoritative egress bytes accounted for the placed replica so far.
    pub accounted_egress_bytes: u64,
    /// Timestamp when this replica state was last refreshed.
    pub updated_at_ms: u64,
    /// Human-readable last runtime error, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
impl SoraInrouReplicaRuntimeStateV1 {
    /// Validate per-replica runtime-state bounds and formatting.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// runtime-state fields are out of range.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora inrou replica runtime state",
            self.schema_version,
            SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
        )?;
        validate_exact_token(
            "sora inrou replica runtime state",
            "service_version",
            &self.service_version,
        )?;
        if self.replica_slot == 0 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "replica_slot",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora inrou replica runtime state",
            "placement_incarnation",
            self.placement_incarnation,
        )?;
        validate_validator_account_peer_id(
            "sora inrou replica runtime state",
            &self.validator_account_id,
            &self.peer_id,
        )?;
        if self.load_factor_bps > 10_000 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "load_factor_bps",
                "must be within 0..=10_000",
            ));
        }
        if self.updated_at_ms == 0 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "updated_at_ms",
                "must be greater than zero",
            ));
        }
        if self.reporting_epoch == 0 {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "reporting_epoch",
                "must be greater than zero",
            ));
        }
        validate_soracloud_digest_hash(
            "sora inrou replica runtime state",
            "materialized_bundle_hash",
            self.materialized_bundle_hash,
        )?;
        if self
            .last_error
            .as_ref()
            .is_some_and(|error| error.trim().is_empty())
        {
            return Err(invalid_field(
                "sora inrou replica runtime state",
                "last_error",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
/// Ordered asynchronous mailbox message used for replicated cross-service calls.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceMailboxMessageV1 {
    /// Schema version; must equal [`SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1`].
    pub schema_version: u16,
    /// Ledger-derived deterministic message identifier.
    ///
    /// Submissions must carry the zero prehash sentinel; the ledger replaces it after binding the
    /// active service revisions and authoritative delivery schedule.
    pub message_id: Hash,
    /// Source service name.
    pub from_service: Name,
    /// Ledger-bound source service revision.
    pub from_service_version: String,
    /// Source handler name.
    pub from_handler: Name,
    /// Destination service name.
    pub to_service: Name,
    /// Ledger-bound destination service revision.
    pub to_service_version: String,
    /// Destination handler name.
    pub to_handler: Name,
    /// Opaque mailbox payload bytes replicated through authoritative state.
    pub payload_bytes: Vec<u8>,
    /// Commitment over the opaque message payload.
    pub payload_commitment: Hash,
    /// Relative delivery delay requested by the source runtime.
    pub delivery_delay_blocks: u32,
    /// Ledger-assigned ordered sequence at which the message was enqueued.
    ///
    /// A [`crate::isi::soracloud::RecordSoracloudMailboxMessage`] submission must set this and the
    /// derived height fields below to zero. Ledger execution assigns the ordered sequence and the
    /// consensus-height schedule from the destination handler's mailbox contract.
    pub enqueue_sequence: u64,
    /// Consensus block height in which the message was enqueued.
    pub enqueue_height: u64,
    /// Ledger-derived earliest height at which the message may execute.
    pub available_after_height: u64,
    /// Ledger-derived height at which the message expires.
    pub expires_at_height: u64,
}

/// Derive the canonical ledger-assigned identity for one ordered mailbox message.
///
/// Every immutable source, destination, payload, delay, and ledger-assigned schedule field is
/// bound. The message identifier itself is deliberately excluded.
#[must_use]
pub fn derive_soracloud_mailbox_message_id_v1(message: &SoraServiceMailboxMessageV1) -> Hash {
    let mut transcript = Vec::new();
    for part in [
        "soracloud:service-mailbox-message:v1".encode(),
        message.schema_version.encode(),
        message.from_service.encode(),
        message.from_service_version.encode(),
        message.from_handler.encode(),
        message.to_service.encode(),
        message.to_service_version.encode(),
        message.to_handler.encode(),
        message.payload_bytes.encode(),
        message.payload_commitment.encode(),
        message.delivery_delay_blocks.encode(),
        message.enqueue_sequence.encode(),
        message.enqueue_height.encode(),
        message.available_after_height.encode(),
        message.expires_at_height.encode(),
    ] {
        transcript.extend(part);
    }
    Hash::new(transcript)
}

impl SoraServiceMailboxMessageV1 {
    /// Validate a ledger-assigned mailbox message.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// availability/expiry heights are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(true)
    }
    /// Validate a mailbox message prepared for ledger submission.
    ///
    /// Submission messages carry zero sentinels for all ledger-owned schedule fields.
    pub fn validate_submission(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(false)
    }
    fn validate_with_sequence_state(
        &self,
        require_assigned_schedule: bool,
    ) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service mailbox message",
            self.schema_version,
            SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        )?;
        if require_assigned_schedule {
            validate_soracloud_digest_hash(
                "sora service mailbox message",
                "message_id",
                self.message_id,
            )?;
        } else if self.message_id != Hash::prehashed([0; Hash::LENGTH]) {
            return Err(invalid_field(
                "sora service mailbox message",
                "message_id",
                "must be the all-zero ledger-assignment sentinel before submission",
            ));
        }
        if require_assigned_schedule {
            validate_nonblank_field(
                "sora service mailbox message",
                "from_service_version",
                &self.from_service_version,
            )?;
            validate_nonblank_field(
                "sora service mailbox message",
                "to_service_version",
                &self.to_service_version,
            )?;
        } else if !self.from_service_version.is_empty() || !self.to_service_version.is_empty() {
            return Err(invalid_field(
                "sora service mailbox message",
                "from_service_version",
                "ledger-bound service versions must be empty before ledger submission",
            ));
        }
        validate_soracloud_digest_hash(
            "sora service mailbox message",
            "payload_commitment",
            self.payload_commitment,
        )?;
        if require_assigned_schedule
            && (self.enqueue_sequence == 0
                || self.enqueue_height == 0
                || self.available_after_height == 0
                || self.expires_at_height == 0)
        {
            return Err(invalid_field(
                "sora service mailbox message",
                "enqueue_sequence",
                "ledger-assigned schedule fields must be greater than zero before persistence",
            ));
        }
        if !require_assigned_schedule
            && (self.enqueue_sequence != 0
                || self.enqueue_height != 0
                || self.available_after_height != 0
                || self.expires_at_height != 0)
        {
            return Err(invalid_field(
                "sora service mailbox message",
                "enqueue_sequence",
                "ledger-assigned schedule fields must be zero before ledger submission",
            ));
        }
        if require_assigned_schedule && self.available_after_height < self.enqueue_height {
            return Err(invalid_field(
                "sora service mailbox message",
                "available_after_height",
                "must be >= enqueue_height",
            ));
        }
        if Hash::new(self.payload_bytes.as_slice()) != self.payload_commitment {
            return Err(invalid_field(
                "sora service mailbox message",
                "payload_commitment",
                "must match payload_bytes",
            ));
        }
        if require_assigned_schedule && self.expires_at_height <= self.available_after_height {
            return Err(invalid_field(
                "sora service mailbox message",
                "expires_at_height",
                "must be greater than available_after_height",
            ));
        }
        if require_assigned_schedule
            && self.message_id != derive_soracloud_mailbox_message_id_v1(self)
        {
            return Err(invalid_field(
                "sora service mailbox message",
                "message_id",
                "must equal the canonical ledger-derived mailbox message identity",
            ));
        }
        Ok(())
    }
}
/// Exact active validator selected to execute one deterministic mailbox message.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraRuntimeDeterministicValidatorHostV1 {
    /// Active public lane whose validator record was selected.
    pub lane_id: LaneId,
    /// Validator account selected by the message-bound rendezvous rule.
    pub validator_account_id: AccountId,
    /// Peer identifier in the exact active validator record.
    pub peer_id: String,
}
impl SoraRuntimeDeterministicValidatorHostV1 {
    /// Validate structural deterministic-validator attribution.
    ///
    /// Active membership and selection eligibility are validated by ledger execution.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_validator_account_peer_id(
            "sora runtime deterministic validator host",
            &self.validator_account_id,
            &self.peer_id,
        )
    }
}
/// One state effect emitted by deterministic ordered-mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraOrderedMailboxStateMutationV1 {
    /// Schema version; must equal [`SORA_ORDERED_MAILBOX_STATE_MUTATION_VERSION_V1`].
    pub schema_version: u16,
    /// Declared service-state binding.
    pub binding_name: Name,
    /// Canonical key scoped by the binding.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Encryption contract enforced by the binding.
    pub encryption: SoraStateEncryptionV1,
    /// Full value for an upsert and explicit null for a delete.
    #[norito(required)]
    pub value_payload: Option<Vec<u8>>,
}
impl SoraOrderedMailboxStateMutationV1 {
    /// Validate the structural mutation envelope.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora ordered mailbox state mutation",
            self.schema_version,
            SORA_ORDERED_MAILBOX_STATE_MUTATION_VERSION_V1,
        )?;
        if self.state_key.trim().is_empty() || !self.state_key.starts_with('/') {
            return Err(invalid_field(
                "sora ordered mailbox state mutation",
                "state_key",
                "must be a non-empty absolute binding key",
            ));
        }
        match (self.operation, self.value_payload.as_ref()) {
            (SoraStateMutationOperationV1::Upsert, Some(payload)) if !payload.is_empty() => Ok(()),
            (SoraStateMutationOperationV1::Delete, None) => Ok(()),
            (SoraStateMutationOperationV1::Upsert, _) => Err(invalid_field(
                "sora ordered mailbox state mutation",
                "value_payload",
                "must be non-empty for an upsert",
            )),
            (SoraStateMutationOperationV1::Delete, Some(_)) => Err(invalid_field(
                "sora ordered mailbox state mutation",
                "value_payload",
                "must be null for a delete",
            )),
        }
    }
}
/// Atomic effects and receipt produced by one deterministic mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraOrderedMailboxResultV1 {
    /// Schema version; must equal [`SORA_ORDERED_MAILBOX_RESULT_VERSION_V1`].
    pub schema_version: u16,
    /// Committed height whose exact state snapshot was executed.
    pub observed_height: u64,
    /// Committed tip hash whose exact state snapshot was executed.
    #[norito(required)]
    pub observed_block_hash: Option<Hash>,
    /// Next authoritative Soracloud sequence observed before execution.
    pub observed_sequence: u64,
    /// Ordered state effects to apply atomically.
    pub state_mutations: Vec<SoraOrderedMailboxStateMutationV1>,
    /// Ordered outbound messages to admit atomically.
    pub outbound_mailbox_messages: Vec<SoraServiceMailboxMessageV1>,
    /// Commitment to the runtime response bytes, including an empty response.
    pub response_commitment: Hash,
    /// Runtime-owned commitment distinguishing success and deterministic failure outcomes.
    pub runtime_execution_commitment: Hash,
    /// Response media type when the handler emitted one.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Exact runtime-state row observed before execution, used as an in-block OCC precondition.
    #[norito(required)]
    pub observed_runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Runtime-state projection to persist after the effects.
    #[norito(required)]
    pub runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Submission-sentinel receipt that consumes the source message.
    pub runtime_receipt: SoraRuntimeReceiptV1,
}
impl SoraOrderedMailboxResultV1 {
    /// Validate the submission envelope before ledger-specific authorization and OCC checks.
    pub fn validate_submission(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora ordered mailbox result",
            self.schema_version,
            SORA_ORDERED_MAILBOX_RESULT_VERSION_V1,
        )?;
        if self.observed_height == 0 {
            return Err(invalid_field(
                "sora ordered mailbox result",
                "observed_height",
                "must be greater than zero",
            ));
        }
        if self.observed_sequence == 0 {
            return Err(invalid_field(
                "sora ordered mailbox result",
                "observed_sequence",
                "must be greater than zero",
            ));
        }
        if self
            .content_type
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(invalid_field(
                "sora ordered mailbox result",
                "content_type",
                "must not be blank when present",
            ));
        }
        for mutation in &self.state_mutations {
            mutation.validate()?;
        }
        for message in &self.outbound_mailbox_messages {
            message.validate_submission()?;
        }
        if let Some(state) = self.observed_runtime_state.as_ref() {
            state.validate()?;
        }
        if let Some(state) = self.runtime_state.as_ref() {
            state.validate()?;
        }
        self.runtime_receipt.validate_submission()?;
        if self.runtime_receipt.mailbox_message_id.is_none() {
            return Err(invalid_field(
                "sora ordered mailbox result",
                "runtime_receipt.mailbox_message_id",
                "must identify the consumed mailbox message",
            ));
        }
        if self.runtime_receipt.execution_host.is_none() {
            return Err(invalid_field(
                "sora ordered mailbox result",
                "runtime_receipt.execution_host",
                "must carry deterministic-validator attribution",
            ));
        }
        Ok(())
    }
}
/// Authoritative execution receipt emitted by the generic Soracloud runtime.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraRuntimeReceiptV1 {
    /// Schema version; must equal [`SORA_RUNTIME_RECEIPT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic receipt identifier.
    pub receipt_id: Hash,
    /// Service that emitted the receipt.
    pub service_name: Name,
    /// Service revision that produced the receipt.
    pub service_version: String,
    /// Handler that executed the workload.
    pub handler_name: Name,
    /// Execution class for the receipt.
    pub handler_class: SoraServiceHandlerClassV1,
    /// Commitment over the request envelope.
    pub request_commitment: Hash,
    /// Commitment over the result envelope.
    pub result_commitment: Hash,
    /// Certification mode used for the response or audit record.
    pub certified_by: SoraCertifiedResponsePolicyV1,
    /// Ledger-assigned ordered sequence that emitted the receipt.
    ///
    /// A [`crate::isi::soracloud::RecordSoracloudRuntimeReceipt`] submission must set this to zero;
    /// ledger execution replaces the sentinel with the next authoritative Soracloud sequence
    /// before validating and persisting the receipt.
    pub emitted_sequence: u64,
    /// Exact active validator selected for deterministic execution, when host-attributed.
    #[norito(required)]
    pub execution_host: Option<SoraRuntimeDeterministicValidatorHostV1>,
    /// Optional mailbox message that triggered the execution.
    #[norito(required)]
    pub mailbox_message_id: Option<Hash>,
    /// Journal artifact hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    /// Checkpoint artifact hash; the wire key is explicitly null when absent.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
}
/// Derive the sequence-independent identifier for a non-mailbox runtime receipt.
///
/// The identifier binds every immutable execution and artifact-attribution field while excluding
/// only the ledger-assigned sequence and the identifier itself.
#[must_use]
pub fn derive_soracloud_local_read_receipt_id_v1(receipt: &SoraRuntimeReceiptV1) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud:local-read-receipt:v1",
        receipt.service_name.as_ref(),
        receipt.service_version.as_str(),
        receipt.handler_name.as_ref(),
        receipt.handler_class,
        receipt.request_commitment,
        receipt.result_commitment,
        receipt.certified_by,
        receipt.execution_host.clone(),
        receipt.journal_artifact_hash,
        receipt.checkpoint_artifact_hash,
    )))
}
impl SoraRuntimeReceiptV1 {
    /// Validate a ledger-assigned runtime receipt.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// handler-class/certification invariants are violated.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(true)
    }
    /// Validate a runtime receipt prepared for ledger submission.
    ///
    /// Submission receipts carry the zero sequence sentinel. Ledger execution assigns the next
    /// authoritative sequence while preserving the deterministic receipt identifier.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the receipt is malformed or already carries a
    /// caller-controlled sequence.
    pub fn validate_submission(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(false)
    }
    fn validate_with_sequence_state(
        &self,
        require_assigned_sequence: bool,
    ) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora runtime receipt",
            self.schema_version,
            SORA_RUNTIME_RECEIPT_VERSION_V1,
        )?;
        validate_exact_token(
            "sora runtime receipt",
            "service_version",
            &self.service_version,
        )?;
        validate_soracloud_digest_hash("sora runtime receipt", "receipt_id", self.receipt_id)?;
        validate_soracloud_digest_hash(
            "sora runtime receipt",
            "request_commitment",
            self.request_commitment,
        )?;
        validate_soracloud_digest_hash(
            "sora runtime receipt",
            "result_commitment",
            self.result_commitment,
        )?;
        if require_assigned_sequence && self.emitted_sequence == 0 {
            return Err(invalid_field(
                "sora runtime receipt",
                "emitted_sequence",
                "must be assigned by the ledger before persistence",
            ));
        }
        if !require_assigned_sequence && self.emitted_sequence != 0 {
            return Err(invalid_field(
                "sora runtime receipt",
                "emitted_sequence",
                "must be zero before ledger submission",
            ));
        }
        if let Some(execution_host) = self.execution_host.as_ref() {
            execution_host.validate()?;
        }
        if let Some(mailbox_message_id) = self.mailbox_message_id {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "mailbox_message_id",
                mailbox_message_id,
            )?;
        }
        if let Some(journal_artifact_hash) = self.journal_artifact_hash {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "journal_artifact_hash",
                journal_artifact_hash,
            )?;
        }
        if let Some(checkpoint_artifact_hash) = self.checkpoint_artifact_hash {
            validate_soracloud_digest_hash(
                "sora runtime receipt",
                "checkpoint_artifact_hash",
                checkpoint_artifact_hash,
            )?;
        }
        match self.handler_class {
            SoraServiceHandlerClassV1::Asset | SoraServiceHandlerClassV1::Query => {
                if self.certified_by == SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "certified_by",
                        "asset/query receipts must remain certified",
                    ));
                }
                if self.mailbox_message_id.is_some() {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "mailbox_message_id",
                        "asset/query receipts must not originate from mailbox delivery",
                    ));
                }
            }
            SoraServiceHandlerClassV1::Update => {
                if self.certified_by != SoraCertifiedResponsePolicyV1::None {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "certified_by",
                        "update receipts use public ordered mailbox execution instead of certified fast-path responses",
                    ));
                }
                if self.mailbox_message_id.is_none() {
                    return Err(invalid_field(
                        "sora runtime receipt",
                        "mailbox_message_id",
                        "update/private_update receipts must identify the consumed mailbox message",
                    ));
                }
            }
        }
        Ok(())
    }
}
