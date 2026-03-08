//! Lane governance manifest loading utilities.
//!
//! These helpers validate that lanes which advertise a governance module in the
//! Nexus catalog have a manifest available on disk and threads the parsed rules
//! into runtime enforcement (queue admission, governance telemetry, etc.).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use hex::decode;
use iroha_config::parameters::actual::{
    GovernanceCatalog, GovernanceModule as ConfigGovernanceModule, LaneRegistry,
};
use iroha_crypto::privacy::{
    LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit, SnarkCircuitId,
};
use iroha_data_model::{
    account::AccountId,
    nexus::{DataSpaceId, LaneCatalog, LaneId, LaneStorageProfile, LaneVisibility},
    prelude::Name,
};
use iroha_logger::{debug, info, warn};
use norito::json::{self, JsonDeserialize, JsonSerialize, Value as JsonValue};

/// Minimal manifest descriptor parsed from disk.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct ManifestFile {
    /// Lane alias the manifest targets.
    pub lane: Option<String>,
    /// Governance module identifier asserted by the manifest.
    pub governance: Option<String>,
    /// Semantic version (major) used to interpret the manifest.
    pub version: Option<u32>,
    /// Committee members or validator identifiers (human readable).
    #[norito(default)]
    pub validators: Option<Vec<String>>,
    /// Quorum threshold applied to the validator set.
    pub quorum: Option<u32>,
    /// Namespaces protected by governance (transactions require explicit approval).
    #[norito(default)]
    pub protected_namespaces: Option<Vec<String>>,
    /// Optional map of governance hooks (module-specific).
    #[norito(default)]
    pub hooks: Option<BTreeMap<String, JsonValue>>,
    /// Optional privacy commitment descriptors consumed by private lanes.
    #[norito(default)]
    pub privacy_commitments: Option<Vec<ManifestPrivacyCommitment>>,
}

/// Manifest-level privacy commitment descriptor.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct ManifestPrivacyCommitment {
    /// Registry identifier assigned to the commitment entry.
    pub id: Option<u16>,
    /// Commitment scheme (`merkle`, `snark`, …).
    pub scheme: Option<String>,
    /// Merkle-specific parameters.
    #[norito(default)]
    pub merkle: Option<ManifestMerkleCommitment>,
    /// zk-SNARK–specific parameters.
    #[norito(default)]
    pub snark: Option<ManifestSnarkCommitment>,
}

/// Merkle commitment parameters advertised in manifests.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct ManifestMerkleCommitment {
    /// Canonical 32-byte root digest encoded as hex.
    pub root: Option<String>,
    /// Maximum allowed audit-path depth.
    pub max_depth: Option<u8>,
}

/// zk-SNARK commitment parameters advertised in manifests.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct ManifestSnarkCommitment {
    /// Circuit identifier assigned by governance.
    pub circuit_id: Option<u16>,
    /// Digest of the verifying key payload.
    pub verifying_key_digest: Option<String>,
    /// Digest of the canonical public-input encoding.
    pub statement_hash: Option<String>,
    /// Digest of the proof bytes bound to the verifying key.
    pub proof_hash: Option<String>,
}

/// Governance catalog overlay loaded from distribution cache.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct GovernanceCatalogFile {
    /// Default governance module identifier applied when lanes omit an override.
    pub default_module: Option<String>,
    /// Registered governance modules keyed by name.
    #[norito(default)]
    pub modules: BTreeMap<String, GovernanceModuleFile>,
}

/// Governance module descriptor loaded from distribution cache.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, Default)]
struct GovernanceModuleFile {
    /// Module type (e.g., `parliament`, `stake_weighted`).
    pub module_type: Option<String>,
    /// Additional parameters defined by the module.
    #[norito(default)]
    pub params: BTreeMap<String, String>,
}

impl From<GovernanceModuleFile> for ConfigGovernanceModule {
    fn from(value: GovernanceModuleFile) -> Self {
        Self {
            module_type: value.module_type,
            params: value.params,
        }
    }
}

/// Status of a lane manifest after loading from disk.
#[derive(Debug, Clone)]
pub struct LaneManifestStatus {
    /// Lane identifier.
    pub lane: LaneId,
    /// Human-readable alias.
    pub alias: String,
    /// Dataspace binding derived from the lane catalog.
    pub dataspace: DataSpaceId,
    /// Declarative visibility profile.
    pub visibility: LaneVisibility,
    /// Storage profile advertised by the lane.
    pub storage: LaneStorageProfile,
    /// Governance module configured in the lane catalog.
    pub governance: Option<String>,
    /// Source path of the manifest if present.
    pub manifest_path: Option<PathBuf>,
    /// Parsed governance rules derived from the manifest.
    pub governance_rules: Option<GovernanceRules>,
    /// Lane privacy commitments derived from the manifest.
    pub privacy_commitments: Vec<LanePrivacyCommitment>,
}

impl LaneManifestStatus {
    fn missing(
        lane: LaneId,
        alias: String,
        dataspace: DataSpaceId,
        visibility: LaneVisibility,
        storage: LaneStorageProfile,
        governance: Option<String>,
    ) -> Self {
        Self {
            lane,
            alias,
            dataspace,
            visibility,
            storage,
            governance,
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        }
    }

    fn builder(
        lane: LaneId,
        alias: String,
        dataspace: DataSpaceId,
        visibility: LaneVisibility,
        storage: LaneStorageProfile,
    ) -> LaneManifestStatusBuilder {
        LaneManifestStatusBuilder::new(lane, alias, dataspace, visibility, storage)
    }

    /// Retrieve parsed governance rules if available.
    #[must_use]
    pub fn rules(&self) -> Option<&GovernanceRules> {
        self.governance_rules.as_ref()
    }

    /// Privacy commitments advertised by the lane manifest.
    #[must_use]
    pub fn privacy_commitments(&self) -> &[LanePrivacyCommitment] {
        &self.privacy_commitments
    }
}

#[derive(Debug)]
struct LaneManifestStatusBuilder {
    lane: LaneId,
    alias: String,
    dataspace: DataSpaceId,
    visibility: LaneVisibility,
    storage: LaneStorageProfile,
    governance: Option<String>,
    manifest_path: Option<PathBuf>,
    governance_rules: Option<GovernanceRules>,
    privacy_commitments: Vec<LanePrivacyCommitment>,
}

impl LaneManifestStatusBuilder {
    fn new(
        lane: LaneId,
        alias: String,
        dataspace: DataSpaceId,
        visibility: LaneVisibility,
        storage: LaneStorageProfile,
    ) -> Self {
        Self {
            lane,
            alias,
            dataspace,
            visibility,
            storage,
            governance: None,
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        }
    }

    fn governance(mut self, governance: Option<String>) -> Self {
        self.governance = governance;
        self
    }

    fn manifest_path(mut self, path: PathBuf) -> Self {
        self.manifest_path = Some(path);
        self
    }

    fn governance_rules(mut self, rules: GovernanceRules) -> Self {
        self.governance_rules = Some(rules);
        self
    }

    fn privacy_commitments(mut self, commitments: Vec<LanePrivacyCommitment>) -> Self {
        self.privacy_commitments = commitments;
        self
    }

    fn build_ready(self) -> Result<LaneManifestStatus, LaneManifestBuilderError> {
        let Some(path) = self.manifest_path else {
            return Err(LaneManifestBuilderError::MissingManifestPath);
        };
        let Some(rules) = self.governance_rules else {
            return Err(LaneManifestBuilderError::MissingGovernanceRules);
        };

        Ok(LaneManifestStatus {
            lane: self.lane,
            alias: self.alias,
            dataspace: self.dataspace,
            visibility: self.visibility,
            storage: self.storage,
            governance: self.governance,
            manifest_path: Some(path),
            governance_rules: Some(rules),
            privacy_commitments: self.privacy_commitments,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaneManifestBuilderError {
    MissingManifestPath,
    MissingGovernanceRules,
}

impl fmt::Display for LaneManifestBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingManifestPath => write!(f, "manifest path not provided"),
            Self::MissingGovernanceRules => write!(f, "governance rules not provided"),
        }
    }
}

impl std::error::Error for LaneManifestBuilderError {}

/// Governance rule set derived from a manifest file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GovernanceRules {
    /// Semantic version of the manifest (major).
    pub version: u32,
    /// Committee members / validators configured for the lane.
    pub validators: Vec<AccountId>,
    /// Quorum threshold applied to the validator set.
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the lane governance module.
    pub protected_namespaces: BTreeSet<Name>,
    /// Typed governance hooks with optional raw values for unknown entries.
    pub hooks: GovernanceHooks,
}

/// Artifacts derived from a manifest file.
#[derive(Debug, Clone)]
struct ManifestArtifacts {
    rules: GovernanceRules,
    privacy_commitments: Vec<LanePrivacyCommitment>,
}

/// Parsed governance hook policies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GovernanceHooks {
    /// Runtime upgrade admission policy.
    pub runtime_upgrade: Option<RuntimeUpgradeHook>,
    /// Unrecognised hooks preserved for future modules.
    pub unknown: BTreeMap<String, JsonValue>,
}

/// Runtime upgrade governance hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUpgradeHook {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Require metadata to accompany runtime-upgrade transactions.
    pub require_metadata: bool,
    /// Metadata key enforced by this hook (defaults to `gov_upgrade_id` when `require_metadata` or
    /// `allowed_ids` are present).
    pub metadata_key: Option<Name>,
    /// Optional allowlist of metadata values (`String`).
    pub allowed_ids: Option<BTreeSet<String>>,
}

impl GovernanceHooks {
    fn from_manifest_hooks(hooks: Option<&BTreeMap<String, JsonValue>>) -> Result<Self, String> {
        let mut parsed = Self::default();
        if let Some(entries) = hooks {
            for (key, value) in entries {
                let trimmed = key.trim();
                if trimmed.is_empty() {
                    return Err("hook names cannot be blank".into());
                }
                match trimmed {
                    "runtime_upgrade" => {
                        let hook = RuntimeUpgradeHook::from_json(value).map_err(|err| {
                            format!("invalid runtime_upgrade hook configuration: {err}")
                        })?;
                        parsed.runtime_upgrade = Some(hook);
                    }
                    other => {
                        parsed.unknown.insert(other.to_string(), value.clone());
                    }
                }
            }
        }
        Ok(parsed)
    }
}

impl RuntimeUpgradeHook {
    fn from_json(value: &JsonValue) -> Result<Self, String> {
        let JsonValue::Object(map) = value else {
            return Err("runtime_upgrade hook must be a JSON object".into());
        };

        let mut allow = true;
        if let Some(entry) = map.get("allow") {
            match entry {
                JsonValue::Bool(flag) => allow = *flag,
                _ => return Err("runtime_upgrade.allow must be a boolean".into()),
            }
        }

        let mut require_metadata = false;
        if let Some(entry) = map.get("require_metadata") {
            match entry {
                JsonValue::Bool(flag) => require_metadata = *flag,
                _ => {
                    return Err("runtime_upgrade.require_metadata must be a boolean".into());
                }
            }
        }

        let mut metadata_key: Option<Name> = None;
        if let Some(entry) = map.get("metadata_key") {
            match entry {
                JsonValue::String(raw) => {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        return Err("runtime_upgrade.metadata_key must not be blank".into());
                    }
                    metadata_key = Some(Name::from_str(trimmed).map_err(|err| {
                        format!("invalid runtime_upgrade.metadata_key `{trimmed}`: {err}")
                    })?);
                }
                _ => return Err("runtime_upgrade.metadata_key must be a string".into()),
            }
        }

        let mut allowed_ids: Option<BTreeSet<String>> = None;
        if let Some(entry) = map.get("allowed_ids") {
            match entry {
                JsonValue::Array(values) => {
                    let mut ids = BTreeSet::new();
                    for value in values {
                        match value {
                            JsonValue::String(raw) => {
                                let trimmed = raw.trim();
                                if trimmed.is_empty() {
                                    return Err(
                                        "runtime_upgrade.allowed_ids entries must not be blank"
                                            .into(),
                                    );
                                }
                                ids.insert(trimmed.to_string());
                            }
                            _ => {
                                return Err(
                                    "runtime_upgrade.allowed_ids entries must be strings".into()
                                );
                            }
                        }
                    }
                    if !ids.is_empty() {
                        allowed_ids = Some(ids);
                    }
                }
                _ => return Err("runtime_upgrade.allowed_ids must be an array".into()),
            }
        }

        if (require_metadata || allowed_ids.is_some()) && metadata_key.is_none() {
            metadata_key = Some(Name::from_str("gov_upgrade_id").map_err(|err| {
                format!("failed to derive default runtime_upgrade metadata key: {err}")
            })?);
        }

        Ok(Self {
            allow,
            require_metadata,
            metadata_key,
            allowed_ids,
        })
    }
}

impl GovernanceRules {
    fn from_manifest(alias: &str, manifest: &ManifestFile) -> Result<Self, String> {
        let version = manifest.version.unwrap_or(1);
        if version != 1 {
            return Err(format!(
                "manifest version {version} is not supported (expected 1)"
            ));
        }

        let mut validators = Vec::new();
        if let Some(entries) = manifest.validators.as_ref() {
            let mut seen = BTreeSet::new();
            for raw in entries {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err("validator entry cannot be blank".into());
                }
                let account = AccountId::parse_encoded(trimmed)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|err| format!("invalid validator id `{trimmed}`: {err}"))?;
                if seen.insert(account.clone()) {
                    validators.push(account);
                }
            }
        }

        let quorum = manifest.quorum;
        if let Some(q) = quorum {
            if q == 0 {
                return Err("validator quorum must be greater than zero".into());
            }
            if !validators.is_empty()
                && usize::try_from(q)
                    .ok()
                    .is_some_and(|q_usize| q_usize > validators.len())
            {
                return Err(format!(
                    "validator quorum {q} exceeds validator set size {} for lane `{alias}`",
                    validators.len()
                ));
            }
        }

        let mut protected_namespaces = BTreeSet::new();
        if let Some(namespaces) = manifest.protected_namespaces.as_ref() {
            for ns in namespaces {
                let trimmed = ns.trim();
                if trimmed.is_empty() {
                    return Err("protected namespace cannot be blank".into());
                }
                let name = Name::from_str(trimmed)
                    .map_err(|err| format!("invalid protected namespace `{trimmed}`: {err}"))?;
                protected_namespaces.insert(name);
            }
        }

        let hooks = GovernanceHooks::from_manifest_hooks(manifest.hooks.as_ref())?;

        Ok(Self {
            version,
            validators,
            quorum,
            protected_namespaces,
            hooks,
        })
    }
}

/// Registry of manifests keyed by lane identifier.
#[derive(Debug, Default)]
pub struct LaneManifestRegistry {
    statuses: BTreeMap<LaneId, LaneManifestStatus>,
}

impl LaneManifestRegistry {
    /// Construct an empty registry (no lanes require manifests).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build the registry from the provided Nexus configuration.
    #[allow(clippy::too_many_lines)]
    pub fn from_config(
        lane_catalog: &LaneCatalog,
        governance_catalog: &GovernanceCatalog,
        registry_cfg: &LaneRegistry,
    ) -> Self {
        let mut statuses = BTreeMap::new();
        let manifest_dir = registry_cfg.manifest_directory.as_deref();
        let cache_dir = registry_cfg.cache_directory.as_deref();

        let mut effective_governance = governance_catalog.clone();
        Self::apply_governance_overlay(cache_dir, &mut effective_governance);

        let manifests_by_alias = Self::collect_manifest_sources(manifest_dir, cache_dir);
        debug!(
            aliases = ?manifests_by_alias
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            "collected lane manifest aliases"
        );

        for lane in lane_catalog.lanes() {
            let alias = lane.alias.clone();
            let governance = lane.governance.clone();
            let dataspace = lane.dataspace_id;
            let visibility = lane.visibility;
            let storage = lane.storage;

            let manifest_path = manifests_by_alias.get(&alias).cloned();
            let status = manifest_path.map_or_else(
                || {
                    LaneManifestStatus::missing(
                        lane.id,
                        alias.clone(),
                        dataspace,
                        visibility,
                        storage,
                        governance.clone(),
                    )
                },
                |path| match Self::validate_manifest(
                    &path,
                    lane.id,
                    &alias,
                    governance.as_deref(),
                    &effective_governance,
                ) {
                    Ok(artifacts) => LaneManifestStatus::builder(
                        lane.id,
                        alias.clone(),
                        dataspace,
                        visibility,
                        storage,
                    )
                    .governance(governance.clone())
                    .manifest_path(path.clone())
                    .governance_rules(artifacts.rules)
                    .privacy_commitments(artifacts.privacy_commitments)
                    .build_ready()
                    .unwrap_or_else(|err| {
                        warn!(
                            lane = %alias,
                            path = %path.display(),
                            reason = %err,
                            "failed to finalize lane manifest status"
                        );
                        LaneManifestStatus::missing(
                            lane.id,
                            alias.clone(),
                            dataspace,
                            visibility,
                            storage,
                            governance.clone(),
                        )
                    }),
                    Err(msg) => {
                        warn!(lane = %alias, reason = %msg, "invalid lane manifest");
                        LaneManifestStatus::missing(
                            lane.id,
                            alias.clone(),
                            dataspace,
                            visibility,
                            storage,
                            governance.clone(),
                        )
                    }
                },
            );
            statuses.insert(lane.id, status);
        }

        // Warn about manifests targeting unknown lanes so operators can clean up.
        for (alias, path) in manifests_by_alias {
            if lane_catalog.by_alias(&alias).is_none() {
                warn!(
                    alias,
                    path = %path.display(),
                    "Found manifest for unknown lane alias"
                );
            }
        }

        // Log governance modules lacking manifest declarations.
        for status in statuses.values() {
            if status.governance.is_some() && status.manifest_path.is_none() {
                if let Some(gov) = status.governance.as_deref()
                    && !governance_catalog.modules.contains_key(gov)
                {
                    warn!(
                        lane = %status.alias,
                        module = gov,
                        "lane references governance module not defined in catalog"
                    );
                }
                debug!(
                    lane = %status.alias,
                    "lane governance manifest missing; queue will reject transactions for this lane"
                );
            }
        }

        Self { statuses }
    }

    fn collect_manifest_sources(
        manifest_dir: Option<&Path>,
        cache_dir: Option<&Path>,
    ) -> BTreeMap<String, PathBuf> {
        let mut manifests = BTreeMap::new();

        if let Some(dir) = manifest_dir {
            if dir.exists() {
                Self::ingest_manifest_directory(dir, &mut manifests, false);
            } else {
                warn!(
                    path = %dir.display(),
                    "lane manifest directory missing; all governance lanes will remain locked until manifests are installed"
                );
            }
        }

        if let Some(dir) = cache_dir {
            if dir.exists() {
                Self::ingest_manifest_directory(dir, &mut manifests, true);
            } else if manifest_dir.is_some() {
                debug!(
                    path = %dir.display(),
                    "lane manifest cache directory missing; continuing with primary directory entries"
                );
            }
        }

        manifests
    }

    fn ingest_manifest_directory(
        dir: &Path,
        manifests: &mut BTreeMap<String, PathBuf>,
        override_existing: bool,
    ) {
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(alias) = Self::manifest_alias_from_path(&path) else {
                        continue;
                    };

                    if manifests.contains_key(&alias) {
                        if override_existing {
                            if let Some(prev) = manifests.get(&alias) {
                                info!(
                                    lane = %alias,
                                    new_path = %path.display(),
                                    old_path = %prev.display(),
                                    "cache manifest overrides existing lane manifest"
                                );
                            }
                        } else {
                            warn!(
                                lane = %alias,
                                path = %path.display(),
                                "duplicate manifest alias in primary directory; skipping"
                            );
                            continue;
                        }
                    }

                    manifests.insert(alias, path);
                }
            }
            Err(err) => {
                warn!(
                    path = %dir.display(),
                    ?err,
                    "failed to read lane manifest directory"
                );
            }
        }
    }

    fn manifest_alias_from_path(path: &Path) -> Option<String> {
        if !path.is_file() {
            return None;
        }
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => {}
            _ => return None,
        }
        let stem = path.file_stem()?.to_str()?.trim();
        if stem.is_empty() {
            return None;
        }
        let alias = stem.strip_suffix(".manifest").unwrap_or(stem).trim();
        if alias.is_empty() {
            return None;
        }
        Some(alias.to_string())
    }

    fn apply_governance_overlay(cache_dir: Option<&Path>, catalog: &mut GovernanceCatalog) {
        let Some(dir) = cache_dir else { return };
        let overlay_path = dir.join("governance_catalog.json");
        if !overlay_path.exists() {
            return;
        }

        let contents = match fs::read_to_string(&overlay_path) {
            Ok(raw) => raw,
            Err(err) => {
                warn!(
                    path = %overlay_path.display(),
                    ?err,
                    "failed to read governance catalog overlay"
                );
                return;
            }
        };

        let parsed: GovernanceCatalogFile = match json::from_json(&contents) {
            Ok(value) => value,
            Err(err) => {
                warn!(
                    path = %overlay_path.display(),
                    ?err,
                    "governance catalog overlay JSON parse error"
                );
                return;
            }
        };

        let mut applied = false;

        if let Some(default_module) = parsed.default_module {
            let trimmed = default_module.trim();
            if trimmed.is_empty() {
                warn!(
                    path = %overlay_path.display(),
                    "default_module entry in governance catalog overlay is blank; ignoring"
                );
            } else {
                catalog.default_module = Some(trimmed.to_string());
                applied = true;
            }
        }

        for (name, module) in parsed.modules {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                warn!(
                    path = %overlay_path.display(),
                    "governance catalog overlay encountered module with blank name; skipping"
                );
                continue;
            }
            catalog.modules.insert(trimmed.to_string(), module.into());
            applied = true;
        }

        if applied {
            info!(
                path = %overlay_path.display(),
                "applied governance catalog overlay from lane registry cache directory"
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_privacy_commitments(
        alias: &str,
        manifest: &ManifestFile,
    ) -> Result<Vec<LanePrivacyCommitment>, String> {
        let Some(entries) = manifest.privacy_commitments.as_ref() else {
            return Ok(Vec::new());
        };

        let mut commitments = Vec::new();
        let mut seen_ids = BTreeSet::new();

        for entry in entries {
            let id = entry.id.ok_or_else(|| {
                format!("privacy commitment entry missing `id` for lane `{alias}`")
            })?;
            if !seen_ids.insert(id) {
                return Err(format!(
                    "privacy commitment id {id} appears multiple times for lane `{alias}`"
                ));
            }

            let scheme_raw = entry.scheme.as_deref().ok_or_else(|| {
                format!("privacy commitment {id} is missing `scheme` for lane `{alias}`")
            })?;
            let scheme = scheme_raw.trim().to_ascii_lowercase();

            match scheme.as_str() {
                "merkle" => {
                    let merkle = entry.merkle.as_ref().ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` must include a `merkle` section"
                    )
                })?;
                    let root_raw = merkle.root.as_deref().ok_or_else(|| {
                        format!(
                            "privacy commitment {id} for lane `{alias}` is missing `merkle.root`"
                        )
                    })?;
                    let max_depth = merkle.max_depth.ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` is missing `merkle.max_depth`"
                    )
                })?;
                    if max_depth == 0 {
                        return Err(format!(
                            "privacy commitment {id} for lane `{alias}` has invalid `merkle.max_depth` (must be > 0)"
                        ));
                    }
                    let root_bytes =
                        Self::parse_hex_digest(alias, id, "merkle.root", root_raw.trim())?;
                    commitments.push(LanePrivacyCommitment::merkle(
                        LaneCommitmentId::new(id),
                        MerkleCommitment::from_root_bytes(root_bytes, max_depth),
                    ));
                }
                "snark" => {
                    let snark = entry.snark.as_ref().ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` must include a `snark` section"
                    )
                })?;
                    let circuit_id = snark.circuit_id.ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` is missing `snark.circuit_id`"
                    )
                })?;
                    let vk_digest = snark.verifying_key_digest.as_deref().ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` is missing `snark.verifying_key_digest`"
                    )
                })?;
                    let statement_hash = snark.statement_hash.as_deref().ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` is missing `snark.statement_hash`"
                    )
                })?;
                    let proof_hash = snark.proof_hash.as_deref().ok_or_else(|| {
                    format!(
                        "privacy commitment {id} for lane `{alias}` is missing `snark.proof_hash`"
                    )
                })?;
                    let circuit = SnarkCircuit::new(
                        SnarkCircuitId::new(circuit_id),
                        Self::parse_hex_digest(
                            alias,
                            id,
                            "snark.verifying_key_digest",
                            vk_digest.trim(),
                        )?,
                        Self::parse_hex_digest(
                            alias,
                            id,
                            "snark.statement_hash",
                            statement_hash.trim(),
                        )?,
                        Self::parse_hex_digest(alias, id, "snark.proof_hash", proof_hash.trim())?,
                    );
                    commitments.push(LanePrivacyCommitment::snark(
                        LaneCommitmentId::new(id),
                        circuit,
                    ));
                }
                other => {
                    return Err(format!(
                        "privacy commitment {id} for lane `{alias}` uses unsupported scheme `{other}`"
                    ));
                }
            }
        }

        Ok(commitments)
    }

    fn parse_hex_digest(
        alias: &str,
        commitment_id: u16,
        field: &str,
        raw: &str,
    ) -> Result<[u8; 32], String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "privacy commitment {commitment_id} for lane `{alias}` has blank `{field}`"
            ));
        }
        let normalized = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);
        if normalized.len() != 64 {
            return Err(format!(
                "privacy commitment {commitment_id} for lane `{alias}` must encode `{field}` as a 32-byte hex digest"
            ));
        }
        let bytes = decode(normalized).map_err(|err| {
            format!(
                "privacy commitment {commitment_id} for lane `{alias}` has invalid `{field}`: {err}"
            )
        })?;
        if bytes.len() != 32 {
            return Err(format!(
                "privacy commitment {commitment_id} for lane `{alias}` `{field}` decoded to {} bytes (expected 32)",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    fn validate_manifest(
        path: &Path,
        lane_id: LaneId,
        alias: &str,
        lane_governance: Option<&str>,
        catalog: &GovernanceCatalog,
    ) -> Result<ManifestArtifacts, String> {
        let Ok(contents) = fs::read_to_string(path) else {
            return Err("unable to read manifest file".to_string());
        };
        let parsed: ManifestFile = match json::from_json(&contents) {
            Ok(value) => value,
            Err(err) => {
                return Err(format!("manifest JSON parse error: {err}"));
            }
        };
        if let Some(manifest_lane) = parsed.lane.as_deref()
            && manifest_lane != alias
        {
            return Err(format!(
                "manifest targets lane \"{manifest_lane}\" but is located at alias \"{alias}\""
            ));
        }
        let manifest_governance = parsed.governance.as_deref();
        if let Some(expected) = lane_governance {
            match manifest_governance {
                Some(manifest_module) => {
                    if manifest_module != expected {
                        return Err(format!(
                            "manifest governance module {manifest_module} does not match lane configuration {expected}"
                        ));
                    }
                    if !catalog.modules.contains_key(expected) {
                        return Err(format!(
                            "lane references governance module `{expected}` not present in catalog"
                        ));
                    }
                }
                None => return Err("manifest missing governance module identifier".into()),
            }
        }

        let privacy_commitments = Self::parse_privacy_commitments(alias, &parsed)?;
        let rules = GovernanceRules::from_manifest(alias, &parsed)?;
        debug!(
            lane = alias,
            path = %path.display(),
            lane_id = lane_id.as_u32(),
            "loaded governance manifest"
        );
        Ok(ManifestArtifacts {
            rules,
            privacy_commitments,
        })
    }

    /// Install manifests into the registry from pre-built statuses (testing/telemetry scaffolding).
    #[cfg(any(test, feature = "telemetry"))]
    pub fn from_statuses(statuses: BTreeMap<LaneId, LaneManifestStatus>) -> Self {
        Self { statuses }
    }

    /// Whether the lane is ready for traffic under its governance manifest.
    ///
    /// # Errors
    ///
    /// Returns [`GovernanceGuardError`] when the lane requires a manifest but none was loaded.
    pub fn ensure_lane_ready(&self, lane_id: LaneId) -> Result<(), GovernanceGuardError> {
        if let Some(status) = self.statuses.get(&lane_id) {
            if status.governance.is_some() && status.manifest_path.is_none() {
                return Err(GovernanceGuardError::missing_manifest(status));
            }
            if status.manifest_path.is_some()
                && matches!(
                    status.storage,
                    LaneStorageProfile::CommitmentOnly | LaneStorageProfile::SplitReplica
                )
                && status.privacy_commitments.is_empty()
            {
                return Err(GovernanceGuardError::missing_privacy_commitments(status));
            }
        }
        Ok(())
    }

    /// Enumerate lanes missing manifests (for logging or tests).
    pub fn missing_entries(&self) -> Vec<&LaneManifestStatus> {
        self.statuses
            .values()
            .filter(|status| status.governance.is_some() && status.manifest_path.is_none())
            .collect()
    }

    /// Collect lane aliases that currently lack manifests.
    pub fn missing_aliases(&self) -> BTreeSet<String> {
        self.missing_entries()
            .iter()
            .map(|status| status.alias.clone())
            .collect()
    }

    /// Retrieve the manifest status for `lane_id`, if available.
    pub fn status(&self, lane_id: LaneId) -> Option<&LaneManifestStatus> {
        self.statuses.get(&lane_id)
    }

    /// Retrieve parsed governance rules for `lane_id`, if available.
    pub fn lane_rules(&self, lane_id: LaneId) -> Option<&GovernanceRules> {
        self.status(lane_id).and_then(LaneManifestStatus::rules)
    }

    /// Retrieve the validator set declared for `lane_id`, if present.
    pub fn lane_validators(&self, lane_id: LaneId) -> Option<Vec<AccountId>> {
        self.lane_rules(lane_id)
            .map(|rules| rules.validators.clone())
    }

    /// Retrieve the quorum declared for `lane_id`, if present.
    pub fn lane_quorum(&self, lane_id: LaneId) -> Option<u32> {
        self.lane_rules(lane_id).and_then(|rules| rules.quorum)
    }

    /// Snapshot the current manifest statuses for all lanes.
    #[must_use]
    pub fn statuses(&self) -> Vec<LaneManifestStatus> {
        self.statuses.values().cloned().collect()
    }
}

/// Governance guard error returned when a lane lacks an active manifest.
#[derive(Debug, Clone)]
pub struct GovernanceGuardError {
    /// Lane identifier.
    pub lane: LaneId,
    /// Human-readable alias.
    pub alias: String,
    /// Governance module configured in the catalog.
    pub governance: Option<String>,
    /// Reason why the lane is not ready for traffic.
    pub reason: GovernanceGuardReason,
}

impl GovernanceGuardError {
    /// Render the failure reason for logs.
    #[must_use]
    pub fn message(&self) -> String {
        match self.reason {
            GovernanceGuardReason::MissingManifest => self
                .governance
                .as_deref()
                .map_or_else(
                    || {
                        format!(
                            "lane \"{}\" ({}) requires a governance manifest but none was loaded",
                            self.alias,
                            self.lane.as_u32(),
                        )
                    },
                    |module| {
                        format!(
                            "lane \"{}\" ({}) references governance module \"{module}\" but no manifest was loaded",
                            self.alias,
                            self.lane.as_u32(),
                        )
                    },
                ),
            GovernanceGuardReason::MissingPrivacyCommitments => format!(
                "lane \"{}\" ({}) is configured for commitment-only storage but the manifest does not declare any privacy commitments",
                self.alias,
                self.lane.as_u32(),
            ),
        }
    }

    /// Retrieve the guard failure reason.
    #[must_use]
    pub const fn reason(&self) -> GovernanceGuardReason {
        self.reason
    }

    fn missing_manifest(status: &LaneManifestStatus) -> Self {
        Self {
            lane: status.lane,
            alias: status.alias.clone(),
            governance: status.governance.clone(),
            reason: GovernanceGuardReason::MissingManifest,
        }
    }

    fn missing_privacy_commitments(status: &LaneManifestStatus) -> Self {
        Self {
            lane: status.lane,
            alias: status.alias.clone(),
            governance: status.governance.clone(),
            reason: GovernanceGuardReason::MissingPrivacyCommitments,
        }
    }
}

/// Reasons surfaced when a lane is gated by governance/manifest checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceGuardReason {
    /// Lane requires a manifest but none was found.
    MissingManifest,
    /// Lane advertises commitment-only storage but has no privacy commitments configured.
    MissingPrivacyCommitments,
}

/// Shared registry handle.
pub type LaneManifestRegistryHandle = Arc<LaneManifestRegistry>;

impl fmt::Display for GovernanceGuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use iroha_config::parameters::actual::{
        GovernanceCatalog, GovernanceModule as ConfigGovernanceModule, LaneRegistry,
    };
    use iroha_data_model::{
        account::AccountId,
        nexus::{LaneCatalog, LaneConfig},
        prelude::Name,
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;
    use tempfile::tempdir;

    use super::*;

    fn account_id_literal(account: &AccountId) -> String {
        account.to_string()
    }

    #[test]
    fn builder_requires_manifest_components() {
        let err = LaneManifestStatus::builder(
            LaneId::new(7),
            "lane".to_string(),
            DataSpaceId::new(11),
            LaneVisibility::Public,
            LaneStorageProfile::default(),
        )
        .governance_rules(GovernanceRules::default())
        .build_ready()
        .expect_err("missing manifest path should fail");
        assert_eq!(err, LaneManifestBuilderError::MissingManifestPath);

        let err = LaneManifestStatus::builder(
            LaneId::new(7),
            "lane".to_string(),
            DataSpaceId::new(11),
            LaneVisibility::Public,
            LaneStorageProfile::default(),
        )
        .manifest_path(PathBuf::from("lane.manifest.json"))
        .build_ready()
        .expect_err("missing governance rules should fail");
        assert_eq!(err, LaneManifestBuilderError::MissingGovernanceRules);
    }

    #[test]
    fn privacy_commitments_parse_from_manifest() {
        let manifest = ManifestFile {
            lane: Some("private".to_string()),
            governance: Some("council".to_string()),
            privacy_commitments: Some(vec![ManifestPrivacyCommitment {
                id: Some(1),
                scheme: Some("merkle".to_string()),
                merkle: Some(ManifestMerkleCommitment {
                    root: Some(
                        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                            .to_string(),
                    ),
                    max_depth: Some(16),
                }),
                snark: None,
            }]),
            ..ManifestFile::default()
        };
        let parsed = LaneManifestRegistry::parse_privacy_commitments("private", &manifest)
            .expect("commitments parsed");
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn commitment_only_lane_without_commitments_is_rejected() {
        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::new(1),
            LaneManifestStatus {
                lane: LaneId::new(1),
                alias: "private".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Restricted,
                storage: LaneStorageProfile::CommitmentOnly,
                governance: Some("council".to_string()),
                manifest_path: Some(PathBuf::from("/tmp/private.manifest.json")),
                governance_rules: Some(GovernanceRules::default()),
                privacy_commitments: Vec::new(),
            },
        );
        let registry = LaneManifestRegistry::from_statuses(statuses);
        let err = registry
            .ensure_lane_ready(LaneId::new(1))
            .expect_err("lane should be gated");
        assert_eq!(
            err.reason(),
            GovernanceGuardReason::MissingPrivacyCommitments
        );
    }

    #[test]
    fn commitment_only_lane_with_commitments_is_allowed() {
        let mut statuses = BTreeMap::new();
        let commitment = LanePrivacyCommitment::merkle(
            LaneCommitmentId::new(1),
            MerkleCommitment::from_root_bytes([0xAA; 32], 12),
        );
        statuses.insert(
            LaneId::new(2),
            LaneManifestStatus {
                lane: LaneId::new(2),
                alias: "private".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Restricted,
                storage: LaneStorageProfile::CommitmentOnly,
                governance: Some("council".to_string()),
                manifest_path: Some(PathBuf::from("/tmp/private.manifest.json")),
                governance_rules: Some(GovernanceRules::default()),
                privacy_commitments: vec![commitment],
            },
        );
        let registry = LaneManifestRegistry::from_statuses(statuses);
        assert!(registry.ensure_lane_ready(LaneId::new(2)).is_ok());
    }

    #[test]
    fn builder_produces_ready_status() {
        let rules = GovernanceRules {
            version: 2,
            quorum: Some(3),
            ..GovernanceRules::default()
        };
        let expected_rules = rules.clone();
        let manifest_path = PathBuf::from("lane.manifest.json");

        let status = LaneManifestStatus::builder(
            LaneId::new(3),
            "lane".to_string(),
            DataSpaceId::new(5),
            LaneVisibility::Restricted,
            LaneStorageProfile::SplitReplica,
        )
        .governance(Some("council".to_string()))
        .manifest_path(manifest_path.clone())
        .governance_rules(rules)
        .build_ready()
        .expect("builder should construct ready status");

        assert_eq!(status.lane, LaneId::new(3));
        assert_eq!(status.alias, "lane");
        assert_eq!(status.dataspace, DataSpaceId::new(5));
        assert_eq!(status.visibility, LaneVisibility::Restricted);
        assert_eq!(status.storage, LaneStorageProfile::SplitReplica);
        assert_eq!(status.governance.as_deref(), Some("council"));
        assert_eq!(status.manifest_path, Some(manifest_path));
        assert_eq!(status.rules(), Some(&expected_rules));
    }

    #[test]
    fn registry_detects_missing_manifest() {
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "governance".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());
        let registry_cfg = LaneRegistry::default();

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        let err = registry
            .ensure_lane_ready(LaneId::new(0))
            .expect_err("missing manifest should trigger governance guard");
        assert_eq!(err.alias, "governance");
        assert_eq!(registry.missing_entries().len(), 1);
    }

    #[test]
    fn registry_loads_present_manifest() {
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());
        let dir = tempdir().expect("tmp dir");
        let path = dir.path().join("gov.manifest.json");
        fs::write(&path, r#"{"lane":"gov","governance":"parliament"}"#).expect("write manifest");
        let registry_cfg = LaneRegistry {
            manifest_directory: Some(path.parent().unwrap().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        assert!(
            registry.ensure_lane_ready(LaneId::new(0)).is_ok(),
            "manifest should satisfy governance guard"
        );
        assert!(registry.missing_entries().is_empty());
        let status = registry.status(LaneId::new(0)).expect("lane status");
        let rules = status.rules().expect("governance rules parsed");
        assert_eq!(rules.version, 1);
        assert!(rules.validators.is_empty());
        assert!(rules.protected_namespaces.is_empty());
    }

    #[test]
    fn cache_manifest_overrides_primary_directory() {
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());

        let primary_dir = tempdir().expect("primary manifest directory");
        let cache_dir = tempdir().expect("cache manifest directory");

        fs::write(
            primary_dir.path().join("gov.manifest.json"),
            r#"{"lane":"gov","governance":"parliament","protected_namespaces":["primary"]}"#,
        )
        .expect("write primary manifest");
        fs::write(
            cache_dir.path().join("gov.manifest.json"),
            r#"{"lane":"gov","governance":"parliament","protected_namespaces":["cached"]}"#,
        )
        .expect("write cache manifest");

        let registry_cfg = LaneRegistry {
            manifest_directory: Some(primary_dir.path().to_path_buf()),
            cache_directory: Some(cache_dir.path().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        let status = registry.status(LaneId::new(0)).expect("lane status");
        let rules = status.rules().expect("rules present");
        let expected_ns = Name::from_str("cached").expect("valid namespace");
        assert!(
            rules.protected_namespaces.contains(&expected_ns),
            "cache manifest should override namespaces"
        );
        assert_eq!(rules.protected_namespaces.len(), 1);
    }

    #[test]
    fn governance_overlay_supplies_missing_module() {
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "council".to_string(),
                governance: Some("council".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let governance = GovernanceCatalog::default();

        let cache_dir = tempdir().expect("cache manifest directory");
        fs::write(
            cache_dir.path().join("council.manifest.json"),
            r#"{"lane":"council","governance":"council"}"#,
        )
        .expect("write cache manifest");
        fs::write(
            cache_dir.path().join("governance_catalog.json"),
            r#"{"default_module":"council","modules":{"council":{"module_type":"council_multisig","params":{"quorum":"2"}}}}"#,
        )
        .expect("write governance overlay");

        let registry_cfg = LaneRegistry {
            manifest_directory: None,
            cache_directory: Some(cache_dir.path().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        let status = registry.status(LaneId::new(0)).expect("lane status");
        assert!(
            status.rules().is_some(),
            "overlay should supply governance module so manifest loads"
        );
        assert!(
            status.manifest_path.is_some(),
            "manifest from cache directory should be registered"
        );
    }

    #[test]
    fn manifest_rejects_invalid_validator() {
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());
        let dir = tempdir().expect("tmp dir");
        let path = dir.path().join("gov.manifest.json");
        fs::write(
            &path,
            r#"{"lane":"gov","governance":"parliament","validators":["not_an_account"]}"#,
        )
        .expect("write manifest");
        let registry_cfg = LaneRegistry {
            manifest_directory: Some(path.parent().unwrap().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        assert!(registry.ensure_lane_ready(LaneId::new(0)).is_err());
    }

    #[test]
    fn manifest_rejects_quorum_larger_than_validator_set() {
        crate::test_alias::ensure();
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());
        let dir = tempdir().expect("tmp dir");
        let path = dir.path().join("gov.manifest.json");
        let alice = account_id_literal(&ALICE_ID);
        fs::write(
            &path,
            format!(
                r#"{{"lane":"gov","governance":"parliament","validators":["{alice}"],"quorum":2}}"#
            ),
        )
        .expect("write manifest");
        let registry_cfg = LaneRegistry {
            manifest_directory: Some(path.parent().unwrap().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        assert!(registry.ensure_lane_ready(LaneId::new(0)).is_err());
    }

    #[test]
    fn manifest_parses_validators_and_namespaces() {
        crate::test_alias::ensure();
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());
        let dir = tempdir().expect("tmp dir");
        let path = dir.path().join("gov.manifest.json");
        let alice = account_id_literal(&ALICE_ID);
        let bob = account_id_literal(&BOB_ID);
        fs::write(
            &path,
            format!(
                r#"{{
                "lane": "gov",
                "governance": "parliament",
                "validators": ["  {alice}  ", "   {alice}  ", "{bob}"],
                "quorum": 2,
                "protected_namespaces": [" treasury ", "compliance"],
                "hooks": {{
                    "runtime_upgrade": {{
                        "allow": true,
                        "require_metadata": true,
                        "metadata_key": "gov_upgrade_id",
                        "allowed_ids": [" upgrade-q1 "]
                    }},
                    "custom_hook": {{"uri":"https://example.com"}}
                }}
            }}"#
            ),
        )
        .expect("write manifest");
        let registry_cfg = LaneRegistry {
            manifest_directory: Some(path.parent().unwrap().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        assert!(registry.ensure_lane_ready(LaneId::new(0)).is_ok());
        let rules = registry
            .lane_rules(LaneId::new(0))
            .expect("governance rules present");
        assert_eq!(rules.validators.len(), 2);
        assert_eq!(rules.quorum, Some(2));
        assert_eq!(rules.protected_namespaces.len(), 2);
        let runtime_hook = rules
            .hooks
            .runtime_upgrade
            .as_ref()
            .expect("runtime upgrade hook parsed");
        assert!(runtime_hook.allow);
        assert!(runtime_hook.require_metadata);
        assert_eq!(
            runtime_hook
                .metadata_key
                .as_ref()
                .expect("metadata key")
                .as_ref(),
            "gov_upgrade_id"
        );
        let allowed_ids = runtime_hook
            .allowed_ids
            .as_ref()
            .expect("allowed ids present");
        assert!(allowed_ids.contains("upgrade-q1"));
        assert!(rules.hooks.unknown.contains_key("custom_hook"));
    }

    #[test]
    fn manifests_allow_validator_reuse_across_lanes() {
        crate::test_alias::ensure();
        let lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                LaneConfig {
                    id: LaneId::new(0),
                    alias: "core".to_string(),
                    governance: Some("parliament".to_string()),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "payments".to_string(),
                    governance: Some("parliament".to_string()),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("valid catalog");
        let mut governance = GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_string(), ConfigGovernanceModule::default());

        let dir = tempdir().expect("tmp dir");
        let alice = account_id_literal(&ALICE_ID);
        let bob = account_id_literal(&BOB_ID);
        let manifest_body = format!(
            r#"{{
            "lane": "%ALIAS%",
            "governance": "parliament",
            "validators": ["{alice}", "{bob}"],
            "quorum": 2
        }}"#
        );
        for alias in ["core", "payments"] {
            let path = dir.path().join(format!("{alias}.manifest.json"));
            let body = manifest_body.replace("%ALIAS%", alias);
            fs::write(&path, body).expect("write manifest");
        }
        let registry_cfg = LaneRegistry {
            manifest_directory: Some(dir.path().to_path_buf()),
            ..LaneRegistry::default()
        };

        let registry = LaneManifestRegistry::from_config(&lane_catalog, &governance, &registry_cfg);
        assert!(registry.ensure_lane_ready(LaneId::new(0)).is_ok());
        assert!(registry.ensure_lane_ready(LaneId::new(1)).is_ok());

        let core_validators = registry
            .lane_validators(LaneId::new(0))
            .expect("core validators parsed");
        let payments_validators = registry
            .lane_validators(LaneId::new(1))
            .expect("payments validators parsed");
        assert_eq!(core_validators.len(), 2);
        assert_eq!(payments_validators.len(), 2);
        assert_eq!(core_validators, payments_validators);
        assert_eq!(registry.lane_quorum(LaneId::new(0)), Some(2));
        assert_eq!(registry.lane_quorum(LaneId::new(1)), Some(2));
    }
}
