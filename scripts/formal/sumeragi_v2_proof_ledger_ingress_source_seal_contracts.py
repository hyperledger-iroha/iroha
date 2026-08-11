"""Ingress geometry and admission source seals for the proof ledger."""

# Exact pure geometry and admission items connecting the concrete 5N+3H+2
# owner model to the authoritative production queue.  Whole-item token seals are
# intentional here: every numeric term contributes to a progress reservation,
# and comments or test-only lookalikes cannot satisfy this contract.
_PRODUCTION_FAIR_V2_INGRESS_TOP_LEVEL_ITEM_SHA256 = {
    "fair_v2_ingress_is_certified_body_request": (
        "749f33ce31fcfe4ecf84e2264c181b909d30043bb759a90cbf6e5ebb1a40d0e0"
    ),
    "fair_v2_ingress_serve_selector_projection": (
        "948057ba574c9a36b6080131f5216bf4b85e93818065023d840982da125843d9"
    ),
    "fair_v2_ingress_leader_wire_selector_projection": (
        "34eebea6c6b1a3aefeb68010a2ea367970cc3f16a7d5adf592e184bcc799201e"
    ),
    "fair_v2_ingress_queue_gate_verdict": (
        "c867fbfccf0d45fff2757bfbec97655de382b225ce9adada9f451e01c3e38e8d"
    ),
    "fair_v2_ingress_required_capacity": (
        "eec628b52b06d4e8d2238cc1d05f3b18a53347e186c438723fc4861d442db550"
    ),
    "fair_v2_ingress_current_protected_slots": (
        "58767f7d72045788225efb6019570df5e4f34d0aa4520a414d225f0ea1449549"
    ),
    "fair_v2_ingress_lane_protected_slots": (
        "9ded1cd333e6cdb7415aced353a72caca4965fd74c705d11d3916faf251e4af8"
    ),
    "fair_v2_ingress_required_byte_capacity": (
        "f3b4cfc1017778a7d7ba68b1e4c3a24c2f00f5f20252d3375eded1e986bd0799"
    ),
    "fair_v2_ingress_compact_len_prefix_bytes": (
        "50cd13b1d620e26eb0502ae9650b7cb66e489073ab407d95a5217177de517d95"
    ),
    "fair_v2_ingress_framed_bytes": (
        "d311f9815d146c9cc4539653a399152e0b3cf32f3d47060d9af23aa19c04e7c5"
    ),
    "fair_v2_ingress_required_manifest_bytes": (
        "5904b0cb28ad048ab778e75e1f99bc19336b6712d817d33ac7a07e050f25c7e3"
    ),
    "fair_v2_ingress_required_quorum_certificate_bytes": (
        "0335a87586cee9c0d8bf68ba87581f873ccbfdce5c316b1e033ccf0019801629"
    ),
    "fair_v2_ingress_required_certified_fence_escape_bytes": (
        "1706b7bac95b09fd83b5f1b111364c47639ec7c74b0ae156c80127277f89838e"
    ),
    "fair_v2_ingress_required_proposal_bytes": (
        "f87150d121741f99778f8108a82cc95a0811afe51edae3cf06ee8e955337985b"
    ),
    "fair_v2_ingress_network_message_bytes_from_block_message": (
        "3e9310afc887443851e090698197623e01be8165583d6643b92223119e412573"
    ),
    "fair_v2_ingress_network_message_bytes": (
        "5e605df8dc71ee5961cf2c40b3dc8c6108c1c745662a5249d533b1dc8c9fd6eb"
    ),
    "fair_v2_ingress_required_p2p_frame_bytes": (
        "a72706f96788cbab7cb43997ec1dc97e8168b2e5dc8e5d1817db641179f8d7ae"
    ),
    "fair_v2_ingress_required_lane_p2p_frame_bytes": (
        "db83412d500e2f53d7959462f230a02afe85cfa87d1ebf05dd1744a1aacf88d6"
    ),
    "fair_v2_ingress_v2_envelope_bytes": (
        "6a5e066c134eb45a8b64fb5e3b62aab6527ef05f4f6c4459666d56929f13723e"
    ),
    "fair_v2_ingress_embedded_peer_id_bytes": (
        "6a511e21a8e16c6b4db002e3276a5d98d9797249a2d1c3b7549931d6bf24f5bb"
    ),
    "fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key": (
        "8456504122db0dfe12337fce4f14d0e107a3802f990bcf3b9f28dc946ee67b8c"
    ),
    "fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes": (
        "67264f5f5699090643b5ba1554f750f89ade5b6f8e36e12f6d10c15607783b9f"
    ),
    "fair_v2_ingress_required_block_sync_p2p_frame_bytes": (
        "dbbe6e8853781b0d42843c59a4028d103a938493d45d5f5e5551356b884a6aec"
    ),
    "fair_v2_ingress_required_recovery_request_bytes_for_key": (
        "d08abcf51ae678b57c615241239b06155f1a723d64d033497f0a2bcdc16127c9"
    ),
    "fair_v2_ingress_required_recovery_request_bytes": (
        "a863f9d5f5083901c388302a2c41873ed324617fc80f6d89dca19a68462a14c3"
    ),
    "fair_v2_ingress_required_commit_certificate_response_bytes_for_key": (
        "f231a220131a213c5bdaae922c075e2f00bae7a39cbfba408ed86149ba42cf14"
    ),
    "fair_v2_ingress_required_commit_certificate_response_bytes": (
        "a799e86fd78987da5fea276ffbda96d6078e628e3943c1edb223aa60c89fb81c"
    ),
    "fair_v2_ingress_required_transport_completion_bytes": (
        "a8816bf106a1a62a8b8c411f5964aeeef1ec14f86d14f2313b5cae353e1b359c"
    ),
}

# Exact semantic projections complement the complete item digests above. Keep
# them outside the monolithic checker so the fail-closed source contract can
# grow without increasing the checker exception budget.
_PRODUCTION_EXACT_OUTPUT_TOKEN_SEQUENCES = {
    "fair_ingress_evidence": """
pub(crate) struct FairV2IngressOwnershipEvidence {
    first: FairV2IngressOwnershipOccurrence,
    latest: FairV2IngressOwnershipOccurrence,
    runtime_physical_cut: Option<u128>,
    leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    leader_wire_runtime_receipt: Option<serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt>,
    admission_count: u128,
    occurrence_count: u128,
    action_counts: [u128; FairV2IngressOwnershipAction::COUNT],
    current_routes: Option<NetworkReplyRoutes>,
    attempts: Vec<FairV2IngressReplyAttempt>,
    attempts_hash: CryptoHash,
}
""",
    "fair_ingress_merge": """
let Some(attempts) = fair_v2_ingress_merge_attempt_cursors(
    &self.attempts,
    &candidate.attempts,
    current_routes.as_ref(),
) else {
    return false;
};
let attempts_hash = fair_v2_ingress_attempt_cursor_hash(&attempts);
let merged = Self {
    first: self.first.clone(),
    latest: candidate.latest,
    leader_wire_token: self.leader_wire_token.clone(),
    leader_wire_runtime_receipt: self.leader_wire_runtime_receipt.clone(),
    runtime_physical_cut: self.runtime_physical_cut,
    admission_count,
    occurrence_count,
    action_counts,
    current_routes,
    attempts,
    attempts_hash,
};
if !merged.validate_exact() {
    return false;
}
*self = merged;
true
""",
    "applied_height_scope": """
rollover_claim.validate_fanout(messages, peers)?;
if matches!(
    rollover_claim,
    ExactOutputRolloverClaim::NonRetireableLaneTransport { .. }
) {
    return Err(
        "non-retireable lane transport must drain before applied-height handoff".to_owned(),
    );
}
let scope = rollover_claim.scope().ok_or_else(|| {
    "Sumeragi v2 exact output has no typed applied-height rollover claim".to_owned()
})?;
if !scope.covers(artifact) {
    return Err("Sumeragi v2 output claim belongs to another creation scope".to_owned());
}
""",
    "durable_history_dispatch": """
if matches!(
    rollover_claim,
    ExactOutputRolloverClaim::DurableCommitCertificateResponse { .. }
        | ExactOutputRolloverClaim::DurableCertifiedBodyResponse { .. }
        | ExactOutputRolloverClaim::DurableLaneCertificateResponse { .. }
        | ExactOutputRolloverClaim::HistoricalAutonomousLaneCertification { .. }
        | ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse { .. }
) {
    return durable_history_source_covers(
        messages,
        rollover_claim,
        &artifact.height_context.network_id,
        artifact.height,
        durable_history.ok_or_else(|| {
            "Sumeragi v2 durable response lacks an independently readable history source"
                .to_owned()
        })?,
    );
}
""",
    "applied_height_handoff": """
self.validate_applied_height_output_handoff_authority(receipt, artifact)?;
let (retired, retired_kura_replica_advert_heights) = {
    let mut pending = self.lock_pending_exact_output()?;
    if self.exact_output_handoff_owner.is_sealed() {
        return Err(
            "Sumeragi v2 applied-height output handoff is already sealed".to_owned(),
        );
    }
    let retired_kura_replica_advert_heights =
        pending.pending_kura_replica_advert_heights()?;
    let retired = pending.handoff_applied_height_to_durable_reconstruction(
        artifact,
        Some(durable_lane_authority),
        Some(self.kura.as_ref()),
    )?;
    (retired, retired_kura_replica_advert_heights)
};
let scheduled_kura_replica_adverts = self
    .kura_replica_advert_refresh
    .schedule_retired_exact_output_heights(
        retired_kura_replica_advert_heights,
        Instant::now(),
    )?;
""",
    "reply_route_merge_receipt": """
let retained_routes = self.reply_routes.clone().ok_or_else(|| { "Sumeragi v2 retained reply fanout lost its bounded route history".to_owned() })?;
let mut candidate_routes = candidate.reply_routes.clone().ok_or_else(|| "Sumeragi v2 reply retry lost its bounded route history".to_owned())?;
let mut candidate_ownership = candidate.ingress_ownership.clone(); let mut merge_attempt = 0usize;
let merge_receipt = loop {
    let (_, prune_receipt) = candidate_routes.retain_active_with_receipt();
    if let Some(ownership) = candidate_ownership.as_mut() { candidate_routes = ownership.project_retained_reply_routes(prune_receipt).ok_or_else(|| { "Sumeragi v2 candidate pruning lost fair-ingress ownership".to_owned() })?; }
    let live_before_merge = candidate_routes.len(); after_candidate_prune(merge_attempt);
    let mut merged_routes = retained_routes.clone();
    match merged_routes.merge_with_receipt(&candidate_routes) {
        Ok(receipt) => break ReplyRouteMergeReceipt::Strict(receipt),
        Err(NetworkReplyRouteError::Inactive) => {
            let (_, prune_receipt) = candidate_routes.retain_active_with_receipt();
            if let Some(ownership) = candidate_ownership.as_mut() { candidate_routes = ownership.project_retained_reply_routes(prune_receipt).ok_or_else(|| { "Sumeragi v2 raced candidate pruning lost fair-ingress ownership".to_owned() })?; }
            if candidate_routes.len() >= live_before_merge { return Err("Sumeragi v2 inactive reply-history retry made no progress".to_owned()); }
            merge_attempt = merge_attempt.checked_add(1).ok_or_else(|| { "Sumeragi v2 reply-history retry count overflowed".to_owned() })?;
        }
        Err(NetworkReplyRouteError::Stale) => {
            if !self.rollover_claim.accepts_superseded_reply_delivery() { return Err("Sumeragi v2 outbound reply fanout contains a stale capability".to_owned(),); }
            let receipt = merged_routes.merge_observed_with_receipt(&candidate_routes).map_err(|error| { format!("invalid superseded Sumeragi v2 reply route history: {error}") })?;
            break ReplyRouteMergeReceipt::Superseded(receipt);
        }
        Err(error) => { return Err(format!("invalid Sumeragi v2 reply route history: {error}")); }
    }
};
""",
    "reply_route_ownership_receipt": """
after_route_merge();
let (merged_routes, ingress_ownership) = match (&self.ingress_ownership, candidate_ownership) {
        (Some(retained), Some(candidate)) => {
            let mut retained = retained.clone(); let receipt_routes = match merge_receipt {
                ReplyRouteMergeReceipt::Strict(receipt) => { retained.merge_downstream_with_strict_receipt(candidate, receipt) }
                ReplyRouteMergeReceipt::Superseded(receipt) => { retained.merge_downstream_with_observed_receipt(candidate, receipt) }
            };
            let Some(receipt_routes) = receipt_routes else { return Err("Sumeragi v2 exact-output coalescing lost fair-ingress ownership".to_owned(),); };
            (receipt_routes, Some(retained))
        }
        (None, None) => {
            let receipt_routes = match merge_receipt {
                ReplyRouteMergeReceipt::Strict(receipt) => { receipt.into_output(&retained_routes, &candidate_routes) }
                ReplyRouteMergeReceipt::Superseded(receipt) => { receipt.into_output(&retained_routes, &candidate_routes) }
            }
            .ok_or_else(|| { "Sumeragi v2 exact-output route receipt changed its exact histories".to_owned() })?;
            (receipt_routes, None)
        }
        (Some(_), None) | (None, Some(_)) => { return Err("Sumeragi v2 exact-output retry changed fair-ingress ownership shape".to_owned(),); }
    };
""",
    "lane_output_retirement_authority": """
let lane_output_is_covered = |lane_message: &BlockMessage| -> Result<bool, String> {
    let authority = durable_lane_authority.ok_or_else(|| { "Sumeragi v2 lane output lacks a typed durable rollover authority".to_owned() })?;
    if authority.covered_source_hash(artifact, lane_message)?.is_some() { return Ok(true); }
    let Some((proposal_height, _)) = lane_output_identity(lane_message) else { return Ok(false); };
    if proposal_height >= artifact.height { return Ok(false); }
    durable_historical_lane_output_source_hash(durable_history.ok_or_else(|| { "historical lane output lacks an independently readable Kura source".to_owned() })?, lane_message,).map(|source| source.is_some())
};
""",
    "lane_output_retirement_use": """
lane_message @ (BlockMessage::LaneBlockProposal(_) | BlockMessage::LaneBlockVote(_) | BlockMessage::LaneBlockQc(_) | BlockMessage::LaneBlockCertificate(_)) if matches!(rollover_claim, ExactOutputRolloverClaim::Lane(_)) => {
    lane_output_is_covered(lane_message)?
}
""",
    "runner_service_owner": """
Arc::clone(&output_guard),
Arc::clone(&block_rx),
Arc::clone(&kura_replica_advert_refresh),
leader_wire_recovery_authority,
exact_output_service_owner,
)
.map_err(V2RunnerError::Service)?;
""",
    "historical_body_guard": """
let served = serve_block_sync_while_guarded(
    output_guard,
    || {
        block_sync_server
            .serve_historical_body(kura, request, &sender, local_key)
    },
""",
    "lane_durable_or_retained_source": """
let durable = self.kura.read_certified_lane_block_artifact(
    descriptor.lane_id,
    descriptor.lane_block_height,
);
let Some(durable) = durable else {
    let retained = durable_sessions.get(&proposal.proposal_hash);
    if !matches!(
        retained,
        Some(DurableLaneSessionSource::Retained {
            proposal: retained_proposal,
            ..
        }) if retained_proposal == proposal
    ) {
        return Err(V2LaneWorkError::Persistence(
            "unfinished winning lane proposal has no bounded successor owner"
                .to_owned(),
        ));
    }
    continue;
};
let autonomous_anchor = self.canonical_autonomous_anchor_matches_kura(proposal);
let autonomous_certificate = require_lane_certificate_execution_role_matches_anchor(
    &durable.prepare_qc,
    autonomous_anchor,
)?;
let autonomous_payload = autonomous_certificate
    .then(|| {
        self.kura.read_autonomous_lane_block_artifact(
            descriptor.lane_id,
            descriptor.lane_block_height,
            chain_id_hash,
            self.context.epoch,
        )
    })
    .flatten()
    .map(|artifact| artifact.executable_payload);
let application_receipt = if autonomous_payload.is_some() {
    None
} else {
    Some(
        self.kura
            .read_lane_block_application_receipt(
                descriptor.lane_id,
                descriptor.lane_block_height,
            )
            .ok_or_else(|| {
                V2LaneWorkError::Persistence(
                    "retained lane CommitQC has no durable application receipt"
                        .to_owned(),
                )
            })?,
    )
};
""",
    "lane_complete_rollover_authority": """
let source = if let Some(payload) = autonomous_payload {
    if payload.origin_proposal != *proposal {
        return Err(V2LaneWorkError::Persistence(
            "autonomous rollover payload differs from its certified proposal"
                .to_owned(),
        ));
    }
    DurableLaneSessionSource::autonomous_certified(
        finality_artifact,
        &durable,
        &payload,
        signer_pops,
    )
} else {
    DurableLaneSessionSource::persistent(
        finality_artifact,
        &durable,
        application_receipt
            .as_ref()
            .expect("ordinary rollover source requires its application receipt"),
        signer_pops,
    )
};
durable_sessions.insert(proposal.proposal_hash, source);
}
Ok(Some(DurableLaneRolloverAuthority::new(
    finality_artifact,
    winning_proposal_hashes,
    durable_sessions,
)))
""",
}
_PRODUCTION_FAIR_V2_INGRESS_CLASS_ITEM_SHA256 = {
    "classify": (
        "4c5af83b512d633256649e19265a22e80ef9d2e5fde50507ec91c075642b98e6"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_IMPL_ITEM_SHA256 = {
    "new_with_source_geometry_and_transport_frame_caps": (
        "a2139d515f4c30889d862f462d52d4aded7e96aadc80b23624f63b721c37ad68"
    ),
    "configure_roster_for_context": (
        "19c00b3b692c6dba9ada1003dff9483ea7a90a47263a506dd6c764f1d24fc68c"
    ),
    "configure_roster_with_byte_requirements": (
        "dd118c20b0f64df0f6c0ff317320c4704bc8376d68c085b493ac763621b59cf9"
    ),
    "open": (
        "0a55b773a082a1d72592558a4941355d16d9c2231f9b08d25b778d25749ebcc1"
    ),
    "try_push_at": (
        "18a355d5296d31f40fc58dd00cd08eb8c79fbec307ee6532bac4a684ab0f60da"
    ),
    "try_recv_if_at_checked": (
        "ab9e90e132fbd369c255a7982b5ceab98e74ed3b535cc494b0b12a57a53bf81f"
    ),
    "try_recv_if_at_checked_classified": (
        "234399223cc9b36bd4c6f3be6dd29040fd0761b67b4a1c38f4fcdd40bf79ac19"
    ),
    "dequeue_selected_locked": (
        "4c94e8f957bbf35bdc21167602d93646070dbb35806761a8068db8832be7fc31"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_TEST_ITEM_SHA256 = {
    "fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions": (
        "9fdace5f2d7203c48221a9ea47be4bf0522a126403cb3dfd6d9397fefd63e989"
    ),
}
