//! Derive committed manifest additions from the immutable startup source authority.

use super::*;
use iroha_data_model::nexus::{DataSpaceCatalog, DataSpaceMetadata, RuntimeLaneManifestV1};

impl LaneManifestRegistry {
    /// Whether these frozen sources are bound to this exact effective catalog.
    ///
    /// A source-preserving lifecycle transition still changes this binding. Compare it at
    /// publication so a concurrent filesystem refresh cannot reinstall stale lane statuses.
    /// Empty and status-only registries do not claim an authenticated catalog binding.
    #[must_use]
    pub fn is_bound_to_catalog(&self, catalog: &LaneCatalog) -> bool {
        self.bound_catalog_hash.is_some_and(|bound| {
            bound == iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(catalog)
        })
    }

    /// Return the immutable startup manifest digest, excluding committed additions.
    ///
    /// Runtime additions have their own authenticated world-state commitment. Keeping
    /// the startup digest separate prevents an additive transition from rewriting the
    /// execution-policy identity of retained predecessor blocks.
    #[must_use]
    pub fn baseline_consensus_policy_digest(&self) -> [u8; 32] {
        self.baseline_source_snapshot.as_ref().map_or_else(
            || self.consensus_policy_digest(),
            |baseline| baseline.consensus_policy_digest(),
        )
    }

    /// Whether this exact lane has a successfully parsed and bound manifest source.
    ///
    /// Committed manifests deliberately have no filesystem path. Source presence
    /// does not replace [`Self::ensure_lane_ready`] or committee eligibility checks.
    #[must_use]
    pub fn has_manifest(&self, lane_id: LaneId) -> bool {
        let Some(status) = self.statuses.get(&lane_id) else {
            return false;
        };
        self.source_snapshot.as_ref().map_or_else(
            || status.manifest_path.is_some(),
            |sources| {
                status.governance_rules.is_some()
                    && sources
                        .manifests_by_alias
                        .get(&status.alias)
                        .is_some_and(|source| source.content_digest.valid && source.parsed.is_ok())
            },
        )
    }

    /// Validate one native runtime manifest against its declared lane and dataspace.
    ///
    /// CLI producers use the same bounded parser and semantic authority as runtime
    /// installation. `governance` must be the effective governance catalog, including
    /// any authenticated startup overlay. This check neither authorizes publication
    /// nor verifies live account, peer, role, consensus-key or PoP eligibility.
    /// Installation additionally enforces immutable baseline ownership, cumulative
    /// source budgets, cross-lane authority consistency and the complete catalog.
    ///
    /// # Errors
    ///
    /// Rejects mismatched lane/dataspace bindings, malformed or oversized sources,
    /// invalid governance rules or privacy commitments, and any roster/quorum that
    /// differs from the physical dataspace's exact `3f+1`/`2f+1` requirement.
    pub fn validate_runtime_manifest(
        addition: &RuntimeLaneManifestV1,
        lane: &LaneConfig,
        dataspace: &DataSpaceMetadata,
        governance: &GovernanceCatalog,
    ) -> Result<(), String> {
        Self::prepare_runtime_manifest(
            addition,
            lane,
            dataspace,
            governance,
            &mut ManifestSourceLoadBudget::default(),
        )
        .map(|_| ())
    }

    fn prepare_runtime_manifest(
        addition: &RuntimeLaneManifestV1,
        lane: &LaneConfig,
        dataspace: &DataSpaceMetadata,
        governance: &GovernanceCatalog,
        budget: &mut ManifestSourceLoadBudget,
    ) -> Result<FrozenLaneManifestSource, String> {
        if addition.lane_id != lane.id {
            return Err("runtime manifest must bind its exact effective lane id".to_owned());
        }
        if lane.dataspace_id != dataspace.id {
            return Err("runtime manifest lane must bind its exact physical dataspace".to_owned());
        }
        let committee_size = dataspace
            .fault_tolerance
            .checked_mul(3)
            .and_then(|f| f.checked_add(1))
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size <= LANE_MANIFEST_MAX_VALIDATORS_V1)
            .ok_or_else(|| {
                "runtime manifest dataspace committee exceeds the native bound".to_owned()
            })?;
        let quorum = dataspace
            .fault_tolerance
            .checked_mul(2)
            .and_then(|f| f.checked_add(1))
            .ok_or_else(|| "runtime manifest dataspace quorum overflowed".to_owned())?;
        let raw = addition.manifest.get().as_bytes();
        if raw.is_empty() || raw.len() > LANE_MANIFEST_MAX_BYTES_V1 {
            return Err("runtime manifest source exceeds the native per-source bound".to_owned());
        }
        budget.charge_bytes(raw.len())?;
        let parsed = Self::parse_bounded_manifest_json(raw, budget)?;
        Self::validate_manifest_source_bounds(&parsed)?;
        if parsed.lane.as_deref() != Some(lane.alias.as_str()) {
            return Err("runtime manifest must name its exact effective lane alias".to_owned());
        }
        let artifacts = Self::validate_parsed_manifest(
            &parsed,
            lane.id,
            &lane.alias,
            lane.governance.as_deref(),
            governance,
        )?;
        if artifacts.rules.validators.len() != committee_size
            || artifacts.rules.validator_bindings.len() != committee_size
            || artifacts.rules.quorum != Some(quorum)
        {
            return Err(
                "runtime manifest requires exact 3f+1 validators and 2f+1 quorum".to_owned(),
            );
        }
        if lane.storage == LaneStorageProfile::CommitmentOnly
            && artifacts.privacy_commitments.is_empty()
        {
            return Err("runtime commitment-only manifest requires privacy commitments".to_owned());
        }
        let canonical = json::to_json(&parsed)
            .map_err(|error| format!("runtime manifest canonical encoding failed: {error}"))?;
        if canonical.len() > LANE_MANIFEST_MAX_CANONICAL_BYTES_V1 {
            return Err("runtime manifest canonical source exceeds the native bound".to_owned());
        }
        Ok(FrozenLaneManifestSource {
            path: None,
            parsed: Ok(parsed),
            content_digest: LaneManifestSourceContentDigestV1 {
                valid: true,
                digest: Hash::new(canonical.as_bytes()).into(),
            },
        })
    }

    /// Derive an effective registry from the full authenticated runtime overlay.
    ///
    /// Always rebuilds from the original startup source, even when `self` already
    /// contains additions. This method performs no filesystem access. The caller
    /// must authenticate the cumulative world-state overlay, enforce append-only
    /// publication, and verify live account, peer, consensus-key, role and PoP
    /// eligibility at its activation height.
    ///
    /// # Errors
    ///
    /// Rejects an unmaterialized baseline, a takeover of any original lane or alias,
    /// malformed or oversized sources, duplicate bindings, or a committee/quorum
    /// differing from the new lane's physical dataspace fault tolerance.
    pub fn with_runtime_additions(
        &self,
        additions: &[RuntimeLaneManifestV1],
        effective_catalog: &LaneCatalog,
        dataspaces: &DataSpaceCatalog,
        governance: &GovernanceCatalog,
    ) -> Result<Self, String> {
        let baseline = self
            .baseline_source_snapshot
            .as_ref()
            .or(self.source_snapshot.as_ref())
            .ok_or_else(|| "runtime manifests require a frozen startup source".to_owned())?;
        if baseline.pending_registry.is_some() {
            return Err("runtime manifests cannot materialize filesystem sources".to_owned());
        }
        if additions.is_empty() {
            return Ok(baseline.bind(effective_catalog, governance));
        }
        if baseline.bound_lanes.is_empty() {
            return Err(
                "runtime manifests require an authenticated startup catalog binding".to_owned(),
            );
        }
        if additions.len() > effective_catalog.lanes().len()
            || additions
                .windows(2)
                .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(
                "runtime manifests require bounded, strictly ordered unique lane ids".to_owned(),
            );
        }
        let baseline_aliases: BTreeSet<_> = baseline
            .bound_lanes
            .values()
            .map(|lane| lane.alias.as_str())
            .chain(baseline.manifests_by_alias.keys().map(String::as_str))
            .collect();
        let mut budget = ManifestSourceLoadBudget::default();
        // Charge already-admitted immutable sources as well, so repeated additive
        // transitions cannot evade the aggregate source budget. No path is reopened.
        for source in baseline.manifests_by_alias.values() {
            let parsed = source.parsed.as_ref().map_err(Clone::clone)?;
            let raw = json::to_json(parsed)
                .map_err(|error| format!("baseline manifest canonical encoding failed: {error}"))?;
            budget.charge_bytes(raw.len())?;
            budget.charge_json(Self::preflight_manifest_json(
                raw.as_bytes(),
                MANIFEST_JSON_LIMITS_V1,
            )?)?;
        }
        if let Some(source) = &baseline.governance_overlay {
            let parsed = source.parsed.as_ref().map_err(Clone::clone)?;
            let raw = json::to_json(parsed).map_err(|error| {
                format!("baseline governance canonical encoding failed: {error}")
            })?;
            budget.charge_bytes(raw.len())?;
            budget.charge_json(Self::preflight_manifest_json(
                raw.as_bytes(),
                MANIFEST_JSON_LIMITS_V1,
            )?)?;
        }
        let mut effective_sources = (**baseline).clone();
        let mut effective_governance = governance.clone();
        baseline.apply_governance_overlay(&mut effective_governance);
        for addition in additions {
            let lane = effective_catalog
                .lanes()
                .iter()
                .find(|lane| lane.id == addition.lane_id)
                .ok_or_else(|| {
                    "runtime manifest lane is absent from the effective catalog".to_owned()
                })?;
            if baseline.bound_lanes.contains_key(&lane.id)
                || baseline_aliases.contains(lane.alias.as_str())
                || effective_sources
                    .manifests_by_alias
                    .contains_key(&lane.alias)
            {
                return Err(
                    "runtime manifest cannot replace an existing lane or source alias".to_owned(),
                );
            }
            let dataspace = dataspaces
                .by_id(lane.dataspace_id)
                .ok_or_else(|| "runtime manifest lane has no physical dataspace".to_owned())?;
            let source = Self::prepare_runtime_manifest(
                addition,
                lane,
                dataspace,
                &effective_governance,
                &mut budget,
            )?;
            effective_sources
                .manifests_by_alias
                .insert(lane.alias.clone(), source);
        }
        effective_sources.consensus_policy_digest =
            effective_sources.compute_consensus_policy_digest();
        let mut effective = Arc::new(effective_sources).bind(effective_catalog, governance);
        effective.baseline_source_snapshot = Some(Arc::clone(baseline));
        effective
            .validate_active_coverage_for_catalog(effective_catalog)
            .map_err(|error| error.to_string())?;
        Ok(effective)
    }
}

#[cfg(test)]
mod tests;
