/// One complete, collision-checked recovery inventory indexed once for an
/// immutable startup authority boundary.
struct HistoricalAutonomousRecoveryInventory {
    records: Vec<HistoricalAutonomousLaneRecoveryRecordV1>,
    by_recovery_id: BTreeMap<Hash, usize>,
    by_group: BTreeMap<LaneQueueReservationGroupIdentityV1, usize>,
}

impl HistoricalAutonomousRecoveryInventory {
    fn read(kura: &Kura) -> Result<Self, V2ReservationLifecycleError> {
        let records = kura.historical_autonomous_lane_recovery_records_bounded(
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
        )?;
        let mut by_recovery_id = BTreeMap::new();
        let mut by_group = BTreeMap::new();
        for (index, record) in records.iter().enumerate() {
            if by_recovery_id.insert(record.recovery_id, index).is_some()
                || by_group
                    .insert(record.reservation_group.identity, index)
                    .is_some()
            {
                return Err(invalid_historical_autonomous_recovery(
                    &record.installation_input(),
                    "bounded historical recovery inventory returned a duplicate identity",
                ));
            }
        }
        Ok(Self {
            records,
            by_recovery_id,
            by_group,
        })
    }

    fn record_for_group(
        &self,
        group: &LaneQueueReservationReconciliationGroupV1,
    ) -> Result<Option<&HistoricalAutonomousLaneRecoveryRecordV1>, V2ReservationLifecycleError>
    {
        let Some(record) = self
            .by_group
            .get(&group.identity)
            .and_then(|index| self.records.get(*index))
        else {
            return Ok(None);
        };
        if record.reservation_group != *group {
            return Err(invalid_historical_autonomous_recovery(
                &record.installation_input(),
                "durable historical recovery has conflicting FIFO group membership",
            ));
        }
        Ok(Some(record))
    }

    fn record_for_install(
        &self,
        install: &HistoricalAutonomousReservationInstallV1,
    ) -> Result<Option<&HistoricalAutonomousLaneRecoveryRecordV1>, V2ReservationLifecycleError>
    {
        let Some(record) = self
            .by_recovery_id
            .get(&install.recovery_id)
            .and_then(|index| self.records.get(*index))
        else {
            return Ok(None);
        };
        if record.installation_input() != *install {
            return Err(invalid_historical_autonomous_recovery(
                install,
                "durable historical recovery conflicts with the requested installation",
            ));
        }
        Ok(Some(record))
    }

    fn exact_record(
        &self,
        expected: &HistoricalAutonomousLaneRecoveryRecordV1,
    ) -> Result<Option<&HistoricalAutonomousLaneRecoveryRecordV1>, V2ReservationLifecycleError>
    {
        let Some(record) = self
            .by_recovery_id
            .get(&expected.recovery_id)
            .and_then(|index| self.records.get(*index))
        else {
            return Ok(None);
        };
        if record != expected {
            return Err(invalid_historical_autonomous_recovery(
                &expected.installation_input(),
                "durable historical recovery conflicts with the expected canonical record",
            ));
        }
        Ok(Some(record))
    }
}

fn historical_autonomous_install_is_durable(
    kura: &Kura,
    inventory: &HistoricalAutonomousRecoveryInventory,
    install: &HistoricalAutonomousReservationInstallV1,
) -> Result<bool, V2ReservationLifecycleError> {
    let Some(record) = inventory.record_for_install(install)? else {
        return Ok(false);
    };
    kura.validate_historical_autonomous_lane_recovery_record_dependencies(record)?;
    Ok(true)
}

/// Rebuild the complete State-aligned authority of one historical autonomous
/// installation. The carrier body is required only at the one-time installer
/// boundary; immutable record validation and hydration deliberately use the
/// retained header/finality/length authorities after canonical-body pruning.
fn preflight_historical_autonomous_lane_recovery_inner(
    state: &State,
    kura: &Kura,
    input: &HistoricalAutonomousReservationInstallV1,
    require_canonical_carrier_body: bool,
    retained_record: Option<&HistoricalAutonomousLaneRecoveryRecordV1>,
) -> Result<HistoricalAutonomousLaneRecoveryRecordV1, V2ReservationLifecycleError> {
    let descriptor = &input.payload.origin_proposal.descriptor;
    let identity = &input.reservation_group.identity;
    let height = input.canonical_body.height;
    if !input.has_valid_identity()
        || height == 0
        || input.historical_context.validate().is_err()
        || input.historical_context.height != height
        || input.historical_context.id() != input.historical_context_id
        || HashOf::new(&input.historical_context) != input.historical_context_hash
        || input.canonical_body.executed_block_wire_len == 0
        || input.canonical_body.executed_block_wire_len > crate::kura::STRICT_INIT_MAX_BLOCK_BYTES
        || input
            .canonical_body
            .execution_commitment
            .validate()
            .is_err()
        || input.canonical_body.executed_block_wire_len
            != input
                .canonical_body
                .execution_commitment
                .executed_block_wire_len
        || input.canonical_body.executed_block_wire_hash
            != input
                .canonical_body
                .execution_commitment
                .executed_block_wire_hash
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "installation identity, protocol context, or signed wire commitment is invalid",
        ));
    }

    let state_height = u64::try_from(state.committed_height())?;
    if state_height < height
        || state.committed_block_hash_at_height(height) != Some(input.canonical_body.block_hash)
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "State does not retain the exact committed carrier hash",
        ));
    }
    let expected_parent = height
        .checked_sub(1)
        .filter(|parent_height| *parent_height != 0)
        .and_then(|parent_height| state.committed_block_hash_at_height(parent_height));
    let (retained_header, finality) = kura
        .v2_finality_artifact_with_header(height)?
        .ok_or(V2ReservationLifecycleError::MissingCanonicalFinality { height })?;
    let state_context = if retained_record.is_none() {
        state.sumeragi_v2_height_context(height).map_err(|error| {
            invalid_historical_autonomous_recovery(
                input,
                format!("State historical context is unreadable: {error}"),
            )
        })?
    } else {
        None
    };
    if retained_header.height().get() != height
        || retained_header.hash() != input.canonical_body.block_hash
        || retained_header.prev_block_hash() != expected_parent
        || finality.height != height
        || finality.block_hash != input.canonical_body.block_hash
        || finality.height_context != input.historical_context
        || HashOf::new(&finality) != input.canonical_body.finality_artifact_hash
        || finality.commit_qc.execution_commitment != input.canonical_body.execution_commitment
        || finality.verify().is_err()
        || finality.validate_for_header(&retained_header).is_err()
        || kura.durable_block_payload_len_by_hash(input.canonical_body.block_hash)
            != Some((height, input.canonical_body.executed_block_wire_len))
        || (retained_record.is_none() && state_context.as_ref() != Some(&input.historical_context))
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "retained header, parent, finality, State context, or durable wire length conflicts",
        ));
    }

    let network_id = input.historical_context.network_id;
    let expected_epoch = input.historical_context.epoch;
    if retained_record.is_none() {
        let world = state.world_view();
        if crate::sumeragi::epoch_for_height_from_world(&world, height) != expected_epoch {
            return Err(invalid_historical_autonomous_recovery(
                input,
                "State historical epoch differs from the retained finality context",
            ));
        }
    }
    let hint = input
        .payload
        .origin_proposal
        .payload_block_hint
        .ok_or_else(|| {
            invalid_historical_autonomous_recovery(
                input,
                "historical payload has no exact canonical carrier hint",
            )
        })?;
    if input.payload.network_id != network_id
        || input.payload.epoch != expected_epoch
        || descriptor.proposal_height != height
        || descriptor.lane_id != identity.lane_id
        || descriptor.dataspace_id != identity.dataspace_id
        || descriptor.lane_incarnation != identity.lane_incarnation
        || descriptor.lane_block_height != identity.lane_block_height
        || descriptor.lane_block_view != identity.lane_block_view
        || descriptor.lane_block_view != 0
        || hint.proposal_height != height
        || hint.proposal_view != input.carrier_view
        || hint.proposal_block_hash != input.canonical_body.block_hash
        || input.payload.reservation_keys != input.reservation_group.ordered_keys
        || input.payload.validate(network_id, expected_epoch).is_err()
        || (retained_record.is_none()
            && (!state.lane_route_and_incarnation_active_at_height(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                height,
            ) || !state
                .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
                    &input.payload.origin_proposal,
                )))
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "payload, route/incarnation, carrier hint, or predecessor authority conflicts",
        ));
    }

    let mut expected_validators = if retained_record.is_some() {
        descriptor.validator_set.clone()
    } else {
        let nexus = state.nexus_snapshot();
        if !nexus.enabled || !super::lane_planner::proposal_lookahead_enabled(&nexus, height) {
            input
                .historical_context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        } else {
            state.authoritative_lane_peer_ids_at_height(descriptor.lane_id, height)
        }
    };
    expected_validators.sort();
    if expected_validators
        .windows(2)
        .any(|pair| pair[0] == pair[1])
        || expected_validators.is_empty()
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "State-aligned historical lane committee is empty or duplicated",
        ));
    }
    let validator_count = u32::try_from(expected_validators.len())?;
    let min_quorum = u32::try_from(
        super::network_topology::commit_quorum_from_len(expected_validators.len()).max(1),
    )?;
    let base_mode_tag = match input.historical_context.mode {
        wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
        wire::ConsensusMode::Npos => wire::NPOS_TAG,
    };
    let context_mode_tag = format!(
        "{base_mode_tag}::height-context:{}::epoch:{}",
        hex::encode(input.historical_context_id.0.as_ref()),
        expected_epoch
    );
    let expected_qc_mode_tag = LaneRelayEnvelope::lane_qc_mode_tag_for(
        descriptor.lane_id,
        descriptor.dataspace_id,
        &context_mode_tag,
    );
    let expected_author =
        deterministic_lane_author(&expected_validators, descriptor.lane_block_height).ok_or_else(
            || {
                invalid_historical_autonomous_recovery(
                    input,
                    "deterministic historical autonomous author is unavailable",
                )
            },
        )?;
    if descriptor.validator_set_hash_version
        != iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1
        || descriptor.validator_set != expected_validators
        || descriptor.validator_set_hash != HashOf::new(&expected_validators)
        || descriptor.validator_count != validator_count
        || descriptor.min_quorum != min_quorum
        || descriptor.qc_mode_tag != expected_qc_mode_tag
        || &input.payload.producer != expected_author
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "historical committee, quorum, QC domain, or deterministic author conflicts",
        ));
    }

    if input.reservation_group.ordered_keys.is_empty()
        || input.reservation_group.ordered_keys.len()
            > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "historical reservation group is empty or exceeds its hard bound",
        ));
    }
    let (reservation_owner_hash, proposal_identity_hash) =
        super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            input.historical_context_id,
            expected_epoch,
            &input.payload.origin_proposal,
            expected_author,
        )
        .map_err(|error| invalid_historical_autonomous_recovery(input, error.to_string()))?;
    let mut reservation_digests = BTreeSet::new();
    let mut transaction_hashes = BTreeSet::new();
    for (key, entrypoint_hash) in input
        .reservation_group
        .ordered_keys
        .iter()
        .zip(&input.payload.entrypoint_hashes)
    {
        if key.validate().is_err()
            || !reservation_key_matches_group(key, identity)
            || Hash::from(key.entrypoint_hash) != *entrypoint_hash
            || key.reservation_owner_hash != reservation_owner_hash
            || key.proposal_identity_hash != proposal_identity_hash
            || !reservation_digests.insert(key.digest())
            || !transaction_hashes.insert(key.signed_transaction_hash)
            || (require_canonical_carrier_body
                && state.has_committed_transaction(key.signed_transaction_hash))
        {
            return Err(invalid_historical_autonomous_recovery(
                input,
                "historical FIFO reservation identity is malformed, duplicated, or committed",
            ));
        }
    }
    if input.reservation_group.ordered_keys.len() != input.payload.entrypoint_hashes.len() {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "historical FIFO reservation order does not cover every executable entrypoint",
        ));
    }

    let validator_pops = if let Some(record) = retained_record {
        record.validator_pops.clone()
    } else {
        match super::lane_planner::pinned_autoscale_validator_pops_for_set(
            state,
            descriptor.lane_id,
            &expected_validators,
        ) {
            Some(Some(pops)) => pops,
            Some(None) => {
                let world = state.world_view();
                expected_validators
                    .iter()
                    .map(|peer| crate::state::live_consensus_key_pop_for_peer(&world, peer, height))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        invalid_historical_autonomous_recovery(
                            input,
                            "operator-managed historical committee lacks a State-aligned PoP",
                        )
                    })?
            }
            None => {
                return Err(invalid_historical_autonomous_recovery(
                    input,
                    "autoscaled historical committee has no exact incarnation-bound PoP vector",
                ));
            }
        }
    };
    if validator_pops.len() != expected_validators.len()
        || expected_validators
            .iter()
            .zip(&validator_pops)
            .any(|(peer, pop)| {
                pop.len() != crate::lane_consensus::LANE_BLS_PROOF_BYTES
                    || iroha_crypto::bls_normal_pop_verify(peer.public_key(), pop).is_err()
            })
    {
        return Err(invalid_historical_autonomous_recovery(
            input,
            "historical validator PoPs are missing, misordered, oversized, or invalid",
        ));
    }

    if require_canonical_carrier_body {
        let canonical = canonical_autonomous_carrier_disposition(
            state,
            kura,
            &input.historical_context,
            state_height,
            network_id,
            expected_epoch,
            &input.reservation_group,
            Some(&input.payload),
        )?;
        match canonical {
            CanonicalAutonomousCarrierInspection::Available(
                CanonicalAutonomousCarrierDisposition::ExactAutonomous(extracted),
            ) if extracted == *input => {}
            CanonicalAutonomousCarrierInspection::MissingBody(_) => {
                return Err(V2ReservationLifecycleError::MissingCanonicalBody { height });
            }
            _ => {
                return Err(invalid_historical_autonomous_recovery(
                    input,
                    "canonical carrier does not contain one unique exact autonomous envelope",
                ));
            }
        }
    }

    Ok(HistoricalAutonomousLaneRecoveryRecordV1::from_install(
        input,
        validator_pops,
    ))
}

/// Read-only all-authority preflight used before the first batch mutation.
pub(crate) fn preflight_historical_autonomous_lane_recovery(
    state: &State,
    kura: &Kura,
    input: &HistoricalAutonomousReservationInstallV1,
) -> Result<HistoricalAutonomousLaneRecoveryRecordV1, V2ReservationLifecycleError> {
    preflight_historical_autonomous_lane_recovery_inner(state, kura, input, true, None)
}

/// Validate a durable record for startup planning and bounded hydration without
/// consulting the prunable canonical block body or mutable current catalog.
/// The retained finality context authenticates the shared roster; independent
/// lane authority and its ordered PoPs were State-validated before the
/// no-clobber record seal and are rechecked structurally and cryptographically
/// here. Kura separately requires the exact active incarnation and sidecars.
pub(crate) fn validate_historical_autonomous_lane_recovery_record(
    state: &State,
    kura: &Kura,
    record: &HistoricalAutonomousLaneRecoveryRecordV1,
) -> Result<(), V2ReservationLifecycleError> {
    let expected = preflight_historical_autonomous_lane_recovery_inner(
        state,
        kura,
        &record.installation_input(),
        false,
        Some(record),
    )?;
    if &expected != record {
        return Err(invalid_historical_autonomous_recovery(
            &record.installation_input(),
            "durable recovery record differs from the current State-aligned historical PoPs",
        ));
    }
    Ok(())
}

/// Persist one State-preflighted runner batch through Kura's single bounded
/// inventory/preflight pass and scan-free per-record durable writes.
pub(crate) fn persist_preflighted_historical_autonomous_lane_recoveries(
    kura: &Kura,
    records: &[HistoricalAutonomousLaneRecoveryRecordV1],
) -> Result<Vec<HistoricalAutonomousLaneRecoveryInstallOutcome>, V2ReservationLifecycleError> {
    Ok(kura
        .persist_historical_autonomous_lane_recovery_records(records)?
        .into_iter()
        .map(|outcome| match outcome {
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed => {
                HistoricalAutonomousLaneRecoveryInstallOutcome::Installed
            }
            HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled => {
                HistoricalAutonomousLaneRecoveryInstallOutcome::AlreadyInstalled
            }
        })
        .collect())
}

/// Revalidate one complete installed runner batch with exactly one bounded
/// inventory scan, one recovery-ID index, and direct immutable dependency
/// checks for every requested record.
pub(crate) fn validate_installed_historical_autonomous_lane_recoveries(
    kura: &Kura,
    expected: &[HistoricalAutonomousLaneRecoveryRecordV1],
) -> Result<(), V2ReservationLifecycleError> {
    if expected.is_empty() {
        return Ok(());
    }
    let inventory = HistoricalAutonomousRecoveryInventory::read(kura)?;
    let mut requested = BTreeMap::<Hash, &HistoricalAutonomousLaneRecoveryRecordV1>::new();
    for record in expected {
        if requested
            .insert(record.recovery_id, record)
            .is_some_and(|existing| existing != record)
        {
            return Err(invalid_historical_autonomous_recovery(
                &record.installation_input(),
                "runner batch aliases one recovery ID to different canonical records",
            ));
        }
    }
    for record in requested.into_values() {
        let Some(installed) = inventory.exact_record(record)? else {
            return Err(
                V2ReservationLifecycleError::HistoricalRecoveryInstallationMissing {
                    recovery_id: record.recovery_id,
                    lane_id: record.payload.origin_proposal.descriptor.lane_id,
                },
            );
        };
        kura.validate_historical_autonomous_lane_recovery_record_dependencies(installed)?;
    }
    Ok(())
}
