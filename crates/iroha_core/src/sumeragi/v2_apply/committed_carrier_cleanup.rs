/// Rebuild and authenticate every complete committed carrier selected by the
/// immutable Queue snapshot, then cross-preflight all carriers before Queue
/// mutates the first owner.
fn finalize_startup_committed_canonical_carriers(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    chain_hash: Hash,
    authorized_commit_groups: Vec<(
        Vec<crate::queue::LaneQueueReservationKeyV2>,
        AutonomousLaneQueueCarrierCleanupAuthorization,
    )>,
) -> Result<usize, V2ReservationLifecycleError> {
    if authorized_commit_groups.is_empty() {
        return Ok(0);
    }
    let anchored_carrier_bound = authorized_commit_groups.len();
    let invalid = |detail: &str| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
        detail: detail.to_owned(),
    };
    let mut planned_authorizations = BTreeMap::new();
    let mut carrier_publications = BTreeMap::new();
    for (ordered_keys, carrier_authorization) in authorized_commit_groups {
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                .map_err(|detail| invalid(detail))?;
        if planned_authorizations
            .insert(
                reservation_group.reservation_group_hash,
                (reservation_group, carrier_authorization),
            )
            .is_some()
        {
            return Err(invalid(
                "startup reconciliation duplicates one committed reservation group",
            ));
        }
        let publication = kura
            .reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(
                &reservation_group,
            )?;
        carrier_publications
            .entry(publication.entry_hash())
            .or_insert(publication);
    }

    let mut carrier_heights = BTreeMap::new();
    let mut source_authorized_carriers = Vec::with_capacity(carrier_publications.len());
    for (entry_hash, publication) in carrier_publications {
        let entry = kura.merge_entry_by_hash(entry_hash)?.ok_or_else(|| {
            invalid("startup source-outcome publication lost its committed merge entry")
        })?;
        let authenticated =
            authenticate_committed_canonical_carrier(state, kura, &entry, chain_hash)?;
        if authenticated.reference.entry_hash != entry_hash
            || carrier_heights
                .insert(authenticated.carrier_height, entry_hash)
                .is_some()
        {
            return Err(invalid(
                "startup source-outcome carriers alias one canonical height or entry identity",
            ));
        }
        let source_authorizations = publication.consume_for_v2_apply(&entry).ok_or_else(|| {
            invalid("startup source-outcome publication differs from its complete carrier")
        })?;
        if authenticated.groups.len() != source_authorizations.len() {
            return Err(invalid(
                "startup carrier groups, applications, and source outcomes differ in cardinality",
            ));
        }

        let mut carrier_groups = Vec::with_capacity(authenticated.groups.len());
        for (group, (source_group, source_authorization)) in
            authenticated.groups.into_iter().zip(source_authorizations)
        {
            let reservation_group = group.reservation_group;
            if reservation_group != source_group
                || lane_queue_reservation_group_binding_from_ordered_keys(group.ordered_keys.iter())
                    .ok()
                    != Some(reservation_group)
            {
                return Err(invalid(
                    "startup carrier source or ApplyCarrier order differs from canonical lanes",
                ));
            }
            let reconstructed_authorization = group
                .application
                .queue_cleanup_authorization()
                .map_err(|detail| invalid(&detail))?;
            let reconstructed_projection = reconstructed_authorization
                .validated_projection_for_group(&reservation_group)
                .ok_or_else(|| {
                    invalid(
                        "startup reconstructed ApplyCarrier authority is malformed for its group",
                    )
                })?;
            if let Some((planned_group, planned_authorization)) =
                planned_authorizations.remove(&reservation_group.reservation_group_hash)
                && (planned_group != reservation_group
                    || planned_authorization.validated_projection_for_group(&reservation_group)
                        != Some(reconstructed_projection))
            {
                return Err(invalid(
                    "startup planned and reconstructed ApplyCarrier authorities disagree",
                ));
            }
            carrier_groups.push((source_authorization, reconstructed_authorization));
        }
        source_authorized_carriers.push((
            authenticated.carrier_height,
            entry_hash,
            entry,
            authenticated.carrier_block_hash,
            carrier_groups,
        ));
    }
    if !planned_authorizations.is_empty() {
        return Err(invalid(
            "startup committed Queue group is absent from its reconstructed carrier",
        ));
    }
    source_authorized_carriers.sort_by_key(|(height, entry_hash, _, _, _)| (*height, *entry_hash));
    let mut carrier_releases = Vec::with_capacity(source_authorized_carriers.len());
    for (height, _, entry, carrier_block_hash, _) in &source_authorized_carriers {
        carrier_releases.push((
            entry.clone(),
            u64::try_from(height.get())?,
            *carrier_block_hash,
        ));
    }
    let committed_cleanup = queue
        .authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(
            source_authorized_carriers
                .into_iter()
                .map(|(_, _, _, _, groups)| groups)
                .collect(),
            anchored_carrier_bound,
        )?;
    let finalized = committed_cleanup.finalized_reservations();
    let (_, terminal_evidence) = committed_cleanup.into_parts();
    for evidence in terminal_evidence {
        kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)?;
    }
    for (entry, carrier_height, carrier_block_hash) in carrier_releases {
        kura.release_post_wsv_lane_artifact_budget_reservation(
            &entry,
            carrier_height,
            carrier_block_hash,
        )?;
    }
    Ok(finalized)
}
