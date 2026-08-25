/// Soracloud action recorded in authoritative service audit history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceLifecycleActionV1 {
    /// First-time admission of a service.
    Deploy,
    /// Admission of a new candidate revision.
    Upgrade,
    /// Deterministic config mutation against the active service deployment.
    ConfigMutation,
    /// Deterministic secret mutation against the active service deployment.
    SecretMutation,
    /// Deterministic state mutation against a declared binding.
    StateMutation,
    /// Deterministic FHE execution that materialized a ciphertext result.
    FheJobRun,
    /// First admission of governance-authenticated FHE material.
    FhePolicyRegister,
    /// Monotonic rotation to a new governance-authenticated FHE material version.
    FhePolicyRotate,
    /// Permanent revocation of the active FHE policy version.
    FhePolicyRevoke,
    /// Policy-gated decryption or health-access request.
    DecryptionRequest,
    /// Certified ciphertext metadata query served from authoritative state.
    CiphertextQuery,
    /// Rollout progression for an admitted candidate revision.
    Rollout,
    /// Reversion to an already admitted baseline revision.
    Rollback,
    /// One exact accepted hosted-service egress checkpoint transition.
    LeaseUsage,
    /// Atomic settlement and successor opening of a hosted-service reporting epoch.
    LeaseReportingEpochRollover,
}
/// Exact authoritative service revision observed by an upgrade signer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoraServiceExactCurrentRevisionPreconditionV1 {
    /// Active service version observed by the signer.
    pub service_version: String,
    /// Active service-manifest hash observed by the signer.
    pub service_manifest_hash: Hash,
    /// Active container-manifest hash observed by the signer.
    pub container_manifest_hash: Hash,
    /// Positive active process generation observed by the signer.
    pub process_generation: u64,
    /// Active config generation observed by the signer.
    pub config_generation: u64,
    /// Active secret generation observed by the signer.
    pub secret_generation: u64,
}
/// Signed compare-and-set condition for a service deploy or upgrade.
///
/// The condition is evaluated against authoritative deployment state in the
/// same ledger transaction that admits the new revision. This prevents a
/// status preflight from becoming a time-of-check/time-of-use race.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "condition", content = "value"))]
pub enum SoraServiceMutationPreconditionV1 {
    /// A first deployment is valid only while no deployment state exists for
    /// the service name carried by the signed bundle.
    ServiceAbsent,
    /// An upgrade is valid only while every field of the observed active
    /// revision still matches authoritative deployment state.
    ExactCurrentRevision(SoraServiceExactCurrentRevisionPreconditionV1),
}
/// Mutation mode recorded for authoritative Soracloud state updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraStateMutationOperationV1 {
    /// Create or replace a state entry.
    Upsert,
    /// Remove an existing state entry.
    Delete,
}
/// Rollout stage tracked for a candidate service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "stage", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraRolloutStageV1 {
    /// Candidate revision is serving a canary fraction of traffic.
    #[default]
    Canary,
    /// Candidate revision has been promoted to full traffic.
    Promoted,
    /// Candidate revision has been rolled back.
    RolledBack,
}
/// Authoritative rollout state tracked for a service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceRolloutStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_ROLLOUT_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic rollout identifier.
    pub rollout_handle: String,
    /// Baseline version retained for traffic splitting and automatic rollback.
    pub baseline_version: String,
    /// Candidate version being evaluated.
    pub candidate_version: String,
    /// Initial canary percentage requested by the deployment policy.
    pub canary_percent: u8,
    /// Current traffic percentage allocated to the candidate.
    pub traffic_percent: u8,
    /// Rollout phase.
    pub stage: SoraRolloutStageV1,
    /// Consecutive health failures recorded for the rollout.
    pub health_failures: u32,
    /// Threshold that triggers automatic rollback.
    pub max_health_failures: u32,
    /// Health window applied to rollout progression.
    pub health_window_secs: u32,
    /// Audit sequence that created the rollout.
    pub created_sequence: u64,
    /// Audit sequence that last updated the rollout.
    pub updated_sequence: u64,
}
impl SoraServiceRolloutStateV1 {
    /// Validate rollout sequencing and percentage constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version, traffic, or handle invariants are violated.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service rollout state",
            self.schema_version,
            SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service rollout state",
            "rollout_handle",
            &self.rollout_handle,
        )?;
        validate_nonblank_field(
            "sora service rollout state",
            "candidate_version",
            &self.candidate_version,
        )?;
        validate_nonblank_field(
            "sora service rollout state",
            "baseline_version",
            &self.baseline_version,
        )?;
        if self.baseline_version == self.candidate_version {
            return Err(invalid_field(
                "sora service rollout state",
                "baseline_version",
                "must differ from candidate_version",
            ));
        }
        if self.canary_percent > 100 {
            return Err(invalid_field(
                "sora service rollout state",
                "canary_percent",
                "must be within 0..=100",
            ));
        }
        if self.traffic_percent > 100 {
            return Err(invalid_field(
                "sora service rollout state",
                "traffic_percent",
                "must be within 0..=100",
            ));
        }
        match self.stage {
            SoraRolloutStageV1::Canary => {
                if !(1..100).contains(&self.canary_percent) {
                    return Err(invalid_field(
                        "sora service rollout state",
                        "canary_percent",
                        "canary rollouts must start with a nonzero partial traffic allocation",
                    ));
                }
                if !(self.canary_percent..100).contains(&self.traffic_percent) {
                    return Err(invalid_field(
                        "sora service rollout state",
                        "traffic_percent",
                        "canary traffic must stay at or above canary_percent and below 100",
                    ));
                }
            }
            SoraRolloutStageV1::Promoted => {
                if self.traffic_percent != 100 {
                    return Err(invalid_field(
                        "sora service rollout state",
                        "traffic_percent",
                        "promoted rollouts must serve 100 percent of traffic",
                    ));
                }
            }
            SoraRolloutStageV1::RolledBack => {
                if self.traffic_percent != 0 {
                    return Err(invalid_field(
                        "sora service rollout state",
                        "traffic_percent",
                        "rolled-back rollouts must serve 0 percent of traffic",
                    ));
                }
            }
        }
        if self.max_health_failures == 0 {
            return Err(invalid_field(
                "sora service rollout state",
                "max_health_failures",
                "must be greater than zero",
            ));
        }
        match self.stage {
            SoraRolloutStageV1::Canary if self.health_failures >= self.max_health_failures => {
                return Err(invalid_field(
                    "sora service rollout state",
                    "health_failures",
                    "canary rollouts must remain below the automatic rollback threshold",
                ));
            }
            SoraRolloutStageV1::Promoted if self.health_failures != 0 => {
                return Err(invalid_field(
                    "sora service rollout state",
                    "health_failures",
                    "promoted rollouts must clear consecutive health failures",
                ));
            }
            SoraRolloutStageV1::RolledBack if self.health_failures < self.max_health_failures => {
                return Err(invalid_field(
                    "sora service rollout state",
                    "health_failures",
                    "rolled-back rollouts must meet the automatic rollback threshold",
                ));
            }
            _ => {}
        }
        if self.health_window_secs == 0 {
            return Err(invalid_field(
                "sora service rollout state",
                "health_window_secs",
                "must be greater than zero",
            ));
        }
        if self.updated_sequence < self.created_sequence {
            return Err(invalid_field(
                "sora service rollout state",
                "updated_sequence",
                "must be greater than or equal to created_sequence",
            ));
        }
        Ok(())
    }
}
/// Authoritative deployment state for the currently active Soracloud service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceDeploymentStateV1 {
    /// Schema version; must equal [`SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose lifecycle is being tracked.
    pub service_name: Name,
    /// Currently active service version.
    pub current_service_version: String,
    /// Hash of the active service manifest.
    pub current_service_manifest_hash: Hash,
    /// Hash of the active container manifest.
    pub current_container_manifest_hash: Hash,
    /// Count of admitted distinct revisions for the service.
    pub revision_count: u32,
    /// Current simulated process generation for the active revision.
    pub process_generation: u64,
    /// Audit sequence that started the current process generation.
    pub process_started_sequence: u64,
    /// Monotonic generation of service config updates.
    pub config_generation: u64,
    /// Monotonic generation of service secret updates.
    pub secret_generation: u64,
    /// Authoritative config entries scoped to the active service deployment.
    pub service_configs: BTreeMap<String, SoraServiceConfigEntryV1>,
    /// Authoritative encrypted secret entries scoped to the active service deployment.
    pub service_secrets: BTreeMap<String, SoraServiceSecretEntryV1>,
    /// Versioned governance-authenticated FHE policy material scoped to this service.
    pub fhe_policy_records: BTreeMap<Name, SoracloudFhePolicyRecordV1>,
    /// Active rollout, when the candidate is still under evaluation.
    #[norito(required)]
    pub active_rollout: Option<SoraServiceRolloutStateV1>,
    /// Most recent rollout observation for the service.
    #[norito(required)]
    pub last_rollout: Option<SoraServiceRolloutStateV1>,
    /// Authoritative hosted-service lease and prepaid economics, when this
    /// deployment targets the HTTP service plane.
    #[norito(required)]
    pub service_lease: Option<SoraServiceLeaseStateV1>,
    /// Authoritative lease-backed storage bindings attached to the deployment.
    pub lease_volume_states: Vec<SoraServiceLeaseVolumeStateV1>,
}
impl SoraServiceDeploymentStateV1 {
    /// Validate active deployment state.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when version, sequence, or rollout
    /// invariants are violated.
    #[allow(
        clippy::too_many_lines,
        reason = "deployment validation keeps the ordered cross-field checks and stable first-error precedence together"
    )]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service deployment state",
            self.schema_version,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service deployment state",
            "current_service_version",
            &self.current_service_version,
        )?;
        validate_soracloud_digest_hash(
            "sora service deployment state",
            "current_service_manifest_hash",
            self.current_service_manifest_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora service deployment state",
            "current_container_manifest_hash",
            self.current_container_manifest_hash,
        )?;
        for (field, value) in [
            ("revision_count", u64::from(self.revision_count)),
            ("process_generation", self.process_generation),
            ("process_started_sequence", self.process_started_sequence),
        ] {
            if value == 0 {
                return Err(invalid_field(
                    "sora service deployment state",
                    field,
                    "must be greater than zero",
                ));
            }
        }
        for (config_name, entry) in &self.service_configs {
            entry.validate()?;
            if entry.config_name != *config_name {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora service deployment state",
                    field: "service_configs",
                    reason: format!(
                        "map key `{config_name}` must match embedded config_name `{}`",
                        entry.config_name
                    ),
                });
            }
        }
        for (secret_name, entry) in &self.service_secrets {
            entry.validate()?;
            if entry.secret_name != *secret_name {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora service deployment state",
                    field: "service_secrets",
                    reason: format!(
                        "map key `{secret_name}` must match embedded secret_name `{}`",
                        entry.secret_name
                    ),
                });
            }
        }
        for (policy_name, record) in &self.fhe_policy_records {
            record.validate()?;
            if record.service_name != self.service_name || record.policy_name != *policy_name {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora service deployment state",
                    field: "fhe_policy_records",
                    reason: format!(
                        "map key `{policy_name}` and embedded service/policy must match deployment `{}`",
                        self.service_name
                    ),
                });
            }
        }
        if let Some(active_rollout) = self.active_rollout.as_ref() {
            active_rollout.validate()?;
            if active_rollout.stage != SoraRolloutStageV1::Canary {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout.stage",
                    "active_rollout may only track canary progress",
                ));
            }
            if !(1..100).contains(&active_rollout.traffic_percent) {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout.traffic_percent",
                    "active canary rollout must split traffic between baseline and candidate",
                ));
            }
            if !(1..100).contains(&active_rollout.canary_percent) {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout.canary_percent",
                    "active canary rollout must start with a nonzero partial traffic allocation",
                ));
            }
            if active_rollout.candidate_version != self.current_service_version {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout.candidate_version",
                    "must match current_service_version",
                ));
            }
        }
        if let Some(last_rollout) = self.last_rollout.as_ref() {
            last_rollout.validate()?;
        }
        match (self.active_rollout.as_ref(), self.last_rollout.as_ref()) {
            (Some(active), Some(last)) if active == last => {}
            (Some(_), Some(_)) => {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout",
                    "must exactly equal last_rollout while a canary is active",
                ));
            }
            (Some(_), None) => {
                return Err(invalid_field(
                    "sora service deployment state",
                    "last_rollout",
                    "must retain the active rollout",
                ));
            }
            (None, Some(last)) if last.stage == SoraRolloutStageV1::Canary => {
                return Err(invalid_field(
                    "sora service deployment state",
                    "active_rollout",
                    "must retain a canary last_rollout until it is promoted or rolled back",
                ));
            }
            (None, Some(_)) | (None, None) => {}
        }
        if let Some(lease) = self.service_lease.as_ref() {
            lease.validate()?;
        }
        let mut volume_names = BTreeSet::new();
        for volume in &self.lease_volume_states {
            volume.validate()?;
            if !volume_names.insert(volume.volume_name.clone()) {
                return Err(SoracloudManifestError::DuplicateLeaseVolume {
                    volume: volume.volume_name.clone(),
                });
            }
        }
        if self.service_lease.is_none() && !self.lease_volume_states.is_empty() {
            return Err(invalid_field(
                "sora service deployment state",
                "lease_volume_states",
                "lease-backed volume state requires an active hosted-service lease",
            ));
        }
        if let Some(lease) = self.service_lease.as_ref()
            && self.lease_volume_states.iter().any(|volume| {
                volume.lease_started_height != lease.lease_started_height
                    || volume.lease_expires_height != lease.lease_expires_height
            })
        {
            return Err(invalid_field(
                "sora service deployment state",
                "lease_volume_states",
                "every volume economic start and expiry must exactly match the hosted-service lease",
            ));
        }
        Ok(())
    }
    /// Maximum authoritative leased-storage bytes retained by the deployment.
    #[must_use]
    pub fn accounted_storage_bytes(&self) -> u64 {
        self.lease_volume_states.iter().fold(0_u64, |acc, volume| {
            acc.saturating_add(volume.max_total_bytes)
        })
    }
    /// Effective hosted-service lease status at the observed consensus height.
    ///
    /// # Errors
    /// Returns a bounded-domain accounting error.
    pub fn hosted_service_lease_status_at(
        &self,
        current_height: u64,
    ) -> Result<Option<SoraServiceLeaseStatusV1>, NumericOperationError> {
        self.service_lease.as_ref().map_or(Ok(None), |lease| {
            lease
                .status_at(current_height, self.accounted_storage_bytes())
                .map(Some)
        })
    }
    /// Effective remaining prepaid runtime balance at the observed consensus height.
    ///
    /// # Errors
    /// Returns a bounded-domain accounting error.
    pub fn hosted_service_remaining_balance(
        &self,
        current_height: u64,
    ) -> Result<Option<Quantity>, NumericOperationError> {
        self.service_lease.as_ref().map_or(Ok(None), |lease| {
            lease
                .remaining_balance(current_height, self.accounted_storage_bytes())
                .map(Some)
        })
    }
    /// Returns `true` when the hosted-service plane may still be routed and
    /// materialized at the observed consensus height.
    ///
    /// # Errors
    /// Returns a bounded-domain accounting error.
    pub fn hosted_service_lease_active_at(
        &self,
        current_height: u64,
    ) -> Result<bool, NumericOperationError> {
        self.service_lease.as_ref().map_or(Ok(false), |lease| {
            lease.is_active_at(current_height, self.accounted_storage_bytes())
        })
    }
}
fn validate_service_material_name(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let trimmed = value.trim();
    validate_nonblank_field(manifest, field, trimmed)?;
    if trimmed.len() != value.len() {
        return Err(invalid_field(
            manifest,
            field,
            "must not include leading or trailing whitespace",
        ));
    }
    if value.starts_with('/') {
        return Err(invalid_field(manifest, field, "must not start with '/'"));
    }
    if value.contains("..") {
        return Err(invalid_field(
            manifest,
            field,
            "must not contain '..' path traversal segments",
        ));
    }
    if value.len() > 256 {
        return Err(invalid_field(manifest, field, "must not exceed 256 bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_field(
            manifest,
            field,
            "must not contain control characters",
        ));
    }
    Ok(())
}
fn validate_nonempty_no_control(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field(manifest, field, value)?;
    if value.trim() != value {
        return Err(invalid_field(
            manifest,
            field,
            "must not include surrounding whitespace",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_field(
            manifest,
            field,
            "must not contain control characters",
        ));
    }
    Ok(())
}
fn validate_distribution_geography_tag(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonempty_no_control(manifest, field, value)?;
    if value.len() > 128 {
        return Err(invalid_field(
            manifest,
            field,
            "geography tags must not exceed 128 bytes",
        ));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(invalid_field(
            manifest,
            field,
            "geography tags must not contain whitespace",
        ));
    }
    Ok(())
}
fn validate_environment_variable_name(
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let manifest = "sora container manifest";
    validate_nonblank_field(manifest, field, value)?;
    if value.trim() != value {
        return Err(invalid_field(
            manifest,
            field,
            "environment variable name must not include surrounding whitespace",
        ));
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(SoracloudManifestError::EmptyField { manifest, field });
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "environment variable name `{value}` must start with an ASCII letter or '_'"
            ),
        });
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "environment variable name `{value}` must use only ASCII letters, digits, and '_'"
            ),
        });
    }
    Ok(())
}
fn validate_config_export_relative_path(value: &str) -> Result<(), SoracloudManifestError> {
    let manifest = "sora container manifest";
    let field = "config_exports";
    let trimmed = value.trim();
    validate_nonblank_field(manifest, field, trimmed)?;
    if trimmed != value {
        return Err(invalid_field(
            manifest,
            field,
            "config export file path must not include surrounding whitespace",
        ));
    }
    if value.starts_with('/') || value.ends_with('/') {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "config export file path `{value}` must stay relative and must not end with '/'"
            ),
        });
    }
    if value.contains('\\') {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!("config export file path `{value}` must use '/' separators only"),
        });
    }
    if value.len() > 512 {
        return Err(invalid_field(
            manifest,
            field,
            "config export file path must not exceed 512 bytes",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_field(
            manifest,
            field,
            "config export file path must not contain control characters",
        ));
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(SoracloudManifestError::InvalidField {
                manifest,
                field,
                reason: format!(
                    "config export file path `{value}` must not contain empty, '.' or '..' segments"
                ),
            });
        }
    }
    Ok(())
}
fn validate_canonical_lower_hex_32(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<[u8; 32], SoracloudManifestError> {
    if value.len() != 64 {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "must contain exactly 64 lowercase hexadecimal characters (found {})",
                value.len()
            ),
        });
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_field(
            manifest,
            field,
            "must use canonical lowercase hexadecimal",
        ));
    }
    let bytes = hex::decode(value).map_err(|error| SoracloudManifestError::InvalidField {
        manifest,
        field,
        reason: format!("must decode as hexadecimal: {error}"),
    })?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!("must decode to exactly 32 bytes (found {})", bytes.len()),
        })
}
fn validate_canonical_sorafs_content_cid(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let expected_text_len = 1 + (MANIFEST_ROOT_CID_LENGTH * 8).div_ceil(5);
    if value.len() != expected_text_len || !value.starts_with('b') {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "must be the canonical {expected_text_len}-byte lowercase multibase base32 rendering of a SoraFS manifest root CID"
            ),
        });
    }
    let bytes = decode_lowercase_multibase_base32(value).ok_or_else(|| {
        invalid_field(
            manifest,
            field,
            "must use canonical lowercase multibase base32 without padding",
        )
    })?;
    ManifestRootCid::try_from_slice(&bytes).map_err(|error| {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!("must encode a canonical SoraFS manifest root CID: {error}"),
        }
    })?;
    if encode_lowercase_multibase_base32(&bytes) != value {
        return Err(invalid_field(
            manifest,
            field,
            "must use the exact canonical lowercase multibase base32 spelling",
        ));
    }
    Ok(())
}
fn encode_lowercase_multibase_base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    let mut encoded = Vec::with_capacity(1 + (bytes.len() * 8).div_ceil(5));
    encoded.push(b'b');
    for byte in bytes {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            let index = usize::try_from((accumulator >> (bits - 5)) & 0x1f)
                .expect("base32 alphabet index fits usize");
            encoded.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = usize::try_from((accumulator << (5 - bits)) & 0x1f)
            .expect("base32 alphabet index fits usize");
        encoded.push(ALPHABET[index]);
    }
    String::from_utf8(encoded).expect("lowercase base32 alphabet is UTF-8")
}
fn decode_lowercase_multibase_base32(value: &str) -> Option<Vec<u8>> {
    let encoded = value.strip_prefix('b')?;
    if encoded.is_empty() {
        return None;
    }
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    let mut decoded = Vec::with_capacity((encoded.len() * 5) / 8);
    for byte in encoded.bytes() {
        let value = match byte {
            b'a'..=b'z' => u32::from(byte - b'a'),
            b'2'..=b'7' => 26 + u32::from(byte - b'2'),
            _ => return None,
        };
        accumulator = (accumulator << 5) | value;
        bits += 5;
        while bits >= 8 {
            decoded.push(((accumulator >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }
    if bits > 0 {
        let padding_mask = (1_u32 << bits) - 1;
        if accumulator & padding_mask != 0 {
            return None;
        }
    }
    Some(decoded)
}
fn validate_inrou_image_member_path(
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let manifest = "sora inrou guest image";
    if value.is_empty() {
        return Err(SoracloudManifestError::EmptyField { manifest, field });
    }
    if value.trim() != value {
        return Err(invalid_field(
            manifest,
            field,
            "must not include surrounding whitespace",
        ));
    }
    if value.len() > 512 {
        return Err(invalid_field(manifest, field, "must not exceed 512 bytes"));
    }
    let Some(relative_path) = value.strip_prefix("/inrou/") else {
        return Err(invalid_field(
            manifest,
            field,
            "must be a canonical absolute member path below `/inrou/`",
        ));
    };
    if relative_path.is_empty() || value.ends_with('/') || value.contains('\\') {
        return Err(invalid_field(
            manifest,
            field,
            "must use canonical `/`-separated portable path components",
        ));
    }
    for component in relative_path.split('/') {
        if !is_portable_inrou_path_component(component) {
            return Err(SoracloudManifestError::InvalidField {
                manifest,
                field,
                reason: format!("contains non-portable path component `{component}`"),
            });
        }
    }
    Ok(())
}
fn is_portable_inrou_path_component(component: &str) -> bool {
    if component.is_empty()
        || component == "."
        || component == ".."
        || !component.is_ascii()
        || component.len() > 255
        || component
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'-'))
        || component.ends_with('.')
    {
        return false;
    }
    let Some(basename) = component.split('.').next() else {
        return false;
    };
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    if let (Some(prefix), Some(suffix)) = (basename.get(..3), basename.get(3..)) {
        let reserved_prefix =
            prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT");
        let reserved_digit = suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9');
        if reserved_prefix && reserved_digit {
            return false;
        }
    }
    true
}
fn validate_bundle_absolute_path(
    manifest: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), SoracloudManifestError> {
    let trimmed = value.trim();
    validate_nonblank_field(manifest, field, trimmed)?;
    if trimmed != value {
        return Err(invalid_field(
            manifest,
            field,
            "must not include surrounding whitespace",
        ));
    }
    if value.len() > 256 {
        return Err(invalid_field(
            manifest,
            field,
            "must not exceed 256 bytes including its leading slash",
        ));
    }
    let Some(relative_path) = value.strip_prefix('/') else {
        return Err(invalid_field(
            manifest,
            field,
            "must be an absolute path within the signed Soracloud bundle",
        ));
    };
    if relative_path.is_empty() || value.ends_with('/') || value.contains('\\') {
        return Err(invalid_field(
            manifest,
            field,
            "must use a canonical nonempty `/`-separated path",
        ));
    }
    for component in relative_path.split('/') {
        if !is_portable_inrou_path_component(component) {
            return Err(SoracloudManifestError::InvalidField {
                manifest,
                field,
                reason: format!("contains non-portable path component `{component}`"),
            });
        }
    }
    Ok(())
}
/// Authoritative config entry tracked for one Soracloud service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceConfigEntryV1 {
    /// Schema version; must equal [`SORA_SERVICE_CONFIG_ENTRY_VERSION_V1`].
    pub schema_version: u16,
    /// Stable service-scoped config identifier.
    pub config_name: String,
    /// Canonical typed config value encoded as canonical JSON text.
    pub value_json: Json,
    /// Deterministic hash of the canonical JSON value.
    pub value_hash: Hash,
    /// Audit sequence of the last update affecting this config entry.
    pub last_update_sequence: u64,
}
impl SoraServiceConfigEntryV1 {
    /// Return the deterministic hash of the canonical JSON value.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the value cannot be encoded canonically.
    pub fn canonical_value_hash(&self) -> Result<Hash, SoracloudManifestError> {
        let payload = canonical_service_config_json_payload(&self.value_json)?;
        Ok(Hash::new(payload))
    }
    /// Validate config entry metadata and hash linkage.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or the
    /// canonical JSON value hash does not match the stored commitment.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service config entry",
            self.schema_version,
            SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
        )?;
        validate_service_material_name(
            "sora service config entry",
            "config_name",
            &self.config_name,
        )?;
        if self.last_update_sequence == 0 {
            return Err(invalid_field(
                "sora service config entry",
                "last_update_sequence",
                "must be greater than zero",
            ));
        }
        let expected = self.canonical_value_hash()?;
        if self.value_hash != expected {
            return Err(invalid_field(
                "sora service config entry",
                "value_hash",
                "must equal the canonical hash of value_json",
            ));
        }
        Ok(())
    }
}
fn canonical_service_config_json_payload(
    value_json: &Json,
) -> Result<Vec<u8>, SoracloudManifestError> {
    let canonical = Json::from_str_norito(value_json.get()).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest: "sora service config entry",
            field: "value_json",
            reason: format!("failed to decode canonical json: {err}"),
        }
    })?;
    if canonical.get() != value_json.get() {
        return Err(invalid_field(
            "sora service config entry",
            "value_json",
            "must use canonical Norito JSON encoding",
        ));
    }
    Ok(canonical.get().as_bytes().to_vec())
}
/// Authoritative encrypted secret entry tracked for one Soracloud service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceSecretEntryV1 {
    /// Schema version; must equal [`SORA_SERVICE_SECRET_ENTRY_VERSION_V1`].
    pub schema_version: u16,
    /// Stable service-scoped secret identifier.
    pub secret_name: String,
    /// Encrypted secret envelope admitted for this service.
    pub envelope: SecretEnvelopeV1,
    /// Audit sequence of the last update affecting this secret entry.
    pub last_update_sequence: u64,
}
impl SoraServiceSecretEntryV1 {
    /// Validate secret-entry metadata and envelope bounds.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or the
    /// embedded secret envelope fails validation.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service secret entry",
            self.schema_version,
            SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
        )?;
        validate_service_material_name(
            "sora service secret entry",
            "secret_name",
            &self.secret_name,
        )?;
        if self.last_update_sequence == 0 {
            return Err(invalid_field(
                "sora service secret entry",
                "last_update_sequence",
                "must be greater than zero",
            ));
        }
        self.envelope.validate()
    }
}

/// Exact replay material for one authoritative service-config transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceConfigMutationV1 {
    /// Create or replace the complete config entry.
    Upsert(SoraServiceConfigEntryV1),
    /// Delete the named entry, which must already exist in the replayed projection.
    Delete(String),
}
impl SoraServiceConfigMutationV1 {
    /// Stable service-scoped entry name affected by the transition.
    #[must_use]
    pub fn config_name(&self) -> &str {
        match self {
            Self::Upsert(entry) => &entry.config_name,
            Self::Delete(config_name) => config_name,
        }
    }

    /// Validate canonical mutation material at its audit sequence.
    pub fn validate_at_sequence(&self, sequence: u64) -> Result<(), SoracloudManifestError> {
        match self {
            Self::Upsert(entry) => {
                entry.validate()?;
                if entry.last_update_sequence != sequence {
                    return Err(invalid_field(
                        "sora service config mutation",
                        "last_update_sequence",
                        "upsert entry must be materialized at the containing audit sequence",
                    ));
                }
            }
            Self::Delete(config_name) => validate_service_material_name(
                "sora service config mutation",
                "config_name",
                config_name,
            )?,
        }
        Ok(())
    }
}

/// Exact replay material for one authoritative encrypted-secret transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraServiceSecretMutationV1 {
    /// Create or replace the complete encrypted-secret entry.
    Upsert(SoraServiceSecretEntryV1),
    /// Delete the named entry, which must already exist in the replayed projection.
    Delete(String),
}
impl SoraServiceSecretMutationV1 {
    /// Stable service-scoped entry name affected by the transition.
    #[must_use]
    pub fn secret_name(&self) -> &str {
        match self {
            Self::Upsert(entry) => &entry.secret_name,
            Self::Delete(secret_name) => secret_name,
        }
    }

    /// Validate canonical mutation material at its audit sequence.
    pub fn validate_at_sequence(&self, sequence: u64) -> Result<(), SoracloudManifestError> {
        match self {
            Self::Upsert(entry) => {
                entry.validate()?;
                if entry.last_update_sequence != sequence {
                    return Err(invalid_field(
                        "sora service secret mutation",
                        "last_update_sequence",
                        "upsert entry must be materialized at the containing audit sequence",
                    ));
                }
            }
            Self::Delete(secret_name) => validate_service_material_name(
                "sora service secret mutation",
                "secret_name",
                secret_name,
            )?,
        }
        Ok(())
    }
}

/// Commit to the complete canonical post-transition config projection.
#[must_use]
pub fn derive_soracloud_service_config_snapshot_hash_v1(
    entries: &BTreeMap<String, SoraServiceConfigEntryV1>,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.service.config.snapshot.v1",
        entries.clone(),
    )))
}
/// Commit to the complete canonical post-transition encrypted-secret projection.
#[must_use]
pub fn derive_soracloud_service_secret_snapshot_hash_v1(
    entries: &BTreeMap<String, SoraServiceSecretEntryV1>,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud.service.secret.snapshot.v1",
        entries.clone(),
    )))
}
/// Authoritative service-state entry tracked for Soracloud bindings.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraServiceStateEntryV1 {
    /// Schema version; must equal [`SORA_SERVICE_STATE_ENTRY_VERSION_V1`].
    pub schema_version: u16,
    /// Service owning the ciphertext state row.
    pub service_name: Name,
    /// Active service revision that produced the latest ciphertext row.
    pub service_version: String,
    /// Binding owning the ciphertext state row.
    pub binding_name: Name,
    /// Canonical state key scoped under the binding prefix.
    pub state_key: String,
    /// Encryption mode for the stored payload.
    pub encryption: SoraStateEncryptionV1,
    /// Full authoritative payload bytes. FHE rows store the encoded ciphertext envelope.
    pub payload: Vec<u8>,
    /// Stored payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Deterministic payload commitment.
    pub payload_commitment: Hash,
    /// Public-key digest bound to admitted FHE ciphertext rows.
    #[norito(required)]
    pub fhe_public_key_digest: Option<Hash>,
    /// Public BFV residual-multiple or noise bound for FHE ciphertext rows, when known.
    ///
    /// This is public deterministic metadata for chained validator-side FHE jobs. Client-provided
    /// FHE rows may set it to `null` until proof-carrying input admission is available.
    #[norito(required)]
    pub fhe_residual_multiple_bound: Option<u128>,
    /// Semantics of `fhe_residual_multiple_bound`.
    ///
    /// `null` means no bound mode is available and is only valid when
    /// `fhe_residual_multiple_bound` is also `null`.
    #[norito(required)]
    pub fhe_bound_mode: Option<BfvCiphertextBoundModeV1>,
    /// Audit sequence of the last update affecting this state key.
    pub last_update_sequence: u64,
    /// Governance linkage hash bound to the last mutation.
    pub governance_tx_hash: Hash,
    /// Action that produced the current ciphertext row.
    pub source_action: SoraServiceLifecycleActionV1,
}
impl SoraServiceStateEntryV1 {
    /// Validate deterministic service-state entry metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the state key is
    /// malformed, or plaintext state is exposed through the ciphertext projection surface.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora service state entry",
            self.schema_version,
            SORA_SERVICE_STATE_ENTRY_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora service state entry",
            "service_version",
            &self.service_version,
        )?;
        validate_nonblank_field("sora service state entry", "state_key", &self.state_key)?;
        if !self.state_key.starts_with('/') {
            return Err(invalid_field(
                "sora service state entry",
                "state_key",
                "must start with '/'",
            ));
        }
        if self.last_update_sequence == 0 {
            return Err(invalid_field(
                "sora service state entry",
                "last_update_sequence",
                "must be greater than zero",
            ));
        }
        let payload_len = u64::try_from(self.payload.len()).map_err(|_| {
            invalid_field(
                "sora service state entry",
                "payload",
                "payload length exceeds supported u64 range",
            )
        })?;
        if self.payload_bytes.get() != payload_len {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora service state entry",
                field: "payload_bytes",
                reason: format!(
                    "declares {} bytes but payload has {} bytes",
                    self.payload_bytes, payload_len
                ),
            });
        }
        if self.payload_commitment != Hash::new(&self.payload) {
            return Err(invalid_field(
                "sora service state entry",
                "payload_commitment",
                "must equal the hash of payload bytes",
            ));
        }
        validate_soracloud_digest_hash(
            "sora service state entry",
            "governance_tx_hash",
            self.governance_tx_hash,
        )?;
        validate_service_state_fhe_bound_metadata(
            self.encryption,
            self.fhe_residual_multiple_bound,
            self.fhe_bound_mode,
            self.fhe_public_key_digest,
        )?;
        if !matches!(
            self.source_action,
            SoraServiceLifecycleActionV1::StateMutation | SoraServiceLifecycleActionV1::FheJobRun
        ) {
            return Err(invalid_field(
                "sora service state entry",
                "source_action",
                "must be StateMutation or FheJobRun",
            ));
        }
        Ok(())
    }
}
fn validate_service_state_fhe_bound_metadata(
    encryption: SoraStateEncryptionV1,
    bound: Option<u128>,
    bound_mode: Option<BfvCiphertextBoundModeV1>,
    public_key_digest: Option<Hash>,
) -> Result<(), SoracloudManifestError> {
    if encryption != SoraStateEncryptionV1::FheCiphertext && bound.is_some() {
        return Err(invalid_field(
            "sora service state entry",
            "fhe_residual_multiple_bound",
            "requires FHE ciphertext encryption",
        ));
    }
    if encryption != SoraStateEncryptionV1::FheCiphertext && bound_mode.is_some() {
        return Err(invalid_field(
            "sora service state entry",
            "fhe_bound_mode",
            "requires FHE ciphertext encryption",
        ));
    }
    if encryption != SoraStateEncryptionV1::FheCiphertext && public_key_digest.is_some() {
        return Err(invalid_field(
            "sora service state entry",
            "fhe_public_key_digest",
            "requires FHE ciphertext encryption",
        ));
    }
    if let Some(public_key_digest) = public_key_digest {
        validate_soracloud_digest_hash(
            "sora service state entry",
            "fhe_public_key_digest",
            public_key_digest,
        )?;
    }
    if encryption == SoraStateEncryptionV1::FheCiphertext && bound_mode.is_some() && bound.is_none()
    {
        return Err(invalid_field(
            "sora service state entry",
            "fhe_bound_mode",
            "requires fhe_residual_multiple_bound",
        ));
    }
    if let Some(bound) = bound {
        if public_key_digest.is_none() {
            return Err(invalid_field(
                "sora service state entry",
                "fhe_public_key_digest",
                "requires admitted FHE public-key digest when bound metadata is present",
            ));
        }
        let Some(bound_mode) = bound_mode else {
            return Err(invalid_field(
                "sora service state entry",
                "fhe_bound_mode",
                "requires explicit bound semantics when fhe_residual_multiple_bound is present",
            ));
        };
        validate_soracloud_bfv_ciphertext_bound_capacity(
            bound,
            bound_mode,
            "sora service state entry",
            "fhe_residual_multiple_bound",
            "sora service state entry FHE ciphertext bound",
        )?;
    }
    Ok(())
}
/// Authoritative record of a policy-gated decryption or health-access request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraDecryptionRequestRecordV1 {
    /// Schema version; must equal [`SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose ciphertext state is being accessed.
    pub service_name: Name,
    /// Active service revision at the time of request recording.
    pub service_version: String,
    /// Snapshotted policy attached to the request for immutable audit.
    pub policy: DecryptionAuthorityPolicyV1,
    /// Recorded decryption request payload.
    pub request: DecryptionRequestV1,
    /// Audit sequence that recorded the request.
    pub sequence: u64,
    /// Provenance signer that authorized the request.
    pub signer: PublicKey,
}
impl SoraDecryptionRequestRecordV1 {
    /// Validate schema version, policy/request linkage, and audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the
    /// service version is empty, the sequence is invalid, or the request does
    /// not satisfy the attached policy snapshot.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora decryption request record",
            self.schema_version,
            SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora decryption request record",
            "service_version",
            &self.service_version,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora decryption request record",
                "sequence",
                "must be greater than zero",
            ));
        }
        self.request.validate_for_policy(&self.policy)?;
        Ok(())
    }
    /// Return the canonical hash of the attached policy snapshot.
    pub fn policy_snapshot_hash(&self) -> Hash {
        Hash::new(Encode::encode(&self.policy))
    }
}
/// Training-job lifecycle status tracked by the authoritative Soracloud model runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraTrainingJobStatusV1 {
    /// Job is actively executing and may emit checkpoints.
    Running,
    /// Job completed its planned target steps successfully.
    Completed,
    /// Job is paused pending a deterministic retry decision.
    RetryPending,
    /// Job exhausted its retry budget and can no longer advance.
    Exhausted,
}
/// Training-job audit action recorded in authoritative Soracloud state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraTrainingJobActionV1 {
    /// A new deterministic training job was created.
    Start,
    /// A checkpoint updated job progress and resource accounting.
    Checkpoint,
    /// A retry request transitioned the job into retry-pending state.
    Retry,
}
/// Authoritative training-job state tracked for Soracloud-managed model workflows.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraTrainingJobRecordV1 {
    /// Schema version; must equal [`SORA_TRAINING_JOB_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns the training job.
    pub service_name: Name,
    /// Active service revision when the job was last updated.
    pub service_version: String,
    /// Logical model name targeted by the job.
    pub model_name: String,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Current lifecycle status.
    pub status: SoraTrainingJobStatusV1,
    /// Size of the deterministic worker group.
    pub worker_group_size: u16,
    /// Total target step count for the job.
    pub target_steps: u32,
    /// Completed steps recorded so far.
    pub completed_steps: u32,
    /// Required checkpoint cadence in steps.
    pub checkpoint_interval_steps: u32,
    /// Latest checkpoint step, when any checkpoint has been recorded.
    #[norito(required)]
    pub last_checkpoint_step: Option<u32>,
    /// Number of checkpoints recorded for the job.
    pub checkpoint_count: u32,
    /// Number of retries consumed.
    pub retry_count: u8,
    /// Maximum allowed retries.
    pub max_retries: u8,
    /// Compute units charged per worker-group step.
    pub step_compute_units: u64,
    /// Total compute budget allocated to the job.
    pub compute_budget_units: u64,
    /// Compute units consumed so far.
    pub compute_consumed_units: u64,
    /// Total storage budget allocated to checkpoints.
    pub storage_budget_bytes: u64,
    /// Storage bytes consumed by checkpoints so far.
    pub storage_consumed_bytes: u64,
    /// Latest metrics hash recorded by a checkpoint.
    #[norito(required)]
    pub latest_metrics_hash: Option<Hash>,
    /// Latest failure/retry reason, when applicable.
    #[norito(required)]
    pub last_failure_reason: Option<String>,
    /// Audit sequence that created the job.
    pub created_sequence: u64,
    /// Audit sequence that last updated the job.
    pub updated_sequence: u64,
}
impl SoraTrainingJobRecordV1 {
    /// Validate training-job invariants and resource-accounting bounds.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the identifiers are empty,
    /// or the recorded step/budget state is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_identity_fields()?;
        self.validate_progress_fields()?;
        self.validate_storage_fields()?;
        self.validate_digest_fields()?;
        self.validate_sequence_fields()
    }
    fn validate_identity_fields(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora training job record",
            self.schema_version,
            SORA_TRAINING_JOB_RECORD_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora training job record",
            "service_version",
            &self.service_version,
        )?;
        validate_nonblank_field("sora training job record", "model_name", &self.model_name)?;
        validate_nonblank_field("sora training job record", "job_id", &self.job_id)?;
        Ok(())
    }
    fn validate_progress_fields(&self) -> Result<(), SoracloudManifestError> {
        if self.worker_group_size == 0 {
            return Err(invalid_field(
                "sora training job record",
                "worker_group_size",
                "must be greater than zero",
            ));
        }
        if self.target_steps == 0 {
            return Err(invalid_field(
                "sora training job record",
                "target_steps",
                "must be greater than zero",
            ));
        }
        if self.checkpoint_interval_steps == 0 {
            return Err(invalid_field(
                "sora training job record",
                "checkpoint_interval_steps",
                "must be greater than zero",
            ));
        }
        if self.checkpoint_interval_steps > self.target_steps {
            return Err(invalid_field(
                "sora training job record",
                "checkpoint_interval_steps",
                "must not exceed target_steps",
            ));
        }
        if self.completed_steps > self.target_steps {
            return Err(invalid_field(
                "sora training job record",
                "completed_steps",
                "must not exceed target_steps",
            ));
        }
        if self.step_compute_units == 0 {
            return Err(invalid_field(
                "sora training job record",
                "step_compute_units",
                "must be greater than zero",
            ));
        }
        if self.compute_budget_units == 0 {
            return Err(invalid_field(
                "sora training job record",
                "compute_budget_units",
                "must be greater than zero",
            ));
        }
        if self.compute_consumed_units > self.compute_budget_units {
            return Err(invalid_field(
                "sora training job record",
                "compute_consumed_units",
                "must not exceed compute_budget_units",
            ));
        }
        if let Some(last_checkpoint_step) = self.last_checkpoint_step
            && (last_checkpoint_step == 0 || last_checkpoint_step > self.completed_steps)
        {
            return Err(invalid_field(
                "sora training job record",
                "last_checkpoint_step",
                "must be within 1..=completed_steps",
            ));
        }
        Ok(())
    }
    fn validate_storage_fields(&self) -> Result<(), SoracloudManifestError> {
        if self.storage_budget_bytes == 0 {
            return Err(invalid_field(
                "sora training job record",
                "storage_budget_bytes",
                "must be greater than zero",
            ));
        }
        if self.storage_consumed_bytes > self.storage_budget_bytes {
            return Err(invalid_field(
                "sora training job record",
                "storage_consumed_bytes",
                "must not exceed storage_budget_bytes",
            ));
        }
        Ok(())
    }
    fn validate_sequence_fields(&self) -> Result<(), SoracloudManifestError> {
        if self.created_sequence == 0 || self.updated_sequence == 0 {
            return Err(invalid_field(
                "sora training job record",
                "sequence",
                "created_sequence and updated_sequence must be greater than zero",
            ));
        }
        if self.updated_sequence < self.created_sequence {
            return Err(invalid_field(
                "sora training job record",
                "updated_sequence",
                "must be >= created_sequence",
            ));
        }
        Ok(())
    }
    fn validate_digest_fields(&self) -> Result<(), SoracloudManifestError> {
        if let Some(latest_metrics_hash) = self.latest_metrics_hash {
            validate_soracloud_digest_hash(
                "sora training job record",
                "latest_metrics_hash",
                latest_metrics_hash,
            )?;
        }
        Ok(())
    }
}
/// Audit record for deterministic training-job lifecycle updates.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraTrainingJobAuditEventV1 {
    /// Schema version; must equal [`SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Training-job action that produced this event.
    pub action: SoraTrainingJobActionV1,
    /// Service that owns the job.
    pub service_name: Name,
    /// Active service revision when the event was emitted.
    pub service_version: String,
    /// Model targeted by the job.
    pub model_name: String,
    /// Job identifier.
    pub job_id: String,
    /// Resulting status after the event.
    pub status: SoraTrainingJobStatusV1,
    /// Completed step count after the event.
    pub completed_steps: u32,
    /// Checkpoint count after the event.
    pub checkpoint_count: u32,
    /// Retry count after the event.
    pub retry_count: u8,
    /// Compute units consumed after the event.
    pub compute_consumed_units: u64,
    /// Storage bytes consumed after the event.
    pub storage_consumed_bytes: u64,
    /// Latest checkpoint step associated with the event.
    #[norito(required)]
    pub last_checkpoint_step: Option<u32>,
    /// Latest metrics hash associated with the event.
    #[norito(required)]
    pub latest_metrics_hash: Option<Hash>,
    /// Latest failure reason associated with the event.
    #[norito(required)]
    pub last_failure_reason: Option<String>,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
}
impl SoraTrainingJobAuditEventV1 {
    /// Validate training-job audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora training job audit event",
            self.schema_version,
            SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora training job audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        validate_nonblank_field(
            "sora training job audit event",
            "service_version",
            &self.service_version,
        )?;
        validate_nonblank_field(
            "sora training job audit event",
            "model_name",
            &self.model_name,
        )?;
        validate_nonblank_field("sora training job audit event", "job_id", &self.job_id)?;
        if let Some(latest_metrics_hash) = self.latest_metrics_hash {
            validate_soracloud_digest_hash(
                "sora training job audit event",
                "latest_metrics_hash",
                latest_metrics_hash,
            )?;
        }
        Ok(())
    }
}
/// Authoritative service-level model registry state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelRegistryV1 {
    /// Schema version; must equal [`SORA_MODEL_REGISTRY_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns the model registry.
    pub service_name: Name,
    /// Active service revision when the model registry was last updated.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Current promoted version, when any.
    #[norito(required)]
    pub current_version: Option<String>,
    /// Audit sequence that last updated the registry.
    pub updated_sequence: u64,
}
impl SoraModelRegistryV1 {
    /// Validate model-registry metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, the
    /// identifiers are empty, or sequencing is invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model registry",
            self.schema_version,
            SORA_MODEL_REGISTRY_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora model registry",
            "service_version",
            &self.service_version,
        )?;
        validate_nonblank_field("sora model registry", "model_name", &self.model_name)?;
        if self
            .current_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model registry",
                "current_version",
                "must not be empty when provided",
            ));
        }
        if self.updated_sequence == 0 {
            return Err(invalid_field(
                "sora model registry",
                "updated_sequence",
                "must be greater than zero",
            ));
        }
        Ok(())
    }
}
/// Audit action recorded for model-weight lifecycle changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraModelWeightActionV1 {
    /// A new weight version was registered.
    Register,
    /// An admitted weight version became the promoted current version.
    Promote,
    /// The model registry rolled back to a prior weight version.
    Rollback,
}
/// Provenance source for model artifacts and weight versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraModelProvenanceKindV1 {
    /// The model was produced by a Soracloud training job.
    TrainingJob,
    /// The model was imported from Hugging Face.
    HfImport,
    /// The model was uploaded through the private Soracloud model-vault path.
    UserUpload,
}
/// Reference to the origin of a model artifact or weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelProvenanceRefV1 {
    /// Origin kind.
    pub kind: SoraModelProvenanceKindV1,
    /// Stable origin identifier.
    pub id: String,
}
impl SoraModelProvenanceRefV1 {
    /// Validate model provenance references.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the referenced identifier is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field("sora model provenance ref", "id", &self.id)?;
        Ok(())
    }
}
/// Package format admitted for SoraFS-backed uploaded-model registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "runtime_format", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraUploadedModelRuntimeFormatV1 {
    /// Hugging Face-style safetensors repository layout.
    #[default]
    HuggingFaceSafetensors,
    /// Deterministic quantized CPU operator-set v1.
    DeterministicQuantizedCpuV1,
}
/// Policy pricing for uploaded-model storage.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraUploadedModelPricingPolicyV1 {
    /// Nominal XOR quantity charged for storing encrypted uploaded-model bytes.
    pub storage_price: Quantity,
}
/// Key-encapsulation suite used to wrap uploaded-model bundle keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kem", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraUploadedModelKeyEncapsulationV1 {
    /// X25519 shared-secret derivation with HKDF-SHA256 expansion.
    #[default]
    X25519HkdfSha256,
}
/// AEAD suite used to wrap uploaded-model bundle keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "aead", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraUploadedModelKeyWrapAeadV1 {
    /// AES-256-GCM symmetric key wrapping.
    #[default]
    Aes256Gcm,
}
fn validate_uploaded_model_x25519_public_key(
    manifest: &'static str,
    field: &'static str,
    public_key: &[u8],
) -> Result<(), SoracloudManifestError> {
    if public_key.len() != SORA_UPLOADED_MODEL_X25519_PUBLIC_KEY_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!(
                "X25519-HKDF-SHA256 public key must be {} bytes, found {}",
                SORA_UPLOADED_MODEL_X25519_PUBLIC_KEY_BYTES,
                public_key.len()
            ),
        });
    }
    X25519Sha256::decode_public_key(public_key).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!("invalid X25519-HKDF-SHA256 public key: {err}"),
        }
    })?;
    Ok(())
}
/// Soracloud-upload recipient metadata advertised for model bundle encryption.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraUploadedModelEncryptionRecipientV1 {
    /// Schema version; must equal [`SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1`].
    pub schema_version: u16,
    /// Stable recipient key identifier.
    pub key_id: String,
    /// Recipient key version under the same `key_id`.
    pub key_version: NonZeroU32,
    /// Key-encapsulation suite expected by the recipient.
    pub kem: SoraUploadedModelKeyEncapsulationV1,
    /// AEAD suite expected for the wrapped bundle key.
    pub aead: SoraUploadedModelKeyWrapAeadV1,
    /// Raw recipient public key bytes for the configured KEM.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub public_key_bytes: Vec<u8>,
    /// Commitment over the recipient public key bytes.
    pub public_key_fingerprint: Hash,
}
impl SoraUploadedModelEncryptionRecipientV1 {
    const MAX_PUBLIC_KEY_BYTES: usize = 256;
    /// Validate advertised upload-recipient metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the recipient metadata is empty or malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora uploaded model encryption recipient",
            self.schema_version,
            SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora uploaded model encryption recipient",
            "key_id",
            &self.key_id,
        )?;
        if self.public_key_bytes.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model encryption recipient",
                field: "public_key_bytes",
            });
        }
        if self.public_key_bytes.len() > Self::MAX_PUBLIC_KEY_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model encryption recipient",
                field: "public_key_bytes",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.public_key_bytes.len(),
                    Self::MAX_PUBLIC_KEY_BYTES
                ),
            });
        }
        match self.kem {
            SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256 => {
                validate_uploaded_model_x25519_public_key(
                    "sora uploaded model encryption recipient",
                    "public_key_bytes",
                    self.public_key_bytes.as_slice(),
                )?;
            }
        }
        validate_soracloud_digest_hash(
            "sora uploaded model encryption recipient",
            "public_key_fingerprint",
            self.public_key_fingerprint,
        )?;
        if Hash::new(self.public_key_bytes.as_slice()) != self.public_key_fingerprint {
            return Err(invalid_field(
                "sora uploaded model encryption recipient",
                "public_key_fingerprint",
                "must match the advertised public_key_bytes",
            ));
        }
        Ok(())
    }
}
/// Wrapped symmetric key used to decrypt one uploaded-model bundle on Soracloud.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraUploadedModelWrappedKeyV1 {
    /// Schema version; must equal [`SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1`].
    pub schema_version: u16,
    /// Recipient key identifier used to unwrap the bundle key.
    pub recipient_key_id: String,
    /// Recipient key version used to unwrap the bundle key.
    pub recipient_key_version: NonZeroU32,
    /// Key-encapsulation suite used to derive the wrapping key.
    pub kem: SoraUploadedModelKeyEncapsulationV1,
    /// AEAD suite used to encrypt the wrapped bundle key.
    pub aead: SoraUploadedModelKeyWrapAeadV1,
    /// Raw ephemeral public key bytes used for the KEM exchange.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ephemeral_public_key: Vec<u8>,
    /// Nonce used by the AEAD wrapping operation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub nonce: Vec<u8>,
    /// Opaque wrapped bundle-key ciphertext bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub wrapped_key_ciphertext: Vec<u8>,
    /// Commitment over `wrapped_key_ciphertext`.
    pub ciphertext_hash: Hash,
    /// Digest over the public AAD bound to the key-wrap operation.
    pub aad_digest: Hash,
}
impl SoraUploadedModelWrappedKeyV1 {
    const MAX_PUBLIC_KEY_BYTES: usize = 256;
    const MAX_NONCE_BYTES: usize = 256;
    const MAX_WRAPPED_KEY_BYTES: usize = 4_096;
    /// Validate wrapped bundle-key metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the wrapped-key envelope is empty or malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora uploaded model wrapped key",
            self.schema_version,
            SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
        )?;
        validate_nonblank_field(
            "sora uploaded model wrapped key",
            "recipient_key_id",
            &self.recipient_key_id,
        )?;
        if self.ephemeral_public_key.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model wrapped key",
                field: "ephemeral_public_key",
            });
        }
        if self.ephemeral_public_key.len() > Self::MAX_PUBLIC_KEY_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model wrapped key",
                field: "ephemeral_public_key",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.ephemeral_public_key.len(),
                    Self::MAX_PUBLIC_KEY_BYTES
                ),
            });
        }
        match self.kem {
            SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256 => {
                validate_uploaded_model_x25519_public_key(
                    "sora uploaded model wrapped key",
                    "ephemeral_public_key",
                    self.ephemeral_public_key.as_slice(),
                )?;
            }
        }
        if self.nonce.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model wrapped key",
                field: "nonce",
            });
        }
        if self.nonce.len() > Self::MAX_NONCE_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model wrapped key",
                field: "nonce",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.nonce.len(),
                    Self::MAX_NONCE_BYTES
                ),
            });
        }
        if self.wrapped_key_ciphertext.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model wrapped key",
                field: "wrapped_key_ciphertext",
            });
        }
        if self.wrapped_key_ciphertext.len() > Self::MAX_WRAPPED_KEY_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model wrapped key",
                field: "wrapped_key_ciphertext",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.wrapped_key_ciphertext.len(),
                    Self::MAX_WRAPPED_KEY_BYTES
                ),
            });
        }
        validate_soracloud_digest_hash(
            "sora uploaded model wrapped key",
            "ciphertext_hash",
            self.ciphertext_hash,
        )?;
        validate_soracloud_digest_hash(
            "sora uploaded model wrapped key",
            "aad_digest",
            self.aad_digest,
        )?;
        if Hash::new(self.wrapped_key_ciphertext.as_slice()) != self.ciphertext_hash {
            return Err(invalid_field(
                "sora uploaded model wrapped key",
                "ciphertext_hash",
                "must match the wrapped_key_ciphertext bytes",
            ));
        }
        Ok(())
    }
}
/// Bundle storage reference and metadata for a user-uploaded Soracloud model.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraUploadedModelBundleV1 {
    /// Schema version; must equal [`SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Pinned weight version label.
    pub weight_version: String,
    /// Admitted model family.
    pub family: String,
    /// Admitted modalities.
    pub modalities: Vec<String>,
    /// Deterministic commitment over the normalized plaintext upload bundle.
    pub plaintext_root: Hash,
    /// Uploaded-model package format.
    pub runtime_format: SoraUploadedModelRuntimeFormatV1,
    /// Canonical bundle root.
    pub bundle_root: Hash,
    /// Approved active `SoraFS` manifest digest containing the encrypted model bundle.
    pub sorafs_manifest_digest: ManifestDigest,
    /// Total chunk count in deterministic ordinal order.
    pub chunk_count: u32,
    /// Total plaintext bytes before encryption.
    pub plaintext_bytes: u64,
    /// Total ciphertext bytes stored in the referenced `SoraFS` bundle.
    pub ciphertext_bytes: u64,
    /// Merkle root over the chunk manifest.
    pub chunk_manifest_root: Hash,
    /// Soracloud upload-recipient metadata used to wrap the bundle key.
    pub upload_recipient: SoraUploadedModelEncryptionRecipientV1,
    /// Wrapped symmetric key used to decrypt the uploaded chunk set.
    pub wrapped_bundle_key: SoraUploadedModelWrappedKeyV1,
    /// Pricing policy snapshot.
    pub pricing_policy: SoraUploadedModelPricingPolicyV1,
    /// Reference to the decryption release policy.
    pub decryption_policy_ref: String,
}
fn validate_uploaded_model_wrapped_key_matches_recipient(
    recipient: &SoraUploadedModelEncryptionRecipientV1,
    wrapped_key: &SoraUploadedModelWrappedKeyV1,
) -> Result<(), SoracloudManifestError> {
    for (field, matches, reason) in [
        (
            "wrapped_bundle_key.recipient_key_id",
            recipient.key_id == wrapped_key.recipient_key_id,
            "must match upload_recipient.key_id",
        ),
        (
            "wrapped_bundle_key.recipient_key_version",
            recipient.key_version == wrapped_key.recipient_key_version,
            "must match upload_recipient.key_version",
        ),
        (
            "wrapped_bundle_key.kem",
            recipient.kem == wrapped_key.kem,
            "must match upload_recipient.kem",
        ),
        (
            "wrapped_bundle_key.aead",
            recipient.aead == wrapped_key.aead,
            "must match upload_recipient.aead",
        ),
    ] {
        if !matches {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model bundle",
                field,
                reason: reason.to_string(),
            });
        }
    }
    Ok(())
}
impl SoraUploadedModelBundleV1 {
    /// Validate uploaded-model bundle metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when required identifiers are empty or sizes are invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora uploaded model bundle",
            self.schema_version,
            SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        )?;
        for (field, value) in [
            ("model_id", self.model_id.as_str()),
            ("weight_version", self.weight_version.as_str()),
            ("family", self.family.as_str()),
            ("decryption_policy_ref", self.decryption_policy_ref.as_str()),
        ] {
            validate_nonblank_field("sora uploaded model bundle", field, value)?;
        }
        if self.modalities.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model bundle",
                field: "modalities",
            });
        }
        let mut seen_modalities = BTreeSet::new();
        for modality in &self.modalities {
            let normalized = modality.trim();
            validate_nonblank_field("sora uploaded model bundle", "modalities", normalized)?;
            if normalized != modality {
                return Err(invalid_field(
                    "sora uploaded model bundle",
                    "modalities",
                    "entries must be canonical without surrounding whitespace",
                ));
            }
            if modality.chars().any(char::is_control) {
                return Err(invalid_field(
                    "sora uploaded model bundle",
                    "modalities",
                    "entries must not contain control characters",
                ));
            }
            if !seen_modalities.insert(modality.as_str()) {
                return Err(invalid_field(
                    "sora uploaded model bundle",
                    "modalities",
                    "entries must be unique",
                ));
            }
        }
        self.upload_recipient.validate()?;
        self.wrapped_bundle_key.validate()?;
        validate_uploaded_model_wrapped_key_matches_recipient(
            &self.upload_recipient,
            &self.wrapped_bundle_key,
        )?;
        for (field, hash) in [
            ("plaintext_root", self.plaintext_root),
            ("bundle_root", self.bundle_root),
            ("chunk_manifest_root", self.chunk_manifest_root),
        ] {
            validate_soracloud_digest_hash("sora uploaded model bundle", field, hash)?;
        }
        if self.chunk_count == 0 || self.plaintext_bytes == 0 || self.ciphertext_bytes == 0 {
            return Err(invalid_field(
                "sora uploaded model bundle",
                "chunk_count",
                "chunk_count, plaintext_bytes, and ciphertext_bytes must be greater than zero",
            ));
        }
        Ok(())
    }
}
/// SoraFS-backed encrypted artifact reference for private uploaded-model execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateModelArtifactRefV1 {
    /// Schema version; must equal [`SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1`].
    pub schema_version: u16,
    /// Approved active `SoraFS` manifest digest containing the encrypted artifact.
    pub sorafs_manifest_digest: ManifestDigest,
    /// Canonical content-DAG root committed by the exact `SoraFS` manifest.
    ///
    /// Carrying the root directly prevents a receipt from combining a valid manifest digest
    /// with an unrelated content identity. Ledger admission checks this value against the pin
    /// registry record produced from the canonical manifest payload.
    pub sorafs_root_cid: ManifestRootCid,
    /// Commitment over the encrypted artifact bytes.
    pub artifact_hash: Hash,
    /// Total encrypted bytes stored by `SoraFS` for the artifact.
    pub ciphertext_bytes: u64,
    /// Stable artifact role, for example `input` or `output`.
    pub artifact_role: String,
}
impl SoraPrivateModelArtifactRefV1 {
    /// Validate encrypted artifact metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when artifact metadata is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private model artifact ref",
            self.schema_version,
            SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
        )?;
        validate_soracloud_digest_hash(
            "sora private model artifact ref",
            "artifact_hash",
            self.artifact_hash,
        )?;
        if self.ciphertext_bytes == 0 {
            return Err(invalid_field(
                "sora private model artifact ref",
                "ciphertext_bytes",
                "must be greater than zero",
            ));
        }
        let role = self.artifact_role.trim();
        validate_nonblank_field("sora private model artifact ref", "artifact_role", role)?;
        if role != self.artifact_role || role.chars().any(char::is_control) {
            return Err(invalid_field(
                "sora private model artifact ref",
                "artifact_role",
                "must be canonical and free of control characters",
            ));
        }
        Ok(())
    }
}

/// Public context bound to one encrypted deterministic model payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateModelArtifactModelContextV1 {
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Exact service revision that admitted this model release.
    pub service_version: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Pinned model weight version.
    pub weight_version: String,
    /// Decryption policy governing the uploaded model.
    pub policy_id: String,
    /// Commitment over the canonical plaintext model payload.
    pub model_plaintext_commitment: Hash,
}

/// Public context bound to one encrypted authorized input payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateModelArtifactInputContextV1 {
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Exact service revision authorized by the decryption record.
    pub service_version: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Pinned model weight version.
    pub weight_version: String,
    /// Decryption policy governing this execution.
    pub policy_id: String,
    /// Exact authoritative decryption request identifier.
    pub decryption_request_id: String,
}

/// Public context bound to one encrypted deterministic output payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateModelArtifactOutputContextV1 {
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Exact service revision authorized by the decryption record.
    pub service_version: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Pinned model weight version.
    pub weight_version: String,
    /// Decryption policy governing this execution.
    pub policy_id: String,
    /// Exact authoritative decryption request identifier.
    pub decryption_request_id: String,
    /// Runtime-blinded commitment over the canonical plaintext input payload.
    pub input_blinded_commitment: Hash,
    /// Runtime-blinded commitment over the canonical plaintext output payload.
    pub output_blinded_commitment: Hash,
    /// Fingerprint of the public key that can unwrap the encrypted output.
    pub output_recipient_key_fingerprint: Hash,
}

/// Exact public context bound to one private-model encrypted artifact.
///
/// Model contexts are stable at upload time. Input and output contexts additionally bind the
/// exact service revision and authorized decryption request. Output contexts carry commitments
/// blinded by runtime custody material so low-entropy inference values cannot be dictionary-tested
/// from public artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "artifact_role", content = "context"))]
#[norito(deny_unknown_fields)]
pub enum SoraPrivateModelArtifactContextV1 {
    /// Encrypted deterministic quantized model package.
    Model(SoraPrivateModelArtifactModelContextV1),
    /// Encrypted input released for one authorized execution.
    Input(SoraPrivateModelArtifactInputContextV1),
    /// Encrypted output produced for one authorized execution.
    Output(SoraPrivateModelArtifactOutputContextV1),
}

impl SoraPrivateModelArtifactContextV1 {
    const MAX_IDENTIFIER_BYTES: usize = 256;

    /// Return the canonical artifact role string used by SoraFS references.
    #[must_use]
    pub const fn artifact_role(&self) -> &'static str {
        match self {
            Self::Model(_) => "model",
            Self::Input(_) => "input",
            Self::Output(_) => "output",
        }
    }

    /// Validate role-specific private artifact context invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when an identifier is non-canonical or a commitment is
    /// malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        let (service_version, model_id, weight_version, policy_id) = match self {
            Self::Model(context) => {
                validate_soracloud_digest_hash(
                    "sora private model artifact context",
                    "model_plaintext_commitment",
                    context.model_plaintext_commitment,
                )?;
                (
                    &context.service_version,
                    &context.model_id,
                    &context.weight_version,
                    &context.policy_id,
                )
            }
            Self::Input(context) => {
                validate_private_model_context_identifier(
                    "decryption_request_id",
                    &context.decryption_request_id,
                    Self::MAX_IDENTIFIER_BYTES,
                )?;
                (
                    &context.service_version,
                    &context.model_id,
                    &context.weight_version,
                    &context.policy_id,
                )
            }
            Self::Output(context) => {
                validate_private_model_context_identifier(
                    "decryption_request_id",
                    &context.decryption_request_id,
                    Self::MAX_IDENTIFIER_BYTES,
                )?;
                for (field, digest) in [
                    ("input_blinded_commitment", context.input_blinded_commitment),
                    (
                        "output_blinded_commitment",
                        context.output_blinded_commitment,
                    ),
                    (
                        "output_recipient_key_fingerprint",
                        context.output_recipient_key_fingerprint,
                    ),
                ] {
                    validate_soracloud_digest_hash(
                        "sora private model artifact context",
                        field,
                        digest,
                    )?;
                }
                (
                    &context.service_version,
                    &context.model_id,
                    &context.weight_version,
                    &context.policy_id,
                )
            }
        };
        for (field, value) in [
            ("service_version", service_version),
            ("model_id", model_id),
            ("weight_version", weight_version),
            ("policy_id", policy_id),
        ] {
            validate_private_model_context_identifier(field, value, Self::MAX_IDENTIFIER_BYTES)?;
        }
        Ok(())
    }
}

fn validate_private_model_context_identifier(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field("sora private model artifact context", field, value)?;
    if value.trim() != value || value.chars().any(char::is_control) {
        return Err(invalid_field(
            "sora private model artifact context",
            field,
            "must be canonical and free of control characters",
        ));
    }
    if value.len() > max_bytes {
        return Err(invalid_field(
            "sora private model artifact context",
            field,
            format!("length {} exceeds max {max_bytes} bytes", value.len()),
        ));
    }
    Ok(())
}

/// Derive the domain-separated commitment for an exact private artifact context.
#[must_use]
pub fn derive_soracloud_private_model_artifact_context_commitment_v1(
    context: &SoraPrivateModelArtifactContextV1,
) -> Hash {
    let mut transcript = b"soracloud:private-model-artifact-context:v1\0".to_vec();
    transcript.extend(context.encode());
    Hash::new(transcript)
}

/// Encode the exact public AAD for X25519/HKDF/AES-256-GCM content-key wrapping.
#[must_use]
pub fn encode_soracloud_private_model_key_wrap_aad_v1(
    context_commitment: Hash,
    recipient: &SoraUploadedModelEncryptionRecipientV1,
    ephemeral_public_key: &[u8],
    nonce: &[u8],
) -> Vec<u8> {
    let mut transcript = b"soracloud:private-model-key-wrap-aad:v1\0".to_vec();
    transcript.extend(context_commitment.encode());
    transcript.extend(recipient.encode());
    transcript.extend(ephemeral_public_key.to_vec().encode());
    transcript.extend(nonce.to_vec().encode());
    transcript
}

/// Encode the exact public AAD for one private artifact payload.
#[must_use]
pub fn encode_soracloud_private_model_payload_aad_v1(
    context_commitment: Hash,
    wrapped_key: &SoraUploadedModelWrappedKeyV1,
    payload_nonce: &[u8],
) -> Vec<u8> {
    let mut transcript = b"soracloud:private-model-payload-aad:v1\0".to_vec();
    transcript.extend(SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_VERSION_V1.encode());
    transcript.extend(context_commitment.encode());
    transcript.extend(wrapped_key.encode());
    transcript.extend(payload_nonce.to_vec().encode());
    transcript
}

/// Canonical encrypted private-model artifact stored as exact SoraFS content.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateModelEncryptedArtifactV1 {
    /// Schema version; must equal [`SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_VERSION_V1`].
    pub schema_version: u16,
    /// Role-specific public context bound by both AEAD layers.
    pub context: SoraPrivateModelArtifactContextV1,
    /// Commitment over `context` using the canonical context domain.
    pub context_commitment: Hash,
    /// Content key encrypted to the intended X25519 recipient.
    pub wrapped_key: SoraUploadedModelWrappedKeyV1,
    /// AES-256-GCM nonce used for payload encryption.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub payload_nonce: Vec<u8>,
    /// Canonical encrypted payload bytes with the appended GCM authentication tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub payload_ciphertext: Vec<u8>,
    /// Commitment over `payload_ciphertext`.
    pub payload_ciphertext_hash: Hash,
    /// Digest over the exact public payload AAD.
    pub payload_aad_digest: Hash,
}

impl SoraPrivateModelEncryptedArtifactV1 {
    /// Validate the canonical envelope and all locally derivable commitments.
    ///
    /// Recipient-specific key-wrap AAD is validated by the decrypting runtime because the
    /// recipient public key is deliberately not duplicated inside the envelope.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the envelope is malformed, oversized, or carries
    /// a non-canonical context, nonce, ciphertext hash, or payload AAD digest.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private model encrypted artifact",
            self.schema_version,
            SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_VERSION_V1,
        )?;
        self.context.validate()?;
        let expected_context =
            derive_soracloud_private_model_artifact_context_commitment_v1(&self.context);
        if self.context_commitment != expected_context {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "context_commitment",
                "must bind the exact canonical artifact context",
            ));
        }
        self.wrapped_key.validate()?;
        if self.wrapped_key.nonce.len() != SORA_PRIVATE_MODEL_AEAD_NONCE_BYTES_V1 {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "wrapped_key.nonce",
                format!(
                    "AES-256-GCM nonce must be exactly {} bytes",
                    SORA_PRIVATE_MODEL_AEAD_NONCE_BYTES_V1
                ),
            ));
        }
        if self.wrapped_key.wrapped_key_ciphertext.len()
            != SORA_PRIVATE_MODEL_AEAD_KEY_BYTES_V1 + SORA_PRIVATE_MODEL_AEAD_TAG_BYTES_V1
        {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "wrapped_key.wrapped_key_ciphertext",
                "must contain one 32-byte content key and one 16-byte AES-GCM tag",
            ));
        }
        if self.payload_nonce.len() != SORA_PRIVATE_MODEL_AEAD_NONCE_BYTES_V1 {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "payload_nonce",
                format!(
                    "AES-256-GCM nonce must be exactly {} bytes",
                    SORA_PRIVATE_MODEL_AEAD_NONCE_BYTES_V1
                ),
            ));
        }
        if self.payload_ciphertext.len() <= SORA_PRIVATE_MODEL_AEAD_TAG_BYTES_V1 {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "payload_ciphertext",
                "must contain non-empty ciphertext and one AES-GCM tag",
            ));
        }
        if self.payload_ciphertext.len() > SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1 {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "payload_ciphertext",
                format!(
                    "length {} exceeds max {} bytes",
                    self.payload_ciphertext.len(),
                    SORA_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1
                ),
            ));
        }
        if Hash::new(self.payload_ciphertext.as_slice()) != self.payload_ciphertext_hash {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "payload_ciphertext_hash",
                "must match the exact encrypted payload bytes",
            ));
        }
        let expected_payload_aad = Hash::new(encode_soracloud_private_model_payload_aad_v1(
            self.context_commitment,
            &self.wrapped_key,
            &self.payload_nonce,
        ));
        if self.payload_aad_digest != expected_payload_aad {
            return Err(invalid_field(
                "sora private model encrypted artifact",
                "payload_aad_digest",
                "must bind the exact canonical payload AAD",
            ));
        }
        Ok(())
    }
}

/// Explicit rounding mode for deterministic private quantized CPU models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "rounding", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraPrivateQuantizedRoundingV1 {
    /// Round to the nearest integer, with ties away from zero.
    #[default]
    NearestAwayFromZero,
}

/// Canonical bounded plaintext model payload decrypted only inside the runtime boundary.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateQuantizedCpuModelV1 {
    /// Schema version; must equal [`SORA_PRIVATE_QUANTIZED_CPU_MODEL_VERSION_V1`].
    pub schema_version: u16,
    /// Number of signed 32-bit integer inputs.
    pub input_len: u32,
    /// Number of signed 32-bit integer outputs.
    pub output_len: u32,
    /// Row-major signed 8-bit weights, `output_len * input_len` entries.
    pub weights_i8: Vec<i8>,
    /// Signed 32-bit output biases.
    pub bias_i32: Vec<i32>,
    /// Non-negative right shift applied after accumulation.
    pub output_shift: u8,
    /// Saturating lower bound for every output.
    pub output_min: i32,
    /// Saturating upper bound for every output.
    pub output_max: i32,
    /// Explicit deterministic rounding mode.
    pub rounding: SoraPrivateQuantizedRoundingV1,
}

impl SoraPrivateQuantizedCpuModelV1 {
    /// Validate model dimensions, encoded vector bounds, and arithmetic parameters.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the model is malformed or exceeds runtime bounds.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private quantized CPU model",
            self.schema_version,
            SORA_PRIVATE_QUANTIZED_CPU_MODEL_VERSION_V1,
        )?;
        let input_len = usize::try_from(self.input_len).expect("u32 fits usize");
        let output_len = usize::try_from(self.output_len).expect("u32 fits usize");
        if input_len == 0 || input_len > SORA_PRIVATE_QUANTIZED_CPU_MAX_INPUTS_V1 {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "input_len",
                format!(
                    "must be in 1..={}",
                    SORA_PRIVATE_QUANTIZED_CPU_MAX_INPUTS_V1
                ),
            ));
        }
        if output_len == 0 || output_len > SORA_PRIVATE_QUANTIZED_CPU_MAX_OUTPUTS_V1 {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "output_len",
                format!(
                    "must be in 1..={}",
                    SORA_PRIVATE_QUANTIZED_CPU_MAX_OUTPUTS_V1
                ),
            ));
        }
        let expected_weights = input_len.checked_mul(output_len).ok_or_else(|| {
            invalid_field(
                "sora private quantized CPU model",
                "weights_i8",
                "model dimensions overflow the platform size",
            )
        })?;
        if expected_weights > SORA_PRIVATE_QUANTIZED_CPU_MAX_WEIGHTS_V1
            || self.weights_i8.len() != expected_weights
        {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "weights_i8",
                format!(
                    "must contain exactly {expected_weights} entries and not exceed {}",
                    SORA_PRIVATE_QUANTIZED_CPU_MAX_WEIGHTS_V1
                ),
            ));
        }
        if self.bias_i32.len() != output_len {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "bias_i32",
                "length must equal output_len",
            ));
        }
        if self.output_shift > 30 {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "output_shift",
                "must be <= 30",
            ));
        }
        if self.output_min > self.output_max {
            return Err(invalid_field(
                "sora private quantized CPU model",
                "output_min",
                "must be <= output_max",
            ));
        }
        Ok(())
    }
}

/// Canonical bounded plaintext input payload decrypted only inside the runtime boundary.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateQuantizedCpuInputV1 {
    /// Schema version; must equal [`SORA_PRIVATE_QUANTIZED_CPU_INPUT_VERSION_V1`].
    pub schema_version: u16,
    /// Signed 32-bit model inputs.
    pub values_i32: Vec<i32>,
}

impl SoraPrivateQuantizedCpuInputV1 {
    /// Validate the canonical input vector bound.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the input is empty or oversized.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private quantized CPU input",
            self.schema_version,
            SORA_PRIVATE_QUANTIZED_CPU_INPUT_VERSION_V1,
        )?;
        if self.values_i32.is_empty()
            || self.values_i32.len() > SORA_PRIVATE_QUANTIZED_CPU_MAX_INPUTS_V1
        {
            return Err(invalid_field(
                "sora private quantized CPU input",
                "values_i32",
                format!(
                    "length must be in 1..={}",
                    SORA_PRIVATE_QUANTIZED_CPU_MAX_INPUTS_V1
                ),
            ));
        }
        Ok(())
    }
}

/// Canonical bounded plaintext output payload encrypted inside the runtime boundary.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateQuantizedCpuOutputV1 {
    /// Schema version; must equal [`SORA_PRIVATE_QUANTIZED_CPU_OUTPUT_VERSION_V1`].
    pub schema_version: u16,
    /// Signed 32-bit deterministic model outputs.
    pub values_i32: Vec<i32>,
}

impl SoraPrivateQuantizedCpuOutputV1 {
    /// Validate the canonical output vector bound.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the output is empty or oversized.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private quantized CPU output",
            self.schema_version,
            SORA_PRIVATE_QUANTIZED_CPU_OUTPUT_VERSION_V1,
        )?;
        if self.values_i32.is_empty()
            || self.values_i32.len() > SORA_PRIVATE_QUANTIZED_CPU_MAX_OUTPUTS_V1
        {
            return Err(invalid_field(
                "sora private quantized CPU output",
                "values_i32",
                format!(
                    "length must be in 1..={}",
                    SORA_PRIVATE_QUANTIZED_CPU_MAX_OUTPUTS_V1
                ),
            ));
        }
        Ok(())
    }
}

/// Runtime version string for deterministic private uploaded-model execution v1.
pub const SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1: &str = "soracloud.quantized-cpu.v1";

/// Receipt committed for deterministic private uploaded-model execution.
///
/// The receipt intentionally carries only commitments and encrypted artifact
/// references. Plaintext input and output bytes remain outside chain state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraPrivateUploadedModelExecutionReceiptV1 {
    /// Schema version; must equal [`SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1`].
    pub schema_version: u16,
    /// Exact genesis-derived network identity that prevents cross-deployment replay.
    pub network_id: NetworkId,
    /// Deterministic receipt identifier.
    pub receipt_id: Hash,
    /// Service that owns the uploaded model.
    pub service_name: Name,
    /// Exact service revision authorized for this execution.
    pub service_version: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Pinned weight version label.
    pub weight_version: String,
    /// Deterministic runtime version that defines operator semantics.
    pub runtime_version: String,
    /// Approved `SoraFS` manifest digest for the encrypted model package.
    pub model_manifest_digest: ManifestDigest,
    /// Canonical uploaded-model bundle root.
    pub model_bundle_root: Hash,
    /// Decryption or release policy identifier used by the execution.
    pub policy_id: String,
    /// Exact committed authorization record that released the encrypted input.
    pub decryption_request_id: String,
    /// Active validator attesting the receipt transaction.
    ///
    /// This is deliberately not remote-executor attribution. A producer must derive this
    /// identity from its own local validator configuration and the ledger verifies it against
    /// the transaction authority.
    pub attesting_validator: SoraRuntimeDeterministicValidatorHostV1,
    /// Encrypted input artifact persisted outside chain state.
    pub input_artifact: SoraPrivateModelArtifactRefV1,
    /// Encrypted output artifact persisted outside chain state.
    pub output_artifact: SoraPrivateModelArtifactRefV1,
    /// Runtime-blinded commitment over the canonical plaintext input envelope.
    pub input_commitment: Hash,
    /// Runtime-blinded commitment over the canonical plaintext output envelope.
    pub output_commitment: Hash,
    /// Exact public recipient metadata to which the encrypted output was wrapped.
    pub output_recipient: SoraUploadedModelEncryptionRecipientV1,
    /// Commitment over the runtime request envelope.
    pub request_commitment: Hash,
    /// Commitment over the runtime result envelope.
    pub result_commitment: Hash,
    /// Ledger-assigned Soracloud sequence that persisted the receipt.
    ///
    /// A `RecordSoracloudPrivateUploadedModelExecutionReceipt` submission must set this to zero;
    /// ledger execution replaces the sentinel with the next authoritative Soracloud sequence.
    pub emitted_sequence: u64,
    /// Ledger-assigned block height at which the private execution receipt was persisted.
    ///
    /// A submission must set this to zero. Ledger execution records the exact block height so
    /// snapshot restore can prove that the decryption authorization was still active.
    pub emitted_block_height: u64,
}

fn append_private_uploaded_model_receipt_transcript_part<T: Encode>(
    transcript: &mut Vec<u8>,
    value: &T,
) {
    transcript.extend(value.encode());
}

/// Derive the canonical V1 request commitment for a private uploaded-model receipt.
///
/// Every field needed to resolve the exact encrypted model, input envelope, and requested output
/// destination is bound. The ledger-assigned sequence and result-side plaintext commitment are
/// deliberately excluded.
#[must_use]
pub fn derive_soracloud_private_model_request_commitment_v1(
    receipt: &SoraPrivateUploadedModelExecutionReceiptV1,
) -> Hash {
    let mut transcript = Vec::new();
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &"soracloud:private-model-request:v1".to_owned(),
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.schema_version);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.network_id);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.service_name);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.service_version,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.model_id);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.weight_version);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.runtime_version,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.model_manifest_digest,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.model_bundle_root,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.policy_id);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.decryption_request_id,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.attesting_validator,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.input_artifact);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_artifact,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.input_commitment,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_recipient,
    );
    Hash::new(transcript)
}

/// Derive the canonical V1 result commitment for a private uploaded-model receipt.
#[must_use]
pub fn derive_soracloud_private_model_result_commitment_v1(
    receipt: &SoraPrivateUploadedModelExecutionReceiptV1,
) -> Hash {
    let mut transcript = Vec::new();
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &"soracloud:private-model-result:v1".to_owned(),
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.schema_version);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.network_id);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.runtime_version,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.request_commitment,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_artifact,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_commitment,
    );
    Hash::new(transcript)
}

/// Derive the canonical sequence-independent V1 private uploaded-model receipt identifier.
///
/// The identifier binds every immutable receipt field while excluding only the identifier itself
/// and the ledger-assigned emission sequence.
#[must_use]
pub fn derive_soracloud_private_uploaded_model_execution_receipt_id_v1(
    receipt: &SoraPrivateUploadedModelExecutionReceiptV1,
) -> Hash {
    let mut transcript = Vec::new();
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &"soracloud:private-model-execution-receipt:v1".to_owned(),
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.schema_version);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.network_id);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.service_name);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.service_version,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.model_id);
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.weight_version);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.runtime_version,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.model_manifest_digest,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.model_bundle_root,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.policy_id);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.decryption_request_id,
    );
    append_private_uploaded_model_receipt_transcript_part(&mut transcript, &receipt.input_artifact);
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_artifact,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.input_commitment,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_commitment,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.output_recipient,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.request_commitment,
    );
    append_private_uploaded_model_receipt_transcript_part(
        &mut transcript,
        &receipt.result_commitment,
    );
    Hash::new(transcript)
}

impl SoraPrivateUploadedModelExecutionReceiptV1 {
    /// Validate ledger-persisted private uploaded-model execution receipt metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the receipt is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(true)
    }
    /// Validate private uploaded-model execution receipt metadata prepared for ledger submission.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the receipt is malformed or carries a
    /// caller-selected sequence.
    pub fn validate_submission(&self) -> Result<(), SoracloudManifestError> {
        self.validate_with_sequence_state(false)
    }
    fn validate_with_sequence_state(
        &self,
        require_assigned_sequence: bool,
    ) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora private uploaded model execution receipt",
            self.schema_version,
            SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
        )?;
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_id", self.model_id.as_str()),
            ("weight_version", self.weight_version.as_str()),
            ("runtime_version", self.runtime_version.as_str()),
            ("policy_id", self.policy_id.as_str()),
            ("decryption_request_id", self.decryption_request_id.as_str()),
        ] {
            validate_nonblank_field(
                "sora private uploaded model execution receipt",
                field,
                value,
            )?;
            if value.trim() != value || value.chars().any(char::is_control) {
                return Err(invalid_field(
                    "sora private uploaded model execution receipt",
                    field,
                    "must be canonical and free of control characters",
                ));
            }
        }
        if self.runtime_version != SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1 {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "runtime_version",
                "must equal the canonical deterministic private-model runtime version",
            ));
        }
        for (field, digest) in [
            ("receipt_id", self.receipt_id),
            ("model_bundle_root", self.model_bundle_root),
            ("input_commitment", self.input_commitment),
            ("output_commitment", self.output_commitment),
            ("request_commitment", self.request_commitment),
            ("result_commitment", self.result_commitment),
        ] {
            validate_soracloud_digest_hash(
                "sora private uploaded model execution receipt",
                field,
                digest,
            )?;
        }
        if require_assigned_sequence && self.emitted_sequence == 0 {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "emitted_sequence",
                "must be assigned by the ledger before persistence",
            ));
        }
        if require_assigned_sequence && self.emitted_block_height == 0 {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "emitted_block_height",
                "must be assigned by the ledger before persistence",
            ));
        }
        if !require_assigned_sequence && self.emitted_sequence != 0 {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "emitted_sequence",
                "must be zero before ledger submission",
            ));
        }
        if !require_assigned_sequence && self.emitted_block_height != 0 {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "emitted_block_height",
                "must be zero before ledger submission",
            ));
        }
        self.input_artifact.validate()?;
        self.output_artifact.validate()?;
        self.output_recipient.validate()?;
        SoraRuntimeExecutionHostV1::DeterministicValidator(self.attesting_validator.clone())
            .validate()?;
        if self.input_artifact.artifact_role != "input" {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "input_artifact.artifact_role",
                "must be `input`",
            ));
        }
        if self.output_artifact.artifact_role != "output" {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "output_artifact.artifact_role",
                "must be `output`",
            ));
        }
        if self.input_artifact.artifact_hash == self.output_artifact.artifact_hash {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "output_artifact.artifact_hash",
                "must differ from the encrypted input artifact hash",
            ));
        }
        let expected_request_commitment =
            derive_soracloud_private_model_request_commitment_v1(self);
        if self.request_commitment != expected_request_commitment {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "request_commitment",
                "must bind the exact canonical model and encrypted input envelope",
            ));
        }
        let expected_result_commitment = derive_soracloud_private_model_result_commitment_v1(self);
        if self.result_commitment != expected_result_commitment {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "result_commitment",
                "must bind the exact canonical runtime and encrypted output envelope",
            ));
        }
        let expected_receipt_id =
            derive_soracloud_private_uploaded_model_execution_receipt_id_v1(self);
        if self.receipt_id != expected_receipt_id {
            return Err(invalid_field(
                "sora private uploaded model execution receipt",
                "receipt_id",
                "must equal the canonical sequence-independent receipt identifier",
            ));
        }
        Ok(())
    }
}
/// Immutable metadata for an admitted model-weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelWeightVersionRecordV1 {
    /// Schema version; must equal [`SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns the model.
    pub service_name: Name,
    /// Active service revision when the version was last updated.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Weight version identifier.
    pub weight_version: String,
    /// Optional lineage parent version.
    #[norito(required)]
    pub parent_version: Option<String>,
    /// Training job that produced this weight version.
    pub training_job_id: String,
    /// Generic provenance source for this weight version.
    #[norito(required)]
    pub source_provenance: Option<SoraModelProvenanceRefV1>,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Audit sequence that registered the version.
    pub registered_sequence: u64,
    /// Audit sequence that promoted the version, when promoted.
    #[norito(required)]
    pub promoted_sequence: Option<u64>,
    /// Gate report hash attached to the promotion, when promoted.
    #[norito(required)]
    pub gate_report_hash: Option<Hash>,
    /// Provenance signer that promoted the version, when promoted.
    #[norito(required)]
    pub promoted_by: Option<PublicKey>,
}
impl SoraModelWeightVersionRecordV1 {
    /// Validate model-weight version metadata and sequencing.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch,
    /// identifiers are empty, or promotion metadata is inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model weight version record",
            self.schema_version,
            SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
        )?;
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_name", self.model_name.as_str()),
            ("weight_version", self.weight_version.as_str()),
            ("dataset_ref", self.dataset_ref.as_str()),
        ] {
            validate_nonblank_field("sora model weight version record", field, value)?;
        }
        if self.training_job_id.trim().is_empty() && self.source_provenance.is_none() {
            return Err(invalid_field(
                "sora model weight version record",
                "source_provenance",
                "training_job_id or source_provenance must be populated",
            ));
        }
        if self
            .parent_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model weight version record",
                "parent_version",
                "must not be empty when provided",
            ));
        }
        if let Some(source_provenance) = &self.source_provenance {
            source_provenance.validate()?;
        }
        for (field, digest) in [
            ("weight_artifact_hash", self.weight_artifact_hash),
            ("training_config_hash", self.training_config_hash),
            ("reproducibility_hash", self.reproducibility_hash),
            (
                "provenance_attestation_hash",
                self.provenance_attestation_hash,
            ),
        ] {
            validate_soracloud_digest_hash("sora model weight version record", field, digest)?;
        }
        if let Some(gate_report_hash) = self.gate_report_hash {
            validate_soracloud_digest_hash(
                "sora model weight version record",
                "gate_report_hash",
                gate_report_hash,
            )?;
        }
        if self.registered_sequence == 0 {
            return Err(invalid_field(
                "sora model weight version record",
                "registered_sequence",
                "must be greater than zero",
            ));
        }
        if self.promoted_sequence.is_some() != self.gate_report_hash.is_some()
            || self.promoted_sequence.is_some() != self.promoted_by.is_some()
        {
            return Err(invalid_field(
                "sora model weight version record",
                "promotion_metadata",
                "promoted_sequence, gate_report_hash, and promoted_by must be populated together",
            ));
        }
        if let Some(promoted_sequence) = self.promoted_sequence
            && promoted_sequence < self.registered_sequence
        {
            return Err(invalid_field(
                "sora model weight version record",
                "promoted_sequence",
                "must be >= registered_sequence",
            ));
        }
        Ok(())
    }
}
/// Audit record for model-weight lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelWeightAuditEventV1 {
    /// Schema version; must equal [`SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1`].
    pub schema_version: u16,
    /// Deterministic Soracloud audit sequence.
    pub sequence: u64,
    /// Model-weight action that produced the event.
    pub action: SoraModelWeightActionV1,
    /// Service that owns the model.
    pub service_name: Name,
    /// Active service revision when the event was emitted.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Version targeted by the event.
    pub target_version: String,
    /// Resulting current version after the event.
    #[norito(required)]
    pub current_version: Option<String>,
    /// Optional lineage parent for the targeted version.
    #[norito(required)]
    pub parent_version: Option<String>,
    /// Promotion gate approval flag, when applicable.
    #[norito(required)]
    pub gate_approved: Option<bool>,
    /// Rollback reason, when applicable.
    #[norito(required)]
    pub rollback_reason: Option<String>,
    /// Provenance signer that authorized the event.
    pub signer: PublicKey,
}
impl SoraModelWeightAuditEventV1 {
    /// Validate model-weight audit metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model weight audit event",
            self.schema_version,
            SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
        )?;
        if self.sequence == 0 {
            return Err(invalid_field(
                "sora model weight audit event",
                "sequence",
                "must be greater than zero",
            ));
        }
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_name", self.model_name.as_str()),
            ("target_version", self.target_version.as_str()),
        ] {
            validate_nonblank_field("sora model weight audit event", field, value)?;
        }
        if self
            .rollback_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model weight audit event",
                "rollback_reason",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
/// Audit action recorded for model-artifact lifecycle changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoraModelArtifactActionV1 {
    /// A completed training job registered an artifact description.
    Register,
}
/// Authoritative record for model artifacts derived from completed training jobs.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoraModelArtifactRecordV1 {
    /// Schema version; must equal [`SORA_MODEL_ARTIFACT_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Active service revision when the artifact was last updated.
    pub service_version: String,
    /// Logical model name.
    pub model_name: String,
    /// Stable artifact identifier under the owning service.
    pub artifact_id: String,
    /// Training job that produced this artifact.
    pub training_job_id: String,
    /// Weight version represented by this artifact, when already pinned.
    #[norito(required)]
    pub weight_version: Option<String>,
    /// Generic provenance source for this artifact.
    #[norito(required)]
    pub source_provenance: Option<SoraModelProvenanceRefV1>,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Audit sequence that registered the artifact.
    pub registered_sequence: u64,
    /// Model weight version that consumed this artifact, when any.
    #[norito(required)]
    pub consumed_by_version: Option<String>,
    /// Referenced uploaded-model chunk-manifest root, when this artifact comes from a user upload.
    #[norito(required)]
    pub chunk_manifest_root: Option<Hash>,
}
impl SoraModelArtifactRecordV1 {
    /// Validate model-artifact metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// required identifiers are empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "sora model artifact record",
            self.schema_version,
            SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
        )?;
        for (field, value) in [
            ("service_version", self.service_version.as_str()),
            ("model_name", self.model_name.as_str()),
            ("artifact_id", self.artifact_id.as_str()),
            ("dataset_ref", self.dataset_ref.as_str()),
        ] {
            validate_nonblank_field("sora model artifact record", field, value)?;
        }
        if self.training_job_id.trim().is_empty() && self.source_provenance.is_none() {
            return Err(invalid_field(
                "sora model artifact record",
                "source_provenance",
                "training_job_id or source_provenance must be populated",
            ));
        }
        if self
            .weight_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model artifact record",
                "weight_version",
                "must not be empty when provided",
            ));
        }
        if self
            .consumed_by_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(invalid_field(
                "sora model artifact record",
                "consumed_by_version",
                "must not be empty when provided",
            ));
        }
        if let Some(source_provenance) = &self.source_provenance {
            source_provenance.validate()?;
        }
        for (field, digest) in [
            ("weight_artifact_hash", self.weight_artifact_hash),
            ("training_config_hash", self.training_config_hash),
            ("reproducibility_hash", self.reproducibility_hash),
            (
                "provenance_attestation_hash",
                self.provenance_attestation_hash,
            ),
        ] {
            validate_soracloud_digest_hash("sora model artifact record", field, digest)?;
        }
        if let Some(chunk_manifest_root) = self.chunk_manifest_root {
            validate_soracloud_digest_hash(
                "sora model artifact record",
                "chunk_manifest_root",
                chunk_manifest_root,
            )?;
        }
        if self
            .source_provenance
            .as_ref()
            .is_some_and(|source| source.kind == SoraModelProvenanceKindV1::UserUpload)
            && self.chunk_manifest_root.is_none()
        {
            return Err(invalid_field(
                "sora model artifact record",
                "chunk_manifest_root",
                "user-upload artifacts must carry uploaded-model storage metadata",
            ));
        }
        if self.registered_sequence == 0 {
            return Err(invalid_field(
                "sora model artifact record",
                "registered_sequence",
                "must be greater than zero",
            ));
        }
        Ok(())
    }
}
