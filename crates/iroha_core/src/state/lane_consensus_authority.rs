//! Exact staged authority for admitted lane work, including a closed drain.

use super::*;

/// Resolve one slot's committee and aligned native PoPs from the completed staged state.
///
/// A closed autoscale route may finish only its exact pre-close admissions under
/// the incarnation's authenticated immutable pin. This does not reopen generic
/// route admission or reconstruct old manifest/stake authority from current data.
pub(super) fn resolve_open_lane_authority(
    state: &StateBlock<'_>,
    lane: &iroha_data_model::nexus::LaneConfig,
    incarnation: Hash,
    height: u64,
) -> Result<(Vec<PeerId>, Vec<Vec<u8>>), String> {
    if height == 0
        || height != state._curr_block.height().get()
        || !state
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .any(|current| current == lane)
        || state.lane_incarnations.get(&lane.id).copied() != Some(incarnation)
        || lane_incarnation_is_zero(incarnation)
    {
        return Err(
            "lane opening authority differs from its exact staged route or carrier".to_owned(),
        );
    }
    let activation = state
        .lane_incarnation_activation_heights
        .get(&lane.id)
        .and_then(|height| height.checked_add(1))
        .ok_or_else(|| "lane opening authority has no activation anchor".to_owned())?;
    if height < activation {
        return Err("lane opening authority precedes incarnation activation".to_owned());
    }
    let drain = decode_autoscale_lane_drain_state(lane).map_err(str::to_owned)?;
    let (committee, pops) = if let Some(drain) = drain
        && height > drain.intent.close_global_height
    {
        let committed_height = u64::try_from(state.height())
            .map_err(|_| "lane opening committed height exceeds u64".to_owned())?;
        if !lane.claims_autoscale_managed()
            || drain.intent.close_global_height > committed_height
            || drain.intent.close_global_height < activation
            || drain.commitment.is_some()
            || !autoscale_lane_drain_state_matches_context(
                lane,
                &drain,
                &state.network_id,
                incarnation,
            )
            || !nexus_autoscale_lane_active_for_authority(
                lane,
                &state.nexus,
                drain.intent.close_global_height,
            )
        {
            return Err(
                "closed lane opening differs from its committed drain authority".to_owned(),
            );
        }
        let route = QueuePlanPendingObligationRouteV1 {
            version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_incarnation: incarnation,
        };
        let members = State::queue_plan_pending_route_members_from_storage(
            state.world.smart_contract_state(),
            route,
        )
        .map_err(|error| error.to_string())?;
        if members.is_empty() {
            return Err("closed lane opening has no admitted pending work".to_owned());
        }
        for (_, member) in members {
            let key = State::queue_plan_pending_obligation_marker_key(
                member.network_id_digest,
                member.entrypoint_hash,
            )
            .map_err(|error| error.to_string())?;
            let payload = state
                .world
                .smart_contract_state()
                .get(&key)
                .ok_or_else(|| "closed lane pending member lost its admission".to_owned())?;
            let obligation =
                State::decode_exact_queue_plan_pending_obligation_marker(&key, payload)
                    .map_err(|error| error.to_string())?;
            let registry_key =
                State::queue_plan_admission_registry_marker_key(&obligation.binding.registry_key())
                    .map_err(|error| error.to_string())?;
            let registry_payload = state
                .world
                .smart_contract_state()
                .get(&registry_key)
                .ok_or_else(|| {
                    "closed lane pending work lost its ranked registry owner".to_owned()
                })?;
            let record = State::decode_exact_queue_plan_admission_registry_record(
                &registry_key,
                registry_payload,
            )
            .map_err(|error| error.to_string())?;
            if obligation.binding.admission_context.proposal_height
                > drain.intent.close_global_height
                || record.claim != obligation.binding.registry_value()
                || record.priority.carrier_height
                    < obligation.binding.admission_context.proposal_height
                || record.priority.carrier_height > drain.intent.close_global_height
                || State::queue_plan_binding_application_evidence_in_view(
                    state,
                    &obligation.binding,
                )? != QueuePlanBindingApplicationEvidence::Pending
            {
                return Err(
                    "closed lane opening requires exact unresolved pre-close admissions".to_owned(),
                );
            }
        }
        let pin = decode_autoscale_lane_committee(lane)
            .map_err(str::to_owned)?
            .ok_or_else(|| "closed lane opening has no immutable committee pin".to_owned())?;
        validate_autoscale_lane_committee_pops(&pin).map_err(str::to_owned)?;
        (pin.validator_set, pin.validator_pops)
    } else {
        let manifests = state
            .pending_autoscale_lifecycle
            .as_ref()
            .map_or(state.lane_manifests.as_ref(), |pending| {
                pending.updated_lane_manifests.as_ref()
            });
        let committee = lane_authority::resolve_from_sources(
            &state.world,
            &state.network_id,
            LaneAuthorityRoute::new(lane.id, lane.dataspace_id),
            manifests,
            &state.nexus,
            height,
        )
        .map_err(|error| error.to_string())?
        .into_validators();
        let pops = if lane.claims_autoscale_managed() {
            let pin = decode_autoscale_lane_committee(lane)
                .map_err(str::to_owned)?
                .ok_or_else(|| "lane opening has no immutable committee pin".to_owned())?;
            if pin.validator_set != committee {
                return Err("lane opening committee differs from its immutable pin".to_owned());
            }
            pin.validator_pops
        } else {
            committee
                .iter()
                .map(|peer| {
                    live_consensus_key_pop_for_peer_on_lane(&state.world, peer, height, lane.id)
                        .ok_or_else(|| "lane opening lacks an authorized validator PoP".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        (committee, pops)
    };
    let roster = committee
        .iter()
        .cloned()
        .map(
            |validator| iroha_data_model::block::consensus_v2::ValidatorPower {
                validator,
                power: 1,
            },
        )
        .collect::<Vec<_>>();
    iroha_data_model::block::consensus_v2::finality::verify_validator_power_roster_pops(
        &roster, &pops,
    )
    .map_err(|error| error.to_string())?;
    Ok((committee, pops))
}
