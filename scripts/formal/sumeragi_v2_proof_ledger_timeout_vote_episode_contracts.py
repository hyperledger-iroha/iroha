# Executed lexically in check_sumeragi_v2_proof_ledger.py.

def _timeout_vote_episode_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
    formal_dir: Path | None = None,
) -> list[str]:
    """Pin the closed Rust TimeoutVote recovery episode and its regressions."""

    base = repo_root / "crates/iroha_core/src/sumeragi"
    ingress_path = base / "mod.rs"
    runner_path = base / "v2_runner.rs"
    runtime_path = base / "v2_runtime.rs"
    worker_path = base / "v2_worker.rs"
    formal_dir = formal_dir or repo_root / "formal/sumeragi_v2"
    errors: list[str] = []
    paths = {
        "ingress": ingress_path,
        "runner": runner_path,
        "runtime": runtime_path,
        "worker": worker_path,
    }
    sources: dict[str, str] = {}
    for role, path in paths.items():
        _loaded_path, sources[role] = _read_reviewed_rust_source(
            repo_root,
            path.relative_to(repo_root).as_posix(),
            errors,
            f"timeout-vote episode {role} source",
        )
    if errors:
        return errors

    ingress_context = (("impl", "FairV2Ingress"),)
    generic_runtime_context = (
        (
            "impl",
            "<",
            "D",
            ":",
            "RuntimeDriver",
            ">",
            "SerializedV2Runtime",
            "<",
            "D",
            ">",
        ),
    )
    concrete_runtime_context = (
        (
            "impl",
            "SerializedV2Runtime",
            "<",
            "SumeragiV2Adapter",
            ">",
        ),
    )
    runtime_test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )
    worker_test_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "test",
            ")",
            "]",
            "pub",
            "(",
            "super",
            ")",
            "mod",
            "tests",
        ),
    )

    items: dict[str, RustItem | None] = {}

    def bind_item(
        key: str,
        role: str,
        name: str,
        context: tuple[tuple[str, ...], ...],
        description: str,
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        path = paths[role]
        item = _require_rust_item(path, sources[role], name, errors)
        items[key] = item
        _require_rust_item_context(
            path,
            item,
            context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        _require_rust_item_token_sha256(
            path,
            item,
            _TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256[key],
            description,
            errors,
        )
        return item

    direct_owner = bind_item(
        "ingress::fair_v2_ingress_is_direct_validator_timeout_vote_owner",
        "ingress",
        "fair_v2_ingress_is_direct_validator_timeout_vote_owner",
        (),
        "direct authenticated validator TimeoutVote owner classifier",
    )
    queue_gate = bind_item(
        "ingress::fair_v2_ingress_queue_gate_verdict",
        "ingress",
        "fair_v2_ingress_queue_gate_verdict",
        (),
        "queue-local fair-ingress barrier verdict",
    )
    shared_selector = bind_item(
        "ingress::select_fair_v2_ingress_candidate",
        "ingress",
        "select_fair_v2_ingress_candidate",
        (),
        "shared strict-before-dependency fair-ingress selector",
    )
    for name, description in (
        (
            "try_recv_if_checked",
            "ordinary checked fair-ingress wrapper",
        ),
        (
            "try_recv_if_checked_retiring_obsolete",
            "test-only ordinary retiring fair-ingress baseline",
        ),
        (
            "try_recv_if_checked_retiring_obsolete_with_barrier_bypass",
            "explicit internal barrier-bypass wrapper",
        ),
        (
            "try_recv_if_at_checked",
            "ordinary timestamped fair-ingress wrapper",
        ),
        (
            "try_recv_if_at_checked_classified",
            "classified fair-ingress selector",
        ),
    ):
        bind_item(
            f"ingress::{name}",
            "ingress",
            name,
            ingress_context,
            description,
            expected_attributes=(
                ("#[cfg(test)]",)
                if name == "try_recv_if_checked_retiring_obsolete"
                else ()
            ),
        )

    queue_gate = bind_item(
        "ingress::fair_v2_ingress_queue_gate_verdict",
        "ingress",
        "fair_v2_ingress_queue_gate_verdict",
        (),
        "queue-local timeout-vote barrier verdict",
    )

    run_inner = bind_item(
        "runner::run_inner",
        "runner",
        "run_inner",
        (),
        "three-mode retained-response runner",
        expected_attributes=("#[allow(clippy::too_many_lines)]",),
    )
    drain = bind_item(
        "runner::drain_v2_ingress",
        "runner",
        "drain_v2_ingress",
        (),
        "three-mode fair-ingress drain",
    )
    validate_owner = bind_item(
        "runtime::RuntimeTimeoutVoteEpisodeOwner::validate_against",
        "runtime",
        "validate_against",
        (("impl", "RuntimeTimeoutVoteEpisodeOwner"),),
        "timeout-vote episode owner cut classifier",
    )
    same_owner = bind_item(
        "runtime::RuntimeTimeoutVoteEpisodeOwner::same_lifecycle_owner_as",
        "runtime",
        "same_lifecycle_owner_as",
        (("impl", "RuntimeTimeoutVoteEpisodeOwner"),),
        "route-neutral immutable timeout-vote lifecycle-owner equality",
    )
    count_transition = bind_item(
        "runtime::RuntimeTimeoutVoteEpisodeAdmissionPlan::count_transition",
        "runtime",
        "count_transition",
        (("impl", "RuntimeTimeoutVoteEpisodeAdmissionPlan"),),
        "exact timeout-vote owner-count projection",
    )
    for name, description in (
        (
            "emitted_timeout_recovery_owner",
            "durably emitted timeout episode authority",
        ),
        (
            "timeout_recovery_episode_allows_clock_blockers",
            "finite timeout-episode clock-blocker gate",
        ),
    ):
        bind_item(
            f"runtime::{name}",
            "runtime",
            name,
            generic_runtime_context,
            description,
        )
    for name, description in (
        (
            "timeout_vote_recovery_candidate_from_fair",
            "pre-runtime timeout-vote candidate projection",
        ),
        (
            "timeout_vote_recovery_candidate_from_runtime",
            "runtime-owned timeout-vote candidate projection",
        ),
        (
            "timeout_vote_recovery_candidate",
            "timeout-vote signer, token, and cut classifier",
        ),
        (
            "timeout_vote_episode_admission_plan",
            "exact timeout-vote owner replacement gate",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "mutating timeout-vote episode handoff",
        ),
        (
            "can_admit_timeout_vote_recovery_episode",
            "read-only timeout-vote episode predicate",
        ),
    ):
        bind_item(
            f"runtime::{name}",
            "runtime",
            name,
            concrete_runtime_context,
            description,
        )

    expected_item_keys = {
        "ingress::fair_v2_ingress_is_direct_validator_timeout_vote_owner",
        "ingress::fair_v2_ingress_queue_gate_verdict",
        "ingress::select_fair_v2_ingress_candidate",
        "ingress::try_recv_if_checked",
        "ingress::try_recv_if_checked_retiring_obsolete",
        "ingress::try_recv_if_checked_retiring_obsolete_with_barrier_bypass",
        "ingress::try_recv_if_at_checked",
        "ingress::try_recv_if_at_checked_classified",
        "ingress::fair_v2_ingress_queue_gate_verdict",
        "runner::run_inner",
        "runner::drain_v2_ingress",
        "runtime::RuntimeTimeoutVoteEpisodeOwner::validate_against",
        "runtime::RuntimeTimeoutVoteEpisodeOwner::same_lifecycle_owner_as",
        "runtime::RuntimeTimeoutVoteEpisodeAdmissionPlan::count_transition",
        "runtime::emitted_timeout_recovery_owner",
        "runtime::timeout_recovery_episode_allows_clock_blockers",
        "runtime::timeout_vote_recovery_candidate_from_fair",
        "runtime::timeout_vote_recovery_candidate_from_runtime",
        "runtime::timeout_vote_recovery_candidate",
        "runtime::timeout_vote_episode_admission_plan",
        "runtime::enqueue_network_with_ingress_ownership",
        "runtime::can_admit_timeout_vote_recovery_episode",
    }
    observed_item_keys = set(_TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256)
    if observed_item_keys != expected_item_keys:
        errors.append(
            "timeout-vote episode Rust source-seal inventory must be exact; "
            f"missing={sorted(expected_item_keys - observed_item_keys)}, "
            f"extra={sorted(observed_item_keys - expected_item_keys)}"
        )

    _require_rust_source_token_sequence(
        ingress_path,
        sources["ingress"],
        """
pub(crate) enum FairV2IngressBarrierBypass {
    None,
    TimeoutVoteEpisode,
}
""",
        "fair-ingress barrier bypass must remain a closed internal two-variant policy",
        errors,
    )
    _require_rust_source_token_sequence(
        ingress_path,
        sources["ingress"],
        """
enum FairV2IngressQueueGateVerdict {
    Blocked,
    Strict,
    Dependency,
}
""",
        "queue-local ingress verdict must remain a closed three-variant policy",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        items["ingress::try_recv_if_checked"],
        "self.try_recv_if_at_checked(Instant::now(), predicate)",
        "ordinary checked ingress must reach only the wrapper which hard-codes no bypass",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        items["ingress::try_recv_if_checked_retiring_obsolete"],
        """
self.try_recv_if_at_checked_classified(
    Instant::now(),
    true,
    FairV2IngressBarrierBypass::None,
    predicate,
)
""",
        "test-only ordinary retirement baseline must pass FairV2IngressBarrierBypass::None",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        items[
            "ingress::try_recv_if_checked_retiring_obsolete_with_barrier_bypass"
        ],
        """
self.try_recv_if_at_checked_classified(Instant::now(), true, barrier_bypass, predicate)
""",
        "only the explicitly named internal wrapper may forward a bypass policy",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        items["ingress::try_recv_if_at_checked"],
        """
self.try_recv_if_at_checked_classified(
    service_attempt_at,
    false,
    FairV2IngressBarrierBypass::None,
    predicate,
)
""",
        "ordinary timestamped ingress must pass FairV2IngressBarrierBypass::None",
        errors,
    )

    _require_exact_rust_tokens(
        ingress_path,
        direct_owner,
        """
fn fair_v2_ingress_is_direct_validator_timeout_vote_owner(
    source: &FairV2IngressSource,
    entry: &FairV2IngressEntry,
) -> bool {
let FairV2IngressSource::Validator(authenticated_source) = source else {
    return false;
};
let Some(token) = entry.leader_wire_token.as_ref() else {
    return false;
};
let Some(ownership) = entry.inbound.ingress_ownership() else {
    return false;
};
fair_v2_ingress_is_timeout_vote(&entry.inbound)
    && entry.inbound.sender() == Some(authenticated_source)
    && entry.inbound.via() == Some(authenticated_source)
    && token.identity.phase == FairV2IngressLeaderWirePhase::TimeoutVote
    && token.source_class == FairV2IngressLeaderWireSourceClass::Control
    && token.identity.semantic_origin == *authenticated_source
    && token.slot.semantic_origin == *authenticated_source
    && ownership.validate_exact()
    && ownership.leader_wire_token() == Some(token)
    && ownership.leader_wire_runtime_receipt().is_none()
    && ownership.runtime_physical_cut().is_none()
    && ownership.physical_admission_ordinal() == Some(entry.admission_ordinal)
}
""",
        "barrier bypass must require a direct validator sender/via, matching token origins, and exact pre-runtime ownership",
        errors,
    )

    selector = items["ingress::try_recv_if_at_checked_classified"]
    _require_rust_token_sequence(
        ingress_path,
        queue_gate,
        """
let timeout_vote_episode_dependency =
    barrier_bypass
        == FairV2IngressBarrierBypass::TimeoutVoteEpisode
        && fair_v2_ingress_is_direct_validator_timeout_vote_owner(source, entry)
        && (leader_wire_barrier.is_some_and(|owner| {
            owner.token.identity.phase
                == FairV2IngressLeaderWirePhase::CertifiedResponse
        }) || (leader_wire_barrier.is_none()
            && (selected_serve_barrier.is_some()
                || certified_body_request_cutoff.is_some())));
let dependency_bypass = !ingress_barrier_allows
    && (serve_fence_escape_dependency
        || timeout_vote_episode_dependency
        || (leader_wire_control_barrier
""",
        "TimeoutVote bypass must be mode-scoped, direct-source checked, limited to a CertifiedResponse leader owner or Serve barrier, and subordinate to a blocked ordinary barrier",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        selector,
        """
let verdict = fair_v2_ingress_queue_gate_verdict(
    source,
    lane,
    index,
    &serve_projection,
    &leader_wire_projection,
    barrier_bypass,
);
(
    entry.admission_ordinal,
    Arc::clone(&entry.inbound),
    verdict,
    entry.leader_wire_token.as_ref().is_some_and(|token| {
        obsolete_leader_wire_tokens.contains(token)
    }),
)
""",
        "classified selector must delegate every candidate to the sealed queue-local verdict",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        selector,
        """
let selected = select_fair_v2_ingress_candidate(
    &candidates,
    |(admission_ordinal, _, gate, obsolete)| (*admission_ordinal, *gate, *obsolete),
    |(_, inbound, _, _)| predicate(inbound.as_ref()),
);
""",
        "barrier dependencies must run only after ordinary candidates and still execute the downstream predicate",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        shared_selector,
        """
for dependency_pass in [false, true] {
    for (source_index, source_candidates) in candidates.iter().enumerate() {
        for candidate in source_candidates {
            let (ordinal, gate, obsolete) = projection(candidate);
            let dependency = gate == FairV2IngressQueueGateVerdict::Dependency;
            if gate == FairV2IngressQueueGateVerdict::Blocked || dependency != dependency_pass {
                continue;
            }
            if obsolete || predicate(candidate) {
                let disposition = if obsolete {
                    FairV2IngressDequeueDisposition::RetireObsolete
                } else {
                    FairV2IngressDequeueDisposition::Admit
                };
                return Some((source_index, ordinal, disposition));
""",
        "shared TimeoutVote selector must preserve strict-before-dependency, Blocked exclusion, downstream predicate, and exact disposition",
        errors,
    )
    if selector is not None:
        selector_tokens = rust_code_tokens(selector.body)
        forbidden_predequeue_claims = [
            token
            for token in (
                "CertifiedResponseClaimMatches",
                "CertifiedResponseClaimAuthorized",
                "claim_certified_body_response",
            )
            if _token_sequence_count(
                selector_tokens,
                rust_code_tokens(token),
            )
        ]
        if forbidden_predequeue_claims:
            errors.append(
                f"{ingress_path}:{selector.line}: the pre-dequeue "
                "CertifiedResponse barrier exception may not require a response "
                "claim acquired only after fair-ingress removal; found "
                f"{forbidden_predequeue_claims!r}"
            )

    _require_rust_source_token_sequence(
        runner_path,
        sources["runner"],
        """
enum V2IngressDrainMode {
    Ordinary,
    CertifiedFenceEscape,
    TimeoutVoteEpisode,
}
""",
        "the runner drain mode must remain exactly Ordinary, CertifiedFenceEscape, and TimeoutVoteEpisode",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        drain,
        """
if mode != V2IngressDrainMode::Ordinary && turn != OuterIngressTurn::Ingress {
    continue;
}
""",
        "non-Ordinary modes must skip Completion and Runtime turns",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        drain,
        """
let barrier_bypass = match mode {
    V2IngressDrainMode::TimeoutVoteEpisode => {
        FairV2IngressBarrierBypass::TimeoutVoteEpisode
    }
    V2IngressDrainMode::Ordinary | V2IngressDrainMode::CertifiedFenceEscape => {
        FairV2IngressBarrierBypass::None
    }
};
""",
        "only TimeoutVoteEpisode mode may use the TimeoutVote barrier bypass; Ordinary and CertifiedFenceEscape must use no bypass",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        drain,
        """
if mode != V2IngressDrainMode::Ordinary {
    let BlockMessage::V2(message) = inbound.message() else {
        return false;
    };
    if message.validate_version().is_err() {
        return false;
    }
    let selected_mode_matches = match mode {
        V2IngressDrainMode::Ordinary => true,
        V2IngressDrainMode::CertifiedFenceEscape => {
            network_ingress_is_certified_fence_escape(&message.payload)
        }
        V2IngressDrainMode::TimeoutVoteEpisode => {
            inbound.ingress_ownership().is_some_and(|ownership| {
                executor.can_admit_timeout_vote_recovery_episode(message, ownership)
            })
        }
    };
    if !selected_mode_matches {
        return false;
    }
}
""",
        "CertifiedFenceEscape and TimeoutVoteEpisode must be pure disjoint drains with only their reviewed predicate",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        run_inner,
        """
if response_backpressured {
    if executor.retained_response_may_admit_certified_fence_escape() {
        drain_v2_ingress(
            &block_rx,
            &mut executor,
            &mut services,
            &mut lane_work,
            &output_guard,
            kura.as_ref(),
            &common_config.key_pair,
            block_sync_server
                .as_mut()
                .expect("block-sync server initialized before ingress"),
            &mut block_sync,
            &mut block_sync_request,
            &mut npos_vrf,
            V2IngressDrainMode::CertifiedFenceEscape,
            1,
        )?;
    }
    drain_v2_ingress(
        &block_rx,
        &mut executor,
        &mut services,
        &mut lane_work,
        &output_guard,
        kura.as_ref(),
        &common_config.key_pair,
        block_sync_server
            .as_mut()
            .expect("block-sync server initialized before ingress"),
        &mut block_sync,
        &mut block_sync_request,
        &mut npos_vrf,
        V2IngressDrainMode::TimeoutVoteEpisode,
        1,
    )?;
    executor.reconcile_retained_response_certified_fence_escape_phase();
    advance_pacemaker_once(&block_rx, &mut executor, &mut services)?;
""",
        "retained-response backpressure must give a conditional one-shot certificate drain and then an unconditional distinct TimeoutVote drain before pacemaker service",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        run_inner,
        """
if !recovering_interrupted_tip {
    drain_v2_ingress(
        &block_rx,
        &mut executor,
        &mut services,
        &mut lane_work,
        &output_guard,
        kura.as_ref(),
        &common_config.key_pair,
        block_sync_server
            .as_mut()
            .expect("block-sync server initialized before ingress"),
        &mut block_sync,
        &mut block_sync_request,
        &mut npos_vrf,
        V2IngressDrainMode::CertifiedFenceEscape,
        1,
    )?;
}
let completion_evidence = services
    .certified_serve_predecessor_completion_evidence(
        executor.remaining_completion_capacity() != 0,
        serve_barrier.scheduler_ordinal(),
    )
    .map_err(V2RunnerError::Service)?;
let predecessor_witness = executor
    .exact_serve_predecessor_episode_witness(
        Instant::now(),
        serve_barrier.scheduler_ordinal(),
        completion_evidence,
    )?;
if let Some(witness) = predecessor_witness {
    let _ = services
        .observe_certified_serve_predecessor_episode_witness(
            serve_barrier,
            witness,
        )
        .map_err(V2RunnerError::Service)?;
}
older_predecessor_remains = predecessor_witness.is_some();
services
    .finish_certified_serve_runtime_episode_turn(
        serve_barrier,
        older_predecessor_remains,
    )
    .map_err(V2RunnerError::Service)?;
""",
        "selected Serve certificate escape must freshly project, re-publish, and "
        "consume the exact predecessor witness inside the claimed older-runtime "
        "episode before that one-shot claim is finished",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        run_inner,
        """
services
    .finish_certified_serve_runtime_episode_turn(
        serve_barrier,
        older_predecessor_remains,
    )
    .map_err(V2RunnerError::Service)?;
}
service_certified_serve_barrier_liveness_turn(
    recovering_interrupted_tip,
    claimed_older_runtime_episode,
    |action| match action {
        CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode => {
            drain_v2_ingress(
                &block_rx,
                &mut executor,
                &mut services,
                &mut lane_work,
                &output_guard,
                kura.as_ref(),
                &common_config.key_pair,
                block_sync_server
                    .as_mut()
                    .expect("block-sync server initialized before ingress"),
                &mut block_sync,
                &mut block_sync_request,
                &mut npos_vrf,
                V2IngressDrainMode::TimeoutVoteEpisode,
                1,
            )
        }
        CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix => {
            if let Some(timeout_recovery_cut) =
                executor.timeout_recovery_lifecycle_cut()?
            {
                services.drain_timeout_recovery_prefix_completion(
                    &mut executor,
                    timeout_recovery_cut,
                )?;
            }
            Ok(())
        }
        CertifiedServeBarrierLivenessAction::Pacemaker => {
            advance_pacemaker_once(&block_rx, &mut executor, &mut services)
        }
    },
)?;
""",
        "selected Serve must keep certificate escape inside the one-shot claim and map the complete timeout-recovery suffix outside that claim",
        errors,
    )

    _require_rust_source_token_sequence(
        runtime_path,
        sources["runtime"],
        """
enum RuntimeTimeoutVoteEpisodeDisposition {
    PreCutDescent,
    RestoredDescent,
    FreshReplenishment,
}
""",
        "timeout-vote episode disposition must remain a closed descent/restored/replenishment classification",
        errors,
    )
    _require_exact_rust_tokens(
        runtime_path,
        same_owner,
        """
fn same_lifecycle_owner_as(&self, other: &Self) -> bool {
    self.token == other.token
}
""",
        "timeout-vote retries must compare only the immutable retained token and keep the incumbent carrier/disposition",
        errors,
    )
    for sequence, description in (
        (
            """
RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent => {
    self.token.scheduler_ordinal() < timeout_ordinal
        && self.token.admission_ordinal() == self.carrier_physical_ordinal
}
""",
            "A=P with scheduler below timeout, including P at or above the physical cut, must be descent",
        ),
        (
            """
RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent => {
    self.token.scheduler_ordinal() < timeout_ordinal
        && self.token.admission_ordinal() < self.carrier_physical_ordinal
        && admission_ordinal < physical_cut
        && carrier_physical_ordinal >= physical_cut
}
""",
            "restored descent must cross the physical cut with a strictly newer carrier",
        ),
        (
            """
RuntimeTimeoutVoteEpisodeDisposition::FreshReplenishment => {
    self.token.scheduler_ordinal() > timeout_ordinal
        && self.token.admission_ordinal() == self.carrier_physical_ordinal
        && carrier_physical_ordinal >= physical_cut
}
""",
            "fresh replenishment must be strictly above the timeout owner and own its post-cut carrier",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            validate_owner,
            sequence,
            description,
            errors,
        )
    _require_rust_token_sequence(
        runtime_path,
        count_transition,
        """
Self::NonCandidate => (0, 0),
Self::FirstAdmission {
    candidate,
    prospective,
} => {
    debug_assert_eq!(prospective.get(&candidate.slot), Some(&candidate.owner));
    (0, 1)
}
Self::CoalescedRetry {
    candidate,
    prospective,
} => {
    debug_assert!(
        prospective
            .get(&candidate.slot)
            .is_some_and(|incumbent| incumbent
                .same_lifecycle_owner_as(&candidate.owner))
    );
    (1, 1)
}
""",
        "timeout-vote admission plans must project exactly 0→0, 0→1, and 1→1",
        errors,
    )

    emitted_owner = items["runtime::emitted_timeout_recovery_owner"]
    _require_rust_token_sequence(
        runtime_path,
        emitted_owner,
        """
if !self.timeout_emitted {
    return Ok(None);
}
if self.timeout_owner.is_some() || self.timeout_owner_physical_cut.is_some() {
    return Err(EnqueueError::FailClosed);
}
let Some(episode) = self.timeout_recovery_episode.as_ref() else {
    return Err(EnqueueError::FailClosed);
};
if !episode.validate_exact()
    || episode.tag != self.round_tag
    || episode.physical_cut > self.ingress_physical_cut
    || episode.timeout_vote_owner_universe != self.driver.timeout_vote_owner_universe()
{
    return Err(EnqueueError::FailClosed);
}
Ok(Some(episode.timeout_owner.clone()))
""",
        "only the exact emitted current-view episode and frozen owner universe may authorize recovery",
        errors,
    )
    blocker_gate = items["runtime::timeout_recovery_episode_allows_clock_blockers"]
    _require_rust_token_sequence(
        runtime_path,
        blocker_gate,
        """
if !self.timeout_emitted
    || !episode.validate_exact()
    || episode.tag != self.round_tag
    || episode.pre_frozen_retransmit.is_some()
    || blockers.timeout
{
    return Ok(false);
}
if !blockers.retransmit {
    return Ok(true);
}
""",
        "the episode may cross neither the absolute timeout nor a pre-frozen retransmit",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        blocker_gate,
        """
(Some(owner), Some(cut)) => Ok(owner.validate_exact()
    && owner.lifecycle_ordinal() > episode.timeout_owner.lifecycle_ordinal()
    && cut != 0
    && cut <= self.ingress_physical_cut),
(None, None) => Err(EnqueueError::FailClosed),
(Some(_), None) | (None, Some(_)) => Err(EnqueueError::FailClosed),
""",
        "only a complete fresh retransmit pair above the timeout owner may remain outside the frozen recovery prefix",
        errors,
    )

    _require_rust_token_sequence(
        runtime_path,
        items["runtime::timeout_vote_recovery_candidate_from_fair"],
        """
let Some(token) = ingress_ownership.leader_wire_token() else {
    return Ok(None);
};
let Some(physical_ordinal) = ingress_ownership.physical_admission_ordinal() else {
    return Ok(None);
};
self.timeout_vote_recovery_candidate(payload, token, physical_ordinal)
""",
        "pre-runtime candidate projection must retain the exact token and physical carrier",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        items["runtime::timeout_vote_recovery_candidate_from_runtime"],
        """
match (token, physical) {
    (Some(token), Some((physical_ordinal, _))) => {
        self.timeout_vote_recovery_candidate(payload, token, physical_ordinal)
    }
    (None, None) => Ok(None),
    (Some(_), None) | (None, Some(_)) => Err(EnqueueError::FailClosed),
}
""",
        "runtime candidate projection must reject partial leader-wire ownership",
        errors,
    )
    candidate = items["runtime::timeout_vote_recovery_candidate"]
    for sequence, description in (
        (
            """
let signer_index = usize::try_from(vote.signer).map_err(|_| EnqueueError::FailClosed)?;
let signer = context
    .roster
    .get(signer_index)
    .ok_or(EnqueueError::FailClosed)?;
""",
            "TimeoutVote signer must resolve through a bounded frozen-roster lookup",
        ),
        (
            """
|| token.identity.semantic_origin != signer.validator
|| token.slot.semantic_origin != signer.validator
""",
            "the token identity and source slot must equal the resolved TimeoutVote signer",
        ),
        (
            """
let disposition = if token.scheduler_ordinal() < timeout_ordinal
    && token.admission_ordinal() == physical_ordinal
{
    RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent
} else if token.scheduler_ordinal() < timeout_ordinal
    && token.admission_ordinal() < physical_ordinal
{
    RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent
} else if token.scheduler_ordinal() > timeout_ordinal
    && token.admission_ordinal() == physical_ordinal
{
    RuntimeTimeoutVoteEpisodeDisposition::FreshReplenishment
} else if token.scheduler_ordinal() == timeout_ordinal {
    return Err(EnqueueError::FailClosed);
} else {
    return Ok(None);
};
""",
            "candidate classification must treat the A=P,S<T straddle as descent and keep replenishment strictly above timeout",
        ),
        (
            """
if !owner.validate_against(timeout_ordinal, episode.physical_cut) {
    return Err(EnqueueError::FailClosed);
}
""",
            "every classified owner must pass the exact frozen-cut validator",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            candidate,
            sequence,
            description,
            errors,
        )

    plan = items["runtime::timeout_vote_episode_admission_plan"]
    for sequence, description in (
        (
            """
if episode.timeout_vote_owner_universe != current_universe
    || candidate.slot.phase != FairV2IngressLeaderWirePhase::TimeoutVote
    || candidate.slot.chunk_index.is_some()
    || !roster.contains(&candidate.slot.semantic_origin)
    || candidate.slot != candidate.owner.token.slot
""",
            "the admission plan must retain the frozen roster slot universe and exact token slot",
        ),
        (
            """
Some(incumbent) if !incumbent.same_lifecycle_owner_as(&candidate.owner) => {
    return Err(EnqueueError::FailClosed);
}
Some(_) => RuntimeTimeoutVoteEpisodeAdmissionPlan::CoalescedRetry {
""",
            "a different owner must fail closed before an exact incumbent can coalesce",
        ),
        (
            """
None => {
    prospective.insert(candidate.slot.clone(), candidate.owner.clone());
    RuntimeTimeoutVoteEpisodeAdmissionPlan::FirstAdmission {
""",
            "only an empty frozen source slot may increase the episode owner count",
        ),
        (
            """
if prospective.len() > roster.len()
    || prospective.iter().any(|(slot, owner)| {
""",
            "the prospective owner map must remain bounded by and validated against the roster",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            plan,
            sequence,
            description,
            errors,
        )

    predicate = items["runtime::can_admit_timeout_vote_recovery_episode"]
    for sequence, description in (
        (
            """
if !matches!(
    &message.payload,
    wire::ConsensusMessageV2Payload::TimeoutVote(_)
) || !wire_payload_matches_current_strict_timeout_recovery_round(
    &message.payload,
    self.driver.wire_context(),
    self.round_tag,
) {
    return false;
}
""",
            "the read-only predicate must accept only strict current-view TimeoutVote shape",
        ),
        (
            """
if ownership.leader_wire_runtime_receipt().is_some()
    || !ownership.validate_exact()
    || !ownership.matches_message(&outer)
    || ownership.runtime_physical_cut().is_some()
    || ownership.runtime_lifecycle_ordinal() != Some(token.scheduler_ordinal())
""",
            "the read-only predicate must require exact receipt-free pre-runtime ownership",
        ),
        (
            """
if !matches!(
    self.timeout_vote_recovery_candidate_from_fair(&message.payload, ownership)
        .and_then(|candidate| self.timeout_vote_episode_admission_plan(candidate)),
    Ok(plan) if plan.count_transition() != (0, 0)
) {
    return false;
}
""",
            "the read-only predicate must require a reviewed count-changing or coalesced episode plan",
        ),
        (
            """
if !matches!(
    self.timeout_recovery_episode_allows_clock_blockers(blockers),
    Ok(true)
) {
    return false;
}
self.can_admit_pre_runtime_leader_wire(message, message, CommandClass::Progress, ownership)
    == Some(true)
""",
            "the episode predicate must retain clock authority and ordinary Progress capacity",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            predicate,
            sequence,
            description,
            errors,
        )

    enqueue = items["runtime::enqueue_network_with_ingress_ownership"]
    if enqueue is not None:
        enqueue_tokens = rust_code_tokens(enqueue.body)
        ordered_sequences = (
            "let timeout_vote_recovery_candidate = match self.timeout_vote_recovery_candidate_from_runtime(",
            "let timeout_vote_admission_plan = match self.timeout_vote_episode_admission_plan(timeout_vote_recovery_candidate)",
            "let preflight = self.command_admission_preflight(tag, class, &command)",
            ".enqueue_authenticated_with_ingress_ownership_and_owner(",
            "Ok(owner) => { if self.register_leader_wire_runtime_receipt(&leader_wire_registration)",
            "episode.admitted_timeout_vote_owners = prospective;",
        )
        positions = [
            _token_sequence_positions(enqueue_tokens, rust_code_tokens(sequence))
            for sequence in ordered_sequences
        ]
        if any(len(position) != 1 for position in positions) or any(
            left[0] >= right[0]
            for left, right in zip(positions, positions[1:])
            if left and right
        ):
            errors.append(
                f"{runtime_path}:{enqueue.line}: timeout-vote owner replacement "
                "must be rejected before preflight and queue publication, while "
                "the prospective map may publish only after enqueue and durable "
                "Runtime-receipt registration"
            )

    regression_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _TIMEOUT_VOTE_EPISODE_RUNTIME_REGRESSION_SHA256.items()
    ):
        item = _require_rust_item(runtime_path, sources["runtime"], name, errors)
        regression_items[name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            runtime_test_context,
            f"timeout-vote episode regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"timeout-vote episode regression {name}",
            errors,
        )

    tc_regression = regression_items.get(
        "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner"
    )
    for sequence, description in (
        (
            """
let source = context.roster[1].validator.clone();
let post_snapshot_source = context.roster[2].validator.clone();
""",
            "the TC fixture must use a distinct real post-snapshot source",
        ),
        (
            """
leader_wire_ingress.try_push(InboundBlockMessage::new(
    BlockMessage::V2(message.clone()),
    Some(post_snapshot_source),
))
""",
            "the post-snapshot TC must acquire real fair-ingress ownership",
        ),
        (
            """
.try_recv_if(|inbound| {
    let BlockMessage::V2(candidate) = inbound.message() else {
        return false;
    };
    inbound.ingress_ownership().is_some_and(|ownership| {
        runtime
            .can_admit_network_message_with_ingress_ownership(candidate, ownership)
    })
})
.is_none()
""",
            "the production predicate must retain the post-snapshot TC behind timeout",
        ),
        (
            """
let mut fresh_inbound = leader_wire_ingress
    .try_recv()
    .expect("force the real post-snapshot carrier across the test-only dequeue seam");
""",
            "the test-only forced dequeue must consume the same real TC carrier",
        ),
        (
            """
assert_eq!(runtime.queued_commands(), queued_before_fresh);
assert_eq!(
    runtime.leader_wire_runtime_receipts.len(),
    receipts_before_fresh
);
assert_eq!(
    runtime.pending_leader_wire_terminals.len(),
    terminals_before_fresh
);
""",
            "fresh post-cut TC backpressure must publish no queue, receipt, or terminal state",
        ),
        (
            """
assert!(
    restored_receipt.token().admission_ordinal() < restored_physical_ordinal,
    "the admitted carrier must be an exact physical replay"
);
""",
            "only the retained strictly older TC replay may cross the timeout cut",
        ),
        (
            """
assert!(matches!(
    timeout_effects.as_slice(),
    [AdapterEffect::Sign {
        request: SignRequest::TimeoutVote(_),
        ..
    }]
));
""",
            "the frozen timeout must execute before the retained TC",
        ),
        (
            """
assert!(matches!(
    tc_effects.as_slice(),
    [AdapterEffect::EnterView { tag, .. }] if tag.view() == 1
));
""",
            "the retained TC must advance the view after the timeout turn",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            tc_regression,
            sequence,
            description,
            errors,
        )
    if tc_regression is not None:
        tc_tokens = rust_code_tokens(tc_regression.body)
        ordered_sequences = (
            "leader_wire_ingress.try_push(InboundBlockMessage::new(",
            "let timeout_owner = runtime.frozen_timeout_owner_for_test(deadline)",
            "leader_wire_ingress.try_recv_if(",
            "let mut fresh_inbound = leader_wire_ingress.try_recv()",
        )
        positions = [
            _token_sequence_positions(tc_tokens, rust_code_tokens(sequence))
            for sequence in ordered_sequences
        ]
        if any(len(position) != 1 for position in positions) or any(
            left[0] >= right[0]
            for left, right in zip(positions, positions[1:])
            if left and right
        ):
            errors.append(
                f"{runtime_path}:{tc_regression.line}: real post-snapshot TC "
                "admission must precede timeout freezing, production rejection, "
                "and the forced test dequeue in that order"
            )
        if _token_sequence_count(
            tc_tokens,
            rust_code_tokens(
                "fresh_runtime.first.physical_admission_ordinal ="
            ),
        ) or _token_sequence_count(
            tc_tokens,
            rust_code_tokens(
                "fresh_runtime.latest.physical_admission_ordinal ="
            ),
        ):
            errors.append(
                f"{runtime_path}:{tc_regression.line}: the fresh post-cut TC "
                "fixture may not fabricate physical ownership fields"
            )

    hybrid_regression = regression_items.get(
        "pre_timeout_scheduler_owner_may_publish_across_the_physical_snapshot"
    )
    for sequence, description in (
        (
            "assert_eq!(token.admission_ordinal(), physical_ordinal);",
            "the straddled descent must retain A=P",
        ),
        (
            "assert!(u128::from(physical_ordinal) >= timeout_physical_cut);",
            "the straddled descent must exercise P at or above the physical cut",
        ),
        (
            "assert!(token.scheduler_ordinal() < timeout_owner.lifecycle_ordinal());",
            "the straddled descent must exercise S below timeout",
        ),
        (
            """
assert_eq!(
    candidate.owner.disposition,
    RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent
);
""",
            "A=P≥Cut,S<timeout must classify as descent",
        ),
        (
            "assert!(!runtime.fail_closed);",
            "the valid hybrid descent must not latch fail closed",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            hybrid_regression,
            sequence,
            description,
            errors,
        )

    fresh_regression = regression_items.get(
        "two_fresh_timeout_vote_slots_replenish_once_and_close_a_four_validator_view"
    )
    for sequence, description in (
        (
            "assert!(token.scheduler_ordinal() > timeout_ordinal);",
            "fresh replenishment regression must be strictly above timeout",
        ),
        (
            "assert_eq!(first_plan.count_transition(), (0, 1));",
            "first fresh source admission must project 0→1",
        ),
        (
            "assert_eq!(coalesced_plan.count_transition(), (1, 1));",
            "same-owner retry must project 1→1",
        ),
        (
            """
assert_eq!(
    runtime.timeout_vote_episode_admission_plan(Some(replaced_token)),
    Err(EnqueueError::FailClosed),
    "a different token cannot replace the occupied source slot"
);
""",
            "same-slot token replacement must fail closed",
        ),
        (
            """
assert_eq!(
    runtime
        .timeout_recovery_episode
        .as_ref()
        .expect("rejected replacement cannot retire the episode")
        .admitted_timeout_vote_owners,
    owners_before_replacement,
    "replacement is rejected before queue or episode refinement"
);
""",
            "rejected replacement must not publish episode refinement",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            fresh_regression,
            sequence,
            description,
            errors,
        )

    restored_regression = regression_items.get(
        "restored_pre_runtime_timeout_vote_releases_only_an_absolute_timeout_cut"
    )
    for sequence, description in (
        (
            "assert_eq!(non_candidate.count_transition(), (0, 0));",
            "unrelated ingress must project 0→0",
        ),
        (
            "RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent",
            "the original pre-cut carrier must remain descent",
        ),
        (
            "RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent",
            "the later retained carrier must remain restored descent",
        ),
        (
            """
restored_candidate
    .owner
    .same_lifecycle_owner_as(&pre_cut_candidate.owner)
""",
            "a restored physical carrier must retain the immutable timeout-vote token",
        ),
        (
            """
assert_ne!(
    restored_candidate.owner.carrier_physical_ordinal,
    pre_cut_candidate.owner.carrier_physical_ordinal
);
assert_ne!(
    restored_candidate.owner.disposition,
    pre_cut_candidate.owner.disposition
);
""",
            "carrier and derived disposition must remain outside lifecycle-owner identity",
        ),
        (
            """
assert!(matches!(
    &coalesced_retry,
    RuntimeTimeoutVoteEpisodeAdmissionPlan::CoalescedRetry { .. }
));
assert_eq!(coalesced_retry.count_transition(), (1, 1));
""",
            "same-token restored retry must be a 1→1 coalesced admission",
        ),
        (
            """
.admitted_timeout_vote_owners
.get(&restored_slot),
Some(&restored_owner),
"coalescing must retain rather than replace the incumbent carrier classification"
""",
            "coalescing must retain the incumbent carrier and disposition",
        ),
        (
            "assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);",
            "TimeoutVote recovery must not consume certified capacity",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            restored_regression,
            sequence,
            description,
            errors,
            count=2
            if description
            == "TimeoutVote recovery must not consume certified capacity"
            else 1,
        )

    reactivation_regression = regression_items.get(
        "restored_timeout_vote_reactivation_binds_fresh_carrier_before_runtime_admission"
    )
    for sequence, description in (
        (
            "assert!(token.admission_ordinal() < replay_physical_ordinal);",
            "restored TimeoutVote must bind a fresh physical carrier",
        ),
        (
            "assert!(u128::from(replay_physical_ordinal) >= timeout_cut);",
            "restored TimeoutVote must exercise the post-cut carrier",
        ),
        (
            """
assert!(
    restored_ingress
        .try_recv_if(|inbound| {
""",
            "restored TimeoutVote must remain at the real checked selector until its runtime predicate succeeds",
        ),
        (
            "assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);",
            "restored TimeoutVote reactivation must retain ordinary Progress capacity",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            reactivation_regression,
            sequence,
            description,
            errors,
        )

    ingress_test_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "test",
            ")",
            "]",
            "mod",
            "authoritative_runtime_gate_tests",
        ),
    )
    for name, expected_sha256 in (
        _TIMEOUT_VOTE_EPISODE_INGRESS_REGRESSION_SHA256.items()
    ):
        item = _require_rust_item(ingress_path, sources["ingress"], name, errors)
        _require_rust_item_context(
            ingress_path,
            item,
            ingress_test_context,
            f"timeout-vote CertifiedResponse-barrier regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            ingress_path,
            item,
            expected_sha256,
            f"timeout-vote CertifiedResponse-barrier regression {name}",
            errors,
        )
        for sequence, description in (
            (
                """
assert_eq!(
    earliest.token.identity.phase,
    super::FairV2IngressLeaderWirePhase::CertifiedResponse
);
""",
                "the leader-wire regression must freeze an exact CertifiedResponse-phase owner",
            ),
            (
                """
.try_recv_if_checked_retiring_obsolete(is_timeout_vote)
.expect("ordinary selection preserves the response barrier")
.is_none()
""",
                "ordinary ingress must preserve the CertifiedResponse barrier",
            ),
            (
                """
.try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
    super::FairV2IngressBarrierBypass::TimeoutVoteEpisode,
    |_| false,
)
""",
                "the leader-wire episode exception must still execute and honor its runtime predicate",
            ),
            (
                """
.try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
    super::FairV2IngressBarrierBypass::TimeoutVoteEpisode,
    is_timeout_vote,
)
""",
                "the exact direct TimeoutVote must cross only the reviewed leader-wire phase",
            ),
            (
                """
record.status == super::FairV2IngressLeaderWireStatus::Ingress
    && record.token.identity.phase
        == super::FairV2IngressLeaderWirePhase::CertifiedResponse
""",
                "the CertifiedResponse barrier owner must remain retained after TimeoutVote selection",
            ),
        ):
            _require_rust_token_sequence(
                ingress_path,
                item,
                sequence,
                description,
                errors,
            )

    for name, expected_sha256 in (
        _TIMEOUT_VOTE_EPISODE_WORKER_REGRESSION_SHA256.items()
    ):
        item = _require_rust_item(worker_path, sources["worker"], name, errors)
        _require_rust_item_context(
            worker_path,
            item,
            worker_test_context,
            f"timeout-vote Serve-barrier regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            worker_path,
            item,
            expected_sha256,
            f"timeout-vote Serve-barrier regression {name}",
            errors,
        )
        for sequence, description in (
            (
                """
ingress
    .try_recv_if_checked_retiring_obsolete(|inbound| {
""",
                "ordinary selection must exercise the selected Serve barrier without bypass",
            ),
            (
                """
.try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
    FairV2IngressBarrierBypass::TimeoutVoteEpisode,
    |_| false,
)
""",
                "the episode bypass must still reject when its downstream predicate rejects",
            ),
            (
                """
.try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
    FairV2IngressBarrierBypass::TimeoutVoteEpisode,
    |inbound| {
""",
                "the exact direct TimeoutVote must reach the authoritative predicate",
            ),
            (
                """
assert_eq!(
    serve_gate
        .selected_barrier()
        .expect("inspect the retained Serve barrier")
        .map(|barrier| barrier.carrier_ordinal()),
    Some(1)
);
""",
                "TimeoutVote predicate service must retain the selected Serve owner",
            ),
        ):
            _require_rust_token_sequence(
                worker_path,
                item,
                sequence,
                description,
                errors,
            )

    expected_runtime_tests = {
        "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
        "restored_pre_runtime_timeout_vote_releases_only_an_absolute_timeout_cut",
        "pre_timeout_scheduler_owner_may_publish_across_the_physical_snapshot",
        "two_fresh_timeout_vote_slots_replenish_once_and_close_a_four_validator_view",
        "restored_timeout_vote_reactivation_binds_fresh_carrier_before_runtime_admission",
    }
    observed_runtime_tests = set(_TIMEOUT_VOTE_EPISODE_RUNTIME_REGRESSION_SHA256)
    if observed_runtime_tests != expected_runtime_tests:
        errors.append(
            "timeout-vote runtime regression seal inventory must be exact; "
            f"missing={sorted(expected_runtime_tests - observed_runtime_tests)}, "
            f"extra={sorted(observed_runtime_tests - expected_runtime_tests)}"
        )
    expected_worker_tests = {
        "timeout_vote_episode_reaches_its_predicate_across_a_selected_serve_barrier"
    }
    observed_worker_tests = set(_TIMEOUT_VOTE_EPISODE_WORKER_REGRESSION_SHA256)
    if observed_worker_tests != expected_worker_tests:
        errors.append(
            "timeout-vote Serve-barrier regression seal inventory must be exact; "
            f"missing={sorted(expected_worker_tests - observed_worker_tests)}, "
            f"extra={sorted(observed_worker_tests - expected_worker_tests)}"
        )
    expected_ingress_tests = {
        "timeout_vote_episode_crosses_only_the_bounded_certified_response_barrier"
    }
    observed_ingress_tests = set(_TIMEOUT_VOTE_EPISODE_INGRESS_REGRESSION_SHA256)
    if observed_ingress_tests != expected_ingress_tests:
        errors.append(
            "timeout-vote CertifiedResponse-barrier regression seal inventory "
            "must be exact; "
            f"missing={sorted(expected_ingress_tests - observed_ingress_tests)}, "
            f"extra={sorted(observed_ingress_tests - expected_ingress_tests)}"
        )

    formal_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if not formal_path.is_file() or formal_path.is_symlink():
        errors.append(
            f"{formal_path}: timeout-vote episode formal source must be a regular file"
        )
        return errors
    formal_source = formal_path.read_text(encoding="utf-8")
    expected_operator_modules = {
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncRecoveryVoteEpochProofs.tla",
        "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla",
    }
    observed_operator_modules = set(_TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256)
    if observed_operator_modules != expected_operator_modules:
        errors.append(
            "timeout-vote operator source-seal module inventory must be exact; "
            f"missing={sorted(expected_operator_modules - observed_operator_modules)}, "
            f"extra={sorted(observed_operator_modules - expected_operator_modules)}"
        )
    expected_theorem_modules = {
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncRecoveryVoteEpochProofs.tla",
        "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla",
        "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
    }
    observed_theorem_modules = set(_TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256)
    if observed_theorem_modules != expected_theorem_modules:
        errors.append(
            "timeout-vote theorem source-seal module inventory must be exact; "
            f"missing={sorted(expected_theorem_modules - observed_theorem_modules)}, "
            f"extra={sorted(observed_theorem_modules - expected_theorem_modules)}"
        )
    formal_seals = _TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256.get(
        "SumeragiV2AsyncNetwork.tla", {}
    )
    theorem_seals = _TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256.get(
        "SumeragiV2AsyncNetwork.tla", {}
    )
    expected_formal_symbols = {
        "AsyncLeaderWireCanonicalLifecyclePayload",
        "AsyncLeaderWireLifecycleSubject",
        "AsyncLeaderWireLifecycleIdentityAt",
        "AsyncLeaderWireLifecycleRecord",
        "AsyncLeaderWireLifecycleTyped",
        "AsyncTimeoutRecoveryVoteOwnerDispositions",
        "AsyncTimeoutRecoveryVoteAdmissionDispositions",
        "AsyncTimeoutRecoveryEpisodeKey",
        "AsyncTimeoutRecoveryEpisodeKeySet",
        "AsyncTimeoutRecoveryEpisodeParameterSet",
        "AsyncTimeoutRecoveryEpisodeFromParameters",
        "AsyncTimeoutRecoveryEpisodeSet",
        "AsyncTimeoutRecoveryVoteOwnerSlot",
        "AsyncTimeoutRecoveryVoteOwnerUniverse",
        "AsyncTimeoutRecoveryVoteOwnerValidForEpisode",
        "AsyncTimeoutRecoveryEpisodeValidIn",
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
        "AsyncTimeoutRecoveryVoteBasicBinding",
        "AsyncTimeoutRecoveryVoteIngressRecordsFor",
        "AsyncTimeoutRecoveryVotePreCutDescent",
        "AsyncTimeoutRecoveryVoteRestoredDescent",
        "AsyncTimeoutRecoveryVoteFreshReplenishment",
        "AsyncTimeoutRecoveryVoteDispositionMatches",
        "AsyncTimeoutRecoveryVoteCandidateOwners",
        "AsyncTimeoutRecoveryVoteCandidateDefined",
        "AsyncTimeoutRecoveryVoteCandidateOwner",
        "AsyncTimeoutRecoveryVoteIncumbents",
        "AsyncTimeoutRecoverySameVoteLifecycleOwner",
        "AsyncTimeoutRecoveryVoteAdmissionPlan",
        "AsyncTimeoutRecoveryVoteAdmissionRequired",
        "AsyncTimeoutRecoveryVoteAdmissionAllowed",
        "AsyncTimeoutRecoveryVoteBarrierException",
        "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier",
        "AsyncServeIngressIndexMayPrecedeAdmittedTarget",
        "AsyncLeaderWireIngressIndexMayPrecedeAdmittedTarget",
        "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
        "AsyncTimeoutRecoveryVoteRuntimeRecordsAfter",
        "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
        "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission",
        "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
        "AsyncTimeoutRecoveryAdmittedVoteSlots",
        "AsyncTimeoutRecoveryRemainingProducerSlots",
        "AsyncTimeoutRecoveryProducerEpisodeMeasure",
        "AsyncTimeoutRecoveryEpisodeCreationReadyIn",
        "AsyncTimeoutRecoveryTransitionGateIn",
        "AsyncTimeoutRecoveryEpisodeRetiresThisStep",
        "AsyncTimeoutRecoveryExistingCaptureClearsThisStep",
        "AsyncTimeoutRecoveryEpisodeAfterTransition",
        "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition",
        "AsyncTimeoutRecoveryNewEpisodeIn",
        "AsyncTimeoutRecoveryNewEpisodesAfterTransition",
    }
    observed_formal_symbols = set(formal_seals)
    if observed_formal_symbols != expected_formal_symbols:
        errors.append(
            "timeout-vote episode formal source-seal inventory must be exact; "
            f"missing={sorted(expected_formal_symbols - observed_formal_symbols)}, "
            f"extra={sorted(observed_formal_symbols - expected_formal_symbols)}"
        )
    expected_theorem_symbols = {
        "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite",
        "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot",
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission",
        "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot",
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode",
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot",
        "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState",
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent",
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode",
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState",
        "AsyncPostGstHasNoControlServiceReset",
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode",
        "AsyncUnchangedCoreStatePreservesTimeoutBoundary",
        "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary",
        "AsyncUnchangedCoreStateExcludesPersistInstall",
        "AsyncFairIngressDrainPreservesRetransmitTimerState",
        "AsyncFairIngressDrainExcludesDirectRetransmit",
        "AsyncTypedOutstandingTagRemovalChangesFunction",
        "AsyncDeferredRetransmitRemovesOutstandingTag",
        "AsyncFairIngressDrainExcludesDeferredRetransmit",
        "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle",
        "AsyncIngressDrainFramesDeferredAndCausalQueues",
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork",
        "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers",
        "AsyncSequenceSetAfterAppendAddsOnlyValue",
        "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue",
        "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin",
        "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase",
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase",
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin",
        "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase",
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind",
        "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase",
        "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle",
        "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame",
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode",
        "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState",
        "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition",
    }
    observed_theorem_symbols = set(theorem_seals)
    if observed_theorem_symbols != expected_theorem_symbols:
        errors.append(
            "timeout-vote episode theorem source-seal inventory must be exact; "
            f"missing={sorted(expected_theorem_symbols - observed_theorem_symbols)}, "
            f"extra={sorted(observed_theorem_symbols - expected_theorem_symbols)}"
        )
    formal_bodies: dict[str, tuple[str, int]] = {}
    for symbol, expected_sha256 in formal_seals.items():
        extracted = _top_level_operator_body(
            formal_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{formal_path}: missing source-sealed timeout-vote operator {symbol}"
            )
            continue
        formal_bodies[symbol] = extracted
        body, line = extracted
        observed_sha256 = hashlib.sha256(
            " ".join(body.split()).encode("utf-8")
        ).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{formal_path}:{line}: timeout-vote episode operator {symbol} "
                f"must match reviewed digest {expected_sha256}; found "
                f"{observed_sha256}"
            )
    theorem_bodies: dict[str, tuple[str, int]] = {}
    for symbol, expected_sha256 in theorem_seals.items():
        extracted = _top_level_theorem_body(
            formal_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{formal_path}: missing source-sealed timeout-vote theorem {symbol}"
            )
            continue
        theorem_bodies[symbol] = extracted
        body, line = extracted
        observed_sha256 = hashlib.sha256(
            " ".join(body.split()).encode("utf-8")
        ).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{formal_path}:{line}: timeout-vote episode theorem {symbol} "
                f"must match reviewed digest {expected_sha256}; found "
                f"{observed_sha256}"
            )

    boundary_filename = "SumeragiV2AsyncRecoveryVoteEpochProofs.tla"
    boundary_path = formal_dir / boundary_filename
    boundary_seals = _TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256.get(
        boundary_filename, {}
    )
    boundary_continuation_filename = (
        "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla"
    )
    boundary_continuation_path = formal_dir / boundary_continuation_filename
    boundary_continuation_seals = _TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256.get(
        boundary_continuation_filename, {}
    )
    expected_boundary_symbols = {
        "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileEpisodeDue",
        "AsyncNextPreservesEmptyServeIngressOwnersWhileEpisodeDue",
        "AsyncNextPreservesServeProducerEpisodeTypeInvariant",
        "AsyncNextPreservesServeProducerEpisodeInvariants",
        "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame",
        "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape",
        "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape",
        "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary",
    }
    expected_boundary_continuation_symbols = {
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
        "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary",
        "AsyncTimeoutRecoveryNewEpisodeDecomposition",
        "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary",
        "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary",
        "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary",
        "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary",
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary",
        "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary",
        "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet",
        "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet",
        "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary",
        "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant",
    }
    observed_boundary_symbols = set(boundary_seals)
    if observed_boundary_symbols != expected_boundary_symbols:
        errors.append(
            "timeout-recovery current-boundary theorem source-seal inventory "
            "must be exact; "
            f"missing={sorted(expected_boundary_symbols - observed_boundary_symbols)}, "
            f"extra={sorted(observed_boundary_symbols - expected_boundary_symbols)}"
        )
    observed_boundary_continuation_symbols = set(boundary_continuation_seals)
    if (
        observed_boundary_continuation_symbols
        != expected_boundary_continuation_symbols
    ):
        errors.append(
            "timeout-recovery current-boundary continuation theorem "
            "source-seal inventory must be exact; "
            "missing="
            f"{sorted(expected_boundary_continuation_symbols - observed_boundary_continuation_symbols)}, "
            "extra="
            f"{sorted(observed_boundary_continuation_symbols - expected_boundary_continuation_symbols)}"
        )
    boundary_source: str | None = None
    if not boundary_path.is_file() or boundary_path.is_symlink():
        errors.append(
            f"{boundary_path}: timeout-recovery current-boundary proofs must "
            "be a regular formal source"
        )
    else:
        try:
            boundary_source = boundary_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{boundary_path}: cannot read timeout-recovery "
                f"current-boundary proofs: {error}"
            )
    boundary_continuation_source: str | None = None
    if (
        not boundary_continuation_path.is_file()
        or boundary_continuation_path.is_symlink()
    ):
        errors.append(
            f"{boundary_continuation_path}: timeout-recovery current-boundary "
            "continuation proofs must be a regular formal source"
        )
    else:
        try:
            boundary_continuation_source = boundary_continuation_path.read_text(
                encoding="utf-8"
            )
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{boundary_continuation_path}: cannot read timeout-recovery "
                f"current-boundary continuation proofs: {error}"
            )
    boundary_operator_bodies: dict[str, tuple[str, int]] = {}
    boundary_theorem_bodies: dict[str, tuple[str, int]] = {}
    boundary_theorem_paths: dict[str, Path] = {}
    if boundary_source is not None:
        boundary_operator_seals = _TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256.get(
            boundary_filename, {}
        )
        expected_boundary_operator_symbols = {
            "AsyncTimeoutRecoveryEpisodeBoundaryIn",
            "AsyncTimeoutRecoveryBoundaryFrameShape",
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor",
        }
        observed_boundary_operator_symbols = set(boundary_operator_seals)
        if observed_boundary_operator_symbols != expected_boundary_operator_symbols:
            errors.append(
                "timeout-recovery boundary operator source-seal inventory "
                "must be exact; "
                f"missing={sorted(expected_boundary_operator_symbols - observed_boundary_operator_symbols)}, "
                f"extra={sorted(observed_boundary_operator_symbols - expected_boundary_operator_symbols)}"
            )
        for symbol, expected_sha256 in boundary_operator_seals.items():
            extracted = _top_level_operator_body(
                boundary_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{boundary_path}: missing source-sealed timeout-recovery "
                    f"boundary operator {symbol}"
                )
                continue
            boundary_operator_bodies[symbol] = extracted
            body, line = extracted
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{boundary_path}:{line}: timeout-recovery boundary "
                    f"operator {symbol} must match reviewed digest "
                    f"{expected_sha256}; found {observed_sha256}"
                )
        for symbol, expected_sha256 in boundary_seals.items():
            extracted = _top_level_theorem_body(
                boundary_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{boundary_path}: missing source-sealed timeout-recovery "
                    f"current-boundary theorem {symbol}"
                )
                continue
            boundary_theorem_bodies[symbol] = extracted
            body, line = extracted
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{boundary_path}:{line}: timeout-recovery current-boundary "
                    f"theorem {symbol} must match reviewed digest "
                    f"{expected_sha256}; found {observed_sha256}"
                )
            boundary_theorem_paths[symbol] = boundary_path
    if boundary_continuation_source is not None:
        boundary_continuation_operator_seals = (
            _TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256.get(
                boundary_continuation_filename, {}
            )
        )
        expected_boundary_continuation_operator_symbols = {
            "AsyncTimeoutRecoveryNewBaseEpisodeIn",
            "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn",
        }
        observed_boundary_continuation_operator_symbols = set(
            boundary_continuation_operator_seals
        )
        if (
            observed_boundary_continuation_operator_symbols
            != expected_boundary_continuation_operator_symbols
        ):
            errors.append(
                "timeout-recovery boundary continuation operator source-seal "
                "inventory must be exact; "
                "missing="
                f"{sorted(expected_boundary_continuation_operator_symbols - observed_boundary_continuation_operator_symbols)}, "
                "extra="
                f"{sorted(observed_boundary_continuation_operator_symbols - expected_boundary_continuation_operator_symbols)}"
            )
        for symbol, expected_sha256 in (
            boundary_continuation_operator_seals.items()
        ):
            extracted = _top_level_operator_body(
                boundary_continuation_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{boundary_continuation_path}: missing source-sealed "
                    f"timeout-recovery boundary continuation operator {symbol}"
                )
                continue
            boundary_operator_bodies[symbol] = extracted
            body, line = extracted
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{boundary_continuation_path}:{line}: timeout-recovery "
                    f"boundary continuation operator {symbol} must match "
                    f"reviewed digest {expected_sha256}; found {observed_sha256}"
                )
        for symbol, expected_sha256 in boundary_continuation_seals.items():
            extracted = _top_level_theorem_body(
                boundary_continuation_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{boundary_continuation_path}: missing source-sealed "
                    f"timeout-recovery current-boundary continuation theorem {symbol}"
                )
                continue
            boundary_theorem_bodies[symbol] = extracted
            boundary_theorem_paths[symbol] = boundary_continuation_path
            body, line = extracted
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{boundary_continuation_path}:{line}: timeout-recovery "
                    f"current-boundary continuation theorem {symbol} must match "
                    f"reviewed digest {expected_sha256}; found {observed_sha256}"
                )

    def boundary_theorem_path(symbol: str) -> Path:
        return boundary_theorem_paths.get(symbol, boundary_path)

    reviewed_timeout_transition_operator_sha256 = {
        "SumeragiV2AsyncNetwork.tla": {
            "AsyncTimeoutRecoveryEpisodeCreationReadyIn": (
                "463fcd6b8a67c383cd62b258d220831cd60d0854df625b77b359ff3859827283"
            ),
            "AsyncTimeoutRecoveryTransitionGateIn": (
                "02e689cac6c1aba321ea319591aff3feb1957c946608e9cc86f3826c854445ed"
            ),
            "AsyncTimeoutRecoveryEpisodeRetiresThisStep": (
                "ec514a038cee6fb9f31e63409040bee22defb732edc7208e3a683779a24ad686"
            ),
            "AsyncTimeoutRecoveryExistingCaptureClearsThisStep": (
                "92a31d4bd2a3c6b1a57c6bbbcf5d67d33da9b575922159aa12074e373226eb1d"
            ),
            "AsyncTimeoutRecoveryEpisodeAfterTransition": (
                "043fa928e89e959c987aa19209e2c3ef68541a551017062b9f38ae1a0d267fbc"
            ),
            "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition": (
                "15b01a6485f2446a013ff833d7ed3831dca42ba6933642afa74bb123622d7f76"
            ),
            "AsyncTimeoutRecoveryNewEpisodeIn": (
                "dfb0f545e1804325d054d841731c71e8da09565a9ae99177980e66b42f828b54"
            ),
            "AsyncTimeoutRecoveryNewEpisodesAfterTransition": (
                "0bb56383a4306a31b7df629c7908fd37f772a5c31305ee641b887c287a46b3f2"
            ),
        },
        boundary_filename: {
            "AsyncTimeoutRecoveryEpisodeBoundaryIn": (
                "a45ca5da201b0d81c538304cfbd4740029c9b8aaa17190f2c0f835a99ec5bc8a"
            ),
            "AsyncTimeoutRecoveryBoundaryFrameShape": (
                "7b851fbaf17ebee465208fbde0c47b447c6cf9e278cc54b369ce00da2146eecf"
            ),
            "AsyncTimeoutRecoveryMutationFrameShape": (
                "f85b7549b33097ba1cf31adbb63d48a85fdf0b61f409b60e128451acbf37656f"
            ),
            "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor": (
                "5bb5d4962bbc9db7fcbe00c53e70d5532ed39550ee5da04acdac6048dc0fc5c3"
            ),
        },
        boundary_continuation_filename: {
            "AsyncTimeoutRecoveryNewBaseEpisodeIn": (
                "aebba5c89eb28cc959098eef0a148c0a3defe3818d71f7aacc07ab8f2fd73136"
            ),
            "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn": (
                "e90869e129c22b6a56cdac3759bcf47be53b7c89276675019eeb1b7e39891d2b"
            ),
        },
    }
    reviewed_operator_sources = {
        "SumeragiV2AsyncNetwork.tla": (formal_path, formal_bodies),
        boundary_filename: (boundary_path, boundary_operator_bodies),
        boundary_continuation_filename: (
            boundary_continuation_path,
            boundary_operator_bodies,
        ),
    }
    for filename, reviewed in reviewed_timeout_transition_operator_sha256.items():
        source_path, bodies = reviewed_operator_sources[filename]
        for symbol, expected_sha256 in reviewed.items():
            extracted = bodies.get(symbol)
            if extracted is None:
                continue
            body, line = extracted
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{source_path}:{line}: timeout-recovery operator {symbol} "
                    "must retain its complete reviewed body after source-seal "
                    f"refresh; expected {expected_sha256}, found "
                    f"{observed_sha256}"
                )

    def require_boundary_theorem_statement(
        symbol: str,
        expected: str,
    ) -> None:
        extracted = boundary_theorem_bodies.get(symbol)
        if extracted is None:
            return
        body, line = extracted
        observed = _tla_statement_without_proof(body)
        expected_normalized = " ".join(expected.split())
        if observed != expected_normalized:
            errors.append(
                f"{boundary_theorem_path(symbol)}:{line}: {symbol} must remain the exact "
                "theorem-level ASSUME/PROVE current-boundary sequent with "
                "arbitrary state records and the reviewed validator bound; "
                f"expected {expected_normalized!r}; found {observed!r}"
            )

    require_boundary_theorem_statement(
        "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary",
        r"""
ASSUME NEW episode,
       AsyncTimeoutRecoveryMutationFrameShape(episode),
       AsyncTimeoutRecoveryEpisodeBoundaryIn(
         episode, context', nodeView', generation', decisions')
PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
           AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
             episode))
      /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
           AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
             episode),
           context', nodeView', generation', decisions')
""",
    )
    require_boundary_theorem_statement(
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
        r"""
ASSUME NEW preClockState, NEW episode,
       AsyncTimeoutRecoveryMutationFrameShape(episode),
       AsyncTimeoutRecoveryEpisodeBoundaryIn(
         episode, context', nodeView', generation', decisions')
PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
           AsyncTimeoutRecoveryEpisodeAfterTransition(
             preClockState, episode))
      /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
           AsyncTimeoutRecoveryEpisodeAfterTransition(
             preClockState, episode),
           context', nodeView', generation', decisions')
""",
    )
    require_boundary_theorem_statement(
        "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary",
        r"""
ASSUME NEW preClockState, NEW timeoutBaseState,
       NEW node \in ValidatorIds,
       AsyncTimeoutRecoveryEpisodeCreationReadyIn(
         preClockState, timeoutBaseState, node)
PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
           AsyncTimeoutRecoveryNewBaseEpisodeIn(
             preClockState, timeoutBaseState, node))
      /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
           AsyncTimeoutRecoveryNewBaseEpisodeIn(
             preClockState, timeoutBaseState, node),
           context', nodeView', generation', decisions')
""",
    )
    require_boundary_theorem_statement(
        "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary",
        r"""
ASSUME NEW preClockState, NEW timeoutBaseState,
       NEW node \in ValidatorIds,
       AsyncTimeoutRecoveryEpisodeCreationReadyIn(
         preClockState, timeoutBaseState, node)
PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
           AsyncTimeoutRecoveryNewEpisodeIn(
             preClockState, timeoutBaseState, node))
      /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
           AsyncTimeoutRecoveryNewEpisodeIn(
             preClockState, timeoutBaseState, node),
           context', nodeView', generation', decisions')
""",
    )
    require_boundary_theorem_statement(
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary",
        r"""
ASSUME NEW state, NEW episode,
       AsyncTimeoutRecoveryMutationFrameShape(episode),
       AsyncTimeoutRecoveryEpisodeBoundaryIn(
         episode, context', nodeView', generation', decisions')
PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
           AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
             state, episode))
      /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
           AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
             state, episode),
           context', nodeView', generation', decisions')
""",
    )

    def require_boundary_exact_statement(
        symbol: str,
        expected: str,
    ) -> None:
        extracted = boundary_theorem_bodies.get(symbol)
        if extracted is None:
            return
        body, line = extracted
        observed = _tla_statement_without_proof(body)
        expected_normalized = " ".join(expected.split())
        if observed != expected_normalized:
            errors.append(
                f"{boundary_theorem_path(symbol)}:{line}: {symbol} must remain the exact "
                "reviewed timeout-recovery boundary theorem statement; "
                f"expected {expected_normalized!r}; found {observed!r}"
            )

    boundary_exact_statements = {
        "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame": r"""
\A episode:
  AsyncTimeoutRecoveryMutationFrameShape(episode)
    => AsyncTimeoutRecoveryBoundaryFrameShape(episode)
""",
        "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape": r"""
\A parameters:
  AsyncTimeoutRecoveryMutationFrameShape(
    AsyncTimeoutRecoveryEpisodeFromParameters(parameters))
""",
        "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape": r"""
\A episode \in AsyncTimeoutRecoveryEpisodeSet:
  AsyncTimeoutRecoveryMutationFrameShape(episode)
""",
        "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary": r"""
\A preClockState, state:
  state.timeoutRecoveryEpisodes \subseteq AsyncTimeoutRecoveryEpisodeSet
    => \A episode \in
         AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
           preClockState, state):
         /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
         /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
              episode, context', nodeView', generation', decisions')
""",
        "AsyncTimeoutRecoveryNewEpisodeDecomposition": r"""
\A preClockState, timeoutBaseState, node:
  AsyncTimeoutRecoveryNewEpisodeIn(
    preClockState, timeoutBaseState, node)
    = IF AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
           preClockState, node)
      THEN AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
             AsyncTimeoutRecoveryNewBaseEpisodeIn(
               preClockState, timeoutBaseState, node))
      ELSE AsyncTimeoutRecoveryNewBaseEpisodeIn(
             preClockState, timeoutBaseState, node)
""",
        "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary": r"""
\A preClockState, timeoutBaseState:
  AsyncTimeoutRecoveryTransitionGateIn(preClockState, timeoutBaseState)
    => \A episode \in
         AsyncTimeoutRecoveryNewEpisodesAfterTransition(
           preClockState, timeoutBaseState):
         /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
         /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
              episode, context', nodeView', generation', decisions')
""",
        "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary": r"""
ASSUME NEW preClockState, NEW timeoutBaseState, NEW state,
       NEW episodes,
       state.timeoutRecoveryEpisodes
         \subseteq AsyncTimeoutRecoveryEpisodeSet,
       AsyncTimeoutRecoveryTransitionGateIn(
         preClockState, timeoutBaseState),
       episodes =
         AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
           preClockState, state)
           \cup
         AsyncTimeoutRecoveryNewEpisodesAfterTransition(
           preClockState, timeoutBaseState)
PROVE \A episode \in episodes:
        /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             episode, context', nodeView', generation', decisions')
""",
        "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary": r"""
ASSUME NEW state, NEW episodes,
       \A episode \in state.timeoutRecoveryEpisodes:
         /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
         /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
              episode, context', nodeView', generation', decisions'),
       episodes =
         {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(state, episode):
            episode \in state.timeoutRecoveryEpisodes}
PROVE \A episode \in episodes:
        /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             episode, context', nodeView', generation', decisions')
""",
        "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet": r"""
AsyncControlServiceStateTypeInvariant
  => asyncControlServiceState.timeoutRecoveryEpisodes
       \subseteq AsyncTimeoutRecoveryEpisodeSet
""",
        "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet": r"""
\A state, resetNodes:
  state.timeoutRecoveryEpisodes
    \subseteq AsyncTimeoutRecoveryEpisodeSet
    => (AsyncControlServiceStateAfterReset(state, resetNodes))
         .timeoutRecoveryEpisodes
         \subseteq AsyncTimeoutRecoveryEpisodeSet
""",
        "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary": r"""
ASSUME AsyncControlServiceStateTypeInvariant,
       AsyncControlServiceSlotTransition
PROVE AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
""",
        "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant": r"""
ASSUME AsyncControlServiceStateTypeInvariant,
       AsyncNext
PROVE AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
""",
    }
    for symbol, expected in boundary_exact_statements.items():
        require_boundary_exact_statement(symbol, expected)

    boundary_required_proof_dependencies = {
        "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame": (
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryBoundaryFrameShape",
        ),
        "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape": (
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryEpisodeFromParameters",
            "AsyncTimeoutRecoveryEpisode",
        ),
        "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape": (
            "AsyncTimeoutRecoveryEpisodeSet",
            "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape",
        ),
        "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary": (
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame",
            "AsyncTimeoutRecoveryEpisodeBoundaryIn",
            "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor",
            "FunctionalReplacePreservesDomain",
            "FunctionalUpdateAwayFromKey",
        ),
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary": (
            "AsyncTimeoutRecoveryEpisodeAfterTransition",
            "AsyncTimeoutRecoveryExistingCaptureClearsThisStep",
            "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary",
            "FunctionalReplacePreservesDomain",
            "FunctionalReplaceUpdateAtKey",
            "FunctionalUpdateAwayFromKey",
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary": (
            "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition",
            "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape",
            "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
            "AsyncTimeoutRecoveryEpisodeRetiresThisStep",
            "AsyncNodeHasDecisionIn",
        ),
        "AsyncTimeoutRecoveryNewEpisodeDecomposition": (
            "AsyncTimeoutRecoveryNewEpisodeIn",
            "AsyncTimeoutRecoveryNewBaseEpisodeIn",
            "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn",
            "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor",
        ),
        "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary": (
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryNewBaseEpisodeIn",
            "AsyncTimeoutRecoveryEpisode",
            "AsyncTimeoutRecoveryEpisodeKey",
            "AsyncTimeoutRecoveryEpisodeCreationReadyIn",
            "AsyncTimeoutRecoveryEpisodeBoundaryIn",
        ),
        "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary": (
            "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary",
            "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary",
            "AsyncTimeoutRecoveryNewEpisodeDecomposition",
        ),
        "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary": (
            "AsyncTimeoutRecoveryNewEpisodesAfterTransition",
            "AsyncTimeoutRecoveryTransitionGateIn",
            "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary",
        ),
        "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary": (
            "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary",
            "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary",
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary": (
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
            "AsyncTimeoutRecoveryMutationFrameShape",
            "AsyncTimeoutRecoveryEpisodeBoundaryIn",
            "FunctionalReplacePreservesDomain",
            "FunctionalUpdateAwayFromKey",
        ),
        "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary": (
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary",
        ),
        "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet": (
            "AsyncControlServiceStateTypeInvariant",
            "AsyncTimeoutRecoveryEpisodeTypeInvariantIn",
            "AsyncTimeoutRecoveryEpisodesIn",
        ),
        "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet": (
            "AsyncControlServiceStateAfterReset",
            "FS_Subset",
        ),
        "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary": (
            "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet",
            "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet",
            "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
            "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary",
            "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary",
            "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
            "AsyncTimeoutRecoveryEpisodesIn",
            "AsyncNodeHasDecisionIn",
        ),
        "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant": (
            "AsyncNext",
            "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary",
        ),
    }
    boundary_exact_proof_dependency_counts = {
        (
            "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
            "AsyncTimeoutRecoveryExistingCaptureClearsThisStep",
        ): 2,
    }
    for symbol, dependencies in boundary_required_proof_dependencies.items():
        extracted = boundary_theorem_bodies.get(symbol)
        if extracted is None:
            continue
        body, line = extracted
        parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
        proof = parts[1] if len(parts) == 2 else ""
        missing = []
        for dependency in dependencies:
            expected_count = boundary_exact_proof_dependency_counts.get(
                (symbol, dependency)
            )
            if expected_count is None:
                if not _tla_dependency_present(proof, dependency):
                    missing.append(dependency)
                continue
            observed_count = len(
                re.findall(rf"\b{re.escape(dependency)}\b", proof)
            )
            if observed_count != expected_count:
                missing.append(
                    f"{dependency} (expected {expected_count}, "
                    f"found {observed_count})"
                )
        if missing:
            errors.append(
                f"{boundary_theorem_path(symbol)}:{line}: timeout-recovery boundary theorem "
                f"{symbol} must retain the exact reviewed proof dependencies; "
                f"missing={missing!r}"
            )

    producer_bridge_exact_statements = {
        "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileEpisodeDue": r"""
\A node \in ValidatorIds:
  /\ AsyncTypeInvariant
  /\ asyncServeProducerEpisodeDue[node]
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ AsyncNetworkStep
  => AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
""",
        "AsyncNextPreservesEmptyServeIngressOwnersWhileEpisodeDue": r"""
\A node \in ValidatorIds:
  /\ AsyncTypeInvariant
  /\ asyncServeProducerEpisodeDue[node]
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ AsyncNext
  => AsyncServeIngressLifecycleOwnerIdentities(node)' = {}
""",
        "AsyncNextPreservesServeProducerEpisodeTypeInvariant": r"""
/\ AsyncServeProducerEpisodeTypeInvariant
/\ AsyncNext
=> AsyncServeProducerEpisodeTypeInvariant'
""",
        "AsyncNextPreservesServeProducerEpisodeInvariants": r"""
/\ AsyncStrongTypeInvariant
/\ AsyncNext
=> /\ AsyncServeProducerEpisodeTypeInvariant'
   /\ AsyncServeProducerEpisodeOwnershipInvariant'
""",
    }
    producer_bridge_required_proof_dependencies = {
        "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileEpisodeDue": (
            "AsyncServeProducerEpisodeBlocksFreshServeAdmission",
            "PopSelectedIngressDoesNotCreateServeIngressOwners",
            "HiddenIngressAdmissionPreservesOtherNodeOwners",
            "ServeIngressAdmissionStutterPreservesOwnerIdentities",
            "AcceptOrReserveExactServeIngressVia",
            "ReserveExactServeCapacityVia",
            "AdvanceExactServeCapacityVia",
            "AsyncServeLifecycleAdmissionRequired",
            "ExactServeTransportAdmissionCanAdvanceVia",
        ),
        "AsyncNextPreservesEmptyServeIngressOwnersWhileEpisodeDue": (
            "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileEpisodeDue",
            "RunnerStepPreservesEmptyServeIngressOwners",
            "FaultStepPreservesEmptyServeIngressOwners",
            "PopSelectedIngressDoesNotCreateServeIngressOwners",
            "ServeReceiverCloseRollbackDoesNotCreateIngressOwners",
            "HiddenIngressAdmissionPreservesOtherNodeOwners",
            "ServeIngressAdmissionStutterPreservesOwnerIdentities",
            "AsyncNext",
        ),
        "AsyncNextPreservesServeProducerEpisodeTypeInvariant": (
            "AsyncNext",
            "AsyncServeProducerEpisodeTransition",
            "FunctionValueHasCodomain",
        ),
        "AsyncNextPreservesServeProducerEpisodeInvariants": (
            "AsyncStrongTypeProjectsAsyncType",
            "AsyncNextPreservesServeProducerEpisodeTypeInvariant",
            "AsyncNextPreservesSchedulerType",
            "AsyncNextPreservesEmptyServeIngressOwnersWhileEpisodeDue",
            "AsyncServeProducerEpisodeFinalRetirementStep",
            "AsyncServeProducerEpisodeTransition",
            "AsyncServeIngressAdmissionOwned",
        ),
    }
    for symbol, expected in producer_bridge_exact_statements.items():
        extracted = boundary_theorem_bodies.get(symbol)
        if extracted is None:
            continue
        body, line = extracted
        statement = _tla_statement_without_proof(body)
        expected_statement = " ".join(expected.split())
        if statement != expected_statement:
            errors.append(
                f"{boundary_path}:{line}: producer-episode bridge theorem "
                f"{symbol} must retain the exact reviewed statement; "
                f"found {statement!r}"
            )
    for symbol, dependencies in (
        producer_bridge_required_proof_dependencies.items()
    ):
        extracted = boundary_theorem_bodies.get(symbol)
        if extracted is None:
            continue
        body, line = extracted
        parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
        proof = parts[1] if len(parts) == 2 else ""
        missing = [
            dependency
            for dependency in dependencies
            if not _tla_dependency_present(proof, dependency)
        ]
        if missing:
            errors.append(
                f"{boundary_path}:{line}: producer-episode bridge theorem "
                f"{symbol} must retain the exact reviewed proof dependencies; "
                f"missing={missing!r}"
            )

    def require_formal_exact(
        symbol: str,
        expected: str,
        description: str,
    ) -> None:
        extracted = formal_bodies.get(symbol)
        if extracted is None:
            return
        body, line = extracted
        observed = " ".join(body.split())
        expected_normalized = " ".join(expected.split())
        if observed != expected_normalized:
            errors.append(
                f"{formal_path}:{line}: {description} must equal only "
                f"{expected_normalized!r}; found {observed!r}"
            )

    require_formal_exact(
        "AsyncLeaderWireCanonicalLifecyclePayload",
        r"""
IF item.kind = "CertifiedResponse"
THEN AsyncCertifiedResponseCanonicalWireIdentity(item)
ELSE AsyncLeaderWireServiceIdentity(item)
""",
        "full concrete leader-wire lifecycle payload identity",
    )
    require_formal_exact(
        "AsyncLeaderWireLifecycleSubject",
        r"""
IF item.kind = "TimeoutVote" THEN NoSubject ELSE DeliverySubject(item)
""",
        "TimeoutVote-only semantic lifecycle-subject normalization",
    )
    require_formal_exact(
        "AsyncLeaderWireLifecycleIdentityAt",
        r"""
[context |-> leaderContext,
 height |-> DeliveryHeight(item),
 view |-> DeliveryView(item),
 subject |-> AsyncLeaderWireLifecycleSubject(item),
 phase |-> item.kind,
 slot |-> AsyncLeaderWireLifecycleSlot(item),
 payload |-> AsyncLeaderWireCanonicalLifecyclePayload(item)]
""",
        "normalized semantic leader-wire identity with full concrete payload",
    )
    require_formal_exact(
        "AsyncLeaderWireLifecycleRecord",
        r"""
[recipient |-> item.envelope.recipient,
 item |-> item,
 identity |-> AsyncLeaderWireLifecycleIdentityAt(item, leaderContext),
 slot |-> AsyncLeaderWireLifecycleSlot(item),
 context |-> leaderContext,
 height |-> DeliveryHeight(item),
 view |-> DeliveryView(item),
 subject |-> AsyncLeaderWireLifecycleSubject(item),
 phase |-> item.kind,
 causalOrigin |->
   AsyncLeaderWireLifecycleCausalOriginAt(item, leaderContext),
 admissionOrdinal |-> admissionOrdinal,
 physicalAdmissionOrdinal |-> physicalAdmissionOrdinal,
 schedulerOrdinal |-> schedulerOrdinal,
 departurePhysicalCut |-> departurePhysicalCut,
 status |-> status,
 ingressPredecessors |-> ingressPredecessors]
""",
        "normalized leader-wire lifecycle record",
    )
    for symbol, expected_count, description in (
        (
            "AsyncLeaderWireLifecycleSubject",
            6,
            "one definition plus LifecycleIdentityAt, LifecycleRecord, "
            "LifecycleTyped, timeout-owner, and timeout-binding consumers",
        ),
        (
            "AsyncLeaderWireCanonicalLifecyclePayload",
            2,
            "one definition plus the concrete lifecycle identity consumer",
        ),
    ):
        observed_count = tla_code_tokens(formal_source).count(symbol)
        if observed_count != expected_count:
            errors.append(
                f"{formal_path}: {description} must reference {symbol} exactly "
                f"{expected_count} time(s); found {observed_count}"
            )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteOwnerDispositions",
        '{"PreCutDescent", "RestoredDescent", "FreshReplenishment"}',
        "closed timeout-vote owner disposition inventory",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionDispositions",
        '{"NonCandidate", "FirstAdmission", "CoalescedRetry"}',
        "closed timeout-vote admission disposition inventory",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeKey",
        r"""
[target |-> origin.target,
 context |-> origin.context,
 leader |-> origin.leader,
 view |-> origin.view,
 subject |-> NoSubject,
 phase |-> "TimeoutVote"]
""",
        "frozen target/context/leader/view/NoSubject/phase timeout episode key",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeKeySet",
        r"""
[target: ValidatorIds,
 context: ContextRecords,
 leader: ValidatorIds,
 view: Views,
 subject: {NoSubject},
 phase: {"TimeoutVote"}]
""",
        "TimeoutVote episode key set with only the normalized NoSubject",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeParameterSet",
        r"""
[node: ValidatorIds,
 timeoutOwnerOrigin: AsyncCandidateCausalOriginSet,
 generation: Generations,
 timeoutOwnerOrdinal: Nat \ {0},
 physicalCut: Nat \ {0},
 preFrozenRetransmitOrdinal: Nat,
 preFrozenRetransmitPhysicalCut: Nat,
 admittedTimeoutVoteOwners:
   SUBSET AsyncTimeoutRecoveryVoteOwnerSet]
""",
        "exact eight-field timeout-recovery episode parameter universe",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeFromParameters",
        r"""
AsyncTimeoutRecoveryEpisode(
  parameters.node,
  parameters.timeoutOwnerOrigin,
  parameters.generation,
  parameters.timeoutOwnerOrdinal,
  parameters.physicalCut,
  parameters.preFrozenRetransmitOrdinal,
  parameters.preFrozenRetransmitPhysicalCut,
  parameters.admittedTimeoutVoteOwners)
""",
        "exact ordered eight-field timeout-recovery episode constructor projection",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeSet",
        r"""
{AsyncTimeoutRecoveryEpisodeFromParameters(parameters):
   parameters \in AsyncTimeoutRecoveryEpisodeParameterSet}
""",
        "exact one-binder timeout-recovery episode image",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteOwnerSlot",
        "[episode |-> key, source |-> source]",
        "frozen episode-and-source timeout-vote slot",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteOwnerUniverse",
        r"""
{AsyncTimeoutRecoveryVoteOwnerSlot(
   AsyncTimeoutRecoveryEpisodeKey(origin), source):
   source \in VotingRoster(origin.context.epoch)}
""",
        "roster-derived frozen timeout-vote owner universe",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteBasicBinding",
        r"""
/\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
     asyncControlServiceState, node)
/\ episode.key.context = context
/\ episode.key.view = nodeView[node]
/\ episode.generation = generation[node]
/\ asyncTimeoutEmitted[node]
/\ ~AsyncNodeHasDecisionIn(node, context, decisions)
/\ item.kind = "TimeoutVote"
/\ item \in asyncSentItems
/\ item.envelope \in TimeoutEnvelopeSet
/\ item.envelope.recipient = node
/\ item.source = item.envelope.vote.signer
/\ item.source \in VotingRoster(episode.key.context.epoch)
/\ DeliveryClass(item) = "Progress"
/\ AsyncControlItemContext(item) = episode.key.context
/\ DeliveryHeight(item) = episode.key.context.height
/\ AsyncControlItemView(item) = episode.key.view
/\ AsyncLeaderWireLifecycleSubject(item) = episode.key.subject
/\ ExactPrepareQcMatchesRef(
     item.envelope.vote.highestPrepareQc,
     item.envelope.vote.highRank,
     item.envelope.vote.highSubject)
/\ item.envelope.vote.highRank <= item.envelope.vote.view
""",
        "direct authenticated current-episode TimeoutVote binding",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteIngressRecordsFor",
        r"""
{record \in asyncLeaderWireLifecycles:
   /\ record.recipient = node
   /\ record.status = "Ingress"
   /\ record.phase = "TimeoutVote"
   /\ record.context = episode.key.context
   /\ record.height = episode.key.context.height
   /\ record.view = episode.key.view
   /\ record.subject = episode.key.subject
   /\ AsyncLeaderWireAdmissionMatchesRecord(item, record)}
""",
        "exact TimeoutVote Ingress-record candidate set",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVotePreCutDescent",
        r"""
/\ record.schedulerOrdinal < episode.timeoutOwnerOrdinal
/\ record.admissionOrdinal = record.physicalAdmissionOrdinal
""",
        "formal A=P,S<timeout descent classification",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteRestoredDescent",
        r"""
/\ record.schedulerOrdinal < episode.timeoutOwnerOrdinal
/\ record.admissionOrdinal < record.physicalAdmissionOrdinal
/\ record.admissionOrdinal < episode.physicalCut
/\ record.physicalAdmissionOrdinal >= episode.physicalCut
""",
        "formal restored descent classification",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteFreshReplenishment",
        r"""
/\ record.schedulerOrdinal > episode.timeoutOwnerOrdinal
/\ record.admissionOrdinal = record.physicalAdmissionOrdinal
/\ record.physicalAdmissionOrdinal >= episode.physicalCut
""",
        "formal strict fresh replenishment classification",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteDispositionMatches",
        r"""
CASE disposition = "PreCutDescent" ->
       AsyncTimeoutRecoveryVotePreCutDescent(record, episode)
  [] disposition = "RestoredDescent" ->
       AsyncTimeoutRecoveryVoteRestoredDescent(record, episode)
  [] disposition = "FreshReplenishment" ->
       AsyncTimeoutRecoveryVoteFreshReplenishment(record, episode)
""",
        "closed descent/restored/replenishment dispatcher",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteCandidateOwners",
        r"""
UNION {
  UNION {
    {AsyncTimeoutRecoveryVoteOwner(
       AsyncTimeoutRecoveryVoteOwnerSlot(episode.key, item.source),
       AsyncLeaderWireServiceIdentity(item),
       record.admissionOrdinal, record.physicalAdmissionOrdinal,
       record.schedulerOrdinal, disposition):
       disposition \in
         {candidateDisposition \in
            AsyncTimeoutRecoveryVoteOwnerDispositions:
            AsyncTimeoutRecoveryVoteDispositionMatches(
              record, episode, candidateDisposition)}}:
    record \in AsyncTimeoutRecoveryVoteIngressRecordsFor(
      node, item, episode)}:
  episode \in
    {candidateEpisode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
       asyncControlServiceState, node):
       AsyncTimeoutRecoveryVoteBasicBinding(
         node, item, candidateEpisode)}}
""",
        "exact episode/record/disposition TimeoutVote candidate projection",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteCandidateDefined",
        r"""
LET candidates == AsyncTimeoutRecoveryVoteCandidateOwners(node, item)
IN /\ IsFiniteSet(candidates)
   /\ Cardinality(candidates) = 1
""",
        "finite singleton timeout-vote candidate predicate",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteCandidateOwner",
        r"""
CHOOSE owner \in
  AsyncTimeoutRecoveryVoteCandidateOwners(node, item): TRUE
""",
        "unique timeout-vote candidate selector",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteIncumbents",
        r"""
{owner \in
   UNION {episode.admittedTimeoutVoteOwners:
     episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
       asyncControlServiceState, node)}:
   owner.slot = candidate.slot}
""",
        "same-slot timeout-vote incumbent set",
    )
    require_formal_exact(
        "AsyncTimeoutRecoverySameVoteLifecycleOwner",
        r"""
/\ left.slot = right.slot
/\ left.identity = right.identity
/\ left.admissionOrdinal = right.admissionOrdinal
/\ left.schedulerOrdinal = right.schedulerOrdinal
""",
        "current formal timeout-vote lifecycle-owner equality",
    )
    runtime_same_owner_exact = same_owner is not None and rust_code_tokens(
        same_owner.source
    ) == rust_code_tokens(
        """
fn same_lifecycle_owner_as(&self, other: &Self) -> bool {
    self.token == other.token
}
"""
    )
    formal_same_owner = formal_bodies.get(
        "AsyncTimeoutRecoverySameVoteLifecycleOwner"
    )
    formal_same_owner_exact = formal_same_owner is not None and " ".join(
        formal_same_owner[0].split()
    ) == " ".join(
        r"""
/\ left.slot = right.slot
/\ left.identity = right.identity
/\ left.admissionOrdinal = right.admissionOrdinal
/\ left.schedulerOrdinal = right.schedulerOrdinal
""".split()
    )
    if not runtime_same_owner_exact or not formal_same_owner_exact:
        mismatch_line = (
            formal_same_owner[1]
            if formal_same_owner is not None
            else (same_owner.line if same_owner is not None else 1)
        )
        errors.append(
            f"{formal_path}:{mismatch_line}: Rust/TLA timeout-vote owner equality "
            "must align on the immutable retained token; carrier ordinal and "
            "derived disposition may differ only while the incumbent remains "
            "authoritative. Add an explicit proved reachability/refinement "
            "contract before accepting any non-identical projection"
        )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionPlan",
        r"""
IF ~AsyncTimeoutRecoveryVoteCandidateDefined(node, item)
THEN {"NonCandidate"}
ELSE LET candidate ==
           AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
         incumbents ==
           AsyncTimeoutRecoveryVoteIncumbents(node, candidate)
     IN IF incumbents = {}
        THEN {"FirstAdmission"}
        ELSE IF \A incumbent \in incumbents:
                  AsyncTimeoutRecoverySameVoteLifecycleOwner(
                    incumbent, candidate)
             THEN {"CoalescedRetry"}
             ELSE {}
""",
        "exact NonCandidate/FirstAdmission/CoalescedRetry/conflict plan",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionRequired",
        r"""
\E episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
     asyncControlServiceState, node):
  AsyncTimeoutRecoveryVoteBasicBinding(node, item, episode)
""",
        "exact timeout-vote admission-required predicate",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionAllowed",
        r"""
\/ ~AsyncTimeoutRecoveryVoteAdmissionRequired(node, item)
\/ /\ AsyncTimeoutRecoveryVoteCandidateDefined(node, item)
   /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)
        \subseteq AsyncTimeoutRecoveryVoteAdmissionDispositions
   /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)
        \cap {"FirstAdmission", "CoalescedRetry"} # {}
""",
        "fail-closed exact timeout-vote admission gate",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteBarrierException",
        r"""
LET item == asyncIngressLanes[node][source][index]
IN /\ source = item.source
   /\ source \in ValidatorIds
   /\ AsyncTimeoutRecoveryVoteAdmissionRequired(node, item)
   /\ AsyncTimeoutRecoveryVoteCandidateDefined(node, item)
   /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)
        \cap {"FirstAdmission", "CoalescedRetry"} # {}
   /\ IngressItemCanDrain(node, item)
""",
        "direct-validator finite TimeoutVote barrier exception",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier",
        r"""
/\ owner.phase = "CertifiedResponse"
/\ owner.status = "Ingress"
/\ AsyncTimeoutRecoveryVoteBarrierException(node, source, index)
""",
        "leader-wire TimeoutVote exception for one exact Ingress CertifiedResponse phase",
    )
    require_formal_exact(
        "AsyncServeIngressIndexMayPrecedeAdmittedTarget",
        r"""
IF ~AsyncServeIngressOwnsSharedPhysicalTurn(node)
THEN TRUE
ELSE LET ownerIdentity ==
           AsyncServeEarliestIngressSchedulerOwnerIdentity(node)
         item == asyncIngressLanes[node][source][index]
     IN \/ index <=
              AsyncServeIngressAdmissionPredecessorCounts(
                node, ownerIdentity)[source]
        \/ /\ item.kind \in AsyncReplyRequestKinds
           /\ AsyncServeLogicalRequestIdentity(node, item)
                = ownerIdentity
        \/ AsyncCertifiedFenceEscapeItem(item)
        \/ AsyncTimeoutRecoveryVoteBarrierException(
             node, source, index)
""",
        "exact selected-Serve timeout-vote exception placement",
    )
    require_formal_exact(
        "AsyncLeaderWireIngressIndexMayPrecedeAdmittedTarget",
        r"""
IF ~AsyncLeaderWireIngressOwnsSharedPhysicalTurn(node)
THEN TRUE
ELSE LET owner ==
           AsyncLeaderWireEarliestPhysicalIngressRecord(node)
         item == asyncIngressLanes[node][source][index]
     IN \/ index <= owner.ingressPredecessors[source]
        \/ /\ AsyncLeaderWireAdmissionMatchesRecord(item, owner)
           /\ AsyncLeaderWireIngressPrefixCleared(owner)
        \/ AsyncCertifiedFenceEscapeAdvancesLeaderWire(item, owner)
        \/ AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier(
             node, source, index, owner)
""",
        "exact CertifiedResponse-only leader-wire timeout-vote exception placement",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteRuntimeRecordsAfter",
        r"""
{record \in asyncLeaderWireLifecycles':
   /\ record.status = "Runtime"
   /\ record.phase = "TimeoutVote"
   /\ AsyncLeaderWireAdmissionMatchesRecord(item, record)}
""",
        "exact post-admission TimeoutVote Runtime-record set",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
        r"""
LET item == AsyncSelectedFairIngressItem(node)
IN /\ IngressDrainStep(node)
   /\ DrainFairIngressSelected(node)
   /\ AsyncTimeoutRecoveryVoteCandidateDefined(node, item)
   /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)
        \cap {"FirstAdmission", "CoalescedRetry"} # {}
   /\ Cardinality(
        AsyncTimeoutRecoveryVoteRuntimeRecordsAfter(item)) = 1
   /\ LET candidate ==
            AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
          runtimeRecord ==
            CHOOSE record \in
              AsyncTimeoutRecoveryVoteRuntimeRecordsAfter(item): TRUE
      IN /\ runtimeRecord.identity = candidate.identity
         /\ runtimeRecord.admissionOrdinal =
              candidate.admissionOrdinal
         /\ runtimeRecord.physicalAdmissionOrdinal =
              candidate.carrierPhysicalOrdinal
         /\ runtimeRecord.schedulerOrdinal =
              candidate.schedulerOrdinal
""",
        "exact TimeoutVote Ingress-to-Runtime admission occurrence",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
        r"""
{node \in ValidatorIds:
   AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)}
""",
        "exact timeout-vote admission-node set",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
        r"""
LET matchingNodes ==
      {node \in AsyncTimeoutRecoveryVoteAdmissionNodesThisStep:
         LET item == AsyncSelectedFairIngressItem(node)
             candidate ==
               AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
         IN candidate.slot.episode = episode.key}
IN IF matchingNodes = {}
   THEN episode
   ELSE LET node == CHOOSE candidateNode \in matchingNodes: TRUE
            item == AsyncSelectedFairIngressItem(node)
            candidate ==
              AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
        IN IF "FirstAdmission"
                \in AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)
           THEN [episode EXCEPT
                   !.admittedTimeoutVoteOwners = @ \cup {candidate}]
           ELSE episode
""",
        "exact first-insert/coalesced-stutter episode transformer",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission",
        r"""
[state EXCEPT
   !.timeoutRecoveryEpisodes =
     {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(state, episode):
        episode \in state.timeoutRecoveryEpisodes}]
""",
        "exact timeout-vote episode-state transformer",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryAdmittedVoteSlots",
        r"""
{owner.slot: owner \in episode.admittedTimeoutVoteOwners}
""",
        "exact admitted timeout-vote slot projection",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryRemainingProducerSlots",
        r"""
episode.timeoutVoteOwnerUniverse
  \ AsyncTimeoutRecoveryAdmittedVoteSlots(episode)
""",
        "finite timeout-vote producer remainder",
    )
    require_formal_exact(
        "AsyncTimeoutRecoveryProducerEpisodeMeasure",
        """
Cardinality(AsyncTimeoutRecoveryRemainingProducerSlots(episode))
""",
        "finite timeout-vote producer-episode measure",
    )

    def require_formal_dependencies(
        symbol: str,
        required: tuple[str, ...],
        description: str,
    ) -> None:
        extracted = formal_bodies.get(symbol)
        if extracted is None:
            return
        body, line = extracted
        missing = [
            dependency
            for dependency in required
            if not _tla_dependency_present(body, dependency)
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: {description} must retain dependencies "
                f"{missing!r}"
            )

    require_formal_dependencies(
        "AsyncLeaderWireLifecycleTyped",
        (
            "AsyncLeaderWireLifecycleIdentityAt",
            "AsyncLeaderWireLifecycleSubject",
            "AsyncLeaderWireLifecycleCausalOriginAt",
        ),
        "typed leader-wire record with TimeoutVote-only subject normalization",
    )
    lifecycle_typed = formal_bodies.get("AsyncLeaderWireLifecycleTyped")
    if lifecycle_typed is not None:
        body, line = lifecycle_typed
        normalized = " ".join(body.split())
        for exact_binding in (
            "record.identity = AsyncLeaderWireLifecycleIdentityAt(record.item, record.context)",
            "record.subject = AsyncLeaderWireLifecycleSubject(record.item)",
        ):
            if exact_binding not in normalized:
                errors.append(
                    f"{formal_path}:{line}: typed leader-wire lifecycle must "
                    f"retain normalized binding {exact_binding!r}"
                )

    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteOwnerUniverse",
        (
            "VotingRoster",
            "AsyncTimeoutRecoveryVoteOwnerSlot",
        ),
        "frozen timeout-vote owner universe",
    )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteOwnerValidForEpisode",
        (
            "VotingRoster",
            "AsyncControlItemContext",
            "AsyncControlItemView",
            "AsyncLeaderWireLifecycleSubject",
        ),
        "timeout-vote owner episode validity",
    )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryEpisodeValidIn",
        (
            "AsyncTimeoutRecoveryEpisodeKey",
            "AsyncTimeoutRecoveryVoteOwnerUniverse",
            "AsyncTimeoutRecoveryVoteOwnerSet",
            "AsyncTimeoutRecoveryVoteOwnerValidForEpisode",
            "IsFiniteSet",
            "Cardinality",
        ),
        "finite frozen timeout-recovery episode validity",
    )
    episode_valid = formal_bodies.get("AsyncTimeoutRecoveryEpisodeValidIn")
    if episode_valid is not None:
        body, line = episode_valid
        normalized = " ".join(body.split())
        required_finite_fragments = (
            "IsFiniteSet(episode.timeoutVoteOwnerUniverse)",
            "IsFiniteSet(episode.admittedTimeoutVoteOwners)",
            "episode.admittedTimeoutVoteOwners \\subseteq AsyncTimeoutRecoveryVoteOwnerSet",
            "Cardinality(episode.admittedTimeoutVoteOwners) <= Cardinality(episode.timeoutVoteOwnerUniverse)",
            "AsyncTimeoutRecoveryVoteOwnerValidForEpisode(owner, episode)",
            "left.slot = right.slot => left = right",
            "episode.key.subject = NoSubject",
        )
        missing = [
            fragment
            for fragment in required_finite_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: finite timeout-recovery episode must "
                f"retain exact owner-set bounds and slot uniqueness; missing {missing!r}"
            )
        forbidden_subject_bindings = (
            "episode.timeoutOwnerOrigin.subject = episode.key.subject",
            "episode.key.subject = episode.timeoutOwnerOrigin.subject",
        )
        forbidden = [
            binding for binding in forbidden_subject_bindings if binding in normalized
        ]
        if forbidden:
            errors.append(
                f"{formal_path}:{line}: timeout episode semantic key must use "
                f"NoSubject independently of the BeginTimeout high subject; found {forbidden!r}"
            )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
        (
            "AsyncTimeoutRecoveryEpisodes",
            "NodeHasDecision",
        ),
        "current timeout-recovery episode boundary",
    )
    current_boundary = formal_bodies.get(
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant"
    )
    if current_boundary is not None:
        body, line = current_boundary
        normalized = " ".join(body.split())
        required_boundary_fragments = (
            "episode.key.context = context",
            "episode.timeoutOwnerOrigin.height = context.height",
            "episode.key.view = nodeView[episode.node]",
            "episode.generation = generation[episode.node]",
            "~NodeHasDecision(episode.node)",
        )
        missing = [
            fragment
            for fragment in required_boundary_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: current timeout-recovery episode "
                "boundary must retain context, height, view, generation, and "
                f"no-decision retirement guards; missing {missing!r}"
            )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteBasicBinding",
        (
            "AsyncTimeoutRecoveryEpisodesForNodeIn",
            "VotingRoster",
            "DeliveryClass",
            "AsyncControlItemContext",
            "AsyncControlItemView",
            "AsyncLeaderWireLifecycleSubject",
            "ExactPrepareQcMatchesRef",
        ),
        "direct authenticated current-view TimeoutVote binding",
    )
    basic = formal_bodies.get("AsyncTimeoutRecoveryVoteBasicBinding")
    if basic is not None:
        body, line = basic
        normalized = " ".join(body.split())
        for exact_binding in (
            'item.kind = "TimeoutVote"',
            "item.envelope.recipient = node",
            "item.source = item.envelope.vote.signer",
            "item.source \\in VotingRoster(episode.key.context.epoch)",
            'DeliveryClass(item) = "Progress"',
            "AsyncLeaderWireLifecycleSubject(item) = episode.key.subject",
            "ExactPrepareQcMatchesRef( item.envelope.vote.highestPrepareQc, item.envelope.vote.highRank, item.envelope.vote.highSubject)",
            "item.envelope.vote.highRank <= item.envelope.vote.view",
        ):
            if exact_binding not in normalized:
                errors.append(
                    f"{formal_path}:{line}: direct TimeoutVote binding must retain "
                    f"{exact_binding!r}"
                )

    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteAdmissionPlan",
        (
            "AsyncTimeoutRecoveryVoteCandidateDefined",
            "AsyncTimeoutRecoveryVoteCandidateOwner",
            "AsyncTimeoutRecoveryVoteIncumbents",
            "AsyncTimeoutRecoverySameVoteLifecycleOwner",
        ),
        "0→0/0→1/1→1 timeout-vote admission plan",
    )
    plan_body = formal_bodies.get("AsyncTimeoutRecoveryVoteAdmissionPlan")
    if plan_body is not None:
        body, line = plan_body
        normalized = " ".join(body.split())
        for disposition in (
            '{"NonCandidate"}',
            '{"FirstAdmission"}',
            '{"CoalescedRetry"}',
        ):
            if disposition not in normalized:
                errors.append(
                    f"{formal_path}:{line}: timeout-vote admission plan must "
                    f"retain exact disposition {disposition}"
                )
        if "ELSE {}" not in normalized:
            errors.append(
                f"{formal_path}:{line}: a different incumbent owner must produce "
                "the empty fail-closed admission plan"
            )

    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteBarrierException",
        (
            "AsyncTimeoutRecoveryVoteAdmissionRequired",
            "AsyncTimeoutRecoveryVoteCandidateDefined",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
            "IngressItemCanDrain",
        ),
        "authoritative timeout-vote barrier predicate",
    )
    cross_body = formal_bodies.get(
        "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier"
    )
    if cross_body is not None:
        body, line = cross_body
        forbidden_claims = [
            token
            for token in (
                "CertifiedResponseClaimMatches",
                "CertifiedResponseClaimAuthorized",
                "AsyncCertifiedResponseClaim",
            )
            if token in body
        ]
        if forbidden_claims:
            errors.append(
                f"{formal_path}:{line}: pre-dequeue CertifiedResponse ownership "
                "cannot require a response claim acquired only after fair-ingress "
                f"dequeue; found {forbidden_claims!r}"
            )

    serve_barrier = formal_bodies.get(
        "AsyncServeIngressIndexMayPrecedeAdmittedTarget"
    )
    if serve_barrier is not None and not _tla_dependency_present(
        serve_barrier[0], "AsyncTimeoutRecoveryVoteBarrierException"
    ):
        errors.append(
            f"{formal_path}:{serve_barrier[1]}: Serve ingress must expose only "
            "the authoritative finite TimeoutVote episode exception"
        )
    leader_barrier = formal_bodies.get(
        "AsyncLeaderWireIngressIndexMayPrecedeAdmittedTarget"
    )
    if leader_barrier is not None:
        body, line = leader_barrier
        if not _tla_dependency_present(
            body, "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier"
        ):
            errors.append(
                f"{formal_path}:{line}: leader-wire ingress must use the exact "
                "CertifiedResponse-phase TimeoutVote exception"
            )
        if _tla_dependency_present(
            body, "AsyncTimeoutRecoveryVoteBarrierException"
        ):
            errors.append(
                f"{formal_path}:{line}: leader-wire ingress may not use the "
                "general TimeoutVote barrier exception directly"
            )

    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
        (
            "IngressDrainStep",
            "DrainFairIngressSelected",
            "AsyncTimeoutRecoveryVoteCandidateDefined",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
            "AsyncTimeoutRecoveryVoteRuntimeRecordsAfter",
        ),
        "exact TimeoutVote Ingress-to-Runtime transition",
    )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
        (
            "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
            "AsyncTimeoutRecoveryVoteCandidateOwner",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
        ),
        "first-admission insert and coalesced-retry stutter transformer",
    )
    transformer = formal_bodies.get(
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission"
    )
    if transformer is not None:
        body, line = transformer
        normalized = " ".join(body.split())
        if (
            '"FirstAdmission" \\in AsyncTimeoutRecoveryVoteAdmissionPlan(node, item)'
            not in normalized
            or "!.admittedTimeoutVoteOwners = @ \\cup {candidate}"
            not in normalized
            or "ELSE episode" not in normalized
        ):
            errors.append(
                f"{formal_path}:{line}: only FirstAdmission may increase the "
                "frozen source-owner set; CoalescedRetry must stutter"
            )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission",
        ("AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",),
        "timeout-vote owner state transformer",
    )
    require_formal_dependencies(
        "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
        (
            "AsyncOrdinaryIngressCarrierStateAfterTransition",
            "AsyncCandidateLifecycleStateAfterCarrierUpdate",
            "AsyncCandidateLifecycleStateAfterCompaction",
            "AsyncCandidateLifecycleStateAfterLeaderWireAdmission",
            "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
            "AsyncCandidateLifecycleStateAfterAdmission",
            "AsyncCandidateLifecycleStateAfterTimeoutOwnership",
            "AsyncTimeoutRecoveryEpisodeStateAfterTransition",
        ),
        "exact timeout-vote pre-admission slot-transition state",
    )
    pre_admission_state = formal_bodies.get(
        "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition"
    )
    if pre_admission_state is not None:
        body, line = pre_admission_state
        normalized = " ".join(body.split())
        required_pre_admission_fragments = (
            "ordinaryCarrierState == AsyncOrdinaryIngressCarrierStateAfterTransition( candidateServiceState)",
            "carrierState == AsyncCandidateLifecycleStateAfterCarrierUpdate( ordinaryCarrierState)",
            "timeoutState == AsyncCandidateLifecycleStateAfterTimeoutOwnership( serveIngressState, lifecycleState)",
            "IN AsyncTimeoutRecoveryEpisodeStateAfterTransition( leaderWireState, serveIngressState, timeoutState)",
        )
        missing = [
            fragment
            for fragment in required_pre_admission_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: the named timeout-recovery "
                "pre-admission state must follow the actual serialized slot "
                "pipeline including the ordinary carrier; missing "
                f"{missing!r}"
            )

    def require_theorem_statement(
        symbol: str,
        expected: str,
        description: str,
    ) -> None:
        extracted = theorem_bodies.get(symbol)
        if extracted is None:
            return
        body, line = extracted
        observed = _tla_statement_without_proof(body)
        expected_normalized = " ".join(expected.split())
        if observed != expected_normalized:
            errors.append(
                f"{formal_path}:{line}: {description} must state exactly "
                f"{expected_normalized!r}; found {observed!r}"
            )

    require_theorem_statement(
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission",
        r"""
\A node \in ValidatorIds:
  LET item == AsyncSelectedFairIngressItem(node)
  IN AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) = {"NonCandidate"}
       => /\ ~AsyncTimeoutRecoveryVoteCandidateDefined(node, item)
          /\ ~AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)
""",
        "NonCandidate exact zero-admission theorem",
    )
    non_candidate = theorem_bodies.get(
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission"
    )
    if non_candidate is not None:
        body, line = non_candidate
        statement = _tla_statement_without_proof(body)
        normalized = " ".join(statement.split())
        required_zero_admission_fragments = (
            "LET item == AsyncSelectedFairIngressItem(node)",
            'AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) = {"NonCandidate"}',
            "~AsyncTimeoutRecoveryVoteCandidateDefined(node, item)",
            "~AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)",
        )
        missing = [
            fragment
            for fragment in required_zero_admission_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: NonCandidate exact zero-admission "
                f"theorem must retain its 0→0 projection; missing {missing!r}"
            )
        forbidden_non_candidate_claims = [
            fragment
            for fragment in (
                '"FirstAdmission"',
                '"CoalescedRetry"',
                "admittedTimeoutVoteOwners = @ \\cup",
                "CandidateServiceRank' < CandidateServiceRank",
                "ProtocolProgress",
            )
            if fragment in statement
        ]
        if forbidden_non_candidate_claims:
            errors.append(
                f"{formal_path}:{line}: NonCandidate exact zero-admission "
                "theorem must not admit an owner or claim protocol progress; "
                f"found {forbidden_non_candidate_claims!r}"
            )
    require_theorem_statement(
        "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite",
        r"""
\A state, episode:
  AsyncTimeoutRecoveryEpisodeValidIn(state, episode)
    => /\ IsFiniteSet(
             AsyncTimeoutRecoveryRemainingProducerSlots(episode))
       /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(episode) \in Nat
       /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)
            <= Cardinality(episode.timeoutVoteOwnerUniverse)
""",
        "finite timeout-vote producer-episode theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot",
        r"""
\A state, episode, candidate, after:
  /\ AsyncTimeoutRecoveryEpisodeValidIn(state, episode)
  /\ candidate.slot \in
       AsyncTimeoutRecoveryRemainingProducerSlots(episode)
  /\ after.timeoutVoteOwnerUniverse = episode.timeoutVoteOwnerUniverse
  /\ after.admittedTimeoutVoteOwners =
       episode.admittedTimeoutVoteOwners \cup {candidate}
    => /\ AsyncTimeoutRecoveryRemainingProducerSlots(after) =
             AsyncTimeoutRecoveryRemainingProducerSlots(episode)
               \ {candidate.slot}
       /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(after) + 1 =
            AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)
""",
        "fresh owner exact remaining-slot removal theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot",
        r"""
\A episode:
  LET matchingNodes ==
        {node \in AsyncTimeoutRecoveryVoteAdmissionNodesThisStep:
           LET item == AsyncSelectedFairIngressItem(node)
               candidate ==
                 AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
           IN candidate.slot.episode = episode.key}
  IN /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
           asyncControlServiceState)
     /\ episode \in AsyncTimeoutRecoveryEpisodes
     /\ matchingNodes # {}
     => LET node == CHOOSE candidateNode \in matchingNodes: TRUE
            item == AsyncSelectedFairIngressItem(node)
            candidate ==
              AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
            after ==
              AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                asyncControlServiceState, episode)
        IN /\ AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)
           /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) =
                {"FirstAdmission"}
           => /\ after.timeoutVoteOwnerUniverse =
                    episode.timeoutVoteOwnerUniverse
              /\ after.admittedTimeoutVoteOwners =
                   episode.admittedTimeoutVoteOwners \cup {candidate}
              /\ candidate.slot \in
                   AsyncTimeoutRecoveryRemainingProducerSlots(episode)
              /\ AsyncTimeoutRecoveryRemainingProducerSlots(after) =
                   AsyncTimeoutRecoveryRemainingProducerSlots(episode)
                     \ {candidate.slot}
              /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(after) + 1
                   = AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)
""",
        "FirstAdmission exact one-slot consumption theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode",
        r"""
\A episode:
  LET matchingNodes ==
        {node \in AsyncTimeoutRecoveryVoteAdmissionNodesThisStep:
           LET item == AsyncSelectedFairIngressItem(node)
               candidate ==
                 AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
           IN candidate.slot.episode = episode.key}
  IN /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
           asyncControlServiceState)
     /\ episode \in AsyncTimeoutRecoveryEpisodes
     /\ matchingNodes # {}
     => LET node == CHOOSE candidateNode \in matchingNodes: TRUE
            item == AsyncSelectedFairIngressItem(node)
            candidate ==
              AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
            after ==
              AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                asyncControlServiceState, episode)
        IN /\ AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)
           /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) =
                {"CoalescedRetry"}
           => /\ after = episode
              /\ after.admittedTimeoutVoteOwners =
                   episode.admittedTimeoutVoteOwners
              /\ candidate.slot \notin
                   AsyncTimeoutRecoveryRemainingProducerSlots(episode)
              /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(after)
                   = AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)
""",
        "CoalescedRetry exact producer-episode stutter theorem",
    )
    coalesced_retry = theorem_bodies.get(
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode"
    )
    if coalesced_retry is not None:
        body, line = coalesced_retry
        statement = _tla_statement_without_proof(body)
        normalized = " ".join(statement.split())
        required_stutter_fragments = (
            'AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) = {"CoalescedRetry"}',
            "after = episode",
            "after.admittedTimeoutVoteOwners = episode.admittedTimeoutVoteOwners",
            "candidate.slot \\notin AsyncTimeoutRecoveryRemainingProducerSlots(episode)",
            "AsyncTimeoutRecoveryProducerEpisodeMeasure(after) = AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)",
        )
        missing = [
            fragment
            for fragment in required_stutter_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: CoalescedRetry must remain count-neutral "
                "with its slot already admitted; missing "
                f"{missing!r}"
            )
        forbidden_fresh_residual = []
        for fragment in (
            '"FreshReplenishment"',
            '"FirstAdmission"',
            "NonDescentEpisodeResidual",
        ):
            if fragment in statement:
                forbidden_fresh_residual.append(fragment)
        if forbidden_fresh_residual:
            errors.append(
                f"{formal_path}:{line}: CoalescedRetry is a count-neutral "
                "stutter and must not open a fresh non-descent residual; found "
                f"{forbidden_fresh_residual!r}"
            )
    require_theorem_statement(
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot",
        r"""
\A episode:
  LET matchingNodes ==
        {node \in AsyncTimeoutRecoveryVoteAdmissionNodesThisStep:
           LET item == AsyncSelectedFairIngressItem(node)
               candidate ==
                 AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
           IN candidate.slot.episode = episode.key}
  IN /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
           asyncControlServiceState)
     /\ episode \in AsyncTimeoutRecoveryEpisodes
     /\ matchingNodes # {}
     => LET node == CHOOSE candidateNode \in matchingNodes: TRUE
            item == AsyncSelectedFairIngressItem(node)
            candidate ==
              AsyncTimeoutRecoveryVoteCandidateOwner(node, item)
            after ==
              AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                asyncControlServiceState, episode)
        IN /\ AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(node)
           /\ candidate.disposition = "FreshReplenishment"
           /\ AsyncTimeoutRecoveryVoteAdmissionPlan(node, item) =
                {"FirstAdmission"}
           => /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(episode) \in Nat
              /\ candidate.slot \in
                   AsyncTimeoutRecoveryRemainingProducerSlots(episode)
              /\ after.timeoutVoteOwnerUniverse =
                   episode.timeoutVoteOwnerUniverse
              /\ after.admittedTimeoutVoteOwners =
                   episode.admittedTimeoutVoteOwners \cup {candidate}
              /\ AsyncTimeoutRecoveryRemainingProducerSlots(after) =
                   AsyncTimeoutRecoveryRemainingProducerSlots(episode)
                     \ {candidate.slot}
              /\ AsyncTimeoutRecoveryProducerEpisodeMeasure(after) + 1 =
                   AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)
""",
        "FirstAdmission-only fresh replenishment finite producer-slot descent",
    )
    fresh_replenishment = theorem_bodies.get(
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot"
    )
    if fresh_replenishment is not None:
        body, line = fresh_replenishment
        statement = _tla_statement_without_proof(body)
        if '"CoalescedRetry"' in statement or "after = episode" in " ".join(
            statement.split()
        ):
            errors.append(
                f"{formal_path}:{line}: fresh count-increasing replenishment "
                "must be FirstAdmission-only; CoalescedRetry remains a separate "
                "count-neutral stutter"
            )
    require_theorem_statement(
        "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState",
        r"""
\A state, episode:
  episode \in state.timeoutRecoveryEpisodes
    => AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(state, episode)
         \in
       (AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission(state)).timeoutRecoveryEpisodes
""",
        "updated timeout episode retention theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent",
        r"""
\A left, right, episode:
  AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(left, episode)
    = AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(right, episode)
""",
        "timeout-vote episode transformer state-independence theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode",
        r"""
\A preClockState, state, episode:
  /\ episode \in state.timeoutRecoveryEpisodes
  /\ ~AsyncTimeoutRecoveryEpisodeRetiresThisStep(episode)
  /\ ~AsyncTimeoutRecoveryExistingCaptureClearsThisStep(
       preClockState, episode.node)
  => episode \in
       AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
         preClockState, state)
""",
        "framed timeout-recovery episode retention theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState",
        r"""
\A node \in ValidatorIds:
  LET item == AsyncSelectedFairIngressItem(node)
  IN /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     => UNCHANGED vars
""",
        "selected TimeoutVote fair-ingress core-state frame",
    )
    require_theorem_statement(
        "AsyncPostGstHasNoControlServiceReset",
        r"""
gst => AsyncControlServiceResetNodesThisStep = {}
""",
        "post-GST control-service reset exclusion theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode",
        r"""
\A node \in ValidatorIds:
  \A episode:
    /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
    /\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
         asyncControlServiceState, node)
    => /\ episode.node = node
       /\ context = episode.key.context
       /\ context.height = episode.timeoutOwnerOrigin.height
       /\ nodeView[episode.node] = episode.key.view
       /\ generation[episode.node] = episode.generation
       /\ ~AsyncNodeHasDecisionIn(episode.node, context, decisions)
""",
        "current timeout-recovery episode boundary projection",
    )
    require_theorem_statement(
        "AsyncUnchangedCoreStatePreservesTimeoutBoundary",
        r"""
\A episode:
  /\ UNCHANGED vars
  /\ context = episode.key.context
  /\ context.height = episode.timeoutOwnerOrigin.height
  /\ nodeView[episode.node] = episode.key.view
  /\ generation[episode.node] = episode.generation
  /\ ~AsyncNodeHasDecisionIn(episode.node, context, decisions)
  => /\ context' = episode.key.context
     /\ context'.height = episode.timeoutOwnerOrigin.height
     /\ nodeView'[episode.node] = episode.key.view
     /\ generation'[episode.node] = episode.generation
     /\ ~AsyncNodeHasDecisionIn(episode.node, context', decisions')
""",
        "unchanged-core timeout boundary preservation theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary",
        r"""
\A node \in ValidatorIds:
  \A episode:
    LET item == AsyncSelectedFairIngressItem(node)
    IN /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
       /\ DrainFairIngressSelected(node)
       /\ item.kind = "TimeoutVote"
       /\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
            asyncControlServiceState, node)
       => /\ episode.node = node
          /\ context' = episode.key.context
          /\ context'.height = episode.timeoutOwnerOrigin.height
          /\ nodeView'[episode.node] = episode.key.view
          /\ generation'[episode.node] = episode.generation
          /\ ~AsyncNodeHasDecisionIn(
               episode.node, context', decisions')
""",
        "selected TimeoutVote current-episode boundary retention theorem",
    )
    require_theorem_statement(
        "AsyncUnchangedCoreStateExcludesPersistInstall",
        r"""
UNCHANGED vars => AsyncPersistInstallCommandsThisStep = {}
""",
        "unchanged-core persist-install exclusion theorem",
    )
    require_theorem_statement(
        "AsyncFairIngressDrainPreservesRetransmitTimerState",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncFairIngressDrainPreservesRetransmitTimerState",
            )
        ],
        "fair-ingress retransmit timer-state frame",
    )
    require_theorem_statement(
        "AsyncFairIngressDrainExcludesDirectRetransmit",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncFairIngressDrainExcludesDirectRetransmit",
            )
        ],
        "fair-ingress direct-retransmit exclusion theorem",
    )
    require_theorem_statement(
        "AsyncTypedOutstandingTagRemovalChangesFunction",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncTypedOutstandingTagRemovalChangesFunction",
            )
        ],
        "typed outstanding-tag removal strict-change theorem",
    )
    require_theorem_statement(
        "AsyncDeferredRetransmitRemovesOutstandingTag",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncDeferredRetransmitRemovesOutstandingTag",
            )
        ],
        "deferred retransmit exact outstanding-tag removal theorem",
    )
    require_theorem_statement(
        "AsyncFairIngressDrainExcludesDeferredRetransmit",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncFairIngressDrainExcludesDeferredRetransmit",
            )
        ],
        "fair-ingress deferred-retransmit exclusion theorem",
    )
    require_theorem_statement(
        "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle",
            )
        ],
        "ingress-drain retransmit-lifecycle noncompletion theorem",
    )
    require_theorem_statement(
        "AsyncIngressDrainFramesDeferredAndCausalQueues",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncIngressDrainFramesDeferredAndCausalQueues",
            )
        ],
        "ingress-drain deferred and causal queue frame theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncTimeoutVoteFairIngressFramesCommandAndWork",
            )
        ],
        "TimeoutVote fair-ingress command and work frame theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers",
        r"""
\A node \in ValidatorIds:
  LET item == AsyncSelectedFairIngressItem(node)
      candidate == DeliveryCandidate(item)
  IN /\ IngressDrainStep(node)
     /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     => /\ asyncDeferredCompletionQueues' =
              asyncDeferredCompletionQueues
        /\ asyncDeferredProgressQueues' = asyncDeferredProgressQueues
        /\ asyncDeferredNormalQueues' = asyncDeferredNormalQueues
        /\ asyncCausalQueues' = asyncCausalQueues
        /\ asyncOutstandingWork' = asyncOutstandingWork
        /\ \/ asyncCommandQueues' = asyncCommandQueues
           \/ asyncCommandQueues' =
                [asyncCommandQueues EXCEPT
                   ![candidate.node] = Append(@, candidate)]
""",
        "TimeoutVote ingress scheduler-carrier frame theorem",
    )
    require_theorem_statement(
        "AsyncSequenceSetAfterAppendAddsOnlyValue",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncSequenceSetAfterAppendAddsOnlyValue",
            )
        ],
        "sequence-set append adds only the appended value theorem",
    )
    require_theorem_statement(
        "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue",
            )
        ],
        "mapped sequence-set append adds only the appended value theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin",
        r"""
\A node \in ValidatorIds:
  LET item == AsyncSelectedFairIngressItem(node)
  IN /\ DOMAIN asyncCommandQueues = ValidatorIds
     /\ (\A owner \in ValidatorIds:
           AsyncQueueTyped(asyncCommandQueues[owner]))
     /\ IngressDrainStep(node)
     /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     => AsyncScheduledCandidateOriginsForNodeAfter(node)
          \subseteq
            AsyncScheduledCandidateOriginsForNode(node)
              \cup {DeliveryCandidate(item).causalOrigin}
""",
        "TimeoutVote ingress exact scheduled-origin extension theorem",
    )
    require_theorem_statement(
        "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase",
            )
        ],
        "proposed timeout causal-origin phase theorem",
    )
    require_theorem_statement(
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase",
            )
        ],
        "owned timeout lifecycle-origin phase theorem",
    )
    require_theorem_statement(
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin",
            )
        ],
        "current timeout causal-origin selection theorem",
    )
    require_theorem_statement(
        "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase",
        r"""
\A node \in ValidatorIds:
  /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
       asyncControlServiceState)
  /\ AsyncTimeoutRecoveryEpisodeOwnedIn(
       asyncControlServiceState, node)
  => AsyncCurrentTimeoutCausalOrigin(node).phase = "BeginTimeout"
""",
        "owned timeout-recovery current-origin phase theorem",
    )
    require_theorem_statement(
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind",
        EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (
                "SumeragiV2AsyncNetwork",
                "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind",
            )
        ],
        "delivery candidate causal-origin phase theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase",
        r"""
\A item:
  item.kind = "TimeoutVote"
    => DeliveryCandidate(item).causalOrigin.phase = "DeliverTimeout"
""",
        "TimeoutVote delivery-origin phase theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle",
        r"""
\A node \in ValidatorIds:
  LET item == AsyncSelectedFairIngressItem(node)
  IN /\ DOMAIN asyncCommandQueues = ValidatorIds
     /\ (\A owner \in ValidatorIds:
           AsyncQueueTyped(asyncCommandQueues[owner]))
     /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
          asyncControlServiceState)
     /\ AsyncTimeoutRecoveryEpisodeOwnedIn(
          asyncControlServiceState, node)
     /\ IngressDrainStep(node)
     /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     => ~AsyncTimeoutLifecycleTransfersThisStep(node)
""",
        "TimeoutVote ingress timeout-lifecycle nontransfer theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame",
        r"""
\A node \in ValidatorIds:
  \A episode:
    LET item == AsyncSelectedFairIngressItem(node)
    IN /\ DOMAIN asyncCommandQueues = ValidatorIds
       /\ (\A owner \in ValidatorIds:
             AsyncQueueTyped(asyncCommandQueues[owner]))
       /\ gst
       /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
       /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
            asyncControlServiceState)
       /\ AsyncRetransmitPeriod \in Nat \ {0}
       /\ asyncNow \in Nat
       /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
       /\ asyncOutstandingTags \in
            [ValidatorIds -> SUBSET AsyncCompletionTags]
       /\ IngressDrainStep(node)
       /\ DrainFairIngressSelected(node)
       /\ item.kind = "TimeoutVote"
       /\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
            asyncControlServiceState, node)
       => /\ episode.node = node
          /\ AsyncControlServiceResetNodesThisStep = {}
          /\ ~AsyncTimeoutRecoveryEpisodeRetiresThisStep(episode)
          /\ \A state:
               ~AsyncTimeoutRecoveryExistingCaptureClearsThisStep(
                  state, node)
""",
        "TimeoutVote ingress complete timeout-recovery frame theorem",
    )
    require_theorem_statement(
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode",
        r"""
\A node \in ValidatorIds:
  \A episode:
  LET item == AsyncSelectedFairIngressItem(node)
      timeoutRecoveryState ==
        AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition
  IN /\ DOMAIN asyncCommandQueues = ValidatorIds
     /\ (\A owner \in ValidatorIds:
           AsyncQueueTyped(asyncCommandQueues[owner]))
     /\ gst
     /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
     /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
          asyncControlServiceState)
     /\ AsyncRetransmitPeriod \in Nat \ {0}
     /\ asyncNow \in Nat
     /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
     /\ asyncOutstandingTags \in
          [ValidatorIds -> SUBSET AsyncCompletionTags]
     /\ AsyncNext
     /\ IngressDrainStep(node)
     /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     /\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
          asyncControlServiceState, node)
     => episode \in timeoutRecoveryState.timeoutRecoveryEpisodes
""",
        "selected TimeoutVote fair-ingress pre-admission episode frame",
    )
    require_theorem_statement(
        "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState",
        r"""
AsyncControlServiceSlotTransition
  => asyncControlServiceState'.timeoutRecoveryEpisodes =
       (AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission(
          AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition))
         .timeoutRecoveryEpisodes
""",
        "atomic timeout-vote admission prime-state projection",
    )
    require_theorem_statement(
        "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition",
        r"""
\A node \in ValidatorIds:
  \A episode:
  LET item == AsyncSelectedFairIngressItem(node)
      timeoutRecoveryState ==
        AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition
  IN /\ DOMAIN asyncCommandQueues = ValidatorIds
     /\ (\A owner \in ValidatorIds:
           AsyncQueueTyped(asyncCommandQueues[owner]))
     /\ gst
     /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
     /\ AsyncTimeoutRecoveryEpisodeTypeInvariantIn(
          asyncControlServiceState)
     /\ AsyncRetransmitPeriod \in Nat \ {0}
     /\ asyncNow \in Nat
     /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
     /\ asyncOutstandingTags \in
          [ValidatorIds -> SUBSET AsyncCompletionTags]
     /\ AsyncNext
     /\ IngressDrainStep(node)
     /\ DrainFairIngressSelected(node)
     /\ item.kind = "TimeoutVote"
     /\ episode \in AsyncTimeoutRecoveryEpisodesForNodeIn(
          asyncControlServiceState, node)
     => AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
          asyncControlServiceState, episode)
          \in asyncControlServiceState'.timeoutRecoveryEpisodes
""",
        "selected TimeoutVote admission prime-state episode retention",
    )

    atomic_transition = theorem_bodies.get(
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation"
    )
    if atomic_transition is not None:
        body, line = atomic_transition
        statement = _tla_statement_without_proof(body)
        normalized = " ".join(statement.split())
        required_atomic_fragments = (
            "ordinaryCarrierState == AsyncOrdinaryIngressCarrierStateAfterTransition( candidateServiceState)",
            "carrierState == AsyncCandidateLifecycleStateAfterCarrierUpdate( ordinaryCarrierState)",
            "timeoutRecoveryState == AsyncTimeoutRecoveryEpisodeStateAfterTransition( leaderWireState, serveIngressState, timeoutState)",
            "timeoutVoteState == AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission( timeoutRecoveryState)",
            "/\\ timeoutRecoveryState.timeoutRecoveryEpisodes = AsyncTimeoutRecoveryRetainedEpisodesAfterTransition( leaderWireState, timeoutState) \\cup AsyncTimeoutRecoveryNewEpisodesAfterTransition( leaderWireState, serveIngressState)",
            "/\\ timeoutVoteState.timeoutRecoveryEpisodes = {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission( timeoutRecoveryState, episode): episode \\in timeoutRecoveryState.timeoutRecoveryEpisodes}",
            "/\\ timeoutState.timeoutRecoveryEpisodes = resetState.timeoutRecoveryEpisodes",
            "/\\ asyncControlServiceState'.timeoutRecoveryEpisodes = timeoutVoteState.timeoutRecoveryEpisodes",
        )
        missing = [
            fragment
            for fragment in required_atomic_fragments
            if fragment not in normalized
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: atomic lifecycle reservation must "
                "thread the ordinary carrier before compaction and frame the "
                "timeout episode through reset, admission, and the prime state; "
                f"missing {missing!r}"
            )

    theorem_proof_dependencies = {
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission": (
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
            "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
        ),
        "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite": (
            "FS_Image",
            "FS_Difference",
            "FS_Subset",
            "FS_CardinalityType",
            "AsyncTimeoutRecoveryProducerEpisodeMeasure",
            "AsyncTimeoutRecoveryRemainingProducerSlots",
            "AsyncTimeoutRecoveryAdmittedVoteSlots",
            "AsyncTimeoutRecoveryEpisodeValidIn",
            "AsyncTimeoutRecoveryVoteOwnerValidForEpisode",
        ),
        "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot": (
            "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite",
            "FS_RemoveElement",
            "FS_CardinalityType",
            "AsyncTimeoutRecoveryEpisodeValidIn",
            "AsyncTimeoutRecoveryProducerEpisodeMeasure",
            "AsyncTimeoutRecoveryRemainingProducerSlots",
            "AsyncTimeoutRecoveryAdmittedVoteSlots",
        ),
        "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot": (
            "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot",
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
            "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
            "AsyncTimeoutRecoveryFirstAdmissionCandidateSlotIsRemaining",
            "AsyncTimeoutRecoveryVoteCandidateOwner",
            "AsyncTimeoutRecoveryEpisodeTypeInvariantIn",
            "AsyncTimeoutRecoveryEpisodes",
            "AsyncTimeoutRecoveryEpisodesIn",
            "AsyncTimeoutRecoveryRemainingProducerSlots",
        ),
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode": (
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
            "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
            "AsyncTimeoutRecoveryProducerEpisodeMeasure",
            "AsyncTimeoutRecoveryRemainingProducerSlots",
            "AsyncTimeoutRecoveryCoalescedRetryCandidateSlotIsAdmitted",
        ),
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot": (
            "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite",
            "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot",
            "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
            "AsyncTimeoutRecoveryVoteAdmissionPlan",
        ),
        "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState": (
            "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission",
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent": (
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission",
        ),
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation": (
            "AsyncControlServiceSlotTransition",
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode": (
            "AsyncTimeoutRecoveryEpisodeAfterTransition",
            "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition",
        ),
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState": (
            "AsyncFairIngressCoreStateTransition",
            "DrainFairIngressSelected",
        ),
        "AsyncPostGstHasNoControlServiceReset": (
            "AsyncControlServiceResetNodesThisStep",
            "PreGstResponsiveRestart",
            "PreGstResponsiveReplay",
        ),
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode": (
            "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
            "AsyncTimeoutRecoveryEpisodesForNodeIn",
            "AsyncNodeHasDecisionIn",
        ),
        "AsyncUnchangedCoreStatePreservesTimeoutBoundary": (
            "AsyncNodeHasDecisionIn",
            "vars",
        ),
        "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary": (
            "AsyncTimeoutVoteFairIngressDrainLeavesCoreState",
            "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode",
            "AsyncUnchangedCoreStatePreservesTimeoutBoundary",
        ),
        "AsyncUnchangedCoreStateExcludesPersistInstall": (
            "AsyncPersistInstallCommandsThisStep",
            "AsyncPersistInstallCommandThisStep",
            "PersistInstallTC",
            "vars",
        ),
        "AsyncFairIngressDrainPreservesRetransmitTimerState": (
            "AsyncFairIngressTimerStateFrame",
            "DrainFairIngressSelected",
        ),
        "AsyncFairIngressDrainExcludesDirectRetransmit": (
            "AsyncFairIngressDrainPreservesRetransmitTimerState",
            "DirectRetransmitStep",
            "RetransmitDue",
        ),
        "AsyncTypedOutstandingTagRemovalChangesFunction": (),
        "AsyncDeferredRetransmitRemovesOutstandingTag": (
            "DeferredRetransmitStep",
        ),
        "AsyncFairIngressDrainExcludesDeferredRetransmit": (
            "AsyncFairIngressDrainPreservesRetransmitTimerState",
            "AsyncTypedOutstandingTagRemovalChangesFunction",
            "AsyncDeferredRetransmitRemovesOutstandingTag",
        ),
        "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle": (
            "AsyncFairIngressDrainExcludesDirectRetransmit",
            "AsyncFairIngressDrainExcludesDeferredRetransmit",
            "AsyncRetransmitLifecycleEpisodeCompletesThisStep",
        ),
        "AsyncIngressDrainFramesDeferredAndCausalQueues": (
            "IngressDrainStep",
            "AsyncDeferredVars",
            "LeaveCausalQueues",
        ),
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork": (
            "AsyncSelectedFairIngressItem",
            "DeliveryCandidate",
            "DrainFairIngressSelected",
            "IngressItemHasAuthenticatedHistory",
            "AsyncControlServiceOccurrenceRetired",
            "CandidateAdmissionCoalesced",
            "AsyncIoExceptServeReservationsVars",
            "EnqueueCandidate",
        ),
        "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers": (
            "AsyncIngressDrainFramesDeferredAndCausalQueues",
            "AsyncTimeoutVoteFairIngressFramesCommandAndWork",
        ),
        "AsyncSequenceSetAfterAppendAddsOnlyValue": (
            "SequenceSet",
            "SeqMonotonic",
            "AppendProperties",
            "LenProperties",
        ),
        "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue": (
            "SequenceSet",
            "AsyncSequenceSetAfterAppendAddsOnlyValue",
        ),
        "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin": (
            "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers",
            "AsyncSequenceSetAfterAppendAddsOnlyValue",
            "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue",
            "AsyncScheduledCandidateOriginsForNode",
            "AsyncScheduledCandidateOriginsForNodeAfter",
            "AsyncScheduledCandidateOriginsForNodeIn",
            "CandidateScheduledIn",
            "AsyncQueueTyped",
            "SequenceSet",
        ),
        "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase": (
            "AsyncProposedTimeoutCausalOrigin",
            "AsyncProposedTimeoutCausalCommand",
            "NoItemCandidate",
            "AsyncCandidateWithIdentityAndOrigin",
            "AsyncNoItemCandidateCausalOriginAt",
            "AsyncCandidateCausalOrigin",
        ),
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase": (
            "AsyncTimeoutRecoveryEpisodeTypeInvariantIn",
            "AsyncTimeoutLifecycleOwned",
            "AsyncTimeoutLifecycleOrdinal",
            "AsyncTimeoutRecoveryEpisodeOwnedIn",
            "AsyncTimeoutRecoveryEpisodeForNodeIn",
            "AsyncTimeoutRecoveryEpisodesForNodeIn",
            "AsyncTimeoutRecoveryEpisodeValidIn",
            "AsyncTimeoutLifecycleOrigin",
        ),
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin": (
            "AsyncCurrentTimeoutCausalOrigin",
            "AsyncProposedTimeoutCausalCommand",
            "NoItemCandidate",
            "AsyncCandidateWithIdentityAndOrigin",
            "TimeoutCausalCommand",
        ),
        "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase": (
            "AsyncTimeoutRecoveryEpisodeTypeInvariantIn",
            "AsyncTimeoutRecoveryEpisodeOwnedIn",
            "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin",
            "AsyncEffectiveTimeoutLifecycleOrigin",
            "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase",
            "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase",
        ),
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind": (
            "DeliveryCandidate",
            "AsyncCandidateWithIdentityAndOrigin",
            "AsyncDeliveryCandidateCausalOriginAt",
            "AsyncCandidateCausalOrigin",
            "DeliveryKind",
        ),
        "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase": (
            "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind",
            "DeliveryKind",
        ),
        "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle": (
            "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin",
            "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase",
            "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase",
            "AsyncTimeoutLifecycleTransfersThisStep",
            "AsyncQueueTyped",
        ),
        "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame": (
            "AsyncPostGstHasNoControlServiceReset",
            "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary",
            "AsyncTimeoutVoteFairIngressDrainLeavesCoreState",
            "AsyncUnchangedCoreStateExcludesPersistInstall",
            "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle",
            "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle",
            "AsyncTimeoutRecoveryEpisodeRetiresThisStep",
            "AsyncTimeoutRecoveryExistingCaptureClearsThisStep",
            "AsyncQueueTyped",
        ),
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode": (
            "AsyncNext",
            "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame",
            "AsyncTimeoutRecoveryResetRetiresExactlyResetNodes",
            "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
            "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode",
            "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
            "AsyncTimeoutRecoveryEpisodesForNodeIn",
            "DrainFairIngressSelected",
            "AsyncQueueTyped",
        ),
        "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState": (
            "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
            "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
        ),
        "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition": (
            "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode",
            "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState",
            "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState",
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent",
            "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
            "AsyncQueueTyped",
        ),
    }
    for symbol, dependencies in theorem_proof_dependencies.items():
        extracted = theorem_bodies.get(symbol)
        if extracted is None:
            continue
        body, line = extracted
        parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
        proof = parts[1] if len(parts) == 2 else ""
        missing = [
            dependency
            for dependency in dependencies
            if not _tla_dependency_present(proof, dependency)
        ]
        if missing:
            errors.append(
                f"{formal_path}:{line}: timeout-vote producer-episode theorem "
                f"{symbol} must retain exact proof dependencies {missing!r}"
            )

    theorem_required_proof_fragments = {
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork": (
            "CASE /\\ ~AsyncControlServiceOccurrenceRetired(Item) "
            "/\\ CandidateAdmissionCoalesced(Candidate)",
            "CASE /\\ ~AsyncControlServiceOccurrenceRetired(Item) "
            "/\\ ~CandidateAdmissionCoalesced(Candidate)",
        ),
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase": (
            "DEFINE Episode == AsyncTimeoutRecoveryEpisodeForNodeIn(",
            "DEF Episode, AsyncTimeoutRecoveryEpisodeForNodeIn, "
            "AsyncTimeoutRecoveryEpisodeOwnedIn",
        ),
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin": (
            "QED BY <2>2 DEF AsyncCurrentTimeoutCausalOrigin",
        ),
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind": (
            "DEF AsyncDeliveryCandidateCausalOriginAt, "
            "AsyncCandidateCausalOrigin",
        ),
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode": (
            "RecoveryState = "
            "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
            "RecoveryState, "
            "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition",
        ),
    }
    for symbol, fragments in theorem_required_proof_fragments.items():
        extracted = theorem_bodies.get(symbol)
        if extracted is None:
            continue
        body, line = extracted
        parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
        proof = parts[1] if len(parts) == 2 else ""
        normalized_proof = " ".join(proof.split())
        missing_fragments = [
            fragment for fragment in fragments if fragment not in normalized_proof
        ]
        if missing_fragments:
            errors.append(
                f"{formal_path}:{line}: timeout-vote producer-episode theorem "
                f"{symbol} must retain exact proof dependencies; missing "
                f"reviewed proof fragments {missing_fragments!r}"
            )

    coalesced_retry = theorem_bodies.get(
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode"
    )
    if coalesced_retry is not None:
        body, line = coalesced_retry
        parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
        proof = parts[1] if len(parts) == 2 else ""
        normalized_proof = " ".join(proof.split())
        direct_transformer_definition = (
            "DEF After, Node, Item, Candidate, MatchingNodes, "
            "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission"
        )
        if direct_transformer_definition not in normalized_proof:
            errors.append(
                f"{formal_path}:{line}: timeout-vote producer-episode theorem "
                "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode "
                "must retain exact proof dependencies "
                "['AsyncTimeoutRecoveryEpisodeAfterVoteAdmission'] in its "
                "direct DEF list"
            )

    bridge_name = (
        "AdequateLeaderFreshTimeoutVoteReplenishmentConsumesProducerSlot"
        "AndOpensNonDescentEpisode"
    )
    bridge_path = formal_dir / "SumeragiV2AdequateLeaderServiceClosureProofs.tla"
    bridge_seals = _TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256.get(
        bridge_path.name, {}
    )
    expected_bridge_symbols = {bridge_name}
    observed_bridge_symbols = set(bridge_seals)
    if observed_bridge_symbols != expected_bridge_symbols:
        errors.append(
            "adequate-leader timeout-replenishment bridge source-seal inventory "
            "must be exact; "
            f"missing={sorted(expected_bridge_symbols - observed_bridge_symbols)}, "
            f"extra={sorted(observed_bridge_symbols - expected_bridge_symbols)}"
        )
    bridge_source = ""
    if not bridge_path.is_file() or bridge_path.is_symlink():
        errors.append(
            f"{bridge_path}: adequate-leader timeout-replenishment bridge "
            "must be a regular formal source"
        )
    else:
        try:
            bridge_source = bridge_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{bridge_path}: cannot read adequate-leader timeout bridge: {error}"
            )
    bridge_body: tuple[str, int] | None = None
    if bridge_source:
        bridge_body = _top_level_theorem_body(
            bridge_source,
            bridge_name,
            preserve_string_contents=True,
        )
        if bridge_body is None:
            errors.append(
                f"{bridge_path}: missing source-sealed adequate-leader timeout "
                f"bridge {bridge_name}"
            )
        else:
            body, line = bridge_body
            observed_sha256 = hashlib.sha256(
                " ".join(body.split()).encode("utf-8")
            ).hexdigest()
            expected_sha256 = bridge_seals.get(bridge_name)
            if observed_sha256 != expected_sha256:
                errors.append(
                    f"{bridge_path}:{line}: adequate-leader timeout bridge "
                    f"{bridge_name} must match reviewed digest {expected_sha256}; "
                    f"found {observed_sha256}"
                )

            statement = _tla_statement_without_proof(body)
            normalized = " ".join(statement.split())
            required_bridge_fragments = (
                "item == AsyncSelectedFairIngressItem(target)",
                "candidate == AsyncTimeoutRecoveryVoteCandidateOwner(target, item)",
                "after == AsyncTimeoutRecoveryEpisodeAfterVoteAdmission( asyncControlServiceState, episode)",
                "introducedOwner == AdequateLeaderFrozenCandidateOwnerIdentity( DeliveryCandidate(item), sourceOccurrenceRank[1], target, leaderContext, leader, leaderView, subject)",
                "episode \\in AsyncTimeoutRecoveryEpisodesForNodeIn( asyncControlServiceState, target)",
                "DeliverySubject(item) = subject",
                "episode.key.target = target",
                "episode.key.context = leaderContext",
                "episode.key.leader = leader",
                "episode.key.view = leaderView",
                "episode.key.subject = NoSubject",
                'episode.key.phase = "TimeoutVote"',
                "candidate.slot.episode = episode.key",
                "candidate.slot \\in episode.timeoutVoteOwnerUniverse",
                'candidate.disposition = "FreshReplenishment"',
                "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep(target)",
                "AdequateLeaderTargetOccurrenceRankFrontier( target, leaderContext, leader, leaderView, subject, sourceOccurrenceRank)",
                "AdequateLeaderTargetEpisodeStartsWithCurrentOwners( target, leaderContext, leader, leaderView, subject, known)",
                "AdequateLeaderTargetCountIncreasingReplenishmentAction( target, leaderContext, leader, leaderView, subject, sourceOccurrenceRank[1])",
                "AdequateLeaderFrozenTargetCandidateIdentity( DeliveryCandidate(item), sourceOccurrenceRank[1], target, leaderContext, leader, leaderView, subject)",
                "/\\ introducedOwner \\in AdequateLeaderTargetRankIntroducedOwnerIdentitySet( target, leaderContext, leader, leaderView, subject, sourceOccurrenceRank[1])",
                "AsyncTimeoutRecoveryProducerEpisodeMeasure(episode) \\in Nat",
                "after \\in AsyncTimeoutRecoveryEpisodes'",
                'AsyncTimeoutRecoveryVoteAdmissionPlan(target, item) = {"FirstAdmission"}',
                "candidate.slot \\in AsyncTimeoutRecoveryRemainingProducerSlots(episode)",
                "AsyncTimeoutRecoveryProducerEpisodeMeasure(after) + 1 = AsyncTimeoutRecoveryProducerEpisodeMeasure(episode)",
                "/\\ introducedOwner \\in AdequateLeaderFrozenOwnerUniverse( target, leaderContext, leader, leaderView, subject)",
                "AdequateLeaderTargetNonDescentEpisodeResidual( target, leaderContext, leader, leaderView, subject, sourceOccurrenceRank, known)'",
            )
            missing = [
                fragment
                for fragment in required_bridge_fragments
                if fragment not in normalized
            ]
            if missing:
                errors.append(
                    f"{bridge_path}:{line}: adequate-leader timeout bridge must "
                    "bind the normalized episode key, external DeliverySubject, "
                    "exact frozen candidate slot, FirstAdmission-only fresh "
                    "replenishment, finite retained producer episode, and "
                    "external-subject "
                    f"non-descent residual; missing {missing!r}"
                )
            forbidden_subject_bridges = (
                "episode.key.subject = subject",
                "subject = episode.key.subject",
            )
            forbidden = [
                binding
                for binding in forbidden_subject_bridges
                if binding in normalized
            ]
            if forbidden:
                errors.append(
                    f"{bridge_path}:{line}: timeout episode key subject must be "
                    "NoSubject while the adequate-leader corridor retains its "
                    f"external block subject; found {forbidden!r}"
                )
            forbidden_coalesced_fragments = (
                '"CoalescedRetry"',
                "after = episode",
            )
            forbidden_coalesced = [
                fragment
                for fragment in forbidden_coalesced_fragments
                if fragment in normalized
            ]
            if forbidden_coalesced:
                errors.append(
                    f"{bridge_path}:{line}: count-neutral CoalescedRetry must "
                    "remain a separate stutter theorem and may not open the "
                    "fresh adequate-leader non-descent residual; found "
                    f"{forbidden_coalesced!r}"
                )
            parts = THEOREM_PROOF_MARKER_RE.split(body, maxsplit=1)
            proof = parts[1] if len(parts) == 2 else ""
            required_bridge_proof_dependencies = (
                "AdequateLeaderTargetNonDescentActionExposesFreshEpisodeIdentity",
                "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot",
                "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition",
                "AsyncStrongTypeInvariant",
                "AsyncSchedulerTypeInvariant",
                "AsyncRuntimeTypeInvariant",
                "AsyncRuntimeScalarTypeInvariant",
                "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep",
                "AdequateLeaderTargetCountIncreasingReplenishmentAction",
                "AdequateLeaderFrozenTargetCorridor",
            )
            missing_dependencies = [
                dependency
                for dependency in required_bridge_proof_dependencies
                if not _tla_dependency_present(proof, dependency)
            ]
            if missing_dependencies:
                errors.append(
                    f"{bridge_path}:{line}: adequate-leader timeout bridge must "
                    "derive its separate non-descent episode from the exact finite "
                    f"producer lemmas; missing {missing_dependencies!r}"
                )

    direct_bridge_consumer = (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
        "AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepFollowsProviders",
    )
    transitive_bridge_consumers = {
        bridge_path.name: (
            "AsyncSpecProvidesAdequateLeaderWirePhysicalFrozenCertificateConvergence",
        ),
        direct_bridge_consumer[0]: (
            "AdequateLeaderFixedSelectedActionsCarryPipelineRank",
            "AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders",
            "AdequateLeaderFixedGlobalBlockerFactsSupplyProviders",
        ),
    }
    reviewed_bridge_consumers = dict(transitive_bridge_consumers)
    reviewed_bridge_consumers.setdefault(direct_bridge_consumer[0], ())
    reviewed_bridge_consumers[direct_bridge_consumer[0]] += (
        direct_bridge_consumer[1],
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStep",
    )
    consumer_bodies: dict[tuple[str, str], tuple[str, int]] = {}
    for filename, consumer_names in reviewed_bridge_consumers.items():
        consumer_path = formal_dir / filename
        if not consumer_path.is_file() or consumer_path.is_symlink():
            errors.append(
                f"{consumer_path}: downstream adequate-leader timeout bridge "
                "consumer module must be a regular file"
            )
            continue
        try:
            consumer_source = consumer_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{consumer_path}: cannot read timeout bridge consumers: {error}"
            )
            continue
        for consumer_name in consumer_names:
            extracted = _top_level_theorem_body(
                consumer_source,
                consumer_name,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{consumer_path}: missing downstream adequate-leader theorem "
                    f"{consumer_name}"
                )
                continue
            consumer_body, consumer_line = extracted
            consumer_bodies[(filename, consumer_name)] = (
                consumer_body,
                consumer_line,
            )

    for (filename, consumer_name), (consumer_body, consumer_line) in (
        consumer_bodies.items()
    ):
        consumer_path = formal_dir / filename
        proof_marker = THEOREM_PROOF_MARKER_RE.search(consumer_body)
        direct_by_region = ""
        if proof_marker is not None and proof_marker.group(0).strip() == "BY":
            proof_tail = consumer_body[proof_marker.end() :]
            direct_by_region = re.split(
                r"(?m)^[ \t]*DEF\b", proof_tail, maxsplit=1
            )[0]
        direct_citation_count = tla_code_tokens(direct_by_region).count(
            bridge_name
        )
        is_direct_consumer = (filename, consumer_name) == direct_bridge_consumer
        expected_count = 1 if is_direct_consumer else 0
        if direct_citation_count != expected_count:
            policy = (
                "must cite"
                if is_direct_consumer
                else "must rely transitively and may not cite"
            )
            errors.append(
                f"{consumer_path}:{consumer_line}: adequate-leader provider "
                f"{consumer_name} {policy} the exact finite timeout "
                f"replenishment bridge {bridge_name} directly in its top-level "
                f"BY dependency list; expected {expected_count}, found "
                f"{direct_citation_count}"
            )

    live_consumer_key = (
        direct_bridge_consumer[0],
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStep",
    )
    live_consumer = consumer_bodies.get(live_consumer_key)
    if live_consumer is not None:
        body, line = live_consumer
        if not _tla_dependency_present(body, direct_bridge_consumer[1]):
            errors.append(
                f"{formal_dir / live_consumer_key[0]}:{line}: temporal "
                "adequate-leader service theorem must consume the exact "
                "origin-episode provider transitively"
            )

    for symbol, (body, line) in theorem_bodies.items():
        statement_tokens = tla_code_tokens(_tla_statement_without_proof(body))
        forbidden = sorted(
            {
                token
                for token in statement_tokens
                if token in {"Decision", "NodeHasDecision", "Progress"}
                or token.endswith("Rank")
                or "IngressRank" in token
                or "ServiceRank" in token
            }
        )
        if forbidden:
            errors.append(
                f"{formal_path}:{line}: timeout-vote replenishment is only a "
                "finite producer-episode case split and must not conclude "
                "protocol progress or main ingress/service-rank descent; "
                f"found {forbidden!r} in {symbol}"
            )
    return errors
