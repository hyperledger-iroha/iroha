"""Coordinator-owned Certified-Serve production source-fidelity contracts."""



def _lane_recovery_cache_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Check the independent production cache owner and its complete method seals."""

    errors: list[str] = []
    cache_path, source = _read_reviewed_rust_source(
        repo_root,
        "crates/iroha_core/src/lane_consensus.rs",
        errors,
        "transactional canonical lane recovery cache source",
    )
    items = {}
    for name, digest in _PRODUCTION_LANE_RECOVERY_CACHE_ITEM_SHA256.items():
        item = _require_qualified_rust_item(
            cache_path, source, "LaneBlockSessionCache", name, errors,
            f"lane recovery cache owner {name}",
        )
        items[name] = item
        _require_rust_item_token_sha256(cache_path, item, digest, name, errors)
    _require_lane_recovery_cache_source_contracts(cache_path, items, errors)
    return errors


def _require_lane_recovery_cache_source_contracts(
    cache_path: Path, cache_items: dict, errors: list[str],
) -> None:
    """Bind bounded, original-quorum-preserving recovery before atomic publication."""

    batch = cache_items.get("insert_recovered_proposals")
    preflight = cache_items.get("preflight_trusted_proposal_replacement")
    trusted = cache_items.get("insert_trusted_proposal_replacing_uncommitted_conflict")
    for expected, description in (
        (
            """
pub(crate) fn insert_recovered_proposals(
    &mut self,
    proposals: &[LaneBlockProposalV1],
) -> Result<(), LaneBlockSessionError> {
    let mut required = BTreeMap::new();
    let mut required_slots = BTreeMap::new();
    let mut ordered_required = Vec::new();
    for proposal in proposals {
        self.preflight_trusted_proposal_replacement(proposal)?;
        let key = LaneBlockSessionKey::from_proposal(proposal);
        let slot = LaneBlockSlotKey::from_session_key(key);
        match required.insert(key, proposal) {
            Some(previous) if previous != proposal => {
                return Err(LaneBlockSessionError::ConflictingProposal);
            }
            Some(_) => {}
            None => ordered_required.push(proposal),
        }
""",
            "lane recovery cache must preflight every input against original quorum evidence and preserve exact first-occurrence caller order",
        ),
        (
            """
if required_slots
    .insert(slot, key.proposal_hash)
    .is_some_and(|previous| previous != key.proposal_hash)
{
    return Err(LaneBlockSessionError::ConflictingProposal);
}
if required.len() > self.capacity {
    return Err(LaneBlockSessionError::RecoveryCapacityExceeded);
}
}
let mut next = self.clone();
for proposal in &ordered_required {
    let key = LaneBlockSessionKey::from_proposal(proposal);
    if next.sessions.contains_key(&key) {
        next.touch(key);
    }
}
""",
            "lane recovery cache must bound the unique consistent required union before cloning and touch required survivors before insertion",
        ),
        (
            """
for proposal in &ordered_required {
    let key = LaneBlockSessionKey::from_proposal(proposal);
    if next.sessions.contains_key(&key) {
        next.touch(key);
    }
}
for proposal in ordered_required {
    next.insert_trusted_proposal_replacing_uncommitted_conflict(proposal.clone())?;
    next.touch(LaneBlockSessionKey::from_proposal(proposal));
}
if required.iter().any(|(key, proposal)| {
    next.sessions.get(key).and_then(|session| session.proposal.as_ref()) != Some(*proposal)
}) {
    return Err(LaneBlockSessionError::RecoveryCapacityExceeded);
}
*self = next;
Ok(())
}
""",
            "lane recovery cache must use trusted insertion in caller order and verify the full exact retained set before atomic publication",
        ),
        (
            "self.clone()",
            "lane recovery cache must stage exactly one cache clone",
        ),
        (
            "*self = next;",
            "lane recovery cache must publish exactly once after required-set verification",
        ),
    ):
        _require_rust_token_sequence(cache_path, batch, expected, description, errors)

    _require_rust_token_sequence(
        cache_path,
        preflight,
        """
fn preflight_trusted_proposal_replacement(
    &self,
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneBlockSessionError> {
    validate_lane_block_proposal(proposal).map_err(LaneBlockSessionError::InvalidProposal)?;
    let key = LaneBlockSessionKey::from_proposal(proposal);
    let first = LaneBlockSessionKey {
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        ..key
    };
    let last = LaneBlockSessionKey {
        proposal_hash: Hash::prehashed([u8::MAX; Hash::LENGTH]),
        ..key
    };
    if self.sessions.range(first..=last).any(|(retained_key, session)| {
        retained_key.proposal_hash != key.proposal_hash && session_has_quorum_certificate(session)
    }) {
        return Err(LaneBlockSessionError::ConflictingProposal);
    }
    Ok(())
}
""",
        "lane recovery replacement preflight must validate the proposal and protect any original same-slot quorum including proposal-less evidence",
        errors,
    )
    _require_rust_token_sequence(
        cache_path,
        trusted,
        """
fn insert_trusted_proposal_replacing_uncommitted_conflict(
    &mut self,
    proposal: LaneBlockProposalV1,
) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
    self.preflight_trusted_proposal_replacement(&proposal)?;
    let key = LaneBlockSessionKey::from_proposal(&proposal);
""",
        "trusted lane proposal replacement must share the original-quorum preflight before every mutation",
        errors,
    )


def _require_lane_public_certificate_source_contracts(
    lane_path: Path, lane_ack_items: dict, lane_items: dict, errors: list[str],
) -> None:
    """Bind public observer certificates without granting committee custody."""

    persist = lane_ack_items.get("V2LaneWorkAdapter::persist_anchored_sessions")
    reconstruct = lane_items.get("reconstruct_durable_lane_certificate")
    for expected, description in (
        (
            """
let pops = self.pops_for_lane_session(&session);
let candidate = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {
    V2LaneWorkError::Persistence(format!(
        "pending committed lane certificate is invalid: {message}"
    ))
})?;
let descriptor = &session.proposal.descriptor;
let autonomous_anchor =
    self.canonical_autonomous_anchor_matches_kura(&session.proposal);
let autonomous_certificate = require_lane_certificate_execution_role_matches_anchor(
    &session.prepare_qc,
    autonomous_anchor,
)?;
if autonomous_certificate && !self.local_can_own_autonomous_payload(&session.proposal) {
""",
            "anchored lane persistence must derive autonomous execution authority from the checked PrepareQC role",
        ),
        (
            """
if autonomous_certificate && !self.local_can_own_autonomous_payload(&session.proposal) {
    let replica = self
        .kura
        .persist_canonical_autonomous_lane_replica(&candidate)
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    if !certified_lane_artifacts_certify_same_decision(
        &replica.bundle.certified,
        &candidate,
    ) || replica.bundle.executable_payload().origin_proposal != session.proposal
    {
        return Err(V2LaneWorkError::Persistence(
            "canonical autonomous replica changed its certified lane decision"
                .to_owned(),
        ));
    }
    persisted = persisted.saturating_add(1);
    continue;
}
self.persist_autonomous_prepare_availability(&session.proposal, &session.prepare_qc)
    .map_err(V2LaneWorkError::Persistence)?;
let durable_exact_proposal = self
    .kura
    .read_certified_lane_block_artifact(
        descriptor.lane_id,
        descriptor.lane_block_height,
    )
    .filter(|durable| durable.proposal == session.proposal);
""",
            "public observer persistence must verify its separate exact replica and retire before committee READY or certified-slot access",
        ),
        (
            "self.persist_autonomous_prepare_availability(&session.proposal, &session.prepare_qc)",
            "anchored lane persistence must have exactly one committee READY persistence call after observer retirement",
        ),
    ):
        _require_rust_token_sequence(lane_path, persist, expected, description, errors)

    for expected, description in (
        (
            """
let artifact = self.kura.read_certified_lane_block_artifact(
    proposal.descriptor.lane_id,
    proposal.descriptor.lane_block_height,
);
let Some(artifact) = artifact else {
    return Ok(None);
};
if artifact.proposal != *proposal {
    return Ok(None);
}
let requester_is_current_validator = self
""",
            "lane recovery reconstruction must begin from the exact certified Kura artifact",
        ),
        (
            """
let requester_is_current_validator = self
    .context
    .roster
    .iter()
    .any(|entry| &entry.validator == sender);
let requester_is_historical_lane_validator =
    artifact.commit_qc.validator_set.contains(sender);
let requester_observes_finalized_public_autonomous_carrier =
    !requester_is_current_validator
        && !requester_is_historical_lane_validator
        && self
            .canonical_finalized_autonomous_payload_for_proposal(proposal)
            .map_err(|error| {
                iroha_logger::error!(
                    %error,
                    height = proposal.descriptor.proposal_height,
                    lane = proposal.descriptor.lane_id.as_u32(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    "failed to validate finalized public carrier for cross-roster certificate recovery"
                );
                self.output_guard.close_admission_for_restart();
            })?
            .is_some();
if !requester_is_current_validator
    && !requester_is_historical_lane_validator
    && !requester_observes_finalized_public_autonomous_carrier
{
    return Err(());
}
Ok(Some(LaneBlockCertificateV1 {
    proposal: artifact.proposal,
    prepare_qc: artifact.prepare_qc,
    commit_qc: artifact.commit_qc,
}))
""",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality and fail stop on authority errors",
        ),
    ):
        _require_rust_token_sequence(lane_path, reconstruct, expected, description, errors)


def _lifecycle_certified_serve_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the only production Certified-Serve lifecycle corridor.

    The sealed path is selector authentication -> durable coordinator
    admission -> complete Ready census -> exact worker reservation ->
    LedgerV1 settlement -> reply delivery -> acknowledgement.  Its adjacent
    ProducerTurn is claimed only by the serialized proposal runner.  Legacy
    queue journals, barriers, gates, and producer episodes are forbidden.
    """

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    relative_paths = {
        "registry": "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "scheduler": "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "turn": "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "projection": "crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs",
        "worker": "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "body_store": "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "height": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
        "ordinary": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "pending": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "runner": "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "launch": "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "scheduler_cases": "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_scheduler_certified_serve_cases.rs",
        "ledger_cases": "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs",
        "startup_cases": "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
    }
    errors: list[str] = []
    sources: dict[str, str] = {}
    paths: dict[str, Path] = {}
    for role, relative in relative_paths.items():
        path = repo_root / relative
        paths[role] = path
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: lifecycle Certified-Serve {role} source must be a regular file"
            )
            continue
        if role.endswith("_cases"):
            sources[role] = path.read_text(encoding="utf-8")
            continue
        reviewed_path, reviewed_source = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            f"lifecycle Certified-Serve {role} source",
        )
        paths[role] = reviewed_path
        sources[role] = reviewed_source
    if errors:
        return errors

    production_roles = (
        "registry",
        "scheduler",
        "turn",
        "projection",
        "worker",
        "body_store",
        "height",
        "ordinary",
        "pending",
        "runner",
        "launch",
    )
    production_tokens = rust_code_tokens(
        "\n".join(sources[role] for role in production_roles)
    )
    for retired in (
        "CertifiedServeAdmission",
        "CertifiedServeLifecycleId",
        "CertifiedServeIngressGate",
        "CertifiedServeIngressReservation",
        "CertifiedServeBarrier",
        "CertifiedServeProducerEpisode",
        "ExactServePredecessor",
        "prepare_certified_request",
        "serve_certified_request_on_routes",
        "producer_episode_due",
        "producer_episode_active",
        "serve_barrier",
        "serve_replacements",
        "pending_serve_requests",
        "next_serve_admission_ordinal",
    ):
        observed = production_tokens.count(retired)
        if observed:
            errors.append(
                f"{base}: retired Certified-Serve owner token {retired!r} must be absent; "
                f"found {observed}"
            )

    def item(
        role: str,
        owner: str | None,
        name: str,
        description: str,
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        if owner is None:
            return _require_rust_item(paths[role], sources[role], name, errors)
        expected_context = (("impl", *rust_code_tokens(owner)),)
        matches = [
            candidate
            for candidate in rust_items(sources[role], name)
            if candidate.brace_context == expected_context
        ]
        if len(matches) != 1:
            errors.append(
                f"{paths[role]}: require exactly one real Rust/Verus function item "
                f"named {owner}::{name}; found {len(matches)}"
            )
            return None
        target = matches[0]
        _require_rust_item_context(
            paths[role],
            target,
            expected_context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        return target

    def sequence(
        role: str,
        owner: str | None,
        name: str,
        description: str,
        markers: tuple[str, ...],
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> None:
        target = item(
            role,
            owner,
            name,
            description,
            expected_attributes=expected_attributes,
        )
        if target is None:
            return
        tokens = rust_code_tokens(target.source)
        cursor = -1
        for marker in markers:
            positions = tuple(
                position
                for position in _token_sequence_positions(tokens, rust_code_tokens(marker))
                if position > cursor
            )
            if not positions:
                errors.append(
                    f"{paths[role]}:{target.line}: {description} must retain ordered "
                    f"marker {marker!r}"
                )
                return
            cursor = positions[0]

    sequence(
        "registry",
        "ConcreteLifecycleWorkRegistry",
        "attest_ready_certified_serve_request",
        "Ready Serve registry attestation",
        (
            "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "record.work_class == LifecycleWorkClass::CertifiedServe",
            "record.state == super::LifecycleState::Ready",
            "exactly_matches_certified_serve_request(authenticated)",
            "frozen_predecessors(",
            "serve.matches_record(record, metadata, digest)",
            "ReadyCertifiedServeAttestationV1",
        ),
    )
    sequence(
        "registry",
        "ConcreteLifecycleWorkRegistry",
        "project_claimed_certified_serve_dispatch",
        "claimed Serve registry projection",
        (
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "coordinator.active_lease.as_ref() != Some(&lease)",
            "attestation.matches_claimed_record(record, ledger, &lease)",
            "exactly_matches_certified_serve_request(&attestation.authenticated)",
            "serve.matches_claimed_record(record, metadata, work.digest, &lease)",
            "ClaimedCertifiedServeDispatchV1",
        ),
    )
    sequence(
        "scheduler",
        "CertifiedServeSchedulerObservationV1",
        "from_live_cuts",
        "typed Serve scheduler observation factory",
        (
            "AuthenticatedSchedulerInputsFactory::new()",
            "capacity.authenticated_predecessor_debt(&factory)",
            "dequeue.selector_debt()",
            "runner.debt()",
        ),
    )
    scheduler_claim = item(
        "scheduler", None, "claim_certified_serve_turn_v1", "complete Ready Serve scheduler claim"
    )
    if scheduler_claim is not None:
        scheduler_tokens = rust_code_tokens(scheduler_claim.source)
        for marker in (
            "exact_ready != coordinator.ready_index",
            "exact_ready.len() != observations.len()",
            "record.work_class != LifecycleWorkClass::CertifiedServe",
            "unmatched.iter().any(Option::is_some)",
            "coordinator.plan_turn(inputs)",
            "lease.work_class() == LifecycleWorkClass::CertifiedServe",
            "coordinator.rollback_unpublished_turn(&lease)",
            "project_claimed_certified_serve_dispatch",
            "coordinator.rollback_unpublished_turn(&rollback)",
        ):
            if not _token_sequence_positions(scheduler_tokens, rust_code_tokens(marker)):
                errors.append(
                    f"{paths['scheduler']}:{scheduler_claim.line}: complete Ready Serve "
                    f"scheduler claim must retain {marker!r}"
                )
        for forbidden in ("#[cfg_attr(not(test), allow(dead_code))]", "TODO"):
            if forbidden in scheduler_claim.source:
                errors.append(
                    f"{paths['scheduler']}:{scheduler_claim.line}: live Serve scheduler "
                    f"claim must not retain stale {forbidden!r}"
                )

    sequence(
        "turn",
        None,
        "prepare_and_dispatch_current_certified_serve",
        "current-height Serve lifecycle transaction",
        (
            "cut.fence_producer_publication_retaining()",
            "prepare_current_certified_serve_pre_admission(",
            "cut.narrow_to_lifecycle(expected_context)",
            "capture_fenced_certified_serve_ingress_selector(lifecycle_cut)",
            "selector.into_locked_certified_serve_dequeue(&authenticated)",
            "capture_lifecycle_certified_serve_capacity(target)",
            "owner.admit_selected_certified_serve",
            "registry.attest_ready_certified_serve_request",
            "CertifiedServeSchedulerObservationV1::from_live_cuts",
            "claim_certified_serve_turn_v1",
            "dequeue.commit()",
            "LifecycleCertifiedServeTaskV1::from_dequeued",
            "reservation.preflight_lifecycle_certified_serve(&task)",
            "reservation.commit_lifecycle_certified_serve(task)",
        ),
    )
    turn = item(
        "turn", None, "prepare_and_dispatch_current_certified_serve", "current-height Serve lifecycle transaction"
    )
    if turn is not None:
        turn_tokens = rust_code_tokens(turn.source)
        for marker in (
            "AdmissionDecision::StutterTerminal",
            "AdmissionDecision::ReplayTerminal",
            "LifecycleCertifiedServeTaskV1::from_terminal_replay",
            "settle_certified_serve_negative",
            "CertifiedServeTerminal",
            "CertifiedServeCapacityPending",
            "CertifiedServeCompetingReady",
            "CertifiedServeReplayQueued",
            "CertifiedServeRetry",
            "RestartRequired",
        ):
            if not _token_sequence_positions(turn_tokens, rust_code_tokens(marker)):
                errors.append(
                    f"{paths['turn']}:{turn.line}: Serve lifecycle transaction must retain "
                    f"branch {marker!r}"
                )

    sequence(
        "turn",
        "LaunchedProductionLifecycleV1",
        "drive_completion_pre_gate",
        "Certified-Serve completion transport and fail-stop publication",
        (
            "take_next_lifecycle_completion()",
            "LifecycleCompletionTakeV1::CertifiedServe(completion)",
            "settle_deliver_and_acknowledge(&mut self.owner, &self.services)",
            "LifecycleCertifiedServeCompletionSettlementV1::Claimed",
            "ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted",
            "LifecycleCertifiedServeCompletionSettlementV1::TerminalReplay",
            "ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted",
            "Err(reason)",
            "iroha_logger::error!(%reason, \"lifecycle Certified-Serve completion failed closed\")",
            "self.close_output_for_restart()",
            "ProductionLifecycleCompletionSelectionV1::RestartRequired",
        ),
    )
    sequence(
        "turn",
        "LaunchedProductionLifecycleV1",
        "drive_ready_completion_turn",
        "fresh Ready completion public dispatcher",
        (
            "self.drive_ready_completion_turn_with_required_ordinal(ready, None)",
        ),
    )
    sequence(
        "turn",
        "LaunchedProductionLifecycleV1",
        "drive_ready_completion_turn_with_required_ordinal",
        "fresh Ready completion dispatch after the Producer eligibility gate",
        (
            "self.owner.classify_completion_ready_work(fence)",
            "ProductionCompletionReadyWorkV1::None",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "ProductionCompletionReadyWorkV1::RetainedDirectOutput",
            "ProductionLifecycleCompletionTurnV1::PassThrough(runner)",
            "ProductionCompletionReadyWorkV1::Invalid",
            "self.close_output_for_restart()",
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "Some(ordinal) => owner.dispatch_completion_requiring_ready_ordinal",
            "None => owner.dispatch_completion_with_runner_debt",
            "dispatch_completion_with_runner_debt",
            "ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast",
            "refanout_recovered_lifecycle_signed_broadcast_with_runner_debt",
            "ProductionLifecycleCompletionTurnV1::Selected(selected)",
        ),
    )
    sequence(
        "turn",
        "LaunchedProductionLifecycleV1",
        "drive_completion_turn_for_test",
        "test-only split Completion turn composition",
        (
            "self.drive_completion_pre_gate(runner, lane_work)",
            "ProductionLifecycleCompletionPreGateV1::Selected(selected)",
            "ProductionLifecycleCompletionTurnV1::Selected(selected)",
            "ProductionLifecycleCompletionPreGateV1::Ordinary(runner)",
            "ProductionLifecycleCompletionTurnV1::PassThrough(runner)",
            "ProductionLifecycleCompletionPreGateV1::Ready(ready)",
            "self.drive_ready_completion_turn(ready)",
        ),
        expected_attributes=("#[cfg(test)]",),
    )
    if rust_items(sources["turn"], "drive_completion_turn"):
        errors.append(
            f"{paths['turn']}: superseded production Completion composition "
            "drive_completion_turn must be absent"
        )

    sequence(
        "worker",
        "LifecycleCertifiedServeTaskV1",
        "from_dequeued_parts",
        "opaque Serve worker task construction",
        (
            "HashOf::new(request) != authenticated.request_hash()",
            "&recipient != &authenticated.request().requester",
            "routes.semantic_target() != &recipient",
            "!ownership.validate_exact()",
            "!ownership.matches_message(inbound.message())",
            "!ownership.matches_semantic_origin(&recipient)",
            "!ownership.matches_reply_routes(Some(routes))",
            "inbound.take_ingress_ownership()",
            "inbound.into_message_sender_and_reply_routes()",
            "authority: Some(authority)",
        ),
    )
    sequence(
        "worker",
        "LifecycleIoCapacityReservation<'_>",
        "preflight_lifecycle_certified_serve",
        "Serve worker exact preflight",
        (
            "task.authority_matches_request()",
            "target.kind() == LifecycleIngressIoTargetKind::CertifiedServe",
            "!state.lifecycle_serves.contains_key(&task.lifecycle_ordinal())",
        ),
    )
    sequence(
        "worker",
        "LifecycleIoCapacityReservation<'_>",
        "commit_lifecycle_certified_serve",
        "Serve worker indexed publication",
        (
            "self.preflight_lifecycle_certified_serve(&task)",
            "state.lifecycle_serves.insert(ordinal, tracked)",
            "V2IoCommand::LifecycleCertifiedServe(task)",
            "self.queue.ready.notify_all()",
            "complete()",
        ),
    )
    sequence(
        "worker",
        "PreparedLifecycleCertifiedServeCompletionV1",
        "settle_deliver_and_acknowledge",
        "Serve completion settlement/delivery/acknowledgement",
        (
            "body_readback.take()",
            "result.task.authority.take()",
            "settle_certified_serve_worker_completed",
            "verify_certified_serve_terminal_replay",
            "services.post_to_peer_on_reply_routes",
            "result.task.recipient.clone()",
            "result.task.reply_routes.clone()",
            "result.task.ingress_ownership.clone()",
            "self.queue.acknowledge_lifecycle_certified_serve",
        ),
    )
    sequence(
        "worker",
        "ProductionV2Services",
        "post_to_peer_on_reply_routes",
        "Serve exact-output route publication",
        (
            "reply_routes.semantic_target() != &peer",
            "!ingress_ownership.validate_exact()",
            "!ingress_ownership.matches_reply_routes(Some(&reply_routes))",
            "begin_fail_stop_operation()",
            "if reply_routes.is_empty()",
            "post_block_message_on_reply_routes_while_guarded",
            "ExactFanoutOwnership::SourceRetained",
            "operation.complete()",
        ),
    )
    sequence(
        "worker",
        "ProductionV2Services",
        "drain_lifecycle_certified_serve_completion",
        "dedicated Serve completion drain",
        (
            "take_lifecycle_certified_serve_completion()",
            "V2IoCompletion::LifecycleCertifiedServe(guarded)",
            "prepare_lifecycle_certified_serve_completion",
        ),
    )
    sequence(
        "body_store",
        "V2BodyStore",
        "read_durable_body_for_certified_serve",
        "store-bound Serve body readback",
        (
            "self.load_canonical_wire(receipt)?",
            "store_identity: self.instance_identity()",
        ),
    )
    sequence(
        "projection",
        "super::ProductionLifecycleOwnerV1",
        "settle_certified_serve_worker_completed",
        "worker Serve terminal publication",
        (
            "self.body_store.is_some()",
            "self.body_store_identity.as_ref()",
            "persist_completed_with_worker_readback",
            "publish_certified_serve_terminal",
        ),
        expected_attributes=("#[cfg(any(not(test), feature = \"bls\"))]",),
    )
    sequence(
        "projection",
        "super::ProductionLifecycleOwnerV1",
        "settle_producer_turn_advanced",
        "adjacent ProducerTurn durable terminalization",
        (
            "prepare_producer_turn_terminal_transition",
            "stage_durable_transaction()",
            "reduce_settle_turn(",
            "publish_producer_turn_terminal_transition",
            "persist_exact_staged_successor(&staged)",
            "self.coordinator = staged",
        ),
    )
    sequence(
        "ordinary",
        None,
        "run_lifecycle_active_height",
        "ordinary runner ProducerTurn handoff",
        (
            "claim_producer_turn_for_local_proposal",
            "schedule_local_proposal(",
            "dispatch_lane_work_effects(",
            "producer_turn_attempt_permit(&mut active_runner)",
            "settle_producer_turn_after_local_proposal",
        ),
    )
    sequence(
        "pending",
        None,
        "run_pending_active_height",
        "pending-Kura Serve/ProducerTurn handoff",
        (
            "settle_certified_serve_completion_for_no_clock_recovery",
            "claim_producer_turn_for_no_clock_recovery",
            "producer_turn_attempt_permit(&mut active_runner)",
            "settle_producer_turn_after_no_clock_recovery",
        ),
    )
    sequence(
        "height",
        "LifecycleProducerClaimDispositionV1",
        "permits_ready_completion",
        "fresh Ready Producer eligibility classifier",
        (
            "matches!(self, Self::Eligible | Self::AwaitingLiveApplyQueue { .. })",
        ),
    )
    sequence(
        "height",
        None,
        "drain_lifecycle_v2_ingress",
        "height-runner Serve completion yield",
        (
            "drive_completion_pre_gate(current_turn, lane_work)",
            "PreGate::Ready(ready) if producer_claim.permits_ready_completion()",
            "producer_claim.required_ready_ordinal()",
            "Some(ordinal) => activated."
            "drive_ready_completion_turn_requiring_ordinal(ready, ordinal)",
            "None => activated.drive_ready_completion_turn(ready)",
            "producer_claim.requires_exact_ready_selection()",
            "completion_selection_stops_batch(&selected)",
            "return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim))",
            "ingress_restart_error(&output_guard)",
        ),
    )
    sequence(
        "launch",
        "ProductionLeaderWireIngressBindingV1",
        "bind",
        "leader-wire-only lifecycle ingress binding",
        (
            "ingress.bind_leader_wire_lifecycle_gate(",
            "ingress.close()",
            "gate: Some(gate)",
        ),
    )
    sequence(
        "launch",
        "ProductionLeaderWireIngressBindingV1",
        "retire",
        "leader-wire-only lifecycle ingress retirement",
        (
            "self.gate.as_ref().cloned()",
            "self.ingress.retire_leader_wire_lifecycle_gate(&gate)",
            "self.gate = None",
        ),
    )
    sequence(
        "launch",
        "ProductionLifecycleOwnerV1",
        "launch",
        "leader-wire-only lifecycle launch transfer",
        (
            "leader_wire_launch.open_gate(",
            "leader_wire_restore.scheduler_ordinal_high_watermark()",
            "ProductionLeaderWireIngressBindingV1::bind(",
            "ProductionV2Services::start_with_apply_service(",
            "leader_wire_ingress_binding,",
        ),
        expected_attributes=(
            "#[allow(clippy::result_large_err)]",
            "#[inline(never)]",
        ),
    )

    seal_specs = (
        ("registry", "ConcreteLifecycleWorkRegistry", "attest_ready_certified_serve_request"),
        ("registry", "ConcreteLifecycleWorkRegistry", "project_claimed_certified_serve_dispatch"),
        ("scheduler", "CertifiedServeSchedulerObservationV1", "from_live_cuts"),
        ("scheduler", None, "claim_certified_serve_turn_v1"),
        ("turn", None, "prepare_and_dispatch_current_certified_serve"),
        ("turn", "LaunchedProductionLifecycleV1", "drive_completion_pre_gate"),
        ("turn", "LaunchedProductionLifecycleV1", "drive_ready_completion_turn"),
        (
            "turn",
            "LaunchedProductionLifecycleV1",
            "drive_ready_completion_turn_with_required_ordinal",
        ),
        ("worker", "LifecycleCertifiedServeTaskV1", "from_dequeued_parts"),
        ("worker", "LifecycleIoCapacityReservation<'_>", "preflight_lifecycle_certified_serve"),
        ("worker", "LifecycleIoCapacityReservation<'_>", "commit_lifecycle_certified_serve"),
        ("worker", "PreparedLifecycleCertifiedServeCompletionV1", "settle_deliver_and_acknowledge"),
        ("worker", "ProductionV2Services", "post_to_peer_on_reply_routes"),
        ("worker", "ProductionV2Services", "drain_lifecycle_certified_serve_completion"),
        ("body_store", "V2BodyStore", "read_durable_body_for_certified_serve"),
        ("projection", "super::ProductionLifecycleOwnerV1", "settle_certified_serve_worker_completed"),
        ("projection", "super::ProductionLifecycleOwnerV1", "settle_producer_turn_advanced"),
        ("ordinary", None, "run_lifecycle_active_height"),
        ("pending", None, "run_pending_active_height"),
        ("height", None, "drain_lifecycle_v2_ingress"),
        ("launch", "ProductionLeaderWireIngressBindingV1", "bind"),
        ("launch", "ProductionLeaderWireIngressBindingV1", "retire"),
        ("launch", "ProductionLifecycleOwnerV1", "launch"),
    )
    observed_seal_keys = {
        f"{role}:{owner + '::' if owner else ''}{name}"
        for role, owner, name in seal_specs
    }
    expected_seal_keys = set(_LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256)
    if observed_seal_keys != expected_seal_keys:
        errors.append(
            f"{base}: lifecycle Certified-Serve item seal inventory mismatch; "
            f"missing={sorted(observed_seal_keys - expected_seal_keys)!r}, "
            f"orphaned={sorted(expected_seal_keys - observed_seal_keys)!r}"
        )
    for role, owner, name in seal_specs:
        key = f"{role}:{owner + '::' if owner else ''}{name}"
        expected = _LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256.get(key)
        sealed_attributes = {
            "projection:super::ProductionLifecycleOwnerV1::settle_certified_serve_worker_completed": (
                "#[cfg(any(not(test), feature = \"bls\"))]",
            ),
            "launch:ProductionLifecycleOwnerV1::launch": (
                "#[allow(clippy::result_large_err)]",
                "#[inline(never)]",
            ),
        }.get(key, ())
        sealed = item(
            role,
            owner,
            name,
            f"lifecycle Certified-Serve sealed item {key}",
            expected_attributes=sealed_attributes,
        )
        if expected is not None:
            _require_rust_item_token_sha256(
                paths[role], sealed, expected, f"lifecycle Certified-Serve item {key}", errors
            )

    for role, names in {
        "scheduler_cases": (
            "certified_serve_claim_rolls_back_when_its_exact_carrier_drifted",
            "certified_serve_scheduler_cannot_overtake_its_ready_predecessor",
            "certified_serve_scheduler_creates_exactly_one_live_claim",
        ),
        "ledger_cases": (
            "launched_terminal_owner_settles_exact_worker_body_readback",
            "launched_terminal_owner_rejects_foreign_worker_store_instance",
        ),
        "startup_cases": (
            "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
        ),
    }.items():
        for name in names:
            test_item = _require_rust_item(paths[role], sources[role], name, errors)
            expected_attributes = (
                ("#[cfg(feature = \"bls\")]", "#[test]")
                if role == "startup_cases"
                else ("#[test]",)
            )
            _require_rust_item_context(
                paths[role],
                test_item,
                (),
                f"lifecycle Certified-Serve regression {name}",
                errors,
                expected_attributes=expected_attributes,
            )

    return errors


def _require_lane_predecessor_ordering_source_contracts(
    lane_path: Path,
    lane_ack_items: dict[str, RustItem | None],
    lane_items: dict[str, RustItem | None],
    errors: list[str],
) -> None:
    """Bind raw recovery transport separately from applied-predecessor output."""

    predecessor = lane_ack_items.get(
        "V2LaneWorkAdapter::proposal_predecessor_is_ready_for_progress"
    )
    _require_exact_rust_tokens(
        lane_path,
        predecessor,
        """
fn proposal_predecessor_is_ready_for_progress(
    &self,
    proposal: &LaneBlockProposalV1
) -> bool {
    let finalized_observer = !self.local_can_own_autonomous_payload(proposal)
        && self
            .canonical_finalized_autonomous_payload_for_proposal(proposal)
            .is_ok_and(|payload| payload.is_some());
    if self
        .historical_autonomous_recovery_record_for_proposal(proposal)
        .is_some()
        || self.autonomous_payload_is_expected_for(proposal)
        || finalized_observer
    {
        self.state
            .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(proposal)
    } else {
        self.state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(proposal)
    }
}
""",
        "lane predecessor readiness must dispatch autonomous and ordinary proofs to their exact applied-state authorities",
        errors,
    )
    preflight = lane_ack_items.get("V2LaneWorkAdapter::preflight_effect_insertion")
    _require_exact_rust_tokens(
        lane_path,
        preflight,
        """
fn preflight_effect_insertion(
    &mut self,
    effect: &V2LaneWorkEffect,
) -> Result<Hash, LaneWorkEffectInsertionOutcome> {
    let predecessor_ready = match effect {
        V2LaneWorkEffect::PostLaneBlock { message, .. } => {
            self.outbound_lane_message_predecessor_is_ready(message)
        }
        V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
            self.proposal_predecessor_is_ready_for_progress(&certificate.proposal)
        }
        _ => true,
    };
    if !predecessor_ready {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    if !lane_work_effect_reply_routes_have_valid_shape(effect) {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    let key = lane_work_effect_key(effect);
    if self.effect_keys.contains(&key) {
        return Err(
            if self
                .effects
                .iter_mut()
                .find(|queued| lane_work_effect_key(queued) == key)
                .is_some_and(|queued| merge_lane_work_effect_reply_routes(queued, effect))
            {
                LaneWorkEffectInsertionOutcome::Duplicate
            } else {
                LaneWorkEffectInsertionOutcome::Rejected
            },
        );
    }
    if !lane_work_effect_reply_routes_are_valid(effect) {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    let ordinary_capacity = self.limits.effect_capacity.get();
    let autonomous_new_view_progress = Self::is_autonomous_new_view_progress_effect(effect)
        || self
            .effects
            .iter()
            .any(Self::is_autonomous_new_view_progress_effect);
    let admission_capacity =
        ordinary_capacity.saturating_add(usize::from(autonomous_new_view_progress));
    if self.effects.len() >= admission_capacity {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    Ok(key)
}
""",
        "ordinary lane effect preflight must retain exact identity, bounded capacity, complete reply-route history, and predecessor readiness",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        preflight,
        """
let predecessor_ready = match effect {
    V2LaneWorkEffect::PostLaneBlock { message, .. } => {
        self.outbound_lane_message_predecessor_is_ready(message)
    }
    V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
        self.proposal_predecessor_is_ready_for_progress(&certificate.proposal)
    }
    _ => true,
};
if !predecessor_ready {
    return Err(LaneWorkEffectInsertionOutcome::Rejected);
}
""",
        "lane effect admission must reject every fresh consensus output whose economic predecessor is not durably applied",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::persist_anchored_sessions"),
        """
if !self.proposal_predecessor_is_ready_for_progress(&session.proposal) {
    retained.push_back(session);
    continue;
}
""",
        "anchored lane persistence must retain a certified successor until its economic predecessor is durably applied",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_items.get("reconstruct_durable_lane_certificate"),
        """
if !self.proposal_predecessor_is_ready_for_progress(proposal) {
    return Ok(None);
}
""",
        "lane recovery reconstruction must not emit a successor certificate before its economic predecessor is durably applied",
        errors,
    )
    hydration = lane_ack_items.get("V2LaneWorkAdapter::hydrate_canonical_lane_artifacts")
    for expected, description in (
        (
            """
if historical_records.len() > hydration_capacity {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "historical autonomous recovery inventory exceeds bounded session capacity".to_owned(),
    ));
}
let mut recovered_historical_records = BTreeMap::new();
let mut historical_ready_records = Vec::new();
for record in historical_records {
    validate_historical_autonomous_lane_recovery_record(
        self.state.as_ref(),
        self.kura.as_ref(),
        &record,
    )
    .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
""",
            "historical lane hydration must bound and authenticate every recovery record before using its proposal",
        ),
        (
            """
let proposal = &record.payload.origin_proposal;
let key = AutonomousLanePayloadKey::from(proposal);
if self
    .historical_autonomous_recovery_records
    .get(&key)
    .is_some_and(|existing| existing != &record)
{
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "historical autonomous recovery slot has conflicting immutable records".to_owned(),
    ));
}
if self.kura.lane_block_application_receipt_available(proposal) {
    continue;
}
self.kura
    .validate_historical_autonomous_lane_recovery_record_dependencies(&record)
    .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
""",
            "historical lane hydration must compare retained immutable identity before terminal skipping and validate pending dependencies",
        ),
        (
            """
if committed != 0 && committed != record.reservation_group.ordered_keys.len() {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "historical autonomous recovery FIFO group is only partially committed".to_owned(),
    ));
}
if committed == record.reservation_group.ordered_keys.len() {
    continue;
}
""",
            "historical lane hydration must reject partially committed FIFO groups and skip only complete groups",
        ),
        (
            """
if certified.proposal != *proposal
    || Kura::validate_certified_lane_block_artifact(&certified).is_err()
    || certified.signer_pops.iter().any(|(key, pop)| {
        descriptor
            .validator_set
            .iter()
            .position(|peer| peer.public_key() == key)
            .and_then(|index| record.validator_pops.get(index))
            != Some(pop)
    })
{
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "historical autonomous recovery record conflicts with its certified slot".to_owned(),
    ));
}
continue;
""",
            "historical lane hydration must authenticate the entire certified proposal and exact signer proofs before skipping its slot",
        ),
        (
            """
match recovered_historical_records.entry(key) {
std::collections::btree_map::Entry::Vacant(entry) => {
    entry.insert(record.clone());
}
std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &record => {}
std::collections::btree_map::Entry::Occupied(_) => {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "historical autonomous recovery slot has conflicting immutable records".to_owned(),
    ));
}
}
historical_ready_records.push(record);
""",
            "historical lane hydration must preserve immutable record identity in the staged required inventory",
        ),
        (
            """
if pending_autonomous_anchor_payloads
    .len()
    .saturating_add(recovered_historical_records.len())
    > hydration_capacity
{
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "current and historical autonomous hydration exceeds bounded capacity".to_owned(),
    ));
}
""",
            "historical and current autonomous payloads must share the exact bounded hydration inventory",
        ),
        (
            """
let pending = self.consensus_storage_read(
    self.state.unapplied_lane_block_artifact_heights_snapshot_cached(),
)?;
let mut raw_proposals = Vec::new();
let mut raw_slots = BTreeSet::new();
""",
            "lane hydration must stage required proposals independently of retained cache occupancy and propagate storage failure",
        ),
        (
            """
if !raw_slots.insert((lane_id, lane_block_height)) {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration contains a duplicate or cyclic slot".to_owned(),
    ));
}
""",
            "raw lane hydration must fail stop on a duplicate or cyclic predecessor slot",
        ),
        (
            """
if raw_proposals.len().saturating_add(route_chain.len())
    >= hydration_capacity
{
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration exceeds bounded session capacity".to_owned(),
    ));
}
let artifact = self
    .consensus_storage_read(
        self.kura
            .read_lane_block_artifact_read_only(lane_id, lane_block_height),
    )?
    .ok_or_else(|| {
        self.output_guard.close_admission_for_restart();
        V2LaneWorkError::Persistence(
            "canonical raw lane hydration is missing an indexed artifact".to_owned(),
        )
    })?;
""",
            "raw lane hydration must fail stop at the exact bounded inventory and read only the indexed immutable artifact",
        ),
        (
            """
|| !canonical_shape
|| !self.lane_route_active(
    ownership.lane_id,
    ownership.dataspace_id,
    ownership.lane_incarnation,
    ownership.proposal_height,
)
|| self
    .state
    .committed_block_hash_at_height(ownership.proposal_height)
    != Some(artifact.proposal_block_hash)
""",
            "raw lane hydration must reject malformed, inactive, or non-canonical carrier ownership",
        ),
        (
            """
if canonical.as_slice() != [artifact.clone()] {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration found non-unique ownership".to_owned(),
    ));
}
""",
            "raw lane hydration must require one exact canonical ownership artifact",
        ),
        (
            """
if !canonical_raw_lane_predecessor_matches_proposal(
    self.state.as_ref(),
    self.kura.as_ref(),
    &proposal,
) {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration found a gap or conflicting predecessor".to_owned(),
    ));
}
lane_block_height = previous_height;
""",
            "raw lane hydration must authenticate every unapplied predecessor link before walking backward",
        ),
        (
            """
route_chain.reverse();
raw_proposals.extend(route_chain);
""",
            "raw lane hydration must restore each predecessor chain in forward application order",
        ),
        (
            """
raw_proposals.extend(
    historical_ready_records
        .iter()
        .map(|record| record.payload.origin_proposal.clone()),
);
raw_proposals.sort_by_key(|proposal| {
    let descriptor = &proposal.descriptor;
    (
        descriptor.proposal_height,
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_block_height,
        proposal.proposal_hash,
    )
});
self.lane_sessions
    .insert_recovered_proposals(&raw_proposals)
    .map_err(|error| {
        self.output_guard.close_admission_for_restart();
        V2LaneWorkError::InvalidContext(format!(
            "canonical lane hydration conflicts with retained recovery sources: {error}"
        ))
    })?;
self.historical_autonomous_recovery_records = recovered_historical_records;
self.pending_autonomous_anchor_payloads = pending_autonomous_anchor_payloads;
for record in historical_ready_records {
    self.authorize_autonomous_ready_from_durable_input(
        &record.payload,
        &record.payload.origin_proposal,
        record.historical_context_id,
    )
    .map_err(|error| {
        self.output_guard.close_admission_for_restart();
        V2LaneWorkError::InvalidContext(error)
    })?;
}
Ok(())
""",
            "raw lane hydration must install independent chains in canonical deterministic order as one complete bounded recovery batch before publishing payloads or historical READY",
        ),
        (
            "self.authorize_autonomous_ready_from_durable_input(",
            "lane hydration must authorize historical READY exactly once after complete batch installation",
        ),
        (
            "self.historical_autonomous_recovery_records =",
            "lane hydration must publish the fresh historical inventory exactly once after complete batch installation",
        ),
        (
            "self.pending_autonomous_anchor_payloads =",
            "lane hydration must publish pending payloads exactly once after complete batch installation",
        ),
        (
            "self.lane_sessions",
            "lane hydration must change the session cache exactly once through the complete recovery batch owner",
        ),
    ):
        _require_rust_token_sequence(lane_path, hydration, expected, description, errors)


# Independent canonical owners: no test helper or adapter-local lookalike may
# satisfy the terminal replay/retirement boundary.
_TERMINAL_LANE_SOURCE_OWNERS = {
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs": (
        ("", "validate_terminal_autonomous_availability"),
        ("", "validate_terminal_autonomous_vote"),
        ("", "validate_terminal_autonomous_qc"),
        ("V2LaneWorkAdapter", "insert_lane_vote"),
        ("V2LaneWorkAdapter", "insert_lane_qc"),
        ("V2LaneWorkAdapter", "insert_lane_certificate"),
        ("V2LaneWorkAdapter", "canonical_finalized_autonomous_payload_for_vote_body"),
        ("V2LaneWorkAdapter", "retire_applied_autonomous_sessions"),
        ("V2LaneWorkAdapter", "drive_lane_sessions"),
        ("V2LaneWorkAdapter", "persist_anchored_sessions"),
        ("V2LaneWorkAdapter", "proposal_can_progress"),
        ("From<&LaneBlockProposalV1> for AutonomousLanePayloadKey", "from"),
    ),
    "crates/iroha_core/src/lane_consensus.rs": (
        ("LaneBlockSessionCache", "retained_vote_bodies"),
        ("LaneBlockSessionCache", "preflight_canonical_evidence"),
        ("LaneBlockSessionCache", "retire_applied_proposals"),
        ("LaneBlockSessionCache", "retain_canonical_rollover_evidence"),
        ("", "validate_vote_matches_proposal"),
    ),
    "crates/iroha_core/src/state/autonomous_predecessor_application.rs": (
        ("State", "certified_autonomous_lane_block_is_globally_applied_cached"),
    ),
}

_TERMINAL_LANE_SOURCE_CONTRACTS = (
    ("From<&LaneBlockProposalV1> for AutonomousLanePayloadKey::from", True,
     "terminal retirement selection must retain the full proposal route, incarnation, and lane height",
        """
fn from(proposal: &LaneBlockProposalV1) -> Self {
    let descriptor = &proposal.descriptor;
    Self {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        lane_block_height: descriptor.lane_block_height,
    }
}
"""),
    ('validate_terminal_autonomous_availability', True,
     'terminal availability must bind Prepare READY to the exact canonical executable payload and require Commit without READY',
        """
fn validate_terminal_autonomous_availability(
    phase: CertPhase,
    actual: Option<&iroha_data_model::block::consensus::LanePayloadAvailabilityBodyV1>,
    payload: &LaneExecutablePayloadV1,
) -> Result<(), String> {
    match (phase, actual) {
        (CertPhase::Prepare, Some(actual)) => {
            let expected = lane_payload_availability_body(
                payload,
                &payload.origin_proposal,
                payload.network_id,
                payload.epoch,
            )
            .map_err(|error| error.to_string())?;
            if *actual != expected {
                return Err(
                    "terminal autonomous READY differs from its canonical payload".to_owned(),
                );
            }
            Ok(())
        }
        (CertPhase::Commit, None) => Ok(()),
        _ => Err("terminal autonomous message has an invalid execution role".to_owned()),
    }
}
"""),
    ('validate_terminal_autonomous_vote', True,
     'terminal votes must authenticate the exact proposal, READY committee PoPs, outer signature, and exact availability before success',
        """
fn validate_terminal_autonomous_vote(
    vote: &LaneBlockVoteV1,
    payload: &LaneExecutablePayloadV1,
) -> Result<(), String> {
    crate::lane_consensus::validate_vote_matches_proposal(vote, &payload.origin_proposal)
        .map_err(|error| error.to_string())?;
    vote.validate_ingress(vote.body.phase)
        .map_err(|error| error.to_string())?;
    validate_terminal_autonomous_availability(
        vote.body.phase,
        vote.payload_availability_vote
            .as_ref()
            .map(|ready| &ready.body),
        payload,
    )
}
"""),
    ('validate_terminal_autonomous_qc', True,
     'terminal QCs must authenticate the exact proposal and complete aggregate before exact availability success',
        """
fn validate_terminal_autonomous_qc(
    qc: &LaneBlockQcV1,
    payload: &LaneExecutablePayloadV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), String> {
    validate_winning_lane_qc(qc, &payload.origin_proposal, signer_pops)?;
    validate_terminal_autonomous_availability(
        qc.body.phase,
        qc.payload_availability_qc.as_ref().map(|ready| &ready.body),
        payload,
    )
}
"""),
    ('V2LaneWorkAdapter::retire_applied_autonomous_sessions', True,
     'terminal retirement must authenticate bounded read-only candidates and exact application, preflight full slots atomically, then clean only selected volatile owners without consuming outputs',
        """
fn retire_applied_autonomous_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
    let mut candidates = Vec::new();
    for body in self.lane_sessions.retained_vote_bodies() {
        if let Some(payload) = self
            .canonical_finalized_autonomous_payload_for_vote_body(&body)
            .map_err(V2LaneWorkError::Persistence)?
        {
            candidates.push(payload.origin_proposal);
        }
    }
    // Drained sessions can leave signer locks without a retained vote body.
    // An absent active namespace cannot supply authority for those locks.
    let nexus = self.state.nexus_snapshot();
    for (lane_id, lane_block_height) in self.lane_sessions.rollover_slots() {
        if !nexus
            .lane_config
            .entries()
            .iter()
            .any(|entry| entry.lane_id == lane_id)
        {
            continue;
        }
        let Some(artifact) = self.consensus_storage_read(
            self.kura
                .read_certified_lane_block_artifact_read_only(lane_id, lane_block_height),
        )?
        else {
            continue;
        };
        Kura::validate_certified_lane_block_artifact(&artifact)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_owned()))?;
        if artifact.prepare_qc.payload_availability_qc.is_none() {
            continue;
        }
        if let Some(payload) = self
            .canonical_finalized_autonomous_payload_for_proposal(&artifact.proposal)
            .map_err(V2LaneWorkError::Persistence)?
        {
            for qc in [&artifact.prepare_qc, &artifact.commit_qc] {
                validate_terminal_autonomous_availability(
                    qc.body.phase,
                    qc.payload_availability_qc.as_ref().map(|ready| &ready.body),
                    &payload,
                )
                .map_err(V2LaneWorkError::Persistence)?;
            }
            candidates.push(artifact.proposal);
        }
    }
    let mut applied = BTreeMap::new();
    for proposal in candidates {
        if !self
            .state
            .certified_autonomous_lane_block_is_globally_applied_cached(&proposal)
        {
            continue;
        }
        let key = AutonomousLanePayloadKey::from(&proposal);
        if applied
            .get(&key)
            .is_some_and(|existing| existing != &proposal)
        {
            return Err(V2LaneWorkError::Persistence(
                "canonical applied autonomous proposals conflict at one exact slot".to_owned(),
            ));
        }
        applied.insert(key, proposal);
    }
    let proposals = applied.values().cloned().collect::<Vec<_>>();
    let retired = self
        .lane_sessions
        .retire_applied_proposals(&proposals)
        .map_err(|error| {
            V2LaneWorkError::Persistence(format!(
                "applied autonomous lane retirement conflicts with retained evidence: {error}"
            ))
        })?;
    self.lane_ready_authorizations.retain(|key, _| {
        !applied.contains_key(&AutonomousLanePayloadKey {
            lane_id: key.lane_id,
            dataspace_id: key.dataspace_id,
            lane_incarnation: key.lane_incarnation,
            lane_block_height: key.lane_block_height,
        })
    });
    for key in applied.into_keys() {
        self.discard_volatile_autonomous_payload(key);
    }
    Ok(retired)
}
"""),
    ('LaneBlockSessionCache::retained_vote_bodies', True,
     'terminal inventory must project every retained proposal or vote/QC body in stable order without mutation or inferred global heights',
        """
pub(crate) fn retained_vote_bodies(&self) -> Vec<LaneBlockVoteBodyV1> {
    self.sessions
        .values()
        .filter_map(|session| {
            session
                .proposal
                .as_ref()
                .map(|proposal| proposal.vote_body(CertPhase::Prepare))
                .or_else(|| session.prepare_qc.as_ref().map(|qc| qc.body.clone()))
                .or_else(|| session.commit_qc.as_ref().map(|qc| qc.body.clone()))
                .or_else(|| {
                    session
                        .prepare_votes
                        .values()
                        .next()
                        .map(|vote| vote.body.clone())
                })
                .or_else(|| {
                    session
                        .commit_votes
                        .values()
                        .next()
                        .map(|vote| vote.body.clone())
                })
        })
        .collect()
}
"""),
    ('LaneBlockSessionCache::preflight_canonical_evidence', True,
     'shared canonical preflight must reject exact-committee orphan Commit quorums and conflicting proposal-less Prepare or Commit QCs before mutation',
        """
fn preflight_canonical_evidence<'a>(
    &self,
    canonical_proposal: impl Fn(LaneBlockCommitSlotKey) -> Option<&'a LaneBlockProposalV1>,
) -> Result<(), LaneBlockSessionError> {
    // A drained session can leave independent signer locks behind. Only
    // exact-route signers from the canonical committee contribute a quorum.
    let mut conflicting_lock_quorums =
        BTreeMap::<(LaneBlockCommitSlotKey, Hash), BTreeSet<PeerId>>::new();
    for ((slot, signer), locked_proposal_hash) in &self.commit_vote_locks {
        let Some(canonical) = canonical_proposal(*slot) else {
            continue;
        };
        let descriptor = &canonical.descriptor;
        if descriptor.lane_id != slot.lane_id
            || descriptor.dataspace_id != slot.dataspace_id
            || descriptor.lane_incarnation != slot.lane_incarnation
            || descriptor.lane_block_height != slot.lane_block_height
            || canonical.proposal_hash == *locked_proposal_hash
            || descriptor.validator_set.binary_search(signer).is_err()
        {
            continue;
        }
        conflicting_lock_quorums
            .entry((*slot, *locked_proposal_hash))
            .or_default()
            .insert(signer.clone());
    }
    if conflicting_lock_quorums.iter().any(|((slot, _), signers)| {
        canonical_proposal(*slot).is_some_and(|canonical| {
            usize::try_from(canonical.descriptor.min_quorum)
                .is_ok_and(|quorum| signers.len() >= quorum)
        })
    }) {
        return Err(LaneBlockSessionError::ConflictingProposal);
    }
    for (key, session) in &self.sessions {
        let slot = LaneBlockCommitSlotKey {
            lane_id: key.lane_id,
            dataspace_id: key.dataspace_id,
            lane_incarnation: key.lane_incarnation,
            lane_block_height: key.lane_block_height,
        };
        let Some(canonical) = canonical_proposal(slot) else {
            continue;
        };
        let proposal_conflicts = session
            .proposal
            .as_ref()
            .is_some_and(|proposal| !proposal.same_consensus_identity(canonical));
        let certified_body_conflicts = session
            .prepare_qc
            .as_ref()
            .is_some_and(|qc| validate_qc_matches_proposal(qc, canonical).is_err())
            || session
                .commit_qc
                .as_ref()
                .is_some_and(|qc| validate_qc_matches_proposal(qc, canonical).is_err());
        if session_has_quorum_certificate(session)
            && (LaneBlockSessionKey::from_proposal(canonical) != *key
                || proposal_conflicts
                || certified_body_conflicts)
        {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }
    }
    Ok(())
}
"""),
    ('LaneBlockSessionCache::retire_applied_proposals', True,
     'applied cache retirement must validate the complete exact full-slot target set before mutation and preserve unselected sessions, locks, claims, recency, and capacity',
        """
pub(crate) fn retire_applied_proposals(
    &mut self,
    proposals: &[LaneBlockProposalV1],
) -> Result<usize, LaneBlockSessionError> {
    let mut canonical = BTreeMap::new();
    for proposal in proposals {
        validate_lane_block_proposal(proposal)
            .map_err(LaneBlockSessionError::InvalidProposal)?;
        let descriptor = &proposal.descriptor;
        let slot = LaneBlockCommitSlotKey {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
        };
        if canonical
            .insert(slot, proposal)
            .is_some_and(|existing| existing != proposal)
        {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }
    }
    if canonical.is_empty() {
        return Ok(0);
    }
    self.preflight_canonical_evidence(|slot| canonical.get(&slot).copied())?;
    let before = self
        .sessions
        .len()
        .saturating_add(self.commit_vote_locks.len());
    self.sessions.retain(|key, _| {
        !canonical.contains_key(&LaneBlockCommitSlotKey {
            lane_id: key.lane_id,
            dataspace_id: key.dataspace_id,
            lane_incarnation: key.lane_incarnation,
            lane_block_height: key.lane_block_height,
        })
    });
    self.commit_vote_locks
        .retain(|(slot, _), _| !canonical.contains_key(slot));
    self.slot_proposals.retain(|slot, _| {
        !canonical.contains_key(&LaneBlockCommitSlotKey {
            lane_id: slot.lane_id,
            dataspace_id: slot.dataspace_id,
            lane_incarnation: slot.lane_incarnation,
            lane_block_height: slot.lane_block_height,
        })
    });
    let retained_sessions = &self.sessions;
    self.order.retain(|key| retained_sessions.contains_key(key));
    // Preserve the selected owner of every unrelated shared-payload claim.
    // A complete rebuild could move that claim between retained views.
    self.entrypoint_claims
        .retain(|_, key| retained_sessions.contains_key(key));
    for (key, session) in retained_sessions {
        let Some(proposal) = &session.proposal else {
            continue;
        };
        for entrypoint_hash in &proposal.descriptor.accepted_transaction_hashes {
            self.entrypoint_claims
                .entry(*entrypoint_hash)
                .or_insert(*key);
        }
    }
    let after = self
        .sessions
        .len()
        .saturating_add(self.commit_vote_locks.len());
    Ok(before.saturating_sub(after))
}
"""),
    ('validate_vote_matches_proposal', True,
     'terminal vote proposal authentication must bind the signer and validate paired READY against the exact complete committee and its PoPs',
        """
pub(crate) fn validate_vote_matches_proposal(
    vote: &LaneBlockVoteV1,
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneBlockSessionError> {
    if vote.body != proposal_vote_body(proposal, vote.body.phase) {
        return Err(LaneBlockSessionError::VoteProposalMismatch);
    }
    if !proposal.descriptor.validator_set.contains(&vote.signer) {
        return Err(LaneBlockSessionError::VoteSignerNotInValidatorSet);
    }
    match &vote.payload_availability_vote {
        Some(availability_vote) => {
            if vote.body.phase != CertPhase::Prepare
                || availability_vote.signer != vote.signer
                || validate_availability_body_matches_proposal(&availability_vote.body, proposal)
                    .is_err()
                || availability_vote
                    .validate_against_validator_set(&proposal.descriptor.validator_set)
                    .is_err()
            {
                return Err(LaneBlockSessionError::AvailabilityMismatch);
            }
        }
        None => {}
    }
    Ok(())
}
"""),
    ('State::certified_autonomous_lane_block_is_globally_applied_cached', True,
     'terminal application must require exact route/incarnation frontier identity or an exact authenticated merge receipt and fail closed on malformed frontier bytes',
        """
pub(crate) fn certified_autonomous_lane_block_is_globally_applied_cached(
    &self,
    proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
) -> bool {
    let descriptor = &proposal.descriptor;
    if descriptor.lane_block_height == 0 {
        return false;
    }
    let world = self.world.view();
    let Ok(frontier) = Self::canonical_merged_lane_frontier_from_world(
        &world,
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
    ) else {
        return false;
    };
    frontier
        == (
            descriptor.lane_block_height,
            Some(descriptor.descriptor_hash),
        )
        || self
            .kura
            .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(proposal)
}
"""),
    ('V2LaneWorkAdapter::insert_lane_vote', False,
     'terminal vote ingress must validate canonical authority and authenticate exact applied replay before the first cache clone or hydration',
        """
fn insert_lane_vote(
        &mut self,
        vote: LaneBlockVoteV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if sender != Some(&vote.signer) {
            return V2LaneIngressOutcome::Rejected;
        }
        let finalized_payload =
            match self.finalized_autonomous_ingress_payload_or_fail_stop(&vote.body) {
                Ok(payload) => payload,
                Err(()) => return V2LaneIngressOutcome::Rejected,
            };
        if !self.lane_vote_body_available(&vote.body)
            || !self.lane_vote_authorized(&vote, active_view)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        if let Some(payload) = finalized_payload.as_ref()
            && self
                .state
                .certified_autonomous_lane_block_is_globally_applied_cached(
                    &payload.origin_proposal,
                )
        {
            return if validate_terminal_autonomous_vote(&vote, payload).is_ok() {
                V2LaneIngressOutcome::Duplicate
            } else {
                V2LaneIngressOutcome::Rejected
            };
        }
        let mut next_sessions = self.lane_sessions.clone();
"""),
    ('V2LaneWorkAdapter::insert_lane_qc', False,
     'terminal qc ingress must validate canonical authority and authenticate exact applied replay before the first cache clone or hydration',
        """
fn insert_lane_qc(
        &mut self,
        qc: LaneBlockQcV1,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let finalized_payload =
            match self.finalized_autonomous_ingress_payload_or_fail_stop(&qc.body) {
                Ok(payload) => payload,
                Err(()) => return V2LaneIngressOutcome::Rejected,
            };
        if !self.lane_vote_body_available(&qc.body) || !self.lane_qc_authorized(&qc, active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        let pops = self.pops_for_lane_qc(&qc);
        if let Some(payload) = finalized_payload.as_ref()
            && self
                .state
                .certified_autonomous_lane_block_is_globally_applied_cached(
                    &payload.origin_proposal,
                )
        {
            return if validate_terminal_autonomous_qc(&qc, payload, &pops).is_ok() {
                V2LaneIngressOutcome::Duplicate
            } else {
                V2LaneIngressOutcome::Rejected
            };
        }
        let mut next_sessions = self.lane_sessions.clone();
"""),
    ('V2LaneWorkAdapter::insert_lane_certificate', False,
     'complete autonomous certificates must require exact Prepare and Commit availability before any historical shortcut',
        """
let finalized_payload =
            match self.finalized_autonomous_ingress_payload_or_fail_stop(&prepare_qc.body) {
                Ok(payload) => payload,
                Err(()) => return V2LaneIngressOutcome::Rejected,
            };
        if finalized_payload
            .as_ref()
            .is_some_and(|payload| payload.origin_proposal != proposal)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        if let Some(payload) = finalized_payload.as_ref()
            && [
                (&prepare_qc, CertPhase::Prepare),
                (&commit_qc, CertPhase::Commit),
            ]
            .into_iter()
            .any(|(qc, phase)| {
                validate_terminal_autonomous_availability(
                    phase,
                    qc.payload_availability_qc.as_ref().map(|ready| &ready.body),
                    payload,
                )
                .is_err()
            })
        {
            return V2LaneIngressOutcome::Rejected;
        }
        if proposal.descriptor.proposal_height < self.context.height {
"""),
    ('V2LaneWorkAdapter::canonical_finalized_autonomous_payload_for_vote_body', False,
     'finalized reader must attach the exact global hint before accepting either exact own application or the exact applied predecessor',
        """
let payload = payload
                .attach_global_hint_exact(
                    carrier_hint,
                    height_context.network_id,
                    height_context.epoch,
                )
                .map_err(|error| {
                    format!("finalized autonomous carrier has an invalid global hint: {error}")
                })?;
            let proposal = &payload.origin_proposal;
            let descriptor = &proposal.descriptor;
            // A completed source can outlive its predecessor receipt and frontier.
            // Check the fully attached proposal so an exact historical merge receipt
            // remains usable after the replicated frontier advances again.
            if !self
                .state
                .certified_autonomous_lane_block_is_globally_applied_cached(proposal)
                && !self
                    .state
                    .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
                        proposal,
                    )
            {
                return Err(
                    "finalized autonomous carrier has neither exact application nor an applied predecessor"
                        .to_owned(),
                );
            }
            if !proposal_hashes.insert(proposal.proposal_hash) {
"""),
    ('V2LaneWorkAdapter::drive_lane_sessions', False,
     'lane drive must retire applied owners and close admission on failure before its first signing work',
        """
fn drive_lane_sessions(&mut self) {
        if let Err(error) = self.retire_applied_autonomous_sessions() {
            iroha_logger::error!(%error, "applied autonomous lane cache retirement failed closed");
            self.output_guard.close_admission_for_restart();
            return;
        }
        self.prune_lane_ready_authorizations();
"""),
    ('V2LaneWorkAdapter::persist_anchored_sessions', False,
     'anchored persistence must retire applied owners inside its fail-stop operation before hydration and collection',
        """
pub(crate) fn persist_anchored_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        // Block sync can deliver and apply the current canonical body after
        // this height's adapter was constructed. Rehydrate its exact Kura
        // ownerships at the rollover boundary so a validator which missed the
        // lane CommitQC retains a bounded proposal source for certificate
        // recovery instead of waiting forever with an already-applied block.
        self.retire_applied_autonomous_sessions()?;
        self.hydrate_canonical_lane_artifacts()?;
        self.collect_committed_lane_sessions();
"""),
    ('V2LaneWorkAdapter::proposal_can_progress', True,
     'lane progress must reject exact own application only for authenticated autonomous roles and preserve ordinary recovery eligibility',
        """
fn proposal_can_progress(&self, proposal: &LaneBlockProposalV1) -> bool {
    let historical = self
        .historical_autonomous_recovery_record_for_proposal(proposal)
        .is_some();
    let finalized_autonomous = self
        .canonical_finalized_autonomous_payload_for_proposal(proposal)
        .is_ok_and(|payload| payload.is_some());
    let finalized_observer =
        !self.local_can_own_autonomous_payload(proposal) && finalized_autonomous;
    if (historical || self.autonomous_payload_is_expected_for(proposal) || finalized_autonomous)
        && self
            .state
            .certified_autonomous_lane_block_is_globally_applied_cached(proposal)
    {
        return false;
    }
    if proposal.descriptor.proposal_height != self.context.height
        && !historical
        && !finalized_observer
    {
        return false;
    }
    !self.kura.lane_block_application_receipt_available(proposal)
        && self.proposal_body_available(proposal)
        && (historical
            || finalized_observer
            || !self.decision_pending()
            || self.proposal_is_bound_to_decided_carrier(proposal))
        && self.proposal_predecessor_is_ready_for_progress(proposal)
}
"""),
    ('LaneBlockSessionCache::retain_canonical_rollover_evidence', False,
     'rollover must share the complete canonical quorum preflight before any retained-session mutation',
        """
self.preflight_canonical_evidence(|slot| {
            let evidence_slot = (
                slot.lane_id,
                slot.dataspace_id,
                slot.lane_incarnation,
                slot.lane_block_height,
            );
            if active_slots.get(&evidence_slot) != Some(&true) {
                return None;
            }
            canonical_proposals
                .get(&(slot.lane_id, slot.lane_block_height))
                .and_then(Option::as_ref)
        })?;
        let mut retained_sessions = BTreeMap::new();
"""),
)


def _terminal_lane_source_fidelity_errors(repo_root: Path = ROOT_DIR) -> list[str]:
    """Load terminal replay owners independently from their canonical source files."""

    errors: list[str] = []
    for relative, declarations in _TERMINAL_LANE_SOURCE_OWNERS.items():
        path, source = _read_reviewed_rust_source(
            repo_root, relative, errors, "terminal lane replay and retirement source",
        )
        items = {}
        for owner, name in declarations:
            qualified = f"{owner}::{name}" if owner else name
            item = _require_terminal_lane_owner(path, source, owner, name, errors)
            items[qualified] = item
            digest = _PRODUCTION_TERMINAL_LANE_ITEM_SHA256.get(qualified)
            if digest is not None:
                _require_rust_item_token_sha256(path, item, digest, qualified, errors)
        _require_terminal_lane_source_contracts(path, items, errors)
    return errors


def _require_terminal_lane_owner(
    path: Path, source: str, owner: str, name: str, errors: list[str],
):
    """Resolve an exact free, inherent, or trait item without test/macro substitutes."""

    context = (rust_code_tokens(f"impl {owner}"),) if owner else ()
    qualified = f"{owner}::{name}" if owner else name
    matches = [item for item in rust_items(source, name) if item.brace_context == context]
    if len(matches) != 1:
        errors.append(f"{path}: require exactly one canonical terminal lane owner {qualified}; found {len(matches)}")
        return None
    item = matches[0]
    _require_rust_item_context(path, item, context, qualified, errors)
    return item


def _require_terminal_lane_source_contracts(
    path: Path, items: dict, errors: list[str],
) -> None:
    """Keep semantic ordering and authority contracts independent of refreshed seals."""

    for qualified, whole_item, description, expected in _TERMINAL_LANE_SOURCE_CONTRACTS:
        item = items.get(qualified)
        if item is None:
            continue
        require = _require_exact_rust_tokens if whole_item else _require_rust_token_sequence
        require(path, item, expected, description, errors)
