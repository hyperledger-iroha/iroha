"""Ingress geometry and admission source seals for the proof ledger."""

# Exact pure geometry and admission items connecting the concrete 5N+3H
# owner model to the authoritative production queue.  Whole-item token seals are
# intentional here: every numeric term contributes to a progress reservation,
# and comments or test-only lookalikes cannot satisfy this contract.
_PRODUCTION_FAIR_V2_INGRESS_TOP_LEVEL_ITEM_SHA256 = {
    "fair_v2_ingress_leader_wire_selector_projection": (
        "08ce0575f38ec671cff556b343d981536dd24fb5bc6c49874b76508cd9cd8104"
    ),
    "fair_v2_ingress_queue_gate_verdict": (
        "73b744b27b3cc5ffe642fec4a017aeca5bb4fb314205afa0b66531d9fdbf776d"
    ),
    "fair_v2_ingress_required_capacity": (
        "8ad1236ac1728e777706855b7fe53bdaf43e85bfe1e613d2fd0efd18ff9eac50"
    ),
    "fair_v2_ingress_current_protected_slots": (
        "cfa5470ddd16fb8d4c0be612b70b2510ce417c5a5bc63455185034cd357dd83e"
    ),
    "fair_v2_ingress_lane_protected_slots": (
        "3bdb6bf94f55225abbf41e692404c4d2556bc3a867e57922d24594ce32055bd6"
    ),
    "fair_v2_ingress_required_byte_capacity": (
        "1eef4625233bae0627b4ea7c94910c1b8600d26328137a51d435583bbf79ffa6"
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
        | ExactOutputRolloverClaim::HistoricalLaneCertification { .. }
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
    "autonomous_new_view_exact_body": """
let origin_view = payload.origin_proposal.descriptor.lane_block_view;
if body.from_view < origin_view {
    return false;
}
let source = if body.from_view == origin_view {
    payload.origin_proposal.clone()
} else {
    let Ok(source) = crate::lane_consensus::retarget_lane_block_proposal_exact_view(
        &payload.origin_proposal,
        body.from_view,
    ) else {
        return false;
    };
    source
};
crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
    &source,
    payload,
    body.target_view,
    expected_network_id,
    expected_epoch,
)
.is_ok_and(|expected| expected == *body)
""",
    "autonomous_payload_identity": """
match message {
    BlockMessage::LaneExecutablePayload(candidate) => {
        candidate.origin_proposal.proposal_hash == payload.origin_proposal.proposal_hash
            && candidate.payload_hash == payload.payload_hash
    }
    BlockMessage::LaneBlockNewViewVote(vote) => {
        vote.body.executable_payload_hash == payload.payload_hash
    }
    BlockMessage::LaneBlockNewViewCertificate(certificate) => {
        certificate.body.executable_payload_hash == payload.payload_hash
    }
    _ => false,
}
""",
    "autonomous_exact_attempt_and_retirement": """
let retired = kura
    .read_autonomous_lane_retired_attempt(
        lane_id,
        lane_block_height,
        proposal_height,
        network_id,
        epoch,
    )
    .map_err(|error| format!("autonomous-lane exact retirement validation failed: {error}"))?
    .ok_or_else(|| {
        "current autonomous-lane output has no exact durable slot retirement".to_owned()
    })?;
let durable_payload = &retired.artifact.executable_payload;
let durable_proposal_hash = durable_payload.origin_proposal.proposal_hash;
let bound_supersession_source = durable_lane_authority.covered_source_hash(
    artifact,
    &BlockMessage::LaneBlockProposal(durable_payload.origin_proposal.clone()),
)?;
if durable_payload.origin_proposal.descriptor.proposal_height != proposal_height
    || durable_lane_authority.winning_proposal_hash(durable_proposal_hash)
    || bound_supersession_source.is_none()
{
    return Err(
        "retired autonomous-lane output is not bound to a nonwinning finalized carrier"
            .to_owned(),
    );
}
if retired.retirement
    != crate::kura::AutonomousLaneSlotRetirementV1::from_payload(durable_payload)
{
    return Err(
        "autonomous-lane output differs from its exact durable slot retirement".to_owned(),
    );
}
""",
    "autonomous_exact_retired_payload": """
if payload.producer != *local_peer
    || payload.origin_proposal.descriptor.proposal_height != proposal_height
    || payload != durable_payload
    || retired.retirement
        != crate::kura::AutonomousLaneSlotRetirementV1::from_payload(payload)
{
    return Err(
        "autonomous-lane payload differs from its exact retired local attempt".to_owned(),
    );
}
""",
    "autonomous_exact_retired_vote": """
if vote.signer != *local_peer
    || body.proposal_height != proposal_height
    || !autonomous_new_view_body_matches_durable_payload(
        body,
        durable_payload,
        network_id,
        epoch,
    )
    || !retired
        .current_proposal
        .descriptor
        .validator_set
        .contains(&vote.signer)
{
    return Err(
        "autonomous NewView vote differs from its exact retired local attempt".to_owned(),
    );
}
""",
    "autonomous_exact_retired_certificate": """
let exact_durable = retired
    .artifact
    .new_view_certificates
    .iter()
    .find(|stored| stored.certificate == *certificate)
    .or_else(|| {
        retired
            .artifact
            .view_checkpoint
            .as_ref()
            .and_then(|checkpoint| {
                (checkpoint.certificate.certificate == *certificate)
                    .then_some(&checkpoint.certificate)
            })
    });
if !certificate.validator_set.contains(local_peer) || exact_durable.is_none() {
    return Err(
        "autonomous NewView certificate lacks an exact retired local retransmit source"
            .to_owned(),
    );
}
""",
    "autonomous_current_height_retirement_fallback": """
let Some(canonical_payload) = canonical_payloads.get(&route).filter(|payload| {
    autonomous_lane_output_matches_payload_identity(envelope.as_message(), payload)
}) else {
    if proposal_height != artifact.height {
        return Err(
            "autonomous-lane output is not owned by the canonical Kura carrier".to_owned(),
        );
    }
    autonomous_lane_output_has_exact_retirement_source(
        envelope.as_message(),
        artifact,
        durable_lane_authority,
        kura,
        local_peer,
        proposal_height,
        network_id,
        epoch,
    )?;
    continue;
};
let proposal_hash = canonical_payload.origin_proposal.proposal_hash;
if proposal_height == artifact.height
    && !durable_lane_authority.winning_proposal_hash(proposal_hash)
{
    return Err(
        "autonomous-lane output is not owned by the finalized winning carrier".to_owned(),
    );
}
""",
    "autonomous_retirement_regression_success": """
for output in [
    BlockMessage::LaneExecutablePayload(retired.payload.clone()),
    BlockMessage::LaneBlockNewViewVote(retired.new_view_vote.clone()),
    BlockMessage::LaneBlockNewViewCertificate(retired.new_view_certificate.clone()),
] {
    assert!(autonomous_lane_output_matches_payload_identity(
        &output,
        &retired.payload,
    ));
}
""",
    "autonomous_retirement_regression_retire_all": """
assert_eq!(
    pending
        .handoff_applied_height_to_durable_reconstruction(
            &artifact,
            Some(&authority),
            Some(retired.kura.as_ref()),
        )
        .expect("exact retired attempt supersedes all autonomous output variants"),
    3
);
assert!(!pending.is_pending());
""",
    "autonomous_retirement_regression_atomic": """
mutated
    .handoff_applied_height_to_durable_reconstruction(
        &artifact,
        Some(&authority),
        Some(retired.kura.as_ref()),
    )
    .expect_err("mutated output cannot borrow the exact retirement");
assert!(mutated.is_pending(), "failed handoff remains atomic");

let unretired = crate::kura::tests::unretired_autonomous_lane_attempt_fixture(&validators[0]);
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
    services_output_guard.as_ref(),
    || block_sync_server.serve_historical_body(kura, request, &sender, local_key),
""",
    "lane_durable_predecessor_source": """
let durable = self.kura.read_certified_lane_block_artifact(
    descriptor.lane_id,
    descriptor.lane_block_height,
);
let Some(durable) = durable else {
    return Ok(None);
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
            network_id,
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
    "lane_complete_durable_rollover_authority": """
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
    "root_parse_exact_output_geometry": """
let lane_profile = network.lane_profile;
let reply_source_capacity = network
    .max_total_connections
    .or(lane_profile.derived_limits().max_total_connections)
    .map_or(
        lane_profile.defaults().max_total_connections,
        NonZeroUsize::get,
    );
let remote_trusted_peer_count = trusted_peers.value().others.len();
if remote_trusted_peer_count > reply_source_capacity {
    emitter.emit(
        Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
            "trusted-peer full fanout requires {remote_trusted_peer_count} remote connections, above the effective network connection capacity {reply_source_capacity}"
        )),
    );
}
if sumeragi.queues.authenticated_non_validator_sources.get() > reply_source_capacity {
    emitter.emit(
        Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
            "sumeragi.queues.authenticated_non_validator_sources ({}) exceeds configured network authenticated-source capacity {reply_source_capacity}",
            sumeragi.queues.authenticated_non_validator_sources,
        )),
    );
}
let effect_work_capacity = (sumeragi.queues.commands.get()
    / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
    .max(1);
let validator_roster_len = trusted_peers.value().validator_roster_len();
let authenticated_non_validator_source_capacity =
    sumeragi.queues.authenticated_non_validator_sources.get();
match actual::sumeragi_v2_body_ingress_required_message_capacity(
    validator_roster_len,
    authenticated_non_validator_source_capacity,
) {
    Some(required_bodies) if sumeragi.queues.bodies.get() < required_bodies => {
        emitter.emit(
            Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                "Sumeragi v2 canonical outer-ingress message capacity {} is below the roster-aware minimum {required_bodies}; configured validator roster is {validator_roster_len}, and authenticated non-validator source capacity is {authenticated_non_validator_source_capacity}",
                sumeragi.queues.bodies,
            )),
        );
    }
    None => {
        emitter.emit(
            Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                "Sumeragi v2 roster-aware canonical outer-ingress message minimum overflowed; configured validator roster is {validator_roster_len}, and authenticated non-validator source capacity is {authenticated_non_validator_source_capacity}",
            )),
        );
    }
    Some(_) => {}
}
let body_source_bytes = sumeragi.queues.body_source_bytes.get();
match actual::sumeragi_v2_body_ingress_required_byte_capacity(
    validator_roster_len,
    authenticated_non_validator_source_capacity,
    body_source_bytes,
) {
    Some(required_body_bytes)
        if sumeragi.queues.body_bytes.get() < required_body_bytes =>
    {
        emitter.emit(
            Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                "Sumeragi v2 aggregate canonical outer-ingress wire-byte capacity {} is below the roster-aware minimum {required_body_bytes}; configured validator roster is {validator_roster_len}, authenticated non-validator source capacity is {authenticated_non_validator_source_capacity}, and each source requires {body_source_bytes} bytes",
                sumeragi.queues.body_bytes,
            )),
        );
    }
    None => {
        emitter.emit(
            Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                "Sumeragi v2 roster-aware aggregate canonical outer-ingress wire-byte minimum overflowed; configured validator roster is {validator_roster_len}, authenticated non-validator source capacity is {authenticated_non_validator_source_capacity}, and each source requires {body_source_bytes} bytes",
            )),
        );
    }
    Some(_) => {}
}
if let Err(error) = actual::sumeragi_v2_lifecycle_capacity_geometry(
    validator_roster_len,
    effect_work_capacity,
    sumeragi.queues.bodies.get(),
    sumeragi.queues.authenticated_non_validator_sources.get(),
) {
    emitter.emit(
        Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
            "{error}; configured validator roster is {validator_roster_len}, authenticated non-validator source capacity is {}, and certified-request capacity is {}",
            sumeragi.queues.authenticated_non_validator_sources,
            sumeragi.queues.bodies,
        )),
    );
}
let geometry = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
    effect_work_capacity,
    sumeragi.queues.bodies.get(),
)
.and_then(|shared_capacity| {
    actual::validate_sumeragi_v2_exact_output_geometry(
        shared_capacity,
        reply_source_capacity,
    )
});
if let Err(error) = geometry {
    emitter.emit(
        Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
            "{error}; configured network reply-source capacity is {reply_source_capacity}"
        )),
    );
}
""",
    "pending_lifecycle_rollover_wait": """
let rollover_ready = activated.with_runner_runtime(
    &mut active_runner,
    |executor, _services, lane_work| {
        super::preflight_finalized_lane_rollover(
            executor,
            lane_work,
            &mut canonical_lane_body_recovered,
        )
    },
)?;
if !rollover_ready {
    activated.with_runner_runtime(&mut active_runner, |_executor, _services, lane_work| {
        committed_lane_status_publisher.publish_if_changed(lane_work)
    });
    let _ = wake_rx.recv_timeout(IDLE_POLL);
    continue;
}
""",
    "finalized_output_rollover": """
let _ = retry_exact_output_and_apply_sidecar_admissions(
    &mut lane_work,
    services,
    control_queue_capacity,
)?;
let _ = lane_work.recover_decided_canonical_lane_body(receipt, artifact)?;
lane_work.persist_anchored_sessions()?;
let _ = lane_work.service_next_historical_recovery()?;
if lane_work.has_pending_historical_recovery() {
    return Err(V2RunnerError::Service(
        "finalized lane output still owns predecessor-height recovery".to_owned(),
    ));
}
if !lane_work.durable_completion_matches_finality(artifact)? {
    return Err(V2RunnerError::Service(
        "finalized lane output has not crossed its local durable completion boundary"
            .to_owned(),
    ));
}
lane_work.prepare_canonical_lane_rollover(artifact)?;
let durable_lane_authority = lane_work
    .durable_lane_rollover_authority(artifact)?
    .ok_or_else(|| {
        V2RunnerError::Service(
            "finalized lane output has not crossed its local durable reconstruction boundary"
                .to_owned(),
        )
    })?;
lane_work.prune_finalized_merge_sidecars()?;
lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority)?;
drain_finalized_lane_work_output(
    &mut lane_work,
    services,
    receipt,
    artifact,
    &durable_lane_authority,
    control_queue_capacity,
)?;
if lane_work.has_pending_committed_output_handoff()
    || lane_work.effect_count() != 0
    || services
        .has_pending_exact_output()
        .map_err(V2RunnerError::Service)?
{
    return Err(V2RunnerError::Service(
        "finalized output remained owned after durable handoff".to_owned(),
    ));
}
let exact_output_handoff = services
    .seal_applied_height_output_handoff(receipt, artifact, &durable_lane_authority)
    .map_err(V2RunnerError::Service)?;
lane_work
    .into_retained_merge_sidecars(exact_output_handoff, artifact, successor)
    .map_err(V2RunnerError::from)
""",
}
_PRODUCTION_FAIR_V2_INGRESS_CLASS_ITEM_SHA256 = {
    "classify": (
        "4c5af83b512d633256649e19265a22e80ef9d2e5fde50507ec91c075642b98e6"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_IMPL_ITEM_SHA256 = {
    "new_with_source_geometry_and_transport_frame_caps": (
        "a41b4736d1aa01919dcbccf0b5c71378682e9ad5fd2c64909972f17f6d6c2be8"
    ),
    "configure_roster_for_context": (
        "19c00b3b692c6dba9ada1003dff9483ea7a90a47263a506dd6c764f1d24fc68c"
    ),
    "configure_roster_with_byte_requirements": (
        "a514f0b70ec918a0457ab4fcf90aecab3a5d3370302ae15cc4b161b5d03c92f3"
    ),
    "open": (
        "5009fc5c34fbcd3f75897ef7f37e7c331e948ac00e555b749ca1a4bff85cadf7"
    ),
    "try_push_at": (
        "eb2a36ec0884655d739e38e34b4031467c0f5ed983c77583b3d331df6cceffac"
    ),
    "try_recv_if_at_checked": (
        "7b3b4b907715d56dba2dcc36dee64db07ef61cd7a456d039e24146cf4d60d20d"
    ),
    "try_recv_if_at_checked_classified": (
        "5f6184b1089ce6fee2ccb7f1ad8dadc5d47b8b017dfec383f7211eaec6fe9060"
    ),
    "dequeue_selected_locked": (
        "14a706a0784006194f78d56b50268fca87a84de28f2bf0447d757f7c9dcd63a8"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_TEST_ITEM_SHA256 = {
    "fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions": (
        "9fdace5f2d7203c48221a9ea47be4bf0522a126403cb3dfd6d9397fefd63e989"
    ),
}

# Exact first-release publication-fence items. The two move-only methods bind
# the final queue preflight and assertion-only dequeue across LedgerV1 fsync;
# the three regressions make same-wire, unrelated-append, and abort behavior
# part of both production inventory and G-UNIT.
_PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256 = {
    "PreparedFairIngressQueueWitness::lock_exact_dequeue_retaining": (
        "66d33b07c062bd6dc4a1b879b0b3624bc0403e59305cbc44763d409f97d109fc"
    ),
    "LockedPreparedFairIngressExactDequeue::commit": (
        "2df7516317611dcc3fc0f959cca1e80a7b6aa3670a90d2add798f744cfebbd4c"
    ),
    "locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit": (
        "ea093accfdb33740bc7f21e9c26b17e74a1d7600c885ff45a5718caed8cb457a"
    ),
    "locked_publication_fence_serializes_unrelated_append_and_preserves_it": (
        "c88fcd11bd701f1a67ffc441fe1cc4bdc08f9be32e5a8270373ea83335a6131f"
    ),
    "dropping_locked_publication_fence_releases_producer_without_dequeue": (
        "a31983eba320245b25089ebfcbc6fbd5a5c024fc76b81946329510cf9177e687"
    ),
}

_TIMEOUT_VOTE_EPISODE_INGRESS_REGRESSION_SHA256 = {
    "ordinary_selector_preserves_certified_response_before_timeout_vote": (
        "b55aca5b506f5394f02da001467ed2f2f6465625e82d9e4cb09642391c349a49"
    ),
}
