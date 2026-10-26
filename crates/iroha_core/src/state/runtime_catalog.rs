// Committed catalog additions are reconstructed from protected World parameters. Static config
// remains the immutable execution-policy baseline; no filesystem manifest is loaded here.

fn runtime_catalog_invalid(reason: impl std::fmt::Display) -> LaneLifecycleError {
    LaneLifecycleError::RuntimeCatalog(reason.to_string())
}

/// Observe the actual model operation before its protocol scopes begin.
/// Original local resource refusals keep their owner after those scopes retire;
/// semantic errors, intrinsic bounds and foreign scoped errors remain rejection.
pub(crate) fn nexus_catalog_validation_attempt<T>(
    operation: impl FnOnce() -> Result<T, iroha_data_model::nexus::NexusCatalogValidationError>,
) -> Result<
    T,
    crate::execution_attempt::ExecutionAttemptError<
        iroha_data_model::nexus::NexusCatalogValidationError,
    >,
> {
    use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
    use iroha_data_model::nexus::NexusCatalogValidationError;

    match norito::core::classify_decode_attempt(|| match operation() {
        Ok(value) => Ok(Ok(value)),
        Err(error) => match error.into_decode_error() {
            Ok(original) => Err(original),
            Err(completed) => Ok(Err(completed)),
        },
    }) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(completed)) => Err(ExecutionAttemptError::Rejected(completed)),
        Err(original) => Err(canonical_decode_attempt_error(original, |original| {
            NexusCatalogValidationError::Codec(original.into_error())
        })),
    }
}

/// Preserve model codec ownership before a lifecycle diagnostic is rendered.
fn runtime_catalog_try<T>(
    operation: impl FnOnce() -> Result<T, iroha_data_model::nexus::NexusCatalogValidationError>,
) -> Result<T, LaneLifecycleError> {
    nexus_catalog_validation_attempt(operation).map_err(|error| match error {
        crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
            LaneLifecycleError::ParameterDeferred(original)
        }
        crate::execution_attempt::ExecutionAttemptError::Rejected(completed) => {
            runtime_catalog_invalid(completed)
        }
    })
}

/// Reconstruct the actual registry while retaining original local codec failures.
/// The observer begins before the loader's bounded parser and writer scopes.
fn runtime_catalog_manifest_registry(
    baseline: &LaneManifestRegistry,
    additions: &[iroha_data_model::nexus::RuntimeLaneManifestV1],
    lanes: &LaneCatalog,
    dataspaces: &DataSpaceCatalog,
    governance: &iroha_config::parameters::actual::GovernanceCatalog,
) -> Result<LaneManifestRegistry, LaneLifecycleError> {
    use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
    use crate::governance::manifest::LaneManifestError;

    match norito::core::classify_decode_attempt(|| {
        match baseline.with_runtime_additions(additions, lanes, dataspaces, governance) {
            Ok(registry) => Ok(Ok(registry)),
            Err(error) => match error.into_decode_error() {
                Ok(original) => Err(original),
                Err(completed) => Ok(Err(completed)),
            },
        }
    }) {
        Ok(Ok(registry)) => Ok(registry),
        Ok(Err(completed)) => Err(runtime_catalog_invalid(completed)),
        Err(original) => match canonical_decode_attempt_error(original, |original| {
            LaneManifestError::Codec(original.into_error())
        }) {
            ExecutionAttemptError::Deferred(original) => {
                Err(LaneLifecycleError::ParameterDeferred(original))
            }
            ExecutionAttemptError::Rejected(completed) => Err(runtime_catalog_invalid(completed)),
        },
    }
}

/// Read the protected cumulative catalog without treating malformed state as absent.
pub(crate) fn runtime_catalog_from_world(
    world: &impl WorldReadOnly,
) -> Result<Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>, LaneLifecycleError> {
    runtime_catalog_from_parameters(world.parameters())
}

/// Decode the protected catalog from an explicitly owned parameter version.
fn runtime_catalog_from_parameters(
    parameters: &Parameters,
) -> Result<Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>, LaneLifecycleError> {
    use iroha_data_model::nexus::NexusRuntimeCatalogV1;
    let Some(custom) = parameters
        .custom()
        .get(&NexusRuntimeCatalogV1::parameter_id())
    else {
        return Ok(None);
    };
    runtime_catalog_try(|| NexusRuntimeCatalogV1::from_custom_parameter(custom))?
        .map(Some)
        .ok_or_else(|| {
            runtime_catalog_invalid("protected runtime catalog parameter identity mismatch")
        })
}

/// Commit the complete validated protected catalog, or return exact absence before its first use.
pub(crate) fn runtime_catalog_root_from_world(
    world: &impl WorldReadOnly,
) -> Result<Option<Hash>, LaneLifecycleError> {
    runtime_catalog_from_world(world)?
        .map(|catalog| runtime_catalog_try(|| catalog.canonical_hash()))
        .transpose()
}

impl StateView<'_> {
    /// Return the native commitment to this snapshot's committed runtime catalog overlay.
    ///
    /// `None` denotes exact absence of the protected parameter before its first transition.
    /// Read this from the same view as the lane catalog and incarnation commitments when
    /// constructing an optimistic catalog transition.
    ///
    /// # Errors
    /// Returns an error when the protected parameter is malformed or cannot be canonically hashed.
    pub fn runtime_catalog_hash(&self) -> Result<Option<Hash>, LaneLifecycleError> {
        runtime_catalog_root_from_world(self.world())
    }
}

/// Reconstruct effective physical dataspaces from an immutable baseline and committed additions.
pub(crate) fn runtime_catalog_dataspaces(
    baseline: &DataSpaceCatalog,
    runtime: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
) -> Result<DataSpaceCatalog, LaneLifecycleError> {
    let Some(runtime) = runtime else {
        return Ok(baseline.clone());
    };
    runtime_catalog_try(|| runtime.validate_structure())?;
    if runtime.baseline_dataspaces_hash != iroha_data_model::nexus::dataspace_catalog_hash(baseline)
    {
        return Err(runtime_catalog_invalid(
            "configured dataspace baseline differs from committed catalog authority",
        ));
    }
    let mut entries = baseline.entries().to_vec();
    entries.extend(
        runtime
            .dataspaces
            .iter()
            .map(|addition| addition.descriptor.clone()),
    );
    DataSpaceCatalog::new(entries).map_err(runtime_catalog_invalid)
}

fn validate_runtime_catalog_committee(
    world: &impl WorldReadOnly,
    network_id: &iroha_data_model::NetworkId,
    nexus: &iroha_config::parameters::actual::Nexus,
    manifests: &LaneManifestRegistry,
    lane: &iroha_data_model::nexus::LaneConfig,
    authority_height: u64,
) -> Result<(), LaneLifecycleError> {
    let rules = manifests.lane_rules(lane.id).ok_or_else(|| {
        runtime_catalog_invalid(format!("lane {} has no accepted runtime manifest", lane.id))
    })?;
    for binding in &rules.validator_bindings {
        if world.accounts().get(&binding.validator).is_none() {
            return Err(runtime_catalog_invalid(format!(
                "lane {} manifest authority account is not registered",
                lane.id
            )));
        }
        if !world.peers().iter().any(|peer| peer == &binding.peer_id)
            || !peer_has_live_consensus_key_for_lane(
                world,
                &binding.peer_id,
                authority_height,
                lane.id,
            )
        {
            return Err(runtime_catalog_invalid(format!(
                "lane {} manifest peer has no registered live consensus key at activation",
                lane.id
            )));
        }
        let pop = live_consensus_key_pop_for_peer_on_lane(
            world,
            &binding.peer_id,
            authority_height,
            lane.id,
        )
        .ok_or_else(|| {
            runtime_catalog_invalid(format!(
                "lane {} manifest peer has no live proof of possession",
                lane.id
            ))
        })?;
        iroha_crypto::bls_normal_pop_verify(binding.peer_id.public_key(), &pop).map_err(|_| {
            runtime_catalog_invalid(format!(
                "lane {} manifest peer has an invalid BLS proof of possession",
                lane.id
            ))
        })?;
    }
    // This enforces exact route ownership and the DS-derived 3f+1 authority using the same
    // selection path as live lane work, including any required beacon for an oversized pool.
    lane_authority::resolve_from_sources(
        world,
        network_id,
        lane_authority::LaneAuthorityRoute::new(lane.id, lane.dataspace_id),
        manifests,
        nexus,
        authority_height,
    )
    .map_err(runtime_catalog_invalid)?;
    Ok(())
}

/// Validate an additive catalog against the pre-transition authenticated state.
///
/// The caller separately binds `pending` to the accepted block's protected parameter and checks
/// its newly added committees against that block's post-execution live key registry.
pub(crate) fn runtime_catalog_transition_dataspaces(
    old_nexus: &iroha_config::parameters::actual::Nexus,
    old_registry: &LaneManifestRegistry,
    old_world: &impl WorldReadOnly,
    pending: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
    plan: &iroha_data_model::nexus::LaneLifecyclePlan,
) -> Result<DataSpaceCatalog, LaneLifecycleError> {
    runtime_catalog_transition_dataspaces_from_parameters(
        old_nexus,
        old_registry,
        old_world.parameters(),
        pending,
        plan,
    )
}

/// Validate against the retained parameter preimage, including a replaced tip's undo.
fn runtime_catalog_transition_dataspaces_from_parameters(
    old_nexus: &iroha_config::parameters::actual::Nexus,
    old_registry: &LaneManifestRegistry,
    original_parameters: &Parameters,
    pending: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
    plan: &iroha_data_model::nexus::LaneLifecyclePlan,
) -> Result<DataSpaceCatalog, LaneLifecycleError> {
    runtime_catalog_try(|| pending.validate_structure())?;
    if plan.additions.is_empty() || !plan.retire.is_empty() {
        return Err(runtime_catalog_invalid(
            "runtime catalog transition must add lanes without retiring existing lanes",
        ));
    }
    let previous = runtime_catalog_from_parameters(original_parameters)?;
    let baseline_dataspaces_hash =
        iroha_data_model::nexus::dataspace_catalog_hash(&old_nexus.configured_dataspace_catalog);
    let baseline_manifests_hash = Hash::prehashed(old_registry.baseline_consensus_policy_digest());
    if pending.baseline_dataspaces_hash != baseline_dataspaces_hash
        || pending.baseline_manifests_hash != baseline_manifests_hash
        || previous.as_ref().is_some_and(|previous| {
            previous.baseline_dataspaces_hash != baseline_dataspaces_hash
                || previous.baseline_manifests_hash != baseline_manifests_hash
        })
    {
        return Err(runtime_catalog_invalid(
            "immutable catalog baseline differs from committed runtime authority",
        ));
    }
    let current_dataspaces =
        runtime_catalog_dataspaces(&old_nexus.configured_dataspace_catalog, previous.as_ref())?;
    if current_dataspaces != old_nexus.dataspace_catalog {
        return Err(runtime_catalog_invalid(
            "effective dataspace catalog differs from its committed baseline and additions",
        ));
    }
    let previous_dataspaces = previous
        .as_ref()
        .map_or(&[][..], |catalog| catalog.dataspaces.as_slice());
    let previous_manifests = previous
        .as_ref()
        .map_or(&[][..], |catalog| catalog.manifests.as_slice());
    if previous_dataspaces
        .iter()
        .any(|addition| !pending.dataspaces.contains(addition))
        || previous_manifests
            .iter()
            .any(|manifest| !pending.manifests.contains(manifest))
    {
        return Err(runtime_catalog_invalid(
            "runtime catalog transition removes or changes a committed addition",
        ));
    }
    for addition in pending
        .dataspaces
        .iter()
        .filter(|addition| !previous_dataspaces.contains(addition))
    {
        if current_dataspaces.by_id(addition.descriptor.id).is_some()
            || current_dataspaces
                .by_alias(&addition.descriptor.alias)
                .is_some()
        {
            return Err(runtime_catalog_invalid(
                "runtime dataspace addition attempts to replace an existing ID or alias",
            ));
        }
    }
    let addition_ids: BTreeSet<_> = plan.additions.iter().map(|lane| lane.id).collect();
    let addition_aliases: BTreeSet<_> = plan.additions.iter().map(|lane| &lane.alias).collect();
    if addition_ids.len() != plan.additions.len()
        || addition_aliases.len() != plan.additions.len()
        || plan.additions.iter().any(|addition| {
            old_nexus
                .lane_catalog
                .lanes()
                .iter()
                .any(|lane| lane.id == addition.id || lane.alias == addition.alias)
        })
    {
        return Err(runtime_catalog_invalid(
            "runtime lane addition duplicates or replaces an existing ID or alias",
        ));
    }
    let new_manifest_ids: BTreeSet<_> = pending
        .manifests
        .iter()
        .filter(|manifest| !previous_manifests.contains(manifest))
        .map(|manifest| manifest.lane_id)
        .collect();
    if new_manifest_ids != addition_ids {
        return Err(runtime_catalog_invalid(
            "each added lane must have exactly one matching new manifest",
        ));
    }
    runtime_catalog_dataspaces(&old_nexus.configured_dataspace_catalog, Some(pending))
}

/// Admit both protected successors before any catalog, parameter or geometry mutation.
/// The second successor is rebound to the exact original retained source; an intermediate
/// graph is never published, and every original charge is released on either refusal.
fn prepare_runtime_catalog_parameters(
    current: &crate::pipeline_gas_policy::RetainedStateParameters,
    runtime: &iroha_data_model::parameter::CustomParameter,
    native: &iroha_data_model::parameter::CustomParameter,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<crate::pipeline_gas_policy::PreparedParameters, LaneLifecycleError> {
    use crate::pipeline_gas_policy::{ParameterIngressAttemptError, ParameterIngressError};
    ParameterIngressAttemptError::observe(|| {
        let first = current.prepare_custom_parameter(runtime, budget)?;
        let intermediate = first
            .into_successor(current, budget)
            .map_err(|(_prepared, original)| ParameterIngressError::from(original))?;
        let second = intermediate.prepare_custom_parameter(native, budget)?;
        let next = second
            .into_successor(&intermediate, budget)
            .map_err(|(_prepared, original)| ParameterIngressError::from(original))?;
        current
            .prepare_admitted(next, budget)
            .map_err(|(_next, original)| ParameterIngressError::from(original))
    })
    .map_err(|actual| {
        match crate::state::parameter_ingress_attempt_error(actual, runtime_catalog_invalid) {
            crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
                LaneLifecycleError::ParameterDeferred(original)
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(completed) => completed,
        }
    })
}

impl StateTransaction<'_, '_> {
    /// Prepare the native lane policy from the same authenticated manifest authority as the
    /// physical catalog. Nothing is staged until every lane and live `PoP` has been admitted.
    fn runtime_catalog_native_lane_policy(
        &self,
        additions: &[iroha_data_model::nexus::LaneConfig],
        manifests: &LaneManifestRegistry,
        authority_height: u64,
    ) -> Result<iroha_data_model::sumeragi_lanes::SumeragiLanePolicy, LaneLifecycleError> {
        use iroha_data_model::sumeragi_lanes::{
            SumeragiFixedLane, SumeragiLaneMember, SumeragiLanePolicy,
        };

        let mut policy = match self
            .world
            .parameters()
            .custom()
            .get(&SumeragiLanePolicy::parameter_id())
        {
            Some(custom) => SumeragiLanePolicy::from_custom_parameter(custom)
                .ok_or_else(|| {
                    runtime_catalog_invalid("native lane policy has another parameter identity")
                })?
                .map_err(runtime_catalog_invalid)?,
            None => SumeragiLanePolicy::for_chain(
                self.world.parameters().sumeragi.clone(),
                self.world
                    .consensus_schedule()
                    .ready(self.block_height())
                    .map_err(runtime_catalog_invalid)?
                    .epoch
                    .da_layout,
            ),
        };
        for lane in additions {
            if lane.id == LaneId::SINGLE
                || policy.fixed_lane(lane.id).is_some()
                || policy.is_elastic(lane.id)
                || self.world.sumeragi_lanes().lane(lane.id).is_some()
            {
                return Err(runtime_catalog_invalid(
                    "runtime catalog lane conflicts with an existing native lane or elastic range",
                ));
            }
            let rules = manifests.lane_rules(lane.id).ok_or_else(|| {
                runtime_catalog_invalid("runtime catalog lane has no native committee manifest")
            })?;
            let mut committee = rules
                .validator_bindings
                .iter()
                .map(|binding| {
                    let pop = live_consensus_key_pop_for_peer_on_lane(
                        &self.world,
                        &binding.peer_id,
                        authority_height,
                        lane.id,
                    )
                    .ok_or_else(|| {
                        runtime_catalog_invalid(
                            "runtime catalog lane has no live committee proof of possession",
                        )
                    })?;
                    Ok(SumeragiLaneMember {
                        peer: binding.peer_id.clone(),
                        pop,
                    })
                })
                .collect::<Result<Vec<_>, LaneLifecycleError>>()?;
            committee
                .sort_by_key(|member| crate::sumeragi::schedule::consensus_key(&member.peer).ok());
            policy.fixed.push(SumeragiFixedLane {
                lane: lane.id,
                dataspace: lane.dataspace_id,
                committee,
            });
            policy.fixed.sort_by_key(|fixed| fixed.lane);
        }
        crate::sumeragi::lanes::step::validate_policy(&policy).map_err(runtime_catalog_invalid)?;
        Ok(policy)
    }

    /// Atomically stage one authorized additive physical DS/lane/manifest catalog transition.
    ///
    /// Every fallible preparation step precedes mutation of this transaction overlay. The parent
    /// block publishes the prepared geometry and registry only after normal accepted execution.
    pub(crate) fn stage_consensus_catalog_transition(
        &mut self,
        payload: &iroha_data_model::nexus::NexusCatalogTransitionV1,
    ) -> Result<(), LaneLifecycleError> {
        let prepared = self.prepare_consensus_catalog_transition(payload)?;
        self.publish_consensus_catalog_transition(prepared);
        Ok(())
    }

    /// Construct the complete successor without mutating this transaction's cut.
    fn prepare_consensus_catalog_transition(
        &self,
        payload: &iroha_data_model::nexus::NexusCatalogTransitionV1,
    ) -> Result<PreparedConsensusCatalogTransition, LaneLifecycleError> {
        use iroha_data_model::nexus::NexusRuntimeCatalogV1;

        runtime_catalog_try(|| payload.validate_structure())?;
        crate::sumeragi::private_dataspace::ensure_parent_execution_separate(
            &self.world,
            payload
                .dataspace_additions
                .iter()
                .map(|entry| entry.descriptor.id)
                .chain(payload.lane_additions.iter().map(|lane| lane.dataspace_id)),
        )
        .map_err(runtime_catalog_invalid)?;
        let block_height = self.block_height();
        if block_height <= 1 {
            return Err(runtime_catalog_invalid(
                "runtime catalog transitions require committed post-genesis history",
            ));
        }
        let authority_height = block_height.checked_add(1).ok_or_else(|| {
            runtime_catalog_invalid("runtime catalog activation height overflows")
        })?;
        if self.lane_lifecycle_already_staged_in_block || self.pending_lane_lifecycle.is_some() {
            return Err(LaneLifecycleError::LifecycleAlreadyStaged);
        }
        if payload.lane_additions.is_empty() {
            return Err(runtime_catalog_invalid(
                "runtime catalog transition requires at least one new lane",
            ));
        }
        let actual_catalog_hash = LaneLifecycleParameterV1::catalog_hash(&self.nexus.lane_catalog);
        if payload.expected_catalog_hash != actual_catalog_hash {
            return Err(LaneLifecycleError::StaleCatalog {
                expected: payload.expected_catalog_hash,
                actual: actual_catalog_hash,
            });
        }
        let actual_incarnation_root =
            lane_lifecycle_incarnation_root(&self.nexus.lane_catalog, &self.lane_incarnations)?;
        if payload.expected_incarnation_root != actual_incarnation_root {
            return Err(LaneLifecycleError::StaleIncarnationRoot {
                expected: payload.expected_incarnation_root,
                actual: actual_incarnation_root,
            });
        }
        let previous_runtime = runtime_catalog_from_world(&self.world)?;
        let actual_runtime_hash = previous_runtime
            .as_ref()
            .map(|catalog| runtime_catalog_try(|| catalog.canonical_hash()))
            .transpose()?;
        if payload.expected_runtime_catalog_hash != actual_runtime_hash {
            return Err(runtime_catalog_invalid(
                "expected runtime catalog root differs from current committed state",
            ));
        }
        let baseline_dataspaces_hash = iroha_data_model::nexus::dataspace_catalog_hash(
            &self.nexus.configured_dataspace_catalog,
        );
        let baseline_manifests_hash =
            Hash::prehashed(self.lane_manifests.baseline_consensus_policy_digest());
        let current_dataspaces = runtime_catalog_dataspaces(
            &self.nexus.configured_dataspace_catalog,
            previous_runtime.as_ref(),
        )?;
        if current_dataspaces != self.nexus.dataspace_catalog {
            return Err(runtime_catalog_invalid(
                "effective dataspace catalog differs from its committed baseline and additions",
            ));
        }
        let mut runtime = previous_runtime.unwrap_or_else(|| NexusRuntimeCatalogV1 {
            version: NexusRuntimeCatalogV1::VERSION,
            baseline_dataspaces_hash,
            baseline_manifests_hash,
            dataspaces: Vec::new(),
            manifests: Vec::new(),
        });
        if runtime.baseline_dataspaces_hash != baseline_dataspaces_hash
            || runtime.baseline_manifests_hash != baseline_manifests_hash
        {
            return Err(runtime_catalog_invalid(
                "immutable catalog baseline differs from committed runtime authority",
            ));
        }
        for addition in &payload.dataspace_additions {
            if current_dataspaces.by_id(addition.descriptor.id).is_some()
                || current_dataspaces
                    .by_alias(&addition.descriptor.alias)
                    .is_some()
            {
                return Err(runtime_catalog_invalid(
                    "runtime dataspace addition attempts to replace an existing ID or alias",
                ));
            }
        }
        let addition_ids: BTreeSet<_> = payload.lane_additions.iter().map(|lane| lane.id).collect();
        let manifest_ids: BTreeSet<_> = payload
            .manifest_additions
            .iter()
            .map(|manifest| manifest.lane_id)
            .collect();
        if addition_ids != manifest_ids {
            return Err(runtime_catalog_invalid(
                "each added lane must have exactly one matching new manifest",
            ));
        }
        for addition in &payload.lane_additions {
            if self
                .nexus
                .lane_catalog
                .lanes()
                .iter()
                .any(|lane| lane.id == addition.id || lane.alias == addition.alias)
            {
                return Err(runtime_catalog_invalid(
                    "runtime lane addition attempts to replace an existing ID or alias",
                ));
            }
        }
        runtime
            .dataspaces
            .extend(payload.dataspace_additions.iter().cloned());
        runtime.dataspaces.sort_by_key(|entry| entry.descriptor.id);
        runtime
            .manifests
            .extend(payload.manifest_additions.iter().cloned());
        runtime.manifests.sort_by_key(|entry| entry.lane_id);
        runtime_catalog_try(|| runtime.validate_structure())?;
        let plan = LaneLifecyclePlan {
            additions: payload.lane_additions.clone(),
            retire: Vec::new(),
        };
        let updated_dataspaces = runtime_catalog_transition_dataspaces(
            &self.nexus,
            &self.lane_manifests,
            &self.world,
            &runtime,
            &plan,
        )?;
        let runtime_parameter = runtime_catalog_try(|| runtime.clone().into_custom_parameter())?;
        let mut prospective_nexus = self.nexus.clone();
        prospective_nexus.dataspace_catalog = updated_dataspaces.clone();
        ensure_lane_lifecycle_compliance_ready(
            &prospective_nexus,
            self.lane_compliance.as_deref(),
            &plan,
        )?;
        let mut lifecycle_update = prepare_lane_lifecycle_update(
            &prospective_nexus,(&self.lane_incarnations, &self.lane_incarnation_lineage, &self.lane_incarnation_activation_heights),
            &self.network_id,
            self._curr_block.hash(),
            &plan,
            block_height,
            false,
        )?;
        lifecycle_update.previous_dataspace_catalog = self.nexus.dataspace_catalog.clone();
        lifecycle_update.updated_dataspace_catalog = updated_dataspaces;
        prospective_nexus.lane_catalog = lifecycle_update.updated_catalog.clone();
        prospective_nexus.lane_config = lifecycle_update.updated_lane_config.clone();
        ensure_homed_dataspaces_keep_lanes(&self.world, &lifecycle_update.updated_catalog)?;
        ensure_live_shared_dataspace_staking_owner_is_not_reset(
            &self.world,
            &self.nexus,
            &prospective_nexus,
            &lifecycle_update.lanes_to_reset,
            block_height,
        )?;
        let updated_lane_manifests = Arc::new(runtime_catalog_manifest_registry(
            self.lane_manifests.as_ref(),
            &runtime.manifests,
            &prospective_nexus.lane_catalog,
            &prospective_nexus.dataspace_catalog,
            &prospective_nexus.governance,
        )?);
        for addition in &payload.lane_additions {
            validate_runtime_catalog_committee(
                &self.world,
                &self.network_id,
                &prospective_nexus,
                &updated_lane_manifests,
                addition,
                authority_height,
            )?;
        }

        let native_policy = self.runtime_catalog_native_lane_policy(
            &payload.lane_additions,
            &updated_lane_manifests,
            block_height.checked_add(2).ok_or_else(|| {
                runtime_catalog_invalid("native lane activation height overflows")
            })?,
        )?;

        let pool = self.world.sccp_committee_generations.allocation_budget();
        let prepared = prepare_runtime_catalog_parameters(
            self.world.parameters.get(),
            &runtime_parameter,
            &native_policy.into_custom_parameter(),
            pool,
        )?;
        let parameters = prepared
            .into_successor(self.world.parameters.get(), pool)
            .map_err(|(_prepared, _original)| {
                LaneLifecycleError::ParameterDeferred(
                    ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
                )
            })?;
        // All retained projections are constructed before the first mutation.
        // TODO: admit the ordinary catalog/manifest/map/snapshot allocations through
        // their concrete owners; this boundary only makes publication allocation-free.
        let axt_authorization_transitioned = self.world.prepare_axt_lane_incarnation_transitions(
            &self.lane_incarnations,
            &lifecycle_update.updated_lane_incarnations,
        );
        let dataspace_catalog = prospective_nexus.dataspace_catalog.clone();
        let lane_privacy_registry = LanePrivacyRegistry::try_from_manifest_registry(
            updated_lane_manifests.as_ref(),
            pool,
        ).map_err(LaneLifecycleError::ParameterDeferred)?;
        let lane_manifests = Arc::clone(&updated_lane_manifests);
        let lane_incarnations = lifecycle_update.updated_lane_incarnations.clone();
        let lane_incarnation_lineage = lifecycle_update.updated_lane_incarnation_lineage.clone();
        let lane_incarnation_activation_heights = lifecycle_update
            .updated_lane_incarnation_activation_heights
            .clone();
        let samples = self.canonical_runtime.autoscale_sample_history.iter().copied().collect();
        let canonical_runtime = SnapshotNexusRuntime::from_nexus_with_autoscale_history(
            &prospective_nexus,
            &lane_incarnations,
            &lane_incarnation_activation_heights,
            &samples,
            &lane_incarnation_lineage,
        );
        let canonical_runtime = (self.canonical_runtime.get() != &canonical_runtime)
            .then_some(canonical_runtime);
        let pending_lane_lifecycle = PendingAutoscaleLaneLifecycle {
            catalog_update: lifecycle_update,
            updated_lane_manifests,
            updated_lane_privacy: lane_privacy_registry.clone(),
            plan,
            transition: PendingAutoscaleTransition::Manual,
            transition_height: block_height,
            expected_incarnation_root: payload.expected_incarnation_root,
            runtime_catalog: Some(runtime),
        };

        Ok(PreparedConsensusCatalogTransition {
            axt_authorization_transitioned,
            dataspace_catalog,
            parameters,
            nexus: prospective_nexus,
            lane_manifests,
            lane_privacy_registry,
            lane_incarnations,
            lane_incarnation_lineage,
            lane_incarnation_activation_heights,
            pending_lane_lifecycle,
            canonical_runtime,
        })
    }

    /// Move the complete successor into the same transaction without allocation.
    fn publish_consensus_catalog_transition(&mut self, prepared: PreparedConsensusCatalogTransition) {
        self.world.axt_authorization_transitioned = prepared.axt_authorization_transitioned;
        self.world.dataspace_catalog = prepared.dataspace_catalog;
        self.world.parameters.replace(prepared.parameters);
        self.nexus = prepared.nexus;
        self.lane_manifests = prepared.lane_manifests;
        self.lane_privacy_registry = prepared.lane_privacy_registry;
        self.lane_incarnations = prepared.lane_incarnations;
        self.lane_incarnation_lineage = prepared.lane_incarnation_lineage;
        self.lane_incarnation_activation_heights = prepared.lane_incarnation_activation_heights;
        self.pending_lane_lifecycle = Some(prepared.pending_lane_lifecycle);
        if let Some(canonical_runtime) = prepared.canonical_runtime {
            self.canonical_runtime.replace(canonical_runtime);
        }
    }
}

/// Concrete outputs of catalog preparation, consumed immediately by its publisher.
/// Parameter graph charges remain attached to the original retained successor.
struct PreparedConsensusCatalogTransition {
    axt_authorization_transitioned: BTreeSet<DataSpaceId>,
    dataspace_catalog: DataSpaceCatalog,
    parameters: RetainedStateParameters,
    nexus: iroha_config::parameters::actual::Nexus,
    lane_manifests: LaneManifestRegistryHandle,
    lane_privacy_registry: LanePrivacyRegistry,
    lane_incarnations: BTreeMap<LaneId, Hash>,
    lane_incarnation_lineage: BTreeMap<LaneId, LaneIncarnationLineage>,
    lane_incarnation_activation_heights: BTreeMap<LaneId, u64>,
    pending_lane_lifecycle: PendingAutoscaleLaneLifecycle,
    canonical_runtime: Option<SnapshotNexusRuntime>,
}

#[cfg(test)]
pub(crate) mod runtime_catalog_tests {
    use super::*;
    include!("runtime_catalog_tests.rs");
}
